//! The site's location and geographic elements (phase 12).
//!
//! `references` fills the `IfcSite`'s `RefLatitude`, `RefLongitude` and `RefElevation` from
//! the model's `SiteLocation`; `emit` writes the terrain, each plant and each draped path
//! (road/driveway/sidewalk) as `IfcGeographicElement`s placed in the site.

use super::Ctx;
use crate::step::{P, StepHeader};
use cadcraft_arch3d::{Mesh, Scene3d};
use cadcraft_doc::arch::{ArchId, ArchModel, SiteLocation};

/// `IfcSite`'s `RefLatitude`, `RefLongitude` and `RefElevation` from the model's site location:
/// latitude and longitude as `IfcCompoundPlaneAngleMeasure` lists (degrees, minutes, seconds,
/// millionths of a second, all with the angle's sign), the elevation of z = 0 in metres.
pub(super) fn references(model: &ArchModel) -> [P; 3] {
    let site = match model.site.as_ref() {
        Some(s) => s,
        None => return [P::Null, P::Null, P::Null],
    };
    let lat = if site.latitude.is_finite() { compound_angle(site.latitude) } else { P::Null };
    let lon = if site.longitude.is_finite() { compound_angle(site.longitude) } else { P::Null };
    let elev = if site.elevation.is_finite() { P::Real(site.elevation) } else { P::Null };
    [lat, lon, elev]
}

/// An `IfcCompoundPlaneAngleMeasure` list from a decimal angle in degrees.
/// Each of the four components carries the sign of the original angle; components are computed
/// from the absolute value in whole millionths so they never round past 60.
fn compound_angle(angle: f64) -> P {
    let total = angle.abs() * 3_600_000_000.0;
    if !total.is_finite() {
        return P::Null;
    }
    let total_millionths = total.round() as i64;
    let sign = if angle < 0.0 { -1 } else { 1 };
    let degrees = total_millionths / 3_600_000_000;
    let minutes = (total_millionths / 60_000_000) % 60;
    let seconds = (total_millionths / 1_000_000) % 60;
    let millis = total_millionths % 1_000_000;
    P::List(vec![P::Int(degrees * sign), P::Int(minutes * sign), P::Int(seconds * sign), P::Int(millis * sign)])
}

/// Write one `IfcGeographicElement` for the given meshes.
fn geographic_element(ctx: &mut Ctx, key: &str, name: &str, object_type: P, meshes: &[&Mesh], predefined: &'static str) {
    if meshes.is_empty() {
        return;
    }
    let g = super::guid::global_id(key);
    let placement = ctx.placement_in_site();
    if let Some(body) = ctx.body(meshes, 0.0) {
        let r = ctx.w.add(
            "IfcGeographicElement",
            vec![P::Str(g), P::Null, P::Str(name.to_string()), P::Null, object_type, P::Ref(placement), P::Ref(body), P::Null, P::Enum(predefined)],
        );
        ctx.contain_in_site(r);
    }
}

/// The terrain (key `terrain`, `.TERRAIN.`), each plant (`plant/<n>`, `.USERDEFINED.` with
/// `ObjectType` `PLANT`) and each road, driveway or sidewalk (`path/<n>`, `.USERDEFINED.`, its kind)
/// as `IfcGeographicElement`s placed in the site (`ctx.placement_in_site`, z from 0), bodies from
/// `ctx.pieces`, contained in the site (`ctx.contain_in_site`).
pub(super) fn emit(ctx: &mut Ctx) {
    // Terrain: one `IfcGeographicElement`, key `terrain`, `.TERRAIN.`
    for id in ctx.pieces.ids("terrain") {
        let meshes = ctx.pieces.get("terrain", id).to_vec();
        if !meshes.is_empty() {
            geographic_element(ctx, "terrain", "terrain", P::Null, &meshes, ".TERRAIN.");
        }
    }

    // Plants: one per plant, key `plant/<n>`, `.USERDEFINED.`, ObjectType `PLANT`
    for id in ctx.pieces.ids("plant") {
        let meshes = ctx.pieces.get("plant", id).to_vec();
        if !meshes.is_empty() {
            let n = id.0;
            geographic_element(ctx, &format!("plant/{n}"), &format!("plant-pl{n}"), P::Str("PLANT".into()), &meshes, ".USERDEFINED.");
        }
    }

    // Paths: one per path, key `path/<n>`, `.USERDEFINED.`, ObjectType `PATH`
    for id in ctx.pieces.ids("path") {
        let meshes = ctx.pieces.get("path", id).to_vec();
        if !meshes.is_empty() {
            let n = id.0;
            let kind = meshes.first().map_or("path", |m| m.material.split('-').nth(1).unwrap_or("path"));
            geographic_element(ctx, &format!("path/{n}"), &format!("{kind}-tm{n}"), P::Str("PATH".into()), &meshes, ".USERDEFINED.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchModel, SiteLocation};

    #[test]
    fn site_references_are_signed_compound_angles() {
        let mut model = ArchModel::default();
        model.site =
            Some(SiteLocation { latitude: -114.07, longitude: 43.65, elevation: 150.0, true_north: 0.0, reference: None, north_arrow_at: None });

        let [lat, lon, elev] = references(&model);

        // -114.07 * 3.6e9 = 410,652,000,000
        // degrees=114, rem=252,000,000
        // minutes=4, rem=12,000,000
        // seconds=12, rem=0
        // -> (-114, -4, -12, 0)
        match &lat {
            P::List(parts) => {
                assert_eq!(parts.len(), 4);
                assert_eq!(parts[0], P::Int(-114));
                assert_eq!(parts[1], P::Int(-4));
                assert_eq!(parts[2], P::Int(-12));
                assert_eq!(parts[3], P::Int(0));
            }
            _ => {
                let msg = format!("expected P::List for latitude, got {lat:?}");
                panic!("{msg}");
            }
        }

        // 43.65 * 3.6e9 = 157,140,000,000
        // degrees=43, rem=2,340,000,000
        // minutes=39, rem=0
        // -> (43, 39, 0, 0)
        match &lon {
            P::List(parts) => {
                assert_eq!(parts.len(), 4);
                assert_eq!(parts[0], P::Int(43));
                assert_eq!(parts[1], P::Int(39));
                assert_eq!(parts[2], P::Int(0));
                assert_eq!(parts[3], P::Int(0));
            }
            _ => {
                let msg = format!("expected P::List for longitude, got {lon:?}");
                panic!("{msg}");
            }
        }

        match elev {
            P::Real(e) => assert_eq!(e, 150.0),
            _ => {
                let msg = format!("expected P::Real for elevation, got {elev:?}");
                panic!("{msg}");
            }
        }
    }

    #[test]
    fn a_site_without_location_has_no_references() {
        let model = ArchModel::default();
        let [lat, lon, elev] = references(&model);
        assert!(matches!(lat, P::Null));
        assert!(matches!(lon, P::Null));
        assert!(matches!(elev, P::Null));
    }

    #[test]
    fn terrain_and_plants_are_geographic_elements_in_the_site() {
        let scene = Scene3d {
            meshes: vec![
                Mesh {
                    positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
                    indices: vec![0, 1, 2, 0, 2, 3],
                    material: "terrain".into(),
                    object: ArchId(0),
                    ..Mesh::default()
                },
                Mesh {
                    positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
                    indices: vec![0, 1, 2, 0, 2, 3],
                    material: "plant-trunk".into(),
                    object: ArchId(0),
                    ..Mesh::default()
                },
                Mesh {
                    positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
                    indices: vec![0, 1, 2, 0, 2, 3],
                    material: "path-asphalt-driveway".into(),
                    object: ArchId(0),
                    ..Mesh::default()
                },
            ],
            ..Scene3d::default()
        };
        let model = ArchModel::default();
        let mut ctx = crate::ifc::Ctx::new(&model, &scene);
        emit(&mut ctx);
        let text = ctx.w.finish(&crate::step::StepHeader::default());
        assert!(text.contains("IFCGEOGRAPHICELEMENT"));
        assert!(text.contains(".TERRAIN."));
        assert!(text.contains(".USERDEFINED."));
    }
}
