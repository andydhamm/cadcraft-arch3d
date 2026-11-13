use serde_json::{Value, json};
use std::fs;

use cadcraft_engine::Session;
use cadcraft_engine::cmd::file::{IoHooks, set_io};

fn load_demo() -> Vec<Value> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/arch3d/showcase/phase-18-demo.json");
    let text = fs::read_to_string(path).expect("phase-18-demo.json");
    let value: Value = serde_json::from_str(&text).expect("phase-18 demo JSON must parse");
    value.as_array().expect("phase-18 demo JSON should be an array").to_vec()
}

fn install_io() {
    set_io(IoHooks { read: |_, _| Err("no file reads in this test".to_string()), write: |_, _| Ok(b"stand-in export".to_vec()), plot: None });
    cadcraft_engine::library::set_library_io(cadcraft_engine::library::LibraryIo {
        read_glb: cadcraft_io::gltf_read::read_glb,
        read_image: cadcraft_io::image_read::read_image,
        encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
        sample: cadcraft_io::library::sample_files,
    });
}

fn replay(script: &[Value]) -> Session {
    let mut session = Session::new();
    session.new_drawing(true);
    for (i, step) in script.iter().enumerate() {
        let cmd = step.get("cmd").and_then(Value::as_str).expect("step missing cmd");
        let params = step.get("params").cloned().unwrap_or(Value::Object(Default::default()));
        session.execute(cmd, &params).unwrap_or_else(|err| panic!("step {i} `{cmd}` failed: {err}"));
    }
    session
}

fn as_array(value: &Value, key: &str) -> Vec<Value> {
    value.get(key).and_then(Value::as_array).map(|a| a.to_vec()).unwrap_or_default()
}

fn polygon_area(poly: &[Value]) -> f64 {
    if poly.is_empty() {
        return 0.0;
    }
    let mut area = 0.0f64;
    for i in 0..poly.len() {
        let p1 = &poly[i];
        let p2 = &poly[(i + 1) % poly.len()];
        let a = p1.as_array().and_then(|a| a.get(0)).and_then(Value::as_f64).unwrap_or(0.0);
        let b = p1.as_array().and_then(|a| a.get(1)).and_then(Value::as_f64).unwrap_or(0.0);
        let c = p2.as_array().and_then(|a| a.get(0)).and_then(Value::as_f64).unwrap_or(0.0);
        let d = p2.as_array().and_then(|a| a.get(1)).and_then(Value::as_f64).unwrap_or(0.0);
        area += a * d - c * b;
    }
    area.abs() / 2.0
}

#[test]
fn debug_phase18_layout() {
    install_io();
    let script = load_demo();
    let mut session = replay(&script);
    let inspect = session.execute("arch.inspect", &json!({"walls": true, "cabinets": true, "rooms": true, "openings": true})).unwrap();
    for wall in inspect["walls"].as_array().unwrap() {
        println!(
            "wall {} start {:?} end {:?} glass={} split={:?}",
            wall["id"],
            wall["start"],
            wall["end"],
            wall.get("glass").unwrap_or(&Value::Null),
            wall.get("split")
        );
    }
    for cab in inspect["cabinets"].as_array().unwrap() {
        println!(
            "cab {} kind={} wall={:?} at={:?} id={} corner={:?}",
            cab["id"],
            cab["kind"],
            cab.get("wall"),
            cab.get("at"),
            cab["id"],
            cab.get("corner")
        );
    }
}

fn first_move_step(script: &[Value]) -> Option<(usize, f64)> {
    for (i, step) in script.iter().enumerate() {
        if step.get("cmd") == Some(&Value::String("countertop.edit".into())) {
            let params = step.get("params")?;
            if let Some(move_val) = params.get("move").and_then(Value::as_f64) {
                if move_val >= 0.3 {
                    return Some((i, move_val));
                }
            }
        }
    }
    None
}

fn probe_hit_part(session: &mut Session, points: &[Vec<f64>]) -> Vec<Value> {
    let req = json!({ "probe": points });
    let out = session.execute("build3d", &req).expect("build3d probe should work");
    out.get("probe").and_then(Value::as_array).cloned().unwrap_or_default()
}

#[test]
fn phase18_demo_replays_without_a_refusal() {
    install_io();
    let script = load_demo();
    let mut session = replay(&script);
    let reply = session
        .execute(
            "arch.inspect",
            &json!({
                "walls": true,
                "rooms": true,
                "openings": true,
                "cabinets": true,
                "surfaces": true,
                "countertops": true,
                "backsplashes": true,
                "moldings": true,
                "moldingLines": true,
            }),
        )
        .expect("inspect should succeed after replay");
    assert!(reply.get("walls").and_then(Value::as_array).is_some(), "inspect should include walls");
    assert!(reply.get("countertops").and_then(Value::as_array).is_some(), "inspect should include countertops");
}

#[test]
fn phase18_demo_kitchen_has_every_feature() {
    install_io();
    let script = load_demo();
    let mut session = replay(&script);
    let inspect = session
        .execute(
            "arch.inspect",
            &json!({
                "walls": true,
                "rooms": true,
                "openings": true,
                "cabinets": true,
                "surfaces": true,
                "countertops": true,
                "backsplashes": true,
                "moldings": true,
                "moldingLines": true,
            }),
        )
        .expect("inspect should succeed");

    let countertops = as_array(&inspect, "countertops");
    let custom = countertops.iter().filter(|row| row.get("auto") == Some(&Value::Bool(false))).collect::<Vec<_>>();
    assert!(!custom.is_empty(), "demo should include at least one custom countertop");
    let custom_row = custom[0];
    let edges = custom_row.get("edges").and_then(Value::as_array).unwrap();
    let waterfall_edge = edges.iter().find(|edge| edge.get("waterfall") == Some(&Value::Bool(true))).expect("custom countertop has a waterfall edge");
    let edge_index = waterfall_edge.get("index").and_then(Value::as_u64).unwrap() as usize;
    let polygon = custom_row.get("polygon").and_then(Value::as_array).unwrap();
    let area = polygon_area(polygon);
    let edge_len = waterfall_edge.get("length").and_then(Value::as_f64).unwrap_or(0.0);
    assert!(area - 0.3 * edge_len > 0.5, "countertop overhang area should exceed 0.5 m2");

    let move_step = first_move_step(&script).expect("demo should include a countertop edit with move >= 0.3");
    assert!(
        edges.iter().any(|edge| edge.get("index") == Some(&Value::Number((edge_index as u64).into()))),
        "the moved edge index must exist in the final custom countertop"
    );
    let first_move_edge = script
        .iter()
        .find_map(|step| {
            if step.get("cmd") == Some(&Value::String("countertop.edit".into())) {
                let params = step.get("params")?;
                let move_val = params.get("move")?.as_f64()?;
                if move_val >= 0.3 {
                    let edge = params.get("edge")?.as_u64()?;
                    return Some(edge as usize);
                }
            }
            None
        })
        .unwrap();
    assert!(
        edges.iter().any(|edge| edge.get("index") == Some(&Value::Number((first_move_edge as u64).into()))),
        "the first moved edge must exist in the final countertop"
    );

    let backsplashes = as_array(&inspect, "backsplashes");
    assert!(
        backsplashes.iter().any(|row| row.get("segments").and_then(Value::as_array).map_or(false, |segments| !segments.is_empty())),
        "demo should have a backsplash with at least one segment"
    );

    let cabinets = as_array(&inspect, "cabinets");
    let corner_cabinets = cabinets.iter().filter(|cab| cab.get("corner").and_then(Value::as_object).is_some()).collect::<Vec<_>>();
    println!("inspect cabinets: {cabinets:#?}");
    println!("corner cabinets: {corner_cabinets:#?}");
    assert_eq!(corner_cabinets.len(), 1, "demo should include exactly one corner cabinet");
    let corner_id = corner_cabinets[0].get("id").and_then(Value::as_str).unwrap().to_string();
    let partner = corner_cabinets[0].get("corner").and_then(|c| c.get("partner")).and_then(Value::as_str).unwrap().to_string();
    let auto_rows = countertops.iter().filter(|row| row.get("auto") == Some(&Value::Bool(true))).collect::<Vec<_>>();
    assert!(!auto_rows.is_empty(), "demo should include an automatic countertop");
    assert!(
        auto_rows.iter().any(|row| {
            let ids =
                row.get("cabinets").and_then(Value::as_array).map(|arr| arr.iter().filter_map(Value::as_str).collect::<Vec<_>>()).unwrap_or_default();
            ids.iter().any(|id| *id == corner_id) && ids.iter().any(|id| *id == partner)
        }),
        "automatic countertop should include the corner cabinet and its partner"
    );

    assert!(
        cabinets.iter().filter(|cab| cab.get("filler") == Some(&Value::Bool(true))).count() >= 2,
        "demo should include at least two filler cabinets"
    );
    assert!(
        cabinets.iter().any(|cab| cab.get("kind") == Some(&Value::String("wall".into())) && cab.get("crown").and_then(Value::as_object).is_some()),
        "demo should include a wall cabinet with a crown"
    );
    assert!(
        cabinets.iter().any(|cab| cab.get("fronts") == Some(&Value::String("open".into()))
            && cab.get("shelves").and_then(Value::as_array).is_some_and(|shelves| !shelves.is_empty())),
        "demo should include an open cabinet with a shelf"
    );
    assert!(
        cabinets.iter().any(|cab| cab.get("kind") == Some(&Value::String("tall".into()))
            && cab
                .get("face")
                .and_then(|face| face.get("rows"))
                .and_then(Value::as_array)
                .map_or(false, |rows| rows.iter().any(|row| row.get("kind") == Some(&Value::String("appliance".into()))))),
        "demo should include a tall cabinet with an appliance face row"
    );
    assert!(move_step.1 >= 0.3, "first move value should be at least 0.3");
}

#[test]
fn phase18_demo_baths_have_showers_a_pony_wall_and_a_niche() {
    install_io();
    let script = load_demo();
    let mut session = replay(&script);
    let inspect = session
        .execute(
            "arch.inspect",
            &json!({
                "walls": true,
                "rooms": true,
                "openings": true,
                "cabinets": true,
                "surfaces": true,
                "countertops": true,
                "backsplashes": true,
                "moldings": true,
                "moldingLines": true,
            }),
        )
        .expect("inspect should succeed");

    let openings = as_array(&inspect, "openings");
    let showers = openings.iter().filter(|row| row.get("style") == Some(&Value::String("shower".into()))).collect::<Vec<_>>();
    assert!(showers.len() >= 2, "demo should include at least two shower openings");

    let walls = as_array(&inspect, "walls");
    let pony = walls
        .iter()
        .find(|wall| wall.get("split").and_then(|split| split.get("upper")).map_or(false, |upper| upper == &Value::String("Glass".into())))
        .expect("demo should include a pony wall with glass upper");
    assert!(pony.get("split").is_some(), "pony wall split should exist");

    let niches = openings.iter().filter(|row| row.get("kind") == Some(&Value::String("niche".into()))).collect::<Vec<_>>();
    assert!(!niches.is_empty(), "demo should include a niche");
}

#[test]
fn phase18_demo_finished_rooms_have_base_and_crown() {
    install_io();
    let script = load_demo();
    let mut session = replay(&script);
    let inspect = session
        .execute(
            "arch.inspect",
            &json!({
                "walls": true,
                "rooms": true,
                "openings": true,
                "cabinets": true,
                "surfaces": true,
                "countertops": true,
                "backsplashes": true,
                "moldings": true,
                "moldingLines": true,
            }),
        )
        .expect("inspect should succeed");

    let rooms = as_array(&inspect, "rooms");
    let finished_types = ["living", "bedroom", "kitchen", "bath", "closet", "utility", "dining", "family", "entry", "laundry"];
    let finished =
        rooms.iter().filter(|room| finished_types.iter().any(|t| room.get("type") == Some(&Value::String((*t).to_string())))).collect::<Vec<_>>();
    assert!(finished.len() >= 4, "demo should include at least four finished rooms");

    for room in finished {
        let room_id = room.get("id").and_then(Value::as_str).unwrap();
        let moldings = as_array(&inspect, "moldings");
        let room_moldings = moldings.iter().filter(|row| row.get("room") == Some(&Value::String(room_id.to_string()))).collect::<Vec<_>>();
        assert!(
            room_moldings.iter().any(|row| row.get("kind") == Some(&Value::String("base".into()))
                && row.get("length").and_then(Value::as_f64).map_or(false, |len| len > 0.0)),
            "base molding missing for room {room_id}"
        );
        assert!(
            room_moldings.iter().any(|row| row.get("kind") == Some(&Value::String("crown".into()))
                && row.get("length").and_then(Value::as_f64).map_or(false, |len| len > 0.0)),
            "crown molding missing for room {room_id}"
        );
    }
}

#[test]
fn phase18_demo_no_molding_on_glass() {
    install_io();
    let script = load_demo();
    let mut session = replay(&script);
    let walls = as_array(&session.execute("arch.inspect", &json!({ "walls": true })).expect("inspect walls"), "walls");
    let mut points = Vec::new();
    for wall in walls {
        if wall.get("glass") == Some(&Value::Bool(true)) {
            let start = wall.get("start").and_then(Value::as_array).unwrap();
            let end = wall.get("end").and_then(Value::as_array).unwrap();
            let x0 = start[0].as_f64().unwrap();
            let y0 = start[1].as_f64().unwrap();
            let x1 = end[0].as_f64().unwrap();
            let y1 = end[1].as_f64().unwrap();
            let dx = x1 - x0;
            let dy = y1 - y0;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.3 {
                let nx = -(dy / len);
                let ny = dx / len;
                let off = wall.get("thickness").and_then(Value::as_f64).unwrap_or(0.01) / 2.0 + 0.01;
                let mx = (x0 + x1) / 2.0;
                let my = (y0 + y1) / 2.0;
                points.push(vec![mx + nx * off, my + ny * off, 0.07]);
                points.push(vec![mx + nx * off, my + ny * off, 2.35]);
            }
        }
    }
    let hits = probe_hit_part(&mut session, &points);
    for hit in hits {
        if let Some(hit_obj) = hit.as_object() {
            if hit_obj.get("part") == Some(&Value::String("molding".into())) {
                panic!("glass-wall probe hit molding: {hit_obj:?}");
            }
        }
    }
}

#[test]
fn phase18_demo_saves_two_cameras() {
    let script = load_demo();
    let camera_saves = script
        .iter()
        .filter(|step| step.get("cmd") == Some(&Value::String("camera".into())))
        .filter_map(|step| step.get("params").and_then(|params| params.get("save")).and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(camera_saves.len() >= 2, "demo should save at least two cameras: {camera_saves:?}");
    assert!(camera_saves.iter().any(|name| *name == "Kitchen"), "Kitchen camera is missing: {camera_saves:?}");
    assert!(camera_saves.iter().any(|name| *name == "Bath"), "Bath camera is missing: {camera_saves:?}");
}
