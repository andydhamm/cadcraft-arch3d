//! The phase-17 rows of `arch.inspect`: stair, railing, room and opening detail fields.
//!
//! [`inspect_details`] walks the rows `run_inspect` already built, zipped with the model in model
//! order, and adds the phase-17 keys. Read-only.

use cadcraft_arch::railing::{built_path, post_points};
use cadcraft_arch3d::openings::glass_plane;
use cadcraft_doc::arch::{ArchModel, RailingHost, RoomType, room_label, stair_label};
use serde_json::{Map as JsonMap, Value, json};

/// Add the phase-17 fields to the `stairs`, `railings`, `rooms` and `openings` rows in `out`.
pub(crate) fn inspect_details(model: &ArchModel, out: &mut JsonMap<String, Value>) {
    if let Some(Value::Array(rows)) = out.get_mut("stairs") {
        for (row, s) in rows.iter_mut().zip(model.stairs.iter()) {
            let Value::Object(obj) = row else { continue };
            obj.insert("openRisers".to_string(), json!(s.open_risers));
            obj.insert("stringers".to_string(), json!(s.stringers.name()));
            obj.insert("underside".to_string(), json!(s.underside));
            obj.insert("railLeft".to_string(), json!(s.rail_left.name()));
            obj.insert("railRight".to_string(), json!(s.rail_right.name()));
            obj.insert("planCut".to_string(), json!(s.plan_cut));
        }
    }
    if let Some(Value::Array(rows)) = out.get_mut("railings") {
        for (row, r) in rows.iter_mut().zip(model.railings.iter()) {
            let Value::Object(obj) = row else { continue };
            let posts: Vec<Value> = post_points(&built_path(&r.path, r.inset), r.post_spacing).iter().map(|p| json!([p.x, p.y])).collect();
            let follows = r.follows.map(|id| if r.host == RailingHost::Deck { room_label(id) } else { stair_label(id) });
            obj.insert("postSpacing".to_string(), json!(r.post_spacing));
            obj.insert("inset".to_string(), json!(r.inset));
            obj.insert("host".to_string(), json!(r.follows.map(|_| r.host.name())));
            obj.insert("auto".to_string(), json!(r.auto));
            obj.insert("posts".to_string(), Value::Array(posts));
            obj.insert("follows".to_string(), json!(follows));
        }
    }
    if let Some(Value::Array(rows)) = out.get_mut("rooms") {
        for (row, r) in rows.iter_mut().zip(model.rooms.iter()) {
            let Value::Object(obj) = row else { continue };
            if r.room_type != RoomType::Deck {
                continue;
            }
            let rails: Vec<&str> = r.deck_rails.iter().map(|s| s.name()).collect();
            obj.insert("boards".to_string(), json!(r.deck_boards));
            obj.insert("rails".to_string(), json!(rails));
        }
    }
    if let Some(Value::Array(rows)) = out.get_mut("openings") {
        for (row, o) in rows.iter_mut().zip(model.openings.iter()) {
            let Value::Object(obj) = row else { continue };
            let grille = o.grille.map(|g| json!({"across": g.across, "down": g.down, "part": if g.top_only { "top" } else { "whole" }}));
            let glass = glass_plane(model, o);
            obj.insert("top".to_string(), json!(o.top.name()));
            obj.insert("topRise".to_string(), json!(o.top_rise));
            obj.insert("roofOffset".to_string(), json!(o.roof_offset));
            obj.insert("grille".to_string(), json!(grille));
            obj.insert("sillSides".to_string(), json!(o.sill_sides.name()));
            obj.insert("casingSides".to_string(), json!(o.casing_sides.name()));
            obj.insert("glassCentre".to_string(), json!(glass.map(|g| [g.centre.x, g.centre.y, g.centre.z])));
            obj.insert("glassNormal".to_string(), json!(glass.map(|g| [g.normal.x, g.normal.y])));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{Opening, OpeningKind, Railing, RailingHost, RailingStyle, Room, RoomType, Stair, Wall};
    use cadcraft_geom::Vec2;
    use serde_json::json;

    fn make_out(stairs: bool, railings: bool, rooms: bool, openings: bool) -> JsonMap<String, Value> {
        let mut out = JsonMap::new();
        if stairs {
            out.insert("stairs".to_string(), Value::Array(vec![json!({"id": "s0"})]));
        }
        if railings {
            out.insert("railings".to_string(), Value::Array(vec![json!({"id": "ra0"})]));
        }
        if rooms {
            out.insert("rooms".to_string(), Value::Array(vec![json!({"id": "r0"})]));
        }
        if openings {
            out.insert("openings".to_string(), Value::Array(vec![json!({"id": "o0"})]));
        }
        out
    }

    #[test]
    fn inspect_details_empty_rows_no_crash() {
        let mut model = ArchModel::default();
        let mut out = JsonMap::new();
        out.insert("stairs".to_string(), Value::Array(vec![]));
        out.insert("railings".to_string(), Value::Array(vec![]));
        out.insert("rooms".to_string(), Value::Array(vec![]));
        out.insert("openings".to_string(), Value::Array(vec![]));
        inspect_details(&model, &mut out);
        assert_eq!(out["stairs"], json!([]));
        assert_eq!(out["railings"], json!([]));
        assert_eq!(out["rooms"], json!([]));
        assert_eq!(out["openings"], json!([]));
    }

    #[test]
    fn inspect_details_stair_defaults() {
        let mut model = ArchModel::default();
        model.push_stair(Stair {
            start: Vec2::new(0.0, 0.0),
            direction: 0.0,
            width: 1.0,
            from_level: 0,
            to_level: 1,
            rise: 2.7,
            risers: 16,
            riser_height: 0.16875,
            tread_depth: 0.254,
            treads: 15,
            run: 3.81,
            ..Stair::default()
        });
        let mut out = make_out(true, false, false, false);
        inspect_details(&model, &mut out);
        let s = &out["stairs"][0];
        assert_eq!(s["openRisers"], false);
        assert_eq!(s["stringers"], "two");
        assert_eq!(s["underside"], 0.3);
        assert_eq!(s["railLeft"], "auto");
        assert_eq!(s["railRight"], "auto");
        assert_eq!(s["planCut"], 1.2);
    }

    #[test]
    fn inspect_details_railing_follows_none_host_null() {
        let mut model = ArchModel::default();
        model.push_railing(Railing {
            path: vec![Vec2::new(0.0, 0.0), Vec2::new(5.0, 0.0)],
            level: 0,
            height: 0.914,
            style: RailingStyle::Balusters,
            thickness: 0.05,
            baluster_gap: 0.099,
            follows: None,
            ..Railing::default()
        });
        let mut out = make_out(false, true, false, false);
        inspect_details(&model, &mut out);
        let r = &out["railings"][0];
        assert!(r["host"].is_null(), "host should be null when follows is None");
        assert!(r["follows"].is_null());
        assert_eq!(r["auto"], false);
        assert_eq!(r["postSpacing"], 1.8);
        assert_eq!(r["inset"], 0.0);
        assert!(r["posts"].is_array());
    }

    #[test]
    fn inspect_details_deck_railing_host_is_room_label() {
        let mut model = ArchModel::default();
        model.push_room(Room {
            name: "Deck".into(),
            room_type: RoomType::Deck,
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
            deck_height: Some(0.9),
            ..Room::default()
        });
        model.push_railing(Railing {
            path: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)],
            level: 0,
            height: 0.914,
            style: RailingStyle::Balusters,
            thickness: 0.05,
            baluster_gap: 0.099,
            follows: Some(ArchId(0)),
            host: RailingHost::Deck,
            ..Railing::default()
        });
        let mut out = make_out(false, true, false, false);
        inspect_details(&model, &mut out);
        let r = &out["railings"][0];
        assert_eq!(r["host"], "deck");
        assert_eq!(r["follows"], "r0", "deck railing follows should be the room label");
    }

    #[test]
    fn inspect_details_opening_without_glass_has_null_glass_fields() {
        let mut model = ArchModel::default();
        let wid = model.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        model.push_opening(Opening::new(wid, 3.0, 0.9, 2.1, OpeningKind::Door));
        let mut out = make_out(false, false, false, true);
        inspect_details(&model, &mut out);
        let o = &out["openings"][0];
        assert!(o["glassCentre"].is_null(), "door has no glass");
        assert!(o["glassNormal"].is_null());
        assert_eq!(o["top"], "square");
        assert_eq!(o["topRise"], 0.0);
        assert_eq!(o["roofOffset"], 0.15);
        assert!(o["grille"].is_null());
        assert_eq!(o["sillSides"], "right");
        assert_eq!(o["casingSides"], "right");
    }

    #[test]
    fn inspect_details_window_has_glass_fields() {
        let mut model = ArchModel::default();
        let wid = model.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        model.push_opening(Opening::new(wid, 4.0, 1.0, 1.5, OpeningKind::Window));
        let mut out = make_out(false, false, false, true);
        inspect_details(&model, &mut out);
        let o = &out["openings"][0];
        assert!(o["glassCentre"].is_array(), "window should have glassCentre");
        assert!(o["glassNormal"].is_array());
        assert_eq!(o["glassCentre"].as_array().unwrap().len(), 3);
        assert_eq!(o["glassNormal"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn inspect_details_room_non_deck_gets_no_board_or_rail_keys() {
        let mut model = ArchModel::default();
        model.push_room(Room {
            name: "Living".into(),
            room_type: RoomType::Living,
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
            ..Room::default()
        });
        let mut out = make_out(false, false, true, false);
        inspect_details(&model, &mut out);
        let r = &out["rooms"][0];
        assert!(r.get("boards").is_none(), "non-deck room should not have boards key");
        assert!(r.get("rails").is_none(), "non-deck room should not have rails key");
    }

    #[test]
    fn inspect_details_deck_room_gets_boards_and_rails() {
        let mut model = ArchModel::default();
        model.push_room(Room {
            name: "Deck".into(),
            room_type: RoomType::Deck,
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
            deck_height: Some(0.9),
            deck_boards: Some(90.0),
            ..Room::default()
        });
        let mut out = make_out(false, false, true, false);
        inspect_details(&model, &mut out);
        let r = &out["rooms"][0];
        assert_eq!(r["boards"], 90.0);
        assert_eq!(r["rails"], json!([]));
    }

    #[test]
    fn inspect_details_stair_with_nan_direction() {
        let mut model = ArchModel::default();
        model.push_stair(Stair {
            start: Vec2::new(0.0, 0.0),
            direction: f64::NAN,
            width: 1.0,
            from_level: 0,
            to_level: 1,
            rise: 2.7,
            risers: 16,
            riser_height: 0.16875,
            tread_depth: 0.254,
            treads: 15,
            run: 3.81,
            ..Stair::default()
        });
        let mut out = make_out(true, false, false, false);
        inspect_details(&model, &mut out);
        let s = &out["stairs"][0];
        assert!(s.get("openRisers").is_some());
        assert!(s.get("stringers").is_some());
        assert!(s.get("underside").is_some());
    }
}
