//! Paint in 3D (phase 14): painted surfaces as their own `paint:<surface key>` meshes and the
//! material definitions they take.
//!
//! These are the final signatures, kept as no-ops until p14-arch3d-paint fills them: [`apply`]
//! runs at the end of every part builder of the scene (`incremental::Sections`), and [`materials`]
//! after every build or update (with the symbols' materials). Every part's inputs already record the
//! paints of its object, so a paint change rebuilds exactly the object it paints.

use cadcraft_doc::ArchModel;

use crate::{Mesh, Scene3d};

/// Split out or rename the painted surfaces among one part's `meshes` as `paint:<surface key>`
/// meshes. A no-op for now.
#[allow(clippy::ptr_arg)] // The final signature: painting splits a mesh into several.
pub fn apply(model: &ArchModel, meshes: &mut Vec<Mesh>) {
    let _ = (model, meshes);
}

/// Add the [`crate::MaterialDef`] (and texture) of every `paint:` mesh in `scene`. A no-op for now.
pub fn materials(model: &ArchModel, scene: &mut Scene3d) {
    let _ = (model, scene);
}
