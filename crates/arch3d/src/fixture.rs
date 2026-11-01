//! Light-fixture housings in 3D (phase 10).

use crate::Mesh;
use cadcraft_arch::light::{housing_diameter, light_point};
use cadcraft_doc::arch::{ArchModel, FixtureShape, LightFixture};
use cadcraft_geom::Vec2;

/// Sides of a round part.
const SIDES: usize = 16;

/// A regular 16-gon of `radius` around `c`, counter-clockwise.
fn poly(c: Vec2, radius: f64) -> Vec<Vec2> {
    (0..SIDES)
        .map(|k| {
            let a = std::f64::consts::TAU * k as f64 / SIDES as f64;
            Vec2::new(c.x + radius * a.cos(), c.y + radius * a.sin())
        })
        .collect()
}

/// A square of side `s` around `c`.
fn square(c: Vec2, s: f64) -> Vec<Vec2> {
    let h = s / 2.0;
    vec![Vec2::new(c.x - h, c.y - h), Vec2::new(c.x + h, c.y - h), Vec2::new(c.x + h, c.y + h), Vec2::new(c.x - h, c.y + h)]
}

/// The half disc of `radius` facing `rotation_deg`, closed by its chord.
fn half_disc(c: Vec2, radius: f64, rotation_deg: f64) -> Vec<Vec2> {
    (0..=12)
        .map(|k| {
            let a = (rotation_deg - 90.0 + 15.0 * k as f64).to_radians();
            Vec2::new(c.x + radius * a.cos(), c.y + radius * a.sin())
        })
        .collect()
}

/// A prism of `ring` between `z0` and `z1`; `None` when the result is empty or non-finite.
fn prism(material: &str, f: &LightFixture, ring: &[Vec2], z0: f64, z1: f64) -> Option<Mesh> {
    if !z0.is_finite() || !z1.is_finite() || ring.iter().any(|p| !p.is_finite()) {
        return None;
    }
    let mut m = Mesh::new(material, f.id);
    m.push_prism(ring, z0, z1);
    finish(m)
}

fn finish(m: Mesh) -> Option<Mesh> {
    let ok = !m.indices.is_empty() && m.positions.iter().all(|p| p.iter().all(|v| v.is_finite()));
    ok.then_some(m)
}

/// The housings of every fixture (on or off): closed prisms with the fixture's id as `object`
/// and `light-…` materials (`light-trim`, `light-shade`, `light-cord`, and `light-lens-lit` for a
/// fixture that is on or `light-lens` for one that is off).
pub fn fixture_meshes(model: &ArchModel) -> Vec<Mesh> {
    let mut out = Vec::new();
    for f in &model.lights {
        let Some(p) = light_point(model, f) else { continue };
        let d = housing_diameter(f);
        if !p.is_finite() || !d.is_finite() || d <= 0.0 {
            continue;
        }
        let r = d / 2.0;
        let c = f.position;
        let zp = p.z;
        let lens = if f.on { "light-lens-lit" } else { "light-lens" };
        let parts: Vec<Option<Mesh>> = match f.shape {
            FixtureShape::RecessedCan => {
                let mut trim = Mesh::new("light-trim", f.id);
                trim.push_prism_with_holes(&poly(c, r), &[poly(c, r - 0.02)], zp - 0.01, zp);
                vec![finish(trim), prism(lens, f, &poly(c, r - 0.02), zp - 0.004, zp - 0.002)]
            }
            FixtureShape::SurfaceDome => {
                let zc = zp + 0.05;
                vec![prism("light-trim", f, &poly(c, r + 0.01), zc - 0.01, zc), prism(lens, f, &poly(c, r), zc - 0.08, zc - 0.01)]
            }
            FixtureShape::Pendant => vec![
                prism("light-shade", f, &poly(c, r), zp - 0.02, zp + 0.18),
                prism(lens, f, &poly(c, r - 0.01), zp - 0.03, zp - 0.02),
                prism("light-cord", f, &square(c, 0.012), zp + 0.18, zp + 0.6),
            ],
            FixtureShape::Sconce => vec![
                prism("light-shade", f, &half_disc(c, r, f.rotation), zp - 0.125, zp + 0.125),
                prism(lens, f, &half_disc(c, r - 0.01, f.rotation), zp - 0.135, zp - 0.125),
            ],
            FixtureShape::FloorLamp => {
                let zf = zp - f.height.unwrap_or(1.5);
                vec![
                    prism("light-trim", f, &poly(c, 0.15), zf, zf + 0.02),
                    prism("light-cord", f, &square(c, 0.025), zf + 0.02, zp - 0.15),
                    prism("light-shade", f, &poly(c, r), zp - 0.15, zp + 0.1),
                    prism(lens, f, &poly(c, 0.03), zp - 0.03, zp + 0.03),
                ]
            }
        };
        out.extend(parts.into_iter().flatten());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;

    fn model_with(names: &[&str]) -> ArchModel {
        let mut m = ArchModel::default();
        for (i, n) in names.iter().enumerate() {
            let mut f = LightFixture::new(ArchId(i as u32 + 1), n, Vec2::new(1.0 + i as f64, 1.0));
            if let Some(t) = cadcraft_arch::light::find_type(n) {
                f = cadcraft_arch::light::fixture_from_type(f.id, &t, f.position);
            }
            m.lights.push(f);
        }
        m
    }

    const ALL: [&str; 5] = ["Recessed pot light", "Pendant", "Wall sconce", "Surface ceiling fixture", "Floor lamp"];

    #[test]
    fn pot_light_trim_sits_flush_under_the_ceiling() {
        let m = model_with(&["Recessed pot light"]);
        let zp = light_point(&m, &m.lights[0]).map(|p| p.z).unwrap_or(f64::NAN);
        let meshes = fixture_meshes(&m);
        let trim = meshes.iter().find(|x| x.material == "light-trim");
        assert!(trim.is_some());
        let b = trim.map(|t| t.bounds()).unwrap_or(crate::mesh::Bounds3::empty());
        assert!((b.max[2] - zp).abs() < 1e-5);
        assert!((b.min[2] - (zp - 0.01)).abs() < 1e-5);
    }

    #[test]
    fn every_fixture_mesh_is_closed() {
        let m = model_with(&ALL);
        let meshes = fixture_meshes(&m);
        assert!(meshes.len() >= 12);
        for mesh in &meshes {
            assert!(mesh.is_manifold(), "{} not closed", mesh.material);
        }
    }

    #[test]
    fn lens_material_follows_on_and_off() {
        let mut m = model_with(&["Pendant"]);
        assert!(fixture_meshes(&m).iter().any(|x| x.material == "light-lens-lit"));
        m.lights[0].on = false;
        let meshes = fixture_meshes(&m);
        assert!(meshes.iter().any(|x| x.material == "light-lens"));
        assert!(!meshes.iter().any(|x| x.material == "light-lens-lit"));
    }

    #[test]
    fn pendant_hangs_below_the_ceiling_on_a_cord() {
        let m = model_with(&["Pendant"]);
        let zp = light_point(&m, &m.lights[0]).map(|p| p.z).unwrap_or(f64::NAN);
        let meshes = fixture_meshes(&m);
        let cord = meshes.iter().find(|x| x.material == "light-cord").map(|c| c.bounds());
        let cord = cord.unwrap_or(crate::mesh::Bounds3::empty());
        assert!((cord.max[2] - (zp + 0.6)).abs() < 1e-5);
        assert!((cord.min[2] - (zp + 0.18)).abs() < 1e-5);
        let shade = meshes.iter().find(|x| x.material == "light-shade").map(|c| c.bounds());
        let shade = shade.unwrap_or(crate::mesh::Bounds3::empty());
        assert!((shade.min[2] - (zp - 0.02)).abs() < 1e-5);
    }

    #[test]
    fn probe_hits_the_light_part() {
        let m = model_with(&["Pendant"]);
        let zp = light_point(&m, &m.lights[0]).map(|p| p.z).unwrap_or(f64::NAN);
        let scene = crate::build(&m);
        let pos = m.lights[0].position;
        let hit = scene.probe(cadcraft_geom::Vec3::new(pos.x, pos.y, zp + 0.1));
        assert_eq!(hit.as_ref().map(|h| h.part.as_str()), Some("light"));
        assert_eq!(hit.map(|h| h.id), Some(cadcraft_doc::arch::light_label(m.lights[0].id)));
    }
}
