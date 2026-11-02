//! Light fixtures (phase 12).

use super::Ctx;

/// One `IfcLightFixture` (key `light/<n>`, name `light-lf<n>`; `.POINTSOURCE.` for point and spot
/// lights, `.DIRECTIONSOURCE.` for area panels) per fixture, its body the housing meshes
/// (`ctx.pieces.get("light", id)`), in its level storey, with a `CADCraft_Light` property set
/// (`Type`, `Lumens`, `ColourTemperature` in kelvin, `On`). Filled by p12-ifc-lights.
pub(super) fn emit(ctx: &mut Ctx) {
    let _ = (ctx);
}
