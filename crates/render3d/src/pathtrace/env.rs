//! The environment map as a sampling table for the path tracer (p19-env-path): world-space
//! radiance baked at texel centres plus the marginal and conditional CDFs the kernel
//! binary-searches (kernel `env_sample` / `env_pdf` in `lights.wgsl`). The CPU `sample` and `pdf`
//! here are the reference the tests check the maths with.
//!
//! Importance is luminance x sin(theta) (theta from the zenith) so the density over the sphere
//! follows the radiance. The table is piecewise constant: a direction maps to one texel, and the
//! radiance, the sampling and the pdf all read that texel.

use crate::environment::Environment;
use std::sync::Mutex;

/// Largest table the path tracer bakes.
pub const MAX_TABLE_W: u32 = 1024;
/// Largest table height.
pub const MAX_TABLE_H: u32 = 512;

/// A baked environment: `radiance` is 3 floats per texel (row 0 the zenith), `marginal` the
/// cumulative row distribution (`height + 1`, 0 to 1), `conditional` each row's cumulative
/// column distribution (`height * (width + 1)`, 0 to 1 per row), `total` the summed importance.
#[derive(Clone, Debug, Default)]
pub struct EnvTable {
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
    /// World-space radiance in nits, rgb per texel.
    pub radiance: Vec<f32>,
    /// Marginal CDF over rows.
    pub marginal: Vec<f32>,
    /// Conditional CDF over columns, one row of `width + 1` per texel row.
    pub conditional: Vec<f32>,
    /// Summed importance (zero: nothing to sample).
    pub total: f32,
}

/// Cache for `env_table`, keyed by `Environment::key`.
/// Stores (key, table) pairs. Bounded to avoid memory bloat.
static ENV_TABLE_CACHE: Mutex<Vec<(u64, EnvTable)>> = Mutex::new(Vec::new());
const ENV_TABLE_CACHE_LEN: usize = 8;

/// Count of table bakes performed (for tests).
thread_local! {
    static ENV_TABLE_BAKES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many env tables this thread has baked (not served from the cache).
pub fn env_table_bake_count() -> usize {
    ENV_TABLE_BAKES.with(|c| c.get())
}

fn luminance(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// Bake `e` (rotation, horizon and intensity applied) into a sampling table of at most
/// 1024 x 512 texels. An invalid environment gives an empty table (`total` 0).
///
/// The result is cached by `Environment::key`. A cache hit returns a clone of the stored table.
pub fn env_table(e: &Environment) -> EnvTable {
    if !e.is_valid() {
        return EnvTable::default();
    }

    // The caller may mutate `rgb`, `rotation_deg`, or `intensity` without refreshing the stored
    // fingerprint. Hash the current content so the table cache stays keyed by what it represents.
    let key = e.compute_key();

    // Check cache
    if let Ok(guard) = ENV_TABLE_CACHE.lock() {
        if let Some((_, table)) = guard.iter().find(|(k, _)| *k == key) {
            return table.clone();
        }
    }

    // Bake
    ENV_TABLE_BAKES.with(|c| c.set(c.get() + 1));
    let table = bake_env_table(e);

    // Store in cache
    if let Ok(mut guard) = ENV_TABLE_CACHE.lock() {
        if guard.len() >= ENV_TABLE_CACHE_LEN {
            guard.remove(0);
        }
        guard.push((key, table.clone()));
    }

    table
}

/// The uncached bake of an environment table.
fn bake_env_table(e: &Environment) -> EnvTable {
    let w = e.width.min(MAX_TABLE_W) as usize;
    let h = e.height.min(MAX_TABLE_H) as usize;
    let pi = std::f32::consts::PI;
    let mut radiance = Vec::with_capacity(w * h * 3);
    let mut weight = vec![0.0f64; w * h];
    for row in 0..h {
        let theta = (row as f32 + 0.5) / h as f32 * pi;
        let (sin_t, cos_t) = theta.sin_cos();
        for col in 0..w {
            let az = ((col as f32 + 0.5) / w as f32 - 0.5) * 2.0 * pi;
            let c = e.radiance([sin_t * az.cos(), sin_t * az.sin(), cos_t]);
            radiance.extend_from_slice(&c);
            if let Some(slot) = weight.get_mut(row * w + col) {
                *slot = f64::from(luminance(c) * sin_t);
            }
        }
    }
    let mut marginal = Vec::with_capacity(h + 1);
    let mut conditional = Vec::with_capacity(h * (w + 1));
    let mut row_sums = Vec::with_capacity(h);
    for row in 0..h {
        let r = weight.get(row * w..(row + 1) * w).unwrap_or(&[]);
        let sum: f64 = r.iter().sum();
        row_sums.push(sum);
        let mut acc = 0.0f64;
        conditional.push(0.0);
        for (col, x) in r.iter().enumerate() {
            acc += x;
            // An empty row gets a flat ramp (never chosen: its marginal slot is empty).
            let t = if sum > 0.0 { acc / sum } else { (col + 1) as f64 / w as f64 };
            conditional.push(t as f32);
        }
        if let Some(last) = conditional.last_mut() {
            *last = 1.0;
        }
    }
    let total: f64 = row_sums.iter().sum();
    let mut acc = 0.0f64;
    marginal.push(0.0);
    for (row, s) in row_sums.iter().enumerate() {
        acc += s;
        let t = if total > 0.0 { acc / total } else { (row + 1) as f64 / h as f64 };
        marginal.push(t as f32);
    }
    if let Some(last) = marginal.last_mut() {
        *last = 1.0;
    }
    EnvTable { width: w as u32, height: h as u32, radiance, marginal, conditional, total: total as f32 }
}

/// The largest index `i` in `0..n` with `cdf[base + i] <= u` (a CDF of `n + 1` entries).
fn search(cdf: &[f32], base: usize, n: usize, u: f32) -> usize {
    let (mut lo, mut hi) = (0usize, n);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if cdf.get(base + mid).is_some_and(|c| *c <= u) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo.min(n.saturating_sub(1))
}

impl EnvTable {
    /// Whether there is anything to sample.
    pub fn is_usable(&self) -> bool {
        self.width > 0 && self.height > 0 && self.total > 0.0 && self.total.is_finite()
    }

    /// The direction for the unit square sample `u` and its density per steradian.
    pub fn sample(&self, u: [f32; 2]) -> ([f32; 3], f32) {
        if !self.is_usable() {
            return ([0.0, 0.0, 1.0], 0.0);
        }
        let (w, h) = (self.width as usize, self.height as usize);
        let u0 = u[0].clamp(0.0, 1.0);
        let u1 = u[1].clamp(0.0, 1.0);
        let row = search(&self.marginal, 0, h, u0);
        let m0 = self.marginal.get(row).copied().unwrap_or(0.0);
        let m1 = self.marginal.get(row + 1).copied().unwrap_or(1.0);
        let fv = if m1 > m0 { ((u0 - m0) / (m1 - m0)).clamp(0.0, 1.0) } else { 0.5 };
        let base = row * (w + 1);
        let col = search(&self.conditional, base, w, u1);
        let c0 = self.conditional.get(base + col).copied().unwrap_or(0.0);
        let c1 = self.conditional.get(base + col + 1).copied().unwrap_or(1.0);
        let fu = if c1 > c0 { ((u1 - c0) / (c1 - c0)).clamp(0.0, 1.0) } else { 0.5 };
        let uu = (col as f32 + fu) / w as f32;
        let vv = (row as f32 + fv) / h as f32;
        let dir = dir_of_uv(uu, vv);
        (dir, self.pdf(dir))
    }

    /// The density per steradian of sampling `dir` (zero when it cannot be sampled).
    pub fn pdf(&self, dir: [f32; 3]) -> f32 {
        if !self.is_usable() || !dir.iter().all(|v| v.is_finite()) {
            return 0.0;
        }
        let (w, h) = (self.width as usize, self.height as usize);
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        if !(len > 0.0) {
            return 0.0;
        }
        let pi = std::f32::consts::PI;
        let (u, v) = uv_of_dir([dir[0] / len, dir[1] / len, dir[2] / len]);
        let col = ((u * w as f32).floor().max(0.0) as usize).min(w - 1);
        let row = ((v * h as f32).floor().max(0.0) as usize).min(h - 1);
        let m = self.marginal.get(row + 1).copied().unwrap_or(0.0) - self.marginal.get(row).copied().unwrap_or(0.0);
        let base = row * (w + 1);
        let c = self.conditional.get(base + col + 1).copied().unwrap_or(0.0) - self.conditional.get(base + col).copied().unwrap_or(0.0);
        let pdf_uv = m * c * (w * h) as f32;
        let sin_t = (v * pi).sin();
        if !(sin_t > 1e-6) {
            return 0.0;
        }
        pdf_uv / (2.0 * pi * pi * sin_t)
    }

    /// The baked radiance of the texel `dir` falls in.
    pub fn lookup(&self, dir: [f32; 3]) -> [f32; 3] {
        if self.width == 0 || self.height == 0 {
            return [0.0; 3];
        }
        let (w, h) = (self.width as usize, self.height as usize);
        let (u, v) = uv_of_dir(dir);
        let col = ((u * w as f32).floor().max(0.0) as usize).min(w - 1);
        let row = ((v * h as f32).floor().max(0.0) as usize).min(h - 1);
        let i = (row * w + col) * 3;
        match self.radiance.get(i..i + 3) {
            Some(c) => [c[0], c[1], c[2]],
            None => [0.0; 3],
        }
    }

    /// The floats to append to the aux buffer: radiance, marginal, then conditional. The offsets
    /// (relative to the start) are `0`, `radiance.len()` and `radiance.len() + marginal.len()`.
    pub fn packed(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.radiance.len() + self.marginal.len() + self.conditional.len());
        out.extend_from_slice(&self.radiance);
        out.extend_from_slice(&self.marginal);
        out.extend_from_slice(&self.conditional);
        out
    }
}

/// The unit direction at equirect (u, v): `u = 0.5 + atan2(y, x)/(2 pi)`, `v = 0.5 - asin(z)/pi`.
pub fn dir_of_uv(u: f32, v: f32) -> [f32; 3] {
    let pi = std::f32::consts::PI;
    let az = (u - 0.5) * 2.0 * pi;
    let el = (0.5 - v) * pi;
    [el.cos() * az.cos(), el.cos() * az.sin(), el.sin()]
}

/// The equirect (u, v) of a unit direction, both in 0..=1.
pub fn uv_of_dir(d: [f32; 3]) -> (f32, f32) {
    let pi = std::f32::consts::PI;
    let u = (0.5 + d[1].atan2(d[0]) / (2.0 * pi)).clamp(0.0, 1.0);
    let v = (0.5 - d[2].clamp(-1.0, 1.0).asin() / pi).clamp(0.0, 1.0);
    (u, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny deterministic generator in [0, 1).
    fn rng(state: &mut u64) -> f32 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*state >> 40) as f32) / (1u64 << 24) as f32
    }

    #[test]
    fn pdf_integrates_to_one_and_finds_bright_texel() {
        // A gradient map: the pdf over the sphere integrates to 1.
        let (w, h) = (32usize, 16usize);
        let mut e = Environment::uniform([1.0; 3], w as u32, h as u32);
        for (i, c) in e.rgb.iter_mut().enumerate() {
            let v = 0.2 + (i % 7) as f32 + (i / w) as f32 * 0.5;
            *c = [v, v * 0.5, v * 0.25];
        }
        let t = env_table(&e);
        assert!(t.is_usable());
        let (n_az, n_el) = (512usize, 256usize);
        let mut sum = 0.0f64;
        for j in 0..n_el {
            let el = (0.5 - (j as f32 + 0.5) / n_el as f32) * std::f32::consts::PI;
            for i in 0..n_az {
                let az = ((i as f32 + 0.5) / n_az as f32 - 0.5) * 2.0 * std::f32::consts::PI;
                let d = [el.cos() * az.cos(), el.cos() * az.sin(), el.sin()];
                sum += f64::from(t.pdf(d)) * f64::from(el.cos());
            }
        }
        let d_area = (2.0 * std::f64::consts::PI / n_az as f64) * (std::f64::consts::PI / n_el as f64);
        let total = sum * d_area;
        assert!((total - 1.0).abs() < 0.01, "pdf integrates to {total}");

        // Sampled directions agree with their pdf, and one bright texel wins >= 90 % of them.
        let mut e = Environment::uniform([1.0; 3], 16, 8);
        e.rgb[3 * 16 + 5] = [10_000.0; 3];
        let t = env_table(&e);
        let mut state = 7u64;
        let (mut hit, n) = (0usize, 10_000usize);
        for _ in 0..n {
            let (d, pdf) = t.sample([rng(&mut state), rng(&mut state)]);
            assert!(pdf > 0.0 && (pdf - t.pdf(d)).abs() <= 1e-3 * pdf);
            let (u, v) = uv_of_dir(d);
            if (u * 16.0) as usize == 5 && (v * 8.0) as usize == 3 {
                hit += 1;
            }
        }
        assert!(hit * 10 >= n * 9, "{hit} of {n} samples in the bright texel");
    }

    #[test]
    fn sky_bake_and_tables_are_built_once_per_sun_angle() {
        // This test verifies that the environment table is cached.
        // We create a valid environment with a specific key.
        let mut e = Environment::uniform([1.0, 1.0, 1.0], 16, 8);
        e.key = 12345; // Force a specific key for testing

        let before = env_table_bake_count();

        // First call should bake
        let _t1 = env_table(&e);
        assert_eq!(env_table_bake_count(), before + 1, "First call must bake");

        // Second call with same key should hit cache
        let _t2 = env_table(&e);
        assert_eq!(env_table_bake_count(), before + 1, "Second call must hit cache");

        // Change key (simulating different sun/look)
        let mut e2 = e.clone();
        e2.key = 67890;

        let _t3 = env_table(&e2);
        assert_eq!(env_table_bake_count(), before + 2, "Different key must bake");

        // Back to original key
        let _t4 = env_table(&e);
        assert_eq!(env_table_bake_count(), before + 2, "Original key must hit cache again");
    }
}
