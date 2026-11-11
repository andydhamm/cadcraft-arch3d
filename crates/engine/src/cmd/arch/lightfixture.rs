//! `lightfixture` and its edits: light fixtures from the catalogue, one at a point or an even
//! grid in a room (phase 10). Since phase 11 a fixture may be an area light (`kind: "area"` with a
//! `width` and `length`, like the catalogue's `LED panel 600`) and may carry an IES photometric
//! file (`ies: "path.ies"`, `ies: null` clears it).

use cadcraft_arch::ies;
use cadcraft_arch::light::{catalogue, find_type, fixture_from_type, grid_positions, strip_length, validate_fixture};
use cadcraft_doc::arch::{
    ArchId, CabinetKind, FixtureShape, IesProfile, LightFixture, LightKind, LightMount, MAX_AREA_SIDE, MAX_FIXTURE_GRID, MAX_IES_BYTES,
    MAX_LIGHT_FIXTURES, MAX_LUMENS_PER_METRE, MAX_STRIP_LENGTH, MAX_STRIP_POINTS, MIN_AREA_SIDE, StripFacing, handle_json, light_handle, light_label,
};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad, point_req, str_param};
use crate::{Result, Session};

/// How far behind a wall cabinet's front face an under-cabinet strip runs (m).
const STRIP_UNDER_BEHIND_FACE: f64 = 0.05;
/// How far under the carcass bottom an under-cabinet strip sits (m).
const STRIP_UNDER_HEIGHT_MARGIN: f64 = 0.005;
/// How far below the ceiling a room ring sits by default (m).
const STRIP_ROOM_DROP: f64 = 0.05;
/// Two cabinets whose front edges are this close (m) are joined.
const STRIP_JOIN_TOL: f64 = 0.01;

/// A strip's resolved placement: its 2D path and how it is hung. The forms (`path`, `under`,
/// `room`) all resolve to this; the fixture is built from it in one place.
struct StripPlan {
    points: Vec<Vec2>,
    closed: bool,
    level: u32,
    height: f64,
    facing: StripFacing,
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("lightfixture", "Light Fixture", run_lightfixture).menu(&["Architecture", "Light Fixture"]).params(
            "{type?, at?: [x,y], room?: \"r0\", rows?, cols?, LED strip (exactly one form): path?: [[x,y],...] + closed? + facing?: \"down\"|\"out\", under?: [\"cb0\",...] (joined wall cabinets), room? + inset? (a closed ring facing out); lumensPerMetre?, level?, height?, rotation?, tilt?, lumens?, kelvin?, cone?, falloff?, kind?: \"point\"|\"spot\"|\"area\", width?, length?, ies?: \"file.ies\"|null, on?}",
        ),
        CommandSpec::new("lightfixture.edit", "Edit Light Fixture", run_lightfixture_edit)
            .menu(&["Architecture", "Edit Light Fixture"])
            .params("{id: \"lf0\", at?, path?, closed?, lumensPerMetre?, facing?: \"down\"|\"out\", height?, rotation?, tilt?, lumens?, kelvin?, cone?, falloff?, kind?, width?, length?, ies?: \"file.ies\"|null, on?}"),
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
        LightKind::Strip => "strip",
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
    if is_strip_request(params) {
        return run_strip(session, params);
    }
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

/// Whether the call asks for a strip: the catalogue type is a strip, or it carries a strip form.
fn is_strip_request(params: &Value) -> bool {
    let typ = str_param(params, "type").unwrap_or("");
    let strip_type = find_type(typ).is_some_and(|t| t.kind == LightKind::Strip);
    strip_type || ["path", "under"].iter().any(|k| params.get(*k).is_some_and(|v| !v.is_null()))
}

fn parse_facing(cmd: &str, value: &Value) -> Result<StripFacing> {
    match value.as_str().map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("down") => Ok(StripFacing::Down),
        Some("out") => Ok(StripFacing::Out),
        _ => Err(bad(cmd, format!("`facing` must be \"down\" or \"out\"; got {value}"))),
    }
}

fn finite_param(cmd: &str, params: &Value, key: &str) -> Result<Option<f64>> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|n| n.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite number; got {v}"))),
    }
}

/// `path`: `[x, y]` points (at most `MAX_STRIP_POINTS`), every coordinate finite.
fn parse_path(cmd: &str, value: &Value) -> Result<Vec<Vec2>> {
    let rows = value.as_array().ok_or_else(|| bad(cmd, format!("`path` must be an array of [x, y] points; got {value}")))?;
    if rows.len() > MAX_STRIP_POINTS {
        return Err(bad(cmd, format!("`path` holds at most {MAX_STRIP_POINTS} points; got {}", rows.len())));
    }
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let xy = row.as_array().filter(|a| a.len() == 2).and_then(|a| Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?)));
        match xy {
            Some((x, y)) if x.is_finite() && y.is_finite() => out.push(Vec2::new(x, y)),
            _ => return Err(bad(cmd, format!("`path` points must be [x, y] with finite numbers; got {row}"))),
        }
    }
    Ok(out)
}

/// The point half way along the path (by length), the strip's position.
fn path_midpoint(points: &[Vec2], closed: bool) -> Vec2 {
    let mut ring: Vec<Vec2> = points.to_vec();
    if closed {
        if let Some(first) = points.first() {
            ring.push(*first);
        }
    }
    let total: f64 = ring.windows(2).map(|w| (w[1] - w[0]).len()).sum();
    let mut left = total / 2.0;
    for w in ring.windows(2) {
        let len = (w[1] - w[0]).len();
        if len > 0.0 && left <= len {
            return w[0] + (w[1] - w[0]) * (left / len);
        }
        left -= len;
    }
    ring.first().copied().unwrap_or(Vec2::new(0.0, 0.0))
}

/// Height (m above the level) of the ceiling over `at` on `level`, as a ceiling fixture would hang.
fn ceiling_over(doc: &cadcraft_doc::Drawing, level: u32, at: Vec2) -> Option<f64> {
    let probe = LightFixture { level, ..LightFixture::new(ArchId(0), "Recessed pot light", at) };
    let z = cadcraft_arch::light::light_point(&doc.arch, &probe)?.z;
    let model = &doc.arch;
    let elevation = model.levels.iter().rev().find(|l| l.index == level).or_else(|| model.levels.last()).map_or(0.0, |l| l.elevation);
    Some(z - elevation)
}

/// One wall cabinet's front edge and the strip's line 0.05 m behind it.
struct CabinetRun {
    front: (Vec2, Vec2),
    path: (Vec2, Vec2),
}

/// Resolve `under`: wall cabinets on one level whose front edges join in a run. The path runs
/// along their front-bottom edges, 0.05 m behind the front face.
fn plan_under(doc: &cadcraft_doc::Drawing, value: &Value) -> Result<StripPlan> {
    let cmd = "lightfixture";
    let ids = value
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= MAX_STRIP_POINTS)
        .ok_or_else(|| bad(cmd, format!("`under` must be a non-empty array of cabinet ids; got {value}")))?;
    let mut level = None;
    let mut elevation: Option<f64> = None;
    let mut runs: Vec<CabinetRun> = Vec::new();
    for raw in ids {
        let label = raw.as_str().map(str::trim).unwrap_or("");
        let number = label.strip_prefix("cb").or_else(|| label.strip_prefix("CB")).unwrap_or(label);
        let cab = number.parse::<u32>().ok().and_then(|n| doc.arch.cabinets.iter().find(|c| c.id.0 == n));
        let Some(cab) = cab else {
            return Err(bad(cmd, format!("`under` names an unknown cabinet {raw}")));
        };
        if cab.kind != CabinetKind::Wall {
            return Err(bad(cmd, format!("`under` takes wall cabinets only; {label} is not one")));
        }
        if level.is_some_and(|l| l != cab.level) || elevation.is_some_and(|e| (e - cab.elevation).abs() > 1e-6) {
            return Err(bad(cmd, format!("`under`: {label} is on another level or hangs at another height than the first cabinet")));
        }
        let usable =
            cab.position.is_finite() && cab.rotation.is_finite() && cab.width.is_finite() && cab.depth.is_finite() && cab.elevation.is_finite();
        if !usable || cab.width <= 0.0 {
            return Err(bad(cmd, format!("`under`: cabinet {label} has no usable size or position")));
        }
        level = Some(cab.level);
        elevation = Some(cab.elevation);
        let u = Vec2::from_angle(cab.rotation.to_radians());
        let v = u.perp();
        let hw = cab.width / 2.0;
        let at = |x: f64, y: f64| cab.position + u * x + v * y;
        runs.push(CabinetRun {
            front: (at(-hw, cab.depth), at(hw, cab.depth)),
            path: (at(-hw, cab.depth - STRIP_UNDER_BEHIND_FACE), at(hw, cab.depth - STRIP_UNDER_BEHIND_FACE)),
        });
    }
    let near = |a: Vec2, b: Vec2| (a - b).len() <= STRIP_JOIN_TOL;
    // Start from a cabinet with a free end (a ring of cabinets has none: take the first).
    let free_end = |i: usize| {
        let Some(me) = runs.get(i) else { return false };
        let joined = |p: Vec2| runs.iter().enumerate().any(|(j, r)| j != i && (near(p, r.front.0) || near(p, r.front.1)));
        !joined(me.front.0) || !joined(me.front.1)
    };
    let start = (0..runs.len()).find(|&i| free_end(i)).unwrap_or(0);
    let Some(first) = runs.get(start) else {
        return Err(bad(cmd, "`under` names no cabinets"));
    };
    let mut used = vec![false; runs.len()];
    if let Some(u) = used.get_mut(start) {
        *u = true;
    }
    // Orient the first cabinet so that its joined end is the tail.
    let joined_at_b = runs.iter().enumerate().any(|(j, r)| j != start && (near(first.front.1, r.front.0) || near(first.front.1, r.front.1)));
    let (mut tail_front, tail_path, head_path) =
        if joined_at_b || runs.len() == 1 { (first.front.1, first.path.1, first.path.0) } else { (first.front.0, first.path.0, first.path.1) };
    let mut points = vec![head_path, tail_path];
    for _ in 1..runs.len() {
        let next = runs.iter().enumerate().find_map(|(j, r)| {
            if used.get(j).copied().unwrap_or(true) {
                None
            } else if near(tail_front, r.front.0) {
                Some((j, r.front.1, r.path.1))
            } else if near(tail_front, r.front.1) {
                Some((j, r.front.0, r.path.0))
            } else {
                None
            }
        });
        let Some((j, front, path)) = next else {
            return Err(bad(cmd, "`under`: the cabinets' front edges do not join (a gap of more than 0.01 m)"));
        };
        if let Some(u) = used.get_mut(j) {
            *u = true;
        }
        tail_front = front;
        points.push(path);
    }
    Ok(StripPlan {
        points,
        closed: false,
        level: level.unwrap_or(0),
        height: elevation.unwrap_or(0.0) - STRIP_UNDER_HEIGHT_MARGIN,
        facing: StripFacing::Down,
    })
}

/// Resolve `room` + `inset`: a closed counter-clockwise ring inset from the room outline,
/// facing out, hung `height` (default ceiling - 0.05).
fn plan_room(doc: &cadcraft_doc::Drawing, params: &Value) -> Result<StripPlan> {
    let cmd = "lightfixture";
    let label = str_param(params, "room").ok_or_else(|| bad(cmd, "`room` must be a room id"))?;
    let room =
        parse_room_id(label).and_then(|id| doc.arch.rooms.iter().find(|r| r.id == id)).ok_or_else(|| bad(cmd, format!("unknown room `{label}`")))?;
    let inset = finite_param(cmd, params, "inset")?.ok_or_else(|| bad(cmd, "`inset` is required with `room`"))?;
    if inset <= 0.0 {
        return Err(bad(cmd, format!("`inset` must be greater than 0 m; got {inset}")));
    }
    let mut outline = room.polygon.clone();
    if outline.len() > 1 && outline.first() == outline.last() {
        outline.pop();
    }
    let regions = cadcraft_geom::skeleton::inset(&outline, &[], inset)
        .map_err(|e| bad(cmd, format!("`inset` {inset} cannot be taken from room `{label}`: {e:?}")))?;
    let [region] = regions.as_slice() else {
        return Err(bad(cmd, format!("`inset` {inset} leaves {} rings in room `{label}`; exactly one is needed", regions.len())));
    };
    let mut ring = region.outer.clone();
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    let area: f64 = ring.iter().zip(ring.iter().cycle().skip(1)).map(|(a, b)| a.x * b.y - b.x * a.y).sum();
    if area < 0.0 {
        ring.reverse();
    }
    let height = match finite_param(cmd, params, "height")? {
        Some(h) => h,
        None => ceiling_over(doc, room.level, path_midpoint(&ring, true)).map_or(2.4 - STRIP_ROOM_DROP, |c| c - STRIP_ROOM_DROP),
    };
    Ok(StripPlan { points: ring, closed: true, level: room.level, height, facing: StripFacing::Out })
}

/// Resolve `path`: the points as given; the height defaults to the ceiling over the path's midpoint.
fn plan_path(doc: &cadcraft_doc::Drawing, params: &Value, value: &Value) -> Result<StripPlan> {
    let cmd = "lightfixture";
    let points = parse_path(cmd, value)?;
    let closed = match params.get("closed") {
        None | Some(Value::Null) => false,
        Some(v) => v.as_bool().ok_or_else(|| bad(cmd, format!("`closed` must be a boolean; got {v}")))?,
    };
    let level = resolve_level(doc, params.get("level"))?;
    let height = match finite_param(cmd, params, "height")? {
        Some(h) => h,
        None => ceiling_over(doc, level, path_midpoint(&points, closed)).unwrap_or(2.4),
    };
    let facing = match params.get("facing") {
        None | Some(Value::Null) => StripFacing::Down,
        Some(v) => parse_facing(cmd, v)?,
    };
    Ok(StripPlan { points, closed, level, height, facing })
}

/// `lightfixture` with `type: "LED strip"`: ONE match over the exclusive forms (`path`, `under`,
/// `room`). Everything is validated on a new fixture before the drawing is touched.
fn run_strip(session: &mut Session, params: &Value) -> Result<Value> {
    let cmd = "lightfixture";
    let doc = session.doc()?;
    let doc = &*doc;
    let given = |k: &str| params.get(k).is_some_and(|v| !v.is_null());
    let forms: Vec<&str> = ["path", "under", "room"].into_iter().filter(|k| given(k)).collect();
    if let Some(k) = ["at", "rows", "cols"].into_iter().find(|k| given(k)) {
        return Err(bad(cmd, format!("an LED strip takes `path`, `under` or `room`, not `{k}`")));
    }
    if str_param(params, "type").is_some_and(|t| find_type(t).is_some_and(|t| t.kind != LightKind::Strip)) {
        return Err(bad(cmd, "`path` and `under` belong to type \"LED strip\""));
    }
    let mut plan = match forms.as_slice() {
        ["path"] => plan_path(doc, params, params.get("path").unwrap_or(&Value::Null))?,
        ["under"] => plan_under(doc, params.get("under").unwrap_or(&Value::Null))?,
        ["room"] => plan_room(doc, params)?,
        [] => return Err(bad(cmd, "an LED strip needs exactly one of `path`, `under` or `room`")),
        many => return Err(bad(cmd, format!("an LED strip takes exactly one of `path`, `under` or `room`; got {}", many.join(" and ")))),
    };
    if forms != ["room"] && given("inset") {
        return Err(bad(cmd, "`inset` belongs to the `room` form"));
    }
    if forms == ["under"] {
        // Under cabinets the height, level and facing are fixed by the run.
        if let Some(k) = ["height", "facing", "level"].into_iter().find(|k| given(k)) {
            return Err(bad(cmd, format!("`{k}` is not taken with `under`: the cabinets set it")));
        }
    }
    if forms == ["room"] {
        if let Some(v) = params.get("facing").filter(|v| !v.is_null()) {
            plan.facing = parse_facing(cmd, v)?;
        }
    }

    let typ = str_param(params, "type").unwrap_or("LED strip");
    let t = find_type(typ).ok_or_else(|| bad(cmd, format!("unknown fixture type `{typ}`; lightfixture.types lists them")))?;
    let id = ArchId(doc.arch.next_light_id);
    let mut fixture = fixture_from_type(id, &t, path_midpoint(&plan.points, plan.closed));
    fixture.level = plan.level;
    fixture.path = plan.points;
    fixture.closed = plan.closed;
    fixture.height = Some(plan.height);
    fixture.facing = plan.facing;
    if let Some(v) = finite_param(cmd, params, "lumensPerMetre")? {
        fixture.lumens_per_metre = v;
    }
    if let Some(v) = finite_param(cmd, params, "kelvin")? {
        fixture.kelvin = v;
    }
    if let Some(v) = params.get("on") {
        fixture.on = v.as_bool().ok_or_else(|| bad(cmd, format!("`on` must be a boolean; got {v}")))?;
    }
    settle_strip(&mut fixture);
    check_fixture(cmd, &fixture)?;

    if doc.arch.lights.len() + 1 > MAX_LIGHT_FIXTURES {
        return Err(bad(cmd, format!("at most {MAX_LIGHT_FIXTURES} light fixtures are allowed")));
    }
    let mut doc_mut = session.doc_mut()?;
    doc_mut.ensure_layer("E-LITE");
    doc_mut.arch.lights.push(fixture);
    doc_mut.arch.next_light_id = doc_mut.arch.next_light_id.saturating_add(1);
    doc_mut.touch_arch();
    Ok(json!({ "lights": [light_label(id)], "handles": [handle_json(light_handle(id))] }))
}

/// Keep a strip's derived values true: its position is the path's midpoint and its lumens are
/// lumens per metre x length.
fn settle_strip(fixture: &mut LightFixture) {
    if fixture.kind != LightKind::Strip {
        return;
    }
    if fixture.path.len() >= 2 && fixture.path.iter().all(|p| p.is_finite()) {
        fixture.position = path_midpoint(&fixture.path, fixture.closed);
    }
    let lumens = fixture.lumens_per_metre * strip_length(fixture);
    if lumens.is_finite() {
        fixture.lumens = lumens;
    }
}

/// The strip keys of `lightfixture.edit`, applied to `fixture`. Returns whether any was given (so
/// that `settle_strip` runs). Strip keys on another kind are refused.
fn edit_strip(fixture: &mut LightFixture, params: &Value) -> Result<bool> {
    let cmd = "lightfixture.edit";
    let present: Vec<&str> = ["path", "closed", "lumensPerMetre", "facing"].into_iter().filter(|k| params.get(*k).is_some()).collect();
    if fixture.kind != LightKind::Strip {
        return match present.first() {
            Some(k) => Err(bad(cmd, format!("`{k}` applies to strips only; this light is a {}", kind_name(fixture.kind)))),
            None => Ok(false),
        };
    }
    if params.get("kind").is_some() {
        return Err(bad(cmd, "a strip's `kind` cannot change"));
    }
    if let Some(v) = params.get("path") {
        fixture.path = parse_path(cmd, v)?;
    }
    if let Some(v) = params.get("closed") {
        fixture.closed = v.as_bool().ok_or_else(|| bad(cmd, format!("`closed` must be a boolean; got {v}")))?;
    }
    if let Some(v) = params.get("lumensPerMetre") {
        fixture.lumens_per_metre =
            v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(cmd, format!("`lumensPerMetre` must be a finite number; got {v}")))?;
    }
    if let Some(v) = params.get("facing") {
        fixture.facing = parse_facing(cmd, v)?;
    }
    Ok(!present.is_empty())
}

fn run_lightfixture_edit(session: &mut Session, params: &Value) -> Result<Value> {
    let raw = str_param(params, "id").ok_or_else(|| bad("lightfixture.edit", "`id` is required"))?;
    let id = parse_light_id(raw).ok_or_else(|| bad("lightfixture.edit", format!("unknown light `{raw}`")))?;
    let ies = read_ies_param("lightfixture.edit", params)?;
    let mut doc = session.doc_mut()?;
    let idx = doc.arch.lights.iter().position(|f| f.id == id).ok_or_else(|| bad("lightfixture.edit", format!("unknown light `{raw}`")))?;
    let mut fixture = doc.arch.lights[idx].clone();
    let strip_edit = edit_strip(&mut fixture, params)?;
    if let Some(at) = params.get("at") {
        if !at.is_null() {
            if fixture.kind == LightKind::Strip {
                return Err(bad("lightfixture.edit", "a strip's position follows its `path`; give `path` instead of `at`"));
            }
            fixture.position = point_req("lightfixture.edit", params, "at")?;
        }
    }
    apply_overrides("lightfixture.edit", &mut fixture, params)?;
    apply_ies(&mut fixture, &ies, params);
    if strip_edit {
        settle_strip(&mut fixture);
    }
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
        assert_eq!(types.len(), 7);
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

    fn wall_cabinet(position: Vec2, width: f64, kind: CabinetKind) -> cadcraft_doc::arch::Cabinet {
        cadcraft_doc::arch::Cabinet { kind, position, rotation: 0.0, width, depth: 0.35, height: 0.7, elevation: 1.372, ..Default::default() }
    }

    #[test]
    fn strip_path_under_cabinets_room_inset_and_refusals() {
        let mut session = Session::new();
        session.doc_mut().unwrap().arch.push_room(square_room());
        let lights = |s: &Session| s.doc().unwrap().arch.lights.clone();

        // A path strip: position is the midpoint, lumens = lm/m x length, height as given.
        let out =
            session.execute("lightfixture", &json!({"type": "LED strip", "path": [[1, 2], [3, 2]], "height": 2.4, "lumensPerMetre": 500})).unwrap();
        assert_eq!(out["lights"][0], "lf0");
        let f = lights(&session)[0].clone();
        assert_eq!((f.kind, f.path.len(), f.closed, f.facing), (LightKind::Strip, 2, false, StripFacing::Down));
        assert_eq!((f.position, f.lumens, f.height), (Vec2::new(2.0, 2.0), 1000.0, Some(2.4)));

        // Height defaults to the room's ceiling (2.4 here); `out` and `closed` are taken.
        session.execute("lightfixture", &json!({"type": "LED strip", "path": [[1, 1], [2, 1], [2, 2]], "closed": true, "facing": "out"})).unwrap();
        let f = lights(&session)[1].clone();
        assert_eq!((f.closed, f.facing, f.height), (true, StripFacing::Out, Some(2.4)));

        // A room ring: one closed path inset 0.5 m from the 4 x 4 outline, 12 m, facing out, ceiling - 0.05.
        session.execute("lightfixture", &json!({"type": "LED strip", "room": "r0", "inset": 0.5})).unwrap();
        let ring = lights(&session)[2].clone();
        assert_eq!((ring.closed, ring.facing), (true, StripFacing::Out));
        assert!((strip_length(&ring) - 12.0).abs() < 1e-6, "{}", strip_length(&ring));
        assert!((ring.height.unwrap_or(0.0) - 2.35).abs() < 1e-9, "{:?}", ring.height);
        assert!(ring.path.iter().all(|p| p.x >= 0.5 - 1e-9 && p.x <= 3.5 + 1e-9 && p.y >= 0.5 - 1e-9 && p.y <= 3.5 + 1e-9));

        // Under a run of two joined wall cabinets: their widths, 5 mm under the carcass, down.
        let (a, b, gap, base) = {
            let doc = session.doc_mut().unwrap();
            let a = doc.arch.push_cabinet(wall_cabinet(Vec2::new(1.3, 0.0), 0.6, CabinetKind::Wall));
            let b = doc.arch.push_cabinet(wall_cabinet(Vec2::new(2.05, 0.0), 0.9, CabinetKind::Wall));
            let gap = doc.arch.push_cabinet(wall_cabinet(Vec2::new(3.5, 0.0), 0.6, CabinetKind::Wall));
            let base = doc.arch.push_cabinet(wall_cabinet(Vec2::new(2.05, 1.0), 0.9, CabinetKind::Base));
            (a, b, gap, base)
        };
        let ids = |v: &[ArchId]| json!(v.iter().map(|i| format!("cb{}", i.0)).collect::<Vec<_>>());
        // Either order of the ids finds the same run.
        session.execute("lightfixture", &json!({"type": "LED strip", "under": ids(&[b, a])})).unwrap();
        let under = lights(&session)[3].clone();
        assert!((strip_length(&under) - 1.5).abs() < 1e-9, "{}", strip_length(&under));
        assert!((under.height.unwrap_or(0.0) - (1.372 - 0.005)).abs() < 1e-9);
        assert_eq!((under.closed, under.facing), (false, StripFacing::Down));
        // The path runs 0.05 m behind the front face (depth 0.35 -> y 0.30), x 1.0..2.5.
        assert!(under.path.iter().all(|p| (p.y - 0.30).abs() < 1e-9), "{:?}", under.path);
        let xs: Vec<f64> = under.path.iter().map(|p| p.x).collect();
        assert!(
            xs.iter().cloned().fold(f64::MAX, f64::min) - 1.0 < 1e-9 && (xs.iter().cloned().fold(f64::MIN, f64::max) - 2.5).abs() < 1e-9,
            "{xs:?}"
        );

        // Refusals change nothing: the light list and the next id stay as they were.
        let before = (lights(&session), session.doc().unwrap().arch.next_light_id);
        let strip = |extra: Value| {
            let mut p = json!({"type": "LED strip"});
            if let (Some(o), Some(e)) = (p.as_object_mut(), extra.as_object()) {
                o.extend(e.clone());
            }
            p
        };
        let refused = [
            (strip(json!({"path": [[0, 0]]})), "path"),
            (strip(json!({"path": [[0, 0], [1, 0]], "lumensPerMetre": -5})), "lumensPerMetre"),
            (strip(json!({"path": [[0, 0], [1, 0]], "facing": "up"})), "facing"),
            (strip(json!({"path": [[0, 0], [1, 0]], "height": "high"})), "height"),
            (strip(json!({"path": [[0, 0], [0.001, 0]]})), "0.01"),
            (strip(json!({"path": [[0, 0], [1, 0]], "under": ids(&[a])})), "exactly one"),
            (strip(json!({"path": [[0, 0], [1, 0]], "at": [1, 1]})), "`at`"),
            (strip(json!({"path": [[0, 0], [1, 0]], "rows": 2})), "`rows`"),
            (strip(json!({"under": ["cb99"]})), "cb99"),
            (strip(json!({"under": ids(&[a, gap])})), "join"),
            (strip(json!({"under": ids(&[base])})), "wall cabinets"),
            (strip(json!({"room": "r0", "inset": -1})), "inset"),
            (strip(json!({"room": "r0", "inset": 2.5})), "inset"),
            (strip(json!({"room": "r9", "inset": 0.5})), "r9"),
            (strip(json!({"room": "r0", "path": [[0, 0], [1, 0]], "inset": 0.5})), "exactly one"),
            (strip(json!({})), "exactly one"),
        ];
        for (params, word) in refused {
            let msg = error_text(&mut session, "lightfixture", &params);
            assert!(msg.contains(word), "{params}: {msg}");
        }
        assert_eq!((lights(&session), session.doc().unwrap().arch.next_light_id), before, "a refused strip places nothing");

        // Edit: the path, lm/m and facing change the derived lumens and position; refusals change nothing.
        session.execute("lightfixture.edit", &json!({"id": "lf0", "path": [[0, 2], [4, 2]], "lumensPerMetre": 250, "facing": "out"})).unwrap();
        let f = lights(&session)[0].clone();
        assert_eq!((f.position, f.lumens, f.facing), (Vec2::new(2.0, 2.0), 1000.0, StripFacing::Out));
        let kept = lights(&session);
        for params in [
            json!({"id": "lf0", "facing": "sideways"}),
            json!({"id": "lf0", "lumensPerMetre": 10001}),
            json!({"id": "lf0", "path": [[0, 0]]}),
            json!({"id": "lf0", "at": [1, 1]}),
            json!({"id": "lf0", "kind": "point"}),
        ] {
            error_text(&mut session, "lightfixture.edit", &params);
        }
        assert_eq!(lights(&session), kept);
        // Strip keys on a pot light are refused too.
        session.execute("lightfixture", &json!({"at": [1.0, 1.0]})).unwrap();
        let msg = error_text(&mut session, "lightfixture.edit", &json!({"id": "lf4", "facing": "out"}));
        assert!(msg.contains("facing"), "{msg}");
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
