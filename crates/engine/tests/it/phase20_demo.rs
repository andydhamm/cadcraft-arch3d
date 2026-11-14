use serde_json::Value;

fn load_demo() -> Value {
    let s = include_str!("../../../../docs/arch3d/showcase/phase-20-demo.json");
    serde_json::from_str(s).expect("phase-20-demo.json is valid JSON")
}

fn load_showcase() -> Value {
    let s = include_str!("../../../../docs/arch3d/showcase/phase-20-showcase.json");
    serde_json::from_str(s).expect("phase-20-showcase.json is valid JSON")
}

fn cmd_names(v: &Value) -> Vec<String> {
    v.as_array()
        .expect("is array")
        .iter()
        .map(|s| s["cmd"].as_str().expect("cmd is string").to_string())
        .collect()
}

#[test]
fn demo_is_array_of_commands() {
    let v = load_demo();
    assert!(v.is_array(), "phase-20-demo.json must be a JSON array");
    assert!(!v.as_array().unwrap().is_empty(), "demo must not be empty");
    for step in v.as_array().unwrap() {
        assert!(step.is_object(), "each step must be an object");
        assert!(step.get("cmd").is_some(), "each step must have a cmd");
        assert!(step.get("params").is_some(), "each step must have params");
    }
}

#[test]
fn showcase_is_array_of_commands() {
    let v = load_showcase();
    assert!(v.is_array(), "phase-20-showcase.json must be a JSON array");
    assert!(!v.as_array().unwrap().is_empty(), "showcase must not be empty");
    for step in v.as_array().unwrap() {
        assert!(step.is_object(), "each step must be an object");
        assert!(step.get("cmd").is_some(), "each step must have a cmd");
        assert!(step.get("params").is_some(), "each step must have params");
    }
}

#[test]
fn demo_has_dimauto_for_both_levels() {
    let v = load_demo();
    let steps = v.as_array().unwrap();
    let mut level0 = false;
    let mut level1 = false;
    for step in steps {
        if step["cmd"] == "dimauto" {
            if step["params"]["level"] == 0 {
                level0 = true;
            }
            if step["params"]["level"] == 1 {
                level1 = true;
            }
        }
    }
    assert!(level0, "demo must have dimauto for level 0");
    assert!(level1, "demo must have dimauto for level 1");
}

#[test]
fn demo_has_all_plan_views() {
    let v = load_demo();
    let names = cmd_names(&v);
    // Six bound plan views
    assert!(names.contains(&"planview".to_string()), "demo must create plan views");
    let mut planview_count = 0;
    let mut has_open = false;
    for step in v.as_array().unwrap() {
        if step["cmd"] == "planview" {
            planview_count += 1;
        }
        if step["cmd"] == "planview.open" {
            has_open = true;
        }
    }
    assert!(
        planview_count >= 6,
        "demo must create at least 6 bound plan views, found {}",
        planview_count
    );
    assert!(has_open, "demo must open a plan view at the end");
}

#[test]
fn showcase_has_dimauto_and_views_for_floor_levels() {
    let v = load_showcase();
    let mut dimauto_levels = Vec::new();
    let mut planview_levels = Vec::new();
    for step in v.as_array().unwrap() {
        if step["cmd"] == "dimauto" {
            let lvl = step["params"]["level"]
                .as_str()
                .expect("showcase dimauto level is a string");
            dimauto_levels.push(lvl.to_string());
        }
        if step["cmd"] == "planview" {
            let lvl = step["params"]["level"]
                .as_str()
                .expect("showcase planview level is a string");
            planview_levels.push(lvl.to_string());
        }
    }
    // The showcase has two floor levels: Lower and Main
    assert!(
        dimauto_levels.contains(&"Lower".to_string()),
        "showcase must dimauto Lower level"
    );
    assert!(
        dimauto_levels.contains(&"Main".to_string()),
        "showcase must dimauto Main level"
    );
    assert!(
        planview_levels.contains(&"Lower".to_string()),
        "showcase must create plan views for Lower"
    );
    assert!(
        planview_levels.contains(&"Main".to_string()),
        "showcase must create plan views for Main"
    );
}
