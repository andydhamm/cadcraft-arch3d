// Tests that the phase-16 demo script replays without refusals.
// The demo was fixed to: (a) not create "Level 1" (it exists by default),
// (b) edit detected rooms by id instead of creating duplicate rooms,
// (c) make the LivingVault camera look into the vault, not straight up.

use serde_json::Value;
use std::fs;

fn load_demo() -> Vec<Value> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/arch3d/showcase/phase-16-demo.json");
    let content = fs::read_to_string(path).expect("phase-16-demo.json");
    let arr: Value = serde_json::from_str(&content).expect("valid JSON");
    arr.as_array().expect("JSON array").clone()
}

#[test]
fn demo_json_is_valid() {
    let steps = load_demo();
    assert!(!steps.is_empty(), "demo should have steps");
    for step in &steps {
        assert!(step.get("cmd").is_some(), "step missing cmd");
        assert!(step.get("params").is_some(), "step missing params");
    }
}

#[test]
fn demo_has_no_duplicate_level1_step() {
    // The bug was that step 0 created "Level 1" which already exists
    let steps = load_demo();
    let level1_steps: Vec<&Value> = steps.iter().filter(|s| s["cmd"] == "level" && s["params"]["name"] == "Level 1").collect();
    assert!(level1_steps.is_empty(), "Level 1 should not be created explicitly (it exists by default)");
}

#[test]
fn demo_edits_detected_rooms() {
    // After roomdetect, the demo should edit rooms by id, not create new ones
    let steps = load_demo();
    let room_steps: Vec<&Value> = steps.iter().filter(|s| s["cmd"] == "room").collect();
    assert_eq!(room_steps.len(), 4, "expected 4 room edit steps");
    for step in &room_steps {
        assert!(step["params"].get("id").is_some(), "room step should have id");
        // Should not have "points" (that would be creating a new room)
        assert!(step["params"].get("points").is_none(), "room step should edit by id, not create with points");
    }
}

#[test]
fn demo_livingvault_camera_looks_into_vault() {
    // The LivingVault camera should be inside the living room looking up
    let steps = load_demo();
    let cam_steps: Vec<&Value> = steps.iter().filter(|s| s["cmd"] == "camera").collect();
    assert_eq!(cam_steps.len(), 2, "expected 2 camera steps");
    let living_vault = cam_steps.iter().find(|s| s["params"]["save"] == "LivingVault").expect("LivingVault camera");
    let eye = living_vault["params"]["eye"].as_array().expect("eye array");
    let target = living_vault["params"]["target"].as_array().expect("target array");
    // Eye should be at a reasonable height (not looking straight up)
    let eye_z = eye[2].as_f64().expect("eye z");
    let target_z = target[2].as_f64().expect("target z");
    assert!(target_z > eye_z, "camera should look upward into the vault");
    // Eye and target should not be directly above each other (no straight-up look)
    let eye_x = eye[0].as_f64().expect("eye x");
    let eye_y = eye[1].as_f64().expect("eye y");
    let target_x = target[0].as_f64().expect("target x");
    let target_y = target[1].as_f64().expect("target y");
    let dx = (eye_x - target_x).abs();
    let dy = (eye_y - target_y).abs();
    assert!(dx > 0.1 || dy > 0.1, "camera should not look straight up (eye and target should differ in x or y)");
}
