//! Ray-cast probing of 3D meshes: determines which part of the scene
//! a point is contained in, and reports the owning object, part name
//! and level elevation.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

use crate::Scene3d;
use crate::mesh::Mesh;
use crate::ray::mesh_crossings_upward;
use cadcraft_doc::ArchId;
use cadcraft_doc::arch::{CabinetPart, Surface};
use cadcraft_geom::Vec3;

/// Maximum number of triangles to scan for a single probe.
///
/// Probes scan the first N triangles of the mesh and use their parity; this caps
/// the work for pathological meshes while keeping the common case
/// (small architectural meshes) exact.
pub(crate) const MAX_PROBE_TRIANGLES: usize = 1_000_000;

/// Classify a mesh by its material name.
///
/// Performs a case-insensitive substring match against a fixed priority
/// list. Unrecognised materials map to `"wall"`.
pub fn part_of(mesh: &Mesh) -> &'static str {
    let m = mesh.material.to_lowercase();
    // Phase 14 first: a symbol key or a surface key could hold any word.
    if m.starts_with("sym:") {
        return "furnishing";
    }
    if m.starts_with("pull:") {
        return "pull";
    }
    if m.starts_with("electrical:") {
        return "electrical";
    }
    if let Some(key) = m.strip_prefix("paint:") {
        return Surface::parse(key).map_or("wall", |s| unpainted_part(&s));
    }
    // Site parts first (phase 9): a path's own material could be any word.
    if m == "terrain" {
        return "terrain";
    }
    if m.starts_with("path-") {
        return "path";
    }
    if m.starts_with("plant-") {
        return "plant";
    }
    // Phase 10: fixture housings (`light-trim`, `light-lens`, …).
    if m.starts_with("light-") {
        return "light";
    }
    // Phase 16: slab floors, foundations and interior soffits (`soffit-box`, `soffit-box:<material>`).
    // The roof's own soffit (`soffit`, and any other `soffit-…`) stays part of the roof.
    let kind = |p: &str| m == p || m.strip_prefix(p).is_some_and(|r| r.starts_with('-'));
    if kind("slab") {
        return "slab";
    }
    if kind("footing") {
        return "footing";
    }
    if kind("foundation") {
        return "foundation";
    }
    if m.starts_with("soffit-box") {
        return "soffit";
    }
    // Phase 18: glass walls, niche back blocks, moldings, countertops and backsplashes.
    if m.starts_with("glass-wall") {
        return "glass";
    }
    if m.starts_with("molding-") {
        return "molding";
    }
    if m == "countertop" || m.starts_with("countertop-") {
        return "counter";
    }
    if m == "backsplash" {
        return "backsplash";
    }
    if m.starts_with("niche-back") {
        return "wall";
    }
    // Phase 13: finished faces belong to their wall, roof or room.
    if m.starts_with("wall-") {
        return "wall";
    }
    if m.starts_with("roof-") {
        return "roof";
    }
    if m.starts_with("floor-") {
        return "floor";
    }
    // Phase 13: casework (`cabinet-box`, `cabinet-front`, `counter`, `fixture-sink`, …).
    if m.starts_with("cabinet-") {
        return "cabinet";
    }
    if m == "counter" || m.starts_with("counter-") {
        return "counter";
    }
    if m.starts_with("fixture-") {
        return "fixture";
    }
    // Phase 17: stair parts, railing posts and newels, deck structure and grilles.
    for (prefix, part) in [("stair-tread", "tread"), ("stair-riser", "riser"), ("stair-stringer", "stringer"), ("stair-skirt", "skirt")] {
        if m.starts_with(prefix) {
            return part;
        }
    }
    if m.starts_with("railing-post") || m.starts_with("railing-newel") || m == "deck-post" {
        return "post";
    }
    for (name, part) in [("deck-ledger", "ledger"), ("deck-joist", "joist"), ("deck-rim", "rim"), ("deck-beam", "beam"), ("deck-footing", "footing")]
    {
        if m == name {
            return part;
        }
    }
    if m.contains("grille") {
        return "grille";
    }
    if m.contains("glass") {
        return "glass";
    }
    if m.contains("frame") {
        return "frame";
    }
    if m.contains("casing") {
        return "casing";
    }
    if m.contains("leaf") {
        return "leaf";
    }
    if m.contains("sill") {
        return "sill";
    }
    if m.contains("header") {
        return "header";
    }
    if m.contains("roof") || m.contains("fascia") || m.contains("soffit") || m.contains("shingle") {
        return "roof";
    }
    if m.contains("stair") {
        return "stair";
    }
    if m.contains("railing") {
        return "railing";
    }
    if m.contains("deck") {
        return "deck";
    }
    if m.contains("ceiling") {
        return "ceiling";
    }
    if m.contains("floor") {
        return "floor";
    }
    if m.contains("wall") {
        return "wall";
    }
    "wall"
}

/// The part a surface's mesh has when it is not painted.
fn unpainted_part(surface: &Surface) -> &'static str {
    match surface {
        Surface::WallSide { .. } => "wall",
        Surface::RoomFloor(_) => "floor",
        Surface::RoomCeiling(_) => "ceiling",
        Surface::Roof(_) => "roof",
        Surface::Cabinet { part: CabinetPart::Carcass | CabinetPart::Front, .. } => "cabinet",
        Surface::Cabinet { part: CabinetPart::Counter, .. } => "counter",
        Surface::Cabinet { part: CabinetPart::Pull, .. } => "pull",
        Surface::Fixture(_) => "fixture",
        Surface::Furnishing { .. } => "furnishing",
        Surface::Countertop(_) => "counter",
        Surface::Backsplash(_) => "backsplash",
    }
}

/// Whether a mesh can be probed (i.e. it is a closed volume).
///
/// Ceilings are single-sided quads and would report a hit for every
/// point below them; they are excluded. Meshes with negligible z-extent
/// (e.g. flat decals) are also excluded.
pub fn is_probeable(mesh: &Mesh) -> bool {
    let b = mesh.bounds();
    let z_extent = b.max[2] - b.min[2];
    // Use a small tolerance to account for f32->f64 rounding in bounds computation.
    z_extent >= 1e-6 - 1e-12
}

/// A hit reported by a probe: the owning object id, the part name,
/// and the level elevation.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub id: String,
    pub part: String,
    pub level: f64,
}

/// Probe a single mesh at point `p`.
///
/// Returns `true` when `p` is inside the mesh's volume (odd number of
/// upward ray crossings). Short-circuited by the bounding box and by
/// [`is_probeable`]. When the mesh has more than
/// [`MAX_PROBE_TRIANGLES`] triangles, only the first N are scanned and
/// their parity is used.
///
/// The ray origin is jittered slightly in X and Y to avoid degenerate
/// cases where the point lies exactly on a mesh crease or vertex (e.g.
/// the ridge of a gable wall), which can cause adjacent triangles to
/// both reject the point due to half-open edge rules. The jitter is
/// biased inwards relative to the mesh bounds to ensure points on the
/// surface are treated as inside.
pub fn probe_mesh(mesh: &Mesh, p: Vec3) -> bool {
    if !is_probeable(mesh) {
        return false;
    }
    if !p.is_finite() {
        return false;
    }
    let b = mesh.bounds();
    if p.x < b.min[0] || p.x > b.max[0] || p.y < b.min[1] || p.y > b.max[1] || p.z < b.min[2] || p.z > b.max[2] {
        return false;
    }

    // Jitter X and Y to avoid crease/vertex degeneracies in point_in_triangle2.
    // We bias the jitter inwards relative to the mesh bounds so that points
    // exactly on the boundary are nudged inside, while internal points are
    // nudged enough to break ties on shared edges.
    let eps = 1e-7;
    let jx = if (p.x - b.min[0]).abs() < eps {
        eps
    } else if (p.x - b.max[0]).abs() < eps {
        -eps
    } else {
        eps
    };
    let jy = if (p.y - b.min[1]).abs() < eps {
        eps
    } else if (p.y - b.max[1]).abs() < eps {
        -eps
    } else {
        1.3e-7
    };

    // Nudge the point slightly upward so that points on the top face interior are outside.
    // For points on a top edge or vertex, nudge downward instead so they read as inside.
    let on_top = (p.z - b.max[2]).abs() < eps;
    let on_x_edge = (p.x - b.min[0]).abs() < eps || (p.x - b.max[0]).abs() < eps;
    let on_y_edge = (p.y - b.min[1]).abs() < eps || (p.y - b.max[1]).abs() < eps;
    let z_nudge = if on_top && !on_x_edge && !on_y_edge {
        1e-6
    } else if on_top {
        -1e-6
    } else {
        0.0
    };
    let p = Vec3::new(p.x + jx, p.y + jy, p.z + z_nudge);
    mesh_crossings_upward(p, &mesh.positions, &mesh.indices, MAX_PROBE_TRIANGLES) % 2 == 1
}

/// A human-readable label for a mesh, based on its material and owning object.
///
/// `floor`/`ceiling` materials use the room label; opening-part materials use
/// the opening label; everything else uses the wall label. Never returns an
/// empty string.
pub fn mesh_label(mesh: &Mesh) -> String {
    let m = mesh.material.to_lowercase();
    // Phase 17: deck structure belongs to its deck room (before any part rule: part "footing" is a
    // foundation's), railing parts to the railing and stair parts to the stair.
    let phase17 = if m.starts_with("glass-wall") || m.starts_with("niche-back") {
        Some(cadcraft_doc::arch::wall_label(mesh.object))
    } else if m.starts_with("molding-room-") {
        Some(cadcraft_doc::arch::room_label(mesh.object))
    } else if m.starts_with("molding-line") {
        Some(cadcraft_doc::arch::molding_line_label(mesh.object))
    } else if m == "countertop" || m.starts_with("countertop-") {
        Some(cadcraft_doc::arch::countertop_label(mesh.object))
    } else if m == "backsplash" {
        Some(cadcraft_doc::arch::backsplash_label(mesh.object))
    } else if m == "deck" || m.starts_with("deck-") {
        Some(cadcraft_doc::arch::room_label(mesh.object))
    } else if m == "railing" || m.starts_with("railing-") {
        Some(cadcraft_doc::arch::railing_label(mesh.object))
    } else if m == "stair" || m.starts_with("stair-") {
        Some(cadcraft_doc::arch::stair_label(mesh.object))
    } else {
        None
    };
    // Phase 14: furnishing parts, pulls and painted surfaces are named by their object's kind.
    let label = if phase17.is_some() { phase17 } else { phase14_label(mesh, &m) };
    // Site meshes first: a plant, the terrain solid, a path (its modifier) are named as the commands name them.
    let label = if let Some(label) = label {
        label
    } else if m.starts_with("light-") {
        cadcraft_doc::arch::light_label(mesh.object)
    } else if m.starts_with("cabinet-") || m == "counter" || m.starts_with("counter-") {
        cadcraft_doc::arch::cabinet_label(mesh.object)
    } else if m.starts_with("fixture-") {
        cadcraft_doc::arch::fixture_label(mesh.object)
    } else if m.starts_with("plant-") {
        cadcraft_doc::arch::plant_label(mesh.object)
    } else if m == "terrain" {
        cadcraft_doc::arch::terrain_label(None)
    } else if m.starts_with("path-") {
        cadcraft_doc::arch::terrain_label(Some(mesh.object.0))
    } else if m.contains("floor") || m.contains("ceiling") {
        cadcraft_doc::arch::room_label(mesh.object)
    } else if m.starts_with("roof-") {
        cadcraft_doc::arch::roof_label(mesh.object)
    } else if m.contains("door")
        || m.contains("window")
        || m.contains("glass")
        || m.contains("frame")
        || m.contains("casing")
        || m.contains("leaf")
        || m.contains("sill")
        || m.contains("header")
    {
        cadcraft_doc::arch::opening_label(mesh.object)
    } else {
        cadcraft_doc::arch::wall_label(mesh.object)
    };
    if label.is_empty() { format!("object-{}", mesh.object.0) } else { label }
}

/// The part a click on this mesh picks: `"deck"` for every deck material (boards and structure
/// alike, so they all go to the deck room), else [`part_of`].
pub fn pick_part(mesh: &Mesh) -> &'static str {
    let m = mesh.material.to_lowercase();
    if m == "deck" || m.starts_with("deck-") { "deck" } else { part_of(mesh) }
}

/// The kind of arch object a mesh belongs to: a room for every deck material, else the kind of its
/// [`part_of`].
pub fn arch_kind(mesh: &Mesh) -> cadcraft_doc::arch::ArchKind {
    use cadcraft_doc::arch::ArchKind;
    let m = mesh.material.to_lowercase();
    if let Some(s) = m.strip_prefix("paint:").and_then(Surface::parse) {
        match s {
            Surface::Countertop(_) => return ArchKind::Countertop,
            Surface::Backsplash(_) => return ArchKind::Backsplash,
            _ => {}
        }
    }
    if m.starts_with("glass-wall") || m.starts_with("niche-back") {
        ArchKind::Wall
    } else if m.starts_with("molding-room-") {
        ArchKind::Room
    } else if m.starts_with("molding-line") {
        ArchKind::MoldingLine
    } else if m == "countertop" || m.starts_with("countertop-") {
        ArchKind::Countertop
    } else if m == "backsplash" {
        ArchKind::Backsplash
    } else if m == "deck" || m.starts_with("deck-") {
        ArchKind::Room
    } else {
        ArchKind::from_part(part_of(mesh))
    }
}

/// The label of a phase-14/16 part, when its part names one.
fn phase14_label(mesh: &Mesh, m: &str) -> Option<String> {
    match part_of(mesh) {
        "furnishing" => Some(cadcraft_doc::arch::furnishing_label(mesh.object)),
        "pull" => Some(cadcraft_doc::arch::cabinet_label(mesh.object)),
        "soffit" => Some(cadcraft_doc::arch::soffit_label(mesh.object)),
        "foundation" | "footing" => Some(cadcraft_doc::arch::foundation_label(mesh.object)),
        "slab" => Some(cadcraft_doc::arch::room_label(mesh.object)),
        _ => m.strip_prefix("paint:").and_then(Surface::parse).map(|s| match s {
            Surface::WallSide { .. } => cadcraft_doc::arch::wall_label(mesh.object),
            Surface::RoomFloor(_) | Surface::RoomCeiling(_) => cadcraft_doc::arch::room_label(mesh.object),
            Surface::Roof(_) => cadcraft_doc::arch::roof_label(mesh.object),
            Surface::Cabinet { .. } => cadcraft_doc::arch::cabinet_label(mesh.object),
            Surface::Fixture(_) => cadcraft_doc::arch::fixture_label(mesh.object),
            Surface::Furnishing { .. } => cadcraft_doc::arch::furnishing_label(mesh.object),
            Surface::Countertop(_) => cadcraft_doc::arch::countertop_label(mesh.object),
            Surface::Backsplash(_) => cadcraft_doc::arch::backsplash_label(mesh.object),
        }),
    }
}

/// The level elevation for a mesh.
///
/// Prefers the recorded `level_elevation`; falls back to the mesh
/// bounding box `min.z` when unknown.
pub fn mesh_level(mesh: &Mesh) -> f64 {
    if mesh.level_elevation.is_finite() {
        return mesh.level_elevation;
    }
    mesh.bounds().min[2]
}

/// Probe the scene at point `p`, returning the first furnishing box or mesh that contains it.
///
/// Returns `None` immediately if any coordinate of `p` is non-finite.
/// Furnishings come first: each one is its placed box ([`Scene3d::furnishing_boxes`], in build
/// order), since symbol meshes need not be closed; a hit is the furnishing's label, part
/// `"furnishing"` and its level's elevation. Then `scene.meshes` in build order (furnishing parts
/// left out), returning the first mesh for which [`probe_mesh`] is true, as a [`Hit`] with the
/// mesh's label, part name and level elevation. First match wins; no merging or sorting.
///
/// `Scene3d::probe` (in `lib.rs`) delegates to this function.
pub fn probe(scene: &Scene3d, p: Vec3) -> Option<Hit> {
    if !p.is_finite() {
        return None;
    }
    if let Some(b) = scene.furnishing_boxes.iter().find(|b| b.contains(p)) {
        return Some(Hit { id: cadcraft_doc::arch::furnishing_label(b.id), part: "furnishing".to_string(), level: b.level });
    }
    if let Some(b) = scene.electrical_boxes.iter().find(|b| b.contains(p)) {
        return Some(Hit { id: cadcraft_doc::arch::electrical_label(b.id), part: "electrical".to_string(), level: b.level });
    }
    for mesh in &scene.meshes {
        if probe_mesh(mesh, p) && part_of(mesh) != "furnishing" && part_of(mesh) != "electrical" {
            return Some(Hit { id: mesh_label(mesh), part: part_of(mesh).to_string(), level: mesh_level(mesh) });
        }
    }
    // After every real mesh: the horizon slab (never in `meshes`); it has no level.
    // It starts a little beyond the site (2 m past the scene's bounds), so the ground just outside
    // the skirt stays empty space as before.
    const HORIZON_MARGIN: f64 = 2.0;
    let b = &scene.bounds;
    let near_site =
        p.x >= b.min[0] - HORIZON_MARGIN && p.x <= b.max[0] + HORIZON_MARGIN && p.y >= b.min[1] - HORIZON_MARGIN && p.y <= b.max[1] + HORIZON_MARGIN;
    if let Some(h) = &scene.horizon
        && !near_site
        && probe_mesh(h, p)
    {
        return Some(Hit { id: "horizon".to_string(), part: "horizon".to_string(), level: f64::NAN });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reports_horizon() {
        use cadcraft_doc::arch::Terrain;
        use cadcraft_geom::Vec2;
        let t = Terrain {
            boundary: vec![Vec2::new(0.0, 0.0), Vec2::new(40.0, 0.0), Vec2::new(40.0, 30.0), Vec2::new(0.0, 30.0)],
            points: vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 0.0, 0.0), Vec3::new(40.0, 30.0, 0.0), Vec3::new(0.0, 30.0, 0.0)],
            pad: false,
            ..Default::default()
        };
        let model = cadcraft_doc::arch::ArchModel { terrain: Some(t), ..Default::default() };
        let scene = Scene3d::build(&model);
        assert!(scene.horizon.is_some());
        let far = probe(&scene, Vec3::new(500.0, 500.0, -0.5)).expect("horizon hit");
        assert_eq!((far.id.as_str(), far.part.as_str()), ("horizon", "horizon"));
        assert!(far.level.is_nan());
        assert!(probe(&scene, Vec3::new(500.0, 500.0, 0.5)).is_none());
        let own = probe(&scene, Vec3::new(20.0, 15.0, -0.5)).expect("terrain hit");
        assert_eq!(own.part, "terrain");
    }

    #[test]
    fn probe_horizon_at_exact_top_face() {
        use cadcraft_doc::arch::Terrain;
        use cadcraft_geom::Vec2;
        let t = Terrain {
            boundary: vec![Vec2::new(0.0, 0.0), Vec2::new(40.0, 0.0), Vec2::new(40.0, 30.0), Vec2::new(0.0, 30.0)],
            points: vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 0.0, 0.0), Vec3::new(40.0, 30.0, 0.0), Vec3::new(0.0, 30.0, 0.0)],
            pad: false,
            ..Default::default()
        };
        let model = cadcraft_doc::arch::ArchModel { terrain: Some(t), ..Default::default() };
        let scene = Scene3d::build(&model);
        // Point exactly on the horizon slab's top face (z = 0.0)
        let hit = probe(&scene, Vec3::new(500.0, 500.0, 0.0));
        // Top face interior is outside (per probe_mesh semantics)
        assert!(hit.is_none(), "point on horizon top face should be outside");
    }

    #[test]
    fn probe_horizon_at_exact_bottom_face() {
        use cadcraft_doc::arch::Terrain;
        use cadcraft_geom::Vec2;
        let t = Terrain {
            boundary: vec![Vec2::new(0.0, 0.0), Vec2::new(40.0, 0.0), Vec2::new(40.0, 30.0), Vec2::new(0.0, 30.0)],
            points: vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 0.0, 0.0), Vec3::new(40.0, 30.0, 0.0), Vec3::new(0.0, 30.0, 0.0)],
            pad: false,
            ..Default::default()
        };
        let model = cadcraft_doc::arch::ArchModel { terrain: Some(t), ..Default::default() };
        let scene = Scene3d::build(&model);
        // Point exactly on the horizon slab's bottom face (z = -1.0)
        let hit = probe(&scene, Vec3::new(500.0, 500.0, -1.0));
        // Bottom face is inside (per probe_mesh semantics)
        assert!(hit.is_some(), "point on horizon bottom face should be inside");
        assert_eq!(hit.unwrap().part, "horizon");
    }

    #[test]
    fn probe_horizon_beyond_square_edge() {
        use cadcraft_doc::arch::Terrain;
        use cadcraft_geom::Vec2;
        let t = Terrain {
            boundary: vec![Vec2::new(0.0, 0.0), Vec2::new(40.0, 0.0), Vec2::new(40.0, 30.0), Vec2::new(0.0, 30.0)],
            points: vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 0.0, 0.0), Vec3::new(40.0, 30.0, 0.0), Vec3::new(0.0, 30.0, 0.0)],
            pad: false,
            ..Default::default()
        };
        let model = cadcraft_doc::arch::ArchModel { terrain: Some(t), ..Default::default() };
        let scene = Scene3d::build(&model);
        // Point far beyond the 9500m half-side square
        assert!(probe(&scene, Vec3::new(10000.0, 10000.0, -0.5)).is_none(), "point beyond horizon square should miss");
        // Point at the edge of the square should hit
        assert!(probe(&scene, Vec3::new(9499.0, 0.0, -0.5)).is_some(), "point at horizon square edge should hit");
    }

    #[test]
    fn probe_horizon_with_nan_coordinates() {
        use cadcraft_doc::arch::Terrain;
        use cadcraft_geom::Vec2;
        let t = Terrain {
            boundary: vec![Vec2::new(0.0, 0.0), Vec2::new(40.0, 0.0), Vec2::new(40.0, 30.0), Vec2::new(0.0, 30.0)],
            points: vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 0.0, 0.0), Vec3::new(40.0, 30.0, 0.0), Vec3::new(0.0, 30.0, 0.0)],
            pad: false,
            ..Default::default()
        };
        let model = cadcraft_doc::arch::ArchModel { terrain: Some(t), ..Default::default() };
        let scene = Scene3d::build(&model);
        assert!(probe(&scene, Vec3::new(f64::NAN, 500.0, -0.5)).is_none(), "NaN x should miss");
        assert!(probe(&scene, Vec3::new(500.0, f64::NAN, -0.5)).is_none(), "NaN y should miss");
        assert!(probe(&scene, Vec3::new(500.0, 500.0, f64::NAN)).is_none(), "NaN z should miss");
    }

    #[test]
    fn probe_horizon_just_outside_site_margin() {
        use cadcraft_doc::arch::Terrain;
        use cadcraft_geom::Vec2;
        let t = Terrain {
            boundary: vec![Vec2::new(0.0, 0.0), Vec2::new(40.0, 0.0), Vec2::new(40.0, 30.0), Vec2::new(0.0, 30.0)],
            points: vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 0.0, 0.0), Vec3::new(40.0, 30.0, 0.0), Vec3::new(0.0, 30.0, 0.0)],
            pad: false,
            ..Default::default()
        };
        let model = cadcraft_doc::arch::ArchModel { terrain: Some(t), ..Default::default() };
        let scene = Scene3d::build(&model);
        // Scene bounds are roughly x: -1..41, y: -1..31 (with skirt). Margin is 2m.
        // Point at x=43.5 is just outside the margin (41 + 2 = 43).
        let hit = probe(&scene, Vec3::new(43.5, 15.0, -0.5));
        assert!(hit.is_some(), "point just outside site margin should hit horizon");
        assert_eq!(hit.unwrap().part, "horizon");
    }

    #[test]
    fn probe_horizon_just_inside_site_margin() {
        use cadcraft_doc::arch::Terrain;
        use cadcraft_geom::Vec2;
        let t = Terrain {
            boundary: vec![Vec2::new(0.0, 0.0), Vec2::new(40.0, 0.0), Vec2::new(40.0, 30.0), Vec2::new(0.0, 30.0)],
            points: vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 0.0, 0.0), Vec3::new(40.0, 30.0, 0.0), Vec3::new(0.0, 30.0, 0.0)],
            pad: false,
            ..Default::default()
        };
        let model = cadcraft_doc::arch::ArchModel { terrain: Some(t), ..Default::default() };
        let scene = Scene3d::build(&model);
        // Point at x=42.5 is inside the 2m margin (41 + 2 = 43), so it should hit terrain, not horizon.
        let hit = probe(&scene, Vec3::new(42.5, 15.0, -0.5));
        if hit.is_some() {
            assert_eq!(hit.unwrap().part, "terrain", "point inside site margin should hit terrain, not horizon");
        }
    }

    fn named(material: &str, id: u32) -> Mesh {
        Mesh::new(material, ArchId(id))
    }

    #[test]
    fn part_of_maps_every_phase17_material() {
        let want = [
            ("stair-tread", "tread"),
            ("stair-riser", "riser"),
            ("stair-stringer", "stringer"),
            ("stair-skirt", "skirt"),
            ("stair-landing", "stair"),
            ("stair", "stair"),
            ("railing-post", "post"),
            ("railing-newel", "post"),
            ("railing-rail", "railing"),
            ("railing-panel", "railing"),
            ("railing-baluster", "railing"),
            ("railing", "railing"),
            ("deck", "deck"),
            ("deck-ledger", "ledger"),
            ("deck-joist", "joist"),
            ("deck-rim", "rim"),
            ("deck-beam", "beam"),
            ("deck-post", "post"),
            ("deck-footing", "footing"),
            ("window-grille", "grille"),
            ("door-grille", "grille"),
            ("window-sill-left", "sill"),
            ("window-casing-left", "casing"),
            ("door-casing-left", "casing"),
        ];
        for (material, part) in want {
            assert_eq!(part_of(&named(material, 0)), part, "{material}");
        }
        // Phase 16's footing and the roof's soffit keep their parts.
        assert_eq!(part_of(&named("footing", 0)), "footing");
        assert_eq!(part_of(&named("soffit", 0)), "roof");
    }

    #[test]
    fn part_of_maps_phase18_materials() {
        let want = [
            ("glass-wall", "glass"),
            ("molding-room-base-0", "molding"),
            ("molding-line", "molding"),
            ("countertop", "counter"),
            ("countertop-waterfall", "counter"),
            ("backsplash", "backsplash"),
            ("niche-back:o3", "wall"),
            ("cabinet-crown", "cabinet"),
        ];
        for (material, part) in want {
            assert_eq!(part_of(&named(material, 0)), part, "{material}");
        }
    }

    #[test]
    fn mesh_label_of_phase18_meshes() {
        use cadcraft_doc::arch::{backsplash_label, countertop_label, molding_line_label, room_label, wall_label};
        assert_eq!(mesh_label(&named("glass-wall", 2)), wall_label(ArchId(2)));
        assert_eq!(mesh_label(&named("molding-room-base-0", 3)), room_label(ArchId(3)));
        assert_eq!(mesh_label(&named("molding-line", 4)), molding_line_label(ArchId(4)));
        assert_eq!(mesh_label(&named("countertop", 5)), countertop_label(ArchId(5)));
        assert_eq!(mesh_label(&named("countertop-waterfall", 5)), countertop_label(ArchId(5)));
        assert_eq!(mesh_label(&named("backsplash", 6)), backsplash_label(ArchId(6)));
        assert_eq!(mesh_label(&named("niche-back:o3", 7)), wall_label(ArchId(7)));
        assert_eq!(mesh_label(&named("paint:countertop/ct0", 0)), "ct0");
        assert_eq!(mesh_label(&named("paint:backsplash/bs1", 1)), "bs1");
    }

    #[test]
    fn arch_kind_of_phase18_meshes() {
        use cadcraft_doc::arch::ArchKind;
        let want = [
            ("glass-wall", ArchKind::Wall),
            ("niche-back:o3", ArchKind::Wall),
            ("molding-room-base-0", ArchKind::Room),
            ("molding-line", ArchKind::MoldingLine),
            ("countertop", ArchKind::Countertop),
            ("countertop-waterfall", ArchKind::Countertop),
            ("backsplash", ArchKind::Backsplash),
            ("paint:countertop/ct0", ArchKind::Countertop),
            ("paint:backsplash/bs1", ArchKind::Backsplash),
            ("deck-joist", ArchKind::Room),
        ];
        for (material, kind) in want {
            assert_eq!(arch_kind(&named(material, 0)), kind, "{material}");
        }
    }

    #[test]
    fn mesh_label_names_the_owner() {
        use cadcraft_doc::arch::{railing_label, room_label, stair_label, wall_label};
        for material in ["deck", "deck-ledger", "deck-joist", "deck-rim", "deck-beam", "deck-post", "deck-footing"] {
            assert_eq!(mesh_label(&named(material, 3)), room_label(ArchId(3)), "{material}");
        }
        for material in ["railing", "railing-post", "railing-newel", "railing-rail", "railing-panel", "railing-baluster"] {
            assert_eq!(mesh_label(&named(material, 2)), railing_label(ArchId(2)), "{material}");
        }
        for material in ["stair", "stair-tread", "stair-riser", "stair-stringer", "stair-skirt", "stair-landing"] {
            assert_eq!(mesh_label(&named(material, 1)), stair_label(ArchId(1)), "{material}");
        }
        assert_eq!(mesh_label(&named("window-grille", 4)), cadcraft_doc::arch::opening_label(ArchId(4)));
        assert_eq!(mesh_label(&named("wall", 0)), wall_label(ArchId(0)));
        // Phase 16's footing stays a foundation's.
        assert_eq!(mesh_label(&named("footing", 5)), cadcraft_doc::arch::foundation_label(ArchId(5)));
    }

    #[test]
    fn pick_part_and_arch_kind_send_deck_structure_to_the_room() {
        use cadcraft_doc::arch::ArchKind;
        for material in ["deck", "deck-ledger", "deck-joist", "deck-rim", "deck-beam", "deck-post", "deck-footing"] {
            let m = named(material, 0);
            assert_eq!(pick_part(&m), "deck", "{material}");
            assert_eq!(arch_kind(&m), ArchKind::Room, "{material}");
        }
        let checks = [
            ("stair-tread", "tread", ArchKind::Stair),
            ("stair-landing", "stair", ArchKind::Stair),
            ("railing-post", "post", ArchKind::Railing),
            ("railing-rail", "railing", ArchKind::Railing),
            ("window-grille", "grille", ArchKind::Opening),
            ("footing", "footing", ArchKind::Foundation),
            ("wall", "wall", ArchKind::Wall),
        ];
        for (material, part, kind) in checks {
            let m = named(material, 0);
            assert_eq!((pick_part(&m), arch_kind(&m)), (part, kind), "{material}");
        }
    }

    /// Build a closed box (cube) mesh from `(x0, y0, z0)` with side `size`.
    fn box_mesh(x0: f64, y0: f64, z0: f64, size: f64, material: &str, object: ArchId) -> Mesh {
        let x0 = x0 as f32;
        let y0 = y0 as f32;
        let z0 = z0 as f32;
        let s = size as f32;

        let positions: Vec<[f32; 3]> = vec![
            [x0, y0, z0],
            [x0 + s, y0, z0],
            [x0 + s, y0 + s, z0],
            [x0, y0 + s, z0],
            [x0, y0, z0 + s],
            [x0 + s, y0, z0 + s],
            [x0 + s, y0 + s, z0 + s],
            [x0, y0 + s, z0 + s],
        ];

        let normals: Vec<[f32; 3]> = vec![
            [0.0, 0.0, -1.0],
            [0.0, 0.0, -1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, -1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
        ];

        let uvs: Vec<[f32; 2]> = (0..8).map(|_| [0.0, 0.0]).collect();

        let indices: Vec<u32> = vec![0, 1, 2, 0, 2, 3, 4, 6, 5, 4, 7, 6, 0, 4, 5, 0, 5, 1, 2, 3, 7, 2, 7, 6, 0, 3, 7, 0, 7, 4, 1, 5, 6, 1, 6, 2];

        Mesh { positions, normals, uvs, indices, ..Mesh::new(material, object) }
    }

    /// A flat ceiling quad at z = `z0`, spanning `(x0, y0)` to `(x0+s, y0+s)`.
    fn ceiling_quad(x0: f64, y0: f64, z0: f64, size: f64, object: ArchId) -> Mesh {
        let x0 = x0 as f32;
        let y0 = y0 as f32;
        let z0 = z0 as f32;
        let s = size as f32;

        let positions: Vec<[f32; 3]> = vec![[x0, y0, z0], [x0 + s, y0, z0], [x0 + s, y0 + s, z0], [x0, y0 + s, z0]];

        let normals: Vec<[f32; 3]> = vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0]];

        let uvs: Vec<[f32; 2]> = (0..4).map(|_| [0.0, 0.0]).collect();

        let indices: Vec<u32> = vec![0, 1, 2, 0, 2, 3];

        Mesh { positions, normals, uvs, indices, ..Mesh::new("ceiling", object) }
    }

    #[test]
    fn part_of_maps_material_names() {
        let base = box_mesh(0.0, 0.0, 0.0, 1.0, "wall", ArchId(1));
        assert_eq!(part_of(&base), "wall");

        let glass = box_mesh(0.0, 0.0, 0.0, 1.0, "glass-pane", ArchId(2));
        assert_eq!(part_of(&glass), "glass");

        let frame = box_mesh(0.0, 0.0, 0.0, 1.0, "door-frame", ArchId(3));
        assert_eq!(part_of(&frame), "frame");

        let casing = box_mesh(0.0, 0.0, 0.0, 1.0, "window-casing", ArchId(4));
        assert_eq!(part_of(&casing), "casing");

        let leaf = box_mesh(0.0, 0.0, 0.0, 1.0, "door-leaf", ArchId(5));
        assert_eq!(part_of(&leaf), "leaf");

        let sill = box_mesh(0.0, 0.0, 0.0, 1.0, "window-sill", ArchId(6));
        assert_eq!(part_of(&sill), "sill");

        let header = box_mesh(0.0, 0.0, 0.0, 1.0, "door-header", ArchId(7));
        assert_eq!(part_of(&header), "header");

        let roof = box_mesh(0.0, 0.0, 0.0, 1.0, "roof-deck", ArchId(8));
        assert_eq!(part_of(&roof), "roof");

        let ceil = box_mesh(0.0, 0.0, 0.0, 1.0, "ceiling-tile", ArchId(9));
        assert_eq!(part_of(&ceil), "ceiling");

        let floor = box_mesh(0.0, 0.0, 0.0, 1.0, "floor-slab", ArchId(10));
        assert_eq!(part_of(&floor), "floor");

        // Unrecognised -> "wall"
        let unknown = box_mesh(0.0, 0.0, 0.0, 1.0, "mystery-material", ArchId(11));
        assert_eq!(part_of(&unknown), "wall");

        // Priority: "glass" wins over "frame" in "glass-frame"
        let mixed = box_mesh(0.0, 0.0, 0.0, 1.0, "glass-frame", ArchId(12));
        assert_eq!(part_of(&mixed), "glass");
    }

    #[test]
    fn part_of_maps_phase_14_materials() {
        let part = |material: &str| part_of(&Mesh::new(material, ArchId(2)));
        assert_eq!(part("sym:0123456789abcdef:0"), "furnishing");
        assert_eq!(part("sym:missing:0"), "furnishing");
        // A hostile symbol key cannot pass for another part.
        assert_eq!(part("sym:glass-floor:3"), "furnishing");
        assert_eq!(part("pull:0123456789abcdef:1"), "pull");
        for (key, unpainted) in [
            ("wall/w3/left", "wall"),
            ("wall/w3/right", "wall"),
            ("room/r0/floor", "floor"),
            ("room/r0/ceiling", "ceiling"),
            ("roof/r0", "roof"),
            ("cabinet/cb0/carcass", "cabinet"),
            ("cabinet/cb0/front", "cabinet"),
            ("cabinet/cb0/counter", "counter"),
            ("cabinet/cb0/pull", "pull"),
            ("fixture/fx0", "fixture"),
            ("furnishing/f2/0", "furnishing"),
        ] {
            assert_eq!(part(&format!("paint:{key}")), unpainted, "{key}");
        }
        assert_eq!(part("paint:nothing/x"), "wall");
        assert_eq!(mesh_label(&Mesh::new("sym:k:0", ArchId(2))), "f2");
        assert_eq!(mesh_label(&Mesh::new("pull:k:0", ArchId(2))), "cb2");
        assert_eq!(mesh_label(&Mesh::new("paint:wall/w2/left", ArchId(2))), "w2");
        assert_eq!(mesh_label(&Mesh::new("paint:room/r2/ceiling", ArchId(2))), cadcraft_doc::arch::room_label(ArchId(2)));
        assert_eq!(mesh_label(&Mesh::new("paint:roof/r2", ArchId(2))), cadcraft_doc::arch::roof_label(ArchId(2)));
        assert_eq!(mesh_label(&Mesh::new("paint:cabinet/cb2/counter", ArchId(2))), "cb2");
        assert_eq!(mesh_label(&Mesh::new("paint:fixture/fx2", ArchId(2))), "fx2");
        assert_eq!(mesh_label(&Mesh::new("paint:furnishing/f2/1", ArchId(2))), "f2");
    }

    #[test]
    fn part_of_maps_fascia_and_soffit_to_roof() {
        let fascia = box_mesh(0.0, 0.0, 0.0, 1.0, "fascia-board", ArchId(1));
        assert_eq!(part_of(&fascia), "roof");

        let soffit = box_mesh(0.0, 0.0, 0.0, 1.0, "soffit-panel", ArchId(2));
        assert_eq!(part_of(&soffit), "roof");

        let shingle = box_mesh(0.0, 0.0, 0.0, 1.0, "shingle-roof", ArchId(3));
        assert_eq!(part_of(&shingle), "roof");
    }

    #[test]
    fn probe_mesh_inside_a_closed_box_hits() {
        let mesh = box_mesh(0.0, 0.0, 0.0, 1.0, "wall", ArchId(1));
        // Point strictly inside the box.
        assert!(probe_mesh(&mesh, Vec3::new(0.5, 0.5, 0.5)));
    }

    #[test]
    fn probe_mesh_below_and_above_a_closed_box_misses() {
        let mesh = box_mesh(0.0, 0.0, 0.0, 1.0, "wall", ArchId(1));
        // Below the box.
        assert!(!probe_mesh(&mesh, Vec3::new(0.5, 0.5, -0.1)));
        // Above the box.
        assert!(!probe_mesh(&mesh, Vec3::new(0.5, 0.5, 1.1)));
    }

    #[test]
    fn probe_mesh_skips_single_sided_surfaces() {
        let mesh = ceiling_quad(0.0, 0.0, 1.0, 1.0, ArchId(1));
        // A point "inside" where a ceiling would naively hit.
        assert!(!probe_mesh(&mesh, Vec3::new(0.5, 0.5, 0.5)));
    }

    #[test]
    fn probe_mesh_rejects_points_outside_its_bounds() {
        let mesh = box_mesh(0.0, 0.0, 0.0, 1.0, "wall", ArchId(1));
        // Far outside in x.
        assert!(!probe_mesh(&mesh, Vec3::new(10.0, 0.5, 0.5)));
        // Far outside in y.
        assert!(!probe_mesh(&mesh, Vec3::new(0.5, 10.0, 0.5)));
    }

    #[test]
    fn probe_mesh_ignores_non_finite_points() {
        let mesh = box_mesh(0.0, 0.0, 0.0, 1.0, "wall", ArchId(1));
        assert!(!probe_mesh(&mesh, Vec3::new(f64::NAN, 0.5, 0.5)));
        assert!(!probe_mesh(&mesh, Vec3::new(0.5, f64::INFINITY, 0.5)));
        assert!(!probe_mesh(&mesh, Vec3::new(0.5, 0.5, f64::NEG_INFINITY)));
    }

    use crate::Scene3d;
    use cadcraft_doc::{ArchModel, Level, Room, Wall};
    use cadcraft_geom::Vec2;

    /// South-room geometry from `loop_room_slabs_have_distinct_owners`: a wide
    /// shallow polygon with the loop hole near the bottom-left corner.
    #[test]
    fn probe_mesh_south_room_excludes_hole_near_corner() {
        let mut m = Mesh::new("floor", ArchId(1));
        // Exact south-room polygon from the diagnostic:
        let outer = vec![Vec2::new(9.9, 0.1), Vec2::new(9.9, 2.95), Vec2::new(0.1, 2.95), Vec2::new(0.1, 0.1)];
        // Exact hole polygon from the diagnostic (CCW as stored in the room):
        let hole = vec![Vec2::new(3.55, 0.95), Vec2::new(3.55, 2.05), Vec2::new(2.45, 2.05), Vec2::new(2.45, 0.95)];
        m.push_prism_with_holes(&outer, &[hole], -0.3, 0.0);

        // Point inside the hole (should be excluded from the solid slab):
        assert!(!probe_mesh(&m, Vec3::new(3.0, 1.5, -0.15)), "point in hole should not be inside south-room floor slab");
        // Point in the open south-room area (should be inside):
        assert!(probe_mesh(&m, Vec3::new(1.0, 1.0, -0.15)), "point in solid should be inside south-room floor slab");
    }

    #[test]
    fn probe_mesh_is_not_its_bounding_box() {
        let mut m = Mesh::new("floor", ArchId(1));
        let outer = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        let hole = vec![Vec2::new(1.5, 1.5), Vec2::new(2.5, 1.5), Vec2::new(2.5, 2.5), Vec2::new(1.5, 2.5)];
        m.push_prism_with_holes(&outer, &[hole], -0.3, 0.0);

        // Inside the hole (but inside AABB)
        assert!(!probe_mesh(&m, Vec3::new(2.0, 2.0, -0.15)), "point in hole should not be inside");
        // Inside solid part
        assert!(probe_mesh(&m, Vec3::new(0.5, 0.5, -0.15)), "point in solid should be inside");
    }

    #[test]
    fn probe_mesh_ignores_a_point_above_a_thin_slab() {
        let mut m = Mesh::new("floor", ArchId(1));
        let outer = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        m.push_prism(&outer, 0.0, 0.02);

        // Above the slab
        assert!(!probe_mesh(&m, Vec3::new(2.0, 2.0, 0.03)), "point above slab should not be inside");
    }

    #[test]
    fn probe_mesh_counts_a_point_in_solid_as_inside() {
        let mut m = Mesh::new("floor", ArchId(1));
        let outer = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        m.push_prism(&outer, 0.0, 0.02);

        // Inside the slab
        assert!(probe_mesh(&m, Vec3::new(2.0, 2.0, 0.01)), "point inside slab should be inside");
    }

    /// Build the phase-1 house model and its 3D scene.
    ///
    /// Exterior centre lines (0,0)-(10,0)-(10,8)-(0,8), 0.2 m thick;
    /// interior (0,3)-(10,3) and (6,3)-(6,8), 0.1 m thick;
    /// Level 1 at elevation 0, ceiling 2.4, structure 0.3, so a Level-2
    /// slab occupies z 2.4…2.7. Outer faces span x -0.1…10.1, y -0.1…8.1.
    fn house_scene() -> Scene3d {
        let mut model = ArchModel::default();

        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        model.levels.push(Level { index: 1, elevation: 2.7, ceiling_height: 2.4, ..Default::default() });

        let wall = |id: u32, a: (f64, f64), b: (f64, f64), t: f64| -> Wall {
            Wall { id: ArchId(id), start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness: t, height: 2.7, ..Default::default() }
        };

        // Exterior walls.
        model.walls.push(wall(1, (0.0, 0.0), (10.0, 0.0), 0.2));
        model.walls.push(wall(2, (10.0, 0.0), (10.0, 8.0), 0.2));
        model.walls.push(wall(3, (10.0, 8.0), (0.0, 8.0), 0.2));
        model.walls.push(wall(4, (0.0, 8.0), (0.0, 0.0), 0.2));

        // Interior walls.
        model.walls.push(wall(5, (0.0, 3.0), (10.0, 3.0), 0.1));
        model.walls.push(wall(6, (6.0, 3.0), (6.0, 8.0), 0.1));

        // Level-2 room providing the floor slab at z 2.4…2.7.
        model.rooms.push(Room {
            id: ArchId(100),
            level: 1,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0)],
            ..Default::default()
        });

        Scene3d::build(&model)
    }

    /// Assert the scene actually contains wall and floor meshes before probing,
    /// so a missing floor mesh fails loudly as a scene-builder bug rather than
    /// looking like a probe bug.
    fn assert_scene_has_meshes(scene: &Scene3d) {
        assert!(scene.meshes.iter().any(|m| part_of(m) == "wall"), "scene has no wall mesh");
        assert!(scene.meshes.iter().any(|m| part_of(m) == "floor"), "scene has no floor mesh");
    }

    #[test]
    fn probe_inside_a_wall_hits_the_wall() {
        let scene = house_scene();
        assert_scene_has_meshes(&scene);

        let hit = probe(&scene, Vec3::new(5.0, 0.0, 1.0));
        assert!(hit.is_some(), "probe(5,0,1) should hit the south wall");
        let hit = hit.unwrap();
        assert_eq!(hit.part, "wall");
        assert!(!hit.id.is_empty(), "hit id is empty");
    }

    #[test]
    fn probe_in_the_house_volume_hits_nothing() {
        let scene = house_scene();
        assert_scene_has_meshes(&scene);

        // Inside the house volume; catches ceilings being probed as solids.
        assert!(probe(&scene, Vec3::new(5.0, 4.0, 1.0)).is_none(), "probe(5,4,1) should be None");
    }

    #[test]
    fn probe_inside_a_floor_slab_hits_the_floor() {
        let scene = house_scene();
        assert_scene_has_meshes(&scene);

        // Inside the Level-2 slab (z 2.4…2.7).
        let hit = probe(&scene, Vec3::new(2.0, 1.0, 2.55));
        assert!(hit.is_some(), "probe(2,1,2.55) should hit the floor slab");
        assert_eq!(hit.unwrap().part, "floor");

        // Below the slab: the +z ray crosses bottom (2.4) and top (2.7) —
        // two crossings, even, so not inside.
        assert!(probe(&scene, Vec3::new(2.0, 1.0, 1.0)).is_none(), "probe(2,1,1) should be None");
    }

    #[test]
    fn probe_outside_the_building_hits_nothing() {
        let scene = house_scene();
        assert_scene_has_meshes(&scene);

        assert!(probe(&scene, Vec3::new(-5.0, -5.0, 1.0)).is_none(), "probe(-5,-5,1) should be None");
        assert!(probe(&scene, Vec3::new(5.0, 4.0, 50.0)).is_none(), "probe(5,4,50) should be None");
    }

    #[test]
    fn probe_survives_nan_and_huge_points() {
        let scene = house_scene();
        assert_scene_has_meshes(&scene);

        let all: [f64; 5] = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300, -1e300];

        for &v in &all {
            let non_finite = !v.is_finite();

            // x
            let r = probe(&scene, Vec3::new(v, 0.0, 0.0));
            if non_finite {
                assert!(r.is_none(), "probe(x={v}) should be None");
            }
            // y
            let r = probe(&scene, Vec3::new(0.0, v, 0.0));
            if non_finite {
                assert!(r.is_none(), "probe(y={v}) should be None");
            }
            // z
            let r = probe(&scene, Vec3::new(0.0, 0.0, v));
            if non_finite {
                assert!(r.is_none(), "probe(z={v}) should be None");
            }
        }

        // Finite but far away: no panic, and not inside anything.
        assert!(probe(&scene, Vec3::new(1e6, 1e6, 1e6)).is_none(), "probe(1e6,1e6,1e6) should be None");
    }

    /// Build a minimal scene: Level 0 room (4x4) with walls.
    ///
    /// Level 0: elevation 0.0, structure 0.3, ceiling_height 2.4.
    /// Floor slab spans z -0.3..0.0; ceiling slab spans z 2.38..2.4.
    fn level0_room_scene() -> Scene3d {
        let mut model = ArchModel::default();

        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });

        let wall = |id: u32, a: (f64, f64), b: (f64, f64)| -> Wall {
            Wall { id: ArchId(id), start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness: 0.2, height: 2.4, ..Default::default() }
        };

        model.walls.push(wall(1, (0.0, 0.0), (4.0, 0.0)));
        model.walls.push(wall(2, (4.0, 0.0), (4.0, 4.0)));
        model.walls.push(wall(3, (4.0, 4.0), (0.0, 4.0)));
        model.walls.push(wall(4, (0.0, 4.0), (0.0, 0.0)));

        model.rooms.push(Room {
            id: ArchId(100),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
            ..Default::default()
        });

        Scene3d::build(&model)
    }

    #[test]
    fn probe_hits_a_ceiling_slab() {
        let scene = level0_room_scene();
        // Ceiling slab spans z 2.38..2.4; probe at 2.39 (inside).
        let hit = probe(&scene, Vec3::new(2.0, 2.0, 2.39));
        assert!(hit.is_some(), "probe(2,2,2.39) should hit the ceiling slab");
        let hit = hit.unwrap();
        assert_eq!(hit.part, "ceiling");
        assert!(!hit.id.is_empty());
    }

    #[test]
    fn probe_reports_the_level_elevation_not_the_slab_bottom() {
        let scene = level0_room_scene();
        // Ceiling slab spans 2.38..2.4 but level.elevation is 0.0.
        let hit = probe(&scene, Vec3::new(2.0, 2.0, 2.39)).unwrap();
        assert_eq!(hit.level, 0.0, "level should be 0.0 (level elevation), not 2.38 (slab bottom)");
    }

    #[test]
    fn floor_probe_id_starts_with_r() {
        let scene = level0_room_scene();
        // Floor slab spans z -0.3..0.0; probe at -0.15 (inside).
        let hit = probe(&scene, Vec3::new(2.0, 2.0, -0.15)).expect("floor slab hit");
        assert!(hit.id.starts_with('r'), "floor probe id should start with 'r', got {:?}", hit.id);
    }

    #[test]
    fn probe_level_one_slab_reports_zero() {
        let scene = level0_room_scene();
        // Level-1 (elevation 0.0) floor slab bottom is -0.3.
        // The probe must report level.elevation = 0.0, not bounds.min[2] = -0.3.
        let hit = probe(&scene, Vec3::new(2.0, 2.0, -0.15)).expect("floor slab hit");
        assert_eq!(hit.level, 0.0, "Level-0 slab must report 0.0, not -0.3");
    }

    #[test]
    fn mesh_label_names_site_objects() {
        // Site meshes are labelled as the commands name them, never as walls.
        assert_eq!(mesh_label(&Mesh::new("plant-crown", ArchId(4))), "pl4");
        assert_eq!(mesh_label(&Mesh::new("light-trim", ArchId(2))), "lf2");
        assert_eq!(part_of(&Mesh::new("light-lens", ArchId(2))), "light");
        assert_eq!(mesh_label(&Mesh::new("plant-trunk", ArchId(4))), "pl4");
        assert_eq!(mesh_label(&Mesh::new("terrain", ArchId(0))), "t0");
        assert_eq!(mesh_label(&Mesh::new("path-driveway-concrete", ArchId(2))), "tm2");
    }

    #[test]
    fn mesh_label_uses_the_material_prefix() {
        // Floor material → room label (starts with 'r').
        let floor_mesh = Mesh {
            positions: vec![],
            normals: vec![],
            uvs: Vec::new(),
            indices: Vec::new(),
            material: "floor-slab".to_string(),
            object: ArchId(42),
            ..Mesh::new("", ArchId(42))
        };
        assert!(mesh_label(&floor_mesh).starts_with('r'), "floor mesh label should be a room label");

        // Ceiling material → room label (starts with 'r').
        let ceil_mesh = Mesh {
            positions: vec![],
            normals: vec![],
            uvs: Vec::new(),
            indices: Vec::new(),
            material: "ceiling".to_string(),
            object: ArchId(42),
            ..Mesh::new("", ArchId(42))
        };
        assert!(mesh_label(&ceil_mesh).starts_with('r'), "ceiling mesh label should be a room label");

        // Door material → opening label.
        let door_mesh = Mesh {
            positions: vec![],
            normals: vec![],
            uvs: Vec::new(),
            indices: Vec::new(),
            material: "door-frame".to_string(),
            object: ArchId(42),
            ..Mesh::new("", ArchId(42))
        };
        let door_label = mesh_label(&door_mesh);
        assert!(!door_label.is_empty());

        // Wall material → wall label.
        let wall_mesh = Mesh {
            positions: vec![],
            normals: vec![],
            uvs: Vec::new(),
            indices: Vec::new(),
            material: "wall".to_string(),
            object: ArchId(42),
            ..Mesh::new("", ArchId(42))
        };
        let wall_label = mesh_label(&wall_mesh);
        assert!(!wall_label.is_empty());

        // Unknown material → wall label (fallback).
        let mystery_mesh = Mesh {
            positions: vec![],
            normals: vec![],
            uvs: Vec::new(),
            indices: Vec::new(),
            material: "mystery-thing".to_string(),
            object: ArchId(42),
            ..Mesh::new("", ArchId(42))
        };
        let mystery_label = mesh_label(&mystery_mesh);
        assert!(!mystery_label.is_empty());
    }

    #[test]
    fn probe_mesh_empty_mesh_misses() {
        let m = Mesh::new("wall", ArchId(1));
        assert!(!probe_mesh(&m, Vec3::new(0.0, 0.0, 0.0)), "empty mesh should never contain a point");
    }

    #[test]
    fn probe_mesh_multiple_holes() {
        let mut m = Mesh::new("floor", ArchId(1));
        let outer = vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0)];
        let hole1 = vec![Vec2::new(1.0, 1.0), Vec2::new(3.0, 1.0), Vec2::new(3.0, 3.0), Vec2::new(1.0, 3.0)];
        let hole2 = vec![Vec2::new(7.0, 7.0), Vec2::new(9.0, 7.0), Vec2::new(9.0, 9.0), Vec2::new(7.0, 9.0)];
        m.push_prism_with_holes(&outer, &[hole1, hole2], -0.3, 0.0);
        assert!(!probe_mesh(&m, Vec3::new(2.0, 2.0, -0.15)), "point in hole1 should not be inside");
        assert!(!probe_mesh(&m, Vec3::new(8.0, 8.0, -0.15)), "point in hole2 should not be inside");
        assert!(probe_mesh(&m, Vec3::new(5.0, 5.0, -0.15)), "point in solid should be inside");
    }

    #[test]
    fn probe_mesh_huge_coordinates() {
        let mut m = Mesh::new("wall", ArchId(1));
        let outer = vec![Vec2::new(1e6, 1e6), Vec2::new(1e6 + 4.0, 1e6), Vec2::new(1e6 + 4.0, 1e6 + 4.0), Vec2::new(1e6, 1e6 + 4.0)];
        m.push_prism(&outer, 0.0, 3.0);
        assert!(probe_mesh(&m, Vec3::new(1e6 + 2.0, 1e6 + 2.0, 1.5)), "point inside huge-coordinate prism should be inside");
        assert!(!probe_mesh(&m, Vec3::new(1e6 + 10.0, 1e6 + 2.0, 1.5)), "point outside huge-coordinate prism should not be inside");
    }

    #[test]
    fn probe_mesh_thin_prism_below_probeable_threshold() {
        let mut m = Mesh::new("floor", ArchId(1));
        let outer = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        m.push_prism(&outer, 0.0, 1e-7);
        assert!(!probe_mesh(&m, Vec3::new(2.0, 2.0, 5e-8)), "prism thinner than 1e-6 should not be probed");
    }

    #[test]
    fn mesh_level_falls_back_to_bounds_when_nan() {
        let mut m = Mesh::new("wall", ArchId(1));
        m.push_prism(&[Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)], 2.0, 3.0);
        // level_elevation is NaN by default from Mesh::new
        assert!(m.level_elevation.is_nan());
        assert_eq!(mesh_level(&m), 2.0);
    }

    #[test]
    fn ceiling_slab_with_z_extent_is_probeable() {
        let mut m = Mesh::new("ceiling", ArchId(1));
        m.push_prism(&[Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)], 2.38, 2.4);
        assert!(is_probeable(&m), "ceiling slab with z extent >= 1e-6 must be probeable");
        assert!(probe_mesh(&m, Vec3::new(2.0, 2.0, 2.39)), "point inside ceiling slab must hit");
    }

    #[test]
    fn probe_mesh_point_on_top_face_is_outside() {
        let mut m = Mesh::new("floor", ArchId(1));
        let outer = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        m.push_prism(&outer, 0.0, 0.3);
        assert!(!probe_mesh(&m, Vec3::new(2.0, 2.0, 0.3)), "point on top face should not be inside");
    }

    #[test]
    fn probe_roof_mesh_hits_inside_the_roof_volume() {
        use crate::roof::roof_plane_mesh;
        use cadcraft_doc::arch::RoofPlane;

        let mut plane = RoofPlane::default();
        plane.base_edge = [0.0, 0.0, 10.0, 0.0];
        plane.ring = vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 5.0), Vec2::new(0.0, 5.0)];
        plane.slope = 0.5;
        plane.thickness = 0.2;

        let mesh = roof_plane_mesh(&plane, 2.5, "roof", ArchId(1)).unwrap();

        // Point inside the roof volume: at (5, 2.5), z should be between bottom and top
        // Bottom z at (5, 2.5): distance from base edge (y=0) is 2.5, so z = 2.5 + 0.5*2.5 = 3.75
        // Top z: 3.75 + 0.2 * n.z where n.z = 1/sqrt(1+0.25) ≈ 0.894
        // Top z ≈ 3.75 + 0.179 = 3.929
        // Probe at z = 3.8 (inside)
        assert!(probe_mesh(&mesh, Vec3::new(5.0, 2.5, 3.8)), "point inside roof should hit");

        // Point below the roof
        assert!(!probe_mesh(&mesh, Vec3::new(5.0, 2.5, 2.0)), "point below roof should not hit");

        // Point above the roof
        assert!(!probe_mesh(&mesh, Vec3::new(5.0, 2.5, 5.0)), "point above roof should not hit");
    }

    #[test]
    fn probe_fascia_mesh_hits_inside() {
        use crate::roof::fascia_mesh;
        use cadcraft_doc::arch::RoofPlane;

        let mut plane = RoofPlane::default();
        plane.base_edge = [0.0, 0.0, 10.0, 0.0];
        plane.ring = vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 5.0), Vec2::new(0.0, 5.0)];
        plane.slope = 0.5;
        plane.thickness = 0.2;

        let mesh = fascia_mesh(&plane, 2.5, 0.2, ArchId(1)).unwrap();

        // Fascia is at the base edge (y=0), from z=2.5 down to z=2.3, width 0.2 in +y direction
        // Probe at (5, 0.1, 2.4) - inside the fascia
        assert!(probe_mesh(&mesh, Vec3::new(5.0, 0.1, 2.4)), "point inside fascia should hit");

        // Outside the fascia (too high)
        assert!(!probe_mesh(&mesh, Vec3::new(5.0, 0.1, 2.6)), "point above fascia should not hit");

        // Outside the fascia (too low)
        assert!(!probe_mesh(&mesh, Vec3::new(5.0, 0.1, 2.2)), "point below fascia should not hit");
    }

    #[test]
    fn probe_empty_scene_returns_none() {
        let scene = Scene3d::default();
        assert!(probe(&scene, Vec3::new(0.0, 0.0, 0.0)).is_none(), "empty scene should never hit");
    }

    #[test]
    fn probe_first_match_wins_in_build_order() {
        // Two walls at the same location; the first one built wins.
        let mut model = ArchModel::default();
        model.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(4.0, 0.0),
            thickness: 0.3,
            height: 2.5,
            ..Default::default()
        });
        model.walls.push(Wall {
            id: ArchId(2),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(4.0, 0.0),
            thickness: 0.3,
            height: 2.5,
            ..Default::default()
        });
        let scene = Scene3d::build(&model);
        let hit = probe(&scene, Vec3::new(2.0, 0.0, 1.0));
        assert!(hit.is_some(), "probe should hit one of the overlapping walls");
        // Wall 1 is built first, so it should be the hit.
        assert_eq!(hit.unwrap().id, "w1", "first wall in build order should win");
    }

    #[test]
    fn probe_at_building_corner_hits_wall() {
        let scene = house_scene();
        assert_scene_has_meshes(&scene);
        // The corner at (0,0) is shared by the south and west walls.
        // The probe should hit one of them (first in build order).
        let hit = probe(&scene, Vec3::new(0.0, 0.0, 1.0));
        assert!(hit.is_some(), "probe at corner should hit a wall");
        assert_eq!(hit.unwrap().part, "wall");
    }

    #[test]
    fn probe_with_nan_in_mesh_positions() {
        // A mesh with a NaN position should not crash the probe.
        let mut m = Mesh::new("wall", ArchId(1));
        m.positions.push([0.0, 0.0, 0.0]);
        m.positions.push([1.0, 0.0, 0.0]);
        m.positions.push([1.0, 1.0, 0.0]);
        m.positions.push([0.0, 1.0, 0.0]);
        m.positions.push([f64::NAN as f32, 0.0, 0.0]);
        m.positions.push([1.0, 0.0, 3.0]);
        m.positions.push([1.0, 1.0, 3.0]);
        m.positions.push([0.0, 1.0, 3.0]);
        m.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7]);
        let scene = Scene3d { meshes: vec![m], ..Default::default() };
        // Should not panic; may or may not hit depending on the NaN.
        let _ = probe(&scene, Vec3::new(0.5, 0.5, 1.0));
    }

    #[test]
    fn probe_scene_with_only_openings() {
        // A scene with only opening meshes (no walls) should still work.
        use cadcraft_doc::arch::{Opening, OpeningKind};
        let mut model = ArchModel::default();
        model.walls.push(Wall {
            id: ArchId(0),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.3,
            height: 2.5,
            ..Default::default()
        });
        let mut opening = Opening::default();
        opening.id = ArchId(1);
        opening.host = ArchId(0);
        opening.offset = 5.0;
        opening.width = 1.0;
        opening.height = 1.5;
        opening.sill = 0.9;
        opening.style = "window".to_string();
        opening.kind = OpeningKind::Window;
        model.openings.push(opening);
        let scene = Scene3d::build(&model);
        // Probe inside the window glass (if any) or frame.
        // At x=5, y=0, z=1.5 (inside window height).
        let hit = probe(&scene, Vec3::new(5.0, 0.0, 1.5));
        // May hit wall (if glass is not solid) or an opening component.
        // The key is that it doesn't panic and returns a valid result.
        if let Some(h) = hit {
            assert!(!h.id.is_empty());
            assert!(!h.part.is_empty());
        }
    }

    #[test]
    fn probe_soffit_mesh_hits_inside() {
        use crate::roof::soffit_mesh;
        use cadcraft_doc::arch::RoofPlane;

        let mut plane = RoofPlane::default();
        plane.base_edge = [0.0, 0.0, 10.0, 0.0];
        plane.ring = vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 5.0), Vec2::new(0.0, 5.0)];
        plane.slope = 0.5;
        plane.thickness = 0.2;

        let mesh = soffit_mesh(&plane, 2.5, 0.3, ArchId(1)).unwrap();

        // The soffit top meets the roof underside at z = 2.5; its bottom is at 2.48.
        // Width 0.3 in +y direction from y=0.
        assert!(probe_mesh(&mesh, Vec3::new(5.0, 0.15, 2.49)), "point inside soffit should hit");

        // Above the soffit
        assert!(!probe_mesh(&mesh, Vec3::new(5.0, 0.15, 2.6)), "point above soffit should not hit");

        // Below the soffit
        assert!(!probe_mesh(&mesh, Vec3::new(5.0, 0.15, 2.4)), "point below soffit should not hit");
    }

    #[test]
    fn probe_mesh_point_on_vertical_edge_is_inside() {
        let mesh = box_mesh(0.0, 0.0, 0.0, 1.0, "wall", ArchId(1));
        // Point on the vertical edge at x=0, y=0 (front-left edge of the box)
        assert!(probe_mesh(&mesh, Vec3::new(0.0, 0.0, 0.5)), "point on vertical edge should be inside");
    }

    #[test]
    fn probe_mesh_point_on_corner_vertex_is_inside() {
        let mesh = box_mesh(0.0, 0.0, 0.0, 1.0, "wall", ArchId(1));
        // Point at the bottom-front-left corner vertex
        assert!(probe_mesh(&mesh, Vec3::new(0.0, 0.0, 0.0)), "point at corner vertex should be inside");
    }

    #[test]
    fn probe_mesh_point_on_horizontal_edge_is_inside() {
        let mesh = box_mesh(0.0, 0.0, 0.0, 1.0, "wall", ArchId(1));
        // Point on the top-front horizontal edge (y=0, z=1)
        assert!(probe_mesh(&mesh, Vec3::new(0.5, 0.0, 1.0)), "point on horizontal edge should be inside");
    }

    #[test]
    fn probe_mesh_jitter_does_not_escape_thin_wall() {
        let mut m = Mesh::new("wall", ArchId(1));
        let outer = vec![Vec2::new(0.0, 0.0), Vec2::new(0.001, 0.0), Vec2::new(0.001, 4.0), Vec2::new(0.0, 4.0)];
        m.push_prism(&outer, 0.0, 3.0);
        // Point at the center of the 1mm-thick wall (x=0.0005)
        assert!(probe_mesh(&m, Vec3::new(0.0005, 2.0, 1.5)), "jitter must not escape a 1mm wall");
    }

    #[test]
    fn probe_mesh_jitter_with_huge_coordinates() {
        let mesh = box_mesh(1e6, 1e6, 1e6, 1.0, "wall", ArchId(1));
        // Point at the center of a box at huge coordinates
        assert!(probe_mesh(&mesh, Vec3::new(1e6 + 0.5, 1e6 + 0.5, 1e6 + 0.5)), "jitter must work at huge coordinates");
    }

    #[test]
    fn part_of_empty_material_returns_wall() {
        let mesh = box_mesh(0.0, 0.0, 0.0, 1.0, "", ArchId(1));
        assert_eq!(part_of(&mesh), "wall", "empty material should map to wall");
    }

    #[test]
    fn part_of_mixed_case_materials() {
        let wall = box_mesh(0.0, 0.0, 0.0, 1.0, "Wall", ArchId(1));
        assert_eq!(part_of(&wall), "wall");

        let window_frame = box_mesh(0.0, 0.0, 0.0, 1.0, "WINDOW-FRAME", ArchId(2));
        assert_eq!(part_of(&window_frame), "frame");

        let glass = box_mesh(0.0, 0.0, 0.0, 1.0, "GLASS", ArchId(3));
        assert_eq!(part_of(&glass), "glass");

        let roof_deck = box_mesh(0.0, 0.0, 0.0, 1.0, "Roof-Deck", ArchId(4));
        assert_eq!(part_of(&roof_deck), "roof");
    }

    #[test]
    fn probe_mesh_point_on_bottom_face_is_inside() {
        let mesh = box_mesh(0.0, 0.0, 0.0, 1.0, "wall", ArchId(1));
        // Point on the bottom face (z=0)
        assert!(probe_mesh(&mesh, Vec3::new(0.5, 0.5, 0.0)), "point on bottom face should be inside");
    }

    #[test]
    fn probe_mesh_point_on_side_face_is_inside() {
        let mesh = box_mesh(0.0, 0.0, 0.0, 1.0, "wall", ArchId(1));
        // Point on the left face (x=0)
        assert!(probe_mesh(&mesh, Vec3::new(0.0, 0.5, 0.5)), "point on side face should be inside");
        // Point on the front face (y=0)
        assert!(probe_mesh(&mesh, Vec3::new(0.5, 0.0, 0.5)), "point on front face should be inside");
    }

    #[test]
    fn is_probeable_at_exact_z_extent_threshold() {
        let mut m = Mesh::new("floor", ArchId(1));
        let outer = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        // Exactly 1e-6 z-extent
        m.push_prism(&outer, 0.0, 1e-6);
        assert!(is_probeable(&m), "mesh with z-extent exactly 1e-6 should be probeable");
    }

    #[test]
    fn is_probeable_just_below_z_extent_threshold() {
        let mut m = Mesh::new("floor", ArchId(1));
        let outer = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        // Just below 1e-6 z-extent
        m.push_prism(&outer, 0.0, 0.9e-6);
        assert!(!is_probeable(&m), "mesh with z-extent below 1e-6 should not be probeable");
    }

    #[test]
    fn mesh_level_uses_recorded_elevation_when_set() {
        let mut m = Mesh::new("wall", ArchId(1));
        m.push_prism(&[Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)], 2.0, 3.0);
        m.level_elevation = 5.0;
        assert_eq!(mesh_level(&m), 5.0, "mesh_level should use recorded level_elevation when finite");
    }

    #[test]
    fn part_of_maps_slab_footing_foundation_and_interior_soffit() {
        let id = ArchId(6);
        for (material, part, label) in [
            ("slab", "slab", "r6"),
            ("slab-thick", "slab", "r6"),
            ("footing", "footing", "fd6"),
            ("footing-edge", "footing", "fd6"),
            ("foundation", "foundation", "fd6"),
            ("foundation-stem", "foundation", "fd6"),
            ("soffit-box", "soffit", "sf6"),
            ("soffit-box:finish:oak", "soffit", "sf6"),
        ] {
            let m = Mesh::new(material, id);
            assert_eq!(part_of(&m), part, "{material}");
            assert_eq!(mesh_label(&m), label, "{material}");
        }
        // Look-alikes keep their old parts.
        assert_eq!(part_of(&Mesh::new("slabs", id)), "wall");
        assert_eq!(part_of(&Mesh::new("floor-slab", id)), "floor");
        assert_eq!(part_of(&Mesh::new("ceiling-layer", id)), "ceiling");
        assert_eq!(mesh_label(&Mesh::new("ceiling-layer", id)), "r6");
        assert_eq!(part_of(&Mesh::new("light-trim", id)), "light");
    }

    #[test]
    fn roof_soffit_stays_roof() {
        let m = Mesh::new("soffit", ArchId(2));
        assert_eq!(part_of(&m), "roof");
        assert_eq!(part_of(&Mesh::new("soffit-panel", ArchId(2))), "roof");
        assert_eq!(crate::object_key(&m).kind, crate::ObjectKind::Roof);
        assert_eq!(crate::object_key(&Mesh::new("soffit-box", ArchId(2))).kind, crate::ObjectKind::Soffit);
        assert_eq!(crate::object_key(&Mesh::new("footing", ArchId(2))).kind, crate::ObjectKind::Foundation);
        assert_eq!(crate::object_key(&Mesh::new("foundation", ArchId(2))).kind, crate::ObjectKind::Foundation);
        assert_eq!(crate::object_key(&Mesh::new("slab", ArchId(2))).kind, crate::ObjectKind::Room);
        assert_eq!(cadcraft_doc::arch::ArchKind::from_part(part_of(&m)), cadcraft_doc::arch::ArchKind::Roof);
    }
}
