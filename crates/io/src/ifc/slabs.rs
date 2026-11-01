//! Floors, decks and ceilings (phase 12).

use super::Ctx;

/// One `IfcSlab` per room floor (`.FLOOR.`, key `floor/<room n>`) and per deck (`.FLOOR.`, key
/// `deck/<room n>`), and one `IfcCovering` (`.CEILING.`, key `ceiling/<room n>`) per ceiling, each
/// with its body from `ctx.pieces` and contained in its room's level storey. Filled by
/// p12-ifc-slabs.
pub(super) fn emit(ctx: &mut Ctx) {
    let _ = (ctx);
}
