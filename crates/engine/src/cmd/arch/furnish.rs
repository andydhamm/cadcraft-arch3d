//! Placing from the library (phase 14) and editing what was placed.
//!
//! `library.place` puts a library item into the drawing. A 3D symbol becomes a furnishing where
//! [`cadcraft_arch::library::place`] says it goes: at the click on the floor, backed onto a wall
//! face, hung from the ceiling, dropped into a base cabinet's counter (a sink), onto a sink (a
//! faucet) or into a gap between base cabinets (a dishwasher). A pull or a front style clicked over
//! a cabinet is swapped onto it instead. The parametric items run their own commands (door,
//! window, plant, lightfixture, cabinet, fixture, backdrop) inside this one, so the whole place is
//! one undo step. Materials and finishes are painted, never placed.
//!
//! `furnishing.edit`, `furnishing.delete` and `furnishing.centre` change a placed furnishing.
//! Every parameter is checked before anything changes: a bad one is refused, never clamped, and a
//! refused command leaves the drawing (and the undo history) as it was.

use cadcraft_arch::join::face_lines;
use cadcraft_arch::library::place::{PlaceOutcome, PlaceRequest, place};
use cadcraft_arch::library::{ItemKind, ItemRecord, layer_for};
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::{
    ArchId, ArchModel, CabinetPart, FrontStyle, Furnishing, FurnishingHost, MAX_FURNISHINGS, Placement, Surface, cabinet_label, furnishing_handle,
    furnishing_label, wall_label,
};
use cadcraft_geom::{Vec2, point_in_polygon};
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, bad, find_command};
use crate::prompt::{Accept, Input, Interactive, Prompt, Step};
use crate::{EngineError, Result, Session};

const PLACE: &str = "library.place";
const EDIT: &str = "furnishing.edit";
const DELETE: &str = "furnishing.delete";
const CENTRE: &str = "furnishing.centre";

/// How far from a wall's centre line a door or window item may be dropped, and how far from a
/// wall face a furnishing may be to centre on it.
const WALL_REACH: f64 = 0.3;
/// The largest plan coordinate (metres) a point may have, as for cabinets and fixtures.
const MAX_COORD: f64 = 1e6;
/// The largest size and elevation (metres).
const MAX_SIZE: f64 = 100.0;
const MAX_ELEVATION: f64 = 100.0;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new(PLACE, "Place From Library", run_place)
            .menu(&["Architecture", "Place From Library"])
            .params(
                "{item: \"armchair\", at: [x, y], rotation?, level?, elevation?, size?: [w, d, h], colour?, top?, bottom?, projection? (backdrop items)}",
            )
            .interactive(|_| Ok(Box::new(PlaceM::default()))),
        CommandSpec::new(EDIT, "Edit Furnishing", run_edit)
            .menu(&["Architecture", "Edit Furnishing"])
            .params("{id: \"f0\", at?: [x, y], rotation?, elevation?, size?: [w, d, h], mirrored?}"),
        CommandSpec::new(DELETE, "Delete Furnishing", run_delete).menu(&["Architecture", "Delete Furnishing"]).params("{id: \"f0\"}"),
        CommandSpec::new(CENTRE, "Center Furnishing", run_centre)
            .menu(&["Architecture", "Center Furnishing"])
            .params("{id: \"f0\", on: \"room\" | \"wall\"}"),
    ]
}

// ---------- parameters ----------

/// The parameters as an object (`null` is the empty one).
fn object<'a>(cmd: &str, p: &'a Value) -> Result<Option<&'a Map<String, Value>>> {
    match p {
        Value::Object(m) => Ok(Some(m)),
        Value::Null => Ok(None),
        _ => Err(bad(cmd, "the parameters must be an object")),
    }
}

/// A field that is present and not `null`.
fn get<'a>(m: Option<&'a Map<String, Value>>, key: &str) -> Option<&'a Value> {
    m.and_then(|m| m.get(key)).filter(|v| !v.is_null())
}

/// A required non-empty string.
fn text<'a>(cmd: &str, m: Option<&'a Map<String, Value>>, key: &str) -> Result<&'a str> {
    match get(m, key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim()),
        Some(Value::String(_)) => Err(bad(cmd, format!("`{key}` is empty"))),
        Some(_) => Err(bad(cmd, format!("`{key}` must be a string"))),
        None => Err(bad(cmd, format!("`{key}` is required"))),
    }
}

/// An optional finite number.
fn finite(cmd: &str, m: Option<&Map<String, Value>>, key: &str) -> Result<Option<f64>> {
    match get(m, key) {
        None => Ok(None),
        Some(v) => v.as_f64().filter(|x| x.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite number"))),
    }
}

/// An optional elevation: a finite number of metres in 0..=100.
fn elevation(cmd: &str, m: Option<&Map<String, Value>>) -> Result<Option<f64>> {
    match finite(cmd, m, "elevation")? {
        Some(e) if !(0.0..=MAX_ELEVATION).contains(&e) => Err(bad(cmd, format!("`elevation` must be between 0 and {MAX_ELEVATION} m; got {e}"))),
        e => Ok(e),
    }
}

/// An optional plan point `[x, y]` of finite numbers within [`MAX_COORD`].
fn point(cmd: &str, m: Option<&Map<String, Value>>, key: &str) -> Result<Option<[f64; 2]>> {
    let Some(v) = get(m, key) else { return Ok(None) };
    let err = || bad(cmd, format!("`{key}` must be [x, y]: two finite numbers within {MAX_COORD} m"));
    let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(err)?;
    let c = |i: usize| a.get(i).and_then(Value::as_f64).filter(|x| x.is_finite() && x.abs() <= MAX_COORD).ok_or_else(err);
    Ok(Some([c(0)?, c(1)?]))
}

/// An optional size `[w, d, h]`: three finite numbers of metres in (0, 100].
fn size(cmd: &str, m: Option<&Map<String, Value>>) -> Result<Option<[f64; 3]>> {
    let Some(v) = get(m, "size") else { return Ok(None) };
    let err = || bad(cmd, format!("`size` must be [w, d, h]: three finite numbers above 0 and at most {MAX_SIZE} m"));
    let a = v.as_array().filter(|a| a.len() == 3).ok_or_else(err)?;
    let c = |i: usize| a.get(i).and_then(Value::as_f64).filter(|x| x.is_finite() && *x > 0.0 && *x <= MAX_SIZE).ok_or_else(err);
    Ok(Some([c(0)?, c(1)?, c(2)?]))
}

/// An optional `true`/`false`.
fn flag(cmd: &str, m: Option<&Map<String, Value>>, key: &str) -> Result<Option<bool>> {
    match get(m, key) {
        None => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(bad(cmd, format!("`{key}` must be true or false"))),
    }
}

/// The level as the cabinet command reads it: an index of an existing level (0 in a drawing
/// without levels) or a level's name; the first level when not given.
fn level(cmd: &str, doc: &Drawing, m: Option<&Map<String, Value>>) -> Result<u32> {
    let Some(v) = get(m, "level") else { return Ok(doc.arch.levels.first().map_or(0, |l| l.index)) };
    let found = match v {
        Value::Number(_) => {
            v.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| doc.arch.level(*n).is_some() || (*n == 0 && doc.arch.levels.is_empty()))
        }
        Value::String(s) => doc.arch.levels.iter().find(|l| l.name == s.trim()).map(|l| l.index),
        _ => None,
    };
    found.ok_or_else(|| bad(cmd, format!("unknown level `{v}`")))
}

/// The furnishing `f<n>` of the drawing (any other id, a cabinet's or a fixture's, is refused).
fn furnishing_id(cmd: &str, doc: &Drawing, raw: &str) -> Result<ArchId> {
    let t = raw.trim().to_ascii_lowercase();
    t.strip_prefix('f')
        .filter(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|d| d.parse::<u32>().ok())
        .map(ArchId)
        .filter(|id| doc.arch.furnishing(*id).is_some())
        .ok_or_else(|| bad(cmd, format!("unknown furnishing `{raw}`")))
}

/// A furnishing's host as the replies (and `arch.inspect`) write it, with object labels.
fn host_json(h: &FurnishingHost) -> Value {
    match h {
        FurnishingHost::None => Value::Null,
        FurnishingHost::Wall { wall } => json!({ "kind": "wall", "wall": wall_label(*wall) }),
        FurnishingHost::Cabinet { cabinet } => json!({ "kind": "cabinet", "cabinet": cabinet_label(*cabinet) }),
        FurnishingHost::Sink { furnishing } => json!({ "kind": "sink", "furnishing": furnishing_label(*furnishing) }),
        FurnishingHost::Gap { left, right } => json!({ "kind": "gap", "left": cabinet_label(*left), "right": cabinet_label(*right) }),
    }
}

/// Drop the symbols no furnishing and no cabinet pull uses any more.
fn drop_unused_symbols(a: &mut ArchModel) {
    let ArchModel { symbols, furnishings, cabinets, .. } = a;
    symbols.retain(|key, _| furnishings.iter().any(|f| f.symbol == *key) || cabinets.iter().any(|c| c.pull_symbol.as_deref() == Some(key.as_str())));
}

// ---------- library.place ----------

/// What every `library.place` takes, checked before the item's kind is looked at.
struct Request {
    at: [f64; 2],
    rotation: Option<f64>,
    level: u32,
    elevation: Option<f64>,
    size: Option<[f64; 3]>,
}

fn run_place(s: &mut Session, p: &Value) -> Result<Value> {
    let id = text(PLACE, object(PLACE, p)?, "item")?.to_string();
    // The item's Set as Default parameters fill in what this call leaves out.
    let p = super::defaults::merged(s, &format!("furnishing.{id}"), p);
    let m = object(PLACE, &p)?;
    s.library.sync_io();
    let item = s.library.item(&id).cloned().ok_or_else(|| bad(PLACE, format!("no library item `{id}`")))?;
    let doc = s.doc()?;
    let req = Request {
        at: point(PLACE, m, "at")?.ok_or_else(|| bad(PLACE, "`at` is required"))?,
        rotation: finite(PLACE, m, "rotation")?,
        level: level(PLACE, doc, m)?,
        elevation: elevation(PLACE, m)?,
        size: size(PLACE, m)?,
    };
    // A copy of a library item places as the item it copies, under the copy's name.
    let item = resolve_copy(s, item)?;
    place_record(s, &item, &req, m)
}

/// A user item that is a copy of another item (no stored objects, a `source_item`) as the record it
/// places: the source's record with the copy's `name` and `size`. Any other item comes back as it is.
fn resolve_copy(s: &Session, item: ItemRecord) -> Result<ItemRecord> {
    if item.kind != ItemKind::Group || !item.id.starts_with("user:") {
        return Ok(item);
    }
    let Some(dir) = s.library.user_dir() else { return Ok(item) };
    let Ok(file) = crate::library::user::read_user_file(&dir, &item.id) else { return Ok(item) };
    let o = &file.objects;
    let Some(source) = file.source_item.filter(|_| o.furnishings.is_empty() && o.cabinets.is_empty() && o.fixtures.is_empty()) else {
        return Ok(item);
    };
    let mut rec = s
        .library
        .item(&source)
        .cloned()
        .ok_or_else(|| bad(PLACE, format!("`{}` is a copy of `{source}`, which is not in the library any more", item.id)))?;
    rec.name = item.name;
    if item.size.is_some() {
        rec.size = item.size;
    }
    Ok(rec)
}

/// Place `item` by its kind: the rules for stock items, shared by a copy through its source.
fn place_record(s: &mut Session, item: &ItemRecord, req: &Request, m: Option<&Map<String, Value>>) -> Result<Value> {
    match item.kind {
        ItemKind::Symbol => place_symbol(s, item, req),
        ItemKind::FrontStyle => swap_front_style(s, item, req),
        ItemKind::Door | ItemKind::Window => place_opening(s, item, req),
        ItemKind::Plant => {
            let species = built_in_name(item, "plant:")?;
            via(s, "plant", json!({ "species": species, "at": req.at }))
        }
        ItemKind::LightFixture => {
            let mut q = json!({ "type": built_in_name(item, "lightfixture:")?, "at": req.at, "level": req.level });
            if let Some(r) = req.rotation {
                q["rotation"] = json!(r);
            }
            via(s, "lightfixture", q)
        }
        ItemKind::Cabinet => via(s, "cabinet", casework_params(built_in_name(item, "cabinet:")?, req)),
        ItemKind::Fixture => via(s, "fixture", casework_params(built_in_name(item, "fixture:")?, req)),
        ItemKind::Backdrop => via(s, "backdrop", backdrop_params(item, m)),
        ItemKind::Finish | ItemKind::Material => Err(bad(PLACE, "a material is painted: use paint.material")),
        ItemKind::Molding => Err(bad(PLACE, "a molding is applied with room.moldings, moldingline or a cabinet's crown")),
        ItemKind::Group => crate::library::user::place_group(s, item, Vec2::new(req.at[0], req.at[1]), req.rotation.unwrap_or(0.0)),
    }
}

/// The name a built-in parametric item carries after its prefix (`plant:Sugar Maple`).
fn built_in_name<'a>(item: &'a ItemRecord, prefix: &str) -> Result<&'a str> {
    item.id.strip_prefix(prefix).filter(|n| !n.is_empty()).ok_or_else(|| bad(PLACE, format!("`{}` is not a built-in `{prefix}` item", item.id)))
}

/// The backdrop command's parameters for a backdrop item: the built-in colour and gradient take
/// the call's `colour` or `top` and `bottom`; an image item its `projection`.
fn backdrop_params(item: &ItemRecord, m: Option<&Map<String, Value>>) -> Value {
    let (mut q, forward): (Value, &[&str]) = match item.id.as_str() {
        "backdrop:colour" => (json!({ "kind": "colour" }), &["colour"]),
        "backdrop:gradient" => (json!({ "kind": "gradient" }), &["top", "bottom"]),
        _ => (json!({ "kind": "image", "item": item.id }), &["projection"]),
    };
    for k in forward {
        if let Some(v) = get(m, k) {
            q[*k] = v.clone();
        }
    }
    q
}

/// The cabinet or fixture command's parameters for a built-in item of `kind`.
fn casework_params(kind: &str, req: &Request) -> Value {
    let mut q = json!({ "kind": kind, "at": req.at, "rotation": req.rotation.unwrap_or(0.0), "level": req.level });
    if let Some([w, d, h]) = req.size {
        q["width"] = json!(w);
        q["depth"] = json!(d);
        q["height"] = json!(h);
    }
    if let Some(e) = req.elevation {
        q["elevation"] = json!(e);
    }
    q
}

/// Run command `id` inside `library.place` (one undo step for both) and reply
/// `{"via": id, ...its reply}`. When it refuses, the drawing is put back as it was.
fn via(s: &mut Session, id: &'static str, params: Value) -> Result<Value> {
    let spec = find_command(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
    let before = s.state()?.doc.clone();
    match (spec.run)(s, &params) {
        Ok(reply) => {
            let mut out = Map::new();
            out.insert("via".to_string(), json!(id));
            match reply {
                Value::Object(r) => out.extend(r.into_iter().filter(|(k, _)| k != "via")),
                other => {
                    out.insert("reply".to_string(), other);
                }
            }
            Ok(Value::Object(out))
        }
        Err(e) => {
            s.state_mut()?.doc = before;
            Err(e)
        }
    }
}

/// A door or window item: the door or window command in the nearest wall (by its centre line,
/// within 0.3 m on the level), at the click's distance along it from the wall's start.
fn place_opening(s: &mut Session, item: &ItemRecord, req: &Request) -> Result<Value> {
    let doc = s.doc()?;
    let at = Vec2::new(req.at[0], req.at[1]);
    let mut best: Option<(f64, ArchId, f64)> = None;
    for w in doc.arch.walls.iter().filter(|w| w.base_level == req.level && !w.divider) {
        let axis = w.end - w.start;
        let len = axis.len();
        if !(len > 1e-9) || !len.is_finite() {
            continue;
        }
        let dir = axis * (1.0 / len);
        let along = (at - w.start).dot(dir);
        let d = at.dist(w.start + dir * along.clamp(0.0, len));
        if d <= WALL_REACH + 1e-9 && best.is_none_or(|(bd, bid, _)| d < bd - 1e-12 || ((d - bd).abs() <= 1e-12 && w.id < bid)) {
            best = Some((d, w.id, along));
        }
    }
    let Some((_, wall, offset)) = best else {
        return Err(bad(PLACE, format!("a door or window goes in a wall: no wall within {WALL_REACH} m of ({}, {})", req.at[0], req.at[1])));
    };
    let cmd = if item.kind == ItemKind::Door { "door" } else { "window" };
    let mut q = json!({ "wall": wall_label(wall), "offset": offset });
    if let Some(style) = item.style.as_deref().filter(|st| !st.is_empty()) {
        q["style"] = json!(style);
    }
    if let Some([w, _, h]) = req.size {
        q["width"] = json!(w);
        q["height"] = json!(h);
    }
    if let Some(e) = req.elevation {
        q["sill"] = json!(e);
    }
    via(s, cmd, q).map_err(|e| match e {
        EngineError::UnknownCommand(_) => bad(PLACE, format!("there is no {cmd} command in this build")),
        other => other,
    })
}

/// A front-style item over a cabinet sets its door or drawer style (`frontstyle:door-<style>`,
/// `frontstyle:drawer-<style>`).
fn swap_front_style(s: &mut Session, item: &ItemRecord, req: &Request) -> Result<Value> {
    let rest = item.id.strip_prefix("frontstyle:").unwrap_or_default();
    let (part, style) = match rest.split_once('-') {
        Some(("door", st)) => ("door", FrontStyle::parse(st)),
        Some(("drawer", st)) => ("drawer", FrontStyle::parse(st)),
        _ => ("", None),
    };
    let Some(style) = style else {
        return Err(bad(PLACE, format!("`{}` is not a front style: its id must be frontstyle:door-<style> or frontstyle:drawer-<style>", item.id)));
    };
    let doc = s.doc()?;
    let r = PlaceRequest {
        symbol_type: item.symbol_type.clone(),
        kind: ItemKind::FrontStyle,
        size: req.size.unwrap_or([1.0, 1.0, 1.0]),
        at: req.at,
        level: req.level,
        rotation: req.rotation,
        ..Default::default()
    };
    let cabinet = match place(&doc.arch, &r).map_err(|e| bad(PLACE, e))? {
        PlaceOutcome::Swap { cabinet, part: CabinetPart::Front } if doc.arch.cabinets.iter().any(|c| c.id == cabinet) => cabinet,
        _ => return Err(bad(PLACE, "a front style goes on a cabinet's fronts")),
    };
    let d = s.doc_mut()?;
    if let Some(c) = d.arch.cabinets.iter_mut().find(|c| c.id == cabinet) {
        if part == "door" {
            c.door_style = style;
        } else {
            c.drawer_style = style;
        }
    }
    d.touch_arch();
    Ok(json!({ "swapped": { "cabinet": cabinet_label(cabinet), "part": part } }))
}

/// A 3D symbol: a new furnishing where the item goes, or a pull swapped onto the cabinet under it.
fn place_symbol(s: &mut Session, item: &ItemRecord, req: &Request) -> Result<Value> {
    let placement = match item.placement.as_deref() {
        None => Placement::Floor,
        Some(p) => Placement::parse(p).ok_or_else(|| bad(PLACE, format!("`{}` has an unknown placement `{p}`", item.id)))?,
    };
    let mesh = s.library.symbol(&item.id).map_err(|e| bad(PLACE, e))?;
    let size = req.size.or(item.size).unwrap_or_else(|| mesh.extents());
    let doc = s.doc()?;
    // `kind` stays the default, `ItemKind::Symbol`: this is a symbol.
    let r = PlaceRequest {
        placement,
        symbol_type: item.symbol_type.clone(),
        size,
        elevation: item.elevation,
        cutout: item.cutout,
        at: req.at,
        level: req.level,
        rotation: req.rotation,
        ..Default::default()
    };
    match place(&doc.arch, &r).map_err(|e| bad(PLACE, e))? {
        PlaceOutcome::Place { at, rotation, elevation, host } => {
            if doc.arch.furnishings.len() >= MAX_FURNISHINGS {
                return Err(bad(PLACE, format!("at most {MAX_FURNISHINGS} furnishings are allowed")));
            }
            let elevation = req.elevation.unwrap_or(elevation);
            if !(at.iter().all(|v| v.is_finite()) && rotation.is_finite() && elevation.is_finite()) {
                return Err(bad(PLACE, "the item would not sit at a finite place"));
            }
            let key = mesh.key.clone();
            let f = Furnishing {
                item: item.id.clone(),
                symbol: key.clone(),
                name: item.name.clone(),
                category: item.category.clone(),
                symbol_type: item.symbol_type.clone(),
                placement,
                level: req.level,
                at,
                rotation,
                elevation,
                size,
                host: host.clone(),
                cutout: item.cutout,
                ..Default::default()
            };
            let d = s.doc_mut()?;
            d.ensure_layer(layer_for(&item.category, &item.symbol_type));
            d.arch.symbols.entry(key).or_insert(mesh);
            let id = d.arch.push_furnishing(f);
            d.touch_arch();
            Ok(json!({
                "furnishing": furnishing_label(id),
                // The object handle every command takes (`handle_json`'s short spelling is not one).
                "handle": furnishing_handle(id).hex(),
                "host": host_json(&host),
                "at": at,
                "rotation": rotation,
                "elevation": elevation,
                "size": size,
            }))
        }
        PlaceOutcome::Swap { cabinet, part: CabinetPart::Pull } if doc.arch.cabinets.iter().any(|c| c.id == cabinet) => {
            let key = mesh.key.clone();
            let d = s.doc_mut()?;
            d.arch.symbols.entry(key.clone()).or_insert(mesh);
            let old = d.arch.cabinets.iter_mut().find(|c| c.id == cabinet).and_then(|c| {
                c.pull = Some(item.id.clone());
                c.pull_symbol.replace(key.clone())
            });
            if old.is_some_and(|o| o != key) {
                drop_unused_symbols(&mut d.arch);
            }
            d.touch_arch();
            Ok(json!({ "swapped": { "cabinet": cabinet_label(cabinet), "part": "pull" } }))
        }
        PlaceOutcome::Swap { .. } => Err(bad(PLACE, format!("`{}` is a 3D symbol: only pulls and front styles swap onto a cabinet", item.id))),
    }
}

// ---------- furnishing.edit / .delete / .centre ----------

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    let m = object(EDIT, p)?;
    let doc = s.doc()?;
    let id = furnishing_id(EDIT, doc, text(EDIT, m, "id")?)?;
    let at = point(EDIT, m, "at")?;
    let rotation = finite(EDIT, m, "rotation")?;
    let elevation = elevation(EDIT, m)?;
    let size = size(EDIT, m)?;
    let mirrored = flag(EDIT, m, "mirrored")?;
    if at.is_none() && rotation.is_none() && elevation.is_none() && size.is_none() && mirrored.is_none() {
        return Err(bad(EDIT, "nothing to change: give at, rotation, elevation, size or mirrored"));
    }
    let mut f = doc.arch.furnishing(id).cloned().ok_or_else(|| bad(EDIT, format!("unknown furnishing `{}`", furnishing_label(id))))?;
    if let Some(at) = at {
        // Moved by hand, a hosted item leaves its host.
        if at != f.at {
            f.host = FurnishingHost::None;
        }
        f.at = at;
    }
    if let Some(r) = rotation {
        f.rotation = r;
    }
    if let Some(e) = elevation {
        f.elevation = e;
    }
    if let Some(sz) = size {
        f.size = sz;
    }
    if let Some(mi) = mirrored {
        f.mirrored = mi;
    }
    let d = s.doc_mut()?;
    if let Some(slot) = d.arch.furnishing_mut(id) {
        *slot = f;
    }
    d.touch_arch();
    Ok(json!({ "furnishing": furnishing_label(id) }))
}

fn run_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let m = object(DELETE, p)?;
    let id = furnishing_id(DELETE, s.doc()?, text(DELETE, m, "id")?)?;
    let d = s.doc_mut()?;
    let a = &mut d.arch;
    a.furnishings.retain(|f| f.id != id);
    // A faucet on a deleted sink stays where it is, free standing.
    for f in &mut a.furnishings {
        if f.host == (FurnishingHost::Sink { furnishing: id }) {
            f.host = FurnishingHost::None;
        }
    }
    a.paints.retain(|key, _| !matches!(Surface::parse(key), Some(Surface::Furnishing { furnishing, .. }) if furnishing == id));
    drop_unused_symbols(a);
    d.touch_arch();
    Ok(json!({ "deleted": furnishing_label(id) }))
}

fn run_centre(s: &mut Session, p: &Value) -> Result<Value> {
    let m = object(CENTRE, p)?;
    let doc = s.doc()?;
    let id = furnishing_id(CENTRE, doc, text(CENTRE, m, "id")?)?;
    let on = text(CENTRE, m, "on")?;
    let f = doc.arch.furnishing(id).cloned().ok_or_else(|| bad(CENTRE, format!("unknown furnishing `{}`", furnishing_label(id))))?;
    let (at, keep_host) = match on {
        "room" => (room_centre(&doc.arch, &f)?, false),
        "wall" => (wall_centre(&doc.arch, &f)?, true),
        other => return Err(bad(CENTRE, format!("`on` must be \"room\" or \"wall\"; got `{other}`"))),
    };
    let d = s.doc_mut()?;
    if let Some(slot) = d.arch.furnishing_mut(id) {
        if !keep_host && at != slot.at {
            slot.host = FurnishingHost::None;
        }
        slot.at = at;
    }
    d.touch_arch();
    Ok(json!({ "furnishing": furnishing_label(id), "on": on, "at": at }))
}

/// The centre of the bounding box of the room (on the furnishing's level) that holds it.
fn room_centre(a: &ArchModel, f: &Furnishing) -> Result<[f64; 2]> {
    let p = Vec2::new(f.at[0], f.at[1]);
    let mut rooms: Vec<_> = a.rooms.iter().filter(|r| r.level == f.level && r.polygon.len() >= 3).collect();
    rooms.sort_by_key(|r| r.id);
    let room = rooms
        .into_iter()
        .find(|r| point_in_polygon(&r.polygon, p) && !r.holes.iter().any(|h| h.len() >= 3 && point_in_polygon(h, p)))
        .ok_or_else(|| bad(CENTRE, format!("{} is not in a room", furnishing_label(f.id))))?;
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for v in &room.polygon {
        lo = [lo[0].min(v.x), lo[1].min(v.y)];
        hi = [hi[0].max(v.x), hi[1].max(v.y)];
    }
    let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    if !c.iter().all(|v| v.is_finite()) {
        return Err(bad(CENTRE, "the room has no finite extent"));
    }
    Ok(c)
}

/// The distance from point `p` to the segment `a`-`b`.
fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let len2 = ab.len2();
    let t = if len2 > 1e-18 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    p.dist(a + ab * t)
}

/// Slide the furnishing along its host wall's face (or the nearest wall face within 0.3 m of its
/// footprint) to the face's midpoint, keeping its distance from the face.
fn wall_centre(a: &ArchModel, f: &Furnishing) -> Result<[f64; 2]> {
    let at = Vec2::new(f.at[0], f.at[1]);
    let (s, c) = f.rotation.to_radians().sin_cos();
    let (hx, hy) = (f.size[0] / 2.0, f.size[1] / 2.0);
    let mut probes = vec![at];
    probes.extend([(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)].map(|(x, y)| Vec2::new(at.x + x * c - y * s, at.y + x * s + y * c)));
    let host = match f.host {
        FurnishingHost::Wall { wall } => a.wall(wall).map(|w| w.id),
        _ => None,
    };
    // (distance, wall id, face index, face)
    let mut best: Option<(f64, ArchId, usize, (Vec2, Vec2))> = None;
    for w in a.walls.iter().filter(|w| host.map_or(w.base_level == f.level && !w.divider, |h| w.id == h)) {
        let Some(faces) = face_lines(w) else { continue };
        for (i, (p, q)) in faces.into_iter().enumerate() {
            if !((q - p).len2() > 1e-18) || !p.is_finite() || !q.is_finite() {
                continue;
            }
            let d = probes.iter().map(|x| segment_distance(*x, p, q)).fold(f64::INFINITY, f64::min);
            if (host.is_some() || d <= WALL_REACH + 1e-9)
                && best.is_none_or(|(bd, bid, bi, _)| d < bd - 1e-12 || ((d - bd).abs() <= 1e-12 && (w.id, i) < (bid, bi)))
            {
                best = Some((d, w.id, i, (p, q)));
            }
        }
    }
    let Some((_, _, _, (p, q))) = best else {
        return Err(bad(CENTRE, format!("{} has no wall face within {WALL_REACH} m to centre on", furnishing_label(f.id))));
    };
    let dir = (q - p).normalized();
    let mid = (p + q) * 0.5;
    let moved = at + dir * (mid - at).dot(dir);
    if !moved.is_finite() {
        return Err(bad(CENTRE, "the wall face has no finite midpoint"));
    }
    Ok([moved.x, moved.y])
}

// ---------- interactive ----------

/// Asks for the library item, then the point (no rubber band), and places it through the JSON form.
#[derive(Default)]
struct PlaceM {
    item: Option<String>,
}

impl Interactive for PlaceM {
    fn name(&self) -> &'static str {
        PLACE
    }

    fn prompt(&self, _s: &Session) -> Prompt {
        match self.item {
            None => Prompt::new("Library item:", Accept::TEXT),
            Some(_) => Prompt::new("Specify insertion point:", Accept::POINT),
        }
    }

    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match (&self.item, i) {
            (_, Input::Cancel | Input::Enter) => Ok(Step::Cancel),
            (None, Input::Text(t) | Input::Keyword(t)) => {
                let t = t.trim();
                s.library.sync_io();
                if s.library.item(t).is_none() {
                    return Err(bad(PLACE, format!("no library item `{t}`")));
                }
                self.item = Some(t.to_string());
                Ok(Step::Continue)
            }
            (Some(item), Input::Point(pt)) if pt.is_finite() => s.execute(PLACE, &json!({ "item": item, "at": [pt.x, pt.y] })).map(|_| Step::Done),
            _ => Ok(Step::Continue),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::testing::install_io;
    use cadcraft_doc::arch::Paint;

    /// A session with the test readers (the sample set, a fake glTF reader: every symbol a box).
    fn session() -> Session {
        install_io();
        Session::new()
    }

    fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
        s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
    }

    fn refused(s: &mut Session, cmd: &str, p: Value) -> String {
        let before = s.doc().map(|d| d.arch.clone()).unwrap_or_default();
        let undo = s.state().map(|st| st.undo.len()).unwrap_or_default();
        let err = match s.execute(cmd, &p) {
            Ok(r) => panic!("{cmd} {p} should be refused, replied {r}"),
            Err(e) => e.to_string(),
        };
        assert_eq!(s.doc().map(|d| d.arch.clone()).unwrap_or_default(), before, "{cmd} {p} changed the model");
        assert_eq!(s.state().map(|st| st.undo.len()).unwrap_or_default(), undo, "{cmd} {p} made an undo step");
        err
    }

    fn model(s: &Session) -> ArchModel {
        s.doc().map(|d| d.arch.clone()).unwrap_or_default()
    }

    fn near(a: f64, b: f64, what: &str) {
        assert!((a - b).abs() < 1e-9, "{what}: {a} != {b}");
    }

    fn record(s: &Session, id: &str) -> ItemRecord {
        s.library.item(id).cloned().unwrap_or_else(|| panic!("no sample item {id}"))
    }

    /// The phase-13 test room: a closed 4 x 4 m ring (inner faces x, y = 0 and 4; w0 along y = -0.1),
    /// 0.2 m thick, its room detected.
    fn room() -> Session {
        let mut s = session();
        let ring = json!([[-0.1, -0.1], [4.1, -0.1], [4.1, 4.1], [-0.1, 4.1], [-0.1, -0.1]]);
        run(&mut s, "wall", json!({ "points": ring, "thickness": 0.2, "exterior": true }));
        run(&mut s, "roomdetect", json!({}));
        s
    }

    /// `room()` with a 0.9 m base cabinet backed onto w0 at x = 0.9 (x 0.45..1.35, y 0..0.6).
    fn kitchen() -> Session {
        let mut s = room();
        run(&mut s, "cabinet", json!({ "kind": "base", "wall": "w0", "offset": 1.0, "width": 0.9, "fronts": "drawerOverDoor" }));
        s
    }

    #[test]
    fn place_a_floor_symbol() {
        let mut s = session();
        let chair = record(&s, "armchair");
        let r = run(&mut s, PLACE, json!({ "item": "armchair", "at": [2.0, 3.0] }));
        assert_eq!(r["furnishing"], "f0", "{r}");
        assert_eq!(r["handle"], json!(furnishing_handle(ArchId(0)).hex()));
        assert_eq!(r["host"], Value::Null);
        assert_eq!(r["at"], json!([2.0, 3.0]));
        assert_eq!((r["rotation"].as_f64(), r["elevation"].as_f64()), (Some(0.0), Some(0.0)));
        assert_eq!(r["size"], json!(chair.size), "the size comes from the catalog record");
        let m = model(&s);
        let f = &m.furnishings[0];
        assert_eq!(
            (f.item.as_str(), f.name.as_str(), f.category.as_str(), f.symbol_type.as_str()),
            ("armchair", chair.name.as_str(), "Furniture", "chair/stool")
        );
        assert_eq!((f.placement, f.level, f.host.clone(), f.mirrored, f.cutout), (Placement::Floor, 0, FurnishingHost::None, false, None));
        assert!(m.symbols.contains_key(&f.symbol), "the furnishing's symbol is in the model");
        assert!(s.doc().is_ok_and(|d| d.layer("I-FURN").is_some()), "its layer exists");

        // Given rotation, elevation and size override; a named level.
        run(&mut s, "level", json!({ "above": 0 }));
        let r = run(
            &mut s,
            PLACE,
            json!({ "item": "armchair", "at": [1.0, 1.0], "rotation": 30, "elevation": 0.25, "size": [0.8, 0.7, 1.0], "level": "Level 2" }),
        );
        let f = model(&s).furnishings[1].clone();
        assert_eq!((f.at, f.rotation, f.elevation, f.size, f.level), ([1.0, 1.0], 30.0, 0.25, [0.8, 0.7, 1.0], 1), "{r}");

        // One undo step each.
        assert_eq!(s.undo().ok().flatten().as_deref(), Some("Place From Library"));
        assert_eq!(model(&s).furnishings.len(), 1);

        // The reply's handle is the object's handle: move takes it as it takes any other.
        let r = run(&mut s, PLACE, json!({ "item": "armchair", "at": [3.0, 1.0] }));
        assert_eq!(r["furnishing"], "f1", "undo took f1 back, so its id is free again: {r}");
        let moved = run(&mut s, "move", json!({ "handles": [r["handle"].clone()], "delta": [0.5, 0.0] }));
        assert_eq!(moved, json!({ "moved": 1 }), "{r}");
        assert_eq!(model(&s).furnishing(ArchId(1)).map(|f| f.at), Some([3.5, 1.0]));
    }

    #[test]
    fn place_puts_the_symbol_in_the_model_once() {
        let mut s = session();
        for at in [[1.0, 1.0], [2.0, 2.0], [3.0, 3.0]] {
            run(&mut s, PLACE, json!({ "item": "armchair", "at": at }));
        }
        let m = model(&s);
        assert_eq!(m.furnishings.len(), 3);
        assert_eq!(m.symbols.len(), 1, "three armchairs share one symbol");
        let key = &m.furnishings[0].symbol;
        assert!(m.furnishings.iter().all(|f| &f.symbol == key) && m.symbols.contains_key(key));
        assert_eq!(m.symbols.get(key).map(|sym| sym.key.as_str()), Some(key.as_str()), "keyed by the symbol's own key");
        run(&mut s, PLACE, json!({ "item": "sofa-futon", "at": [1.0, 3.0] }));
        assert_eq!(model(&s).symbols.len(), 2, "another item adds its own symbol");
    }

    #[test]
    fn place_on_a_wall_and_a_ceiling() {
        let mut s = room();
        let rack = record(&s, "towel-rack");
        let [_, depth, _] = rack.size.unwrap_or_default();
        // Clicked 0.2 m off w0's inner face (y = 0): its back on the face, at its catalog elevation.
        let r = run(&mut s, PLACE, json!({ "item": "towel-rack", "at": [2.0, 0.2] }));
        assert_eq!(r["host"], json!({ "kind": "wall", "wall": "w0" }), "{r}");
        let f = model(&s).furnishings[0].clone();
        near(f.at[0], 2.0, "rack x");
        near(f.at[1], depth / 2.0, "rack back on the face");
        near(f.elevation, rack.elevation.unwrap_or(-1.0), "catalog elevation");
        near(f.rotation, 180.0, "its front faces into the room");
        assert_eq!(f.host, FurnishingHost::Wall { wall: ArchId(0) });
        // A given elevation overrides the catalog's.
        let r = run(&mut s, PLACE, json!({ "item": "towel-rack", "at": [3.0, 0.1], "elevation": 1.1 }));
        near(r["elevation"].as_f64().unwrap_or_default(), 1.1, "given elevation");

        // A ceiling item in the 2.4 m room has its top at the ceiling.
        let pendant = record(&s, "light-pendant");
        let h = pendant.size.unwrap_or_default()[2];
        let r = run(&mut s, PLACE, json!({ "item": "light-pendant", "at": [3.0, 3.0] }));
        near(r["elevation"].as_f64().unwrap_or_default() + h, 2.4, "pendant top");
        assert_eq!(model(&s).furnishings[2].placement, Placement::Ceiling);

        // Far from a wall face, outside every room: refused, nothing placed.
        assert!(refused(&mut s, PLACE, json!({ "item": "towel-rack", "at": [2.0, 2.0] })).contains("no wall face within 0.3 m"));
        assert!(refused(&mut s, PLACE, json!({ "item": "light-pendant", "at": [9.0, 9.0] })).contains("ceiling item goes in a room"));
    }

    #[test]
    fn place_a_sink_into_a_cabinet() {
        let mut s = kitchen();
        let sink = record(&s, "own:sink-kitchen");
        let r = run(&mut s, PLACE, json!({ "item": "own:sink-kitchen", "at": [0.95, 0.35] }));
        assert_eq!(r["host"], json!({ "kind": "cabinet", "cabinet": "cb0" }), "{r}");
        let f = model(&s).furnishings[0].clone();
        near(f.at[0], 0.9, "sink centred on the cabinet");
        near(f.at[1], 0.3, "sink centred on its depth");
        assert_eq!(f.cutout, sink.cutout, "the cut-out comes from the record");
        assert_eq!(f.placement, Placement::Counter);
        // The faucet snaps to the sink's back centre, on its top.
        let r = run(&mut s, PLACE, json!({ "item": "faucet-kitchen-chrome", "at": [0.9, 0.3] }));
        assert_eq!(r["host"], json!({ "kind": "sink", "furnishing": "f0" }), "{r}");
        let faucet = model(&s).furnishings[1].clone();
        near(faucet.at[1] - faucet.size[1] / 2.0, 0.3 - f.size[1] / 2.0, "faucet back on the sink's back edge");
        near(faucet.elevation, f.elevation + f.size[2], "faucet on the sink's top");
        assert!(s.doc().is_ok_and(|d| d.layer("P-FLOR-FIXT").is_some()));
    }

    #[test]
    fn place_a_pull_swaps_instead() {
        let mut s = kitchen();
        let r = run(&mut s, PLACE, json!({ "item": "own:pull-bar-128", "at": [0.9, 0.3] }));
        assert_eq!(r, json!({ "swapped": { "cabinet": "cb0", "part": "pull" } }));
        let m = model(&s);
        assert!(m.furnishings.is_empty(), "a swap places nothing");
        let bar = m.cabinets[0].pull_symbol.clone().unwrap_or_default();
        assert_eq!(m.cabinets[0].pull.as_deref(), Some("own:pull-bar-128"));
        assert!(m.symbols.contains_key(&bar), "the pull's symbol is in the model");
        // Swapping again replaces it; the bar's symbol, now unused, goes.
        run(&mut s, PLACE, json!({ "item": "own:pull-knob-32", "at": [0.9, 0.62] }));
        let m = model(&s);
        let knob = m.cabinets[0].pull_symbol.clone().unwrap_or_default();
        assert_eq!(m.cabinets[0].pull.as_deref(), Some("own:pull-knob-32"));
        assert_eq!(m.symbols.keys().cloned().collect::<Vec<_>>(), vec![knob]);
        // One undo step brings the bar back.
        assert!(s.undo().is_ok());
        assert_eq!(model(&s).cabinets[0].pull.as_deref(), Some("own:pull-bar-128"));
        assert!(model(&s).symbols.contains_key(&bar));
        // A pull over no cabinet.
        assert!(refused(&mut s, PLACE, json!({ "item": "own:pull-knob-32", "at": [2.0, 2.0] })).contains("no cabinet under"));
    }

    #[test]
    fn place_a_front_style_swaps_door_or_drawer() {
        let mut s = kitchen();
        let r = run(&mut s, PLACE, json!({ "item": "frontstyle:door-raised-panel", "at": [0.9, 0.62] }));
        assert_eq!(r, json!({ "swapped": { "cabinet": "cb0", "part": "door" } }));
        let r = run(&mut s, PLACE, json!({ "item": "frontstyle:drawer-shaker", "at": [0.9, 0.62] }));
        assert_eq!(r, json!({ "swapped": { "cabinet": "cb0", "part": "drawer" } }));
        let c = model(&s).cabinets[0].clone();
        assert_eq!((c.door_style, c.drawer_style), (FrontStyle::RaisedPanel, FrontStyle::Shaker));
        assert!(model(&s).furnishings.is_empty());
        // Undo takes back only the last swap.
        assert!(s.undo().is_ok());
        let c = model(&s).cabinets[0].clone();
        assert_eq!((c.door_style, c.drawer_style), (FrontStyle::RaisedPanel, FrontStyle::Slab));
        assert!(refused(&mut s, PLACE, json!({ "item": "frontstyle:door-shaker", "at": [2.0, 2.0] })).contains("no cabinet under"));
        // A front-style item whose id names no part and style is refused.
        s.library.add_user_item(ItemRecord {
            id: "user:odd-front".into(),
            name: "Odd".into(),
            kind: ItemKind::FrontStyle,
            licence: "CC0-1.0".into(),
            ..Default::default()
        });
        assert!(refused(&mut s, PLACE, json!({ "item": "user:odd-front", "at": [0.9, 0.62] })).contains("is not a front style"));
    }

    #[test]
    fn place_a_door_item_runs_the_door_command() {
        let mut s = room();
        // 0.2 m from w0's centre line (y = -0.1), 1.1 m along it from its start (-0.1, -0.1).
        let r = run(&mut s, PLACE, json!({ "item": "door:hinged", "at": [1.0, 0.1] }));
        assert_eq!(r["via"], "door", "{r}");
        assert_eq!(r["openings"][0]["host"], "w0", "the door command's own reply: {r}");
        let o = model(&s).openings[0].clone();
        assert_eq!((o.host, o.style.as_str()), (ArchId(0), "hinged"));
        near(o.offset, 1.1, "offset along the wall from its start");
        // The door command runs inside this one: one undo step, named for it.
        assert_eq!(s.undo().ok().flatten().as_deref(), Some("Place From Library"));
        assert!(model(&s).openings.is_empty());
        // A window item; the nearest wall wins (w1 is the east wall, x = 4.1).
        let r = run(&mut s, PLACE, json!({ "item": "window:casement", "at": [3.95, 2.0] }));
        assert_eq!((r["via"].as_str(), r["openings"][0]["host"].as_str()), (Some("window"), Some("w1")), "{r}");
        assert_eq!(model(&s).openings[0].style, "casement");
        // No wall within 0.3 m of the centre line.
        assert!(refused(&mut s, PLACE, json!({ "item": "door:hinged", "at": [2.0, 2.0] })).contains("no wall within 0.3 m"));
        // The door command's own refusal leaves no trace either (it runs, then refuses the style).
        let odd = ItemRecord {
            id: "user:odd-door".into(),
            name: "Odd".into(),
            kind: ItemKind::Door,
            style: Some("gothic".into()),
            licence: "CC0-1.0".into(),
            ..Default::default()
        };
        s.library.add_user_item(odd);
        let e = refused(&mut s, PLACE, json!({ "item": "user:odd-door", "at": [1.0, 0.1] }));
        assert!(e.contains("`door`") && e.contains("gothic"), "{e}");
    }

    #[test]
    fn place_other_built_ins_run_their_commands() {
        let mut s = room();
        let species = cadcraft_arch::plant::catalogue().first().map(|p| p.name.clone()).unwrap_or_default();
        let r = run(&mut s, PLACE, json!({ "item": format!("plant:{species}"), "at": [2.0, 2.0] }));
        assert_eq!(r["via"], "plant", "{r}");
        assert_eq!(model(&s).plants.len(), 1);
        let lt = cadcraft_arch::light::catalogue().first().map(|t| t.name).unwrap_or_default();
        let r = run(&mut s, PLACE, json!({ "item": format!("lightfixture:{lt}"), "at": [1.0, 1.0] }));
        assert_eq!((r["via"].as_str(), model(&s).lights.len()), (Some("lightfixture"), 1), "{r}");
        let r = run(&mut s, PLACE, json!({ "item": "cabinet:base", "at": [1.0, 3.0], "rotation": 90 }));
        assert_eq!((r["via"].as_str(), r["cabinets"].clone()), (Some("cabinet"), json!(["cb0"])), "{r}");
        near(model(&s).cabinets[0].rotation, 90.0, "cabinet rotation");
        let r = run(&mut s, PLACE, json!({ "item": "fixture:toilet", "at": [3.0, 3.0], "size": [0.4, 0.7, 0.8] }));
        assert_eq!(r["via"], "fixture", "{r}");
        near(model(&s).fixtures[0].width, 0.4, "fixture width from size");
        // Each is one undo step.
        let undo = s.state().map(|st| st.undo.len()).unwrap_or_default();
        run(&mut s, PLACE, json!({ "item": "fixture:sink", "at": [2.0, 3.0] }));
        assert_eq!(s.state().map(|st| st.undo.len()).unwrap_or_default(), undo + 1);
        // A user item (a group) goes to the user catalog's placer.
        s.library.add_user_item(ItemRecord {
            id: "user:corner".into(),
            name: "Corner".into(),
            kind: ItemKind::Group,
            licence: "CC0-1.0".into(),
            ..Default::default()
        });
        assert!(refused(&mut s, PLACE, json!({ "item": "user:corner", "at": [1.0, 1.0] })).contains("user items are placed by p14-cmd-user-catalog"));
    }

    #[test]
    fn place_refuses_materials_and_bad_numbers() {
        let mut s = room();
        for item in ["finish:brick", "material-oak"] {
            assert!(refused(&mut s, PLACE, json!({ "item": item, "at": [2.0, 2.0] })).contains("a material is painted: use paint.material"));
        }
        let hostile = [
            (json!({ "item": "no-such-item", "at": [1, 1] }), "no library item"),
            (json!({ "item": "armchair" }), "`at` is required"),
            (json!({ "at": [1, 1] }), "`item` is required"),
            (json!({ "item": 7, "at": [1, 1] }), "`item` must be a string"),
            (json!({ "item": "armchair", "at": [1e308, 1] }), "`at` must be"),
            (json!({ "item": "armchair", "at": "here" }), "`at` must be"),
            (json!({ "item": "armchair", "at": [1, 1, 1] }), "`at` must be"),
            (json!({ "item": "armchair", "at": [1, 1], "level": "Level 9" }), "unknown level"),
            (json!({ "item": "armchair", "at": [1, 1], "level": 3 }), "unknown level"),
            (json!({ "item": "armchair", "at": [1, 1], "level": -1 }), "unknown level"),
            (json!({ "item": "armchair", "at": [1, 1], "size": [0, 1, 1] }), "`size` must be"),
            (json!({ "item": "armchair", "at": [1, 1], "size": [1, 1] }), "`size` must be"),
            (json!({ "item": "armchair", "at": [1, 1], "size": [1e9, 1, 1] }), "`size` must be"),
            (json!({ "item": "armchair", "at": [1, 1], "size": [-1, 1, 1] }), "`size` must be"),
            (json!({ "item": "armchair", "at": [1, 1], "rotation": "north" }), "`rotation` must be a finite number"),
            (json!({ "item": "armchair", "at": [1, 1], "elevation": -2 }), "`elevation` must be between 0 and 100"),
            (json!({ "item": "armchair", "at": [1, 1], "elevation": 101 }), "`elevation` must be between 0 and 100"),
            (json!("armchair"), "must be an object"),
        ];
        for (p, want) in hostile {
            let e = refused(&mut s, PLACE, p.clone());
            assert!(e.contains(want), "{p}: {e}");
        }
        assert!(model(&s).furnishings.is_empty() && model(&s).symbols.is_empty());
    }

    #[test]
    fn edit_and_undo_a_furnishing() {
        let mut s = room();
        run(&mut s, PLACE, json!({ "item": "armchair", "at": [1.0, 1.0] }));
        run(&mut s, PLACE, json!({ "item": "towel-rack", "at": [2.0, 0.2] }));
        let r = run(&mut s, EDIT, json!({ "id": "f0", "size": [0.8, 0.7, 1.0], "elevation": 0.1, "rotation": 90, "mirrored": true }));
        assert_eq!(r, json!({ "furnishing": "f0" }));
        let f = model(&s).furnishings[0].clone();
        assert_eq!((f.size, f.elevation, f.rotation, f.mirrored, f.at), ([0.8, 0.7, 1.0], 0.1, 90.0, true, [1.0, 1.0]), "only what is given changes");
        // A hosted item keeps its host when it is not moved, and loses it when it is.
        run(&mut s, EDIT, json!({ "id": "F1", "elevation": 1.2 }));
        assert_eq!(model(&s).furnishings[1].host, FurnishingHost::Wall { wall: ArchId(0) });
        run(&mut s, EDIT, json!({ "id": "f1", "at": [2.5, 1.0] }));
        let rack = model(&s).furnishings[1].clone();
        assert_eq!((rack.host, rack.at, rack.elevation), (FurnishingHost::None, [2.5, 1.0], 1.2));
        // Each edit is one undo step.
        assert_eq!(s.undo().ok().flatten().as_deref(), Some("Edit Furnishing"));
        assert_eq!(model(&s).furnishings[1].host, FurnishingHost::Wall { wall: ArchId(0) });
        assert!(s.undo().is_ok() && s.undo().is_ok());
        let f = model(&s).furnishings[0].clone();
        assert_eq!((f.size[0], f.rotation, f.mirrored), (record(&s, "armchair").size.unwrap_or_default()[0], 0.0, false));
        // Refused, and nothing changes.
        for (p, want) in [
            (json!({ "id": "f9", "size": [1, 1, 1] }), "unknown furnishing"),
            (json!({ "id": "cb0", "rotation": 1 }), "unknown furnishing"),
            (json!({ "id": "fx0", "rotation": 1 }), "unknown furnishing"),
            (json!({ "id": "f0" }), "nothing to change"),
            (json!({ "id": "f0", "size": [0, 1, 1] }), "`size` must be"),
            (json!({ "id": "f0", "at": [1, "y"] }), "`at` must be"),
            (json!({ "id": "f0", "elevation": 500 }), "`elevation` must be"),
            (json!({ "id": "f0", "mirrored": "yes" }), "`mirrored` must be true or false"),
            (json!({ "id": "f0", "rotation": [] }), "`rotation` must be"),
        ] {
            let e = refused(&mut s, EDIT, p.clone());
            assert!(e.contains(want), "{p}: {e}");
        }
    }

    #[test]
    fn delete_drops_unused_symbols() {
        let mut s = kitchen();
        run(&mut s, PLACE, json!({ "item": "armchair", "at": [2.0, 2.0] })); // f0
        run(&mut s, PLACE, json!({ "item": "armchair", "at": [3.0, 2.0] })); // f1
        run(&mut s, PLACE, json!({ "item": "own:sink-kitchen", "at": [0.9, 0.3] })); // f2
        run(&mut s, PLACE, json!({ "item": "faucet-kitchen-chrome", "at": [0.9, 0.3] })); // f3
        run(&mut s, PLACE, json!({ "item": "own:pull-knob-32", "at": [0.9, 0.3] }));
        if let Ok(d) = s.doc_mut() {
            d.arch.paints.insert(Surface::Furnishing { furnishing: ArchId(2), part: 0 }.key(), Paint::default());
            d.arch.paints.insert(Surface::Furnishing { furnishing: ArchId(0), part: 0 }.key(), Paint::default());
        }
        let chair = model(&s).furnishings[0].symbol.clone();
        let sink = model(&s).furnishings[2].symbol.clone();
        let knob = model(&s).cabinets[0].pull_symbol.clone().unwrap_or_default();
        assert_eq!(model(&s).symbols.len(), 4);
        // The sink: its symbol goes, the faucet on it stays free standing, its paint goes.
        assert_eq!(run(&mut s, DELETE, json!({ "id": "f2" })), json!({ "deleted": "f2" }));
        let m = model(&s);
        assert!(m.furnishing(ArchId(2)).is_none() && !m.symbols.contains_key(&sink));
        assert_eq!(m.furnishing(ArchId(3)).map(|f| f.host.clone()), Some(FurnishingHost::None));
        assert_eq!(m.paints.keys().cloned().collect::<Vec<_>>(), vec!["furnishing/f0/0".to_string()]);
        assert!(m.symbols.contains_key(&knob), "a pull's symbol stays while the cabinet uses it");
        // One armchair of two: the shared symbol stays until the last goes.
        run(&mut s, DELETE, json!({ "id": "f1" }));
        assert!(model(&s).symbols.contains_key(&chair));
        run(&mut s, DELETE, json!({ "id": "f0" }));
        assert!(!model(&s).symbols.contains_key(&chair));
        // Undo brings it back with its symbol.
        assert_eq!(s.undo().ok().flatten().as_deref(), Some("Delete Furnishing"));
        assert!(model(&s).symbols.contains_key(&chair) && model(&s).furnishing(ArchId(0)).is_some());
        for (p, want) in
            [(json!({ "id": "cb0" }), "unknown furnishing"), (json!({ "id": "f2" }), "unknown furnishing"), (json!({}), "`id` is required")]
        {
            let e = refused(&mut s, DELETE, p.clone());
            assert!(e.contains(want), "{p}: {e}");
        }
    }

    #[test]
    fn centre_in_a_room() {
        let mut s = room();
        run(&mut s, PLACE, json!({ "item": "armchair", "at": [1.0, 1.0] })); // f0
        let r = run(&mut s, CENTRE, json!({ "id": "f0", "on": "room" }));
        assert_eq!((r["furnishing"].as_str(), r["on"].as_str()), (Some("f0"), Some("room")), "{r}");
        let f = model(&s).furnishings[0].clone();
        near(f.at[0], 2.0, "centred x");
        near(f.at[1], 2.0, "centred y");
        assert_eq!(r["at"], json!(f.at));
        assert!(s.undo().is_ok());
        assert_eq!(model(&s).furnishings[0].at, [1.0, 1.0], "one undo step");
        // On its host wall: slid to the face's midpoint, keeping its distance and its host.
        run(&mut s, PLACE, json!({ "item": "towel-rack", "at": [0.7, 0.2] })); // f1
        let y = model(&s).furnishings[1].at[1];
        let r = run(&mut s, CENTRE, json!({ "id": "f1", "on": "wall" }));
        let f = model(&s).furnishings[1].clone();
        near(f.at[0], 2.0, "the middle of w0's inner face");
        near(f.at[1], y, "the same distance from the face");
        assert_eq!(f.host, FurnishingHost::Wall { wall: ArchId(0) }, "{r}");
        // A free item whose footprint is within 0.3 m of a face: the bookcase against the east wall.
        run(&mut s, PLACE, json!({ "item": "bookcase", "at": [3.7, 1.0], "rotation": 90 })); // f2
        run(&mut s, CENTRE, json!({ "id": "f2", "on": "wall" }));
        let f = model(&s).furnishings[2].clone();
        near(f.at[0], 3.7, "the bookcase keeps its distance from w1's inner face");
        near(f.at[1], 2.0, "slid to the middle of w1's inner face");
        // No wall near, no room around: refused.
        assert!(refused(&mut s, CENTRE, json!({ "id": "f0", "on": "wall" })).contains("no wall face within 0.3 m"));
        run(&mut s, EDIT, json!({ "id": "f0", "at": [9.0, 9.0] }));
        assert!(refused(&mut s, CENTRE, json!({ "id": "f0", "on": "room" })).contains("not in a room"));
        assert!(refused(&mut s, CENTRE, json!({ "id": "f0", "on": "ceiling" })).contains("`on` must be"));
        assert!(refused(&mut s, CENTRE, json!({ "id": "cb0", "on": "room" })).contains("unknown furnishing"));
    }

    #[test]
    fn place_interactively_asks_for_the_item_then_the_point() {
        let mut s = session();
        assert!(s.start(PLACE).is_ok());
        assert_eq!(s.current_prompt().map(|p| p.accept), Some(Accept::TEXT));
        assert!(s.input(Input::Text("no-such-item".into())).is_ok());
        assert_eq!(s.current_prompt().map(|p| p.accept), Some(Accept::TEXT), "an unknown item asks again");
        assert!(s.input(Input::Text("armchair".into())).is_ok());
        let p = s.current_prompt().unwrap_or_default();
        assert_eq!((p.accept, p.base), (Accept::POINT, None), "a point, with no rubber band");
        assert!(s.input(Input::Point(Vec2::new(2.0, 1.0))).is_ok());
        assert!(s.current_prompt().is_none());
        assert_eq!(model(&s).furnishings.first().map(|f| f.at), Some([2.0, 1.0]));
    }

    #[test]
    fn backdrop_items_name_the_backdrop_kind() {
        let s = session();
        let p = json!({ "colour": "#102030", "top": "#ffffff", "bottom": "#000000", "projection": "equirect" });
        let m = p.as_object();
        assert_eq!(backdrop_params(&record(&s, "backdrop:colour"), m), json!({ "kind": "colour", "colour": "#102030" }));
        assert_eq!(backdrop_params(&record(&s, "backdrop:gradient"), m), json!({ "kind": "gradient", "top": "#ffffff", "bottom": "#000000" }));
        let sky = backdrop_params(&record(&s, "own:backdrop-sky"), m);
        assert_eq!(sky, json!({ "kind": "image", "item": "own:backdrop-sky", "projection": "equirect" }));
        assert_eq!(backdrop_params(&record(&s, "own:backdrop-sky"), None), json!({ "kind": "image", "item": "own:backdrop-sky" }));
    }

    #[test]
    fn the_stubs_keep_their_final_signatures() {
        let s = session();
        let p = json!({ "item": "armchair", "at": [1, 2] });
        assert_eq!(super::super::defaults::merged(&s, "furnishing.armchair", &p), p);
        let f: fn(&mut Session, &ItemRecord, Vec2, f64) -> Result<Value> = crate::library::user::place_group;
        let mut s = session();
        let e = f(&mut s, &ItemRecord::default(), Vec2::ZERO, 0.0).map(|_| ()).map_err(|e| e.to_string());
        assert_eq!(e, Err("user items are placed by p14-cmd-user-catalog".to_string()));
    }
}
