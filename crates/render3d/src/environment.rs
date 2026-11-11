//! Environment light in the path tracer (p19-env-path): an `Environment` map the path tracer
//! importance-samples with MIS, used as light and/or as the picture background.
//!
//! Linear nits, row 0 is the zenith. `u = 0.5 + atan2(d.y, d.x)/(2 pi)`,
//! `v = 0.5 - asin(d.z)/pi` (the backdrop.wgsl convention). `rotation_deg` rotates the map
//! counter-clockwise about `+Z`; a positive `horizon_deg` raises the map's horizon.
//! `visible` means the environment also paints the picture background (a Sky/HDRI backdrop)
//! as well as lighting; otherwise it is light only.

/// A square environment map the path tracer importance-samples.
#[derive(Clone, Debug)]
pub struct Environment {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<[f32; 3]>,
    pub intensity: f32,
    pub rotation_deg: f32,
    pub horizon_deg: f32,
    pub visible: bool,
    pub daylight: bool,
    pub key: u64,
}

impl Default for Environment {
    fn default() -> Self {
        Self { width: 0, height: 0, rgb: Vec::new(), intensity: 0.0, rotation_deg: 0.0, horizon_deg: 0.0, visible: false, daylight: false, key: 0 }
    }
}

impl Environment {
    /// Every texel filled with `rgb` at full intensity, visible (so it doubles as background).
    pub fn uniform(rgb: [f32; 3], w: u32, h: u32) -> Self {
        let mut e = Self::default();
        e.width = w;
        e.height = h;
        e.rgb = vec![rgb; w as usize * h as usize];
        e.intensity = 1.0;
        e.visible = true;
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
        self.intensity.is_finite() && self.rotation_deg.is_finite() && self.horizon_deg.is_finite() && self.rgb.iter().all(|c| c.iter().all(ok))
    }

    /// Radiance in direction `d` (undo rotation, lift the horizon, bilinear, scale by
    /// `intensity`). Zero when the map is invalid or `d` is not finite.
    pub fn radiance(&self, d: [f32; 3]) -> [f32; 3] {
        if !self.is_valid() || !d.iter().all(|v| v.is_finite()) {
            return [0.0; 3];
        }
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if !(len.is_finite() && len > 0.0) {
            return [0.0; 3];
        }
        let d = [d[0] / len, d[1] / len, d[2] / len];

        let rad = std::f32::consts::PI / 180.0;
        // Undo the counter-clockwise rotation about +Z.
        let a = -self.rotation_deg * rad;
        let x = d[0] * a.cos() - d[1] * a.sin();
        let y = d[0] * a.sin() + d[1] * a.cos();
        // Raise the map's horizon: read the map at elevation minus the shift.
        let dz = (d[2] - (self.horizon_deg * rad).sin()).clamp(-1.0, 1.0);

        // u wraps, v clamps.
        let u_full = (0.5 + y.atan2(x) / (2.0 * std::f32::consts::PI)) * self.width as f32;
        let u0 = u_full.floor() as i32;
        let fu = u_full.fract();
        let u0 = u0.rem_euclid(self.width as i32);
        let u1 = (u0 + 1).rem_euclid(self.width as i32);

        let v_full = 0.5 - dz.asin() / std::f32::consts::PI;
        let v0 = v_full * self.height as f32;
        let fv = v_full.fract().clamp(0.0, 1.0);
        let v0 = v0.clamp(0.0, self.height as f32 - 1.0);
        let v0i = v0 as i32;
        let v1i = (v0 + 1.0) as i32;
        let v0i = v0i.clamp(0, self.height as i32 - 1) as usize;
        let v1i = v1i.clamp(0, self.height as i32 - 1) as usize;

        let at = |i: usize, u: i32| -> [f32; 3] {
            let uu = (u as i64 % self.width as i64) as usize;
            self.rgb[i * self.width as usize + uu]
        };
        let p00 = at(v0i, u0);
        let p01 = at(v0i, u1);
        let p10 = at(v1i, u0);
        let p11 = at(v1i, u1);
        let lerp3 = |a: [f32; 3], b: [f32; 3], t: f32| [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]), a[2] + t * (b[2] - a[2])];
        let top = lerp3(p00, p01, fu);
        let bottom = lerp3(p10, p11, fu);
        let col = lerp3(top, bottom, fv);
        let i = self.intensity;
        [col[0] * i, col[1] * i, col[2] * i]
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
        push(if self.visible { 1u64 } else { 0u64 });
        push(if self.daylight { 1u64 } else { 0u64 });
        h
    }

    /// [p19-hdri] An environment built from the look's HDRI backdrop. Stub: not available yet.
    pub fn hdri_environment(_model: &cadcraft_doc::arch::ArchModel, _look: &cadcraft_doc::arch::Look) -> Option<Environment> {
        None
    }

    /// [p19-sky] The Preetham sky for the given sun. Stub: not available yet.
    pub fn sky_environment(
        _model: &cadcraft_doc::arch::ArchModel,
        _look: &cadcraft_doc::arch::Look,
        _sun_dir: Option<[f64; 3]>,
    ) -> Option<Environment> {
        None
    }
}

/// The one environment for the drawing and look: an HDRI (if the look names one),
/// else the Preetham sky (if the look asks for it), else nothing.
pub fn environment_for(
    model: &cadcraft_doc::arch::ArchModel,
    look: &cadcraft_doc::arch::Look,
    sun_dir: Option<[f64; 3]>,
) -> Option<std::sync::Arc<Environment>> {
    let env = Environment::hdri_environment(model, look).or_else(|| Environment::sky_environment(model, look, sun_dir));
    env.filter(|e| e.is_valid()).map(std::sync::Arc::new)
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
