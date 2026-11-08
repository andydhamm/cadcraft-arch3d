//! Tray ceilings (phase 16; owner: p16-trays): the parts of a room's ceiling recessed up into the
//! attic, and the sides that close them.

use super::CeilingIn;
use crate::mesh::Mesh;
use crate::planar::Piece;

/// The ceiling `pieces` of `i`'s room, raised by its trays.
pub(crate) fn apply(i: &CeilingIn, pieces: Vec<Piece>) -> Vec<Piece> {
    let _trays = &i.trays; // read once trays are built
    pieces
}

/// The sides of `i`'s trays, down to the finished ceiling `drop` below the rough one.
pub(crate) fn side_meshes(_i: &CeilingIn, _drop: f64) -> Vec<Mesh> {
    Vec::new()
}
