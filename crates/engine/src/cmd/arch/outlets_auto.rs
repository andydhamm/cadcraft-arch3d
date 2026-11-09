//! Auto-place outlets for a room.
//!
//! The room's usable wall runs are enumerated in `cadcraft_arch::outlets`, then a
//! single pass of `auto_outlets` decides where outlets belong. The command either
//! appends those devices to the document in one undo step or returns the would-be
//! labels when `dryRun` is set.

use cadcraft_arch::outlets::{auto_outlets, outlet_gaps};
use cadcraft_arch::room::wall_on_level;
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::{ArchId, Room, RoomType, electrical_label, room_label};
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::{Result, Session};

fn parse_room_id(doc: &Drawing, raw: &Value, cmd: &str) -> Result<ArchId> {
    let text = raw.as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad(cmd, "`room` must be a room label such as `r0`"))?;
    let digits = text.strip_prefix('r').or_else(|| text.strip_prefix('R')).unwrap_or(text);
    let id = digits.parse::<u32>().map(ArchId).map_err(|_| bad(cmd, format!("unknown room `{text}`")))?;
    let room = doc.arch.room(id).ok_or_else(|| bad(cmd, format!("room `{text}` does not exist")))?;
    if !doc.arch.walls.iter().any(|wall| wall_on_level(&doc.arch, wall, room.level)) {
        return Err(bad(cmd, format!("room `{text}` has no walls on its level")));
    }
    Ok(id)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("outlets.auto", "Auto Outlets", run_outlets_auto)
            .menu(&["Architecture", "Electrical", "Auto Outlets"])
            .params("{room: \"r<n>\", dryRun?: bool}"),
    ]
}

fn run_outlets_auto(s: &mut Session, params: &Value) -> Result<Value> {
    let cmd = "outlets.auto";
    let room_raw = params.get("room").filter(|v| !v.is_null()).ok_or_else(|| bad(cmd, "`room` is required"))?;
    let room = parse_room_id(s.doc()?, room_raw, cmd)?;
    let dry_run = params.get("dryRun").and_then(Value::as_bool).unwrap_or(false);

    let room_def = s.doc()?.arch.room(room).cloned().ok_or_else(|| bad(cmd, format!("room `{}` does not exist", room_label(room))))?;
    let planned = auto_outlets(&s.doc()?.arch, room);

    let mut staged = s.doc()?.arch.clone();
    let placed = if dry_run {
        let mut ids = Vec::new();
        for device in &planned {
            let id = staged.push_electrical(device.clone());
            ids.push(electrical_label(id));
        }
        ids
    } else {
        let mut doc = s.doc_mut()?;
        let mut ids = Vec::new();
        for device in &planned {
            let id = doc.arch.push_electrical(device.clone());
            ids.push(electrical_label(id));
        }
        doc.touch_arch();
        ids
    };

    let model = if dry_run { &staged } else { &s.doc()?.arch };
    let gap = outlet_gaps(model, room);
    Ok(json!({
        "placed": placed,
        "maxGap": gap.max_gap,
        "kitchen": room_def.room_type == RoomType::Kitchen,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_arch::room::detect_rooms;
    use cadcraft_doc::ArchModel;
    use cadcraft_doc::arch::{Cabinet, CabinetKind, RoomType, Wall};
    use cadcraft_geom::Vec2;
    use serde_json::json;

    fn room64() -> ArchModel {
        let mut m = ArchModel::default();
        for (a, b) in [((-0.1, -0.1), (6.1, -0.1)), ((6.1, -0.1), (6.1, 4.1)), ((6.1, 4.1), (-0.1, 4.1)), ((-0.1, 4.1), (-0.1, -0.1))] {
            m.push_wall(Wall { start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness: 0.2, ..Default::default() });
        }
        m.rooms = detect_rooms(&m);
        if let Some(r) = m.rooms.first_mut() {
            r.room_type = RoomType::Bedroom;
            r.name = "Room".into();
        }
        m
    }

    #[test]
    fn auto_outlets_meet_the_spacing_rule() {
        let mut s = Session::new();
        s.doc_mut().unwrap().arch = room64();
        let reply = s.execute("outlets.auto", &json!({ "room": "r0" })).unwrap();
        let placed = reply["placed"].as_array().unwrap();
        assert!(!placed.is_empty());
        let max_gap = reply["maxGap"].as_f64().unwrap();
        assert!(max_gap <= 1.8 + 1e-6, "gap {max_gap} exceeds 1.8 m");
        assert_eq!(reply["kitchen"], Value::Bool(false));
    }

    #[test]
    fn dry_run_places_nothing() {
        let mut s = Session::new();
        s.doc_mut().unwrap().arch = room64();
        let before = s.doc().unwrap().arch.electrical.len();
        let reply = s.execute("outlets.auto", &json!({ "room": "r0", "dryRun": true })).unwrap();
        assert_eq!(s.doc().unwrap().arch.electrical.len(), before);
        assert!(!reply["placed"].as_array().unwrap().is_empty());
    }

    #[test]
    fn missing_room_is_refused() {
        let mut s = Session::new();
        s.doc_mut().unwrap().arch = room64();
        let result = s.execute("outlets.auto", &json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn non_string_room_is_refused() {
        let mut s = Session::new();
        s.doc_mut().unwrap().arch = room64();
        let result = s.execute("outlets.auto", &json!({ "room": 0 }));
        assert!(result.is_err());
    }

    #[test]
    fn nonexistent_room_is_refused() {
        let mut s = Session::new();
        s.doc_mut().unwrap().arch = room64();
        let result = s.execute("outlets.auto", &json!({ "room": "r99" }));
        assert!(result.is_err());
    }

    #[test]
    fn room_with_no_walls_on_level_is_refused() {
        let mut m = ArchModel::default();
        // Walls on level 0
        for (a, b) in [((-0.1, -0.1), (6.1, -0.1)), ((6.1, -0.1), (6.1, 4.1)), ((6.1, 4.1), (-0.1, 4.1)), ((-0.1, 4.1), (-0.1, -0.1))] {
            m.push_wall(Wall { start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness: 0.2, ..Default::default() });
        }
        // A room on level 1 (no walls on level 1)
        let room = Room {
            id: ArchId(0),
            level: 1,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(3.0, 3.0), Vec2::new(0.0, 3.0)],
            ..Default::default()
        };
        m.rooms.push(room);

        let mut s = Session::new();
        s.doc_mut().unwrap().arch = m;
        let result = s.execute("outlets.auto", &json!({ "room": "r0" }));
        assert!(result.is_err());
    }

    #[test]
    fn tiny_room_places_few_or_no_outlets() {
        let mut m = ArchModel::default();
        // A 1x1 m room
        for (a, b) in [((-0.1, -0.1), (1.1, -0.1)), ((1.1, -0.1), (1.1, 1.1)), ((1.1, 1.1), (-0.1, 1.1)), ((-0.1, 1.1), (-0.1, -0.1))] {
            m.push_wall(Wall { start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness: 0.2, ..Default::default() });
        }
        m.rooms = detect_rooms(&m);
        if let Some(r) = m.rooms.first_mut() {
            r.room_type = RoomType::Bedroom;
        }

        let mut s = Session::new();
        s.doc_mut().unwrap().arch = m;
        let reply = s.execute("outlets.auto", &json!({ "room": "r0" })).unwrap();
        let placed = reply["placed"].as_array().unwrap();
        // A 1x1 room should need very few outlets (0, 1, or 2)
        assert!(placed.len() <= 2, "tiny room should not need many outlets: {}", placed.len());
    }

    #[test]
    fn a_kitchen_gets_counter_gfci() {
        let mut m = room64();
        if let Some(r) = m.rooms.first_mut() {
            r.room_type = RoomType::Kitchen;
        }
        let counter_positions = [1.0, 1.75];
        for (x, w) in [(1.0, 0.6), (1.75, 0.9)] {
            let id = cadcraft_doc::ArchId(m.cabinets.len() as u32 + 0x50000);
            m.cabinets.push(Cabinet {
                id,
                kind: CabinetKind::Base,
                counter: true,
                position: Vec2::new(x as f64, 0.0),
                rotation: 0.0,
                width: w,
                depth: 0.6,
                height: 0.9,
                ..Default::default()
            });
        }

        let mut s = Session::new();
        s.doc_mut().unwrap().arch = m;
        let reply = s.execute("outlets.auto", &json!({ "room": "r0" })).unwrap();
        assert_eq!(reply["kitchen"], Value::Bool(true));
        let placed = reply["placed"].as_array().unwrap();
        assert!(!placed.is_empty());
        let mut found_gfci = false;
        for item in placed {
            let label = item.as_str().unwrap();
            for dev in s.doc().unwrap().arch.electrical.iter() {
                if electrical_label(dev.id) == label && dev.kind == cadcraft_doc::arch::ElectricalKind::Gfci {
                    found_gfci = true;
                    break;
                }
            }
        }
        assert!(found_gfci, "kitchen should place GFCI outlets: {reply}");
    }
}
