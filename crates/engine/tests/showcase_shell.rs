//! Split-entry, basement-plus-main showcase house shell.
//!
//! Replays `docs/arch3d/showcase/bilevel.json` — the shell (levels, walls,
//! openings, stairs and detected/typed rooms) — and asserts the result is a
//! genuine split-entry bi-level: a landing level strictly between a basement
//! floor and an above-grade floor, with the required stairs and room area.

use std::path::PathBuf;

use cadcraft_engine::Session;
use serde_json::{json, Value};

/// The showcase shell file, relative to the workspace root.
fn showcase_path() -> PathBuf {
    PathBuf::from_env_var_or(
        "CARGO_MANIFEST_DIR",
        "/Users/you/dev/arch3d/crates/engine",
    )
        .join("../../docs/arch3d/showcase/bilevel.json")
}
<<<<<<< REPLACE
//! Split-entry, basement-plus-main showcase house shell.
//!
//! Replays `docs/arch3d/showcase/bilevel.json` — the shell (levels, walls,
//! openings, stairs and detected/typed rooms) — and asserts the result is a
//! genuine split-entry bi-level: a landing level strictly between a basement
//! floor and an above-grade floor, with the required stairs and room area.

use std::path::PathBuf;

use cadcraft_engine::Session;
use serde_json::{json, Value};

/// The showcase shell file, relative to the workspace root.
fn showcase_path() -> PathBuf {
    PathBuf::from_env_var_or(
        "CARGO_MANIFEST_DIR",
        "/Users/you/dev/arch3d/crates/engine",
    )
        .join("../../docs/arch3d/showcase/bilevel.json")
}

/// Replay the showcase shell script into a fresh metric session.
fn replay(session: &mut Session) -> &mut Session {
    let script = std::fs::read_to_string(showcase_path())
        .expect("could not read showcase shell script");
    let steps: Vec<Value> =
        serde_json::from_str(&script).expect("showcase shell script is not valid JSON");
    for (i, step) in steps.iter().enumerate() {
        let cmd = step["cmd"].as_str().expect("missing cmd");
        let params = step["params"].as_object().expect("missing params").as_json();
        session.execute(cmd, params).expect(format!(
            "showcase step {} ({}={:03}) failed",
            i, cmd, params
        ));
    }
    session
}

#[test]
fn the_shell_replays() {
    let mut s = Session::new();
    s.new_drawing(true);
    replay(&mut s);
}

#[test]
fn the_shell_is_a_split_entry_bilevel() {
    let mut s = Session::new();
    s.new_drawing(true);
    replay(&mut s);

    let r: Value =
        session.execute(
            "arch.inspect",
            json!({"levels":true,"rooms":true,"walls":true,"stairs":true}),
        ).unwrap();
    ...
}
