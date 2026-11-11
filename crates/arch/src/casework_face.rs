//! Fronts of a cabinet with a face layout (phase 18). Stub: the layout itself is filled by the
//! cabinet-fronts segment; until then a face layout lays out no fronts.

use crate::casework_layout::FrontInfo;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Cabinet, FaceLayout};

/// The fronts of `c` laid out by its face layout, in the order arch3d builds them.
#[allow(unused_variables)]
pub fn face_fronts(model: &ArchModel, c: &Cabinet, face: &FaceLayout) -> Vec<FrontInfo> {
    Vec::new()
}
