//! Filler strips: the panel and the plinth of a filler cabinet.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::Cabinet;

use crate::Mesh;

/// The meshes of a filler (`c.filler`): its panel and its plinth only; its crown and its run's
/// counter come from the molding and counter parts.
///
/// Stub: owner p18-fillers-shelves.
pub fn filler_meshes(_model: &ArchModel, _c: &Cabinet) -> Vec<Mesh> {
    Vec::new()
}
