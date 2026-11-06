//! `outlet`, `switch`, `electrical.edit` and `electrical.delete` (phase 15).

use std::str::FromStr;

use cadcraft_arch::electrical::{device_room, place_device};
use cadcraft_color::Color;
use cadcraft_doc::arch::{
    ArchId, ElectricalDevice, ElectricalKind, WallSide, electrical_handle, electrical_label, parse_electrical_label, room_label, wall_label,
};
use cadcraft_doc::{ArchModel, Handle};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::{Result, Session};

const OUTLET_KINDS: &[ElectricalKind] = &[ElectricalKind::Duplex, ElectricalKind::Gfci, ElectricalKind::Outlet240];
const SWITCH_KINDS: &[ElectricalKind] = &[ElectricalKind::SinglePole, ElectricalKind::ThreeWay, ElectricalKind::Dimmer];

fn ensure_electrical_layers(doc: &mut cadcraft_doc::Drawing) {
    if doc.ensure_layer("E-POWR") {
        let idx = doc.layers.iter_mut().position(|l| l.name.eq_ignore_ascii_case("E-POWR")).unwrap();
        doc.layers[idx].color = Color::Index(1);
    }
    if doc.ensure_layer("E-LITE") {
        let idx = doc.layers.iter_mut().position(|l| l.name.eq_ignore_ascii_case("E-LITE")).unwrap();
        doc.layers[idx].color = Color::Index(2);
    }
    for layer in &mut doc.layers {
        match layer.name.as_str() {
            "E-POWR" | "E-LITE" => {
                layer.color = match layer.name.as_str() {
                    "E-POWR" => Color::Index(1),
                    _ => Color::Index(2),
                };
            }
            _ => {}
        }
    }
}

fn parse_wall_id(doc: &cadcraft_doc::Drawing, raw: &Value, cmd: &str) -> Result<ArchId> {
    let text = raw.as_str().filter(|s| !s.trim().is_empty()).ok_or_else(|| bad(cmd, "`wall` must be a wall label such as `w0`"))?;
    let parsed = ArchId::from_str(text).map_err(|_| bad(cmd, format!("unknown wall `{text}`")))?;
    if doc.arch.wall(parsed).is_none() {
        return Err(bad(cmd, format!("wall `{text}` does not exist")));
    }
    Ok(parsed)
}

fn parse_side(raw: &Value, cmd: &str) -> Result<WallSide> {
    match raw.as_str().map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("left") => Ok(WallSide::Left),
        Some("right") => Ok(WallSide::Right),
        _ => Err(bad(cmd, "`side` must be `left` or `right`")),
    }
}

fn parse_level(doc: &cadcraft_doc::Drawing, raw: Option<&Value>, wall: ArchId, cmd: &str) -> Result<u32> {
    let wall_def = doc.arch.wall(wall).map(|w| w.base_level).unwrap_or(0);
    let value = match raw.filter(|v| !v.is_null()) {
        Some(v) => v,
        None => return Ok(wall_def),
    };
    if let Some(n) = value.as_u64() {
        let level = n as u32;
        if doc.arch.levels.iter().any(|l| l.index == level) {
            return Ok(level);
        }
    }
    if let Some(text) = value.as_str().map(str::trim).filter(|s| !s.is_empty()) {
        if let Ok(index) = text.parse::<u32>() {
            if doc.arch.levels.iter().any(|l| l.index == index) {
                return Ok(index);
            }
        }
        if let Some(level) = doc.arch.levels.iter().find(|l| l.name == text).map(|l| l.index) {
            return Ok(level);
        }
    }
    Err(bad(cmd, format!("unknown level `{}`", value)))
}

fn parse_point(raw: &Value, cmd: &str) -> Result<Vec2> {
    let arr = raw.as_array().filter(|v| v.len() == 2).ok_or_else(|| bad(cmd, "`point` must be an `[x, y]` array"))?;
    let x = arr[0].as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`point[0]` must be a finite number"))?;
    let y = arr[1].as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`point[1]` must be a finite number"))?;
    Ok(Vec2::new(x, y))
}

fn parse_offset(raw: &Value, cmd: &str) -> Result<f64> {
    let off = raw.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`offset` must be a finite number"))?;
    if !(0.0..=1e9).contains(&off) {
        return Err(bad(cmd, "`offset` must be a finite number"));
    }
    Ok(off)
}

fn parse_height(raw: Option<&Value>, cmd: &str, default: f64) -> Result<f64> {
    let Some(raw) = raw.filter(|v| !v.is_null()) else {
        return Ok(default);
    };
    let h = raw.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`height` must be a finite number"))?;
    if !(0.0..=3.0).contains(&h) {
        return Err(bad(cmd, "`height` must be between 0 and 3 metres"));
    }
    Ok(h)
}

fn parse_kind(raw: Option<&Value>, cmd: &str, allowed: &[ElectricalKind]) -> Result<ElectricalKind> {
    let value = raw.ok_or_else(|| bad(cmd, "`kind` is required"))?;
    let text = value.as_str().ok_or_else(|| bad(cmd, "`kind` must be a string"))?;
    let kind = ElectricalKind::parse(text)
        .ok_or_else(|| bad(cmd, format!("`kind` must be one of {}", allowed.iter().map(|k| k.key()).collect::<Vec<_>>().join(" | "))))?;
    if !allowed.contains(&kind) {
        let names = allowed.iter().map(|k| k.key()).collect::<Vec<_>>().join(" | ");
        return Err(bad(cmd, format!("`kind` must be one of {names}; got `{text}`")));
    }
    Ok(kind)
}

fn build_device(
    doc: &cadcraft_doc::Drawing,
    cmd: &str,
    kind: ElectricalKind,
    wall_id: ArchId,
    offset: f64,
    side: WallSide,
    height: f64,
    counter: bool,
    level: u32,
) -> Result<(ElectricalDevice, cadcraft_arch::electrical::DevicePlace)> {
    let wall = doc.arch.wall(wall_id).ok_or_else(|| bad(cmd, format!("wall `{}` does not exist", wall_label(wall_id))))?;
    let level_def = doc.arch.levels.iter().find(|l| l.index == level).ok_or_else(|| bad(cmd, format!("unknown level `{level}`")))?;
    if !wall.spans_level(level_def, &doc.arch.levels) {
        return Err(bad(cmd, format!("level `{level}` does not fall within wall `{}`'s span", wall_label(wall_id))));
    }
    let device = ElectricalDevice { id: ArchId(0), kind, wall: wall_id, side, offset, height, counter, level, ..Default::default() };
    let place = place_device(&doc.arch, &device).map_err(|e| bad(cmd, e))?;
    Ok((device, place))
}

fn reply_device(cmd: &str, doc: &cadcraft_doc::Drawing, device: &ElectricalDevice, place: &cadcraft_arch::electrical::DevicePlace) -> Result<Value> {
    let _ = cmd;
    let id = doc.arch.electrical.iter().find(|d| d.id == device.id).ok_or_else(|| bad("electrical", "device was not added"))?;
    let room = device_room(&doc.arch, id).map(room_label).map(|label| Value::String(label)).unwrap_or(Value::Null);
    Ok(json!({
        "device": electrical_label(id.id),
        "handle": electrical_handle(id.id).hex(),
        "kind": id.kind.key(),
        "layer": id.kind.layer(),
        "at": [place.at.x, place.at.y],
        "z": place.z,
        "wall": wall_label(id.wall),
        "side": match id.side { WallSide::Left => "left", WallSide::Right => "right" },
        "offset": id.offset,
        "room": room,
    }))
}

fn run_device(s: &mut Session, cmd: &str, kind_allowed: &[ElectricalKind], allow_counter: bool, params: &Value) -> Result<Value> {
    let d = s.doc()?.clone();
    let wall = parse_wall_id(&d, params.get("wall").unwrap_or(&Value::Null), cmd)?;
    let side = parse_side(params.get("side").ok_or_else(|| bad(cmd, "`side` is required"))?, cmd)?;
    let kind = parse_kind(params.get("kind"), cmd, kind_allowed)?;
    let level = parse_level(&d, params.get("level"), wall, cmd)?;
    let height = parse_height(params.get("height"), cmd, kind.default_height())?;
    let mut off = None;
    if let Some(value) = params.get("offset") {
        off = Some(parse_offset(value, cmd)?);
    } else if let Some(value) = params.get("point") {
        let point = parse_point(value, cmd)?;
        let wall = d.arch.wall(wall).ok_or_else(|| bad(cmd, format!("wall `{}` does not exist", wall_label(wall))))?;
        let dir = (wall.end - wall.start).normalized();
        if dir == Vec2::ZERO {
            return Err(bad(cmd, "wall direction is zero"));
        }
        let delta = point - wall.start;
        let projected = delta.dot(dir);
        off = Some(projected);
    }
    let offset = off.ok_or_else(|| bad(cmd, "either `offset` or `point` is required"))?;
    if !allow_counter && params.get("counter").is_some() {
        return Err(bad(cmd, "`counter` is only valid for outlets"));
    }
    let counter = params.get("counter").and_then(Value::as_bool).unwrap_or(false);
    if !allow_counter && counter {
        return Err(bad(cmd, "`counter` is only valid for outlets"));
    }
    if !allow_counter && kind.is_switch() {
        // no-op
    }
    let device = ElectricalDevice { id: ArchId(0), kind, wall: wall, side, offset, height, counter, level, ..Default::default() };
    let place = place_device(&d.arch, &device).map_err(|e| bad(cmd, e))?;
    let mut doc = s.doc_mut()?;
    ensure_electrical_layers(&mut doc);
    let id = doc.arch.push_electrical(device.clone());
    let room = device_room(&doc.arch, &device).map(room_label).map(|label| Value::String(label)).unwrap_or(Value::Null);
    Ok(json!({
        "device": electrical_label(id),
        "handle": electrical_handle(id).hex(),
        "kind": device.kind.key(),
        "layer": device.kind.layer(),
        "at": [place.at.x, place.at.y],
        "z": place.z,
        "wall": wall_label(device.wall),
        "side": match device.side { WallSide::Left => "left", WallSide::Right => "right" },
        "offset": device.offset,
        "room": room,
    }))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("outlet", "Outlet", run_outlet).menu(&["Architecture", "Electrical", "Outlet"]).params(
            "{kind: duplex|gfci|240v, wall: \"w0\", offset?: m | point?: [x,y], side: left|right, height?: m, counter?: bool, level?: name|index}",
        ),
        CommandSpec::new("switch", "Switch", run_switch).menu(&["Architecture", "Electrical", "Switch"]).params(
            "{kind: single-pole|three-way|dimmer, wall: \"w0\", offset?: m | point?: [x,y], side: left|right, height?: m, level?: name|index}",
        ),
        CommandSpec::new("electrical.edit", "Edit Electrical", run_electrical_edit)
            .menu(&["Architecture", "Electrical", "Edit"])
            .params("{id: \"el0\", offset?: m, height?: m, side?: left|right, kind?: duplex|gfci|240v|single-pole|three-way|dimmer}"),
        CommandSpec::new("electrical.delete", "Delete Electrical", run_electrical_delete)
            .menu(&["Architecture", "Electrical", "Delete"])
            .params("{id: \"el0\"}"),
    ]
}

fn run_outlet(s: &mut Session, params: &Value) -> Result<Value> {
    run_device(s, "outlet", OUTLET_KINDS, true, params)
}

fn run_switch(s: &mut Session, params: &Value) -> Result<Value> {
    run_device(s, "switch", SWITCH_KINDS, false, params)
}

fn run_electrical_edit(s: &mut Session, params: &Value) -> Result<Value> {
    let cmd = "electrical.edit";
    let raw = params.get("id").and_then(Value::as_str).filter(|t| !t.trim().is_empty()).ok_or_else(|| bad(cmd, "`id` is required"))?;
    let id = parse_electrical_label(raw).ok_or_else(|| bad(cmd, format!("unknown electrical id `{raw}`")))?;
    let mut doc = s.doc_mut()?;
    let idx = doc.arch.electrical.iter().position(|d| d.id == id).ok_or_else(|| bad(cmd, format!("unknown electrical id `{raw}`")))?;
    let mut device = doc.arch.electrical[idx].clone();
    if let Some(v) = params.get("offset") {
        let offset = parse_offset(v, cmd)?;
        device.offset = offset;
    }
    if let Some(v) = params.get("height") {
        let height = parse_height(Some(v), cmd, device.height)?;
        device.height = height;
    }
    if let Some(v) = params.get("side") {
        device.side = parse_side(v, cmd)?;
    }
    if let Some(v) = params.get("kind") {
        let kind = ElectricalKind::parse(v.as_str().ok_or_else(|| bad(cmd, "`kind` must be a string"))?)
            .ok_or_else(|| bad(cmd, format!("unknown electrical kind `{}`", v)))?;
        if device.kind.is_switch() != kind.is_switch() {
            return Err(bad(cmd, "`kind` must keep the device in the same family (outlet or switch)"));
        }
        device.kind = kind;
    }
    let place = place_device(&doc.arch, &device).map_err(|e| bad(cmd, e))?;
    doc.arch.electrical[idx] = device.clone();
    let room = device_room(&doc.arch, &device).map(room_label).map(|label| Value::String(label)).unwrap_or(Value::Null);
    Ok(json!({
        "device": electrical_label(device.id),
        "handle": electrical_handle(device.id).hex(),
        "kind": device.kind.key(),
        "layer": device.kind.layer(),
        "at": [place.at.x, place.at.y],
        "z": place.z,
        "wall": wall_label(device.wall),
        "side": match device.side { WallSide::Left => "left", WallSide::Right => "right" },
        "offset": device.offset,
        "room": room,
    }))
}

fn run_electrical_delete(s: &mut Session, params: &Value) -> Result<Value> {
    let cmd = "electrical.delete";
    let raw = params.get("id").and_then(Value::as_str).filter(|t| !t.trim().is_empty()).ok_or_else(|| bad(cmd, "`id` is required"))?;
    let id = parse_electrical_label(raw).ok_or_else(|| bad(cmd, format!("unknown electrical id `{raw}`")))?;
    let mut doc = s.doc_mut()?;
    let idx = doc.arch.electrical.iter().position(|d| d.id == id).ok_or_else(|| bad(cmd, format!("unknown electrical id `{raw}`")))?;
    let is_switch = doc.arch.electrical[idx].kind.is_switch();
    if is_switch {
        doc.arch.connections.retain(|c| c.from != id);
    }
    doc.arch.electrical.remove(idx);
    Ok(json!({ "deleted": electrical_label(id) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::{ArchId, ArchModel, Drawing, Wall};
    use serde_json::json;

    fn wall(model: &mut ArchModel, id: u32, start: Vec2, end: Vec2) -> ArchId {
        model.push_wall(Wall { id: ArchId(id), start, end, thickness: 0.2, height: 2.4, ..Wall::default() })
    }

    #[test]
    fn outlet_places_on_the_wall_face_at_0_3() {
        let mut s = Session::empty();
        s.new_drawing(false);
        let wall_id = {
            let mut doc = s.doc_mut().unwrap();
            let w = wall(&mut doc.arch, 0, Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0));
            doc.arch.rooms.push(cadcraft_doc::Room {
                id: ArchId(0),
                level: 0,
                polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
                ..Default::default()
            });
            w
        };
        let r = s.execute("outlet", &json!({ "kind": "duplex", "wall": "w0", "offset": 1.5, "side": "left" })).unwrap();
        assert_eq!(r["kind"], "duplex");
        assert_eq!(r["device"], "el0");
        assert_eq!(r["layer"], "E-POWR");
        assert_eq!(r["wall"], "w0");
        assert_eq!(r["side"], "left");
        assert_eq!(r["offset"], 1.5);
        let at = &r["at"].as_array().unwrap();
        assert!((at[0].as_f64().unwrap() - 1.5).abs() < 1e-9);
        assert!((at[1].as_f64().unwrap() - 0.1).abs() < 1e-9);
        assert!((r["z"].as_f64().unwrap() - 0.3).abs() < 1e-9);
        let doc = s.doc().unwrap();
        assert_eq!(doc.arch.electrical[0].kind, ElectricalKind::Duplex);
        assert_eq!(doc.arch.electrical[0].wall, wall_id);
    }

    #[test]
    fn switch_defaults_to_1_2() {
        let mut s = Session::empty();
        s.new_drawing(false);
        let mut doc = s.doc_mut().unwrap();
        let w = wall(&mut doc.arch, 0, Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0));
        doc.arch.rooms.push(cadcraft_doc::Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
            ..Default::default()
        });
        let _ = w;
        drop(doc);
        let r = s.execute("switch", &json!({ "kind": "single-pole", "wall": "w0", "offset": 2.0, "side": "right" })).unwrap();
        assert_eq!(r["kind"], "single-pole");
        assert_eq!(r["layer"], "E-LITE");
        assert!((r["z"].as_f64().unwrap() - 1.2).abs() < 1e-9);
        assert_eq!(r["side"], "right");
    }

    #[test]
    fn counter_outlet_needs_a_counter() {
        let mut s = Session::empty();
        s.new_drawing(false);
        let mut doc = s.doc_mut().unwrap();
        let _ = wall(&mut doc.arch, 0, Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0));
        doc.arch.rooms.push(cadcraft_doc::Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
            ..Default::default()
        });
        drop(doc);
        assert!(s.execute("outlet", &json!({ "kind": "duplex", "wall": "w0", "offset": 1.0, "side": "left", "counter": true })).is_err());
    }

    #[test]
    fn edit_keeps_the_device_kind_family() {
        let mut s = Session::empty();
        s.new_drawing(false);
        let mut doc = s.doc_mut().unwrap();
        let _ = wall(&mut doc.arch, 0, Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0));
        doc.arch.rooms.push(cadcraft_doc::Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
            ..Default::default()
        });
        drop(doc);
        s.execute("outlet", &json!({ "kind": "duplex", "wall": "w0", "offset": 1.0, "side": "left" })).unwrap();
        s.execute("switch", &json!({ "kind": "single-pole", "wall": "w0", "offset": 4.0, "side": "right" })).unwrap();
        let outlet = s.execute("electrical.edit", &json!({ "id": "el0", "kind": "gfci" })).unwrap();
        assert_eq!(outlet["kind"], "gfci");
        let err = s.execute("electrical.edit", &json!({ "id": "el0", "kind": "single-pole" })).unwrap_err();
        assert!(err.to_string().contains("family"));
        let sw = s.execute("electrical.edit", &json!({ "id": "el1", "kind": "dimmer" })).unwrap();
        assert_eq!(sw["kind"], "dimmer");
    }

    #[test]
    fn deleting_a_switch_deletes_its_connections() {
        let mut s = Session::empty();
        s.new_drawing(false);
        let mut doc = s.doc_mut().unwrap();
        let _ = wall(&mut doc.arch, 0, Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0));
        doc.arch.rooms.push(cadcraft_doc::Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
            ..Default::default()
        });
        drop(doc);
        s.execute("switch", &json!({ "kind": "single-pole", "wall": "w0", "offset": 1.0, "side": "left" })).unwrap();
        let switch_id = ArchId(0);
        let mut doc = s.doc_mut().unwrap();
        let conn = cadcraft_doc::arch::Connection { id: ArchId(0), from: switch_id, to: vec![ArchId(0)], bulge: 0.5, ..Default::default() };
        doc.arch.connections.push(conn);
        doc.arch.connections.push(cadcraft_doc::arch::Connection {
            id: ArchId(1),
            from: ArchId(0),
            to: vec![ArchId(1)],
            bulge: -0.5,
            ..Default::default()
        });
        drop(doc);
        assert_eq!(s.execute("electrical.delete", &json!({ "id": "el0" })).unwrap()["deleted"], "el0");
        assert!(s.doc().unwrap().arch.connections.iter().all(|c| c.from != switch_id));
    }

    #[test]
    fn electrical_edits_keep_the_cache_exact() {
        use cadcraft_arch3d::{Scene3d, SceneCache};
        let mut s = Session::empty();
        s.new_drawing(false);
        let mut doc = s.doc_mut().unwrap();
        // add a wall and room for electrical placement
        let w0 = wall(&mut doc.arch, 0, Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0));
        doc.arch.rooms.push(cadcraft_doc::Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
            ..Default::default()
        });
        assert_eq!(w0, ArchId(0));
        drop(doc);
        s.execute("outlet", &json!({ "kind": "duplex", "wall": "w0", "offset": 1.5, "side": "left" })).unwrap();
        s.execute("switch", &json!({ "kind": "single-pole", "wall": "w0", "offset": 4.5, "side": "right" })).unwrap();
        let mut m = s.doc().unwrap().arch.clone();
        let (mut cache, _) = SceneCache::new(&m);
        let before = cache.scene().digest();
        m.electrical[0].offset = 2.3;
        m.electrical[1].height = 1.4;
        let stats = cache.update(&m);
        let rebuilt = Scene3d::build(&m);
        assert_eq!(cache.scene().digest(), rebuilt.digest(), "cache should exactly match the full build after electrical edits");
        assert!(stats.rebuilt > 0 || before == rebuilt.digest(), "cache did not update");
    }
}
