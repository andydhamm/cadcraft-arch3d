use cadcraft_doc::arch::ArchModel;
use serde_json::{Map as JsonMap, Value};

pub(crate) fn inspect_room_finishes(model: &ArchModel, out: &mut JsonMap<String, Value>) {
    let Some(rooms) = out.get_mut("rooms").and_then(Value::as_array_mut) else {
        return;
    };
    for (row, room) in rooms.iter_mut().zip(model.rooms.iter()) {
        let mut obj = row.as_object_mut().unwrap();
        let f = cadcraft_arch::room_finish::template(room.room_type);
        obj.insert(
            "typeFinishes".to_string(),
            Value::Object(JsonMap::from_iter([
                ("floor".to_string(), Value::String(format!("finish:{}", f.floor.name()))),
                ("wall".to_string(), Value::String(format!("finish:{}", f.wall.name()))),
                ("ceiling".to_string(), Value::String(format!("finish:{}", f.ceiling.name()))),
            ])),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchId, Room, RoomType};
    use cadcraft_geom::Vec2;
    use serde_json::{Map, json};

    #[test]
    fn inspect_room_rows_report_type_finishes() {
        let mut model = ArchModel::default();
        model.push_room(Room {
            id: ArchId(1),
            room_type: RoomType::Kitchen,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(3.0, 2.0), Vec2::new(0.0, 2.0)],
            ..Room::default()
        });
        let mut out = Map::new();
        out.insert("rooms".to_string(), Value::Array(vec![json!({ "id": "r1" })]));
        inspect_room_finishes(&model, &mut out);
        let row = out["rooms"].as_array().unwrap().first().unwrap();
        assert_eq!(row["typeFinishes"], json!({ "floor": "finish:tile", "wall": "finish:drywall", "ceiling": "finish:plaster" }));
    }

    /// An empty rooms array is handled gracefully (no panic, no change).
    #[test]
    fn inspect_room_finishes_empty_rooms_array() {
        let model = ArchModel::default();
        let mut out = Map::new();
        out.insert("rooms".to_string(), Value::Array(vec![]));
        inspect_room_finishes(&model, &mut out);
        assert_eq!(out["rooms"].as_array().unwrap().len(), 0);
    }

    /// No "rooms" key in output: the function returns early without adding one.
    #[test]
    fn inspect_room_finishes_no_rooms_key() {
        let model = ArchModel::default();
        let mut out = Map::new();
        out.insert("walls".to_string(), Value::Array(vec![json!({ "id": "w0" })]));
        inspect_room_finishes(&model, &mut out);
        assert!(out.get("rooms").is_none());
        assert_eq!(out["walls"].as_array().unwrap().len(), 1);
    }

    /// More rows than model rooms: extra rows are left untouched (zip semantics).
    #[test]
    fn inspect_room_finishes_more_rows_than_rooms() {
        let mut model = ArchModel::default();
        model.push_room(Room {
            id: ArchId(1),
            room_type: RoomType::Kitchen,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(3.0, 2.0), Vec2::new(0.0, 2.0)],
            ..Room::default()
        });
        let mut out = Map::new();
        out.insert("rooms".to_string(), Value::Array(vec![json!({ "id": "r1" }), json!({ "id": "r2" })]));
        inspect_room_finishes(&model, &mut out);
        let rooms = out["rooms"].as_array().unwrap();
        assert_eq!(rooms.len(), 2);
        assert!(rooms[0].get("typeFinishes").is_some());
        assert!(rooms[1].get("typeFinishes").is_none());
    }

    /// Different room types get different floor finishes in the template.
    #[test]
    fn inspect_room_finishes_varies_by_type() {
        let mut model = ArchModel::default();
        model.push_room(Room {
            id: ArchId(1),
            room_type: RoomType::Garage,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(3.0, 2.0), Vec2::new(0.0, 2.0)],
            ..Room::default()
        });
        model.push_room(Room {
            id: ArchId(2),
            room_type: RoomType::Bedroom,
            polygon: vec![Vec2::new(4.0, 0.0), Vec2::new(7.0, 0.0), Vec2::new(7.0, 2.0), Vec2::new(4.0, 2.0)],
            ..Room::default()
        });
        model.push_room(Room {
            id: ArchId(3),
            room_type: RoomType::Living,
            polygon: vec![Vec2::new(8.0, 0.0), Vec2::new(11.0, 0.0), Vec2::new(11.0, 2.0), Vec2::new(8.0, 2.0)],
            ..Room::default()
        });
        let mut out = Map::new();
        out.insert("rooms".to_string(), Value::Array(vec![json!({ "id": "r1" }), json!({ "id": "r2" }), json!({ "id": "r3" })]));
        inspect_room_finishes(&model, &mut out);
        let rooms = out["rooms"].as_array().unwrap();
        assert_eq!(rooms[0]["typeFinishes"]["floor"], "finish:concrete");
        assert_eq!(rooms[1]["typeFinishes"]["floor"], "finish:carpet");
        assert_eq!(rooms[2]["typeFinishes"]["floor"], "finish:wood");
        assert_eq!(rooms[0]["typeFinishes"]["wall"], "finish:drywall");
        assert_eq!(rooms[1]["typeFinishes"]["wall"], "finish:drywall");
        assert_eq!(rooms[2]["typeFinishes"]["wall"], "finish:drywall");
        assert_eq!(rooms[0]["typeFinishes"]["ceiling"], "finish:plaster");
        assert_eq!(rooms[1]["typeFinishes"]["ceiling"], "finish:plaster");
        assert_eq!(rooms[2]["typeFinishes"]["ceiling"], "finish:plaster");
    }
}
