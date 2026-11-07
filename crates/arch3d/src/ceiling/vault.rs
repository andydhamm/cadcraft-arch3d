//! Vaulted ceilings (phase 16; owner: p16-vault-ceiling): a room's ceiling following the roof
//! underside over it.

use super::CeilingIn;
use crate::planar::Piece;

/// The level ceiling `pieces` of `i`'s room, following the roof underside where the room vaults.
pub(crate) fn apply(i: &CeilingIn, pieces: Vec<Piece>) -> Vec<Piece> {
    let _roofs = &i.roofs; // read once the vault is built
    pieces
}
