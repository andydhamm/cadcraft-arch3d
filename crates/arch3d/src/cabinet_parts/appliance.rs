//! Appliance fronts and bodies.

use cadcraft_arch::casework_layout::FrontInfo;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::Cabinet;

use crate::Mesh;

/// The meshes of the appliance fronts in `layout`.
///
/// Stub: owner p18-face-appliances.
pub fn appliance_meshes(_model: &ArchModel, _c: &Cabinet, _layout: &[FrontInfo]) -> Vec<Mesh> {
    Vec::new()
}
