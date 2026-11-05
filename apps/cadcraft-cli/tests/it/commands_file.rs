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
    let dir = std::env::temp_dir().join(format!("cadcraft-cli-cmds-{}", std::process::id()));
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
    let (code, _) = run_and_stderr(&[
        "run",
        "--sample",
        "--commands",
        &file.display().to_string(),
    ]);
    assert_eq!(code, 0, "commands file with valid steps should run without error");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A commands file whose first step lacks a string `cmd` field: refused before any command runs,
/// with an error that names the offending step index.
#[test]
fn a_bad_commands_file_names_the_step() {
    let dir = std::env::temp_dir().join(format!("cadcraft-cli-cmds-{}", std::process::id()));
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
    let (code, stderr) = run_and_stderr(&[
        "run",
        "--sample",
        "--commands",
        &file.display().to_string(),
    ]);
    assert_eq!(code, 2, "a commands file with a bad step should be a usage error (exit 2)");
    assert!(
        stderr.contains("step 0"),
        "error should name the offending step index: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
