//! The `material` command: assign a finish to walls, roofs or rooms.
//!
//! JSON parameters (exactly one target key, plus `finish`):
//! - `walls`: a list of wall ids, `"all"`, or `"exterior"` (exterior, non-divider walls).
//! - `roofs`: a list of roof ids or `"all"`.
//! - `rooms`: a list of room ids or `"all"`.
//! - `finish`: `siding|shingles|brick|wood|none` (`none` clears the finish). The finish
//!   must suit the target (walls, roofs and floors accept different sets).
//!
//! Everything is validated before the model is written. Returns
//! `{"target": "walls", "finish": "brick" | null, "ids": [...]}`.

use std::str::FromStr;

use cadcraft_doc::arch::{ArchId, Finish, roof_label, room_label, wall_label};
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad, str_param};
use crate::{Result, Session};

/// Command specs for the material module.
pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec::new("material", "Material", run_material)
        .menu(&["Architecture", "Material"])
        .params("{walls: [\"w0\",...] | \"exterior\" | \"all\", roofs: [\"r0\",...] | \"all\", rooms: [\"r1\",...] | \"all\"} + finish: \"siding\"|\"shingles\"|\"brick\"|\"wood\"|\"none\"")]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Walls,
    Roofs,
    Rooms,
}

impl Target {
    /// The JSON key and the plural noun.
    fn plural(self) -> &'static str {
        match self {
            Target::Walls => "walls",
            Target::Roofs => "roofs",
            Target::Rooms => "rooms",
        }
    }

    /// "wall" | "roof" | "floor" — the noun used in the not-allowed message.
    fn noun(self) -> &'static str {
        match self {
            Target::Walls => "wall",
            Target::Roofs => "roof",
            Target::Rooms => "floor",
        }
    }

    fn prefix(self) -> char {
        if self == Target::Walls { 'w' } else { 'r' }
    }

    fn accepts(self, f: Finish) -> bool {
        match self {
            Target::Walls => f.for_walls(),
            Target::Roofs => f.for_roofs(),
            Target::Rooms => f.for_floors(),
        }
    }

    /// The finish names this target accepts, in `Finish::ALL` order.
    fn allowed(self) -> Vec<&'static str> {
        Finish::ALL.iter().copied().filter(|f| self.accepts(*f)).map(Finish::name).collect()
    }
}

/// Parse an id: a plain integer, the integer with the target's letter prefix
/// (case-insensitive), or a finite non-negative integer number.
fn parse_arch_id(prefix: char, v: &Value) -> Option<ArchId> {
    // Only strings: a bare JSON number is not an id (the segment's spec; phase-13/11 sends `[1, 2, 3]` as garbage).
    let n: u64 = match v {
        Value::String(s) => {
            let s = s.trim();
            let digits = match s.chars().next() {
                Some(c) if c.to_ascii_lowercase() == prefix => s.get(c.len_utf8()..)?,
                _ => s,
            };
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            u64::from_str(digits).ok()?
        }
        _ => return None,
    };
    u32::try_from(n).ok().map(ArchId)
}

fn run_material(s: &mut Session, p: &Value) -> Result<Value> {
    // 1. Target.
    let present: Vec<&str> = ["walls", "roofs", "rooms"].into_iter().filter(|k| p.get(*k).is_some_and(|v| !v.is_null())).collect();
    let target = match present.as_slice() {
        ["walls"] => Target::Walls,
        ["roofs"] => Target::Roofs,
        ["rooms"] => Target::Rooms,
        _ => return Err(bad("material", "give exactly one of `walls`, `roofs`, `rooms`")),
    };
    let key = target.plural();

    // 2. Finish.
    let Some(raw) = str_param(p, "finish") else {
        return Err(bad("material", "`finish` is required (siding, shingles, brick, wood, none)"));
    };
    let set: Option<Finish> = if raw == "none" {
        None
    } else {
        match Finish::parse(raw) {
            Some(f) => Some(f),
            None => {
                return Err(bad("material", format!("unknown finish `{raw}` (siding, shingles, brick, wood, none)")));
            }
        }
    };

    // 3. Target/finish match.
    if let Some(f) = set {
        if !target.accepts(f) {
            return Err(bad("material", format!("{} is not a {} finish ({})", f.name(), target.noun(), target.allowed().join(", "))));
        }
    }

    // 4. Resolve ids (read-only).
    let Some(sel) = p.get(key) else {
        return Err(bad("material", "give exactly one of `walls`, `roofs`, `rooms`"));
    };
    let mut resolved: Vec<ArchId> = Vec::new();
    {
        let d = s.doc()?;
        let a = &d.arch;
        match sel {
            Value::Array(list) => {
                if list.is_empty() {
                    return Err(bad("material", "the id list is empty"));
                }
                for v in list {
                    let Some(id) = parse_arch_id(target.prefix(), v) else {
                        return Err(bad("material", format!("malformed {} id `{}`", target.noun(), v)));
                    };
                    let (exists, label) = match target {
                        Target::Walls => (a.walls.iter().any(|w| w.id == id), wall_label(id)),
                        Target::Roofs => (a.roofs.iter().any(|r| r.id == id), roof_label(id)),
                        Target::Rooms => (a.rooms.iter().any(|r| r.id == id), room_label(id)),
                    };
                    if !exists {
                        return Err(bad("material", format!("unknown {} `{label}`", target.noun())));
                    }
                    resolved.push(id);
                }
                resolved.sort_by_key(|i| i.0);
                resolved.dedup();
            }
            Value::String(name) => {
                match (name.as_str(), target) {
                    ("all", Target::Walls) => resolved.extend(a.walls.iter().map(|w| w.id)),
                    ("all", Target::Roofs) => resolved.extend(a.roofs.iter().map(|r| r.id)),
                    ("all", Target::Rooms) => resolved.extend(a.rooms.iter().map(|r| r.id)),
                    ("exterior", Target::Walls) => resolved.extend(a.walls.iter().filter(|w| w.exterior && !w.divider).map(|w| w.id)),
                    _ => {
                        return Err(bad(
                            "material",
                            format!(
                                "unknown selector `{name}` (a list of ids, or `all`{})",
                                if target == Target::Walls { ", or `exterior`" } else { "" }
                            ),
                        ));
                    }
                }
                resolved.sort_by_key(|i| i.0);
                resolved.dedup();
                if resolved.is_empty() {
                    return Err(bad("material", format!("no {} in the drawing", target.plural())));
                }
            }
            _ => return Err(bad("material", format!("`{key}` must be a list of ids or `all`"))),
        }
    }

    // 5. Write.
    let d = s.doc_mut()?;
    let mut ids: Vec<String> = Vec::with_capacity(resolved.len());
    for id in &resolved {
        match target {
            Target::Walls => {
                if let Some(w) = d.arch.walls.iter_mut().find(|w| w.id == *id) {
                    w.finish = set;
                    ids.push(wall_label(*id));
                }
            }
            Target::Roofs => {
                if let Some(r) = d.arch.roofs.iter_mut().find(|r| r.id == *id) {
                    r.covering = set;
                    ids.push(roof_label(*id));
                }
            }
            Target::Rooms => {
                if let Some(r) = d.arch.rooms.iter_mut().find(|r| r.id == *id) {
                    r.floor_finish = set;
                    ids.push(room_label(*id));
                }
            }
        }
    }
    d.touch_arch();

    // 6. Reply.
    Ok(json!({ "target": key, "finish": set.map(Finish::name), "ids": ids }))
}

#[cfg(test)]
mod tests {
    use cadcraft_doc::arch::{Finish, Roof, Room, wall_label};
    use serde_json::{Value, json};

    use crate::Session;

    fn house() -> Session {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0.0,0.0],[10.0,0.0],[10.0,8.0],[0.0,8.0]], "closed": true, "thickness": 0.2, "exterior": true}))
            .unwrap();
        s.execute("wall", &json!({"points": [[0.0,3.0],[10.0,3.0]], "thickness": 0.1})).unwrap();
        s
    }

    fn finishes(s: &Session) -> Vec<Option<Finish>> {
        s.doc().unwrap().arch.walls.iter().map(|w| w.finish).collect()
    }

    /// Ids are strings (`"w1"` or `"1"`); a bare JSON number is hostile input and is refused, never read as an id
    /// (phase-13/11_hostile.py: `{"walls": [1, 2, 3]}` finished w1..w3).
    #[test]
    fn material_refuses_numbers_for_ids() {
        let mut s = house();
        let e = s.execute("material", &json!({"walls": [1, 2, 3], "finish": "brick"})).unwrap_err();
        assert!(e.to_string().contains("malformed wall id"), "{e}");
        assert!(finishes(&s).iter().all(Option::is_none), "{:?}", finishes(&s));
        let r = s.execute("material", &json!({"walls": ["w1", "2"], "finish": "brick"})).unwrap();
        assert_eq!(r["ids"], json!(["w1", "w2"]));
    }

    #[test]
    fn material_sets_the_exterior_walls_finish() {
        let mut s = house();
        let r = s.execute("material", &json!({"walls": "exterior", "finish": "brick"})).unwrap();
        assert_eq!(r["target"], "walls");
        assert_eq!(r["finish"], "brick");
        let want: Vec<String> = s.doc().unwrap().arch.walls.iter().filter(|w| w.exterior && !w.divider).map(|w| wall_label(w.id)).collect();
        assert_eq!(want.len(), 4);
        assert_eq!(r["ids"], json!(want));
        for w in &s.doc().unwrap().arch.walls {
            assert_eq!(w.finish, if w.exterior { Some(Finish::Brick) } else { None });
        }
    }

    #[test]
    fn material_refuses_a_finish_the_target_cannot_take() {
        let mut s = house();
        s.doc_mut().unwrap().arch.push_roof(Roof::default());
        let e = s.execute("material", &json!({"roofs": "all", "finish": "brick"})).unwrap_err().to_string();
        assert!(e.contains("brick") && e.contains("shingles") && e.contains("wood"), "{e}");
        assert!(s.doc().unwrap().arch.roofs.iter().all(|r| r.covering.is_none()));
    }

    #[test]
    fn material_refuses_an_unknown_id_before_writing() {
        let mut s = house();
        let rev = s.doc().unwrap().arch_revision;
        for w in [json!(["w99"]), json!(["nope"]), json!([])] {
            assert!(s.execute("material", &json!({"walls": w, "finish": "siding"})).is_err());
        }
        assert!(finishes(&s).iter().all(Option::is_none));
        assert_eq!(s.doc().unwrap().arch_revision, rev);
    }

    #[test]
    fn material_none_clears_a_finish() {
        let mut s = house();
        s.execute("material", &json!({"walls": "exterior", "finish": "brick"})).unwrap();
        let r = s.execute("material", &json!({"walls": "exterior", "finish": "none"})).unwrap();
        assert_eq!(r["finish"], Value::Null);
        assert!(finishes(&s).iter().all(Option::is_none));
    }

    #[test]
    fn material_is_undoable() {
        let mut s = house();
        let rev = s.doc().unwrap().arch_revision;
        s.execute("material", &json!({"walls": "exterior", "finish": "siding"})).unwrap();
        assert_eq!(s.doc().unwrap().arch_revision, rev + 1);
        assert!(finishes(&s).iter().any(Option::is_some));
        assert_eq!(s.undo().unwrap(), Some("Material".to_string()));
        assert!(finishes(&s).iter().all(Option::is_none));
        assert_eq!(s.doc().unwrap().arch_revision, rev);
    }

    #[test]
    fn material_never_panics_on_garbage() {
        for p in [
            json!({}),
            json!({"walls": "all"}),
            json!({"walls": 5, "finish": "brick"}),
            json!({"walls": [[1, 2]], "finish": "brick"}),
            json!({"walls": ["w0"], "roofs": ["r0"], "finish": "brick"}),
            json!({"walls": "all", "finish": null}),
            json!({"walls": "all", "finish": "BRICK"}),
        ] {
            let mut s = Session::new();
            assert!(s.execute("material", &p).is_err(), "{p}");
        }
    }

    /// The wall prefix is case-insensitive and a plain integer without the prefix is accepted.
    #[test]
    fn material_accepts_case_insensitive_prefix() {
        let mut s = house();
        let r = s.execute("material", &json!({"walls": ["W1", "2"], "finish": "siding"})).unwrap();
        assert_eq!(r["ids"], json!(["w1", "w2"]));
        // A prefix with no digits is malformed.
        let e = s.execute("material", &json!({"walls": ["W"], "finish": "siding"})).unwrap_err();
        assert!(e.to_string().contains("malformed"), "{e}");
    }

    /// Duplicate ids are deduped and the reply lists ids in ascending order.
    #[test]
    fn material_dedupes_and_sorts_ids() {
        let mut s = house();
        let r = s.execute("material", &json!({"walls": ["w3", "w1", "w1", "w2"], "finish": "siding"})).unwrap();
        assert_eq!(r["ids"], json!(["w1", "w2", "w3"]));
    }

    /// The roofs target sets `covering` on each roof.
    #[test]
    fn material_sets_roof_covering() {
        let mut s = Session::new();
        s.doc_mut().unwrap().arch.push_roof(Roof::default());
        s.doc_mut().unwrap().arch.push_roof(Roof::default());
        let r = s.execute("material", &json!({"roofs": "all", "finish": "shingles"})).unwrap();
        assert_eq!(r["target"], "roofs");
        assert_eq!(r["finish"], "shingles");
        assert_eq!(r["ids"], json!(["r0", "r1"]));
        for roof in &s.doc().unwrap().arch.roofs {
            assert_eq!(roof.covering, Some(Finish::Shingles));
        }
    }

    /// The rooms target sets `floor_finish` on each room.
    #[test]
    fn material_sets_room_floor_finish() {
        let mut s = Session::new();
        s.doc_mut().unwrap().arch.push_room(Room::default());
        let r = s.execute("material", &json!({"rooms": "all", "finish": "wood"})).unwrap();
        assert_eq!(r["target"], "rooms");
        assert_eq!(r["finish"], "wood");
        assert_eq!(r["ids"], json!(["r0"]));
        assert_eq!(s.doc().unwrap().arch.rooms[0].floor_finish, Some(Finish::Wood));
    }

    /// The `exterior` selector is only valid for walls.
    #[test]
    fn material_exterior_only_for_walls() {
        let mut s = Session::new();
        s.doc_mut().unwrap().arch.push_roof(Roof::default());
        let e = s.execute("material", &json!({"roofs": "exterior", "finish": "shingles"})).unwrap_err();
        assert!(e.to_string().contains("unknown selector"), "{e}");
    }

    /// An unknown selector string is refused with a hint about the valid selectors.
    #[test]
    fn material_unknown_selector() {
        let mut s = house();
        let e = s.execute("material", &json!({"walls": "interior", "finish": "brick"})).unwrap_err();
        let msg = e.to_string();
        assert!(msg.contains("unknown selector"), "{msg}");
        assert!(msg.contains("exterior"), "{msg}");
    }

    /// Malformed ids are refused with a message naming the element.
    #[test]
    fn material_refuses_malformed_ids() {
        let mut s = house();
        for id in [json!("w-1"), json!("w1.5"), json!("w"), json!(""), json!("w ")] {
            let e = s.execute("material", &json!({"walls": [id], "finish": "siding"})).unwrap_err();
            assert!(e.to_string().contains("malformed"), "{e}");
        }
    }

    /// An id larger than u32::MAX is refused.
    #[test]
    fn material_huge_id_is_refused() {
        let mut s = house();
        let e = s.execute("material", &json!({"walls": ["w4294967296"], "finish": "siding"})).unwrap_err();
        assert!(e.to_string().contains("malformed"), "{e}");
    }
}
