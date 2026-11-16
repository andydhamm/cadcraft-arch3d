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

/// Replays the three showcase scripts and returns the session, panicking on any refused step.
fn replay_showcase() -> cadcraft_engine::Session {
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

    // Same set-up as showcase_phase18.rs: headless file I/O and the library hooks the
    // showcase's own library items (pulls, etc.) need.
    cadcraft_engine::cmd::file::set_io(cadcraft_engine::cmd::file::IoHooks {
        read: |_, _| Err("no file reads in this test".to_string()),
        write: |_, _| Ok(b"stand-in export".to_vec()),
        plot: None,
    });
    cadcraft_engine::library::set_library_io(cadcraft_engine::library::LibraryIo {
        read_glb: cadcraft_io::gltf_read::read_glb,
        read_image: cadcraft_io::image_read::read_image,
        encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
        sample: cadcraft_io::library::sample_files,
    });

    let mut session = cadcraft_engine::Session::new();
    session.new_drawing(true);

    // Replay all three showcase scripts in order; panic (with details) on any error.
    let scripts = ["docs/arch3d/showcase/bilevel.json", "docs/arch3d/showcase/phase-20-showcase.json", "docs/arch3d/showcase/phase-21-showcase.json"];
    session.set_views_deferred(true);
    for rel in &scripts {
        let path = format!("{}/{}", root, rel);
        let content = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {} failed: {e}", rel));
        let steps: Vec<Value> = serde_json::from_str(&content).unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", rel));
        for (i, step) in steps.iter().enumerate() {
            let cmd = step["cmd"].as_str().unwrap_or("??");
            let params = step["params"].clone();
            session.execute(cmd, &params).unwrap_or_else(|err| panic!("step {} ({}) in {} failed: {}", i, cmd, rel, err));
        }
    }
    session.set_views_deferred(false);

    session
}

/// Replays the whole showcase (bilevel -> phase-20 -> phase-21) and asserts a full
/// sheet set: no refused step, >= 10 sheets, the first with a camera image, and every
/// required source present (plans, elevations, sections, interior elevations, schedules).
#[test]
fn phase21_showcase_replays_into_a_full_sheet_set() {
    let mut session = replay_showcase();

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
                let source_kind = w.get("source").and_then(|s| s.get("kind")).and_then(|t| t.as_str());
                source_kind == Some("camera")
            })
        })
        .unwrap_or(false);
    assert!(has_camera, "first sheet ({}) must contain a camera source: {}", first_name, first);
    let has_image = first
        .get("views")
        .and_then(|v| v.as_array())
        .map(|views| {
            views.iter().any(|w| {
                let source_kind = w.get("source").and_then(|s| s.get("kind")).and_then(|t| t.as_str());
                if source_kind == Some("camera") {
                    w.get("image").map(|i| i.as_object().is_some() || i.as_str().is_some()).unwrap_or(false)
                } else {
                    false
                }
            })
        })
        .unwrap_or(false);
    assert!(has_image, "first sheet ({}) must have a non-null image", first_name);

    // Sources (kind, name) of every view placed on a sheet.
    let sources: Vec<(String, String)> = sheets
        .iter()
        .flat_map(|s| s.get("views").and_then(|v| v.as_array()).cloned().unwrap_or_default())
        .filter_map(|w| {
            let src = w.get("source")?;
            let kind = src.get("kind")?.as_str()?.to_string();
            let name = src.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            Some((kind, name))
        })
        .collect();
    let count = |kind: &str| sources.iter().filter(|(k, _)| k == kind).count();
    assert!(sources.iter().any(|(k, n)| k == "plan" && n == "Site"), "no `Site` plan on a sheet: {sources:?}");
    assert!(sources.iter().any(|(k, n)| k == "plan" && n == "Roof Plan"), "no `Roof Plan` plan on a sheet: {sources:?}");
    assert!(count("elevation") >= 4, "expected >= 4 elevation views: {sources:?}");
    assert!(count("section") >= 2, "expected >= 2 section views: {sources:?}");
    assert!(count("interior") >= 2, "expected >= 2 interior views: {sources:?}");

    // Schedules: all four kinds are on sheets.
    let schedules = inspect.get("schedules").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    let has_schedule = |kind: &str| {
        sources.iter().filter(|(k, _)| k == "schedule").any(|(_, n)| {
            schedules.iter().any(|r| {
                let named = |key: &str| r.get(key).and_then(|x| x.as_str()) == Some(n.as_str());
                (named("id") || named("name")) && r.get("kind").and_then(|x| x.as_str()) == Some(kind)
            })
        })
    };
    assert!(has_schedule("door"), "missing door schedule");
    assert!(has_schedule("window"), "missing window schedule");
    assert!(has_schedule("cabinet"), "missing cabinet schedule");
    assert!(has_schedule("room"), "missing room schedule");
}

/// The replayed set has a sheet numbered A-121 with three Entry plan sources,
/// 14 sheets in total, and all sheet numbers are unique.
#[test]
fn phase21_showcase_has_an_entry_plans_sheet() {
    let mut session = replay_showcase();

    let inspect = session.execute("arch.inspect", &json!({"sheets": true})).unwrap_or_else(|e| panic!("arch.inspect failed: {e}"));

    let sheets = inspect["sheets"].as_array().expect("sheets should be an array");
    assert_eq!(sheets.len(), 14, "expected 14 sheets, found {}", sheets.len());

    // Find the Entry Plans sheet (A-121).
    let entry_sheet =
        sheets.iter().find(|s| s.get("number").and_then(|n| n.as_str()) == Some("A-121")).expect("no sheet numbered A-121 (Entry Plans)");

    let views = entry_sheet.get("views").and_then(|v| v.as_array()).expect("A-121 must have a views array");
    assert_eq!(views.len(), 3, "A-121 should have 3 views, found {}", views.len());

    let expected_names = ["Entry Floor Plan", "Entry Electrical", "Entry Reflected Ceiling"];
    for expected in &expected_names {
        assert!(
            views.iter().any(|w| {
                let kind = w.get("source").and_then(|s| s.get("kind")).and_then(|t| t.as_str());
                kind == Some("plan") && w.get("source").and_then(|s| s.get("name")).and_then(|n| n.as_str()) == Some(expected)
            }),
            "A-121 is missing plan source `{}`",
            expected
        );
    }

    // All sheet numbers are unique.
    let mut seen = std::collections::HashSet::new();
    for s in sheets {
        let number = s.get("number").and_then(|n| n.as_str()).unwrap_or("unnamed");
        assert!(seen.insert(number), "duplicate sheet number: {}", number);
    }
}

/// All sheet numbers in the phase-21 showcase are unique.
#[test]
fn phase21_sheet_numbers_are_unique() {
    let root = workspace_root();
    let path = format!("{}/docs/arch3d/showcase/phase-21-showcase.json", root);
    let content = fs::read_to_string(&path).expect("phase-21-showcase.json should exist");
    let steps: Vec<Value> = serde_json::from_str(&content).expect("phase-21-showcase.json should be valid JSON");

    let mut seen = std::collections::HashSet::new();
    for step in &steps {
        let cmd = step.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
        if cmd != "sheet" {
            continue;
        }
        let number = step.get("params").and_then(|p| p.get("number")).and_then(|n| n.as_str()).unwrap_or("unnamed");
        if !seen.insert(number) {
            panic!("duplicate sheet number: {}", number);
        }
    }
}

/// The final command in the phase-21 showcase is `layout.set {"name": "Model"}`.
#[test]
fn phase21_layout_set_is_final_command() {
    let root = workspace_root();
    let path = format!("{}/docs/arch3d/showcase/phase-21-showcase.json", root);
    let content = fs::read_to_string(&path).expect("phase-21-showcase.json should exist");
    let steps: Vec<Value> = serde_json::from_str(&content).expect("phase-21-showcase.json should be valid JSON");

    let last = steps.last().expect("phase-21-showcase.json should not be empty");
    let cmd = last.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
    assert_eq!(cmd, "layout.set", "last command should be layout.set, found {}", cmd);
    let name = last.get("params").and_then(|p| p.get("name")).and_then(|n| n.as_str()).unwrap_or("");
    assert_eq!(name, "Model", "layout.set should set name to Model, found {}", name);
}

/// Every `scale` value in the phase-21 showcase is a positive finite number.
#[test]
fn phase21_all_scales_are_positive_finite() {
    let root = workspace_root();
    let path = format!("{}/docs/arch3d/showcase/phase-21-showcase.json", root);
    let content = fs::read_to_string(&path).expect("phase-21-showcase.json should exist");
    let steps: Vec<Value> = serde_json::from_str(&content).expect("phase-21-showcase.json should be valid JSON");

    for (i, step) in steps.iter().enumerate() {
        let params = step.get("params").cloned().unwrap_or_default();
        if let Some(scale) = params.get("scale").and_then(|s| s.as_f64()) {
            assert!(scale.is_finite(), "step {} scale is not finite: {}", i, scale);
            assert!(scale > 0.0, "step {} scale is not positive: {}", i, scale);
        }
    }
}

/// Every command in the phase-21 showcase has a string `cmd` and an object `params`.
#[test]
fn phase21_command_structure_is_valid() {
    let root = workspace_root();
    let path = format!("{}/docs/arch3d/showcase/phase-21-showcase.json", root);
    let content = fs::read_to_string(&path).expect("phase-21-showcase.json should exist");
    let steps: Vec<Value> = serde_json::from_str(&content).expect("phase-21-showcase.json should be valid JSON");

    for (i, step) in steps.iter().enumerate() {
        let cmd = step.get("cmd").and_then(|v| v.as_str());
        assert!(cmd.is_some(), "step {} missing string cmd", i);
        let params = step.get("params");
        assert!(params.map(|p| p.is_object()).unwrap_or(false), "step {} params is not an object", i);
    }
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

/// The Kitchen's interior elevation must draw at least one cabinet line.
#[test]
fn phase21_showcase_kitchen_interior_elevation_draws_cabinets() {
    let mut session = replay_showcase();

    let inspect = session.execute("arch.inspect", &json!({"views": true, "rooms": true})).unwrap_or_else(|e| panic!("arch.inspect failed: {e}"));

    let rooms = inspect["rooms"].as_array().expect("rooms should be an array");
    let kitchen = rooms.iter().find(|r| r.get("name").and_then(|n| n.as_str()) == Some("Kitchen")).expect("a room named Kitchen must exist");
    let kitchen_id = kitchen.get("id").expect("kitchen must have an id");

    let views = inspect["views"].as_array().expect("views should be an array");
    let view = views
        .iter()
        .find(|v| v.get("kind").and_then(|k| k.as_str()) == Some("interior") && v.get("room") == Some(kitchen_id))
        .unwrap_or_else(|| panic!("no interior view for Kitchen (id={kitchen_id})"));

    let view_name = view.get("name").and_then(|n| n.as_str()).unwrap_or("unnamed");
    let error = view.get("error").and_then(|e| e.as_str());
    if let Some(e) = error {
        panic!("Kitchen interior view '{}' has error: {}", view_name, e);
    }

    let lines = view.get("lines").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    let cabinet_lines: Vec<_> = lines.iter().filter(|l| l.get("kind").and_then(|k| k.as_str()) == Some("cabinet")).collect();
    if cabinet_lines.is_empty() {
        let kinds: Vec<String> = lines.iter().filter_map(|l| l.get("kind").and_then(|k| k.as_str())).map(|s| s.to_string()).collect();
        let wall = view.get("wall").map(|w| w.to_string()).unwrap_or_default();
        panic!(
            "Kitchen interior view '{}' has no cabinet lines ({} lines total, kinds: {:?}, wall: {})\nKitchen room: {}",
            view_name,
            lines.len(),
            kinds,
            wall,
            kitchen
        );
    }
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
