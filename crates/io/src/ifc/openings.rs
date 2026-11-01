//! Openings, doors and windows (phase 12).

use super::Ctx;

/// For each opening whose host wall is in `ctx.walls`: an `IfcOpeningElement` (`.OPENING.`,
/// key `opening/<n>`) whose body is the void it cuts through its wall, an `IfcRelVoidsElement`
/// (`rel/voids/<n>`, the wall → the opening element), the `IfcDoor` or `IfcWindow` (key `door/<n>`
/// or `window/<n>`, body from `ctx.pieces.get("opening", id)`, `OverallHeight` and `OverallWidth`
/// from the opening) and an `IfcRelFillsElement` (`rel/fills/<n>`, the opening element → the door or
/// window), all in the host wall's storey. Filled by p12-ifc-openings.
pub(super) fn emit(ctx: &mut Ctx) {
    let _ = (ctx);
}
