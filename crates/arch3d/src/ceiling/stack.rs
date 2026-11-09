//! The ceiling finish stack (phase 16): the layers hung below the rough ceiling, and the solids a
//! room's ceiling shows.
//!
//! Under an empty stack a room's ceiling is today's 20 mm solid under the rough ceiling. Under a
//! stack each material layer is a solid of its own thickness, hung below the layers above it
//! (an air gap holds nothing), and the bottom one is the finished ceiling people see.

use super::CeilingIn;
use crate::mesh::Mesh;
use crate::planar::{Piece, push_between};

/// The ceiling solid under an empty stack (m), as [`crate::floors::ceiling_surface`] builds it.
const CEILING_SOLID: f64 = 0.02;

/// How far the finished ceiling sits below the rough one: the stack's total thickness (m).
pub(crate) fn drop(i: &CeilingIn) -> f64 {
    cadcraft_arch::ceiling::stack::thickness(&i.room.ceiling_stack)
}

/// The ceiling's solids under the rough `pieces`.
///
/// Under an empty stack: one `ceiling` mesh holding a 20 mm slab under every piece (for a flat
/// room this is [`crate::floors::ceiling_surface`] of the room with the fixture openings cut).
/// Under a stack: one mesh per material layer, each holding that layer's band under every piece
/// (its thickness below the layers above it, measured plumb), and nothing for an air gap. The
/// bottom layer's mesh comes first as `ceiling` (the finished face: paint, the ceiling count and
/// exports see it); the layers above it follow, top to bottom, as `ceiling-layer`. Layers with a
/// non-finite or non-positive thickness are skipped (a stack with none left is the empty stack),
/// and only the first [`cadcraft_arch::ceiling::stack::MAX_CEILING_LAYERS`] count. Empty when
/// nothing was built.
pub(crate) fn slabs(i: &CeilingIn, pieces: &[Piece]) -> Vec<Mesh> {
    if cadcraft_arch::ceiling::stack::layers(&i.room.ceiling_stack).next().is_none() {
        return twenty_mm(i, pieces);
    }
    let mut layers = Vec::new();
    let mut o = 0.0;
    for layer in cadcraft_arch::ceiling::stack::layers(&i.room.ceiling_stack) {
        let t = layer.thickness;
        if layer.material.is_some() {
            let mut m = Mesh::new("ceiling-layer", i.room.id);
            m.level_elevation = i.level.elevation;
            for p in pieces {
                band(&mut m, p, o, o + t);
            }
            if !m.positions.is_empty() {
                layers.push(m);
            }
        }
        o += t;
    }
    // The bottom layer is the finished ceiling: named `ceiling`, and first.
    if let Some(mut bottom) = layers.pop() {
        bottom.material = "ceiling".to_string();
        layers.insert(0, bottom);
    }
    layers
}

/// Today's ceiling: a 20 mm slab under every piece, one `ceiling` mesh.
fn twenty_mm(i: &CeilingIn, pieces: &[Piece]) -> Vec<Mesh> {
    let mut m = Mesh::new("ceiling", i.room.id);
    m.level_elevation = i.level.elevation;
    for p in pieces {
        if p.plane.is_flat() {
            m.push_prism_with_holes(&p.outer, &p.holes, p.plane.c - CEILING_SOLID, p.plane.c);
        } else {
            push_between(&mut m, &p.outer, &p.holes, p.plane.offset(-CEILING_SOLID), p.plane);
        }
    }
    if m.positions.is_empty() { Vec::new() } else { vec![m] }
}

/// Append to `m` the band of piece `p` from `from` to `to` below its plane (`from` < `to`, both
/// measured down from the rough ceiling). Nothing is added for numbers the mesh cannot hold.
fn band(m: &mut Mesh, p: &Piece, from: f64, to: f64) {
    let (top, bottom) = (p.plane.offset(-from), p.plane.offset(-to));
    if !top.is_finite() || !bottom.is_finite() {
        return;
    }
    if p.plane.is_flat() {
        let (zb, zt) = (bottom.c, top.c);
        if zt > zb && (zb as f32).is_finite() && (zt as f32).is_finite() {
            m.push_prism_with_holes(&p.outer, &p.holes, zb, zt);
        }
    } else {
        push_between(m, &p.outer, &p.holes, bottom, top);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planar::PlaneZ;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{CeilingLayer, Level, Room};
    use cadcraft_geom::{Vec2, Vec3};

    #[test]
    fn a_sloped_piece_gets_a_20_mm_slab_under_its_plane() {
        let ring = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        let i = CeilingIn {
            room: Room { id: ArchId(5), ..Default::default() },
            level: Level { elevation: 1.0, ..Default::default() },
            ..Default::default()
        };
        let pieces = vec![Piece::new(ring.clone(), Vec::new(), PlaneZ::new(0.0, 0.5, 3.0)), Piece::new(ring, Vec::new(), PlaneZ::flat(f64::NAN))];
        let out = slabs(&i, &pieces);
        assert_eq!(out.len(), 1);
        let m = &out[0];
        assert_eq!((m.material.as_str(), m.object, m.level_elevation), ("ceiling", ArchId(5), 1.0));
        assert!(m.is_manifold());
        assert!(crate::probe::probe_mesh(m, Vec3::new(2.0, 2.0, 3.99)));
        assert!(!crate::probe::probe_mesh(m, Vec3::new(2.0, 2.0, 3.97)));
        assert!(slabs(&i, &[]).is_empty());
        assert_eq!(drop(&i), 0.0);
    }

    fn square() -> Vec<Vec2> {
        vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)]
    }

    fn input(stack: Vec<CeilingLayer>) -> CeilingIn {
        CeilingIn {
            room: Room { id: ArchId(7), ceiling_stack: stack, ..Default::default() },
            level: Level { elevation: 2.7, ..Default::default() },
            ..Default::default()
        }
    }

    fn same(a: &Mesh, b: &Mesh) -> bool {
        a.positions == b.positions
            && a.normals == b.normals
            && a.uvs == b.uvs
            && a.indices == b.indices
            && a.material == b.material
            && a.object == b.object
            && a.level_elevation.to_bits() == b.level_elevation.to_bits()
    }

    fn z_range(m: &Mesh) -> (f64, f64) {
        let b = m.bounds();
        (b.min[2], b.max[2])
    }

    #[test]
    fn drop_is_the_stack_thickness() {
        let suspended = cadcraft_arch::ceiling::stack::template("suspended").unwrap_or_default();
        assert!((drop(&input(suspended)) - 0.2287).abs() < 1e-12);
        let drywall = cadcraft_arch::ceiling::stack::template("drywall").unwrap_or_default();
        assert!((drop(&input(drywall)) - 0.0127).abs() < 1e-12);
        assert_eq!(drop(&input(Vec::new())), 0.0);
        assert_eq!(drop(&input(vec![CeilingLayer::new("x", Some("drywall"), f64::NAN)])), 0.0);
    }

    #[test]
    fn a_flat_suspended_stack_builds_the_tile_and_the_drywall() {
        let i = input(cadcraft_arch::ceiling::stack::template("Suspended").unwrap_or_default());
        let out = slabs(&i, &[Piece::new(square(), Vec::new(), PlaneZ::flat(5.1))]);
        let names: Vec<(&str, ArchId, f64)> = out.iter().map(|m| (m.material.as_str(), m.object, m.level_elevation)).collect();
        assert_eq!(names, vec![("ceiling", ArchId(7), 2.7), ("ceiling-layer", ArchId(7), 2.7)], "the bottom layer first");
        let (tile, drywall) = (z_range(&out[0]), z_range(&out[1]));
        assert!((tile.0 - (5.1 - 0.2287)).abs() < 1e-6 && (tile.1 - (5.1 - 0.2127)).abs() < 1e-6, "{tile:?}");
        assert!((drywall.0 - (5.1 - 0.0127)).abs() < 1e-6 && (drywall.1 - 5.1).abs() < 1e-6, "{drywall:?}");
        assert!(out.iter().all(Mesh::is_manifold));
        // The air gap holds nothing.
        let hit = |z: f64| out.iter().any(|m| crate::probe::probe_mesh(m, Vec3::new(2.0, 2.0, z)));
        assert!(hit(5.1 - 0.22) && !hit(5.1 - 0.1) && hit(5.1 - 0.005));
        assert!(!hit(5.1 - 0.24) && !hit(5.11));
    }

    #[test]
    fn stack_on_a_sloped_piece_offsets_each_layer() {
        let i = input(cadcraft_arch::ceiling::stack::template("suspended").unwrap_or_default());
        let slope = PlaneZ::new(0.0, 0.5, 3.0);
        let pieces = vec![Piece::new(square(), Vec::new(), slope), Piece::new(square(), Vec::new(), PlaneZ::flat(f64::NAN))];
        let out = slabs(&i, &pieces);
        assert_eq!(out.iter().map(|m| m.material.as_str()).collect::<Vec<_>>(), vec!["ceiling", "ceiling-layer"]);
        assert!(out.iter().all(Mesh::is_manifold));
        // Each layer keeps its thickness measured plumb under the sloped rough ceiling.
        for (x, y) in [(1.0, 0.5), (2.0, 2.0), (3.0, 3.5)] {
            let top = slope.z_at(Vec2::new(x, y));
            let at = |dz: f64| Vec3::new(x, y, top - dz);
            assert!(crate::probe::probe_mesh(&out[0], at(0.22)), "the tile at ({x}, {y})");
            assert!(crate::probe::probe_mesh(&out[1], at(0.006)), "the drywall at ({x}, {y})");
            for dz in [0.1, 0.2, 0.24, -0.01] {
                assert!(!out.iter().any(|m| crate::probe::probe_mesh(m, at(dz))), "nothing at {dz} below ({x}, {y})");
            }
        }
    }

    #[test]
    fn hostile_layers_are_skipped_in_the_slabs() {
        let flat = [Piece::new(square(), Vec::new(), PlaneZ::flat(2.4))];
        // Only the valid tile is built, right under the rough ceiling.
        let i = input(vec![
            CeilingLayer::new("a", Some("drywall"), f64::NAN),
            CeilingLayer::new("b", Some("drywall"), 0.0),
            CeilingLayer::new("c", None, -0.5),
            CeilingLayer::new("d", Some("ceiling-tile"), 0.016),
            CeilingLayer::new("e", Some("drywall"), f64::NEG_INFINITY),
        ]);
        let out = slabs(&i, &flat);
        assert_eq!(out.len(), 1);
        let (lo, hi) = z_range(&out[0]);
        assert!((lo - 2.384).abs() < 1e-6 && (hi - 2.4).abs() < 1e-6, "{lo}..{hi}");
        assert_eq!(out[0].material, "ceiling");
        // A stack with nothing valid left is the empty stack: today's 20 mm solid.
        let none = input(vec![CeilingLayer::new("a", Some("drywall"), f64::NAN), CeilingLayer::new("b", None, 0.0)]);
        let (got, want) = (slabs(&none, &flat), slabs(&input(Vec::new()), &flat));
        assert_eq!(got.len(), 1);
        assert!(got.iter().zip(&want).all(|(a, b)| same(a, b)), "the 20 mm solid");
        // Air alone builds nothing; huge layers build nothing they cannot hold; neither panics.
        assert!(slabs(&input(vec![CeilingLayer::new("air", None, 0.2)]), &flat).is_empty());
        let huge = input(vec![CeilingLayer::new("a", Some("drywall"), 1e300), CeilingLayer::new("b", Some("drywall"), f64::MAX)]);
        assert!(slabs(&huge, &flat).iter().all(|m| m.positions.iter().flatten().all(|v| v.is_finite())));
        // Far more layers than a stack may hold: at most MAX_CEILING_LAYERS meshes.
        let many = input(vec![CeilingLayer::new("a", Some("drywall"), 0.01); 10_000]);
        let out = slabs(&many, &flat);
        assert_eq!(out.len(), cadcraft_arch::ceiling::stack::MAX_CEILING_LAYERS);
        assert!((drop(&many) - 0.08).abs() < 1e-12);
    }
}
