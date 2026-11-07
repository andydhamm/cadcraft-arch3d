//! Wall rises (phase 16; owner: p16-vault-walls): the part of a wall bounding a vaulted room that
//! rises from its top to the roof underside, along its thickness.

use cadcraft_arch::join::WallShape;
use cadcraft_doc::{ArchModel, Wall};

use crate::mesh::Mesh;

/// Everything a wall's rise is made from.
#[allow(dead_code)] // built once rise_input is
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RiseIn {}

/// The inputs of `wall`'s rise, or `None` when it has none. `shape` is its join shape, `span` its
/// base and top, `roof_top` whether a roof clips its top.
pub(crate) fn rise_input(_model: &ArchModel, _wall: &Wall, _shape: Option<&WallShape>, _span: Option<(f64, f64)>, _roof_top: bool) -> Option<RiseIn> {
    None
}

/// The meshes of `wall`'s rise.
pub(crate) fn rise_meshes(_wall: &Wall, _rise: &RiseIn) -> Vec<Mesh> {
    Vec::new()
}
