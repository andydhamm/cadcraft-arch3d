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

/// A session with the library's file readers installed and a library that keeps nothing on disk
/// (the built-in and the embedded sample items only), as `scene_cache_equals_full.rs` does.
fn showcase_session() -> Session {
    cadcraft_engine::library::set_library_io(cadcraft_engine::library::LibraryIo {
        read_glb: cadcraft_io::gltf_read::read_glb,
        read_image: cadcraft_io::image_read::read_image,
        encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
        sample: cadcraft_io::library::sample_files,
    });
    let mut s = Session::new();
    s.new_drawing(true);
    s.library = cadcraft_engine::library::Library::with_user_dir(None);
    s
}

/// Fixture or furnishing rows whose `room` equals `room_id`.
fn rows_in_room(rows: &[Value], room_id: &str) -> Vec<Value> {
    rows.iter().filter(|row| row["room"].as_str() == Some(room_id)).cloned().collect()
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

fn room_level_index(levels: &[Value], room: &Value) -> u32 {
    if let Some(idx) = room.get("levelIndex").and_then(|v| v.as_u64()) {
        return idx as u32;
    }
    if let Some(name) = room.get("level").and_then(|v| v.as_str()) {
        return level_index(levels, name);
    }
    panic!("room level");
}

/// The ids of rooms of the given type in an inspect reply.
fn room_ids_of_type(rooms: &[Value], typ: &str) -> Vec<String> {
    rooms.iter().filter(|rm| rm["type"].as_str() == Some(typ)).filter_map(|rm| rm["id"].as_str().map(str::to_string)).collect()
}

#[test]
fn the_shell_replays() {
    let mut s = showcase_session();
    replay(&mut s);
}

#[test]
fn the_shell_is_a_split_entry_bilevel() {
    let mut s = showcase_session();
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
        let lvl = room_level_index(&levels, ro);
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

#[test]
fn the_shell_has_two_bath_rooms() {
    let mut s = showcase_session();
    replay(&mut s);
    let r: Value = s.execute("arch.inspect", &json!({"rooms": true})).expect("arch.inspect failed");
    let rooms: Vec<Value> = r["rooms"].as_array().expect("rooms array").clone();
    assert_eq!(room_ids_of_type(&rooms, "bath").len(), 2, "a bath on Lower and one on Main");
    assert_eq!(room_ids_of_type(&rooms, "kitchen").len(), 1, "one kitchen");
    assert_eq!(room_ids_of_type(&rooms, "living").len(), 1, "one living");
    assert!(!room_ids_of_type(&rooms, "bedroom").is_empty(), "at least one bedroom");
}

#[test]
fn the_shell_library_returns_none_for_unknown_items() {
    let s = showcase_session();

    // Non-existent item ids must return None, not panic or return a default.
    let unknown = ["does-not-exist-12345", "own:pull-bar-999", "furniture-xyz", "", "own:pull-bar-128-extra"];
    for id in unknown {
        assert!(s.library.item(id).is_none(), "library.item(\"{id}\") should be None");
    }
}

#[test]
fn the_shell_session_replays_independently() {
    // Two separate sessions must both be able to replay the showcase script.
    let mut s1 = showcase_session();
    replay(&mut s1);

    let mut s2 = showcase_session();
    replay(&mut s2);

    // Both sessions should have the same number of rooms after replay.
    let r1: Value = s1.execute("arch.inspect", &json!({"rooms": true})).expect("inspect s1");
    let r2: Value = s2.execute("arch.inspect", &json!({"rooms": true})).expect("inspect s2");
    let rooms1 = r1["rooms"].as_array().expect("rooms s1").len();
    let rooms2 = r2["rooms"].as_array().expect("rooms s2").len();
    assert_eq!(rooms1, rooms2, "sessions should have the same room count after replay");
    assert!(rooms1 > 0, "replay should produce rooms");
}

#[test]
fn the_shell_session_survives_hostile_commands() {
    let mut s = showcase_session();
    replay(&mut s);

    // Hostile or malformed commands should return errors, not crash the session.
    let hostile = [
        ("wall", json!({})),
        ("wall", json!({"points": []})),
        ("wall", json!({"points": [[0.0, 0.0]]})),
        ("wall", json!({"points": [[0.0, 0.0], [1.0, 1.0]], "thickness": -1.0})),
        ("wall", json!({"points": [[0.0, 0.0], [1.0, 1.0]], "thickness": 1e12})),
        ("room", json!({})),
        ("room", json!({"type": "nonexistent-type"})),
        ("library.place", json!({})),
        ("library.place", json!({"item": "does-not-exist"})),
        ("cabinet", json!({})),
        ("cabinet", json!({"kind": "base", "wall": "w999", "offset": 0.0})),
        ("arch.inspect", json!({"bogusKey": true})),
    ];

    for (cmd, params) in hostile {
        let result = s.execute(cmd, &params);
        // Either succeeds or returns a clean error — never panics.
        match result {
            Ok(_) => {}
            Err(_) => {}
        }
    }

    // The session should still be usable after all the hostile commands.
    let r: Value = s.execute("arch.inspect", &json!({"rooms": true})).expect("session still works");
    assert!(r["rooms"].as_array().is_some(), "rooms array should still be present");
}

#[test]
fn the_shell_library_lists_sample_items() {
    let mut s = showcase_session();

    // Every sample item id must be present in the library.
    let ids = [
        "own:pull-bar-128",
        "own:pull-knob-32",
        "own:sink-kitchen",
        "own:range-30",
        "own:dishwasher-60",
        "sofa-chaise",
        "bed-double-wood",
        "towel-rack",
        "plant-small",
    ];
    for id in ids {
        assert!(s.library.item(id).is_some(), "sample item `{id}` is not listed in the library");
    }

    // The bar pull is a `pull`, and its mesh can be read.
    assert!(
        s.library.item("own:pull-bar-128").map_or(false, |it| it.symbol_type == "pull"),
        "item `own:pull-bar-128` should have symbol_type `pull`"
    );
    assert!(s.library.symbol("own:pull-bar-128").is_ok(), "the symbol mesh for `own:pull-bar-128` could not be read");
}

#[test]
fn the_kitchen_has_casework_and_appliances() {
    let mut s = showcase_session();
    replay(&mut s);
    let r: Value = s.execute("arch.inspect", &json!({"cabinets": true, "fixtures": true})).expect("arch.inspect failed");
    let cabs: Vec<Value> = r["cabinets"].as_array().expect("cabinets").clone();
    assert!(cabs.iter().filter(|c| c["kind"].as_str() == Some("base")).count() >= 5, "five base cabinets");
    assert!(cabs.iter().filter(|c| c["kind"].as_str() == Some("wall")).count() >= 2, "two wall cabinets");
    assert!(cabs.iter().any(|c| c["pull"].as_str() == Some("own:pull-bar-128")), "a bar pull");
    let kinds: Vec<&str> = r["fixtures"].as_array().expect("fixtures").iter().filter_map(|f| f["kind"].as_str()).collect();
    for want in ["range", "refrigerator", "sink", "dishwasher"] {
        assert!(kinds.contains(&want), "fixture `{want}` missing from {kinds:?}");
    }
}

#[test]
fn the_showcase_is_furnished() {
    let mut s = showcase_session();
    replay(&mut s);

    let r: Value = match s.execute("arch.inspect", &json!({"rooms": true, "furnishings": true, "fixtures": true})) {
        Ok(v) => v,
        Err(err) => panic!("arch.inspect failed: {err}"),
    };
    let rooms: Vec<Value> = match r["rooms"].as_array() {
        Some(rows) => rows.clone(),
        None => panic!("rooms array missing from arch.inspect"),
    };
    let furn: Vec<Value> = match r["furnishings"].as_array() {
        Some(rows) => rows.clone(),
        None => panic!("furnishings array missing from arch.inspect"),
    };
    let fix: Vec<Value> = match r["fixtures"].as_array() {
        Some(rows) => rows.clone(),
        None => panic!("fixtures array missing from arch.inspect"),
    };

    // The kitchen's fixtures cover range, refrigerator, sink and dishwasher.
    let kitchens = room_ids_of_type(&rooms, "kitchen");
    assert!(!kitchens.is_empty(), "no kitchen found in {rooms:?}");
    for kid in &kitchens {
        let rows = rows_in_room(&fix, kid);
        for want in ["range", "refrigerator", "sink", "dishwasher"] {
            assert!(rows.iter().any(|row| row["kind"].as_str() == Some(want)), "kitchen {kid} missing fixture `{want}`; rows={rows:?}");
        }
    }

    // Two bath rooms, each covering toilet, sink|lavatory and tub|bathtub|shower.
    let baths = room_ids_of_type(&rooms, "bath");
    assert_eq!(baths.len(), 2, "expected 2 baths, found {baths:?}");
    for kid in &baths {
        let rows = rows_in_room(&fix, kid);
        let kinds: Vec<&str> = rows.iter().filter_map(|row| row["kind"].as_str()).collect();
        assert!(kinds.iter().any(|k| *k == "toilet"), "bath {kid} missing toilet; kinds={kinds:?}");
        assert!(kinds.iter().any(|k| *k == "sink" || *k == "lavatory"), "bath {kid} missing sink or lavatory; kinds={kinds:?}");
        assert!(kinds.iter().any(|k| *k == "tub" || *k == "bathtub" || *k == "shower"), "bath {kid} missing tub/bathtub/shower; kinds={kinds:?}");
    }

    // Every bedroom has a furnishing whose item contains "bed".
    let bedrooms = room_ids_of_type(&rooms, "bedroom");
    assert!(!bedrooms.is_empty(), "no bedroom found in {rooms:?}");
    for kid in &bedrooms {
        let rows = rows_in_room(&furn, kid);
        assert!(
            rows.iter().any(|row| row["item"].as_str().map_or(false, |i| i.contains("bed"))),
            "bedroom {kid} has no bed furnishing; items={rows:?}"
        );
    }

    // Every room that is not a closet/openBelow has a furnishing or fixture in it.
    let mut failed = Vec::new();
    for rm in &rooms {
        let typ = match rm["type"].as_str() {
            Some(v) => v,
            None => "",
        };
        if typ == "closet" || typ == "openBelow" {
            continue;
        }
        let rid = match rm["id"].as_str() {
            Some(v) => v,
            None => "",
        };
        let has_item = !rows_in_room(&furn, rid).is_empty() || !rows_in_room(&fix, rid).is_empty();
        if !has_item {
            failed.push(rid.to_string());
        }
    }
    assert!(failed.is_empty(), "rooms with no furnishing or fixture: {failed:?}");
}

#[test]
fn no_room_is_left_empty() {
    let mut s = showcase_session();
    replay(&mut s);
    let r: Value = s.execute("arch.inspect", &json!({"rooms": true, "furnishings": true, "fixtures": true})).expect("arch.inspect failed");
    let rooms: Vec<Value> = r["rooms"].as_array().expect("rooms").clone();
    let mut owned: Vec<Value> = r["furnishings"].as_array().expect("furnishings").clone();
    owned.extend(r["fixtures"].as_array().expect("fixtures").clone());
    let empty: Vec<String> = rooms
        .iter()
        .filter(|rm| !matches!(rm["type"].as_str(), Some("closet") | Some("openBelow")))
        .filter(|rm| !owned.iter().any(|o| o["room"].as_str() == rm["id"].as_str()))
        .filter_map(|rm| rm["id"].as_str().map(str::to_string))
        .collect();
    assert!(empty.is_empty(), "rooms with nothing in them: {empty:?}");
}

#[test]
fn the_showcase_is_lit_and_painted() {
    let mut s = showcase_session();
    replay(&mut s);
    let r: Value =
        s.execute("arch.inspect", &json!({"rooms": true, "lights": true, "electrical": true, "surfaces": true})).expect("arch.inspect failed");
    let rooms = r["rooms"].as_array().expect("rooms");
    let lights = r["lights"].as_array().expect("lights");
    let devices = r["electrical"].as_array().expect("electrical");
    let circuits = r["circuits"].as_array().expect("circuits");
    for rm in rooms.iter().filter(|rm| !matches!(rm["type"].as_str(), Some("closet") | Some("openBelow"))) {
        let rid = rm["id"].as_str().expect("room id");
        let room_lights: Vec<&str> = lights.iter().filter(|l| l["room"].as_str() == Some(rid)).filter_map(|l| l["id"].as_str()).collect();
        assert!(!room_lights.is_empty(), "room {rid} has no light");
        assert!(
            devices
                .iter()
                .any(|d| d["room"].as_str() == Some(rid) && d["layer"].as_str() == Some("E-LITE") && d["kind"].as_str() == Some("single-pole")),
            "room {rid} has no switch on E-LITE"
        );
        let wired = circuits
            .iter()
            .any(|c| c["lights"].as_array().is_some_and(|ls| ls.iter().any(|l| l.as_str().is_some_and(|id| room_lights.contains(&id)))));
        assert!(wired, "room {rid} has no light in a circuit");
    }
    for row in r["outletGaps"].as_array().expect("outletGaps") {
        let gap = row["maxGap"].as_f64().expect("maxGap");
        assert!(gap <= 1.8, "room {} outlet gap {gap} exceeds 1.8", row["room"]);
        if let Some(spacing) = row["counter"]["maxSpacing"].as_f64() {
            assert!(spacing <= 1.2, "room {} counter spacing {spacing} exceeds 1.2", row["room"]);
        }
    }
    let defaults: Vec<String> = r["surfaces"]
        .as_array()
        .expect("surfaces")
        .iter()
        .filter(|x| x["material"].as_str().is_some_and(|m| m.starts_with("default:")))
        .filter_map(|x| x["surface"].as_str().map(str::to_string))
        .collect();
    assert!(defaults.is_empty(), "surfaces still on a default material: {defaults:?}");
}

/// The level index of a level with the given `kind` in an inspect reply.
fn level_index_by_kind(levels: &[Value], kind: &str) -> Option<usize> {
    levels.iter().position(|l| l["kind"].as_str() == Some(kind))
}

/// The elevation of a level at the given index, or `None`.
fn level_elevation(levels: &[Value], idx: usize) -> Option<f64> {
    levels.get(idx).and_then(|l| l["elevation"].as_f64())
}

#[test]
fn the_showcase_sits_on_a_site() {
    let mut s = showcase_session();
    replay(&mut s);

    let r: Value = s.execute("arch.inspect", &json!({"levels": true, "site": true})).unwrap_or_else(|err| panic!("arch.inspect failed: {err}"));

    let levels: Vec<Value> = r["levels"].as_array().expect("levels array").clone();
    let site: &Value = &r["site"];

    let floor_idxs: Vec<usize> = levels.iter().enumerate().filter(|(_, l)| l["kind"].as_str() == Some("floor")).map(|(i, _)| i).collect();
    assert_eq!(floor_idxs.len(), 2, "expected two floor levels, found {floor_idxs:?}");
    let lower_idx = floor_idxs.iter().min().copied().expect("lower floor index");
    let main_idx = floor_idxs.iter().max().copied().expect("main floor index");
    let lower_elev = level_elevation(&levels, lower_idx).expect("lower elevation");
    let main_elev = level_elevation(&levels, main_idx).expect("main elevation");

    let landing_idx = level_index_by_kind(&levels, "landing").expect("a landing level");
    let landing_elev = level_elevation(&levels, landing_idx).expect("landing elevation");
    assert!(
        landing_elev > lower_elev && landing_elev < main_elev,
        "landing elevation {} not strictly between {} and {}",
        landing_elev,
        lower_elev,
        main_elev
    );

    assert_eq!(levels[lower_idx]["gradeClass"].as_str().expect("lower gradeClass"), "basement");
    assert_eq!(levels[main_idx]["gradeClass"].as_str().expect("main gradeClass"), "aboveGrade");
    assert!(r["aboveGradeArea"].as_f64().expect("aboveGradeArea") >= 371.6, "aboveGradeArea too small: {:#?} >= 371.6", r["aboveGradeArea"]);

    let paths = site["paths"].as_array().expect("site.paths array");
    let kinds: Vec<&str> = paths.iter().filter_map(|p| p["kind"].as_str()).collect();
    assert!(kinds.contains(&"driveway"), "no driveway path: {kinds:?}");
    assert!(kinds.contains(&"sidewalk"), "no sidewalk path: {kinds:?}");
    assert!(kinds.contains(&"bed"), "no bed path: {kinds:?}");

    let plants = site["plants"].as_array().expect("site.plants array");
    let plant_count = plants.len();
    assert!(plant_count >= 4, "expected at least 4 plants, found {plant_count}");
    let forms: Vec<&str> = plants.iter().filter_map(|p| p["form"].as_str()).collect();
    assert!(forms.contains(&"tree"), "no tree plants: {forms:?}");
    assert!(forms.contains(&"shrub"), "no shrub plants: {forms:?}");

    if let Some(cam) = r["cameras"].as_array() {
        let saved = cam.iter().filter_map(|c| c["name"].as_str()).collect::<Vec<&str>>();
        assert!(saved.len() >= 4, "expected at least 4 saved cameras, found {saved:?}");
    }
}
