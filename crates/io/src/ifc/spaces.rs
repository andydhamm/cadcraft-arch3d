//! Spaces (phase 12).
//!
//! One `IfcSpace` (`.SPACE.`, key `space/<n>`, name the room's name, `LongName` its type) per
//! room, its body the room polygon (with holes) extruded from the floor to the ceiling height,
//! aggregated in its level storey (`ctx.aggregate_space`), with a `CADCraft_RoomAreas` property
//! set (`InteriorArea`, `StandardArea` as `IFCAREAMEASURE`) and an `IfcElementQuantity`
//! `Qto_SpaceBaseQuantities` (`NetFloorArea` = interior, `GrossFloorArea` = standard).

use super::Ctx;
use super::guid::global_id;
use crate::step::P;
use cadcraft_arch3d::Mesh;
use cadcraft_doc::arch::{Level, RoomType};

/// A human-readable IFC long name for a room type.
fn room_type_long_name(t: RoomType) -> &'static str {
    match t {
        RoomType::Living => "LIVING ROOM",
        RoomType::Bedroom => "BEDROOM",
        RoomType::Kitchen => "KITCHEN",
        RoomType::Bath => "BATHROOM",
        RoomType::Closet => "CLOSET",
        RoomType::Utility => "UTILITY",
        RoomType::Dining => "DINING ROOM",
        RoomType::Family => "FAMILY ROOM",
        RoomType::Entry => "ENTRY",
        RoomType::Laundry => "LAUNDRY",
        RoomType::Garage => "GARAGE",
        RoomType::Porch => "PORCH",
        RoomType::Deck => "DECK",
        RoomType::OpenBelow => "OPEN BELOW",
        RoomType::Other => "OTHER",
    }
}

/// One `IfcSpace` per room: the space's body is the room polygon extruded to the ceiling, the
/// long name is the room type, and the areas (interior and standard) are exposed through a
/// `CADCraft_RoomAreas` property set and a `Qto_SpaceBaseQuantities` element quantity.
pub(super) fn emit(ctx: &mut Ctx) {
    for room in &ctx.model.rooms {
        let n = room.id.0;
        let slot = ctx.storey_for(room.level);
        let level = ctx.model.levels.iter().find(|l| l.index == room.level).cloned().unwrap_or(Level::default());

        let name = if room.name.trim().is_empty() { format!("r{}", n) } else { room.name.clone() };

        let long_name = room_type_long_name(room.room_type);

        // The body is written when the polygon is valid (>= 3 points, all finite, holes finite)
        // AND the areas are finite (a corrupt room yields a body-less space).
        let polygon_ok =
            room.polygon.len() >= 3 && room.polygon.iter().all(|p| p.is_finite()) && room.holes.iter().all(|h| h.iter().all(|p| p.is_finite()));
        let interior_finite = room.interior_area.is_finite();
        let standard_finite = room.standard_area.is_finite();
        let has_body = polygon_ok && interior_finite && standard_finite;

        // Write IfcSpace directly (not via ctx.element which would contain it).
        // IFC4: IfcProduct (GlobalId, OwnerHistory, Name, Description, ObjectType,
        // ObjectPlacement, Representation) + IfcSpace (LongName, [Type fields]).
        let placement = ctx.placement_in_storey(slot);
        let params = if has_body {
            let z0 = room.floor_height.unwrap_or(0.0);
            let z1 = z0 + room.ceiling_height.unwrap_or(level.ceiling_height);
            let mut mesh = Mesh::default();
            mesh.material = "space".into();
            mesh.object = room.id;
            mesh.push_prism_with_holes(&room.polygon, &room.holes, z0, z1);
            let body = ctx.body(&[&mesh], 0.0).map_or(P::Null, P::Ref);
            vec![
                P::str(global_id(&format!("space/{}", n))),
                P::Null,
                P::str(&name),
                P::Null,
                P::Null,
                P::Ref(placement),
                body,
                P::str(long_name),
                P::Enum("ELEMENT"),
                P::Enum("SPACE"),
                P::Null,
            ]
        } else {
            vec![
                P::str(global_id(&format!("space/{}", n))),
                P::Null,
                P::str(&name),
                P::Null,
                P::Null,
                P::Ref(placement),
                P::Null,
                P::str(long_name),
                P::Enum("ELEMENT"),
                P::Enum("SPACE"),
                P::Null,
            ]
        };
        let space = ctx.w.add("IfcSpace", params);
        ctx.aggregate_space(slot, space);

        // CADCraft_RoomAreas property set (only when at least one area is finite).
        let mut props = Vec::new();
        if interior_finite {
            props.push(("InteriorArea", P::typed("IFCAREAMEASURE", P::Real(room.interior_area))));
        }
        if standard_finite {
            props.push(("StandardArea", P::typed("IFCAREAMEASURE", P::Real(room.standard_area))));
        }
        if !props.is_empty() {
            ctx.pset("CADCraft_RoomAreas", &format!("space/{}", n), props, &[space]);
        }

        // Qto_SpaceBaseQuantities: NetFloorArea and GrossFloorArea as IfcQuantityArea.
        let mut quantities = Vec::new();
        if interior_finite {
            let q = ctx.w.add("IfcQuantityArea", vec![P::str("NetFloorArea"), P::Null, P::Null, P::Real(room.interior_area), P::Null]);
            quantities.push(q);
        }
        if standard_finite {
            let q = ctx.w.add("IfcQuantityArea", vec![P::str("GrossFloorArea"), P::Null, P::Null, P::Real(room.standard_area), P::Null]);
            quantities.push(q);
        }
        if !quantities.is_empty() {
            let qto = ctx.w.add(
                "IfcElementQuantity",
                vec![
                    P::str(global_id(&format!("qto/Qto_SpaceBaseQuantities/space/{}", n))),
                    P::Null,
                    P::str("Qto_SpaceBaseQuantities"),
                    P::Null,
                    P::Null,
                    P::refs(&quantities),
                ],
            );
            ctx.rel("IfcRelDefinesByProperties", &format!("rel/props/Qto_SpaceBaseQuantities/space/{}", n), vec![P::refs(&[space]), P::Ref(qto)]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{IfcOptions, write_model};
    use cadcraft_arch3d::Scene3d;
    use cadcraft_doc::arch::{ArchModel, Room, RoomType};
    use cadcraft_geom::Vec2;

    /// A model with a single room on level 0, ceiling height 2.55 m (floor 0).
    fn room_model() -> ArchModel {
        let mut m = ArchModel::default();
        m.push_room(Room {
            id: cadcraft_doc::ArchId(m.next_room_id),
            name: "Living Room".into(),
            room_type: RoomType::Living,
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
            holes: Vec::new(),
            ceiling_height: Some(2.55),
            floor_height: None,
            interior_area: 24.0,
            standard_area: 26.4,
            ..Room::default()
        });
        m
    }

    #[test]
    fn each_room_is_a_space_with_its_areas() {
        let m = room_model();
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert!(text.contains("IFCSPACE"), "no IfcSpace entity: {text}");
        assert!(text.contains("'Living Room'"), "room name missing: {text}");
        assert!(text.contains("CADCraft_RoomAreas"), "room areas pset missing: {text}");
        assert!(text.contains("InteriorArea"), "InteriorArea missing: {text}");
        assert!(text.contains("StandardArea"), "StandardArea missing: {text}");
        assert!(text.contains("NetFloorArea"), "NetFloorArea missing: {text}");
        assert!(text.contains("GrossFloorArea"), "GrossFloorArea missing: {text}");
    }

    #[test]
    fn a_space_spans_floor_to_ceiling() {
        let m = room_model();
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        // z0 = 0.0, z1 = 0.0 + 2.55 = 2.55 (storey coordinates, no offset).
        assert!(text.contains("2.55"), "ceiling z 2.55 not found: {text}");
    }

    #[test]
    fn every_cartesian_point_is_one_coordinate_list() {
        // IfcCartesianPoint has one attribute, Coordinates (a list): IFCCARTESIANPOINT((x,y,z)).
        // IfcOpenShell refuses IfcCartesianPoint(2.55) as CP2Dor3D (phase-12/11).
        let m = room_model();
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        for (at, _) in text.match_indices("=IFCCARTESIANPOINT(") {
            let rest = &text[at + "=IFCCARTESIANPOINT(".len()..];
            assert!(rest.starts_with('('), "a point without its coordinate list: {}", &text[at..at + 60.min(text.len() - at)]);
        }
    }

    #[test]
    fn spaces_are_aggregated_in_their_storey() {
        let m = room_model();
        let with_space = text_with_count(&m);
        let empty = ArchModel::default();
        let no_space = text_with_count(&empty);
        assert!(with_space > no_space, "expected more IfcRelAggregates with a space: {} vs {}", with_space, no_space);
    }

    fn text_with_count(model: &ArchModel) -> usize {
        let text = write_model(model, &Scene3d::build(model), &IfcOptions::default());
        text.matches("IFCRELAGGREGATES").count()
    }

    #[test]
    fn a_room_with_an_empty_name_gets_an_rn_name() {
        let mut m = room_model();
        m.rooms[0].name = "".into();
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert!(text.contains("'r0'"), "empty room name should fall back to r0: {text}");
        assert!(!text.contains("'Living Room'"), "original name should not appear: {text}");
    }

    #[test]
    fn a_room_with_nan_areas_has_no_body_and_no_area_data() {
        let mut m = room_model();
        m.rooms[0].interior_area = f64::NAN;
        m.rooms[0].standard_area = f64::NAN;
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert!(text.contains("IFCSPACE"), "space should still be written: {text}");
        assert!(!text.contains("CADCraft_RoomAreas"), "no areas pset when both areas are NaN: {text}");
        assert!(!text.contains("NetFloorArea"), "no NetFloorArea when interior is NaN: {text}");
        assert!(!text.contains("GrossFloorArea"), "no GrossFloorArea when standard is NaN: {text}");
    }

    #[test]
    fn a_room_with_only_interior_area_finite_writes_only_that() {
        let mut m = room_model();
        m.rooms[0].standard_area = f64::NAN;
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert!(text.contains("CADCraft_RoomAreas"), "areas pset should be written: {text}");
        assert!(text.contains("InteriorArea"), "InteriorArea should be written: {text}");
        assert!(!text.contains("StandardArea"), "StandardArea should not be written when NaN: {text}");
        assert!(text.contains("NetFloorArea"), "NetFloorArea should be written: {text}");
        assert!(!text.contains("GrossFloorArea"), "GrossFloorArea should not be written when NaN: {text}");
    }

    #[test]
    fn a_room_with_fewer_than_three_points_has_no_body() {
        let mut m = room_model();
        m.rooms[0].polygon = vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0)];
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert!(text.contains("IFCSPACE"), "space should still be written: {text}");
        assert!(!text.contains("IFCTRANGULATEDFACESET"), "no body means no triangulated face set: {text}");
        assert!(text.contains("CADCraft_RoomAreas"), "areas pset should still be written: {text}");
    }

    #[test]
    fn a_room_with_floor_height_uses_it_as_z0() {
        let mut m = room_model();
        m.rooms[0].floor_height = Some(0.5);
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        // z0 = 0.5, z1 = 0.5 + 2.55 = 3.05
        assert!(text.contains("3.05"), "ceiling z 3.05 not found: {text}");
    }

    #[test]
    fn multiple_rooms_produce_multiple_spaces() {
        let mut m = room_model();
        m.push_room(Room {
            id: cadcraft_doc::ArchId(m.next_room_id),
            name: "Bedroom".into(),
            room_type: RoomType::Bedroom,
            level: 0,
            polygon: vec![Vec2::new(6.0, 0.0), Vec2::new(12.0, 0.0), Vec2::new(12.0, 4.0), Vec2::new(6.0, 4.0)],
            holes: Vec::new(),
            ceiling_height: Some(2.55),
            floor_height: None,
            interior_area: 24.0,
            standard_area: 26.4,
            ..Room::default()
        });
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        let count = text.matches("IFCSPACE").count();
        assert_eq!(count, 2, "expected 2 IfcSpace entities, got {count}: {text}");
        assert!(text.contains("'Living Room'"), "first room name missing: {text}");
        assert!(text.contains("'Bedroom'"), "second room name missing: {text}");
    }
}
