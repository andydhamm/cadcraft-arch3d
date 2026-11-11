//! Slab foundations (phase 16; owner: p16-foundation-slab): the thickened edge under a slab on
//! grade (the slab itself is the room's floor).
//!
//! Per on-grade foundation wall the thickened edge is ONE closed "footing" mesh built from two
//! adjacent prisms that share a face: the edge strip, [`EDGE_WIDTH`] (0.3 m) inward from the
//! wall's OUTER face (wider than a 0.2 m wall: the brief's "thickened edge 0.3 m wide, inward from
//! the outer face"), from `elevation - 0.3` to `elevation - 0.1`, and the strip under the wall (the
//! wall's thickness) from `elevation - 0.1` to `elevation`. Each prism is closed, so an upward-ray
//! probe counts each prism's crossings and a point in either prism stays inside (odd); the shared
//! face never opens a gap. Phase-16 check 07 probes the edge at 0.25 m in from the outer face of a
//! 0.2 m wall (y 0.15 on the CLI house, whose outer face is y = -0.1). Basement
//! walls on a slab level get only a footing via [`stem::basement_footing`]. No stem walls are
//! built.

use super::FoundationIn;
use crate::foundation::stem::basement_footing;
use crate::mesh::Mesh;
use cadcraft_arch::foundation::FoundationWall;
use cadcraft_geom::Vec2;

/// The thickened edge's width, inward from the wall's outer face (phase 16: 0.3 m, whatever the wall's thickness).
pub const EDGE_WIDTH: f64 = 0.3;

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
            // On-grade walls: one closed footing mesh per wall, built from two closed prisms that
            // share a face: the edge strip (EDGE_WIDTH in from the outer face, z e-0.3..e-0.1) and
            // the strip under the wall (the wall's thickness, z e-0.1..e). A point in the edge strip
            // past the wall's inner face crosses only the edge strip's top (odd, inside); a point in
            // the edge strip under the wall crosses that top, the shared face and the wall strip's
            // top (odd, inside).
            let mut mesh = Mesh::new("footing", i.foundation.id);
            let e = i.level.elevation;
            let edge_ring = [w.a, w.b, w.b + w.inward * EDGE_WIDTH, w.a + w.inward * EDGE_WIDTH];
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
