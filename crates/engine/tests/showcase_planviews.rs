use cadcraft_doc::Drawing;
use cadcraft_engine::Session;
use serde_json::{Value, json};

/// Reads a JSON file from the repository relative to CARGO_MANIFEST_DIR.
fn read_json(path: &str) -> Value {
    let full_path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path);
    let content = std::fs::read_to_string(&full_path).expect("failed to read JSON file");
    serde_json::from_str(&content).expect("failed to parse JSON")
}

#[test]
fn phase_20_showcase_planviews_replay() {
    let mut session = Session::new();

    // Create levels Lower and Main as bilevel.json does
    session.execute("level", &json!({"name": "Lower", "elevation": 0.0, "ceiling_height": 2.4})).expect("create Lower level");
    session.execute("level", &json!({"name": "Main", "elevation": 2.7, "ceiling_height": 2.4})).expect("create Main level");

    let steps = read_json("docs/arch3d/showcase/phase-20-showcase.json");
    let steps_arr = steps.as_array().expect("steps should be an array");

    for step in steps_arr {
        let cmd = step["cmd"].as_str().expect("step should have cmd");
        let params = step["params"].clone();

        // Only execute planview commands for this specific test
        if cmd.starts_with("planview") {
            let result = session.execute(cmd, &params);
            assert!(result.is_ok(), "planview command failed: {:?} with params {:?}", result, params);
        }
    }

    // Verify that the expected plan views exist
    let inspect_result = session.execute("arch.inspect", &json!({"planViews": true})).expect("arch.inspect failed");
    let plan_views = inspect_result["planViews"].as_array().expect("planViews should be an array");

    let view_names: Vec<String> = plan_views.iter().filter_map(|v| v["name"].as_str().map(String::from)).collect();

    let expected = [
        "Lower Electrical",
        "Main Electrical",
        "Lower Floor Plan",
        "Main Floor Plan",
        "Lower Reflected Ceiling",
        "Main Reflected Ceiling",
        "Roof Plan",
        "Site",
    ];

    for name in expected {
        assert!(view_names.contains(&name.to_string()), "Missing plan view: {}", name);
    }
}

#[test]
fn showcase_scripts_are_step_arrays() {
    let p20 = read_json("docs/arch3d/showcase/phase-20-showcase.json");
    let p21 = read_json("docs/arch3d/showcase/phase-21-showcase.json");

    for (name, value) in [("phase-20", p20), ("phase-21", p21)] {
        let arr = value.as_array().unwrap_or_else(|| panic!("{} is not an array", name));
        assert!(!arr.is_empty(), "{} is empty", name);

        for step in arr {
            assert!(step.is_object(), "{} contains non-object step", name);
            assert!(step["cmd"].is_string(), "{} step missing cmd string", name);
            assert!(step["params"].is_object(), "{} step missing params object", name);
        }
    }
}
use cadcraft_doc::Drawing;
use cadcraft_engine::Session;
use serde_json::{Value, json};

/// Reads a JSON file from the repository relative to CARGO_MANIFEST_DIR.
fn read_json(path: &str) -> Value {
    let full_path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path);
    let content = std::fs::read_to_string(&full_path).expect("failed to read JSON file");
    serde_json::from_str(&content).expect("failed to parse JSON")
}

#[test]
fn phase_20_showcase_planviews_replay() {
    let mut session = Session::new();

    // Create levels Lower and Main as bilevel.json does
    session.execute("level", &json!({"name": "Lower", "elevation": 0.0, "ceiling_height": 2.4})).expect("create Lower level");
    session.execute("level", &json!({"name": "Main", "elevation": 2.7, "ceiling_height": 2.4})).expect("create Main level");

    let steps = read_json("docs/arch3d/showcase/phase-20-showcase.json");
    let steps_arr = steps.as_array().expect("steps should be an array");

    for step in steps_arr {
        let cmd = step["cmd"].as_str().expect("step should have cmd");
        let params = step["params"].clone();

        // Only execute planview commands for this specific test
        if cmd.starts_with("planview") {
            let result = session.execute(cmd, &params);
            assert!(result.is_ok(), "planview command failed: {:?} with params {:?}", result, params);
        }
    }

    // Verify that the expected plan views exist
    let inspect_result = session.execute("arch.inspect", &json!({"planViews": true})).expect("arch.inspect failed");
    let plan_views = inspect_result["planViews"].as_array().expect("planViews should be an array");

    let view_names: Vec<String> = plan_views.iter().filter_map(|v| v["name"].as_str().map(String::from)).collect();

    let expected = [
        "Lower Electrical",
        "Main Electrical",
        "Lower Floor Plan",
        "Main Floor Plan",
        "Lower Reflected Ceiling",
        "Main Reflected Ceiling",
        "Roof Plan",
        "Site",
    ];

    for name in expected {
        assert!(view_names.contains(&name.to_string()), "Missing plan view: {}", name);
    }
}

#[test]
fn showcase_scripts_are_step_arrays() {
    let p20 = read_json("docs/arch3d/showcase/phase-20-showcase.json");
    let p21 = read_json("docs/arch3d/showcase/phase-21-showcase.json");

    for (name, value) in [("phase-20", p20), ("phase-21", p21)] {
        let arr = value.as_array().unwrap_or_else(|| panic!("{} is not an array", name));
        assert!(!arr.is_empty(), "{} is empty", name);

        for step in arr {
            assert!(step.is_object(), "{} contains non-object step", name);
            assert!(step["cmd"].is_string(), "{} step missing cmd string", name);
            assert!(step["params"].is_object(), "{} step missing params object", name);
        }
    }
}
