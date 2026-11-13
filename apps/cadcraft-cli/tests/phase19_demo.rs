use std::fs;
use std::path::PathBuf;
use std::sync::Once;

use cadcraft_engine::{Session, library::LibraryIo};
use serde_json::{Value, json};

fn install_library_io() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        cadcraft_engine::library::set_library_io(LibraryIo {
            read_glb: |_| Err("glb reader not used in the demo tests".to_string()),
            read_image: cadcraft_io::image_read::read_image,
            encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
            sample: cadcraft_io::library::sample_files,
        });
    });
}

fn demo_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/arch3d/showcase/phase-19-demo.json")
}

fn replay_demo() -> Session {
    let script = fs::read_to_string(demo_path()).expect("phase-19 demo script should exist");
    let steps: Vec<Value> = serde_json::from_str(&script).expect("phase-19 demo script is valid JSON");

    install_library_io();
    let mut session = Session::new();
    for (idx, step) in steps.iter().enumerate() {
        let cmd = step.get("cmd").and_then(Value::as_str).unwrap_or_else(|| panic!("step {idx} missing string cmd: {step:?}"));
        let params = step.get("params").cloned().unwrap_or(Value::Object(serde_json::Map::new()));
        let result = session.execute(cmd, &params);
        assert!(result.is_ok(), "step {idx} `{cmd}` failed: {result:?}");
    }
    session
}

#[test]
fn phase19_demo_replays_without_an_error() {
    replay_demo();
}

#[test]
fn phase19_demo_saves_three_cameras_with_their_own_looks() {
    let mut session = replay_demo();
    let inspect = session.execute("arch.inspect", &json!({ "cameras": true, "sunAngles": true })).expect("arch.inspect should list cameras");
    let cameras = inspect["cameras"].as_array().expect("cameras list should exist");
    let by_name: std::collections::BTreeMap<_, _> =
        cameras.iter().filter_map(|cam| cam.get("name").and_then(Value::as_str).map(|name| (name.to_string(), cam))).collect();

    assert!(by_name.contains_key("dusk"), "dusk camera missing: {cameras:?}");
    assert!(by_name.contains_key("noon-hdri"), "noon-hdri camera missing: {cameras:?}");
    assert!(by_name.contains_key("overview"), "overview camera missing: {cameras:?}");

    let dusk = by_name["dusk"];
    let dusk_look = dusk.get("look").and_then(Value::as_object).expect("dusk look missing");
    assert_eq!(dusk["kind"], "full");
    assert_eq!(dusk_look["lightSet"], "Evening");
    assert_eq!(dusk_look["sun"]["angle"], "dusk");
    assert_eq!(dusk_look["backdrop"]["kind"], "sky");
    let dusk_ev = dusk_look["exposure"]["ev100"].as_f64().expect("dusk EV should be numeric");
    assert!((dusk_ev - 4.0).abs() < 1e-9, "expected EV100=4, got {dusk_ev}");

    let noon = by_name["noon-hdri"];
    let noon_look = noon.get("look").and_then(Value::as_object).expect("noon-hdri look missing");
    assert!(noon["kind"].is_string());
    assert_eq!(noon_look["backdrop"]["kind"], "hdri");
    assert_eq!(noon_look["backdrop"]["image"], "polyhaven:kloofendal-puresky");
    assert_eq!(noon_look["sun"]["angle"], "noon");

    let overview = by_name["overview"];
    let overview_look = overview.get("look").and_then(Value::as_object).expect("overview look missing");
    assert_eq!(overview["kind"], "overview");
    assert_eq!(overview_look["cutaway"], true);
}

#[test]
fn phase19_demo_has_tray_and_cabinet_strips() {
    let mut session = replay_demo();
    let inspect = session.execute("arch.inspect", &json!({ "lights": true, "trays": true })).expect("arch.inspect should list fixtures and trays");

    let lights = inspect["lights"].as_array().expect("lights list should exist");
    let under_cabinet = lights.iter().any(|light| {
        light.get("kind") == Some(&Value::String("strip".to_string()))
            && light.get("fixture") == Some(&Value::String("LED strip".to_string()))
            && light.get("length").and_then(Value::as_f64).is_some_and(|len| len > 0.0 && len <= 3.0)
            && light
                .get("path")
                .and_then(Value::as_array)
                .and_then(|path| path.first())
                .and_then(Value::as_array)
                .and_then(|pt| pt.first())
                .and_then(Value::as_f64)
                .is_some_and(|x| x > 9.0)
    });
    let tray_strip = lights
        .iter()
        .any(|light| light.get("kind") == Some(&Value::String("strip".to_string())) && light.get("tray").and_then(Value::as_str).is_some());
    let trays = inspect["trays"].as_array().expect("trays list should exist");

    assert!(under_cabinet, "expected an under-cabinet strip in the light list: {lights:?}");
    assert!(tray_strip, "expected a strip bound to a tray in the light list: {lights:?}");
    assert!(!trays.is_empty(), "expected at least one tray in the model: {trays:?}");
}
