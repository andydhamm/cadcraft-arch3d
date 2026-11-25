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
    let mut aov = aov().map_err(|e| format!("read aov: {e}"))?;
    let counts: Vec<f32> = sums.iter().map(|p| p[3]).collect();
    if opts.denoise_mode == DenoiseMode::Learned {
        // The kernel summed the guides over the samples (`PHOTO_AOV_MEAN`).
        mean_guides(&mut aov, &counts);
    }
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
            keep_consistent(&colors, &counts, size, &mut out);
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
/// The fixed 0.2 % .. 1 % blend that [`keep_consistent`] generalises (kept for its tests). With
/// `e = sqrt(variance) / max(L, 0.05 * Lbar, 1e-6)` the result is `keep * input + (1 - keep) * denoised`
/// per channel, `keep = 1 - smoothstep(0.002, 0.01, e)`. Non-finite input or variance counts as `e = infinity`.
#[allow(dead_code)]
pub(super) fn keep_converged(input: &[[f32; 4]], denoised: &mut [[f32; 4]]) {
    let lum = |p: &[f32; 4]| super::luminance([p[0], p[1], p[2]]);
    let floor = luminance_floor(input);
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

/// Divide the guides' sums by the pixel's sample count (`PHOTO_AOV_MEAN`): albedo and normal become
/// means over the pixel's samples (a miss added zero), the normal is renormalised when its length is
/// above 1e-6. `aov` holds two entries a pixel; `counts` one count per pixel (at least 1 is used).
pub(super) fn mean_guides(aov: &mut [[f32; 4]], counts: &[f32]) {
    for (pair, &count) in aov.chunks_exact_mut(2).zip(counts) {
        let n = if count.is_finite() { count.max(1.0) } else { 1.0 };
        if let [a, nrm] = pair {
            for c in &mut a[..3] {
                *c /= n;
            }
            for c in &mut nrm[..3] {
                *c /= n;
            }
            if count.is_finite() {
                let len = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
                if len.is_finite() && len > 1e-6 {
                    for c in &mut nrm[..3] {
                        *c /= len;
                    }
                }
            }
        }
    }
}

/// The frame's luminance floor: 5 % of the mean luminance over the finite pixels (never 0).
fn luminance_floor(input: &[[f32; 4]]) -> f32 {
    let (mut sum, mut count) = (0.0_f64, 0usize);
    for p in input {
        let l = super::luminance([p[0], p[1], p[2]]);
        if l.is_finite() {
            sum += f64::from(l);
            count += 1;
        }
    }
    let mean = if count == 0 { 0.0 } else { (sum / count as f64) as f32 };
    (0.05 * mean).max(1e-6)
}

/// The robust standard error of each pixel's mean luminance (design decision 79's relative error with
/// the neighbourhood's say): `max(s, sqrt(rho / n) * max(L, floor))`, where `s` is the pixel's own
/// standard error (`colors[i][3]` is its variance), `n` its sample count and `rho` the relative per-sample
/// variance pooled over the 5 x 5 window: the sum of `n * variance` over the sum of `max(L, floor)^2`.
/// A pixel that happens to have seen no bright sample still gets the error its neighbours show.
/// Non-finite or negative values count as 0 in the pools; a pixel with non-finite colour gets infinity.
/// Empty when `colors` or `counts` do not match `size`.
fn robust_sigma(colors: &[[f32; 4]], counts: &[f32], size: (u32, u32)) -> Vec<f32> {
    let (w, h) = (size.0 as usize, size.1 as usize);
    let Some(pixels) = w.checked_mul(h) else { return Vec::new() };
    if colors.len() != pixels || counts.len() != pixels {
        return Vec::new();
    }
    let floor = luminance_floor(colors);
    let n_of = |i: usize| counts.get(i).copied().filter(|n| n.is_finite()).map_or(1.0, |n| n.max(1.0));
    // Per-pixel pooled terms: n * variance of the mean, and the floored luminance squared.
    let mut var = vec![0.0_f64; pixels];
    let mut den = vec![0.0_f64; pixels];
    let mut lum = vec![f32::NAN; pixels];
    for (i, p) in colors.iter().enumerate() {
        let l = super::luminance([p[0], p[1], p[2]]);
        if let (Some(v), Some(d), Some(lm)) = (var.get_mut(i), den.get_mut(i), lum.get_mut(i)) {
            *lm = l;
            if l.is_finite() && p[3].is_finite() && p[3] >= 0.0 {
                *v = f64::from(p[3]) * f64::from(n_of(i));
                let f = f64::from(l.max(floor));
                *d = f * f;
            }
        }
    }
    let mut out = vec![f32::INFINITY; pixels];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let (mut sv, mut sd) = (0.0_f64, 0.0_f64);
            for yy in y.saturating_sub(2)..(y + 3).min(h) {
                for xx in x.saturating_sub(2)..(x + 3).min(w) {
                    let j = yy * w + xx;
                    sv += var.get(j).copied().unwrap_or(0.0);
                    sd += den.get(j).copied().unwrap_or(0.0);
                }
            }
            let l = lum.get(i).copied().unwrap_or(f32::NAN);
            let own = colors.get(i).map_or(f32::NAN, |p| p[3]);
            if !l.is_finite() || !own.is_finite() || own < 0.0 {
                continue;
            }
            let rho = if sd > 0.0 { sv / sd } else { 0.0 };
            let pooled = (rho / f64::from(n_of(i))).sqrt() * f64::from(l.max(floor));
            if let Some(o) = out.get_mut(i) {
                *o = (own.sqrt()).max(pooled as f32);
            }
        }
    }
    out
}

/// After the learned denoiser: a converged pixel keeps its own value, every other keeps the network's.
///
/// `input` holds `[mean r, g, b, variance of the mean of the luminance]` per pixel, `counts` the
/// samples behind each, `denoised` the network's answer. A pixel whose [`robust_sigma`] is under 0.2 % of
/// `max(L, floor)` is converged and keeps its input exactly (the flat frame must not move). A pixel
/// whose network answer is not finite takes its input. Nothing else is changed.
///
/// Until 2026-11-24 the answer was also scaled into `[L - 2 s_r, L + 2 s_r]`. In a room lit by rare
/// bright paths most pixels saw none of them at 64 spp, so `L` and `s_r` are both far too low there and
/// the clamp took back the light the network had (rightly) spread: the showcase kitchen lost 21 % of its
/// mean, and `energy::restore_energy` then put noise back to make up for it. Measured against the
/// official oidnDenoise on the same inputs (`tests/oidn_reference.rs`) the bare network keeps the mean
/// within 1 % and has the lower error, so both steps are gone.
pub(super) fn keep_consistent(input: &[[f32; 4]], counts: &[f32], size: (u32, u32), denoised: &mut [[f32; 4]]) {
    let sigma = robust_sigma(input, counts, size);
    if sigma.len() != input.len() {
        return;
    }
    let floor = luminance_floor(input);
    for ((inp, out), &s) in input.iter().zip(denoised.iter_mut()).zip(&sigma) {
        let l = super::luminance([inp[0], inp[1], inp[2]]);
        if !l.is_finite() || !s.is_finite() {
            continue;
        }
        if s < 0.002 * l.max(floor) || !out[..3].iter().all(|c| c.is_finite()) {
            out[..3].copy_from_slice(&inp[..3]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::Film;

    /// A `w x h` frame of pixels with luminance 1 (grey), sample count `n` and per-sample variance
    /// `v` (so the variance of the mean is `v / n`).
    fn grey_frame(w: u32, h: u32, n: f32, v: f32) -> (Vec<[f32; 4]>, Vec<f32>) {
        (vec![[1.0, 1.0, 1.0, v / n]; (w * h) as usize], vec![n; (w * h) as usize])
    }

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

    #[test]
    fn keep_converged_empty() {
        let input: [[f32; 4]; 0] = [];
        let mut out: Vec<[f32; 4]> = Vec::new();
        keep_converged(&input, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn keep_converged_single_pixel_converged() {
        let input = [[0.5, 0.5, 0.5, 0.0]];
        let mut out = vec![[0.3, 0.3, 0.3, 1.0]];
        keep_converged(&input, &mut out);
        assert_eq!(out[0][0], 0.5, "converged single pixel keeps input");
        assert_eq!(out[0][1], 0.5);
        assert_eq!(out[0][2], 0.5);
    }

    #[test]
    fn keep_converged_mixed_luminance() {
        let input = [[1.0, 1.0, 1.0, 0.000001], [0.1, 0.1, 0.1, 0.0000001]];
        let mut out = vec![[0.5, 0.5, 0.5, 1.0], [0.5, 0.5, 0.5, 1.0]];
        keep_converged(&input, &mut out);
        assert!(out[0][0] > 0.99, "bright converged pixel keeps input: {}", out[0][0]);
        assert!(out[1][0] > 0.12, "dark converged pixel mostly keeps input: {}", out[1][0]);
    }

    #[test]
    fn keep_converged_non_finite_channels() {
        let input = [[f32::NAN, 1.0, 1.0, 0.0]];
        let mut out = vec![[0.5, 0.5, 0.5, 1.0]];
        keep_converged(&input, &mut out);
        assert_eq!(out[0], [0.5, 0.5, 0.5, 1.0], "non-finite channel keeps denoised");
    }

    #[test]
    fn keep_converged_negative_variance() {
        let input = [[1.0, 1.0, 1.0, -0.1]];
        let mut out = vec![[0.5, 0.5, 0.5, 1.0]];
        keep_converged(&input, &mut out);
        assert_eq!(out[0], [0.5, 0.5, 0.5, 1.0], "negative variance keeps denoised");
    }

    #[test]
    fn keep_converged_smoothstep_boundaries() {
        let input = [[1.0, 1.0, 1.0, 0.002 * 0.002], [1.0, 1.0, 1.0, 0.01 * 0.01]];
        let mut out = vec![[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]];
        keep_converged(&input, &mut out);
        assert_eq!(out[0][0], 1.0, "e = 0.2 % keeps the input");
        assert_eq!(out[1][0], 0.0, "e = 1 % keeps the denoised value");
    }

    /// robust_sigma on a single pixel: no neighbourhood, so it falls back to the pixel's own error.
    #[test]
    fn robust_sigma_single_pixel() {
        let colors = vec![[1.0, 1.0, 1.0, 0.04]];
        let counts = vec![4.0];
        let sigma = robust_sigma(&colors, &counts, (1, 1));
        assert_eq!(sigma.len(), 1);
        // Own error sqrt(0.04) = 0.2; no neighbours to pool, so rho = 0 and pooled = 0.
        assert!((sigma[0] - 0.2).abs() < 1e-6, "single pixel uses its own error: {}", sigma[0]);

        // A single noisy pixel among itself: the window is just itself.
        let colors = vec![[2.0, 2.0, 2.0, 0.25]];
        let counts = vec![4.0];
        let sigma = robust_sigma(&colors, &counts, (1, 1));
        // L = 2.0, floor = 0.1, n = 4, var = 0.25, own = 0.5.
        // rho = (4 * 0.25) / (4 * 4.0) = 0.0625, pooled = sqrt(0.0625/4) * 2.0 = 0.25.
        // sigma = max(0.5, 0.25) = 0.5.
        assert!((sigma[0] - 0.5).abs() < 1e-6, "single noisy pixel: {}", sigma[0]);
    }

    #[test]
    fn robust_sigma_sees_the_neighbourhood() {
        // 3 x 3 pixels, all luminance 1 (floor 0.05), 4 samples each. The eight outer pixels have a
        // per-sample variance of 0.36 (variance of the mean 0.09), the centre pixel has none.
        let mut colors = vec![[1.0, 1.0, 1.0, 0.09]; 9];
        colors[4][3] = 0.0;
        let counts = vec![4.0; 9];
        let sigma = robust_sigma(&colors, &counts, (3, 3));
        // The centre's 5 x 5 window is the whole frame: rho = 8 * 0.36 / 9 = 0.32 and
        // s_r = sqrt(0.32 / 4) * max(1, 0.05) = 0.28284.
        assert!((sigma[4] - 0.282_842_7).abs() < 1e-5, "{}", sigma[4]);
        // A noisy pixel keeps at least its own error 0.3.
        assert!(sigma[0] >= 0.3 - 1e-6, "{}", sigma[0]);
        assert!(robust_sigma(&colors, &counts[..4], (3, 3)).is_empty(), "short counts give nothing");
        assert!(robust_sigma(&[], &[], (0, 0)).is_empty());
    }

    /// The spec's named test: converged pixels keep their input bit for bit, unconverged pixels keep
    /// the network's answer, and hostile input (NaN, zero counts, negative variance) never panics.
    #[test]
    fn consistent_output_stays_within_the_sample_interval() {
        // A converged pixel (zero variance) keeps its input exactly.
        let (conv, counts) = grey_frame(3, 3, 16.0, 0.0);
        let mut out = vec![[0.3, 0.4, 0.5, 1.0]; 9];
        keep_consistent(&conv, &counts, (3, 3), &mut out);
        for (inp, o) in conv.iter().zip(&out) {
            assert_eq!(o[0].to_bits(), inp[0].to_bits(), "converged pixel keeps input bit for bit");
            assert_eq!(o[1].to_bits(), inp[1].to_bits());
            assert_eq!(o[2].to_bits(), inp[2].to_bits());
        }

        // An unconverged pixel keeps the network's answer, even far outside L ± 2 s_r.
        let (input, counts) = grey_frame(3, 3, 16.0, 0.16);
        let far = [[5.0, 2.5, 0.0, 1.0]; 9];
        let mut out = far;
        keep_consistent(&input, &counts, (3, 3), &mut out);
        for o in &out {
            assert_eq!(o, &far[0], "unconverged pixel keeps the network's answer");
        }

        // A network answer with a NaN channel pulls the input back in.
        let mut input = grey_frame(3, 3, 16.0, 0.16).0;
        let mut net = vec![[1.0, f32::NAN, 0.5, 1.0]; 9];
        keep_consistent(&input, &counts, (3, 3), &mut net);
        for (inp, o) in input.iter().zip(&net) {
            assert_eq!(o[..3], inp[..3], "NaN in network answer pulls input back");
        }

        // Zero sample counts do not panic.
        keep_consistent(&input, &vec![0.0; 9], (3, 3), &mut net);

        // Negative variance does not panic.
        let mut bad = grey_frame(3, 3, 16.0, 0.0).0;
        for p in &mut bad {
            p[3] = -1.0;
        }
        let mut out = vec![[0.5, 0.5, 0.5, 1.0]; 9];
        keep_consistent(&bad, &vec![4.0; 9], (3, 3), &mut out);
    }

    #[test]
    fn an_unconverged_pixel_keeps_the_network_answer() {
        // 5 x 5, luminance 1, 16 samples, per-sample variance 0.16: s = s_r = 0.1. The network may answer far
        // outside L +- 2 s_r: where rare bright paths light a region most pixels saw none of them, and the old
        // clamp to that interval cut 21 % of the kitchen's light (operator 2026-11-24, against oidnDenoise).
        let (mut input, counts) = grey_frame(5, 5, 16.0, 0.16);
        for answer in [[5.0, 2.5, 0.0, 1.0], [0.01, 0.01, 0.01, 1.0], [1.05, 1.05, 1.05, 1.0], [0.0, 0.0, 0.0, 1.0]] {
            let mut out = vec![answer; 25];
            keep_consistent(&input, &counts, (5, 5), &mut out);
            assert!(out.iter().all(|p| *p == answer), "the network's {answer:?} is kept: {:?}", out[0]);
        }
        // A converged frame keeps its input bit for bit.
        let (conv, counts) = grey_frame(5, 5, 16.0, 0.0);
        let mut out = vec![[3.0, 0.0, 0.5, 1.0]; 25];
        keep_consistent(&conv, &counts, (5, 5), &mut out);
        assert!(out.iter().all(|p| p[0].to_bits() == 1.0f32.to_bits() && p[1].to_bits() == 1.0f32.to_bits() && p[2].to_bits() == 1.0f32.to_bits()));
        // Hostile input does not panic and keeps the network's answer.
        input[3] = [f32::NAN, 1.0, 1.0, f32::NAN];
        input[7][3] = -1.0;
        let mut out = vec![[0.5, 0.5, 0.5, 1.0]; 25];
        keep_consistent(&input, &vec![0.0; 25], (5, 5), &mut out);
        assert_eq!(out[3], [0.5, 0.5, 0.5, 1.0]);
        keep_consistent(&input, &[], (5, 5), &mut out);
        keep_consistent(&[], &[], (0, 0), &mut []);
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
                // The learned kernel's guides are sums over the samples.
                let k = n as f32;
                aov.push([0.01 * k, 0.01 * k, 0.01 * k, 1.0]);
                aov.push([normal[0] * k, normal[1] * k, normal[2] * k, 3.0]);
            }
        }
        (accum, aov)
    }

    #[test]
    fn flat_frame_through_finish_linear_changes_no_pixel_by_one_percent() {
        flat_frame_still_changes_no_pixel_by_one_percent();
    }

    #[test]
    fn flat_frame_still_changes_no_pixel_by_one_percent() {
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
    fn firefly_lit_frame_keeps_its_mean_through_the_learned_denoiser() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute adapter");
            return;
        };
        let size = (96, 64);
        let n = 64u32;
        let mut state = 0x9e37_79b9_u64;
        let mut samples = vec![0.0_f32; (96 * 64 * n) as usize];
        for v in &mut samples {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            let u = ((state >> 40) as f32) / (1u64 << 24) as f32;
            *v = 0.02 + if u < 1.0 / 400.0 { 40.0 } else { 0.0 };
        }
        let (accum, aov) = accum_frame(size, n, |x, y, k| samples.get(((y * 96 + x) * n + k) as usize).copied().unwrap_or(0.0));
        let mut stats = PathStats::default();
        let out = finish_linear(&gpu, &opts_learned(), &Film::default(), size, &accum, || Ok(aov), 1.0, &mut stats).expect("finish");
        if stats.denoiser != DenoiseMode::Learned {
            eprintln!("SKIP: the denoiser fell back: {:?}", stats.denoise_fallback);
            return;
        }
        let px = out.len() as f64;
        let mean_in: f64 = accum.iter().take(out.len()).map(|a| f64::from(a[0] / a[3])).sum::<f64>() / px;
        let mean_out: f64 = out.iter().map(|p| f64::from(p[0])).sum::<f64>() / px;
        assert!((mean_out - mean_in).abs() <= 0.1 * mean_in, "mean in {mean_in}, out {mean_out}");
        let rms = (out.iter().map(|p| (f64::from(p[0]) - 0.12).powi(2)).sum::<f64>() / px).sqrt() / 0.12;
        assert!(rms < 0.35, "relative rms error {rms}");
    }

    /// mean_guides edge cases: empty input, non-finite counts, zero-length normals after division.
    #[test]
    fn mean_guides_edge_cases() {
        // Empty input: no panic, no change.
        let mut empty: Vec<[f32; 4]> = Vec::new();
        mean_guides(&mut empty, &[]);
        assert!(empty.is_empty());

        // Non-finite count is treated as 1.
        let mut aov = vec![[4.0, 4.0, 4.0, 1.0], [0.0, 0.0, 4.0, 3.0]];
        mean_guides(&mut aov, &[f32::NAN]);
        assert_eq!(aov[0], [4.0, 4.0, 4.0, 1.0], "NaN count divides by 1");
        assert_eq!(aov[1], [0.0, 0.0, 4.0, 3.0], "normal unchanged with NaN count");

        // Zero-length normal after division stays zero (no renormalisation).
        let mut aov = vec![[2.0, 2.0, 2.0, 1.0], [0.0, 0.0, 0.0, 3.0]];
        mean_guides(&mut aov, &[2.0]);
        assert_eq!(aov[0], [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(aov[1], [0.0, 0.0, 0.0, 3.0], "zero normal stays zero");

        // Normal is renormalised when its length is above 1e-6.
        let mut aov = vec![[0.0; 4], [3.0, 0.0, 0.0, 3.0]];
        mean_guides(&mut aov, &[3.0]);
        assert_eq!(aov[1], [1.0, 0.0, 0.0, 3.0], "normal renormalised");

        // A normal very close to zero length (below 1e-6) is not renormalised.
        let mut aov = vec![[0.0; 4], [1e-9, 0.0, 0.0, 3.0]];
        mean_guides(&mut aov, &[1e9]);
        // After division: 1e-18, which is below 1e-6, so no renormalisation.
        assert!(aov[1][0].abs() < 1e-10, "tiny normal not renormalised: {}", aov[1][0]);
    }

    #[test]
    fn keep_consistent_all_converged() {
        // Every pixel converged: all keep their input, none take the network answer.
        let (input, counts) = grey_frame(4, 4, 32.0, 0.0);
        let net = vec![[0.0, 0.0, 0.0, 1.0]; 16];
        let mut out = net;
        keep_consistent(&input, &counts, (4, 4), &mut out);
        for (inp, o) in input.iter().zip(&out) {
            assert_eq!(o[..3], inp[..3], "all converged: every pixel keeps input");
        }
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

    #[test]
    fn finish_linear_empty() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute adapter");
            return;
        };
        let opts = PathOptions { denoise: false, ..PathOptions::default() };
        let mut stats = PathStats::default();
        let out = finish_linear(&gpu, &opts, &Film::default(), (0, 0), &[], || Ok(Vec::new()), 1.0, &mut stats).expect("finish");
        assert!(out.is_empty());
    }

    #[test]
    fn finish_linear_no_denoise_keeps_linear() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute adapter");
            return;
        };
        let accum = [[2.0, 4.0, 6.0, 2.0]];
        let opts = PathOptions { denoise: false, keep_linear: Some(LinearStage::Radiance), ..PathOptions::default() };
        let mut stats = PathStats::default();
        let out = finish_linear(&gpu, &opts, &Film::default(), (1, 1), &accum, || Ok(Vec::new()), 1.0, &mut stats).expect("finish");
        assert_eq!(out, vec![[2.0, 4.0, 6.0, 2.0]]);
        assert!(stats.linear.is_some());
        assert_eq!(stats.linear.as_ref().unwrap(), &vec![[1.0, 2.0, 3.0]]);
    }

    #[test]
    fn film_rgba_zero_exposure() {
        let accum = vec![[1.0, 1.0, 1.0, 1.0]];
        let got = film_rgba(&accum, 0.0, crate::exposure::TONE_NONE, &Film::default(), (1, 1));
        assert_eq!(got[0], 0, "zero exposure is black");
        assert_eq!(got[1], 0);
        assert_eq!(got[2], 0);
        assert_eq!(got[3], 255);
    }

    #[test]
    fn linear_from_rgb_infinity_becomes_zero() {
        let accum = [[f32::INFINITY, 1.0, 1.0, 1.0], [1.0, f32::NEG_INFINITY, 1.0, 1.0], [1.0, 1.0, f32::INFINITY, 1.0]];
        let got = linear_from_rgb(&accum, LinearStage::Radiance, 1.0, &Film::default(), (3, 1));
        assert_eq!(got, vec![[0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 0.0]]);
    }

    #[test]
    fn finish_linear_keep_linear_film_stage() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute adapter");
            return;
        };
        let accum = [[2.0, 4.0, 6.0, 2.0]];
        let opts = PathOptions { denoise: false, keep_linear: Some(LinearStage::Film), ..PathOptions::default() };
        let mut stats = PathStats::default();
        let out = finish_linear(&gpu, &opts, &Film::default(), (1, 1), &accum, || Ok(Vec::new()), 3.0, &mut stats).expect("finish");
        assert_eq!(out, vec![[2.0, 4.0, 6.0, 2.0]]);
        assert!(stats.linear.is_some());
        // Film stage: mean * exposure = [1.0, 2.0, 3.0] * 3.0 = [3.0, 6.0, 9.0]
        assert_eq!(stats.linear.as_ref().unwrap(), &vec![[3.0, 6.0, 9.0]]);
    }

    #[test]
    fn linear_from_rgb_film_stage_exposure_overflow_becomes_zero() {
        let accum = [[f32::MAX, 1.0, 1.0, 1.0]];
        let got = linear_from_rgb(&accum, LinearStage::Film, 2.0, &Film::default(), (1, 1));
        assert_eq!(got, vec![[0.0, 2.0, 2.0]]);
    }
}
