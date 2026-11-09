//! Stem foundations (phase 16; owner: p16-foundation-stem): a concrete wall on a footing under
//! every exterior wall on grade.

use super::FoundationIn;
use crate::mesh::Mesh;

/// The stem walls and footings of a stem foundation.
pub(crate) fn meshes(i: &FoundationIn) -> Vec<Mesh> {
    let _level = &i.level; // read once stem foundations are built
    Vec::new()
}
