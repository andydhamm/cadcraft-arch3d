//! Tests for the phase-17 rows of `arch.inspect` (`inspect_detail.rs`, `deck_inspect.rs`).

use crate::Session;
use cadcraft_doc::arch::{Grille, Opening, OpeningKind, OpeningSides, OpeningTop, RailSetting, Railing, RailingHost, Room, RoomType, Stair, Wall};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

fn inspect(s: &mut Session, p: Value) -> Value {
    s.execute("arch.inspect", &p).unwrap_or(Value::Null)
}

fn with_model(f: impl FnOnce(&mut cadcraft_doc::arch::ArchModel)) -> Session {
    let mut s = Session::new();
    if let Ok(d) = s.doc_mut() {
        f(&mut d.arch);
        d.touch_arch();
    }
    s
}

fn square() -> Vec<Vec2> {
    vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)]
}

#[test]
fn stair_rows_report_the_detail_defaults() {
    let mut s = with_model(|m| {
        m.push_stair(Stair::default());
    });
    let v = inspect(&mut s, json!({"stairs": true}));
    let st = &v["stairs"][0];
    assert_eq!(st["openRisers"], json!(false));
    assert_eq!(st["stringers"], "two");
    assert_eq!(st["underside"], json!(0.3));
    assert_eq!(st["railLeft"], "auto");
    assert_eq!(st["railRight"], "auto");
    assert_eq!(st["planCut"], json!(1.2));
}

#[test]
fn railing_rows_report_spacing_inset_host_auto_posts() {
    let mut s = with_model(|m| {
        m.push_railing(Railing { path: vec![Vec2::new(0.0, 0.0), Vec2::new(5.0, 0.0)], ..Railing::default() });
    });
    let v = inspect(&mut s, json!({"railings": true}));
    let r = &v["railings"][0];
    assert_eq!(r["postSpacing"], json!(1.8));
    assert_eq!(r["inset"], json!(0.0));
    assert!(r["host"].is_null() && r["follows"].is_null());
    assert_eq!(r["auto"], json!(false));
    assert_eq!(r["posts"].as_array().map(Vec::len), Some(4));
    assert!((r["posts"][1][0].as_f64().unwrap_or(0.0) - 5.0 / 3.0).abs() < 1e-9);
}

#[test]
fn well_guard_follows_its_stair_label() {
    let mut s = with_model(|m| {
        let st = m.push_stair(Stair::default());
        m.push_railing(Railing { follows: Some(st), ..Railing::default() });
    });
    let v = inspect(&mut s, json!({"railings": true}));
    assert_eq!(v["railings"][0]["host"], "well");
    assert_eq!(v["railings"][0]["follows"], "s0");
}

#[test]
fn deck_railing_follows_its_room_label() {
    let mut s = with_model(|m| {
        let rm = m.push_room(Room { room_type: RoomType::Deck, polygon: square(), ..Room::default() });
        m.push_railing(Railing { follows: Some(rm), host: RailingHost::Deck, auto: true, ..Railing::default() });
    });
    let v = inspect(&mut s, json!({"railings": true}));
    assert_eq!(v["railings"][0]["host"], "deck");
    assert_eq!(v["railings"][0]["follows"], "r0");
    assert_eq!(v["railings"][0]["auto"], json!(true));
}

#[test]
fn room_rows_report_boards_and_rails_for_decks_only() {
    let mut s = with_model(|m| {
        m.push_room(Room { room_type: RoomType::Living, polygon: square(), ..Room::default() });
        m.push_room(Room {
            room_type: RoomType::Deck,
            polygon: square(),
            deck_boards: Some(30.0),
            deck_rails: vec![RailSetting::Off, RailSetting::On],
            ..Room::default()
        });
    });
    let v = inspect(&mut s, json!({"rooms": true}));
    assert!(v["rooms"][0].get("boards").is_none() && v["rooms"][0].get("rails").is_none());
    assert_eq!(v["rooms"][1]["boards"], json!(30.0));
    assert_eq!(v["rooms"][1]["rails"], json!(["off", "on"]));
}

#[test]
fn opening_rows_report_top_grille_sides_and_glass() {
    let mut s = with_model(|m| {
        let w = m.push_wall(Wall { start: Vec2::new(10.0, 8.0), end: Vec2::new(0.0, 8.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        m.push_opening(Opening::new(w, 6.0, 0.9, 1.2, OpeningKind::Window));
        m.push_opening(Opening {
            sill: 0.9,
            top: OpeningTop::Round,
            grille: Some(Grille { across: 3, down: 2, top_only: true, ..Grille::default() }),
            sill_sides: OpeningSides::BOTH,
            ..Opening::new(w, 2.0, 0.9, 1.2, OpeningKind::Window)
        });
        m.push_opening(Opening::new(w, 8.0, 0.9, 2.0, OpeningKind::Door));
    });
    let v = inspect(&mut s, json!({"openings": true}));
    let o = &v["openings"][0];
    assert_eq!(o["top"], "square");
    assert_eq!(o["roofOffset"], json!(0.15));
    assert!(o["grille"].is_null());
    assert_eq!(o["sillSides"], "right");
    assert_eq!(o["casingSides"], "right");
    assert!(o["glassCentre"].is_array());
    assert_eq!(o["glassNormal"][1].as_f64(), Some(-1.0));
    let g = &v["openings"][1];
    assert_eq!(g["top"], "round");
    assert_eq!(g["grille"], json!({"across": 3, "down": 2, "part": "top"}));
    assert_eq!(g["sillSides"], "both");
    let door = &v["openings"][2];
    assert!(door["glassCentre"].is_null() && door["glassNormal"].is_null());
}

#[test]
fn decks_rows_stub_has_the_row_shape() {
    let mut s = with_model(|m| {
        m.push_room(Room { room_type: RoomType::Living, polygon: square(), ..Room::default() });
        m.push_room(Room { room_type: RoomType::Deck, polygon: square(), deck_height: Some(0.5), ..Room::default() });
    });
    assert!(inspect(&mut s, json!({"rooms": true})).get("decks").is_none());
    let v = inspect(&mut s, json!({"decks": true}));
    let rows = v["decks"].as_array().cloned().unwrap_or_default();
    assert_eq!(rows.len(), 1);
    let d = &rows[0];
    assert_eq!(d["id"], "r1");
    assert_eq!(d["top"], json!(0.5));
    assert_eq!(d["boardDirection"], json!(0.0));
    assert!(d["ledger"].is_null());
    for k in ["joists", "rims", "beams", "posts", "postGrades"] {
        assert_eq!(d[k], json!([]), "{k}");
    }
}
