//! What a path-traced still does to the linear image once the accumulator is read (phase 22):
//! the denoiser the options ask for, and (p22-r3d-film, p22-r3d-linear-out) the film and the kept
//! linear image. With a neutral film and no kept image the result goes back into the accumulator
//! and the GPU resolve runs on it exactly as it did before phase 22.

use super::{PathOptions, PathStats};
use cadcraft_doc::arch::{DenoiseMode, Film};

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
    stats: &mut PathStats,
) -> Result<Vec<[f32; 4]>, String> {
    let pixels = (size.0 as usize) * (size.1 as usize);
    let sums = accum.get(..pixels).ok_or_else(|| "accumulator is smaller than the image".to_string())?;
    // Neutral film and nothing kept: the resolve is the one it always was. A film or a kept
    // linear image is applied after this point by later packages.
    let _plain = film.is_neutral() && opts.keep_linear.is_none();
    stats.denoiser = DenoiseMode::None;
    stats.denoised = false;
    if !opts.denoise || opts.denoise_mode == DenoiseMode::None {
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
        match super::learned::denoise(gpu, &colors, &aov, size) {
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
    Ok(out)
}
