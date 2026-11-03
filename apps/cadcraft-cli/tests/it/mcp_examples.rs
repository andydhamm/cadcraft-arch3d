use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).map(Path::to_path_buf).unwrap_or_else(|| Path::new(".").to_path_buf())
}

fn run_mcp_session(file: &Path, temp_dir: &Path) -> Result<(), String> {
    let content = std::fs::read_to_string(file).map_err(|e| format!("read {}: {}", file.display(), e))?;

    let mut child = Command::new(env!("CARGO_BIN_EXE_cadcraft-cli"))
        .arg("mcp")
        .current_dir(temp_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn mcp: {}", e))?;

    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    stdin.write_all(content.as_bytes()).map_err(|e| format!("write stdin: {}", e))?;
    drop(stdin);

    let output = child.wait_with_output().map_err(|e| format!("wait mcp: {}", e))?;

    if !output.status.success() {
        return Err(format!("mcp exited with {}: {}", output.status, String::from_utf8_lossy(&output.stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for (i, line) in stdout.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let val: serde_json::Value = serde_json::from_str(line).map_err(|e| format!("line {}: invalid JSON: {}", i + 1, e))?;

        if val.get("error").is_some() {
            return Err(format!("line {}: error reply: {}", i + 1, line));
        }

        if let Some(result) = val.get("result") {
            if let Some(is_err) = result.get("isError") {
                if is_err.as_bool() == Some(true) {
                    return Err(format!("line {}: isError true: {}", i + 1, line));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn mcp_examples_replay_without_errors() {
    let root = repo_root();
    let examples_dir = root.join("docs/arch3d/mcp-examples");
    let temp_dir = tempfile::tempdir().expect("tempdir");

    // Link examples dir
    let link_path = temp_dir.path().join("examples");
    let target = root.join("examples");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &link_path).expect("symlink examples");
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&target, &link_path).expect("symlink examples");

    let mut files: Vec<_> = std::fs::read_dir(&examples_dir)
        .expect("read examples dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|s| s == "jsonl").unwrap_or(false))
        .collect();
    files.sort();

    for file in files {
        run_mcp_session(&file, temp_dir.path()).unwrap_or_else(|e| panic!("{}: {}", file.display(), e));
    }
}

#[test]
fn mcp_examples_cover_materials_casework_and_exports() {
    let root = repo_root();
    let examples_dir = root.join("docs/arch3d/mcp-examples");

    let check_file = |name: &str, keywords: &[&str]| {
        let path = examples_dir.join(name);
        let content = std::fs::read_to_string(&path).expect("read file");
        for kw in keywords {
            assert!(content.contains(kw), "{} missing {}", name, kw);
        }
    };

    check_file("03-materials.jsonl", &["material", "finish"]);
    check_file("04-kitchen-casework.jsonl", &["cabinet", "fixture"]);
    check_file("06-exports.jsonl", &["export", "ifc", "obj", "glb"]);
}
