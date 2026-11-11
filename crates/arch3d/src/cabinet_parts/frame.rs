//! Face frames and the plane the fronts sit in.

use cadcraft_arch::casework_face::{default_face, face_box, row_heights};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Cabinet, Construction, Fronts, FrontMount};

use crate::Mesh;
use crate::casework::{Frame, level_elevation};

/// Width of a face-frame member (m).
const FRAME: f64 = 0.038;
/// Thickness of the box's front edge (m).
const EDGE: f64 = 0.019;

/// The face frame of a cabinet: framed boxes get stiles and rails (a mesh `cabinet-frame`), a
/// frameless inset box shows the front edges of its sides, top and bottom; a frameless overlay box,
/// a filler and an open cabinet get none.
pub fn frame_meshes(model: &ArchModel, c: &Cabinet) -> Vec<Mesh> {
    if c.filler || c.fronts == Fronts::Open {
        return Vec::new();
    }
    let inset = c.mount == FrontMount::Inset;
    let framed = c.construction == Construction::Framed;
    if !framed && !inset {
        return Vec::new();
    }
    let floor = level_elevation(model, c.level);
    let toe = if c.toe_kick { 0.1 } else { 0.0 };
    let (z0, z1) = (floor + c.elevation + toe, floor + c.elevation + c.height);
    let half = c.width / 2.0;
    // Overlay frames stand just behind the fronts, inset ones in the plane of the fronts.
    let (y0, y1) = if inset { (c.depth - EDGE, c.depth) } else { (c.depth - FRAME, c.depth - EDGE) };
    let member = if framed { FRAME } else { EDGE };
    if !(z1 - z0 > 2.0 * member && c.width > 2.0 * member && z0.is_finite() && z1.is_finite() && half.is_finite()) {
        return Vec::new();
    }
    let fr = Frame { position: c.position, rotation: c.rotation };
    let mut mesh = Mesh::new("cabinet-frame", c.id);
    fr.box_(&mut mesh, -half, -half + member, y0, y1, z0, z1);
    fr.box_(&mut mesh, half - member, half, y0, y1, z0, z1);
    fr.box_(&mut mesh, -half + member, half - member, y0, y1, z1 - member, z1);
    fr.box_(&mut mesh, -half + member, half - member, y0, y1, z0, z0 + member);
    if framed && inset {
        // Mid rails fill the reveals between rows.
        let face = c.face.clone().unwrap_or_else(|| default_face(c));
        if let Some(heights) = row_heights(c, &face) {
            let b = face_box(c);
            let mut top = b.v1;
            for h in heights.iter().take(heights.len().saturating_sub(1)) {
                let bottom = top - h;
                fr.box_(&mut mesh, -half + member, half - member, y0, y1, floor + bottom - b.separator, floor + bottom);
                top = bottom - b.separator;
            }
        }
    }
    if mesh.indices.is_empty() { Vec::new() } else { vec![mesh] }
}

/// The `(y0, y1)` plane the fronts of `c` occupy in its frame: `(depth - 0.019, depth)` whatever the
/// mode (overlay fronts stand in front of the box and frame, inset fronts sit flush inside them).
pub fn front_plane(c: &Cabinet) -> (f64, f64) {
    (c.depth - EDGE, c.depth)
}
