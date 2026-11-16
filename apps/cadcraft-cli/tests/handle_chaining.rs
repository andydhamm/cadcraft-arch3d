//! A `--cmd` can act on a handle an earlier `--cmd` in the same run returned (#1663).
//!
//! `mview` answers `{"viewports":["100"]}`, but a following `viewport.set` could not use that
//! handle: it had to be guessed, selected by point, or activated with `mspace`. A `--cmd` parameter
//! value of exactly `{last}` is now filled in with the last handle an earlier `--cmd` returned.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cadcraft-cli"))
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("run cadcraft-cli")
}

/// The issue's sequence: `viewport.set` straight after `mview`, addressing the new viewport by the
/// handle the runner remembers.
#[test]
fn viewport_set_after_mview_uses_the_returned_handle() {
    let out = run(&[
        "run",
        "--metric",
        "--cmd",
        r#"layout.set {"name":"Layout1"}"#,
        "--cmd",
        r#"mview {"p1":[20,20],"p2":[200,150]}"#,
        "--cmd",
        r#"viewport.set {"handle":"{last}","scale":1}"#,
    ]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "exit {:?}\nstdout: {stdout}\nstderr: {stderr}", out.status);
    let mview = stdout.lines().find(|l| l.contains("\"viewports\"")).unwrap_or_else(|| panic!("no mview result in {stdout:?}"));
    let created: serde_json::Value = serde_json::from_str(mview).expect("mview result is JSON");
    let handle = created["viewports"].as_array().and_then(|a| a.first()).and_then(|h| h.as_str()).expect("a viewport handle");
    assert!(handle.eq_ignore_ascii_case("100"), "{mview}");
    let set = stdout.lines().find(|l| l.contains("\"viewHeight\"")).unwrap_or_else(|| panic!("no viewport.set result in {stdout:?}"));
    let vp: serde_json::Value = serde_json::from_str(set).expect("viewport.set result is JSON");
    assert_eq!(vp[0]["handle"].as_str(), Some(handle), "viewport.set changed the viewport `{handle}`: {set}");
    assert_eq!(vp[0]["scale"].as_f64(), Some(1.0), "{set}");
}

/// `{last}` with no handle to fill in is a clear error, not the token reaching the command.
#[test]
fn last_without_an_earlier_handle_is_an_error() {
    let out = run(&["run", "--metric", "--cmd", r#"viewport.set {"handle":"{last}","scale":1}"#]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "stdout: {:?}\nstderr: {stderr}", String::from_utf8_lossy(&out.stdout));
    assert!(stderr.contains("{last}"), "the error names the placeholder: {stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

/// The last JSON value a run printed: command results are one JSON value a line (`null` is not
/// printed).
fn last_json(stdout: &str) -> serde_json::Value {
    let line = stdout
        .lines()
        .rev()
        .find(|l| {
            let t = l.trim_start();
            t.starts_with('{') || t.starts_with('[')
        })
        .unwrap_or_else(|| panic!("no JSON result in {stdout:?}"));
    serde_json::from_str(line).expect("command result is JSON")
}

/// `{last}` was not replaced: the error names the placeholder, and the run fails cleanly.
fn assert_needs_an_earlier_handle(out: &Output) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "stdout: {:?}\nstderr: {stderr}", String::from_utf8_lossy(&out.stdout));
    assert!(stderr.contains("{last}"), "the error names the placeholder: {stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

/// A user-defined name that happens to look like a handle is not a returned handle. `group`
/// answers `{"name":"100","members":1}`: `100` is the first line's handle, but the name is not an
/// object reference and must not replace the last handle the `line` command returned (`101`).
#[test]
fn a_group_name_that_looks_like_a_handle_is_not_last() {
    let out = run(&[
        "run",
        "--metric",
        "--cmd",
        r#"line {"points":[[0,0],[10,0]]}"#,
        "--cmd",
        r#"line {"points":[[0,10],[10,10]]}"#,
        "--cmd",
        r#"group {"name":"100","handles":["101"]}"#,
        "--cmd",
        r#"erase {"handles":["{last}"]}"#,
        "--cmd",
        r#"entities {"type":"LINE"}"#,
    ]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "exit {:?}\nstdout: {stdout}\nstderr: {stderr}", out.status);
    let r = last_json(&stdout);
    assert_eq!(r["count"].as_u64(), Some(1), "one line is left: {stdout}");
    let left: Vec<&str> = r["entities"].as_array().into_iter().flatten().filter_map(|e| e["handle"].as_str()).collect();
    assert_eq!(left, ["100"], "the group name 100 did not replace the handle 101 the line command returned: {stdout}");
}

/// `new` makes another drawing current: the handle an earlier `--cmd` returned in the old drawing
/// is not a handle of the new one.
#[test]
fn last_is_cleared_when_new_switches_drawings() {
    let out = run(&[
        "run",
        "--metric",
        "--cmd",
        r#"line {"points":[[0,0],[10,0]]}"#,
        "--cmd",
        r#"new {"metric":true}"#,
        "--cmd",
        r#"erase {"handles":["{last}"]}"#,
    ]);
    assert_needs_an_earlier_handle(&out);
}

/// `open` makes the opened file the current drawing: a handle from the drawing before it is stale.
#[test]
fn last_is_cleared_when_open_switches_drawings() {
    let dir = std::env::temp_dir().join(format!("cc-cli-last-open-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let dxf = dir.join("opened.dxf");
    let sample = run(&["sample", "bracket", dxf.to_str().expect("temp path")]);
    assert!(sample.status.success(), "sample failed: {}", String::from_utf8_lossy(&sample.stderr));
    let open = format!(r#"open {{"path":{}}}"#, serde_json::to_string(dxf.to_str().expect("temp path")).expect("path JSON"));
    let out = run(&["run", "--metric", "--cmd", r#"line {"points":[[0,0],[10,0]]}"#, "--cmd", &open, "--cmd", r#"erase {"handles":["{last}"]}"#]);
    assert_needs_an_earlier_handle(&out);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `document.switch` changes the current drawing without adding one: a handle remembered from the
/// drawing left behind must not be used in the one switched to.
#[test]
fn last_is_cleared_when_document_switch_changes_drawings() {
    let out = run(&[
        "run",
        "--metric",
        "--cmd",
        r#"line {"points":[[0,0],[10,0]]}"#,
        "--cmd",
        r#"new {"metric":true}"#,
        "--cmd",
        r#"line {"points":[[0,10],[10,10]]}"#,
        "--cmd",
        r#"document.switch {"index":0}"#,
        "--cmd",
        r#"erase {"handles":["{last}"]}"#,
    ]);
    assert_needs_an_earlier_handle(&out);
}

/// A handle a command returns after a switch belongs to the drawing now current, so it is kept.
#[test]
fn a_handle_returned_after_a_switch_is_last() {
    let out = run(&[
        "run",
        "--metric",
        "--cmd",
        r#"line {"points":[[0,0],[10,0]]}"#,
        "--cmd",
        r#"new {"metric":true}"#,
        "--cmd",
        r#"document.switch {"index":0}"#,
        "--cmd",
        r#"line {"points":[[0,10],[10,10]]}"#,
        "--cmd",
        r#"erase {"handles":["{last}"]}"#,
        "--cmd",
        r#"entities {"type":"LINE"}"#,
    ]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "exit {:?}\nstdout: {stdout}\nstderr: {stderr}", out.status);
    let r = last_json(&stdout);
    assert_eq!(r["count"].as_u64(), Some(1), "one line is left: {stdout}");
    let left: Vec<&str> = r["entities"].as_array().into_iter().flatten().filter_map(|e| e["handle"].as_str()).collect();
    assert_eq!(left, ["100"], "the line created after the switch was erased: {stdout}");
}

/// A script can change the current drawing too (`new {"metric":true}` at a script line), so the
/// remembered handle is cleared after a script that changed it.
#[test]
fn last_is_cleared_by_a_script_that_changes_drawings() {
    let out = run(&[
        "run",
        "--metric",
        "--cmd",
        r#"line {"points":[[0,0],[10,0]]}"#,
        "--script",
        r#"new {"metric":true}"#,
        "--cmd",
        r#"erase {"handles":["{last}"]}"#,
    ]);
    assert_needs_an_earlier_handle(&out);
}

/// Only the exact token is a placeholder: ordinary JSON strings are left alone.
#[test]
fn ordinary_values_are_not_placeholders() {
    let dir = std::env::temp_dir().join(format!("cc-cli-last-placeholder-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let out_dxf = dir.join("text.dxf");
    let out = run(&[
        "run",
        "--metric",
        "--cmd",
        r#"layout.set {"name":"Layout1"}"#,
        "--cmd",
        r#"mview {"p1":[20,20],"p2":[200,150]}"#,
        "--cmd",
        r#"text {"at":[0,0],"height":2,"text":"last and {last}"}"#,
        "--save",
        out_dxf.to_str().expect("temp path"),
    ]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "exit {:?}\nstderr: {stderr}", out.status);
    let saved = std::fs::read_to_string(&out_dxf).expect("saved DXF");
    assert!(saved.contains("last and {last}"), "the text is stored literally");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Writes the phase-1 house commands (wall, elevation, door, two windows) to a JSON
/// `--commands` file and returns the path.
fn write_house_commands(path: &std::path::Path) {
    std::fs::write(
        path,
        r#"[
  {"cmd": "wall", "params": {"points": [[0,0],[10,0],[10,8],[0,8],[0,0]], "thickness": 0.2, "exterior": true}},
  {"cmd": "elevation", "params": {"side": "south", "name": "South", "insert": [0, 25], "scale": 50}},
  {"cmd": "door", "params": {"wall": "w0", "offset": 3.0, "width": 0.9}},
  {"cmd": "window", "params": {"wall": "w0", "offset": 6.0, "width": 1.0, "height": 1.5}},
  {"cmd": "window", "params": {"wall": "w0", "offset": 8.0, "width": 1.0, "height": 1.5}}
]
"#,
    )
    .expect("write commands file");
}

/// The number of line entries in the named view from a `--cmd arch.inspect {"views": true}`
/// run: the first JSON line of stdout that is not null, under `views`, for the view with that
/// `name`, counting the view's `lines`.
fn view_line_count(stdout: &str, view_name: &str) -> usize {
    let line = stdout.lines().rev().find(|l| l.trim_start().starts_with('{')).expect("no JSON result in stdout");
    let r: serde_json::Value = serde_json::from_str(line).expect("inspect result is JSON");
    let views = r["views"].as_array().expect("inspect has a views array");
    let view = views.iter().find(|v| v["name"].as_str() == Some(view_name)).expect("view not found");
    view["lines"].as_array().map(|a| a.len()).unwrap_or_default()
}

/// `--commands` replayed with views deferred still gives fresh lines when a reading
/// `arch.inspect` follows; the South elevation carries the door and both windows.
#[test]
fn run_commands_with_views_gives_the_same_drawing_as_before() {
    let dir = std::env::temp_dir().join(format!("p21t-cli-views-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let cmds = dir.join("house.json");
    write_house_commands(&cmds);
    let out = run(&["run", "--metric", "--commands", cmds.to_str().expect("commands path"), "--cmd", r#"arch.inspect {"views": true}"#]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "exit {:?}\nstdout: {stdout}\nstderr: {stderr}", out.status);
    let n = view_line_count(&stdout, "South");
    assert!(n > 4, "South view has no lines for door and windows: {stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The catch-up before `--save` writes fresh views: a drawing saved from a `--commands` run
/// inspects to the same South-view line count as a run that saved nothing.
#[test]
fn run_defers_views_but_saves_them_fresh() {
    let dir = std::env::temp_dir().join(format!("p21t-cli-views-save-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let cmds = dir.join("house.json");
    write_house_commands(&cmds);
    let out_dxf = dir.join("house.dxf");
    let out = run(&["run", "--metric", "--commands", cmds.to_str().expect("commands path"), "--save", out_dxf.to_str().expect("dxf path")]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "exit {:?}\nstderr: {stderr}", out.status);
    let out2 = run(&["run", "--metric", "--cmd", r#"arch.inspect {"views": true}"#, out_dxf.to_str().expect("dxf path")]);
    let stdout = String::from_utf8_lossy(&out2.stdout);
    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(out2.status.success(), "re-open exit {:?}\nstderr: {stderr2}", out2.status);
    let saved = view_line_count(&stdout, "South");
    let fresh_dir = std::env::temp_dir().join(format!("p21t-cli-views-fresh-{}", std::process::id()));
    std::fs::create_dir_all(&fresh_dir).expect("temp dir");
    let cmds2 = fresh_dir.join("house.json");
    write_house_commands(&cmds2);
    let out3 = run(&["run", "--metric", "--commands", cmds2.to_str().expect("commands path"), "--cmd", r#"arch.inspect {"views": true}"#]);
    let stdout3 = String::from_utf8_lossy(&out3.stdout);
    let fresh = view_line_count(&stdout3, "South");
    assert_eq!(saved, fresh, "saved view lines {saved} != fresh {fresh}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&fresh_dir);
}

/// A `--script` step does not defer views (the switch is not guarded there): the same house
/// plus a harmless script still inspects to the fresh South-view line count.
#[test]
fn run_script_steps_do_not_defer() {
    let dir = std::env::temp_dir().join(format!("p21t-cli-views-script-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let cmds = dir.join("house.json");
    write_house_commands(&cmds);
    let out = run(&[
        "run",
        "--metric",
        "--commands",
        cmds.to_str().expect("commands path"),
        "--script",
        r#"entities {"type":"LINE"}
"#,
        "--cmd",
        r#"arch.inspect {"views": true}"#,
    ]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "exit {:?}\nstdout: {stdout}\nstderr: {stderr}", out.status);
    let n = view_line_count(&stdout, "South");
    assert!(n > 4, "South view has no lines after script: {stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--export` triggers the view catch-up before writing, just like `--save`.
#[test]
fn run_export_triggers_view_catchup() {
    let dir = std::env::temp_dir().join(format!("p21t-cli-views-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let cmds = dir.join("house.json");
    write_house_commands(&cmds);
    let out_dxf = dir.join("house-export.dxf");
    let out = run(&["run", "--metric", "--commands", cmds.to_str().expect("commands path"), "--export", out_dxf.to_str().expect("dxf path")]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "exit {:?}\nstderr: {stderr}", out.status);
    let out2 = run(&["run", "--metric", "--cmd", r#"arch.inspect {"views": true}"#, out_dxf.to_str().expect("dxf path")]);
    let stdout = String::from_utf8_lossy(&out2.stdout);
    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(out2.status.success(), "re-open exit {:?}\nstderr: {stderr2}", out2.status);
    let exported = view_line_count(&stdout, "South");
    let fresh_dir = std::env::temp_dir().join(format!("p21t-cli-views-fresh-export-{}", std::process::id()));
    std::fs::create_dir_all(&fresh_dir).expect("temp dir");
    let cmds2 = fresh_dir.join("house.json");
    write_house_commands(&cmds2);
    let out3 = run(&["run", "--metric", "--commands", cmds2.to_str().expect("commands path"), "--cmd", r#"arch.inspect {"views": true}"#]);
    let stdout3 = String::from_utf8_lossy(&out3.stdout);
    let fresh = view_line_count(&stdout3, "South");
    assert_eq!(exported, fresh, "exported view lines {exported} != fresh {fresh}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&fresh_dir);
}

/// A reading command (`arch.inspect {"views": true}`) between model-building steps
/// triggers view regeneration, so it sees the changes made so far.
#[test]
fn run_reading_command_between_build_steps_regenerates() {
    let dir = std::env::temp_dir().join(format!("p21t-cli-views-read-between-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let cmds = dir.join("house.json");
    std::fs::write(
        &cmds,
        r#"[
  {"cmd": "wall", "params": {"points": [[0,0],[10,0],[10,8],[0,8],[0,0]], "thickness": 0.2, "exterior": true}},
  {"cmd": "elevation", "params": {"side": "south", "name": "South", "insert": [0, 25], "scale": 50}},
  {"cmd": "door", "params": {"wall": "w0", "offset": 3.0, "width": 0.9}},
  {"cmd": "arch.inspect", "params": {"views": true}},
  {"cmd": "window", "params": {"wall": "w0", "offset": 6.0, "width": 1.0, "height": 1.5}},
  {"cmd": "window", "params": {"wall": "w0", "offset": 8.0, "width": 1.0, "height": 1.5}},
  {"cmd": "arch.inspect", "params": {"views": true}}
]
"#,
    )
    .expect("write commands file");
    let out = run(&["run", "--metric", "--commands", cmds.to_str().expect("commands path")]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "exit {:?}\nstdout: {stdout}\nstderr: {stderr}", out.status);
    let inspect_lines: Vec<&str> = stdout.lines().filter(|l| l.trim_start().starts_with('{') && l.contains("\"views\"")).collect();
    assert_eq!(inspect_lines.len(), 2, "two inspect results expected: {stdout}");
    let first: serde_json::Value = serde_json::from_str(inspect_lines[0]).expect("first inspect is JSON");
    let second: serde_json::Value = serde_json::from_str(inspect_lines[1]).expect("second inspect is JSON");
    let first_views = first["views"].as_array().expect("views array");
    let second_views = second["views"].as_array().expect("views array");
    let first_south = first_views.iter().find(|v| v["name"].as_str() == Some("South")).expect("South view in first inspect");
    let second_south = second_views.iter().find(|v| v["name"].as_str() == Some("South")).expect("South view in second inspect");
    let first_lines = first_south["lines"].as_array().map(|a| a.len()).unwrap_or_default();
    let second_lines = second_south["lines"].as_array().map(|a| a.len()).unwrap_or_default();
    assert!(second_lines > first_lines, "second inspect ({second_lines}) should have more lines than first ({first_lines})");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A failing `--cmd` step does not prevent the view catch-up before `--save`.
#[test]
fn run_failing_command_still_catches_up_before_save() {
    let dir = std::env::temp_dir().join(format!("p21t-cli-views-fail-save-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let cmds = dir.join("house.json");
    write_house_commands(&cmds);
    let out_dxf = dir.join("house-fail.dxf");
    let out = run(&[
        "run",
        "--metric",
        "--commands",
        cmds.to_str().expect("commands path"),
        "--cmd",
        r#"door {"wall": "w999", "offset": 3.0, "width": 0.9}"#,
        "--save",
        out_dxf.to_str().expect("dxf path"),
    ]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "run should fail due to the bad door command");
    assert!(stderr.contains("w999"), "error should mention the missing wall: {stderr}");
    assert!(out_dxf.exists(), "the file should be saved despite the failing command");
    let out2 = run(&["run", "--metric", "--cmd", r#"arch.inspect {"views": true}"#, out_dxf.to_str().expect("dxf path")]);
    let stdout = String::from_utf8_lossy(&out2.stdout);
    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(out2.status.success(), "re-open exit {:?}\nstderr: {stderr2}", out2.status);
    let saved = view_line_count(&stdout, "South");
    let fresh_dir = std::env::temp_dir().join(format!("p21t-cli-views-fresh-fail-{}", std::process::id()));
    std::fs::create_dir_all(&fresh_dir).expect("temp dir");
    let cmds2 = fresh_dir.join("house.json");
    write_house_commands(&cmds2);
    let out3 = run(&["run", "--metric", "--commands", cmds2.to_str().expect("commands path"), "--cmd", r#"arch.inspect {"views": true}"#]);
    let stdout3 = String::from_utf8_lossy(&out3.stdout);
    let fresh = view_line_count(&stdout3, "South");
    assert_eq!(saved, fresh, "saved view lines {saved} != fresh {fresh}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&fresh_dir);
}
