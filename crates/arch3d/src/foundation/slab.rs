//! Slab foundations (phase 16; owner: p16-foundation-slab): the thickened edge under a slab on
//! grade (the slab itself is the room's floor).
//!
//! Per on-grade foundation wall the thickened edge is one closed "footing" mesh made of two
//! adjacent, non-overlapping prisms: a 0.3 m strip (inward from the outer face) from
//! `elevation - 0.3` to `elevation - 0.1`, and the strip under the wall (wall thickness) from
//! `elevation - 0.1` to `elevation`. Basement walls on a slab level get only a footing via
//! [`stem::basement_footing`]. No stem walls are built.

use cadcraft_doc::arch::FOUNDATION_EDGE_DEPTH;

use super::FoundationIn;
use crate::mesh::Mesh;

/// The 0.3 m strip (inward from the outer face) of the thickened edge, from `elevation - 0.3`
/// to `elevation - 0.1`.
fn edge_strip(i: &FoundationIn, w: &FoundationWall) -> Option<Mesh> {
    let e = i.level.elevation;
    let z0 = e - 0.3;
    let z1 = e - 0.1;
    let ring = [w.a, w.b, w.b + w.inward * 0.3, w.a + w.inward * 0.3];
    prism(i, "footing", &ring, z0, z1)
}

/// The strip under the wall (wall thickness) of the thickened edge, from `elevation - 0.1`
/// to `elevation`.
fn wall_strip(i: &FoundationIn, w: &FoundationWall) -> Option<Mesh> {
    let e = i.level.elevation;
    let z0 = e - 0.1;
    let z1 = e;
    let ring = [w.a, w.b, w.b + w.inward * w.thickness, w.a + w.inward * w.thickness];
    prism(i, "footing", &ring, z0, z1)
}

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

/// The thickened edges of a slab foundation.
pub(crate) fn meshes(i: &FoundationIn) -> Vec<Mesh> {
    let mut out = Vec::new();
    for w in &i.walls {
        if w.basement {
            // Basement walls on a slab level get only a footing (no stem).
            if let Some(f) = super::basement_footing(i, w) {
                out.push(f);
            }
        } else {
            // On-grade walls: the thickened edge is two adjacent, non-overlapping prisms.
            if let Some(s) = edge_strip(i, w) {
                out.push(s);
            }
            if let Some(s) = wall_strip(i, w) {
                out.push(s);
            }
        }
    }
    out
}
