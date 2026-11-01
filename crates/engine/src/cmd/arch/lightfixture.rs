//! `lightfixture` and its edits: light fixtures from the catalogue, one at a point or an even
//! grid in a room (phase 10). Filled in by `p10-cmd-lightfixture`.

use cadcraft_arch::light::{catalogue, find_type, fixture_from_type, grid_positions, validate_fixture};
use cadcraft_doc::arch::{ArchId, LightFixture, LightKind, LightMount, MAX_FIXTURE_GRID, MAX_LIGHT_FIXTURES, handle_json, light_handle, light_label};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad, point_req, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("lightfixture", "Light Fixture", run_lightfixture).menu(&["Architecture", "Light Fixture"]).params(
            "{type?, at?: [x,y], room?: \"r0\", rows?, cols?, level?, height?, rotation?, tilt?, lumens?, kelvin?, cone?, falloff?, kind?, on?}",
        ),
        CommandSpec::new("lightfixture.edit", "Edit Light Fixture", run_lightfixture_edit)
            .menu(&["Architecture", "Edit Light Fixture"])
            .params("{id: \"lf0\", at?, height?, rotation?, tilt?, lumens?, kelvin?, cone?, falloff?, kind?, on?}"),
        CommandSpec::new("lightfixture.delete", "Delete Light Fixture", run_lightfixture_delete)
            .menu(&["Architecture", "Delete Light Fixture"])
            .params("{id}"),
        CommandSpec::new("lightfixture.types", "Light Fixture Types", run_lightfixture_types)
            .menu(&["Architecture", "Light Fixture Types"])
            .noundo()
            .params("{}"),
    ]
}

fn parse_light_id(label: &str) -> Option<ArchId> {
    let text = label.trim();
    if text.is_empty() {
        return None;
    }
    let digits = text
        .strip_prefix("lf")
        .or_else(|| text.strip_prefix("LF"))
        .or_else(|| text.strip_prefix("Lf"))
        .or_else(|| text.strip_prefix("lF"))
        .or_else(|| text.strip_prefix('l'))
        .or_else(|| text.strip_prefix('L'))
        .unwrap_or(text);
    digits.parse::<u32>().ok().map(ArchId)
}

fn parse_room_id(label: &str) -> Option<ArchId> {
    let text = label.trim();
    if text.is_empty() {
        return None;
    }
    let digits = text.strip_prefix('r').or_else(|| text.strip_prefix('R')).unwrap_or(text);
    digits.parse::<u32>().ok().map(ArchId)
}

fn parse_kind_name(raw: &str) -> Result<LightKind> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "point" => Ok(LightKind::Point),
        "spot" => Ok(LightKind::Spot),
        _ => Err(bad("lightfixture", format!("`kind` must be \"point\" or \"spot\"; got `{raw}`"))),
    }
}

fn kind_name(kind: LightKind) -> &'static str {
    match kind {
        LightKind::Point => "point",
        LightKind::Spot => "spot",
    }
}

fn mount_name(mount: LightMount) -> &'static str {
    match mount {
        LightMount::Ceiling => "ceiling",
        LightMount::Wall => "wall",
        LightMount::Floor => "floor",
    }
}

fn parse_u32_value(value: &Value, key: &str, cmd: &str, min: u32, max: u32) -> Result<u32> {
    let n = value.as_u64().filter(|n| *n >= u64::from(min) && *n <= u64::from(max));
    if let Some(n) = n {
        return Ok(n as u32);
    }
    Err(bad(cmd, format!("`{key}` must be a whole number between {min} and {max}")))
}

fn resolve_level(doc: &cadcraft_doc::Drawing, value: Option<&Value>) -> Result<u32> {
    let fallback = doc.arch.levels.first().map(|l| l.index).unwrap_or(0);
    let Some(value) = value.filter(|v| !v.is_null()) else {
        return Ok(fallback);
    };

    if let Some(n) = value.as_u64() {
        let level = n as u32;
        if doc.arch.levels.iter().any(|l| l.index == level) || doc.arch.levels.is_empty() {
            return Ok(level);
        }
    }

    if let Some(s) = value.as_str() {
        let trimmed = s.trim();
        if let Ok(level) = trimmed.parse::<u32>() {
            return Ok(level);
        }
        let level = doc.arch.levels.iter().find(|l| l.name == trimmed).map(|l| l.index);
        if let Some(level) = level {
            return Ok(level);
        }
    }

    Err(bad("lightfixture", format!("unknown level `{}`", value)))
}

fn apply_overrides(fixture: &mut LightFixture, params: &Value) -> Result<()> {
    if let Some(value) = params.get("height") {
        if value.is_null() {
            fixture.height = None;
        } else {
            let parsed = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad("lightfixture", "`height` must be a finite number"))?;
            fixture.height = Some(parsed);
        }
    }
    if let Some(value) = params.get("rotation") {
        fixture.rotation = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad("lightfixture", "`rotation` must be a finite number"))?;
    }
    if let Some(value) = params.get("tilt") {
        fixture.tilt = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad("lightfixture", "`tilt` must be a finite number"))?;
    }
    if let Some(value) = params.get("lumens") {
        fixture.lumens = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad("lightfixture", "`lumens` must be a finite number"))?;
    }
    if let Some(value) = params.get("kelvin") {
        fixture.kelvin = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad("lightfixture", "`kelvin` must be a finite number"))?;
    }
    if let Some(value) = params.get("cone") {
        fixture.cone = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad("lightfixture", "`cone` must be a finite number"))?;
    }
    if let Some(value) = params.get("falloff") {
        fixture.falloff = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad("lightfixture", "`falloff` must be a finite number"))?;
    }
    if let Some(value) = params.get("kind") {
        let raw = value.as_str().ok_or_else(|| bad("lightfixture", "`kind` must be a string"))?;
        fixture.kind = parse_kind_name(raw)?;
    }
    if let Some(value) = params.get("on") {
        fixture.on = value.as_bool().ok_or_else(|| bad("lightfixture", "`on` must be a boolean"))?;
    }
    Ok(())
}

fn fixture_from_params(doc: &cadcraft_doc::Drawing, position: Vec2, level: u32, params: &Value) -> Result<LightFixture> {
    let typ = str_param(params, "type").unwrap_or("Recessed pot light");
    let Some(t) = find_type(typ) else {
        return Err(bad("lightfixture", format!("unknown fixture type `{typ}`; lightfixture.types lists them")));
    };
    let id = ArchId(doc.arch.next_light_id);
    let mut fixture = fixture_from_type(id, &t, position);
    fixture.level = level;
    apply_overrides(&mut fixture, params)?;
    validate_fixture(&fixture).map_err(|msg| bad("lightfixture", msg))?;
    Ok(fixture)
}

fn grid_like(doc: &cadcraft_doc::Drawing, room_id: ArchId, rows: u32, cols: u32, params: &Value) -> Result<Vec<LightFixture>> {
    let room = doc.arch.rooms.iter().find(|r| r.id == room_id).ok_or_else(|| bad("lightfixture", format!("unknown room `r{room_id:?}`")))?;
    let positions = grid_positions(&room.polygon, rows, cols);
    if positions.is_empty() {
        return Ok(Vec::new());
    }

    let mut fixtures = Vec::with_capacity(positions.len());
    let mut next_id = doc.arch.next_light_id;
    for pos in positions {
        let mut fixture = fixture_from_params(doc, pos, room.level, params)?;
        if fixture.id.0 != next_id {
            fixture.id = ArchId(next_id);
        }
        validate_fixture(&fixture).map_err(|msg| bad("lightfixture", msg))?;
        fixtures.push(fixture);
        next_id = next_id.saturating_add(1);
    }
    Ok(fixtures)
}

fn run_lightfixture(session: &mut Session, params: &Value) -> Result<Value> {
    let doc = session.doc()?;
    let has_at = params.get("at").is_some_and(|v| !v.is_null());
    let has_room = params.get("room").is_some_and(|v| !v.is_null());
    if has_at == has_room {
        return Err(bad("lightfixture", "exactly one of `at` or `room` is required"));
    }

    if has_at {
        let at = point_req("lightfixture", params, "at")?;
        let level = resolve_level(&doc, params.get("level"))?;
        let id = ArchId(doc.arch.next_light_id);
        let mut fixture = fixture_from_params(&doc, at, level, params)?;
        let mut doc_mut = session.doc_mut()?;
        if doc_mut.arch.lights.len() + 1 > MAX_LIGHT_FIXTURES {
            return Err(bad("lightfixture", format!("at most {} light fixtures are allowed", MAX_LIGHT_FIXTURES)));
        }
        fixture.id = id;
        doc_mut.ensure_layer("E-LITE");
        doc_mut.arch.lights.push(fixture);
        doc_mut.arch.next_light_id = doc_mut.arch.next_light_id.saturating_add(1);
        doc_mut.touch_arch();
        let handle = handle_json(light_handle(id));
        return Ok(json!({ "lights": [light_label(id)], "handles": [handle] }));
    }

    let room_label = str_param(params, "room").ok_or_else(|| bad("lightfixture", "`room` is required"))?;
    let room_id = parse_room_id(room_label).ok_or_else(|| bad("lightfixture", format!("unknown room `{room_label}`")))?;
    if !doc.arch.rooms.iter().any(|r| r.id == room_id) {
        return Err(bad("lightfixture", format!("unknown room `{room_label}`")));
    }
    let rows = match params.get("rows") {
        None | Some(Value::Null) => 2,
        Some(v) => parse_u32_value(v, "rows", "lightfixture", 1, MAX_FIXTURE_GRID)?,
    };
    let cols = match params.get("cols") {
        None | Some(Value::Null) => 2,
        Some(v) => parse_u32_value(v, "cols", "lightfixture", 1, MAX_FIXTURE_GRID)?,
    };

    let mut fixtures = Vec::new();
    let mut next_id = doc.arch.next_light_id;
    for pos in grid_positions(&doc.arch.rooms.iter().find(|r| r.id == room_id).unwrap().polygon, rows, cols) {
        let mut fixture = fixture_from_params(&doc, pos, doc.arch.rooms.iter().find(|r| r.id == room_id).unwrap().level, params)?;
        fixture.id = ArchId(next_id);
        validate_fixture(&fixture).map_err(|msg| bad("lightfixture", msg))?;
        fixtures.push(fixture);
        next_id = next_id.saturating_add(1);
    }

    if fixtures.len() + doc.arch.lights.len() > MAX_LIGHT_FIXTURES {
        return Err(bad("lightfixture", format!("at most {} light fixtures are allowed", MAX_LIGHT_FIXTURES)));
    }

    let mut doc_mut = session.doc_mut()?;
    doc_mut.ensure_layer("E-LITE");
    let labels = fixtures.iter().map(|f| light_label(f.id)).collect::<Vec<_>>();
    let handles = fixtures.iter().map(|f| handle_json(light_handle(f.id))).collect::<Vec<_>>();
    doc_mut.arch.lights.extend(fixtures);
    doc_mut.arch.next_light_id = next_id;
    doc_mut.touch_arch();
    Ok(json!({ "lights": labels, "handles": handles }))
}

fn run_lightfixture_edit(session: &mut Session, params: &Value) -> Result<Value> {
    let raw = str_param(params, "id").ok_or_else(|| bad("lightfixture.edit", "`id` is required"))?;
    let id = parse_light_id(raw).ok_or_else(|| bad("lightfixture.edit", format!("unknown light `{raw}`")))?;
    let mut doc = session.doc_mut()?;
    let idx = doc.arch.lights.iter().position(|f| f.id == id).ok_or_else(|| bad("lightfixture.edit", format!("unknown light `{raw}`")))?;
    let mut fixture = doc.arch.lights[idx].clone();
    if let Some(at) = params.get("at") {
        if !at.is_null() {
            fixture.position = point_req("lightfixture.edit", params, "at")?;
        }
    }
    apply_overrides(&mut fixture, params)?;
    validate_fixture(&fixture).map_err(|msg| bad("lightfixture.edit", msg))?;
    doc.arch.lights[idx] = fixture;
    doc.touch_arch();
    Ok(json!({ "light": light_label(id) }))
}

fn run_lightfixture_delete(session: &mut Session, params: &Value) -> Result<Value> {
    let raw = str_param(params, "id").ok_or_else(|| bad("lightfixture.delete", "`id` is required"))?;
    let id = parse_light_id(raw).ok_or_else(|| bad("lightfixture.delete", format!("unknown light `{raw}`")))?;
    let mut doc = session.doc_mut()?;
    let idx = doc.arch.lights.iter().position(|f| f.id == id).ok_or_else(|| bad("lightfixture.delete", format!("unknown light `{raw}`")))?;
    doc.arch.lights.remove(idx);
    doc.touch_arch();
    Ok(json!({ "light": light_label(id) }))
}

fn run_lightfixture_types(_: &mut Session, _: &Value) -> Result<Value> {
    let list = catalogue()
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "shape": t.shape.name(),
                "kind": kind_name(t.kind),
                "mount": mount_name(t.mount),
                "lumens": t.lumens,
                "kelvin": t.kelvin,
                "cone": t.cone,
                "falloff": t.falloff,
                "diameter": t.diameter,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "types": list }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchId, Room, RoomType};
    use cadcraft_geom::Vec2;
    use serde_json::json;

    #[test]
    fn lightfixture_places_one_at_a_point() {
        let mut session = Session::new();
        let out = session.execute("lightfixture", &json!({"at": [1.5, 2.0], "type": "Recessed pot light"})).unwrap();
        assert_eq!(out["lights"][0], "lf0");
        assert_eq!(out["handles"][0], "a0048000");
        assert_eq!(session.doc().unwrap().arch.lights.len(), 1);
        assert_eq!(session.doc().unwrap().arch.lights[0].position, Vec2::new(1.5, 2.0));
    }

    #[test]
    fn lightfixture_lays_a_two_by_two_grid_in_a_room() {
        let mut session = Session::new();
        let room = Room {
            id: ArchId(0),
            name: "Room 1".into(),
            room_type: RoomType::Living,
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
            ..Room::default()
        };
        session.doc_mut().unwrap().arch.push_room(room);
        let out = session.execute("lightfixture", &json!({"room": "r0", "rows": 2, "cols": 2})).unwrap();
        assert_eq!(out["lights"].as_array().unwrap().len(), 4);
        let positions = session.doc().unwrap().arch.lights.iter().map(|l| l.position).collect::<Vec<_>>();
        assert!(positions.iter().any(|p| p.x == 1.0 && p.y == 1.0));
        assert!(positions.iter().any(|p| p.x == 3.0 && p.y == 1.0));
        assert!(positions.iter().any(|p| p.x == 1.0 && p.y == 3.0));
        assert!(positions.iter().any(|p| p.x == 3.0 && p.y == 3.0));
        for fixture in &session.doc().unwrap().arch.lights {
            assert_eq!(fixture.level, 0);
        }
    }

    #[test]
    fn lightfixture_refuses_bad_values_and_places_nothing() {
        let mut session = Session::new();
        let room = Room {
            id: ArchId(0),
            name: "Room 1".into(),
            room_type: RoomType::Living,
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
            ..Room::default()
        };
        session.doc_mut().unwrap().arch.push_room(room);
        assert!(session.execute("lightfixture", &json!({"room": "r0", "rows": 2, "cols": 2, "lumens": -1.0})).is_err());
        assert!(session.execute("lightfixture", &json!({"room": "r99", "rows": 2, "cols": 2})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_edit_moves_and_changes_one() {
        let mut session = Session::new();
        session.execute("lightfixture", &json!({"at": [1.0, 1.0], "lumens": 800.0})).unwrap();
        session.execute("lightfixture.edit", &json!({"id": "lf0", "at": [2.5, 3.5], "lumens": 1200.0, "kind": "point"})).unwrap();
        let f = &session.doc().unwrap().arch.lights[0];
        assert_eq!(f.id, ArchId(0));
        assert_eq!(f.position, Vec2::new(2.5, 3.5));
        assert_eq!(f.lumens, 1200.0);
        assert_eq!(f.kind, LightKind::Point);
    }

    #[test]
    fn lightfixture_delete_removes_it() {
        let mut session = Session::new();
        session.execute("lightfixture", &json!({"at": [1.0, 1.0]})).unwrap();
        let out = session.execute("lightfixture.delete", &json!({"id": "lf0"})).unwrap();
        assert_eq!(out["light"], "lf0");
        assert!(session.doc().unwrap().arch.lights.is_empty());
    }

    #[test]
    fn lightfixture_types_lists_the_catalogue() {
        let mut session = Session::new();
        let out = session.execute("lightfixture.types", &json!({})).unwrap();
        let types = out["types"].as_array().unwrap();
        assert_eq!(types.len(), 5);
        assert_eq!(types[0]["name"], "Recessed pot light");
        assert_eq!(types[0]["shape"], "recessed-can");
    }

    #[test]
    fn lightfixture_refuses_non_number_lumens() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"at": [1.0, 1.0], "lumens": "high"})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_refuses_lumens_above_max() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"at": [1.0, 1.0], "lumens": 200001.0})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_refuses_kelvin_below_min() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"at": [1.0, 1.0], "kelvin": 999.0})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_refuses_kelvin_above_max() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"at": [1.0, 1.0], "kelvin": 12001.0})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_refuses_cone_above_max() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"at": [1.0, 1.0], "cone": 171.0})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_refuses_falloff_below_cone() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"at": [1.0, 1.0], "cone": 60.0, "falloff": 50.0})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_refuses_negative_tilt() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"at": [1.0, 1.0], "tilt": -1.0})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_refuses_tilt_above_90() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"at": [1.0, 1.0], "tilt": 91.0})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_refuses_zero_rows() {
        let mut session = Session::new();
        let room = Room {
            id: ArchId(0),
            name: "Room 1".into(),
            room_type: RoomType::Living,
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
            ..Room::default()
        };
        session.doc_mut().unwrap().arch.push_room(room);
        assert!(session.execute("lightfixture", &json!({"room": "r0", "rows": 0, "cols": 2})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_refuses_rows_above_max() {
        let mut session = Session::new();
        let room = Room {
            id: ArchId(0),
            name: "Room 1".into(),
            room_type: RoomType::Living,
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
            ..Room::default()
        };
        session.doc_mut().unwrap().arch.push_room(room);
        assert!(session.execute("lightfixture", &json!({"room": "r0", "rows": 33, "cols": 2})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_edit_refuses_invalid() {
        let mut session = Session::new();
        session.execute("lightfixture", &json!({"at": [1.0, 1.0]})).unwrap();
        assert!(session.execute("lightfixture.edit", &json!({"id": "lf0", "lumens": -1.0})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights[0].lumens, 800.0);
    }

    #[test]
    fn lightfixture_delete_unknown() {
        let mut session = Session::new();
        session.execute("lightfixture", &json!({"at": [1.0, 1.0]})).unwrap();
        assert!(session.execute("lightfixture.delete", &json!({"id": "lf99"})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 1);
    }

    #[test]
    fn lightfixture_refuses_unknown_type() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"at": [1.0, 1.0], "type": "Chandelier"})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_ensures_elite_layer() {
        let mut session = Session::new();
        session.execute("lightfixture", &json!({"at": [1.0, 1.0]})).unwrap();
        assert!(session.doc().unwrap().layer("E-LITE").is_some());
    }

    #[test]
    fn lightfixture_refuses_both_at_and_room() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"at": [1.0, 1.0], "room": "r0"})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_refuses_invalid_at() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"at": [1.0]})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }
}
