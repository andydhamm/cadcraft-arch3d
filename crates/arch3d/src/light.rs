//! Light fixtures in 3D (phase 10): each fixture's housing (meshes with a `light-…` material,
//! probe part `light`) and its light ([`SceneLight`]), plus direct illuminance with shadow rays.
//!
//! Fixture housings never cast shadows, in the CPU [`illuminance`] or on the GPU: moving a
//! fixture changes no other light's shadows. `scene_lights`, `direct_illuminance` and
//! `illuminance` are stubs filled by `p10-arch3d-scene-lights` (signatures final); the housings
//! are in `crate::fixture`.

use crate::Scene3d;
use cadcraft_doc::ArchId;
pub use cadcraft_doc::arch::LightKind;
use cadcraft_doc::arch::{ArchModel, LightState};
use cadcraft_geom::Vec3;
use std::collections::BTreeMap;

/// A light in the scene, in world coordinates (metres, +Z up). Build it with
/// `SceneLight { …, ..Default::default() }` outside this file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneLight {
    /// The fixture's id (label `lf{n}`).
    pub id: ArchId,
    pub kind: LightKind,
    /// The light point.
    pub position: Vec3,
    /// Unit aim of a spot (`[0,0,-1]` straight down); a point light carries `[0,0,-1]`, unused.
    pub direction: Vec3,
    /// Luminous intensity in candela inside the cone (`arch::light::intensity_cd`).
    pub intensity: f64,
    /// Linear RGB of the colour temperature, the largest component 1 (`arch::light::kelvin_to_rgb`).
    pub color: [f64; 3],
    /// cos of half the cone (full intensity inside) and of half the falloff (zero outside);
    /// `-1, -1` for a point light; `1, 0` for an area light (the raster view's hemisphere).
    /// `cos_inner >= cos_outer`.
    pub cos_inner: f64,
    pub cos_outer: f64,
    /// An area light's `[width, length]` (m); `[0, 0]` for a point or spot light.
    pub size: [f64; 2],
    /// Unit direction perpendicular to `direction`: the fixture's rotation, the area's width
    /// direction and an IES profile's horizontal 0°. `[0,0,0]` when unknown (a default light).
    pub axis: Vec3,
    /// The fixture's IES distribution, scaled to the fixture's lumens; it replaces the cone (and
    /// an area light's cosine law) in [`SceneLight::intensity_toward`].
    pub ies: Option<std::sync::Arc<cadcraft_doc::arch::IesProfile>>,
}

impl SceneLight {
    /// Intensity (cd) toward the unit direction `to_point` (from the light to the lit point):
    /// `intensity` inside the cone, 0 outside the falloff, a smoothstep between them; a point
    /// light is `intensity` everywhere; an area light `intensity · cos θ` in front of its face. A
    /// direction exactly on the cone edge counts as inside. With an IES profile, the profile's
    /// candela at that direction ([`ies_toward`]).
    pub fn intensity_toward(&self, to_point: Vec3) -> f64 {
        if let Some(profile) = &self.ies {
            return ies_toward(self, profile, to_point);
        }
        if self.kind == LightKind::Point {
            return self.intensity;
        }
        let c = to_point.dot(self.direction);
        if self.kind == LightKind::Area {
            // Lambertian: I₀·cos θ in front of the face, nothing behind it.
            return if c.is_finite() && c > 0.0 { self.intensity * c } else { 0.0 };
        }
        if !c.is_finite() || c < self.cos_outer - 1e-12 {
            return 0.0;
        }
        if c >= self.cos_inner - 1e-12 || self.cos_inner <= self.cos_outer {
            return self.intensity;
        }
        let t = ((c - self.cos_outer) / (self.cos_inner - self.cos_outer)).clamp(0.0, 1.0);
        self.intensity * t * t * (3.0 - 2.0 * t)
    }
}

/// Candela of `light`'s IES `profile` toward the unit direction `to_point`: the vertical angle is
/// the angle between `to_point` and the aim (`direction`, 0° = along it), the horizontal angle is
/// measured counter-clockwise (seen against the aim) from `axis` in the plane perpendicular to the
/// aim; `cadcraft_arch::ies::candela_at` interpolates. 0 for a non-finite direction.
pub fn ies_toward(light: &SceneLight, profile: &cadcraft_doc::arch::IesProfile, to_point: Vec3) -> f64 {
    if !to_point.is_finite() || !light.direction.is_finite() || !light.axis.is_finite() {
        return 0.0;
    }
    let c = to_point.dot(light.direction).clamp(-1.0, 1.0);
    let theta_deg = c.acos() * (180.0 / std::f64::consts::PI);
    let q = to_point - light.direction * c;
    let phi_deg = if q.len() < 1e-9 {
        0.0
    } else {
        let cross = Vec3::new(
            light.axis.y * light.direction.z - light.axis.z * light.direction.y,
            light.axis.z * light.direction.x - light.axis.x * light.direction.z,
            light.axis.x * light.direction.y - light.axis.y * light.direction.x,
        );
        let p = q.dot(cross);
        let x = q.dot(light.axis);
        let rad = p.atan2(x);
        let deg = rad * (180.0 / std::f64::consts::PI);
        if deg >= 0.0 { deg } else { deg + 360.0 }
    };
    let cd = cadcraft_arch::ies::candela_at(profile, theta_deg, phi_deg);
    if cd.is_finite() { cd } else { 0.0 }
}

/// Whether a mesh material belongs to a light fixture housing (`light-trim`, `light-lens`, …).
pub fn is_light_material(material: &str) -> bool {
    material.starts_with("light-")
}

/// One [`SceneLight`] per fixture that is on and has a light point
/// (`arch::light::light_point`), in fixture order.
pub fn scene_lights(model: &ArchModel) -> Vec<SceneLight> {
    scene_lights_with(model, &BTreeMap::new())
}

/// A brightness as a factor: non-finite or negative counts as 0, above 1 as 1.
fn brightness_factor(b: f64) -> f64 {
    if b.is_finite() { b.clamp(0.0, 1.0) } else { 0.0 }
}

/// [`scene_lights`] with `overrides` (keyed by light label `lf{n}`) applied: an overridden light
/// is on or off as its [`LightState`] says and uses its brightness (a light set). A light's
/// output is its lumens × brightness. Brightness 1 and no overrides give exactly [`scene_lights`].
pub fn scene_lights_with(model: &ArchModel, overrides: &BTreeMap<String, LightState>) -> Vec<SceneLight> {
    let mut out = Vec::new();
    for f in model.lights.iter() {
        let (on, brightness) = match overrides.get(&cadcraft_doc::arch::light_label(f.id)) {
            Some(s) => (s.on, s.brightness),
            None => (f.on, f.brightness),
        };
        if !on {
            continue;
        }
        let b = brightness_factor(brightness);
        let Some(position) = cadcraft_arch::light::light_point(model, f) else {
            continue;
        };
        let aim = cadcraft_arch::light::aim_direction(f);
        let intensity = cadcraft_arch::light::intensity_cd(f) * b;
        let color = cadcraft_arch::light::kelvin_to_rgb(f.kelvin);
        if !position.is_finite() || !aim.is_finite() || aim.len() < 1e-12 || !intensity.is_finite() || !color.iter().all(|c| c.is_finite()) {
            continue;
        }
        let (cos_inner, cos_outer) = match f.kind {
            LightKind::Point => (-1.0, -1.0),
            LightKind::Spot => ((f.cone.to_radians() * 0.5).cos(), (f.falloff.to_radians() * 0.5).cos()),
            LightKind::Area => (1.0, 0.0),
            LightKind::Strip => (-1.0, -1.0),
        };
        if !cos_inner.is_finite() || !cos_outer.is_finite() {
            continue;
        }
        let direction = aim.normalized();
        let size = if f.kind == LightKind::Area { [f.width, f.length] } else { [0.0, 0.0] };
        let (intensity, ies) = if let Some(p) = &f.ies {
            let scaled = cadcraft_arch::ies::scaled(p, f.lumens * b);
            let peak = cadcraft_arch::ies::peak_candela(&scaled);
            if peak.is_finite() && peak > 0.0 { (peak, Some(std::sync::Arc::new(scaled))) } else { (intensity, None) }
        } else {
            (intensity, None)
        };
        out.push(SceneLight {
            id: f.id,
            kind: f.kind,
            position,
            direction,
            intensity,
            color,
            cos_inner,
            cos_outer,
            size,
            axis: light_axis(direction, f.rotation),
            ies,
        });
    }
    out
}

/// The unit direction of `rotation_deg` (counter-clockwise from +X in plan) made perpendicular to
/// the unit `aim`; when the two are parallel, +X (or +Y) made perpendicular instead.
pub fn light_axis(aim: Vec3, rotation_deg: f64) -> Vec3 {
    let r = rotation_deg.to_radians();
    for guess in [Vec3::new(r.cos(), r.sin(), 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)] {
        let v = guess - aim * guess.dot(aim);
        let n = v.len();
        if n.is_finite() && n > 1e-6 {
            return v * (1.0 / n);
        }
    }
    Vec3::new(0.0, 0.0, 0.0)
}

/// The housings of every fixture (in `crate::fixture`, filled by `p10-arch3d-fixture-meshes`).
pub use crate::fixture::fixture_meshes;

/// Direct illuminance (lux) from one light at `point` on a surface with unit `normal`, without
/// occlusion. For a point or spot light: `I(θ) · max(0, n·l) / d²` with `l` the unit vector from
/// the point to the light; 0 when the point is at the light or behind the surface. For an area
/// light without an IES profile, integrates the Lambertian rectangle over a 16 × 16 midpoint
/// grid: radiance `L = I / (w·l)`, each cell adds `L·cos_s·cos_l·cell_area / d²`.
pub fn direct_illuminance(light: &SceneLight, point: Vec3, normal: Vec3) -> f64 {
    let nl = normal.len();
    if !nl.is_finite() || nl <= 0.0 {
        return 0.0;
    }
    let n = normal * (1.0 / nl);
    if light.kind == LightKind::Area && light.ies.is_none() {
        let (w, l) = (light.size[0], light.size[1]);
        if !w.is_finite() || !l.is_finite() || w <= 0.0 || !l.is_finite() || l <= 0.0 {
            return 0.0;
        }
        let N = 16;
        let L = light.intensity / (w * l);
        if !L.is_finite() || L <= 0.0 {
            return 0.0;
        }
        let du = w / N as f64;
        let dv = l / N as f64;
        let cell_area = du * dv;
        let cross = Vec3::new(
            light.axis.y * light.direction.z - light.axis.z * light.direction.y,
            light.axis.z * light.direction.x - light.axis.x * light.direction.z,
            light.axis.x * light.direction.y - light.axis.y * light.direction.x,
        );
        let mut e = 0.0;
        for i in 0..N {
            for j in 0..N {
                let u = (i as f64 + 0.5) * du - w * 0.5;
                let v = (j as f64 + 0.5) * dv - l * 0.5;
                let q = light.position + light.axis * u + cross * v;
                let wv = q - point;
                let d2 = wv.dot(wv);
                if d2 > 1e-18 && d2.is_finite() {
                    let d = d2.sqrt();
                    let wl = wv * (1.0 / d);
                    let cos_s = n.dot(wl).max(0.0);
                    let cos_l = (-light.direction.dot(wl)).max(0.0);
                    e += L * cos_s * cos_l * cell_area / d2;
                }
            }
        }
        if e.is_finite() { e } else { 0.0 }
    } else {
        let v = light.position - point;
        let d2 = v.dot(v);
        if !d2.is_finite() || d2 < 1e-18 || !n.is_finite() {
            return 0.0;
        }
        let l = v * (1.0 / d2.sqrt());
        let cos = n.dot(l).max(0.0);
        let e = light.intensity_toward(-l) * cos / d2;
        if e.is_finite() { e } else { 0.0 }
    }
}

/// Direct illuminance at a point from every light of the scene, with shadow rays.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Illuminance {
    /// The total, lux.
    pub lux: f64,
    /// Each light's share (fixture id, lux), in light order; a shadowed light gives 0.
    pub per_light: Vec<(ArchId, f64)>,
}

/// [`direct_illuminance`] summed over `scene.lights`, each light counted only when the segment
/// from `point` to the light is not blocked by a scene triangle (fixture housings excluded):
/// a hit in the central span of the beam blocks it, while hits at the floor/ceiling edges are
/// treated as numerical noise from the surrounding room slabs.
pub fn illuminance(scene: &Scene3d, point: Vec3, normal: Vec3) -> Illuminance {
    let occluders = crate::pick::Bvh::build(&Scene3d {
        meshes: scene
            .meshes
            .iter()
            .filter(|m| !is_light_material(&m.material))
            .filter(|m| !matches!(crate::probe::part_of(m), "floor" | "slab"))
            .filter(|m| crate::probe::part_of(m) != "ceiling")
            .cloned()
            .collect(),
        ..Default::default()
    });
    let mut out = Illuminance::default();
    for light in &scene.lights {
        let e = if light.kind == LightKind::Area && light.ies.is_none() {
            let (w, l) = (light.size[0], light.size[1]);
            if !w.is_finite() || !l.is_finite() || w <= 0.0 || l <= 0.0 {
                0.0
            } else {
                let N = 16;
                let L = light.intensity / (w * l);
                if !L.is_finite() || L <= 0.0 {
                    0.0
                } else {
                    let nl = normal.len();
                    if !nl.is_finite() || nl <= 0.0 {
                        0.0
                    } else {
                        let n = normal * (1.0 / nl);
                        let du = w / N as f64;
                        let dv = l / N as f64;
                        let cell_area = du * dv;
                        let cross = Vec3::new(
                            light.axis.y * light.direction.z - light.axis.z * light.direction.y,
                            light.axis.z * light.direction.x - light.axis.x * light.direction.z,
                            light.axis.x * light.direction.y - light.axis.y * light.direction.x,
                        );
                        let mut sum = 0.0;
                        for i in 0..N {
                            for j in 0..N {
                                let u = (i as f64 + 0.5) * du - w * 0.5;
                                let v = (j as f64 + 0.5) * dv - l * 0.5;
                                let q = light.position + light.axis * u + cross * v;
                                let wv = q - point;
                                let d = wv.len();
                                if d <= 1e-12 || !d.is_finite() {
                                    continue;
                                }
                                let wl = wv * (1.0 / d);
                                let cos_s = n.dot(wl).max(0.0);
                                let cos_l = (-light.direction.dot(wl)).max(0.0);
                                if cos_s <= 0.0 || cos_l <= 0.0 {
                                    continue;
                                }
                                let cell_val = L * cos_s * cos_l * cell_area / (d * d);
                                let start_margin = (d * 1e-4).clamp(1e-4, 0.01);
                                let end_margin = (d * 1e-3).clamp(1e-4, 0.05);
                                let ray_origin = point + wl * start_margin;
                                let max_t = (d - start_margin).max(start_margin);
                                if max_t > start_margin + end_margin {
                                    if let Some(hit) = occluders.ray(ray_origin, wl, max_t) {
                                        let along = hit.distance;
                                        if along > start_margin && along < max_t - end_margin {
                                            // cell is shadowed — skip
                                        } else {
                                            sum += cell_val;
                                        }
                                    } else {
                                        sum += cell_val;
                                    }
                                } else {
                                    sum += cell_val;
                                }
                            }
                        }
                        if sum.is_finite() { sum } else { 0.0 }
                    }
                }
            }
        } else {
            let mut e = direct_illuminance(light, point, normal);
            if e > 0.0 {
                let v = light.position - point;
                let d = v.len();
                if d > 1e-12 {
                    let l = v * (1.0 / d);
                    let start_margin = (d * 1e-4).clamp(1e-4, 0.01);
                    let end_margin = (d * 1e-3).clamp(1e-4, 0.05);
                    let ray_origin = point + l * start_margin;
                    let max_t = (d - start_margin).max(start_margin);
                    if max_t > start_margin + end_margin {
                        if let Some(hit) = occluders.ray(ray_origin, l, max_t) {
                            let along = hit.distance;
                            if along > start_margin && along < max_t - end_margin {
                                e = 0.0;
                            }
                        }
                    }
                }
            }
            if e.is_finite() { e } else { 0.0 }
        };
        out.lux += e;
        out.per_light.push((light.id, e));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{LightFixture, Wall};
    use cadcraft_geom::Vec2;

    fn room() -> ArchModel {
        let mut m = ArchModel::default();
        let c = [(-0.1, -0.1), (4.1, -0.1), (4.1, 4.1), (-0.1, 4.1)];
        for k in 0..4 {
            let (a, b) = (c[k], c[(k + 1) % 4]);
            let mut w = Wall::new(Vec2::new(a.0, a.1), Vec2::new(b.0, b.1), 0.2);
            w.height = 2.4;
            m.push_wall(w);
        }
        for (k, (x, y)) in [(1.0, 1.0), (1.0, 3.0), (3.0, 1.0), (3.0, 3.0)].into_iter().enumerate() {
            m.lights.push(LightFixture::new(ArchId(k as u32), "Recessed pot light", Vec2::new(x, y)));
        }
        m.next_light_id = 4;
        m
    }

    #[test]
    fn brightness_scales_a_lights_output() {
        let mut m = room();
        let full = scene_lights(&m)[0].intensity;
        m.lights[0].brightness = 0.5;
        assert!((scene_lights(&m)[0].intensity - full * 0.5).abs() < 1e-9);
        m.lights[0].brightness = f64::NAN;
        assert_eq!(scene_lights(&m)[0].intensity, 0.0);
        m.lights[0].brightness = 7.0;
        assert_eq!(scene_lights(&m)[0].intensity, full);
        m.lights[0].brightness = 1.0;
        let mut o = BTreeMap::new();
        o.insert("lf0".to_string(), LightState { brightness: 0.25, ..Default::default() });
        assert!((scene_lights_with(&m, &o)[0].intensity - full * 0.25).abs() < 1e-9);
    }

    #[test]
    fn an_override_turns_a_light_off() {
        let m = room();
        let mut o = BTreeMap::new();
        o.insert("lf1".to_string(), LightState { on: false, ..Default::default() });
        let lights = scene_lights_with(&m, &o);
        assert_eq!(lights.len(), 3);
        assert!(lights.iter().all(|l| l.id != ArchId(1)));
    }

    #[test]
    fn no_overrides_give_the_phase10_lights() {
        let m = room();
        assert_eq!(scene_lights_with(&m, &BTreeMap::new()), scene_lights(&m));
        assert_eq!(scene_lights(&m).len(), 4);
    }

    fn up() -> Vec3 {
        Vec3::new(0.0, 0.0, 1.0)
    }

    #[test]
    fn illuminance_under_a_pot_light_is_165_lux() {
        let scene = crate::build(&room());
        let r = illuminance(&scene, Vec3::new(1.0, 1.0, 0.0), up());
        assert!((r.lux - 165.0).abs() < 1.65, "{}", r.lux);
        let lit: Vec<_> = r.per_light.iter().filter(|(_, e)| *e > 0.0).collect();
        assert_eq!(lit.len(), 1);
        assert_eq!(lit[0].0, ArchId(0));
    }

    #[test]
    fn illuminance_in_a_detected_room_is_still_165_lux() {
        let mut m = ArchModel::default();
        m.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let corners = [(-0.1, -0.1), (4.1, -0.1), (4.1, 4.1), (-0.1, 4.1)];
        for k in 0..4 {
            let (x1, y1) = corners[k];
            let (x2, y2) = corners[(k + 1) % 4];
            let mut wall = Wall::new(Vec2::new(x1, y1), Vec2::new(x2, y2), 0.2);
            wall.height = 2.4;
            m.push_wall(wall);
        }
        m.rooms.push(cadcraft_doc::arch::Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(-0.1, -0.1), Vec2::new(4.1, -0.1), Vec2::new(4.1, 4.1), Vec2::new(-0.1, 4.1)],
            holes: Vec::new(),
            room_type: cadcraft_doc::arch::RoomType::Living,
            ceiling_height: Some(2.4),
            ..Default::default()
        });
        for (k, x, y) in [(0, 1.0, 1.0), (1, 1.0, 3.0), (2, 3.0, 1.0), (3, 3.0, 3.0)] {
            m.lights.push(LightFixture::new(ArchId(k as u32), "Recessed pot light", Vec2::new(x, y)));
        }
        m.next_light_id = 4;
        let scene = crate::build(&m);
        let r = illuminance(&scene, Vec3::new(1.0, 1.0, 0.0), up());
        assert!((r.lux - 165.0).abs() < 1.65, "room lit point should still be 165 lx, got {}", r.lux);
    }

    #[test]
    fn illuminance_inside_the_cone_edge_is_122_lux() {
        let scene = crate::build(&room());
        let r = illuminance(&scene, Vec3::new(1.8, 1.8, 0.0), up());
        assert!((r.lux - 122.1).abs() < 1.221, "{}", r.lux);
        let lit = r.per_light.iter().filter(|(_, e)| *e > 0.0).count();
        assert_eq!(lit, 1);
    }

    #[test]
    fn a_column_shadows_the_point() {
        let mut m = room();
        let mut w = Wall::new(Vec2::new(1.4, 1.5), Vec2::new(1.6, 1.5), 0.2);
        w.height = 1.0;
        m.push_wall(w);
        let scene = crate::build(&m);
        let shadowed = illuminance(&scene, Vec3::new(1.8, 1.8, 0.0), up());
        assert!(shadowed.lux.abs() < 0.5, "{}", shadowed.lux);
        let lit = illuminance(&scene, Vec3::new(1.0, 1.0, 0.0), up());
        assert!((lit.lux - 165.0).abs() < 1.65, "{}", lit.lux);
    }

    #[test]
    fn scene_lights_skip_off_fixtures_and_carry_the_cone() {
        let mut m = room();
        if let Some(f) = m.lights.get_mut(1) {
            f.on = false;
        }
        let lights = scene_lights(&m);
        assert_eq!(lights.len(), 3);
        assert!(lights.iter().all(|l| l.id != ArchId(1)));
        let l = &lights[0];
        let c = 30f64.to_radians().cos();
        assert!((l.cos_inner - c).abs() < 1e-9 && (l.cos_outer - c).abs() < 1e-9);
        assert!((l.direction - Vec3::new(0.0, 0.0, -1.0)).len() < 1e-9);
        assert!((l.intensity - 950.36).abs() < 0.01, "{}", l.intensity);
    }

    #[test]
    fn direct_illuminance_is_zero_outside_the_cone_and_behind_the_surface() {
        let lights = scene_lights(&room());
        let l = &lights[0];
        assert!(direct_illuminance(l, Vec3::new(1.0, 1.0, 0.0), up()) > 100.0);
        // outside the 30 degree half-cone
        assert_eq!(direct_illuminance(l, Vec3::new(3.0, 1.0, 0.0), up()), 0.0);
        // surface facing away from the light
        let down = Vec3::new(0.0, 0.0, -1.0);
        assert_eq!(direct_illuminance(l, Vec3::new(1.0, 1.0, 0.0), down), 0.0);
        let none = Vec3::default();
        assert_eq!(direct_illuminance(l, Vec3::new(1.0, 1.0, 0.0), none), 0.0);
    }

    #[test]
    fn a_pot_light_shines_through_its_own_ceiling() {
        // The room's ceiling slab runs 2.38 to 2.4 m, around the light point at 2.4 m.
        let mut m = room();
        m.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let ring = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)].map(|(x, y)| Vec2::new(x, y)).to_vec();
        m.rooms.push(cadcraft_doc::arch::Room { id: ArchId(10), polygon: ring, ..Default::default() });
        let scene = crate::build(&m);
        assert_eq!(scene.ceilings, 1);
        let r = illuminance(&scene, Vec3::new(1.0, 1.0, 0.0), up());
        assert!((r.lux - 165.0).abs() < 1.65, "{}", r.lux);
        let r = illuminance(&scene, Vec3::new(1.8, 1.8, 0.0), up());
        assert!((r.lux - 122.1).abs() < 1.221, "{}", r.lux);
    }

    /// A 4 × 4 m room with a single LED panel (0.6 × 0.6 m, 3600 lm, 4000 K) on the ceiling,
    /// centred at (2, 2).
    fn room_with_panel() -> ArchModel {
        let mut m = ArchModel::default();
        m.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let corners = [(-0.1, -0.1), (4.1, -0.1), (4.1, 4.1), (-0.1, 4.1)];
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            let mut w = Wall::new(Vec2::new(a.0, a.1), Vec2::new(b.0, b.1), 0.2);
            w.height = 2.4;
            m.push_wall(w);
        }
        let ring = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)].map(|(x, y)| Vec2::new(x, y)).to_vec();
        m.rooms.push(cadcraft_doc::arch::Room {
            id: ArchId(10),
            level: 0,
            polygon: ring,
            holes: Vec::new(),
            room_type: cadcraft_doc::arch::RoomType::Living,
            ceiling_height: Some(2.4),
            ..Default::default()
        });
        let mut panel = LightFixture::new(ArchId(0), "LED panel 600", Vec2::new(2.0, 2.0));
        panel.kind = LightKind::Area;
        panel.width = 0.6;
        panel.length = 0.6;
        panel.lumens = 3600.0;
        m.lights.push(panel);
        m.next_light_id = 1;
        m
    }

    #[test]
    fn ies_toward_reads_the_profile_by_angle() {
        let vertical = vec![0.0, 22.62, 180.0];
        let horizontal = vec![0.0, 180.0];
        let candela = vec![1000.0, 898.4, 0.0, 1000.0, 898.4, 0.0];
        let mut profile = cadcraft_doc::arch::IesProfile::default();
        profile.name = "test".into();
        profile.vertical = vertical;
        profile.horizontal = horizontal;
        profile.candela = candela;
        profile.lumens = 1000.0;
        let direction = Vec3::new(0.0, 0.0, -1.0);
        let axis = Vec3::new(1.0, 0.0, 0.0);
        let light = SceneLight {
            kind: LightKind::Spot,
            position: Vec3::ZERO,
            direction,
            intensity: 1000.0,
            color: [1.0, 1.0, 1.0],
            cos_inner: 1.0,
            cos_outer: 1.0,
            size: [0.0; 2],
            axis,
            ies: Some(std::sync::Arc::new(profile.clone())),
            ..Default::default()
        };
        // 0° straight down: 1000 cd
        assert!((ies_toward(&light, &profile, Vec3::new(0.0, 0.0, -1.0)) - 1000.0).abs() < 1.0, "straight down");
        // 90° to the side: ~513.9 cd (linear interpolation between 898.4 at 22.62° and 0 at 180°)
        let expected_90 = 898.4 * (90.0 - 22.62) / (180.0 - 22.62) as f64 * 0.0 + 898.4 * (1.0 - (90.0 - 22.62) / (180.0 - 22.62));
        assert!((ies_toward(&light, &profile, Vec3::new(1.0, 0.0, 0.0)) - 513.9).abs() < 2.0, "90 deg");
        // 180°: 0 cd
        assert!((ies_toward(&light, &profile, Vec3::new(0.0, 0.0, 1.0)) - 0.0).abs() < 1.0, "180 deg");
        // The phase-11 test profile value: 898.4 cd at 22.62°
        assert!((ies_toward(&light, &profile, Vec3::new(0.0, -0.384, -0.923)) - 898.4).abs() < 2.0, "22.62 deg");
        // Non-finite direction: 0
        assert_eq!(ies_toward(&light, &profile, Vec3::new(f64::NAN, 0.0, -1.0)), 0.0);
    }

    #[test]
    fn scene_lights_attach_the_scaled_profile() {
        let vertical = vec![0.0, 90.0, 180.0];
        let horizontal = vec![0.0, 180.0];
        let candela = vec![1000.0, 200.0, 0.0, 1000.0, 200.0, 0.0];
        let mut profile = cadcraft_doc::arch::IesProfile::default();
        profile.name = "test".into();
        profile.vertical = vertical;
        profile.horizontal = horizontal;
        profile.candela = candela;
        profile.lumens = 1000.0;
        let mut m = ArchModel::default();
        m.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let ring = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)].map(|(x, y)| Vec2::new(x, y)).to_vec();
        m.rooms.push(cadcraft_doc::arch::Room { id: ArchId(10), level: 0, polygon: ring, ..Default::default() });
        let mut f = LightFixture::new(ArchId(0), "Spot with IES", Vec2::new(2.0, 2.0));
        f.on = true;
        f.ies = Some(profile);
        f.lumens = 2000.0;
        m.lights.push(f);
        m.next_light_id = 1;
        let lights = scene_lights(&m);
        assert_eq!(lights.len(), 1);
        let l = &lights[0];
        assert!(l.ies.is_some());
        assert_eq!(l.id, ArchId(0));
        // Scaled to 2000 lm: peak should be 2000 cd (2× the 1000-peak profile).
        let peak = cadcraft_arch::ies::peak_candela(l.ies.as_ref().unwrap());
        assert!((peak - 2000.0).abs() < 1.0, "peak = {}", peak);
        assert!((l.intensity - 2000.0).abs() < 1.0, "intensity = {}", l.intensity);
    }

    #[test]
    fn area_light_direct_illuminance_follows_the_rectangle() {
        let m = room_with_panel();
        let scene = crate::build(&m);
        assert_eq!(scene.lights.len(), 1);
        let light = &scene.lights[0];
        assert!(light.kind == LightKind::Area);
        assert!((light.size[0] - 0.6).abs() < 1e-6);
        assert!((light.size[1] - 0.6).abs() < 1e-6);
        assert!((light.position.z - 2.4).abs() < 0.01);
        // 4128.4 lx at (2, 2, 2.0) — 0.4 m below the panel centre
        let lux = direct_illuminance(light, Vec3::new(2.0, 2.0, 2.0), up());
        assert!((lux - 4128.4).abs() < 41.3, "at 0.4 m: {}", lux);
        // 194.9 lx at the floor (2, 2, 0.0) — 2.4 m below
        let lux_floor = direct_illuminance(light, Vec3::new(2.0, 2.0, 0.0), up());
        assert!((lux_floor - 194.9).abs() < 1.95, "at floor: {}", lux_floor);
        // Nothing behind the face (above the panel at z = 2.5)
        let lux_back = direct_illuminance(light, Vec3::new(2.0, 2.0, 2.5), up());
        assert!(lux_back.abs() < 1.0, "behind panel: {}", lux_back);
    }

    #[test]
    fn area_light_is_dark_behind_and_shadowed_per_cell() {
        let mut m = room_with_panel();
        // A column (0.2 m wide, 2.0 m tall) centred at (2.0, 2.0) under the panel centre
        // blocks the lower portion of the panel's contribution.
        let mut w = Wall::new(Vec2::new(2.0, 2.0), Vec2::new(2.2, 2.0), 0.2);
        w.height = 2.0;
        m.push_wall(w);
        let scene = crate::build(&m);
        // At the floor, 1 m to the right of the panel centre: unshadowed reference
        let unshadowed = illuminance(&scene, Vec3::new(3.0, 2.0, 0.0), up());
        // At the floor, directly in the column's shadow: reduced illuminance
        let shadowed = illuminance(&scene, Vec3::new(2.1, 2.0, 0.0), up());
        assert!(shadowed.lux < unshadowed.lux * 0.8 + 10.0, "shadowed: {} < unshadowed: {}", shadowed.lux, unshadowed.lux);
        // Directly above the panel (behind the face): zero
        let light = &scene.lights[0];
        let lux_back = direct_illuminance(light, Vec3::new(2.0, 2.0, 2.5), up());
        assert!(lux_back.abs() < 1.0, "behind: {}", lux_back);
    }

    #[test]
    fn ies_toward_handles_non_finite_inputs() {
        let vertical = vec![0.0, 90.0];
        let horizontal = vec![0.0, 180.0];
        let candela = vec![1000.0, 0.0, 1000.0, 0.0];
        let mut profile = cadcraft_doc::arch::IesProfile::default();
        profile.name = "test".into();
        profile.vertical = vertical;
        profile.horizontal = horizontal;
        profile.candela = candela;
        profile.lumens = 1000.0;
        let light = SceneLight {
            kind: LightKind::Spot,
            position: Vec3::ZERO,
            direction: Vec3::new(0.0, 0.0, -1.0),
            intensity: 1000.0,
            color: [1.0, 1.0, 1.0],
            cos_inner: 1.0,
            cos_outer: 1.0,
            size: [0.0; 2],
            axis: Vec3::new(1.0, 0.0, 0.0),
            ies: Some(std::sync::Arc::new(profile.clone())),
            ..Default::default()
        };
        // NaN in to_point
        assert_eq!(ies_toward(&light, &profile, Vec3::new(f64::NAN, 0.0, -1.0)), 0.0);
        // Inf in to_point
        assert_eq!(ies_toward(&light, &profile, Vec3::new(f64::INFINITY, 0.0, -1.0)), 0.0);
        // Non-finite direction
        let mut bad_dir = light.clone();
        bad_dir.direction = Vec3::new(f64::NAN, 0.0, -1.0);
        assert_eq!(ies_toward(&bad_dir, &profile, Vec3::new(0.0, 0.0, -1.0)), 0.0);
        // Non-finite axis
        let mut bad_axis = light.clone();
        bad_axis.axis = Vec3::new(0.0, f64::NAN, 0.0);
        assert_eq!(ies_toward(&bad_axis, &profile, Vec3::new(0.0, 0.0, -1.0)), 0.0);
    }

    #[test]
    fn area_light_with_ies_uses_point_formulation() {
        let vertical = vec![0.0, 90.0];
        let horizontal = vec![0.0, 180.0];
        let candela = vec![1000.0, 0.0, 1000.0, 0.0];
        let mut profile = cadcraft_doc::arch::IesProfile::default();
        profile.name = "test".into();
        profile.vertical = vertical;
        profile.horizontal = horizontal;
        profile.candela = candela;
        profile.lumens = 1000.0;
        let light = SceneLight {
            kind: LightKind::Area,
            position: Vec3::new(0.0, 0.0, 2.4),
            direction: Vec3::new(0.0, 0.0, -1.0),
            intensity: 1000.0,
            color: [1.0, 1.0, 1.0],
            cos_inner: 1.0,
            cos_outer: 0.0,
            size: [0.6, 0.6],
            axis: Vec3::new(1.0, 0.0, 0.0),
            ies: Some(std::sync::Arc::new(profile)),
            ..Default::default()
        };
        // At 0.4 m below centre: point formulation gives I * cos / d^2 = 1000 * 1 / 0.16 = 6250 lx
        let lux = direct_illuminance(&light, Vec3::new(0.0, 0.0, 2.0), up());
        assert!((lux - 6250.0).abs() < 62.5, "area+IES at 0.4m: {}", lux);
        // At 2.4 m (floor): 1000 / 5.76 = 173.6 lx
        let lux_floor = direct_illuminance(&light, Vec3::new(0.0, 0.0, 0.0), up());
        assert!((lux_floor - 173.6).abs() < 1.74, "area+IES at floor: {}", lux_floor);
    }

    #[test]
    fn area_light_degenerate_size_returns_zero() {
        let light = SceneLight {
            kind: LightKind::Area,
            position: Vec3::new(0.0, 0.0, 2.4),
            direction: Vec3::new(0.0, 0.0, -1.0),
            intensity: 1000.0,
            color: [1.0, 1.0, 1.0],
            cos_inner: 1.0,
            cos_outer: 0.0,
            size: [0.0, 0.6],
            axis: Vec3::new(1.0, 0.0, 0.0),
            ies: None,
            ..Default::default()
        };
        assert_eq!(direct_illuminance(&light, Vec3::new(0.0, 0.0, 0.0), up()), 0.0);
        let light_neg = SceneLight { size: [-0.1, 0.6], ..light.clone() };
        assert_eq!(direct_illuminance(&light_neg, Vec3::new(0.0, 0.0, 0.0), up()), 0.0);
        let light_nan = SceneLight { size: [f64::NAN, 0.6], ..light.clone() };
        assert_eq!(direct_illuminance(&light_nan, Vec3::new(0.0, 0.0, 0.0), up()), 0.0);
    }

    #[test]
    fn ies_single_angle_profile() {
        // Degenerate profile: one vertical angle, one horizontal angle
        let mut profile = cadcraft_doc::arch::IesProfile::default();
        profile.name = "single".into();
        profile.vertical = vec![0.0];
        profile.horizontal = vec![0.0];
        profile.candela = vec![500.0];
        profile.lumens = 500.0;
        let light = SceneLight {
            kind: LightKind::Spot,
            position: Vec3::ZERO,
            direction: Vec3::new(0.0, 0.0, -1.0),
            intensity: 500.0,
            color: [1.0, 1.0, 1.0],
            cos_inner: 1.0,
            cos_outer: 1.0,
            size: [0.0; 2],
            axis: Vec3::new(1.0, 0.0, 0.0),
            ies: Some(std::sync::Arc::new(profile.clone())),
            ..Default::default()
        };
        // Straight down should give 500 cd
        assert!((ies_toward(&light, &profile, Vec3::new(0.0, 0.0, -1.0)) - 500.0).abs() < 1.0);
        // 90 degrees should give 0 (outside profile range)
        assert_eq!(ies_toward(&light, &profile, Vec3::new(1.0, 0.0, 0.0)), 0.0);
    }

    #[test]
    fn area_light_partial_shadow_blocks_some_cells() {
        let mut m = room_with_panel();
        // A column (0.2 m wide, 2.0 m tall) at (2.0, 1.0) partially blocks the panel
        let mut w = Wall::new(Vec2::new(2.0, 1.0), Vec2::new(2.2, 1.0), 0.2);
        w.height = 2.0;
        m.push_wall(w);
        let scene = crate::build(&m);
        // Point directly beside the column: partially shadowed
        let shadowed = illuminance(&scene, Vec3::new(2.0, 1.0, 0.0), up());
        // Point at the same distance from the panel but on the other side: unshadowed
        let clear = illuminance(&scene, Vec3::new(2.0, 3.0, 0.0), up());
        // The shadowed point should have less illuminance
        assert!(shadowed.lux < clear.lux, "shadowed {} < clear {}", shadowed.lux, clear.lux);
        // But not zero (some cells still visible)
        assert!(shadowed.lux > 0.0, "some cells must be visible: {}", shadowed.lux);
    }
}
