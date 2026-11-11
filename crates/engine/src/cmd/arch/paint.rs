//! The painters (phase 14): `paint.material` (Material Painter), `paint.object` (Object Painter)
//! and `paint.pick` (Material Eyedropper).
//!
//! Surfaces are addressed by their key (`wall/w3/left`, `cabinet/cb0/pull`, ...), materials by a
//! finish name, `#rrggbb` or a library item id. Everything is validated before the drawing is
//! written, so a refused call changes nothing; a successful one is one undo step. Hostile
//! parameters are refused, never repaired.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use cadcraft_arch::library::{ItemKind, ItemRecord};
use cadcraft_arch::paint::{Scope, effective_material, finish_fits, select, select_objects, surfaces};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchId, CabinetPart, Finish, FrontStyle, ImageRgba, MaterialRef, Paint, Surface, SymbolMesh, furnishing_label};
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::prompt::{Accept, Input, Interactive, Prompt, Step};
use crate::{Result, Session};

/// The scopes, as the parameters spell them.
const SCOPES: [&str; 5] = ["component", "object", "room", "floor", "plan"];

/// Command specs for the painters.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("paint.material", "Material Painter", run_material)
            .menu(&["Architecture", "Material Painter"])
            .params("{material: \"siding\" | \"#c04020\" | \"material-oak\", target: \"wall/w3/left\", scope: \"component\"|\"object\"|\"room\"|\"floor\"|\"plan\", mode?: \"paint\"|\"blend\"}")
            .interactive(|_| Ok(Box::new(PaintM::new(Kind::Material)))),
        CommandSpec::new("paint.object", "Object Painter", run_object)
            .menu(&["Architecture", "Object Painter"])
            .params("{item: \"own:pull-knob-32\", target: \"cabinet/cb0/pull\", scope: \"component\"|\"object\"|\"room\"|\"floor\"|\"plan\"}")
            .interactive(|_| Ok(Box::new(PaintM::new(Kind::Object)))),
        CommandSpec::new("paint.pick", "Material Eyedropper", run_pick)
            .menu(&["Architecture", "Material Eyedropper"])
            .noundo()
            .params("{target: \"wall/w3/left\"}")
            .interactive(|_| Ok(Box::new(PaintM::new(Kind::Pick)))),
    ]
}

// ---- parameters ----

fn object<'a>(cmd: &str, p: &'a Value) -> Result<&'a Map<String, Value>> {
    match p {
        Value::Object(m) => Ok(m),
        _ => Err(bad(cmd, "the parameters must be an object")),
    }
}

/// A required non-empty string.
fn text<'a>(cmd: &str, m: &'a Map<String, Value>, key: &str) -> Result<&'a str> {
    match m.get(key).filter(|v| !v.is_null()) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim()),
        Some(Value::String(_)) => Err(bad(cmd, format!("`{key}` is empty"))),
        Some(_) => Err(bad(cmd, format!("`{key}` must be a string"))),
        None => Err(bad(cmd, format!("`{key}` is required"))),
    }
}

/// An optional string; a present value of another type is refused.
fn opt_text<'a>(cmd: &str, m: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>> {
    match m.get(key).filter(|v| !v.is_null()) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.trim())),
        Some(_) => Err(bad(cmd, format!("`{key}` must be a string"))),
    }
}

/// At most 128 characters of hostile text, for an error message.
fn clip(s: &str) -> String {
    s.chars().take(128).collect()
}

fn parse_target(cmd: &str, m: &Map<String, Value>) -> Result<Surface> {
    let t = text(cmd, m, "target")?;
    Surface::parse(t).ok_or_else(|| bad(cmd, format!("`{}` is not a surface key (wall/w3/left, room/r0/floor, cabinet/cb0/pull, ...)", clip(t))))
}

/// The required scope.
fn parse_scope(cmd: &str, m: &Map<String, Value>) -> Result<Scope> {
    let s = text(cmd, m, "scope")?;
    Scope::parse(s).ok_or_else(|| bad(cmd, format!("unknown scope `{}` ({})", clip(s), SCOPES.join(", "))))
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

// ---- paint.material ----

/// A material resolved against the library: what to store, its mean colour and its image.
struct Resolved {
    material: MaterialRef,
    mean: [u8; 3],
    image: Option<(String, ImageRgba)>,
    normal: Option<(String, ImageRgba)>,
}

fn finish_mean(f: Finish) -> [u8; 3] {
    cadcraft_arch3d::texture::mean_srgb(&cadcraft_arch3d::texture::texture(f))
}

fn resolve_material(s: &mut Session, raw: &str) -> Result<Resolved> {
    const CMD: &str = "paint.material";
    let m = MaterialRef::parse(raw).ok_or_else(|| bad(CMD, format!("`{}` is not a material", clip(raw))))?;
    match m {
        MaterialRef::Finish(f) => Ok(Resolved { material: m, mean: finish_mean(f), image: None, normal: None }),
        MaterialRef::Colour(c) => Ok(Resolved { material: m, mean: c, image: None, normal: None }),
        MaterialRef::Item(id) => {
            s.library.sync_io();
            let rec = s.library.item(&id).cloned().ok_or_else(|| bad(CMD, format!("no library item `{}`", clip(&id))))?;
            match rec.kind {
                ItemKind::Finish => {
                    let f = id
                        .strip_prefix("finish:")
                        .and_then(Finish::parse)
                        .ok_or_else(|| bad(CMD, format!("`{}` is not a finish this build knows", clip(&id))))?;
                    Ok(Resolved { material: MaterialRef::Finish(f), mean: finish_mean(f), image: None, normal: None })
                }
                ItemKind::Material => {
                    let img = s.library.image(&id).map_err(|e| bad(CMD, e))?;
                    let normal = match s.library.normal_map_bytes(&id).map_err(|e| bad(CMD, e))? {
                        Some(bytes) => {
                            let io = crate::library::library_io().ok_or_else(|| bad(CMD, crate::library::NO_READERS))?;
                            let img = (io.read_image)(&bytes, 1024).map_err(|e| bad(CMD, e))?;
                            if !img.is_valid() {
                                return Err(bad(CMD, format!("`{}`'s normal map is not valid", clip(&id))));
                            }
                            Some((format!("{id}:normal"), img))
                        }
                        None => None,
                    };
                    Ok(Resolved { material: MaterialRef::Item(id.clone()), mean: img.mean_srgb(), image: Some((id, img)), normal })
                }
                _ => Err(bad(CMD, format!("`{}` is not a material", clip(&id)))),
            }
        }
    }
}

fn run_material(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "paint.material";
    let m = object(CMD, p)?;
    let raw = text(CMD, m, "material")?;
    let target = parse_target(CMD, m)?;
    let scope = parse_scope(CMD, m)?;
    let blend = match opt_text(CMD, m, "mode")? {
        None | Some("paint") => false,
        Some("blend") => true,
        Some(o) => return Err(bad(CMD, format!("unknown mode `{}` (paint, blend)", clip(o)))),
    };
    let chosen = select(&s.doc()?.arch, &target, scope).map_err(|e| bad(CMD, e))?;
    let res = resolve_material(s, raw)?;
    if let MaterialRef::Finish(f) = &res.material
        && let Some(sf) = chosen.iter().find(|sf| !finish_fits(sf, *f))
    {
        return Err(bad(CMD, format!("{} does not fit {}", f.name(), sf.key())));
    }
    let d = s.doc_mut()?;
    let mut uses_image = false;
    for sf in &chosen {
        let key = sf.key();
        let old = d.arch.paints.remove(&key);
        let paint = if blend {
            // The surface keeps its material (painted or its own) and takes the new one's colour.
            Paint { tint: Some(res.mean), ..old.unwrap_or_default() }
        } else {
            Paint { material: Some(res.material.clone()), ..Default::default() }
        };
        // Painting a surface back to its unpainted material leaves no entry.
        let plain = paint.tint.is_none() && paint.material.as_ref().is_none_or(|m| m.key() == effective_material(&d.arch, sf));
        if !plain {
            uses_image |= !blend;
            d.arch.paints.insert(key, paint);
        }
    }
    if uses_image {
        if let Some((id, img)) = res.image {
            d.arch.images.insert(id, img);
        }
        if let Some((id, img)) = res.normal {
            d.arch.images.insert(id, img);
        }
    }
    d.touch_arch();
    let mut painted: Vec<String> = chosen.iter().map(Surface::key).collect();
    painted.sort();
    painted.dedup();
    Ok(json!({ "painted": painted }))
}

// ---- paint.object ----

/// What an item replaces.
enum Replace {
    Pull { id: String, mesh: SymbolMesh },
    Front { door: bool, style: FrontStyle },
    Furnishing { rec: Box<ItemRecord>, size: [f64; 3], mesh: SymbolMesh },
}

fn front_style(rec: &ItemRecord) -> Option<(bool, FrontStyle)> {
    let rest = rec.id.strip_prefix("frontstyle:")?;
    let (kind, name) = rest.split_once('-')?;
    let door = match kind {
        "door" => true,
        "drawer" => false,
        _ => return None,
    };
    let style = rec.style.as_deref().and_then(FrontStyle::parse).or_else(|| FrontStyle::parse(name))?;
    Some((door, style))
}

fn is_pull(sf: &Surface) -> bool {
    matches!(sf, Surface::Cabinet { part: CabinetPart::Pull, .. })
}

fn is_front(sf: &Surface) -> bool {
    matches!(sf, Surface::Cabinet { part: CabinetPart::Front, .. })
}

fn is_furnishing(sf: &Surface) -> bool {
    matches!(sf, Surface::Furnishing { .. })
}

/// What item `id` does to the chosen surfaces; refused when it does not fit every one of them.
fn plan_replace(s: &mut Session, id: &str, target: &Surface, chosen: &[Surface]) -> Result<Replace> {
    const CMD: &str = "paint.object";
    s.library.sync_io();
    let rec = s.library.item(id).cloned().ok_or_else(|| bad(CMD, format!("no library item `{}`", clip(id))))?;
    let fits = |want: fn(&Surface) -> bool, what: &str| {
        if chosen.iter().all(want) { Ok(()) } else { Err(bad(CMD, format!("`{}` is {what}, which does not fit {}", clip(id), target.key()))) }
    };
    match rec.kind {
        ItemKind::Symbol if rec.symbol_type == "pull" => {
            fits(is_pull, "a pull")?;
            let mesh = s.library.symbol(id).map_err(|e| bad(CMD, e))?;
            Ok(Replace::Pull { id: rec.id, mesh })
        }
        ItemKind::Symbol => {
            let what = if rec.symbol_type.is_empty() { "a furnishing".to_string() } else { format!("a {}", rec.symbol_type) };
            fits(is_furnishing, &what)?;
            let mesh = s.library.symbol(id).map_err(|e| bad(CMD, e))?;
            let size = rec.size.unwrap_or_else(|| mesh.extents());
            if !size.iter().all(|v| v.is_finite() && *v > 0.0) {
                return Err(bad(CMD, format!("`{}` has no usable size", clip(id))));
            }
            Ok(Replace::Furnishing { rec: Box::new(rec), size, mesh })
        }
        ItemKind::FrontStyle => {
            let (door, style) = front_style(&rec).ok_or_else(|| bad(CMD, format!("`{}` is not a door or drawer front style", clip(id))))?;
            fits(is_front, "a front style")?;
            Ok(Replace::Front { door, style })
        }
        _ => Err(bad(CMD, format!("`{}` cannot be painted onto a surface", clip(id)))),
    }
}

/// Apply a checked replacement to the objects behind `chosen`.
fn apply_replace(a: &mut ArchModel, plan: Replace, chosen: &[Surface]) {
    let cabinets: Vec<ArchId> = chosen
        .iter()
        .filter_map(|sf| match sf {
            Surface::Cabinet { cabinet, .. } => Some(*cabinet),
            _ => None,
        })
        .collect();
    let furnishings: Vec<ArchId> = chosen
        .iter()
        .filter_map(|sf| match sf {
            Surface::Furnishing { furnishing, .. } => Some(*furnishing),
            _ => None,
        })
        .collect();
    // The symbols the replaced objects used before.
    let mut old_symbols: Vec<String> = Vec::new();
    match plan {
        Replace::Pull { id, mesh } => {
            for c in a.cabinets.iter_mut().filter(|c| cabinets.contains(&c.id)) {
                old_symbols.extend(c.pull_symbol.replace(mesh.key.clone()));
                c.pull = Some(id.clone());
            }
            a.symbols.insert(mesh.key.clone(), mesh);
        }
        Replace::Front { door, style } => {
            for c in a.cabinets.iter_mut().filter(|c| cabinets.contains(&c.id)) {
                if door {
                    c.door_style = style;
                } else {
                    c.drawer_style = style;
                }
            }
        }
        Replace::Furnishing { rec, size, mesh } => {
            let mut new_symbol = Vec::new();
            for f in a.furnishings.iter_mut().filter(|f| furnishings.contains(&f.id)) {
                if f.symbol != mesh.key {
                    new_symbol.push(f.id);
                }
                old_symbols.push(std::mem::replace(&mut f.symbol, mesh.key.clone()));
                f.item = rec.id.clone();
                f.name = rec.name.clone();
                f.category = rec.category.clone();
                f.symbol_type = rec.symbol_type.clone();
                f.size = size;
                f.cutout = rec.cutout;
            }
            // Part paints of the old symbol do not address parts of the new one.
            for fid in new_symbol {
                let prefix = format!("furnishing/{}/", furnishing_label(fid));
                a.paints.retain(|k, _| !k.starts_with(&prefix));
            }
            a.symbols.insert(mesh.key.clone(), mesh);
        }
    }
    // An old symbol nothing uses any more leaves the drawing.
    for key in old_symbols {
        let used = a.furnishings.iter().any(|f| f.symbol == key) || a.cabinets.iter().any(|c| c.pull_symbol.as_deref() == Some(key.as_str()));
        if !used {
            a.symbols.remove(&key);
        }
    }
}

fn run_object(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "paint.object";
    let m = object(CMD, p)?;
    let id = text(CMD, m, "item")?;
    let target = parse_target(CMD, m)?;
    let scope = parse_scope(CMD, m)?;
    let chosen = select_objects(&s.doc()?.arch, &target, scope).map_err(|e| bad(CMD, e))?;
    let plan = plan_replace(s, id, &target, &chosen)?;
    let d = s.doc_mut()?;
    apply_replace(&mut d.arch, plan, &chosen);
    d.touch_arch();
    let mut replaced: Vec<String> = chosen.iter().map(Surface::key).collect();
    replaced.sort();
    replaced.dedup();
    Ok(json!({ "replaced": replaced }))
}

// ---- paint.pick ----

/// The library item of the object a surface belongs to: a cabinet's pull, its door style, a
/// furnishing's item.
fn item_at(model: &ArchModel, sf: &Surface) -> Option<String> {
    let cabinet = |id: &ArchId| model.cabinets.iter().find(|c| c.id == *id);
    match sf {
        Surface::Cabinet { cabinet: id, part: CabinetPart::Pull } => cabinet(id).and_then(|c| c.pull.clone()),
        Surface::Cabinet { cabinet: id, part: CabinetPart::Front } => cabinet(id).map(|c| format!("frontstyle:door-{}", c.door_style.name())),
        Surface::Furnishing { furnishing, .. } => model.furnishing(*furnishing).map(|f| f.item.clone()),
        _ => None,
    }
}

fn run_pick(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "paint.pick";
    let m = object(CMD, p)?;
    let target = parse_target(CMD, m)?;
    let uid = s.state()?.uid;
    let model = &s.doc()?.arch;
    if !surfaces(model).iter().any(|i| i.surface == target) {
        return Err(bad(CMD, format!("no such surface: {}", target.key())));
    }
    let material = effective_material(model, &target);
    let tint = model.paints.get(&target.key()).and_then(|p| p.tint).map(hex);
    remember(uid, Picked { material: material.clone(), item: item_at(model, &target) });
    Ok(json!({ "material": material, "tint": tint }))
}

// ---- interactive forms ----

/// What the eyedropper picked up last: the material key and the library item of the object it
/// was on (a pull, a front, a furnishing).
#[derive(Debug)]
struct Picked {
    material: String,
    item: Option<String>,
}

/// The eyedropper's last pick in each drawing (by its uid), until a painter started from the menu
/// in that drawing takes it.
static PICKS: Mutex<BTreeMap<u64, Picked>> = Mutex::new(BTreeMap::new());

/// Picks kept at most: past it the oldest drawing's goes first.
const MAX_PICKS: usize = 16;

fn remember(uid: u64, p: Picked) {
    let mut picks = PICKS.lock().unwrap_or_else(PoisonError::into_inner);
    picks.insert(uid, p);
    while picks.len() > MAX_PICKS {
        picks.pop_first();
    }
}

fn take_pick(uid: u64) -> Option<Picked> {
    PICKS.lock().unwrap_or_else(PoisonError::into_inner).remove(&uid)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Material,
    Object,
    Pick,
}

/// The painters ask for the material or item (unless the eyedropper's last pick supplies one),
/// then for surfaces to paint (a 3D pick arrives as the surface key typed in) until Enter; the
/// eyedropper asks for one surface. Each runs the JSON command's own code, and the whole
/// interactive run is one undo step.
struct PaintM {
    kind: Kind,
    value: Option<String>,
    scope: &'static str,
}

impl PaintM {
    fn new(kind: Kind) -> PaintM {
        PaintM { kind, value: None, scope: "component" }
    }

    /// The pick's material or item when this painter can use it.
    fn usable_pick(&self, s: &mut Session, p: Picked) -> Option<String> {
        s.library.sync_io();
        let kind_of = |id: &str| s.library.item(id).map(|r| r.kind);
        match self.kind {
            Kind::Material => match MaterialRef::parse(&p.material)? {
                MaterialRef::Item(id) => matches!(kind_of(&id), Some(ItemKind::Material | ItemKind::Finish)).then_some(p.material),
                _ => Some(p.material),
            },
            Kind::Object => p.item.filter(|id| matches!(kind_of(id), Some(ItemKind::Symbol | ItemKind::FrontStyle))),
            Kind::Pick => None,
        }
    }
}

impl Interactive for PaintM {
    fn name(&self) -> &'static str {
        match self.kind {
            Kind::Material => "paint.material",
            Kind::Object => "paint.object",
            Kind::Pick => "paint.pick",
        }
    }

    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        if self.kind != Kind::Pick
            && let Ok(uid) = s.state().map(|st| st.uid)
            && let Some(p) = take_pick(uid)
        {
            self.value = self.usable_pick(s, p);
        }
        Ok(Step::Continue)
    }

    fn prompt(&self, _s: &Session) -> Prompt {
        match (&self.value, self.kind) {
            (_, Kind::Pick) => Prompt::new("Select a surface to sample:", Accept::TEXT),
            (None, Kind::Material) => Prompt::new("Enter a material (finish name, #rrggbb or library item):", Accept::TEXT),
            (None, _) => Prompt::new("Enter a library item:", Accept::TEXT),
            (Some(v), _) => {
                Prompt::new(format!("Select a surface to paint with {v} or [Component/Object/Room/Floor/Plan] <{}>:", self.scope), Accept::TEXT)
                    .kw(&["Component", "Object", "Room", "Floor", "Plan"])
            }
        }
    }

    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let t = match i {
            Input::Cancel => return Ok(Step::Cancel),
            Input::Enter => return Ok(Step::Done),
            Input::Text(t) | Input::Keyword(t) => t.trim().to_string(),
            _ => return Ok(Step::Continue),
        };
        if t.is_empty() {
            return Ok(Step::Done);
        }
        if self.kind == Kind::Pick {
            run_pick(s, &json!({ "target": t }))?;
            return Ok(Step::Done);
        }
        let Some(value) = self.value.clone() else {
            self.value = Some(t);
            return Ok(Step::Continue);
        };
        let lower = t.to_ascii_lowercase();
        if let Some(sc) = SCOPES.iter().find(|sc| **sc == lower) {
            self.scope = sc;
            return Ok(Step::Continue);
        }
        // The command's own code: the run's changes become one undo step when it ends.
        match self.kind {
            Kind::Material => run_material(s, &json!({ "material": value, "target": t, "scope": self.scope }))?,
            _ => run_object(s, &json!({ "item": value, "target": t, "scope": self.scope }))?,
        };
        Ok(Step::Continue)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;
    use crate::library::testing::session;
    use crate::temp_dir::TempDir;
    use cadcraft_doc::arch::{Cabinet, Furnishing};
    use cadcraft_doc::{Room, Wall};
    use cadcraft_geom::Vec2;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    /// Two rooms side by side (r0 at x 0..4, r1 at x 10..14), each with a south wall whose left
    /// side faces it (w0, w1), and cabinets with the same pull: cb0 and cb1 in r0, cb2 in r1.
    fn setup(dir: &std::path::Path) -> Session {
        let mut s = session(dir);
        let d = s.doc_mut().unwrap();
        for x in [0.0, 10.0] {
            d.arch.push_wall(Wall::new(Vec2::new(x, 0.0), Vec2::new(x + 4.0, 0.0), 0.2));
            d.arch.push_room(Room { polygon: rect(x, 0.0, x + 4.0, 4.0), ..Default::default() });
        }
        for x in [1.0, 2.0, 11.0] {
            d.arch.push_cabinet(Cabinet {
                width: 0.6,
                depth: 0.6,
                position: Vec2::new(x, 1.0),
                pull: Some("own:pull-bar-128".into()),
                ..Default::default()
            });
        }
        d.touch_arch();
        s
    }

    fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
        s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
    }

    fn paint(s: &mut Session, material: &str, target: &str, scope: &str) -> Value {
        run(s, "paint.material", json!({"material": material, "target": target, "scope": scope}))
    }

    fn material(s: &Session, key: &str) -> Option<MaterialRef> {
        s.doc().unwrap().arch.paints.get(key).and_then(|p| p.material.clone())
    }

    fn undo_depth(s: &Session) -> usize {
        s.state().unwrap().undo.len()
    }

    #[test]
    fn paint_room_scope_takes_like_materials_in_the_room() {
        let dir = TempDir::new("paint-room");
        let mut s = setup(&dir);
        // Room 0's ceiling and both rooms' inner wall sides share one colour.
        for t in ["room/r0/ceiling", "wall/w0/left", "wall/w1/left"] {
            paint(&mut s, "#102030", t, "component");
        }
        let r = paint(&mut s, "#c04020", "wall/w0/left", "room");
        // The like surfaces in room 0 only: not its floor (another material), not room 1's wall.
        assert_eq!(r["painted"], json!(["room/r0/ceiling", "wall/w0/left"]));
        assert_eq!(material(&s, "wall/w0/left"), Some(MaterialRef::Colour([0xc0, 0x40, 0x20])));
        assert_eq!(material(&s, "wall/w1/left"), Some(MaterialRef::Colour([0x10, 0x20, 0x30])));
        assert_eq!(material(&s, "room/r0/floor"), None);
        // Both carcasses in room 0, not the one in room 1.
        let r = paint(&mut s, "#ffffff", "cabinet/cb0/carcass", "room");
        assert_eq!(r["painted"], json!(["cabinet/cb0/carcass", "cabinet/cb1/carcass"]));
        // An exterior side has no room: just itself.
        let r = paint(&mut s, "#ffffff", "wall/w0/right", "room");
        assert_eq!(r["painted"], json!(["wall/w0/right"]));
    }

    #[test]
    fn paint_plan_scope_takes_every_like_material() {
        let dir = TempDir::new("paint-plan");
        let mut s = setup(&dir);
        let r = paint(&mut s, "brick", "wall/w0/right", "plan");
        // Every wall side shares the default wall material, so all four are like surfaces.
        assert_eq!(r["painted"], json!(["wall/w0/left", "wall/w0/right", "wall/w1/left", "wall/w1/right"]));
        assert_eq!(material(&s, "wall/w1/left"), Some(MaterialRef::Finish(Finish::Brick)));
        let r = paint(&mut s, "#ffffff", "room/r0/ceiling", "plan");
        assert_eq!(r["painted"], json!(["room/r0/ceiling", "room/r1/ceiling"]));
        // Object scope: every surface of the clicked cabinet.
        let r = paint(&mut s, "#808080", "cabinet/cb2/front", "object");
        assert_eq!(r["painted"], json!(["cabinet/cb2/carcass", "cabinet/cb2/front", "cabinet/cb2/pull"]));
    }

    #[test]
    fn paint_blend_keeps_the_material_and_tints() {
        let dir = TempDir::new("paint-blend");
        let mut s = setup(&dir);
        paint(&mut s, "brick", "wall/w0/right", "component");
        run(&mut s, "paint.material", json!({"material": "#c04020", "target": "wall/w0/right", "scope": "component", "mode": "blend"}));
        let p = s.doc().unwrap().arch.paints.get("wall/w0/right").cloned().unwrap();
        assert_eq!(p.material, Some(MaterialRef::Finish(Finish::Brick)));
        assert_eq!(p.tint, Some([0xc0, 0x40, 0x20]));
        // On an unpainted surface the material stays unpainted and the tint is the finish's mean.
        run(&mut s, "paint.material", json!({"material": "wood", "target": "room/r1/floor", "scope": "component", "mode": "blend"}));
        let p = s.doc().unwrap().arch.paints.get("room/r1/floor").cloned().unwrap();
        assert_eq!(p.material, None);
        assert_eq!(p.tint, Some(finish_mean(Finish::Wood)));
        // An image material keeps the image and tints it; blending with an image tints by its mean
        // and stores no image the surface does not use.
        paint(&mut s, "material-oak", "room/r0/floor", "component");
        run(&mut s, "paint.material", json!({"material": "#3060a0", "target": "room/r0/floor", "scope": "component", "mode": "blend"}));
        let p = s.doc().unwrap().arch.paints.get("room/r0/floor").cloned().unwrap();
        assert_eq!((p.material, p.tint), (Some(MaterialRef::Item("material-oak".into())), Some([0x30, 0x60, 0xa0])));
        run(&mut s, "paint.material", json!({"material": "material-marble", "target": "room/r0/ceiling", "scope": "component", "mode": "blend"}));
        let marble = s.library.image("material-marble").unwrap().mean_srgb();
        let m = &s.doc().unwrap().arch;
        assert_eq!(m.paints.get("room/r0/ceiling").and_then(|p| p.tint), Some(marble));
        assert!(!m.images.contains_key("material-marble"));
        assert!(m.images.contains_key("material-oak"));
    }

    #[test]
    fn paint_refuses_unfit_finishes_and_unknown_targets() {
        let dir = TempDir::new("paint-refuse");
        let mut s = setup(&dir);
        // The left side of w0 and room 0's floor share one colour, so siding at plan scope takes
        // the floor too, which siding does not fit.
        paint(&mut s, "#123456", "wall/w0/left", "component");
        paint(&mut s, "#123456", "room/r0/floor", "component");
        let before = s.doc().unwrap().arch.clone();
        let depth = undo_depth(&s);
        let e = s.execute("paint.material", &json!({"material": "siding", "target": "wall/w0/left", "scope": "plan"})).unwrap_err().to_string();
        assert!(e.contains("room/r0/floor"), "{e}");
        for p in [
            json!({"material": "shingles", "target": "wall/w0/left", "scope": "component"}),
            json!({"material": "shingles", "target": "room/r0/floor", "scope": "component"}),
            json!({"material": "brick", "target": "roof/r0", "scope": "component"}),
            json!({"material": "#c04020", "target": "wall/w9/left", "scope": "component"}),
            json!({"material": "#c04020", "target": "wall/w0/up", "scope": "component"}),
            json!({"material": "#c04020", "target": "nonsense", "scope": "component"}),
            json!({"material": "#c04020", "target": 7, "scope": "component"}),
            json!({"material": "#c04020", "target": "wall/w0/left", "scope": "galaxy"}),
            json!({"material": "#c04020", "target": "wall/w0/left", "scope": 2}),
            json!({"material": "#c04020", "target": "wall/w0/left"}),
            json!({"material": "#c04020", "target": "wall/w0/left", "scope": "component", "mode": "smear"}),
            json!({"material": "#c04020", "target": "wall/w0/left", "scope": "component", "mode": true}),
            json!({"material": "#12345", "target": "wall/w0/left", "scope": "component"}),
            json!({"material": "#c0402g", "target": "wall/w0/left", "scope": "component"}),
            json!({"material": "unobtainium", "target": "wall/w0/left", "scope": "component"}),
            json!({"material": "default:floor", "target": "room/r0/floor", "scope": "component"}),
            json!({"material": "armchair", "target": "wall/w0/left", "scope": "component"}),
            json!({"material": "own:pull-knob-32", "target": "wall/w0/left", "scope": "component"}),
            json!({"material": "", "target": "wall/w0/left", "scope": "component"}),
            json!({"material": 5, "target": "wall/w0/left", "scope": "component"}),
            json!({"target": "wall/w0/left", "scope": "component"}),
            json!(["wall/w0/left"]),
        ] {
            assert!(s.execute("paint.material", &p).is_err(), "{p}");
        }
        for p in [json!({"target": "wall/w9/left"}), json!({"target": "roof/r7"}), json!({"target": 3}), json!({})] {
            assert!(s.execute("paint.pick", &p).is_err(), "{p}");
        }
        assert_eq!(s.doc().unwrap().arch, before);
        assert_eq!(undo_depth(&s), depth);
    }

    #[test]
    fn paint_stores_the_item_image() {
        let dir = TempDir::new("paint-image");
        let mut s = setup(&dir);
        paint(&mut s, "material-oak", "room/r0/floor", "component");
        assert_eq!(material(&s, "room/r0/floor"), Some(MaterialRef::Item("material-oak".into())));
        let want = s.library.image("material-oak").unwrap();
        assert!(want.is_valid());
        assert_eq!(s.doc().unwrap().arch.images.get("material-oak"), Some(&want));
        // The eyedropper's key form names the same item.
        paint(&mut s, "item:material-oak", "room/r1/floor", "component");
        assert_eq!(material(&s, "room/r1/floor"), Some(MaterialRef::Item("material-oak".into())));
        assert_eq!(s.doc().unwrap().arch.images.len(), 1);
        // A library finish item paints the finish and stores no image.
        paint(&mut s, "finish:wood", "room/r1/floor", "component");
        assert_eq!(material(&s, "room/r1/floor"), Some(MaterialRef::Finish(Finish::Wood)));
        assert_eq!(s.doc().unwrap().arch.images.len(), 1);
    }

    #[test]
    fn item_normal_map_loads_with_paint() {
        let dir = TempDir::new("paint-normal-item");
        let mut s = setup(&dir);
        std::fs::create_dir_all(dir.join("user")).unwrap();
        let image = cadcraft_doc::arch::ImageRgba { width: 64, height: 64, rgba: vec![255; 64 * 64 * 4], ..Default::default() };
        let mut normal = cadcraft_doc::arch::ImageRgba { width: 64, height: 64, rgba: vec![0; 64 * 64 * 4], ..Default::default() };
        for (i, px) in normal.rgba.iter_mut().enumerate() {
            if i % 4 == 0 {
                *px = 255;
            } else if i % 4 == 1 {
                *px = 128;
            } else if i % 4 == 2 {
                *px = 255;
            } else {
                *px = 255;
            }
        }
        std::fs::write(dir.join("user").join("test-mat.png"), cadcraft_io::png::encode_rgba8(image.width, image.height, &image.rgba).unwrap())
            .unwrap();
        std::fs::write(
            dir.join("user").join("test-mat-normal.png"),
            cadcraft_io::png::encode_rgba8(normal.width, normal.height, &normal.rgba).unwrap(),
        )
        .unwrap();
        s.library.add_user_item(ItemRecord {
            id: "test-mat".into(),
            name: "Test Mat".into(),
            category: "Materials".into(),
            folder: "Images".into(),
            kind: ItemKind::Material,
            file: Some("test-mat.png".into()),
            normal_map: Some("test-mat-normal.png".into()),
            licence: "CC0-1.0".into(),
            source: "local".into(),
            catalog: "user".into(),
            read_only: false,
            ..Default::default()
        });
        let before = s.doc().unwrap().arch.clone();
        let result = paint(&mut s, "test-mat", "room/r0/floor", "component");
        assert_eq!(result["painted"], json!(["room/r0/floor"]));
        assert_eq!(material(&s, "room/r0/floor"), Some(MaterialRef::Item("test-mat".into())));
        assert_eq!(s.doc().unwrap().arch.images.get("test-mat").map(|img| img.is_valid()), Some(true));
        assert_eq!(s.doc().unwrap().arch.images.get("test-mat:normal").map(|img| img.is_valid()), Some(true));
        assert_ne!(s.doc().unwrap().arch, before);
    }

    #[test]
    fn paint_is_one_undo_step() {
        let dir = TempDir::new("paint-undo");
        let mut s = setup(&dir);
        let before = s.doc().unwrap().arch.clone();
        let depth = undo_depth(&s);
        let r = paint(&mut s, "#c04020", "wall/w0/right", "plan");
        assert_eq!(r["painted"].as_array().map(Vec::len), Some(4));
        assert_eq!(s.doc().unwrap().arch.paints.len(), 4);
        assert_eq!(undo_depth(&s), depth + 1);
        run(&mut s, "undo", json!({}));
        assert_eq!(s.doc().unwrap().arch, before);
        run(&mut s, "redo", json!({}));
        assert_eq!(s.doc().unwrap().arch.paints.len(), 4);
        // Painting a surface back to its unpainted material removes its entry.
        s.doc_mut().unwrap().arch.rooms[0].floor_finish = Some(Finish::Wood);
        paint(&mut s, "#c04020", "room/r0/floor", "component");
        assert!(s.doc().unwrap().arch.paints.contains_key("room/r0/floor"));
        let r = paint(&mut s, "wood", "room/r0/floor", "component");
        assert_eq!(r["painted"], json!(["room/r0/floor"]));
        assert!(!s.doc().unwrap().arch.paints.contains_key("room/r0/floor"));
        let r = run(&mut s, "paint.pick", json!({"target": "room/r0/floor"}));
        assert_eq!(r["material"], "finish:wood");
    }

    #[test]
    fn paint_object_replaces_like_pulls_in_the_room() {
        let dir = TempDir::new("paint-object");
        let mut s = setup(&dir);
        let depth = undo_depth(&s);
        let r = run(&mut s, "paint.object", json!({"item": "own:pull-knob-32", "target": "cabinet/cb0/pull", "scope": "room"}));
        assert_eq!(r["replaced"], json!(["cabinet/cb0/pull", "cabinet/cb1/pull"]));
        assert_eq!(undo_depth(&s), depth + 1);
        let knob = s.library.symbol("own:pull-knob-32").unwrap();
        let m = &s.doc().unwrap().arch;
        let pulls: Vec<_> = m.cabinets.iter().map(|c| c.pull.clone().unwrap_or_default()).collect();
        assert_eq!(pulls, ["own:pull-knob-32", "own:pull-knob-32", "own:pull-bar-128"]);
        assert_eq!(m.cabinets[0].pull_symbol.as_deref(), Some(knob.key.as_str()));
        assert_eq!(m.symbols.get(&knob.key), Some(&knob));
        // Door and drawer front styles.
        run(&mut s, "paint.object", json!({"item": "frontstyle:door-shaker", "target": "cabinet/cb2/front", "scope": "component"}));
        run(&mut s, "paint.object", json!({"item": "frontstyle:drawer-raised-panel", "target": "cabinet/cb2/front", "scope": "component"}));
        let c = &s.doc().unwrap().arch.cabinets[2];
        assert_eq!((c.door_style, c.drawer_style), (FrontStyle::Shaker, FrontStyle::RaisedPanel));
        assert_eq!(s.doc().unwrap().arch.cabinets[1].door_style, FrontStyle::Slab);
        // Plan scope takes every like pull; the old symbol nothing uses leaves the drawing.
        let r = run(&mut s, "paint.object", json!({"item": "own:pull-bar-128", "target": "cabinet/cb1/pull", "scope": "plan"}));
        assert_eq!(r["replaced"], json!(["cabinet/cb0/pull", "cabinet/cb1/pull"]));
        assert!(!s.doc().unwrap().arch.symbols.contains_key(&knob.key));
        run(&mut s, "undo", json!({}));
        assert_eq!(s.doc().unwrap().arch.cabinets[0].pull.as_deref(), Some("own:pull-knob-32"));
    }

    #[test]
    fn paint_object_replaces_like_furnishings() {
        let dir = TempDir::new("paint-object-furnishing");
        let mut s = setup(&dir);
        let chair = s.library.symbol("armchair").unwrap();
        let d = s.doc_mut().unwrap();
        for (at, rotation) in [([1.0, 3.0], 0.0), ([3.0, 3.0], 90.0), ([12.0, 3.0], 0.0)] {
            let f = Furnishing {
                item: "armchair".into(),
                symbol: chair.key.clone(),
                at,
                rotation,
                elevation: 0.1,
                size: [0.6, 0.6, 0.9],
                ..Default::default()
            };
            d.arch.push_furnishing(f);
        }
        d.arch.symbols.insert(chair.key.clone(), chair.clone());
        d.arch.paints.insert("furnishing/f1/0".into(), Paint { tint: Some([1, 2, 3]), ..Default::default() });
        d.touch_arch();
        let r = run(&mut s, "paint.object", json!({"item": "sofa-futon", "target": "furnishing/f1/0", "scope": "room"}));
        assert_eq!(r["replaced"], json!(["furnishing/f0/0", "furnishing/f1/0"]));
        let sofa = s.library.symbol("sofa-futon").unwrap();
        let m = &s.doc().unwrap().arch;
        let f1 = m.furnishing(ArchId(1)).unwrap();
        assert_eq!(
            (f1.item.as_str(), f1.symbol.as_str(), f1.at, f1.rotation, f1.elevation),
            ("sofa-futon", sofa.key.as_str(), [3.0, 3.0], 90.0, 0.1)
        );
        assert_eq!(f1.size, s.library.item("sofa-futon").and_then(|r| r.size).unwrap());
        assert_eq!(m.furnishing(ArchId(2)).unwrap().item, "armchair");
        // The armchair symbol stays for f2; the old part paint is gone.
        assert!(m.symbols.contains_key(&chair.key) && m.symbols.contains_key(&sofa.key));
        assert!(!m.paints.contains_key("furnishing/f1/0"));
        // A pull is no furnishing.
        assert!(s.execute("paint.object", &json!({"item": "own:pull-knob-32", "target": "furnishing/f2/0", "scope": "component"})).is_err());
    }

    #[test]
    fn paint_object_refuses_a_mismatch() {
        let dir = TempDir::new("paint-mismatch");
        let mut s = setup(&dir);
        let before = s.doc().unwrap().arch.clone();
        let depth = undo_depth(&s);
        for p in [
            json!({"item": "sofa-chaise", "target": "cabinet/cb0/pull", "scope": "room"}),
            json!({"item": "sofa-chaise", "target": "cabinet/cb0/front", "scope": "component"}),
            json!({"item": "own:pull-knob-32", "target": "wall/w0/left", "scope": "room"}),
            json!({"item": "own:pull-knob-32", "target": "cabinet/cb0/front", "scope": "component"}),
            json!({"item": "own:pull-knob-32", "target": "cabinet/cb9/pull", "scope": "room"}),
            json!({"item": "own:pull-knob-32", "target": "cabinet/cb0/pull", "scope": "galaxy"}),
            json!({"item": "own:pull-knob-32", "target": "cabinet/cb0/pull"}),
            json!({"item": "frontstyle:door-shaker", "target": "cabinet/cb0/pull", "scope": "component"}),
            json!({"item": "material-oak", "target": "cabinet/cb0/pull", "scope": "component"}),
            json!({"item": "finish:brick", "target": "cabinet/cb0/carcass", "scope": "component"}),
            json!({"item": "cabinet:base", "target": "cabinet/cb0/front", "scope": "component"}),
            json!({"item": "no-such-item", "target": "cabinet/cb0/pull", "scope": "component"}),
            json!({"item": 3, "target": "cabinet/cb0/pull", "scope": "component"}),
            json!({"target": "cabinet/cb0/pull", "scope": "component"}),
        ] {
            assert!(s.execute("paint.object", &p).is_err(), "{p}");
        }
        let e = s.execute("paint.object", &json!({"item": "sofa-chaise", "target": "cabinet/cb0/pull", "scope": "room"})).unwrap_err().to_string();
        assert!(e.contains("cabinet/cb0/pull"), "{e}");
        assert_eq!(s.doc().unwrap().arch, before);
        assert_eq!(undo_depth(&s), depth);
    }

    #[test]
    fn pick_reads_the_effective_material() {
        let dir = TempDir::new("paint-pick");
        let mut s = setup(&dir);
        let r = run(&mut s, "paint.pick", json!({"target": "wall/w0/left"}));
        assert_eq!(r, json!({"material": "default:wall", "tint": null}));
        paint(&mut s, "brick", "wall/w0/left", "component");
        run(&mut s, "paint.material", json!({"material": "#c04020", "target": "wall/w0/left", "scope": "component", "mode": "blend"}));
        let r = run(&mut s, "paint.pick", json!({"target": "wall/w0/left"}));
        assert_eq!(r, json!({"material": "finish:brick", "tint": "#c04020"}));
        let r = run(&mut s, "paint.pick", json!({"target": "cabinet/cb0/pull"}));
        assert_eq!(r, json!({"material": "item:own:pull-bar-128", "tint": null}));
        // The eyedropper changes nothing and records no undo step.
        let before = s.doc().unwrap().arch.clone();
        let depth = undo_depth(&s);
        run(&mut s, "paint.pick", json!({"target": "room/r0/ceiling"}));
        assert_eq!((s.doc().unwrap().arch.clone(), undo_depth(&s)), (before, depth));
    }

    #[test]
    fn the_painters_take_the_eyedropper_pick() {
        let dir = TempDir::new("paint-interactive");
        let mut s = setup(&dir);
        paint(&mut s, "brick", "wall/w0/right", "component");
        let depth = undo_depth(&s);
        s.start("paint.pick").unwrap();
        s.input(Input::Text("wall/w0/right".into())).unwrap();
        assert!(s.running.is_none());
        // The Material Painter starts with the pick and paints surfaces until Enter: one undo step.
        s.start("paint.material").unwrap();
        assert!(s.current_prompt().unwrap().message.contains("finish:brick"));
        s.input(Input::Text("wall/w1/right".into())).unwrap();
        s.input(Input::Keyword("Room".into())).unwrap();
        s.input(Input::Text("cabinet/cb0/carcass".into())).unwrap();
        s.input(Input::Enter).unwrap();
        assert!(s.running.is_none());
        assert_eq!(material(&s, "wall/w1/right"), Some(MaterialRef::Finish(Finish::Brick)));
        assert_eq!(material(&s, "cabinet/cb1/carcass"), Some(MaterialRef::Finish(Finish::Brick)));
        assert_eq!(undo_depth(&s), depth + 1);
        // The pick was used: the next painter asks for its material; a refused surface re-prompts.
        s.start("paint.material").unwrap();
        assert!(s.current_prompt().unwrap().message.starts_with("Enter a material"));
        s.input(Input::Text("#c04020".into())).unwrap();
        s.input(Input::Text("wall/w9/left".into())).unwrap();
        assert!(s.running.is_some());
        s.input(Input::Text("wall/w0/left".into())).unwrap();
        s.input(Input::Enter).unwrap();
        assert_eq!(material(&s, "wall/w0/left"), Some(MaterialRef::Colour([0xc0, 0x40, 0x20])));
        // The Object Painter takes the item the eyedropper found on a pull.
        run(&mut s, "paint.object", json!({"item": "own:pull-knob-32", "target": "cabinet/cb2/pull", "scope": "component"}));
        s.start("paint.pick").unwrap();
        s.input(Input::Text("cabinet/cb2/pull".into())).unwrap();
        s.start("paint.object").unwrap();
        s.input(Input::Keyword("Room".into())).unwrap();
        s.input(Input::Text("cabinet/cb0/pull".into())).unwrap();
        s.input(Input::Enter).unwrap();
        let pulls: Vec<_> = s.doc().unwrap().arch.cabinets.iter().map(|c| c.pull.clone().unwrap_or_default()).collect();
        assert_eq!(pulls, ["own:pull-knob-32"; 3]);
        // A pick the painter cannot use (a default material) is not offered.
        s.start("paint.pick").unwrap();
        s.input(Input::Text("room/r0/floor".into())).unwrap();
        s.start("paint.material").unwrap();
        assert!(s.current_prompt().unwrap().message.starts_with("Enter a material"));
        s.input(Input::Cancel).unwrap();
    }
}
