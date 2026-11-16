//! Runs the standard-library Python tests of `tools/showcase/render_all.py`
//! (the gallery tool) so the Rust gate sees them. Each test runs one
//! `unittest` case; it is skipped, with a note, when `python3` is not installed.

use std::path::PathBuf;
use std::process::Command;

fn run_case(name: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = root.join("tools/showcase/test_render_all.py");
    let out = Command::new("python3")
        .current_dir(&root)
        .arg("-m")
        .arg("unittest")
        .arg(format!("test_render_all.RenderAll.{name}"))
        .env("PYTHONPATH", root.join("tools/showcase"))
        .output();
    let out = match out {
        Ok(o) => o,
        Err(e) => {
            eprintln!("python3 not available ({e}); skipping {name}");
            return;
        }
    };
    assert!(file.exists(), "missing {}", file.display());
    assert!(out.status.success(), "{name} failed:\n{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

#[test]
fn test_plan_lists_every_gallery_step() {
    run_case("test_plan_lists_every_gallery_step");
}

#[test]
fn test_drawing_set_steps_replay_three_scripts() {
    run_case("test_drawing_set_steps_replay_three_scripts");
}

#[test]
fn test_no_set_skips_sheets_and_pdf() {
    run_case("test_no_set_skips_sheets_and_pdf");
}

#[test]
fn test_manifest_kinds_and_script_hashes() {
    run_case("test_manifest_kinds_and_script_hashes");
}

#[test]
fn test_slug_and_names_are_stable() {
    run_case("test_slug_and_names_are_stable");
}

#[test]
fn test_plan_filters_invalid_views() {
    run_case("test_plan_filters_invalid_views");
}

#[test]
fn test_plan_filters_invalid_sheets() {
    run_case("test_plan_filters_invalid_sheets");
}

#[test]
fn test_plan_duplicate_view_names() {
    run_case("test_plan_duplicate_view_names");
}

#[test]
fn test_plan_mixed_level_kinds() {
    run_case("test_plan_mixed_level_kinds");
}

#[test]
fn test_readme_gallery_matches_the_plan() {
    run_case("test_readme_gallery_matches_the_plan");
}
