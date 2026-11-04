//! The painters (phase 14): `paint.material` (Material Painter), `paint.object` (Object Painter)
//! and `paint.pick` (Material Eyedropper).
//!
//! Surfaces are addressed by their key (`wall/w3/left`, `cabinet/cb0/pull`, ...), materials by a
//! finish name, `#rrggbb` or a library item id. Everything is validated before the drawing is
//! written, so a refused call changes nothing; a successful one is one undo step. Hostile
//! parameters are refused, never repaired.

use std::sync::{Mutex, PoisonError};

use cadcraft_arch::library::{ItemKind, ItemRecord};
use cadcraft_arch::paint::{Scope, effective_material, finish_fits, select, select_objects, surfaces};
use cadcraft_doc::arch::{ArchId, CabinetPart, Finish, FrontStyle, MaterialRef, Paint, Surface};
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::prompt::{Accept, Input, Interactive, Prompt, Step};
use crate::{Result, Session};

/// Command specs for the painters.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("paint.material", "Material Painter", run_material)
            .menu(&["Architecture", "Material Painter"])
            .params("{material: \"siding\" | \"#c04020\" | \"material-oak\", target: \"wall/w3/left\", scope?: \"component\"|\"object\"|\"room\"|\"floor\"|\"plan\", mode?: \"paint\"|\"blend\"}")
            .interactive(|_| Ok(Box::new(PaintM::new(Kind::Material)))),
        CommandSpec::new("paint.object", "Object Painter", run_object)
            .menu(&["Architecture", "Object Painter"])
            .params("{item: \"own:pull-knob-32\", target: \"cabinet/cb0/pull\", scope?: \"component\"|\"object\"|\"room\"|\"floor\"|\"plan\"}")
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

fn clip(s: &str) -> String {
    s.chars().take(128).collect()
}

fn parse_target(cmd: &str, m: &Map<String, Value>) -> Result<Surface> {
    let t = text(cmd, m, "target")?;
    Surface::parse(t).ok_or_else(|| bad(cmd, format!("`{}` is not a surface key (wall/w3/left, room/r0/floor, cabinet/cb0/pull, ...)", clip(t))))
}

fn parse_scope(cmd: &str, m: &Map<String, Value>) -> Result<Scope> {
    match opt_text(cmd, m, "scope")? {
        None => Ok(Scope::Component),
        Some(s) => Scope::parse(s).ok_or_else(|| bad(cmd, format!("unknown scope `{}` (component, object, room, floor, plan)", clip(s)))),
    }
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

// ---- paint.material ----

/// A material resolved against the library: what to store and its mean colour.
struct Resolved {
    material: MaterialRef,
    mean: [u8; 3],
    image: Option<(String, cadcraft_doc::arch::ImageRgba)>,
}

fn finish_mean(f: Finish) -> [u8; 3] {
    cadcraft_arch3d::texture::mean_srgb(&cadcraft_arch3d::texture::texture(f))
}

fn resolve_material(s: &mut Session, raw: &str) -> Result<Resolved> {
    const CMD: &str = "paint.material";
    let m = MaterialRef::parse(raw).ok_or_else(|| bad(CMD, format!("`{}` is not a material", clip(raw))))?;
    match m {
        MaterialRef::Finish(f) => Ok(Resolved { material: m, mean: finish_mean(f), image: None }),
        MaterialRef::Colour(c) => Ok(Resolved { material: m, mean: c, image: None }),
        MaterialRef::Item(id) => {
            s.library.sync_io();
            let rec = s.library.item(&id).cloned().ok_or_else(|| bad(CMD, format!("no library item `{}`", clip(&id))))?;
            match rec.kind {
                ItemKind::Finish => {
                    let f = id
                        .strip_prefix("finish:")
                        .and_then(Finish::parse)
                        .ok_or_else(|| bad(CMD, format!("`{}` is not a finish this build knows", clip(&id))))?;
                    Ok(Resolved { material: MaterialRef::Finish(f), mean: finish_mean(f), image: None })
                }
                ItemKind::Material => {
                    let img = s.library.image(&id).map_err(|e| bad(CMD, e))?;
                    Ok(Resolved { material: MaterialRef::Item(id.clone()), mean: img.mean_srgb(), image: Some((id, img)) })
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
    let chosen = {
        let model = &s.doc()?.arch;
        select(model, &target, scope).map_err(|e| bad(CMD, e))?
    };
    let res = resolve_material(s, raw)?;
    if let MaterialRef::Finish(f) = &res.material {
        for sf in &chosen {
            if !finish_fits(sf, *f) {
                return Err(bad(CMD, format!("{} does not fit {}", f.name(), sf.key())));
            }
        }
    }
    let d = s.doc_mut()?;
    if let Some((id, img)) = res.image {
        d.arch.images.insert(id, img);
    }
    for sf in &chosen {
        let key = sf.key();
        let old = d.arch.paints.remove(&key);
        let paint = if blend {
            Paint { material: old.and_then(|o| o.material), tint: Some(res.mean) }
        } else {
            Paint { material: Some(res.material.clone()), tint: None }
        };
        // Painting back to the unpainted material leaves no entry.
        let plain = !blend && effective_material(&d.arch, sf) == res.material.key();
        if !plain {
            d.arch.paints.insert(key, paint);
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
    Pull { id: String, key: String, mesh: cadcraft_doc::arch::SymbolMesh },
    Front { door: bool, style: FrontStyle },
    Furnishing { rec: ItemRecord, key: String, mesh: cadcraft_doc::arch::SymbolMesh },
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

fn run_object(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "paint.object";
    let m = object(CMD, p)?;
    let id = text(CMD, m, "item")?;
    let target = parse_target(CMD, m)?;
    let scope = parse_scope(CMD, m)?;
    let chosen = {
        let model = &s.doc()?.arch;
        select_objects(model, &target, scope).map_err(|e| bad(CMD, e))?
    };
    s.library.sync_io();
    let rec = s.library.item(id).cloned().ok_or_else(|| bad(CMD, format!("no library item `{}`", clip(id))))?;
    let mismatch = |what: &str| bad(CMD, format!("`{}` is {what}, which does not fit {}", clip(id), target.key()));
    let is_pull = |sf: &Surface| matches!(sf, Surface::Cabinet { part: CabinetPart::Pull, .. });
    let is_front = |sf: &Surface| matches!(sf, Surface::Cabinet { part: CabinetPart::Front, .. });
    let is_furn = |sf: &Surface| matches!(sf, Surface::Furnishing { .. });
    let plan = match rec.kind {
        ItemKind::Symbol if rec.symbol_type == "pull" => {
            if !chosen.iter().all(is_pull) {
                return Err(mismatch("a pull"));
            }
            let mesh = s.library.symbol(id).map_err(|e| bad(CMD, e))?;
            Replace::Pull { id: rec.id.clone(), key: mesh.key.clone(), mesh }
        }
        ItemKind::Symbol => {
            if !chosen.iter().all(is_furn) {
                return Err(mismatch(&format!("a {}", if rec.symbol_type.is_empty() { "furnishing" } else { rec.symbol_type.as_str() })));
            }
            let mesh = s.library.symbol(id).map_err(|e| bad(CMD, e))?;
            Replace::Furnishing { key: mesh.key.clone(), rec, mesh }
        }
        ItemKind::FrontStyle => {
            let (door, style) = front_style(&rec).ok_or_else(|| bad(CMD, format!("`{}` is not a door or drawer front style", clip(id))))?;
            if !chosen.iter().all(is_front) {
                return Err(mismatch("a front style"));
            }
            Replace::Front { door, style }
        }
        _ => return Err(bad(CMD, format!("`{}` cannot be painted onto a surface", clip(id)))),
    };

    let d = s.doc_mut()?;
    let a = &mut d.arch;
    let cabinet_ids: Vec<ArchId> = chosen
        .iter()
        .filter_map(|sf| match sf {
            Surface::Cabinet { cabinet, .. } => Some(*cabinet),
            _ => None,
        })
        .collect();
    match plan {
        Replace::Pull { id, key, mesh } => {
            for c in a.cabinets.iter_mut().filter(|c| cabinet_ids.contains(&c.id)) {
                c.pull = Some(id.clone());
                c.pull_symbol = Some(key.clone());
            }
            a.symbols.insert(key, mesh);
        }
        Replace::Front { door, style } => {
            for c in a.cabinets.iter_mut().filter(|c| cabinet_ids.contains(&c.id)) {
                if door {
                    c.door_style = style;
                } else {
                    c.drawer_style = style;
                }
            }
        }
        Replace::Furnishing { rec, key, mesh } => {
            let ids: Vec<ArchId> = chosen
                .iter()
                .filter_map(|sf| match sf {
                    Surface::Furnishing { furnishing, .. } => Some(*furnishing),
                    _ => None,
                })
                .collect();
            let size = rec.size.unwrap_or_else(|| mesh.extents());
            for f in a.furnishings.iter_mut().filter(|f| ids.contains(&f.id)) {
                f.item = rec.id.clone();
                f.symbol = key.clone();
                f.name = rec.name.clone();
                f.category = rec.category.clone();
                f.symbol_type = rec.symbol_type.clone();
                f.size = size;
                f.cutout = rec.cutout;
            }
            // The old symbol's part paints no longer address parts of this one.
            for fid in &ids {
                let prefix = format!("furnishing/{}/", cadcraft_doc::arch::furnishing_label(*fid));
                a.paints.retain(|k, _| !k.starts_with(&prefix));
            }
            a.symbols.insert(key, mesh);
        }
    }
    // Drop symbols nothing uses any more.
    let used: Vec<String> = a.furnishings.iter().map(|f| f.symbol.clone()).chain(a.cabinets.iter().filter_map(|c| c.pull_symbol.clone())).collect();
    a.symbols.retain(|k, _| used.contains(k));
    d.touch_arch();
    let mut replaced: Vec<String> = chosen.iter().map(Surface::key).collect();
    replaced.sort();
    replaced.dedup();
    Ok(json!({ "replaced": replaced }))
}

// ---- paint.pick ----

fn run_pick(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "paint.pick";
    let m = object(CMD, p)?;
    let target = parse_target(CMD, m)?;
    let model = &s.doc()?.arch;
    if !surfaces(model).iter().any(|i| i.surface == target) {
        return Err(bad(CMD, format!("no such surface: {}", target.key())));
    }
    let material = effective_material(model, &target);
    let tint = model.paints.get(&target.key()).and_then(|p| p.tint).map(hex);
    remember(&material);
    Ok(json!({ "material": material, "tint": tint }))
}

// ---- interactive forms ----

/// The material key of the last eyedropper pick, for the painters' prompts.
static LAST_PICK: Mutex<Option<String>> = Mutex::new(None);

fn remember(material: &str) {
    *LAST_PICK.lock().unwrap_or_else(PoisonError::into_inner) = Some(material.to_string());
}

/// The last pick when it is something a painter can use (`finish:`, `item:`, `colour:`).
fn last_pick() -> Option<String> {
    LAST_PICK.lock().unwrap_or_else(PoisonError::into_inner).clone().filter(|k| ["finish:", "item:", "colour:"].iter().any(|p| k.starts_with(p)))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Material,
    Object,
    Pick,
}

/// Asks for the material or item (unless the eyedropper supplied one), then the surface (a 3D
/// pick arrives as its key typed in), then runs the JSON command: one implementation, one undo.
struct PaintM {
    kind: Kind,
    value: Option<String>,
    scope: &'static str,
}

impl PaintM {
    fn new(kind: Kind) -> PaintM {
        PaintM { kind, value: None, scope: "component" }
    }
}

const SCOPES: [&str; 5] = ["component", "object", "room", "floor", "plan"];

impl Interactive for PaintM {
    fn name(&self) -> &'static str {
        match self.kind {
            Kind::Material => "paint.material",
            Kind::Object => "paint.object",
            Kind::Pick => "paint.pick",
        }
    }

    fn begin(&mut self, _s: &mut Session) -> Result<Step> {
        if let Some(k) = last_pick() {
            match self.kind {
                Kind::Material => self.value = Some(k),
                Kind::Object => self.value = k.strip_prefix("item:").map(str::to_string),
                Kind::Pick => {}
            }
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
            Input::Text(t) | Input::Keyword(t) => t.trim().to_string(),
            _ => return Ok(Step::Continue),
        };
        if t.is_empty() {
            return Ok(Step::Continue);
        }
        if self.kind == Kind::Pick {
            s.execute("paint.pick", &json!({ "target": t }))?;
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
        let key = if self.kind == Kind::Material { "material" } else { "item" };
        let name = self.name();
        s.execute(name, &json!({ key: value, "target": t, "scope": self.scope }))?;
        Ok(Step::Done)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;
    use crate::library::testing::session;
    use crate::temp_dir::TempDir;
    use cadcraft_doc::arch::Cabinet;
    use cadcraft_doc::{Room, Wall};
    use cadcraft_geom::Vec2;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    /// Two rooms side by side, each with a south wall facing it (left side), and a cabinet with a
    /// pull in each room plus one with a pull in room B only.
    fn setup(dir: &std::path::Path) -> Session {
        let mut s = session(dir);
        let d = s.doc_mut().unwrap();
        for x in [0.0, 10.0] {
            d.arch.push_wall(Wall::new(Vec2::new(x, 0.0), Vec2::new(x + 4.0, 0.0), 0.2));
            d.arch.push_room(Room { polygon: rect(x, 0.0, x + 4.0, 4.0), ..Default::default() });
        }
        for (x, pull) in [(1.0, "own:pull-bar-128"), (2.0, "own:pull-bar-128"), (11.0, "own:pull-bar-128")] {
            d.arch.push_cabinet(Cabinet { width: 0.6, depth: 0.6, position: Vec2::new(x, 1.0), pull: Some(pull.into()), ..Default::default() });
        }
        d.touch_arch();
        s
    }

    fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
        s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
    }

    fn paints(s: &Session) -> usize {
        s.doc().unwrap().arch.paints.len()
    }

    #[test]
    fn paint_room_scope_takes_like_materials_in_the_room() {
        let dir = TempDir::new("paint-room");
        let mut s = setup(&dir);
        let r = run(&mut s, "paint.material", json!({"material": "#c04020", "target": "room/r0/floor", "scope": "room"}));
        assert_eq!(r["painted"], json!(["room/r0/floor"]));
        // The wall's left side faces room 0 with the same default wall material as nothing else
        // in the room: scope room takes like materials only.
        let r = run(&mut s, "paint.material", json!({"material": "#102030", "target": "wall/w0/left", "scope": "room"}));
        assert_eq!(r["painted"], json!(["wall/w0/left"]));
        let m = &s.doc().unwrap().arch;
        assert_eq!(m.paints.get("room/r0/floor").and_then(|p| p.material.clone()), Some(MaterialRef::Colour([0xc0, 0x40, 0x20])));
        assert!(!m.paints.contains_key("room/r1/floor"));
        assert!(!m.paints.contains_key("wall/w1/left"));
    }

    #[test]
    fn paint_plan_scope_takes_every_like_material() {
        let dir = TempDir::new("paint-plan");
        let mut s = setup(&dir);
        let r = run(&mut s, "paint.material", json!({"material": "brick", "target": "wall/w0/right", "scope": "plan"}));
        // Every wall side shares the default wall material, so all four are like surfaces.
        assert_eq!(r["painted"], json!(["wall/w0/left", "wall/w0/right", "wall/w1/left", "wall/w1/right"]));
        let r = run(&mut s, "paint.material", json!({"material": "#ffffff", "target": "room/r0/ceiling", "scope": "plan"}));
        assert_eq!(r["painted"], json!(["room/r0/ceiling", "room/r1/ceiling"]));
    }

    #[test]
    fn paint_blend_keeps_the_material_and_tints() {
        let dir = TempDir::new("paint-blend");
        let mut s = setup(&dir);
        run(&mut s, "paint.material", json!({"material": "brick", "target": "wall/w0/right"}));
        run(&mut s, "paint.material", json!({"material": "#c04020", "target": "wall/w0/right", "mode": "blend"}));
        let p = s.doc().unwrap().arch.paints.get("wall/w0/right").cloned().unwrap();
        assert_eq!(p, Paint { material: Some(MaterialRef::Finish(Finish::Brick)), tint: Some([0xc0, 0x40, 0x20]) });
        // On an unpainted surface the material stays unpainted and the tint is the finish's mean.
        run(&mut s, "paint.material", json!({"material": "wood", "target": "room/r1/floor", "mode": "blend"}));
        let p = s.doc().unwrap().arch.paints.get("room/r1/floor").cloned().unwrap();
        assert_eq!(p.material, None);
        assert_eq!(p.tint, Some(finish_mean(Finish::Wood)));
    }

    #[test]
    fn paint_refuses_unfit_finishes_and_unknown_targets() {
        let dir = TempDir::new("paint-refuse");
        let mut s = setup(&dir);
        let before = s.doc().unwrap().arch.clone();
        for p in [
            json!({"material": "shingles", "target": "wall/w0/left"}),
            json!({"material": "#c04020", "target": "wall/w9/left"}),
            json!({"material": "#c04020", "target": "nonsense"}),
            json!({"material": "#c04020", "target": "wall/w0/left", "scope": "galaxy"}),
            json!({"material": "#c04020", "target": "wall/w0/left", "mode": "smear"}),
            json!({"material": "no-such-item", "target": "wall/w0/left"}),
            json!({"material": "armchair", "target": "wall/w0/left"}),
            json!({"material": 5, "target": "wall/w0/left"}),
            json!({"target": "wall/w0/left"}),
        ] {
            assert!(s.execute("paint.material", &p).is_err(), "{p}");
        }
        assert!(s.execute("paint.pick", &json!({"target": "wall/w9/left"})).is_err());
        assert_eq!(s.doc().unwrap().arch, before);
    }

    #[test]
    fn paint_stores_the_item_image() {
        let dir = TempDir::new("paint-image");
        let mut s = setup(&dir);
        run(&mut s, "paint.material", json!({"material": "material-oak", "target": "room/r0/floor"}));
        let m = &s.doc().unwrap().arch;
        assert_eq!(m.paints.get("room/r0/floor").and_then(|p| p.material.clone()), Some(MaterialRef::Item("material-oak".into())));
        assert!(m.images.get("material-oak").is_some_and(|i| i.is_valid()));
        // A library finish item paints the finish.
        run(&mut s, "paint.material", json!({"material": "finish:wood", "target": "room/r1/floor"}));
        let m = &s.doc().unwrap().arch;
        assert_eq!(m.paints.get("room/r1/floor").and_then(|p| p.material.clone()), Some(MaterialRef::Finish(Finish::Wood)));
    }

    #[test]
    fn paint_is_one_undo_step() {
        let dir = TempDir::new("paint-undo");
        let mut s = setup(&dir);
        let before = s.doc().unwrap().arch.clone();
        let r = run(&mut s, "paint.material", json!({"material": "#c04020", "target": "wall/w0/right", "scope": "plan"}));
        assert_eq!(r["painted"].as_array().map(Vec::len), Some(4));
        assert_eq!(paints(&s), 4);
        run(&mut s, "undo", json!({}));
        assert_eq!(s.doc().unwrap().arch, before);
        // Painting the unpainted material back removes the entry.
        run(&mut s, "paint.material", json!({"material": "#c04020", "target": "room/r0/floor"}));
        assert_eq!(paints(&s), 1);
        run(&mut s, "paint.material", json!({"material": "default:floor", "target": "room/r0/floor"}));
        let n = paints(&s);
        assert!(n <= 1);
    }

    #[test]
    fn paint_object_replaces_like_pulls_in_the_room() {
        let dir = TempDir::new("paint-object");
        let mut s = setup(&dir);
        let r = run(&mut s, "paint.object", json!({"item": "own:pull-knob-32", "target": "cabinet/cb0/pull", "scope": "room"}));
        assert_eq!(r["replaced"], json!(["cabinet/cb0/pull", "cabinet/cb1/pull"]));
        let m = &s.doc().unwrap().arch;
        let pulls: Vec<_> = m.cabinets.iter().map(|c| c.pull.clone().unwrap_or_default()).collect();
        assert_eq!(pulls, ["own:pull-knob-32", "own:pull-knob-32", "own:pull-bar-128"]);
        assert!(m.cabinets[0].pull_symbol.as_ref().is_some_and(|k| m.symbols.contains_key(k)));
        // A front style.
        run(&mut s, "paint.object", json!({"item": "frontstyle:door-shaker", "target": "cabinet/cb2/front"}));
        assert_eq!(s.doc().unwrap().arch.cabinets[2].door_style, FrontStyle::Shaker);
    }

    #[test]
    fn paint_object_refuses_a_mismatch() {
        let dir = TempDir::new("paint-mismatch");
        let mut s = setup(&dir);
        let before = s.doc().unwrap().arch.clone();
        for p in [
            json!({"item": "sofa-chaise", "target": "cabinet/cb0/pull"}),
            json!({"item": "own:pull-knob-32", "target": "wall/w0/left"}),
            json!({"item": "frontstyle:door-shaker", "target": "cabinet/cb0/pull"}),
            json!({"item": "material-oak", "target": "cabinet/cb0/pull"}),
            json!({"item": "no-such-item", "target": "cabinet/cb0/pull"}),
            json!({"target": "cabinet/cb0/pull"}),
        ] {
            assert!(s.execute("paint.object", &p).is_err(), "{p}");
        }
        assert_eq!(s.doc().unwrap().arch, before);
    }

    #[test]
    fn pick_reads_the_effective_material() {
        let dir = TempDir::new("paint-pick");
        let mut s = setup(&dir);
        let r = run(&mut s, "paint.pick", json!({"target": "wall/w0/left"}));
        assert_eq!(r, json!({"material": "default:wall", "tint": null}));
        run(&mut s, "paint.material", json!({"material": "brick", "target": "wall/w0/left"}));
        run(&mut s, "paint.material", json!({"material": "#c04020", "target": "wall/w0/left", "mode": "blend"}));
        let r = run(&mut s, "paint.pick", json!({"target": "wall/w0/left"}));
        assert_eq!(r, json!({"material": "finish:brick", "tint": "#c04020"}));
    }
}
