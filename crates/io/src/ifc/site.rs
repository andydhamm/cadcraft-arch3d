//! The site's location and geographic elements (phase 12).
//!
//! `references` fills the `IfcSite`'s `RefLatitude`, `RefLongitude` and `RefElevation` from
//! the model's `SiteLocation`; `emit` writes the terrain, each plant and each draped path
//! (road/driveway/sidewalk) as `IfcGeographicElement`s placed in the site.

use super::Ctx;
use crate::step::P;
use cadcraft_doc::arch::{ArchModel, ModifierKind, PathKind, TerrainModifier};

/// `IfcSite`'s `RefLatitude`, `RefLongitude` and `RefElevation` from the model's site location:
/// latitude and longitude as `IfcCompoundPlaneAngleMeasure` lists (degrees, minutes, seconds,
/// millionths of a second, all with the angle's sign), the elevation of z = 0 in metres; `$` each
/// without a site. Filled by p12-ifc-site.
pub(super) fn references(model: &ArchModel) -> [P; 3] {
    let _ = (model);
    [P::Null, P::Null, P::Null]
}

/// The terrain (key `terrain`, `.TERRAIN.`), each plant (`plant/<n>`, `.USERDEFINED.` with
/// `ObjectType` `PLANT`) and each road, driveway or sidewalk (`path/<n>`, `.USERDEFINED.`, its kind)
/// as `IfcGeographicElement`s placed in the site (`ctx.placement_in_site`, z from 0), bodies from
/// `ctx.pieces`, contained in the site (`ctx.contain_in_site`). Filled by p12-ifc-site.
pub(super) fn emit(ctx: &mut Ctx) {
    let _ = (ctx);
}
