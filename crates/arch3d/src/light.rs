//! Light fixtures in 3D (phase 10): each fixture's housing (meshes with a `light-…` material,
//! probe part `light`) and its light ([`SceneLight`]), plus direct illuminance with shadow rays.
//!
//! Fixture housings never cast shadows, in the CPU [`illuminance`] or on the GPU: moving a
//! fixture changes no other light's shadows. `scene_lights`, `direct_illuminance` and
//! `illuminance` are stubs filled by `p10-arch3d-scene-lights` (signatures final); the housings
//! are in `crate::fixture`.

use crate::Scene3d;
use cadcraft_doc::ArchId;
use cadcraft_doc::arch::ArchModel;
pub use cadcraft_doc::arch::LightKind;
use cadcraft_geom::Vec3;

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
    /// `-1, -1` for a point light. `cos_inner >= cos_outer`.
    pub cos_inner: f64,
    pub cos_outer: f64,
}

impl SceneLight {
    /// Intensity (cd) toward the unit direction `to_point` (from the light to the lit point):
    /// `intensity` inside the cone, 0 outside the falloff, a smoothstep between them; a point
    /// light is `intensity` everywhere. A direction exactly on the cone edge counts as inside.
    pub fn intensity_toward(&self, to_point: Vec3) -> f64 {
        if self.kind == LightKind::Point {
            return self.intensity;
        }
        let c = to_point.dot(self.direction);
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

/// Whether a mesh material belongs to a light fixture housing (`light-trim`, `light-lens`, …).
pub fn is_light_material(material: &str) -> bool {
    material.starts_with("light-")
}

/// One [`SceneLight`] per fixture that is on and has a light point
/// (`arch::light::light_point`), in fixture order.
pub fn scene_lights(model: &ArchModel) -> Vec<SceneLight> {
    let mut out = Vec::new();
    for f in model.lights.iter().filter(|f| f.on) {
        let Some(position) = cadcraft_arch::light::light_point(model, f) else {
            continue;
        };
        let aim = cadcraft_arch::light::aim_direction(f);
        let intensity = cadcraft_arch::light::intensity_cd(f);
        let color = cadcraft_arch::light::kelvin_to_rgb(f.kelvin);
        if !position.is_finite() || !aim.is_finite() || aim.len() < 1e-12 || !intensity.is_finite() || !color.iter().all(|c| c.is_finite()) {
            continue;
        }
        let (cos_inner, cos_outer) = match f.kind {
            LightKind::Point => (-1.0, -1.0),
            LightKind::Spot => ((f.cone.to_radians() * 0.5).cos(), (f.falloff.to_radians() * 0.5).cos()),
        };
        if !cos_inner.is_finite() || !cos_outer.is_finite() {
            continue;
        }
        out.push(SceneLight { id: f.id, kind: f.kind, position, direction: aim.normalized(), intensity, color, cos_inner, cos_outer });
    }
    out
}

/// The housings of every fixture (in `crate::fixture`, filled by `p10-arch3d-fixture-meshes`).
pub use crate::fixture::fixture_meshes;

/// Direct illuminance (lux) from one light at `point` on a surface with unit `normal`, without
/// occlusion: `I(θ) · max(0, n·l) / d²` with `l` the unit vector from the point to the light.
/// 0 when the point is at the light (d < 1e-9) or behind the surface.
pub fn direct_illuminance(light: &SceneLight, point: Vec3, normal: Vec3) -> f64 {
    let v = light.position - point;
    let d2 = v.dot(v);
    if !d2.is_finite() || d2 < 1e-18 || !normal.is_finite() {
        return 0.0;
    }
    let nl = normal.len();
    if !nl.is_finite() || nl <= 0.0 {
        return 0.0;
    }
    let l = v * (1.0 / d2.sqrt());
    let cos = (normal * (1.0 / nl)).dot(l).max(0.0);
    let e = light.intensity_toward(-l) * cos / d2;
    if e.is_finite() { e } else { 0.0 }
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
            .filter(|m| crate::probe::part_of(m) != "floor")
            .filter(|m| crate::probe::part_of(m) != "ceiling")
            .cloned()
            .collect(),
        ..Default::default()
    });
    let mut out = Illuminance::default();
    for light in &scene.lights {
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
                        // Ignore hits right at the lit point or right next to the light: the floor,
                        // the ceiling plane and the fixture housing can sit on those edges without
                        // making the beam truly shadowed. Other geometry still blocks it.
                        let along = hit.distance;
                        if along > start_margin && along < max_t - end_margin {
                            e = 0.0;
                        }
                    }
                }
            }
        }
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
}
