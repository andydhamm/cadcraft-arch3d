//! Second-order spherical harmonics of an [`Environment`] (p19-env-raster): the nine rgb
//! coefficients the raster views light diffuse surfaces with.
//!
//! [`project`] gives the radiance coefficients `L_lm` (real SH, `l <= 2`, in nits), [`convolve`]
//! turns them into the clamped-cosine convolved coefficients the GPU stores (Ramamoorthi and
//! Hanrahan: `A0 = pi`, `A1 = 2 pi / 3`, `A2 = pi / 4`) and [`irradiance`] is the illuminance in
//! lux on a surface with normal `n`. Axes are drawing space (x right, y up, z up).

use crate::environment::Environment;
use std::sync::Mutex;

/// Most texels of the grid the projection samples: 256 x 128.
const MAX_W: u32 = 256;
const MAX_H: u32 = 128;
/// Fewest: a tiny map is sampled bilinearly on a grid this fine so the quadrature stays good.
const MIN_W: u32 = 64;
const MIN_H: u32 = 32;

/// Cache for `project`, keyed by `Environment::key`.
static SH_CACHE: Mutex<Vec<(u64, [[f32; 3]; 9])>> = Mutex::new(Vec::new());
const SH_CACHE_LEN: usize = 32;

/// Count of SH projections performed (for tests).
thread_local! {
    static SH_PROJECTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many SH projections this thread has performed (not served from the cache).
pub fn sh_projection_count() -> usize {
    SH_PROJECTIONS.with(|c| c.get())
}

/// The nine real SH basis values in direction `d` (unit), in the order
/// `00, 1-1, 10, 11, 2-2, 2-1, 20, 21, 22`.
pub fn basis(d: [f32; 3]) -> [f32; 9] {
    let [x, y, z] = d;
    [
        0.282_095,
        0.488_603 * y,
        0.488_603 * z,
        0.488_603 * x,
        1.092_548 * x * y,
        1.092_548 * y * z,
        0.315_392 * (3.0 * z * z - 1.0),
        1.092_548 * x * z,
        0.546_274 * (x * x - y * y),
    ]
}

/// The clamped-cosine band factors `A_l` for each of the nine coefficients.
const BAND: [f32; 9] = {
    let pi = std::f32::consts::PI;
    let (a0, a1, a2) = (pi, 2.0 * pi / 3.0, pi / 4.0);
    [a0, a1, a1, a1, a2, a2, a2, a2, a2]
};

/// Project `env` (through [`Environment::radiance`], so rotation, horizon and intensity count)
/// onto nine SH coefficients of radiance. Texel solid angle is `(2 pi / w)(pi / h) sin(theta)`,
/// renormalised so the weights sum to `4 pi` and a uniform map is exact. An invalid map gives zeros.
///
/// The result is cached by `Environment::key`.
pub fn project(env: &Environment) -> [[f32; 3]; 9] {
    if !env.is_valid() {
        return [[0.0f32; 3]; 9];
    }

    // The caller may have mutated the environment without updating `key`; hash the content so the
    // cache tracks the actual map instead of a stale fingerprint.
    let key = env.compute_key();

    // Check cache
    if let Ok(guard) = SH_CACHE.lock() {
        if let Some((_, sh)) = guard.iter().find(|(k, _)| *k == key) {
            return *sh;
        }
    }

    // Project
    SH_PROJECTIONS.with(|c| c.set(c.get() + 1));
    let sh = project_uncached(env);

    // Store in cache
    if let Ok(mut guard) = SH_CACHE.lock() {
        if guard.len() >= SH_CACHE_LEN {
            guard.remove(0);
        }
        guard.push((key, sh));
    }

    sh
}

/// The uncached projection of an environment onto SH coefficients.
fn project_uncached(env: &Environment) -> [[f32; 3]; 9] {
    let mut out = [[0.0f32; 3]; 9];
    let gw = env.width.clamp(MIN_W, MAX_W);
    let gh = env.height.clamp(MIN_H, MAX_H);
    let pi = std::f64::consts::PI;
    let mut acc = [[0.0f64; 3]; 9];
    let mut total = 0.0f64;
    for j in 0..gh {
        let el = (0.5 - (f64::from(j) + 0.5) / f64::from(gh)) * pi;
        let weight = (2.0 * pi / f64::from(gw)) * (pi / f64::from(gh)) * el.cos();
        for i in 0..gw {
            let az = ((f64::from(i) + 0.5) / f64::from(gw) - 0.5) * 2.0 * pi;
            let d = [(el.cos() * az.cos()) as f32, (el.cos() * az.sin()) as f32, el.sin() as f32];
            let l = env.radiance(d);
            let y = basis(d);
            total += weight;
            for (a, yk) in acc.iter_mut().zip(y) {
                for (c, lc) in a.iter_mut().zip(l) {
                    *c += weight * f64::from(lc) * f64::from(yk);
                }
            }
        }
    }
    if !(total > 0.0) {
        return out;
    }
    let scale = 4.0 * pi / total;
    for (o, a) in out.iter_mut().zip(acc) {
        for (oc, ac) in o.iter_mut().zip(a) {
            let v = (ac * scale) as f32;
            *oc = if v.is_finite() { v } else { 0.0 };
        }
    }
    out
}

/// The radiance coefficients `sh` multiplied by the clamped-cosine band factors: what the GPU
/// stores, so the shader only sums `coefficient * basis`.
pub fn convolve(sh: &[[f32; 3]; 9]) -> [[f32; 3]; 9] {
    let mut out = *sh;
    for (o, a) in out.iter_mut().zip(BAND) {
        for c in o.iter_mut() {
            *c *= a;
        }
    }
    out
}

/// Illuminance (lux, per channel) on a diffuse surface with unit normal `n`, from the radiance
/// coefficients `sh` of [`project`]. A non-finite `n` gives zero.
pub fn irradiance(sh: &[[f32; 3]; 9], n: [f32; 3]) -> [f32; 3] {
    if !n.iter().all(|v| v.is_finite()) {
        return [0.0; 3];
    }
    let y = basis(n);
    let mut e = [0.0f32; 3];
    for ((c, a), yk) in sh.iter().zip(BAND).zip(y) {
        for (ek, ck) in e.iter_mut().zip(c) {
            *ek += ck * a * yk;
        }
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn reset_sh_cache_for_test() {
        let mut cache = SH_CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.clear();
        SH_PROJECTIONS.with(|c| c.set(0));
    }

    fn dirs() -> Vec<[f32; 3]> {
        let mut v = vec![[0.0, 0.0, 1.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]];
        let r = 0.5f32.sqrt();
        v.extend([[r, r, 0.0], [r, 0.0, r], [0.0, -r, -r]]);
        v
    }

    #[test]
    fn uniform_upper_hemisphere_and_rotation() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_sh_cache_for_test();

        let pi = std::f32::consts::PI;
        // A uniform L gives pi L for every normal.
        let uniform = Environment::uniform([2.0, 1.0, 0.5], 32, 16);
        let sh = project(&uniform);
        for n in dirs() {
            let e = irradiance(&sh, n);
            for (got, l) in e.iter().zip([2.0f32, 1.0, 0.5]) {
                assert!((got - pi * l).abs() <= pi * l * 0.005, "{n:?}: {e:?}");
            }
        }
        // Light only above the horizon: pi up, nothing down.
        let (w, h) = (128u32, 64u32);
        let mut up = Environment::uniform([0.0; 3], w, h);
        for row in 0..h / 2 {
            for col in 0..w {
                up.rgb[(row * w + col) as usize] = [1.0; 3];
            }
        }
        let sh = project(&up);
        let top = irradiance(&sh, [0.0, 0.0, 1.0])[0];
        assert!((top - pi).abs() < pi * 0.02, "up {top}");
        assert!(irradiance(&sh, [0.0, 0.0, -1.0])[0].abs() < 0.1 * pi, "down");
        // Rotation about +Z: a bright patch toward +x moves to +y at 90 degrees.
        let mut patch = Environment::uniform([0.0; 3], 64, 32);
        for row in 8..24 {
            for col in 28..36 {
                patch.rgb[row * 64 + col] = [10.0; 3];
            }
        }
        let before = project(&patch);
        assert!(irradiance(&before, [1.0, 0.0, 0.0])[0] > 3.0 * irradiance(&before, [0.0, 1.0, 0.0])[0]);
        patch.rotation_deg = 90.0;
        let after = project(&patch);
        assert!(irradiance(&after, [0.0, 1.0, 0.0])[0] > 3.0 * irradiance(&after, [1.0, 0.0, 0.0])[0]);
        // Intensity scales it; an invalid map and a hostile normal give zero.
        patch.intensity = 2.0;
        patch.rotation_deg = 0.0;
        let doubled = irradiance(&project(&patch), [1.0, 0.0, 0.0])[0];
        assert!((doubled - 2.0 * irradiance(&before, [1.0, 0.0, 0.0])[0]).abs() < 1e-3 * doubled.abs().max(1.0));
        assert_eq!(project(&Environment::default()), [[0.0; 3]; 9]);
        assert_eq!(irradiance(&before, [f32::NAN, 0.0, 1.0]), [0.0; 3]);
    }

    #[test]
    fn second_sky_render_of_an_angle_rebuilds_nothing() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_sh_cache_for_test();

        // Cache by the actual environment content, not by the caller's stale key field.
        let mut e = Environment::uniform([0.731, 0.419, 0.277], 17, 9);
        e.key = 9999; // stale mirror of an older key

        let before = sh_projection_count();

        let _sh1 = project(&e);
        assert_eq!(sh_projection_count(), before + 1, "First projection must run");

        let _sh2 = project(&e);
        assert_eq!(sh_projection_count(), before + 1, "Second projection with the same content must hit cache");

        // A different map is different work even if the old key field says otherwise.
        let mut e2 = e.clone();
        e2.rgb[0] = [2.0, 2.0, 2.0];
        e2.key = 8888;
        let _sh3 = project(&e2);
        assert_eq!(sh_projection_count(), before + 2, "Different content must project");
    }
}
