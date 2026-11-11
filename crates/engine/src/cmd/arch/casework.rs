//! `cabinet`, `fixture` and their edits (phase 13): kitchen and bath casework and the plumbing and
//! appliance fixtures, placed against a wall (`wall` + `offset`) or at a point (`at`).
//!
//! Every numeric field must be a finite JSON number of the right type; a bad value is refused,
//! never coerced or clamped, and on any refusal nothing is placed. The `arch.inspect` part
//! (`{"cabinets":true}`, `{"fixtures":true}`) lives here too ([`inspect_casework`]).

use cadcraft_arch::casework::{cabinet_defaults, cabinet_layer, fixture_defaults, fixture_layer, on_wall, validate_cabinet, validate_fixture};
use cadcraft_arch::room::room_at;
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::{
    ArchId, ArchModel, Cabinet, CabinetKind, Fixture, FixtureKind, FrontStyle, Fronts, MAX_CABINETS, MAX_FIXTURES, SymbolMesh, cabinet_handle,
    cabinet_label, fixture_handle, fixture_label, handle_json, room_label,
};
use cadcraft_geom::Vec2;
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::prompt::{Accept, Input, Interactive, Prompt, Step};
use crate::{Result, Session};

/// Most cabinets one `widths` run may place.
const MAX_RUN: usize = 32;

const CABINET_KINDS: [(&str, CabinetKind); 3] = [("base", CabinetKind::Base), ("wall", CabinetKind::Wall), ("tall", CabinetKind::Tall)];
const FIXTURE_KINDS: [(&str, FixtureKind); 8] = [
    ("sink", FixtureKind::Sink),
    ("lavatory", FixtureKind::Lavatory),
    ("toilet", FixtureKind::Toilet),
    ("bathtub", FixtureKind::Bathtub),
    ("shower", FixtureKind::Shower),
    ("range", FixtureKind::Range),
    ("refrigerator", FixtureKind::Refrigerator),
    ("dishwasher", FixtureKind::Dishwasher),
];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("cabinet", "Cabinet", run_cabinet)
            .menu(&["Architecture", "Cabinet"])
            .params(
                "{kind: \"base\"|\"wall\"|\"tall\", wall?: \"w0\", offset?, at?: [x,y], rotation?, level?, width?, depth?, height?, elevation?, fronts?: \"doors\"|\"drawers\"|\"drawerOverDoor\", doors?, drawers?, counter?, widths?: [m, ...], doorStyle?: \"slab\"|\"shaker\"|\"raised-panel\", drawerStyle?: \"slab\"|\"shaker\"|\"raised-panel\", pull?: \"<item-id>\"}",
            )
            .interactive(|_| Ok(Box::new(CaseworkM::new(false)))),
        CommandSpec::new("cabinet.edit", "Edit Cabinet", run_cabinet_edit)
            .menu(&["Architecture", "Edit Cabinet"])
            .params("{id: \"cb0\", kind?, wall?, offset?, at?, rotation?, level?, width?, depth?, height?, elevation?, fronts?, doors?, drawers?, counter?, doorStyle?, drawerStyle?, pull?}"),
        CommandSpec::new("cabinet.delete", "Delete Cabinet", run_cabinet_delete).menu(&["Architecture", "Delete Cabinet"]).params("{id}"),
        CommandSpec::new("fixture", "Fixture", run_fixture)
            .menu(&["Architecture", "Fixture"])
            .params(
                "{kind: \"sink\"|\"lavatory\"|\"toilet\"|\"bathtub\"|\"shower\"|\"range\"|\"refrigerator\"|\"dishwasher\", wall?, offset?, at?, rotation?, level?, width?, depth?, height?, elevation?}",
            )
            .interactive(|_| Ok(Box::new(CaseworkM::new(true)))),
        CommandSpec::new("fixture.edit", "Edit Fixture", run_fixture_edit)
            .menu(&["Architecture", "Edit Fixture"])
            .params("{id: \"fx0\", kind?, wall?, offset?, at?, rotation?, level?, width?, depth?, height?, elevation?}"),
        CommandSpec::new("fixture.delete", "Delete Fixture", run_fixture_delete).menu(&["Architecture", "Delete Fixture"]).params("{id}"),
        CommandSpec::new("fixture.types", "Fixture Types", run_fixture_types).menu(&["Architecture", "Fixture Types"]).noundo().params("{}"),
    ]
}

// ---------- parameter parsing ----------

fn kind_name_cabinet(k: CabinetKind) -> &'static str {
    CABINET_KINDS.iter().find(|(_, c)| *c == k).map_or("base", |(n, _)| n)
}

fn kind_name_fixture(k: FixtureKind) -> &'static str {
    FIXTURE_KINDS.iter().find(|(_, c)| *c == k).map_or("sink", |(n, _)| n)
}

/// A door or drawer style ("slab", "shaker" or "raised-panel"); a bad value is refused.
fn parse_front_style(cmd: &str, p: &Value, key: &str) -> Result<Option<FrontStyle>> {
    match text(cmd, p, key)? {
        None => Ok(None),
        Some(s) => {
            FrontStyle::parse(s).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be \"slab\", \"shaker\" or \"raised-panel\"; got `{s}`")))
        }
    }
}

/// Resolve a `pull` item id to its mesh. Refuses unknown items and items whose
/// `symbol_type` is not "pull". Returns the mesh so it can be stored in the drawing.
fn resolve_pull(cmd: &str, s: &mut Session, raw: &str) -> Result<SymbolMesh> {
    let rec = match s.library.item(raw) {
        Some(r) => r,
        None => return Err(bad(cmd, format!("unknown pull item `{raw}`"))),
    };
    if rec.symbol_type != "pull" {
        return Err(bad(cmd, format!("`{raw}` is not a pull item")));
    }
    match s.library.symbol(raw) {
        Ok(mesh) => Ok(mesh),
        Err(e) => Err(bad(cmd, format!("could not read pull `{raw}`: {e}"))),
    }
}

fn fronts_name(f: Fronts) -> &'static str {
    match f {
        Fronts::Doors => "doors",
        Fronts::Drawers => "drawers",
        Fronts::DrawerOverDoor => "drawerOverDoor",
        Fronts::Open => "open",
    }
}

fn present<'a>(p: &'a Value, key: &str) -> Option<&'a Value> {
    p.get(key).filter(|v| !v.is_null())
}

fn num(cmd: &str, p: &Value, key: &str) -> Result<Option<f64>> {
    match present(p, key) {
        None => Ok(None),
        Some(v) => v.as_f64().filter(|x| x.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite number"))),
    }
}

fn count(cmd: &str, p: &Value, key: &str) -> Result<Option<u32>> {
    match present(p, key) {
        None => Ok(None),
        Some(v) => v.as_u64().filter(|n| *n <= 1000).map(|n| Some(n as u32)).ok_or_else(|| bad(cmd, format!("`{key}` must be a whole number"))),
    }
}

fn flag(cmd: &str, p: &Value, key: &str) -> Result<Option<bool>> {
    match present(p, key) {
        None => Ok(None),
        Some(v) => v.as_bool().map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be true or false"))),
    }
}

fn text<'a>(cmd: &str, p: &'a Value, key: &str) -> Result<Option<&'a str>> {
    match present(p, key) {
        None => Ok(None),
        Some(v) => v.as_str().map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be a string"))),
    }
}

fn point(cmd: &str, p: &Value, key: &str) -> Result<Option<Vec2>> {
    let Some(v) = present(p, key) else { return Ok(None) };
    let err = || bad(cmd, format!("`{key}` must be [x, y] of finite numbers"));
    let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(err)?;
    let x = a.first().and_then(Value::as_f64).filter(|x| x.is_finite()).ok_or_else(err)?;
    let y = a.get(1).and_then(Value::as_f64).filter(|y| y.is_finite()).ok_or_else(err)?;
    Ok(Some(Vec2::new(x, y)))
}

fn parse_id(raw: &str, prefix: &str) -> Option<ArchId> {
    let t = raw.trim().to_ascii_lowercase();
    t.strip_prefix(prefix)?.parse::<u32>().ok().map(ArchId)
}

fn wall_id(cmd: &str, doc: &Drawing, raw: &str) -> Result<ArchId> {
    let id = parse_id(raw, "w").ok_or_else(|| bad(cmd, format!("unknown wall `{raw}`")))?;
    if doc.arch.wall(id).is_none() {
        return Err(bad(cmd, format!("unknown wall `{raw}`")));
    }
    Ok(id)
}

fn level_index(cmd: &str, doc: &Drawing, v: &Value) -> Result<u32> {
    let found = match v {
        Value::Number(_) => {
            v.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| doc.arch.level(*n).is_some() || (*n == 0 && doc.arch.levels.is_empty()))
        }
        Value::String(s) => doc.arch.levels.iter().find(|l| l.name == s.trim()).map(|l| l.index),
        _ => None,
    };
    found.ok_or_else(|| bad(cmd, format!("unknown level `{v}`")))
}

fn first_level(doc: &Drawing) -> u32 {
    doc.arch.levels.first().map_or(0, |l| l.index)
}

/// Placement and size fields shared by cabinets and fixtures.
struct Common {
    wall: Option<ArchId>,
    offset: Option<f64>,
    at: Option<Vec2>,
    rotation: Option<f64>,
    level: Option<u32>,
    width: Option<f64>,
    depth: Option<f64>,
    height: Option<f64>,
    elevation: Option<f64>,
}

fn parse_common(cmd: &str, doc: &Drawing, p: &Value) -> Result<Common> {
    let wall = match text(cmd, p, "wall")? {
        Some(raw) => Some(wall_id(cmd, doc, raw)?),
        None => None,
    };
    let level = match present(p, "level") {
        Some(v) => Some(level_index(cmd, doc, v)?),
        None => None,
    };
    Ok(Common {
        wall,
        offset: num(cmd, p, "offset")?,
        at: point(cmd, p, "at")?,
        rotation: num(cmd, p, "rotation")?,
        level,
        width: num(cmd, p, "width")?,
        depth: num(cmd, p, "depth")?,
        height: num(cmd, p, "height")?,
        elevation: num(cmd, p, "elevation")?,
    })
}

/// Where an item goes: `shift` metres along its width axis from the base position (used by runs).
fn place(cmd: &str, doc: &Drawing, c: &Common, width: f64, depth: f64, shift: f64) -> Result<(Vec2, f64, u32)> {
    match (c.wall, c.at) {
        (Some(_), Some(_)) => Err(bad(cmd, "give either `wall` + `offset` or `at`, not both")),
        (Some(wall), None) => {
            if c.rotation.is_some() {
                return Err(bad(cmd, "`rotation` comes from the wall; do not give it with `wall`"));
            }
            let offset = c.offset.ok_or_else(|| bad(cmd, "`offset` is required with `wall`"))?;
            on_wall(&doc.arch, wall, offset + shift, width, depth).map_err(|e| bad(cmd, e.to_string()))
        }
        (None, Some(at)) => {
            if c.offset.is_some() {
                return Err(bad(cmd, "`offset` needs a `wall`"));
            }
            let rot = c.rotation.unwrap_or(0.0);
            let r = rot.to_radians();
            let pos = Vec2::new(at.x + r.cos() * shift, at.y + r.sin() * shift);
            Ok((pos, rot, c.level.unwrap_or_else(|| first_level(doc))))
        }
        (None, None) => Err(bad(cmd, "`wall` + `offset` or `at` is required")),
    }
}

fn parse_fronts(cmd: &str, p: &Value) -> Result<Option<Fronts>> {
    match text(cmd, p, "fronts")? {
        None => Ok(None),
        Some("doors") => Ok(Some(Fronts::Doors)),
        Some("drawers") => Ok(Some(Fronts::Drawers)),
        Some("drawerOverDoor") => Ok(Some(Fronts::DrawerOverDoor)),
        Some(o) => Err(bad(cmd, format!("`fronts` must be \"doors\", \"drawers\" or \"drawerOverDoor\"; got `{o}`"))),
    }
}

fn parse_cabinet_kind(cmd: &str, p: &Value) -> Result<Option<CabinetKind>> {
    match text(cmd, p, "kind")? {
        None => Ok(None),
        Some(s) => CABINET_KINDS
            .iter()
            .find(|(n, _)| *n == s)
            .map(|(_, k)| Some(*k))
            .ok_or_else(|| bad(cmd, format!("`kind` must be \"base\", \"wall\" or \"tall\"; got `{s}`"))),
    }
}

fn parse_fixture_kind(cmd: &str, p: &Value) -> Result<Option<FixtureKind>> {
    match text(cmd, p, "kind")? {
        None => Ok(None),
        Some(s) => FIXTURE_KINDS.iter().find(|(n, _)| *n == s).map(|(_, k)| Some(*k)).ok_or_else(|| bad(cmd, format!("unknown fixture kind `{s}`"))),
    }
}

fn set<T>(target: &mut T, value: Option<T>) {
    if let Some(v) = value {
        *target = v;
    }
}

// ---------- cabinet ----------

fn run_cabinet(session: &mut Session, params: &Value) -> Result<Value> {
    const CMD: &str = "cabinet";
    // The kind's Set as Default parameters fill in what this call leaves out.
    let merged_params = match params.get("kind").and_then(Value::as_str) {
        Some(k) => super::defaults::merged(session, &format!("cabinet.{k}"), params),
        None => params.clone(),
    };
    let params = &merged_params;
    let kind = parse_cabinet_kind(CMD, params)?.ok_or_else(|| bad(CMD, "`kind` is required"))?;

    let pull_raw = text(CMD, params, "pull")?;
    let pull_mesh: Option<SymbolMesh> = match pull_raw {
        Some(raw) => Some(resolve_pull(CMD, session, raw)?),
        None => None,
    };
    let pull_symbol = pull_mesh.as_ref().map(|m| m.key.clone());

    let doc = session.doc()?;
    let c = parse_common(CMD, &doc, params)?;
    let mut base = cabinet_defaults(kind);
    set(&mut base.width, c.width);
    set(&mut base.depth, c.depth);
    set(&mut base.height, c.height);
    set(&mut base.elevation, c.elevation);
    set(&mut base.fronts, parse_fronts(CMD, params)?);
    set(&mut base.door_style, parse_front_style(CMD, params, "doorStyle")?);
    set(&mut base.drawer_style, parse_front_style(CMD, params, "drawerStyle")?);
    base.pull = pull_raw.map(String::from);
    base.pull_symbol = pull_symbol;
    let doors = count(CMD, params, "doors")?;
    set(&mut base.drawers, count(CMD, params, "drawers")?);
    set(&mut base.counter, flag(CMD, params, "counter")?);

    let widths: Vec<f64> = match present(params, "widths") {
        None => vec![base.width],
        Some(v) => {
            if c.width.is_some() {
                return Err(bad(CMD, "give either `width` or `widths`, not both"));
            }
            let a = v
                .as_array()
                .filter(|a| !a.is_empty() && a.len() <= MAX_RUN)
                .ok_or_else(|| bad(CMD, format!("`widths` must list 1 to {MAX_RUN} widths")))?;
            a.iter()
                .map(|w| w.as_f64().filter(|x| x.is_finite()).ok_or_else(|| bad(CMD, "`widths` must be finite numbers")))
                .collect::<Result<_>>()?
        }
    };
    if doc.arch.cabinets.len().saturating_add(widths.len()) > MAX_CABINETS {
        return Err(bad(CMD, format!("at most {MAX_CABINETS} cabinets are allowed")));
    }

    let first = widths.first().copied().unwrap_or(base.width);
    let mut cabinets = Vec::with_capacity(widths.len());
    let mut edge = 0.0; // start of this cabinet relative to the first one's left edge
    for w in &widths {
        let mut cab = base.clone();
        cab.width = *w;
        cab.doors = doors.unwrap_or(if *w > 0.6 { 2 } else { 1 });
        let shift = edge + w / 2.0 - first / 2.0;
        edge += w;
        let (pos, rot, level) = place(CMD, &doc, &c, *w, cab.depth, shift)?;
        cab.position = pos;
        cab.rotation = rot;
        cab.level = level;
        validate_cabinet(&cab).map_err(|e| bad(CMD, e.to_string()))?;
        cabinets.push(cab);
    }

    let mut d = session.doc_mut()?;
    if let Some(mesh) = pull_mesh {
        d.arch.symbols.insert(mesh.key.clone(), mesh);
    }
    let mut labels = Vec::new();
    let mut handles = Vec::new();
    for cab in cabinets {
        d.ensure_layer(cabinet_layer(cab.kind));
        let id = d.arch.push_cabinet(cab);
        labels.push(cabinet_label(id));
        handles.push(handle_json(cabinet_handle(id)));
    }
    d.touch_arch();
    Ok(json!({ "cabinets": labels, "handles": handles }))
}

fn run_cabinet_edit(session: &mut Session, params: &Value) -> Result<Value> {
    const CMD: &str = "cabinet.edit";
    let raw = text(CMD, params, "id")?.ok_or_else(|| bad(CMD, "`id` is required"))?;
    let id = parse_id(raw, "cb").ok_or_else(|| bad(CMD, format!("unknown cabinet `{raw}`")))?;

    let pull_mesh: Option<SymbolMesh>;
    let clear_pull: bool;
    let new_pull: Option<String>;
    match params.get("pull") {
        None => {
            pull_mesh = None;
            clear_pull = false;
            new_pull = None;
        }
        Some(v) if v.is_null() => {
            pull_mesh = None;
            clear_pull = true;
            new_pull = None;
        }
        Some(v) => {
            let raw_pull = v.as_str().map(String::from).ok_or_else(|| bad(CMD, "`pull` must be a string or null"))?;
            let mesh = resolve_pull(CMD, session, &raw_pull)?;
            pull_mesh = Some(mesh);
            clear_pull = false;
            new_pull = Some(raw_pull);
        }
    }

    let doc = session.doc()?;
    let idx = doc.arch.cabinets.iter().position(|c| c.id == id).ok_or_else(|| bad(CMD, format!("unknown cabinet `{raw}`")))?;
    let mut cab = doc.arch.cabinets.get(idx).cloned().ok_or_else(|| bad(CMD, format!("unknown cabinet `{raw}`")))?;
    if present(params, "widths").is_some() {
        return Err(bad(CMD, "`widths` is not an edit field"));
    }
    let c = parse_common(CMD, &doc, params)?;
    set(&mut cab.kind, parse_cabinet_kind(CMD, params)?);
    set(&mut cab.width, c.width);
    set(&mut cab.depth, c.depth);
    set(&mut cab.height, c.height);
    set(&mut cab.elevation, c.elevation);
    set(&mut cab.fronts, parse_fronts(CMD, params)?);
    set(&mut cab.door_style, parse_front_style(CMD, params, "doorStyle")?);
    set(&mut cab.drawer_style, parse_front_style(CMD, params, "drawerStyle")?);
    set(&mut cab.doors, count(CMD, params, "doors")?);
    set(&mut cab.drawers, count(CMD, params, "drawers")?);
    set(&mut cab.counter, flag(CMD, params, "counter")?);

    if clear_pull {
        cab.pull = None;
        cab.pull_symbol = None;
    } else if let Some(raw_pull) = new_pull {
        cab.pull = Some(raw_pull);
        cab.pull_symbol = pull_mesh.as_ref().map(|m| m.key.clone());
    }
    if c.wall.is_some() || c.at.is_some() {
        let (pos, rot, level) = place(CMD, &doc, &c, cab.width, cab.depth, 0.0)?;
        cab.position = pos;
        cab.rotation = rot;
        cab.level = level;
    } else {
        if c.offset.is_some() {
            return Err(bad(CMD, "`offset` needs a `wall`"));
        }
        set(&mut cab.rotation, c.rotation);
        set(&mut cab.level, c.level);
    }
    validate_cabinet(&cab).map_err(|e| bad(CMD, e.to_string()))?;

    let mut d = session.doc_mut()?;
    if let Some(mesh) = pull_mesh {
        d.arch.symbols.insert(mesh.key.clone(), mesh);
    }
    d.ensure_layer(cabinet_layer(cab.kind));
    if let Some(slot) = d.arch.cabinets.get_mut(idx) {
        *slot = cab;
    }
    d.touch_arch();
    Ok(json!({ "cabinet": cabinet_label(id) }))
}

fn run_cabinet_delete(session: &mut Session, params: &Value) -> Result<Value> {
    const CMD: &str = "cabinet.delete";
    let raw = text(CMD, params, "id")?.ok_or_else(|| bad(CMD, "`id` is required"))?;
    let id = parse_id(raw, "cb").ok_or_else(|| bad(CMD, format!("unknown cabinet `{raw}`")))?;
    let mut d = session.doc_mut()?;
    let idx = d.arch.cabinets.iter().position(|c| c.id == id).ok_or_else(|| bad(CMD, format!("unknown cabinet `{raw}`")))?;
    d.arch.cabinets.remove(idx);
    d.touch_arch();
    Ok(json!({ "deleted": cabinet_label(id) }))
}

// ---------- fixture ----------

fn run_fixture(session: &mut Session, params: &Value) -> Result<Value> {
    const CMD: &str = "fixture";
    let doc = session.doc()?;
    let kind = parse_fixture_kind(CMD, params)?.ok_or_else(|| bad(CMD, "`kind` is required"))?;
    let c = parse_common(CMD, &doc, params)?;
    let mut fx = fixture_defaults(kind);
    set(&mut fx.width, c.width);
    set(&mut fx.depth, c.depth);
    set(&mut fx.height, c.height);
    set(&mut fx.elevation, c.elevation);
    if doc.arch.fixtures.len() >= MAX_FIXTURES {
        return Err(bad(CMD, format!("at most {MAX_FIXTURES} fixtures are allowed")));
    }
    let (pos, rot, level) = place(CMD, &doc, &c, fx.width, fx.depth, 0.0)?;
    fx.position = pos;
    fx.rotation = rot;
    fx.level = level;
    validate_fixture(&fx).map_err(|e| bad(CMD, e.to_string()))?;

    let mut d = session.doc_mut()?;
    d.ensure_layer(fixture_layer(fx.kind));
    let id = d.arch.push_fixture(fx);
    d.touch_arch();
    Ok(json!({ "fixtures": [fixture_label(id)], "handles": [handle_json(fixture_handle(id))] }))
}

fn run_fixture_edit(session: &mut Session, params: &Value) -> Result<Value> {
    const CMD: &str = "fixture.edit";
    let raw = text(CMD, params, "id")?.ok_or_else(|| bad(CMD, "`id` is required"))?;
    let id = parse_id(raw, "fx").ok_or_else(|| bad(CMD, format!("unknown fixture `{raw}`")))?;
    let doc = session.doc()?;
    let idx = doc.arch.fixtures.iter().position(|f| f.id == id).ok_or_else(|| bad(CMD, format!("unknown fixture `{raw}`")))?;
    let mut fx = doc.arch.fixtures.get(idx).cloned().ok_or_else(|| bad(CMD, format!("unknown fixture `{raw}`")))?;
    let c = parse_common(CMD, &doc, params)?;
    set(&mut fx.kind, parse_fixture_kind(CMD, params)?);
    set(&mut fx.width, c.width);
    set(&mut fx.depth, c.depth);
    set(&mut fx.height, c.height);
    set(&mut fx.elevation, c.elevation);
    if c.wall.is_some() || c.at.is_some() {
        let (pos, rot, level) = place(CMD, &doc, &c, fx.width, fx.depth, 0.0)?;
        fx.position = pos;
        fx.rotation = rot;
        fx.level = level;
    } else {
        if c.offset.is_some() {
            return Err(bad(CMD, "`offset` needs a `wall`"));
        }
        set(&mut fx.rotation, c.rotation);
        set(&mut fx.level, c.level);
    }
    validate_fixture(&fx).map_err(|e| bad(CMD, e.to_string()))?;

    let mut d = session.doc_mut()?;
    d.ensure_layer(fixture_layer(fx.kind));
    if let Some(slot) = d.arch.fixtures.get_mut(idx) {
        *slot = fx;
    }
    d.touch_arch();
    Ok(json!({ "fixture": fixture_label(id) }))
}

fn run_fixture_delete(session: &mut Session, params: &Value) -> Result<Value> {
    const CMD: &str = "fixture.delete";
    let raw = text(CMD, params, "id")?.ok_or_else(|| bad(CMD, "`id` is required"))?;
    let id = parse_id(raw, "fx").ok_or_else(|| bad(CMD, format!("unknown fixture `{raw}`")))?;
    let mut d = session.doc_mut()?;
    let idx = d.arch.fixtures.iter().position(|f| f.id == id).ok_or_else(|| bad(CMD, format!("unknown fixture `{raw}`")))?;
    d.arch.fixtures.remove(idx);
    d.touch_arch();
    Ok(json!({ "deleted": fixture_label(id) }))
}

fn run_fixture_types(_: &mut Session, _: &Value) -> Result<Value> {
    let types: Vec<Value> = FIXTURE_KINDS
        .iter()
        .map(|(name, kind)| {
            let f = fixture_defaults(*kind);
            json!({ "kind": name, "width": f.width, "depth": f.depth, "height": f.height, "elevation": f.elevation, "layer": fixture_layer(*kind) })
        })
        .collect();
    Ok(json!({ "types": types }))
}

// ---------- arch.inspect ----------

fn level_elevation(model: &ArchModel, level: u32) -> f64 {
    model.level(level).map_or(0.0, |l| l.elevation)
}

/// Add `cabinets` and `fixtures` to `out` when `p` asks for them. Read-only.
pub(crate) fn inspect_casework(model: &ArchModel, p: &Value, out: &mut Map<String, Value>) {
    if p.get("cabinets").and_then(Value::as_bool) == Some(true) {
        let list: Vec<Value> = model
            .cabinets
            .iter()
            .map(|c| {
                let top = (c.counter && c.kind != CabinetKind::Wall)
                    .then(|| level_elevation(model, c.level) + c.elevation + c.height + c.counter_thickness);
                let host = cadcraft_arch::casework::host_wall(model, c);
                json!({
                    "wall": host.map_or(Value::Null, |(id, _, _)| Value::String(cadcraft_doc::arch::wall_label(id))),
                    "alongWall": host.map_or(Value::Null, |(_, s0, s1)| json!([s0, s1])),
                    "id": cabinet_label(c.id), "handle": handle_json(cabinet_handle(c.id)), "kind": kind_name_cabinet(c.kind),
                    "level": c.level, "at": [c.position.x, c.position.y], "rotation": c.rotation,
                    "width": c.width, "depth": c.depth, "height": c.height, "elevation": c.elevation,
                    "fronts": fronts_name(c.fronts), "doors": c.doors, "drawers": c.drawers, "counter": c.counter,
                    "counterTop": top, "layer": cabinet_layer(c.kind),
                })
            })
            .collect();
        out.insert("cabinets".to_string(), Value::Array(list));
    }
    if p.get("fixtures").and_then(Value::as_bool) == Some(true) {
        let list: Vec<Value> = model
            .fixtures
            .iter()
            .map(|f| {
                json!({
                    "id": fixture_label(f.id), "handle": handle_json(fixture_handle(f.id)), "kind": kind_name_fixture(f.kind),
                    "level": f.level, "at": [f.position.x, f.position.y], "rotation": f.rotation,
                    "width": f.width, "depth": f.depth, "height": f.height, "elevation": f.elevation, "layer": fixture_layer(f.kind),
                    "room": room_at(model, f.level, f.position).map(room_label),
                })
            })
            .collect();
        out.insert("fixtures".to_string(), Value::Array(list));
    }
}

// ---------- interactive ----------

/// Asks for the kind, then a point; places with the defaults at rotation 0 through the JSON form.
struct CaseworkM {
    fixture: bool,
    kind: Option<&'static str>,
}

impl CaseworkM {
    fn new(fixture: bool) -> Self {
        Self { fixture, kind: None }
    }

    fn kind_from(&self, raw: &str) -> Option<&'static str> {
        let l = raw.trim().to_ascii_lowercase();
        if self.fixture { FIXTURE_KINDS.iter().map(|(n, _)| *n).find(|n| *n == l) } else { CABINET_KINDS.iter().map(|(n, _)| *n).find(|n| *n == l) }
    }
}

impl Interactive for CaseworkM {
    fn name(&self) -> &'static str {
        if self.fixture { "fixture" } else { "cabinet" }
    }

    fn prompt(&self, _s: &Session) -> Prompt {
        if self.kind.is_none() {
            let (msg, kw): (&str, &[&str]) = if self.fixture {
                ("Fixture kind", &["Sink", "Lavatory", "Toilet", "Bathtub", "Shower", "Range", "Refrigerator", "Dishwasher"])
            } else {
                ("Cabinet kind", &["Base", "Wall", "Tall"])
            };
            Prompt::new(format!("{msg} [{}]:", kw.join("/")), Accept::TEXT).kw(kw)
        } else {
            Prompt::new("Specify insertion point:", Accept::POINT)
        }
    }

    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match (self.kind, i) {
            (_, Input::Cancel) => Ok(Step::Cancel),
            (None, Input::Keyword(k)) | (None, Input::Text(k)) => {
                self.kind = self.kind_from(&k);
                Ok(Step::Continue)
            }
            (Some(kind), Input::Point(pt)) if pt.is_finite() => {
                let cmd = self.name();
                s.execute(cmd, &json!({ "kind": kind, "at": [pt.x, pt.y], "rotation": 0.0 })).map(|_| Step::Done)
            }
            _ => Ok(Step::Continue),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::Wall;

    fn session() -> Session {
        let mut s = Session::new();
        if let Ok(mut d) = s.doc_mut() {
            d.arch.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
            d.touch_arch();
        }
        s
    }

    fn run(s: &mut Session, cmd: &str, p: Value) -> Result<Value> {
        s.execute(cmd, &p)
    }

    fn cabinets(s: &Session) -> Vec<Cabinet> {
        s.doc().map(|d| d.arch.cabinets.clone()).unwrap_or_default()
    }

    /// A session on a 4 m wall whose library holds a "pull" item `own:bar` and a non-pull `own:chair`.
    fn pull_session(dir: &crate::temp_dir::TempDir) -> Session {
        use cadcraft_arch::library::{ItemKind, ItemRecord};
        std::fs::create_dir_all(dir.join("user")).ok();
        dir.file("user/bar.glb", crate::library::testing::glb([0.1, 0.02, 0.02]));
        dir.file("user/chair.glb", crate::library::testing::glb([0.5, 0.5, 0.9]));
        let mut s = crate::library::testing::session(dir);
        if let Ok(mut d) = s.doc_mut() {
            d.arch.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
            d.touch_arch();
        }
        for (id, ty, file) in [("own:bar", "pull", "bar.glb"), ("own:chair", "chair", "chair.glb")] {
            s.library.add_user_item(ItemRecord {
                id: id.into(),
                name: id.into(),
                kind: ItemKind::Symbol,
                symbol_type: ty.into(),
                licence: "CC0-1.0".into(),
                file: Some(file.into()),
                ..Default::default()
            });
        }
        s
    }

    fn symbol_keys(s: &Session) -> Vec<String> {
        s.doc().map(|d| d.arch.symbols.keys().cloned().collect()).unwrap_or_default()
    }

    #[test]
    fn cabinet_takes_styles_and_a_pull() {
        let dir = crate::temp_dir::TempDir::new("casework-pull");
        let mut s = pull_session(&dir);
        let r = run(
            &mut s,
            "cabinet",
            json!({ "kind": "base", "wall": "w0", "offset": 1.0, "doorStyle": "shaker", "drawerStyle": "raised-panel", "pull": "own:bar" }),
        );
        assert!(r.is_ok(), "{r:?}");
        let c = cabinets(&s).first().cloned().unwrap_or_default();
        assert_eq!(c.door_style, FrontStyle::Shaker);
        assert_eq!(c.drawer_style, FrontStyle::RaisedPanel);
        assert_eq!(c.pull.as_deref(), Some("own:bar"));
        let key = c.pull_symbol.clone().unwrap_or_default();
        assert!(!key.is_empty());
        assert_eq!(symbol_keys(&s), vec![key]);
        // Without the fields the defaults hold.
        let r = run(&mut s, "cabinet", json!({ "kind": "wall", "wall": "w0", "offset": 3.0 }));
        assert!(r.is_ok(), "{r:?}");
        let c = cabinets(&s).get(1).cloned().unwrap_or_default();
        assert_eq!((c.door_style, c.drawer_style, c.pull), (FrontStyle::Slab, FrontStyle::Slab, None));
    }

    #[test]
    fn cabinet_edit_changes_and_clears_the_pull() {
        let dir = crate::temp_dir::TempDir::new("casework-pull-edit");
        let mut s = pull_session(&dir);
        assert!(run(&mut s, "cabinet", json!({ "kind": "base", "wall": "w0", "offset": 1.0 })).is_ok());
        let r = run(&mut s, "cabinet.edit", json!({ "id": "cb0", "pull": "own:bar", "doorStyle": "shaker" }));
        assert!(r.is_ok(), "{r:?}");
        let c = cabinets(&s).first().cloned().unwrap_or_default();
        assert_eq!(c.pull.as_deref(), Some("own:bar"));
        assert!(c.pull_symbol.is_some());
        assert_eq!(c.door_style, FrontStyle::Shaker);
        assert_eq!(c.drawer_style, FrontStyle::Slab);
        // Editing something else keeps the pull.
        assert!(run(&mut s, "cabinet.edit", json!({ "id": "cb0", "drawerStyle": "shaker" })).is_ok());
        let c = cabinets(&s).first().cloned().unwrap_or_default();
        assert_eq!((c.pull.as_deref(), c.drawer_style), (Some("own:bar"), FrontStyle::Shaker));
        // null removes it.
        assert!(run(&mut s, "cabinet.edit", json!({ "id": "cb0", "pull": null })).is_ok());
        let c = cabinets(&s).first().cloned().unwrap_or_default();
        assert_eq!((c.pull, c.pull_symbol), (None, None));
    }

    #[test]
    fn a_run_shares_its_parts() {
        let dir = crate::temp_dir::TempDir::new("casework-pull-run");
        let mut s = pull_session(&dir);
        let r = run(
            &mut s,
            "cabinet",
            json!({ "kind": "base", "wall": "w0", "offset": 0.5, "widths": [0.6, 0.9, 0.45], "doorStyle": "raised-panel", "drawerStyle": "shaker", "pull": "own:bar" }),
        );
        assert!(r.is_ok(), "{r:?}");
        let cs = cabinets(&s);
        assert_eq!(cs.len(), 3);
        let first = cs.first().and_then(|c| c.pull_symbol.clone());
        for c in &cs {
            assert_eq!(c.door_style, FrontStyle::RaisedPanel);
            assert_eq!(c.drawer_style, FrontStyle::Shaker);
            assert_eq!(c.pull.as_deref(), Some("own:bar"));
            assert_eq!(c.pull_symbol, first);
        }
        assert_eq!(symbol_keys(&s).len(), 1);
    }

    #[test]
    fn unknown_styles_and_pulls_are_refused_before_any_change() {
        let dir = crate::temp_dir::TempDir::new("casework-pull-refused");
        let mut s = pull_session(&dir);
        assert!(run(&mut s, "cabinet", json!({ "kind": "base", "wall": "w0", "offset": 1.0 })).is_ok());
        let before = s.doc().map(|d| d.arch.clone()).ok();
        for p in [
            json!({ "kind": "base", "wall": "w0", "offset": 2.0, "doorStyle": "arched" }),
            json!({ "kind": "base", "wall": "w0", "offset": 2.0, "drawerStyle": 3 }),
            json!({ "kind": "base", "wall": "w0", "offset": 2.0, "pull": "nope" }),
            json!({ "kind": "base", "wall": "w0", "offset": 2.0, "pull": "own:chair" }),
            json!({ "kind": "base", "wall": "w0", "offset": 2.0, "pull": 5 }),
            json!({ "kind": "base", "wall": "w0", "offset": 2.0, "pull": "own:bar", "doorStyle": "arched" }),
        ] {
            assert!(run(&mut s, "cabinet", p.clone()).is_err(), "{p}");
        }
        for p in [
            json!({ "id": "cb0", "doorStyle": "Shaker" }),
            json!({ "id": "cb0", "pull": "nope" }),
            json!({ "id": "cb0", "pull": "own:chair" }),
            json!({ "id": "cb0", "pull": "own:bar", "drawerStyle": "fancy" }),
            json!({ "id": "cb0", "pull": 7 }),
        ] {
            assert!(run(&mut s, "cabinet.edit", p.clone()).is_err(), "{p}");
        }
        let after = s.doc().map(|d| d.arch.clone()).ok();
        assert_eq!(before, after);
        assert!(symbol_keys(&s).is_empty());
    }

    #[test]
    fn cabinet_on_a_wall_backs_onto_its_left_face() {
        let mut s = session();
        let r = run(&mut s, "cabinet", json!({ "kind": "base", "wall": "w0", "offset": 1.0 })).unwrap_or(Value::Null);
        assert_eq!(r["cabinets"], json!(["cb0"]), "{r}");
        assert!(r["handles"][0].as_str().is_some_and(|h| h.starts_with("a005")), "{r}");
        let c = cabinets(&s);
        let c = c.first().cloned().unwrap_or_default();
        assert!((c.position.x - 1.0).abs() < 1e-9 && (c.position.y - 0.1).abs() < 1e-9);
        assert!(c.rotation.abs() < 1e-9);
        assert_eq!(c.doors, 1);
        // The wall cabinet layer and a wide cabinet's two doors.
        let r = run(&mut s, "cabinet", json!({ "kind": "wall", "wall": "w0", "offset": 2.5, "width": 0.9 }));
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(cabinets(&s).get(1).map(|c| c.doors), Some(2));
        let layers = s.doc().map(|d| d.layer("A-FLOR-CASE-UPPR").is_some()).unwrap_or(false);
        assert!(layers);
    }

    #[test]
    fn cabinet_run_places_edge_to_edge() {
        let mut s = session();
        let r = run(&mut s, "cabinet", json!({ "kind": "base", "wall": "w0", "offset": 0.5, "widths": [0.6, 0.9, 0.45] })).unwrap_or(Value::Null);
        assert_eq!(r["cabinets"], json!(["cb0", "cb1", "cb2"]), "{r}");
        let c = cabinets(&s);
        let xs: Vec<f64> = c.iter().map(|c| c.position.x).collect();
        assert!((xs[0] - 0.5).abs() < 1e-9);
        assert!((xs[1] - (0.5 + 0.3 + 0.45)).abs() < 1e-9, "{xs:?}");
        assert!((xs[2] - (0.5 + 0.3 + 0.9 + 0.225)).abs() < 1e-9, "{xs:?}");
        assert_eq!(c.iter().map(|c| c.doors).collect::<Vec<_>>(), vec![1, 2, 1]);
    }

    #[test]
    fn cabinet_refuses_a_wall_cabinet_counter() {
        let mut s = session();
        assert!(run(&mut s, "cabinet", json!({ "kind": "wall", "at": [1, 1], "counter": true })).is_err());
        assert!(run(&mut s, "cabinet", json!({ "kind": "base", "at": [1, 1], "width": "0.6" })).is_err());
        assert!(run(&mut s, "cabinet", json!({ "kind": ["base"], "at": [1, 1] })).is_err());
        assert!(run(&mut s, "cabinet", json!({ "kind": "base", "at": [1, 1], "fronts": "glass" })).is_err());
        assert!(run(&mut s, "cabinet", json!({ "kind": "base", "at": [1, 1], "width": 0.6, "widths": [0.6] })).is_err());
        assert!(run(&mut s, "cabinet", json!({ "kind": "base", "at": [1, 1], "widths": [] })).is_err());
        assert!(cabinets(&s).is_empty());
    }

    #[test]
    fn cabinet_refuses_a_run_past_the_wall_end_and_places_nothing() {
        let mut s = session();
        let r = run(&mut s, "cabinet", json!({ "kind": "base", "wall": "w0", "offset": 3.0, "widths": [0.6, 0.6, 0.6] }));
        assert!(r.is_err());
        assert!(cabinets(&s).is_empty());
        let r = run(&mut s, "cabinet", json!({ "kind": "base", "at": [0, 0], "widths": vec![0.3; 33] }));
        assert!(r.is_err());
        assert!(cabinets(&s).is_empty());
    }

    #[test]
    fn cabinet_edit_and_delete() {
        let mut s = session();
        let _ = run(&mut s, "cabinet", json!({ "kind": "base", "at": [1, 1] }));
        let r = run(&mut s, "cabinet.edit", json!({ "id": "cb0", "width": 0.9, "rotation": 90, "drawers": 2 }));
        assert!(r.is_ok(), "{r:?}");
        let c = cabinets(&s).first().cloned().unwrap_or_default();
        assert!((c.width - 0.9).abs() < 1e-12 && (c.rotation - 90.0).abs() < 1e-12 && c.drawers == 2);
        assert!(run(&mut s, "cabinet.edit", json!({ "id": "cb0", "width": 5.0 })).is_err());
        assert!(run(&mut s, "cabinet.edit", json!({ "id": "cb9", "width": 0.5 })).is_err());
        assert!((cabinets(&s).first().map_or(0.0, |c| c.width) - 0.9).abs() < 1e-12);
        let r = run(&mut s, "cabinet.delete", json!({ "id": "cb0" })).unwrap_or(Value::Null);
        assert_eq!(r["deleted"], "cb0");
        assert!(cabinets(&s).is_empty());
        assert!(run(&mut s, "cabinet.delete", json!({ "id": "cb0" })).is_err());
    }

    #[test]
    fn fixture_kinds_take_their_defaults() {
        let mut s = session();
        for (name, kind) in FIXTURE_KINDS {
            let r = run(&mut s, "fixture", json!({ "kind": name, "at": [2, 2] }));
            assert!(r.is_ok(), "{name}: {r:?}");
            let d = fixture_defaults(kind);
            let f = s.doc().ok().and_then(|doc| doc.arch.fixtures.last().cloned()).unwrap_or_default();
            assert_eq!(f.kind, kind);
            assert!((f.width - d.width).abs() < 1e-12 && (f.elevation - d.elevation).abs() < 1e-12);
        }
        let t = run(&mut s, "fixture.types", json!({})).unwrap_or(Value::Null);
        assert_eq!(t["types"].as_array().map(Vec::len), Some(8));
        let r = run(&mut s, "fixture", json!({ "kind": "toilet", "wall": "w0", "offset": 1.0 })).unwrap_or(Value::Null);
        assert_eq!(r["fixtures"], json!(["fx8"]));
        assert!(run(&mut s, "fixture.edit", json!({ "id": "fx8", "rotation": 45 })).is_ok());
        assert!(run(&mut s, "fixture.delete", json!({ "id": "fx8" })).is_ok());
    }

    #[test]
    fn fixture_refuses_a_hostile_size() {
        let mut s = session();
        for p in [
            json!({ "kind": "sink", "at": [0, 0], "width": 1e300 }),
            json!({ "kind": "sink", "at": [0, 0], "width": -1 }),
            json!({ "kind": "sink", "at": [0, 0], "height": "tall" }),
            json!({ "kind": "sink", "at": [0, 0], "elevation": 9 }),
            json!({ "kind": "bidet", "at": [0, 0] }),
            json!({ "kind": "sink", "wall": "w9", "offset": 1 }),
            json!({ "kind": "sink", "at": [0, 0], "level": "attic" }),
            json!({ "kind": "sink", "at": [1e9, 0] }),
            json!({ "kind": "sink" }),
        ] {
            assert!(run(&mut s, "fixture", p.clone()).is_err(), "{p}");
        }
        assert!(s.doc().map(|d| d.arch.fixtures.is_empty()).unwrap_or(false));
    }

    #[test]
    fn inspect_cabinets_report_counter_top() {
        let mut s = session();
        let _ = run(&mut s, "cabinet", json!({ "kind": "base", "wall": "w0", "offset": 1.0 }));
        let v = run(&mut s, "arch.inspect", json!({ "cabinets": true })).unwrap_or(Value::Null);
        let c = &v["cabinets"][0];
        assert_eq!(c["wall"], "w0", "{v}");
        let a = c["alongWall"].as_array().cloned().unwrap_or_default();
        assert_eq!(a.len(), 2, "{v}");
        assert!((a[0].as_f64().unwrap_or(0.0) - 0.7).abs() < 1e-9 && (a[1].as_f64().unwrap_or(0.0) - 1.3).abs() < 1e-9, "{v}");
        assert!((c["counterTop"].as_f64().unwrap_or(0.0) - (0.876 + 0.038)).abs() < 1e-9, "{v}");
    }

    #[test]
    fn inspect_lists_cabinets_with_counter_top() {
        let mut s = session();
        let _ = run(&mut s, "cabinet", json!({ "kind": "base", "wall": "w0", "offset": 1.0 }));
        let _ = run(&mut s, "cabinet", json!({ "kind": "wall", "wall": "w0", "offset": 2.0 }));
        let _ = run(&mut s, "fixture", json!({ "kind": "range", "wall": "w0", "offset": 3.0 }));
        let v = run(&mut s, "arch.inspect", json!({ "cabinets": true, "fixtures": true })).unwrap_or(Value::Null);
        let cabs = v["cabinets"].as_array().cloned().unwrap_or_default();
        assert_eq!(cabs.len(), 2, "{v}");
        assert_eq!(cabs[0]["id"], "cb0");
        assert_eq!(cabs[0]["kind"], "base");
        assert_eq!(cabs[0]["fronts"], "drawerOverDoor");
        assert_eq!(cabs[0]["layer"], "A-FLOR-CASE");
        assert!((cabs[0]["counterTop"].as_f64().unwrap_or(0.0) - 0.914).abs() < 1e-9, "{v}");
        assert!(cabs[1]["counterTop"].is_null());
        assert_eq!(v["fixtures"][0]["id"], "fx0");
        assert_eq!(v["fixtures"][0]["layer"], "A-FLOR-APPL");
    }

    #[test]
    fn inspect_fixture_and_furnishing_rows_report_room() {
        let mut s = Session::new();
        if let Ok(mut d) = s.doc_mut() {
            d.arch.rooms.push(cadcraft_doc::arch::Room {
                id: ArchId(0),
                level: 0,
                polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
                room_type: cadcraft_doc::arch::RoomType::Living,
                ..cadcraft_doc::arch::Room::default()
            });
            d.touch_arch();
        }

        // A toilet fixture inside the 6x4 room reports r0.
        let r = run(&mut s, "fixture", json!({ "kind": "toilet", "at": [3.0, 2.0] }));
        assert!(r.is_ok(), "{r:?}");
        let v = run(&mut s, "arch.inspect", json!({ "fixtures": true })).unwrap_or(Value::Null);
        assert_eq!(v["fixtures"][0]["room"], json!("r0"));

        // The same fixture moved outside the room reports null.
        let r = run(&mut s, "fixture.edit", json!({ "id": "fx0", "at": [9.0, 9.0] }));
        assert!(r.is_ok(), "{r:?}");
        let v2 = run(&mut s, "arch.inspect", json!({ "fixtures": true })).unwrap_or(Value::Null);
        assert_eq!(v2["fixtures"][0]["room"], Value::Null);

        // A furnishing at the room's centre reports r0.
        if let Ok(mut d) = s.doc_mut() {
            d.arch.push_furnishing(cadcraft_doc::arch::Furnishing {
                item: "armchair".into(),
                at: [3.0, 2.0],
                ..cadcraft_doc::arch::Furnishing::default()
            });
            d.touch_arch();
        }
        let v3 = run(&mut s, "arch.inspect", json!({ "furnishings": true })).unwrap_or(Value::Null);
        assert_eq!(v3["furnishings"][0]["room"], json!("r0"));
    }

    #[test]
    fn inspect_fixture_in_room_hole_reports_null() {
        let mut s = Session::new();
        if let Ok(mut d) = s.doc_mut() {
            d.arch.rooms.push(cadcraft_doc::arch::Room {
                id: ArchId(0),
                level: 0,
                polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 6.0), Vec2::new(0.0, 6.0)],
                holes: vec![vec![Vec2::new(2.0, 2.0), Vec2::new(4.0, 2.0), Vec2::new(4.0, 4.0), Vec2::new(2.0, 4.0)]],
                room_type: cadcraft_doc::arch::RoomType::Living,
                ..cadcraft_doc::arch::Room::default()
            });
            d.touch_arch();
        }

        // Fixture inside the hole.
        let r = run(&mut s, "fixture", json!({ "kind": "toilet", "at": [3.0, 3.0] }));
        assert!(r.is_ok(), "{r:?}");
        let v = run(&mut s, "arch.inspect", json!({ "fixtures": true })).unwrap_or(Value::Null);
        assert_eq!(v["fixtures"][0]["room"], Value::Null, "fixture in hole should report null room");

        // Fixture in the room but outside the hole.
        let r = run(&mut s, "fixture", json!({ "kind": "sink", "at": [1.0, 1.0] }));
        assert!(r.is_ok(), "{r:?}");
        let v2 = run(&mut s, "arch.inspect", json!({ "fixtures": true })).unwrap_or(Value::Null);
        assert_eq!(v2["fixtures"][1]["room"], json!("r0"), "fixture in room should report r0");
    }

    #[test]
    fn inspect_fixture_on_wrong_level_reports_null() {
        let mut s = Session::new();
        if let Ok(mut d) = s.doc_mut() {
            d.arch.rooms.push(cadcraft_doc::arch::Room {
                id: ArchId(0),
                level: 0,
                polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
                room_type: cadcraft_doc::arch::RoomType::Living,
                ..cadcraft_doc::arch::Room::default()
            });
            d.arch.levels.push(cadcraft_doc::arch::Level {
                index: 1,
                name: "Level 2".to_string(),
                elevation: 3.0,
                ..cadcraft_doc::arch::Level::default()
            });
            d.touch_arch();
        }

        // Fixture on level 1, room on level 0.
        let r = run(&mut s, "fixture", json!({ "kind": "toilet", "at": [3.0, 2.0], "level": 1 }));
        assert!(r.is_ok(), "{r:?}");
        let v = run(&mut s, "arch.inspect", json!({ "fixtures": true })).unwrap_or(Value::Null);
        assert_eq!(v["fixtures"][0]["room"], Value::Null, "fixture on different level should report null room");
    }

    #[test]
    fn inspect_free_standing_cabinet_reports_null_wall() {
        let mut s = session();
        // Place a cabinet free-standing (not against a wall)
        let r = run(&mut s, "cabinet", json!({ "kind": "base", "at": [2.0, 2.0] }));
        assert!(r.is_ok(), "{r:?}");
        let v = run(&mut s, "arch.inspect", json!({ "cabinets": true })).unwrap_or(Value::Null);
        let c = &v["cabinets"][0];
        assert_eq!(c["wall"], Value::Null, "free-standing cabinet should report null wall: {v}");
        assert_eq!(c["alongWall"], Value::Null, "free-standing cabinet should report null alongWall: {v}");
    }

    #[test]
    fn inspect_multiple_cabinets_each_report_own_host() {
        let mut s = Session::new();
        if let Ok(mut d) = s.doc_mut() {
            d.arch.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
            d.arch.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, 10.0), 0.2));
            d.touch_arch();
        }
        // Cabinet on w0 (horizontal wall)
        let r = run(&mut s, "cabinet", json!({ "kind": "base", "wall": "w0", "offset": 1.0 }));
        assert!(r.is_ok(), "{r:?}");
        // Cabinet on w1 (vertical wall)
        let r = run(&mut s, "cabinet", json!({ "kind": "base", "wall": "w1", "offset": 2.0 }));
        assert!(r.is_ok(), "{r:?}");
        let v = run(&mut s, "arch.inspect", json!({ "cabinets": true })).unwrap_or(Value::Null);
        assert_eq!(v["cabinets"][0]["wall"], "w0", "{v}");
        let a0 = v["cabinets"][0]["alongWall"].as_array().cloned().unwrap_or_default();
        assert!((a0[0].as_f64().unwrap_or(0.0) - 0.7).abs() < 1e-9 && (a0[1].as_f64().unwrap_or(0.0) - 1.3).abs() < 1e-9, "{v}");
        assert_eq!(v["cabinets"][1]["wall"], "w1", "{v}");
        let a1 = v["cabinets"][1]["alongWall"].as_array().cloned().unwrap_or_default();
        assert!((a1[0].as_f64().unwrap_or(0.0) - 1.7).abs() < 1e-9 && (a1[1].as_f64().unwrap_or(0.0) - 2.3).abs() < 1e-9, "{v}");
    }

    #[test]
    fn inspect_cabinet_at_wall_end_reports_correct_span() {
        let mut s = session();
        // Wall is 4m; place cabinet at offset 3.7 so it ends exactly at the wall end
        let r = run(&mut s, "cabinet", json!({ "kind": "base", "wall": "w0", "offset": 3.7 }));
        assert!(r.is_ok(), "{r:?}");
        let v = run(&mut s, "arch.inspect", json!({ "cabinets": true })).unwrap_or(Value::Null);
        let c = &v["cabinets"][0];
        assert_eq!(c["wall"], "w0", "{v}");
        let a = c["alongWall"].as_array().cloned().unwrap_or_default();
        assert!((a[0].as_f64().unwrap_or(0.0) - 3.4).abs() < 1e-9 && (a[1].as_f64().unwrap_or(0.0) - 4.0).abs() < 1e-9, "{v}");
    }
}
