//! Putting back the light the learned denoiser takes out of firefly-lit regions (phase 22).
//!
//! A scene lit by rare bright paths has a very large relative error per pixel. The network works in a
//! log-like domain and treats those samples as outliers, so in a region lit mostly by them its answer sits
//! near the median, far below the mean, and `post::keep_consistent` cannot repair that: its interval
//! `[L - 2 s, L + 2 s]` has the lower bound 0 on such pixels. The bias is squared into the error against
//! a converged reference. The network keeps its job (structure, edges, hue) and runs exactly as
//! `assets/denoise/README.md` says; this is our own step after it: the local level is taken from the
//! unbiased samples. Over the pixels the network actually changed ("free" pixels) the input luminance and
//! the denoised luminance are blurred with the same wide box kernel, and each free pixel's colour is scaled
//! by the ratio of the two blurred levels (clamped). Where the network removed the light entirely, the
//! remainder comes back as a grey at the local mean level.

/// Largest scale a pixel's colour gets.
const MAX_RATIO: f32 = 16.0;
/// Smallest scale a pixel's colour gets.
const MIN_RATIO: f32 = 0.25;

/// Scale the free pixels of `denoised` so their local mean luminance matches the one of `input`.
///
/// `input` holds the mean colours (`w` unused), `denoised` the learned result after `keep_consistent`.
/// A pixel is free when its denoised colour differs from its input colour and both luminances are finite
/// and not negative; the others (converged pixels, misses, non-finite pixels) are left bit for bit.
/// Returns unchanged when the slices do not hold `w * h` pixels or the frame is empty.
pub(super) fn restore_energy(input: &[[f32; 4]], size: (u32, u32), denoised: &mut [[f32; 4]]) {
    let (w, h) = (size.0 as usize, size.1 as usize);
    let Some(pixels) = w.checked_mul(h) else { return };
    if pixels == 0 || input.len() != pixels || denoised.len() != pixels {
        return;
    }
    let lum = |p: &[f32; 4]| super::luminance([p[0], p[1], p[2]]);
    let mut free = vec![false; pixels];
    let mut a = vec![0.0_f64; pixels];
    let mut b = vec![0.0_f64; pixels];
    let mut m = vec![0.0_f64; pixels];
    for (i, (inp, out)) in input.iter().zip(denoised.iter()).enumerate() {
        let (li, ld) = (lum(inp), lum(out));
        let differs = inp[..3] != out[..3];
        if differs && li.is_finite() && ld.is_finite() && li >= 0.0 && ld >= 0.0 {
            if let (Some(f), Some(pa), Some(pb), Some(pm)) = (free.get_mut(i), a.get_mut(i), b.get_mut(i), m.get_mut(i)) {
                *f = true;
                *pa = f64::from(li);
                *pb = f64::from(ld);
                *pm = 1.0;
            }
        }
    }
    let sigma = (w.min(h) as f32 / 12.0).clamp(16.0, 32.0);
    let r = sigma.round() as usize;
    for plane in [&mut a, &mut b, &mut m] {
        for _ in 0..3 {
            box_blur(plane, w, h, r);
        }
    }
    for (i, out) in denoised.iter_mut().enumerate() {
        if !free.get(i).copied().unwrap_or(false) {
            continue;
        }
        let (pa, pb, pw) = (a.get(i).copied().unwrap_or(0.0), b.get(i).copied().unwrap_or(0.0), m.get(i).copied().unwrap_or(0.0));
        let q = if pb > 1e-12 && pa.is_finite() && pb.is_finite() {
            pa / pb
        } else if pa <= 0.0 {
            1.0
        } else {
            f64::INFINITY
        };
        let ratio = if q.is_nan() { 1.0 } else { (q as f32).clamp(MIN_RATIO, MAX_RATIO) };
        for c in &mut out[..3] {
            *c *= ratio;
        }
        if q > f64::from(MAX_RATIO) && pw > 0.0 {
            let grey = (pa - f64::from(MAX_RATIO) * pb) / pw;
            if grey.is_finite() && grey > 0.0 {
                for c in &mut out[..3] {
                    *c += grey as f32;
                }
            }
        }
    }
}

/// One separable box-blur pass of radius `r` (running sums, windows truncated at the border, not
/// normalised). O(`w * h`) whatever `r` is.
fn box_blur(plane: &mut [f64], w: usize, h: usize, r: usize) {
    if plane.len() != w * h {
        return;
    }
    let mut prefix = vec![0.0_f64; w.max(h) + 1];
    // Rows.
    for row in plane.chunks_exact_mut(w) {
        blur_line(row, &mut prefix, r);
    }
    // Columns.
    let mut col = vec![0.0_f64; h];
    for x in 0..w {
        for (y, c) in col.iter_mut().enumerate() {
            *c = plane.get(y * w + x).copied().unwrap_or(0.0);
        }
        blur_line(&mut col, &mut prefix, r);
        for (y, c) in col.iter().enumerate() {
            if let Some(p) = plane.get_mut(y * w + x) {
                *p = *c;
            }
        }
    }
}

/// Replace each element of `line` by the sum of the elements within `r` of it (truncated at the ends).
fn blur_line(line: &mut [f64], prefix: &mut [f64], r: usize) {
    let n = line.len();
    if prefix.len() < n + 1 {
        return;
    }
    let mut acc = 0.0_f64;
    if let Some(p0) = prefix.get_mut(0) {
        *p0 = 0.0;
    }
    for (i, v) in line.iter().enumerate() {
        acc += *v;
        if let Some(p) = prefix.get_mut(i + 1) {
            *p = acc;
        }
    }
    for (i, v) in line.iter_mut().enumerate() {
        let hi = (i + r + 1).min(n);
        let lo = i.saturating_sub(r);
        *v = prefix.get(hi).copied().unwrap_or(0.0) - prefix.get(lo).copied().unwrap_or(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rand01(state: &mut u64) -> f32 {
        *state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((*state >> 40) as f32) / (1u64 << 24) as f32
    }

    fn lum(p: &[f32; 4]) -> f64 {
        f64::from(super::super::luminance([p[0], p[1], p[2]]))
    }

    fn mean_lum(v: &[[f32; 4]]) -> f64 {
        v.iter().map(lum).sum::<f64>() / v.len().max(1) as f64
    }

    #[test]
    fn restore_energy_returns_a_uniform_loss() {
        let mut s = 7_u64;
        let input: Vec<[f32; 4]> = (0..96 * 64)
            .map(|_| {
                let v = 0.5 + rand01(&mut s);
                [v, v, v, 0.0]
            })
            .collect();
        let mut out = vec![[0.7, 0.7, 0.7, 0.0]; 96 * 64];
        restore_energy(&input, (96, 64), &mut out);
        let (mi, mo) = (mean_lum(&input), mean_lum(&out));
        assert!((mo - mi).abs() / mi < 0.02, "in {mi}, out {mo}");
        let sd = (out.iter().map(|p| (lum(p) - mo).powi(2)).sum::<f64>() / out.len() as f64).sqrt();
        assert!(sd / mo < 0.1, "sd/mean {}", sd / mo);
    }

    #[test]
    fn restore_energy_leaves_an_unbiased_result_alone() {
        let mut s = 11_u64;
        let input: Vec<[f32; 4]> = (0..96 * 64)
            .map(|_| {
                let v = 1.0 + (rand01(&mut s) - 0.5) * 0.1;
                [v, v, v, 0.0]
            })
            .collect();
        let mut out = vec![[1.0, 1.0, 1.0, 0.0]; 96 * 64];
        restore_energy(&input, (96, 64), &mut out);
        for p in &out {
            assert!((p[0] - 1.0).abs() <= 0.02, "{p:?}");
        }
    }

    #[test]
    fn restore_energy_keeps_held_pixels_bit_for_bit() {
        let (w, h) = (48usize, 32usize);
        let input: Vec<[f32; 4]> = (0..w * h).map(|i| [1.0 + (i % 7) as f32 * 0.1, 0.9, 0.8, 0.0]).collect();
        let mut out: Vec<[f32; 4]> = input.iter().map(|p| [p[0] * 0.5, p[1] * 0.5, p[2] * 0.5, 0.0]).collect();
        let mut held = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if (10..20).contains(&x) && (10..20).contains(&y) || x == 40 {
                    held[i] = true;
                    out[i] = input[i];
                }
            }
        }
        let before = out.clone();
        restore_energy(&input, (w as u32, h as u32), &mut out);
        let mut scaled = 0;
        for i in 0..w * h {
            if held[i] {
                assert!(out[i].iter().zip(&before[i]).all(|(a, b)| a.to_bits() == b.to_bits()), "pixel {i} moved");
            } else if out[i][0] > before[i][0] * 1.5 {
                scaled += 1;
            }
        }
        assert!(scaled > 100, "free pixels beside them are rescaled: {scaled}");
    }

    #[test]
    fn restore_energy_fills_light_the_network_removed() {
        let (w, h) = (96usize, 64usize);
        // One pixel in 64 on average, placed at random (a lattice with a period that divides the width
        // would leave whole columns without any).
        let mut s = 3_u64;
        let input: Vec<[f32; 4]> = (0..w * h).map(|_| if rand01(&mut s) < 1.0 / 64.0 { [64.0, 64.0, 64.0, 0.0] } else { [0.0; 4] }).collect();
        let mut out = vec![[1e-6, 1e-6, 1e-6, 0.0]; w * h];
        restore_energy(&input, (w as u32, h as u32), &mut out);
        let (mi, mo) = (mean_lum(&input), mean_lum(&out));
        assert!((mo - mi).abs() / mi < 0.1, "in {mi}, out {mo}");
    }

    #[test]
    fn restore_energy_survives_hostile_input() {
        let bad = [f32::NAN, f32::INFINITY, -3.0, 1.0];
        let mut input = vec![[1.0, 1.0, 1.0, 0.0]; 20 * 3];
        let mut out = vec![[0.5, 0.5, 0.5, 0.0]; 20 * 3];
        input[5] = [bad[0], 1.0, 1.0, 0.0];
        input[6] = [bad[1], 1.0, 1.0, 0.0];
        input[7] = [bad[2], bad[2], bad[2], 0.0];
        out[8] = [bad[0], 0.0, 0.0, 0.0];
        out[9] = [bad[1], 0.0, 0.0, 0.0];
        let before = out.clone();
        restore_energy(&input, (20, 3), &mut out);
        for i in [5, 6, 7, 8, 9] {
            assert!(out[i].iter().zip(&before[i]).all(|(a, b)| a.to_bits() == b.to_bits()), "pixel {i} changed");
        }
        restore_energy(&input, (20, 3), &mut out[..10]);
        restore_energy(&[], (0, 0), &mut []);
        restore_energy(&input, (0, 5), &mut out);
        restore_energy(&[[2.0; 4]], (1, 1), &mut [[1.0; 4]]);
        restore_energy(&input, (u32::MAX, u32::MAX), &mut out);
        let narrow = vec![[2.0, 2.0, 2.0, 0.0]; 3 * 40];
        let mut o = vec![[1.0, 1.0, 1.0, 0.0]; 3 * 40];
        restore_energy(&narrow, (3, 40), &mut o);
        assert!(o.iter().all(|p| p[0].is_finite()));
    }
}
