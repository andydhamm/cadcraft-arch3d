//! Slab foundations (phase 16; owner: p16-foundation-slab): the thickened edge under a slab on
//! grade (the slab itself is the room's floor).
//!
//! Per on-grade foundation wall the thickened edge is ONE closed "footing" mesh built from two
//! adjacent, non-overlapping prisms that share a face: a 0.3 m strip (inward from the outer
//! face) from `elevation - 0.3` to `elevation - 0.1`, and the strip under the wall (wall
//! thickness) from `elevation - 0.1` to `elevation`. Because the union is closed, an upward-ray
//! probe counts each prism's crossings and a point in either prism stays inside (odd); the shared
//! face never opens a gap. Basement walls on a slab level get only a footing via
//! [`stem::basement_footing`]. No stem walls are built.

use super::FoundationIn;
use crate::foundation::stem::basement_footing;
use crate::mesh::Mesh;
use cadcraft_arch::foundation::FoundationWall;
use cadcraft_geom::Vec2;

/// Appends a closed prism over `ring` from `z0` to `z1` to `m` when the heights are finite, the
/// prism has a height and the ring points are finite; a degenerate ring appends nothing.
fn prism_into(m: &mut Mesh, i: &FoundationIn, ring: &[Vec2], z0: f64, z1: f64) {
    if !(z0.is_finite() && z1.is_finite() && z1 > z0 + 1e-9) || !ring.iter().all(|p| p.is_finite()) {
        return;
    }
    m.push_prism(ring, z0, z1);
    m.level_elevation = i.level.elevation;
}

/// The thickened edges of a slab foundation.
pub fn meshes(i: &FoundationIn) -> Vec<Mesh> {
    let mut out = Vec::new();
    for w in &i.walls {
        if w.basement {
            // Basement walls on a slab level get only a footing (no stem).
            if let Some(f) = basement_footing(i, w) {
                out.push(f);
            }
        } else {
            // On-grade walls: one closed footing mesh per wall, built from two adjacent prisms
            // (edge strip and wall strip) that share a face.
            let mut mesh = Mesh::new("footing", i.foundation.id);
            let e = i.level.elevation;
            let edge_ring = [w.a, w.b, w.b + w.inward * 0.3, w.a + w.inward * 0.3];
            prism_into(&mut mesh, i, &edge_ring, e - 0.3, e - 0.1);
            let wall_ring = [w.a, w.b, w.b + w.inward * w.thickness, w.a + w.inward * w.thickness];
            prism_into(&mut mesh, i, &wall_ring, e - 0.1, e);
            if !mesh.positions.is_empty() {
                out.push(mesh);
            }
        }
    }
    out
}
