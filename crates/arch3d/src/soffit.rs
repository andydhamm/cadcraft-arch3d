//! Interior soffits (phase 16; owner: p16-soffits): boxes hung from the ceiling, and the material
//! definitions of their faces.
//!
//! Their meshes are `soffit-box` (or `soffit-box:<material key>`), part `soffit`; the roof's own
//! soffit (material `soffit`) stays part `roof`.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Level, Soffit};

use crate::Scene3d;
use crate::mesh::Mesh;

/// Everything a soffit's meshes are made from.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SoffitIn {
    pub soffit: Soffit,
    pub level: Level,
}

/// The inputs of soffit `s` in `model`.
pub(crate) fn soffit_input(model: &ArchModel, s: &Soffit) -> SoffitIn {
    SoffitIn { soffit: s.clone(), level: crate::level_for(model, s.level) }
}

/// The meshes of one soffit.
pub(crate) fn soffit_meshes(i: &SoffitIn) -> Vec<Mesh> {
    let _inputs = (&i.soffit, &i.level); // read once soffits are built
    Vec::new()
}

/// Add the material definitions of the soffits' faces to `scene`.
pub(crate) fn materials(_model: &ArchModel, _scene: &mut Scene3d) {}
