//! The `deck` command: create a deck room from a drawn polygon.
//!
//! JSON parameters:
//! - `points`: required, `[[x, y], ...]` — a closed ring (the last point repeats
//!   the first within 1e-6). After dropping the trailing duplicate, 4 or more
//!   distinct points must form a simple polygon with finite area at least
//!   `MIN_ROOM_AREA`.
//! - `height`: optional, metres above grade; must be finite with `0 < h < 10`.
//!   Absent means the default deck height (0.9 m).
//! - `name`: optional, the deck's name.
//! - `level`: optional, a level's name or its numeric index; absent means index 0.
//!   Unknown names/indices are refused (never a silent default).
//!
//! - `boards`: optional, a finite number of degrees with `|boards| <= 360`, stored
//!   normalised into `[0, 180)`; null is automatic (parallel to the ledger).
//! - `rails`: optional, `"auto" | "on" | "off"` for every edge, or an array with exactly one
//!   name per edge of the stored ring (edge i runs from vertex i to i+1, after the trailing
//!   duplicate is dropped and the ring made counter-clockwise). All "auto" is stored empty.
//!
//! Returns `{"id":"r0","handle":"...","deckHeight":h,"interiorArea":a,"boards":deg|null,
//! "rails":[names],"railings":[labels]}`.
//!
//! `deck.edit` (`{id, boards?, rails?, height?}`) changes those values on an existing deck.

use cadcraft_arch::poly::{ensure_ccw, is_simple, region_area, ring_area};
use cadcraft_arch::railing_auto::{RailHostRef, regenerate};
use cadcraft_doc::arch::{MAX_ROOMS, MIN_ROOM_AREA, RailSetting, RailingHost, Room, RoomType};
use cadcraft_doc::{ArchId, railing_label};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad, distinct_points, points_param};
use crate::prompt::{Accept, Input, Interactive, Prompt, Step};
use crate::{EngineError, Result, Session};

/// Default deck height above grade (metres).
const DEFAULT_DECK_HEIGHT: f64 = 0.9;

/// Maximum accepted deck height (metres).
const MAX_DECK_HEIGHT: f64 = 10.0;

/// Command specs for the deck module.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("deck", "Deck", run_deck)
            .menu(&["Architecture", "Deck"])
            .alias(&["dk"])
            .params("{points: closed ring [[x,y],...], height?, name?, level?, boards?: deg|null, rails?: auto|on|off|[one per edge]}")
            .interactive(|_| Ok(Box::new(DeckM::default()))),
        CommandSpec::new("deck.edit", "Edit Deck", run_deck_edit)
            .menu(&["Architecture", "Edit Deck"])
            .params("{id: \"r3\", boards?: deg|null, rails?, height?}"),
    ]
}

/// Parse `boards`: `Ok(None)` is automatic (null), `Ok(Some(deg))` is normalised into `[0, 180)`.
/// The value must be a finite number with `|boards| <= 360`.
fn parse_boards(cmd: &str, v: &Value) -> Result<Option<f64>> {
    if v.is_null() {
        return Ok(None);
    }
    match v.as_f64().filter(|b| b.is_finite() && b.abs() <= 360.0) {
        Some(b) => {
            let n = b.rem_euclid(180.0);
            Ok(Some(if n < 180.0 { n } else { 0.0 }))
        }
        None => Err(bad(cmd, "`boards` must be a number of degrees with |boards| <= 360, or null")),
    }
}

/// Parse `rails` for a ring of `edges` edges: one name for every edge, or an array with one name
/// per edge. A list that is all "auto" is stored empty.
fn parse_rails(cmd: &str, v: &Value, edges: usize) -> Result<Vec<RailSetting>> {
    let one = |v: &Value| -> Result<RailSetting> {
        v.as_str().and_then(RailSetting::parse).ok_or_else(|| bad(cmd, "`rails` names must be \"auto\", \"on\" or \"off\""))
    };
    let list: Vec<RailSetting> = match v {
        Value::String(_) => vec![one(v)?; edges],
        Value::Array(a) => {
            if a.len() != edges {
                return Err(bad(cmd, format!("`rails` needs exactly one name per edge ({edges}), got {}", a.len())));
            }
            a.iter().map(one).collect::<Result<Vec<_>>>()?
        }
        _ => return Err(bad(cmd, "`rails` must be \"auto\", \"on\", \"off\" or an array with one name per edge")),
    };
    Ok(if list.iter().all(|r| r.is_auto()) { Vec::new() } else { list })
}

/// Validate a deck height: finite and in `(0, MAX_DECK_HEIGHT)`.
fn parse_height(cmd: &str, v: &Value) -> Result<f64> {
    let h = v.as_f64().ok_or_else(|| bad(cmd, "`height` must be a number"))?;
    if !h.is_finite() || !(h > 0.0 && h < MAX_DECK_HEIGHT) {
        return Err(bad(cmd, format!("`height` {} must be finite and in (0, {})", h, MAX_DECK_HEIGHT)));
    }
    Ok(h)
}

/// The per-edge rail names of a deck room, "auto" for a missing entry.
fn rails_json(room: &Room) -> Value {
    let names: Vec<&str> = (0..room.polygon.len()).map(|i| room.deck_rails.get(i).copied().unwrap_or_default().name()).collect();
    json!(names)
}

/// Labels of the deck's automatic railings.
fn deck_rail_labels(arch: &cadcraft_doc::arch::ArchModel, id: ArchId) -> Vec<String> {
    arch.railings.iter().filter(|r| r.auto && r.follows == Some(id) && r.host == RailingHost::Deck).map(|r| railing_label(r.id)).collect()
}

/// Run `deck.edit`: change a deck's boards, rails or height.
fn run_deck_edit(s: &mut Session, p: &Value) -> Result<Value> {
    let err = || bad("deck.edit", "`id` must name a deck room, like \"r3\"");
    let id = match p.get("id") {
        Some(Value::String(t)) => {
            let t = t.trim().to_ascii_lowercase();
            t.strip_prefix('r').unwrap_or(&t).parse::<u32>().map(ArchId).map_err(|_| err())?
        }
        Some(Value::Number(n)) => n.as_u64().and_then(|n| u32::try_from(n).ok()).map(ArchId).ok_or_else(err)?,
        _ => return Err(err()),
    };
    let d = s.doc_mut()?;
    let pos = d.arch.room_index(id).ok_or_else(|| bad("deck.edit", format!("unknown room `r{}`", id.0)))?;
    let edges = {
        let room = d.arch.rooms.get(pos).ok_or_else(err)?;
        if room.room_type != RoomType::Deck {
            return Err(bad("deck.edit", format!("room `r{}` is not a deck", id.0)));
        }
        room.polygon.len()
    };
    // Validate everything before writing.
    let boards = p.get("boards").map(|v| parse_boards("deck.edit", v)).transpose()?;
    let rails = p.get("rails").filter(|v| !v.is_null()).map(|v| parse_rails("deck.edit", v, edges)).transpose()?;
    let height = p.get("height").filter(|v| !v.is_null()).map(|v| parse_height("deck.edit", v)).transpose()?;
    if boards.is_none() && rails.is_none() && height.is_none() {
        return Err(bad("deck.edit", "nothing to change: give boards, rails or height"));
    }
    if let Some(room) = d.arch.rooms.get_mut(pos) {
        if let Some(b) = boards {
            room.deck_boards = b;
        }
        if let Some(r) = &rails {
            room.deck_rails = r.clone();
        }
        if let Some(h) = height {
            room.deck_height = Some(h);
        }
    }
    if rails.is_some() {
        let made = regenerate(&mut d.arch, RailHostRef::Deck(id));
        if !made.is_empty() {
            d.ensure_layer("A-RAIL");
        }
    }
    cadcraft_arch::area::refresh_areas(&mut d.arch);
    d.touch_arch();
    let room = d.arch.rooms.get(pos).ok_or_else(err)?;
    Ok(json!({
        "deck": format!("r{}", id.0),
        "deckHeight": room.deck_height,
        "boards": room.deck_boards,
        "rails": rails_json(room),
        "railings": deck_rail_labels(&d.arch, id),
    }))
}

/// Resolve a level reference (a level name or its numeric index) to a position in
/// the arch model's levels. Mirrors `cmd/arch/room.rs`'s `find_room_level` but
/// reads a plain `&str`. An unknown name/index is an error, never a silent
/// fallback to level 0.
fn find_level(arch: &cadcraft_doc::arch::ArchModel, ref_val: &str) -> Result<usize> {
    if let Some(i) = ref_val.trim().parse::<u64>().ok() {
        if i < arch.levels.len() as u64 {
            return Ok(i as usize);
        }
    }
    if let Some((i, l)) = arch.levels.iter().enumerate().find(|(_, l)| l.name == ref_val.trim()) {
        return Ok(i);
    }
    Err(EngineError::Other(format!("deck: unknown level `{ref_val}`")))
}

/// Run the `deck` command from JSON parameters. Never opens a dialog.
fn run_deck(s: &mut Session, p: &Value) -> Result<Value> {
    let d = s.doc_mut()?;
    let arch = &d.arch;

    // --- Points: required, distinct, explicitly closed, >= 4, finite, simple,
    // CCW-normalised, finite area above MIN_ROOM_AREA ---
    let pts = points_param(p, "points").ok_or_else(|| bad("deck", "`points` [[x,y],...] (closed ring, 4 or more) is required"))?;
    let pts = distinct_points(&pts);
    // The ring must be explicitly closed: the last point repeats the first.
    if pts.len() < 2 || !pts[pts.len() - 1].near(pts[0], 1e-6) {
        return Err(bad("deck", "points must be a closed ring: repeat the first point at the end"));
    }
    // Drop the trailing duplicate.
    let ring: Vec<Vec2> = pts[..pts.len() - 1].to_vec();
    if ring.len() < 4 {
        return Err(bad("deck", "`points` [[x,y],...] must have 4 or more distinct points"));
    }
    for pt in &ring {
        if !pt.is_finite() {
            return Err(bad("deck", "non-finite coordinate in `points`"));
        }
    }
    if !is_simple(&ring) {
        return Err(bad("deck", "`points` do not form a simple polygon (self-intersecting or zero area)"));
    }
    let ring = ensure_ccw(&ring);
    // Reject non-finite or too-small areas (e.g. 1e308 corners).
    let area = ring_area(&ring);
    if !area.is_finite() || area < MIN_ROOM_AREA {
        return Err(bad("deck", "`points` do not enclose a valid deck area"));
    }

    // --- Height: optional, finite, 0 < h < 10 (default 0.9 m) ---
    let height = match p.get("height") {
        None | Some(Value::Null) => DEFAULT_DECK_HEIGHT,
        Some(v) => parse_height("deck", v)?,
    };

    // --- Boards and rails (phase 17), validated against the stored ring ---
    let boards = match p.get("boards") {
        None => None,
        Some(v) => parse_boards("deck", v)?,
    };
    let rails = match p.get("rails").filter(|v| !v.is_null()) {
        None => Vec::new(),
        Some(v) => parse_rails("deck", v, ring.len())?,
    };

    // --- Level: name or index (string or number), else 0; unknown is an error ---
    let level = match p.get("level") {
        None => 0,
        Some(Value::Null) => 0,
        Some(Value::String(n)) => find_level(arch, n)?,
        Some(Value::Number(n)) => {
            let i = n.as_u64().unwrap_or(0);
            if i < arch.levels.len() as u64 {
                i as usize
            } else {
                return Err(bad("deck", format!("`level` index {} out of range", i)));
            }
        }
        Some(other) => return Err(bad("deck", format!("`level` must be a string or number, got `{other}`"))),
    };

    // --- Name ---
    let name = p.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default();

    // --- Areas ---
    let interior_area = region_area(&ring, &[]);

    // --- Cap, then build and push (all checks above ran before touching the model) ---
    if d.arch.rooms.len() >= MAX_ROOMS {
        return Err(bad("deck", "room limit reached"));
    }
    // The standard area is measured against the surrounding walls, so the room must
    // first sit in the model. `standard_area` only reads `arch`; that immutable
    // borrow ends before `push_room` mutates `d`.
    let mut room = Room {
        name,
        room_type: RoomType::Deck,
        level: level as u32,
        polygon: ring,
        deck_height: Some(height),
        deck_boards: boards,
        deck_rails: rails,
        ..Room::default()
    };
    let standard_area = cadcraft_arch::area::standard_area(arch, &room);
    room.standard_area = standard_area;
    let id = d.arch.push_room(room);
    let made = regenerate(&mut d.arch, RailHostRef::Deck(id));
    if !made.is_empty() {
        d.ensure_layer("A-RAIL");
    }
    cadcraft_arch::area::refresh_areas(&mut d.arch);
    d.touch_arch();

    let rails_reply = d.arch.room(id).map(rails_json).unwrap_or_else(|| json!([]));
    Ok(json!({
        "id": format!("r{}", id.0),
        "handle": cadcraft_doc::arch::handle_json(cadcraft_arch::room_handle(id)),
        "deckHeight": height,
        "interiorArea": interior_area,
        "boards": boards,
        "rails": rails_reply,
        "railings": made.iter().map(|r| railing_label(*r)).collect::<Vec<_>>(),
    }))
}

/// Interactive `deck` machine: collect points for a deck polygon, then finish with
/// the `Close` keyword or an empty Enter. The machine delegates to the JSON `deck`
/// command on finish so there is exactly one implementation path and one undo step.
pub struct DeckM {
    pts: Vec<Vec2>,
    base: Option<Vec2>,
}

impl Default for DeckM {
    fn default() -> Self {
        Self { pts: Vec::new(), base: None }
    }
}

impl Interactive for DeckM {
    fn name(&self) -> &'static str {
        "deck"
    }

    fn prompt(&self, _s: &Session) -> Prompt {
        let message = if self.pts.is_empty() { "Specify start point:".to_string() } else { "Specify next point or [Close]:".to_string() };
        let keywords = if self.pts.is_empty() { Vec::new() } else { vec!["Close".to_string()] };
        Prompt {
            message,
            keywords,
            default: None,
            accept: Accept::POINT,
            base: self.base,
            corner: false,
            grip: None,
            picker: None,
            distance: false,
            deferred: false,
            raw: false,
            picks_one: false,
        }
    }

    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let closes = match i {
            Input::Enter => true,
            Input::Keyword(_) => true,
            Input::Text(ref txt) => {
                let l = txt.trim().to_ascii_lowercase();
                l.is_empty() || l == "close"
            }
            _ => false,
        };
        if closes {
            // Fewer than 3 points: re-prompt instead of erroring (never a model change).
            if self.pts.len() < 3 {
                return Ok(Step::Continue);
            }
            let params = self.build_params();
            return s.execute("deck", &params).map(|_| Step::Done);
        }
        match i {
            Input::Point(pt) => {
                if !pt.is_finite() {
                    return Ok(Step::Continue);
                }
                self.pts.push(pt);
                self.base = Some(pt);
                Ok(Step::Continue)
            }
            Input::Cancel => Ok(Step::Cancel),
            _ => Ok(Step::Continue),
        }
    }
}

impl DeckM {
    /// Build the JSON parameter object the JSON `deck` command accepts. Only the
    /// fields the interactive form set (points) are emitted.
    fn build_params(&self) -> Value {
        let points: Vec<Value> = self.pts.iter().map(|p| json!([p.x, p.y])).collect();
        json!({ "points": points })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use cadcraft_doc::arch::{RailSetting, RoomType};
    use serde_json::json;

    fn new_session() -> Session {
        Session::new()
    }

    /// A 3 x 3 m closed square (area 9.0).
    fn square_points() -> Value {
        json!([[0.0, 0.0], [3.0, 0.0], [3.0, 3.0], [0.0, 3.0], [0.0, 0.0]])
    }

    #[test]
    fn deck_is_a_deck_room_with_its_own_height() {
        let mut s = new_session();
        let r = s
            .execute(
                "deck",
                &json!({
                    "points": square_points(),
                    "height": 0.9,
                    "name": "Back Deck",
                }),
            )
            .unwrap();
        let d = s.doc().unwrap();
        assert_eq!(d.arch.rooms.len(), 1);
        assert_eq!(d.arch.rooms[0].room_type, RoomType::Deck);
        assert_eq!(d.arch.rooms[0].deck_height, Some(0.9));
        assert_eq!(d.arch.rooms[0].name, "Back Deck");
        assert_eq!(r["id"].as_str().unwrap(), "r0");
        assert_eq!(r["deckHeight"].as_f64().unwrap(), 0.9);
        assert_eq!(r["interiorArea"].as_f64().unwrap(), 9.0);
    }

    #[test]
    fn deck_refuses_bad_height_and_open_ring() {
        let mut s = new_session();

        // Bad height: 0 (must be > 0).
        let r = s.execute("deck", &json!({ "points": square_points(), "height": 0.0 }));
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);

        // Bad height: negative.
        let r = s.execute("deck", &json!({ "points": square_points(), "height": -1.0 }));
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);

        // Bad height: too large (>= 10).
        let r = s.execute("deck", &json!({ "points": square_points(), "height": 11.0 }));
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);

        // Open ring (last point does not repeat the first).
        let r = s.execute(
            "deck",
            &json!({
                "points": [[0.0, 0.0], [3.0, 0.0], [3.0, 3.0], [0.0, 3.0]],
            }),
        );
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    #[test]
    fn deck_accepts_numeric_level() {
        let mut s = new_session();
        let r = s
            .execute(
                "deck",
                &json!({
                    "points": square_points(),
                    "level": 0,
                }),
            )
            .unwrap();
        assert_eq!(r["id"].as_str().unwrap(), "r0");
        assert_eq!(s.doc().unwrap().arch.rooms[0].level, 0);
    }

    #[test]
    fn deck_refuses_nan_in_points() {
        let mut s = new_session();
        let r = s.execute(
            "deck",
            &json!({
                "points": [[0.0, 0.0], [3.0, 0.0], [3.0, f64::NAN], [0.0, 3.0], [0.0, 0.0]],
            }),
        );
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    #[test]
    fn deck_refuses_self_intersecting_polygon() {
        let mut s = new_session();
        // Bowtie: (0,0)-(3,3)-(0,3)-(3,0)-(0,0)
        let r = s.execute(
            "deck",
            &json!({
                "points": [[0.0, 0.0], [3.0, 3.0], [0.0, 3.0], [3.0, 0.0], [0.0, 0.0]],
            }),
        );
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    #[test]
    fn deck_refuses_area_below_min_room_area() {
        let mut s = new_session();
        // 0.1 x 0.1 m square = 0.01 m² < MIN_ROOM_AREA (0.05)
        let r = s.execute(
            "deck",
            &json!({
                "points": [[0.0, 0.0], [0.1, 0.0], [0.1, 0.1], [0.0, 0.1], [0.0, 0.0]],
            }),
        );
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    fn with(extra: Value) -> Value {
        let mut m = json!({ "points": square_points() });
        if let (Some(a), Some(b)) = (m.as_object_mut(), extra.as_object()) {
            for (k, v) in b {
                a.insert(k.clone(), v.clone());
            }
        }
        m
    }

    #[test]
    fn deck_accepts_boards_and_rails() {
        let mut s = new_session();
        let r = s.execute("deck", &with(json!({"boards": 120, "rails": ["auto", "on", "off", "auto"]}))).unwrap();
        assert_eq!(r["boards"], json!(120.0));
        assert_eq!(r["rails"], json!(["auto", "on", "off", "auto"]));
        assert!(r["railings"].is_array());
        let room = &s.doc().unwrap().arch.rooms[0];
        assert_eq!(room.deck_boards, Some(120.0));
        assert_eq!(room.deck_rails, vec![RailSetting::Auto, RailSetting::On, RailSetting::Off, RailSetting::Auto]);
        // Normalised into [0, 180); a single name covers every edge; all-auto is stored empty.
        let r = s.execute("deck", &with(json!({"boards": -90, "rails": "off"}))).unwrap();
        assert_eq!(r["boards"], json!(90.0));
        assert_eq!(r["rails"], json!(["off", "off", "off", "off"]));
        s.execute("deck", &with(json!({"boards": 360, "rails": ["auto", "auto", "auto", "auto"]}))).unwrap();
        let room = &s.doc().unwrap().arch.rooms[2];
        assert_eq!(room.deck_boards, Some(0.0));
        assert!(room.deck_rails.is_empty());
        s.execute("deck", &with(json!({"boards": null}))).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms[3].deck_boards, None);
    }

    #[test]
    fn deck_refuses_boards_1e308_and_400() {
        for b in [json!(1e308), json!(400), json!(-360.5), json!("45"), json!(true)] {
            let mut s = new_session();
            assert!(s.execute("deck", &with(json!({ "boards": b }))).is_err(), "{b}");
            assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
        }
    }

    #[test]
    fn deck_refuses_rails_of_the_wrong_length_and_bad_names() {
        for rails in
            [json!(["on", "off"]), json!(["on", "on", "on", "on", "on"]), json!([]), json!("sometimes"), json!(["on", 1, "on", "on"]), json!(5)]
        {
            let mut s = new_session();
            assert!(s.execute("deck", &with(json!({ "rails": rails }))).is_err(), "{rails}");
            assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
        }
    }

    #[test]
    fn deck_edit_boards_null_clears() {
        let mut s = new_session();
        s.execute("deck", &with(json!({"boards": 30}))).unwrap();
        let r = s.execute("deck.edit", &json!({"id": "r0", "boards": 200})).unwrap();
        assert_eq!(r["boards"], json!(20.0));
        assert_eq!(r["deck"], json!("r0"));
        let r = s.execute("deck.edit", &json!({"id": "r0", "boards": null})).unwrap();
        assert!(r["boards"].is_null());
        assert_eq!(s.doc().unwrap().arch.rooms[0].deck_boards, None);
        let r = s.execute("deck.edit", &json!({"id": "r0", "rails": "on"})).unwrap();
        assert_eq!(r["rails"], json!(["on", "on", "on", "on"]));
        assert!(s.execute("deck.edit", &json!({"id": "r0"})).is_err());
        assert!(s.execute("deck.edit", &json!({"id": "r0", "boards": 1e308})).is_err());
    }

    #[test]
    fn deck_edit_height_is_validated() {
        let mut s = new_session();
        s.execute("deck", &with(json!({}))).unwrap();
        for h in [json!(0), json!(-1), json!(10), json!(1e308), json!("1")] {
            assert!(s.execute("deck.edit", &json!({"id": "r0", "height": h})).is_err(), "{h}");
        }
        assert_eq!(s.doc().unwrap().arch.rooms[0].deck_height, Some(0.9));
        let r = s.execute("deck.edit", &json!({"id": "r0", "height": 1.2})).unwrap();
        assert_eq!(r["deckHeight"], json!(1.2));
    }

    #[test]
    fn deck_edit_refuses_a_room_that_is_not_a_deck() {
        let mut s = new_session();
        s.execute("room", &json!({"points": [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0], [0.0, 0.0]]})).unwrap();
        let before = s.doc().unwrap().arch.rooms.clone();
        assert!(!before.is_empty());
        assert!(s.execute("deck.edit", &json!({"id": "r0", "boards": 10})).is_err());
        assert!(s.execute("deck.edit", &json!({"id": "r7", "boards": 10})).is_err());
        assert_eq!(s.doc().unwrap().arch.rooms, before);
    }

    #[test]
    fn deck_edit_is_one_undo_step() {
        let mut s = new_session();
        s.execute("deck", &with(json!({"boards": 30}))).unwrap();
        s.execute("deck.edit", &json!({"id": "r0", "boards": 60, "rails": "off", "height": 1.5})).unwrap();
        assert_eq!(s.undo().unwrap(), Some("Edit Deck".to_string()));
        let room = &s.doc().unwrap().arch.rooms[0];
        assert_eq!((room.deck_boards, room.deck_height), (Some(30.0), Some(0.9)));
        assert!(room.deck_rails.is_empty());
    }
}
