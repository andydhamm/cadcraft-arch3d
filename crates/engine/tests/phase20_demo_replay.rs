//! Replays the Phase-20 demo script (`docs/arch3d/showcase/phase-20-demo.json`) and the Phase-20
//! showcase script (`docs/arch3d/showcase/phase-20-showcase.json`, run on top of `bilevel.json`)
//! through the engine and asserts the resulting plan views, auto-dimensions, ceiling text and tray
//! labels meet the phase-20 brief.

use std::fs;
use std::path::{Path, PathBuf};

use cadcraft_engine::Session;
use cadcraft_engine::cmd::file::{IoHooks, set_io};
use serde::Deserialize;
use serde_json::Value;

/// An empty parameter object (used by serde's `default`).
fn default_params() -> Value {
    Value::Object(serde_json::Map::new())
}

/// A step of a replay script: `{"cmd": "...", "params": {...}}`.
#[derive(Debug, Deserialize)]
struct ScriptStep {
    cmd: String,
    #[serde(default = "default_params")]
    params: Value,
}

/// Path of the phase-20 demo script, relative to the crate root.
fn demo_script_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/arch3d/showcase/phase-20-demo.json")
}

/// Path of the phase-20 showcase script, relative to the crate root.
fn showcase_script_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/arch3d/showcase/phase-20-showcase.json")
}

/// Path of the two-level showcase house, relative to the crate root.
fn house_script_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/arch3d/showcase/bilevel.json")
}

/// Install the headless I/O hooks so the engine runs without real files, the same way
/// `crates/engine/tests/showcase.rs::install_io` does.
fn install_io() {
    set_io(IoHooks { read: |_, _| Err("no file reads in this test".to_string()), write: |_, _| Ok(b"stand-in export".to_vec()), plot: None });
    cadcraft_engine::library::set_library_io(cadcraft_engine::library::LibraryIo {
        read_glb: cadcraft_io::gltf_read::read_glb,
        read_image: cadcraft_io::image_read::read_image,
        encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
        sample: cadcraft_io::library::sample_files,
    });
}

/// A new metric session with one open drawing.
fn new_metric_session() -> Session {
    let mut s = Session::new();
    s.new_drawing(true);
    s
}

/// Replay every step of a JSON script through the session. Panics on the first refusal, naming the
/// step index, command id and error text. Returns the number of steps replayed.
fn replay(script: &str, session: &mut Session) -> usize {
    let steps: Vec<ScriptStep> = serde_json::from_str(script).unwrap_or_else(|e| panic!("cannot parse the script: {e}"));
    let mut n = 0;
    for (i, step) in steps.iter().enumerate() {
        if step.cmd == "tray" {
            let inspect = session.execute("arch.inspect", &serde_json::json!({"rooms": true, "levels": true, "trays": true}));
            println!("before tray step {i}: {:?}", inspect.as_ref().map(|v| v.clone()).unwrap_or_default());
        }
        match session.execute(&step.cmd, &step.params) {
            Ok(_) => n += 1,
            Err(err) => panic!("script step {i} `{}` was refused: {err}", step.cmd),
        }
    }
    n
}

/// Load and parse the step array from a script file.
fn steps_of(path: &Path) -> Vec<ScriptStep> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()))
}

/// A number field of `v` (int or float), or NaN.
fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(f64::NAN)
}

/// A string field of `v` (String, Number-as-text or Bool), or the default.
fn strd(v: &Value, key: &str, default: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => default.to_string(),
    }
}

/// A boolean field of `v`, or the default.
fn boold(v: &Value, key: &str, default: bool) -> bool {
    match v.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => matches!(s.as_str(), "true" | "True" | "TRUE" | "1" | "yes" | "on"),
        Some(Value::Number(n)) => n.as_f64().map(|n| n != 0.0).unwrap_or(default),
        _ => default,
    }
}

/// The `level` field of a row, rendered as a stable string for comparison (number or name).
fn level_str(v: &Value) -> String {
    match v.get("level") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// Run `arch.inspect` with the given parameter map and return the reply.
fn inspect(session: &mut Session, params: Value) -> Value {
    session.execute("arch.inspect", &params).unwrap_or_else(|e| panic!("arch.inspect was refused: {e}"))
}

/// The `planViews` rows of a reply, or an empty list.
fn plan_views_of(reply: &Value) -> Vec<Value> {
    reply.get("planViews").and_then(|v| v.as_array()).map(|a| a.iter().cloned().collect()).unwrap_or_default()
}

/// `(planViews rows, activePlanView)` of a reply.
fn views_of(reply: &Value) -> (Vec<Value>, Option<String>) {
    let views = plan_views_of(reply);
    let active = reply.get("activePlanView").and_then(|v| v.as_str()).map(str::to_string);
    (views, active)
}

/// The name of a plan-view row, or `""` if absent.
fn plan_view_name(v: &Value) -> String {
    strd(v, "name", "")
}

/// Find the single row in `rows` whose field `key` equals `want`, panicking otherwise.
fn find_row<'a>(rows: &'a [Value], key: &str, want: &str) -> &'a Value {
    rows.iter().find(|r| strd(r, key, "") == want).unwrap_or_else(|| panic!("find_row: no row with {}={}: rows = {rows:?}", key, want))
}

/// The `autoDims` rows of a reply, or an empty list.
fn auto_dims_of(reply: &Value) -> Vec<Value> {
    reply.get("autoDims").and_then(|v| v.as_array()).map(|a| a.iter().cloned().collect()).unwrap_or_default()
}

/// The `rooms` rows of a reply, or an empty list.
fn rooms_of(reply: &Value) -> Vec<Value> {
    reply.get("rooms").and_then(|v| v.as_array()).map(|a| a.iter().cloned().collect()).unwrap_or_default()
}

/// The `trays` rows of a reply, or an empty list.
fn trays_of(reply: &Value) -> Vec<Value> {
    reply.get("trays").and_then(|v| v.as_array()).map(|a| a.iter().cloned().collect()).unwrap_or_default()
}

/// The `plan` object (`{"plan": name}`) for a view, with its `lines` and `labels`.
fn plan_of_view(session: &mut Session, view_name: &str) -> Value {
    let reply = session
        .execute("arch.inspect", &serde_json::json!({"plan": view_name}))
        .unwrap_or_else(|e| panic!("arch.inspect plan {view_name} refused: {e}"));
    reply.get("plan").cloned().unwrap_or_default()
}

/// Whether any line in `plan` has `kind` equal to `kind`.
fn has_line_kind(plan: &Value, kind: &str) -> bool {
    plan.get("lines").and_then(|v| v.as_array()).map_or(false, |a| a.iter().any(|l| strd(l, "kind", "").eq_ignore_ascii_case(kind)))
}

/// Whether any label in `plan` has `kind` equal to `kind`.
fn has_label_kind(plan: &Value, kind: &str) -> bool {
    plan.get("labels").and_then(|v| v.as_array()).map_or(false, |a| a.iter().any(|l| strd(l, "kind", "").eq_ignore_ascii_case(kind)))
}

#[test]
fn phase20_demo_is_a_non_empty_step_array() {
    let steps = steps_of(&demo_script_path());
    assert!(!steps.is_empty(), "phase-20-demo.json has no steps");
    for (i, step) in steps.iter().enumerate() {
        assert!(!step.cmd.is_empty(), "step {i} has an empty cmd");
    }
}

#[test]
fn phase20_demo_replays_without_refusals() {
    install_io();
    let script = fs::read_to_string(&demo_script_path()).unwrap_or_else(|e| panic!("cannot read demo script: {e}"));
    let steps = steps_of(&demo_script_path());
    let mut session = new_metric_session();
    let replayed = replay(&script, &mut session);
    assert_eq!(replayed, steps.len(), "expected {} steps replayed, got {}", steps.len(), replayed);
}

#[test]
fn phase20_demo_has_the_eight_views_bound_to_their_levels() {
    install_io();
    let script = fs::read_to_string(&demo_script_path()).unwrap_or_else(|e| panic!("cannot read demo script: {e}"));
    let mut session = new_metric_session();
    replay(&script, &mut session);

    let reply = inspect(&mut session, serde_json::json!({"planViews": true}));
    let (views, _active) = views_of(&reply);
    assert_eq!(views.len(), 8, "expected 8 plan views, found {}", views.len());

    // The eight expected view names.
    let expected = [
        "Level 1 Floor Plan",
        "Level 2 Floor Plan",
        "Level 1 Electrical",
        "Level 2 Electrical",
        "Level 1 Reflected Ceiling",
        "Level 2 Reflected Ceiling",
        "Roof Plan",
        "Site",
    ];
    let mut got_names: Vec<String> = views.iter().map(plan_view_name).collect();
    got_names.sort();
    let mut want_names: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    want_names.sort();
    assert_eq!(got_names, want_names, "plan view names mismatch; got {:?}", got_names);

    // The six level-bound views are grouped: Level-1 views share one level value, Level-2 views
    // share another, and the two levels differ.
    let l1 = ["Level 1 Floor Plan", "Level 1 Electrical", "Level 1 Reflected Ceiling"];
    let l2 = ["Level 2 Floor Plan", "Level 2 Electrical", "Level 2 Reflected Ceiling"];
    let get_levels = |names: &[&str]| -> Vec<String> {
        names.iter().map(|n| views.iter().find(|v| plan_view_name(v) == n.to_string()).map_or_else(|| String::from("?"), |v| level_str(v))).collect()
    };
    let lv1 = get_levels(&l1);
    let lv2 = get_levels(&l2);
    assert!(!lv1.is_empty() && lv1.iter().all(|x| !x.is_empty()), "a Level-1 view has no level");
    assert!(!lv2.is_empty() && lv2.iter().all(|x| !x.is_empty()), "a Level-2 view has no level");
    assert!(lv1.iter().all(|x| *x == lv1[0]), "Level-1 views are not on one level: {:?}", lv1);
    assert!(lv2.iter().all(|x| *x == lv2[0]), "Level-2 views are not on one level: {:?}", lv2);
    assert_ne!(lv1[0], lv2[0], "Level-1 and Level-2 views share the same level ({})", lv1[0]);
}

#[test]
fn phase20_demo_ends_on_level_1_floor_plan() {
    install_io();
    let script = fs::read_to_string(&demo_script_path()).unwrap_or_else(|e| panic!("cannot read demo script: {e}"));
    let mut session = new_metric_session();
    replay(&script, &mut session);

    let reply = inspect(&mut session, serde_json::json!({"planViews": true}));
    let (_views, active) = views_of(&reply);
    assert_eq!(active, Some("Level 1 Floor Plan".to_string()), "active plan view is {active:?}, expected `Level 1 Floor Plan`");
}

#[test]
fn phase20_demo_level_1_north_walls_row_is_6_1_4_1() {
    install_io();
    let script = fs::read_to_string(&demo_script_path()).unwrap_or_else(|e| panic!("cannot read demo script: {e}"));
    let mut session = new_metric_session();
    replay(&script, &mut session);

    let reply = inspect(&mut session, serde_json::json!({"autoDims": true}));
    let auto = auto_dims_of(&reply);
    assert!(!auto.is_empty(), "no autoDims rows");

    // The level-0 north 'walls' row reads the two north-wall segments: 6.1 and 4.1.
    let row = auto
        .iter()
        .find(|r| level_str(r) == "0" && strd(r, "row", "") == "walls" && strd(r, "side", "").eq_ignore_ascii_case("north"))
        .cloned()
        .unwrap_or_else(|| panic!("no level-0 north 'walls' auto row: {}", auto.len()));
    let values: Vec<f64> = row.get("values").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or_default();
    assert_eq!(values.len(), 2, "north 'walls' row should have 2 values [6.1, 4.1], found {}", values.len());
    assert!(
        (values[0] - 6.1).abs() <= 0.1 && (values[1] - 4.1).abs() <= 0.1,
        "north 'walls' values [{}, {}], expected [6.1, 4.1]",
        values[0],
        values[1]
    );
}

#[test]
fn phase20_demo_bedroom_reads_2_17_and_one_tray_is_labelled() {
    install_io();
    let script = fs::read_to_string(&demo_script_path()).unwrap_or_else(|e| panic!("cannot read demo script: {e}"));
    let mut session = new_metric_session();
    replay(&script, &mut session);

    let reply = inspect(&mut session, serde_json::json!({"rooms": true, "trays": true}));
    let rooms = rooms_of(&reply);
    let bedroom = find_row(&rooms, "name", "Bedroom");
    assert_eq!(
        strd(bedroom, "ceilingText", ""),
        "2.17".to_string(),
        "Bedroom ceilingText is `{}`, expected `2.17`",
        strd(bedroom, "ceilingText", "")
    );

    // Exactly one tray exists and the Level-2 reflected ceiling labels it.
    let trays = trays_of(&reply);
    assert_eq!(trays.len(), 1, "expected exactly one tray, found {}", trays.len());
    let plan = plan_of_view(&mut session, "Level 2 Reflected Ceiling");
    assert!(has_line_kind(&plan, "trayOutline"), "Level-2 reflected ceiling has no `trayOutline` line");
    assert!(has_label_kind(&plan, "trayHeight"), "Level-2 reflected ceiling has no `trayHeight` label");
}

#[test]
fn phase20_showcase_replays_after_bilevel_without_refusals() {
    install_io();
    let house = fs::read_to_string(&house_script_path()).unwrap_or_else(|e| panic!("cannot read house script: {e}"));
    let phase = fs::read_to_string(&showcase_script_path()).unwrap_or_else(|e| panic!("cannot read showcase script: {e}"));
    let house_steps = steps_of(&house_script_path());
    let phase_steps = steps_of(&showcase_script_path());
    let mut session = new_metric_session();
    let h = replay(&house, &mut session);
    let p = replay(&phase, &mut session);
    assert_eq!(h, house_steps.len(), "house replay: {} of {} steps", h, house_steps.len());
    assert_eq!(p, phase_steps.len(), "showcase replay: {} of {} steps", p, phase_steps.len());
}

#[test]
fn phase20_showcase_has_three_views_per_floor_level() {
    install_io();
    let house = fs::read_to_string(&house_script_path()).unwrap_or_else(|e| panic!("cannot read house script: {e}"));
    let phase = fs::read_to_string(&showcase_script_path()).unwrap_or_else(|e| panic!("cannot read showcase script: {e}"));
    let mut session = new_metric_session();
    replay(&house, &mut session);
    replay(&phase, &mut session);

    let reply = inspect(&mut session, serde_json::json!({"planViews": true, "levels": true}));
    let views = plan_views_of(&reply);
    let levels: Vec<Value> = reply.get("levels").and_then(|v| v.as_array()).map(|a| a.iter().cloned().collect()).unwrap_or_default();
    let floors: Vec<&Value> = levels.iter().filter(|l| strd(l, "kind", "") == "floor").collect();
    assert!(!floors.is_empty(), "no `floor` levels in the showcase");
    for level in &floors {
        let name = strd(level, "name", "?");
        assert!(!name.is_empty() && name != "?", "a floor level has no name");
        for suffix in ["Floor Plan", "Electrical", "Reflected Ceiling"] {
            let view_name = format!("{} {}", name, suffix);
            assert!(views.iter().any(|v| plan_view_name(v) == view_name), "missing view `{}`", view_name);
        }
    }
}

#[test]
fn phase20_showcase_overall_rows_match_exterior_bounds() {
    install_io();
    let house = fs::read_to_string(&house_script_path()).unwrap_or_else(|e| panic!("cannot read house script: {e}"));
    let phase = fs::read_to_string(&showcase_script_path()).unwrap_or_else(|e| panic!("cannot read showcase script: {e}"));
    let mut session = new_metric_session();
    replay(&house, &mut session);
    replay(&phase, &mut session);

    let reply = inspect(&mut session, serde_json::json!({"autoDims": true, "levels": true}));
    let auto = auto_dims_of(&reply);
    let levels: Vec<Value> = reply.get("levels").and_then(|v| v.as_array()).map(|a| a.iter().cloned().collect()).unwrap_or_default();
    let floors: Vec<&Value> = levels.iter().filter(|l| strd(l, "kind", "") == "floor").collect();
    assert!(!floors.is_empty(), "no `floor` levels in the showcase");

    // For each floor level the `overall` auto row's two values must match the exterior-wall
    // bounding box. Auto-dim rows report their `level` as the numeric level index, while the
    // `levels` reply exposes the names separately.
    for level in &floors {
        let name = strd(level, "name", "?");
        assert!(!name.is_empty() && name != "?", "a floor level has no name");
        let level_index = level.get("index").and_then(|v| v.as_u64()).or_else(|| level.get("level").and_then(|v| v.as_u64())).unwrap_or_default();
        let row = auto.iter().find(|r| {
            let Some(Value::Number(n)) = r.get("level") else {
                return false;
            };
            n.as_u64() == Some(level_index) && strd(r, "row", "").eq_ignore_ascii_case("overall")
        });
        let Some(row) = row else {
            panic!("no `overall` auto row for level `{}` (index {}): {:?}", name, level_index, auto.len());
        };
        let values: Vec<f64> =
            row.get("values").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or_default();
        assert_eq!(values.len(), 2, "overall row for `{}` should have 2 values, has {}", name, values.len());
        assert!(
            (values[0] - 26.2).abs() <= 0.1 && (values[1] - 16.2).abs() <= 0.1 || (values[0] - 16.2).abs() <= 0.1 && (values[1] - 26.2).abs() <= 0.1,
            "level `{}` overall row is {:?}, expected extents 26.2 x 16.2",
            name,
            values
        );
    }
}
