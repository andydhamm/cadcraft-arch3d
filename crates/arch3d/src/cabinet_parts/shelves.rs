//! Open shelving: the open carcass and the shelves.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::Cabinet;

use crate::Mesh;

/// The carcass of a cabinet with open shelving, or `None` for a solid carcass.
///
/// Stub: owner p18-fillers-shelves.
pub fn open_carcass(_model: &ArchModel, _c: &Cabinet) -> Option<Mesh> {
    None
}

/// The shelves of a cabinet.
///
/// Stub: owner p18-fillers-shelves.
pub fn shelf_meshes(_model: &ArchModel, _c: &Cabinet) -> Vec<Mesh> {
    Vec::new()
}
