use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).map(Path::to_path_buf).unwrap_or_else(|| Path::new(".").to_path_buf())
}

fn examples_dir() -> PathBuf {
    repo_root().join("docs/arch3d/mcp-examples")
}

fn list_example_files() -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(examples_dir())
        .expect("read examples dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|s| s == "jsonl").unwrap_or(false))
        .collect();
    files.sort();
    files
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
    let temp_dir = tempfile::tempdir().expect("tempdir");

    // Link examples dir
    let link_path = temp_dir.path().join("examples");
    let target = root.join("examples");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &link_path).expect("symlink examples");
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&target, &link_path).expect("symlink examples");

    for file in list_example_files() {
        run_mcp_session(&file, temp_dir.path()).unwrap_or_else(|e| panic!("{}: {}", file.display(), e));
    }
}

#[test]
fn mcp_examples_have_minimum_count() {
    let files = list_example_files();
    assert!(files.len() >= 8, "expected at least 8 example files, found {}", files.len());
}

#[test]
fn mcp_examples_structure_is_valid() {
    for file in list_example_files() {
        let content = std::fs::read_to_string(&file).expect("read file");
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        assert!(lines.len() >= 2, "{}: expected at least 2 lines", file.display());

        let mut ids = std::collections::HashSet::new();
        for (i, line) in lines.iter().enumerate() {
            let val: serde_json::Value =
                serde_json::from_str(line).unwrap_or_else(|e| panic!("{} line {}: invalid JSON: {}", file.display(), i + 1, e));

            // Every request must have an id
            let id = val.get("id").unwrap_or_else(|| panic!("{} line {}: missing id", file.display(), i + 1));
            let id_str = id.to_string();
            assert!(ids.insert(id_str.clone()), "{} line {}: duplicate id {}", file.display(), i + 1, id_str);

            // Every request must have a method
            let method = val.get("method").unwrap_or_else(|| panic!("{} line {}: missing method", file.display(), i + 1));
            assert!(method.is_string(), "{} line {}: method must be a string", file.display(), i + 1);

            // First line must be initialize with correct protocol version
            if i == 0 {
                assert_eq!(method.as_str(), Some("initialize"), "{}: first line must be initialize", file.display());
                let pv = val.get("params").and_then(|p| p.get("protocolVersion"));
                assert_eq!(pv.and_then(|v| v.as_str()), Some("2025-06-18"), "{}: wrong protocol version", file.display());
            }

            // Second line must be new_drawing with metric:true
            if i == 1 {
                assert_eq!(method.as_str(), Some("tools/call"), "{}: second line must be tools/call", file.display());
                let name = val.get("params").and_then(|p| p.get("name"));
                assert_eq!(name.and_then(|v| v.as_str()), Some("new_drawing"), "{}: second line must call new_drawing", file.display());
                let metric = val.get("params").and_then(|p| p.get("arguments")).and_then(|a| a.get("metric"));
                assert_eq!(metric.and_then(|v| v.as_bool()), Some(true), "{}: new_drawing must be metric", file.display());
            }
        }
    }
}

#[test]
fn mcp_examples_readme_lists_all_files() {
    let readme_path = examples_dir().join("README.md");
    let readme = std::fs::read_to_string(&readme_path).expect("read README.md");
    for file in list_example_files() {
        let name = file.file_name().unwrap().to_string_lossy();
        assert!(readme.contains(&*name), "README.md does not mention {}", name);
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
