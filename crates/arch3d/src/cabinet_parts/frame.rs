//! Face frames and the plane the fronts sit in.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::Cabinet;

use crate::Mesh;

/// The face frame of a cabinet.
///
/// Stub: owner p18-cabinet-fronts.
pub fn frame_meshes(_model: &ArchModel, _c: &Cabinet) -> Vec<Mesh> {
    Vec::new()
}

/// The `(y0, y1)` plane the fronts of `c` occupy in its frame: today `(depth - 0.019, depth)`.
///
/// Stub: owner p18-cabinet-fronts.
pub fn front_plane(c: &Cabinet) -> (f64, f64) {
    (c.depth - 0.019, c.depth)
}
