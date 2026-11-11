//! Room moldings command and helpers.

use crate::cmd::{CommandSpec, Session, bad, f64_req, str_param};
use crate::{EngineError, Result};
use cadcraft_doc::arch::{ArchId, ArchModel, MoldingKind, MoldingProfile, Room, RoomMolding};
use serde_json::{Map as JsonMap, Value, json};

/// Maximum moldings per room.
const MAX_ROOM_MOLDINGS: usize = 16;

/// Command specs for room moldings.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("room.moldings", "Room Moldings", run_room_moldings)
            .menu(&["Architecture", "Room Moldings"])
            .params("{room: id|ids|all, moldings: [{kind, profile?, size?, offset?}, ...]}"),
    ]
}

/// Resolve a molding kind from a string.
fn parse_kind(s: &str) -> Option<MoldingKind> {
    match s {
        "base" => Some(MoldingKind::Base),
        "chair" => Some(MoldingKind::Chair),
        "crown" => Some(MoldingKind::Crown),
        _ => None,
    }
}

/// Default offset for a molding kind.
fn default_offset(kind: MoldingKind) -> f64 {
    match kind {
        MoldingKind::Base => 0.0,
        MoldingKind::Chair => 0.9,
        MoldingKind::Crown => 0.0,
    }
}

/// Resolve a room label to an ArchId.
fn room_id_from_label(model: &ArchModel, label: &str) -> Option<ArchId> {
    model.rooms.iter().find(|r| cadcraft_doc::arch::room_label(r.id) == label).map(|r| r.id)
}

/// Run the room.moldings command.
fn run_room_moldings(s: &mut Session, p: &Value) -> Result<Value> {
    let room_param = p.get("room").ok_or_else(|| bad("room.moldings", "missing `room`"))?;
    let moldings_param = p.get("moldings").ok_or_else(|| bad("room.moldings", "missing `moldings`"))?;

    let moldings_arr = moldings_param.as_array().ok_or_else(|| bad("room.moldings", "`moldings` must be an array"))?;
    if moldings_arr.len() > MAX_ROOM_MOLDINGS {
        return Err(bad("room.moldings", format!("too many moldings (max {MAX_ROOM_MOLDINGS})")));
    }

    // Parse new moldings list
    let mut new_moldings = Vec::new();
    for m in moldings_arr {
        let kind_str = m.get("kind").and_then(Value::as_str).ok_or_else(|| bad("room.moldings", "missing `kind`"))?;
        let kind = parse_kind(kind_str).ok_or_else(|| bad("room.moldings", format!("unknown kind `{kind_str}`")))?;

        let offset = m.get("offset").and_then(Value::as_f64).unwrap_or(default_offset(kind));
        if !offset.is_finite() || offset < 0.0 || offset > 3.0 {
            return Err(bad("room.moldings", "`offset` out of range [0, 3]"));
        }

        let profile = if let Some(size) = m.get("size").and_then(Value::as_array) {
            if size.len() != 2 {
                return Err(bad("room.moldings", "`size` must be [w, h]"));
            }
            let w = size[0].as_f64().ok_or_else(|| bad("room.moldings", "`size` values must be numbers"))?;
            let h = size[1].as_f64().ok_or_else(|| bad("room.moldings", "`size` values must be numbers"))?;
            if w < 0.002 || w > 0.5 || h < 0.002 || h > 0.5 {
                return Err(bad("room.moldings", "`size` out of range [0.002, 0.5]"));
            }
            // Create a simple rectangular profile scaled
            let mut prof = MoldingProfile::default();
            prof.points = vec![[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
            prof
        } else {
            // Default profile
            cadcraft_arch::molding_profile::default_profile(kind)
        };

        new_moldings.push(RoomMolding { kind, profile, offset, ..Default::default() });
    }

    let d = s.doc_mut()?;

    // Resolve target rooms
    let targets: Vec<ArchId> = match room_param {
        Value::String(s) => {
            if s == "all" {
                d.arch.rooms.iter().map(|r| r.id).collect()
            } else {
                let id = room_id_from_label(&d.arch, s).ok_or_else(|| bad("room.moldings", format!("unknown room `{s}`")))?;
                vec![id]
            }
        }
        Value::Array(arr) => {
            let mut ids = Vec::new();
            for v in arr {
                if let Some(s) = v.as_str() {
                    let id = room_id_from_label(&d.arch, s).ok_or_else(|| bad("room.moldings", format!("unknown room `{s}`")))?;
                    ids.push(id);
                } else {
                    return Err(bad("room.moldings", "`room` array items must be strings"));
                }
            }
            ids
        }
        _ => return Err(bad("room.moldings", "`room` must be a string or array")),
    };

    if targets.is_empty() {
        return Err(bad("room.moldings", "no rooms selected"));
    }

    // Apply to each room
    for id in &targets {
        if let Some(room) = d.arch.rooms.iter_mut().find(|r| r.id == *id) {
            room.moldings = new_moldings.clone();
        }
    }

    // Ensure layer exists
    d.ensure_layer("A-FLOR-TRIM");

    d.touch_arch();

    let room_labels: Vec<String> = targets.iter().map(|id| cadcraft_doc::arch::room_label(*id)).collect();
    Ok(json!({
        "rooms": room_labels,
        "moldings": new_moldings.len()
    }))
}

/// Inherit moldings from an old room to a new room if the new room has none.
pub(crate) fn inherit(new: &mut Room, old: &[Room]) {
    if !new.moldings.is_empty() {
        return;
    }
    let Some(pt) = cadcraft_arch::label::label_point(&new.polygon, &new.holes) else {
        return;
    };
    for o in old {
        if o.level != new.level {
            continue;
        }
        if cadcraft_geom::point_in_polygon(&o.polygon, pt) && !o.holes.iter().any(|h| cadcraft_geom::point_in_polygon(h, pt)) {
            new.moldings = o.moldings.clone();
            break;
        }
    }
}

/// Inspect room moldings and opening trims.
pub(crate) fn inspect_moldings(model: &ArchModel, p: &Value, out: &mut JsonMap<String, Value>) {
    if p.get("moldings").and_then(Value::as_bool) == Some(true) {
        let mut rows = Vec::new();
        for room in &model.rooms {
            for (idx, m) in room.moldings.iter().enumerate() {
                let runs = cadcraft_arch::molding_runs::room_runs(model, room);
                let room_runs: Vec<_> = runs.iter().filter(|r| r.index == idx).collect();

                let length: f64 = room_runs.iter().map(|r| cadcraft_arch::molding_runs::run_length(r)).sum();
                let z = room_runs.first().map(|r| (r.z[0], r.z[0] + m.profile.size()[1]));

                let run_objs: Vec<Value> = room_runs
                    .iter()
                    .map(|r| {
                        json!({
                            "points": r.path.iter().map(|p| json!([p.x, p.y])).collect::<Vec<_>>(),
                            "closed": r.closed
                        })
                    })
                    .collect();

                rows.push(json!({
                    "room": cadcraft_doc::arch::room_label(room.id),
                    "index": idx,
                    "kind": m.kind.key(),
                    "profile": "default", // TODO: resolve item name if stored
                    "size": m.profile.size(),
                    "offset": m.offset,
                    "z": z.map(|(a, b)| json!([a, b])).unwrap_or(Value::Null),
                    "length": length,
                    "runs": run_objs,
                    "layer": "A-FLOR-TRIM"
                }));
            }
        }
        out.insert("moldings".to_string(), Value::Array(rows));
    }

    // Add trim info to openings if they are present
    if let Some(Value::Array(openings)) = out.get_mut("openings") {
        for o_row in openings.iter_mut() {
            if let Some(o_id) = o_row.get("id").and_then(Value::as_str) {
                if let Some(o) = model.openings.iter().find(|o| cadcraft_doc::arch::opening_label(o.id) == o_id) {
                    let left = cadcraft_arch::opening_trim::trim_interval(model, o, cadcraft_doc::arch::Side::Left);
                    let right = cadcraft_arch::opening_trim::trim_interval(model, o, cadcraft_doc::arch::Side::Right);

                    let left_json = left.map(|(a, b)| json!([a, b])).unwrap_or(Value::Null);
                    let right_json = right.map(|(a, b)| json!([a, b])).unwrap_or(Value::Null);
                    let left_w = left.map(|(a, b)| b - a).unwrap_or(0.0);
                    let right_w = right.map(|(a, b)| b - a).unwrap_or(0.0);

                    if let Some(obj) = o_row.as_object_mut() {
                        obj.insert("trim".to_string(), json!({"left": left_json, "right": right_json}));
                        obj.insert("trimWidth".to_string(), json!({"left": left_w, "right": right_w}));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use serde_json::json;

    #[test]
    fn create_replace_clear_are_single_undo_steps() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0], [0.0, 0.0]], "thickness": 0.2})).unwrap();
        s.execute("roomdetect", &json!({})).unwrap();
        let room_id = s.doc().unwrap().arch.rooms[0].id;
        let label = cadcraft_doc::arch::room_label(room_id);

        let rev = s.doc().unwrap().arch_revision;

        // Create
        let res = s.execute(
            "room.moldings",
            &json!({
                "room": label,
                "moldings": [{"kind": "base", "size": [0.1, 0.1]}]
            }),
        );
        assert!(res.is_ok(), "{res:?}");
        assert_eq!(s.doc().unwrap().arch_revision, rev + 1);

        // Replace
        let res = s.execute(
            "room.moldings",
            &json!({
                "room": label,
                "moldings": [{"kind": "crown", "size": [0.1, 0.1]}]
            }),
        );
        assert!(res.is_ok(), "{res:?}");
        assert_eq!(s.doc().unwrap().arch_revision, rev + 2);

        // Clear
        let res = s.execute(
            "room.moldings",
            &json!({
                "room": label,
                "moldings": []
            }),
        );
        assert!(res.is_ok(), "{res:?}");
        assert_eq!(s.doc().unwrap().arch_revision, rev + 3);

        // Undo
        s.undo().unwrap();
        assert_eq!(s.doc().unwrap().arch_revision, rev + 2);
        assert_eq!(s.doc().unwrap().arch.rooms[0].moldings.len(), 1);
    }

    #[test]
    fn defaults_per_kind() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0], [0.0, 0.0]], "thickness": 0.2})).unwrap();
        s.execute("roomdetect", &json!({})).unwrap();
        let room_id = s.doc().unwrap().arch.rooms[0].id;
        let label = cadcraft_doc::arch::room_label(room_id);

        s.execute(
            "room.moldings",
            &json!({
                "room": label,
                "moldings": [
                    {"kind": "base"},
                    {"kind": "chair"},
                    {"kind": "crown"}
                ]
            }),
        )
        .unwrap();

        let room = &s.doc().unwrap().arch.rooms[0];
        assert_eq!(room.moldings[0].offset, 0.0);
        assert_eq!(room.moldings[1].offset, 0.9);
        assert_eq!(room.moldings[2].offset, 0.0);
    }

    #[test]
    fn all_rooms() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0], [0.0, 0.0]], "thickness": 0.2})).unwrap();
        s.execute("wall", &json!({"points": [[0.0, 4.0], [10.0, 4.0]], "thickness": 0.1})).unwrap();
        s.execute("roomdetect", &json!({})).unwrap();

        s.execute(
            "room.moldings",
            &json!({
                "room": "all",
                "moldings": [{"kind": "base"}]
            }),
        )
        .unwrap();

        for r in &s.doc().unwrap().arch.rooms {
            assert_eq!(r.moldings.len(), 1);
        }
    }

    #[test]
    fn refusals_leave_nothing() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0], [0.0, 0.0]], "thickness": 0.2})).unwrap();
        s.execute("roomdetect", &json!({})).unwrap();
        let rev = s.doc().unwrap().arch_revision;

        // Bad room
        let res = s.execute("room.moldings", &json!({"room": "r99", "moldings": []}));
        assert!(res.is_err());
        assert_eq!(s.doc().unwrap().arch_revision, rev);

        // Bad kind
        let res = s.execute("room.moldings", &json!({"room": "r0", "moldings": [{"kind": "bad"}]}));
        assert!(res.is_err());
        assert_eq!(s.doc().unwrap().arch_revision, rev);
    }

    #[test]
    fn roomdetect_inherits_moldings() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0], [0.0, 0.0]], "thickness": 0.2})).unwrap();
        s.execute("roomdetect", &json!({})).unwrap();

        s.execute(
            "room.moldings",
            &json!({
                "room": "r0",
                "moldings": [{"kind": "base", "size": [0.1, 0.1]}]
            }),
        )
        .unwrap();

        assert_eq!(s.doc().unwrap().arch.rooms[0].moldings.len(), 1);

        s.execute("roomdetect", &json!({})).unwrap();

        assert_eq!(s.doc().unwrap().arch.rooms[0].moldings.len(), 1);
    }

    #[test]
    fn inspect_moldings_rows_and_opening_trim() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0], [0.0, 0.0]], "thickness": 0.2})).unwrap();
        s.execute("roomdetect", &json!({})).unwrap();
        s.execute(
            "room.moldings",
            &json!({
                "room": "r0",
                "moldings": [{"kind": "base", "size": [0.1, 0.1]}]
            }),
        )
        .unwrap();
        s.execute("door", &json!({"wall": "w0", "offset": 5.0, "width": 0.9, "height": 2.1})).unwrap();

        let v = s.execute("arch.inspect", &json!({"moldings": true, "openings": true})).unwrap();
        assert!(v["moldings"].is_array());
        assert_eq!(v["moldings"].as_array().unwrap().len(), 1);

        let o = &v["openings"][0];
        assert!(o["trim"].is_object());
        assert!(o["trimWidth"].is_object());
    }
}
