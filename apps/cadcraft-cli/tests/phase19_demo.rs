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
        if cmd == "render" {
            continue;
        }
        let params = step.get("params").cloned().unwrap_or(Value::Object(serde_json::Map::new()));
        let result = session.execute(cmd, &params);
        assert!(result.is_ok(), "step {idx} `{cmd}` failed: {result:?}");
    }
    session
}

#[test]
fn phase19_demo_replays_without_error() {
    replay_demo();
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

/// 11-15: phase-19 check 11 got `filter.hidden` 0 through the overview camera. The script added a
/// second level named "Level 0" (index 1) beside the drawing's own level 0 and built on it, while
/// the camera's `level: 0` named the empty one; and the fitted view's centre pixel lay on the
/// partition wall, not a floor.
#[test]
fn phase19_demo_overview_cuts_away_walls_and_centres_on_a_floor() {
    let mut session = replay_demo();
    let inspect = session.execute("arch.inspect", &json!({ "walls": true })).expect("arch.inspect should list levels and walls");
    let levels = inspect["levels"].as_array().expect("levels list should exist");
    assert_eq!(levels.len(), 1, "the demo builds on the drawing's one level: {levels:?}");
    assert_eq!(levels[0]["name"], "Level 0");
    let walls = inspect["walls"].as_array().expect("walls list should exist");
    assert!(!walls.is_empty() && walls.iter().all(|w| w["level"] == 0), "every wall stands on level 0: {walls:?}");

    let pick = session
        .execute("pick3d", &json!({ "camera": "overview", "size": [320, 240], "pixel": [160, 120] }))
        .expect("pick3d through the overview camera");
    let hidden = pick["filter"]["hidden"].as_u64().expect("filter.hidden is a count");
    assert!(hidden > 0, "the overview's cutaway hides camera-facing walls: {pick}");
    assert_eq!(pick["hit"]["part"], "floor", "the overview's centre pixel shows a floor: {pick}");
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
fn phase19_demo_steps_are_well_formed() {
    let steps = load_demo_steps();
    assert!(!steps.is_empty(), "demo script should not be empty");
    for (i, step) in steps.iter().enumerate() {
        let cmd = step.get("cmd").and_then(Value::as_str);
        assert!(cmd.is_some(), "step {i} missing string cmd: {step:?}");
        let params = step.get("params");
        assert!(params.is_some(), "step {i} missing params: {step:?}");
        assert!(params.unwrap().is_object(), "step {i} params should be an object: {step:?}");
    }
    let mut saved = std::collections::BTreeMap::new();
    for step in &steps {
        if step["cmd"] == "camera" {
            if let (Some(n), Some(k)) = (step["params"]["save"].as_str(), step["params"]["kind"].as_str()) {
                saved.insert(n.to_string(), k.to_string());
            }
        }
    }
    let want: std::collections::BTreeMap<String, String> =
        [("dusk", "full"), ("noon-hdri", "full"), ("overview", "overview")].iter().map(|(n, k)| (n.to_string(), k.to_string())).collect();
    assert_eq!(saved, want, "exactly three saved cameras");
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

#[test]
fn phase19_demo_renders_stay_small() {
    for step in load_demo_steps().iter().filter(|s| s["cmd"] == "render") {
        let p = &step["params"];
        let size = p["size"].as_array().expect("render size is [w, h]");
        assert_eq!(size.len(), 2, "render size is [w, h]: {p}");
        assert!(size[0].as_u64().is_some_and(|w| w <= 320) && size[1].as_u64().is_some_and(|h| h <= 240), "render at most 320 x 240: {p}");
        if p.get("final").is_some() || p["technique"] == "path" {
            assert!(p["spp"].as_u64().is_some_and(|v| v <= 16), "path render spp <= 16: {p}");
            assert!(p["bounces"].as_u64().is_some_and(|v| v <= 2), "path render bounces <= 2: {p}");
            assert_eq!(p["denoise"], Value::Bool(false), "path render denoise is false: {p}");
        }
    }
    let mut session = replay_demo();
    let inspect = session.execute("arch.inspect", &json!({ "cameras": true })).expect("arch.inspect lists cameras");
    let look = |name: &str| -> Value {
        inspect["cameras"].as_array().expect("cameras").iter().find(|c| c["name"] == name).unwrap_or_else(|| panic!("camera {name} missing"))["look"]
            .clone()
    };
    assert_eq!(look("noon-hdri")["technique"], "path");
    assert_eq!(look("dusk")["technique"], "standard");
    assert_eq!(look("overview")["technique"], "standard", "the overview camera must not inherit the path technique");
    assert_ne!(look("overview")["backdrop"]["kind"], "hdri", "the overview camera must not inherit the HDRI backdrop");
}

#[test]
fn phase19_demo_has_expected_object_counts() {
    let mut session = replay_demo();
    let inspect = session
        .execute(
            "arch.inspect",
            &json!({
                "walls": true, "openings": true, "rooms": true,
                "lights": true, "cabinets": true, "trays": true,
                "cameras": true, "sunAngles": true
            }),
        )
        .expect("arch.inspect should return model data");

    let walls = inspect["walls"].as_array().expect("walls list");
    let openings = inspect["openings"].as_array().expect("openings list");
    let rooms = inspect["rooms"].as_array().expect("rooms list");
    let lights = inspect["lights"].as_array().expect("lights list");
    let cabinets = inspect["cabinets"].as_array().expect("cabinets list");
    let trays = inspect["trays"].as_array().expect("trays list");
    let cameras = inspect["cameras"].as_array().expect("cameras list");
    let sun_angles = inspect["sunAngles"].as_array().expect("sunAngles list");

    assert_eq!(walls.len(), 5, "expected 5 walls: {walls:?}");
    assert_eq!(openings.len(), 5, "expected 5 openings (1 door + 4 windows): {openings:?}");
    assert_eq!(rooms.len(), 2, "expected 2 rooms: {rooms:?}");
    assert_eq!(lights.len(), 6, "expected 6 light fixtures: {lights:?}");
    assert_eq!(cabinets.len(), 4, "expected 4 cabinets: {cabinets:?}");
    assert_eq!(trays.len(), 1, "expected 1 tray: {trays:?}");
    assert_eq!(cameras.len(), 3, "expected 3 cameras: {cameras:?}");
    assert_eq!(sun_angles.len(), 2, "expected 2 sun angles: {sun_angles:?}");
}

#[test]
fn phase19_demo_light_sets_cover_all_fixtures() {
    let steps = load_demo_steps();
    let mut fixture_ids = Vec::new();
    let mut light_sets = std::collections::BTreeMap::new();

    for step in &steps {
        let cmd = step.get("cmd").and_then(Value::as_str).unwrap();
        if cmd == "lightfixture" {
            fixture_ids.push(format!("lf{}", fixture_ids.len()));
        }
        if cmd == "lightset" {
            if let Some(name) = step.get("params").and_then(|p| p.get("name")).and_then(Value::as_str) {
                let lights = step["params"]["lights"].as_object().cloned().unwrap_or_default();
                light_sets.insert(name.to_string(), lights);
            }
        }
    }

    assert_eq!(fixture_ids.len(), 6, "expected 6 light fixtures");
    assert_eq!(light_sets.len(), 2, "expected 2 light sets");

    for (name, lights) in &light_sets {
        for fixture in &fixture_ids {
            assert!(lights.contains_key(fixture), "light set {name} missing fixture {fixture}: {lights:?}");
        }
    }
}

#[test]
fn phase19_demo_script_has_39_steps() {
    let steps = load_demo_steps();
    assert_eq!(steps.len(), 39, "demo script should have exactly 39 steps");
}

#[test]
fn phase19_demo_terrain_contains_house() {
    let steps = load_demo_steps();
    let mut terrain_boundary = None;
    let mut wall_points = Vec::new();

    for step in &steps {
        let cmd = step.get("cmd").and_then(Value::as_str).unwrap();
        if cmd == "terrain" {
            terrain_boundary = step.get("params").and_then(|p| p.get("boundary")).and_then(Value::as_array);
        }
        if cmd == "wall" {
            if let Some(points) = step.get("params").and_then(|p| p.get("points")).and_then(Value::as_array) {
                for pt in points {
                    if let (Some(x), Some(y)) = (pt[0].as_f64(), pt[1].as_f64()) {
                        wall_points.push((x, y));
                    }
                }
            }
        }
    }

    let boundary = terrain_boundary.expect("demo should have a terrain boundary");
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for pt in boundary {
        let x = pt[0].as_f64().unwrap();
        let y = pt[1].as_f64().unwrap();
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }

    for (x, y) in &wall_points {
        assert!(
            *x >= min_x && *x <= max_x && *y >= min_y && *y <= max_y,
            "wall point ({x}, {y}) is outside terrain boundary [{min_x}, {min_y}] to [{max_x}, {max_y}]"
        );
    }
}

/// The saved `dusk` camera as `(eye, target, fov)`, read through `arch.inspect`.
fn dusk_pose(session: &mut Session) -> ([f64; 3], [f64; 3], f64) {
    let inspect = session.execute("arch.inspect", &json!({ "cameras": true })).expect("arch.inspect should list cameras");
    let cameras = inspect["cameras"].as_array().expect("cameras list should exist");
    let dusk = cameras.iter().find(|c| c["name"] == "dusk").expect("dusk camera missing");
    let num = |v: &Value, what: &str| v.as_f64().unwrap_or_else(|| panic!("dusk {what} should be a number: {v}"));
    // `eye` is the plan position [x, y]; its height is `eyeHeight`.
    let eye = [num(&dusk["eye"][0], "eye x"), num(&dusk["eye"][1], "eye y"), num(&dusk["eyeHeight"], "eyeHeight")];
    let target = [num(&dusk["target"][0], "target x"), num(&dusk["target"][1], "target y"), num(&dusk["target"][2], "target z")];
    (eye, target, num(&dusk["fov"], "fov"))
}

#[test]
fn phase19_demo_dusk_camera_shows_the_lit_window_large_enough() {
    use cadcraft_arch3d::camera::{Camera, FullCamera, project_point};
    use cadcraft_geom::Vec3;

    let mut session = replay_demo();
    let (eye, target, fov) = dusk_pose(&mut session);
    let camera = Camera::Full(FullCamera::new(Vec3::new(eye[0], eye[1], eye[2]), Vec3::new(target[0], target[1], target[2]), fov));
    let size = (320u32, 240u32);
    // The south window's glass: x 6.4..7.6 on wall w0's outer face (y = -0.1), z 0.9..2.1.
    let corners = [[6.4, -0.1, 0.9], [7.6, -0.1, 0.9], [7.6, -0.1, 2.1], [6.4, -0.1, 2.1]];
    let pts: Vec<[f32; 2]> = corners.iter().map(|&p| project_point(&camera, size, p).expect("window corner should project")).collect();
    for p in &pts {
        assert!(p[0] >= 0.0 && p[0] <= 320.0 && p[1] >= 0.0 && p[1] <= 240.0, "window corner {p:?} is outside the frame");
    }
    let min_x = pts.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
    let max_x = pts.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
    let min_y = pts.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    let max_y = pts.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    let area = (max_x - min_x) * (max_y - min_y);
    assert!(area >= 500.0, "window covers only {area} px at 320x240: {pts:?}");
}

#[test]
fn phase19_demo_dusk_camera_is_level_and_on_the_site() {
    let mut session = replay_demo();
    let (eye, target, _) = dusk_pose(&mut session);
    let steps = load_demo_steps();
    let terrain = steps.iter().find(|s| s["cmd"] == "terrain").expect("demo has a terrain step");
    let boundary: Vec<[f64; 2]> = terrain["params"]["boundary"]
        .as_array()
        .expect("terrain boundary")
        .iter()
        .map(|p| [p[0].as_f64().expect("boundary x"), p[1].as_f64().expect("boundary y")])
        .collect();
    let min_x = boundary.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
    let max_x = boundary.iter().map(|p| p[0]).fold(f64::MIN, f64::max);
    let min_y = boundary.iter().map(|p| p[1]).fold(f64::MAX, f64::min);
    let max_y = boundary.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
    assert!(eye[0] >= min_x && eye[0] <= max_x && eye[1] >= min_y && eye[1] <= max_y, "dusk eye {eye:?} is outside the terrain boundary");
    let horizontal = ((target[0] - eye[0]).powi(2) + (target[1] - eye[1]).powi(2)).sqrt();
    let pitch = (target[2] - eye[2]).atan2(horizontal).to_degrees();
    assert!(pitch > -3.0 && pitch <= 0.0, "dusk pitch {pitch} degrees is not in (-3, 0]");
}

#[test]
fn phase19_demo_dusk_light_set_brightens_the_house() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let dusk_png = tmp.path().join("dusk.png");
    let dark_png = tmp.path().join("dark.png");
    let render = |light_set: Option<&str>, out: &PathBuf| {
        let mut params = json!({ "camera": "dusk", "size": [320, 240], "out": out.display().to_string() });
        if let (Some(name), Some(map)) = (light_set, params.as_object_mut()) {
            map.insert("lightSet".to_string(), json!(name));
        }
        format!("render {params}")
    };
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cadcraft-cli"))
        .env("CADCRAFT_RENDER_ADAPTER", "fallback")
        .args(["run", "--metric", "--commands"])
        .arg(demo_path())
        .args(["--cmd", &render(None, &dusk_png), "--cmd", &render(Some("Day"), &dark_png)])
        .output()
        .expect("run cadcraft-cli");
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.code() == Some(3) || stderr.contains("NO_ADAPTER") {
        eprintln!("SKIP: no render adapter");
        return;
    }
    assert!(output.status.success(), "render failed (rc={:?}): {stderr}", output.status.code());
    let read = |p: &PathBuf| cadcraft_io::image_read::read_image(&fs::read(p).expect("render output should exist"), 4096).expect("decode png");
    let (dusk, dark) = (read(&dusk_png), read(&dark_png));
    assert_eq!((dusk.width, dusk.height), (320, 240));
    assert_eq!((dark.width, dark.height), (320, 240));
    let sum = |rgba: &[u8], i: usize| -> i32 { rgba.get(i * 4..i * 4 + 3).map_or(0, |c| c.iter().map(|&v| i32::from(v)).sum()) };
    let brighter = (0..320 * 240).filter(|&i| sum(&dusk.rgba, i) - sum(&dark.rgba, i) >= 30).count();
    assert!(brighter >= 768, "only {brighter} pixels are 30+ brighter under Evening (need 768)");
}

#[test]
fn phase19_demo_dusk_camera_eye_height_is_human_scale() {
    let mut session = replay_demo();
    let (eye, _, _) = dusk_pose(&mut session);
    assert!(eye[2] >= 1.5 && eye[2] <= 2.0, "dusk eye height {} m is not human scale (1.5..2.0)", eye[2]);
}

#[test]
fn phase19_demo_dusk_camera_sees_full_south_face() {
    use cadcraft_arch3d::camera::{Camera, FullCamera, project_point};
    use cadcraft_geom::Vec3;

    let mut session = replay_demo();
    let (eye, target, fov) = dusk_pose(&mut session);
    let camera = Camera::Full(FullCamera::new(Vec3::new(eye[0], eye[1], eye[2]), Vec3::new(target[0], target[1], target[2]), fov));
    let size = (320u32, 240u32);
    // South face: wall w0 runs (0,0) to (10,0), thickness 0.2, outer face at y = -0.1, height 2.4 m.
    let corners = [[0.0, -0.1, 0.0], [10.0, -0.1, 0.0], [10.0, -0.1, 2.4], [0.0, -0.1, 2.4]];
    for &p in &corners {
        let proj = project_point(&camera, size, p).expect("south face corner should project");
        assert!(
            proj[0] >= 0.0 && proj[0] <= 320.0 && proj[1] >= 0.0 && proj[1] <= 240.0,
            "south face corner {p:?} projects outside frame at {proj:?}"
        );
    }
}

#[test]
fn phase19_demo_dusk_camera_fov_is_reasonable() {
    let mut session = replay_demo();
    let (_, _, fov) = dusk_pose(&mut session);
    assert!(fov >= 30.0 && fov <= 90.0, "dusk fov {} is not in the reasonable range 30..90", fov);
}

#[test]
fn phase19_demo_dusk_camera_is_last_used() {
    let steps = load_demo_steps();
    let mut last_use = None;
    for (i, step) in steps.iter().enumerate() {
        if step["cmd"] == "camera" && step["params"].get("use").is_some() {
            last_use = Some((i, step["params"]["use"].as_str().expect("camera use name")));
        }
    }
    assert_eq!(last_use, Some((steps.len() - 1, "dusk")), "the script should end by using the dusk camera");
}

#[test]
fn phase19_demo_overview_camera_is_on_level_zero_with_correct_target() {
    let mut session = replay_demo();
    let inspect = session.execute("arch.inspect", &json!({ "cameras": true })).expect("arch.inspect should list cameras");
    let cameras = inspect["cameras"].as_array().expect("cameras list should exist");
    let overview = cameras.iter().find(|c| c["name"] == "overview").expect("overview camera missing");
    assert_eq!(overview["level"], 0, "overview camera should be on level 0");
    let target = overview["target"].as_array().expect("overview target should be an array");
    assert_eq!(target.len(), 3, "overview target should have 3 components");
    assert!((target[0].as_f64().unwrap() - 5.0).abs() < 1e-9, "overview target x should be 5.0");
    assert!((target[1].as_f64().unwrap() - 2.0).abs() < 1e-9, "overview target y should be 2.0");
    assert!((target[2].as_f64().unwrap() - 0.0).abs() < 1e-9, "overview target z should be 0.0");
}
