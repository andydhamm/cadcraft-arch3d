//! Sheets (phase 21): `sheet`, `sheet.edit`, `sheet.delete`, `sheet.list`, `sheet.open` and
//! `sheetset`.
//!
//! A sheet is a CADCraft layout with one locked viewport per view (a plan view, an elevation,
//! section or interior elevation, or a schedule) at a paper scale, a title under each viewport and
//! a title block (`title_block`). The data (`Sheet`, `SheetView`, `SheetSet`) is
//! `cadcraft_doc::arch::sheet`'s; viewports are drawn by the renderer, plotting is the plot
//! command's. Lengths are in metres, paper in millimetres, plan axes x right, y up.
//!
//! Every command validates everything before its first change and puts the drawing back when a
//! later step fails, so a refused command changes nothing and pushes no undo step.

use crate::Session;
use crate::cmd::{CommandSpec, bad};
use crate::error::Result;
use cadcraft_doc::arch::planview::PlanKind;
use cadcraft_doc::arch::sheet::{
    LAYER_TITLE, LINK_SCHEDULE, LINK_SHEET_IMAGE, LINK_SHEET_VIEW, LINK_VIEW_TITLE, MAX_SHEET_TEXT, MAX_SHEET_VIEWS, MAX_SHEETS, Sheet, SheetSource,
    SheetView, link_tags, read_link, schedule_label, set_link, validate_sheet,
};
use cadcraft_doc::arch::{ArchId, view_label};
use cadcraft_doc::{Common, Drawing, EntityKind, HAlign, Space, Text, VAlign};
use cadcraft_geom::{Bounds2, Vec2, Vec3};
use serde_json::{Map as JsonMap, Value, json};

/// Margin of the drawing area on the paper, mm.
const MARGIN_MM: f64 = 25.0;
/// Gap between auto-placed viewports, mm.
const GAP_MM: f64 = 20.0;
/// Width left free at the right of a landscape sheet for the title block, mm.
const TITLE_STRIP_MM: f64 = 110.0;
/// Height of a view title, mm.
const TITLE_HEIGHT_MM: f64 = 3.5;
/// Distance from a viewport's bottom edge to its title's baseline, mm.
const TITLE_DROP_MM: f64 = 8.0;
/// Growth of a plan view's frame (auto dimensions sit 2 m out), m.
const PLAN_MARGIN_M: f64 = 2.5;
/// Growth of an elevation, section or interior view's frame, m.
const VIEW_MARGIN_M: f64 = 1.5;
/// Growth of a schedule table's frame, m.
const SCHEDULE_MARGIN_M: f64 = 0.2;
/// Default width of a camera image on the sheet, mm.
const CAMERA_WIDTH_MM: f64 = 250.0;
/// Default render size of a camera image, pixels.
const CAMERA_PIXELS: [u32; 2] = [960, 640];
/// Largest absolute paper coordinate accepted for `at`, mm.
const MAX_AT_MM: f64 = 1e5;

/// Command specs for the sheet commands.
pub fn specs() -> Vec<CommandSpec> {
    macro_rules! view {
        () => {
            "{view: plan view | \"v<n>\" | view name | \"sc<n>\" | schedule name, level?: name|index, scale: \"1:50\"|50, at?: [x, y]} | {camera: saved camera, width?: mm, pixels?: [w, h], at?: [x, y]}"
        };
    }
    vec![
        CommandSpec::new("sheet", "Sheet", run_sheet).menu(&["Architecture", "Sheet"]).params(concat!(
            "{name, number?, title?, size?: paper (default \"ARCH D\"), landscape?: bool, views: [",
            view!(),
            "]}"
        )),
        CommandSpec::new("sheet.edit", "Edit Sheet", run_edit).menu(&["Architecture", "Edit Sheet"]).params(concat!(
            "{name, rename?, number?, title?, add?: [",
            view!(),
            "], remove?: [view number], order?: index}"
        )),
        CommandSpec::new("sheet.delete", "Delete Sheet", run_delete).menu(&["Architecture", "Delete Sheet"]).params("{name}"),
        CommandSpec::new("sheet.list", "List Sheets", run_list).menu(&["Architecture", "List Sheets"]).noundo().params("{}"),
        CommandSpec::new("sheet.open", "Open Sheet", run_open)
            .menu(&["Architecture", "Open Sheet"])
            .noundo()
            .params("{name} | {view: \"v<n>\" | view name}"),
        CommandSpec::new("sheetset", "Sheet Set", run_sheetset).menu(&["Architecture", "Sheet Set"]).params("{project?: string, date?: string}"),
    ]
}

// ---------- parameter helpers ----------

/// The value of `key` unless it is absent or null.
fn present<'a>(p: &'a Value, key: &str) -> Option<&'a Value> {
    match p.get(key) {
        None | Some(Value::Null) => None,
        Some(v) => Some(v),
    }
}

/// A string parameter that may be absent.
fn opt_str(cmd: &str, p: &Value, key: &str) -> Result<Option<String>> {
    match present(p, key) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(bad(cmd, format!("`{key}` must be a string"))),
    }
}

/// A required string parameter.
fn req_str(cmd: &str, p: &Value, key: &str) -> Result<String> {
    opt_str(cmd, p, key)?.ok_or_else(|| bad(cmd, format!("`{key}` (string) is required")))
}

/// A boolean parameter that may be absent.
fn opt_bool(cmd: &str, p: &Value, key: &str) -> Result<Option<bool>> {
    match present(p, key) {
        None => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(bad(cmd, format!("`{key}` must be true or false"))),
    }
}

/// A list parameter that may be absent.
fn opt_list<'a>(cmd: &str, p: &'a Value, key: &str) -> Result<Option<&'a Vec<Value>>> {
    match present(p, key) {
        None => Ok(None),
        Some(Value::Array(a)) => Ok(Some(a)),
        Some(_) => Err(bad(cmd, format!("`{key}` must be a list"))),
    }
}

/// A non-negative whole number.
fn whole(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_f64().filter(|f| f.is_finite() && *f >= 0.0 && f.fract() == 0.0 && *f <= 4.0e9).map(|f| f as u64))
}

/// A paper point `[x, y]` in millimetres.
fn point_mm(cmd: &str, v: &Value, what: &str) -> Result<[f64; 2]> {
    let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| bad(cmd, format!("{what} must be [x, y]")))?;
    let mut out = [0.0; 2];
    for (o, n) in out.iter_mut().zip(a) {
        *o = n
            .as_f64()
            .filter(|f| f.is_finite() && f.abs() <= MAX_AT_MM)
            .ok_or_else(|| bad(cmd, format!("{what} must be two finite numbers within 1e5")))?;
    }
    Ok(out)
}

/// A scale: "1:50" or 50, between 1 and 10000.
fn parse_scale(v: &Value) -> Option<f64> {
    let n = match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let t = s.trim();
            let t = t.strip_prefix("1:").unwrap_or(t);
            t.trim().parse::<f64>().ok()
        }
        _ => None,
    };
    n.filter(|n| n.is_finite() && (1.0..=10000.0).contains(n))
}

/// "1:50" for a scale denominator.
fn scale_text(scale: f64) -> String {
    format!("1:{scale}")
}

/// Run `f`; when it fails, put the drawing back as it was before.
fn guarded<T>(s: &mut Session, f: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
    let before = s.state()?.doc.clone();
    let r = f(s);
    if r.is_err()
        && let Ok(st) = s.state_mut()
    {
        st.doc = before;
    }
    r
}

/// Paper size in millimetres for a paper name and orientation: (canonical name, width, height).
fn paper_dims(cmd: &str, size: &str, landscape: bool) -> Result<(String, f64, f64)> {
    let ps = cadcraft_render::paper_size(size).ok_or_else(|| bad(cmd, format!("unknown paper size `{size}`")))?;
    let (lo, hi) = (ps.width_mm.min(ps.height_mm), ps.width_mm.max(ps.height_mm));
    let (w, h) = if landscape { (hi, lo) } else { (lo, hi) };
    Ok((ps.name.to_string(), w, h))
}

/// Millimetres per paper unit of the drawing.
fn paper_unit(d: &Drawing) -> f64 {
    let u = cadcraft_render::paper::paper_unit_mm(d);
    if u.is_finite() && u > 0.0 { u } else { 1.0 }
}

// ---------- views ----------

/// A view asked for, resolved against the drawing.
#[derive(Clone, Debug)]
struct NewView {
    source: SheetSource,
    /// The text of the view's title (without number and scale).
    title: String,
    /// Scale denominator; 0 for a camera image.
    scale: f64,
    at: Option<[f64; 2]>,
}

/// A level given as a name or an index.
fn parse_level(d: &Drawing, i: usize, v: &Value) -> Result<u32> {
    let levels = &d.arch.levels;
    let found = match v {
        Value::String(n) => {
            let n = n.trim();
            levels.iter().position(|l| l.name.trim().eq_ignore_ascii_case(n)).or_else(|| n.parse::<usize>().ok().filter(|k| *k < levels.len()))
        }
        other => whole(other).and_then(|k| usize::try_from(k).ok()).filter(|k| *k < levels.len()),
    };
    found.and_then(|k| u32::try_from(k).ok()).ok_or_else(|| bad("sheet", format!("views[{i}].level: unknown level {v}")))
}

/// What a `view` string names.
enum Named {
    Plan(String, Option<u32>),
    View(ArchId),
    Schedule(ArchId),
}

/// Resolve a view string to a plan view, an ArchView or a schedule; two kinds is ambiguous.
fn resolve_named(d: &Drawing, i: usize, name: &str) -> Result<Named> {
    let n = name.trim();
    let mut found: Vec<(&str, Named)> = Vec::new();
    if let Some(pv) = d.arch.plan_view(n) {
        found.push(("plan view", Named::Plan(pv.name.clone(), pv.level)));
    }
    if let Some(v) = d.arch.view_by_ref(n) {
        found.push(("view", Named::View(v.id)));
    } else if d.arch.views.iter().filter(|v| v.name.trim().eq_ignore_ascii_case(n)).count() > 1 {
        return Err(bad("sheet", format!("views[{i}].view: `{n}` names more than one view")));
    }
    if let Some(sc) = d.arch.schedule_by_ref(n) {
        found.push(("schedule", Named::Schedule(sc.id)));
    }
    if found.len() > 1 {
        let kinds: Vec<&str> = found.iter().map(|(k, _)| *k).collect();
        return Err(bad("sheet", format!("views[{i}].view: `{n}` is ambiguous: it names a {}", kinds.join(" and a "))));
    }
    found.pop().map(|(_, v)| v).ok_or_else(|| bad("sheet", format!("views[{i}].view: unknown view `{n}`")))
}

/// Parse and resolve one entry of `views` / `add`.
fn parse_view(cmd: &str, d: &Drawing, i: usize, v: &Value) -> Result<NewView> {
    if !v.is_object() {
        return Err(bad(cmd, format!("views[{i}] must be an object")));
    }
    let at = match present(v, "at") {
        None => None,
        Some(a) => Some(point_mm(cmd, a, &format!("views[{i}].at"))?),
    };
    if present(v, "camera").is_some() {
        if present(v, "view").is_some() || present(v, "scale").is_some() || present(v, "level").is_some() {
            return Err(bad(cmd, format!("views[{i}]: a camera view takes no `view`, `scale` or `level`")));
        }
        let name = req_str(cmd, v, "camera").map_err(|_| bad(cmd, format!("views[{i}].camera must be a camera name")))?;
        let cam = d.arch.camera_by_name(&name).ok_or_else(|| bad(cmd, format!("views[{i}].camera: unknown camera `{name}`")))?;
        let width = match present(v, "width") {
            None => CAMERA_WIDTH_MM,
            Some(w) => w
                .as_f64()
                .filter(|w| w.is_finite() && *w > 0.0 && *w <= 2000.0)
                .ok_or_else(|| bad(cmd, format!("views[{i}].width must be in (0, 2000] mm")))?,
        };
        let pixels = match present(v, "pixels") {
            None => CAMERA_PIXELS,
            Some(px) => {
                let a = px.as_array().filter(|a| a.len() == 2).ok_or_else(|| bad(cmd, format!("views[{i}].pixels must be [width, height]")))?;
                let mut out = [0u32; 2];
                for (o, n) in out.iter_mut().zip(a) {
                    *o = whole(n)
                        .filter(|k| (16..=4096).contains(k))
                        .and_then(|k| u32::try_from(k).ok())
                        .ok_or_else(|| bad(cmd, format!("views[{i}].pixels must be whole numbers within 16..=4096")))?;
                }
                out
            }
        };
        return Ok(NewView { title: cam.name.clone(), source: SheetSource::Camera { name: cam.name.clone(), width, pixels }, scale: 0.0, at });
    }
    let name = req_str(cmd, v, "view").map_err(|_| bad(cmd, format!("views[{i}] needs a `view` name or a `camera`")))?;
    let scale = present(v, "scale")
        .ok_or_else(|| bad(cmd, format!("views[{i}].scale is required")))
        .and_then(|sv| parse_scale(sv).ok_or_else(|| bad(cmd, format!("views[{i}].scale must be \"1:n\" or a number n within 1..=10000"))))?;
    let level_param = present(v, "level");
    let named = resolve_named(d, i, &name)?;
    let (source, title) = match named {
        Named::Plan(pv_name, pv_level) => {
            let level = match level_param {
                Some(l) => Some(parse_level(d, i, l)?),
                None => pv_level.or(d.arch.plan_level).filter(|l| (*l as usize) < d.arch.levels.len()),
            };
            (SheetSource::Plan { view: pv_name.clone(), level }, pv_name)
        }
        Named::View(id) => {
            if level_param.is_some() {
                return Err(bad(cmd, format!("views[{i}].level applies to plan views only")));
            }
            let title = d.arch.view(id).map(|x| x.name.clone()).unwrap_or_default();
            (SheetSource::View { id }, if title.trim().is_empty() { view_label(id) } else { title })
        }
        Named::Schedule(id) => {
            if level_param.is_some() {
                return Err(bad(cmd, format!("views[{i}].level applies to plan views only")));
            }
            let title = d.arch.schedule(id).map(|x| x.name.clone()).unwrap_or_default();
            (SheetSource::Schedule { id }, if title.trim().is_empty() { schedule_label(id) } else { title })
        }
    };
    Ok(NewView { source, title, scale, at })
}

/// Parse a list of views (`views` or `add`).
fn parse_views(cmd: &str, d: &Drawing, list: &[Value]) -> Result<Vec<NewView>> {
    list.iter().enumerate().map(|(i, v)| parse_view(cmd, d, i, v)).collect()
}

/// The four corners of a wall's footprint.
fn wall_corners(w: &cadcraft_doc::Wall) -> Option<[Vec2; 4]> {
    if !w.start.is_finite() || !w.end.is_finite() || !(w.thickness.is_finite() && w.thickness > 0.0) {
        return None;
    }
    let len = w.start.dist(w.end);
    if !(len.is_finite() && len > 1e-12) {
        return None;
    }
    let perp = (w.end - w.start).normalized().perp() * (w.thickness / 2.0);
    Some([w.start - perp, w.end - perp, w.end + perp, w.start + perp])
}

/// The model-space frame of a plan view's viewport: the walls and rooms of its level, the roofs
/// of a roof plan and the terrain boundary when the view's layer rules turn `C-*` on, grown.
fn plan_frame(d: &Drawing, view: &str, level: Option<u32>) -> Bounds2 {
    let mut b = Bounds2::EMPTY;
    for w in d.arch.walls.iter().filter(|w| level.is_none_or(|l| w.base_level == l)) {
        if let Some(c) = wall_corners(w) {
            for p in c {
                b.add(p);
            }
        }
    }
    for r in d.arch.rooms.iter().filter(|r| level.is_none_or(|l| r.level == l)) {
        for p in &r.polygon {
            b.add(*p);
        }
    }
    let pv = d.arch.plan_view(view);
    if pv.is_some_and(|p| p.kind == PlanKind::Roof) {
        for roof in &d.arch.roofs {
            for plane in &roof.planes {
                for p in &plane.ring {
                    b.add(*p);
                }
            }
        }
    }
    let site_on = pv.is_some_and(|p| p.layers.rules.iter().any(|r| r.on == Some(true) && r.pattern.trim().to_ascii_lowercase().starts_with("c-")));
    if site_on && let Some(t) = &d.arch.terrain {
        for p in &t.boundary {
            b.add(*p);
        }
    }
    b.expand(PLAN_MARGIN_M)
}

/// The extents of a schedule's TABLE in model space.
fn schedule_extents(d: &Drawing, id: ArchId) -> Bounds2 {
    let label = schedule_label(id);
    let mut b = Bounds2::EMPTY;
    for e in d.model.iter() {
        if matches!(e.kind, EntityKind::Table(_))
            && let Some((role, key, _)) = read_link(&e.common.xdata)
            && role == LINK_SCHEDULE
            && key == label
        {
            b = b.union(&cadcraft_doc::entity_bounds(d, e, 0));
        }
    }
    b
}

/// The model-space frame a view's viewport shows; empty when there is nothing to show.
fn frame_of(d: &Drawing, source: &SheetSource) -> Bounds2 {
    let b = match source {
        SheetSource::Plan { view, level } => plan_frame(d, view, *level),
        SheetSource::View { id } => match d.arch.view(*id).and_then(|v| v.bounds()) {
            Some([x0, y0, x1, y1]) => Bounds2::from_points([Vec2::new(x0, y0), Vec2::new(x1, y1)]).expand(VIEW_MARGIN_M),
            None => Bounds2::EMPTY,
        },
        SheetSource::Schedule { id } => schedule_extents(d, *id).expand(SCHEDULE_MARGIN_M),
        SheetSource::Camera { .. } => Bounds2::EMPTY,
    };
    if b.is_empty() || !b.min.is_finite() || !b.max.is_finite() { Bounds2::EMPTY } else { b }
}

/// A view with its place on the paper.
struct Placed {
    number: u32,
    nv: NewView,
    /// The viewport rectangle on the paper, mm.
    rect: Bounds2,
    /// Paper mm per metre.
    k_mm: f64,
    /// The model point at the viewport's centre.
    centre: Vec2,
}

impl Placed {
    fn record(&self) -> SheetView {
        let c = self.rect.center();
        SheetView {
            number: self.number,
            source: self.nv.source.clone(),
            scale: self.nv.scale,
            at: [c.x, c.y],
            size: [self.rect.width(), self.rect.height()],
        }
    }
}

/// Size every view, place the ones without `at` left to right from the top-left of the drawing
/// area, and check that every viewport lies inside the paper. `existing` are the views already on
/// the sheet; auto-placement starts below them. Numbers run from `first`.
fn place_views(d: &Drawing, views: Vec<NewView>, existing: &[SheetView], first: u32, paper: (f64, f64), landscape: bool) -> Result<Vec<Placed>> {
    let (pw, ph) = paper;
    // Sizes first: a source with nothing to show is refused before anything is placed.
    let mut sized: Vec<(NewView, f64, Vec2, [f64; 2])> = Vec::with_capacity(views.len());
    for (i, nv) in views.into_iter().enumerate() {
        let (k_mm, centre, size) = match &nv.source {
            SheetSource::Camera { width, pixels, .. } => {
                let aspect = f64::from(pixels[1]) / f64::from(pixels[0].max(1));
                (0.0, Vec2::ZERO, [*width, width * aspect])
            }
            other => {
                let frame = frame_of(d, other);
                if frame.is_empty() {
                    return Err(bad("sheet", format!("views[{i}]: nothing to show for `{}`", nv.title)));
                }
                let k = 1000.0 / nv.scale;
                (k, frame.center(), [frame.width() * k, frame.height() * k])
            }
        };
        if !size.iter().all(|x| x.is_finite() && *x > 0.0) {
            return Err(bad("sheet", format!("views[{i}]: nothing to show for `{}`", nv.title)));
        }
        sized.push((nv, k_mm, centre, size));
    }
    let right = if landscape { pw - TITLE_STRIP_MM } else { pw - MARGIN_MM };
    let mut x = MARGIN_MM;
    let mut y_top = existing.iter().map(|v| v.at[1] - v.size[1] / 2.0 - GAP_MM).fold(ph - MARGIN_MM, f64::min);
    let mut row_h = 0.0_f64;
    let mut out = Vec::with_capacity(sized.len());
    for (i, (nv, k_mm, centre, size)) in sized.into_iter().enumerate() {
        let number = first.saturating_add(u32::try_from(i).unwrap_or(u32::MAX));
        let rect = match nv.at {
            Some([ax, ay]) => Bounds2::new(Vec2::new(ax - size[0] / 2.0, ay - size[1] / 2.0), Vec2::new(ax + size[0] / 2.0, ay + size[1] / 2.0)),
            None => {
                if x > MARGIN_MM && x + size[0] > right {
                    y_top -= row_h + GAP_MM;
                    x = MARGIN_MM;
                    row_h = 0.0;
                }
                let r = Bounds2::new(Vec2::new(x, y_top - size[1]), Vec2::new(x + size[0], y_top));
                x += size[0] + GAP_MM;
                row_h = row_h.max(size[1]);
                r
            }
        };
        let eps = 1e-9;
        if rect.min.x < -eps || rect.min.y < -eps || rect.max.x > pw + eps || rect.max.y > ph + eps {
            return Err(bad("sheet", format!("view {number} does not fit the sheet")));
        }
        out.push(Placed { number, nv, rect, k_mm, centre });
    }
    Ok(out)
}

/// Make the viewports, titles and images of `placed` on a layout; returns the records.
fn apply_views(s: &mut Session, layout: &str, sheet: &str, placed: &[Placed]) -> Result<Vec<SheetView>> {
    let unit = paper_unit(s.doc()?);
    let mut out = Vec::with_capacity(placed.len());
    for p in placed {
        let sv = p.record();
        if matches!(p.nv.source, SheetSource::Camera { .. }) {
            super::sheet_camera::place_camera_image(s, sheet, &sv)?;
        } else {
            let rect = Bounds2::new(p.rect.min * (1.0 / unit), p.rect.max * (1.0 / unit));
            let h = crate::cmd::layout::add_scaled_viewport(s, layout, rect, p.centre, p.k_mm / unit)?;
            let d = s.doc_mut()?;
            d.modify_entity(h, |e| set_link(&mut e.common.xdata, LINK_SHEET_VIEW, sheet, p.number))?;
        }
        let mut text = format!("{}  {}", p.number, p.nv.title.to_uppercase());
        if p.nv.scale > 0.0 {
            text.push_str("  ");
            text.push_str(&scale_text(p.nv.scale));
        }
        let at = Vec3::new(p.rect.min.x / unit, (p.rect.min.y - TITLE_DROP_MM) / unit, 0.0);
        let common = Common { layer: LAYER_TITLE.into(), xdata: link_tags(LINK_VIEW_TITLE, sheet, p.number), ..Common::default() };
        let kind = EntityKind::Text(Text {
            insert: at,
            align_pt: None,
            height: TITLE_HEIGHT_MM / unit,
            value: text,
            rotation: 0.0,
            width_factor: 1.0,
            oblique: 0.0,
            style: "Standard".into(),
            halign: HAlign::Left,
            valign: VAlign::Baseline,
        });
        s.doc_mut()?.add(&Space::Paper(layout.to_string()), common, kind)?;
        out.push(sv);
    }
    Ok(out)
}

// ---------- sheet numbers and records ----------

fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Today's date as "YYYY-MM-DD"; empty on wasm.
fn today() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let days = i64::try_from(secs / 86_400).unwrap_or(0);
        // Civil date from days since 1970-01-01 (proleptic Gregorian).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        format!("{year:04}-{month:02}-{day:02}")
    }
    #[cfg(target_arch = "wasm32")]
    {
        String::new()
    }
}

/// The reply row of a sheet: the `arch.inspect` row plus `"sheet"`.
fn sheet_row(d: &Drawing, name: &str) -> Value {
    let mut out = JsonMap::new();
    super::sheet_inspect::inspect_sheets(d, &json!({"sheets": true}), &mut out);
    let row =
        out.get("sheets").and_then(Value::as_array).and_then(|a| a.iter().find(|r| r.get("name").and_then(Value::as_str) == Some(name))).cloned();
    let mut row = row.unwrap_or_else(|| json!({}));
    if let Some(m) = row.as_object_mut() {
        m.insert("sheet".into(), json!(name));
    }
    row
}

fn check_title(cmd: &str, t: &str) -> Result<()> {
    if t.chars().count() > MAX_SHEET_TEXT || t.chars().any(char::is_control) {
        return Err(bad(cmd, format!("`title` must be at most {MAX_SHEET_TEXT} characters without control characters")));
    }
    Ok(())
}

/// The sheet number to use: the one asked for (unique) or the smallest unused positive integer.
fn pick_number(cmd: &str, d: &Drawing, asked: Option<String>, except: Option<usize>) -> Result<String> {
    let used = |n: &str| d.arch.sheets.iter().enumerate().any(|(i, sh)| Some(i) != except && same(&sh.number, n));
    match asked {
        Some(n) => {
            let n = n.trim().to_string();
            if used(&n) {
                return Err(bad(cmd, format!("sheet number `{n}` is already used")));
            }
            Ok(n)
        }
        None => (1u32..=u32::try_from(MAX_SHEETS + 2).unwrap_or(300))
            .map(|k| k.to_string())
            .find(|n| !used(n))
            .ok_or_else(|| bad(cmd, "no free sheet number")),
    }
}

// ---------- sheet ----------

fn run_sheet(s: &mut Session, p: &Value) -> Result<Value> {
    guarded(s, |s| make_sheet(s, p))
}

fn make_sheet(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "sheet";
    // Regenerated views are part of the drawing; `guarded` undoes this on a refusal.
    super::views::refresh_views(s);
    let d = s.doc()?;
    if d.arch.sheets.len() >= MAX_SHEETS {
        return Err(bad(CMD, format!("a drawing holds at most {MAX_SHEETS} sheets")));
    }
    let name = crate::cmd::layout::check_name(CMD, d, &req_str(CMD, p, "name")?, None)?;
    if d.arch.sheet(&name).is_some() {
        return Err(bad(CMD, format!("sheet `{name}` already exists")));
    }
    let number = pick_number(CMD, d, opt_str(CMD, p, "number")?, None)?;
    let title = opt_str(CMD, p, "title")?.unwrap_or_else(|| name.clone());
    check_title(CMD, &title)?;
    let size = opt_str(CMD, p, "size")?.unwrap_or_else(|| "ARCH D".to_string());
    let landscape = opt_bool(CMD, p, "landscape")?.unwrap_or(true);
    let (paper_name, pw, ph) = paper_dims(CMD, &size, landscape)?;
    let list = opt_list(CMD, p, "views")?.ok_or_else(|| bad(CMD, "`views` (list) is required"))?;
    if list.is_empty() || list.len() > MAX_SHEET_VIEWS {
        return Err(bad(CMD, format!("`views` must hold 1..={MAX_SHEET_VIEWS} views")));
    }
    let new_views = parse_views(CMD, d, list)?;
    let placed = place_views(d, new_views, &[], 1, (pw, ph), landscape)?;
    let mut sheet =
        Sheet { name: name.clone(), number, title, size: paper_name.clone(), landscape, views: placed.iter().map(Placed::record).collect() };
    validate_sheet(&sheet, d.arch.levels.len()).map_err(|e| bad(CMD, e))?;
    // Validated: make it.
    crate::cmd::layout::new_bare_layout(s, &name, &paper_name, landscape)?;
    sheet.views = apply_views(s, &name, &name, &placed)?;
    let d = s.doc_mut()?;
    d.arch.sheets.push(sheet);
    if d.arch.sheet_set.date.is_empty() {
        d.arch.sheet_set.date = today();
    }
    super::title_block::refresh_title_blocks(s)?;
    Ok(sheet_row(s.doc()?, &name))
}

// ---------- sheet.edit ----------

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    guarded(s, |s| edit_sheet(s, p))
}

/// The entities of a layout that belong to sheet `key`: (handle, role, number).
fn linked(d: &Drawing, layout: &str, key: &str) -> Vec<(cadcraft_doc::Handle, String, u32)> {
    let Some(l) = d.layout(layout) else { return Vec::new() };
    let mut v: Vec<_> =
        l.entities.iter().filter_map(|e| read_link(&e.common.xdata).filter(|(_, k, _)| same(k, key)).map(|(r, _, n)| (e.handle, r, n))).collect();
    v.sort_by_key(|x| x.0.0);
    v
}

fn edit_sheet(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "sheet.edit";
    super::views::refresh_views(s);
    let d = s.doc()?;
    let asked = req_str(CMD, p, "name")?;
    let idx = d.arch.sheets.iter().position(|sh| same(&sh.name, &asked)).ok_or_else(|| bad(CMD, format!("unknown sheet `{asked}`")))?;
    let old = d.arch.sheets.get(idx).cloned().ok_or_else(|| bad(CMD, "unknown sheet"))?;
    if !["rename", "number", "title", "add", "remove", "order"].iter().any(|k| present(p, k).is_some()) {
        return Err(bad(CMD, "give at least one of rename, number, title, add, remove, order"));
    }
    let layout = d.layout(&old.name).map(|l| l.name.clone()).ok_or_else(|| bad(CMD, format!("the layout of sheet `{}` is missing", old.name)))?;
    let rename = match opt_str(CMD, p, "rename")? {
        Some(r) => {
            let r = crate::cmd::layout::check_name(CMD, d, &r, Some(&layout))?;
            if d.arch.sheets.iter().enumerate().any(|(i, sh)| i != idx && same(&sh.name, &r)) {
                return Err(bad(CMD, format!("sheet `{r}` already exists")));
            }
            Some(r)
        }
        None => None,
    };
    let number = match opt_str(CMD, p, "number")? {
        Some(n) => Some(pick_number(CMD, d, Some(n), Some(idx))?),
        None => None,
    };
    let title = opt_str(CMD, p, "title")?;
    if let Some(t) = &title {
        check_title(CMD, t)?;
    }
    let mut remove: Vec<u32> = Vec::new();
    if let Some(list) = opt_list(CMD, p, "remove")? {
        for v in list {
            let n = whole(v).and_then(|n| u32::try_from(n).ok()).ok_or_else(|| bad(CMD, "`remove` must list view numbers"))?;
            if !old.views.iter().any(|x| x.number == n) {
                return Err(bad(CMD, format!("sheet `{}` has no view {n}", old.name)));
            }
            if remove.contains(&n) {
                return Err(bad(CMD, format!("view {n} is listed twice in `remove`")));
            }
            remove.push(n);
        }
    }
    let order = match present(p, "order") {
        None => None,
        Some(v) => {
            let o = whole(v).and_then(|o| usize::try_from(o).ok()).filter(|o| *o < d.arch.sheets.len());
            Some(o.ok_or_else(|| bad(CMD, format!("`order` must be a whole number below {}", d.arch.sheets.len())))?)
        }
    };
    let remaining: Vec<SheetView> = old.views.iter().filter(|v| !remove.contains(&v.number)).cloned().collect();
    let added = match opt_list(CMD, p, "add")? {
        Some(list) => parse_views(CMD, d, list)?,
        None => Vec::new(),
    };
    if remaining.len() + added.len() > MAX_SHEET_VIEWS {
        return Err(bad(CMD, format!("a sheet holds at most {MAX_SHEET_VIEWS} views")));
    }
    let first = remaining.iter().map(|v| v.number).max().unwrap_or(0).saturating_add(1);
    let (_, pw, ph) = paper_dims(CMD, &old.size, old.landscape)?;
    let placed = place_views(d, added, &remaining, first, (pw, ph), old.landscape)?;
    let mut new = Sheet {
        name: rename.clone().unwrap_or_else(|| old.name.clone()),
        number: number.unwrap_or_else(|| old.number.clone()),
        title: title.unwrap_or_else(|| old.title.clone()),
        size: old.size.clone(),
        landscape: old.landscape,
        views: remaining,
    };
    new.views.extend(placed.iter().map(Placed::record));
    validate_sheet(&new, d.arch.levels.len()).map_err(|e| bad(CMD, e))?;
    // Validated: change the drawing.
    if !remove.is_empty() {
        let gone: Vec<_> = linked(s.doc()?, &layout, &old.name)
            .into_iter()
            .filter(|(_, r, n)| remove.contains(n) && [LINK_SHEET_VIEW, LINK_VIEW_TITLE, LINK_SHEET_IMAGE].contains(&r.as_str()))
            .collect();
        let d = s.doc_mut()?;
        for (h, _, _) in gone {
            d.remove_entity(h);
        }
    }
    let mut layout = layout;
    if let Some(to) = &rename {
        crate::cmd::layout::run_rename(s, &json!({"from": layout, "to": to}))?;
        layout = to.clone();
        let links = linked(s.doc()?, &layout, &old.name);
        let d = s.doc_mut()?;
        for (h, role, n) in links {
            d.modify_entity(h, |e| set_link(&mut e.common.xdata, &role, to, n))?;
        }
    }
    if !placed.is_empty() {
        let recs = apply_views(s, &layout, &new.name, &placed)?;
        let keep = new.views.len().saturating_sub(recs.len());
        new.views.truncate(keep);
        new.views.extend(recs);
    }
    let d = s.doc_mut()?;
    if let Some(slot) = d.arch.sheets.get_mut(idx) {
        *slot = new.clone();
    }
    if let Some(o) = order {
        let rec = d.arch.sheets.remove(idx);
        d.arch.sheets.insert(o.min(d.arch.sheets.len()), rec);
    }
    super::title_block::refresh_title_blocks(s)?;
    Ok(sheet_row(s.doc()?, &new.name))
}

// ---------- sheet.delete, sheet.list, sheet.open ----------

fn run_delete(s: &mut Session, p: &Value) -> Result<Value> {
    guarded(s, |s| {
        const CMD: &str = "sheet.delete";
        let asked = req_str(CMD, p, "name")?;
        let d = s.doc()?;
        let sheet = d.arch.sheet(&asked).ok_or_else(|| bad(CMD, format!("unknown sheet `{asked}`")))?;
        let name = sheet.name.clone();
        if let Some(l) = d.layout(&name) {
            let layout = l.name.clone();
            crate::cmd::layout::run_delete(s, &json!({"name": layout}))?;
        }
        let d = s.doc_mut()?;
        d.arch.sheets.retain(|sh| !same(&sh.name, &name));
        Ok(json!({"deleted": name}))
    })
}

fn run_list(s: &mut Session, _p: &Value) -> Result<Value> {
    let d = s.doc()?;
    let sheets: Vec<Value> =
        d.arch.sheets.iter().map(|sh| json!({"name": sh.name, "number": sh.number, "title": sh.title, "views": sh.views.len()})).collect();
    Ok(json!({"sheets": sheets}))
}

fn run_open(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "sheet.open";
    let name = opt_str(CMD, p, "name")?;
    let view = opt_str(CMD, p, "view")?;
    let d = s.doc()?;
    let sheet = match (name, view) {
        (Some(n), None) => d.arch.sheet(&n).ok_or_else(|| bad(CMD, format!("unknown sheet `{n}`")))?,
        (None, Some(v)) => {
            let id = d.arch.view_by_ref(&v).map(|x| x.id);
            let plan = d.arch.plan_view(&v).map(|x| x.name.clone());
            if id.is_none() && plan.is_none() {
                return Err(bad(CMD, format!("unknown view `{v}`")));
            }
            d.arch
                .sheets
                .iter()
                .find(|sh| {
                    sh.views.iter().any(|sv| match &sv.source {
                        SheetSource::View { id: i } => Some(*i) == id,
                        SheetSource::Plan { view, .. } => plan.as_ref().is_some_and(|p| same(p, view)),
                        _ => false,
                    })
                })
                .ok_or_else(|| bad(CMD, format!("view `{v}` is on no sheet")))?
        }
        _ => return Err(bad(CMD, "give exactly one of `name` or `view`")),
    };
    let (sheet_name, layout) = (sheet.name.clone(), d.layout(&sheet.name).map(|l| l.name.clone()));
    let layout = layout.ok_or_else(|| bad(CMD, format!("the layout of sheet `{sheet_name}` is missing")))?;
    crate::cmd::view::run_layout_set(s, &json!({"name": layout}))?;
    Ok(json!({"sheet": sheet_name, "layout": layout}))
}

// ---------- sheetset ----------

fn run_sheetset(s: &mut Session, p: &Value) -> Result<Value> {
    guarded(s, |s| {
        const CMD: &str = "sheetset";
        let text = |key: &str, max: usize| -> Result<Option<String>> {
            match opt_str(CMD, p, key)? {
                Some(t) if t.chars().count() > max || t.chars().any(char::is_control) => {
                    Err(bad(CMD, format!("`{key}` must be at most {max} characters without control characters")))
                }
                other => Ok(other),
            }
        };
        let project = text("project", 128)?;
        let date = text("date", 32)?;
        if project.is_none() && date.is_none() {
            return Err(bad(CMD, "give `project` or `date`"));
        }
        let d = s.doc_mut()?;
        if let Some(pr) = project {
            d.arch.sheet_set.project = pr;
        }
        if let Some(dt) = date {
            d.arch.sheet_set.date = dt;
        }
        let reply = json!({"project": d.arch.sheet_set.project, "date": d.arch.sheet_set.date});
        super::title_block::refresh_title_blocks(s)?;
        Ok(reply)
    })
}

/// Drop the sheets whose layout no longer exists (a `layout.delete`). Read-only unless one goes.
pub(crate) fn prune_sheets(s: &mut Session) {
    let Ok(d) = s.doc() else { return };
    if d.arch.sheets.iter().all(|sh| d.layout(&sh.name).is_some()) {
        return;
    }
    let names: Vec<String> = d.layouts.iter().map(|l| l.name.clone()).collect();
    if let Ok(d) = s.doc_mut() {
        d.arch.sheets.retain(|sh| names.iter().any(|n| n.eq_ignore_ascii_case(&sh.name)));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use cadcraft_doc::Wall;
    use cadcraft_doc::arch::sheet::Schedule;
    use std::sync::Arc;

    /// A 10 x 8 m box of 0.2 m walls on level 0.
    fn house() -> Session {
        let mut s = Session::new();
        {
            let d = s.doc_mut().unwrap();
            let c = [Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0)];
            for i in 0..4 {
                d.arch.push_wall(Wall { start: c[i], end: c[(i + 1) % 4], thickness: 0.2, height: 2.7, ..Wall::default() });
            }
            d.touch_arch();
        }
        s
    }

    fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
        s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
    }

    fn refused(s: &mut Session, cmd: &str, p: Value) -> String {
        let before = s.state().unwrap().doc.clone();
        let undo = s.state().unwrap().undo.len();
        let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} must be refused")).to_string();
        assert!(Arc::ptr_eq(&before, &s.state().unwrap().doc) || *before == *s.doc().unwrap(), "{cmd} {p} changed the drawing");
        assert_eq!(s.state().unwrap().undo.len(), undo, "{cmd} {p} pushed an undo step");
        e
    }

    fn elevation(s: &mut Session) -> Value {
        run(s, "elevation", json!({"side": "south", "dimensions": false}))
    }

    fn viewports(s: &Session, layout: &str) -> Vec<cadcraft_doc::Viewport> {
        let l = s.doc().unwrap().layout(layout).unwrap();
        l.entities
            .iter()
            .filter_map(|e| match &e.kind {
                EntityKind::Viewport(v) => Some(v.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn sheet_makes_a_layout_with_scaled_linked_viewports() {
        let mut s = house();
        elevation(&mut s);
        let r = run(
            &mut s,
            "sheet",
            json!({"name": "Plans", "number": "A-101", "title": "Floor Plan",
            "views": [{"view": "Floor Plan", "level": 0, "scale": "1:50", "at": [300, 330]}, {"view": "v0", "scale": 50, "at": [300, 100]}]}),
        );
        assert_eq!(r["layout"], "Plans", "{r}");
        assert_eq!(r["number"], "A-101", "{r}");
        let paper = r["paper"].as_array().unwrap();
        assert!((paper[0].as_f64().unwrap() - 914.4).abs() < 0.5 && (paper[1].as_f64().unwrap() - 609.6).abs() < 0.5, "{r}");
        let views = r["views"].as_array().unwrap();
        assert_eq!(views.len(), 2, "{r}");
        assert_eq!(views[0]["source"]["kind"], "plan", "{r}");
        assert_eq!(views[1]["source"]["kind"], "elevation", "{r}");
        for v in views {
            assert!(v["viewport"].is_string(), "{v}");
            assert!((v["scale"].as_f64().unwrap() - 50.0).abs() < 1e-9, "{v}");
        }
        let vps = viewports(&s, "Plans");
        assert_eq!(vps.len(), 2);
        assert!(vps.iter().all(|v| v.locked));
        let d = s.doc().unwrap();
        let mut titles: Vec<(u32, String)> = d
            .layout("Plans")
            .unwrap()
            .entities
            .iter()
            .filter_map(|e| {
                let (role, _, n) = read_link(&e.common.xdata)?;
                match &e.kind {
                    EntityKind::Text(t) if role == LINK_VIEW_TITLE => Some((n, t.value.clone())),
                    _ => None,
                }
            })
            .collect();
        titles.sort();
        assert_eq!(titles.len(), 2, "{titles:?}");
        assert_eq!(titles[0].1, "1  FLOOR PLAN  1:50");
        assert_eq!(d.arch.sheets.len(), 1);
        assert!(!d.arch.sheet_set.date.is_empty() || cfg!(target_arch = "wasm32"));
    }

    #[test]
    fn elevation_viewport_is_twenty_mm_per_metre_at_1_50() {
        let mut s = house();
        elevation(&mut s);
        let b = s.doc().unwrap().arch.views[0].bounds().unwrap();
        let r = run(&mut s, "sheet", json!({"name": "E", "views": [{"view": "v0", "scale": "1:50", "at": [300, 300]}]}));
        let size = r["views"][0]["size"].as_array().unwrap();
        assert!((size[0].as_f64().unwrap() - (b[2] - b[0] + 3.0) * 20.0).abs() < 1e-6, "{r}");
        assert!((size[1].as_f64().unwrap() - (b[3] - b[1] + 3.0) * 20.0).abs() < 1e-6, "{r}");
        let vp = &viewports(&s, "E")[0];
        assert!((vp.scale() - 20.0).abs() < 1e-6, "{}", vp.scale());
        assert!((vp.width - size[0].as_f64().unwrap()).abs() < 1e-6);
    }

    #[test]
    fn plan_view_frame_is_the_level_plus_margin() {
        let mut s = house();
        let r = run(&mut s, "sheet", json!({"name": "P", "views": [{"view": "Floor Plan", "scale": 100, "at": [300, 300]}]}));
        // Walls span 10.2 x 8.2 m with their thickness; the frame adds 2.5 m on every side.
        let size = r["views"][0]["size"].as_array().unwrap();
        assert!((size[0].as_f64().unwrap() - (10.2 + 5.0) * 10.0).abs() < 1e-6, "{r}");
        assert!((size[1].as_f64().unwrap() - (8.2 + 5.0) * 10.0).abs() < 1e-6, "{r}");
        let vp = &viewports(&s, "P")[0];
        assert!((vp.view_center.x - 5.0).abs() < 1e-6 && (vp.view_center.y - 4.0).abs() < 1e-6, "{:?}", vp.view_center);
        let e = refused(&mut s, "sheet", json!({"name": "Q", "views": [{"view": "Floor Plan", "level": "3", "scale": 100}]}));
        assert!(e.contains("level"), "{e}");
    }

    #[test]
    fn schedule_frame_is_its_table() {
        let mut s = house();
        let table: cadcraft_doc::Table = serde_json::from_value(json!({
            "insert": serde_json::to_value(Vec3::new(20.0, 10.0, 0.0)).unwrap(),
            "colWidths": [1.0, 1.0], "rowHeights": [0.5, 0.5, 0.5], "cells": []
        }))
        .unwrap();
        let mut common = Common::default();
        set_link(&mut common.xdata, LINK_SCHEDULE, "sc0", 0);
        {
            let d = s.doc_mut().unwrap();
            d.arch.schedules.push(Schedule { name: "Doors".into(), columns: vec!["number".into()], ..Schedule::default() });
            d.add(&Space::Model, common, EntityKind::Table(table)).unwrap();
        }
        let tb = {
            let d = s.doc().unwrap();
            let e = d.model.iter().find(|e| matches!(e.kind, EntityKind::Table(_))).unwrap();
            cadcraft_doc::entity_bounds(d, e, 0)
        };
        let r = run(&mut s, "sheet", json!({"name": "S", "views": [{"view": "Doors", "scale": 50, "at": [300, 300]}]}));
        let size = r["views"][0]["size"].as_array().unwrap();
        assert!((size[0].as_f64().unwrap() - (tb.width() + 0.4) * 20.0).abs() < 1e-6, "{r} {tb:?}");
        assert!((size[1].as_f64().unwrap() - (tb.height() + 0.4) * 20.0).abs() < 1e-6, "{r}");
        assert_eq!(r["views"][0]["source"]["kind"], "schedule", "{r}");
    }

    #[test]
    fn auto_placement_fills_left_to_right() {
        let mut s = house();
        let one = json!({"view": "Floor Plan", "scale": 200});
        let r = run(&mut s, "sheet", json!({"name": "Auto", "views": [one.clone(), one.clone(), one.clone(), one]}));
        let v = r["views"].as_array().unwrap();
        let at = |i: usize| (v[i]["at"][0].as_f64().unwrap(), v[i]["at"][1].as_f64().unwrap());
        let size = (v[0]["size"][0].as_f64().unwrap(), v[0]["size"][1].as_f64().unwrap());
        // The first view's top-left corner sits at the margins.
        assert!((at(0).0 - size.0 / 2.0 - MARGIN_MM).abs() < 1e-6 && (at(0).1 + size.1 / 2.0 - (609.6 - MARGIN_MM)).abs() < 1e-6, "{r}");
        assert!((at(1).0 - at(0).0 - size.0 - GAP_MM).abs() < 1e-6 && (at(1).1 - at(0).1).abs() < 1e-9, "{r}");
        assert!(at(2).0 > at(1).0 || at(2).1 < at(1).1, "{r}");
        assert!(at(3).1 <= at(0).1 + 1e-9);
    }

    #[test]
    fn views_outside_the_paper_are_refused() {
        let mut s = house();
        let e = refused(&mut s, "sheet", json!({"name": "Big", "views": [{"view": "Floor Plan", "scale": 1}]}));
        assert!(e.contains("does not fit the sheet"), "{e}");
        let e = refused(&mut s, "sheet", json!({"name": "Off", "views": [{"view": "Floor Plan", "scale": 100, "at": [900, 300]}]}));
        assert!(e.contains("view 1 does not fit"), "{e}");
        assert!(s.doc().unwrap().layout("Big").is_none());
    }

    #[test]
    fn ambiguous_view_names_are_refused() {
        let mut s = house();
        s.doc_mut().unwrap().arch.schedules.push(Schedule { name: "Floor Plan".into(), columns: vec!["number".into()], ..Schedule::default() });
        let e = refused(&mut s, "sheet", json!({"name": "A", "views": [{"view": "Floor Plan", "scale": 100}]}));
        assert!(e.contains("ambiguous") && e.contains("plan view") && e.contains("schedule"), "{e}");
        let e = refused(&mut s, "sheet", json!({"name": "A", "views": [{"view": "Nope", "scale": 100}]}));
        assert!(e.contains("Nope"), "{e}");
    }

    #[test]
    fn sheet_numbers_default_and_stay_unique() {
        let mut s = house();
        let v = json!([{"view": "Floor Plan", "scale": 200}]);
        assert_eq!(run(&mut s, "sheet", json!({"name": "a", "views": v}))["number"], "1");
        assert_eq!(run(&mut s, "sheet", json!({"name": "b", "number": "3", "views": v}))["number"], "3");
        assert_eq!(run(&mut s, "sheet", json!({"name": "c", "views": v}))["number"], "2");
        assert_eq!(run(&mut s, "sheet", json!({"name": "d", "views": v}))["number"], "4");
        run(&mut s, "sheet", json!({"name": "f", "number": "A-1", "views": v}));
        let e = refused(&mut s, "sheet", json!({"name": "g", "number": "a-1", "views": v}));
        assert!(e.contains("already used"), "{e}");
        let e = refused(&mut s, "sheet", json!({"name": "F", "views": v}));
        assert!(e.contains("already exists"), "{e}");
    }

    #[test]
    fn edit_rename_number_title_add_remove_order() {
        let mut s = house();
        elevation(&mut s);
        let v = json!([{"view": "Floor Plan", "scale": 200, "at": [200, 300]}, {"view": "v0", "scale": 100, "at": [500, 300]}]);
        run(&mut s, "sheet", json!({"name": "One", "number": "A-1", "views": v}));
        run(&mut s, "sheet", json!({"name": "Two", "number": "A-2", "views": [{"view": "Floor Plan", "scale": 200}]}));
        let r = run(
            &mut s,
            "sheet.edit",
            json!({"name": "One", "rename": "Uno", "number": "B-1", "title": "First", "remove": [1],
            "add": [{"view": "Reflected Ceiling", "scale": 200, "at": [200, 150]}]}),
        );
        assert_eq!(r["layout"], "Uno", "{r}");
        assert_eq!(r["number"], "B-1", "{r}");
        assert_eq!(r["title"], "First", "{r}");
        let nums: Vec<u64> = r["views"].as_array().unwrap().iter().map(|v| v["number"].as_u64().unwrap()).collect();
        assert_eq!(nums, vec![2, 3], "{r}");
        for v in r["views"].as_array().unwrap() {
            assert!(v["viewport"].is_string(), "links follow the rename: {v}");
        }
        let d = s.doc().unwrap();
        assert!(d.layout("One").is_none() && d.layout("Uno").is_some());
        assert_eq!(viewports(&s, "Uno").len(), 2);
        let texts = d.layout("Uno").unwrap().entities.iter().filter(|e| matches!(e.kind, EntityKind::Text(_))).count();
        assert_eq!(texts, 2, "the removed view's title went with it");
        let r = run(&mut s, "sheet.edit", json!({"name": "Two", "order": 0}));
        assert_eq!(r["sheet"], "Two");
        let names: Vec<String> = s.doc().unwrap().arch.sheets.iter().map(|x| x.name.clone()).collect();
        assert_eq!(names, vec!["Two", "Uno"]);
        refused(&mut s, "sheet.edit", json!({"name": "Two"}));
        refused(&mut s, "sheet.edit", json!({"name": "Two", "order": 2}));
        refused(&mut s, "sheet.edit", json!({"name": "Two", "number": "b-1"}));
        refused(&mut s, "sheet.edit", json!({"name": "Two", "remove": [9]}));
        refused(&mut s, "sheet.edit", json!({"name": "Nope", "title": "x"}));
    }

    #[test]
    fn delete_removes_the_layout() {
        let mut s = house();
        run(&mut s, "sheet", json!({"name": "Gone", "views": [{"view": "Floor Plan", "scale": 200}]}));
        let r = run(&mut s, "sheet.delete", json!({"name": "gone"}));
        assert_eq!(r["deleted"], "Gone");
        let d = s.doc().unwrap();
        assert!(d.layout("Gone").is_none() && d.arch.sheets.is_empty());
        refused(&mut s, "sheet.delete", json!({"name": "Gone"}));
    }

    #[test]
    fn open_by_name_and_by_view() {
        let mut s = house();
        elevation(&mut s);
        run(&mut s, "sheet", json!({"name": "Plans", "views": [{"view": "Floor Plan", "scale": 200}]}));
        run(&mut s, "sheet", json!({"name": "Elev", "views": [{"view": "v0", "scale": 100}]}));
        let r = run(&mut s, "sheet.open", json!({"name": "Plans"}));
        assert_eq!(r, json!({"sheet": "Plans", "layout": "Plans"}));
        assert_eq!(s.layout_space().layout(), Some("Plans"));
        let r = run(&mut s, "sheet.open", json!({"view": "v0"}));
        assert_eq!(r["sheet"], "Elev", "{r}");
        assert_eq!(s.layout_space().layout(), Some("Elev"));
        refused(&mut s, "sheet.open", json!({"view": "v9"}));
        refused(&mut s, "sheet.open", json!({"name": "Nope"}));
        refused(&mut s, "sheet.open", json!({}));
        let l = run(&mut s, "sheet.list", json!({}));
        assert_eq!(l["sheets"][0], json!({"name": "Plans", "number": "1", "title": "Plans", "views": 1}), "{l}");
    }

    #[test]
    fn sheetset_updates_the_set() {
        let mut s = house();
        let r = run(&mut s, "sheetset", json!({"project": "House", "date": "2026-11-09"}));
        assert_eq!(r, json!({"project": "House", "date": "2026-11-09"}));
        let r = run(&mut s, "sheetset", json!({"project": "Other"}));
        assert_eq!(r, json!({"project": "Other", "date": "2026-11-09"}));
        assert_eq!(s.doc().unwrap().arch.sheet_set.project, "Other");
        refused(&mut s, "sheetset", json!({}));
        refused(&mut s, "sheetset", json!({"project": 5}));
        refused(&mut s, "sheetset", json!({"project": "x".repeat(129)}));
        refused(&mut s, "sheetset", json!({"date": "x".repeat(33)}));
    }

    #[test]
    fn a_deleted_layout_prunes_its_sheet() {
        let mut s = house();
        run(&mut s, "sheet", json!({"name": "Plans", "views": [{"view": "Floor Plan", "scale": 200}]}));
        run(&mut s, "sheet", json!({"name": "Other", "views": [{"view": "Floor Plan", "scale": 200}]}));
        run(&mut s, "layout.delete", json!({"name": "Plans"}));
        let names: Vec<String> = s.doc().unwrap().arch.sheets.iter().map(|x| x.name.clone()).collect();
        assert_eq!(names, vec!["Other"]);
    }

    #[test]
    fn refusals_change_nothing_and_push_no_undo() {
        let mut s = house();
        elevation(&mut s);
        let v = json!([{"view": "v0", "scale": "1:50"}]);
        run(&mut s, "sheet", json!({"name": "Sheet A", "number": "A-1", "views": v}));
        let bad_ones = [
            json!({"name": "Bad", "views": [{"view": "Nope", "scale": "1:50"}]}),
            json!({"name": "Bad", "size": "ARCH Z", "views": v}),
            json!({"name": "Bad", "views": [{"view": "v0", "scale": "1:0"}]}),
            json!({"name": "Bad", "views": [{"view": "v0", "scale": -50}]}),
            json!({"name": "Sheet A", "views": v}),
            json!({"name": "Bad", "number": "A-1", "views": v}),
            json!({"name": "Bad", "views": "v0"}),
            json!({"name": "Bad", "views": []}),
            json!({"name": "", "views": v}),
            json!({"name": "Bad", "views": [{"camera": "Nope"}]}),
            json!({"name": "Bad", "views": [{"view": "v0", "scale": "1:50", "at": ["a", 1]}]}),
            json!({"name": "Bad", "views": [{"view": "v0", "scale": "1:50", "at": [1e9, 1]}]}),
            json!({"name": "Bad", "views": [{"view": "v0", "scale": "1:50", "level": 0}]}),
            json!({"name": "Bad", "title": 4, "views": v}),
            json!({"name": "Bad", "landscape": "yes", "views": v}),
            json!({"name": "Bad", "views": [5]}),
            json!({"name": "a<b", "views": v}),
        ];
        let layouts = s.doc().unwrap().layouts.len();
        for p in bad_ones {
            refused(&mut s, "sheet", p);
        }
        assert_eq!(s.doc().unwrap().arch.sheets.len(), 1);
        assert_eq!(s.doc().unwrap().layouts.len(), layouts);
    }

    #[test]
    fn a_sheet_is_one_undo_step() {
        let mut s = house();
        let undo = s.state().unwrap().undo.len();
        run(&mut s, "sheet", json!({"name": "U", "views": [{"view": "Floor Plan", "scale": 200}, {"view": "Reflected Ceiling", "scale": 200}]}));
        assert_eq!(s.state().unwrap().undo.len(), undo + 1);
        run(&mut s, "undo", json!({}));
        let d = s.doc().unwrap();
        assert!(d.layout("U").is_none() && d.arch.sheets.is_empty());
    }
}
