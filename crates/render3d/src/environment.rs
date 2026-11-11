//! Environment light in the path tracer (p19-env-path): an `Environment` map the path tracer
//! importance-samples with MIS, used as light and/or as the picture background.
//!
//! Linear nits, row 0 is the zenith. `u = 0.5 + atan2(d.y, d.x)/(2 pi)`,
//! `v = 0.5 - asin(d.z)/pi` (the backdrop.wgsl convention). `rotation_deg` rotates the map
//! counter-clockwise about `+Z`; a positive `horizon_deg` raises the map's horizon.
//! `visible` means the environment also paints the picture background (a Sky/HDRI backdrop)
//! as well as lighting; otherwise it is light only.

use cadcraft_doc::arch::{ArchModel, Look};
use std::sync::Arc;

/// An equirect environment map the path tracer importance-samples.
#[derive(Clone, Debug, Default)]
pub struct Environment {
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
    /// Linear nits, row 0 is the zenith.
    pub rgb: Vec<[f32; 3]>,
    /// Scale applied to every texel.
    pub intensity: f32,
    /// Counter-clockwise rotation about +Z, degrees.
    pub rotation_deg: f32,
    /// Degrees the map's horizon is raised (negative lowers it).
    pub horizon_deg: f32,
    /// Also the picture background (a sky/HDRI backdrop), not only light.
    pub visible: bool,
    /// The environment is daylight (exposure treats it like a sun).
    pub daylight: bool,
    /// Fingerprint of the map (set by the producer; see `compute_key`).
    pub key: u64,
}

impl Environment {
    /// Every texel filled with `rgb` at full intensity, visible (so it doubles as background).
    pub fn uniform(rgb: [f32; 3], w: u32, h: u32) -> Self {
        let mut e = Self {
            width: w,
            height: h,
            rgb: vec![rgb; (w as usize).saturating_mul(h as usize).min(1 << 24)],
            intensity: 1.0,
            visible: true,
            ..Default::default()
        };
        e.key = e.compute_key();
        e
    }

    /// Legal size, correct pixel count, finite non-negative radiance everywhere.
    pub fn is_valid(&self) -> bool {
        let valid_w = (1..=4096).contains(&self.width);
        let valid_h = (1..=2048).contains(&self.height);
        let len_ok = self.rgb.len() == self.width as usize * self.height as usize;
        if !(valid_w && valid_h && len_ok) {
            return false;
        }
        let ok = |v: &f32| v.is_finite() && *v >= 0.0;
        self.intensity.is_finite()
            && self.intensity >= 0.0
            && self.rotation_deg.is_finite()
            && self.horizon_deg.is_finite()
            && self.rgb.iter().all(|c| c.iter().all(ok))
    }

    /// Radiance in direction `d` (undo the rotation, shift the elevation by `-horizon_deg`,
    /// bilinear with `u` wrapping and `v` clamping, scale by `intensity`). Zero when the map is
    /// invalid or `d` is not finite.
    pub fn radiance(&self, d: [f32; 3]) -> [f32; 3] {
        if !self.is_valid() || !d.iter().all(|v| v.is_finite()) {
            return [0.0; 3];
        }
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if !(len.is_finite() && len > 0.0) {
            return [0.0; 3];
        }
        let d = [d[0] / len, d[1] / len, d[2] / len];
        let pi = std::f32::consts::PI;
        let az = d[1].atan2(d[0]) - self.rotation_deg.to_radians();
        let el = (d[2].clamp(-1.0, 1.0).asin() - self.horizon_deg.to_radians()).clamp(-pi / 2.0, pi / 2.0);
        let (w, h) = (self.width as usize, self.height as usize);
        // Texel centres sit at +0.5.
        let fu = (0.5 + az / (2.0 * pi)) * w as f32 - 0.5;
        let fv = ((0.5 - el / pi) * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
        let u0f = fu.floor();
        let tu = fu - u0f;
        let u0 = (u0f as i64).rem_euclid(w as i64) as usize;
        let u1 = (u0 + 1) % w;
        let v0 = (fv.floor() as usize).min(h - 1);
        let v1 = (v0 + 1).min(h - 1);
        let tv = fv - v0 as f32;
        let at = |row: usize, col: usize| self.rgb.get(row * w + col).copied().unwrap_or([0.0; 3]);
        let lerp = |a: [f32; 3], b: [f32; 3], t: f32| [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]), a[2] + t * (b[2] - a[2])];
        let col = lerp(lerp(at(v0, u0), at(v0, u1), tu), lerp(at(v1, u0), at(v1, u1), tu), tv);
        let out = [col[0] * self.intensity, col[1] * self.intensity, col[2] * self.intensity];
        if out.iter().all(|v| v.is_finite()) { out } else { [0.0; 3] }
    }

    /// A stable fingerprint of the map parameters (never the pixels), so `PartialEq` is cheap
    /// and the pane can compare `Lighting` every frame.
    pub fn compute_key(&self) -> u64 {
        let mut h: u64 = 0x811c9dc5;
        let mut push = |x: u64| h = (h ^ x).wrapping_mul(0x01000193);
        push(self.width as u64);
        push(self.height as u64);
        push(self.intensity.to_bits() as u64);
        push(self.rotation_deg.to_bits() as u64);
        push(self.horizon_deg.to_bits() as u64);
        push(u64::from(self.visible));
        push(u64::from(self.daylight));
        for c in &self.rgb {
            for v in c {
                push(v.to_bits() as u64);
            }
        }
        h
    }
}

/// [p19-hdri] An environment built from the look's HDRI backdrop. Stub: not available yet.
pub fn hdri_environment(_model: &ArchModel, _look: &Look) -> Option<Environment> {
    None
}

/// [p19-sky] The sky environment for the given sun. Stub: not available yet.
pub fn sky_environment(_model: &ArchModel, _look: &Look, _sun_dir: Option<[f64; 3]>) -> Option<Environment> {
    None
}

/// The one environment for the drawing and look: an HDRI (if the look names one),
/// else the sky (if the look asks for it), else nothing.
pub fn environment_for(model: &ArchModel, look: &Look, sun_dir: Option<[f64; 3]>) -> Option<Arc<Environment>> {
    let env = hdri_environment(model, look).or_else(|| sky_environment(model, look, sun_dir));
    env.filter(|e| e.is_valid()).map(Arc::new)
}

// Compare the map's parameters and fingerprint, never its pixels.
impl std::cmp::PartialEq for Environment {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width
            && self.height == other.height
            && self.intensity == other.intensity
            && self.rotation_deg == other.rotation_deg
            && self.horizon_deg == other.horizon_deg
            && self.visible == other.visible
            && self.daylight == other.daylight
            && self.key == other.key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radiance_applies_rotation_and_horizon() {
        // 9 x 4 map: one bright texel (column 4, row 1): azimuth 0 (+x), elevation 22.5 degrees.
        let mut e = Environment::uniform([0.0; 3], 9, 4);
        e.rgb[9 + 4] = [10.0, 10.0, 10.0];
        let el = 22.5f32.to_radians();
        let dir = |az_deg: f32, el: f32| {
            let a = az_deg.to_radians();
            [el.cos() * a.cos(), el.cos() * a.sin(), el.sin()]
        };
        let at = |e: &Environment, az: f32, el: f32| e.radiance(dir(az, el))[0];
        assert!((at(&e, 0.0, el) - 10.0).abs() < 1e-3);
        assert!(at(&e, 90.0, el) < 1e-3);
        // Rotating counter-clockwise by 90 degrees moves the bright texel to +y.
        e.rotation_deg = 90.0;
        assert!((at(&e, 90.0, el) - 10.0).abs() < 1e-3);
        assert!(at(&e, 0.0, el) < 1e-3);
        // Raising the horizon by 30 degrees lifts it to elevation 52.5.
        e.rotation_deg = 0.0;
        e.horizon_deg = 30.0;
        assert!((at(&e, 0.0, el + 30f32.to_radians()) - 10.0).abs() < 1e-3);
        assert!(at(&e, 0.0, el - 40f32.to_radians()) < 1e-3);
        // Intensity scales, invalid or non-finite input gives zero.
        e.horizon_deg = 0.0;
        e.intensity = 2.0;
        assert!((at(&e, 0.0, el) - 20.0).abs() < 1e-2);
        assert_eq!(e.radiance([f32::NAN, 0.0, 1.0]), [0.0; 3]);
        assert_eq!(Environment::default().radiance([0.0, 0.0, 1.0]), [0.0; 3]);
    }
}
