//! Split-entry, basement-plus-main showcase house shell.
//!
//! Replays `docs/arch3d/showcase/bilevel.json` — the shell (levels, walls,
//! openings, stairs and detected/typed rooms) — and asserts the result is a
//! genuine split-entry bi-level: a landing level strictly between a basement
//! floor and an above-grade floor, with the required stairs and room area.

use std::path::PathBuf;

use cadcraft_engine::Session;
use serde_json::{Value, json};

/// Tolerance for comparing computed elevations against inspected extents.
const TOL: f64 = 0.01;

/// The showcase shell file, relative to the workspace root.
fn showcase_path() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../docs/arch3d/showcase/bilevel.json")
}

/// Replay the showcase shell script into a fresh metric session.
fn replay(session: &mut Session) -> &mut Session {
    let script = std::fs::read_to_string(showcase_path()).expect("could not read showcase shell script");
    let steps: Vec<Value> = serde_json::from_str(&script).expect("showcase shell script is not valid JSON");
    for (i, step) in steps.iter().enumerate() {
        let cmd = step["cmd"].as_str().expect("missing cmd").to_string();
        let params = step["params"].clone();
        session.execute(cmd.as_str(), &params).unwrap_or_else(|err| panic!("showcase step {} ({}={}) failed: {err}", i, &cmd, params));
    }
    session
}

/// The level index of the named level in an inspect reply.
fn level_index(levels: &[Value], name: &str) -> u32 {
    for v in levels {
        if let Some(n) = v.get("name").and_then(|x| x.as_str()) {
            if n == name {
                return v["index"].as_u64().expect("level index missing") as u32;
            }
        }
    }
    panic!("level named `{name}` not found");
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

    let r: Value = s.execute("arch.inspect", &json!({"levels":true,"rooms":true,"walls":true,"stairs":true})).expect("arch.inspect failed");

    let levels: Vec<Value> = r["levels"].as_array().expect("levels array").clone();
    let stairs: Vec<Value> = r["stairs"].as_array().expect("stairs array").clone();
    let rooms: Vec<Value> = r["rooms"].as_array().expect("rooms array").clone();
    let walls: Vec<Value> = r["walls"].as_array().expect("walls array").clone();

    let floor_levels: Vec<&Value> = levels.iter().filter(|l| l["kind"].as_str().map_or(false, |k| k == "floor")).collect();
    let landing_levels: Vec<&Value> = levels.iter().filter(|l| l["kind"].as_str().map_or(false, |k| k == "landing")).collect();

    assert_eq!(floor_levels.len(), 2, "expected two floor levels, found {}", floor_levels.len());
    assert_eq!(landing_levels.len(), 1, "expected one landing level, found {}", landing_levels.len());

    let floors: Vec<f64> = floor_levels.iter().map(|l| l["elevation"].as_f64().expect("level elevation missing")).collect();
    let lower_elev = floors.iter().cloned().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
    let main_elev = floors.iter().cloned().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
    let landing = &landing_levels[0];
    let landing_elev = landing["elevation"].as_f64().expect("landing elevation missing");

    assert!(
        landing_elev > lower_elev && landing_elev < main_elev,
        "landing elevation {} not strictly between {} and {}",
        landing_elev,
        lower_elev,
        main_elev
    );

    let lower = level_index(&levels, "Lower");
    let main = level_index(&levels, "Main");
    let entry = level_index(&levels, "Entry");

    // Grade classes: the lowest floor is a basement, the highest is above grade.
    let lower_val = &levels[lower as usize];
    let main_val = &levels[main as usize];
    assert_eq!(lower_val["gradeClass"].as_str().expect("lower gradeClass"), "basement");
    assert_eq!(main_val["gradeClass"].as_str().expect("main gradeClass"), "aboveGrade");
    assert!(r["aboveGradeArea"].as_f64().expect("aboveGradeArea") >= 371.6, "aboveGradeArea too small: {:?} >= 371.6", r["aboveGradeArea"]);

    // Stair from the landing to Main, and one from Lower to the landing.
    let mut found_landing_to_main = false;
    let mut found_lower_to_landing = false;
    for st in &stairs {
        let from = st["fromLevel"].as_u64().expect("stair fromLevel") as u32;
        let to = st["toLevel"].as_u64().expect("stair toLevel") as u32;
        if from == entry && to == main {
            found_landing_to_main = true;
        }
        if from == lower && to == entry {
            found_lower_to_landing = true;
        }
    }
    assert!(found_landing_to_main, "no stair from the landing to Main");
    assert!(found_lower_to_landing, "no stair from Lower to the landing");

    // Rooms detected on all three levels.
    let mut lower_rooms = 0;
    let mut main_rooms = 0;
    let mut entry_rooms = 0;
    for ro in &rooms {
        let lvl = ro["level"].as_u64().expect("room level") as u32;
        match lvl {
            x if x == lower => lower_rooms += 1,
            x if x == main => main_rooms += 1,
            x if x == entry => entry_rooms += 1,
            _ => {}
        }
    }
    assert!(lower_rooms > 0 && main_rooms > 0 && entry_rooms > 0, "rooms: lower={lower_rooms}, main={main_rooms}, entry={entry_rooms}");

    // The spanning wall: bottom == Lower elevation, top == Main elevation + Main ceiling.
    let main_ceiling = main_val["ceilingHeight"].as_f64().expect("main ceiling height");
    let expected_top = main_elev + main_ceiling;
    let spanning = walls.iter().find(|w| !w["topLevel"].is_null()).expect("no spanning wall");
    let bottom = spanning["bottom"].as_f64().expect("spanning wall bottom");
    let top = spanning["top"].as_f64().expect("spanning wall top");
    assert!((bottom - lower_elev).abs() < TOL, "spanning wall bottom {} != Lower elevation {}", bottom, lower_elev);
    assert!((top - expected_top).abs() < TOL, "spanning wall top {} != Main elevation + ceiling {}", top, expected_top);
}
