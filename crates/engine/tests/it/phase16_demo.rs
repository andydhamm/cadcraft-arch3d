use std::fs;

fn load_demo() -> serde_json::Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/arch3d/showcase/phase-16-demo.json");
    let text = fs::read_to_string(path).expect("phase-16-demo.json exists");
    serde_json::from_str(&text).expect("phase-16-demo.json is valid JSON")
}

#[test]
fn demo_is_json_array_of_steps() {
    let demo = load_demo();
    assert!(demo.is_array(), "demo must be a JSON array");
    let steps = demo.as_array().expect("array");
    assert!(!steps.is_empty(), "demo must have at least one step");
    for (i, step) in steps.iter().enumerate() {
        assert!(step.is_object(), "step {} must be an object", i);
        let obj = step.as_object().expect("object");
        assert!(obj.contains_key("cmd"), "step {} must have 'cmd'", i);
        assert!(obj.contains_key("params"), "step {} must have 'params'", i);
        assert!(obj["cmd"].is_string(), "step {} cmd must be a string", i);
        assert!(obj["params"].is_object(), "step {} params must be an object", i);
    }
}

#[test]
fn demo_has_required_level_commands() {
    let demo = load_demo();
    let steps = demo.as_array().expect("array");
    let cmds: Vec<&str> = steps
        .iter()
        .map(|s| s["cmd"].as_str().expect("cmd string"))
        .collect();

    // Must create a Basement level
    assert!(cmds.contains(&"level"), "demo must have a level command");
    let level_steps: Vec<&serde_json::Value> = steps
        .iter()
        .filter(|s| s["cmd"] == "level")
        .collect();
    assert!(!level_steps.is_empty(), "no level commands found");

    // Check that Basement level is created with elevation -2.7
    let basement = level_steps.iter().find(|s| {
        s["params"]["name"] == "Basement"
    });
    assert!(basement.is_some(), "must have a Basement level");
    let basement = basement.unwrap();
    assert_eq!(
        basement["params"]["elevation"].as_f64(),
        Some(-2.7),
        "Basement elevation must be -2.7"
    );
}

#[test]
fn demo_has_wall_and_room_commands() {
    let demo = load_demo();
    let steps = demo.as_array().expect("array");
    let cmds: Vec<&str> = steps
        .iter()
        .map(|s| s["cmd"].as_str().expect("cmd string"))
        .collect();

    assert!(cmds.contains(&"wall"), "demo must have wall commands");
    assert!(cmds.contains(&"roomdetect"), "demo must have roomdetect");
    assert!(cmds.contains(&"room"), "demo must have room commands");

    // Count walls - should be at least 10 (6 exterior + 3 interior + basement)
    let wall_count = cmds.iter().filter(|c| **c == "wall").count();
    assert!(wall_count >= 10, "expected at least 10 walls, got {}", wall_count);

    // Check for vaulted living room (flatCeiling: false)
    let room_steps: Vec<&serde_json::Value> = steps
        .iter()
        .filter(|s| s["cmd"] == "room")
        .collect();
    let living = room_steps.iter().find(|s| {
        s["params"]["name"] == "Living" || s["params"]["id"] == "r0"
    });
    assert!(living.is_some(), "must have a Living room");
    let living = living.unwrap();
    assert_eq!(
        living["params"]["flatCeiling"].as_bool(),
        Some(false),
        "Living room must have flatCeiling: false for vault"
    );
}

#[test]
fn demo_has_roof_tray_soffit_foundation() {
    let demo = load_demo();
    let steps = demo.as_array().expect("array");
    let cmds: Vec<&str> = steps
        .iter()
        .map(|s| s["cmd"].as_str().expect("cmd string"))
        .collect();

    assert!(cmds.contains(&"roofauto"), "demo must have roofauto");
    assert!(cmds.contains(&"tray"), "demo must have a tray command");
    assert!(cmds.contains(&"soffit"), "demo must have a soffit command");
    assert!(cmds.contains(&"foundation"), "demo must have a foundation command");

    // Foundation must be stem type
    let foundation = steps.iter().find(|s| s["cmd"] == "foundation");
    assert!(foundation.is_some(), "no foundation command");
    let foundation = foundation.unwrap();
    assert_eq!(
        foundation["params"]["type"].as_str(),
        Some("stem"),
        "foundation must be stem type"
    );
}

#[test]
fn demo_has_saved_cameras() {
    let demo = load_demo();
    let steps = demo.as_array().expect("array");
    let camera_steps: Vec<&serde_json::Value> = steps
        .iter()
        .filter(|s| s["cmd"] == "camera")
        .collect();

    assert!(
        camera_steps.len() >= 2,
        "demo must have at least 2 saved cameras, got {}",
        camera_steps.len()
    );

    // Check camera names
    let names: Vec<&str> = camera_steps
        .iter()
        .filter_map(|s| s["params"]["save"].as_str())
        .collect();
    assert!(
        names.contains(&"SouthWest"),
        "must have SouthWest camera, got {:?}",
        names
    );
    assert!(
        names.contains(&"LivingVault"),
        "must have LivingVault camera, got {:?}",
        names
    );
}

#[test]
fn demo_cabinet_offsets_are_valid() {
    let demo = load_demo();
    let steps = demo.as_array().expect("array");
    let cabinet_steps: Vec<&serde_json::Value> = steps
        .iter()
        .filter(|s| s["cmd"] == "cabinet")
        .collect();

    assert!(
        !cabinet_steps.is_empty(),
        "demo must have cabinet commands"
    );

    // All cabinets must have valid numeric offsets
    for (i, cab) in cabinet_steps.iter().enumerate() {
        let offset = cab["params"]["offset"].as_f64();
        assert!(offset.is_some(), "cabinet {} must have numeric offset", i);
        let offset = offset.unwrap();
        assert!(offset > 0.0, "cabinet {} offset must be positive", i);
    }

    // Wall cabinets (kind: wall) should exist for the soffit
    let wall_cabs: Vec<&serde_json::Value> = cabinet_steps
        .iter()
        .filter(|c| c["params"]["kind"] == "wall")
        .collect();
    assert!(
        wall_cabs.len() >= 4,
        "expected at least 4 wall cabinets for soffit, got {}",
        wall_cabs.len()
    );
}
