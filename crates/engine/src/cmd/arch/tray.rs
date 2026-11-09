//! Ceiling tray commands (phase 16): create, edit, delete, and inspect.

use std::collections::{HashSet, VecDeque};

use cadcraft_arch::ceiling::tray::{outline_in, top_ring_in, validate as validate_tray};
use cadcraft_arch::ceiling::{reseat, rough_ceiling_in, seated_ceiling_items};
use cadcraft_doc::arch::{
    ArchId, ArchModel, CEILING_LAYER, MAX_TRAY_NESTING, MAX_TRAYS, Tray, TraySides, handle_json, room_label, tray_handle, tray_label,
};
use cadcraft_geom::{Vec2, point_in_polygon};
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, bad, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("tray", "Tray Ceiling", run_tray)
            .menu(&["Architecture", "Tray Ceiling"])
            .params("{room: \"r1\", inset: m, depth: m, sides?: \"vertical\" | number} | {tray: \"tr0\", inset, depth, sides?} | {polygon: [[x,y],...], depth, inset?: 0, sides?, level?: name|index}"),
        CommandSpec::new("tray.edit", "Edit Tray", run_tray_edit)
            .menu(&["Architecture", "Edit Tray"])
            .params("{id: \"tr0\", inset?: m, depth?: m, sides?: \"vertical\" | number, polygon?: [[x,y],...]}"),
        CommandSpec::new("tray.delete", "Delete Tray", run_tray_delete)
            .menu(&["Architecture", "Delete Tray"])
            .params("{id: \"tr0\"}"),
    ]
}

fn parse_id(cmd: &str, p: &Value, key: &str, prefix: &str) -> Result<ArchId> {
    let raw = str_param(p, key).ok_or_else(|| bad(cmd, format!("`{key}` (\"{prefix}0\") is required")))?;
    let text = raw.trim();
    let digits = if text.eq_ignore_ascii_case(prefix) {
        "0".to_string()
    } else if let Some(rest) = text.get(prefix.len()..) {
        rest.to_string()
    } else {
        text.to_string()
    };
    digits.parse::<u32>().map(ArchId).map_err(|_| bad(cmd, format!("unknown {prefix} `{raw}`")))
}

fn parse_tray_id(cmd: &str, p: &Value, key: &str) -> Result<ArchId> {
    parse_id(cmd, p, key, "tr")
}

fn parse_room_id(cmd: &str, p: &Value, key: &str) -> Result<ArchId> {
    parse_id(cmd, p, key, "r")
}

fn parse_level(cmd: &str, model: &ArchModel, v: &Value) -> Result<u32> {
    match v {
        Value::Null => Ok(0),
        Value::Number(n) => n
            .as_u64()
            .and_then(|i| u32::try_from(i).ok())
            .filter(|i| model.level(*i).is_some())
            .ok_or_else(|| bad(cmd, "`level` must be a level name or index")),
        Value::String(s) => {
            let t = s.trim();
            if let Ok(i) = t.parse::<u32>() {
                if model.level(i).is_some() {
                    return Ok(i);
                }
            }
            model
                .levels
                .iter()
                .find(|level| level.name == *t || level.name.eq_ignore_ascii_case(t))
                .map(|level| level.index)
                .ok_or_else(|| bad(cmd, format!("unknown level `{s}`")))
        }
        _ => Err(bad(cmd, "`level` must be a level name or index")),
    }
}

fn parse_sides(cmd: &str, p: &Value) -> Result<TraySides> {
    match p.get("sides") {
        None | Some(Value::Null) => Ok(TraySides::Vertical),
        Some(v) => match v {
            Value::String(s) if s.trim().eq_ignore_ascii_case("vertical") => Ok(TraySides::Vertical),
            _ => v
                .as_f64()
                .filter(|deg| deg.is_finite())
                .map(TraySides::Pitched)
                .ok_or_else(|| bad(cmd, "`sides` must be \"vertical\" or a finite angle in degrees")),
        },
    }
}

fn parse_ring(cmd: &str, v: &Value) -> Result<Vec<Vec2>> {
    let Some(arr) = v.as_array() else {
        return Err(bad(cmd, "`polygon` must be a closed ring of [x, y] points"));
    };
    let mut ring = Vec::with_capacity(arr.len());
    for pt in arr {
        let Some(pair) = pt.as_array() else {
            return Err(bad(cmd, "`polygon` must be a closed ring of [x, y] points"));
        };
        let Some(x) = pair.first().and_then(Value::as_f64) else {
            return Err(bad(cmd, "`polygon` must be a closed ring of [x, y] points"));
        };
        let Some(y) = pair.get(1).and_then(Value::as_f64) else {
            return Err(bad(cmd, "`polygon` must be a closed ring of [x, y] points"));
        };
        let p = Vec2::new(x, y);
        if !p.is_finite() {
            return Err(bad(cmd, "`polygon` must use finite points"));
        }
        ring.push(p);
    }
    if ring.len() < 4 || ring.first() != ring.last() {
        return Err(bad(cmd, "`polygon` must be a closed ring with at least 4 points"));
    }
    Ok(ring)
}

fn parse_f64(cmd: &str, p: &Value, key: &str) -> Result<f64> {
    p.get(key).and_then(Value::as_f64).filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite number")))
}

fn room_for_polygon(model: &ArchModel, level: u32, polygon: &[Vec2]) -> Option<cadcraft_doc::arch::Room> {
    let point = cadcraft_geom::region::interior_point(polygon)?;
    model
        .rooms
        .iter()
        .filter(|r| r.level == level)
        .find(|r| point_in_polygon(&r.polygon, point) && !r.holes.iter().any(|h| point_in_polygon(h, point)))
        .cloned()
}

fn tray_depth(model: &ArchModel, tray_id: ArchId) -> usize {
    let mut depth = 0usize;
    let mut current = model.trays.iter().find(|t| t.id == tray_id).and_then(|t| t.parent);
    while let Some(parent_id) = current {
        depth += 1;
        current = model.trays.iter().find(|t| t.id == parent_id).and_then(|t| t.parent);
    }
    depth
}

fn tray_descendants(model: &ArchModel, root: ArchId) -> Vec<ArchId> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut q = VecDeque::new();
    q.push_back(root);
    while let Some(id) = q.pop_front() {
        if !seen.insert(id) {
            continue;
        }
        out.push(id);
        for child in model.trays.iter().filter(|t| t.parent == Some(id)) {
            if seen.len() <= MAX_TRAY_NESTING + 1 {
                q.push_back(child.id);
            }
        }
    }
    out
}

fn parse_create_tray(cmd: &str, model: &ArchModel, p: &Value) -> Result<Tray> {
    let mut t = Tray::default();
    t.id = ArchId(model.next_tray_id);
    let keys = [p.get("room").is_some(), p.get("tray").is_some(), p.get("polygon").is_some()];
    if keys.iter().filter(|b| **b).count() != 1 {
        return Err(bad(cmd, "exactly one of `room`, `tray` or `polygon` is required"));
    }

    if p.get("room").is_some() {
        t.room = parse_room_id(cmd, p, "room")?;
        let room = model.room(t.room).ok_or_else(|| bad(cmd, format!("unknown room `{}`", room_label(t.room))))?;
        t.level = room.level;
        t.inset = parse_f64(cmd, p, "inset")?;
        t.depth = parse_f64(cmd, p, "depth")?;
        t.sides = parse_sides(cmd, p)?;
        return Ok(t);
    }

    if p.get("tray").is_some() {
        let parent_id = parse_tray_id(cmd, p, "tray")?;
        let parent = model.tray(parent_id).ok_or_else(|| bad(cmd, format!("unknown tray `{}`", tray_label(parent_id))))?;
        t.parent = Some(parent_id);
        t.room = parent.room;
        t.level = parent.level;
        t.inset = parse_f64(cmd, p, "inset")?;
        t.depth = parse_f64(cmd, p, "depth")?;
        t.sides = parse_sides(cmd, p)?;
        return Ok(t);
    }

    let polygon = parse_ring(cmd, p.get("polygon").unwrap())?;
    let level = parse_level(cmd, model, p.get("level").unwrap_or(&Value::Number(0.into())))?;
    let room = room_for_polygon(model, level, &polygon).ok_or_else(|| bad(cmd, "the tray polygon does not sit in a room on that level"))?;
    t.room = room.id;
    t.level = room.level;
    t.inset = p.get("inset").filter(|v| !v.is_null()).map(|_| parse_f64(cmd, p, "inset")).transpose()?.unwrap_or(0.0);
    t.depth = parse_f64(cmd, p, "depth")?;
    t.sides = parse_sides(cmd, p)?;
    t.polygon = polygon.clone();

    // The drawn polygon becomes the parent tray's outline only when it is fully inside that tray's outline and inside the room;
    // otherwise there is no parent.
    let mut best_parent: Option<ArchId> = None;
    let mut best_area: f64 = f64::INFINITY;
    for cand in model.trays.iter() {
        if cand.room != room.id || cand.level != room.level || cand.id == t.id {
            continue;
        }
        if let Ok(outline) = outline_in(&room, &model.trays, cand) {
            if outline.iter().all(|p| p.is_finite()) && polygon.iter().all(|p| point_in_polygon(&outline, *p) && point_in_polygon(&room.polygon, *p))
            {
                let area = (0..outline.len())
                    .fold(0.0, |acc, i| {
                        let a = outline[i];
                        let b = outline[(i + 1) % outline.len()];
                        acc + 0.5 * (a.x * b.y - a.y * b.x)
                    })
                    .abs();
                if area < best_area {
                    best_area = area;
                    best_parent = Some(cand.id);
                }
            }
        }
    }
    t.parent = best_parent;
    Ok(t)
}

fn clean_f64(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let rounded = (value * 1_000_000.0).round() / 1_000_000.0;
    if (rounded - value).abs() < 1e-12 { rounded } else { value }
}

fn points_json(ring: &[Vec2]) -> Value {
    Value::Array(ring.iter().map(|p| json!([clean_f64(p.x), clean_f64(p.y)])).collect())
}

fn tray_row(model: &ArchModel, tray: &Tray) -> Value {
    let room = model.room(tray.room);
    let outline = room.and_then(|r| outline_in(r, &model.trays, tray).ok());
    let sides = match tray.sides {
        TraySides::Vertical => Value::String("vertical".to_string()),
        TraySides::Pitched(deg) => Value::from(deg),
    };
    let ceiling = room.and_then(|r| {
        top_ring_in(r, &model.trays, tray).ok().and_then(|ring| {
            let p = cadcraft_geom::region::interior_point(&ring)?;
            rough_ceiling_in(model, r, p)
        })
    });
    json!({
        "id": tray_label(tray.id),
        "handle": handle_json(tray_handle(tray.id)),
        "level": tray.level,
        "room": room_label(tray.room),
        "parent": tray.parent.map(tray_label),
        "inset": tray.inset,
        "depth": tray.depth,
        "sides": sides,
        "polygon": if tray.polygon.is_empty() { Value::Null } else { points_json(&tray.polygon) },
        "outline": outline.map_or(Value::Null, |ring| points_json(&ring)),
        "ceiling": ceiling,
        "layer": tray.layer,
    })
}

fn run_tray(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "tray";
    let model = s.doc()?.arch.clone();
    let mut tray = parse_create_tray(cmd, &model, p)?;
    if model.trays.len() >= MAX_TRAYS {
        return Err(bad(cmd, format!("a drawing holds at most {MAX_TRAYS} trays")));
    }
    tray.layer = CEILING_LAYER.to_string();
    validate_tray(&model, &tray).map_err(|e| bad(cmd, e))?;

    let mut doc = s.doc_mut()?;
    let level = tray.level;
    let items = seated_ceiling_items(&doc.arch, level);
    let id = doc.arch.push_tray(tray.clone());
    doc.ensure_layer(CEILING_LAYER);
    reseat(&mut doc.arch, &items);
    doc.touch_arch();

    let room = doc.arch.room(tray.room).ok_or_else(|| bad(cmd, format!("unknown room `{}`", room_label(tray.room))))?;
    let tray_obj = doc.arch.trays.iter().find(|t| t.id == id).ok_or_else(|| bad(cmd, "tray not found after insert"))?;
    let outline = outline_in(room, &doc.arch.trays, tray_obj).unwrap_or_default();
    let top = top_ring_in(room, &doc.arch.trays, tray_obj).ok();
    let ceiling = top.and_then(|ring| {
        let p = cadcraft_geom::region::interior_point(&ring)?;
        rough_ceiling_in(&doc.arch, room, p)
    });

    Ok(json!({
        "tray": tray_label(id),
        "handle": handle_json(tray_handle(id)),
        "room": room_label(tray.room),
        "parent": tray.parent.map(tray_label),
        "outline": outline,
        "ceiling": ceiling,
    }))
}

fn run_tray_edit(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "tray.edit";
    let id = parse_tray_id(cmd, p, "id")?;
    let model = s.doc()?.arch.clone();
    let current = model.tray(id).ok_or_else(|| bad(cmd, format!("unknown tray `{}`", tray_label(id))))?.clone();
    let mut edited = current.clone();

    if p.get("inset").filter(|v| !v.is_null()).is_some() {
        edited.inset = parse_f64(cmd, p, "inset")?;
    }
    if p.get("depth").filter(|v| !v.is_null()).is_some() {
        edited.depth = parse_f64(cmd, p, "depth")?;
    }
    if p.get("sides").filter(|v| !v.is_null()).is_some() {
        edited.sides = parse_sides(cmd, p)?;
    }
    if let Some(v) = p.get("polygon").filter(|v| !v.is_null()) {
        edited.polygon = parse_ring(cmd, v)?;
    }

    let mut trial = model.clone();
    if let Some(slot) = trial.trays.iter_mut().find(|t| t.id == id) {
        *slot = edited.clone();
    } else {
        return Err(bad(cmd, format!("unknown tray `{}`", tray_label(id))));
    }

    let affected = tray_descendants(&trial, id);
    for aid in &affected {
        let Some(t) = trial.trays.iter().find(|row| row.id == *aid) else { continue };
        validate_tray(&trial, t).map_err(|e| bad(cmd, e))?;
    }

    let mut doc = s.doc_mut()?;
    let level = edited.level;
    let items = seated_ceiling_items(&doc.arch, level);
    if let Some(slot) = doc.arch.trays.iter_mut().find(|t| t.id == id) {
        *slot = edited.clone();
    }
    doc.ensure_layer(CEILING_LAYER);
    reseat(&mut doc.arch, &items);
    doc.touch_arch();
    Ok(tray_row(&doc.arch, &edited))
}

fn run_tray_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "tray.delete";
    let id = parse_tray_id(cmd, p, "id")?;
    let model = s.doc()?.arch.clone();
    let current = model.tray(id).ok_or_else(|| bad(cmd, format!("unknown tray `{}`", tray_label(id))))?;
    let deleted = tray_descendants(&model, id);
    let mut doc = s.doc_mut()?;
    let level = current.level;
    let items = seated_ceiling_items(&doc.arch, level);
    doc.arch.trays.retain(|t| !deleted.contains(&t.id));
    doc.ensure_layer(CEILING_LAYER);
    reseat(&mut doc.arch, &items);
    doc.touch_arch();
    Ok(json!({ "deleted": deleted.iter().map(|did| tray_label(*did)).collect::<Vec<_>>() }))
}

pub(crate) fn inspect_trays(model: &ArchModel, p: &Value, out: &mut Map<String, Value>) {
    if p.get("trays").and_then(Value::as_bool) != Some(true) {
        return;
    }
    let rows: Vec<Value> = model.trays.iter().map(|t| tray_row(model, t)).collect();
    out.insert("trays".to_string(), Value::Array(rows));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_phase1_house(s: &mut Session) {
        s.execute(
            "wall",
            &json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0], [0.0, 0.0]], "thickness": 0.2, "height": 2.4, "exterior": true}),
        )
        .unwrap();
        s.execute("wall", &json!({"points": [[0.0, 3.0], [10.0, 3.0]], "thickness": 0.1, "height": 2.4, "type": "interior"})).unwrap();
        s.execute("wall", &json!({"points": [[6.0, 3.0], [6.0, 8.0]], "thickness": 0.1, "height": 2.4, "type": "interior"})).unwrap();
        s.execute("roomdetect", &json!({})).unwrap();
        s.execute(
            "roofauto",
            &json!({"pitchRise": 6, "pitchRun": 12, "walls": [{"wall": "w1", "directive": "gable"}, {"wall": "w3", "directive": "gable"}]}),
        )
        .unwrap();
    }

    fn nw_room_id(s: &mut Session) -> String {
        let inspect = s.execute("arch.inspect", &json!({"rooms": true})).unwrap();
        let rooms = inspect["rooms"].as_array().unwrap();
        rooms
            .iter()
            .filter(|room| {
                let poly = room["polygon"].as_array().unwrap();
                let avg_x = poly.iter().map(|p| p[0].as_f64().unwrap()).sum::<f64>() / poly.len() as f64;
                let avg_y = poly.iter().map(|p| p[1].as_f64().unwrap()).sum::<f64>() / poly.len() as f64;
                avg_x < 5.0 && avg_y > 4.0
            })
            .min_by(|a, b| {
                let a_avg = a["polygon"].as_array().unwrap().iter().map(|p| p[1].as_f64().unwrap()).sum::<f64>()
                    / a["polygon"].as_array().unwrap().len() as f64;
                let b_avg = b["polygon"].as_array().unwrap().iter().map(|p| p[1].as_f64().unwrap()).sum::<f64>()
                    / b["polygon"].as_array().unwrap().len() as f64;
                a_avg.partial_cmp(&b_avg).unwrap()
            })
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn eave_room_id(s: &mut Session) -> String {
        let inspect = s.execute("arch.inspect", &json!({"rooms": true})).unwrap();
        let rooms = inspect["rooms"].as_array().unwrap();
        rooms
            .iter()
            .min_by(|a, b| {
                let a_avg = a["polygon"].as_array().unwrap().iter().map(|p| p[1].as_f64().unwrap()).sum::<f64>()
                    / a["polygon"].as_array().unwrap().len() as f64;
                let b_avg = b["polygon"].as_array().unwrap().iter().map(|p| p[1].as_f64().unwrap()).sum::<f64>()
                    / b["polygon"].as_array().unwrap().len() as f64;
                a_avg.partial_cmp(&b_avg).unwrap()
            })
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn tray_create_edit_delete_are_single_undo_steps() {
        let mut s = Session::new();
        build_phase1_house(&mut s);
        let before = s.doc().unwrap().arch_revision;
        let room = nw_room_id(&mut s);
        s.execute("tray", &json!({"room": room, "inset": 0.6, "depth": 0.3})).unwrap();
        assert_eq!(s.doc().unwrap().arch_revision, before + 1);
        let tray = s.execute("arch.inspect", &json!({"trays": true})).unwrap()["trays"].as_array().unwrap()[0]["id"].as_str().unwrap().to_string();
        s.execute("tray.edit", &json!({"id": tray, "depth": 0.4})).unwrap();
        assert_eq!(s.doc().unwrap().arch_revision, before + 2);
        s.execute("tray.delete", &json!({"id": tray})).unwrap();
        assert_eq!(s.doc().unwrap().arch_revision, before + 3);
        let label = s.undo().unwrap();
        assert!(label.is_some());
        s.undo().unwrap();
        s.undo().unwrap();
        assert_eq!(s.doc().unwrap().arch_revision, before);
    }

    #[test]
    fn tray_refusals_leave_nothing() {
        let mut s = Session::new();
        build_phase1_house(&mut s);
        let room = eave_room_id(&mut s);
        let rev = s.doc().unwrap().arch_revision;

        let invalid_depth = 3.0;
        let err = s.execute("tray", &json!({"room": room, "inset": 0.6, "depth": invalid_depth}));
        let msg = err.as_ref().unwrap_err().to_string();
        println!("bad tray msg: {msg}");
        println!("tray inspect after bad tray: {}", s.execute("arch.inspect", &json!({"trays": true})).unwrap()["trays"]);
        assert!(msg.contains("roof") || msg.contains("flat ceiling") || msg.contains("tray") || msg.contains("usable number"));
        assert!(s.execute("arch.inspect", &json!({"trays": true})).unwrap()["trays"].as_array().unwrap().is_empty());

        let room_doc = s.doc().unwrap().arch.rooms.iter().find(|r| room_label(r.id) == room).unwrap().clone();
        let center = cadcraft_arch::ceiling::rough_ceiling_in(
            &s.doc().unwrap().arch,
            &room_doc,
            cadcraft_geom::region::interior_point(&room_doc.polygon).unwrap(),
        )
        .unwrap();
        assert!((center - 2.4).abs() < 1e-6);

        let mut room_mut = s.doc_mut().unwrap();
        room_mut.arch.rooms.iter_mut().find(|r| room_label(r.id) == room).unwrap().flat_ceiling = false;
        drop(room_mut);

        let err2 = s.execute("tray", &json!({"room": room, "inset": 0.6, "depth": 0.3}));
        assert!(err2.is_err());

        let bad_json = [
            json!({"room": room, "inset": "bad", "depth": 0.3}),
            json!({"polygon": [[0,0],[1,0],[1,1],[0,1]], "depth": 0.3}),
            json!({"tray": "tr999", "inset": 0.6, "depth": 0.3}),
        ];
        for b in bad_json {
            let before_rev = s.doc().unwrap().arch_revision;
            assert!(s.execute("tray", &b).is_err());
            assert_eq!(s.doc().unwrap().arch_revision, before_rev);
        }
        assert_eq!(s.doc().unwrap().arch_revision, rev);
    }

    #[test]
    fn tray_delete_removes_nested_trays() {
        let mut s = Session::new();
        build_phase1_house(&mut s);
        let room = nw_room_id(&mut s);
        s.execute("tray", &json!({"room": room, "inset": 0.6, "depth": 0.3})).unwrap();
        let parent = s.execute("arch.inspect", &json!({"trays": true})).unwrap()["trays"].as_array().unwrap()[0]["id"].as_str().unwrap().to_string();

        let model = s.doc().unwrap().arch.clone();
        let parent_t = model.trays[0].clone();
        let child = Tray {
            id: ArchId(model.next_tray_id),
            room: parent_t.room,
            level: parent_t.level,
            parent: Some(parent_t.id),
            inset: 0.6,
            depth: 0.1,
            ..Default::default()
        };
        let nested = [0.3, 0.4, 0.5, 0.6]
            .into_iter()
            .find_map(|inset| {
                let result = s.execute("tray", &json!({"tray": parent, "inset": inset, "depth": 0.1}));
                match result {
                    Ok(_) => Some(inset),
                    Err(_) => None,
                }
            })
            .expect("a valid nested tray should fit inside the parent tray");
        let _ = nested;

        let deleted = s.execute("tray.delete", &json!({"id": parent})).unwrap()["deleted"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(deleted.len(), 2);
        assert!(deleted.iter().any(|v| v == &parent));
        assert!(s.execute("arch.inspect", &json!({"trays": true})).unwrap()["trays"].as_array().unwrap().is_empty());
    }

    #[test]
    fn inspect_trays_rows() {
        let mut s = Session::new();
        build_phase1_house(&mut s);
        let room = nw_room_id(&mut s);
        let tray_id = s.execute("tray", &json!({"room": room, "inset": 0.6, "depth": 0.3})).unwrap()["tray"].as_str().unwrap().to_string();
        let inspect = s.execute("arch.inspect", &json!({"trays": true})).unwrap();
        let rows = inspect["trays"].as_array().unwrap();
        println!("tray rows: {rows:#?}");
        let row = rows.iter().find(|r| r["id"] == tray_id).unwrap();
        assert_eq!(row["parent"], Value::Null);
        assert_eq!(row["depth"], 0.3);
        assert_eq!(row["inset"], 0.6);
        assert_eq!(row["sides"], "vertical");
        let outline = row["outline"].as_array().unwrap();
        for p in outline {
            let a = p.as_array().unwrap();
            assert!(a[0].as_f64().unwrap() >= 0.7 && a[0].as_f64().unwrap() <= 5.35);
            assert!(a[1].as_f64().unwrap() >= 3.65 && a[1].as_f64().unwrap() <= 7.3);
        }
        assert!((row["ceiling"].as_f64().unwrap() - 2.7).abs() < 1e-6);
    }

    /// A drawn polygon inside tr0's outline gets `room` = NW and `parent` = tr0; one outside
    /// every tray gets `parent` null; a polygon straddling two rooms or partly outside its room
    /// is refused.
    #[test]
    fn polygon_tray_finds_its_room_and_parent() {
        let mut s = Session::new();
        build_phase1_house(&mut s);
        let room = nw_room_id(&mut s);
        let rev = s.doc().unwrap().arch_revision;

        // tr0 in the NW room: its outline is (0.7..5.35, 3.65..7.3).
        s.execute("tray", &json!({"room": room, "inset": 0.6, "depth": 0.3})).unwrap();
        let parent_id =
            s.execute("arch.inspect", &json!({"trays": true})).unwrap()["trays"].as_array().unwrap()[0]["id"].as_str().unwrap().to_string();

        // A polygon fully inside tr0's outline, still inside the room.
        let inside = vec![[2.0, 4.0], [4.0, 4.0], [4.0, 6.0], [2.0, 6.0], [2.0, 4.0]];
        let res = s.execute("tray", &json!({"polygon": inside.clone(), "depth": 0.2})).unwrap();
        assert_eq!(res["room"], room);
        assert_eq!(res["parent"], parent_id);
        let handle = res["tray"].as_str().unwrap().to_string();
        s.execute("tray.delete", &json!({"id": handle})).unwrap();

        // A polygon inside the room but outside tr0's outline: parent is null.
        let outside = vec![[0.2, 3.2], [0.5, 3.2], [0.5, 3.5], [0.2, 3.5], [0.2, 3.2]];
        let res_out = s.execute("tray", &json!({"polygon": outside.clone(), "depth": 0.2})).unwrap();
        assert_eq!(res_out["room"], room);
        assert_eq!(res_out["parent"], Value::Null);
        let outside_handle = res_out["tray"].as_str().unwrap().to_string();

        // A polygon straddling the NW / South boundary: refused, nothing written.
        let straddle = vec![[2.0, 2.8], [4.0, 2.8], [4.0, 5.0], [2.0, 5.0], [2.0, 2.8]];
        let before = s.doc().unwrap().arch_revision;
        let err = s.execute("tray", &json!({"polygon": straddle.clone(), "depth": 0.2}));
        assert!(err.is_err());
        assert_eq!(s.doc().unwrap().arch_revision, before);
        assert_eq!(s.doc().unwrap().arch_revision, rev + 4);

        let gone = s.execute("tray.delete", &json!({"id": outside_handle})).unwrap();
        assert_eq!(gone["deleted"].as_array().unwrap().len(), 1);
    }
}
