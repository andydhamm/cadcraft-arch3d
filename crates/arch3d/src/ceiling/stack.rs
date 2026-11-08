//! The ceiling finish stack (phase 16; owner: p16-ceiling-stack): the layers hung below the rough
//! ceiling, and the solids a room's ceiling shows.

use super::CeilingIn;
use crate::mesh::Mesh;
use crate::planar::{Piece, push_between};

/// How far the finished ceiling sits below the rough one: the stack's total thickness (m).
pub(crate) fn drop(_i: &CeilingIn) -> f64 {
    0.0
}

/// The ceiling's solids under the rough `pieces`: one `ceiling` mesh holding a 20 mm slab under
/// every piece, empty when nothing was built. For a flat room this is
/// [`crate::floors::ceiling_surface`] of the room with the fixture openings cut.
pub(crate) fn slabs(i: &CeilingIn, pieces: &[Piece]) -> Vec<Mesh> {
    let mut m = Mesh::new("ceiling", i.room.id);
    m.level_elevation = i.level.elevation;
    for p in pieces {
        if p.plane.is_flat() {
            m.push_prism_with_holes(&p.outer, &p.holes, p.plane.c - 0.02, p.plane.c);
        } else {
            push_between(&mut m, &p.outer, &p.holes, p.plane.offset(-0.02), p.plane);
        }
    }
    if m.positions.is_empty() { Vec::new() } else { vec![m] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planar::PlaneZ;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{Level, Room};
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
}
