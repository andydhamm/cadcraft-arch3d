//! Spaces (phase 12).

use super::Ctx;

/// One `IfcSpace` (`.SPACE.`, key `space/<n>`, name the room's name, `LongName` its type) per
/// room, its body the room polygon (with holes) extruded from the floor to the ceiling height,
/// aggregated in its level storey (`ctx.aggregate_space`), with a `CADCraft_RoomAreas` property set
/// (`InteriorArea`, `StandardArea` as `IFCAREAMEASURE`) and an `IfcElementQuantity`
/// `Qto_SpaceBaseQuantities` (`NetFloorArea` = interior, `GrossFloorArea` = standard). Filled by
/// p12-ifc-spaces.
pub(super) fn emit(ctx: &mut Ctx) {
    let _ = (ctx);
}
