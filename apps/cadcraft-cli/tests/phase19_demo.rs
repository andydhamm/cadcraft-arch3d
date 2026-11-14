use std::fs;
use std::path::PathBuf;
use std::sync::Once;

use cadcraft_engine::{Session, library::LibraryIo};
use serde_json::{Value, json};

fn load_demo_steps() -> Vec<Value> {
    let script = fs::read_to_string(demo_path()).expect("phase-19 demo script should exist");
    serde_json::from_str(&script).expect("phase-19 demo script is valid JSON")
}

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
    let steps = load_demo_steps();

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
fn phase19_demo_wall_references_are_valid() {
    let steps = load_demo_steps();
    let mut walls_created = Vec::new();

    for step in &steps {
        let cmd = step.get("cmd").and_then(Value::as_str).unwrap();
        if cmd == "wall" {
            // A wall command with N points creates N-1 wall segments (closed or open).
            let points = step.get("params").and_then(|p| p.get("points")).and_then(Value::as_array);
            let n = points.map(|pts| pts.len() - 1).unwrap_or(1);
            for _ in 0..n {
                walls_created.push(format!("w{}", walls_created.len()));
            }
        }
        if cmd == "window" || cmd == "door" {
            if let Some(wall) = step.get("params").and_then(|p| p.get("wall")).and_then(Value::as_str) {
                assert!(
                    walls_created.contains(&wall.to_string()),
                    "window/door references wall {wall} but only {walls_created:?} exist at that point"
                );
            }
        }
    }
}

#[test]
fn phase19_demo_room_references_are_valid() {
    let steps = load_demo_steps();
    let mut rooms_created = Vec::new();

    for step in &steps {
        let cmd = step.get("cmd").and_then(Value::as_str).unwrap();
        if cmd == "room" {
            if let Some(id) = step.get("params").and_then(|p| p.get("id")).and_then(Value::as_str) {
                rooms_created.push(id.to_string());
            }
        }
        if cmd == "lightfixture" {
            if let Some(room) = step.get("params").and_then(|p| p.get("room")).and_then(Value::as_str) {
                assert!(
                    rooms_created.contains(&room.to_string()),
                    "lightfixture references room {room} but only {rooms_created:?} exist at that point"
                );
            }
        }
    }
}

#[test]
fn phase19_demo_cabinet_references_are_valid() {
    let steps = load_demo_steps();
    let mut cabinets_created = Vec::new();

    for step in &steps {
        let cmd = step.get("cmd").and_then(Value::as_str).unwrap();
        if cmd == "cabinet" {
            cabinets_created.push(format!("cb{}", cabinets_created.len()));
        }
        if cmd == "lightfixture" {
            if let Some(under) = step.get("params").and_then(|p| p.get("under")).and_then(Value::as_array) {
                for cab in under {
                    if let Some(cab_str) = cab.as_str() {
                        assert!(
                            cabinets_created.contains(&cab_str.to_string()),
                            "lightfixture under references cabinet {cab_str} but only {cabinets_created:?} exist at that point"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn phase19_demo_tray_reference_is_valid() {
    let steps = load_demo_steps();
    let mut trays_created = Vec::new();

    for step in &steps {
        let cmd = step.get("cmd").and_then(Value::as_str).unwrap();
        if cmd == "tray" {
            trays_created.push(format!("tr{}", trays_created.len()));
        }
        if cmd == "lightfixture" {
            if let Some(tray) = step.get("params").and_then(|p| p.get("tray")).and_then(Value::as_str) {
                assert!(
                    trays_created.contains(&tray.to_string()),
                    "lightfixture references tray {tray} but only {trays_created:?} exist at that point"
                );
            }
        }
    }
}

#[test]
fn phase19_demo_light_set_references_are_valid() {
    let steps = load_demo_steps();
    let mut light_sets_created = Vec::new();

    for step in &steps {
        let cmd = step.get("cmd").and_then(Value::as_str).unwrap();
        if cmd == "lightset" {
            if let Some(name) = step.get("params").and_then(|p| p.get("name")).and_then(Value::as_str) {
                light_sets_created.push(name.to_string());
            }
        }
        if cmd == "look" || cmd == "camera" {
            if let Some(light_set) = step.get("params").and_then(|p| p.get("lightSet")).and_then(Value::as_str) {
                assert!(
                    light_sets_created.contains(&light_set.to_string()),
                    "{cmd} references lightSet {light_set} but only {light_sets_created:?} exist at that point"
                );
            }
        }
    }
}

#[test]
fn phase19_demo_sun_angle_references_are_valid() {
    let steps = load_demo_steps();
    let mut sun_angles_created = Vec::new();

    for step in &steps {
        let cmd = step.get("cmd").and_then(Value::as_str).unwrap();
        if cmd == "sunangle" {
            if let Some(name) = step.get("params").and_then(|p| p.get("name")).and_then(Value::as_str) {
                sun_angles_created.push(name.to_string());
            }
        }
        if cmd == "look" || cmd == "camera" {
            if let Some(angle) = step.get("params").and_then(|p| p.get("sun")).and_then(|s| s.get("angle")).and_then(Value::as_str) {
                assert!(
                    sun_angles_created.contains(&angle.to_string()),
                    "{cmd} references sun angle {angle} but only {sun_angles_created:?} exist at that point"
                );
            }
        }
    }
}

#[test]
fn phase19_demo_camera_references_are_valid() {
    let steps = load_demo_steps();
    let mut cameras_created = Vec::new();

    for step in &steps {
        let cmd = step.get("cmd").and_then(Value::as_str).unwrap();
        if cmd == "camera" {
            if let Some(save) = step.get("params").and_then(|p| p.get("save")).and_then(Value::as_str) {
                cameras_created.push(save.to_string());
            }
            if let Some(use_cam) = step.get("params").and_then(|p| p.get("use")).and_then(Value::as_str) {
                assert!(
                    cameras_created.contains(&use_cam.to_string()),
                    "camera use references {use_cam} but only {cameras_created:?} exist at that point"
                );
            }
        }
    }
}

#[test]
fn phase19_demo_steps_have_cmd_and_params() {
    let steps = load_demo_steps();
    assert!(!steps.is_empty(), "demo script should not be empty");
    for (i, step) in steps.iter().enumerate() {
        let cmd = step.get("cmd").and_then(Value::as_str);
        assert!(cmd.is_some(), "step {i} missing string cmd: {step:?}");
        let params = step.get("params");
        assert!(params.is_some(), "step {i} missing params: {step:?}");
        assert!(params.unwrap().is_object(), "step {i} params should be an object: {step:?}");
    }
}

#[test]
fn phase19_demo_roof_before_tray() {
    let steps = load_demo_steps();
    let mut roof_idx = None;
    let mut tray_idx = None;
    for (i, step) in steps.iter().enumerate() {
        let cmd = step.get("cmd").and_then(Value::as_str).unwrap();
        if cmd == "roofauto" && roof_idx.is_none() {
            roof_idx = Some(i);
        }
        if cmd == "tray" && tray_idx.is_none() {
            tray_idx = Some(i);
        }
    }
    assert!(roof_idx.is_some(), "demo should have a roofauto command");
    assert!(tray_idx.is_some(), "demo should have a tray command");
    assert!(roof_idx.unwrap() < tray_idx.unwrap(), "roofauto (step {}) must come before tray (step {})", roof_idx.unwrap(), tray_idx.unwrap());
}

#[test]
fn phase19_demo_camera_names_are_unique() {
    let steps = load_demo_steps();
    let mut names = Vec::new();
    for step in &steps {
        let cmd = step.get("cmd").and_then(Value::as_str).unwrap();
        if cmd == "camera" {
            if let Some(save) = step.get("params").and_then(|p| p.get("save")).and_then(Value::as_str) {
                assert!(!names.contains(&save.to_string()), "duplicate camera name {save}");
                names.push(save.to_string());
            }
        }
    }
    assert!(names.contains(&"dusk".to_string()), "dusk camera missing");
    assert!(names.contains(&"noon-hdri".to_string()), "noon-hdri camera missing");
    assert!(names.contains(&"overview".to_string()), "overview camera missing");
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
