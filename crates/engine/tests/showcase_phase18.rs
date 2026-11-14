//! Compact digest of the Phase-18 showcase house.
//!
//! `house()` replays the showcase script exactly once per test binary and caches
//! (the number of steps in the file, the steps replayed, and the full
//! `arch.inspect` reply). Every test in this file goes through `house()`, so
//! the (slow) replay and inspect run only once and a failing assertion shows
//! a short `digest` of the house instead of the whole inspect JSON.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use cadcraft_engine::Session;
use cadcraft_engine::cmd::file::{IoHooks, set_io};
use serde::Deserialize;
use serde_json::Value;

/// One step of a showcase script (the same shape used by `showcase.rs`).
#[derive(Debug, Deserialize)]
struct ScriptStep {
    cmd: String,
    #[serde(default = "default_params")]
    params: Value,
}

fn default_params() -> Value {
    Value::Object(serde_json::Map::new())
}

/// Install the headless I/O hooks so the engine runs without real files, the
/// same way `crates/engine/tests/user_guide.rs` does.
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
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/arch3d/showcase/bilevel.json").to_path_buf()
}

/// Replay every step of the JSON script, panicking (and naming the command) on
/// the first refused step. Returns the number of steps replayed.
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

/// Replays the showcase script once per binary and caches the results.
///
/// Returns `(steps in the file, steps replayed, the full `arch.inspect` reply)`.
/// No test replays again: the replay is slow and a re-run can trip the gate.
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
            "surfaces": true,
            "countertops": true,
            "backsplashes": true,
            "moldings": true,
            "electrical": true,
        });
        let reply = session.execute("arch.inspect", &params).unwrap();
        (steps.len(), replayed, reply)
    });
    result
}

/// Text of `v.get(key)`; scalars render as-is, everything else (and a missing
/// key) renders as `-`. Never panics on a missing key.
fn field(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => "-".to_string(),
    }
}

/// `(x,y)` for a `[x, y]` value; `-` otherwise.
fn point(v: &Value) -> String {
    if let Some(pair) = v.as_array() {
        if let (Some(x), Some(y)) = (pair.get(0).and_then(|n| n.as_f64()), pair.get(1).and_then(|n| n.as_f64())) {
            return format!("({},{})", x, y);
        }
    }
    "-".to_string()
}

/// `(minx,miny)-(maxx,maxy)` of the `polygon` array in `v`; `(-,-)-(-,-)`
/// when it cannot be read.
fn ring_bbox(v: &Value) -> String {
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    if let Some(arr) = v.get("polygon").and_then(|a| a.as_array()) {
        for p in arr {
            if let Some(pair) = p.as_array() {
                if let Some(x) = pair.get(0).and_then(|n| n.as_f64()) {
                    xs.push(x);
                }
                if let Some(y) = pair.get(1).and_then(|n| n.as_f64()) {
                    ys.push(y);
                }
            }
        }
    }
    if xs.is_empty() || ys.is_empty() {
        return "(-,-)-(-,-)".to_string();
    }
    format!(
        "({},{})-({},{})",
        xs.iter().copied().reduce(f64::min).unwrap_or(f64::NAN),
        ys.iter().copied().reduce(f64::min).unwrap_or(f64::NAN),
        xs.iter().copied().reduce(f64::max).unwrap_or(f64::NAN),
        ys.iter().copied().reduce(f64::max).unwrap_or(f64::NAN),
    )
}

/// A value that may be a point, a point list, or a scalar, rendered as
/// `(x,y)` / `(minx,miny)-(maxx,maxy)` / its text / `-`.
fn coord(v: &Value) -> String {
    match v {
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(a) => {
            let mut xs = Vec::new();
            let mut ys = Vec::new();
            for e in a {
                if let Some(p) = e.as_array() {
                    if let (Some(x), Some(y)) = (p.get(0).and_then(|n| n.as_f64()), p.get(1).and_then(|n| n.as_f64())) {
                        xs.push(x);
                        ys.push(y);
                    }
                }
            }
            if xs.is_empty() || ys.is_empty() {
                "-".to_string()
            } else if xs.len() == 1 {
                format!("({},{})", xs[0], ys[0])
            } else {
                format!(
                    "({},{})-({},{})",
                    xs.iter().copied().reduce(f64::min).unwrap_or(f64::NAN),
                    ys.iter().copied().reduce(f64::min).unwrap_or(f64::NAN),
                    xs.iter().copied().reduce(f64::max).unwrap_or(f64::NAN),
                    ys.iter().copied().reduce(f64::max).unwrap_or(f64::NAN),
                )
            }
        }
        _ => "-".to_string(),
    }
}

/// True if the object, or any of its `edges` entries, carries a truthy `waterfall`.
fn has_waterfall(v: &Value) -> bool {
    if let Some(b) = v.get("waterfall").and_then(|x| x.as_bool()) {
        return b;
    }
    if let Some(edges) = v.get("edges").and_then(|x| x.as_array()) {
        for e in edges {
            if let Some(b) = e.get("waterfall").and_then(|x| x.as_bool()) {
                return b;
            }
        }
    }
    false
}

/// The `type` token of a `room ` line, or `None`.
fn room_type(line: &str) -> Option<String> {
    let after_room = line.strip_prefix("room ")?;
    let level_pos = after_room.find(" level=")?;
    after_room[..level_pos].split_whitespace().last().map(|t| t.to_string())
}

/// Build the digest: one line per object, using only `.get()` so a missing key
/// prints `-` instead of panicking.
fn digest_lines(reply: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    // Rooms: `room <id> <name> <type> level=<level> bbox=...`
    if let Some(arr) = reply.get("rooms").and_then(|a| a.as_array()) {
        for r in arr.iter() {
            out.push(format!("room {} {} {} level={} bbox={}", field(r, "id"), field(r, "name"), field(r, "type"), field(r, "level"), ring_bbox(r),));
        }
    }
    // Walls: `wall <id> <start> -> <end> t=... level=... glass=... split=...`
    if let Some(arr) = reply.get("walls").and_then(|a| a.as_array()) {
        for w in arr.iter() {
            out.push(format!(
                "wall {} {} -> {} t={} level={} glass={} split={}",
                field(w, "id"),
                point(w.get("start").unwrap_or(&Value::Null)),
                point(w.get("end").unwrap_or(&Value::Null)),
                field(w, "thickness"),
                field(w, "level"),
                field(w, "glass"),
                field(w, "split"),
            ));
        }
    }
    // Cabinets
    if let Some(arr) = reply.get("cabinets").and_then(|a| a.as_array()) {
        for c in arr.iter() {
            out.push(format!(
                "cab {} {} at={} w={} wall={} room={} corner={} filler={} crown={} shelves={} face={}",
                field(c, "id"),
                field(c, "kind"),
                coord(c.get("at").unwrap_or(&Value::Null)),
                field(c, "width"),
                field(c, "alongWall"),
                field(c, "room"),
                field(c, "corner"),
                field(c, "filler"),
                field(c, "crown"),
                field(c, "shelves"),
                field(c, "face"),
            ));
        }
    }
    // Fixtures
    if let Some(arr) = reply.get("fixtures").and_then(|a| a.as_array()) {
        for fx in arr.iter() {
            out.push(format!(
                "fixture {} {} at={} room={}",
                field(fx, "id"),
                field(fx, "kind"),
                coord(fx.get("at").unwrap_or(&Value::Null)),
                field(fx, "room"),
            ));
        }
    }
    // Openings
    if let Some(arr) = reply.get("openings").and_then(|a| a.as_array()) {
        for o in arr.iter() {
            out.push(format!("opening {} {} style={} host={}", field(o, "id"), field(o, "kind"), field(o, "style"), field(o, "host"),));
        }
    }
    // Countertops and backsplashes
    if let Some(arr) = reply.get("countertops").and_then(|a| a.as_array()) {
        for ct in arr.iter() {
            out.push(format!(
                "countertop {} auto={} cabinets={} waterfall={}",
                field(ct, "id"),
                field(ct, "auto"),
                field(ct, "cabinets"),
                has_waterfall(ct),
            ));
        }
    }
    if let Some(arr) = reply.get("backsplashes").and_then(|a| a.as_array()) {
        for bs in arr.iter() {
            out.push(format!("backsplash {} segments={}", field(bs, "id"), bs.get("segments").and_then(|s| s.as_u64()).unwrap_or(0),));
        }
    }
    // Surfaces: DEFAULT (still default) and FLOOR (room floor) rows
    if let Some(arr) = reply.get("surfaces").and_then(|a| a.as_array()) {
        for r in arr.iter() {
            let surface = field(r, "surface");
            let material = field(r, "material");
            if material.starts_with("default:") {
                out.push(format!("DEFAULT {} {}", surface, material));
            } else if surface.contains("/floor") {
                out.push(format!("FLOOR {} {}", surface, material));
            }
        }
    }
    // Outlet gaps that exceed the limits
    if let Some(arr) = reply.get("outletGaps").and_then(|a| a.as_array()) {
        for g in arr.iter() {
            let max_gap = g.get("maxGap").and_then(|n| n.as_f64()).unwrap_or(f64::NAN);
            let counter_spacing = g.get("counter").and_then(|c| c.get("maxSpacing")).and_then(|n| n.as_f64()).unwrap_or(f64::NAN);
            if max_gap > 1.8 || counter_spacing > 1.2 {
                let fmt = |x: f64| {
                    if x.is_finite() { x.to_string() } else { "-".to_string() }
                };
                out.push(format!("GAP {} maxGap={} counter={}", field(g, "room"), fmt(max_gap), fmt(counter_spacing),));
            }
        }
    }
    // Moldings, restricted to kitchen and bath rooms
    let kitchen_bath: BTreeSet<String> = if let Some(rooms) = reply.get("rooms").and_then(|a| a.as_array()) {
        rooms
            .iter()
            .filter_map(|r| {
                if r.get("type").and_then(|t| t.as_str()) == Some("kitchen") || r.get("type").and_then(|t| t.as_str()) == Some("bath") {
                    r.get("id").and_then(|t| t.as_str()).map(|t| t.to_string())
                } else {
                    None
                }
            })
            .collect()
    } else {
        BTreeSet::new()
    };
    if let Some(arr) = reply.get("moldings").and_then(|a| a.as_array()) {
        for m in arr.iter() {
            let room = field(m, "room");
            if kitchen_bath.contains(&room) {
                out.push(format!("molding {} {} len={}", room, field(m, "kind"), field(m, "length"),));
            }
        }
    }
    out
}

/// One-line-per-object digest of the inspect reply; never panics on a missing
/// key (those print as `-`).
fn digest(reply: &Value) -> String {
    digest_lines(reply).join("\n")
}

/// Every step of the (phase-18) showcase script replays without being refused.
/// `house()`'s replay panics on the first refused step, which keeps this honest.
#[test]
fn showcase_replays_with_its_phase18_steps() {
    let (total, replayed, _reply) = house();
    assert_eq!(total, replayed, "expected {} steps, replayed {}", total, replayed);
}

/// The digest names a kitchen and two baths, plus a wall and a cabinet line;
/// no line is longer than 240 chars. Panics with the digest on failure.
#[test]
fn showcase_digest_names_rooms_walls_and_cabinets() {
    let (_total, _replayed, reply) = house();
    let digest_text = digest(&reply);
    let lines: Vec<&str> = digest_text.lines().collect();
    let has = |needle: &str| lines.iter().any(|l| l.starts_with(needle));
    assert!(has("room"), "digest has no `room` line\n{}", digest_text);
    assert!(has("wall"), "digest has no `wall` line\n{}", digest_text);
    assert!(has("cab"), "digest has no `cab` line\n{}", digest_text);
    let kitchen = lines.iter().any(|l| room_type(l).as_deref() == Some("kitchen"));
    assert!(kitchen, "digest has no kitchen-typed `room` line\n{}", digest_text);
    let bath_count = lines.iter().filter_map(|l| room_type(l)).filter(|t| t == "bath").count();
    assert!(bath_count >= 2, "digest has only {} bath-typed `room` line(s), need at least 2\n{}", bath_count, digest_text);
    for line in lines {
        assert!(line.len() <= 240, "digest line too long ({} chars): {}\n{}", line.len(), line, digest_text);
    }
}

/// Hand-run entry point: prints the digest. The following segments read the
/// house with
/// `cargo test -p cadcraft-engine --test showcase_phase18 print_showcase_digest -- --ignored --nocapture`.
#[test]
#[ignore]
fn print_showcase_digest() {
    let (_total, _replayed, reply) = house();
    println!("{}", digest(&reply));
}
