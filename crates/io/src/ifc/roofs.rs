//! Roofs (phase 12).

use super::Ctx;

/// One `IfcRoof` (key `roof/<n>`, no body of its own) per roof, aggregating (`IfcRelAggregates`,
/// `rel/aggregates/roof/<n>`) one `IfcSlab` (`.ROOF.`, key `roofplane/<n>/<i>`) per roof-plane mesh
/// and one `IfcCovering` (`.CLADDING.`, keys `fascia/<n>` and `soffit/<n>`) for its fascia and its
/// soffit, all in the roof's level storey (the `IfcRoof` is contained; its parts are not).
/// Filled by p12-ifc-roofs.
pub(super) fn emit(ctx: &mut Ctx) {
    let _ = (ctx);
}
