//! Corner cabinets: blind and diagonal carcasses and fronts.

use cadcraft_arch::casework_corner::CornerInfo;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::Cabinet;

use crate::Mesh;

/// The carcass of a corner cabinet, or `None` when `c` builds the ordinary one.
///
/// Stub: owner p18-corners.
pub fn carcass(_model: &ArchModel, _c: &Cabinet, _info: Option<&CornerInfo>) -> Option<Mesh> {
    None
}

/// The fronts of a corner cabinet, or `None` when `c` builds the ordinary ones.
///
/// Stub: owner p18-corners.
pub fn fronts(_model: &ArchModel, _c: &Cabinet, _info: Option<&CornerInfo>) -> Option<Mesh> {
    None
}
