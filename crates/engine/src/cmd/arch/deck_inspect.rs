//! The `decks` rows of `arch.inspect` with `decks: true` (phase 17).
//!
//! One row per Deck room: its top, the direction its boards run (degrees in [0, 180)) and the frame
//! under it (`cadcraft_arch::deck::frame`): the ledger, joists, rims, beams as centre lines, and the
//! posts with the grade each stands on. A deck without a frame keeps `ledger` null and empty lists.

use cadcraft_arch::deck::{self, Member};
use cadcraft_doc::arch::{ArchModel, RoomType, room_label, wall_label};
use cadcraft_geom::Vec2;
use serde_json::{Map as JsonMap, Value, json};

fn point(p: Vec2) -> Value {
    json!([p.x, p.y])
}

fn line(m: &Member) -> Value {
    json!([point(m.a), point(m.b)])
}

/// Add `decks` to `out` when `p` asks for it. Read-only.
pub(crate) fn inspect_decks(model: &ArchModel, p: &Value, out: &mut JsonMap<String, Value>) {
    if p.get("decks").and_then(Value::as_bool) != Some(true) {
        return;
    }
    let grade = deck::site_grade(model);
    let rows: Vec<Value> = model
        .rooms
        .iter()
        .filter(|r| r.room_type == RoomType::Deck)
        .map(|r| {
            let elevation = model.level(r.level).map_or(0.0, |l| l.elevation);
            let frame = deck::frame(model, r, &grade);
            let boards = match &frame {
                Some(f) => f.board_dir.y.atan2(f.board_dir.x).to_degrees(),
                None => r.deck_boards.filter(|b| b.is_finite()).unwrap_or(0.0),
            };
            let boards = boards.rem_euclid(180.0);
            let boards = if boards.is_finite() { boards } else { 0.0 };
            let mut row = json!({
                "id": room_label(r.id),
                "name": r.name,
                "level": r.level,
                "top": elevation + r.deck_height.unwrap_or(0.0),
                "boardDirection": boards,
                "ledger": null,
                "joists": [],
                "rims": [],
                "beams": [],
                "posts": [],
                "postGrades": [],
            });
            if let (Some(f), Some(obj)) = (&frame, row.as_object_mut()) {
                let edges = deck::deck_edges(model, r);
                if let (Some(l), Some(e)) = (&f.ledger, deck::ledger_edge(&edges).and_then(|i| edges.get(i))) {
                    obj.insert("ledger".to_string(), json!({ "edge": e.index, "wall": e.wall.map(wall_label), "line": line(l) }));
                }
                obj.insert("joists".to_string(), Value::Array(f.joists.iter().map(line).collect()));
                obj.insert("rims".to_string(), Value::Array(f.rims.iter().map(line).collect()));
                obj.insert("beams".to_string(), Value::Array(f.beams.iter().map(line).collect()));
                obj.insert("posts".to_string(), Value::Array(f.posts.iter().map(|p| point(p.at)).collect()));
                obj.insert("postGrades".to_string(), Value::Array(f.posts.iter().map(|p| json!(p.grade)).collect()));
            }
            row
        })
        .collect();
    out.insert("decks".to_string(), Value::Array(rows));
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::Room;
    use cadcraft_doc::{ArchId, Wall};

    fn model(boards: Option<f64>) -> ArchModel {
        let mut m = ArchModel::default();
        m.push_wall(Wall::new(Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0), 0.2));
        m.rooms.push(Room {
            id: ArchId(3),
            room_type: RoomType::Deck,
            polygon: vec![Vec2::new(2.0, 8.1), Vec2::new(6.0, 8.1), Vec2::new(6.0, 11.1), Vec2::new(2.0, 11.1)],
            deck_height: Some(0.9),
            deck_boards: boards,
            ..Room::default()
        });
        m
    }

    fn rows(m: &ArchModel) -> Vec<Value> {
        let mut out = JsonMap::new();
        inspect_decks(m, &json!({"decks": true}), &mut out);
        out.get("decks").and_then(Value::as_array).cloned().unwrap_or_default()
    }

    #[test]
    fn decks_rows_report_the_frame() {
        let m = model(None);
        let rows = rows(&m);
        assert_eq!(rows.len(), 1);
        let d = &rows[0];
        assert_eq!(d["id"], "r3");
        assert_eq!(d["boardDirection"].as_f64(), Some(0.0));
        assert_eq!(d["ledger"]["wall"], "w0");
        assert_eq!(d["ledger"]["edge"], 0);
        assert_eq!(d["joists"].as_array().map(Vec::len), Some(9));
        assert_eq!(d["rims"].as_array().map(Vec::len), Some(3));
        assert_eq!(d["beams"].as_array().map(Vec::len), Some(1));
        assert_eq!(d["posts"].as_array().map(Vec::len), Some(3));
        assert_eq!(d["postGrades"], json!([0.0, 0.0, 0.0]));
        let x = d["posts"][1][0].as_f64().unwrap_or(f64::NAN);
        assert!((x - 4.0).abs() < 1e-9);
        // The direction is reported in [0, 180).
        let turned = rows_of(Some(-90.0));
        assert_eq!(turned[0]["boardDirection"].as_f64().map(|v| v.round()), Some(90.0));
        // No frame: nothing but the row.
        let mut flat = model(None);
        flat.rooms[0].deck_height = None;
        let r = &rows_of_model(&flat)[0];
        assert!(r["ledger"].is_null() && r["joists"] == json!([]) && r["posts"] == json!([]));
        // Not asked: no rows.
        let mut out = JsonMap::new();
        inspect_decks(&m, &json!({}), &mut out);
        assert!(out.is_empty());
    }

    fn rows_of(boards: Option<f64>) -> Vec<Value> {
        rows(&model(boards))
    }

    fn rows_of_model(m: &ArchModel) -> Vec<Value> {
        rows(m)
    }
}
