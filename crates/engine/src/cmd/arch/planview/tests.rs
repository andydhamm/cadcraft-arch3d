#![allow(clippy::unwrap_used)]

use super::*;
use crate::Session;
use cadcraft_doc::arch::{Level, Plant, Room, Stair, Terrain, Wall};
use cadcraft_geom::Vec2;
use serde_json::json;

fn quad(x: f64, y: f64) -> Vec<Vec2> {
    vec![Vec2::new(x, y), Vec2::new(x + 4.0, y), Vec2::new(x + 4.0, y + 3.0), Vec2::new(x, y + 3.0)]
}

/// A two-level model with a wall and a room on each level.
fn session() -> Session {
    let mut s = Session::new();
    {
        let d = s.doc_mut().unwrap();
        d.arch.levels = vec![
            Level { index: 0, name: "L0".into(), ..Level::default() },
            Level { index: 1, name: "L1".into(), elevation: 3.0, ..Level::default() },
        ];
        d.arch.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        d.arch.push_room(Room { polygon: quad(0.0, 0.0), name: "Down".into(), level: 0, ..Room::default() });
        d.arch.push_room(Room { polygon: quad(0.0, 10.0), name: "Up".into(), level: 1, ..Room::default() });
    }
    s
}

fn layer_on(s: &Session, name: &str) -> Option<bool> {
    s.doc().unwrap().layer(name).map(|l| l.on)
}

fn names(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().filter_map(|n| n.as_str().map(str::to_string)).collect()
}

#[test]
fn create_from_working_opens_and_activates() {
    let mut s = session();
    let r = s.execute("planview", &json!({"name": "Mine"})).unwrap();
    assert_eq!(r["planView"], "Mine");
    assert_eq!(names(&r["open"]), vec!["Mine"]);
    assert_eq!(r["active"], "Mine");
    let d = s.doc().unwrap();
    let working = d.arch.plan_view("Working").unwrap().clone();
    let mine = d.arch.plan_view("Mine").unwrap();
    assert_eq!(mine.layers.rules, working.layers.rules);
    assert_eq!(mine.layers.name, "Mine");
    // Not opened when asked not to.
    let r = s.execute("planview", &json!({"name": "Quiet", "open": false})).unwrap();
    assert_eq!(names(&r["open"]), vec!["Mine"]);
    assert_eq!(r["active"], "Mine");
}

#[test]
fn duplicate_copies_every_field() {
    let mut s = session();
    s.execute(
        "planview",
        &json!({"name": "Src", "level": "L1", "kind": "roof", "annotation": {"layer": "A-ANNO-X", "dimStyle": "Standard"},
                "labels": {"area": true, "ceiling": true}, "reference": {"level": 0, "color": "#102030", "transparency": 40},
                "zoom": {"center": [1, 2], "width": 8, "height": 6}, "layers": {"rules": [{"pattern": "A-*", "on": false}], "capture": true}, "open": false}),
    )
    .unwrap();
    s.execute("planview", &json!({"name": "Copy", "from": "src", "open": false})).unwrap();
    let d = s.doc().unwrap();
    let (a, b) = (d.arch.plan_view("Src").unwrap(), d.arch.plan_view("Copy").unwrap());
    assert_eq!(b.name, "Copy");
    assert_eq!((a.level, a.kind, &a.annotation, a.labels, a.reference, a.zoom), (b.level, b.kind, &b.annotation, b.labels, b.reference, b.zoom));
    assert_eq!(a.layers.rules, b.layers.rules);
    assert_eq!(a.layers.layers, b.layers.layers);
    assert!(!b.layers.layers.is_empty());
    assert_eq!(b.reference.map(|r| r.color), Some([16, 32, 48]));
    assert_eq!(b.level, Some(1));
}

#[test]
fn rename_follows_tabs_and_active() {
    let mut s = session();
    s.execute("planview", &json!({"name": "A"})).unwrap();
    s.execute("planview", &json!({"name": "B"})).unwrap();
    let r = s.execute("planview.edit", &json!({"name": "a", "rename": "Alpha"})).unwrap();
    assert_eq!(r["planView"], "Alpha");
    assert_eq!(names(&r["open"]), vec!["Alpha", "B"]);
    assert_eq!(r["active"], "B");
    let r = s.execute("planview.edit", &json!({"name": "B", "rename": "Beta"})).unwrap();
    assert_eq!(r["active"], "Beta");
    let d = s.doc().unwrap();
    assert!(d.arch.plan_view("A").is_none());
    assert_eq!(d.arch.plan_view("Alpha").unwrap().layers.name, "Alpha");
    assert!(s.execute("planview.edit", &json!({"name": "Alpha", "rename": "working"})).is_err());
    // A case-only rename of itself is fine.
    s.execute("planview.edit", &json!({"name": "Alpha", "rename": "ALPHA"})).unwrap();
}

#[test]
fn delete_closes_its_tab_and_activates_the_last_remaining() {
    let mut s = session();
    s.execute("planview", &json!({"name": "A"})).unwrap();
    s.execute("planview", &json!({"name": "B", "layers": {"rules": [{"pattern": "*", "on": false}]}})).unwrap();
    assert_eq!(layer_on(&s, "A-WALL"), Some(false));
    let r = s.execute("planview.delete", &json!({"name": "B"})).unwrap();
    assert_eq!(r["deleted"], "B");
    assert_eq!(names(&r["open"]), vec!["A"]);
    assert_eq!(r["active"], "A");
    // A was applied again (Working's rules: everything on).
    assert_eq!(layer_on(&s, "A-WALL"), Some(true));
    let r = s.execute("planview.delete", &json!({"name": "A"})).unwrap();
    assert!(r["open"].as_array().unwrap().is_empty());
    assert!(r["active"].is_null());
    assert!(s.doc().unwrap().arch.plan_view("A").is_none());
}

#[test]
fn open_applies_level_layers_annotation_and_zoom() {
    let mut s = session();
    s.execute(
        "planview",
        &json!({"name": "Up", "level": 1, "annotation": {"layer": "A-ANNO-UP", "dimStyle": "Standard"}, "open": false,
                "layers": {"rules": [{"pattern": "*", "on": true, "transparency": 0}, {"pattern": "A-WALL", "transparency": 60}, {"pattern": "A-DOOR", "on": false}]},
                "zoom": {"center": [5, 4], "width": 12, "height": 9}}),
    )
    .unwrap();
    let r = s.execute("planview.open", &json!({"name": "up"})).unwrap();
    assert_eq!(r["planView"], "Up");
    assert_eq!(r["level"], 1);
    assert_eq!(r["kind"], "floor");
    assert_eq!(r["clayer"], "A-ANNO-UP");
    assert_eq!(r["active"], "Up");
    assert!(r["layers"]["dimmed"].as_u64().unwrap() >= 1);
    assert!(r["layers"]["off"].as_u64().unwrap() >= 1);
    let d = s.doc().unwrap();
    assert_eq!(d.arch.plan_level, Some(1));
    assert_eq!(d.layer("A-WALL").unwrap().transparency, 60);
    assert_eq!(d.header.str("DIMSTYLE", ""), "Standard");
    let v = s.state().unwrap().view();
    assert!((v.center.x - 5.0).abs() < 1e-9 && (v.center.y - 4.0).abs() < 1e-9 && v.height >= 9.0 - 1e-9);
}

#[test]
fn open_keeps_the_shown_level_for_an_any_level_view() {
    let mut s = session();
    s.execute("level.show", &json!({"level": 1})).unwrap();
    s.execute("planview.open", &json!({"name": "Electrical"})).unwrap();
    assert_eq!(s.doc().unwrap().arch.plan_level, Some(1));
}

#[test]
fn open_creates_the_annotation_layer_and_the_arch_layers_first() {
    let mut s = session();
    {
        let d = s.doc_mut().unwrap();
        d.arch.push_stair(Stair::default());
        d.arch.terrain = Some(Terrain::default());
        d.arch.plants.push(Plant::default());
        d.layers.retain(|l| l.name != "A-WALL" && l.name != "A-STAIR");
        assert!(d.layer("A-ROOM").is_none() && d.layer("L-PLNT").is_none());
    }
    s.execute("planview.open", &json!({"name": "Reflected Ceiling"})).unwrap();
    for n in ["A-WALL", "A-ROOM", "A-STAIR", "C-TOPO-MAJR", "C-ROAD", "L-PLNT", "A-ANNO-NRTH", "A-CLNG-ANNO"] {
        assert!(layer_on(&s, n).is_some(), "{n} exists");
    }
    for n in ["C-TOPO-MAJR", "L-PLNT", "A-STAIR"] {
        assert_eq!(layer_on(&s, n), Some(false), "{n} off");
    }
    assert_eq!(layer_on(&s, "A-WALL"), Some(true));
    assert_eq!(s.doc().unwrap().header.str("CLAYER", ""), "A-CLNG-ANNO");
}

#[test]
fn ensure_plan_layers_creates_without_applying() {
    let mut s = session();
    let d = s.doc_mut().unwrap();
    d.arch.plants.push(Plant::default());
    let v = d.arch.plan_view("Reflected Ceiling").unwrap().clone();
    let before_rev = d.arch_revision;
    ensure_plan_layers(d, &v);
    assert!(d.layer("L-PLNT").is_some_and(|l| l.on && l.transparency == 0));
    assert!(d.layer("A-CLNG-ANNO").is_some());
    assert!(d.layer("A-WALL").is_some_and(|l| l.on));
    assert!(d.arch.active_plan_view.is_none() && d.arch.open_plan_views.is_empty());
    assert_eq!(d.arch_revision, before_rev);
}

#[test]
fn close_activates_the_last_open_tab() {
    let mut s = session();
    for n in ["A", "B", "C"] {
        s.execute("planview", &json!({"name": n})).unwrap();
    }
    let r = s.execute("planview.close", &json!({"name": "a"})).unwrap();
    assert_eq!(r["closed"], "A");
    assert_eq!(r["active"], "C");
    let r = s.execute("planview.close", &json!({})).unwrap();
    assert_eq!((r["closed"].as_str(), r["active"].as_str()), (Some("C"), Some("B")));
    let r = s.execute("planview.close", &json!({"name": "B"})).unwrap();
    assert!(r["active"].is_null() && r["open"].as_array().unwrap().is_empty());
    assert!(s.execute("planview.close", &json!({})).is_err());
    assert!(s.execute("planview.close", &json!({"name": "A"})).is_err());
}

#[test]
fn capture_snapshots_the_current_layers() {
    let mut s = session();
    s.execute("layer.set", &json!({"name": "A-DOOR", "on": false})).unwrap();
    s.execute("planview", &json!({"name": "Snap", "layers": {"capture": true, "rules": []}, "open": false})).unwrap();
    s.execute("layer.set", &json!({"name": "A-DOOR", "on": true})).unwrap();
    let n = s.doc().unwrap().layers.len();
    assert_eq!(s.doc().unwrap().arch.plan_view("Snap").unwrap().layers.layers.len(), n);
    s.execute("planview.open", &json!({"name": "Snap"})).unwrap();
    assert_eq!(layer_on(&s, "A-DOOR"), Some(false));
}

#[test]
fn edit_of_the_active_view_reapplies() {
    let mut s = session();
    s.execute("planview", &json!({"name": "V"})).unwrap();
    assert_eq!(layer_on(&s, "A-WALL"), Some(true));
    s.execute("planview.edit", &json!({"name": "V", "layers": {"rules": [{"pattern": "A-WALL", "on": false}]}, "level": 1})).unwrap();
    assert_eq!(layer_on(&s, "A-WALL"), Some(false));
    assert_eq!(s.doc().unwrap().arch.plan_level, Some(1));
    // An inactive view is edited without touching the layers.
    s.execute("planview", &json!({"name": "W", "open": false})).unwrap();
    s.execute("planview.edit", &json!({"name": "W", "layers": {"rules": [{"pattern": "*", "on": true}]}})).unwrap();
    assert_eq!(layer_on(&s, "A-WALL"), Some(false));
}

#[test]
fn refusals_change_nothing_and_push_no_undo() {
    let mut s = session();
    s.execute("planview", &json!({"name": "V"})).unwrap();
    let doc_before = s.doc().unwrap().clone();
    let undo_before = s.state().unwrap().undo.len();
    let long = "x".repeat(65);
    let bad_calls: Vec<(&str, Value)> = vec![
        ("planview", json!({})),
        ("planview", json!({"name": ""})),
        ("planview", json!({"name": 7})),
        ("planview", json!({"name": long})),
        ("planview", json!({"name": "working"})),
        ("planview", json!({"name": "A", "from": "Nope"})),
        ("planview", json!({"name": "A", "level": 9})),
        ("planview", json!({"name": "A", "kind": "attic"})),
        ("planview", json!({"name": "A", "layers": {"rules": "all"}})),
        ("planview", json!({"name": "A", "layers": {"rules": [{"pattern": "*", "transparency": 95}]}})),
        ("planview", json!({"name": "A", "layers": {"rules": [{"pattern": "x".repeat(300)}]}})),
        ("planview", json!({"name": "A", "reference": {"level": 5}})),
        ("planview", json!({"name": "A", "zoom": {"center": [0, 0], "width": -1, "height": 2}})),
        ("planview", json!({"name": "A", "labels": {"area": "yes"}})),
        ("planview", json!({"name": "A", "annotation": {"layer": "bad*name"}})),
        ("planview", json!({"name": "A", "bogus": 1})),
        ("planview", json!({"name": "A", "open": "yes"})),
        ("planview.open", json!({"name": "Nope"})),
        ("planview.edit", json!({"name": "Floor Plan", "rename": "Working"})),
        ("planview.edit", json!({"name": "Nope"})),
        ("planview.delete", json!({"name": "Nope"})),
        ("planview.close", json!({"name": "Nope"})),
        ("planview.close", json!({"name": "Site"})),
    ];
    for (cmd, params) in bad_calls {
        let e = s.execute(cmd, &params).unwrap_err().to_string();
        assert!(!e.contains('\n'), "{cmd} {params}: {e}");
        assert_eq!(*s.doc().unwrap(), doc_before, "{cmd} {params} changed the drawing");
        assert_eq!(s.state().unwrap().undo.len(), undo_before, "{cmd} {params} pushed an undo step");
    }
}

#[test]
fn each_command_is_one_undo_step_and_never_touches_arch_revision() {
    let mut s = session();
    let rev = s.doc().unwrap().arch_revision;
    let calls = [
        ("planview", json!({"name": "V", "level": 1})),
        ("planview.edit", json!({"name": "V", "rename": "W"})),
        ("planview.open", json!({"name": "Site"})),
        ("planview.close", json!({"name": "Site"})),
        ("planview.delete", json!({"name": "W"})),
    ];
    for (cmd, params) in calls {
        let before = s.state().unwrap().undo.len();
        s.execute(cmd, &params).unwrap();
        assert_eq!(s.state().unwrap().undo.len(), before + 1, "{cmd}");
        assert_eq!(s.doc().unwrap().arch_revision, rev, "{cmd} touched arch_revision");
    }
    let before = s.state().unwrap().undo.len();
    s.execute("planview.list", &json!({})).unwrap();
    assert_eq!(s.state().unwrap().undo.len(), before);
    // One undo takes the last command back.
    s.undo().unwrap();
    assert!(s.doc().unwrap().arch.plan_view("W").is_some());
}

#[test]
fn apply_plan_view_is_pure_on_a_drawing() {
    let mut d = Drawing::new_metric();
    d.arch.levels = vec![Level { index: 0, name: "L0".into(), ..Level::default() }, Level { index: 1, name: "L1".into(), ..Level::default() }];
    d.arch.rooms.push(Room::default());
    let mut v = d.arch.plan_view("Floor Plan").unwrap().clone();
    v.level = Some(1);
    v.annotation.dim_style = "Standard".into();
    let rev = d.arch_revision;
    let r = apply_plan_view(&mut d, &v);
    assert_eq!(d.arch.plan_level, Some(1));
    assert_eq!(d.arch.active_plan_view.as_deref(), Some("Floor Plan"));
    assert_eq!(d.arch.open_plan_views, vec!["Floor Plan".to_string()]);
    assert_eq!(r.on + r.off + r.dimmed, d.layers.len());
    assert!(d.layer("A-ROOM").is_some());
    assert_eq!(d.arch_revision, rev);
    // A second application changes nothing more.
    let again = d.clone();
    apply_plan_view(&mut d, &v);
    assert_eq!(d.arch.open_plan_views.len(), 1);
    assert_eq!(d.layers, again.layers);
    // An out-of-range level leaves the shown level.
    v.level = Some(9);
    apply_plan_view(&mut d, &v);
    assert_eq!(d.arch.plan_level, Some(1));
}

#[test]
fn level_show_after_open_moves_labels_with_the_walls() {
    let mut s = session();
    s.execute("planview", &json!({"name": "Up", "level": 1, "labels": {"name": true}})).unwrap();
    let plan = s.execute("arch.inspect", &json!({"plan": true})).unwrap();
    assert_eq!(plan["plan"]["level"], 1);
    let texts = |p: &Value| -> Vec<String> {
        p["plan"]["labels"].as_array().unwrap().iter().filter_map(|l| l["text"].as_str().map(str::to_string)).collect()
    };
    assert!(texts(&plan).iter().any(|t| t.contains("Up")) && !texts(&plan).iter().any(|t| t.contains("Down")));
    s.execute("level.show", &json!({"level": 0})).unwrap();
    let plan = s.execute("arch.inspect", &json!({"plan": true})).unwrap();
    assert_eq!(plan["plan"]["level"], 0);
    assert!(texts(&plan).iter().any(|t| t.contains("Down")) && !texts(&plan).iter().any(|t| t.contains("Up")));
    assert_eq!(s.doc().unwrap().arch.active_plan_view.as_deref(), Some("Up"));
}
