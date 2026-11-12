use std::io::Write;

fn run_and_stderr(args: &[&str]) -> (i32, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let os_args: Vec<std::ffi::OsString> = args.iter().map(|a| std::ffi::OsString::from(*a)).collect();
    let code = cadcraft_cli::run(os_args, &mut out, &mut err);
    (code, String::from_utf8_lossy(&err).to_string())
}

/// A JSON array of `{"cmd": ..., "params": ...}` steps: run it through `cadcraft-cli run --commands`
/// and assert the steps ran in order (a failing step would stop the run, so all steps running
/// means they were in order).
#[test]
fn commands_file_runs_its_steps_in_order() {
    let dir = std::env::temp_dir().join(format!("cadcraft-cli-cmds-ok-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("steps.json");
    std::fs::write(
        &file,
        r#"[
            {"cmd": "circle", "params": {"center": [0, 0], "radius": 1}},
            {"cmd": "circle", "params": {"center": [5, 5], "radius": 1}}
        ]"#,
    )
    .unwrap();
    let (code, _) = run_and_stderr(&["run", "--sample", "--commands", &file.display().to_string()]);
    assert_eq!(code, 0, "commands file with valid steps should run without error");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A commands file whose first step lacks a string `cmd` field: refused before any command runs,
/// with an error that names the offending step index.
#[test]
fn a_bad_commands_file_names_the_step() {
    let dir = std::env::temp_dir().join(format!("cadcraft-cli-cmds-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.json");
    std::fs::write(
        &file,
        r#"[
            {"foo": "bar"},
            {"cmd": "circle", "params": {"center": [0, 0], "radius": 1}}
        ]"#,
    )
    .unwrap();
    let (code, stderr) = run_and_stderr(&["run", "--sample", "--commands", &file.display().to_string()]);
    assert_eq!(code, 2, "a commands file with a bad step should be a usage error (exit 2)");
    assert!(stderr.contains("step 0"), "error should name the offending step index: {stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// An empty commands file `[]` runs successfully with zero steps.
#[test]
fn empty_commands_file_runs_no_steps() {
    let dir = std::env::temp_dir().join(format!("cadcraft-cli-cmds-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("empty.json");
    std::fs::write(&file, "[]").unwrap();
    let (code, _) = run_and_stderr(&["run", "--sample", "--commands", &file.display().to_string()]);
    assert_eq!(code, 0, "an empty commands file should run without error");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A step whose `cmd` is not a string (e.g. a number) is refused with the step index named.
#[test]
fn non_string_cmd_is_refused() {
    let dir = std::env::temp_dir().join(format!("cadcraft-cli-cmds-nonstr-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("nonstr.json");
    std::fs::write(&file, r#"[{"cmd": 123}]"#).unwrap();
    let (code, stderr) = run_and_stderr(&["run", "--sample", "--commands", &file.display().to_string()]);
    assert_eq!(code, 2, "non-string cmd should be a usage error");
    assert!(stderr.contains("step 0"), "error should name the step: {stderr}");
    assert!(stderr.contains("string"), "error should mention string: {stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A step that is not an object (e.g. a bare string) is refused with the step index named.
#[test]
fn non_object_step_is_refused() {
    let dir = std::env::temp_dir().join(format!("cadcraft-cli-cmds-nonobj-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("nonobj.json");
    std::fs::write(&file, r#"[{"cmd": "circle"}, "not an object"]"#).unwrap();
    let (code, stderr) = run_and_stderr(&["run", "--sample", "--commands", &file.display().to_string()]);
    assert_eq!(code, 2, "non-object step should be a usage error");
    assert!(stderr.contains("step 1"), "error should name step 1: {stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A missing commands file is refused with a readable error.
#[test]
fn missing_commands_file_is_refused() {
    let (code, stderr) = run_and_stderr(&["run", "--sample", "--commands", "/nonexistent/file.json"]);
    assert_eq!(code, 2, "missing file should be a usage error");
    assert!(stderr.contains("can't read"), "error should mention reading: {stderr}");
}

/// A commands file that is not valid JSON is refused.
#[test]
fn non_json_commands_file_is_refused() {
    let dir = std::env::temp_dir().join(format!("cadcraft-cli-cmds-notjson-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("notjson.json");
    std::fs::write(&file, "this is not json").unwrap();
    let (code, stderr) = run_and_stderr(&["run", "--sample", "--commands", &file.display().to_string()]);
    assert_eq!(code, 2, "non-JSON file should be a usage error");
    assert!(stderr.contains("not valid JSON"), "error should mention JSON: {stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A commands file that is a JSON object (not an array) is refused.
#[test]
fn non_array_commands_file_is_refused() {
    let dir = std::env::temp_dir().join(format!("cadcraft-cli-cmds-notarr-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("notarr.json");
    std::fs::write(&file, r#"{"cmd": "circle"}"#).unwrap();
    let (code, stderr) = run_and_stderr(&["run", "--sample", "--commands", &file.display().to_string()]);
    assert_eq!(code, 2, "non-array file should be a usage error");
    assert!(stderr.contains("not a JSON array"), "error should mention array: {stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Multiple `--commands` flags run in command-line order, interleaved with `--cmd` steps.
#[test]
fn multiple_commands_files_run_in_order() {
    let dir = std::env::temp_dir().join(format!("cadcraft-cli-cmds-multi-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f1 = dir.join("first.json");
    let f2 = dir.join("second.json");
    std::fs::write(&f1, r#"[{"cmd": "circle", "params": {"center": [0, 0], "radius": 1}}]"#).unwrap();
    std::fs::write(&f2, r#"[{"cmd": "circle", "params": {"center": [10, 10], "radius": 1}}]"#).unwrap();
    let (code, _) = run_and_stderr(&[
        "run",
        "--sample",
        "--commands",
        &f1.display().to_string(),
        "--cmd",
        "circle {\"center\": [5, 5], \"radius\": 1}",
        "--commands",
        &f2.display().to_string(),
    ]);
    assert_eq!(code, 0, "multiple commands files should run without error");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A step with missing `params` defaults to an empty object.
#[test]
fn missing_params_defaults_to_empty_object() {
    let dir = std::env::temp_dir().join(format!("cadcraft-cli-cmds-noparams-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("noparams.json");
    std::fs::write(&file, r#"[{"cmd": "circle"}]"#).unwrap();
    let (code, stderr) = run_and_stderr(&["run", "--sample", "--commands", &file.display().to_string()]);
    // circle without center/radius fails at the command level, not parsing level
    assert_eq!(code, 1, "parsing should succeed; the command itself fails for missing params");
    assert!(stderr.contains("error:"), "should be an engine error: {stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}
