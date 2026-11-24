//! What a path-traced still does to the linear image once the accumulator is read (phase 22):
//! the denoiser the options ask for, and (p22-r3d-film, p22-r3d-linear-out) the film and the kept
//! linear image. With a neutral film and no kept image the result goes back into the accumulator
//! and the GPU resolve runs on it exactly as it did before phase 22.

use super::{LinearStage, PathOptions, PathStats};
use cadcraft_doc::arch::{DenoiseMode, Film};

fn linear_from_rgb(pixels: &[[f32; 4]], stage: LinearStage, exposure: f32, film: &Film, size: (u32, u32)) -> Vec<[f32; 3]> {
    let mut rgb: Vec<[f32; 3]> = pixels
        .iter()
        .map(|p| {
            let n = p[3].max(1.0);
            [p[0] / n, p[1] / n, p[2] / n]
        })
        .collect();
    for px in &mut rgb {
        for c in &mut px[0..3] {
            if !c.is_finite() {
                *c = 0.0;
            }
        }
    }
    if stage == LinearStage::Film {
        for px in &mut rgb {
            px[0] *= exposure;
            px[1] *= exposure;
            px[2] *= exposure;
            for c in &mut px[0..3] {
                if !c.is_finite() {
                    *c = 0.0;
                }
            }
        }
        crate::film::apply(film, &mut rgb, size);
        for px in &mut rgb {
            for c in &mut px[0..3] {
                if !c.is_finite() {
                    *c = 0.0;
                }
            }
        }
    }
    rgb
}

/// The CPU resolve a non-neutral film needs: mean radiance (as `finish_linear` left it) times the
/// final `exposure`, the film, the tone curve `tone` (an `exposure::TONE_*` code), sRGB encode,
/// opaque RGBA8. The GPU resolve stays the path for a neutral film.
pub(super) fn film_rgba(pixels: &[[f32; 4]], exposure: f32, tone: u32, film: &Film, size: (u32, u32)) -> Vec<u8> {
    let rgb = linear_from_rgb(pixels, LinearStage::Film, exposure, film, size);
    let mut out = Vec::with_capacity(rgb.len() * 4);
    for px in &rgb {
        for &c in px {
            out.push(srgb_u8(crate::exposure::tone_map(c, tone)));
        }
        out.push(255);
    }
    out
}

/// sRGB-encode `v` (clamped to 0..=1) to 0..=255, as the GPU resolve does.
fn srgb_u8(v: f32) -> u8 {
    let c = v.clamp(0.0, 1.0);
    let e = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (e * 255.0 + 0.5) as u8
}

/// The colours the resolve reads: the accumulator's radiance means, denoised as `opts` ask.
///
/// `accum` is the whole accumulator (`2 · w · h` entries: the sums, then the second half
/// `(luminance-squared sum, hits, clamp loss, adaptive state)`); `aov` reads the guides only when a
/// denoiser needs them. Sets `stats.denoised`, `denoiser`, `denoise_fallback` and `denoise_seconds`.
/// `Err` carries a message for a failed render. A learned request that cannot run falls back to
/// the filter and says why in `stats.denoise_fallback`.
pub(super) fn finish_linear(
    gpu: &super::device::ComputeGpu,
    opts: &PathOptions,
    film: &Film,
    size: (u32, u32),
    accum: &[[f32; 4]],
    aov: impl FnOnce() -> Result<Vec<[f32; 4]>, String>,
    exposure: f32,
    stats: &mut PathStats,
) -> Result<Vec<[f32; 4]>, String> {
    let pixels = (size.0 as usize) * (size.1 as usize);
    let sums = accum.get(..pixels).ok_or_else(|| "accumulator is smaller than the image".to_string())?;
    // Neutral film and nothing kept: the resolve is the one it always was. A film or a kept
    // linear image is applied after this point by later packages.
    let _plain = film.is_neutral() && opts.keep_linear.is_none();
    stats.denoiser = DenoiseMode::None;
    stats.denoised = false;
    stats.linear = None;
    if !opts.denoise || opts.denoise_mode == DenoiseMode::None {
        if let Some(stage) = opts.keep_linear {
            stats.linear = Some(linear_from_rgb(sums, stage, exposure, film, size));
        }
        return Ok(sums.to_vec());
    }
    let aov = aov().map_err(|e| format!("read aov: {e}"))?;
    let colors: Vec<[f32; 4]> = sums
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let n = p[3].max(1.0);
            let variance = super::tracer::PathTracer::variance_of_mean(*p, accum.get(pixels + i).map_or(0.0, |a| a[0]));
            [p[0] / n, p[1] / n, p[2] / n, variance]
        })
        .collect();
    let start = std::time::Instant::now();
    let learned = if opts.denoise_mode == DenoiseMode::Learned {
        match super::learned::denoise(gpu, super::learned::model_for(gpu, opts.denoise_model), &colors, &aov, size) {
            Ok(out) => Some(out),
            Err(why) => {
                stats.denoise_fallback = Some(why);
                None
            }
        }
    } else {
        None
    };
    let (out, used) = match learned {
        Some(out) => (out, DenoiseMode::Learned),
        None => (
            super::denoise::denoise_guided(gpu, &colors, &aov, size, opts.roughness_guide).map_err(|e| format!("denoise: {e}"))?,
            DenoiseMode::Filter,
        ),
    };
    stats.denoise_seconds = start.elapsed().as_secs_f32();
    stats.denoiser = used;
    stats.denoised = true;
    if let Some(stage) = opts.keep_linear {
        stats.linear = Some(linear_from_rgb(&out, stage, exposure, film, size));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::Film;

    #[test]
    fn film_resolve_matches_the_gpu_resolve_when_nearly_neutral() {
        use crate::exposure::{TONE_ACES, TONE_HABLE};
        let accum: Vec<[f32; 4]> = (0..16).map(|i| [i as f32 * 0.07, i as f32 * 0.03, 0.5, 2.0]).collect();
        let film = Film { vignette: 1.0e-9, ..Film::default() };
        for tone in [TONE_ACES, TONE_HABLE] {
            let got = film_rgba(&accum, 1.5, tone, &film, (4, 4));
            for (i, p) in accum.iter().enumerate() {
                for c in 0..3 {
                    // The shader's path: mean x exposure, tone curve, sRGB.
                    let lin = crate::exposure::tone_map(p[c] / p[3].max(1.0) * 1.5, tone);
                    let want = if lin <= 0.003_130_8 { lin * 12.92 } else { 1.055 * lin.powf(1.0 / 2.4) - 0.055 };
                    let want = (want * 255.0 + 0.5) as i32;
                    assert!((i32::from(got[i * 4 + c]) - want).abs() <= 1, "pixel {i} channel {c}");
                }
                assert_eq!(got[i * 4 + 3], 255);
            }
        }
    }

    #[test]
    fn radiance_stage_equals_accumulated_means() {
        let accum = [[10.0, 20.0, 30.0, 2.0], [30.0, 60.0, 90.0, 3.0], [f32::NAN, 0.0, 0.0, 1.0]];
        let got = linear_from_rgb(&accum, LinearStage::Radiance, 1.0, &Film::default(), (1, 3));
        assert_eq!(got, vec![[5.0, 10.0, 15.0], [10.0, 20.0, 30.0], [0.0, 0.0, 0.0]]);
    }

    #[test]
    fn film_stage_is_radiance_times_exposure_with_a_neutral_film() {
        let rgb = [[1.0, 2.0, 3.0, 1.0], [3.0, 4.0, 5.0, 1.0]];
        let got = linear_from_rgb(&rgb, LinearStage::Film, 2.0, &Film::default(), (2, 1));
        assert_eq!(got, vec![[2.0, 4.0, 6.0], [6.0, 8.0, 10.0]]);
    }

    #[test]
    fn keeping_linear_does_not_change_the_png() {
        let original = vec![[1.0, 2.0, 3.0, 1.0], [4.0, 5.0, 6.0, 1.0]];
        let mut before = original.clone();
        let _ = linear_from_rgb(&before, LinearStage::Radiance, 1.0, &Film::default(), (2, 1));
        assert_eq!(before, original);
    }

    #[test]
    fn furnace_linear_is_two_everywhere() {
        let rgb = vec![[2.0, 2.0, 2.0, 1.0]; 4];
        let got = linear_from_rgb(&rgb, LinearStage::Radiance, 1.0, &Film::default(), (2, 2));
        assert!(got.iter().all(|px| px == &[2.0, 2.0, 2.0]));
    }
}
