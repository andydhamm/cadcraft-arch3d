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
        Some(mut out) => {
            keep_converged(&colors, &mut out);
            (out, DenoiseMode::Learned)
        }
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

/// After a learned denoise: a pixel whose standard error is already small keeps its own value.
///
/// `input` holds `[mean r, g, b, variance of the mean of the luminance]` per pixel, `denoised` the
/// network's answer for the same pixels. With `e = sqrt(variance) / max(L, 0.05 * Lbar, 1e-6)` (the
/// relative error of design decision 79; `L` the pixel's mean luminance, `Lbar` the frame's mean) the
/// result is `keep * input + (1 - keep) * denoised` per channel, `keep = 1 - smoothstep(0.002, 0.01, e)`.
/// A pixel under 0.2 % error is more exact than the 1 % the network may move it; one at 1 % or more is
/// the network's answer unchanged. Non-finite input or variance counts as `e = infinity`.
pub(super) fn keep_converged(input: &[[f32; 4]], denoised: &mut [[f32; 4]]) {
    let lum = |p: &[f32; 4]| super::luminance([p[0], p[1], p[2]]);
    let (mut sum, mut count) = (0.0_f64, 0usize);
    for p in input {
        let l = lum(p);
        if l.is_finite() {
            sum += f64::from(l);
            count += 1;
        }
    }
    let mean = if count == 0 { 0.0 } else { (sum / count as f64) as f32 };
    let floor = (0.05 * mean).max(1e-6);
    for (inp, out) in input.iter().zip(denoised.iter_mut()) {
        let l = lum(inp);
        let var = inp[3];
        let finite = l.is_finite() && var.is_finite() && var >= 0.0 && inp[..3].iter().all(|c| c.is_finite());
        let e = if finite { var.sqrt() / l.max(floor) } else { f32::INFINITY };
        let t = ((e - 0.002) / 0.008).clamp(0.0, 1.0);
        let keep = if e.is_nan() { 0.0 } else { 1.0 - t * t * (3.0 - 2.0 * t) };
        if keep <= 0.0 {
            continue;
        }
        for c in 0..3 {
            if let (Some(o), Some(i)) = (out.get_mut(c), inp.get(c)) {
                *o = keep * *i + (1.0 - keep) * *o;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::Film;

    #[test]
    fn keep_converged_blends_by_relative_error() {
        // Grey of luminance 1 everywhere, so e = sqrt(variance).
        let input = [[1.0, 1.0, 1.0, 0.0], [1.0, 1.0, 1.0, 0.05 * 0.05], [1.0, 1.0, 1.0, 0.006 * 0.006], [1.0, 1.0, 1.0, f32::NAN]];
        let mut out = vec![[0.5, 0.5, 0.5, 0.0]; 4];
        keep_converged(&input, &mut out);
        assert_eq!(out[0], [1.0, 1.0, 1.0, 0.0], "converged keeps the input");
        assert_eq!(out[1], [0.5, 0.5, 0.5, 0.0], "e = 5 % keeps the denoised value");
        assert!(out[2][0] > 0.5 && out[2][0] < 1.0, "e = 0.6 % is in between: {}", out[2][0]);
        assert_eq!(out[3], [0.5, 0.5, 0.5, 0.0], "NaN variance keeps the denoised value");
    }

    fn opts_learned() -> PathOptions {
        PathOptions { denoise: true, denoise_mode: DenoiseMode::Learned, ..PathOptions::default() }
    }

    /// Accumulator (sums then second half) and guides for a frame; `sample(x, y, k)` is sample k's value.
    fn accum_frame(size: (u32, u32), n: u32, sample: impl Fn(u32, u32, u32) -> f32) -> (Vec<[f32; 4]>, Vec<[f32; 4]>) {
        let mut accum = vec![[0.0_f32; 4]; (size.0 * size.1 * 2) as usize];
        let px = (size.0 * size.1) as usize;
        for y in 0..size.1 {
            for x in 0..size.0 {
                let i = (y * size.0 + x) as usize;
                let (mut s, mut s2) = (0.0_f32, 0.0_f32);
                for k in 0..n {
                    let v = sample(x, y, k);
                    s += v;
                    s2 += v * v;
                }
                accum[i] = [s, s, s, n as f32];
                accum[px + i] = [s2, 1.0, 0.0, 0.0];
            }
        }
        let mut aov = Vec::new();
        for y in 0..size.1 {
            for x in 0..size.0 {
                let normal = if x < size.0 / 3 {
                    [1.0, 0.0, 0.0]
                } else if y < size.1 / 2 {
                    [0.0, 1.0, 0.0]
                } else {
                    [0.0, 0.0, 1.0]
                };
                aov.push([0.01, 0.01, 0.01, 1.0]);
                aov.push([normal[0], normal[1], normal[2], 3.0]);
            }
        }
        (accum, aov)
    }

    #[test]
    fn flat_frame_through_finish_linear_changes_no_pixel_by_one_percent() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute adapter");
            return;
        };
        let size = (160, 96);
        let (accum, aov) = accum_frame(size, 256, |_, _, _| 0.18);
        let mut stats = PathStats::default();
        let out = finish_linear(&gpu, &opts_learned(), &Film::default(), size, &accum, || Ok(aov), 1.0, &mut stats).expect("finish");
        assert_eq!(stats.denoiser, DenoiseMode::Learned, "fallback: {:?}", stats.denoise_fallback);
        for (i, p) in out.iter().enumerate() {
            for c in 0..3 {
                assert!((p[c] - 0.18).abs() <= 0.0018, "pixel {i} channel {c} is {}", p[c]);
            }
        }
    }

    #[test]
    fn noisy_frame_is_still_denoised_by_the_learned_model() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute adapter");
            return;
        };
        let size = (96, 64);
        let clean = |x: u32, y: u32| 0.2 + 0.5 * (x as f32 / 96.0) * (0.5 + 0.5 * y as f32 / 64.0);
        let mut state = 0x1234_5678_u64;
        let mut noise = vec![0.0_f32; (96 * 64 * 16) as usize];
        for v in &mut noise {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            *v = (((state >> 40) as f32) / (1u64 << 24) as f32 - 0.5) * 0.8;
        }
        let (accum, aov) = accum_frame(size, 16, |x, y, k| (clean(x, y) + noise[((y * 96 + x) * 16 + k) as usize]).max(0.0));
        let mut stats = PathStats::default();
        let out = finish_linear(&gpu, &opts_learned(), &Film::default(), size, &accum, || Ok(aov), 1.0, &mut stats).expect("finish");
        assert_eq!(stats.denoiser, DenoiseMode::Learned, "fallback: {:?}", stats.denoise_fallback);
        let (mut e_in, mut e_out) = (0.0_f64, 0.0_f64);
        for y in 0..size.1 {
            for x in 0..size.0 {
                let i = (y * size.0 + x) as usize;
                let want = f64::from(clean(x, y));
                let (a, o) = (accum[i], out[i]);
                e_in += (f64::from(a[0] / a[3]) - want).powi(2);
                e_out += (f64::from(o[0]) - want).powi(2);
            }
        }
        assert!(e_out <= 0.5 * e_in, "mse in {e_in}, out {e_out}");
    }

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

    #[test]
    fn linear_from_rgb_empty() {
        let got = linear_from_rgb(&[], LinearStage::Radiance, 1.0, &Film::default(), (0, 0));
        assert!(got.is_empty());
    }

    #[test]
    fn linear_from_rgb_alpha_zero_no_crash() {
        let accum = [[1.0, 2.0, 3.0, 0.0]];
        let got = linear_from_rgb(&accum, LinearStage::Radiance, 1.0, &Film::default(), (1, 1));
        assert_eq!(got, vec![[1.0, 2.0, 3.0]]);
    }

    #[test]
    fn srgb_u8_clamps_and_handles_nan() {
        assert_eq!(srgb_u8(-1.0), 0);
        assert_eq!(srgb_u8(2.0), 255);
        assert_eq!(srgb_u8(f32::NAN), 0);
        assert_eq!(srgb_u8(0.0), 0);
        assert_eq!(srgb_u8(1.0), 255);
    }

    #[test]
    fn film_rgba_tone_none_is_identity() {
        use crate::exposure::TONE_NONE;
        let accum = vec![[0.5, 0.5, 0.5, 1.0]];
        let got = film_rgba(&accum, 1.0, TONE_NONE, &Film::default(), (1, 1));
        let expected = srgb_u8(0.5);
        assert_eq!(got[0], expected);
        assert_eq!(got[1], expected);
        assert_eq!(got[2], expected);
        assert_eq!(got[3], 255);
    }
}
