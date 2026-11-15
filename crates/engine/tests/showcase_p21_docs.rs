use std::fs;

use serde_json::{Value, json};

fn workspace_root() -> String {
    let mut dir = std::env::current_dir().unwrap();
    loop {
        if dir.join("Cargo.toml").exists() && dir.join("docs").exists() {
            return dir.to_string_lossy().to_string();
        }
        if !dir.pop() {
            panic!("could not find workspace root");
        }
    }
}

#[test]
fn phase_21_showcase_json_exists_and_is_array() {
    let root = workspace_root();
    let path = format!("{}/docs/arch3d/showcase/phase-21-showcase.json", root);
    let content = fs::read_to_string(&path).expect("phase-21-showcase.json should exist");

    // Basic JSON array validation without serde
    let trimmed = content.trim();
    assert!(trimmed.starts_with('['), "phase-21-showcase.json should start with '['");
    assert!(trimmed.ends_with(']'), "phase-21-showcase.json should end with ']'");

    // Count entries by counting {"cmd" occurrences
    let entry_count = content.matches("{\"cmd\"").count();
    assert!(entry_count >= 10, "phase-21-showcase.json should have at least 10 entries, found {}", entry_count);

    // Verify it has the required command types
    assert!(content.contains("\"sheetset\""), "should have a sheetset command");
    assert!(content.contains("\"schedule\""), "should have schedule commands");
    assert!(content.contains("\"elevation\""), "should have elevation commands");
    assert!(content.contains("\"sheet\""), "should have sheet commands");
    assert!(content.contains("\"layout.set\""), "should end with layout.set");
}

/// Every `sheet` step in the phase-21 showcase names its views: a non-empty `views`
/// list, or exactly one of `view` / `planView` / `camera`.
#[test]
fn phase21_showcase_sheet_steps_all_name_their_views() {
    let root = workspace_root();
    let path = format!("{}/docs/arch3d/showcase/phase-21-showcase.json", root);
    let content = fs::read_to_string(&path).expect("phase-21-showcase.json should exist");
    let steps: Vec<Value> = serde_json::from_str(&content).expect("phase-21-showcase.json should be valid JSON");
    let mut sheet_count = 0;
    for (i, step) in steps.iter().enumerate() {
        let cmd = step.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
        if cmd != "sheet" {
            continue;
        }
        sheet_count += 1;
        let params = step.get("params").cloned().unwrap_or_default();
        let non_empty_views = params.get("views").and_then(|v| v.as_array()).map(|v| !v.is_empty()).unwrap_or(false);
        let named = non_empty_views || params.get("view").is_some() || params.get("planView").is_some() || params.get("camera").is_some();
        if !named {
            panic!(
                "sheet step {} (name={}) does not name its views (no non-empty 'views', 'view', 'planView' or 'camera')",
                i,
                params.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed")
            );
        }
    }
    assert!(sheet_count >= 10, "expected at least 10 sheets, found {}", sheet_count);
}

/// Replays the whole showcase (bilevel -> phase-20 -> phase-21) and asserts a full
/// sheet set: no refused step, >= 10 sheets, the first with a camera image, and every
/// required source present (plans, elevations, sections, interior elevations, schedules).
#[test]
fn phase21_showcase_replays_into_a_full_sheet_set() {
    let root = workspace_root();

    // Install a fake renderer that writes a tiny PNG, so `sheet` camera views can run
    // in a headless test without a real GPU. Mirrors the test module in
    // crates/engine/src/cmd/arch/sheet_camera.rs.
    {
        use cadcraft_engine::cmd::file::set_image_decoder;
        use cadcraft_engine::{RenderHooks, RenderRequest, RenderStats, set_render_hooks};
        let render = |req: &RenderRequest| -> std::result::Result<Value, String> {
            let out = req.out.as_ref().ok_or_else(|| "missing image path".to_string())?;
            let raster = cadcraft_doc::Raster::new(2, 1, vec![0, 0, 0, 255, 255, 255, 255, 255])
                .ok_or_else(|| "failed to build a 2x1 RGBA raster".to_string())?;
            let png = cadcraft_io::raster::encode_png(&raster).map_err(|e| format!("encode PNG: {e}"))?;
            std::fs::write(out, png).map_err(|e| format!("write PNG: {e}"))?;
            Ok(json!({"ok": true, "path": out.to_string_lossy()}))
        };
        set_render_hooks(RenderHooks::new(render, RenderStats::default));
        set_image_decoder(|b| cadcraft_io::raster::decode(b).map_err(|e| e.to_string()));
    }

    let mut session = cadcraft_engine::Session::new();

    // Replay all three showcase scripts in order; panic (with details) on any error.
    let scripts = ["docs/arch3d/showcase/bilevel.json", "docs/arch3d/showcase/phase-20-showcase.json", "docs/arch3d/showcase/phase-21-showcase.json"];
    for rel in &scripts {
        let path = format!("{}/{}", root, rel);
        let content = fs::read_to_string(&path).unwrap_or_else(|e| format!("read {} failed: {e}", rel));
        let steps: Vec<Value> = serde_json::from_str(&content).unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", rel));
        for (i, step) in steps.iter().enumerate() {
            let cmd = step["cmd"].as_str().unwrap_or("??");
            let params = step["params"].clone();
            session.execute(cmd, &params).unwrap_or_else(|err| panic!("step {} ({}) in {} failed: {}", i, cmd, rel, err));
        }
    }

    let inspect = session
        .execute("arch.inspect", &json!({"sheets": true, "views": true, "schedules": true}))
        .unwrap_or_else(|e| panic!("arch.inspect failed: {e}"));

    // >= 10 sheets.
    let sheets = inspect["sheets"].as_array().expect("sheets should be an array");
    assert!(sheets.len() >= 10, "expected >= 10 sheets, found {}", sheets.len());

    // The first sheet must carry a camera render with a non-null image.
    let first = &sheets[0];
    let first_name = first.get("name").and_then(|n| n.as_str()).unwrap_or("unnamed");
    let has_camera = first
        .get("views")
        .and_then(|v| v.as_array())
        .map(|views| {
            views.iter().any(|w| {
                let source_type = w.get("source").and_then(|s| s.get("type")).and_then(|t| t.as_str());
                source_type == Some("camera")
            })
        })
        .unwrap_or(false);
    assert!(has_camera, "first sheet ({}) must contain a camera source", first_name);
    let has_image = first
        .get("views")
        .and_then(|v| v.as_array())
        .map(|views| {
            views.iter().any(|w| {
                let source_type = w.get("source").and_then(|s| s.get("type")).and_then(|t| t.as_str());
                if source_type == Some("camera") {
                    w.get("image").map(|i| i.as_object().is_some() || i.as_str().is_some()).unwrap_or(false)
                } else {
                    false
                }
            })
        })
        .unwrap_or(false);
    assert!(has_image, "first sheet ({}) must have a non-null image", first_name);

    // View sources from arch.inspect.
    let view_items: Vec<&Value> =
        inspect.get("views").and_then(|v| v.as_array()).map(|arr| arr.iter().map(|v| v as &Value).collect::<Vec<_>>()).unwrap_or_default();

    let view_names: Vec<String> = view_items.iter().filter_map(|v| v.get("name").and_then(|n| n.as_str())).map(String::from).collect();
    let view_types: Vec<String> = view_items.iter().filter_map(|v| v.get("type").and_then(|t| t.as_str())).map(String::from).collect();

    let has = |kind: &str| view_names.iter().any(|n| n.to_lowercase().contains(kind));
    let has_type = |kind: &str| view_types.iter().any(|t| t.to_lowercase().contains(kind));

    assert!(has("site"), "no `Site` view source");
    assert!(has("roof plan"), "no `Roof Plan` view source");
    let elev_count = view_names.iter().filter(|n| n.to_lowercase().contains("elev")).count();
    assert!(elev_count >= 4, "expected >= 4 elevation views, found {}", elev_count);
    let sec_count = view_names.iter().filter(|n| n.to_lowercase().contains("section")).count();
    assert!(sec_count >= 2, "expected >= 2 section views, found {}", sec_count);
    let interior_count = view_names.iter().filter(|n| n.to_lowercase().contains("interior")).count();
    assert!(interior_count >= 2, "expected >= 2 interior elevation views, found {}", interior_count);
    assert!(has_type("camera") || has("camera"), "no camera view source");

    // Schedules: all four kinds present.
    let has_schedule = |kind: &str| {
        inspect
            .get("schedules")
            .and_then(|s| s.as_array())
            .map(|arr| arr.iter().any(|r| r.get("kind").and_then(|k| k.as_str()) == Some(kind)))
            .unwrap_or(false)
    };
    assert!(has_schedule("door"), "missing door schedule");
    assert!(has_schedule("window"), "missing window schedule");
    assert!(has_schedule("cabinet"), "missing cabinet schedule");
    assert!(has_schedule("room"), "missing room schedule");
}

#[test]
fn readme_mentions_phase_21_and_plot() {
    let root = workspace_root();
    let path = format!("{}/docs/arch3d/showcase/README.md", root);
    let content = fs::read_to_string(&path).expect("README.md should exist");

    assert!(content.contains("phase-21-showcase.json"), "README.md should mention phase-21-showcase.json");
    assert!(content.contains("plot"), "README.md should mention the plot command");
    assert!(content.contains("showcase-set.pdf"), "README.md should mention showcase-set.pdf");
}

#[test]
fn user_guide_has_p21_sections() {
    let root = workspace_root();
    let path = format!("{}/docs/arch3d/USER-GUIDE.md", root);
    let content = fs::read_to_string(&path).expect("USER-GUIDE.md should exist");

    assert!(content.contains("## Schedules and labels"), "USER-GUIDE.md should have '## Schedules and labels' section");
    assert!(content.contains("## Interior elevations and view notes"), "USER-GUIDE.md should have '## Interior elevations and view notes' section");
    assert!(content.contains("## Sheets"), "USER-GUIDE.md should have '## Sheets' section");

    // Sections should be before "Out of scope"
    let out_of_scope = content.find("## Out of scope").expect("USER-GUIDE.md should have '## Out of scope'");
    let schedules = content.find("## Schedules and labels").expect("missing schedules section");
    let interior = content.find("## Interior elevations and view notes").expect("missing interior elevations section");
    let sheets = content.find("## Sheets").expect("missing sheets section");

    assert!(schedules < out_of_scope, "Schedules section should be before Out of scope");
    assert!(interior < out_of_scope, "Interior elevations section should be before Out of scope");
    assert!(sheets < out_of_scope, "Sheets section should be before Out of scope");
}
