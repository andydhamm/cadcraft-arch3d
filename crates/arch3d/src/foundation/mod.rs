//! Foundations (phase 16; owner: p16-foundation-stem): stem walls on footings, or a slab on grade
//! with a thickened edge, under the exterior walls on grade.
//!
//! Meshes: stem walls `foundation`, footings and slab edges `footing`, slab floors `slab` (the
//! room's floor, built in the room's part).

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Foundation, FoundationKind, Level};

use crate::mesh::Mesh;

pub(crate) mod slab;
pub(crate) mod stem;

/// Everything a foundation's meshes are made from.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct FoundationIn {
    pub foundation: Foundation,
    pub level: Level,
}

/// The inputs of foundation `f` in `model`.
pub(crate) fn foundation_input(model: &ArchModel, f: &Foundation) -> FoundationIn {
    FoundationIn { foundation: f.clone(), level: crate::level_for(model, f.level) }
}

/// The meshes of one foundation, by its kind.
pub(crate) fn foundation_meshes(i: &FoundationIn) -> Vec<Mesh> {
    match i.foundation.kind {
        FoundationKind::Stem => stem::meshes(i),
        FoundationKind::Slab => slab::meshes(i),
    }
}
