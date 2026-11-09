//! The `room` command: create an architectural room from a drawn polygon.
//!
//! JSON parameters (create path):
//! - `points`: required, `[[x, y], ...]` — an explicitly closed ring (the last
//!   point repeats the first within 1e-6). After dropping the trailing duplicate,
//!   4 or more distinct points must form a simple polygon with finite area at
//!   least `MIN_ROOM_AREA`.
//! - `name`: optional, the room's name.
//! - `type`: optional, case-insensitive
//!   `living|bedroom|kitchen|bath|closet|utility|garage|porch|deck|openbelow|open-below|open below|other`.
//!   Unknown values are refused (never coerced to `Other`).
//! - `level`: optional, a level's name or its numeric index; absent means index 0.
//!   Unknown names/indices are refused (never a silent default).
//! - `ceiling` / `floor`: optional positive heights, stored as the room's
//!   ceiling height / floor height. The ceiling height is the room's ROUGH ceiling
//!   (the underside of the structure above, from the level's floor).
//! - `flatCeiling`: optional `true` (the default: a flat ceiling) or `false` (under a
//!   roof the ceiling follows the roof's underside: a cathedral or vaulted ceiling).
//!   Any other JSON value is refused.
//! - `ceilingStack`: optional finish stack under the rough ceiling, top to bottom: a
//!   template name (`cadcraft_arch::ceiling::stack::template_names`, case-insensitive)
//!   or `[{"name"?, "material": string | null (an air gap), "thickness"}, ...]`,
//!   checked by `stack::validate`. Anything else is refused.
//! - A flat ceiling that would rise above the roof underside over the room is refused
//!   (`cadcraft_arch::ceiling::vault::check_flat_ceiling`).
//!
//!
//! JSON parameters (edit path — `id` present; `points`, `level` are ignored):
//! - `id`: the room to edit — `"r12"` (case-insensitive, the leading `r` may be
//!   omitted) or a plain non-negative integer; unknown ids are refused.
//! - `name` / `type` / `ceiling` / `floor` / `flatCeiling` / `ceilingStack`: only
//!   keys present change (an absent `ceiling` or `floor` keeps the stored height,
//!   `null` clears it, a number sets it); all are validated before the model is
//!   written, and an edit with none of them is refused. An edit that raises a flat
//!   ceiling (or turns `flatCeiling` on) is refused above the roof underside; one that
//!   keeps or lowers it never is. The level's ceiling-hung items are re-seated after
//!   the edit. Areas are refreshed by the area pass.
//!
//! Returns `{"id": "r0", "handle": "...", "type": ..., "interiorArea": n,
//! "standardArea": n, "flatCeiling": bool, "ceilingStack": [{name, material,
//! thickness}]}` for create; `{"id": "r0", "name": ..., "type": ...,
//! "interiorArea": n, "standardArea": n, "flatCeiling", "ceilingStack"}` for edit.

use std::str::FromStr;

use cadcraft_arch::ceiling::{flat_ceiling_z_with, reseat, seated_ceiling_items, stack, vault};
use cadcraft_arch::poly::{ensure_ccw, is_simple, region_area, ring_area};
use cadcraft_doc::arch::{ArchId, CeilingLayer, MAX_ROOMS, MIN_ROOM_AREA, Room, RoomType};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad, distinct_points, points_param, size_param, str_param};
use crate::prompt::{Accept, Input, Interactive, Prompt, Step};
use crate::{EngineError, Result, Session};

/// Command specs for the room module.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("room", "Room", run_room)
            .menu(&["Architecture", "Room"])
            .alias(&["rm"])
            .params(
                "{points: closed-ring [[x,y],...], name?, type?, level?, ceiling?, floor?, flatCeiling?: bool, \
                 ceilingStack?: name | [{name?, material: string|null, thickness}, ...]} \
                 | {id, name?, type?, ceiling?: m|null, floor?: m|null, flatCeiling?: bool, ceilingStack?: name|[...]}",
            )
            .interactive(|_| Ok(Box::new(RoomM::default()))),
    ]
}

/// Map a case-insensitive room-type name to a [`RoomType`].
///
/// Recognised names: `living`, `bedroom`, `kitchen`, `bath`, `closet`, `utility`,
/// `garage`, `porch`, `deck`, `openbelow` / `open-below` / `open below`, and `other`.
/// Unknown strings return [`None`]; the caller refuses them rather than coercing to
/// [`RoomType::Other`].
fn parse_room_type(s: &str) -> Option<RoomType> {
    match s.trim().to_ascii_lowercase().as_str() {
        "living" => Some(RoomType::Living),
        "bedroom" => Some(RoomType::Bedroom),
        "kitchen" => Some(RoomType::Kitchen),
        "bath" => Some(RoomType::Bath),
        "closet" => Some(RoomType::Closet),
        "utility" => Some(RoomType::Utility),
        "dining" => Some(RoomType::Dining),
        "family" => Some(RoomType::Family),
        "entry" => Some(RoomType::Entry),
        "laundry" => Some(RoomType::Laundry),
        "garage" => Some(RoomType::Garage),
        "porch" => Some(RoomType::Porch),
        "deck" => Some(RoomType::Deck),
        "openbelow" | "open-below" | "open below" => Some(RoomType::OpenBelow),
        "other" => Some(RoomType::Other),
        _ => None,
    }
}

/// Resolve a level reference (a level name or its numeric index) to a position in the
/// arch model's levels. Mirrors `cmd/arch/level.rs`'s `find_level` but reads a plain `&str`.
/// An unknown name/index is an error, never a silent fallback to level 0.
fn find_room_level(arch: &cadcraft_doc::arch::ArchModel, ref_val: &str) -> Result<usize> {
    let trimmed = ref_val.trim();
    if let Some(i) = trimmed.parse::<u64>().ok() {
        if i < arch.levels.len() as u64 {
            return Ok(i as usize);
        }
    }
    if trimmed.eq_ignore_ascii_case("ground") || trimmed.eq_ignore_ascii_case("level 0") {
        if let Some(i) = arch.levels.iter().position(|l| l.index == 0) {
            return Ok(i);
        }
    }
    if let Some((i, _)) = arch.levels.iter().enumerate().find(|(_, l)| l.name.eq_ignore_ascii_case(trimmed)) {
        return Ok(i);
    }
    Err(EngineError::Other(format!("room: unknown level `{ref_val}`")))
}

/// Parse an id for the edit path: a string like `"r12"` (case-insensitive, the
/// leading `r`/`R` is stripped) or a plain non-negative integer. Anything else
/// (fractional, negative, non-finite, or not a number) is `None`.
fn parse_room_id(v: &Value) -> Option<ArchId> {
    let n64 = match v {
        Value::Number(n) => {
            let f = n.as_f64()?;
            if !(f.is_finite() && f >= 0.0 && (f % 1.0) == 0.0 && f <= u32::MAX as f64) {
                return None;
            }
            f as u64
        }
        Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return None;
            }
            let digits = if s.starts_with('r') || s.starts_with('R') { &s[1..] } else { s };
            u64::from_str(digits.trim()).ok()?
        }
        _ => return None,
    };
    Some(ArchId(n64 as u32))
}

/// The lowerCamelCase key of a [`RoomType`], matching the JSON output names
/// (`openbelow` in input maps to `openBelow` in output). Single table of type
/// keys, shared by `room_type_name` and the `room` command replies.
pub(crate) fn room_type_key(t: RoomType) -> &'static str {
    match t {
        RoomType::Living => "living",
        RoomType::Bedroom => "bedroom",
        RoomType::Kitchen => "kitchen",
        RoomType::Bath => "bath",
        RoomType::Closet => "closet",
        RoomType::Utility => "utility",
        RoomType::Dining => "dining",
        RoomType::Family => "family",
        RoomType::Entry => "entry",
        RoomType::Laundry => "laundry",
        RoomType::Garage => "garage",
        RoomType::Porch => "porch",
        RoomType::Deck => "deck",
        RoomType::OpenBelow => "openBelow",
        RoomType::Other => "other",
    }
}

/// The lowerCamelCase name of a [`RoomType`], matching the JSON output names
/// (`openbelow` in input maps to `openBelow` in output).
#[allow(dead_code)]
fn room_type_name(t: RoomType) -> &'static str {
    room_type_key(t)
}

/// `flatCeiling`: absent is `None`; `true` / `false`; any other JSON value (null included) is refused.
fn flat_ceiling_param(p: &Value) -> Result<Option<bool>> {
    match p.get("flatCeiling") {
        None => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(other) => Err(bad("room", format!("`flatCeiling` must be true or false, got `{other}`"))),
    }
}

/// `ceilingStack`: absent is `None`; a template name (case-insensitive); or a list of
/// `{"name"?, "material": string | null, "thickness"}` layers, top to bottom, that
/// `stack::validate` accepts. Anything else is refused.
fn ceiling_stack_param(p: &Value) -> Result<Option<Vec<CeilingLayer>>> {
    let v = match p.get("ceilingStack") {
        None => return Ok(None),
        Some(v) => v,
    };
    let layers = match v {
        Value::String(name) => stack::template(name)
            .ok_or_else(|| bad("room", format!("unknown ceiling stack `{name}`; known: {}", stack::template_names().join(", "))))?,
        Value::Array(items) => {
            if items.len() > stack::MAX_CEILING_LAYERS {
                return Err(bad("room", format!("a ceiling stack has at most {} layers, got {}", stack::MAX_CEILING_LAYERS, items.len())));
            }
            let mut layers = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                let n = i + 1;
                let o = item.as_object().ok_or_else(|| bad("room", format!("ceiling layer {n} must be {{\"material\", \"thickness\"}}")))?;
                let name = match o.get("name") {
                    None | Some(Value::Null) => "",
                    Some(Value::String(t)) => t.as_str(),
                    Some(_) => return Err(bad("room", format!("ceiling layer {n}: `name` must be a string"))),
                };
                let material = match o.get("material") {
                    Some(Value::Null) => None,
                    Some(Value::String(m)) => Some(m.as_str()),
                    None => return Err(bad("room", format!("ceiling layer {n}: `material` (a name, or null for an air gap) is required"))),
                    Some(_) => return Err(bad("room", format!("ceiling layer {n}: `material` must be a name or null"))),
                };
                let thickness = o
                    .get("thickness")
                    .and_then(Value::as_f64)
                    .filter(|t| t.is_finite())
                    .ok_or_else(|| bad("room", format!("ceiling layer {n}: `thickness` (metres) is required")))?;
                layers.push(CeilingLayer::new(name, material, thickness));
            }
            layers
        }
        other => {
            return Err(bad(
                "room",
                format!("`ceilingStack` must be a stack name ({}) or a list of layers, got `{other}`", stack::template_names().join(", ")),
            ));
        }
    };
    stack::validate(&layers).map_err(|m| bad("room", m))?;
    Ok(Some(layers))
}

/// An edited height: absent is `None` (keep), `null` is `Some(None)` (clear), a positive number
/// is `Some(Some(x))`; anything else is refused like `size_param`.
fn edit_height_param(p: &Value, key: &str) -> Result<Option<Option<f64>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(_) => size_param("room", p, key, false).map(Some),
    }
}

/// The ceiling stack as reply JSON: `[{name, material, thickness}]`, `material` null for an air gap.
pub(crate) fn ceiling_stack_json(stack: &[CeilingLayer]) -> Value {
    Value::Array(stack.iter().map(|l| json!({ "name": l.name, "material": l.material, "thickness": l.thickness })).collect())
}

/// Refuse a flat ceiling on `room` above the roof underside over it (no-op for a vaulted room or
/// one without a ceiling).
fn guard_flat_ceiling(arch: &cadcraft_doc::arch::ArchModel, room: &Room) -> Result<()> {
    if room.flat_ceiling
        && let Some(z) = flat_ceiling_z_with(room, arch.level(room.level))
    {
        vault::check_flat_ceiling(arch, room, z).map_err(|m| bad("room", m))?;
    }
    Ok(())
}

/// Run the `room` command from JSON parameters. Never opens a dialog.
fn run_room(s: &mut Session, p: &Value) -> Result<Value> {
    // If a level is requested but the model has no levels yet, make sure the
    // ground level exists so `find_room_level` can resolve index 0. This runs
    // before the edit path and never commits on its own (no `touch_arch`), so a
    // refused call leaves no undo step.
    if p.get("level").is_some_and(|v| !v.is_null()) && s.doc().is_ok_and(|d| d.arch.levels.is_empty()) {
        let d = s.doc_mut()?;
        d.arch.ensure_ground_level();
    }

    // --- Edit path: modify an existing room by `id`. Every field is validated
    // before the model is written (on a copy of the room, read through `doc()`, so a
    // refusal leaves no undo step), only present fields are touched, and the arch
    // revision is touched exactly once. ---
    if let Some(id_val) = p.get("id") {
        let label = id_val.to_string();
        let id = parse_room_id(id_val).ok_or_else(|| bad("room", format!("no such room `{label}`")))?;
        let name = str_param(p, "name").map(|t| t.to_string());
        let room_type = match str_param(p, "type") {
            None => None,
            Some(t) => match parse_room_type(t) {
                Some(rt) => Some(rt),
                None => return Err(bad("room", format!("unknown room type `{t}`"))),
            },
        };
        let ceiling = edit_height_param(p, "ceiling")?;
        let floor = edit_height_param(p, "floor")?;
        let flat = flat_ceiling_param(p)?;
        let ceiling_stack = ceiling_stack_param(p)?;

        let (idx, edited, seated) = {
            let arch = &s.doc()?.arch;
            let idx = arch.room_index(id).ok_or_else(|| bad("room", format!("no such room `{label}`")))?;
            let stored = arch.rooms.get(idx).ok_or_else(|| bad("room", format!("no such room `{label}`")))?;
            if name.is_none() && room_type.is_none() && ceiling.is_none() && floor.is_none() && flat.is_none() && ceiling_stack.is_none() {
                return Err(bad("room", "nothing to change"));
            }
            let mut edited = stored.clone();
            if let Some(n) = name {
                edited.name = n;
            }
            if let Some(rt) = room_type {
                edited.room_type = rt;
            }
            if let Some(c) = ceiling {
                edited.ceiling_height = c;
            }
            if let Some(f) = floor {
                edited.floor_height = f;
            }
            if let Some(f) = flat {
                edited.flat_ceiling = f;
            }
            if let Some(st) = ceiling_stack {
                edited.ceiling_stack = st;
            }
            // Only an edit that raises a flat ceiling is checked against the roof (turning
            // `flatCeiling` on, a ceiling above the stored one, or a ceiling where the stored
            // room had none), so a ceiling a roof edit left above the underside can still be
            // renamed or lowered.
            let new_z = if edited.flat_ceiling { flat_ceiling_z_with(&edited, arch.level(edited.level)) } else { None };
            if let Some(z) = new_z {
                let old_z = flat_ceiling_z_with(stored, arch.level(stored.level));
                let raises = !stored.flat_ceiling || old_z.is_none_or(|o| z > o + 1e-9);
                if raises {
                    vault::check_flat_ceiling(arch, &edited, z).map_err(|m| bad("room", m))?;
                }
            }
            let seated = seated_ceiling_items(arch, edited.level);
            (idx, edited, seated)
        };

        let d = s.doc_mut()?;
        if let Some(r) = d.arch.rooms.get_mut(idx) {
            *r = edited;
        }
        cadcraft_arch::area::refresh_areas(&mut d.arch);
        reseat(&mut d.arch, &seated);
        d.touch_arch();

        let room = d.arch.rooms.get(idx).ok_or_else(|| bad("room", format!("no such room `{label}`")))?;
        return Ok(json!({
            "id": format!("r{}", id.0),
            "name": room.name,
            "type": room_type_key(room.room_type),
            "interiorArea": room.interior_area,
            "standardArea": room.standard_area,
            "flatCeiling": room.flat_ceiling,
            "ceilingStack": ceiling_stack_json(&room.ceiling_stack),
        }));
    }

    let d = s.doc()?;
    let arch = &d.arch;

    // --- Points: required, distinct, explicitly closed, >= 4, finite, simple,
    // CCW-normalised, finite area above MIN_ROOM_AREA ---
    let pts = points_param(p, "points").ok_or_else(|| bad("room", "`points` [[x,y],...] (closed ring, 4 or more) is required"))?;
    let pts = distinct_points(&pts);
    // The ring must be explicitly closed: the last point repeats the first.
    if pts.len() < 2 || !pts[pts.len() - 1].near(pts[0], 1e-6) {
        return Err(bad("room", "points must be a closed ring: repeat the first point at the end"));
    }
    // Drop the trailing duplicate.
    let ring: Vec<Vec2> = pts[..pts.len() - 1].to_vec();
    if ring.len() < 4 {
        return Err(bad("room", "`points` [[x,y],...] must have 4 or more distinct points"));
    }
    for pt in &ring {
        if !pt.is_finite() {
            return Err(bad("room", "non-finite coordinate in `points`"));
        }
    }
    if !is_simple(&ring) {
        return Err(bad("room", "`points` do not form a simple polygon (self-intersecting or zero area)"));
    }
    let ring = ensure_ccw(&ring);
    // Reject non-finite or too-small areas (e.g. 1e308 corners).
    let area = ring_area(&ring);
    if !area.is_finite() || area < MIN_ROOM_AREA {
        return Err(bad("room", "`points` do not enclose a valid room area"));
    }

    // --- Level: name or index, else 0; unknown is an error, never a silent default ---
    // `level` must be a string (a level name or a numeric index string). A present
    // non‑string (e.g. a bare JSON number) is malformed, not "absent".
    let level = match p.get("level") {
        None => 0,
        Some(Value::Null) => 0,
        Some(Value::String(n)) => find_room_level(arch, n)?,
        Some(other) => return Err(bad("room", format!("`level` must be a string (a level name or index), got `{other}`"))),
    };

    // --- Type: known names only; unknown is refused, absent is Other ---
    let room_type = match str_param(p, "type") {
        None => RoomType::default(),
        Some(t) => match parse_room_type(t) {
            Some(rt) => rt,
            None => return Err(bad("room", format!("unknown room type `{t}`"))),
        },
    };

    // --- Ceiling / floor: optional, positive, finite (refused otherwise) ---
    let ceiling = size_param("room", p, "ceiling", false)?;
    let floor = size_param("room", p, "floor", false)?;
    let flat_ceiling = flat_ceiling_param(p)?.unwrap_or(true);
    let ceiling_stack = ceiling_stack_param(p)?.unwrap_or_default();

    // --- Name ---
    let name = str_param(p, "name").map(|s| s.to_string()).unwrap_or_default();

    // --- Areas ---
    let interior_area = region_area(&ring, &[]);

    // --- Cap, then build and push (all checks above ran before touching the model) ---
    if d.arch.rooms.len() >= MAX_ROOMS {
        return Err(bad("room", "room limit reached"));
    }
    // The standard area is measured against the surrounding walls, so the room must
    // first sit in the model. `standard_area` only reads `arch`; that immutable
    // borrow ends before `push_room` mutates the drawing.
    let mut room = Room {
        name,
        room_type,
        level: level as u32,
        polygon: ring,
        ceiling_height: ceiling,
        floor_height: floor,
        interior_area,
        standard_area: 0.0,
        flat_ceiling,
        ceiling_stack,
        ..Default::default()
    };
    let standard_area = cadcraft_arch::area::standard_area(arch, &room);
    room.standard_area = standard_area;
    guard_flat_ceiling(arch, &room)?;
    let stack_json = ceiling_stack_json(&room.ceiling_stack);
    let d = s.doc_mut()?;
    let id = d.arch.push_room(room);
    cadcraft_arch::area::refresh_areas(&mut d.arch);
    d.touch_arch();

    Ok(json!({
        "id": format!("r{}", id.0),
        "handle": cadcraft_doc::arch::handle_json(cadcraft_arch::room_handle(id)),
        "type": room_type_key(room_type),
        "interiorArea": interior_area,
        "standardArea": standard_area,
        "flatCeiling": flat_ceiling,
        "ceilingStack": stack_json,
    }))
}

/// Interactive `room` machine: collect points for a room polygon, then finish with the
/// `Close` keyword or an empty Enter. The machine delegates to the JSON `room` command
/// on finish so there is exactly one implementation path and one undo step.
pub struct RoomM {
    pts: Vec<Vec2>,
    base: Option<Vec2>,
}

impl Default for RoomM {
    fn default() -> Self {
        Self { pts: Vec::new(), base: None }
    }
}

impl Interactive for RoomM {
    fn name(&self) -> &'static str {
        "room"
    }

    fn prompt(&self, _s: &Session) -> Prompt {
        let message = if self.pts.is_empty() { "Specify start point:".to_string() } else { "Specify next point or [Close]:".to_string() };
        let keywords = if self.pts.is_empty() { Vec::new() } else { vec!["Close".to_string()] };
        Prompt {
            message,
            keywords,
            default: None,
            accept: Accept::POINT,
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
        let closes = match i {
            Input::Enter => true,
            Input::Keyword(_) => true,
            Input::Text(ref txt) => {
                let l = txt.trim().to_ascii_lowercase();
                l.is_empty() || l == "close"
            }
            _ => false,
        };
        if closes {
            // Fewer than 3 points: re-prompt instead of erroring (never a model change).
            if self.pts.len() < 3 {
                return Ok(Step::Continue);
            }
            let params = self.build_params();
            return s.execute("room", &params).map(|_| Step::Done);
        }
        match i {
            Input::Point(pt) => {
                if !pt.is_finite() {
                    return Ok(Step::Continue);
                }
                self.pts.push(pt);
                self.base = Some(pt);
                Ok(Step::Continue)
            }
            Input::Cancel => Ok(Step::Cancel),
            _ => Ok(Step::Continue),
        }
    }
}

impl RoomM {
    /// Build the JSON parameter object the JSON `room` command accepts. Only the fields the
    /// interactive form set (points) are emitted.
    fn build_params(&self) -> Value {
        let points: Vec<Value> = self.pts.iter().map(|p| json!([p.x, p.y])).collect();
        json!({ "points": points })
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

    /// A 3 x 3 m closed square (area 9.0).
    fn square_points() -> Value {
        json!([[0.0, 0.0], [3.0, 0.0], [3.0, 3.0], [0.0, 3.0], [0.0, 0.0]])
    }

    #[test]
    fn room_command_creates_a_room() {
        let mut s = new_session();
        let before = s.doc().unwrap().arch.rooms.len();
        let p = json!({ "points": square_points() });
        let r = s.execute("room", &p).unwrap();
        let d = s.doc().unwrap();
        assert_eq!(d.arch.rooms.len(), before + 1);
        assert_eq!(r["id"].as_str().unwrap(), "r0");
        assert_eq!(r["interiorArea"].as_f64().unwrap(), 9.0);
        assert_eq!(r["standardArea"].as_f64().unwrap(), 9.0);
        assert_eq!(r["handle"], "a0010000");
        assert_eq!(d.arch.room(d.arch.rooms.last().unwrap().id).unwrap().polygon.len(), 4);
        assert_eq!(d.arch.room(d.arch.rooms.last().unwrap().id).unwrap().level, 0);
        assert_eq!(d.arch.room(d.arch.rooms.last().unwrap().id).unwrap().room_type, RoomType::Other);
    }

    /// The new room types ("dining", "family", "entry", "laundry") parse and
    /// report by key; parsing is trim- and case-insensitive; "attic" is
    /// refused.
    #[test]
    fn new_room_types_parse() {
        let types = [("dining", RoomType::Dining), ("family", RoomType::Family), ("entry", RoomType::Entry), ("laundry", RoomType::Laundry)];
        for (input, want) in types {
            assert_eq!(parse_room_type(input), Some(want));
            assert_eq!(room_type_key(want), input);
            let mut s = new_session();
            let r = s.execute("room", &json!({ "points": square_points(), "type": input })).unwrap();
            assert_eq!(r["type"].as_str().unwrap(), input);
            assert_eq!(s.doc().unwrap().arch.rooms[0].room_type, want);
        }
        // Trim + case-insensitive.
        assert_eq!(parse_room_type("DINING"), Some(RoomType::Dining));
        assert_eq!(parse_room_type(" dining "), Some(RoomType::Dining));
        let mut s = new_session();
        let r = s.execute("room", &json!({ "points": square_points(), "type": " dining " })).unwrap();
        assert_eq!(r["type"].as_str().unwrap(), "dining");
        assert_eq!(s.doc().unwrap().arch.rooms[0].room_type, RoomType::Dining);
        // "attic" is refused.
        let mut s = new_session();
        let r = s.execute("room", &json!({ "points": square_points(), "type": "attic" }));
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    #[test]
    fn room_rejects_an_unknown_type() {
        let mut s = new_session();
        let p = json!({
            "points": square_points(),
            "type": "bogus",
        });
        let r = s.execute("room", &p);
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    #[test]
    fn room_rejects_an_unclosed_polygon() {
        // A self-intersecting "polygon" is not a valid closed region.
        let mut s = new_session();
        let p = json!({ "points": [[0.0, 0.0], [2.0, 2.0], [2.0, 0.0], [0.0, 2.0]] });
        let r = s.execute("room", &p);
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);

        // And a 2-point "polygon" (after implicit closure) is also rejected.
        let p = json!({ "points": [[0.0, 0.0], [3.0, 0.0]] });
        assert!(s.execute("room", &p).is_err());
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    #[test]
    fn room_command_is_undoable() {
        let mut s = new_session();
        let before = s.doc().unwrap().arch.rooms.len();
        let p = json!({ "points": square_points() });
        let _ = s.execute("room", &p).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms.len(), before + 1);
        assert_eq!(s.undo().unwrap(), Some("Room".to_string()));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), before);
    }

    #[test]
    fn room_command_never_panics_on_garbage() {
        let mut s = new_session();
        let garbage = [
            json!({}),
            json!({"points": "nope"}),
            json!({"points": []}),
            json!({"points": [[1.0, 1.0], [1.0, 1.0], [2.0, 2.0]]}),
            json!({"points": [[f64::NAN, 0.0], [1.0, 0.0], [1.0, 1.0]]}),
            json!({"points": [[0.0, 0.0], [2.0, 2.0], [2.0, 0.0], [0.0, 2.0]]}),
            json!({"id": "r0"}),
            json!({"points": square_points(), "level": "nope"}),
            json!({"points": square_points(), "level": 7}),
        ];
        for g in garbage {
            let r = s.execute("room", &g);
            assert!(r.is_err(), "garbage `{g}` should be refused");
            assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
        }
    }

    /// Edit `type` on an existing room; the id is accepted, the response reports
    /// the lowerCamelCase type name, and untouched fields (name/area) are intact.
    #[test]
    fn room_edit_changes_the_type() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points(), "name": "Old" })).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms[0].room_type, RoomType::Other);
        let r = s.execute("room", &json!({ "id": "r0", "type": "kitchen" })).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms[0].room_type, RoomType::Kitchen);
        assert_eq!(s.doc().unwrap().arch.rooms[0].name, "Old");
        assert_eq!(r["id"].as_str().unwrap(), "r0");
        assert_eq!(r["type"].as_str().unwrap(), "kitchen");
        assert_eq!(r["name"].as_str().unwrap(), "Old");
        assert_eq!(r["interiorArea"].as_f64().unwrap(), 9.0);
    }

    /// An unknown or malformed id is refused and the model is untouched.
    #[test]
    fn room_edit_rejects_unknown_id() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points() })).unwrap();
        let r = s.execute("room", &json!({ "id": "r999", "type": "kitchen" }));
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        // Malformed / unparseable ids are also refused.
        for id in [json!("r"), json!("r-1"), json!("1e3"), json!(7.5), json!(-3), json!(true), json!(null)] {
            let r = s.execute("room", &json!({ "id": id, "type": "kitchen" }));
            assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        }
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 1);
        assert_eq!(s.doc().unwrap().arch.rooms[0].room_type, RoomType::Other);
    }

    /// An unknown type is refused, never coerced to Other, and the room is
    /// otherwise untouched.
    #[test]
    fn room_edit_rejects_unknown_type() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points() })).unwrap();
        let r = s.execute("room", &json!({ "id": "r0", "type": "not-a-type" }));
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms[0].room_type, RoomType::Other);
    }

    /// A bad `ceiling` (zero, negative, non-number) is refused without mutating;
    /// a valid edit still lands afterwards.
    #[test]
    fn room_edit_rejects_a_bad_ceiling() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points() })).unwrap();
        for c in [json!(0.0), json!(-0.5), json!("tall"), json!(true)] {
            let r = s.execute("room", &json!({ "id": "r0", "ceiling": c }));
            assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
            assert_eq!(s.doc().unwrap().arch.rooms[0].ceiling_height, None);
        }
        // A valid edit still works after the rejections.
        let r = s.execute("room", &json!({ "id": "r0", "ceiling": 3.0 })).unwrap();
        assert_eq!(r["interiorArea"].as_f64().unwrap(), 9.0);
        assert_eq!(s.doc().unwrap().arch.rooms[0].ceiling_height, Some(3.0));
        assert_eq!(s.doc().unwrap().arch.rooms[0].floor_height, None);
    }

    /// An edit is a single undoable step: undo restores the pre-edit state of
    /// every field.
    #[test]
    fn room_edit_is_one_undo_step() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points() })).unwrap();
        let _ = s.execute("room", &json!({ "id": "r0", "name": "Kitchen", "type": "kitchen", "ceiling": 3.0 })).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms[0].name, "Kitchen");
        // One undo brings us back to the created (pre-edit) state.
        assert_eq!(s.undo().unwrap(), Some("Room".to_string()));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 1);
        assert_eq!(s.doc().unwrap().arch.rooms[0].name, "");
        assert_eq!(s.doc().unwrap().arch.rooms[0].room_type, RoomType::Other);
        assert_eq!(s.doc().unwrap().arch.rooms[0].ceiling_height, None);
    }

    /// The south room's standard area is measured against the host walls'
    /// reference lines (outer face for exterior, centre line for interior), so it
    /// exceeds the interior area.
    #[test]
    fn room_standard_area_follows_wall_reference_lines() {
        let mut s = new_session();

        // Phase-1 house: four exterior walls (thickness 0.2), two interior
        // walls (thickness 0.1, `exterior: false`).
        let ext: f64 = 0.2;
        for w in [
            json!({ "points": [[0.0, 0.0], [10.0, 0.0]], "thickness": ext }),
            json!({ "points": [[10.0, 0.0], [10.0, 8.0]], "thickness": ext }),
            json!({ "points": [[10.0, 8.0], [0.0, 8.0]], "thickness": ext }),
            json!({ "points": [[0.0, 8.0], [0.0, 0.0]], "thickness": ext }),
            json!({ "points": [[0.0, 3.0], [10.0, 3.0]], "thickness": 0.1, "exterior": false }),
            json!({ "points": [[6.0, 3.0], [6.0, 8.0]], "thickness": 0.1, "exterior": false }),
        ] {
            let _ = s.execute("wall", &w).unwrap();
        }

        // The south room: 9.8 wide x 2.85 deep = 27.93 interior.
        let p = json!({
            "points": [[0.1, 0.1], [9.9, 0.1], [9.9, 2.95], [0.1, 2.95], [0.1, 0.1]],
        });
        let r = s.execute("room", &p).unwrap();
        assert!((r["interiorArea"].as_f64().unwrap() - 27.93).abs() < 1e-3);
        // Exterior faces pushed to -0.1 / 10.1, interior wall to its centre line
        // at y = 3.0 -> 10.2 x 3.1 = 31.62.
        assert!((r["standardArea"].as_f64().unwrap() - 31.62).abs() < 1e-3);
    }

    /// With no host walls the offset is degenerate, so the standard area falls
    /// back to the interior area.
    #[test]
    fn room_standard_area_falls_back_without_walls() {
        let mut s = new_session();
        let r = s.execute("room", &json!({ "points": square_points() })).unwrap();
        let interior = r["interiorArea"].as_f64().unwrap();
        let standard = r["standardArea"].as_f64().unwrap();
        assert!((standard - interior).abs() < 1e-9);
        assert!((interior - 9.0).abs() < 1e-9);
    }

    /// Two rooms sharing an interior wall each get their standard area measured
    /// to the wall's centre line, not to each other. Uses the phase-1 house
    /// geometry with the documented reference values.
    #[test]
    fn room_standard_area_with_two_adjacent_rooms() {
        let mut s = new_session();

        // Phase-1 house walls
        let ext: f64 = 0.2;
        for w in [
            json!({ "points": [[0.0, 0.0], [10.0, 0.0]], "thickness": ext }),
            json!({ "points": [[10.0, 0.0], [10.0, 8.0]], "thickness": ext }),
            json!({ "points": [[10.0, 8.0], [0.0, 8.0]], "thickness": ext }),
            json!({ "points": [[0.0, 8.0], [0.0, 0.0]], "thickness": ext }),
            json!({ "points": [[0.0, 3.0], [10.0, 3.0]], "thickness": 0.1, "exterior": false }),
            json!({ "points": [[6.0, 3.0], [6.0, 8.0]], "thickness": 0.1, "exterior": false }),
        ] {
            let _ = s.execute("wall", &w).unwrap();
        }

        // South room: interior 9.8 x 2.85 = 27.93, standard 10.2 x 3.1 = 31.62
        let south = s
            .execute(
                "room",
                &json!({
                    "points": [[0.1, 0.1], [9.9, 0.1], [9.9, 2.95], [0.1, 2.95], [0.1, 0.1]],
                }),
            )
            .unwrap();
        assert!((south["interiorArea"].as_f64().unwrap() - 27.93).abs() < 1e-3);
        assert!((south["standardArea"].as_f64().unwrap() - 31.62).abs() < 1e-3);

        // North-west room: interior 5.85 x 4.85 = 28.3725, standard 6.1 x 5.1 = 31.11
        let nw = s
            .execute(
                "room",
                &json!({
                    "points": [[0.1, 3.05], [5.95, 3.05], [5.95, 7.9], [0.1, 7.9], [0.1, 3.05]],
                }),
            )
            .unwrap();
        assert!((nw["interiorArea"].as_f64().unwrap() - 28.3725).abs() < 1e-3);
        assert!((nw["standardArea"].as_f64().unwrap() - 31.11).abs() < 1e-3);
    }

    /// The standard area is always at least as large as the interior area,
    /// because it is measured to the walls' reference lines which lie outside
    /// (or on) the room's inner edges.
    #[test]
    fn room_standard_area_is_at_least_interior_area() {
        let mut s = new_session();

        // A room with walls on all four sides
        for w in [
            json!({ "points": [[0.0, 0.0], [5.0, 0.0]], "thickness": 0.2 }),
            json!({ "points": [[5.0, 0.0], [5.0, 4.0]], "thickness": 0.2 }),
            json!({ "points": [[5.0, 4.0], [0.0, 4.0]], "thickness": 0.2 }),
            json!({ "points": [[0.0, 4.0], [0.0, 0.0]], "thickness": 0.2 }),
        ] {
            let _ = s.execute("wall", &w).unwrap();
        }

        let r = s
            .execute(
                "room",
                &json!({
                    "points": [[0.1, 0.1], [4.9, 0.1], [4.9, 3.9], [0.1, 3.9], [0.1, 0.1]],
                }),
            )
            .unwrap();

        let interior = r["interiorArea"].as_f64().unwrap();
        let standard = r["standardArea"].as_f64().unwrap();
        assert!(standard >= interior - 1e-9, "standard {} < interior {}", standard, interior);
    }

    /// Editing a room's type (which triggers `refresh_areas`) does not change
    /// its standard area, confirming the create and edit paths agree on the
    /// computed value.
    #[test]
    fn room_standard_area_unchanged_by_type_edit() {
        let mut s = new_session();

        let _ = s
            .execute(
                "room",
                &json!({
                    "points": square_points(),
                    "name": "Test",
                }),
            )
            .unwrap();

        let before = s.doc().unwrap().arch.rooms[0].standard_area;

        let _ = s
            .execute(
                "room",
                &json!({
                    "id": "r0",
                    "type": "kitchen",
                }),
            )
            .unwrap();

        let after = s.doc().unwrap().arch.rooms[0].standard_area;
        assert!((before - after).abs() < 1e-9, "standard area changed from {} to {}", before, after);
    }

    /// A non-closed point list (3 points, last != first) is refused with the
    /// explicit-closure error, and the model is untouched.
    #[test]
    fn room_requires_an_explicitly_closed_ring() {
        let mut s = new_session();
        // Three points that form a triangle without repeating the first at the end.
        let p = json!({ "points": [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]] });
        let r = s.execute("room", &p);
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    /// A closed triangle (4 points, last == first) only yields 3 distinct
    /// vertices after dropping the duplicate, so it is refused.
    #[test]
    fn room_rejects_a_closed_triangle() {
        let mut s = new_session();
        // Closed triangle: 4 points where the last repeats the first.
        let p = json!({ "points": [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]] });
        let r = s.execute("room", &p);
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    /// Coordinates of magnitude 1e308 make the area computation overflow to
    /// infinity; such a ring is refused before any model write.
    #[test]
    fn room_rejects_non_finite_area() {
        let mut s = new_session();
        let big = 1e308f64;
        let p = json!({
            "points": [[0.0, 0.0], [big, 0.0], [big, big], [0.0, big], [0.0, 0.0]],
        });
        let r = s.execute("room", &p);
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    /// The new explicit-closure requirement still accepts a valid closed
    /// rectangle, so normal creation continues to work.
    #[test]
    fn room_create_still_accepts_a_closed_rectangle() {
        let mut s = new_session();
        let p = json!({
            "points": [[0.0, 0.0], [3.0, 0.0], [3.0, 3.0], [0.0, 3.0], [0.0, 0.0]],
        });
        let r = s.execute("room", &p).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 1);
        assert_eq!(r["id"].as_str().unwrap(), "r0");
        assert_eq!(r["interiorArea"].as_f64().unwrap(), 9.0);
        assert_eq!(r["standardArea"].as_f64().unwrap(), 9.0);
    }

    /// `room` create path stores both interior and standard areas on the model.
    #[test]
    fn room_create_stores_areas() {
        let mut s = new_session();
        let p = json!({ "points": square_points() });
        let r = s.execute("room", &p).unwrap();
        let d = s.doc().unwrap();
        assert_eq!(d.arch.rooms.len(), 1);
        assert!(d.arch.rooms[0].interior_area > 0.0, "stored interior_area should be > 0");
        assert!(d.arch.rooms[0].standard_area > 0.0, "stored standard_area should be > 0");
        assert!((d.arch.rooms[0].interior_area - r["interiorArea"].as_f64().unwrap()).abs() < 1e-9);
        assert!((d.arch.rooms[0].standard_area - r["standardArea"].as_f64().unwrap()).abs() < 1e-9);
    }

    /// `room` edit path refreshes stored areas via `refresh_areas`.
    #[test]
    fn room_edit_refreshes_stored_areas() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points() })).unwrap();
        let _ = s
            .execute(
                "wall",
                &json!({
                    "points": [[0.0, -0.1], [3.0, -0.1]],
                    "thickness": 0.2,
                    "exterior": true,
                }),
            )
            .unwrap();
        let r = s.execute("room", &json!({ "id": "r0", "name": "Test" })).unwrap();
        let d = s.doc().unwrap();
        assert!(d.arch.rooms[0].standard_area > 0.0, "stored standard_area should be > 0 after refresh, got {}", d.arch.rooms[0].standard_area);
        assert!(
            (d.arch.rooms[0].standard_area - r["standardArea"].as_f64().unwrap()).abs() < 1e-9,
            "stored standard_area should match the reported standardArea"
        );
    }

    /// A closed ring with area below MIN_ROOM_AREA (0.05) is refused.
    #[test]
    fn room_rejects_tiny_room_below_min_area() {
        let mut s = new_session();
        // 0.1 x 0.1 m square = 0.01 m², below MIN_ROOM_AREA of 0.05.
        let p = json!({
            "points": [[0.0, 0.0], [0.1, 0.0], [0.1, 0.1], [0.0, 0.1], [0.0, 0.0]],
        });
        let r = s.execute("room", &p);
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    /// A closed ring with a NaN coordinate is refused.
    #[test]
    fn room_rejects_nan_in_closed_ring() {
        let mut s = new_session();
        let p = json!({
            "points": [[f64::NAN, 0.0], [3.0, 0.0], [3.0, 3.0], [0.0, 3.0], [0.0, 0.0]],
        });
        let r = s.execute("room", &p);
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    /// A closed ring with an infinity coordinate is refused.
    #[test]
    fn room_rejects_infinity_in_closed_ring() {
        let mut s = new_session();
        let p = json!({
            "points": [[f64::INFINITY, 0.0], [3.0, 0.0], [3.0, 3.0], [0.0, 3.0], [0.0, 0.0]],
        });
        let r = s.execute("room", &p);
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    /// A bare JSON number for `level` is refused (must be a string).
    #[test]
    fn room_rejects_level_as_bare_number() {
        let mut s = new_session();
        let p = json!({
            "points": square_points(),
            "level": 1,
        });
        let r = s.execute("room", &p);
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    /// `level: null` is treated as absent and defaults to level 0.
    #[test]
    fn room_accepts_level_as_null() {
        let mut s = new_session();
        let p = json!({
            "points": square_points(),
            "level": null,
        });
        let r = s.execute("room", &p).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 1);
        assert_eq!(r["id"].as_str().unwrap(), "r0");
        assert_eq!(s.doc().unwrap().arch.rooms[0].level, 0);
    }

    /// A rectangle with an extra collinear point on one edge is accepted
    /// (still a simple polygon).
    #[test]
    fn room_accepts_collinear_extra_point() {
        let mut s = new_session();
        // Rectangle with an extra point at (1.5, 0) on the bottom edge.
        let p = json!({
            "points": [[0.0, 0.0], [1.5, 0.0], [3.0, 0.0], [3.0, 3.0], [0.0, 3.0], [0.0, 0.0]],
        });
        let r = s.execute("room", &p).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 1);
        assert_eq!(r["id"].as_str().unwrap(), "r0");
        assert_eq!(r["interiorArea"].as_f64().unwrap(), 9.0);
    }

    /// All points collinear (zero area) is refused.
    #[test]
    fn room_rejects_collinear_degenerate_polygon() {
        let mut s = new_session();
        // Five points all on the x-axis, closed ring.
        let p = json!({
            "points": [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0], [0.0, 0.0]],
        });
        let r = s.execute("room", &p);
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 0);
    }

    /// Consecutive duplicate points in the middle of the ring are handled
    /// by `distinct_points` and the room is created.
    #[test]
    fn room_accepts_consecutive_duplicate_points() {
        let mut s = new_session();
        // Duplicate (3.0, 0.0) in the middle.
        let p = json!({
            "points": [[0.0, 0.0], [3.0, 0.0], [3.0, 0.0], [3.0, 3.0], [0.0, 3.0], [0.0, 0.0]],
        });
        let r = s.execute("room", &p).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms.len(), 1);
        assert_eq!(r["id"].as_str().unwrap(), "r0");
        assert_eq!(r["interiorArea"].as_f64().unwrap(), 9.0);
    }

    /// Editing multiple fields at once (name, type, ceiling, floor) applies all
    /// of them in a single undo step.
    #[test]
    fn room_edit_multiple_fields_at_once() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points() })).unwrap();
        let r = s
            .execute(
                "room",
                &json!({
                    "id": "r0",
                    "name": "Master Bedroom",
                    "type": "bedroom",
                    "ceiling": 3.2,
                    "floor": 0.15,
                }),
            )
            .unwrap();
        let room = &s.doc().unwrap().arch.rooms[0];
        assert_eq!(room.name, "Master Bedroom");
        assert_eq!(room.room_type, RoomType::Bedroom);
        assert_eq!(room.ceiling_height, Some(3.2));
        assert_eq!(room.floor_height, Some(0.15));
        assert_eq!(r["name"].as_str().unwrap(), "Master Bedroom");
        assert_eq!(r["type"].as_str().unwrap(), "bedroom");
    }

    /// The edit path accepts a plain numeric id (no "r" prefix) and an
    /// uppercase "R" prefix.
    #[test]
    fn room_edit_accepts_numeric_and_uppercase_ids() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points() })).unwrap();
        // Plain number.
        let r = s.execute("room", &json!({ "id": 0, "name": "By number" })).unwrap();
        assert_eq!(r["name"].as_str().unwrap(), "By number");
        // Uppercase R prefix.
        let r = s.execute("room", &json!({ "id": "R0", "name": "By uppercase" })).unwrap();
        assert_eq!(r["name"].as_str().unwrap(), "By uppercase");
        // String without r prefix.
        let r = s.execute("room", &json!({ "id": "0", "name": "By string" })).unwrap();
        assert_eq!(r["name"].as_str().unwrap(), "By string");
    }

    /// An edit with an id but no other fields is refused as "nothing to change"
    /// and leaves the model untouched.
    #[test]
    fn room_edit_rejects_nothing_to_change() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points() })).unwrap();
        let r = s.execute("room", &json!({ "id": "r0" }));
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
        assert_eq!(s.doc().unwrap().arch.rooms[0].name, "");
    }

    /// The edit path validates `floor` the same way as `ceiling`: positive,
    /// finite, a number.
    #[test]
    fn room_edit_rejects_a_bad_floor() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points() })).unwrap();
        for f in [json!(0.0), json!(-1.0), json!("thick"), json!(true)] {
            let r = s.execute("room", &json!({ "id": "r0", "floor": f }));
            assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
            assert_eq!(s.doc().unwrap().arch.rooms[0].floor_height, None);
        }
        let r = s.execute("room", &json!({ "id": "r0", "floor": 0.2 })).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms[0].floor_height, Some(0.2));
        assert_eq!(r["interiorArea"].as_f64().unwrap(), 9.0);
    }

    /// Editing a room's properties does not change its polygon, level, or holes.
    #[test]
    fn room_edit_preserves_geometry() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points() })).unwrap();
        let before_polygon = s.doc().unwrap().arch.rooms[0].polygon.clone();
        let before_level = s.doc().unwrap().arch.rooms[0].level;
        let before_holes = s.doc().unwrap().arch.rooms[0].holes.clone();
        let _ = s
            .execute(
                "room",
                &json!({
                    "id": "r0",
                    "name": "Changed",
                    "type": "kitchen",
                    "ceiling": 2.8,
                    "floor": 0.1,
                }),
            )
            .unwrap();
        let after = &s.doc().unwrap().arch.rooms[0];
        assert_eq!(after.polygon, before_polygon);
        assert_eq!(after.level, before_level);
        assert_eq!(after.holes, before_holes);
        assert_eq!(after.name, "Changed");
        assert_eq!(after.room_type, RoomType::Kitchen);
    }

    /// An empty string is a valid room name on the edit path.
    #[test]
    fn room_edit_accepts_empty_name() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points(), "name": "Original" })).unwrap();
        let r = s.execute("room", &json!({ "id": "r0", "name": "" })).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms[0].name, "");
        assert_eq!(r["name"].as_str().unwrap(), "");
    }

    /// Non-finite ceiling values (NaN, ±Infinity) are refused on the edit path.
    #[test]
    fn room_edit_rejects_non_finite_ceiling() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points(), "ceiling": 3.0 })).unwrap();
        // JSON has no NaN or infinity: `json!` turns them into null, which (phase 16) clears the
        // room's own ceiling height. Spelled-out non-numbers are refused and change nothing.
        for c in [json!("NaN"), json!("inf"), json!([1.0])] {
            let r = s.execute("room", &json!({ "id": "r0", "ceiling": c }));
            assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));
            assert_eq!(s.doc().unwrap().arch.rooms[0].ceiling_height, Some(3.0));
        }
        for c in [json!(f64::NAN), json!(f64::INFINITY), json!(-f64::INFINITY)] {
            assert!(c.is_null());
            s.execute("room", &json!({ "id": "r0", "ceiling": c })).unwrap();
            assert_eq!(s.doc().unwrap().arch.rooms[0].ceiling_height, None);
        }
    }

    /// The edit path ignores `level` and `points` (they are create-only fields).
    #[test]
    fn room_edit_ignores_level_and_points() {
        let mut s = new_session();
        let _ = s.execute("room", &json!({ "points": square_points() })).unwrap();
        let before_polygon = s.doc().unwrap().arch.rooms[0].polygon.clone();
        let before_level = s.doc().unwrap().arch.rooms[0].level;
        let r = s
            .execute(
                "room",
                &json!({
                    "id": "r0",
                    "name": "Changed",
                    "level": "1",
                    "points": [[0.0, 0.0], [5.0, 0.0], [5.0, 5.0], [0.0, 5.0], [0.0, 0.0]],
                }),
            )
            .unwrap();
        let after = &s.doc().unwrap().arch.rooms[0];
        assert_eq!(after.name, "Changed");
        assert_eq!(after.level, before_level);
        assert_eq!(after.polygon, before_polygon);
        assert_eq!(r["name"].as_str().unwrap(), "Changed");
    }

    /// The phase-1 house's NE room alone: r0 = 6.05..9.9 x 3.05..7.9 on level 0.
    fn ne_room() -> Value {
        json!([[6.05, 3.05], [9.9, 3.05], [9.9, 7.9], [6.05, 7.9], [6.05, 3.05]])
    }

    fn suspended() -> Value {
        json!([
            {"name": "drywall", "material": "drywall", "thickness": 0.0127},
            {"name": "air", "material": null, "thickness": 0.2},
            {"material": "ceiling-tile", "thickness": 0.016}
        ])
    }

    #[test]
    fn room_accepts_flat_ceiling_and_stack() {
        let mut s = new_session();
        // Create with both keys.
        s.execute("room", &json!({ "points": ne_room(), "flatCeiling": false, "ceilingStack": suspended() })).unwrap();
        let r = s.doc().unwrap().arch.rooms[0].clone();
        assert!(!r.flat_ceiling);
        assert_eq!(r.ceiling_stack.len(), 3);
        assert_eq!(r.ceiling_stack[1].material, None);
        assert_eq!(r.ceiling_stack[2].name, "");
        assert_eq!(r.ceiling_stack[2].material.as_deref(), Some("ceiling-tile"));
        assert!((cadcraft_arch::ceiling::finished_ceiling_at(&s.doc().unwrap().arch, 0, Vec2::new(8.0, 5.0)).unwrap() - 2.1713).abs() < 1e-9);
        // Edit: a template name (case-insensitive) and the flag back on.
        s.execute("room", &json!({ "id": "r0", "ceilingStack": " NONE ", "flatCeiling": true })).unwrap();
        let r = s.doc().unwrap().arch.rooms[0].clone();
        assert!(r.flat_ceiling && r.ceiling_stack.is_empty());
        // Each key alone is a change.
        s.execute("room", &json!({ "id": "r0", "flatCeiling": false })).unwrap();
        assert!(!s.doc().unwrap().arch.rooms[0].flat_ceiling);
        s.execute("room", &json!({ "id": "r0", "ceilingStack": [{"material": "drywall", "thickness": 0.0127}] })).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms[0].ceiling_stack.len(), 1);
        // A fresh room is flat with no stack.
        s.execute("room", &json!({ "points": square_points() })).unwrap();
        let r = s.doc().unwrap().arch.rooms[1].clone();
        assert!(r.flat_ceiling && r.ceiling_stack.is_empty());
    }

    #[test]
    fn room_refuses_bad_ceiling_params() {
        let mut s = new_session();
        s.execute("room", &json!({ "points": ne_room() })).unwrap();
        let rooms_before = s.doc().unwrap().arch.rooms.clone();
        let nine: Vec<Value> = (0..9).map(|_| json!({"material": "drywall", "thickness": 0.01})).collect();
        let bad_flags = [json!("no"), json!(1), json!(null), json!([true])];
        let bad_stacks = [
            json!("plaster"),
            json!([{"material": "drywall", "thickness": -0.01}]),
            json!([{"material": null, "thickness": 0.1}]),
            json!([{"material": "drywall", "thickness": 1.5}]),
            json!(nine),
            json!({"material": "drywall"}),
            json!([{"thickness": 0.1}]),
            json!([{"material": 5, "thickness": 0.1}]),
            json!([{"material": "drywall", "thickness": "thin"}]),
            json!([{"material": "drywall"}]),
            json!([{"name": 3, "material": "drywall", "thickness": 0.1}]),
            json!(["drywall"]),
            json!(7),
            json!(null),
        ];
        for f in &bad_flags {
            for p in [json!({ "id": "r0", "flatCeiling": f }), json!({ "points": square_points(), "flatCeiling": f })] {
                let r = s.execute("room", &p);
                assert!(matches!(r, Err(EngineError::BadParams { .. })), "{p}: {r:?}");
            }
        }
        for st in &bad_stacks {
            for p in [json!({ "id": "r0", "ceilingStack": st }), json!({ "points": square_points(), "ceilingStack": st })] {
                let r = s.execute("room", &p);
                assert!(matches!(r, Err(EngineError::BadParams { .. })), "{p}: {r:?}");
            }
        }
        // An unknown template names the known ones.
        let e = s.execute("room", &json!({ "id": "r0", "ceilingStack": "plaster" })).unwrap_err().to_string();
        for n in cadcraft_arch::ceiling::stack::template_names() {
            assert!(e.contains(n), "{e}");
        }
        assert_eq!(s.doc().unwrap().arch.rooms, rooms_before, "refused edits change nothing");
    }

    #[test]
    fn room_edit_keeps_ceiling_when_absent() {
        let mut s = new_session();
        s.execute("room", &json!({ "points": square_points(), "ceiling": 3.0, "floor": 0.2 })).unwrap();
        for p in [json!({ "id": "r0", "name": "Hall" }), json!({ "id": "r0", "type": "bedroom" }), json!({ "id": "r0", "flatCeiling": false })] {
            s.execute("room", &p).unwrap();
            let r = &s.doc().unwrap().arch.rooms[0];
            assert_eq!((r.ceiling_height, r.floor_height), (Some(3.0), Some(0.2)), "{p}");
        }
        s.execute("room", &json!({ "id": "r0", "floor": 0.1 })).unwrap();
        let r = &s.doc().unwrap().arch.rooms[0];
        assert_eq!((r.ceiling_height, r.floor_height), (Some(3.0), Some(0.1)));
    }

    #[test]
    fn room_edit_null_clears_ceiling() {
        let mut s = new_session();
        s.execute("room", &json!({ "points": square_points(), "ceiling": 3.0, "floor": 0.2 })).unwrap();
        s.execute("room", &json!({ "id": "r0", "ceiling": null })).unwrap();
        let r = &s.doc().unwrap().arch.rooms[0];
        assert_eq!((r.ceiling_height, r.floor_height), (None, Some(0.2)));
        s.execute("room", &json!({ "id": "r0", "floor": null })).unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms[0].floor_height, None);
        // One undo brings the floor back.
        s.undo().unwrap();
        assert_eq!(s.doc().unwrap().arch.rooms[0].floor_height, Some(0.2));
    }

    #[test]
    fn room_reply_reports_ceiling_fields() {
        let mut s = new_session();
        let r = s.execute("room", &json!({ "points": ne_room() })).unwrap();
        assert_eq!(r["flatCeiling"], true);
        assert_eq!(r["ceilingStack"], json!([]));
        let r = s.execute("room", &json!({ "id": "r0", "flatCeiling": false, "ceilingStack": suspended() })).unwrap();
        assert_eq!(r["flatCeiling"], false);
        assert_eq!(
            r["ceilingStack"],
            json!([
                {"name": "drywall", "material": "drywall", "thickness": 0.0127},
                {"name": "air", "material": null, "thickness": 0.2},
                {"name": "", "material": "ceiling-tile", "thickness": 0.016}
            ])
        );
    }

    /// A room edit that moves the ceiling re-seats the ceiling items hung from it; an item moved
    /// off the ceiling by hand stays.
    #[test]
    fn room_edit_reseats_ceiling_items() {
        use cadcraft_doc::arch::{Furnishing, Placement};
        let mut s = new_session();
        s.execute("room", &json!({ "points": ne_room() })).unwrap();
        let hung =
            |at: [f64; 2], elevation: f64| Furnishing { placement: Placement::Ceiling, at, elevation, size: [0.4, 0.4, 0.86], ..Default::default() };
        {
            let d = s.doc_mut().unwrap();
            d.arch.ensure_ground_level();
            d.arch.push_furnishing(hung([8.0, 5.0], 2.4 - 0.86));
            d.arch.push_furnishing(hung([9.0, 6.0], 1.0));
            d.touch_arch();
        }
        s.execute("room", &json!({ "id": "r0", "ceiling": 2.7 })).unwrap();
        let f = &s.doc().unwrap().arch.furnishings;
        assert!((f[0].elevation + f[0].size[2] - 2.7).abs() < 1e-9, "{}", f[0].elevation);
        assert!((f[1].elevation - 1.0).abs() < 1e-12);
        s.execute("room", &json!({ "id": "r0", "ceilingStack": [{"material": "drywall", "thickness": 0.0127}] })).unwrap();
        let f = &s.doc().unwrap().arch.furnishings;
        assert!((f[0].elevation + f[0].size[2] - 2.6873).abs() < 1e-9, "{}", f[0].elevation);
    }
}
