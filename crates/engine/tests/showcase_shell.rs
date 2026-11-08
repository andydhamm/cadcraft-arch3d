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
