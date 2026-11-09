//! Stem foundations (phase 16; owner: p16-foundation-stem): a concrete wall on a footing under
//! every exterior wall on grade.
//!
//! Each foundation wall gets its own meshes (where two walls meet, their stems and footings
//! overlap at the corner, and separate closed meshes keep every probe's parity right):
//! - on grade: a stem `foundation`, 0.2 m thick with its outer face flush with the wall above,
//!   from the footing's top up to the underside of the level's floor structure, on a footing
//!   `footing` 0.6 x 0.25 m centred under it, its bottom `footing_depth` below the level's floor;
//! - a basement's wall is its own foundation wall: only a footing, centred under the wall,
//!   directly under the basement's floor structure ([`basement_footing`]).

use cadcraft_arch::foundation::{FoundationWall, footing_outline};
use cadcraft_doc::arch::{FOOTING_THICKNESS, STEM_THICKNESS};
use cadcraft_geom::Vec2;

use super::FoundationIn;
use crate::mesh::Mesh;

/// A closed prism of `material` over `ring` from `z0` to `z1`, owned by the foundation, or `None`
/// when the heights are not finite, the prism has no height or the ring builds nothing.
fn prism(i: &FoundationIn, material: &str, ring: &[Vec2], z0: f64, z1: f64) -> Option<Mesh> {
    if !(z0.is_finite() && z1.is_finite() && z1 > z0 + 1e-9) || !ring.iter().all(|p| p.is_finite()) {
        return None;
    }
    let mut mesh = Mesh::new(material, i.foundation.id);
    mesh.push_prism(ring, z0, z1);
    mesh.level_elevation = i.level.elevation;
    (!mesh.positions.is_empty()).then_some(mesh)
}

/// The stem wall's plan outline under `w`: the wall's outer face and 0.2 m inward of it.
fn stem_outline(w: &FoundationWall) -> [Vec2; 4] {
    let inner = w.inward * STEM_THICKNESS;
    [w.a, w.b, w.b + inner, w.a + inner]
}

/// The footing under a basement wall: over [`footing_outline`] (centred under the wall's centre
/// line), from 0.25 m below the underside of the basement's floor structure up to it.
pub(crate) fn basement_footing(i: &FoundationIn, w: &FoundationWall) -> Option<Mesh> {
    let top = i.level.elevation - i.level.structure;
    prism(i, "footing", &footing_outline(w), top - FOOTING_THICKNESS, top)
}

/// The stem walls and footings of a stem foundation. A footing depth that is not finite or leaves
/// the footing's top above the floor builds nothing on grade (the command refuses one).
pub fn meshes(i: &FoundationIn) -> Vec<Mesh> {
    let e = i.level.elevation;
    let s = i.level.structure;
    let depth = i.foundation.footing_depth;
    let on_grade = depth.is_finite() && depth >= FOOTING_THICKNESS;
    let bottom = e - depth;
    let mut out = Vec::new();
    for w in &i.walls {
        if w.basement {
            out.extend(basement_footing(i, w));
        } else if on_grade {
            out.extend(prism(i, "foundation", &stem_outline(w), bottom + FOOTING_THICKNESS, e - s));
            out.extend(prism(i, "footing", &footing_outline(w), bottom, bottom + FOOTING_THICKNESS));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{Foundation, Level};

    fn south() -> FoundationWall {
        FoundationWall {
            wall: ArchId(0),
            a: Vec2::new(-0.1, -0.1),
            b: Vec2::new(10.1, -0.1),
            inward: Vec2::new(0.0, 1.0),
            thickness: 0.2,
            basement: false,
            ..Default::default()
        }
    }

    fn input(depth: f64, level: Level, walls: Vec<FoundationWall>) -> FoundationIn {
        FoundationIn {
            foundation: Foundation { id: ArchId(3), footing_depth: depth, ..Default::default() },
            level,
            walls,
            basement: false,
            ..Default::default()
        }
    }

    fn z_range(m: &Mesh) -> (f64, f64) {
        let b = m.bounds();
        (b.min[2], b.max[2])
    }

    #[test]
    fn a_stem_and_a_footing_per_wall() {
        let out = meshes(&input(1.2, Level::default(), vec![south()]));
        assert_eq!(out.iter().map(|m| m.material.as_str()).collect::<Vec<_>>(), vec!["foundation", "footing"]);
        let (stem, footing) = (&out[0], &out[1]);
        assert!(stem.is_manifold() && footing.is_manifold());
        assert_eq!((stem.object, stem.level_elevation), (ArchId(3), 0.0));
        let (lo, hi) = z_range(stem);
        assert!((lo + 0.95).abs() < 1e-6 && (hi + 0.3).abs() < 1e-6, "{lo}..{hi}");
        let (lo, hi) = z_range(footing);
        assert!((lo + 1.2).abs() < 1e-6 && (hi + 0.95).abs() < 1e-6, "{lo}..{hi}");
        let b = stem.bounds();
        assert!((b.min[1] + 0.1).abs() < 1e-6 && (b.max[1] - 0.1).abs() < 1e-6 && (b.min[0] + 0.1).abs() < 1e-6, "{b:?}");
    }

    #[test]
    fn basement_walls_get_only_a_footing_under_the_floor_structure() {
        let mut w = south();
        w.basement = true;
        let level = Level { elevation: -2.7, ..Default::default() };
        let out = meshes(&input(1.2, level, vec![w]));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].material, "footing");
        let (lo, hi) = z_range(&out[0]);
        assert!((lo + 3.25).abs() < 1e-6 && (hi + 3.0).abs() < 1e-6, "{lo}..{hi}");
        assert_eq!(out[0].level_elevation, -2.7);
    }

    #[test]
    fn a_footing_too_shallow_for_a_stem_keeps_only_the_footing() {
        // The footing's top (-0.2) is above the floor structure's underside (-0.3): no stem.
        let out = meshes(&input(0.45, Level::default(), vec![south()]));
        assert_eq!(out.iter().map(|m| m.material.as_str()).collect::<Vec<_>>(), vec!["footing"]);
    }

    #[test]
    fn hostile_inputs_build_nothing_and_never_panic() {
        let nan_level = Level { elevation: f64::NAN, ..Default::default() };
        let nan_structure = Level { structure: f64::NAN, ..Default::default() };
        let mut bad_wall = south();
        bad_wall.a = Vec2::new(f64::NAN, 0.0);
        let mut zero = south();
        zero.b = zero.a;
        let mut flat_inward = south();
        flat_inward.inward = Vec2::new(f64::INFINITY, 0.0);
        assert!(meshes(&input(f64::NAN, Level::default(), vec![south()])).is_empty());
        assert!(meshes(&input(f64::INFINITY, Level::default(), vec![south()])).is_empty());
        assert!(meshes(&input(1.2, nan_level, vec![south()])).is_empty());
        // A NaN structure loses the stem; the footing does not depend on it.
        assert_eq!(meshes(&input(1.2, nan_structure.clone(), vec![south()])).len(), 1);
        let mut basement = south();
        basement.basement = true;
        assert!(meshes(&input(1.2, nan_structure, vec![basement])).is_empty());
        for w in [bad_wall, zero, flat_inward, FoundationWall::default()] {
            assert!(meshes(&input(1.2, Level::default(), vec![w])).is_empty());
        }
        assert!(meshes(&input(1.2, Level::default(), Vec::new())).is_empty());
        assert!(meshes(&input(-5.0, Level::default(), vec![south()])).is_empty());
    }
}
