//! The incremental 3D rebuild (`SceneCache::update`) gives exactly the full build (`Scene3d::build`)
//! after any edit, on models drawn with the commands: two levels, rooms, a roof, a stair, light
//! fixtures and a terrain; that house with cabinets and casework fixtures; and the 300-wall sample
//! with 150 openings. Seeded random edits of every kind; after each one the cached scene has the
//! full build's meshes in the full build's order, the same counts, and a `rebuilt` count no larger
//! than what the edit can reach.

use std::collections::BTreeMap;

use cadcraft_arch3d::{Scene3d, SceneCache};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_engine::Session;
use serde_json::{Value, json};

fn model(cmds: &[(&str, Value)]) -> ArchModel {
    let mut s = Session::empty();
    s.new_drawing(false);
    for (cmd, params) in cmds {
        s.execute(cmd, params).unwrap_or_else(|e| panic!("{cmd} {params}: {e:?}"));
    }
    s.doc().expect("a drawing").arch.clone()
}

fn house() -> ArchModel {
    model(&[
        ("arch.sample", json!({"house2": true})),
        ("roofauto", json!({"pitchRise": 6, "pitchRun": 12})),
        ("lightfixture", json!({"room": "r0", "rows": 2, "cols": 2})),
        ("lightfixture", json!({"at": [1.5, 2.0], "type": "Recessed pot light"})),
        ("stair", json!({"start": [1.0, 1.0], "direction": 90.0})),
        ("terrain", json!({"boundary": [[-10.0, -10.0], [25.0, -10.0], [25.0, 20.0], [-10.0, 20.0]], "elevation": -0.3})),
    ])
}

/// What must match: the digest, every mesh's material and owner in order, every count.
fn signature(s: &Scene3d) -> (String, Vec<(String, u32)>, [usize; 12]) {
    (
        s.digest(),
        s.meshes.iter().map(|m| (m.material.clone(), m.object.0)).collect(),
        [s.openings_built, s.floors, s.ceilings, s.roofs, s.stairs, s.railings, s.decks, s.terrain, s.paths, s.plants, s.fixtures, s.lights.len()],
    )
}

/// One seeded edit; returns its name.
fn edit(m: &mut ArchModel, state: &mut u64) -> &'static str {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let r = (*state >> 33) as usize;
    let d = if r & 1 == 0 { 0.15 } else { -0.15 };
    match r % 11 {
        0 if !m.walls.is_empty() => {
            let i = r % m.walls.len();
            m.walls[i].end.x += d;
            "move a wall end"
        }
        1 if !m.openings.is_empty() => {
            let i = r % m.openings.len();
            m.openings[i].offset = (m.openings[i].offset + d).max(0.3);
            "move an opening"
        }
        2 if !m.openings.is_empty() => {
            let i = r % m.openings.len();
            m.openings[i].width = (m.openings[i].width + d).max(0.4);
            "resize an opening"
        }
        3 if !m.rooms.is_empty() => {
            let i = r % m.rooms.len();
            m.rooms[i].ceiling_height = Some(m.rooms[i].ceiling_height.unwrap_or(2.4) + d.abs());
            "raise a room's ceiling"
        }
        4 if !m.lights.is_empty() => {
            let i = r % m.lights.len();
            m.lights[i].position.x += d;
            "move a light"
        }
        5 if !m.lights.is_empty() => {
            let i = r % m.lights.len();
            m.lights[i].on = !m.lights[i].on;
            "toggle a light"
        }
        6 if !m.roofs.is_empty() => {
            let i = r % m.roofs.len();
            for p in &mut m.roofs[i].planes {
                p.slope *= 1.05;
            }
            "steepen the roof"
        }
        7 if !m.stairs.is_empty() => {
            let i = r % m.stairs.len();
            m.stairs[i].width = (m.stairs[i].width + d).max(0.6);
            "widen the stair"
        }
        8 if m.walls.len() > 4 => {
            let mut w = m.walls[r % m.walls.len()].clone();
            w.id = ArchId(m.next_id);
            m.next_id += 1;
            w.start.y += 0.5;
            w.end.y += 0.5;
            m.walls.push(w);
            "add a wall"
        }
        9 if m.walls.len() > 4 => {
            let id = m.walls.remove(r % m.walls.len()).id;
            m.openings.retain(|o| o.host != id);
            "remove a wall"
        }
        _ if !m.walls.is_empty() => {
            let i = r % m.walls.len();
            m.walls[i].start.y += d;
            "move a wall start"
        }
        _ => "nothing",
    }
}

/// One seeded casework edit; returns its name.
fn casework_edit(m: &mut ArchModel, state: &mut u64) -> &'static str {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let r = (*state >> 33) as usize;
    let d = if r & 1 == 0 { 0.05 } else { -0.05 };
    match r % 7 {
        0 if !m.cabinets.is_empty() => {
            let i = r % m.cabinets.len();
            m.cabinets[i].position.x += d;
            "move a cabinet"
        }
        1 if !m.cabinets.is_empty() => {
            let i = r % m.cabinets.len();
            m.cabinets[i].width = (m.cabinets[i].width + d).max(0.3);
            "resize a cabinet"
        }
        2 if !m.cabinets.is_empty() => {
            let i = r % m.cabinets.len();
            m.cabinets[i].counter = !m.cabinets[i].counter;
            "toggle a counter"
        }
        3 if !m.cabinets.is_empty() => {
            let i = r % m.cabinets.len();
            m.cabinets[i].drawers = (m.cabinets[i].drawers + 1) % 4;
            "change a cabinet's drawers"
        }
        4 if !m.fixtures.is_empty() => {
            let i = r % m.fixtures.len();
            m.fixtures[i].position.y += d;
            "move a fixture"
        }
        5 if !m.fixtures.is_empty() => {
            let i = r % m.fixtures.len();
            m.fixtures[i].rotation += 90.0;
            "turn a fixture"
        }
        _ => edit(m, state),
    }
}

/// `edits` seeded edits of `m`; after each, the cache equals the full build. Returns, per edit kind,
/// the most objects one edit of that kind rebuilt.
fn cache_follows_every_edit(name: &str, m: ArchModel, edits: usize, seed: u64) -> BTreeMap<&'static str, usize> {
    cache_follows(name, m, edits, seed, edit)
}

/// [`cache_follows_every_edit`] with another seeded edit.
fn cache_follows(
    name: &str,
    mut m: ArchModel,
    edits: usize,
    seed: u64,
    edit: fn(&mut ArchModel, &mut u64) -> &'static str,
) -> BTreeMap<&'static str, usize> {
    let (mut cache, full) = SceneCache::new(&m);
    assert!(!full.incremental && full.rebuilt == full.objects, "{name}: {full:?}");
    let mut state = seed;
    let mut most: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut objects = full.objects;
    for k in 0..edits {
        let before = cache.scene().digest();
        let what = edit(&mut m, &mut state);
        let stats = cache.update(&m);
        let rebuilt = Scene3d::build(&m);
        assert_eq!(signature(cache.scene()), signature(&rebuilt), "{name}, edit {k} ({what}): the cache is not the full build; {stats:?}");
        assert!(stats.incremental && stats.rebuilt <= stats.objects, "{name}, edit {k} ({what}): {stats:?}");
        // A removal can change the scene by taking meshes away alone; any other change rebuilt something.
        if rebuilt.digest() != before && stats.objects >= objects {
            assert!(stats.rebuilt >= 1, "{name}, edit {k} ({what}) changed the scene but rebuilt nothing: {stats:?}");
        }
        objects = stats.objects;
        let e = most.entry(what).or_default();
        *e = (*e).max(stats.rebuilt);
    }
    most
}

#[test]
fn the_scene_cache_follows_every_edit_of_a_two_level_house() {
    let most = cache_follows_every_edit("house", house(), 150, 7);
    // A ceiling carries the lights in it (their height), so raising one rebuilds the room and its 4 fixtures.
    assert!(most.values().all(|&n| n <= 6), "{most:?}");
}

#[test]
fn the_scene_cache_follows_every_casework_edit() {
    let mut cmds = vec![
        ("arch.sample", json!({"house2": true})),
        ("cabinet", json!({"kind": "base", "wall": "w1", "offset": 1.0, "widths": [0.6, 0.9, 0.6]})),
        ("cabinet", json!({"kind": "wall", "wall": "w1", "offset": 1.0})),
        ("cabinet", json!({"kind": "tall", "wall": "w2", "offset": 1.0})),
    ];
    for (kind, at) in [("sink", [2.0, 1.0]), ("toilet", [6.0, 1.0]), ("refrigerator", [3.0, 6.0]), ("bathtub", [7.0, 6.0])] {
        cmds.push(("fixture", json!({"kind": kind, "at": at})));
    }
    let m = model(&cmds);
    assert!(m.cabinets.len() >= 3 && m.fixtures.len() == 4, "{} cabinets, {} fixtures", m.cabinets.len(), m.fixtures.len());
    let cabinets = m.cabinets.len();
    let most = cache_follows("casework", m, 120, 13, casework_edit);
    // A fixture edit rebuilds only that fixture. A cabinet edit rebuilds the cabinets of its level
    // (phase 14: counter runs, gaps and cut-outs reach across a level's base cabinets), no more.
    for kind in ["move a fixture", "turn a fixture"] {
        assert_eq!(most.get(kind), Some(&1), "{kind}: {most:?}");
    }
    for kind in ["move a cabinet", "resize a cabinet", "toggle a counter", "change a cabinet's drawers"] {
        assert!(most.get(kind).is_some_and(|&n| (1..=cabinets).contains(&n)), "{kind}: {most:?}");
    }
}

#[test]
fn the_scene_cache_follows_every_edit_of_the_300_wall_sample() {
    let m = model(&[("arch.sample", json!({"walls": 300, "openings": 150}))]);
    let most = cache_follows_every_edit("sample", m, 60, 11);
    // One wall edit reaches the wall, the walls joined to it and the openings they host, not the 450 objects.
    assert!(most.values().all(|&n| n <= 12), "{most:?}");
}

#[test]
fn the_benchmark_house_bench_edit_rebuilds_a_few_objects() {
    // Phase-13 check 08 times this edit: `arch.sample {"house": true}` names an interior wall end and a point 0.3 m
    // along the wall; the stretch takes the X join there with it, out and back.
    let mut s = Session::empty();
    s.new_drawing(false);
    let r = s.execute("arch.sample", &json!({"house": true})).expect("the benchmark house");
    let b = &r["bench"];
    let (mut cache, full) = SceneCache::new(&s.doc().expect("a drawing").arch);
    for to in [&b["to"], &b["from"]] {
        s.execute("arch.wall.stretch", &json!({"wall": b["wall"], "end": b["end"], "to": to})).expect("the bench stretch");
        let m = s.doc().expect("a drawing").arch.clone();
        let stats = cache.update(&m);
        assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)), "the bench edit to {to}: {stats:?}");
        assert!(stats.incremental && stats.rebuilt >= 1 && stats.rebuilt * 10 <= full.objects, "the bench edit to {to}: {stats:?}");
    }
}

#[test]
fn an_update_without_an_edit_rebuilds_nothing() {
    let m = house();
    let (mut cache, _) = SceneCache::new(&m);
    let stats = cache.update(&m);
    assert_eq!((stats.rebuilt, stats.incremental), (0, true));
    assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)));
}

#[test]
fn a_level_edit_rebuilds_everything() {
    let mut m = house();
    let (mut cache, _) = SceneCache::new(&m);
    m.levels[0].elevation += 0.5;
    let stats = cache.update(&m);
    assert!(!stats.incremental && stats.rebuilt == stats.objects, "{stats:?}");
    assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)));
}

// ---------- phase 14: furnishings, paint and casework parts ----------

/// The next seeded number.
fn next(state: &mut u64) -> usize {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*state >> 33) as usize
}

/// `list[r % len]`, or `None` for an empty list.
fn pick<T: Clone>(list: &[T], r: usize) -> Option<T> {
    list.get(r.checked_rem(list.len())?).cloned()
}

fn arch_of(s: &Session) -> ArchModel {
    s.doc().expect("a drawing").arch.clone()
}

/// How many furnishings, cabinets and painted surfaces differ between two models (added, removed or changed).
fn touched(a: &ArchModel, b: &ArchModel) -> usize {
    let mut n = 0;
    for f in &a.furnishings {
        n += usize::from(b.furnishing(f.id) != Some(f));
    }
    n += b.furnishings.iter().filter(|f| a.furnishing(f.id).is_none()).count();
    for c in &a.cabinets {
        n += usize::from(b.cabinets.iter().find(|x| x.id == c.id) != Some(c));
    }
    n += b.cabinets.iter().filter(|c| !a.cabinets.iter().any(|x| x.id == c.id)).count();
    for (k, p) in &a.paints {
        n += usize::from(b.paints.get(k) != Some(p));
    }
    n += b.paints.keys().filter(|k| !a.paints.contains_key(*k)).count();
    n
}

/// One seeded edit of the furnished house through the commands; returns its name and whether it was accepted.
fn furnished_edit(s: &mut Session, state: &mut u64) -> (&'static str, bool) {
    let r = next(state);
    let r2 = next(state);
    let m = arch_of(s);
    let ids: Vec<usize> = m.furnishings.iter().map(|f| f.id.0 as usize).collect();
    let handles: Vec<String> = m.furnishings.iter().map(|f| cadcraft_doc::arch::furnishing_handle(f.id).hex()).collect();
    let cabs: Vec<usize> = m.cabinets.iter().map(|c| c.id.0 as usize).collect();
    let f_id = pick(&ids, r2).map(|i| format!("f{i}"));
    let f_handle = pick(&handles, r2);
    let unit = |k: usize| (k % 9) as f64 / 4.0 - 1.0;
    let at = [1.0 + (r2 % 80) as f64 / 10.0, 0.5 + (r2 / 80 % 60) as f64 / 10.0];
    let (name, cmd, params): (&'static str, &str, Option<Value>) = match r % 15 {
        0 => ("furnishing at", "furnishing.edit", f_id.map(|id| json!({"id": id, "at": at}))),
        1 => ("furnishing rotation", "furnishing.edit", f_id.map(|id| json!({"id": id, "rotation": (r2 % 4) as f64 * 90.0 + 15.0}))),
        2 => {
            let size = [0.4 + (r2 % 5) as f64 / 10.0, 0.5, 0.6 + (r2 % 3) as f64 / 10.0];
            ("furnishing size", "furnishing.edit", f_id.map(|id| json!({"id": id, "size": size})))
        }
        3 => ("furnishing mirrored", "furnishing.edit", f_id.map(|id| json!({"id": id, "mirrored": r2 % 2 == 0}))),
        4 => ("furnishing delete", "furnishing.delete", f_id.map(|id| json!({"id": id}))),
        5 => {
            let items = [
                "armchair",
                "plant-small",
                "table-coffee-90",
                "bookcase",
                "own:dishwasher-60",
                "towel-rack",
                "light-pendant",
                "faucet-kitchen-chrome",
                "own:sink-kitchen",
            ];
            ("place another", "library.place", pick(&items, r2 / 7).map(|i| json!({"item": i, "at": at, "level": "Level 1"})))
        }
        6..=8 => {
            let side = if r2 % 2 == 0 { "left" } else { "right" };
            let wall_ids: Vec<u32> = m.walls.iter().filter(|w| w.base_level == 1).map(|w| w.id.0).collect();
            let room_ids: Vec<u32> = m.rooms.iter().map(|x| x.id.0).collect();
            let roof_ids: Vec<u32> = m.roofs.iter().map(|x| x.id.0).collect();
            let cab_ids: Vec<u32> = m.cabinets.iter().map(|c| c.id.0).collect();
            let (target, materials): (String, &[&str]) = match r2 % 5 {
                0 => (format!("wall/w{}/{side}", pick(&wall_ids, r2 / 5).unwrap_or(0)), &["brick", "siding", "#c04020", "material-oak"]),
                1 => (
                    format!("room/r{}/{}", pick(&room_ids, r2 / 5).unwrap_or(0), if r2 % 2 == 0 { "floor" } else { "ceiling" }),
                    &["material-marble", "#3060a0", "material-oak"],
                ),
                2 => (
                    format!("cabinet/cb{}/{}", pick(&cab_ids, r2 / 5).unwrap_or(0), ["carcass", "front", "counter", "pull"][r2 / 3 % 4]),
                    &["material-oak", "material-marble", "#808080"],
                ),
                3 => (format!("furnishing/f{}/0", pick(&ids, r2 / 5).unwrap_or(0)), &["#c04020", "material-oak", "material-fabric"]),
                _ => (format!("roof/r{}", pick(&roof_ids, r2 / 5).unwrap_or(0)), &["shingles", "#404040"]),
            };
            let material = pick(materials, r2 / 11).unwrap_or("#c04020");
            let scope = ["component", "object", "room", "floor", "plan"][r / 15 % 5];
            let mut p = json!({"material": material, "target": target, "scope": scope});
            if r / 75 % 3 == 0 {
                p["mode"] = json!("blend");
            }
            ("paint material", "paint.material", Some(p))
        }
        9 => {
            let items = ["own:pull-knob-32", "own:pull-bar-128", "frontstyle:door-shaker", "frontstyle:drawer-raised-panel"];
            let scope = ["component", "room", "plan"][r / 15 % 3];
            let p = pick(&items, r2 / 3)
                .zip(pick(&cabs, r2 / 17))
                .map(|(i, c)| json!({"item": i, "target": format!("cabinet/cb{c}/pull"), "scope": scope}));
            ("paint object", "paint.object", p)
        }
        10..=12 => {
            let styles = ["slab", "shaker", "raised-panel"];
            let key = ["doorStyle", "drawerStyle", "pull"][r % 3];
            let value = if key == "pull" { json!(["own:pull-knob-32", "own:pull-bar-128"][r2 % 2]) } else { json!(styles[r2 % 3]) };
            ("cabinet part", "cabinet.edit", pick(&cabs, r2 / 3).map(|c| json!({"id": format!("cb{c}"), key: value})))
        }
        13 => {
            let h = f_handle.map(|h| json!([h]));
            match r2 % 3 {
                0 => ("move furnishing", "move", h.map(|h| json!({"handles": h, "delta": [unit(r2 / 3) * 0.5, 0.25]}))),
                1 => ("copy furnishing", "copy", h.map(|h| json!({"handles": h, "delta": [0.3, unit(r2 / 3) * 0.5 + 0.6]}))),
                _ => ("mirror furnishing", "mirror", h.map(|h| json!({"handles": h, "p1": [5.0, 0.0], "p2": [5.0, 8.0]}))),
            }
        }
        _ => ("undo", "undo", Some(json!({}))),
    };
    match params {
        Some(p) => (name, s.execute(cmd, &p).is_ok()),
        None => (name, false),
    }
}

#[test]
fn furnishing_paint_and_part_edits_keep_the_cache_exact() {
    cadcraft_engine::library::set_library_io(cadcraft_engine::library::LibraryIo {
        read_glb: cadcraft_io::gltf_read::read_glb,
        read_image: cadcraft_io::image_read::read_image,
        encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
        sample: cadcraft_io::library::sample_files,
    });
    let mut s = Session::empty();
    s.new_drawing(false);
    // A library that keeps nothing on disk: the built-in and the embedded sample items only.
    s.library = cadcraft_engine::library::Library::with_user_dir(None);
    let setup: Vec<(&str, Value)> = vec![
        ("arch.sample", json!({"house2": true})),
        ("roofauto", json!({"pitchRise": 6, "pitchRun": 12})),
        ("lightfixture", json!({"room": "r0", "rows": 2, "cols": 2})),
        ("stair", json!({"start": [1.0, 1.0], "direction": 90.0})),
        // Level 1's south wall is w5: two base cabinets with a 0.6 m gap between, and a wider one for the sink.
        ("cabinet", json!({"kind": "base", "wall": "w5", "offset": 1.0, "width": 0.6})),
        ("cabinet", json!({"kind": "base", "wall": "w5", "offset": 2.2, "width": 0.6})),
        ("cabinet", json!({"kind": "base", "wall": "w5", "offset": 4.0, "width": 0.9})),
    ];
    for (cmd, p) in &setup {
        s.execute(cmd, p).unwrap_or_else(|e| panic!("{cmd} {p}: {e:?}"));
    }
    let mut hosts = Vec::new();
    for p in [
        json!({"item": "armchair", "at": [6.0, 5.0], "level": "Level 1"}),
        json!({"item": "towel-rack", "at": [7.0, 0.2], "level": "Level 1"}),
        json!({"item": "light-pendant", "at": [6.0, 3.0], "level": "Level 1"}),
        json!({"item": "own:sink-kitchen", "at": [4.0, 0.4], "level": "Level 1"}),
        json!({"item": "own:dishwasher-60", "at": [1.6, 0.4], "level": "Level 1"}),
        json!({"item": "faucet-kitchen-chrome", "at": [4.0, 0.3], "level": "Level 1"}),
    ] {
        let r = s.execute("library.place", &p).unwrap_or_else(|e| panic!("library.place {p}: {e:?}"));
        hosts.push(r["host"]["kind"].as_str().unwrap_or("none").to_string());
    }
    assert_eq!(hosts, ["none", "wall", "none", "cabinet", "gap", "sink"], "where the six furnishings went");
    let mut m = arch_of(&s);
    assert_eq!(m.furnishings.len(), 6);
    let (mut cache, full) = SceneCache::new(&m);
    assert!(!full.incremental);
    assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)));
    let mut state = 2014;
    let mut accepted: BTreeMap<&'static str, usize> = BTreeMap::new();
    for k in 0..120 {
        let before = m;
        let (what, ok) = furnished_edit(&mut s, &mut state);
        m = arch_of(&s);
        let stats = cache.update(&m);
        let full = Scene3d::build(&m);
        assert_eq!(signature(cache.scene()), signature(&full), "edit {k} ({what}, accepted: {ok}): the cache is not the full build; {stats:?}");
        assert_eq!(cache.scene().digest(), full.digest(), "edit {k} ({what}): digests differ");
        let allowed = touched(&before, &m) + m.cabinets.len();
        assert!(
            stats.incremental && stats.rebuilt <= allowed,
            "edit {k} ({what}): rebuilt {} of {}, at most {allowed} expected; {stats:?}",
            stats.rebuilt,
            stats.objects
        );
        if ok {
            *accepted.entry(what).or_default() += 1;
        }
    }
    for what in ["furnishing at", "furnishing delete", "place another", "paint material", "paint object", "cabinet part", "undo"] {
        assert!(accepted.get(what).is_some_and(|&n| n >= 1), "no accepted {what}: {accepted:?}");
    }
}

#[test]
fn undo_of_a_furnishing_edit_restores_the_previous_scene() {
    cadcraft_engine::library::set_library_io(cadcraft_engine::library::LibraryIo {
        read_glb: cadcraft_io::gltf_read::read_glb,
        read_image: cadcraft_io::image_read::read_image,
        encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
        sample: cadcraft_io::library::sample_files,
    });
    let mut s = Session::empty();
    s.new_drawing(false);
    s.library = cadcraft_engine::library::Library::with_user_dir(None);
    s.execute("arch.sample", &json!({"house2": true})).unwrap();
    s.execute("library.place", &json!({"item": "armchair", "at": [6.0, 5.0], "level": "Level 1"})).unwrap();
    let m_before = arch_of(&s);
    let (mut cache, _) = SceneCache::new(&m_before);
    let before = signature(cache.scene());
    // Move the furnishing.
    s.execute("furnishing.edit", &json!({"id": "f0", "at": [7.0, 6.0]})).unwrap();
    let m_after = arch_of(&s);
    cache.update(&m_after);
    let after = signature(cache.scene());
    assert_ne!(before, after, "moving the furnishing should change the scene");
    // Undo.
    s.execute("undo", &json!({})).unwrap();
    let m_restored = arch_of(&s);
    cache.update(&m_restored);
    let restored = signature(cache.scene());
    assert_eq!(restored, before, "undo should restore the previous scene");
    assert_eq!(restored, signature(&Scene3d::build(&m_restored)), "cache must equal the full build after undo");
}

#[test]
fn hostile_furnishing_edits_are_refused_and_leave_the_cache_unchanged() {
    cadcraft_engine::library::set_library_io(cadcraft_engine::library::LibraryIo {
        read_glb: cadcraft_io::gltf_read::read_glb,
        read_image: cadcraft_io::image_read::read_image,
        encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
        sample: cadcraft_io::library::sample_files,
    });
    let mut s = Session::empty();
    s.new_drawing(false);
    s.library = cadcraft_engine::library::Library::with_user_dir(None);
    s.execute("arch.sample", &json!({"house2": true})).unwrap();
    s.execute("library.place", &json!({"item": "armchair", "at": [6.0, 5.0], "level": "Level 1"})).unwrap();
    let m = arch_of(&s);
    let (mut cache, _) = SceneCache::new(&m);
    let before = signature(cache.scene());
    // Non-existent furnishing id.
    assert!(s.execute("furnishing.edit", &json!({"id": "f999", "at": [1.0, 1.0]})).is_err());
    // Delete a non-existent furnishing.
    assert!(s.execute("furnishing.delete", &json!({"id": "f999"})).is_err());
    // Paint a non-existent wall.
    assert!(s.execute("paint.material", &json!({"material": "oak", "target": "wall/w999/left", "scope": "component"})).is_err());
    // Edit a non-existent cabinet.
    assert!(s.execute("cabinet.edit", &json!({"id": "cb999", "doorStyle": "shaker"})).is_err());
    // The cache must be unchanged.
    let after = signature(cache.scene());
    assert_eq!(after, before, "hostile edits must not change the cache");
    assert_eq!(after, signature(&Scene3d::build(&arch_of(&s))), "cache must equal the full build");
}

#[test]
fn copying_one_furnishing_twenty_times_rebuilds_one_object_each_time() {
    cadcraft_engine::library::set_library_io(cadcraft_engine::library::LibraryIo {
        read_glb: cadcraft_io::gltf_read::read_glb,
        read_image: cadcraft_io::image_read::read_image,
        encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
        sample: cadcraft_io::library::sample_files,
    });
    let mut s = Session::empty();
    s.new_drawing(false);
    s.library = cadcraft_engine::library::Library::with_user_dir(None);
    s.execute("arch.sample", &json!({"house2": true})).unwrap();
    s.execute("library.place", &json!({"item": "armchair", "at": [6.0, 5.0], "level": "Level 1"})).unwrap();
    let m0 = arch_of(&s);
    let f = &m0.furnishings[0];
    let handle = cadcraft_doc::arch::furnishing_handle(f.id).hex();
    let (mut cache, _) = SceneCache::new(&m0);
    for i in 0..20 {
        let dx = 0.5 + (i as f64) * 0.1;
        s.execute("copy", &json!({"handles": [handle], "delta": [dx, 0.0]})).unwrap();
        let m = arch_of(&s);
        let stats = cache.update(&m);
        assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)), "copy {i}: cache must equal the full build");
        assert!(stats.incremental, "copy {i} must be incremental");
        // Only the new furnishing should be rebuilt.
        assert!(stats.rebuilt <= 2, "copy {i} rebuilt {} objects, expected at most 2", stats.rebuilt);
    }
}

#[test]
fn deleting_all_furnishings_removes_their_meshes_from_the_scene() {
    cadcraft_engine::library::set_library_io(cadcraft_engine::library::LibraryIo {
        read_glb: cadcraft_io::gltf_read::read_glb,
        read_image: cadcraft_io::image_read::read_image,
        encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
        sample: cadcraft_io::library::sample_files,
    });
    let mut s = Session::empty();
    s.new_drawing(false);
    s.library = cadcraft_engine::library::Library::with_user_dir(None);
    s.execute("arch.sample", &json!({"house2": true})).unwrap();
    for item in ["armchair", "bookcase", "table-coffee-90"] {
        s.execute("library.place", &json!({"item": item, "at": [6.0, 5.0], "level": "Level 1"})).unwrap();
    }
    let m = arch_of(&s);
    assert_eq!(m.furnishings.len(), 3);
    let (mut cache, _) = SceneCache::new(&m);
    let before = cache.scene().meshes.len();
    // Delete all three furnishings.
    let ids: Vec<String> = m.furnishings.iter().map(|f| format!("f{}", f.id.0)).collect();
    for id in &ids {
        s.execute("furnishing.delete", &json!({"id": id})).unwrap();
    }
    let m2 = arch_of(&s);
    assert_eq!(m2.furnishings.len(), 0);
    let stats = cache.update(&m2);
    let after = cache.scene().meshes.len();
    assert!(after < before, "deleting furnishings must remove meshes: {} -> {}", before, after);
    assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m2)), "cache must equal the full build after deletion");
    // No mesh should have a furnishing material.
    for mesh in &cache.scene().meshes {
        assert!(!mesh.material.starts_with("sym:"), "furnishing mesh remains: {}", mesh.material);
    }
    assert!(stats.incremental, "deletion must be incremental");
}
