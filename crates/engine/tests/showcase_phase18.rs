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

#[test]
fn digest_handles_empty_reply() {
    let reply = serde_json::json!({});
    let d = digest(&reply);
    assert_eq!(d, "");
}

#[test]
fn digest_handles_missing_fields() {
    let reply = serde_json::json!({
        "rooms": [{"id": "r1"}],
        "walls": [{"id": "w1"}],
        "cabinets": [{"id": "c1"}],
        "fixtures": [{"id": "f1"}],
        "openings": [{"id": "o1"}],
        "countertops": [{"id": "ct1"}],
        "backsplashes": [{"id": "bs1"}],
        "surfaces": [{"surface": "room/r1/floor"}],
        "outletGaps": [{"room": "r1", "maxGap": 2.0}],
        "moldings": [{"room": "r1"}]
    });
    let d = digest(&reply);
    let lines: Vec<&str> = d.lines().collect();
    assert!(lines.iter().any(|l| l.starts_with("room r1 - - level=- bbox=(-,-)-(-,-)")), "missing room line: {}", d);
    assert!(lines.iter().any(|l| l.starts_with("wall w1 - -> - t=- level=- glass=- split=-")), "missing wall line: {}", d);
    assert!(
        lines.iter().any(|l| l.starts_with("cab c1 - at=- w=- wall=- room=- corner=- filler=- crown=- shelves=- face=-")),
        "missing cab line: {}",
        d
    );
    assert!(lines.iter().any(|l| l.starts_with("fixture f1 - at=- room=-")), "missing fixture line: {}", d);
    assert!(lines.iter().any(|l| l.starts_with("opening o1 - style=- host=-")), "missing opening line: {}", d);
    assert!(lines.iter().any(|l| l.starts_with("countertop ct1 auto=- cabinets=- waterfall=false")), "missing countertop line: {}", d);
    assert!(lines.iter().any(|l| l.starts_with("backsplash bs1 segments=0")), "missing backsplash line: {}", d);
    assert!(lines.iter().any(|l| l.starts_with("FLOOR room/r1/floor -")), "missing floor line: {}", d);
    assert!(lines.iter().any(|l| l.starts_with("GAP r1 maxGap=2 counter=-")), "missing gap line: {}", d);
}

#[test]
fn digest_handles_nan_coordinates() {
    let nan = serde_json::json!(f64::NAN);
    let reply = serde_json::json!({
        "rooms": [{
            "id": "r1",
            "name": "Test",
            "type": "bedroom",
            "level": "1",
            "polygon": [[nan, nan]]
        }]
    });
    let d = digest(&reply);
    let lines: Vec<&str> = d.lines().collect();
    let room_line = lines.iter().find(|l| l.starts_with("room r1")).expect("missing room line: {}", d);
    assert!(room_line.contains("NaN"), "bbox should contain NaN: {}", room_line);
}

#[test]
fn room_type_parses_various_formats() {
    assert_eq!(room_type("room r1 Kitchen kitchen level=1 bbox=(0,0)-(10,10)"), Some("kitchen".to_string()));
    assert_eq!(room_type("room r1 - bath level=1 bbox=(0,0)-(10,10)"), Some("bath".to_string()));
    assert_eq!(room_type("room - Living Room living level=1 bbox=(0,0)-(10,10)"), Some("living".to_string()));
    assert_eq!(room_type("room r1 - - level=1 bbox=(0,0)-(10,10)"), Some("-".to_string()));
    assert_eq!(room_type("wall w1 - -> - t=- level=- glass=- split=-"), None);
    assert_eq!(room_type("room r1"), None);
}

#[test]
fn has_waterfall_detects_all_cases() {
    assert!(!has_waterfall(&serde_json::json!({})));
    assert!(has_waterfall(&serde_json::json!({"waterfall": true})));
    assert!(!has_waterfall(&serde_json::json!({"waterfall": false})));
    assert!(has_waterfall(&serde_json::json!({"edges": [{"waterfall": true}, {"waterfall": false}]})));
    assert!(!has_waterfall(&serde_json::json!({"edges": [{"waterfall": false}, {"waterfall": false}]})));
    assert!(has_waterfall(&serde_json::json!({"waterfall": true, "edges": [{"waterfall": false}]})));
    assert!(!has_waterfall(&serde_json::json!({"waterfall": "true"})));
}

#[test]
fn digest_line_length_bounded() {
    let long_id = "r".repeat(40);
    let long_name = "N".repeat(40);
    let long_type = "t".repeat(40);
    let long_level = "l".repeat(40);
    let reply = serde_json::json!({
        "rooms": [{
            "id": long_id,
            "name": long_name,
            "type": long_type,
            "level": long_level,
            "polygon": [[0.0, 0.0], [10.0, 10.0]]
        }]
    });
    let d = digest(&reply);
    for line in d.lines() {
        assert!(line.len() <= 240, "line too long ({} chars): {}", line.len(), line);
    }
}

#[test]
fn coord_handles_various_inputs() {
    assert_eq!(coord(&serde_json::json!(42)), "42");
    assert_eq!(coord(&serde_json::json!(true)), "true");
    assert_eq!(coord(&serde_json::json!("hello")), "hello");
    assert_eq!(coord(&serde_json::json!([1.0, 2.0])), "(1,2)");
    assert_eq!(coord(&serde_json::json!([[1.0, 2.0], [3.0, 4.0]])), "(1,2)-(3,4)");
    assert_eq!(coord(&serde_json::json!(null)), "-");
    assert_eq!(coord(&serde_json::json!({})), "-");
    assert_eq!(coord(&serde_json::json!([])), "-");
    assert_eq!(coord(&serde_json::json!([1, 2, 3])), "-");
}

#[test]
fn point_handles_various_inputs() {
    assert_eq!(point(&serde_json::json!([1.0, 2.0])), "(1,2)");
    assert_eq!(point(&serde_json::json!(null)), "-");
    assert_eq!(point(&serde_json::json!("not a point")), "-");
    assert_eq!(point(&serde_json::json!([1.0])), "-");
    assert_eq!(point(&serde_json::json!([1.0, "y"])), "-");
}

#[test]
fn moldings_filtered_to_kitchen_bath() {
    let reply = serde_json::json!({
        "rooms": [
            {"id": "k1", "type": "kitchen"},
            {"id": "b1", "type": "bath"},
            {"id": "lr", "type": "living"}
        ],
        "moldings": [
            {"room": "k1", "kind": "base", "length": 5.0},
            {"room": "b1", "kind": "base", "length": 3.0},
            {"room": "lr", "kind": "base", "length": 10.0}
        ]
    });
    let d = digest(&reply);
    let lines: Vec<&str> = d.lines().collect();
    assert!(lines.iter().any(|l| l.starts_with("molding k1 base len=5")), "missing kitchen molding: {}", d);
    assert!(lines.iter().any(|l| l.starts_with("molding b1 base len=3")), "missing bath molding: {}", d);
    assert!(!lines.iter().any(|l| l.starts_with("molding lr")), "living room molding should be filtered: {}", d);
}

#[test]
fn digest_ignores_unknown_keys() {
    let reply = serde_json::json!({
        "rooms": [{"id": "r1", "name": "Test", "type": "bedroom", "level": "1", "polygon": [[0.0, 0.0], [10.0, 10.0]]}],
        "unknown_key": "value",
        "another_unknown": [1, 2, 3]
    });
    let d = digest(&reply);
    let lines: Vec<&str> = d.lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly one line: {}", d);
    assert!(lines[0].starts_with("room r1"), "unexpected line: {}", lines[0]);
}
