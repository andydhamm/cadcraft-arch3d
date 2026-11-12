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
}
