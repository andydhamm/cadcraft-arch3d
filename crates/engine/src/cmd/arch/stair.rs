//! The `stair` command: lay out a stair flight between two levels.
//!
//! JSON parameters:
//!
//! - `start`: required, `[x, y]` — centre of the bottom nosing line.
//! - `direction`: required, degrees counter-clockwise from +x; stored in
//!   radians on the model.
//! - `width`: optional, metres; defaults to 1.0, must be finite in `(0, 5]`.
//! - `fromLevel` / `toLevel`: optional, a level name or its numeric index
//!   (index tried first, then name, like the `level` command). Absent (or
//!   null) means the first level / the last level; a present but unknown
//!   level is refused. They name the two levels the stair joins, in either
//!   order: the pair is ordered by elevation and the stored `from_level` is
//!   the lower, `to_level` the upper. Equal elevations (one level: a deck stair
//!   to grade) need an explicit `rise` and cut no well. `start` is
//!   the centre of the bottom nosing line on the lower level.
//! - `rise`: optional, metres; when present must be finite and positive and is
//!   used as given. When absent it is derived as `upper.elevation -
//!   lower.elevation`.
//! - `shape`: optional, `straight|l|u` (case-insensitive, default
//!   `straight`); only straight flights are meshed (phase-5 gap note).
//! - `tread` / `maxRiser` / `minTread`: optional positive metres; default to
//!   `MIN_TREAD` / `MAX_RISER` / `MIN_TREAD`.
//!
//! A well is considered whenever the upper level is above the lower (never by
//! list index). Headroom is measured against the upper level's slab underside
//! (`upper.elevation - upper.structure`), with riser tops counted from the
//! lower level's elevation (the stair's base). When the upper
//! floor lacks `MIN_HEADROOM` clear of the flight, the stair's well becomes an
//! `Open Below` room on the upper level ringed by a guard railing that follows
//! the stair, and is cut as a hole into the upper room that contains it.
//!
//! - `openRisers` (bool), `stringers` (`two|centre|center|none`), `underside`
//!   (metres in `[MIN_UNDERSIDE, MAX_UNDERSIDE]`), `railLeft` / `railRight`
//!   (`auto|on|off`), `planCut` (metres in `[0.3, 3.0]`): optional detail; wrong
//!   types and out-of-range values are refused before anything is written.
//!
//! Returns `{"id":"s0","handle":"...","risers":n,"riserHeight":h,"run":r,
//! "well":bool,"openRisers":..,"stringers":..,"underside":..,"railLeft":..,
//! "railRight":..,"planCut":..,"railings":[labels]}`.
//!
//! `stair.edit` changes only the detail values given on an existing stair
//! (never its geometry) as one undo step.

use std::f64::consts::PI;

use cadcraft_arch::poly::ring_area;
use cadcraft_arch::railing_auto::{RailHostRef, regenerate};
use cadcraft_arch::stair::{GUARD_HEIGHT, MAX_RISER, MIN_HEADROOM, MIN_TREAD, plan_stair, well_guard_path, well_polygon};
use cadcraft_doc::arch::{
    MAX_UNDERSIDE, MIN_UNDERSIDE, RailSetting, Railing, RailingHost, Room, RoomType, Stair, StairShape, Stringers, stair_handle,
};
use cadcraft_doc::{ArchId, railing_label};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad, f64_or, f64_req, point_req, size_param};
use crate::prompt::{Accept, Input, Interactive, Prompt, Step};
use crate::{EngineError, Result, Session};

/// Prompt accept: number or free text (a level index or name, or a measure).
const TEXT_OR_NUMBER: Accept = Accept { point: false, number: true, text: true, select: false, enter: true };

/// Command specs for the stair module.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("stair", "Stair", run_stair)
            .menu(&["Architecture", "Stair"])
            .alias(&["st"])
            .params(
                "{start:[x,y], direction:deg, width?, fromLevel?|toLevel?|rise?, shape?, tread?, maxRiser?, minTread?, openRisers?, \
                 stringers?: two|centre|none, underside?, railLeft?|railRight?: auto|on|off, planCut?}",
            )
            .interactive(|_| Ok(Box::new(StairM::default()))),
        CommandSpec::new("stair.edit", "Edit Stair", run_stair_edit)
            .menu(&["Architecture", "Edit Stair"])
            .params("{id: \"s0\", openRisers?, stringers?, underside?, railLeft?, railRight?, planCut?}"),
    ]
}

/// Plan cut height limits (metres above the from-level floor).
const MIN_PLAN_CUT: f64 = 0.3;
const MAX_PLAN_CUT: f64 = 3.0;

/// The phase-17 detail values a `stair` or `stair.edit` call gave; `None` is absent.
#[derive(Default)]
struct Detail {
    open_risers: Option<bool>,
    stringers: Option<Stringers>,
    underside: Option<f64>,
    rail_left: Option<RailSetting>,
    rail_right: Option<RailSetting>,
    plan_cut: Option<f64>,
}

impl Detail {
    fn any(&self) -> bool {
        self.open_risers.is_some()
            || self.stringers.is_some()
            || self.underside.is_some()
            || self.rail_left.is_some()
            || self.rail_right.is_some()
            || self.plan_cut.is_some()
    }

    /// Parse and validate every detail key (null = absent); nothing is written.
    fn parse(cmd: &str, p: &Value) -> Result<Detail> {
        let present = |k: &str| p.get(k).filter(|v| !v.is_null());
        let name = |k: &str, what: &str| -> Result<Option<String>> {
            match present(k) {
                None => Ok(None),
                Some(Value::String(t)) => Ok(Some(t.clone())),
                Some(_) => Err(bad(cmd, format!("`{k}` must be a string ({what})"))),
            }
        };
        let ranged = |k: &str, lo: f64, hi: f64| -> Result<Option<f64>> {
            match present(k) {
                None => Ok(None),
                Some(v) => match v.as_f64().filter(|x| x.is_finite() && *x >= lo && *x <= hi) {
                    Some(x) => Ok(Some(x)),
                    None => Err(bad(cmd, format!("`{k}` must be a number in [{lo}, {hi}]"))),
                },
            }
        };
        let rail = |k: &str| -> Result<Option<RailSetting>> {
            match name(k, "auto, on or off")? {
                None => Ok(None),
                Some(t) => RailSetting::parse(&t).map(Some).ok_or_else(|| bad(cmd, format!("`{k}` `{t}` must be auto, on or off"))),
            }
        };
        let open_risers = match present("openRisers") {
            None => None,
            Some(Value::Bool(b)) => Some(*b),
            Some(_) => return Err(bad(cmd, "`openRisers` must be true or false")),
        };
        let stringers = match name("stringers", "two, centre or none")? {
            None => None,
            Some(t) => Some(Stringers::parse(&t).ok_or_else(|| bad(cmd, format!("`stringers` `{t}` must be two, centre or none")))?),
        };
        Ok(Detail {
            open_risers,
            stringers,
            underside: ranged("underside", MIN_UNDERSIDE, MAX_UNDERSIDE)?,
            rail_left: rail("railLeft")?,
            rail_right: rail("railRight")?,
            plan_cut: ranged("planCut", MIN_PLAN_CUT, MAX_PLAN_CUT)?,
        })
    }

    /// Set the given values on a stair.
    fn apply(&self, st: &mut Stair) {
        if let Some(v) = self.open_risers {
            st.open_risers = v;
        }
        if let Some(v) = self.stringers {
            st.stringers = v;
        }
        if let Some(v) = self.underside {
            st.underside = v;
        }
        if let Some(v) = self.rail_left {
            st.rail_left = v;
        }
        if let Some(v) = self.rail_right {
            st.rail_right = v;
        }
        if let Some(v) = self.plan_cut {
            st.plan_cut = v;
        }
    }
}

/// Labels of the automatic railings along a stair's flight, in model order.
fn auto_rail_labels(arch: &cadcraft_doc::arch::ArchModel, id: ArchId) -> Vec<String> {
    arch.railings
        .iter()
        .filter(|r| r.auto && r.follows == Some(id) && matches!(r.host, RailingHost::StairLeft | RailingHost::StairRight))
        .map(|r| railing_label(r.id))
        .collect()
}

/// The six detail values of a stair as reply JSON members.
fn detail_json(st: &Stair) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("openRisers".into(), json!(st.open_risers));
    m.insert("stringers".into(), json!(st.stringers.name()));
    m.insert("underside".into(), json!(st.underside));
    m.insert("railLeft".into(), json!(st.rail_left.name()));
    m.insert("railRight".into(), json!(st.rail_right.name()));
    m.insert("planCut".into(), json!(st.plan_cut));
    m
}

/// Parse a stair id: `"s3"` or the bare number (string or JSON number).
fn parse_stair_id(cmd: &str, p: &Value) -> Result<ArchId> {
    let err = || bad(cmd, "`id` must name a stair, like \"s0\"");
    let raw = match p.get("id") {
        Some(Value::String(t)) => t.trim().to_ascii_lowercase(),
        Some(Value::Number(n)) => n.as_u64().ok_or_else(err)?.to_string(),
        _ => return Err(err()),
    };
    let digits = raw.strip_prefix('s').unwrap_or(&raw);
    digits.parse::<u32>().map(ArchId).map_err(|_| err())
}

/// Run `stair.edit`: change the detail values given on an existing stair.
fn run_stair_edit(s: &mut Session, p: &Value) -> Result<Value> {
    let id = parse_stair_id("stair.edit", p)?;
    let detail = Detail::parse("stair.edit", p)?;
    if !detail.any() {
        return Err(bad("stair.edit", "nothing to change: give openRisers, stringers, underside, railLeft, railRight or planCut"));
    }
    let d = s.doc_mut()?;
    let pos = d.arch.stair_index(id).ok_or_else(|| bad("stair.edit", format!("unknown stair `s{}`", id.0)))?;
    let rails_changed = {
        let st = d.arch.stairs.get_mut(pos).ok_or_else(|| bad("stair.edit", "unknown stair"))?;
        let before = (st.rail_left, st.rail_right);
        detail.apply(st);
        before != (st.rail_left, st.rail_right)
    };
    if rails_changed {
        let made = regenerate(&mut d.arch, RailHostRef::Stair(id));
        if !made.is_empty() {
            d.ensure_layer("A-RAIL");
        }
    }
    d.touch_arch();
    let mut m = d.arch.stairs.get(pos).map(detail_json).unwrap_or_default();
    m.insert("stair".into(), json!(format!("s{}", id.0)));
    m.insert("railings".into(), json!(auto_rail_labels(&d.arch, id)));
    Ok(Value::Object(m))
}

/// Find a level by name (string) or index (number) and return its position in
/// the `levels` vec. Mirrors `cmd/arch/level.rs::find_level`: a numeric string
/// is tried as an index first, then as a name; unknown references are an
/// error.
fn find_level(arch: &cadcraft_doc::arch::ArchModel, ref_val: &Value) -> Result<usize> {
    if let Some(n) = ref_val.as_str() {
        // Try as index first.
        if let Some(i) = n.parse::<u64>().ok() {
            if i < arch.levels.len() as u64 {
                return Ok(i as usize);
            }
        }
        // Then as name.
        if let Some((i, _l)) = arch.levels.iter().enumerate().find(|(_i, l)| l.name == n) {
            return Ok(i);
        }
        return Err(EngineError::Other(format!("stair: unknown level '{n}'")));
    }
    if let Some(i) = ref_val.as_u64() {
        if i < arch.levels.len() as u64 {
            return Ok(i as usize);
        }
        return Err(EngineError::Other(format!("stair: unknown level index {i}")));
    }
    Err(EngineError::Other("stair: invalid level reference".into()))
}

/// Map a case-insensitive shape name to a [`StairShape`]; `None` for unknown
/// strings (refused, never coerced).
fn parse_stair_shape(s: &str) -> Option<StairShape> {
    match s.trim().to_ascii_lowercase().as_str() {
        "straight" => Some(StairShape::Straight),
        "l" => Some(StairShape::L),
        "u" => Some(StairShape::U),
        _ => None,
    }
}

/// Run the `stair` command from JSON parameters. Programmatic calls never open
/// dialogs.
fn run_stair(s: &mut Session, p: &Value) -> Result<Value> {
    // --- start: required finite point ---
    let start = point_req("stair", p, "start")?;
    if !start.is_finite() {
        return Err(bad("stair", "start must be a finite point"));
    }

    // --- direction: degrees CCW from +x, stored as radians ---
    let direction_deg = f64_req("stair", p, "direction")?;
    let direction = direction_deg * PI / 180.0;
    if !direction.is_finite() {
        return Err(bad("stair", "direction must be a finite number of degrees"));
    }

    // --- width: default 1.0 m, finite in (0, 5] ---
    let width = f64_or(p, "width", 1.0);
    if !(width > 0.0 && width <= 5.0) {
        return Err(bad("stair", format!("width {width} must be a finite number in (0, 5]")));
    }

    // --- shape: straight | l | u, default straight; unknown refused ---
    let shape = match p.get("shape") {
        None | Some(Value::Null) => StairShape::Straight,
        Some(Value::String(t)) => match parse_stair_shape(t) {
            Some(sh) => sh,
            None => return Err(bad("stair", format!("shape `{t}` must be one of: straight, l or u"))),
        },
        Some(_) => return Err(bad("stair", "shape must be a string")),
    };

    // --- phase-17 detail: validated before anything is written ---
    let detail = Detail::parse("stair", p)?;

    // --- levels: optional (first / last); a present unknown level -> error ---
    let from_ref = p.get("fromLevel").filter(|v| !v.is_null());
    let to_ref = p.get("toLevel").filter(|v| !v.is_null());

    let d = s.doc_mut()?;
    let arch = &d.arch;
    let from_i = match from_ref {
        Some(v) => find_level(arch, v)?,
        None => 0,
    };
    let to_i = match to_ref {
        Some(v) => find_level(arch, v)?,
        None => arch.levels.len().saturating_sub(1),
    };
    let first = arch.levels.get(from_i).ok_or_else(|| bad("stair", "unknown fromLevel"))?;
    let second = arch.levels.get(to_i).ok_or_else(|| bad("stair", "unknown toLevel"))?;
    // The two levels are ordered by elevation, whichever way they were named:
    // the lower is `from_level`, the upper `to_level`. Non-finite elevations are
    // refused. Equal elevations (one level, e.g. a deck stair to grade) are a
    // stair only with an explicit `rise` (a derived rise is then 0 and refused
    // below), and it climbs to no upper floor, so it cuts no well.
    if !(first.elevation.is_finite() && second.elevation.is_finite()) {
        return Err(bad("stair", "the two levels must have finite elevations"));
    }
    let (from_i, to_i, from_level, to_level) =
        if second.elevation >= first.elevation { (from_i, to_i, first, second) } else { (to_i, from_i, second, first) };
    let climbs = to_level.elevation > from_level.elevation;

    // --- rise: param if finite & positive, else derived (finite & positive) ---
    let rise = match p.get("rise") {
        None | Some(Value::Null) => {
            let r = to_level.elevation - from_level.elevation;
            if r.is_finite() && r > 0.0 {
                r
            } else {
                return Err(bad("stair", "derived rise must be finite and positive"));
            }
        }
        Some(v) => match v.as_f64().filter(|x| x.is_finite()) {
            Some(x) if x > 0.0 => x,
            _ => return Err(bad("stair", "rise must be a finite positive number")),
        },
    };

    // --- optional positive sizes; absent => default ---
    let tread = size_param("stair", p, "tread", false)?;
    let max_riser = size_param("stair", p, "maxRiser", false)?;
    let min_tread = size_param("stair", p, "minTread", false)?;

    // --- plan; no write on failure ---
    let plan = plan_stair(rise, width, tread, max_riser.unwrap_or(MAX_RISER), min_tread.unwrap_or(MIN_TREAD));
    let plan = match plan {
        Some(pl) => pl,
        None => return Err(bad("stair", "no valid stair plan for the given rise, width and tread")),
    };

    // --- well + guard, computed before any write; headroom vs the upper slab
    // underside ---
    let mut stair = Stair {
        start,
        direction,
        width,
        from_level: from_i as u32,
        to_level: to_i as u32,
        rise,
        shape,
        risers: plan.risers,
        riser_height: plan.riser_height,
        tread_depth: plan.tread_depth,
        treads: plan.treads,
        run: plan.run,
        landing_depth: plan.landing_depth,
        well: Vec::new(),
        well_room: None,
        layer: "A-STAIR".into(),
        ..Stair::default()
    };
    detail.apply(&mut stair);
    let slab_underside = to_level.elevation - to_level.structure;
    // Levels are ordered by elevation above, so the upper level is always
    // higher; riser tops are counted from the lower level's elevation.
    let base_z = from_level.elevation;
    let well_poly = if climbs { well_polygon(&stair, &plan, base_z, slab_underside, MIN_HEADROOM) } else { None };
    let has_well = well_poly.is_some();
    let guard = well_poly.as_ref().and_then(|_| well_guard_path(&stair, &plan, base_z, slab_underside, MIN_HEADROOM));
    let well = well_poly.unwrap_or_default();

    // --- one mutation + one revision bump; never a partial write ---
    let stair_id = d.arch.push_stair({
        stair.well = well.clone();
        stair
    });

    if has_well {
        // Cut the well out of the upper room that contains it (never invent
        // a room).
        if let Some(c) = cadcraft_geom::polygon_centroid(&well) {
            let target = d.arch.rooms.iter().position(|r| {
                r.level == to_i as u32
                    && cadcraft_geom::point_in_polygon(&r.polygon, c)
                    && !r.holes.iter().any(|h| cadcraft_geom::point_in_polygon(h, c))
            });
            if let Some(room) = target.and_then(|i| d.arch.rooms.get_mut(i)) {
                room.holes.push(well.clone());
            }
        }
        // Upper-level room for the open well.
        let mut room =
            Room { name: "Open Below".into(), room_type: RoomType::OpenBelow, level: to_i as u32, polygon: well.clone(), ..Room::default() };
        room.interior_area = ring_area(&well);
        room.standard_area = cadcraft_arch::area::standard_area(&d.arch, &room);
        let room_id = d.arch.push_room(room);

        // Guard railing ringing the well, following the stair.
        if let Some(guard) = guard {
            d.arch.push_railing(Railing {
                path: guard,
                level: to_i as u32,
                height: GUARD_HEIGHT,
                follows: Some(stair_id),
                layer: "A-RAIL".into(),
                ..Railing::default()
            });
        }

        // Link the room to the stored stair.
        if let Some(pos) = d.arch.stair_index(stair_id) {
            d.arch.stairs[pos].well_room = Some(room_id);
        }
    }

    // The side railings the stair should have (the well guard above stays).
    let rails = regenerate(&mut d.arch, RailHostRef::Stair(stair_id));
    if !rails.is_empty() {
        d.ensure_layer("A-RAIL");
    }

    cadcraft_arch::area::refresh_areas(&mut d.arch);
    d.touch_arch();

    let mut m = d.arch.stair(stair_id).map(detail_json).unwrap_or_default();
    m.insert("id".into(), json!(format!("s{}", stair_id.0)));
    m.insert("handle".into(), json!(cadcraft_doc::arch::handle_json(stair_handle(stair_id))));
    m.insert("risers".into(), json!(plan.risers));
    m.insert("riserHeight".into(), json!(plan.riser_height));
    m.insert("run".into(), json!(plan.run));
    m.insert("well".into(), json!(has_well));
    m.insert("railings".into(), json!(rails.iter().map(|r| railing_label(*r)).collect::<Vec<_>>()));
    Ok(Value::Object(m))
}

/// Interactive `stair` machine: collect the start point, then direction,
/// width (optional), from level and to level one prompt at a time, then
/// delegate to the JSON `stair` command on finish so there is exactly one
/// implementation path and one undo step. Mirrors the `room` machine style.
pub struct StairM {
    /// 0 = start point, 1 = direction, 2 = width, 3 = from level,
    /// 4 = to level.
    phase: u8,
    start: Option<Vec2>,
    direction: Option<String>,
    width: Option<String>,
    from_level: Option<String>,
    to_level: Option<String>,
    base: Option<Vec2>,
}

impl Default for StairM {
    fn default() -> Self {
        Self { phase: 0, start: None, direction: None, width: None, from_level: None, to_level: None, base: None }
    }
}

impl StairM {
    /// The text of the input (free text or a matched keyword), or `None` for
    /// points/enter/cancel/other.
    fn text_of(i: &Input) -> Option<String> {
        match i {
            Input::Text(t) => Some(t.to_string()),
            Input::Keyword(k) => Some(k.to_string()),
            _ => None,
        }
    }

    /// The parameters the JSON `stair` command accepts, built from the values
    /// collected so far (only fields set are emitted).
    fn build_params(&self) -> Value {
        let mut m = serde_json::Map::new();
        if let Some(pt) = self.start {
            m.insert("start".into(), json!([pt.x, pt.y]));
        }
        if let Some(d) = self.direction.as_deref() {
            m.insert("direction".into(), json!(d));
        }
        if let Some(w) = self.width.as_deref() {
            m.insert("width".into(), json!(w));
        }
        if let Some(f) = self.from_level.as_deref() {
            m.insert("fromLevel".into(), json!(f));
        }
        if let Some(t) = self.to_level.as_deref() {
            m.insert("toLevel".into(), json!(t));
        }
        Value::Object(m)
    }
}

impl Interactive for StairM {
    fn name(&self) -> &'static str {
        "stair"
    }

    fn prompt(&self, _s: &Session) -> Prompt {
        let (message, keywords, accept) = match self.phase {
            0 => ("Specify start point:".to_string(), Vec::new(), Accept::POINT),
            1 => ("Specify direction (degrees) or [Cancel]:".to_string(), vec!["Cancel".to_string()], TEXT_OR_NUMBER),
            2 => ("Specify width in metres or [Enter for 1]:".to_string(), Vec::new(), TEXT_OR_NUMBER),
            3 => ("Specify from level or [Cancel]:".to_string(), vec!["Cancel".to_string()], TEXT_OR_NUMBER),
            4 => ("Specify to level:".to_string(), vec![], TEXT_OR_NUMBER),
            _ => ("Specify to level:".to_string(), Vec::new(), TEXT_OR_NUMBER),
        };
        Prompt {
            message,
            keywords,
            default: None,
            accept,
            base: self.base,
            corner: false,
            grip: None,
            picker: None,
            distance: false,
            deferred: false,
            raw: false,
            picks_one: false,
        }
    }

    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        // Cancel (the input, or a "cancel" keyword/text) exits the command.
        match &i {
            Input::Cancel => return Ok(Step::Cancel),
            Input::Keyword(k) => {
                if k.to_ascii_lowercase() == "cancel" {
                    return Ok(Step::Cancel);
                }
            }
            Input::Text(t) => {
                if t.to_ascii_lowercase() == "cancel" {
                    return Ok(Step::Cancel);
                }
            }
            _ => {}
        }

        // Phase 0: start point (points only; anything else re-prompts).
        if self.phase == 0 {
            if let Input::Point(pt) = i {
                if pt.is_finite() {
                    self.start = Some(pt);
                    self.base = Some(pt);
                    self.phase = 1;
                }
            }
            return Ok(Step::Continue);
        }

        let Some(txt) = StairM::text_of(&i) else {
            // Enter etc.
            // Width is the only phase where "no answer" means "use the
            // default"; every required phase re-prompts.
            if self.phase == 2 {
                self.phase = 3;
            }
            return Ok(Step::Continue);
        };

        let txt = txt.trim();
        match self.phase {
            // Direction: required.
            1 => {
                if !txt.is_empty() {
                    self.direction = Some(txt.to_string());
                    self.phase = 2;
                }
                return Ok(Step::Continue);
            }
            // Width: optional; empty = default.
            2 => {
                if !txt.is_empty() {
                    self.width = Some(txt.to_string());
                }
                self.phase = 3;
                return Ok(Step::Continue);
            }
            // From level: required.
            3 => {
                if !txt.is_empty() {
                    self.from_level = Some(txt.to_string());
                    self.phase = 4;
                }
                return Ok(Step::Continue);
            }
            // To level: required; a non-empty answer finishes the command.
            4 | _ => {
                if !txt.is_empty() {
                    self.to_level = Some(txt.to_string());
                    return s.execute("stair", &self.build_params()).map(|_| Step::Done);
                }
                // Empty text / enter: to level is required, re-prompt.
                Ok(Step::Continue)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use cadcraft_doc::arch::RoomType;
    use serde_json::json;

    fn new_session() -> Session {
        Session::new()
    }

    /// Add a second level (Level 2 at 2.7 m) above the default Level 1.
    fn two_level_session() -> Session {
        let mut s = new_session();
        let _ = s.execute("level", &json!({"above": 0}));
        s
    }

    #[test]
    fn stair_command_16_risers_for_two_levels() {
        let mut s = two_level_session();
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": 0, "toLevel": 1 })).unwrap();
        assert_eq!(r["id"].as_str().unwrap(), "s0");
        assert_eq!(r["risers"].as_u64(), Some(16));
        let riser_height = r["riserHeight"].as_f64().unwrap();
        let run = r["run"].as_f64().unwrap();
        assert!((riser_height - 0.16875).abs() < 1e-9);
        assert!((run - 3.81).abs() < 1e-6);
    }

    /// Push a 20 m square room on `level` centred on the origin.
    fn add_big_room(s: &mut Session, level: u32) {
        let d = s.doc_mut().unwrap();
        let polygon = vec![Vec2::new(-10.0, -10.0), Vec2::new(10.0, -10.0), Vec2::new(10.0, 10.0), Vec2::new(-10.0, 10.0)];
        d.arch.push_room(Room { name: "Hall".into(), level, polygon, ..Room::default() });
    }

    #[test]
    fn stair_without_levels_uses_first_and_last_level() {
        let mut s = two_level_session();
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0 })).unwrap();
        assert_eq!(r["risers"].as_u64(), Some(16));
        let st = &s.doc().unwrap().arch.stairs[0];
        assert_eq!((st.from_level, st.to_level), (0, 1));
        // One level and no rise: nothing to derive, refused.
        let mut one = new_session();
        assert!(one.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0 })).is_err());
        assert_eq!(one.doc().unwrap().arch.stairs.len(), 0);
        // A present unknown level is still refused.
        assert!(s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "toLevel": 99 })).is_err());
    }

    /// Phase-5 check 02: a one-level plan's deck stair to grade, given only its rise. Both levels default to the one level
    /// (equal elevations), which is a stair, not an error, when the rise is given; it climbs to no upper floor, so no well.
    #[test]
    fn one_level_stair_with_a_rise_is_made_without_a_well() {
        let mut s = new_session();
        let r = s.execute("stair", &json!({ "start": [2.0, -0.5], "direction": 90.0, "width": 0.9, "rise": 0.9 })).unwrap();
        assert_eq!(r["risers"].as_u64(), Some(5));
        assert_eq!(r["well"].as_bool(), Some(false));
        let d = s.doc().unwrap();
        assert_eq!(d.arch.stairs.len(), 1);
        assert!(d.arch.stairs[0].well.is_empty());
        assert!(d.arch.railings.iter().all(|r| !(r.host == RailingHost::Well && !r.auto)));
        assert!(d.arch.rooms.iter().all(|r| r.room_type != RoomType::OpenBelow));
        // Without a rise, equal elevations still have nothing to climb.
        assert!(s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0 })).is_err());
    }

    #[test]
    fn deck_stair_to_grade_makes_no_well() {
        let mut s = two_level_session();
        add_big_room(&mut s, 0);
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": 1, "toLevel": 0, "rise": 2.7 })).unwrap();
        // Behaviour changed: the pair is ordered by elevation (lower first), so the well is
        // cut, and the stair keeps it.
        assert_eq!(r["well"].as_bool(), Some(true));
        let d = s.doc().unwrap();
        assert_eq!((d.arch.stairs[0].from_level, d.arch.stairs[0].to_level), (0, 1));
        assert!(!d.arch.stairs[0].well.is_empty());
    }

    #[test]
    fn top_down_pair_is_ordered_not_refused() {
        let mut s = two_level_session();
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": 1, "toLevel": 0 })).unwrap();
        assert_eq!(r["risers"].as_u64(), Some(16));
        let st = &s.doc().unwrap().arch.stairs[0];
        assert_eq!((st.from_level, st.to_level), (0, 1));
    }

    /// Level 0 renamed Main at 2.7 m and a lower level made after it at 0.
    fn main_above_lower_session() -> Session {
        let mut s = two_level_session();
        let d = s.doc_mut().unwrap();
        d.arch.levels[0].name = "Main".into();
        d.arch.levels[0].elevation = 2.7;
        d.arch.levels[1].name = "Lower".into();
        d.arch.levels[1].elevation = 0.0;
        s
    }

    #[test]
    fn stair_orders_its_levels_by_elevation() {
        for (from, to) in [("Lower", "Main"), ("Main", "Lower")] {
            let mut s = main_above_lower_session();
            add_big_room(&mut s, 0);
            add_big_room(&mut s, 1);
            let r = s.execute("stair", &json!({ "start": [2.0, 1.0], "direction": 90.0, "fromLevel": from, "toLevel": to })).unwrap();
            assert_eq!(r["well"].as_bool(), Some(true), "{from} -> {to}");
            let d = s.doc().unwrap();
            let st = &d.arch.stairs[0];
            assert_eq!((st.from_level, st.to_level), (1, 0), "{from} -> {to}");
            let open = d.arch.rooms.iter().find(|r| r.room_type == RoomType::OpenBelow).unwrap();
            assert_eq!(open.level, 0);
            let main_hall = d.arch.rooms.iter().find(|r| r.level == 0 && r.name == "Hall").unwrap();
            assert_eq!(main_hall.holes.len(), 1);
        }
        // Equal elevations are still refused.
        let mut s = main_above_lower_session();
        s.doc_mut().unwrap().arch.levels[1].elevation = 2.7;
        assert!(s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": 0, "toLevel": 1 })).is_err());
    }

    #[test]
    fn stair_from_an_upper_level_measures_headroom_from_its_base() {
        let mut s = two_level_session();
        let _ = s.execute("level", &json!({"above": 1}));
        s.execute("stair", &json!({ "start": [2.0, 1.0], "direction": 90.0, "fromLevel": 1, "toLevel": 2 })).unwrap();
        let st = &s.doc().unwrap().arch.stairs[0];
        let ys: Vec<f64> = st.well.iter().map(|v| v.y).collect();
        let (lo, hi) = (ys.iter().cloned().fold(f64::MAX, f64::min), ys.iter().cloned().fold(f64::MIN, f64::max));
        assert!((lo - 1.508).abs() < 1e-9 && (hi - 4.81).abs() < 1e-9, "{ys:?}");
        let xs: Vec<f64> = st.well.iter().map(|v| v.x).collect();
        assert!(xs.iter().all(|x| (x - 1.5).abs() < 1e-9 || (x - 2.5).abs() < 1e-9));
    }

    #[test]
    fn stair_well_becomes_a_hole_of_the_upper_room() {
        let mut s = two_level_session();
        add_big_room(&mut s, 0);
        add_big_room(&mut s, 1);
        s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": 0, "toLevel": 1 })).unwrap();
        let d = s.doc().unwrap();
        let well = d.arch.stairs[0].well.clone();
        assert!(!well.is_empty());
        let upper = d.arch.rooms.iter().find(|r| r.level == 1 && r.name == "Hall").unwrap();
        assert_eq!(upper.holes, vec![well]);
        let lower = d.arch.rooms.iter().find(|r| r.level == 0).unwrap();
        assert!(lower.holes.is_empty());
    }

    #[test]
    fn stair_well_room_area_matches_the_flight() {
        let mut s = two_level_session();
        add_big_room(&mut s, 1);
        s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": 0, "toLevel": 1 })).unwrap();
        let d = s.doc().unwrap();
        let well_area = ring_area(&d.arch.stairs[0].well);
        assert!(well_area > 0.0);
        let upper = d.arch.rooms.iter().find(|r| r.name == "Hall").unwrap();
        assert!((upper.interior_area - (400.0 - well_area)).abs() < 1e-6, "{}", upper.interior_area);
        let open = d.arch.rooms.iter().find(|r| r.room_type == RoomType::OpenBelow).unwrap();
        assert!((open.interior_area - well_area).abs() < 1e-6);
    }

    #[test]
    fn stair_command_refuses_unknown_level() {
        let mut s = new_session();
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": 0, "toLevel": 99 }));
        assert!(r.is_err());
        assert_eq!(s.doc().unwrap().arch.stairs.len(), 0);
    }

    #[test]
    fn stair_command_refuses_spiral_shape() {
        let mut s = two_level_session();
        let r = s.execute(
            "stair",
            &json!({
                "start": [0.0, 0.0],
                "direction": 90.0,
                "fromLevel": 0,
                "toLevel": 1,
                "shape": "spiral",
            }),
        );
        assert!(r.is_err());
        assert_eq!(s.doc().unwrap().arch.stairs.len(), 0);
    }

    #[test]
    fn stair_command_creates_well_room_and_guard() {
        let mut s = two_level_session();
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": 0, "toLevel": 1 })).unwrap();
        let d = s.doc().unwrap();
        assert!(r["well"].as_bool().unwrap());
        assert_eq!(d.arch.stairs.len(), 1);
        assert!(!d.arch.stairs[0].well.is_empty());
        assert_eq!(d.arch.stairs[0].well_room, Some(d.arch.rooms.last().unwrap().id));
        let room = d.arch.rooms.last().unwrap();
        assert_eq!(room.room_type, RoomType::OpenBelow);
        assert!(!room.polygon.is_empty());
        let guards: Vec<_> = d.arch.railings.iter().filter(|r| r.host == RailingHost::Well && !r.auto).collect();
        assert_eq!(guards.len(), 1);
        assert_eq!(guards[0].follows, Some(d.arch.stairs[0].id));
    }

    fn detail_stair(extra: Value) -> Value {
        let mut m = json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": 0, "toLevel": 1 });
        if let (Some(a), Some(b)) = (m.as_object_mut(), extra.as_object()) {
            for (k, v) in b {
                a.insert(k.clone(), v.clone());
            }
        }
        m
    }

    #[test]
    fn stair_accepts_the_detail_params() {
        let mut s = two_level_session();
        let r = s
            .execute(
                "stair",
                &detail_stair(json!({
                    "openRisers": true, "stringers": "Center", "underside": 0.4, "railLeft": "ON", "railRight": "off", "planCut": 1.5
                })),
            )
            .unwrap();
        assert_eq!(r["openRisers"], json!(true));
        assert_eq!(r["stringers"], json!("centre"));
        assert_eq!(r["underside"], json!(0.4));
        assert_eq!(r["railLeft"], json!("on"));
        assert_eq!(r["railRight"], json!("off"));
        assert_eq!(r["planCut"], json!(1.5));
        assert!(r["railings"].is_array());
        let st = &s.doc().unwrap().arch.stairs[0];
        assert!(st.open_risers);
        assert_eq!(st.stringers, Stringers::Centre);
        assert_eq!((st.rail_left, st.rail_right), (RailSetting::On, RailSetting::Off));
        // Defaults when absent.
        let mut s = two_level_session();
        let r = s.execute("stair", &detail_stair(json!({}))).unwrap();
        assert_eq!(r["stringers"], json!("two"));
        assert_eq!(r["railLeft"], json!("auto"));
    }

    #[test]
    fn stair_refuses_bad_detail_params() {
        for bad_params in [
            json!({"openRisers": "yes"}),
            json!({"stringers": "three"}),
            json!({"stringers": 2}),
            json!({"underside": 0.1}),
            json!({"underside": 1.5}),
            json!({"underside": "0.4"}),
            json!({"railLeft": "maybe"}),
            json!({"railRight": true}),
            json!({"planCut": 0.2}),
            json!({"planCut": 3.5}),
            json!({"planCut": 1e308}),
        ] {
            let mut s = two_level_session();
            assert!(s.execute("stair", &detail_stair(bad_params.clone())).is_err(), "{bad_params}");
            let a = &s.doc().unwrap().arch;
            assert!(a.stairs.is_empty() && a.rooms.is_empty() && a.railings.is_empty(), "{bad_params}");
        }
    }

    #[test]
    fn stair_edit_changes_only_given_fields_and_is_one_undo_step() {
        let mut s = two_level_session();
        s.execute("stair", &detail_stair(json!({"openRisers": true, "stringers": "centre"}))).unwrap();
        let r = s.execute("stair.edit", &json!({"id": "s0", "stringers": "none", "planCut": 2.0})).unwrap();
        assert_eq!(r["stair"], json!("s0"));
        assert_eq!(r["stringers"], json!("none"));
        assert_eq!(r["openRisers"], json!(true));
        assert_eq!(r["planCut"], json!(2.0));
        assert!(r["railings"].is_array());
        let st = &s.doc().unwrap().arch.stairs[0];
        assert_eq!((st.stringers, st.open_risers, st.plan_cut), (Stringers::None, true, 2.0));
        assert_eq!(s.undo().unwrap(), Some("Edit Stair".to_string()));
        let st = &s.doc().unwrap().arch.stairs[0];
        assert_eq!((st.stringers, st.open_risers, st.plan_cut), (Stringers::Centre, true, 1.2));
        // The bare number names the stair too.
        s.execute("stair.edit", &json!({"id": 0, "underside": 0.5})).unwrap();
        assert_eq!(s.doc().unwrap().arch.stairs[0].underside, 0.5);
    }

    #[test]
    fn stair_edit_refuses_unknown_ids_and_empty_edits() {
        let mut s = two_level_session();
        s.execute("stair", &detail_stair(json!({}))).unwrap();
        let before = s.doc().unwrap().arch.stairs.clone();
        for bad_params in [
            json!({"id": "s9", "stringers": "none"}),
            json!({"id": "w0", "stringers": "none"}),
            json!({"stringers": "none"}),
            json!({"id": "s0"}),
            json!({"id": "s0", "colour": "red"}),
            json!({"id": "s0", "stringers": "none", "underside": 9.0}),
        ] {
            assert!(s.execute("stair.edit", &bad_params).is_err(), "{bad_params}");
            assert_eq!(s.doc().unwrap().arch.stairs, before, "{bad_params}");
        }
    }

    #[test]
    fn stair_edit_of_a_rail_setting_keeps_drawn_railings_and_the_well_guard() {
        let mut s = two_level_session();
        s.execute("stair", &detail_stair(json!({}))).unwrap();
        s.execute("railing", &json!({"path": [[10.0, 0.0], [14.0, 0.0]]})).unwrap();
        let before: Vec<_> = s.doc().unwrap().arch.railings.iter().filter(|r| !r.auto).cloned().collect();
        assert_eq!(before.len(), 2);
        let r = s.execute("stair.edit", &json!({"id": "s0", "railLeft": "off"})).unwrap();
        assert!(r["railings"].is_array());
        let after: Vec<_> = s.doc().unwrap().arch.railings.iter().filter(|r| !r.auto).cloned().collect();
        assert_eq!(before, after);
        assert_eq!(s.doc().unwrap().arch.stairs[0].rail_left, RailSetting::Off);
    }

    #[test]
    fn stair_command_writes_one_undo_step() {
        let mut s = two_level_session();
        let _ = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": 0, "toLevel": 1 }));
        assert_eq!(s.doc().unwrap().arch.stairs.len(), 1);
        assert_eq!(s.undo().unwrap(), Some("Stair".to_string()));
        assert_eq!(s.doc().unwrap().arch.stairs.len(), 0);
    }

    #[test]
    fn stair_command_refuses_same_level() {
        let mut s = new_session();
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": 0, "toLevel": 0 }));
        assert!(r.is_err());
        assert_eq!(s.doc().unwrap().arch.stairs.len(), 0);
    }

    #[test]
    fn stair_command_explicit_rise() {
        let mut s = two_level_session();
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": 0, "toLevel": 1, "rise": 1.0 })).unwrap();
        // 1.0 m rise: n=6 gives 0.1667 m risers (closest to 0.16875 without exceeding 0.196)
        assert_eq!(r["risers"].as_u64(), Some(6));
        let riser_height = r["riserHeight"].as_f64().unwrap();
        assert!((riser_height - 1.0 / 6.0).abs() < 1e-9);
    }

    #[test]
    fn stair_command_level_by_name() {
        let mut s = two_level_session();
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": "Level 1", "toLevel": "Level 2" })).unwrap();
        assert_eq!(r["risers"].as_u64(), Some(16));
    }

    #[test]
    fn stair_command_width_zero() {
        let mut s = two_level_session();
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "width": 0.0, "fromLevel": 0, "toLevel": 1 }));
        assert!(r.is_err());
        assert_eq!(s.doc().unwrap().arch.stairs.len(), 0);
    }

    #[test]
    fn stair_command_width_too_large() {
        let mut s = two_level_session();
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "width": 5.1, "fromLevel": 0, "toLevel": 1 }));
        assert!(r.is_err());
        assert_eq!(s.doc().unwrap().arch.stairs.len(), 0);
    }

    #[test]
    fn stair_command_refuses_nan_start() {
        let mut s = two_level_session();
        let r = s.execute("stair", &json!({ "start": [f64::NAN, 0.0], "direction": 90.0, "fromLevel": 0, "toLevel": 1 }));
        assert!(r.is_err());
        assert_eq!(s.doc().unwrap().arch.stairs.len(), 0);
    }

    #[test]
    fn stair_command_null_levels_treated_as_absent() {
        let mut s = two_level_session();
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": null, "toLevel": null })).unwrap();
        assert_eq!(r["risers"].as_u64(), Some(16));
        let st = &s.doc().unwrap().arch.stairs[0];
        assert_eq!((st.from_level, st.to_level), (0, 1));
    }

    #[test]
    fn stair_command_three_levels_stair_between_first_and_last() {
        let mut s = two_level_session();
        let _ = s.execute("level", &json!({"above": 1}));
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0 })).unwrap();
        let st = &s.doc().unwrap().arch.stairs[0];
        assert_eq!((st.from_level, st.to_level), (0, 2));
        // Rise is 5.4 m (2 levels × 2.7 m); n minimises |5.4/n - 0.16875|
        // n=32 gives 0.16875 exactly
        assert_eq!(r["risers"].as_u64(), Some(32));
    }

    #[test]
    fn stair_command_well_with_no_containing_room() {
        let mut s = two_level_session();
        // No rooms on upper level — well is created but no hole is added
        s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "fromLevel": 0, "toLevel": 1 })).unwrap();
        let d = s.doc().unwrap();
        assert!(!d.arch.stairs[0].well.is_empty());
        // Open Below room exists but no other room has a hole
        let other_rooms: Vec<_> = d.arch.rooms.iter().filter(|r| r.room_type != RoomType::OpenBelow).collect();
        assert!(other_rooms.iter().all(|r| r.holes.is_empty()));
    }

    #[test]
    fn stair_command_width_boundary_five() {
        let mut s = two_level_session();
        let r = s.execute("stair", &json!({ "start": [0.0, 0.0], "direction": 90.0, "width": 5.0, "fromLevel": 0, "toLevel": 1 })).unwrap();
        assert_eq!(s.doc().unwrap().arch.stairs[0].width, 5.0);
    }
}
