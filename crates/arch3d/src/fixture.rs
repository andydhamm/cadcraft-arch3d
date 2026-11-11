//! Light-fixture housings in 3D (phase 10).

use crate::Mesh;
use cadcraft_arch::light::{housing_diameter, light_point};
use cadcraft_doc::arch::{ArchModel, FixtureShape, Level, LightFixture, Room};
use cadcraft_geom::{Line, Vec2, point_in_polygon};

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

/// A rotated rectangle of `w` x `l` around `c`, CCW, width along `rot_deg`.
fn rect(c: Vec2, w: f64, l: f64, rot_deg: f64) -> Vec<Vec2> {
    let hw = w / 2.0;
    let hl = l / 2.0;
    let a = rot_deg.to_radians();
    let (cos_a, sin_a) = (a.cos(), a.sin());
    // Local corners: (-hw, -hl), (hw, -hl), (hw, hl), (-hw, hl)
    // Rotate and translate
    let corners = [(-hw, -hl), (hw, -hl), (hw, hl), (-hw, hl)];
    corners.iter().map(|&(x, y)| Vec2::new(c.x + x * cos_a - y * sin_a, c.y + x * sin_a + y * cos_a)).collect()
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
    model.lights.iter().flat_map(|f| fixture_housing(f, light_point(model, f))).collect()
}

/// One strip light's housing: a 12 mm wide by 6 mm high box (`light-strip`) along each segment of
/// the path at the light height, plus a 1 mm lens (`light-lens-lit` when on, `light-lens` when
/// off) on the bottom face. Empty when the path has fewer than two points.
fn strip_housing(f: &LightFixture, zp: f64) -> Vec<Option<Mesh>> {
    if f.path.len() < 2 {
        return Vec::new();
    }
    let lens = if f.on { "light-lens-lit" } else { "light-lens" };
    let hw = 0.012 / 2.0;
    let mut parts: Vec<Option<Mesh>> = Vec::new();
    for segment in f.path.windows(2) {
        let (a, b) = (segment[0], segment[1]);
        if !a.is_finite() || !b.is_finite() {
            continue;
        }
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 || !len.is_finite() {
            continue;
        }
        let ux = dx / len;
        let uy = dy / len;
        let px = -uy * hw;
        let py = ux * hw;
        let ring = vec![Vec2::new(a.x + px, a.y + py), Vec2::new(b.x + px, b.y + py), Vec2::new(b.x - px, b.y - py), Vec2::new(a.x - px, a.y - py)];
        parts.push(prism("light-strip", f, &ring, zp - 0.006, zp));
        parts.push(prism(lens, f, &ring, zp - 0.006, zp - 0.005));
    }
    parts
}

/// One fixture's housing with its light at `point` ([`light_point`]); none without a point. The
/// incremental rebuild reuses it while the fixture and its point are unchanged.
pub fn fixture_housing(f: &LightFixture, point: Option<cadcraft_geom::Vec3>) -> Vec<Mesh> {
    let Some(p) = point else { return Vec::new() };
    let d = housing_diameter(f);
    if !p.is_finite() || !d.is_finite() || d <= 0.0 {
        return Vec::new();
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
        FixtureShape::Panel => {
            let w = if f.width > 0.0 { f.width } else { 0.6 };
            let l = if f.length > 0.0 { f.length } else { 0.6 };
            let outer = rect(c, w, l, f.rotation);
            let inner = rect(c, w - 0.06, l - 0.06, f.rotation);
            let mut frame_m = Mesh::new("light-trim", f.id);
            frame_m.push_prism_with_holes(&outer, &[inner.clone()], zp - 0.01, zp);
            let diffuser = prism(lens, f, &inner, zp - 0.004, zp - 0.002);
            vec![finish(frame_m), diffuser]
        }
        FixtureShape::Strip => strip_housing(f, zp),
    };
    parts.into_iter().flatten().collect()
}

/// Openings in `room`'s ceiling for the recessed cans and panels mounted in it.
///
/// For recessed cans, the disc of the trim. For panels, the outer rectangle.
/// A fixture is included if its light point lies within the 20 mm under the room's finished
/// ceiling at its point (`cadcraft_arch::ceiling::finished_ceiling_in`: the rough ceiling, less
/// the finish stack), it is on the room's level, and its cut shape fits entirely within the room
/// (not crossing edges or holes) and does not overlap with previous cuts.
pub fn ceiling_cutouts(model: &ArchModel, room: &Room, level: &Level) -> Vec<Vec<Vec2>> {
    if crate::floors::ceiling_top(room, level).is_none() {
        return Vec::new();
    }
    if room.polygon.len() < 3 {
        return Vec::new();
    }
    let rings = std::iter::once(&room.polygon).chain(room.holes.iter().filter(|h| h.len() >= 3));
    let edges: Vec<Line> = rings.flat_map(|r| (0..r.len()).map(move |k| Line::new(r[k], r[(k + 1) % r.len()]))).collect();

    // Store cuts as (center, radius) for collision detection.
    // For panels, radius is the half-diagonal.
    let mut cut: Vec<(Vec2, f64)> = Vec::new();
    let mut out: Vec<Vec<Vec2>> = Vec::new();

    for f in model.lights.iter().filter(|f| f.level == room.level) {
        if f.shape != FixtureShape::RecessedCan && f.shape != FixtureShape::Panel {
            continue;
        }

        let c = f.position;
        if !c.is_finite() {
            continue;
        }
        let Some(top) = cadcraft_arch::ceiling::finished_ceiling_in(model, room, c) else { continue };
        let Some(p) = light_point(model, f) else { continue };
        if !p.is_finite() {
            continue;
        }
        if p.z > top + 1e-9 || p.z < top - 0.02 - 1e-9 {
            continue;
        }

        if f.shape == FixtureShape::RecessedCan {
            let r = housing_diameter(f) / 2.0;
            if !r.is_finite() || r <= 0.0 {
                continue;
            }
            if !point_in_polygon(&room.polygon, c) {
                continue;
            }
            if room.holes.iter().any(|h| h.len() >= 3 && point_in_polygon(h, c)) {
                continue;
            }
            if edges.iter().any(|e| e.dist(c) <= r + 1e-6) {
                continue;
            }
            if cut.iter().any(|&(o, ro)| o.dist(c) <= r + ro + 1e-6) {
                continue;
            }
            cut.push((c, r));
            out.push(poly(c, r));
        } else if f.shape == FixtureShape::Panel {
            if !f.width.is_finite() || !f.length.is_finite() {
                continue;
            }
            let w = if f.width > 0.0 { f.width } else { 0.6 };
            let l = if f.length > 0.0 { f.length } else { 0.6 };

            let outer = rect(c, w, l, f.rotation);
            // Check if all corners are inside the room polygon and not in any hole
            if !outer.iter().all(|pt| point_in_polygon(&room.polygon, *pt)) {
                continue;
            }
            if room.holes.iter().any(|h| h.len() >= 3 && outer.iter().any(|pt| point_in_polygon(h, *pt))) {
                continue;
            }

            // Check distance to room edges.
            // Require every room-edge distance from the centre to exceed the half-diagonal.
            let half_diag = (w * w + l * l).sqrt() / 2.0;
            if edges.iter().any(|e| e.dist(c) <= half_diag + 1e-6) {
                continue;
            }

            // Check overlap with previous cuts using bounding circles
            if cut.iter().any(|&(o, ro)| o.dist(c) <= half_diag + ro + 1e-6) {
                continue;
            }

            cut.push((c, half_diag));
            out.push(outer);
        }
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

    /// A 4 x 4 m room under a 2.4 m ceiling with a recessed pot light at each of `at`.
    fn pots_in_a_room(at: &[(f64, f64)]) -> ArchModel {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let ring = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)].map(|(x, y)| Vec2::new(x, y)).to_vec();
        m.rooms.push(Room { id: ArchId(1), polygon: ring, ..Default::default() });
        let t = cadcraft_arch::light::find_type("Recessed pot light");
        for (i, &(x, y)) in at.iter().enumerate() {
            let id = ArchId(i as u32);
            let p = Vec2::new(x, y);
            m.lights.push(
                t.as_ref()
                    .map(|t| cadcraft_arch::light::fixture_from_type(id, t, p))
                    .unwrap_or_else(|| LightFixture::new(id, "Recessed pot light", p)),
            );
        }
        m
    }

    #[test]
    fn the_ceiling_opens_where_a_pot_light_sits() {
        let m = pots_in_a_room(&[(1.0, 1.0)]);
        let scene = crate::build(&m);
        // The trim ring (2.39 to 2.4 m) is inside the 20 mm ceiling slab's height: the slab leaves it a hole.
        let trim = scene.probe(cadcraft_geom::Vec3::new(1.065, 1.0, 2.395));
        assert_eq!(trim.as_ref().map(|h| h.part.as_str()), Some("light"));
        assert_eq!(trim.map(|h| h.id), Some(cadcraft_doc::arch::light_label(m.lights[0].id)));
        assert_eq!(scene.probe(cadcraft_geom::Vec3::new(1.0, 1.0, 2.385)), None, "open under the lens");
        let beside = scene.probe(cadcraft_geom::Vec3::new(2.0, 2.0, 2.39));
        assert_eq!(beside.map(|h| h.part), Some("ceiling".to_string()));
        let ceiling = scene.meshes.iter().find(|x| x.material == "ceiling");
        assert!(ceiling.is_some_and(|c| c.is_manifold()));
    }

    #[test]
    fn a_can_over_the_room_edge_or_on_another_cut_gets_none() {
        // (0.05, 2) reaches past the wall face at x = 0; (3.1, 3) overlaps the cut of (3, 3).
        let m = pots_in_a_room(&[(0.05, 2.0), (3.0, 3.0), (3.1, 3.0)]);
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        assert_eq!(cuts.len(), 1);
        assert!(cuts[0].iter().all(|p| (*p - Vec2::new(3.0, 3.0)).len() < 0.076));
        let scene = crate::build(&m);
        let ceiling = scene.meshes.iter().find(|x| x.material == "ceiling");
        assert!(ceiling.is_some_and(|c| c.is_manifold()));
    }

    #[test]
    fn a_can_hung_below_the_ceiling_or_in_another_room_gets_no_cut() {
        let mut m = pots_in_a_room(&[(1.0, 1.0), (6.0, 1.0)]);
        m.lights[0].height = Some(2.0);
        assert!(ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]).is_empty());
    }

    /// Phase 16: a can sits at the finished ceiling (rough minus the finish stack), and the cut
    /// compares its light point with that finished ceiling, so a stacked room still gets its cut.
    #[test]
    fn a_can_under_a_finish_stack_still_gets_its_cut() {
        let mut m = pots_in_a_room(&[(1.0, 1.0)]);
        m.rooms[0].ceiling_stack =
            vec![cadcraft_doc::arch::CeilingLayer { material: Some("drywall".into()), thickness: 0.0127, ..Default::default() }];
        let z = light_point(&m, &m.lights[0]).map(|p| p.z).unwrap_or(f64::NAN);
        assert!((z - 2.3873).abs() < 1e-9, "{z}");
        assert_eq!(ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]).len(), 1);
        // A can hung by hand at the rough ceiling is above the finished one: no cut.
        m.lights[0].height = Some(2.4);
        assert!(ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]).is_empty());
    }

    /// A 4 x 4 m room under a 2.4 m ceiling with a panel at `at`.
    fn panel_in_a_room(at: (f64, f64)) -> ArchModel {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let ring = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)].map(|(x, y)| Vec2::new(x, y)).to_vec();
        m.rooms.push(Room { id: ArchId(1), polygon: ring, ..Default::default() });
        let t = cadcraft_arch::light::find_type("LED panel 600");
        let id = ArchId(0);
        let p = Vec2::new(at.0, at.1);
        m.lights.push(t.as_ref().map(|t| cadcraft_arch::light::fixture_from_type(id, t, p)).unwrap_or_else(|| {
            let mut f = LightFixture::new(id, "LED panel 600", p);
            f.shape = FixtureShape::Panel;
            f.width = 0.6;
            f.length = 0.6;
            f
        }));
        m
    }

    #[test]
    fn panel_frame_and_diffuser_sit_in_the_ceiling() {
        let m = panel_in_a_room((2.0, 2.0));
        let scene = crate::build(&m);
        let zp = light_point(&m, &m.lights[0]).map(|p| p.z).unwrap_or(f64::NAN);

        // Frame is from zp - 0.01 to zp. Probe at 2.395 (inside 2.39 to 2.40)
        let frame_hit = scene.probe(cadcraft_geom::Vec3::new(2.28, 2.0, 2.395));
        assert_eq!(frame_hit.as_ref().map(|h| h.part.as_str()), Some("light"));
        assert_eq!(frame_hit.map(|h| h.id), Some(cadcraft_doc::arch::light_label(m.lights[0].id)));

        // Ceiling at (1,1) should still be there
        let ceiling_hit = scene.probe(cadcraft_geom::Vec3::new(1.0, 1.0, 2.39));
        assert_eq!(ceiling_hit.as_ref().map(|h| h.part.as_str()), Some("ceiling"));

        // Under diffuser (zp - 0.004 to zp - 0.002) -> 2.396 to 2.398.
        // Probe at 2.385 should be open (no ceiling, no light part)
        let open_hit = scene.probe(cadcraft_geom::Vec3::new(2.0, 2.0, 2.385));
        assert_eq!(open_hit, None, "open under the diffuser");

        // Check ceiling mesh is manifold
        let ceiling = scene.meshes.iter().find(|x| x.material == "ceiling");
        assert!(ceiling.is_some_and(|c| c.is_manifold()));
    }

    #[test]
    fn the_ceiling_opens_for_a_panel() {
        let m = panel_in_a_room((2.0, 2.0));
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        assert_eq!(cuts.len(), 1);
        // Check the cut is a rectangle of 0.6x0.6 centered at 2,2
        let cut = &cuts[0];
        assert_eq!(cut.len(), 4);
        // Vertices should be roughly (1.7, 1.7), (2.3, 1.7), (2.3, 2.3), (1.7, 2.3)
        let expected = [Vec2::new(1.7, 1.7), Vec2::new(2.3, 1.7), Vec2::new(2.3, 2.3), Vec2::new(1.7, 2.3)];
        for (i, p) in cut.iter().enumerate() {
            assert!((p.x - expected[i].x).abs() < 1e-6);
            assert!((p.y - expected[i].y).abs() < 1e-6);
        }
    }

    #[test]
    fn a_panel_over_the_room_edge_gets_no_cut() {
        // Panel at (0.2, 2) with width 0.6 -> extends to x = -0.1, outside room
        let m = panel_in_a_room((0.2, 2.0));
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        assert!(cuts.is_empty());
    }

    #[test]
    fn panel_lens_material_follows_on_and_off() {
        let mut m = panel_in_a_room((2.0, 2.0));
        assert!(fixture_meshes(&m).iter().any(|x| x.material == "light-lens-lit"));
        m.lights[0].on = false;
        let meshes = fixture_meshes(&m);
        assert!(meshes.iter().any(|x| x.material == "light-lens"));
        assert!(!meshes.iter().any(|x| x.material == "light-lens-lit"));
    }

    #[test]
    fn rotated_panel_still_cuts_ceiling() {
        let mut m = panel_in_a_room((2.0, 2.0));
        m.lights[0].rotation = 90.0;
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        assert_eq!(cuts.len(), 1);
        let cut = &cuts[0];
        assert_eq!(cut.len(), 4);
        // For a square rotated 90 degrees, the set of corners is the same
        let scene = crate::build(&m);
        let frame_hit = scene.probe(cadcraft_geom::Vec3::new(2.28, 2.0, 2.395));
        assert_eq!(frame_hit.as_ref().map(|h| h.part.as_str()), Some("light"));
        let ceiling = scene.meshes.iter().find(|x| x.material == "ceiling");
        assert!(ceiling.is_some_and(|c| c.is_manifold()));
    }

    #[test]
    fn rotated_rectangular_panel_cuts_correctly() {
        let mut m = panel_in_a_room((2.0, 2.0));
        m.lights[0].width = 0.4;
        m.lights[0].length = 0.8;
        m.lights[0].rotation = 90.0;
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        assert_eq!(cuts.len(), 1);
        let cut = &cuts[0];
        assert_eq!(cut.len(), 4);
        // After 90-degree rotation, width and length swap
        // Original corners: (1.8, 1.6), (2.2, 1.6), (2.2, 2.4), (1.8, 2.4)
        // Rotated 90 degrees: (2.2, 1.8), (2.2, 2.2), (1.8, 2.2), (1.8, 1.8)
        // But as a set, the bounding box is now 0.8 wide and 0.4 tall
        let xs: Vec<f64> = cut.iter().map(|p| p.x).collect();
        let ys: Vec<f64> = cut.iter().map(|p| p.y).collect();
        let x_range = xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min);
        let y_range = ys.iter().cloned().fold(f64::MIN, f64::max) - ys.iter().cloned().fold(f64::MAX, f64::min);
        assert!((x_range - 0.8).abs() < 1e-6, "x range should be 0.8 after rotation");
        assert!((y_range - 0.4).abs() < 1e-6, "y range should be 0.4 after rotation");
    }

    #[test]
    fn panel_with_custom_dimensions() {
        let mut m = panel_in_a_room((2.0, 2.0));
        m.lights[0].width = 0.3;
        m.lights[0].length = 0.4;
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        assert_eq!(cuts.len(), 1);
        let cut = &cuts[0];
        assert_eq!(cut.len(), 4);
        let expected = [Vec2::new(1.85, 1.8), Vec2::new(2.15, 1.8), Vec2::new(2.15, 2.2), Vec2::new(1.85, 2.2)];
        for (i, p) in cut.iter().enumerate() {
            assert!((p.x - expected[i].x).abs() < 1e-6);
            assert!((p.y - expected[i].y).abs() < 1e-6);
        }
    }

    #[test]
    fn two_panels_that_do_not_overlap() {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let ring = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)].map(|(x, y)| Vec2::new(x, y)).to_vec();
        m.rooms.push(Room { id: ArchId(1), polygon: ring, ..Default::default() });
        let t = cadcraft_arch::light::find_type("LED panel 600");
        for (i, (x, y)) in [(1.0, 2.0), (3.0, 2.0)].iter().enumerate() {
            let id = ArchId(i as u32);
            let p = Vec2::new(*x, *y);
            m.lights.push(t.as_ref().map(|t| cadcraft_arch::light::fixture_from_type(id, t, p)).unwrap_or_else(|| {
                let mut f = LightFixture::new(id, "LED panel 600", p);
                f.shape = FixtureShape::Panel;
                f.width = 0.6;
                f.length = 0.6;
                f
            }));
        }
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        assert_eq!(cuts.len(), 2);
        let scene = crate::build(&m);
        let ceiling = scene.meshes.iter().find(|x| x.material == "ceiling");
        assert!(ceiling.is_some_and(|c| c.is_manifold()));
    }

    #[test]
    fn panel_with_nan_position_is_skipped() {
        let mut m = panel_in_a_room((2.0, 2.0));
        m.lights[0].position = Vec2::new(f64::NAN, 2.0);
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        assert!(cuts.is_empty());
    }

    #[test]
    fn a_panel_larger_than_the_room_gets_no_cut() {
        let mut m = panel_in_a_room((2.0, 2.0));
        m.lights[0].width = 5.0;
        m.lights[0].length = 5.0;
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        assert!(cuts.is_empty());
    }

    #[test]
    fn a_panel_touching_the_room_edge_gets_no_cut() {
        // Panel at (2.0, 2.0) with width 4.0 touches both left and right edges exactly
        let mut m = panel_in_a_room((2.0, 2.0));
        m.lights[0].width = 4.0;
        m.lights[0].length = 0.6;
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        assert!(cuts.is_empty());
    }

    #[test]
    fn overlapping_panels_get_separate_cuts() {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let ring = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)].map(|(x, y)| Vec2::new(x, y)).to_vec();
        m.rooms.push(Room { id: ArchId(1), polygon: ring, ..Default::default() });
        let t = cadcraft_arch::light::find_type("LED panel 600");
        // Two panels that overlap: at (1.0, 2.0) and (1.3, 2.0) with width 0.6
        for (i, (x, y)) in [(1.0, 2.0), (1.3, 2.0)].iter().enumerate() {
            let id = ArchId(i as u32);
            let p = Vec2::new(*x, *y);
            m.lights.push(t.as_ref().map(|t| cadcraft_arch::light::fixture_from_type(id, t, p)).unwrap_or_else(|| {
                let mut f = LightFixture::new(id, "LED panel 600", p);
                f.shape = FixtureShape::Panel;
                f.width = 0.6;
                f.length = 0.6;
                f
            }));
        }
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        // The second panel overlaps the first, so only one cut
        assert_eq!(cuts.len(), 1);
    }

    #[test]
    fn panel_with_zero_dimensions_is_skipped() {
        let mut m = panel_in_a_room((2.0, 2.0));
        m.lights[0].width = 0.0;
        m.lights[0].length = 0.0;
        // Zero dimensions fall back to defaults (0.6), so a cut should still be made
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        assert_eq!(cuts.len(), 1);
    }

    #[test]
    fn panel_with_nan_dimensions_is_skipped() {
        let mut m = panel_in_a_room((2.0, 2.0));
        m.lights[0].width = f64::NAN;
        m.lights[0].length = 0.6;
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        assert!(cuts.is_empty());
    }

    #[test]
    fn panel_in_room_with_hole_is_skipped() {
        let mut m = panel_in_a_room((2.0, 2.0));
        // Add a hole that the panel corners fall into
        let hole = [Vec2::new(1.5, 1.5), Vec2::new(2.5, 1.5), Vec2::new(2.5, 2.5), Vec2::new(1.5, 2.5)];
        m.rooms[0].holes.push(hole.to_vec());
        let cuts = ceiling_cutouts(&m, &m.rooms[0], &m.levels[0]);
        assert!(cuts.is_empty());
    }
}
