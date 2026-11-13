use std::fs;

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
