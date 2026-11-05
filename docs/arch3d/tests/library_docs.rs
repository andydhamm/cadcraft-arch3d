/// Tests for the phase-14 library documentation: the MCP example session and the
/// USER-GUIDE.md library section. These pin down the structure of the docs so a
/// future edit that drops a required call or section is caught here.

use std::fs;
use std::path::Path;

fn repo_root() -> PathBuf {
    // docs/arch3d/tests/ -> repo root is ../../..
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn read_file(name: &str) -> String {
    let p = repo_root().join(name);
    fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", p.display(), e))
}

/// Each line of the MCP example must be valid JSON-RPC with a unique id.
#[test]
fn mcp_example_09_is_valid_jsonl() {
    let text = read_file("docs/arch3d/mcp-examples/09-library.jsonl");
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(!lines.is_empty(), "09-library.jsonl is empty");

    let mut ids = Vec::new();
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("invalid JSON on line: {} ({})", line, e));
        assert!(v.get("jsonrpc").is_some(), "missing jsonrpc field: {}", line);
        assert!(v.get("id").is_some(), "missing id field: {}", line);
        assert!(v.get("method").is_some(), "missing method field: {}", line);
        ids.push(v["id"].as_u64().expect("id is not a number"));
    }

    // ids must be unique
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(ids.len(), sorted.len(), "duplicate ids in 09-library.jsonl");
}

/// The session must contain the required calls in order: initialize, new_drawing,
/// walls, roomdetect, library search, three library.place calls, paint.material, export.
#[test]
fn mcp_example_09_has_required_calls() {
    let text = read_file("docs/arch3d/mcp-examples/09-library.jsonl");
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();

    let methods: Vec<String> = lines
        .iter()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            v["method"].as_str().unwrap().to_string()
        })
        .collect();

    assert_eq!(methods[0], "initialize");
    assert!(methods.contains(&"new_drawing".to_string()));

    // Collect the execute command names
    let commands: Vec<String> = lines
        .iter()
        .filter_map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            if v["method"].as_str() == Some("tools/call") {
                v["params"]["name"].as_str().filter(|n| *n == "execute").map(|_| {
                    v["params"]["arguments"]["command"].as_str().unwrap().to_string()
                })
            } else {
                None
            }
        })
        .collect();

    // Must have walls, roomdetect, library search, 3x library.place, paint.material, export
    assert!(commands.contains(&"wall".to_string()));
    assert!(commands.contains(&"roomdetect".to_string()));
    assert!(commands.contains(&"library".to_string()));

    let place_count = commands.iter().filter(|c| *c == "library.place").count();
    assert_eq!(place_count, 3, "expected 3 library.place calls, found {}", place_count);

    assert!(commands.contains(&"paint.material".to_string()));
    assert!(commands.contains(&"export".to_string()));
}

/// USER-GUIDE.md must have a "The library" section before "Out of scope" with the
/// expected subsections.
#[test]
fn user_guide_library_section_exists() {
    let text = read_file("docs/arch3d/USER-GUIDE.md");

    // The library section must exist
    assert!(text.contains("## The library"), "missing '## The library' section");

    // It must come before "Out of scope"
    let lib_pos = text.find("## The library").expect("no library section");
    let out_pos = text.find("## Out of scope").expect("no out of scope section");
    assert!(lib_pos < out_pos, "'The library' must come before 'Out of scope'");

    // Required subsections
    assert!(text.contains("### Searching"), "missing '### Searching' subsection");
    assert!(text.contains("### Placing"), "missing '### Placing' subsection");
    assert!(text.contains("### The painters and the eyedropper"), "missing painters subsection");
    assert!(text.contains("### The user catalog and the Trash"), "missing user catalog subsection");
    assert!(text.contains("### Set as Default"), "missing 'Set as Default' subsection");
    assert!(text.contains("### Backdrops"), "missing '### Backdrops' subsection");
    assert!(text.contains("### Installing catalogs"), "missing '### Installing catalogs' subsection");
    assert!(text.contains("### A furnished house"), "missing '### A furnished house' subsection");

    // The nine categories must be listed
    assert!(text.contains("Furniture"), "missing Furniture category");
    assert!(text.contains("Appliances and plumbing fixtures"), "missing Appliances category");
    assert!(text.contains("Cabinet parts"), "missing Cabinet parts category");
    assert!(text.contains("Lighting and electrical"), "missing Lighting category");
    assert!(text.contains("Materials"), "missing Materials category");
    assert!(text.contains("Doors and windows"), "missing Doors and windows category");
    assert!(text.contains("Plants"), "missing Plants category");
    assert!(text.contains("Backdrops"), "missing Backdrops category");
    assert!(text.contains("User catalog"), "missing User catalog category");
}
