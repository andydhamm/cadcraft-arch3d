//! Stairs and railings (phase 12).

use super::Ctx;

/// One `IfcStair` (key `stair/<n>`, `PredefinedType` from the stair's shape) per stair in its
/// `from_level` storey, and one `IfcRailing` (`.HANDRAIL.` for a stair's own rail, else
/// `.GUARDRAIL.`; key `railing/<n>`) per railing in its level storey, bodies from `ctx.pieces`.
/// Filled by p12-ifc-stairs.
pub(super) fn emit(ctx: &mut Ctx) {
    let _ = (ctx);
}
