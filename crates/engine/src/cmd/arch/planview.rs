//! Saved plan views as commands: create, duplicate (`from`), edit and rename, delete, open, close
//! and list. A view is a named object in the drawing (`ArchModel::plan_views`): a level (or any
//! level), a layer state of wildcard rules and/or captured layers with transparency, an
//! annotation set (layer, text style, dimension style) and a zoom. Open views are tabs
//! (`open_plan_views`), one of them active.
//!
//! Every command edits clones, validates, and only then commits with one `doc_mut`, so a refusal
//! changes nothing and pushes no undo step. Nothing here calls `touch_arch`: the 2D plan is
//! rebuilt on every build, exactly as `level.show` leaves it.

use cadcraft_doc::arch::{AnnotationSet, MAX_PLAN_VIEWS, PlanKind, PlanView, ReferenceDisplay, RoomLabelShow, ViewZoom, validate_plan_view};
use cadcraft_doc::{Drawing, LayerRule, MAX_LAYER_PATTERN, MAX_LAYER_RULES, Space};
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::{EngineError, Result, Session, View};

const MENU: &str = "Plan Views";

/// The plan view commands.
macro_rules! view_fields {
    () => {
        "level?: name|index|\"any\"|null, kind?: \"floor\"|\"reflectedCeiling\"|\"roof\", layers?: {rules?: [{pattern, on?: bool, transparency?: 0..90}], capture?: bool}, annotation?: {layer?, textStyle?, dimStyle?}, labels?: {name?, area?, dimensions?, ceiling?, numbers?: bool}, reference?: {level, color?: \"#rrggbb\", transparency?: 0..90}|null, zoom?: \"current\"|{center: [x, y], width, height}|null"
    };
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("planview", "New Plan View", run_new).menu(&["Architecture", MENU, "New Plan View"]).params(concat!(
            "{name, from?: view name (default \"Working\"), ",
            view_fields!(),
            ", open?: bool (default true)}"
        )),
        CommandSpec::new("planview.edit", "Edit Plan View", run_edit).menu(&["Architecture", MENU, "Edit Plan View"]).params(concat!(
            "{name, rename?, ",
            view_fields!(),
            "}"
        )),
        CommandSpec::new("planview.delete", "Delete Plan View", run_delete).menu(&["Architecture", MENU, "Delete Plan View"]).params("{name}"),
        CommandSpec::new("planview.open", "Open Plan View", run_open).menu(&["Architecture", MENU, "Open Plan View"]).params("{name}"),
        CommandSpec::new("planview.close", "Close Plan View", run_close)
            .menu(&["Architecture", MENU, "Close Plan View"])
            .params("{name?: default the active view}"),
        CommandSpec::new("planview.list", "List Plan Views", run_list).menu(&["Architecture", MENU, "List Plan Views"]).params("{}").noundo(),
    ]
}

/// What applying a plan view left in the layer table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ApplyReport {
    /// Layers on at transparency 0.
    pub on: usize,
    /// Layers off.
    pub off: usize,
    /// Layers on at a transparency above 0.
    pub dimmed: usize,
}

/// Create every layer the plan of `v` can draw the model's objects on (so its rules can hide
/// them), then the annotation layer when it has one. Applies nothing.
pub(crate) fn ensure_plan_layers(d: &mut Drawing, v: &PlanView) {
    for name in cadcraft_arch::plan::layers_in_use(&d.arch, v.kind) {
        d.ensure_layer(&name);
    }
    if !v.annotation.layer.is_empty() {
        d.ensure_layer(&v.annotation.layer);
    }
}

/// Apply `v` to the drawing: its layers, shown level, annotation defaults, and make it the active
/// tab (opening the tab when absent). Pure on the drawing; the zoom is the session's.
pub(crate) fn apply_plan_view(d: &mut Drawing, v: &PlanView) -> ApplyReport {
    ensure_plan_layers(d, v);
    v.layers.apply(&mut d.layers);
    if let Some(l) = v.level
        && (l as usize) < d.arch.levels.len()
    {
        d.arch.plan_level = Some(l);
    }
    let layer = d.layer(&v.annotation.layer).map(|l| l.name.clone());
    if let Some(name) = layer.filter(|_| !v.annotation.layer.is_empty()) {
        d.header.set_str("CLAYER", &name);
    }
    if !v.annotation.text_style.is_empty()
        && let Some(name) = d.text_style(&v.annotation.text_style).map(|t| t.name.clone())
    {
        d.header.set_str("TEXTSTYLE", &name);
    }
    if !v.annotation.dim_style.is_empty()
        && let Some(name) = d.dim_style(&v.annotation.dim_style).map(|t| t.name.clone())
    {
        d.header.set_str("DIMSTYLE", &name);
    }
    d.arch.active_plan_view = Some(v.name.clone());
    if !d.arch.open_plan_views.iter().any(|n| n.eq_ignore_ascii_case(&v.name)) {
        d.arch.open_plan_views.push(v.name.clone());
    }
    let mut r = ApplyReport::default();
    for l in &d.layers {
        if !l.on {
            r.off += 1;
        } else if l.transparency > 0 {
            r.dimmed += 1;
        } else {
            r.on += 1;
        }
    }
    r
}

/// Show `v`'s zoom, when it has one, as a named view's model branch restores its area.
fn apply_zoom(s: &mut Session, v: &PlanView) -> Result<()> {
    let Some(z) = v.zoom else { return Ok(()) };
    if s.layout_space() != Space::Model && s.state()?.active_viewport().is_none() {
        crate::cmd::view::run_layout_set(s, &json!({ "name": "Model" }))?;
    }
    let (w, h) = s.viewport_px;
    let st = s.state_mut()?;
    let aspect = st.zoom_aspect(w / h.max(1.0)).max(1e-9);
    st.set_zoom_frame(View { center: cadcraft_geom::Vec2::new(z.center[0], z.center[1]), height: z.height.max(z.width / aspect), twist: 0.0 });
    Ok(())
}

/// Open the plan view `name`: apply it (layers, level, annotation), make its tab active, and show
/// its zoom. Used by `planview.open` and by exports.
pub(crate) fn open_plan_view(s: &mut Session, name: &str) -> Result<Value> {
    let v = s.doc()?.arch.plan_view(name).cloned().ok_or_else(|| bad("planview.open", format!("`name`: no plan view `{}`", name.trim())))?;
    let report = apply_plan_view(s.doc_mut()?, &v);
    apply_zoom(s, &v)?;
    let d = s.doc()?;
    Ok(json!({
        "planView": v.name,
        "level": d.arch.plan_level,
        "kind": v.kind.key(),
        "layers": { "on": report.on, "off": report.off, "dimmed": report.dimmed },
        "clayer": d.header.str("CLAYER", "0"),
        "open": d.arch.open_plan_views,
        "active": d.arch.active_plan_view,
    }))
}

// ---------------------------------------------------------------- parsing

fn object<'a>(cmd: &str, key: &str, v: &'a Value, allowed: &[&str]) -> Result<&'a Map<String, Value>> {
    let m = v.as_object().ok_or_else(|| bad(cmd, format!("`{key}` must be an object")))?;
    if let Some(k) = m.keys().find(|k| !allowed.contains(&k.as_str())) {
        let at = if key.is_empty() { k.to_string() } else { format!("{key}.{k}") };
        return Err(bad(cmd, format!("unknown parameter `{at}`")));
    }
    Ok(m)
}

fn string(cmd: &str, key: &str, v: &Value) -> Result<String> {
    v.as_str().map(str::to_string).ok_or_else(|| bad(cmd, format!("`{key}` must be a string")))
}

fn boolean(cmd: &str, key: &str, v: &Value) -> Result<bool> {
    v.as_bool().ok_or_else(|| bad(cmd, format!("`{key}` must be true or false")))
}

fn transparency(cmd: &str, key: &str, v: &Value) -> Result<u8> {
    v.as_u64().filter(|t| *t <= 90).map(|t| t as u8).ok_or_else(|| bad(cmd, format!("`{key}` must be a whole number 0..90")))
}

fn finite(cmd: &str, key: &str, v: &Value) -> Result<f64> {
    v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite number")))
}

/// A level by name or index, as `level.show` resolves them.
fn level_index(cmd: &str, key: &str, d: &Drawing, v: &Value) -> Result<u32> {
    let levels = &d.arch.levels;
    let found = match v {
        Value::Number(n) => n.as_u64().filter(|i| (*i as usize) < levels.len()),
        Value::String(n) => match n.trim().parse::<u64>() {
            Ok(i) => Some(i).filter(|i| (*i as usize) < levels.len()),
            Err(_) => levels.iter().position(|l| l.name.eq_ignore_ascii_case(n.trim())).map(|i| i as u64),
        },
        _ => return Err(bad(cmd, format!("`{key}` must be a level name or index"))),
    };
    found.and_then(|i| u32::try_from(i).ok()).ok_or_else(|| bad(cmd, format!("`{key}`: no such level")))
}

fn parse_color(cmd: &str, key: &str, v: &Value) -> Result<[u8; 3]> {
    let s = v.as_str().ok_or_else(|| bad(cmd, format!("`{key}` must be \"#rrggbb\"")))?;
    let hex = s.strip_prefix('#').filter(|h| h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()));
    let n = hex.and_then(|h| u32::from_str_radix(h, 16).ok()).ok_or_else(|| bad(cmd, format!("`{key}` must be \"#rrggbb\"")))?;
    Ok([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

/// Apply the view fields of `p` (everything but the name, `from`, `rename` and `open`) to `view`.
fn parse_fields(cmd: &str, p: &Map<String, Value>, view: &mut PlanView, d: &Drawing, current: Option<ViewZoom>) -> Result<()> {
    if let Some(v) = p.get("level") {
        view.level = match v {
            Value::Null => None,
            Value::String(s) if s.trim().eq_ignore_ascii_case("any") => None,
            _ => Some(level_index(cmd, "level", d, v)?),
        };
    }
    if let Some(v) = p.get("kind") {
        let k = string(cmd, "kind", v)?;
        view.kind = PlanKind::from_key(&k).ok_or_else(|| bad(cmd, "`kind` must be floor, reflectedCeiling or roof"))?;
    }
    if let Some(v) = p.get("layers") {
        let m = object(cmd, "layers", v, &["rules", "capture"])?;
        if let Some(r) = m.get("rules") {
            let list = r.as_array().ok_or_else(|| bad(cmd, "`layers.rules` must be a list"))?;
            if list.len() > MAX_LAYER_RULES {
                return Err(bad(cmd, format!("`layers.rules` has more than {MAX_LAYER_RULES} rules")));
            }
            let mut rules = Vec::with_capacity(list.len());
            for (i, item) in list.iter().enumerate() {
                let at = format!("layers.rules[{i}]");
                let rm = object(cmd, &at, item, &["pattern", "on", "transparency"])?;
                let pattern =
                    string(cmd, &format!("{at}.pattern"), rm.get("pattern").ok_or_else(|| bad(cmd, format!("`{at}.pattern` is required")))?)?;
                if pattern.trim().is_empty() || pattern.chars().count() > MAX_LAYER_PATTERN || pattern.chars().any(|c| c.is_control()) {
                    return Err(bad(cmd, format!("`{at}.pattern` must be 1..={MAX_LAYER_PATTERN} characters without control characters")));
                }
                let on = match rm.get("on") {
                    None | Some(Value::Null) => None,
                    Some(b) => Some(boolean(cmd, &format!("{at}.on"), b)?),
                };
                let t = match rm.get("transparency") {
                    None | Some(Value::Null) => None,
                    Some(t) => Some(transparency(cmd, &format!("{at}.transparency"), t)?),
                };
                rules.push(LayerRule { pattern, on, transparency: t });
            }
            view.layers.rules = rules;
        }
        if let Some(c) = m.get("capture")
            && boolean(cmd, "layers.capture", c)?
        {
            view.layers.layers = d.layers.clone();
        }
    }
    if let Some(v) = p.get("annotation") {
        let m = object(cmd, "annotation", v, &["layer", "textStyle", "dimStyle"])?;
        let mut a: AnnotationSet = view.annotation.clone();
        if let Some(x) = m.get("layer") {
            a.layer = string(cmd, "annotation.layer", x)?;
        }
        if let Some(x) = m.get("textStyle") {
            a.text_style = string(cmd, "annotation.textStyle", x)?;
        }
        if let Some(x) = m.get("dimStyle") {
            a.dim_style = string(cmd, "annotation.dimStyle", x)?;
        }
        view.annotation = a;
    }
    if let Some(v) = p.get("labels") {
        let m = object(cmd, "labels", v, &["name", "area", "dimensions", "ceiling", "numbers"])?;
        let mut l: RoomLabelShow = view.labels;
        for (key, slot) in [
            ("name", &mut l.name),
            ("area", &mut l.area),
            ("dimensions", &mut l.dimensions),
            ("ceiling", &mut l.ceiling),
            ("numbers", &mut l.numbers),
        ] {
            if let Some(x) = m.get(key) {
                *slot = boolean(cmd, &format!("labels.{key}"), x)?;
            }
        }
        view.labels = l;
    }
    if let Some(v) = p.get("reference") {
        view.reference = match v {
            Value::Null => None,
            _ => {
                let m = object(cmd, "reference", v, &["level", "color", "transparency"])?;
                let mut r: ReferenceDisplay = view.reference.unwrap_or_default();
                match m.get("level") {
                    Some(l) => r.level = level_index(cmd, "reference.level", d, l)?,
                    None if view.reference.is_none() => return Err(bad(cmd, "`reference.level` is required")),
                    None => {}
                }
                if let Some(c) = m.get("color") {
                    r.color = parse_color(cmd, "reference.color", c)?;
                }
                if let Some(t) = m.get("transparency") {
                    r.transparency = transparency(cmd, "reference.transparency", t)?;
                }
                Some(r)
            }
        };
    }
    if let Some(v) = p.get("zoom") {
        view.zoom = match v {
            Value::Null => None,
            Value::String(s) if s == "current" => Some(current.ok_or_else(|| bad(cmd, "`zoom`: there is no current view"))?),
            Value::Object(_) => {
                let m = object(cmd, "zoom", v, &["center", "width", "height"])?;
                let c = m.get("center").and_then(Value::as_array).filter(|a| a.len() == 2).ok_or_else(|| bad(cmd, "`zoom.center` must be [x, y]"))?;
                let width = finite(cmd, "zoom.width", m.get("width").unwrap_or(&Value::Null))?;
                let height = finite(cmd, "zoom.height", m.get("height").unwrap_or(&Value::Null))?;
                if !(0.0 < width && width <= 1e9) || !(0.0 < height && height <= 1e9) {
                    return Err(bad(cmd, "`zoom.width` and `zoom.height` must be in (0, 1e9]"));
                }
                Some(ViewZoom {
                    center: [finite(cmd, "zoom.center", &c[0])?, finite(cmd, "zoom.center", &c[1])?],
                    width,
                    height,
                    ..Default::default()
                })
            }
            _ => return Err(bad(cmd, "`zoom` must be \"current\", {center, width, height} or null")),
        };
    }
    Ok(())
}

const FIELD_KEYS: [&str; 7] = ["level", "kind", "layers", "annotation", "labels", "reference", "zoom"];

fn params_object<'a>(cmd: &str, p: &'a Value, extra: &[&str]) -> Result<&'a Map<String, Value>> {
    let mut allowed: Vec<&str> = FIELD_KEYS.to_vec();
    allowed.extend_from_slice(extra);
    object(cmd, "", p, &allowed)
}

fn required_name(cmd: &str, m: &Map<String, Value>, key: &str) -> Result<String> {
    string(cmd, key, m.get(key).ok_or_else(|| bad(cmd, format!("`{key}` is required")))?)
}

fn check(cmd: &str, v: &PlanView, d: &Drawing) -> Result<()> {
    validate_plan_view(v, d.arch.levels.len()).map_err(|m| bad(cmd, m))
}

/// The view as shown now: centre, the height of the zoom frame and the width its aspect gives.
fn current_zoom(s: &Session) -> Option<ViewZoom> {
    let st = s.state().ok()?;
    let f = st.zoom_frame();
    let (w, h) = s.viewport_px;
    let aspect = st.zoom_aspect(w / h.max(1.0)).max(1e-9);
    let z = ViewZoom { center: [f.center.x, f.center.y], width: f.height * aspect, height: f.height, ..Default::default() };
    (z.center.iter().all(|c| c.is_finite()) && z.width.is_finite() && z.height.is_finite() && z.width > 0.0 && z.height > 0.0).then_some(z)
}

fn same_name(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

fn tabs(d: &Drawing) -> Value {
    json!({ "open": d.arch.open_plan_views, "active": d.arch.active_plan_view })
}

// ---------------------------------------------------------------- commands

fn run_new(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "planview";
    let m = params_object(CMD, p, &["name", "from", "open"])?;
    let name = required_name(CMD, m, "name")?;
    let open = match m.get("open") {
        None => true,
        Some(b) => boolean(CMD, "open", b)?,
    };
    let current = if m.get("zoom").is_some_and(|z| z.as_str() == Some("current")) { current_zoom(s) } else { None };
    let d = s.doc()?;
    let base = match m.get("from") {
        Some(f) => {
            let f = string(CMD, "from", f)?;
            d.arch.plan_view(&f).cloned().ok_or_else(|| bad(CMD, format!("`from`: no plan view `{}`", f.trim())))?
        }
        None => d.arch.plan_view("Working").cloned().unwrap_or_default(),
    };
    let mut view = base;
    view.name = name.trim().to_string();
    parse_fields(CMD, m, &mut view, d, current)?;
    view.layers.name = view.name.clone();
    if d.arch.plan_views.iter().any(|v| same_name(&v.name, &view.name)) {
        return Err(bad(CMD, format!("`name`: plan view `{}` already exists", view.name)));
    }
    if d.arch.plan_views.len() >= MAX_PLAN_VIEWS {
        return Err(bad(CMD, format!("a drawing holds at most {MAX_PLAN_VIEWS} plan views")));
    }
    check(CMD, &view, d)?;
    {
        let d = s.doc_mut()?;
        d.arch.plan_views.push(view.clone());
        if open {
            apply_plan_view(d, &view);
        }
    }
    if open {
        apply_zoom(s, &view)?;
    }
    let mut out = tabs(s.doc()?);
    out["planView"] = json!(view.name);
    Ok(out)
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "planview.edit";
    let m = params_object(CMD, p, &["name", "rename"])?;
    let name = required_name(CMD, m, "name")?;
    let current = if m.get("zoom").is_some_and(|z| z.as_str() == Some("current")) { current_zoom(s) } else { None };
    let d = s.doc()?;
    let index = d
        .arch
        .plan_views
        .iter()
        .position(|v| same_name(&v.name, &name))
        .ok_or_else(|| bad(CMD, format!("`name`: no plan view `{}`", name.trim())))?;
    let Some(old) = d.arch.plan_views.get(index) else { return Err(bad(CMD, "`name`: no such plan view")) };
    let old_name = old.name.clone();
    let mut view = old.clone();
    if let Some(r) = m.get("rename") {
        let r = string(CMD, "rename", r)?;
        if d.arch.plan_views.iter().enumerate().any(|(i, v)| i != index && same_name(&v.name, &r)) {
            return Err(bad(CMD, format!("`rename`: plan view `{}` already exists", r.trim())));
        }
        view.name = r.trim().to_string();
    }
    parse_fields(CMD, m, &mut view, d, current)?;
    view.layers.name = view.name.clone();
    check(CMD, &view, d)?;
    let d = s.doc_mut()?;
    if let Some(slot) = d.arch.plan_views.get_mut(index) {
        *slot = view.clone();
    }
    for t in d.arch.open_plan_views.iter_mut() {
        if same_name(t, &old_name) {
            *t = view.name.clone();
        }
    }
    let was_active = d.arch.active_plan_view.as_deref().is_some_and(|a| same_name(a, &old_name));
    if was_active {
        d.arch.active_plan_view = Some(view.name.clone());
        apply_plan_view(d, &view);
    }
    let mut out = tabs(d);
    out["planView"] = json!(view.name);
    Ok(out)
}

/// After the tab `gone` left the open list: when it was active, the last remaining tab becomes
/// active and is applied; with none left no view is active and the layers stay.
fn reactivate(d: &mut Drawing, was_active: bool) {
    if !was_active {
        return;
    }
    d.arch.active_plan_view = None;
    let next = d.arch.open_plan_views.last().and_then(|n| d.arch.plan_view(n)).cloned();
    if let Some(v) = next {
        apply_plan_view(d, &v);
    }
}

fn run_delete(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "planview.delete";
    let m = object(CMD, "", p, &["name"])?;
    let name = required_name(CMD, m, "name")?;
    let d = s.doc()?;
    let index = d
        .arch
        .plan_views
        .iter()
        .position(|v| same_name(&v.name, &name))
        .ok_or_else(|| bad(CMD, format!("`name`: no plan view `{}`", name.trim())))?;
    let deleted = d.arch.plan_views.get(index).map(|v| v.name.clone()).unwrap_or_default();
    let d = s.doc_mut()?;
    d.arch.plan_views.remove(index);
    let was_active = d.arch.active_plan_view.as_deref().is_some_and(|a| same_name(a, &deleted));
    d.arch.open_plan_views.retain(|t| !same_name(t, &deleted));
    reactivate(d, was_active);
    let mut out = tabs(d);
    out["deleted"] = json!(deleted);
    Ok(out)
}

fn run_open(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "planview.open";
    let m = object(CMD, "", p, &["name"])?;
    let name = required_name(CMD, m, "name")?;
    open_plan_view(s, &name)
}

fn run_close(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "planview.close";
    let m = object(CMD, "", p, &["name"])?;
    let d = s.doc()?;
    let name = match m.get("name") {
        Some(n) => string(CMD, "name", n)?,
        None => d.arch.active_plan_view.clone().ok_or_else(|| bad(CMD, "`name` is required: no plan view is active"))?,
    };
    let closed = d
        .arch
        .open_plan_views
        .iter()
        .find(|t| same_name(t, &name))
        .cloned()
        .ok_or_else(|| bad(CMD, format!("`name`: plan view `{}` is not open", name.trim())))?;
    let d = s.doc_mut()?;
    let was_active = d.arch.active_plan_view.as_deref().is_some_and(|a| same_name(a, &closed));
    d.arch.open_plan_views.retain(|t| !same_name(t, &closed));
    reactivate(d, was_active);
    let mut out = tabs(d);
    out["closed"] = json!(closed);
    Ok(out)
}

fn run_list(s: &mut Session, p: &Value) -> Result<Value> {
    object("planview.list", "", p, &[])?;
    let d = s.doc()?;
    let views: Vec<Value> = d.arch.plan_views.iter().map(|v| json!({ "name": v.name, "level": v.level, "kind": v.kind.key() })).collect();
    let mut out = tabs(d);
    out["planViews"] = Value::Array(views);
    Ok(out)
}

#[cfg(test)]
mod tests;
