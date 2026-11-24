//! The still's film (phase 22): white balance, bloom, vignette and grade on display-linear values
//! (radiance times exposure) before the tone curve. Pure CPU code, so the result is deterministic
//! and the same on every adapter.

use cadcraft_doc::arch::Film;

/// The most taps one bloom pass reads per pixel.
const MAX_TAPS: usize = 129;
/// Bloom's three Gaussian widths in pixels at a 480-pixel short side.
const BLOOM_SIGMAS: [f32; 3] = [2.0, 6.0, 18.0];

/// Apply the still's film to linear RGB pixels (row-major, `size.0 x size.1`) before the tone curve.
///
/// In order: white balance, bloom, vignette, grade. A neutral film leaves every value untouched;
/// otherwise non-finite values become 0 first. A `pixels` slice whose length is not `w * h` only
/// gets the per-pixel steps (white balance and grade).
pub fn apply(film: &Film, pixels: &mut [[f32; 3]], size: (u32, u32)) {
    if film.is_neutral() || pixels.is_empty() {
        return;
    }
    for px in pixels.iter_mut() {
        for c in px.iter_mut() {
            if !c.is_finite() {
                *c = 0.0;
            }
        }
    }
    let (w, h) = (size.0 as usize, size.1 as usize);
    let shaped = w.checked_mul(h) == Some(pixels.len());

    if let Some(k) = film.white_balance {
        let rgb = cadcraft_arch::light::kelvin_to_rgb(k);
        let g = if rgb[1] > 0.0 { rgb[1] } else { 1.0 };
        let div = rgb.map(|c| if c > 0.0 { (c / g) as f32 } else { 1.0 });
        for px in pixels.iter_mut() {
            for (c, d) in px.iter_mut().zip(div) {
                *c /= d;
            }
        }
    }
    if shaped && film.bloom > 0.0 {
        bloom(pixels, w, h, film.bloom as f32, film.bloom_threshold as f32);
    }
    if shaped && film.vignette != 0.0 {
        vignette(pixels, w, h, film.vignette as f32);
    }
    grade(film, pixels);
}

/// The combined bloom kernel for a frame of `short` pixels on its short side: `(offset, weight)`
/// taps, at most [`MAX_TAPS`], weights summing to 1.
fn bloom_kernel(short: usize) -> Vec<(i32, f32)> {
    let scale = (short as f32 / 480.0).max(1.0e-3);
    let widest = BLOOM_SIGMAS[2] * scale;
    let reach = (3.0 * widest).ceil().max(1.0);
    let half_max = ((MAX_TAPS - 1) / 2) as f32;
    let stride = (reach / half_max).ceil().max(1.0) as i32;
    let half = ((reach / stride as f32).floor() as i32).clamp(1, half_max as i32);
    let mut taps = Vec::with_capacity(2 * half as usize + 1);
    let mut total = 0.0f32;
    for k in -half..=half {
        let d = (k * stride) as f32;
        let mut wt = 0.0;
        for s in BLOOM_SIGMAS {
            let sigma = s * scale;
            wt += (-(d * d) / (2.0 * sigma * sigma)).exp() / sigma;
        }
        total += wt;
        taps.push((k * stride, wt));
    }
    if total > 0.0 {
        for t in &mut taps {
            t.1 /= total;
        }
    }
    taps
}

/// One separable pass of `kernel` along `n` samples spaced `step` apart in `src`, over `lines`
/// lines whose starts are `line_step` apart. Each source spreads its value over the taps that fall
/// inside the frame, rescaled to sum to 1, so the pass conserves energy exactly at the borders.
fn blur_pass(src: &[[f32; 3]], dst: &mut [[f32; 3]], kernel: &[(i32, f32)], n: usize, step: usize, lines: usize, line_step: usize) {
    let inside: Vec<f32> = (0..n as i64)
        .map(|i| {
            let s: f32 = kernel.iter().filter(|(o, _)| (0..n as i64).contains(&(i + i64::from(*o)))).map(|t| t.1).sum();
            if s > 0.0 { 1.0 / s } else { 0.0 }
        })
        .collect();
    for line in 0..lines {
        let base = line * line_step;
        for (i, norm) in inside.iter().enumerate() {
            let Some(v) = src.get(base + i * step) else { continue };
            if v == &[0.0, 0.0, 0.0] {
                continue;
            }
            for &(o, wt) in kernel {
                let j = i as i64 + i64::from(o);
                if j < 0 || j >= n as i64 {
                    continue;
                }
                if let Some(d) = dst.get_mut(base + j as usize * step) {
                    let f = wt * norm;
                    d[0] += v[0] * f;
                    d[1] += v[1] * f;
                    d[2] += v[2] * f;
                }
            }
        }
    }
}

fn bloom(pixels: &mut [[f32; 3]], w: usize, h: usize, strength: f32, threshold: f32) {
    let threshold = threshold.max(0.0);
    let mut src = vec![[0.0f32; 3]; pixels.len()];
    for (p, s) in pixels.iter_mut().zip(src.iter_mut()) {
        for c in 0..3 {
            let excess = (p[c] - threshold).max(0.0) * strength;
            p[c] -= excess;
            s[c] = excess;
        }
    }
    let kernel = bloom_kernel(w.min(h));
    let mut tmp = vec![[0.0f32; 3]; pixels.len()];
    blur_pass(&src, &mut tmp, &kernel, w, 1, h, w);
    let mut out = vec![[0.0f32; 3]; pixels.len()];
    blur_pass(&tmp, &mut out, &kernel, h, w, w, 1);
    for (p, o) in pixels.iter_mut().zip(out) {
        for c in 0..3 {
            p[c] += o[c];
        }
    }
}

fn vignette(pixels: &mut [[f32; 3]], w: usize, h: usize, v: f32) {
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let corner2 = cx * cx + cy * cy;
    if corner2 <= 0.0 {
        return;
    }
    for (i, p) in pixels.iter_mut().enumerate() {
        let dx = (i % w) as f32 + 0.5 - cx;
        let dy = (i / w) as f32 + 0.5 - cy;
        let f = 1.0 - v * (dx * dx + dy * dy) / corner2;
        for c in p.iter_mut() {
            *c *= f;
        }
    }
}

fn grade(film: &Film, pixels: &mut [[f32; 3]]) {
    let (contrast, saturation, lift) = (film.contrast as f32, film.saturation as f32, film.lift as f32);
    if contrast == 1.0 && saturation == 1.0 && lift == 0.0 {
        return;
    }
    for p in pixels.iter_mut() {
        for c in p.iter_mut() {
            *c = ((*c - 0.18) * contrast + 0.18).max(0.0);
        }
        let lum = 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
        for c in p.iter_mut() {
            *c = lum + (*c - lum) * saturation + lift;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sum(p: &[[f32; 3]]) -> f64 {
        p.iter().flatten().map(|&c| f64::from(c)).sum()
    }

    #[test]
    fn neutral_film_touches_nothing() {
        let orig = vec![[0.5, f32::NAN, -2.0], [9.0, 0.1, 0.0], [f32::INFINITY, 1.0, 1.0], [0.2, 0.3, 0.4]];
        let mut p = orig.clone();
        apply(&Film::default(), &mut p, (2, 2));
        for (a, b) in p.iter().flatten().zip(orig.iter().flatten()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn bloom_conserves_energy_and_spreads() {
        let (w, h) = (96usize, 64usize);
        let mut p = vec![[0.1f32; 3]; w * h];
        p[5 * w + 3] = [50.0, 40.0, 30.0]; // near a corner
        p[32 * w + 48] = [80.0, 80.0, 80.0];
        let before = sum(&p);
        let film = Film { bloom: 0.8, bloom_threshold: 1.0, ..Film::default() };
        apply(&film, &mut p, (w as u32, h as u32));
        assert!((sum(&p) - before).abs() / before < 0.005);
        assert!(p[32 * w + 48][0] < 80.0);
        assert!(p[32 * w + 49][0] > 0.1 + 1.0e-4, "neighbour gains light");
        assert!(p[32 * w + 54][0] > 0.1, "light reaches further out");
    }

    #[test]
    fn vignette_is_one_minus_v_at_the_corner() {
        let (w, h) = (200usize, 100usize);
        let mut p = vec![[1.0f32; 3]; w * h];
        let film = Film { vignette: 0.5, ..Film::default() };
        apply(&film, &mut p, (w as u32, h as u32));
        assert!((p[0][0] - 0.5).abs() < 0.01, "corner {}", p[0][0]);
        assert!((p[h / 2 * w + w / 2][0] - 1.0).abs() < 1.0e-3);
    }

    #[test]
    fn white_balance_neutralises_its_kelvin() {
        for k in [3000.0, 4500.0, 9000.0] {
            let rgb = cadcraft_arch::light::kelvin_to_rgb(k);
            let g = rgb[1];
            let mut p = vec![[(rgb[0] / g) as f32 * 0.5, 0.5, (rgb[2] / g) as f32 * 0.5]];
            let film = Film { white_balance: Some(k), ..Film::default() };
            apply(&film, &mut p, (1, 1));
            for c in p[0] {
                assert!((c - 0.5).abs() < 1.0e-4, "{k}: {:?}", p[0]);
            }
        }
    }

    #[test]
    fn grade_pivots_on_mid_grey() {
        let film = Film { contrast: 1.5, ..Film::default() };
        let mut p = vec![[0.18; 3]];
        apply(&film, &mut p, (1, 1));
        assert!(p[0].iter().all(|c| (c - 0.18).abs() < 1.0e-6));
        let film = Film { contrast: 2.0, saturation: 0.0, lift: 0.05, ..Film::default() };
        let mut p = vec![[0.28, 0.18, 0.08]];
        apply(&film, &mut p, (1, 1));
        // contrast gives (0.38, 0.18, -0.02 -> 0); saturation 0 collapses to the luminance.
        let lum = 0.2126 * 0.38 + 0.7152 * 0.18;
        assert!(p[0].iter().all(|c| (c - (lum + 0.05)).abs() < 1.0e-5), "{:?}", p[0]);
    }
}
