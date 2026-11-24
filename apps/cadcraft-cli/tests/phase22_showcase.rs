//! The phase-22 showcase script: three Photoreal cameras added after `bilevel.json`.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Once;

use cadcraft_engine::{Session, library::LibraryIo};
use serde_json::{Value, json};

fn script(name: &str) -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/arch3d/showcase").join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name} should exist: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name} is valid JSON: {e}"))
}

fn install_library_io() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        cadcraft_engine::library::set_library_io(LibraryIo {
            read_glb: cadcraft_io::gltf_read::read_glb,
            read_image: cadcraft_io::image_read::read_image,
            encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
            sample: cadcraft_io::library::sample_files,
        });
    });
}

fn replay(session: &mut Session, steps: &[Value]) {
    session.set_views_deferred(true);
    for (idx, step) in steps.iter().enumerate() {
        let cmd = step.get("cmd").and_then(Value::as_str).unwrap_or_else(|| panic!("step {idx} missing cmd: {step:?}"));
        let params = step.get("params").cloned().unwrap_or(Value::Object(Default::default()));
        let result = session.execute(cmd, &params);
        assert!(result.is_ok(), "step {idx} `{cmd}` failed: {result:?}");
    }
    session.set_views_deferred(false);
}

/// Counts of walls, rooms and furnishings.
fn counts(session: &mut Session) -> [usize; 3] {
    let r = session.execute("arch.inspect", &json!({"walls": true, "rooms": true, "furnishings": true})).expect("arch.inspect");
    ["walls", "rooms", "furnishings"].map(|k| r[k].as_array().map_or(0, Vec::len))
}

/// Replays bilevel.json, then the phase-22 script, in one session. Returns the session and the
/// model counts taken between the two scripts.
fn house() -> (Session, [usize; 3]) {
    install_library_io();
    let mut session = Session::new();
    replay(&mut session, &script("bilevel.json"));
    let before = counts(&mut session);
    replay(&mut session, &script("phase-22-showcase.json"));
    (session, before)
}

fn cameras(session: &mut Session) -> (BTreeMap<String, Value>, Value) {
    let r = session.execute("arch.inspect", &json!({"cameras": true})).expect("arch.inspect cameras");
    let list = r["cameras"].as_array().expect("cameras list").clone();
    let map = list.into_iter().filter_map(|c| c.get("name").and_then(Value::as_str).map(|n| (n.to_string(), c.clone()))).collect();
    (map, r)
}

#[test]
fn the_phase22_script_adds_three_photoreal_cameras() {
    let (mut session, _) = house();
    let (cams, _) = cameras(&mut session);
    for name in ["Kitchen-photo", "Living-photo", "Dusk-photo"] {
        let cam = cams.get(name).unwrap_or_else(|| panic!("{name} missing: {:?}", cams.keys()));
        assert_eq!(cam["look"]["technique"], "photoreal", "{name} technique");
    }
}

#[test]
fn the_photoreal_cameras_carry_their_lenses_and_looks() {
    let (mut session, _) = house();
    let (cams, _) = cameras(&mut session);
    let num = |v: &Value| v.as_f64().unwrap_or(f64::NAN);

    let k = &cams["Kitchen-photo"]["look"]["lens"];
    assert_eq!(k["projection"], "panini");
    assert!((num(&k["compression"]) - 0.7).abs() < 1e-9);

    let l = &cams["Living-photo"]["look"]["lens"];
    assert_eq!(l["projection"], "panini");
    assert!((num(&l["compression"]) - 0.7).abs() < 1e-9);
    assert!((num(&l["aperture"]) - 8.0).abs() < 1e-9);
    assert!((num(&l["focus"]) - 6.0).abs() < 1e-9);

    let d = &cams["Dusk-photo"]["look"];
    assert!((num(&d["lens"]["shift"]) - 0.15).abs() < 1e-9);
    assert_eq!(d["sun"]["angle"], "dusk");
    assert_eq!(d["lightSet"], "Evening");
}

#[test]
fn the_phase22_script_changes_nothing_but_cameras() {
    let (mut session, before) = house();
    assert_eq!(counts(&mut session), before, "walls, rooms and furnishings are unchanged");
    let (cams, r) = cameras(&mut session);
    for name in ["Front", "Back", "Kitchen", "Living", "dusk", "noon-hdri", "overview"] {
        assert!(cams.contains_key(name), "{name} should survive: {:?}", cams.keys());
    }
    assert_eq!(r["activeCamera"], "Front");
}
