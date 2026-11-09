//! Replay the Phase-15 showcase script (`docs/arch3d/showcase/bilevel.json`)
//! through the engine and assert the resulting model meets the brief.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use cadcraft_engine::Session;
use serde_json::Value;

/// Install the headless I/O hooks so the engine runs without real files, the same
/// way `crates/engine/tests/user_guide.rs::install_io` does.
fn install_io() {
    cadcraft_engine::set_io(cadcraft_engine::IoHooks {
        read: |_, _| Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no file reads in this test")),
        write: |_, _| Ok(b"stand-in export".to_vec()),
        plot: None,
    });
    cadcraft_engine::library::set_library_io(cadcraft_engine::library::LibraryIo {
        read_glb: cadcraft_io::gltf_read::read_glb,
        read_image: cadcraft_io::image_read::read_image,
        encode_png: cadcraft_io::png::encode_png,
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
    v.get(key).and_then(|x| x.as_str()).unwrap_or(default).to_string()
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

/// Rows (furnishing or fixture) whose "room" field matches `rid`.
fn rows_for(rid: &str, rows: &[Value]) -> Vec<&Value> {
    rows.iter().filter(|r| field_text(r, "room") == rid).collect()
}

/// Replay every step of the JSON script. Panics on the first failure, naming the
/// step index, command id and error text. Returns the number of steps replayed.
fn replay(script: &str, session: &mut Session) -> usize {
    let steps: Vec<Value> = serde_json::from_str::<Vec<Value>>(script).unwrap_or_else(|e| panic!("cannot parse the showcase script: {e}"));
    let mut n = 0;
    for (i, step) in steps.iter().enumerate() {
        let cmd = field_text(step, "cmd");
        let params = step.get("params").cloned().unwrap_or_default();
        match session.execute(&cmd, &params) {
            Ok(_) => n += 1,
            Err(err) => panic!("showcase script step {i} `{cmd}` failed: {err}"),
        }
    }
    n
}

/// Assert that the replayed model satisfies the showcase brief.
fn meets_the_brief(reply: &Value) {
    let levels: Vec<&Value> = match reply.get("levels") {
        Some(a) if a.is_array() => a.as_array().iter().cloned().collect(),
        _ => panic!("arch.inspect reply has no `levels` array"),
    };

    // ---- Levels ----
    let landing = levels.iter().find(|l| strd(l, "kind", "") == "landing");
    assert!(landing.is_some(), "no level of kind `landing`");
    let landing = landing.unwrap();
    let floors: Vec<&Value> = levels.iter().filter(|l| strd(l, "kind", "") == "floor").collect();
    assert_eq!(floors.len(), 2, "expected exactly two `floor` levels, found {}", floors.len());
    let (lower, upper) =
        if num(landing, "elevation") < 0.0 && num(lower, "elevation") > num(upper, "elevation") { (lower, upper) } else { (upper, lower) };
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
    let lo = std::min(num(lower, "elevation"), num(upper, "elevation"));
    let hi = std::max(num(lower, "elevation"), num(upper, "elevation"));
    let le = num(landing, "elevation");
    assert!(lo < le && le < hi, "landing elevation {} is not strictly between the floor elevations {..}", le, lo, hi);
    let area = num(reply, "aboveGradeArea");
    assert!(area >= 371.6, "aboveGradeArea {} is below 371.6", area);

    // ---- Stairs ----
    let stairs: Vec<&Value> = match reply.get("stairs") {
        Some(a) if a.is_array() => a.as_array().iter().cloned().collect(),
        _ => panic!("arch.inspect reply has no `stairs` array"),
    };
    let landing_idx = field_text(landing, "index")
        .parse::<i64>()
        .unwrap_or_else(|_| levels.iter().position(|l| std::ptr::eq(l as *const Value, landing as *const Value)).as_i64());
    let lower_idx = field_text(lower, "index")
        .parse::<i64>()
        .unwrap_or_else(|_| levels.iter().position(|l| std::ptr::eq(l as *const Value, lower as *const Value)).as_i64());
    let upper_idx = field_text(upper, "index")
        .parse::<i64>()
        .unwrap_or_else(|_| levels.iter().position(|l| std::ptr::eq(l as *const Value, upper as *const Value)).as_i64());
    let up_stair = stairs.iter().any(|st| {
        field_text(st, "fromLevel").parse::<i64>().ok() == Some(landing_idx) && field_text(st, "toLevel").parse::<i64>().ok() == Some(upper_idx)
    });
    let down_stair = stairs.iter().any(|st| {
        field_text(st, "fromLevel").parse::<i64>().ok() == Some(lower_idx) && field_text(st, "toLevel").parse::<i64>().ok() == Some(landing_idx)
    });
    assert!(up_stair, "no stair from landing (idx {}) to main (idx {})", landing_idx, upper_idx);
    assert!(down_stair, "no stair from lower (idx {}) to landing (idx {})", lower_idx, landing_idx);

    // ---- Rooms ----
    let rooms: Vec<&Value> = match reply.get("rooms") {
        Some(a) if a.is_array() => a.as_array().iter().cloned().collect(),
        _ => panic!("arch.inspect reply has no `rooms` array"),
    };
    let count = |ty: &str| rooms.iter().filter(|r| strd(r, "type", "") == ty).count();
    for (ty, want) in [("living", 1usize), ("kitchen", 1), ("dining", 1), ("family", 1), ("bath", 2), ("bedroom", 3)] {
        assert_eq!(count(ty), want, "expected {} `{} ` room(s), found {}", want, ty, count(ty));
    }

    let furns: Vec<&Value> = match reply.get("furnishings") {
        Some(a) if a.is_array() => a.as_array().iter().cloned().collect(),
        _ => panic!("arch.inspect reply has no `furnishings` array"),
    };
    let fixs: Vec<&Value> = match reply.get("fixtures") {
        Some(a) if a.is_array() => a.as_array().iter().cloned().collect(),
        _ => panic!("arch.inspect reply has no `fixtures` array"),
    };
    // Every non-closet/openBelow room must be served by at least one furnishing or fixture row.
    for r in rooms.iter() {
        let ry = strd(r, "type", "");
        if ry == "closet" || ry == "openBelow" {
            continue;
        }
        let rid = room_id(r);
        let served = rows_for(rid, &furns).iter().any(|_| true) || rows_for(rid, &fixs).iter().any(|_| true);
        assert!(served, "room `{}` (type `{}`) has no furnishing or fixture row", rid, ry);
    }

    // ---- Kitchen coverage ----
    if let Some(kit) = rooms.iter().find(|r| strd(r, "type", "") == "kitchen") {
        let rid = room_id(kit);
        let rows: Vec<&Value> = rows_for(rid, &furns).iter().chain(rows_for(rid, &fixs).iter()).collect();
        assert!(rows.iter().any(|r| row_text(r).contains("range") || row_text(r).contains("cooktop")), "kitchen `{}` lacks a range|cooktop row", rid);
        assert!(rows.iter().any(|r| row_text(r).contains("refrigerator")), "kitchen `{}` lacks a refrigerator row", rid);
        assert!(rows.iter().any(|r| row_text(r).contains("sink")), "kitchen `{}` lacks a sink row", rid);
        assert!(rows.iter().any(|r| row_text(r).contains("dishwasher")), "kitchen `{}` lacks a dishwasher row", rid);
    }

    // ---- Bath coverage ----
    for b in rooms.iter().filter(|r| strd(r, "type", "") == "bath") {
        let rid = room_id(b);
        let rows: Vec<&Value> = rows_for(rid, &furns).iter().chain(rows_for(rid, &fixs).iter()).collect();
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
        let rows: Vec<&Value> = rows_for(rid, &furns).iter().chain(rows_for(rid, &fixs).iter()).collect();
        assert!(rows.iter().any(|r| row_text(r).contains("bed")), "bedroom `{}` lacks a bed item", rid);
    }

    // ---- Surfaces ----
    if let Some(surf) = reply.get("surfaces").and_then(|v| v.as_array()) {
        for r in surf.iter() {
            assert!(!strd(r, "material", "").starts_with("default:"), "surface material is default: {}", strd(r, "material", ""));
        }
    }

    // ---- Lights / devices / circuits ----
    let lights: Vec<&Value> = match reply.get("lights") {
        Some(a) if a.is_array() => a.as_array().iter().cloned().collect(),
        _ => panic!("arch.inspect reply has no `lights` array"),
    };
    let devices: Vec<&Value> = match reply.get("devices") {
        Some(a) if a.is_array() => a.as_array().iter().cloned().collect(),
        _ => panic!("arch.inspect reply has no `devices` array"),
    };
    let circuits: Vec<&Value> = match reply.get("circuits") {
        Some(a) if a.is_array() => a.as_array().iter().cloned().collect(),
        _ => panic!("arch.inspect reply has no `circuits` array"),
    };
    let all_circuit_lights: HashSet<String> = circuits
        .iter()
        .flat_map(|c| match c.get("lights") {
            Some(a) if a.is_array() => a.as_array().iter().map(|l| field_text(l, "id")),
            _ => Vec::<String>::new(),
        })
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
            if let Some(counter) = g.get("counter") {
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
fn the_showcase_script_replays() {
    install_io();
    let script = fs::read_to_string(script_path()).expect("cannot read the showcase script");
    let mut session = Session::new();
    session.new_drawing(true);
    let steps = replay(&script, &mut session);
    assert!(steps > 0, "the showcase script replayed no steps");
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
    meets_the_brief(&reply);
}
