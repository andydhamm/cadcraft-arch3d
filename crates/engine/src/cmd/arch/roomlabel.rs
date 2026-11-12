//! The `room.label` command: choose which area a room's plan label shows.
//!
//! JSON parameters: `room` (`"r3"`, a leading `r` optional) and `area` (`"interior"` or
//! `"standard"`), both required. Returns `{"room", "area"}`. A refused request changes nothing
//! and pushes no undo step. The 2D plan is rebuilt on every build, so no arch revision is needed.

use cadcraft_doc::arch::{ArchId, LabelArea};
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::{Result, Session};

/// Command specs for the room label module.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("room.label", "Room Label", run_room_label)
            .menu(&["Architecture", "Room Label"])
            .params("{room: \"r1\", area: \"interior\"|\"standard\"}"),
    ]
}

fn run_room_label(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "room.label";
    let raw = match p.get("room") {
        Some(Value::String(t)) => t.trim().to_string(),
        Some(_) => return Err(bad(CMD, "`room` must be a string like \"r1\"")),
        None => return Err(bad(CMD, "`room` (\"r1\") is required")),
    };
    let area = match p.get("area") {
        Some(Value::String(t)) => LabelArea::from_key(t.trim()).ok_or_else(|| bad(CMD, format!("unknown `area` `{t}` (interior or standard)")))?,
        Some(_) => return Err(bad(CMD, "`area` must be \"interior\" or \"standard\"")),
        None => return Err(bad(CMD, "`area` (\"interior\" or \"standard\") is required")),
    };
    let digits = raw.get(..1).filter(|pre| pre.eq_ignore_ascii_case("r")).and_then(|_| raw.get(1..)).unwrap_or(&raw);
    let id = digits.parse::<u32>().map(ArchId).map_err(|_| bad(CMD, format!("unknown room `{raw}`")))?;
    let index = s.doc()?.arch.rooms.iter().position(|r| r.id == id).ok_or_else(|| bad(CMD, format!("unknown room `{raw}`")))?;
    let d = s.doc_mut()?;
    let room = d.arch.rooms.get_mut(index).ok_or_else(|| bad(CMD, format!("unknown room `{raw}`")))?;
    room.label_area = area;
    Ok(json!({"room": format!("r{}", id.0), "area": area.key()}))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::arch::Room;
    use cadcraft_geom::Vec2;

    fn session() -> Session {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.push_room(Room { polygon: vec![Vec2::ZERO, Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)], ..Room::default() });
        s
    }

    fn undo_len(s: &Session) -> usize {
        s.state().map(|st| st.undo.len()).unwrap_or_default()
    }

    #[test]
    fn room_label_sets_area_kind_one_undo_step_no_arch_revision() {
        let mut s = session();
        let (depth, rev) = (undo_len(&s), s.doc().unwrap().arch_revision);
        let r = s.execute("room.label", &json!({"room": "r0", "area": "standard"})).unwrap();
        assert_eq!(r, json!({"room": "r0", "area": "standard"}));
        assert_eq!(s.doc().unwrap().arch.rooms[0].label_area, LabelArea::Standard);
        assert_eq!(undo_len(&s), depth + 1);
        assert_eq!(s.doc().unwrap().arch_revision, rev);
        s.execute("room.label", &json!({"room": "0", "area": "interior"})).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms[0].label_area, LabelArea::Interior);
    }

    #[test]
    fn refusals_change_nothing() {
        let mut s = session();
        let depth = undo_len(&s);
        let before = s.doc().unwrap().arch.rooms.clone();
        for p in [
            json!({}),
            json!({"room": "r0"}),
            json!({"area": "standard"}),
            json!({"room": "r9", "area": "standard"}),
            json!({"room": "r0", "area": "gross"}),
            json!({"room": 0, "area": "standard"}),
            json!({"room": "r0", "area": 1}),
            json!({"room": "rx", "area": "standard"}),
            json!({"room": null, "area": null}),
        ] {
            assert!(s.execute("room.label", &p).is_err(), "{p}");
        }
        assert_eq!(undo_len(&s), depth);
        assert_eq!(s.doc().unwrap().arch.rooms, before);
    }
}
