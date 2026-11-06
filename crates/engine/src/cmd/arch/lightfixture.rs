//! `lightfixture` and its edits: light fixtures from the catalogue, one at a point or an even
//! grid in a room (phase 10). Since phase 11 a fixture may be an area light (`kind: "area"` with a
//! `width` and `length`, like the catalogue's `LED panel 600`) and may carry an IES photometric
//! file (`ies: "path.ies"`, `ies: null` clears it).

use cadcraft_arch::ies;
use cadcraft_arch::light::{catalogue, find_type, fixture_from_type, grid_positions, validate_fixture};
use cadcraft_doc::arch::{
    ArchId, IesProfile, LightFixture, LightKind, LightMount, MAX_AREA_SIDE, MAX_FIXTURE_GRID, MAX_IES_BYTES, MAX_LIGHT_FIXTURES, MIN_AREA_SIDE,
    handle_json, light_handle, light_label,
};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad, point_req, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("lightfixture", "Light Fixture", run_lightfixture).menu(&["Architecture", "Light Fixture"]).params(
            "{type?, at?: [x,y], room?: \"r0\", rows?, cols?, level?, height?, rotation?, tilt?, lumens?, kelvin?, cone?, falloff?, kind?: \"point\"|\"spot\"|\"area\", width?, length?, ies?: \"file.ies\"|null, on?}",
        ),
        CommandSpec::new("lightfixture.edit", "Edit Light Fixture", run_lightfixture_edit)
            .menu(&["Architecture", "Edit Light Fixture"])
            .params("{id: \"lf0\", at?, height?, rotation?, tilt?, lumens?, kelvin?, cone?, falloff?, kind?, width?, length?, ies?: \"file.ies\"|null, on?}"),
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

fn parse_kind_name(cmd: &str, raw: &str) -> Result<LightKind> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "point" => Ok(LightKind::Point),
        "spot" => Ok(LightKind::Spot),
        "area" => Ok(LightKind::Area),
        _ => Err(bad(cmd, format!("`kind` must be \"point\", \"spot\" or \"area\"; got `{raw}`"))),
    }
}

fn kind_name(kind: LightKind) -> &'static str {
    match kind {
        LightKind::Point => "point",
        LightKind::Spot => "spot",
        LightKind::Area => "area",
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

fn apply_overrides(cmd: &str, fixture: &mut LightFixture, params: &Value) -> Result<()> {
    if let Some(value) = params.get("height") {
        if value.is_null() {
            fixture.height = None;
        } else {
            let parsed = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`height` must be a finite number"))?;
            fixture.height = Some(parsed);
        }
    }
    if let Some(value) = params.get("rotation") {
        fixture.rotation = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`rotation` must be a finite number"))?;
    }
    if let Some(value) = params.get("tilt") {
        let tilt = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`tilt` must be a finite number"))?;
        if tilt < 0.0 || tilt > 90.0 {
            return Err(bad(cmd, "`tilt` must be between 0 and 90 degrees"));
        }
        fixture.tilt = tilt;
    }
    if let Some(value) = params.get("lumens") {
        fixture.lumens = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`lumens` must be a finite number"))?;
    }
    if let Some(value) = params.get("kelvin") {
        fixture.kelvin = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`kelvin` must be a finite number"))?;
    }
    if let Some(value) = params.get("cone") {
        fixture.cone = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`cone` must be a finite number"))?;
    }
    if let Some(value) = params.get("falloff") {
        fixture.falloff = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`falloff` must be a finite number"))?;
    }
    if let Some(value) = params.get("kind") {
        let raw = value.as_str().ok_or_else(|| bad(cmd, "`kind` must be a string"))?;
        fixture.kind = parse_kind_name(cmd, raw)?;
    }
    // An area's size (m). The range is checked with the rest of the fixture, by `check_fixture`.
    if let Some(value) = params.get("width") {
        fixture.width = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`width` must be a finite number"))?;
    }
    if let Some(value) = params.get("length") {
        fixture.length = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`length` must be a finite number"))?;
    }
    if let Some(value) = params.get("on") {
        fixture.on = value.as_bool().ok_or_else(|| bad(cmd, "`on` must be a boolean"))?;
    }
    Ok(())
}

/// What an `ies` parameter asks of the fixture(s): nothing, to drop the profile, or to take the
/// one read from the file.
enum IesChange {
    Keep,
    Clear,
    Set(IesProfile),
}

/// Read an IES file (`path`) into a profile. A file over [`MAX_IES_BYTES`] is refused from its
/// size before anything is read; the read itself is capped too and takes a regular file only
/// (never an endless device). Text that is not UTF-8 (older files carry Latin-1 characters in their
/// keywords) is read lossily: only the keyword text can change, the numbers are ASCII. A refusal
/// by the parser carries its line (`IES line 7: ...`).
#[cfg(not(target_arch = "wasm32"))]
fn read_ies_file(cmd: &str, path: &str) -> Result<IesProfile> {
    let unreadable = |e: std::io::Error| bad(cmd, format!("could not read `{path}`: {e}"));
    let too_big = || bad(cmd, format!("IES: file over {MAX_IES_BYTES} bytes"));
    let meta = std::fs::metadata(path).map_err(unreadable)?;
    if meta.len() > MAX_IES_BYTES as u64 {
        return Err(too_big());
    }
    let bytes = crate::capped_read::read_capped(std::path::Path::new(path), MAX_IES_BYTES).map_err(|e| {
        // The file grew past the limit between the size check and the read.
        if e.kind() == std::io::ErrorKind::InvalidData { too_big() } else { unreadable(e) }
    })?;
    let text = String::from_utf8_lossy(&bytes);
    let file_name = std::path::Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path);
    ies::parse(&text, file_name).map_err(|e| bad(cmd, e.to_string()))
}

#[cfg(target_arch = "wasm32")]
fn read_ies_file(cmd: &str, _path: &str) -> Result<IesProfile> {
    Err(bad(cmd, "IES files are not available in the web build"))
}

/// The `ies` parameter: absent keeps the fixture's profile, `null` clears it, a path string reads
/// that file (once, however many fixtures a grid lays).
fn read_ies_param(cmd: &str, params: &Value) -> Result<IesChange> {
    match params.get("ies") {
        None => Ok(IesChange::Keep),
        Some(Value::Null) => Ok(IesChange::Clear),
        Some(value) => {
            let path = value.as_str().map(str::trim).filter(|p| !p.is_empty()).ok_or_else(|| bad(cmd, "`ies` must be a path string or null"))?;
            Ok(IesChange::Set(read_ies_file(cmd, path)?))
        }
    }
}

/// Give `fixture` the profile `change` asks for. A new profile also sets the fixture's lumens to
/// the file's rated lumens (when the file states them: an absolute file has none), unless the
/// same call gives `lumens` itself.
fn apply_ies(fixture: &mut LightFixture, change: &IesChange, params: &Value) {
    match change {
        IesChange::Keep => {}
        IesChange::Clear => fixture.ies = None,
        IesChange::Set(profile) => {
            if profile.lumens > 0.0 && params.get("lumens").is_none() {
                fixture.lumens = profile.lumens;
            }
            fixture.ies = Some(profile.clone());
        }
    }
}

/// An area light's `width` then `length` must lie within `MIN_AREA_SIDE..=MAX_AREA_SIDE` (a point
/// or spot light ignores them). `validate_fixture` refuses the same with the same words; the
/// command checks it first so that it never depends on which version of the arch crate it is
/// built with.
fn check_area_sides(fixture: &LightFixture) -> std::result::Result<(), String> {
    if fixture.kind != LightKind::Area {
        return Ok(());
    }
    for (name, side) in [("width", fixture.width), ("length", fixture.length)] {
        if !side.is_finite() || side < MIN_AREA_SIDE || side > MAX_AREA_SIDE {
            return Err(format!("{name} must be {MIN_AREA_SIDE}..={MAX_AREA_SIDE}, got {side}"));
        }
    }
    Ok(())
}

/// Every change a command makes to a fixture goes through here: the message names the field.
fn check_fixture(cmd: &str, fixture: &LightFixture) -> Result<()> {
    check_area_sides(fixture).map_err(|msg| bad(cmd, msg))?;
    validate_fixture(fixture).map_err(|msg| bad(cmd, msg))
}

fn fixture_from_params(doc: &cadcraft_doc::Drawing, position: Vec2, level: u32, params: &Value, ies: &IesChange) -> Result<LightFixture> {
    let typ = str_param(params, "type").unwrap_or("Recessed pot light");
    let Some(t) = find_type(typ) else {
        return Err(bad("lightfixture", format!("unknown fixture type `{typ}`; lightfixture.types lists them")));
    };
    let id = ArchId(doc.arch.next_light_id);
    let mut fixture = fixture_from_type(id, &t, position);
    fixture.level = level;
    apply_overrides("lightfixture", &mut fixture, params)?;
    apply_ies(&mut fixture, ies, params);
    check_fixture("lightfixture", &fixture)?;
    Ok(fixture)
}

fn grid_like(doc: &cadcraft_doc::Drawing, room_id: ArchId, rows: u32, cols: u32, params: &Value) -> Result<Vec<LightFixture>> {
    let room = doc.arch.rooms.iter().find(|r| r.id == room_id).ok_or_else(|| bad("lightfixture", format!("unknown room `r{room_id:?}`")))?;
    let positions = grid_positions(&room.polygon, rows, cols);
    if positions.is_empty() {
        return Ok(Vec::new());
    }
    let ies = read_ies_param("lightfixture", params)?;

    let mut fixtures = Vec::with_capacity(positions.len());
    let mut next_id = doc.arch.next_light_id;
    for pos in positions {
        let mut fixture = fixture_from_params(doc, pos, room.level, params, &ies)?;
        if fixture.id.0 != next_id {
            fixture.id = ArchId(next_id);
        }
        check_fixture("lightfixture", &fixture)?;
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
        let ies = read_ies_param("lightfixture", params)?;
        let mut fixture = fixture_from_params(&doc, at, level, params, &ies)?;
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

    let ies = read_ies_param("lightfixture", params)?;
    let mut fixtures = Vec::new();
    let mut next_id = doc.arch.next_light_id;
    for pos in grid_positions(&doc.arch.rooms.iter().find(|r| r.id == room_id).unwrap().polygon, rows, cols) {
        let mut fixture = fixture_from_params(&doc, pos, doc.arch.rooms.iter().find(|r| r.id == room_id).unwrap().level, params, &ies)?;
        fixture.id = ArchId(next_id);
        check_fixture("lightfixture", &fixture)?;
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
    let ies = read_ies_param("lightfixture.edit", params)?;
    let mut doc = session.doc_mut()?;
    let idx = doc.arch.lights.iter().position(|f| f.id == id).ok_or_else(|| bad("lightfixture.edit", format!("unknown light `{raw}`")))?;
    let mut fixture = doc.arch.lights[idx].clone();
    if let Some(at) = params.get("at") {
        if !at.is_null() {
            fixture.position = point_req("lightfixture.edit", params, "at")?;
        }
    }
    apply_overrides("lightfixture.edit", &mut fixture, params)?;
    apply_ies(&mut fixture, &ies, params);
    check_fixture("lightfixture.edit", &fixture)?;
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
    doc.arch.connections.retain_mut(|c| {
        c.to.retain(|light| *light != id);
        !c.to.is_empty()
    });
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
                "width": t.width,
                "length": t.length,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "types": list }))
}

/// The IES file the phase-11 acceptance checks write: a 1000 lm downlight, 1000 cd straight down
/// and 900 cd at 22.5 degrees. Shared with the `arch.inspect` tests.
#[cfg(test)]
pub(super) const TEST_DOWNLIGHT_IES: &str = "IESNA:LM-63-2002\n[TEST] cadcraft phase-11 test downlight\n[LUMINAIRE] Test downlight 1000 lm\nTILT=NONE\n1 1000 1 5 1 1 2 0.15 0.15 0.1\n1.0 1.0 12\n0 22.5 45 67.5 90\n0\n1000 900 600 200 0\n";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temp_dir::TempDir;
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
        assert_eq!(types.len(), 6);
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

    #[test]
    fn lightfixture_refuses_nan_lumens() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"at": [1.0, 1.0], "lumens": serde_json::Value::Null})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_refuses_huge_lumens() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"at": [1.0, 1.0], "lumens": 1e308})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_refuses_at_cap() {
        let mut session = Session::new();
        for i in 0..1024 {
            let out = session.execute("lightfixture", &json!({"at": [f64::from(i % 10), f64::from(i / 10)]}));
            assert!(out.is_ok(), "fixture {i} failed: {:?}", out.err());
        }
        assert_eq!(session.doc().unwrap().arch.lights.len(), 1024);
        assert!(session.execute("lightfixture", &json!({"at": [1.0, 1.0]})).is_err());
    }

    #[test]
    fn lightfixture_edit_refuses_unknown_id() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture.edit", &json!({"id": "lf99"})).is_err());
    }

    #[test]
    fn lightfixture_grid_with_one_row_and_col() {
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
        let out = session.execute("lightfixture", &json!({"room": "r0", "rows": 1, "cols": 1})).unwrap();
        assert_eq!(out["lights"].as_array().unwrap().len(), 1);
    }

    fn square_room() -> Room {
        Room {
            id: ArchId(0),
            name: "Room 1".into(),
            room_type: RoomType::Living,
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
            ..Room::default()
        }
    }

    fn error_text(session: &mut Session, cmd: &str, params: &Value) -> String {
        session.execute(cmd, params).expect_err("must be refused").to_string()
    }

    #[test]
    fn lightfixture_places_the_led_panel_as_an_area_light() {
        let mut session = Session::new();
        let out = session.execute("lightfixture", &json!({"type": "LED panel 600", "at": [2.0, 2.0]})).unwrap();
        assert_eq!(out["lights"][0], "lf0");
        {
            let doc = session.doc().unwrap();
            let f = &doc.arch.lights[0];
            assert_eq!((f.kind, f.shape, f.mount), (LightKind::Area, cadcraft_doc::arch::FixtureShape::Panel, LightMount::Ceiling));
            assert_eq!((f.width, f.length), (0.6, 0.6));
            assert_eq!((f.lumens, f.kelvin), (3600.0, 4000.0));
            assert_eq!(f.ies, None);
        }

        // Any fixture becomes an area by `kind` and a size; the size can be set and changed later.
        session.execute("lightfixture", &json!({"at": [1.0, 1.0], "kind": "Area", "width": 1.2, "length": 0.3})).unwrap();
        {
            let doc = session.doc().unwrap();
            assert_eq!((doc.arch.lights[1].kind, doc.arch.lights[1].width, doc.arch.lights[1].length), (LightKind::Area, 1.2, 0.3));
        }
        session.execute("lightfixture.edit", &json!({"id": "lf0", "width": 1.0, "length": 0.5})).unwrap();
        let doc = session.doc().unwrap();
        assert_eq!((doc.arch.lights[0].width, doc.arch.lights[0].length), (1.0, 0.5));

        // A grid of panels in a room: every one is an area light.
        session.doc_mut().unwrap().arch.push_room(square_room());
        let grid = session.execute("lightfixture", &json!({"type": "LED panel 600", "room": "r0", "rows": 1, "cols": 2})).unwrap();
        assert_eq!(grid["lights"].as_array().unwrap().len(), 2);
        let doc = session.doc().unwrap();
        assert!(doc.arch.lights[3..].iter().all(|f| f.kind == LightKind::Area && f.width == 0.6 && f.length == 0.6));

        // The catalogue lists each type's size: the panel's 0.6 x 0.6, the others' none.
        let types = session.execute("lightfixture.types", &json!({})).unwrap();
        let types = types["types"].as_array().unwrap();
        let panel = types.iter().find(|t| t["name"] == "LED panel 600").unwrap();
        assert_eq!((panel["kind"].as_str(), panel["shape"].as_str()), (Some("area"), Some("panel")));
        assert_eq!((panel["width"].as_f64(), panel["length"].as_f64()), (Some(0.6), Some(0.6)));
        let pot = types.iter().find(|t| t["name"] == "Recessed pot light").unwrap();
        assert_eq!((pot["width"].as_f64(), pot["length"].as_f64()), (Some(0.0), Some(0.0)));
    }

    #[test]
    fn lightfixture_edit_reads_an_ies_file_and_takes_its_lumens() {
        let dir = TempDir::new("ies-edit");
        let file = dir.file("downlight.ies", TEST_DOWNLIGHT_IES);
        let mut session = Session::new();
        session.execute("lightfixture", &json!({"type": "Recessed pot light", "at": [1.0, 1.0]})).unwrap();
        assert_eq!(session.doc().unwrap().arch.lights[0].lumens, 800.0);

        session.execute("lightfixture.edit", &json!({"id": "lf0", "ies": file})).unwrap();
        {
            let doc = session.doc().unwrap();
            let f = &doc.arch.lights[0];
            let profile = f.ies.as_ref().expect("the profile is set");
            assert_eq!(profile.name, "Test downlight 1000 lm");
            assert_eq!((profile.vertical.len(), profile.horizontal.len(), profile.candela.len()), (5, 1, 5));
            assert_eq!(profile.lumens, 1000.0);
            assert_eq!(f.lumens, 1000.0, "the fixture takes the file's lumens");
        }

        // Other edits keep the profile; a `lumens` in the same call as the file wins over the file's.
        session.execute("lightfixture.edit", &json!({"id": "lf0", "kelvin": 2700.0})).unwrap();
        assert!(session.doc().unwrap().arch.lights[0].ies.is_some());
        session.execute("lightfixture.edit", &json!({"id": "lf0", "ies": file, "lumens": 500.0})).unwrap();
        {
            let doc = session.doc().unwrap();
            assert_eq!(doc.arch.lights[0].lumens, 500.0);
            assert_eq!(doc.arch.lights[0].ies.as_ref().map(|p| p.lumens), Some(1000.0));
        }

        // `ies: null` drops the profile and leaves the lumens alone.
        session.execute("lightfixture.edit", &json!({"id": "lf0", "ies": null})).unwrap();
        {
            let doc = session.doc().unwrap();
            assert_eq!(doc.arch.lights[0].ies, None);
            assert_eq!(doc.arch.lights[0].lumens, 500.0);
        }

        // A file in absolute photometry (-1 lm) has no lumens to take.
        let absolute = dir.file("absolute.ies", TEST_DOWNLIGHT_IES.replace("1 1000 1 5", "1 -1 1 5"));
        session.execute("lightfixture.edit", &json!({"id": "lf0", "ies": absolute})).unwrap();
        {
            let doc = session.doc().unwrap();
            assert_eq!(doc.arch.lights[0].ies.as_ref().map(|p| p.lumens), Some(-1.0));
            assert_eq!(doc.arch.lights[0].lumens, 500.0);
        }

        // A file without [LUMINAIRE] or [TEST] names its profile by the file name.
        let plain = dir.file(
            "plain.ies",
            TEST_DOWNLIGHT_IES.replace("[TEST] cadcraft phase-11 test downlight\n", "").replace("[LUMINAIRE] Test downlight 1000 lm\n", ""),
        );
        session.execute("lightfixture.edit", &json!({"id": "lf0", "ies": plain})).unwrap();
        assert_eq!(session.doc().unwrap().arch.lights[0].ies.as_ref().map(|p| p.name.clone()), Some("plain.ies".to_string()));

        // `lightfixture` takes the file too, once for a whole grid.
        session.doc_mut().unwrap().arch.push_room(square_room());
        let out = session.execute("lightfixture", &json!({"room": "r0", "rows": 2, "cols": 2, "ies": file})).unwrap();
        assert_eq!(out["lights"].as_array().unwrap().len(), 4);
        let doc = session.doc().unwrap();
        for f in &doc.arch.lights[1..] {
            assert_eq!(f.lumens, 1000.0);
            assert_eq!(f.ies.as_ref().map(|p| p.name.as_str()), Some("Test downlight 1000 lm"));
        }
    }

    #[test]
    fn lightfixture_refuses_non_finite_area_size() {
        let mut session = Session::new();
        assert!(session.execute("lightfixture", &json!({"type": "LED panel 600", "at": [2.0, 2.0], "width": "nan"})).is_err());
        assert!(session.execute("lightfixture", &json!({"type": "LED panel 600", "at": [2.0, 2.0], "length": "long"})).is_err());
        assert_eq!(session.doc().unwrap().arch.lights.len(), 0);
    }

    #[test]
    fn lightfixture_area_to_point_ignores_size() {
        let mut session = Session::new();
        session.execute("lightfixture", &json!({"type": "LED panel 600", "at": [2.0, 2.0]})).unwrap();
        session.execute("lightfixture.edit", &json!({"id": "lf0", "kind": "point"})).unwrap();
        let doc = session.doc().unwrap();
        let f = &doc.arch.lights[0];
        assert_eq!(f.kind, LightKind::Point);
        // width and length are still stored but not checked for non-area lights
        assert_eq!((f.width, f.length), (0.6, 0.6));
    }

    #[test]
    fn lightfixture_ies_with_rotation_and_tilt() {
        let dir = TempDir::new("ies-rot-tilt");
        let file = dir.file("downlight.ies", TEST_DOWNLIGHT_IES);
        let mut session = Session::new();
        session.execute("lightfixture", &json!({"type": "Recessed pot light", "at": [1.0, 1.0], "rotation": 45.0, "tilt": 30.0})).unwrap();
        session.execute("lightfixture.edit", &json!({"id": "lf0", "ies": file})).unwrap();
        let doc = session.doc().unwrap();
        let f = &doc.arch.lights[0];
        assert_eq!(f.rotation, 45.0);
        assert_eq!(f.tilt, 30.0);
        assert!(f.ies.is_some());
    }

    #[test]
    fn lightfixture_ies_null_when_none_set() {
        let mut session = Session::new();
        session.execute("lightfixture", &json!({"type": "Recessed pot light", "at": [1.0, 1.0]})).unwrap();
        session.execute("lightfixture.edit", &json!({"id": "lf0", "ies": null})).unwrap();
        let doc = session.doc().unwrap();
        assert_eq!(doc.arch.lights[0].ies, None);
    }

    #[test]
    fn lightfixture_ies_file_trailing_whitespace() {
        let dir = TempDir::new("ies-trailing");
        let file = dir.file("downlight.ies", format!("{}\n\n\n", TEST_DOWNLIGHT_IES));
        let mut session = Session::new();
        session.execute("lightfixture", &json!({"type": "Recessed pot light", "at": [1.0, 1.0]})).unwrap();
        session.execute("lightfixture.edit", &json!({"id": "lf0", "ies": file})).unwrap();
        let doc = session.doc().unwrap();
        assert!(doc.arch.lights[0].ies.is_some());
    }

    #[test]
    fn lightfixture_refuses_bad_area_sizes_and_bad_ies_files() {
        let dir = TempDir::new("ies-bad");
        let mut session = Session::new();

        // An area's width is checked first, then its length; the message names the field.
        let msg = error_text(&mut session, "lightfixture", &json!({"type": "LED panel 600", "at": [2.0, 2.0], "width": 5.0}));
        assert!(msg.contains("width must be 0.01..=4, got 5") && !msg.contains("length"), "{msg}");
        let msg = error_text(&mut session, "lightfixture", &json!({"type": "LED panel 600", "at": [2.0, 2.0], "length": 0.001}));
        assert!(msg.contains("length") && !msg.contains("width"), "{msg}");
        let msg = error_text(&mut session, "lightfixture", &json!({"type": "LED panel 600", "at": [2.0, 2.0], "width": 9.0, "length": 9.0}));
        assert!(msg.contains("width") && !msg.contains("length"), "{msg}");
        // A pot light made an area has no size yet, and an area of nothing is not an area.
        let msg = error_text(&mut session, "lightfixture", &json!({"at": [1.0, 1.0], "kind": "area"}));
        assert!(msg.contains("width"), "{msg}");
        let msg = error_text(&mut session, "lightfixture", &json!({"at": [1.0, 1.0], "kind": "area", "width": "wide"}));
        assert!(msg.contains("`width`"), "{msg}");
        let msg = error_text(&mut session, "lightfixture", &json!({"at": [1.0, 1.0], "kind": "tube"}));
        assert!(msg.contains("`kind` must be \"point\", \"spot\" or \"area\""), "{msg}");
        assert!(session.doc().unwrap().arch.lights.is_empty(), "a refused command places nothing");

        // A refused edit changes nothing.
        session.execute("lightfixture", &json!({"type": "LED panel 600", "at": [2.0, 2.0]})).unwrap();
        let before = session.doc().unwrap().arch.lights[0].clone();
        let msg = error_text(&mut session, "lightfixture.edit", &json!({"id": "lf0", "width": 5.0}));
        assert!(msg.contains("width") && msg.contains("lightfixture.edit"), "{msg}");
        let msg = error_text(&mut session, "lightfixture.edit", &json!({"id": "lf0", "length": f64::INFINITY}));
        assert!(msg.contains("length"), "{msg}");
        assert_eq!(session.doc().unwrap().arch.lights[0], before);

        // IES files: no TILT line, a parse error with its line, too large, missing, not a file.
        let no_tilt = dir.file("no-tilt.ies", TEST_DOWNLIGHT_IES.replace("TILT=NONE\n", ""));
        let msg = error_text(&mut session, "lightfixture.edit", &json!({"id": "lf0", "ies": no_tilt}));
        assert!(msg.contains("IES") && msg.contains("TILT"), "{msg}");
        let garbled = dir.file("garbled.ies", TEST_DOWNLIGHT_IES.replace("0 22.5 45 67.5 90", "0 22.5 nine 67.5 90"));
        let msg = error_text(&mut session, "lightfixture.edit", &json!({"id": "lf0", "ies": garbled}));
        assert!(msg.contains("IES line 7:"), "{msg}");
        let huge = dir.file("huge.ies", vec![b'x'; MAX_IES_BYTES + 1]);
        let msg = error_text(&mut session, "lightfixture.edit", &json!({"id": "lf0", "ies": huge}));
        assert!(msg.contains("IES: file over 1000000 bytes"), "{msg}");
        let msg = error_text(&mut session, "lightfixture.edit", &json!({"id": "lf0", "ies": dir.path_str("missing.ies")}));
        assert!(msg.contains("could not read"), "{msg}");
        let msg = error_text(&mut session, "lightfixture.edit", &json!({"id": "lf0", "ies": dir.to_string_lossy()}));
        assert!(msg.contains("could not read"), "{msg}");
        for not_a_path in [json!(5), json!(""), json!(true)] {
            let msg = error_text(&mut session, "lightfixture.edit", &json!({"id": "lf0", "ies": not_a_path}));
            assert!(msg.contains("`ies` must be a path string or null"), "{msg}");
        }
        assert_eq!(session.doc().unwrap().arch.lights[0], before, "a refused file changes nothing");

        // A refused file places nothing either, and the profile already set survives a bad one.
        let msg = error_text(&mut session, "lightfixture", &json!({"at": [3.0, 3.0], "ies": no_tilt}));
        assert!(msg.contains("lightfixture") && msg.contains("TILT"), "{msg}");
        assert_eq!(session.doc().unwrap().arch.lights.len(), 1);
        let good = dir.file("good.ies", TEST_DOWNLIGHT_IES);
        session.execute("lightfixture.edit", &json!({"id": "lf0", "ies": good})).unwrap();
        let with_profile = session.doc().unwrap().arch.lights[0].clone();
        assert!(with_profile.ies.is_some());
        error_text(&mut session, "lightfixture.edit", &json!({"id": "lf0", "ies": no_tilt}));
        assert_eq!(session.doc().unwrap().arch.lights[0], with_profile);
    }
}
