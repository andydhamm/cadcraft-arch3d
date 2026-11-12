//! The per-frame look of the raster views (phase 19 groundwork): everything a frame decides once
//! (exposure, tone map, ambient, clip box, near plane, Lambert override, the directional light's
//! lux) and the per-draw fill that copies it into each draw's [`PbrUniform`].
//!
//! The default [`FrameLook`] is the pre-phase-19 look: [`fill_draw`] then leaves every draw's block
//! exactly as [`PbrUniform::new`] made it. Later packages fill the stubs below, each labelled with
//! the package that owns it; none of them changes a layout or a signature.

use cadcraft_arch3d::light::{direct_illuminance, is_light_material};
use cadcraft_arch3d::pick::Bvh;
use cadcraft_geom::Vec3;

use crate::camera::{Camera, ray_at};
use crate::exposure::{daylight, exposure_scale, tone_code};
use crate::lit_frame::Lighting;
use crate::pathtrace::luminance;
use crate::pipeline::{PBR_FLAG_CLIP, PBR_FLAG_LAMBERT, PbrUniform, STYLE_HIDDEN_LINE, STYLE_TECHNICAL};
use crate::types::{ArchId, Mesh, Scene3d};

/// What one frame looks like, decided once and written into every draw.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameLook {
    /// Exposure scale (0 = unset: the pipeline's own).
    pub exposure: f32,
    /// Tone-map code (0 = the pipeline's own; see `PbrUniform::tone_map`).
    pub tone_map: u32,
    /// Ambient illuminance in lux (0 = unset).
    pub ambient_lux: f32,
    /// `PBR_FLAG_*` bits for every draw.
    pub flags: u32,
    /// Clip box `[min, max]` in drawing space, when the view is clipped.
    pub clip: Option<[[f32; 3]; 2]>,
    /// Near plane in metres, when the view sets one.
    pub near: Option<f32>,
    /// Lambert albedo override, when the view draws a plain Lambert look.
    pub lambert: Option<f32>,
    /// Lux of the raster's directional light (0 = none: at night it stays 0).
    pub sun_lux: f32,
    /// How the exposure was chosen: `"auto"`, `"manual"` or `"fixed"` (empty for an unlit frame).
    pub exposure_mode: &'static str,
}

/// Rays the exposure meter casts across the frame.
const METER_COLUMNS: u32 = 32;
const METER_ROWS: u32 = 24;
/// Most fixtures the meter adds up per ray.
const METER_MAX_LIGHTS: usize = 256;

/// What exposure metering remembers between frames: the BVH of the scene (and the ground quad)
/// it casts rays against, rebuilt only when the scene or the ground flag changes.
#[derive(Default)]
pub struct Meter {
    /// The scene key the frame is drawn under (`Offscreen::encode_with` sets it before each frame).
    pub scene_key: u64,
    /// What the BVH below was built for: scene key, ground, mesh count, scene revision.
    key: Option<(u64, bool, usize, u64)>,
    bvh: Option<Bvh>,
    /// Per BVH object id: the index of the scene mesh, or `GROUND` / `HORIZON` for the extras.
    sources: Vec<usize>,
    ground: Option<Mesh>,
}

const GROUND: usize = usize::MAX;

impl Meter {
    /// Build the BVH for `scene` (housings left out), plus the ground quad when `ground` and the
    /// scene's horizon slab.
    fn prepare(&mut self, scene: &Scene3d, ground: bool) {
        let key = (self.scene_key, ground, scene.meshes.len(), scene.revision);
        if self.key == Some(key) {
            return;
        }
        let mut meshes: Vec<Mesh> = Vec::new();
        self.sources.clear();
        self.ground = None;
        let usable = |m: &Mesh| !m.positions.is_empty() && !m.indices.is_empty();
        let push = |source: usize, m: &Mesh, meshes: &mut Vec<Mesh>, sources: &mut Vec<usize>| {
            let Ok(id) = u32::try_from(sources.len()) else { return };
            // Every mesh gets its own object id, so a hit says which mesh it struck.
            let mut c = m.clone();
            c.object = ArchId(id);
            meshes.push(c);
            sources.push(source);
        };
        for (i, m) in scene.meshes.iter().enumerate() {
            if usable(m) && !is_light_material(&m.material) {
                push(i, m, &mut meshes, &mut self.sources);
            }
        }
        if ground {
            let g = crate::shadow::ground_mesh(&scene.bounds);
            if usable(&g) {
                push(GROUND, &g, &mut meshes, &mut self.sources);
                self.ground = Some(g);
            }
        }
        if let Some(h) = scene.horizon.as_ref().filter(|h| usable(h)) {
            push(GROUND - 1, h, &mut meshes, &mut self.sources);
        }
        self.bvh = Some(Bvh::build(&Scene3d { meshes, ..Default::default() }));
        self.key = Some(key);
    }

    /// Log-average luminance (nits) over a grid of rays through the frame's pixel centres, for a
    /// frame lit as `f` and `lighting` say; `None` when no ray hits anything.
    pub fn log_average(
        &mut self,
        scene: &Scene3d,
        camera: &Camera,
        size: (u32, u32),
        lighting: &Lighting,
        ground: bool,
        f: &FrameLook,
    ) -> Option<f32> {
        self.prepare(scene, ground);
        let bvh = self.bvh.as_ref()?;
        let sun = lighting.sun.map(|l| {
            let d = l.normalized().dir;
            Vec3::new(f64::from(d[0]), f64::from(d[1]), f64::from(d[2]))
        });
        let (mut sum, mut count) = (0.0f64, 0usize);
        for j in 0..METER_ROWS {
            for i in 0..METER_COLUMNS {
                let px = (i as f32 + 0.5) / METER_COLUMNS as f32 * size.0 as f32;
                let py = (j as f32 + 0.5) / METER_ROWS as f32 * size.1 as f32;
                let Some((origin, dir)) = ray_at(camera, size, [px, py]) else { continue };
                let Some(hit) = bvh.ray(origin, dir, 1.0e9) else { continue };
                let Some(&source) = self.sources.get(hit.object.0 as usize) else { continue };
                let mesh = match source {
                    GROUND => self.ground.as_ref(),
                    s if s == GROUND - 1 => scene.horizon.as_ref(),
                    s => scene.meshes.get(s),
                };
                let Some(mesh) = mesh else { continue };
                let mut n = triangle_normal(mesh, hit.point).unwrap_or(dir * -1.0);
                if n.dot(dir) > 0.0 {
                    n = n * -1.0;
                }
                let albedo = lighting.lambert.unwrap_or_else(|| luminance(crate::pipeline::material_albedo(&mesh.material)));
                let mut irradiance = f64::from(f.ambient_lux);
                if let Some(s) = sun {
                    let ndl = n.dot(s).max(0.0);
                    if ndl > 0.0 && f.sun_lux > 0.0 {
                        let blocked = lighting.sun_shadow && bvh.ray(hit.point + n * 1.0e-3, s, 1.0e6).is_some();
                        if !blocked {
                            irradiance += f64::from(f.sun_lux) * ndl;
                        }
                    }
                }
                for light in scene.lights.iter().take(METER_MAX_LIGHTS) {
                    irradiance += direct_illuminance(light, hit.point, n);
                }
                let l = f64::from(albedo) / std::f64::consts::PI * irradiance;
                if l.is_finite() {
                    sum += (1.0e-4 + l.max(0.0)).ln();
                    count += 1;
                }
            }
        }
        (count > 0).then(|| (sum / count as f64).exp() as f32)
    }
}

/// The unit normal of the triangle of `mesh` that contains `p` (the nearest one by plane
/// distance); `None` when no triangle does.
fn triangle_normal(mesh: &Mesh, p: Vec3) -> Option<Vec3> {
    let at = |i: u32| mesh.positions.get(i as usize).map(|v| Vec3::new(f64::from(v[0]), f64::from(v[1]), f64::from(v[2])));
    let mut best: Option<(f64, Vec3)> = None;
    for g in mesh.indices.chunks_exact(3) {
        let (Some(a), Some(b), Some(c)) = (at(g[0]), at(g[1]), at(g[2])) else { continue };
        let n = (b - a).cross(c - a);
        let len2 = n.dot(n);
        if !(len2 > 1.0e-18) {
            continue;
        }
        let nu = n * (1.0 / len2.sqrt());
        let d = nu.dot(p - a).abs();
        if best.as_ref().is_some_and(|(bd, _)| *bd <= d) {
            continue;
        }
        // Inside test with barycentric coordinates, a little slack for the rounded hit point.
        let (v0, v1, v2) = (b - a, c - a, p - a);
        let (d00, d01, d11, d20, d21) = (v0.dot(v0), v0.dot(v1), v1.dot(v1), v2.dot(v0), v2.dot(v1));
        let den = d00 * d11 - d01 * d01;
        if !(den.abs() > 1.0e-18) {
            continue;
        }
        let u = (d11 * d20 - d01 * d21) / den;
        let v = (d00 * d21 - d01 * d20) / den;
        let slack = -1.0e-4;
        if u >= slack && v >= slack && u + v <= 1.0 - slack {
            best = Some((d, nu));
        }
    }
    best.map(|(_, n)| n)
}

/// The look of one frame: the default, then [`exposure_look`], then [`view_look`]. `ground` is the
/// render's ground flag (the raster ground quad exists only at upload, so the meter must be told).
pub fn frame_look(scene: &Scene3d, camera: &Camera, size: (u32, u32), lighting: &Lighting, ground: bool, meter: &mut Meter) -> FrameLook {
    let mut f = FrameLook::default();
    exposure_look(&mut f, scene, camera, size, lighting, ground, meter);
    view_look(&mut f, lighting);
    f
}

/// Exposure, tone map, ambient lux, the Lambert albedo and the sun's lux of a lit frame.
///
/// The sun's lux is the look's only while the frame has a sun (on and above the horizon), else 0;
/// the ambient is 0.25 of it under a sky, 0 without, and at night (no sun) the 0.02 display
/// ambient of the old look expressed in lux at the legacy 0.01 scale (6.2832 lx).
pub fn exposure_look(f: &mut FrameLook, scene: &Scene3d, camera: &Camera, size: (u32, u32), lighting: &Lighting, ground: bool, meter: &mut Meter) {
    let look = &lighting.look;
    f.tone_map = tone_code(look.tone_map);
    f.lambert = lighting.lambert.filter(|a| a.is_finite());
    let intensity = if look.sun.intensity.is_finite() { look.sun.intensity.max(0.0) as f32 } else { 0.0 };
    let sun = lighting.sun.is_some();
    f.sun_lux = if sun { intensity } else { 0.0 };
    f.ambient_lux = match (sun, look.sky.on) {
        (true, true) => 0.25 * intensity,
        (true, false) => 0.0,
        (false, _) => crate::lights::NIGHT_AMBIENT * std::f32::consts::PI / crate::lights::DEFAULT_EXPOSURE,
    };
    // A bare-number exposure of the request arrives as a `Lighting::exposure` off the default.
    let legacy = (lighting.exposure.to_bits() != crate::lights::DEFAULT_EXPOSURE.to_bits() && lighting.exposure.is_finite())
        .then(|| f64::from(lighting.exposure));
    let (scale, mode) = {
        let look_now = &*f;
        exposure_scale(look, legacy, || meter.log_average(scene, camera, size, lighting, ground, look_now), daylight(lighting), lighting.meter_night)
    };
    f.exposure = if scale.is_finite() && scale >= 0.0 { scale } else { crate::lights::DEFAULT_EXPOSURE };
    f.exposure_mode = mode;
}

/// The clip box of a look as `[min, max]` in drawing space; `None` without one or when a corner is
/// not finite (a hostile box clips nothing rather than everything).
pub fn look_clip(look: &cadcraft_doc::arch::Look) -> Option<[[f32; 3]; 2]> {
    let c = look.clip.as_ref()?;
    if !c.min.iter().chain(c.max.iter()).all(|v| v.is_finite()) {
        return None;
    }
    let f = |v: [f64; 3]| [v[0] as f32, v[1] as f32, v[2] as f32];
    Some([f(c.min), f(c.max)])
}

/// Clip box and near plane of the view: copied from the look (`Look::clip`, `Look::near_clip`). A
/// near clip outside (0, 1000] m is ignored. The Lambert override is `exposure_look`'s.
pub fn view_look(f: &mut FrameLook, lighting: &Lighting) {
    f.clip = look_clip(&lighting.look);
    f.near = lighting.look.near_clip.filter(|n| n.is_finite() && *n > 0.0 && *n <= 1000.0).map(|n| n as f32);
}

/// Copy the frame's look into one draw's block; `material` is the draw's mesh material name (the
/// ground quad's is `"ground"`).
pub fn fill_draw(p: &mut PbrUniform, f: &FrameLook, scene: &Scene3d, material: &str) {
    fill_exposure(p, f);
    fill_normal_map(p, f, scene, material);
    fill_clip(p, f);
}

/// Exposure, tone map, ambient lux, the sun's lux and the Lambert albedo into the draw. A frame
/// without a tone map (the unlit legacy look) and the keep-as-is styles (hidden-line, technical)
/// are left as they are. The sun's intensity is the frame's lux, never the look's directly: a frame
/// without a sun writes 0.
pub fn fill_exposure(p: &mut PbrUniform, f: &FrameLook) {
    if f.tone_map == 0 || p.style == STYLE_HIDDEN_LINE || p.style == STYLE_TECHNICAL {
        return;
    }
    p.tone_map = f.tone_map;
    p.exposure = f.exposure;
    p.ambient_lux = f.ambient_lux;
    p.sun_intensity = f.sun_lux;
    if let Some(a) = f.lambert {
        p.lambert_albedo = a;
        p.flags |= PBR_FLAG_LAMBERT;
    }
}

/// The draw's normal map layer and strength [p19-normal-maps-render]. A stub.
pub fn fill_normal_map(_p: &mut PbrUniform, _f: &FrameLook, _scene: &Scene3d, _material: &str) {}

/// The clip box and `PBR_FLAG_CLIP` into the draw; a frame without a clip leaves the block as it is.
pub fn fill_clip(p: &mut PbrUniform, f: &FrameLook) {
    if let Some([min, max]) = f.clip {
        p.flags |= PBR_FLAG_CLIP;
        p.clip_min = min;
        p.clip_max = max;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Orbit;
    use crate::light::Light;
    use crate::pipeline::{Draw, STYLE_SHADED};
    use crate::types::{ArchId, Mesh};

    use cadcraft_doc::arch::{ExposureSetting, ToneMap};
    use cadcraft_geom::Vec2;

    #[test]
    fn default_frame_look_changes_no_draw() {
        // The unlit frame's look is the default one, and filling it changes nothing.
        let scene = Scene3d::default();
        let look = FrameLook::default();
        let before = PbrUniform::new(&Light::DEFAULT, 1.0, &Draw::default(), STYLE_SHADED, [1.0, 2.0, 3.0]);
        let mut after = before;
        fill_draw(&mut after, &look, &scene, "wall");
        fill_draw(&mut after, &look, &scene, "ground");
        assert_eq!(bytemuck::bytes_of(&before), bytemuck::bytes_of(&after));
    }

    /// A 10 m square slab whose top face is at z = 0.
    fn plane() -> Scene3d {
        let mut m = Mesh::new("floor", ArchId(1));
        m.push_prism(&[Vec2::new(-5.0, -5.0), Vec2::new(5.0, -5.0), Vec2::new(5.0, 5.0), Vec2::new(-5.0, 5.0)], -0.1, 0.0);
        Scene3d { bounds: m.bounds(), meshes: vec![m], ..Default::default() }
    }

    /// Straight down at `target`, a metre above it.
    fn down_at(target: [f64; 3]) -> Camera {
        let mut o = Orbit::at_origin(0.0, 89.0, 1.0, 45.0);
        o.target = Vec3::new(target[0], target[1], target[2]);
        Camera::Orbit(o)
    }

    /// Sun straight overhead, 100 000 lx, no sky.
    fn overhead(albedo: Option<f32>) -> Lighting {
        let mut look = cadcraft_doc::arch::Look::default();
        look.sky.on = false;
        Lighting { sun: Some(Light { dir: [0.0, 0.0, 1.0], ..Light::DEFAULT }), look, lambert: albedo, meter_night: true, ..Lighting::default() }
    }

    #[test]
    fn meter_of_a_uniform_plane_is_its_luminance() {
        let scene = plane();
        let camera = down_at([0.0, 0.0, 0.0]);
        let mut meter = Meter::default();
        // Lambert albedo 0.5 under 100 000 lx: L = 0.5 / pi * 1e5.
        let lighting = overhead(Some(0.5));
        let f = frame_look(&scene, &camera, (64, 48), &lighting, false, &mut meter);
        let l = meter.log_average(&scene, &camera, (64, 48), &lighting, false, &f);
        assert!(l.is_some_and(|l| (l - 15_915.5).abs() < 15_915.5 * 0.01), "{l:?}");
        // Albedo 0.05 and 0.1 auto-expose to middle grey: 0.18 / L.
        for (albedo, want) in [(0.05, 1.131e-4), (0.1, 5.655e-5)] {
            let f = frame_look(&scene, &camera, (64, 48), &overhead(Some(albedo)), false, &mut meter);
            assert_eq!(f.exposure_mode, "auto");
            assert!((f.exposure - want).abs() < want * 0.03, "albedo {albedo}: {}", f.exposure);
        }
        // The material's own albedo when there is no override: the floor's luminance.
        let f = frame_look(&scene, &camera, (64, 48), &overhead(None), false, &mut meter);
        let own = crate::pathtrace::luminance(crate::pipeline::material_albedo("floor"));
        let l = meter.log_average(&scene, &camera, (64, 48), &overhead(None), false, &f);
        assert!(l.is_some_and(|l| (l - own / std::f32::consts::PI * 1.0e5).abs() < l * 0.01), "{l:?} for {own}");
        // A manual exposure never meters.
        let mut lighting = overhead(Some(0.5));
        lighting.look.exposure = ExposureSetting::Manual { ev100: 14.0 };
        let f = frame_look(&scene, &camera, (64, 48), &lighting, false, &mut meter);
        assert_eq!(f.exposure_mode, "manual");
        assert!((f.exposure - 5.086e-5).abs() < 1e-7, "{}", f.exposure);
        assert_eq!(f.tone_map, 1);
    }

    #[test]
    fn meter_sees_the_ground_quad() {
        // The ground quad is in no Scene3d: only the `ground` flag brings it. The camera looks at
        // open ground away from the one cube.
        let mut cube = Mesh::new("wall", ArchId(2));
        cube.push_prism(&[Vec2::new(10.0, 10.0), Vec2::new(11.0, 10.0), Vec2::new(11.0, 11.0), Vec2::new(10.0, 11.0)], 0.0, 1.0);
        let scene = Scene3d { bounds: cube.bounds(), meshes: vec![cube], ..Default::default() };
        let camera = down_at([12.0, 12.0, 0.0]);
        let lighting = overhead(Some(0.5));
        let mut meter = Meter::default();
        let f = frame_look(&scene, &camera, (64, 48), &lighting, true, &mut meter);
        let on = meter.log_average(&scene, &camera, (64, 48), &lighting, true, &f);
        assert!(on.is_some_and(|l| (l - 15_915.5).abs() < 15_915.5 * 0.01), "{on:?}");
        assert_eq!(meter.log_average(&scene, &camera, (64, 48), &lighting, false, &f), None, "no ground, nothing to meter");
        // With nothing hit the daylight scale is the day exposure.
        let f = frame_look(&scene, &camera, (64, 48), &lighting, false, &mut meter);
        assert_eq!(f.exposure, crate::pathtrace::DAY_EXPOSURE);
    }

    #[test]
    fn fill_clip_box_and_flag() {
        use cadcraft_doc::arch::ClipBox;
        let scene = Scene3d::default();
        // The look's box and near reach the frame look.
        let mut lighting = Lighting::default();
        lighting.look.clip = Some(ClipBox { min: [-1.0, -2.0, -3.0], max: [4.0, 5.0, 2.0] });
        lighting.look.near_clip = Some(0.75);
        let mut f = FrameLook::default();
        view_look(&mut f, &lighting);
        assert_eq!((f.clip, f.near), (Some([[-1.0, -2.0, -3.0], [4.0, 5.0, 2.0]]), Some(0.75)));
        let before = PbrUniform::new(&Light::DEFAULT, 1.0, &Draw::default(), STYLE_SHADED, [0.0; 3]);
        let mut p = before;
        fill_draw(&mut p, &f, &scene, "wall");
        assert_eq!(p.flags & PBR_FLAG_CLIP, PBR_FLAG_CLIP);
        assert_eq!((p.clip_min, p.clip_max), ([-1.0, -2.0, -3.0], [4.0, 5.0, 2.0]));
        // Hidden-line and technical draws are clipped too, and the Lambert bit is untouched.
        let mut h = PbrUniform::new(&Light::DEFAULT, 1.0, &Draw::default(), STYLE_HIDDEN_LINE, [0.0; 3]);
        fill_draw(&mut h, &f, &scene, "wall");
        assert_eq!(h.flags & PBR_FLAG_CLIP, PBR_FLAG_CLIP);
        assert_eq!(p.flags & PBR_FLAG_LAMBERT, 0);
        // No box: the draw is exactly as it was made.
        let mut none = FrameLook::default();
        lighting.look.clip = None;
        lighting.look.near_clip = None;
        view_look(&mut none, &lighting);
        assert_eq!((none.clip, none.near), (None, None));
        let mut q = before;
        fill_draw(&mut q, &none, &scene, "wall");
        assert_eq!(bytemuck::bytes_of(&before), bytemuck::bytes_of(&q));
        // Hostile values are ignored, never clamped.
        lighting.look.clip = Some(ClipBox { min: [f64::NAN, 0.0, 0.0], max: [1.0; 3] });
        for near in [0.0, -1.0, f64::NAN, 1.0e6] {
            lighting.look.near_clip = Some(near);
            let mut g = FrameLook::default();
            view_look(&mut g, &lighting);
            assert_eq!((g.clip, g.near), (None, None), "near {near}");
        }
    }

    #[test]
    fn technical_and_hidden_line_not_exposed() {
        let scene = Scene3d::default();
        let mut look = FrameLook { tone_map: 1, exposure: 1.0e-4, ambient_lux: 5.0e4, sun_lux: 1.0e5, lambert: Some(0.5), ..Default::default() };
        look.exposure_mode = "auto";
        for style in [crate::pipeline::STYLE_TECHNICAL, crate::pipeline::STYLE_HIDDEN_LINE] {
            let before = PbrUniform::new(&Light::DEFAULT, 0.0, &Draw::default(), style, [0.0; 3]);
            let mut after = before;
            fill_draw(&mut after, &look, &scene, "wall");
            assert_eq!(bytemuck::bytes_of(&before), bytemuck::bytes_of(&after), "style {style}");
        }
        // A shaded draw takes the whole look, and a frame with no tone map takes nothing.
        let before = PbrUniform::new(&Light::DEFAULT, 1.0, &Draw::default(), STYLE_SHADED, [0.0; 3]);
        let mut p = before;
        fill_draw(&mut p, &look, &scene, "wall");
        assert_eq!((p.tone_map, p.exposure, p.ambient_lux, p.sun_intensity, p.lambert_albedo), (1, 1.0e-4, 5.0e4, 1.0e5, 0.5));
        assert_eq!(p.flags & crate::pipeline::PBR_FLAG_LAMBERT, crate::pipeline::PBR_FLAG_LAMBERT);
        look.tone_map = 0;
        let mut q = before;
        fill_draw(&mut q, &look, &scene, "wall");
        assert_eq!(bytemuck::bytes_of(&before), bytemuck::bytes_of(&q));
    }

    #[test]
    fn night_lit_frame_has_no_sun_term() {
        let scene = plane();
        let camera = down_at([0.0, 0.0, 0.0]);
        let mut meter = Meter::default();
        // A frame without a sun: the look's 100 000 lx sun never reaches the draw.
        let night = Lighting { sun: None, ..Lighting::default() };
        let f = frame_look(&scene, &camera, (64, 48), &night, false, &mut meter);
        assert_eq!(f.sun_lux, 0.0);
        assert_eq!(f.tone_map, 1);
        assert!((f.ambient_lux - 6.2832).abs() < 1e-3, "{}", f.ambient_lux);
        // A legacy night keeps the 0.01 scale: the old 0.02 ambient, not metered.
        assert_eq!(f.exposure, 0.01);
        assert_eq!(f.exposure_mode, "auto");
        let mut p = PbrUniform::new(&Light::DEFAULT, 1.0, &Draw::default(), STYLE_SHADED, [0.0; 3]);
        fill_draw(&mut p, &f, &scene, "floor");
        assert_eq!(p.sun_intensity, 0.0);
        assert!((p.ambient_lux * p.exposure - 0.0628).abs() < 1e-3);
        // A metered night (the pane): the same lux, a scale that lifts it, still no sun.
        let metered = Lighting { sun: None, lambert: Some(0.1), meter_night: true, ..Lighting::default() };
        let f = frame_look(&scene, &camera, (64, 48), &metered, false, &mut meter);
        assert_eq!(f.sun_lux, 0.0);
        assert!((f.exposure - 0.9).abs() < 0.9 * 0.03, "{}", f.exposure);
        // The day frame writes the look's sun.
        let f = frame_look(&scene, &camera, (64, 48), &Lighting::default(), false, &mut meter);
        assert_eq!((f.sun_lux, f.ambient_lux), (100_000.0, 25_000.0));
        // A hand-built fixed exposure is used as given.
        let fixed = Lighting { exposure: 0.25, ..Lighting::default() };
        let f = frame_look(&scene, &camera, (64, 48), &fixed, false, &mut meter);
        assert_eq!((f.exposure, f.exposure_mode), (0.25, "fixed"));
    }

    #[test]
    fn tone_map_follows_the_look() {
        let scene = Scene3d::default();
        let camera = Camera::Orbit(Orbit::at_origin(30.0, 30.0, 10.0, 45.0));
        let mut meter = Meter::default();
        for (t, code) in [(ToneMap::Aces, 1), (ToneMap::Hable, 2), (ToneMap::None, 3)] {
            let mut l = Lighting::default();
            l.look.tone_map = t;
            assert_eq!(frame_look(&scene, &camera, (64, 64), &l, true, &mut meter).tone_map, code);
        }
    }
}
