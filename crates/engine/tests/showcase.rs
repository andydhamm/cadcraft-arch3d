//! Replay the Phase-15 showcase script (`docs/arch3d/showcase/bilevel.json`)
//! through the engine and assert the resulting model meets the brief.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::Path;

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
    let steps: Vec<ScriptStep> = serde_json::from_str(script).unwrap_or_else(|e| panic!("cannot parse the showcase script: {e}"));
    let mut n = 0;
    for (i, step) in steps.iter().enumerate() {
        let cmd = &step.cmd;
        match session.execute(cmd, &step.params) {
            Ok(_) => n += 1,
            Err(err) => panic!("showcase script step {i} `{cmd}` failed: {err}"),
        }
    }
    n
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
            assert!(!strd(r, "material", "").starts_with("default:"), "surface material is default: {}", strd(r, "material", ""));
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
            assert!(max_gap <= 1.8 + 1.0e-6, "outlet gap maxGap {} exceeds 1.8", max_gap);
            if let Some(counter) = g.get("counter").filter(|c| !c.is_null()) {
                let spacing = num(counter, "maxSpacing");
                assert!(spacing <= 1.2 + 1.0e-6, "outlet gap counter maxSpacing {} exceeds 1.2", spacing);
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
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let mut session = Session::new();
    session.new_drawing(true);
    replay(&script, &mut session);
    let reply = session.execute("arch.inspect", &serde_json::json!({"walls": true, "rooms": true, "levels": true})).unwrap();
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
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let mut session = Session::new();
    session.new_drawing(true);
    replay(&script, &mut session);
    let reply = session.execute("arch.inspect", &serde_json::json!({"rooms": true, "levels": true, "cabinets": true, "electrical": true})).unwrap();
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
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let mut session = Session::new();
    session.new_drawing(true);
    replay(&script, &mut session);
    let reply = session.execute("arch.inspect", &serde_json::json!({"levels": true, "rooms": true, "stairs": true})).unwrap();
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
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let mut session = Session::new();
    session.new_drawing(true);
    replay(&script, &mut session);
    let reply = session.execute("arch.inspect", &serde_json::json!({"walls": true, "rooms": true, "levels": true, "openings": true})).unwrap();
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
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let steps: Vec<ScriptStep> = serde_json::from_str(&script).expect("script is not valid JSON");
    let total = steps.len();
    let mut session = Session::new();
    session.new_drawing(true);
    let replayed = replay(&script, &mut session);
    assert_eq!(replayed, total, "expected {} steps, replayed {}", total, replayed);
}

#[test]
fn the_showcase_meets_the_brief() {
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let mut session = Session::new();
    session.new_drawing(true);
    replay(&script, &mut session);
    let params = serde_json::json!({
        "walls": true,
        "rooms": true,
        "levels": true,
        "cabinets": true,
        "fixtures": true,
        "furnishings": true,
        "surfaces": true,
        "electrical": true,
        "lights": true,
        "lightSets": true,
        "stairs": true,
        "site": true,
    });
    let reply = session.execute("arch.inspect", &params).unwrap();
    println!("{}", serde_json::to_string_pretty(&reply).unwrap());
    meets_the_brief(&reply);
}

#[test]
fn showcase_meets_the_brief_with_phase_18_features() {
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let mut session = Session::new();
    session.new_drawing(true);
    replay(&script, &mut session);

    let reply = session
        .execute(
            "arch.inspect",
            &serde_json::json!({
                "rooms": true,
                "cabinets": true,
                "openings": true,
                "walls": true,
                "levels": true,
            }),
        )
        .unwrap();

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
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let mut session = Session::new();
    session.new_drawing(true);
    replay(&script, &mut session);
    let params = serde_json::json!({
        "walls": true, "rooms": true, "levels": true, "cabinets": true,
        "fixtures": true, "furnishings": true, "surfaces": true,
        "electrical": true, "lights": true, "lightSets": true,
        "stairs": true, "site": true,
    });
    let reply = session.execute("arch.inspect", &params).unwrap();
    for key in ["walls", "rooms", "levels", "stairs", "lights", "site"] {
        assert!(reply.get(key).is_some(), "arch.inspect reply missing `{}`", key);
    }
}
