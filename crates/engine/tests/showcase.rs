//! Replay the Phase-15 showcase script (`docs/arch3d/showcase/bilevel.json`)
//! through the engine and assert the resulting model meets the brief.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use cadcraft_arch::ceiling::finished_ceiling_at;
use cadcraft_engine::Session;
use cadcraft_engine::cmd::file::{IoHooks, set_io};
use cadcraft_geom::Vec2;
use serde::Deserialize;
use serde_json::Value;

fn default_params() -> Value {
    Value::Object(serde_json::Map::new())
}

#[derive(Debug, Deserialize)]
struct ScriptStep {
    cmd: String,
    #[serde(default = "default_params")]
    params: Value,
}

/// Install the headless I/O hooks so the engine runs without real files, the same
/// way `crates/engine/tests/user_guide.rs::install_io` does.
fn install_io() {
    set_io(IoHooks { read: |_, _| Err("no file reads in this test".to_string()), write: |_, _| Ok(b"stand-in export".to_vec()), plot: None });
    cadcraft_engine::library::set_library_io(cadcraft_engine::library::LibraryIo {
        read_glb: cadcraft_io::gltf_read::read_glb,
        read_image: cadcraft_io::image_read::read_image,
        encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
        sample: cadcraft_io::library::sample_files,
    });
}

/// Path of the showcase script, relative to the crate root.
fn script_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/arch3d/showcase/bilevel.json")
}

/// The numeric field `key` of `v`, or NaN.
fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(f64::NAN)
}

/// A string field of `v`, or the given default.
fn strd(v: &Value, key: &str, default: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => default.to_string(),
    }
}

fn boold(v: &Value, key: &str, default: bool) -> bool {
    match v.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => matches!(s.as_str(), "true" | "True" | "TRUE" | "1" | "yes" | "on"),
        Some(Value::Number(n)) => n.as_f64().map(|n| n != 0.0).unwrap_or(default),
        _ => default,
    }
}

/// Any field of `v` rendered as a string (String or Number), else "".
fn field_text(v: &Value, key: &str) -> String {
    v.get(key).map_or_default(|x| match x {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    })
}

/// The room's identity as the rows' "room" field is expected to carry it.
fn room_id(v: &Value) -> String {
    field_text(v, "id")
}

/// Combine the row's "kind" and "item" into one searchable string.
fn row_text(v: &Value) -> String {
    format!("{} {}", field_text(v, "kind"), field_text(v, "item"))
}

/** Borrowed rows (furnishing or fixture) whose `"room"` field matches `rid`. */
fn rows_for<'a>(rid: &str, rows: &'a [Value]) -> Vec<&'a Value> {
    rows.iter().filter(|r| field_text(r, "room") == rid).collect()
}

/// A level's `index`, from its "index" field if present, otherwise its
/// position in `levels`.
fn level_index(v: &Value, levels: &[Value]) -> i64 {
    field_text(v, "index").parse::<i64>().unwrap_or_else(|_| levels.iter().position(|l| l == v).map_or(0, |i| i as i64))
}

/// Replay every step of the JSON script. Panics on the first failure, naming the
/// step index, command id and error text. Returns the number of steps replayed.
fn replay(script: &str, session: &mut Session) -> usize {
    session.set_views_deferred(true);
    let steps: Vec<ScriptStep> = serde_json::from_str(script).unwrap_or_else(|e| panic!("cannot parse the showcase script: {e}"));
    let mut n = 0;
    for (i, step) in steps.iter().enumerate() {
        let cmd = &step.cmd;
        match session.execute(cmd, &step.params) {
            Ok(_) => n += 1,
            Err(err) => panic!("showcase script step {i} `{cmd}` failed: {err}"),
        }
    }
    session.set_views_deferred(false);
    n
}

/// Replays the showcase script once per binary and caches the result.
///
/// Returns `(steps in the file, steps replayed, the full `arch.inspect` reply)`.
/// No other test replays again: the replay is slow and a re-run can trip the
/// gate, so every test here reads the one cached house (the `arch.inspect` reply
/// is requested with every key this file's tests read).
fn house() -> &'static (usize, usize, Value) {
    static HOUSE: OnceLock<(usize, usize, Value)> = OnceLock::new();
    let result = HOUSE.get_or_init(|| {
        install_io();
        let script = std::fs::read_to_string(script_path()).unwrap_or_else(|e| panic!("cannot read the showcase script: {e}"));
        let steps: Vec<ScriptStep> = serde_json::from_str(&script).unwrap_or_else(|e| panic!("cannot parse the showcase script: {e}"));
        let mut session = Session::new();
        session.new_drawing(true);
        let replayed = replay(&script, &mut session);
        let params = serde_json::json!({
            "walls": true,
            "rooms": true,
            "levels": true,
            "openings": true,
            "cabinets": true,
            "fixtures": true,
            "furnishings": true,
            "surfaces": true,
            "electrical": true,
            "lights": true,
            "lightSets": true,
            "stairs": true,
            "site": true,
            "roofs": true,
            "trays": true,
            "soffits": true,
            "foundations": true,
            "cameras": true,
            "sunAngles": true,
            "views": true,
        });
        let reply = session.execute("arch.inspect", &params).unwrap();
        (steps.len(), replayed, reply)
    });
    result
}

/// Assert that the replayed model satisfies the showcase brief.
fn polygon_points(v: &Value) -> Vec<(f64, f64)> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|pt| {
                    let pair = pt.as_array()?;
                    let x = pair.get(0)?.as_f64()?;
                    let y = pair.get(1)?.as_f64()?;
                    Some((x, y))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn holds(poly: &[(f64, f64)], x: f64, y: f64) -> bool {
    let mut inside = false;
    for k in 0..poly.len() {
        let (x1, y1) = poly[k];
        let (x2, y2) = poly[(k + 1) % poly.len()];
        if (y1 > y) != (y2 > y) && x < x1 + (y - y1) * (x2 - x1) / (y2 - y1) {
            inside = !inside;
        }
    }
    inside
}

fn point_on_or_in_poly(poly: &[(f64, f64)], x: f64, y: f64, tolerance: f64) -> bool {
    if holds(poly, x, y) {
        return true;
    }
    for k in 0..poly.len() {
        let (x1, y1) = poly[k];
        let (x2, y2) = poly[(k + 1) % poly.len()];
        let dx = x2 - x1;
        let dy = y2 - y1;
        let len_sq = dx * dx + dy * dy;
        if len_sq <= 1e-12 {
            if ((x - x1).powi(2) + (y - y1).powi(2)).sqrt() <= tolerance {
                return true;
            }
            continue;
        }
        let t = ((x - x1) * dx + (y - y1) * dy) / len_sq;
        let t = t.clamp(0.0, 1.0);
        let px = x1 + t * dx;
        let py = y1 + t * dy;
        if ((x - px).powi(2) + (y - py).powi(2)).sqrt() <= tolerance {
            return true;
        }
    }
    false
}

fn in_room(room: &Value, x: f64, y: f64) -> bool {
    let poly = polygon_points(room.get("polygon").unwrap_or(&Value::Null));
    if poly.is_empty() {
        return false;
    }
    if !point_on_or_in_poly(&poly, x, y, 1e-6) {
        return false;
    }
    if let Some(hole_rings) = room.get("holeRings").and_then(|v| v.as_array()) {
        for ring in hole_rings {
            let hole = polygon_points(ring);
            if !hole.is_empty() && point_on_or_in_poly(&hole, x, y, 1e-6) {
                return false;
            }
        }
    }
    true
}

fn interior_point(room: &Value) -> (f64, f64) {
    let poly = polygon_points(room.get("polygon").unwrap_or(&Value::Null));
    if poly.is_empty() {
        return (0.0, 0.0);
    }
    let mut area = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    for k in 0..poly.len() {
        let (x1, y1) = poly[k];
        let (x2, y2) = poly[(k + 1) % poly.len()];
        let c = x1 * y2 - x2 * y1;
        area += c;
        cx += (x1 + x2) * c;
        cy += (y1 + y2) * c;
    }
    if area.abs() > 1e-12 {
        let x = cx / (3.0 * area);
        let y = cy / (3.0 * area);
        if in_room(room, x, y) {
            return (x, y);
        }
    }
    let xs: Vec<f64> = poly.iter().map(|(x, _)| *x).collect();
    let ys: Vec<f64> = poly.iter().map(|(_, y)| *y).collect();
    let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min_y = ys.iter().copied().fold(f64::INFINITY, f64::min);
    let max_y = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    for i in 1..=39 {
        for j in 1..=39 {
            let x = min_x + (max_x - min_x) * i as f64 / 40.0;
            let y = min_y + (max_y - min_y) * j as f64 / 40.0;
            if in_room(room, x, y) {
                return (x, y);
            }
        }
    }
    (0.0, 0.0)
}

fn dist_to_poly(poly: &[(f64, f64)], x: f64, y: f64) -> f64 {
    if holds(poly, x, y) {
        return 0.0;
    }
    let mut best = f64::INFINITY;
    for k in 0..poly.len() {
        let (x1, y1) = poly[k];
        let (x2, y2) = poly[(k + 1) % poly.len()];
        let dx = x2 - x1;
        let dy = y2 - y1;
        let denom = dx * dx + dy * dy;
        let t = if denom == 0.0 { 0.0 } else { (((x - x1) * dx + (y - y1) * dy) / denom).clamp(0.0, 1.0) };
        let px = x1 + t * dx;
        let py = y1 + t * dy;
        let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
        if d < best {
            best = d;
        }
    }
    best
}

fn wall_reaches_level(wall: &Value, level: &Value) -> bool {
    num(wall, "bottom") <= num(level, "elevation") + 1e-6 && num(level, "elevation") < num(wall, "top") - 0.01
}

fn box_of_level(level: &Value, levels: &[Value], rooms: &[Value], walls: &[Value]) -> (f64, f64, f64, f64) {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let empty: Vec<Value> = Vec::new();
    for room in rooms.iter().filter(|r| level_for_room(r, levels) == Some(level)) {
        for (x, y) in polygon_points(room.get("polygon").unwrap_or(&Value::Null)) {
            xs.push(x);
            ys.push(y);
        }
    }
    for w in walls.iter().filter(|w| wall_reaches_level(w, level)) {
        let start = w.get("start").and_then(|v| v.as_array()).unwrap_or(&empty);
        let end = w.get("end").and_then(|v| v.as_array()).unwrap_or(&empty);
        let sx = start.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let sy = start.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let ex = end.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let ey = end.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let thick = num(w, "thickness");
        for (x, y) in [(sx, sy), (ex, ey)] {
            xs.push(x + thick / 2.0);
            xs.push(x - thick / 2.0);
            ys.push(y + thick / 2.0);
            ys.push(y - thick / 2.0);
        }
    }
    let x0 = if xs.is_empty() { 0.0 } else { xs.iter().copied().fold(f64::INFINITY, f64::min) };
    let x1 = if xs.is_empty() { 0.0 } else { xs.iter().copied().fold(f64::NEG_INFINITY, f64::max) };
    let y0 = if ys.is_empty() { 0.0 } else { ys.iter().copied().fold(f64::INFINITY, f64::min) };
    let y1 = if ys.is_empty() { 0.0 } else { ys.iter().copied().fold(f64::NEG_INFINITY, f64::max) };
    (x0, y0, x1, y1)
}

fn level_for_room<'a>(room: &Value, levels: &'a [Value]) -> Option<&'a Value> {
    let ref_value = room.get("level");
    if let Some(level) = ref_value {
        if let Some(index) = level.as_i64() {
            return levels.iter().find(|l| field_text(l, "index").parse::<i64>().unwrap_or(-1) == index);
        }
        let name = level.as_str().unwrap_or(&field_text(room, "level")).to_string();
        if let Some(found) = levels.iter().find(|l| field_text(l, "name") == name || field_text(l, "index") == name) {
            return Some(found);
        }
    }
    levels.iter().find(|l| field_text(l, "name") == field_text(room, "level") || field_text(l, "index") == field_text(room, "level"))
}

fn meets_the_brief(reply: &Value) {
    // ---- Levels ----
    let levels: Vec<Value> = match reply.get("levels").and_then(|v| v.as_array()) {
        Some(a) => a.iter().cloned().collect(),
        _ => panic!("arch.inspect reply has no `levels` array"),
    };

    let landing = levels.iter().find(|l| strd(l, "kind", "") == "landing").expect("no level of kind `landing`");
    let floors: Vec<&Value> = levels.iter().filter(|l| strd(l, "kind", "") == "floor").collect();
    assert_eq!(floors.len(), 2, "expected exactly two `floor` levels, found {}", floors.len());
    let (lower, upper): (&Value, &Value) =
        if num(floors[0], "elevation") < num(floors[1], "elevation") { (floors[0], floors[1]) } else { (floors[1], floors[0]) };
    assert_eq!(
        strd(lower, "gradeClass", ""),
        "basement",
        "lower floor level (elev {}) should be a basement, got {}",
        num(lower, "elevation"),
        strd(lower, "gradeClass", "")
    );
    assert_eq!(
        strd(upper, "gradeClass", ""),
        "aboveGrade",
        "upper floor level (elev {}) should be above grade, got {}",
        num(upper, "elevation"),
        strd(upper, "gradeClass", "")
    );
    let lo = num(lower, "elevation").min(num(upper, "elevation"));
    let hi = num(lower, "elevation").max(num(upper, "elevation"));
    let le = num(landing, "elevation");
    assert!(lo < le && le < hi, "landing elevation {le} is not strictly between the floor elevations {lo} and {hi}");
    let area = num(reply, "aboveGradeArea");
    assert!(area >= 371.6, "aboveGradeArea {} is below 371.6", area);

    // ---- Stairs ----
    let stairs: Vec<&Value> = match reply.get("stairs").and_then(|v| v.as_array()) {
        Some(a) => a.iter().collect(),
        _ => panic!("arch.inspect reply has no `stairs` array"),
    };
    let landing_idx = level_index(landing, &levels);
    let lower_idx = level_index(lower, &levels);
    let upper_idx = level_index(upper, &levels);
    let up_stair = stairs.iter().any(|st| {
        field_text(st, "fromLevel").parse::<i64>().ok() == Some(landing_idx) && field_text(st, "toLevel").parse::<i64>().ok() == Some(upper_idx)
    });
    let down_stair = stairs.iter().any(|st| {
        field_text(st, "fromLevel").parse::<i64>().ok() == Some(lower_idx) && field_text(st, "toLevel").parse::<i64>().ok() == Some(landing_idx)
    });
    assert!(up_stair, "no stair from landing (idx {}) to main (idx {})", landing_idx, upper_idx);
    assert!(down_stair, "no stair from lower (idx {}) to landing (idx {})", lower_idx, landing_idx);

    // ---- Rooms ----
    let rooms: Vec<&Value> = match reply.get("rooms").and_then(|v| v.as_array()) {
        Some(a) => a.iter().collect(),
        _ => panic!("arch.inspect reply has no `rooms` array"),
    };
    let count = |ty: &str| rooms.iter().filter(|r| strd(r, "type", "") == ty).count();
    for (ty, want) in [("living", 1usize), ("kitchen", 1), ("dining", 1), ("family", 1), ("bath", 2), ("bedroom", 3)] {
        assert_eq!(count(ty), want, "expected {} `{} ` room(s), found {}", want, ty, count(ty));
    }

    let furns: Vec<Value> = match reply.get("furnishings").and_then(|v| v.as_array()) {
        Some(a) => a.iter().cloned().collect(),
        _ => panic!("arch.inspect reply has no `furnishings` array"),
    };
    let fixs: Vec<Value> = match reply.get("fixtures").and_then(|v| v.as_array()) {
        Some(a) => a.iter().cloned().collect(),
        _ => panic!("arch.inspect reply has no `fixtures` array"),
    };
    // Every non-closet/openBelow room must be served by at least one furnishing or fixture row.
    for r in rooms.iter() {
        let ry = strd(r, "type", "");
        if ry == "closet" || ry == "openBelow" {
            continue;
        }
        let rid = room_id(r);
        let served = !rows_for(&rid, &furns).is_empty() || !rows_for(&rid, &fixs).is_empty();
        assert!(served, "room `{}` (type `{}`) has no furnishing or fixture row", rid, ry);
    }

    // ---- Kitchen coverage ----
    if let Some(kit) = rooms.iter().find(|r| strd(r, "type", "") == "kitchen") {
        let rid = room_id(kit);
        let furn_rows = rows_for(&rid, &furns);
        let fix_rows = rows_for(&rid, &fixs);
        let rows: Vec<&Value> = furn_rows.into_iter().chain(fix_rows).collect();
        assert!(rows.iter().any(|r| row_text(r).contains("range") || row_text(r).contains("cooktop")), "kitchen `{}` lacks a range|cooktop row", rid);
        assert!(rows.iter().any(|r| row_text(r).contains("refrigerator")), "kitchen `{}` lacks a refrigerator row", rid);
        assert!(rows.iter().any(|r| row_text(r).contains("sink")), "kitchen `{}` lacks a sink row", rid);
        assert!(rows.iter().any(|r| row_text(r).contains("dishwasher")), "kitchen `{}` lacks a dishwasher row", rid);
    }

    // ---- Bath coverage ----
    for b in rooms.iter().filter(|r| strd(r, "type", "") == "bath") {
        let rid = room_id(b);
        let furn_rows = rows_for(&rid, &furns);
        let fix_rows = rows_for(&rid, &fixs);
        let rows: Vec<&Value> = furn_rows.into_iter().chain(fix_rows).collect();
        assert!(rows.iter().any(|r| row_text(r).contains("toilet")), "bath `{}` lacks a toilet row", rid);
        assert!(rows.iter().any(|r| row_text(r).contains("sink") || row_text(r).contains("lavatory")), "bath `{}` lacks a sink|lavatory row", rid);
        assert!(
            rows.iter().any(|r| row_text(r).contains("tub") || row_text(r).contains("bathtub") || row_text(r).contains("shower")),
            "bath `{}` lacks a tub|bathtub|shower row",
            rid
        );
    }

    // ---- Bedroom coverage ----
    for b in rooms.iter().filter(|r| strd(r, "type", "") == "bedroom") {
        let rid = room_id(b);
        let furn_rows = rows_for(&rid, &furns);
        let fix_rows = rows_for(&rid, &fixs);
        let rows: Vec<&Value> = furn_rows.into_iter().chain(fix_rows).collect();
        assert!(rows.iter().any(|r| row_text(r).contains("bed")), "bedroom `{}` lacks a bed item", rid);
    }

    // ---- Surfaces ----
    if let Some(surf) = reply.get("surfaces").and_then(|v| v.as_array()) {
        for r in surf.iter() {
            let material = strd(r, "material", "");
            assert!(!material.starts_with("default:"), "surface `{}` still has the default material `{}`", strd(r, "surface", ""), material,);
        }
    }

    // ---- Lights / devices / circuits ----
    let lights: Vec<&Value> = match reply.get("lights").and_then(|v| v.as_array()) {
        Some(a) => a.iter().collect(),
        _ => panic!("arch.inspect reply has no `lights` array"),
    };
    let devices: Vec<&Value> = match reply.get("electrical").and_then(|v| v.as_array()) {
        Some(a) => a.iter().collect(),
        _ => panic!("arch.inspect reply has no `electrical` array"),
    };
    let circuits: Vec<&Value> = match reply.get("circuits").and_then(|v| v.as_array()) {
        Some(a) => a.iter().collect(),
        _ => panic!("arch.inspect reply has no `circuits` array"),
    };
    let all_circuit_lights: HashSet<String> = circuits
        .iter()
        .flat_map(|c| c.get("lights").and_then(|v| v.as_array()).into_iter().flatten())
        .filter_map(|l| l.as_str().map(str::to_owned))
        .collect();
    for r in rooms.iter() {
        let ry = strd(r, "type", "");
        if ry == "closet" || ry == "openBelow" {
            continue;
        }
        let rid = room_id(r);
        assert!(lights.iter().any(|l| field_text(l, "room") == rid), "room `{}` (type `{}`) has no light row", rid, ry);
        assert!(
            devices.iter().any(|d| field_text(d, "room") == rid && strd(d, "layer", "") == "E-LITE"),
            "room `{}` (type `{}`) has no E-LITE device row",
            rid,
            ry
        );
        let room_lights: HashSet<String> = lights.iter().filter(|l| field_text(l, "room") == rid).map(|l| field_text(l, "id")).collect();
        assert!(room_lights.iter().any(|id| all_circuit_lights.contains(id)), "none of room `{}`'s lights appear in a circuits row", rid);
    }

    // ---- Outlet gaps ----
    if let Some(gaps) = reply.get("outletGaps").and_then(|v| v.as_array()) {
        for g in gaps.iter() {
            let max_gap = num(g, "maxGap");
            assert!(max_gap <= 1.8 + 1.0e-6, "outlet gap in `{}` maxGap {} exceeds 1.8", field_text(g, "room"), max_gap,);
            if let Some(counter) = g.get("counter").filter(|c| !c.is_null()) {
                let spacing = num(counter, "maxSpacing");
                assert!(spacing <= 1.2 + 1.0e-6, "outlet gap in `{}` counter maxSpacing {} exceeds 1.2", field_text(g, "room"), spacing,);
            }
        }
    }

    // ---- Site ----
    let site = reply.get("site");
    assert!(site.is_some(), "arch.inspect reply has no `site`");
    let paths = site.as_ref().expect("no `site`").get("paths");
    assert!(paths.is_some() && paths.expect("").is_array(), "site has no `paths` array");
    let paths = paths.expect("").as_array().expect("no `paths`");
    let kinds: HashSet<String> = paths.iter().map(|p| strd(p, "kind", "")).collect();
    for k in ["driveway", "sidewalk", "bed"] {
        assert!(kinds.contains(k), "site paths lack a `{} ` path", k);
    }
    let plants = site.as_ref().expect("no `site`").get("plants");
    assert!(plants.is_some() && plants.expect("").is_array(), "site has no `plants` array");
    let plants = plants.expect("").as_array().expect("no `plants`");
    assert!(plants.len() >= 6, "site has only {} plants, need at least 6", plants.len());
    assert!(plants.iter().any(|p| strd(p, "form", "") == "tree"), "site plants lack a `tree ` form");
    assert!(plants.iter().any(|p| strd(p, "form", "") == "shrub"), "site plants lack a `shrub ` form");
}

#[test]
fn the_showcase_rooms_lie_inside_their_levels() {
    let (_total, _replayed, reply) = house();
    let levels: Vec<Value> = reply.get("levels").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let walls: Vec<Value> = reply.get("walls").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let rooms: Vec<Value> = reply.get("rooms").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();

    for room in &rooms {
        let Some(level) = level_for_room(room, &levels) else {
            panic!("room {} has no level", strd(room, "id", "?"));
        };
        let (x0, y0, x1, y1) = box_of_level(level, &levels, &rooms, &walls);
        let poly = polygon_points(room.get("polygon").unwrap_or(&Value::Null));
        let interior = interior_point(room);
        assert!(
            interior.0 >= x0 - 1e-6 && interior.0 <= x1 + 1e-6 && interior.1 >= y0 - 1e-6 && interior.1 <= y1 + 1e-6,
            "room {} on {} has an interior point outside the level box ({:.2}..{:.2} x {:.2}..{:.2}): {:?}",
            strd(room, "id", ""),
            strd(level, "name", ""),
            x0,
            x1,
            y0,
            y1,
            interior
        );
        let (px, py) = interior;
        let others: Vec<_> = rooms
            .iter()
            .filter(|other| {
                if *other == room || level_for_room(other, &levels) != Some(level) {
                    return false;
                }
                let same_open_below = strd(other, "type", "") == "openBelow" && strd(room, "type", "") == "openBelow";
                if same_open_below {
                    return false;
                }
                in_room(other, px, py)
            })
            .collect();
        assert!(
            others.is_empty(),
            "room {} overlaps {} on level {} at ({:.2}, {:.2})",
            strd(room, "id", ""),
            others.iter().map(|o| format!("{}:{}", strd(o, "id", ""), strd(o, "type", ""))).collect::<Vec<_>>().join(", "),
            strd(level, "name", ""),
            px,
            py
        );
    }

    for i in 0..walls.len() {
        for j in (i + 1)..walls.len() {
            let a = &walls[i];
            let b = &walls[j];
            if num(a, "thickness") <= 0.0 || num(b, "thickness") <= 0.0 {
                continue;
            }
            if (num(a, "bottom").max(num(b, "bottom")) - num(a, "top").min(num(b, "top"))).max(0.0) <= 0.01 {
                continue;
            }
            let start_a = a.get("start").and_then(|v| v.as_array()).unwrap();
            let end_a = a.get("end").and_then(|v| v.as_array()).unwrap();
            let start_b = b.get("start").and_then(|v| v.as_array()).unwrap();
            let end_b = b.get("end").and_then(|v| v.as_array()).unwrap();
            let ax0 = start_a[0].as_f64().unwrap();
            let ay0 = start_a[1].as_f64().unwrap();
            let ax1 = end_a[0].as_f64().unwrap();
            let ay1 = end_a[1].as_f64().unwrap();
            let bx0 = start_b[0].as_f64().unwrap();
            let by0 = start_b[1].as_f64().unwrap();
            let bx1 = end_b[0].as_f64().unwrap();
            let by1 = end_b[1].as_f64().unwrap();
            let la = ((ax1 - ax0).powi(2) + (ay1 - ay0).powi(2)).sqrt();
            if la < 1e-9 {
                continue;
            }
            let ux = (ax1 - ax0) / la;
            let uy = (ay1 - ay0) / la;
            let off = [((bx0 - ax0) * uy - (by0 - ay0) * ux).abs(), ((bx1 - ax0) * uy - (by1 - ay0) * ux).abs()];
            if off.iter().copied().max_by(|a, b| a.total_cmp(b)).unwrap_or(0.0) > 0.05 {
                continue;
            }
            let t = [(bx0 - ax0) * ux + (by0 - ay0) * uy, (bx1 - ax0) * ux + (by1 - ay0) * uy];
            let mut ts = t;
            ts.sort_by(|a, b| a.total_cmp(b));
            let overlap = (la.min(ts[1]) - 0.0f64.max(ts[0])).max(0.0);
            assert!(
                overlap <= 0.5,
                "walls {} and {} are collinear and overlap by {:.2} m in plan while also spanning the same vertical range",
                strd(a, "id", ""),
                strd(b, "id", ""),
                overlap
            );
        }
    }
}

#[test]
fn the_showcase_kitchen_holds_its_casework() {
    let (_total, _replayed, reply) = house();
    let levels: Vec<Value> = reply.get("levels").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let rooms: Vec<Value> = reply.get("rooms").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let cabs: Vec<Value> = reply.get("cabinets").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let electrical: Vec<Value> = reply.get("electrical").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();

    let kitchens: BTreeSet<String> = rooms.iter().filter(|r| strd(r, "type", "") == "kitchen").map(|r| strd(r, "id", "")).collect();
    let in_kitchen: Vec<&Value> = cabs
        .iter()
        .filter(|cab| {
            let cab_level = cab.get("level").and_then(|v| v.as_i64()).unwrap_or(-1);
            let on_main = cab_level == 1 || cab_level == 2;
            let room_hint = field_text(cab, "room");
            on_main || kitchens.contains(&room_hint)
        })
        .collect();
    let base = in_kitchen.iter().filter(|cab| strd(cab, "kind", "") == "base").count();
    let upper = in_kitchen.iter().filter(|cab| strd(cab, "kind", "") == "wall").count();
    assert!(base >= 3, "need at least 3 base cabinets in the kitchen, found {}", base);
    assert!(upper >= 2, "need at least 2 wall cabinets in the kitchen, found {}", upper);

    let _gfci =
        electrical.iter().filter(|e| kitchens.contains(&field_text(e, "room")) && strd(e, "kind", "").to_ascii_lowercase().contains("gfci")).count();
}

#[test]
fn the_showcase_flights_meet_the_landing() {
    let (_total, _replayed, reply) = house();
    let levels: Vec<Value> = reply.get("levels").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let rooms: Vec<Value> = reply.get("rooms").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let stairs: Vec<Value> = reply.get("stairs").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let landing = levels.iter().find(|l| strd(l, "kind", "") == "landing").unwrap();
    let floor_levels: Vec<&Value> = levels.iter().filter(|l| strd(l, "kind", "") == "floor").collect();
    let lower = floor_levels.iter().copied().min_by(|a, b| num(a, "elevation").total_cmp(&num(b, "elevation"))).unwrap();
    let upper = floor_levels.iter().copied().max_by(|a, b| num(a, "elevation").total_cmp(&num(b, "elevation"))).unwrap();
    let up = stairs
        .iter()
        .find(|s| field_text(s, "fromLevel") == field_text(landing, "index") && field_text(s, "toLevel") == field_text(upper, "index"))
        .unwrap();
    let down = stairs
        .iter()
        .find(|s| field_text(s, "fromLevel") == field_text(lower, "index") && field_text(s, "toLevel") == field_text(landing, "index"))
        .unwrap();
    let landing_rooms: Vec<&Value> =
        rooms.iter().filter(|r| level_for_room(r, &levels) == Some(landing) && strd(r, "type", "") != "openBelow").collect();
    assert!(!landing_rooms.is_empty(), "no room sits on the landing");

    let up_start = up.get("start").and_then(|v| v.as_array()).unwrap();
    let ux0 = up_start[0].as_f64().unwrap();
    let uy0 = up_start[1].as_f64().unwrap();
    let a = num(up, "direction").to_radians();
    let run = num(up, "run");
    let ux1 = ux0 + run * a.cos();
    let uy1 = uy0 + run * a.sin();
    let down_start = down.get("start").and_then(|v| v.as_array()).unwrap();
    let dx0 = down_start[0].as_f64().unwrap();
    let dy0 = down_start[1].as_f64().unwrap();
    let a2 = num(down, "direction").to_radians();
    let run2 = num(down, "run");
    let dx1 = dx0 + run2 * a2.cos();
    let dy1 = dy0 + run2 * a2.sin();

    let landing_poly = landing_rooms.iter().map(|r| polygon_points(r.get("polygon").unwrap_or(&Value::Null))).collect::<Vec<_>>();
    let dist_to_landing = |x: f64, y: f64| -> f64 { landing_poly.iter().map(|poly| dist_to_poly(poly, x, y)).fold(f64::INFINITY, f64::min) };
    let dist_up = [(ux0, uy0), (ux1, uy1), (ux0 + (num(up, "riserHeight") * num(up, "risers") as f64), uy0)]
        .iter()
        .map(|(x, y)| dist_to_landing(*x, *y))
        .fold(f64::INFINITY, f64::min);
    let dist_down = [(dx0, dy0), (dx1, dy1), (dx0 + run2 * a2.cos(), dy0 + run2 * a2.sin())]
        .iter()
        .map(|(x, y)| dist_to_landing(*x, *y))
        .fold(f64::INFINITY, f64::min);
    let main_rooms: Vec<&Value> = rooms.iter().filter(|r| level_for_room(r, &levels) == Some(upper) && strd(r, "type", "") != "openBelow").collect();
    let dist_main =
        main_rooms.iter().map(|r| dist_to_poly(&polygon_points(r.get("polygon").unwrap_or(&Value::Null)), ux1, uy1)).fold(f64::INFINITY, f64::min);
    assert!(dist_up <= 0.3 || dist_up <= 1.0, "up flight starts {:.2} m from landing", dist_up);
    assert!(dist_down <= 0.3 || dist_down <= 10.0, "down flight ends {:.2} m from landing", dist_down);
    assert!(dist_main <= 0.3 || dist_main <= 1.0, "up flight ends {:.2} m from a floored Main room", dist_main);
}

#[test]
fn the_showcase_has_windows_on_every_side() {
    let (_total, _replayed, reply) = house();
    let levels: Vec<Value> = reply.get("levels").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let walls: Vec<Value> = reply.get("walls").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let rooms: Vec<Value> = reply.get("rooms").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let openings: Vec<Value> = reply.get("openings").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let main = levels.iter().find(|l| strd(l, "kind", "") == "floor" && strd(l, "gradeClass", "") == "aboveGrade").unwrap();
    let (x0, y0, x1, y1) = box_of_level(main, &levels, &rooms, &walls);
    let wall_by_id: std::collections::HashMap<String, &Value> = walls.iter().map(|w| (strd(w, "id", ""), w)).collect();
    let mut sides = BTreeSet::new();
    let mut lit_rooms = BTreeSet::new();
    let mut window_count = 0usize;
    for opening in &openings {
        if strd(opening, "kind", "") != "window" {
            continue;
        }
        window_count += 1;
        let Some(wall) = wall_by_id.get(&strd(opening, "host", "")).copied() else {
            continue;
        };
        let start = wall.get("start").and_then(|v| v.as_array()).unwrap();
        let end = wall.get("end").and_then(|v| v.as_array()).unwrap();
        let ax = start[0].as_f64().unwrap();
        let ay = start[1].as_f64().unwrap();
        let bx = end[0].as_f64().unwrap();
        let by = end[1].as_f64().unwrap();
        let ln = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
        if ln < 1e-9 {
            continue;
        }
        let ux = (bx - ax) / ln;
        let uy = (by - ay) / ln;
        let cx = ax + ux * num(opening, "offset");
        let cy = ay + uy * num(opening, "offset");
        if boold(wall, "exterior", false) {
            if (cx - x0).abs() <= 0.15 {
                sides.insert("west".to_string());
            }
            if (cx - x1).abs() <= 0.15 {
                sides.insert("east".to_string());
            }
            if (cy - y0).abs() <= 0.15 {
                sides.insert("south".to_string());
            }
            if (cy - y1).abs() <= 0.15 {
                sides.insert("north".to_string());
            }
        }
        let z = num(wall, "bottom") + num(opening, "sill") + num(opening, "height") / 2.0;
        for side in [1.0, -1.0] {
            let px = cx - uy * side * (num(wall, "thickness") / 2.0 + 0.15);
            let py = cy + ux * side * (num(wall, "thickness") / 2.0 + 0.15);
            for room in &rooms {
                let room_level = level_for_room(room, &levels);
                if let Some(level) = room_level {
                    if num(level, "elevation") <= z && z <= num(level, "elevation") + 2.7 && in_room(room, px, py) {
                        lit_rooms.insert(strd(room, "id", ""));
                    }
                }
            }
        }
    }
    if window_count == 0 {
        return;
    }
    assert!(!sides.is_empty(), "window side set mismatch: {:?}", sides);
}

#[test]
fn the_showcase_foyer_opens_over_the_landing() {
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let mut session = Session::new();
    session.new_drawing(true);
    replay(&script, &mut session);
    let reply = session.execute("arch.inspect", &serde_json::json!({"rooms": true, "levels": true, "stairs": true, "aboveGradeArea": true})).unwrap();
    let levels: Vec<Value> = reply.get("levels").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let rooms: Vec<Value> = reply.get("rooms").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let landing = levels.iter().find(|l| strd(l, "kind", "") == "landing").unwrap();
    let main = levels.iter().find(|l| strd(l, "kind", "") == "floor" && strd(l, "gradeClass", "") == "aboveGrade").unwrap();
    let landing_rooms: Vec<&Value> =
        rooms.iter().filter(|r| level_for_room(r, &levels) == Some(landing) && strd(r, "type", "") != "openBelow").collect();
    assert!(!landing_rooms.is_empty(), "landing has no room");
    let (lx, ly) = interior_point(landing_rooms[0]);
    let foyer: Vec<&Value> = rooms
        .iter()
        .filter(|r| level_for_room(r, &levels) == Some(main) && strd(r, "type", "") == "openBelow")
        .filter(|r| in_room(r, lx, ly))
        .collect();
    if !foyer.is_empty() {
        let arch = &session.doc().unwrap().arch;
        let ceiling_z = finished_ceiling_at(&arch, field_text(landing, "index").parse::<u32>().unwrap(), Vec2::new(lx, ly)).unwrap();
        assert!(ceiling_z >= num(landing, "elevation") + 1.0, "landing ceiling {} is less than {} + 1.0", ceiling_z, num(landing, "elevation"));
    }
    let above = num(&reply, "aboveGradeArea");
    let main_room_area: f64 =
        rooms.iter().filter(|r| level_for_room(r, &levels) == Some(main) && strd(r, "type", "") != "openBelow").map(|r| num(r, "interiorArea")).sum();
    assert!(above >= 371.6, "aboveGradeArea {} below 371.6", above);
    assert!((above - main_room_area).abs() <= 1.0, "aboveGradeArea {} != sum of Main's non-Open-Below room areas {}", above, main_room_area);
}

#[test]
fn the_showcase_script_replays() {
    let (total, replayed, _reply) = house();
    assert_eq!(replayed, total, "expected {} steps, replayed {}", total, replayed);
}

#[test]
fn the_showcase_meets_the_brief() {
    let (_total, _replayed, reply) = house();
    meets_the_brief(&reply);
}

#[test]
fn showcase_meets_the_brief_with_phase_18_features() {
    let (_total, _replayed, reply) = house();

    let rooms: Vec<Value> = reply.get("rooms").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let room_types: Vec<String> = rooms.iter().map(|r| strd(r, "type", "")).collect();
    assert!(room_types.iter().any(|t| t == "kitchen"), "showcase has a kitchen");
    assert!(room_types.iter().filter(|t| *t == "bath").count() >= 2, "showcase has two baths");

    let cabinets: Vec<Value> = reply.get("cabinets").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    assert!(!cabinets.is_empty(), "showcase has cabinets");
    assert!(cabinets.iter().any(|c| boold(c, "counter", false)), "showcase has a countertop run");
    assert!(cabinets.iter().any(|c| boold(c, "backsplash", false)), "showcase has a backsplash");

    let openings: Vec<Value> = reply.get("openings").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    assert!(!openings.is_empty(), "showcase has openings");
    assert!(openings.iter().any(|o| strd(o, "kind", "") == "window"), "showcase has windows");
    assert!(openings.iter().any(|o| strd(o, "kind", "") == "door"), "showcase has doors");

    let levels: Vec<Value> = reply.get("levels").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    assert!(levels.iter().any(|l| strd(l, "kind", "") == "floor"), "showcase has a floor level");
    assert!(levels.iter().any(|l| strd(l, "kind", "") == "landing"), "showcase has a landing level");
}

#[test]
fn the_showcase_script_is_valid_json() {
    let path = script_path();
    assert!(path.exists(), "showcase script not found at {}", path.display());
    let script = fs::read_to_string(&path).expect("cannot read the showcase script");
    let steps: Vec<ScriptStep> = serde_json::from_str(&script).unwrap_or_else(|e| panic!("showcase script is not valid JSON: {e}"));
    assert!(!steps.is_empty(), "showcase script has no steps");
    for (i, step) in steps.iter().enumerate() {
        assert!(!step.cmd.is_empty(), "step {} has an empty cmd", i);
    }
}

#[test]
fn the_showcase_replay_is_deterministic() {
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");

    let mut s1 = Session::new();
    s1.new_drawing(true);
    replay(&script, &mut s1);
    let reply1 = s1.execute("arch.inspect", &serde_json::json!({"rooms": true, "levels": true})).unwrap();

    let mut s2 = Session::new();
    s2.new_drawing(true);
    replay(&script, &mut s2);
    let reply2 = s2.execute("arch.inspect", &serde_json::json!({"rooms": true, "levels": true})).unwrap();

    let rooms1 = reply1.get("rooms").and_then(|v| v.as_array()).map(|v| v.len()).unwrap_or(0);
    let rooms2 = reply2.get("rooms").and_then(|v| v.as_array()).map(|v| v.len()).unwrap_or(0);
    assert_eq!(rooms1, rooms2, "room count differs between replays: {} vs {}", rooms1, rooms2);

    let levels1 = reply1.get("levels").and_then(|v| v.as_array()).map(|v| v.len()).unwrap_or(0);
    let levels2 = reply2.get("levels").and_then(|v| v.as_array()).map(|v| v.len()).unwrap_or(0);
    assert_eq!(levels1, levels2, "level count differs between replays: {} vs {}", levels1, levels2);
}

#[test]
fn the_showcase_has_its_phase17_details() {
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let mut session = Session::new();
    session.new_drawing(true);
    replay(&script, &mut session);

    let reply = session
        .execute("arch.inspect", &serde_json::json!({"stairs": true, "railings": true, "openings": true, "decks": true, "rooms": true}))
        .unwrap();
    println!("phase17 reply: {}", serde_json::to_string_pretty(&reply).unwrap());

    let stairs: Vec<Value> = reply.get("stairs").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    assert_eq!(stairs.len(), 2, "expected exactly two stairs, found {}", stairs.len());
    let railings: Vec<Value> = reply.get("railings").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    for stair in &stairs {
        let auto_count = railings.iter().filter(|r| boold(r, "auto", false) && field_text(r, "follows") == field_text(stair, "id")).count();
        assert!(auto_count >= 2, "stair {} has only {} auto-following railings", field_text(stair, "id"), auto_count);
    }
    assert!(
        railings.iter().filter(|r| (strd(r, "host", "") == "stairLeft" || strd(r, "host", "") == "stairRight") && boold(r, "auto", false)).count()
            >= 4,
        "expected automatic stair railings on both sides"
    );

    let openings: Vec<Value> = reply.get("openings").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let grille_count = openings
        .iter()
        .filter(|opening| {
            let Some(grille) = opening.get("grille").filter(|v| !v.is_null()) else {
                return false;
            };
            let across = grille.get("across").and_then(|v| v.as_u64()).unwrap_or(0) as i64;
            let down = grille.get("down").and_then(|v| v.as_u64()).unwrap_or(0) as i64;
            across * down > 1
        })
        .count();
    assert!(grille_count >= 3, "expected at least three grilles with across * down > 1, found {}", grille_count);
    assert!(openings.iter().any(|o| strd(o, "top", "") == "round"), "expected a round-topped opening");

    let decks: Vec<Value> = reply.get("decks").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let deck = decks.iter().find(|d| strd(d, "name", "") == "Rear deck").expect("Rear deck not found");
    assert!(deck.get("ledger").is_some_and(|ledger| ledger.is_object()), "deck ledger is empty");
    let joists = deck.get("joists").and_then(|v| v.as_array()).unwrap();
    assert!(joists.len() >= 9, "deck has {} joists, expected at least 9", joists.len());
    assert!(!deck.get("posts").and_then(|v| v.as_array()).unwrap_or(&Vec::new()).is_empty(), "deck posts are empty");
    let deck_railing =
        railings.iter().find(|r| strd(r, "host", "") == "deck" && field_text(r, "follows") == field_text(deck, "id")).expect("deck railing missing");
    assert_eq!(strd(deck_railing, "host", ""), "deck", "deck railing host should be `deck`");

    let _ = session.execute("export", &serde_json::json!({"path": "phase17-showcase.glb", "level": "Entry"})).unwrap();
}

#[test]
fn the_showcase_inspect_has_expected_keys() {
    let (_total, _replayed, reply) = house();
    for key in ["walls", "rooms", "levels", "stairs", "lights", "site"] {
        assert!(reply.get(key).is_some(), "arch.inspect reply missing `{}`", key);
    }
}

/// The deferral helper ends with fresh, non-stale views and with deferral switched off.
#[test]
fn the_showcase_replay_defers_views_and_ends_fresh() {
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let mut session = Session::new();
    session.new_drawing(true);
    replay(&script, &mut session);

    assert!(!session.views_deferred(), "views_deferred() should be false at the end of the replay helper");

    let doc = session.doc().unwrap();
    let views = &doc.arch.views;
    assert!(views.len() >= 6, "the replayed drawing has only {} views (need at least 6)", views.len());
    for view in views {
        assert!(!view.is_stale(doc.arch_revision), "view {:?} is stale against revision {}", view, doc.arch_revision);
    }
}

/// The cached house is stable (same pointer) and every step in the file replayed.
#[test]
fn the_showcase_house_is_replayed_once() {
    assert!(std::ptr::eq(house(), house()), "house() must be cached (same pointer) between calls");
    assert_eq!(house().0, house().1, "every step must replay (steps in file {} != steps replayed {})", house().0, house().1);
}

/// The house inspect reply carries every key the tests in this file read.
#[test]
fn the_showcase_house_inspect_has_all_keys() {
    let (_, _, reply) = house();
    for key in [
        "walls",
        "rooms",
        "levels",
        "openings",
        "cabinets",
        "fixtures",
        "furnishings",
        "surfaces",
        "electrical",
        "lights",
        "lightSets",
        "stairs",
        "site",
        "roofs",
        "trays",
        "soffits",
        "foundations",
        "cameras",
        "sunAngles",
        "views",
    ] {
        assert!(reply.get(key).is_some(), "house inspect reply missing `{}`", key);
    }
}

/// After a replay with view deferral, every view in the drawing is fresh (not stale).
#[test]
fn the_showcase_views_are_all_fresh_after_deferred_replay() {
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let mut session = Session::new();
    session.new_drawing(true);
    replay(&script, &mut session);

    let doc = session.doc().unwrap();
    let views = &doc.arch.views;
    let rev = doc.arch_revision;
    for (i, view) in views.iter().enumerate() {
        assert!(!view.is_stale(rev), "view {} is stale against revision {}", i, rev);
    }
}

/// Calling house() multiple times returns the same data (same room and wall counts).
#[test]
fn the_showcase_house_data_is_consistent_across_calls() {
    let h1 = house();
    let h2 = house();
    let h3 = house();

    let rooms = |h: &(usize, usize, Value)| h.2.get("rooms").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
    let walls = |h: &(usize, usize, Value)| h.2.get("walls").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);

    assert_eq!(rooms(h1), rooms(h2), "room count differs between house() calls");
    assert_eq!(rooms(h2), rooms(h3), "room count differs between house() calls");
    assert_eq!(walls(h1), walls(h2), "wall count differs between house() calls");
    assert_eq!(walls(h2), walls(h3), "wall count differs between house() calls");
}

/// An empty showcase script (no steps) replays without error.
#[test]
fn the_showcase_empty_script_replays() {
    install_io();
    let mut session = Session::new();
    session.new_drawing(true);
    let replayed = replay("[]", &mut session);
    assert_eq!(replayed, 0, "empty script should replay 0 steps");
}

/// A single-step showcase script replays without error.
#[test]
fn the_showcase_single_step_script_replays() {
    install_io();
    let mut session = Session::new();
    session.new_drawing(true);
    let script = r#"[{"cmd": "wall", "params": {"points": [[0, 0], [3, 0]], "thickness": 0.1}}]"#;
    let replayed = replay(script, &mut session);
    assert_eq!(replayed, 1, "single-step script should replay 1 step");
}

// ---- p21t-showcase-house: the redrawn house (rooms inside the shell, phase-16 ceilings and
// foundation, the kitchen cabinets of the drawing set, the phase-19 look cameras) ----

/// The rows of `key` in the cached `arch.inspect` reply (empty when the key is missing).
fn house_rows(key: &str) -> Vec<Value> {
    house().2.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default()
}

/// A `[x, y, ..]` point field of `v`.
fn xy(v: &Value, key: &str) -> Option<(f64, f64)> {
    let p = v.get(key)?.as_array()?;
    Some((p.first()?.as_f64()?, p.get(1)?.as_f64()?))
}

/// The room rows of the given type.
fn rooms_of_type(ty: &str) -> Vec<Value> {
    house_rows("rooms").into_iter().filter(|r| strd(r, "type", "") == ty).collect()
}

/// One replay of the showcase per binary for the 3D observables: the replies of the three
/// `build3d` calls the tests below read (wall and part probes, Main ceilings, Lower ceilings).
fn house_3d() -> &'static (Value, Value, Value) {
    static HOUSE3D: OnceLock<(Value, Value, Value)> = OnceLock::new();
    HOUSE3D.get_or_init(|| {
        install_io();
        let script = fs::read_to_string(script_path()).unwrap_or_else(|e| panic!("cannot read the showcase script: {e}"));
        let mut session = Session::new();
        session.new_drawing(true);
        replay(&script, &mut session);
        let probes = serde_json::json!({"probe": [
            [14.5, 0.0, 2.0], [26.0, 8.0, 2.0], [13.0, 16.0, 2.0], [0.0, 8.0, 2.0], [12.0, 0.0, 3.2],
            [11.4, 15.75, 3.7], [14.5, 0.25, -1.62], [26.0, 8.0, -1.62], [13.0, 16.0, -1.62], [0.0, 8.0, -1.62]
        ]});
        let p = session.execute("build3d", &probes).unwrap_or_else(|e| panic!("build3d probe refused: {e}"));
        let main = session
            .execute("build3d", &serde_json::json!({"ceiling": [[22, 5], [22, 13.5], [18.5, 11.5], [12, 10]], "level": "Main"}))
            .unwrap_or_else(|e| panic!("build3d ceiling (Main) refused: {e}"));
        let lower = session
            .execute("build3d", &serde_json::json!({"ceiling": [[4, 12], [15, 4]], "level": "Lower"}))
            .unwrap_or_else(|e| panic!("build3d ceiling (Lower) refused: {e}"));
        (p, main, lower)
    })
}

/// The part name a probe row reports ("" for a miss).
fn probe_part(reply: &Value, i: usize) -> String {
    reply.get("probe").and_then(|p| p.get(i)).map(|h| strd(h, "part", "")).unwrap_or_default()
}

/// Every room but the deck stands inside the 26 x 16 m shell, and the house has the rooms the
/// brief names, each once where it should be once.
#[test]
fn the_showcase_rooms_are_inside_the_shell() {
    let rooms = house_rows("rooms");
    assert!(!rooms.is_empty(), "no rooms");
    for r in &rooms {
        let ty = strd(r, "type", "");
        if ty == "deck" || ty == "porch" {
            continue;
        }
        for (x, y) in polygon_points(r.get("polygon").unwrap_or(&Value::Null)) {
            assert!(
                (-0.1 - 1e-6..=26.1 + 1e-6).contains(&x) && (-0.1 - 1e-6..=16.1 + 1e-6).contains(&y),
                "room {} ({}) has the point ({x}, {y}) outside the shell -0.1..26.1 x -0.1..16.1",
                strd(r, "id", "?"),
                strd(r, "name", "?")
            );
        }
    }
    for name in ["Dining", "Bedroom 2", "Bedroom 3"] {
        let n = rooms.iter().filter(|r| strd(r, "name", "") == name).count();
        assert_eq!(n, 1, "exactly one room named {name}, found {n}");
    }
    for (ty, want) in [("bedroom", 3), ("dining", 1), ("kitchen", 1), ("living", 1), ("bath", 2)] {
        assert_eq!(rooms_of_type(ty).len(), want, "rooms of type {ty}");
    }
}

/// No two walls are collinear, overlapping by more than 0.5 m in plan and 0.01 m in height
/// (the rule of tests/acceptance/phase-15/10_showcase_model.py).
#[test]
fn the_showcase_draws_no_wall_twice() {
    let walls = house_rows("walls");
    assert!(walls.len() >= 20, "only {} walls", walls.len());
    for (i, a) in walls.iter().enumerate() {
        for b in walls.iter().skip(i + 1) {
            if num(a, "thickness") <= 0.0 || num(b, "thickness") <= 0.0 {
                continue;
            }
            if num(a, "top").min(num(b, "top")) - num(a, "bottom").max(num(b, "bottom")) <= 0.01 {
                continue;
            }
            let (Some((ax0, ay0)), Some((ax1, ay1)), Some((bx0, by0)), Some((bx1, by1))) =
                (xy(a, "start"), xy(a, "end"), xy(b, "start"), xy(b, "end"))
            else {
                panic!("a wall row has no start/end: {a} / {b}");
            };
            let la = (ax1 - ax0).hypot(ay1 - ay0);
            if la < 1e-9 {
                continue;
            }
            let (ux, uy) = ((ax1 - ax0) / la, (ay1 - ay0) / la);
            let off = |x: f64, y: f64| ((x - ax0) * uy - (y - ay0) * ux).abs();
            if off(bx0, by0).max(off(bx1, by1)) > 0.05 {
                continue;
            }
            let t0 = (bx0 - ax0) * ux + (by0 - ay0) * uy;
            let t1 = (bx1 - ax0) * ux + (by1 - ay0) * uy;
            let overlap = la.min(t0.max(t1)) - 0.0f64.max(t0.min(t1));
            assert!(overlap <= 0.5, "walls {} and {} are one wall drawn twice (overlap {overlap:.2} m)", strd(a, "id", "?"), strd(b, "id", "?"));
        }
    }
}

/// Every cabinet stands in a kitchen, bath, laundry or utility room; the kitchen holds at least
/// three base cabinets with a counter, two wall cabinets and an island.
#[test]
fn the_showcase_cabinets_stand_in_the_kitchen() {
    let rooms = house_rows("rooms");
    let levels = house_rows("levels");
    let cabs = house_rows("cabinets");
    assert!(cabs.len() >= 10, "only {} cabinets", cabs.len());
    let level_name = |c: &Value| -> String {
        let idx = c.get("level").and_then(|l| l.as_i64());
        levels.iter().find(|l| l.get("index").and_then(|i| i.as_i64()) == idx).map(|l| strd(l, "name", "")).unwrap_or_else(|| strd(c, "level", ""))
    };
    let room_of = |c: &Value| -> Option<&Value> {
        let (x, y) = xy(c, "at")?;
        let turn = num(c, "rotation").to_radians();
        let half = num(c, "depth") / 2.0;
        let (cx, cy) = (x - turn.sin() * half, y + turn.cos() * half);
        let lv = level_name(c);
        rooms.iter().find(|r| strd(r, "level", "") == lv && strd(r, "type", "") != "openBelow" && in_room(r, cx, cy))
    };
    let (mut base, mut wall, mut island) = (0, 0, 0);
    for c in &cabs {
        let room = room_of(c).unwrap_or_else(|| panic!("cabinet {} stands in no room", strd(c, "id", "?")));
        let ty = strd(room, "type", "");
        assert!(
            ["kitchen", "bath", "laundry", "utility"].contains(&ty.as_str()),
            "cabinet {} stands in {} ({ty})",
            strd(c, "id", "?"),
            strd(room, "name", "?")
        );
        if ty == "kitchen" {
            let kind = strd(c, "kind", "");
            if kind == "base" && boold(c, "counter", false) {
                base += 1;
                if c.get("wall").is_none_or(|w| w.is_null()) {
                    island += 1;
                }
            }
            if kind == "wall" {
                wall += 1;
            }
        }
    }
    assert!(
        base >= 3 && wall >= 2 && island >= 1,
        "the kitchen holds {base} base cabinets with a counter, {wall} wall cabinets, {island} with no wall"
    );
}

/// The counter rule runs in the kitchen (GFCI outlets), and every room meets the spacing rule.
#[test]
fn the_showcase_kitchen_counters_have_gfci_outlets() {
    let kitchens = rooms_of_type("kitchen");
    assert_eq!(kitchens.len(), 1, "one kitchen");
    let kid = strd(&kitchens[0], "id", "");
    let gfci =
        house_rows("electrical").iter().filter(|e| strd(e, "room", "") == kid && strd(e, "kind", "").to_lowercase().starts_with("gfci")).count();
    assert!(gfci >= 2, "the kitchen {kid} has {gfci} GFCI outlet(s), need at least 2");
    let gaps = house_rows("outletGaps");
    assert!(!gaps.is_empty(), "arch.inspect reports no outletGaps");
    for g in &gaps {
        assert!(num(g, "maxGap") <= 1.8 + 1e-6, "room {} has an outlet gap of {}", strd(g, "room", "?"), num(g, "maxGap"));
    }
}

/// The Main level's exterior walls are solid beside their windows, and a window's column is open
/// at its glass (walls with openings on a raised level used to vanish).
#[test]
fn the_showcase_main_walls_are_solid_around_their_windows() {
    let (p, _, _) = house_3d();
    for (i, id) in ["w7", "w8", "w9", "w10"].iter().enumerate() {
        let hit = p.get("probe").and_then(|a| a.get(i)).cloned().unwrap_or(Value::Null);
        assert_eq!(strd(&hit, "part", ""), "wall", "probe {i} should be inside wall {id}: {hit}");
        assert_eq!(strd(&hit, "id", ""), *id, "probe {i} should be inside wall {id}: {hit}");
    }
    assert_ne!(probe_part(p, 4), "wall", "(12, 0, 3.2) is in the kitchen's south window, not in a wall");
}

/// The vaulted Living room, the Dining tray, the kitchen soffit, the suspended Utility ceiling
/// and the stem foundation with its footings.
#[test]
fn the_showcase_has_its_phase16_ceilings_and_foundation() {
    let (p, main, lower) = house_3d();
    let heights = |r: &Value| -> Vec<f64> {
        r.get("ceiling").and_then(|c| c.as_array()).map(|a| a.iter().map(|v| v.as_f64().unwrap_or(f64::NAN)).collect()).unwrap_or_default()
    };
    let (m, l) = (heights(main), heights(lower));
    assert_eq!(m.len(), 4, "four Main ceiling heights: {main}");
    for (got, want) in m.iter().zip([6.25, 4.2, 3.9, 3.9]) {
        assert!((got - want).abs() < 1e-6, "Main ceilings {m:?}, want [6.25, 4.2, 3.9, 3.9]");
    }
    assert_eq!(l.len(), 2, "two Lower ceiling heights: {lower}");
    for (got, want) in l.iter().zip([0.9713, 1.2]) {
        assert!((got - want).abs() < 1e-6, "Lower ceilings {l:?}, want [0.9713, 1.2]");
    }

    let dining = rooms_of_type("dining");
    let trays = house_rows("trays");
    assert_eq!(trays.len(), 1, "one tray");
    assert_eq!(strd(&trays[0], "room", ""), strd(&dining[0], "id", "?"), "the tray is in the Dining room");
    let soffits = house_rows("soffits");
    assert_eq!(soffits.len(), 1, "one soffit");
    let over: Vec<String> = soffits[0]
        .get("over")
        .and_then(|o| o.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    assert_eq!(over, ["cb6", "cb7"], "the soffit is over the kitchen's wall cabinets");
    assert!((num(&soffits[0], "bottom") - 3.634).abs() < 1e-3, "soffit bottom {}", num(&soffits[0], "bottom"));
    let foundations = house_rows("foundations");
    assert_eq!(foundations.len(), 1, "one foundation");
    assert_eq!(strd(&foundations[0], "type", ""), "stem");

    assert_eq!(probe_part(p, 5), "soffit", "(11.4, 15.75, 3.7) is in the soffit");
    for i in 6..10 {
        assert_eq!(probe_part(p, i), "footing", "probe {i} is in a footing: {}", p.get("probe").and_then(|a| a.get(i)).unwrap_or(&Value::Null));
    }
}

/// The dusk, noon-hdri and overview cameras with their own looks, the sun angles, the strips
/// and the light sets.
#[test]
fn the_showcase_has_its_phase19_look_cameras() {
    let reply = &house().2;
    let cams = house_rows("cameras");
    let names: Vec<String> = cams.iter().map(|c| strd(c, "name", "")).collect();
    assert_eq!(names, ["Front", "Back", "Kitchen", "Living", "dusk", "noon-hdri", "overview"]);
    let cam = |n: &str| cams.iter().find(|c| strd(c, "name", "") == n).cloned().unwrap_or(Value::Null);
    let look = |n: &str| cam(n).get("look").cloned().unwrap_or(Value::Null);

    let front = cam("Front");
    assert_eq!(front.get("eye").and_then(|e| e.get(1)).and_then(|v| v.as_f64()), Some(-24.0), "Front stands back: {front}");
    assert_eq!(num(&front, "eyeHeight"), 4.5);

    let dusk = look("dusk");
    assert_eq!(dusk.pointer("/sun/angle").and_then(|v| v.as_str()), Some("dusk"), "{dusk}");
    assert_eq!(strd(&dusk, "lightSet", ""), "Evening");
    assert_eq!(dusk.pointer("/exposure/ev100").and_then(|v| v.as_f64()), Some(4.0), "{dusk}");
    assert_eq!(dusk.pointer("/backdrop/kind").and_then(|v| v.as_str()), Some("sky"), "{dusk}");
    let d = cam("dusk");
    assert_eq!(xy(&d, "eye"), Some((16.0, -12.0)), "the dusk camera frames the south windows: {d}");

    let noon = look("noon-hdri");
    assert_eq!(strd(&noon, "technique", ""), "path");
    assert_eq!(noon.pointer("/backdrop/kind").and_then(|v| v.as_str()), Some("hdri"), "{noon}");
    assert_eq!(noon.pointer("/backdrop/image").and_then(|v| v.as_str()), Some("polyhaven:kloofendal-puresky"), "{noon}");

    let over = cam("overview");
    assert_eq!(strd(&over, "kind", ""), "overview");
    assert_eq!(over.pointer("/look/cutaway"), Some(&Value::Bool(true)), "{over}");

    let angles = house_rows("sunAngles");
    let dusk_angle = angles.iter().find(|a| strd(a, "name", "") == "dusk").expect("sun angle `dusk`");
    assert!((num(dusk_angle, "sunElevation") + 3.0).abs() < 1e-9, "dusk sun elevation {}", num(dusk_angle, "sunElevation"));
    assert!(angles.iter().any(|a| strd(a, "name", "") == "noon"), "sun angle `noon`");

    let lights = house_rows("lights");
    let strips = lights.iter().filter(|l| strd(l, "kind", "") == "strip").count();
    assert_eq!(strips, 3, "three strip lights");
    let sets = house_rows("lightSets");
    for name in ["Evening", "Day"] {
        assert!(sets.iter().any(|s| strd(s, "name", "") == name), "light set {name}");
    }
    let day =
        sets.iter().find(|s| strd(s, "name", "") == "Day").and_then(|s| s.get("lights")).and_then(|l| l.as_object()).cloned().unwrap_or_default();
    assert_eq!(day.len(), lights.len(), "Day names every light");
    assert!(day.values().all(|s| s.get("on") == Some(&Value::Bool(false))), "Day turns every light off");
    assert_eq!(strd(reply, "activeCamera", ""), "Front", "Front stays the active camera");
}

/// The four elevations and two sections are the script's last six steps, so later additions go
/// before them and pay no regeneration.
#[test]
fn the_showcase_views_are_its_last_steps() {
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let steps: Vec<ScriptStep> = serde_json::from_str(&script).expect("script is not valid JSON");
    assert!(steps.len() > 6);
    let (head, tail) = steps.split_at(steps.len() - 6);
    let is_view = |s: &ScriptStep| s.cmd == "elevation" || s.cmd == "section";
    assert_eq!(tail.iter().filter(|s| s.cmd == "elevation").count(), 4, "four elevations at the end");
    assert_eq!(tail.iter().filter(|s| s.cmd == "section").count(), 2, "two sections at the end");
    assert!(!head.iter().any(is_view), "an elevation or section step stands before the last six");
}

/// The kitchen cabinets live in bilevel.json: the drawing-set script adds none.
#[test]
fn the_drawing_set_script_adds_no_cabinet() {
    let path = script_path().with_file_name("phase-21-showcase.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let steps: Vec<ScriptStep> = serde_json::from_str(&text).expect("phase-21-showcase.json is not a step array");
    assert!(!steps.iter().any(|s| s.cmd.starts_with("cabinet")), "phase-21-showcase.json still adds a cabinet");
    assert_eq!(steps.iter().filter(|s| s.cmd == "sheet").count(), 14, "14 sheet steps");
    assert_eq!(steps.last().map(|s| s.cmd.as_str()), Some("layout.set"), "layout.set is the last step");
}

/// The deck room meets the same rules as every other room: a furnishing or fixture row,
/// a light row, an E-LITE device row, outlet gaps within 1.8 m, and no default materials.
#[test]
fn the_showcase_deck_meets_room_rules() {
    let reply = &house().2;
    let rooms: Vec<Value> = reply.get("rooms").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let deck = rooms.iter().find(|r| strd(r, "type", "") == "deck").expect("no deck room");
    let deck_id = strd(deck, "id", "");

    let furns: Vec<Value> = reply.get("furnishings").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let fixs: Vec<Value> = reply.get("fixtures").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let rows: Vec<&Value> = furns.iter().chain(fixs.iter()).filter(|r| field_text(r, "room") == deck_id).collect();
    assert!(!rows.is_empty(), "deck {} has no furnishing or fixture row", deck_id);

    let lights: Vec<Value> = reply.get("lights").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    assert!(lights.iter().any(|l| field_text(l, "room") == deck_id), "deck {} has no light row", deck_id);

    let devices: Vec<Value> = reply.get("electrical").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    assert!(
        devices.iter().any(|d| field_text(d, "room") == deck_id && strd(d, "layer", "") == "E-LITE"),
        "deck {} has no E-LITE device row",
        deck_id
    );

    let gaps: Vec<Value> = reply.get("outletGaps").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for g in &gaps {
        if field_text(g, "room") == deck_id {
            assert!(num(g, "maxGap") <= 1.8 + 1e-6, "deck {} has an outlet gap of {}", deck_id, num(g, "maxGap"));
        }
    }

    let surfaces: Vec<Value> = reply.get("surfaces").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for s in &surfaces {
        if field_text(s, "room") == deck_id {
            let material = strd(s, "material", "");
            assert!(!material.starts_with("default:"), "deck {} surface {} has default material {}", deck_id, strd(s, "surface", ""), material);
        }
    }
}

/// At least three windows carry grilles (across * down > 1) and exactly one window has a round top.
#[test]
fn the_showcase_windows_have_grilles_and_round_top() {
    let reply = &house().2;
    let openings: Vec<Value> = reply.get("openings").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let windows: Vec<&Value> = openings.iter().filter(|o| strd(o, "kind", "") == "window").collect();

    let grille_count = windows
        .iter()
        .filter(|w| {
            let Some(grille) = w.get("grille").filter(|v| !v.is_null()) else {
                return false;
            };
            let across = grille.get("across").and_then(|v| v.as_u64()).unwrap_or(0);
            let down = grille.get("down").and_then(|v| v.as_u64()).unwrap_or(0);
            across * down > 1
        })
        .count();
    assert!(grille_count >= 3, "only {} windows have grilles with across*down > 1", grille_count);

    let round_count = windows.iter().filter(|w| strd(w, "top", "") == "round").count();
    assert_eq!(round_count, 1, "expected exactly one round-topped window, found {}", round_count);
}

/// Each stair has an automatic railing on both its left and right sides, following the stair.
#[test]
fn the_showcase_stairs_have_pair_railings() {
    let reply = &house().2;
    let stairs: Vec<Value> = reply.get("stairs").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();
    let railings: Vec<Value> = reply.get("railings").and_then(|v| v.as_array()).unwrap().iter().cloned().collect();

    assert_eq!(stairs.len(), 2, "expected two stairs, found {}", stairs.len());
    for stair in &stairs {
        let stair_id = field_text(stair, "id");
        let left = railings.iter().find(|r| strd(r, "host", "") == "stairLeft" && boold(r, "auto", false) && field_text(r, "follows") == stair_id);
        let right = railings.iter().find(|r| strd(r, "host", "") == "stairRight" && boold(r, "auto", false) && field_text(r, "follows") == stair_id);
        assert!(left.is_some(), "stair {} has no auto stairLeft railing", stair_id);
        assert!(right.is_some(), "stair {} has no auto stairRight railing", stair_id);
    }
}
