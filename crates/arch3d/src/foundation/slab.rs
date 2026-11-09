//! Slab foundations (phase 16; owner: p16-foundation-slab): the thickened edge under a slab on
//! grade (the slab itself is the room's floor).

use super::FoundationIn;
use crate::mesh::Mesh;

/// The thickened edges of a slab foundation.
pub(crate) fn meshes(_i: &FoundationIn) -> Vec<Mesh> {
    Vec::new()
}
