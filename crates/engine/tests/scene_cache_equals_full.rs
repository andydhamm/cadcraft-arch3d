//! The incremental 3D rebuild (`SceneCache::update`) gives exactly the full build (`Scene3d::build`)
//! after any edit, on models drawn with the commands: two levels, rooms, a roof, a stair, light
//! fixtures and a terrain; that house with cabinets and casework fixtures; and the 300-wall sample
//! with 150 openings. Seeded random edits of every kind; after each one the cached scene has the
//! full build's meshes in the full build's order, the same counts, and a `rebuilt` count no larger
//! than what the edit can reach.

use std::collections::BTreeMap;

use cadcraft_arch3d::{Scene3d, SceneCache};
use cadcraft_doc::{
    ArchId, ArchModel,
    arch::{
        CabinetKind, CabinetMolding, CeilingLayer, CornerPref, CounterEdge, Countertop, EdgeProfile, FaceKind, FaceLayout, FaceRow, FoundationKind,
        MoldingKind, MoldingProfile, OpeningKind, RoomMolding, RoomType,
    },
};
use cadcraft_engine::Session;
use cadcraft_geom::Vec2;
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
        ("outlet", json!({"kind": "duplex", "wall": "w0", "offset": 1.5, "side": "left"})),
        ("switch", json!({"kind": "single-pole", "wall": "w0", "offset": 5.0, "side": "right"})),
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
    match r % 12 {
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
        8 if !m.electrical.is_empty() => {
            let i = r % m.electrical.len();
            m.electrical[i].offset = (m.electrical[i].offset + d).clamp(0.2, 8.0);
            "move an electrical device"
        }
        9 if m.walls.len() > 4 => {
            let mut w = m.walls[r % m.walls.len()].clone();
            w.id = ArchId(m.next_id);
            m.next_id += 1;
            w.start.y += 0.5;
            w.end.y += 0.5;
            m.walls.push(w);
            "add a wall"
        }
        10 if m.walls.len() > 4 => {
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

#[test]
fn the_scene_cache_follows_every_phase_16_edit() {
    let mut s = Session::empty();
    s.new_drawing(false);
    for (cmd, params) in [
        ("wall", json!({"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": true})),
        ("wall", json!({"points": [[0, 3], [10, 3]], "thickness": 0.1, "height": 2.4, "type": "interior"})),
        ("wall", json!({"points": [[6, 3], [6, 8]], "thickness": 0.1, "height": 2.4, "type": "interior"})),
        ("roomdetect", json!({})),
        ("roofauto", json!({"pitchRise": 6, "pitchRun": 12, "walls": [{"wall": "w1", "directive": "gable"}, {"wall": "w3", "directive": "gable"}]})),
    ] {
        s.execute(cmd, &params).unwrap_or_else(|e| panic!("{cmd} {params}: {e:?}"));
    }
    let mut m = s.doc().expect("a drawing").arch.clone();
    let room_by_point = |x: f64, y: f64| {
        let mut best = None;
        let mut best_dist = f64::INFINITY;
        for room in &m.rooms {
            let poly = &room.polygon;
            if poly.is_empty() {
                continue;
            }
            let area = poly.iter().enumerate().fold(0.0, |sum, (i, p)| {
                let q = &poly[(i + 1) % poly.len()];
                sum + (p.x * q.y - q.x * p.y)
            });
            let cx = poly.iter().map(|p| p.x).sum::<f64>() / poly.len() as f64;
            let cy = poly.iter().map(|p| p.y).sum::<f64>() / poly.len() as f64;
            let dist = (cx - x).hypot(cy - y);
            if area != 0.0 && dist < best_dist {
                best = Some(room.id);
                best_dist = dist;
            }
        }
        best.expect("three rooms under the gable")
    };
    let s_room = room_by_point(5.0, 1.5);
    let nw_room = room_by_point(3.0, 5.5);
    let ne_room = room_by_point(8.0, 5.5);
    let tray_id = m.push_tray(cadcraft_doc::arch::Tray { room: nw_room, level: 0, inset: 0.6, depth: 0.3, ..Default::default() });
    let _soffit_id = m.push_soffit(cadcraft_doc::arch::Soffit {
        level: 0,
        polygon: vec![
            cadcraft_geom::Vec2::new(7.0, 7.3),
            cadcraft_geom::Vec2::new(9.0, 7.3),
            cadcraft_geom::Vec2::new(9.0, 7.9),
            cadcraft_geom::Vec2::new(7.0, 7.9),
        ],
        bottom: Some(2.1),
        ..Default::default()
    });
    let _foundation_id =
        m.push_foundation(cadcraft_doc::arch::Foundation { level: 0, kind: FoundationKind::Stem, footing_depth: 1.2, ..Default::default() });
    m.lights.push(cadcraft_doc::arch::LightFixture::new(ArchId(0), "Surface ceiling fixture", cadcraft_geom::Vec2::new(7.0, 4.0)));
    fn room_index(model: &ArchModel, room: ArchId) -> usize {
        model.rooms.iter().position(|r| r.id == room).unwrap_or_else(|| panic!("missing room {room:?}"))
    }
    let mut cache = SceneCache::new(&m).0;
    let label_order = [
        "flat toggle S",
        "flat toggle NW",
        "flat toggle NE",
        "stack S",
        "stack NW",
        "stack NE",
        "soffit shift",
        "soffit bottom",
        "tray depth",
        "tray inset",
        "tray add",
        "tray delete",
        "roof steepen",
        "footing depth",
        "kind flip",
    ];
    let mut state = 17u64;
    let mut most: BTreeMap<&'static str, usize> = BTreeMap::new();
    for k in 0..120 {
        let label = label_order[k % label_order.len()];
        let before = signature(cache.scene());
        match label {
            "flat toggle S" => {
                let idx = room_index(&m, s_room);
                m.rooms[idx].flat_ceiling = !m.rooms[idx].flat_ceiling;
            }
            "flat toggle NW" => {
                let idx = room_index(&m, nw_room);
                m.rooms[idx].flat_ceiling = !m.rooms[idx].flat_ceiling;
            }
            "flat toggle NE" => {
                let idx = room_index(&m, ne_room);
                m.rooms[idx].flat_ceiling = !m.rooms[idx].flat_ceiling;
            }
            "stack S" => {
                let idx = room_index(&m, s_room);
                let room = &mut m.rooms[idx];
                room.ceiling_stack =
                    if room.ceiling_stack.is_empty() { vec![CeilingLayer::new("drywall", Some("drywall"), 0.0127)] } else { Vec::new() };
            }
            "stack NW" => {
                let idx = room_index(&m, nw_room);
                let room = &mut m.rooms[idx];
                room.ceiling_stack =
                    if room.ceiling_stack.is_empty() { vec![CeilingLayer::new("drywall", Some("drywall"), 0.0127)] } else { Vec::new() };
            }
            "stack NE" => {
                let idx = room_index(&m, ne_room);
                let room = &mut m.rooms[idx];
                room.ceiling_stack =
                    if room.ceiling_stack.is_empty() { vec![CeilingLayer::new("drywall", Some("drywall"), 0.0127)] } else { Vec::new() };
            }
            "soffit shift" => {
                let soffit = &mut m.soffits[0];
                let delta = if soffit.polygon.iter().all(|p| p.y > 7.2) { -0.1 } else { 0.1 };
                soffit.polygon = soffit.polygon.iter().map(|p| cadcraft_geom::Vec2::new(p.x, p.y + delta)).collect();
            }
            "soffit bottom" => {
                let soffit = &mut m.soffits[0];
                soffit.bottom = Some(if soffit.bottom == Some(2.1) { 2.05 } else { 2.1 });
            }
            "tray depth" => {
                let tray = &mut m.trays[0];
                tray.depth = if tray.depth < 0.35 { 0.4 } else { 0.2 };
            }
            "tray inset" => {
                let tray = &mut m.trays[0];
                tray.inset = if tray.inset < 0.6 { 0.7 } else { 0.5 };
            }
            "tray add" => {
                let parent = m.trays[0].id;
                m.push_tray(cadcraft_doc::arch::Tray { room: nw_room, level: 0, parent: Some(parent), inset: 0.6, depth: 0.1, ..Default::default() });
            }
            "tray delete" => {
                if m.trays.len() > 1 {
                    let last = m.trays.last().unwrap().id;
                    m.trays.retain(|t| t.id != last);
                }
            }
            "roof steepen" => {
                for roof in &mut m.roofs {
                    for plane in &mut roof.planes {
                        plane.slope = if plane.slope > 1.0 { plane.slope / 1.05 } else { plane.slope * 1.05 };
                    }
                }
            }
            "footing depth" => {
                let footing = &mut m.foundations[0];
                // A footing-depth edit is only meaningful while the foundation is a stem foundation;
                // when the kind is slab the edit is a no-op, and the separate kind-flip edit owns
                // the kind change. Keeping the depth edit pure avoids falsely invalidating room slab
                // floors when the foundation kind changes.
                if footing.kind == FoundationKind::Slab {
                    continue;
                }
                footing.footing_depth = if footing.footing_depth <= 1.1 {
                    (footing.footing_depth + 0.1).clamp(0.8, 1.6)
                } else {
                    (footing.footing_depth - 0.1).clamp(0.8, 1.6)
                };
            }
            "kind flip" => {
                let foundation = &mut m.foundations[0];
                foundation.kind = if foundation.kind == FoundationKind::Stem { FoundationKind::Slab } else { FoundationKind::Stem };
            }
            _ => unreachable!("unexpected phase-16 label: {label}"),
        }
        let stats = cache.update(&m);
        let rebuilt = Scene3d::build(&m);
        assert_eq!(signature(cache.scene()), signature(&rebuilt), "phase-16 edit {k} ({label})");
        assert!(stats.incremental && stats.rebuilt <= stats.objects, "phase-16 edit {k} ({label}): {stats:?}");
        let seen = most.entry(label).or_default();
        *seen = (*seen).max(stats.rebuilt);
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let before_digest = before.0;
        let after_digest = rebuilt.digest();
        assert_ne!(before_digest, after_digest, "phase-16 edit {k} ({label}) changed no geometry");
    }
    // Formula from the phase-16 dependency graph: each edit rebuilds only the room, the walls it rises to,
    // the soffits overlapping it, and the ceiling lights in that room that are not under a soffit.
    let bounds: BTreeMap<&str, usize> = BTreeMap::from([
        ("flat toggle S", 3),
        ("flat toggle NW", 4),
        ("flat toggle NE", 6),
        ("stack S", 1),
        ("stack NW", 1),
        ("stack NE", 3),
        ("soffit shift", 1),
        ("soffit bottom", 1),
        ("tray depth", 1),
        ("tray inset", 1),
        ("tray add", 1),
        ("tray delete", 1),
        ("roof steepen", 12),
        ("footing depth", 1),
        ("kind flip", 4),
    ]);
    for (label, bound) in bounds {
        let built = most.get(label).copied().unwrap_or(0);
        assert!(built <= bound, "{label}: rebuilt {built} > {bound}; most: {most:?}");
    }
}

#[test]
fn phase_16_edits_through_commands_keep_the_cache_exact() {
    let mut s = Session::empty();
    s.new_drawing(false);
    let setup: Vec<(&str, Value)> = vec![
        ("wall", json!({"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": true})),
        ("wall", json!({"points": [[0, 3], [10, 3]], "thickness": 0.1, "height": 2.4, "type": "interior"})),
        ("wall", json!({"points": [[6, 3], [6, 8]], "thickness": 0.1, "height": 2.4, "type": "interior"})),
        ("roomdetect", json!({})),
        ("roofauto", json!({"pitchRise": 6, "pitchRun": 12, "walls": [{"wall": "w1", "directive": "gable"}, {"wall": "w3", "directive": "gable"}]})),
        ("tray", json!({"room": "r1", "inset": 0.6, "depth": 0.3})),
        ("soffit", json!({"rect": [[7.0, 7.3], [9.0, 7.9]], "bottom": 2.1})),
        ("foundation", json!({"type": "stem"})),
        ("build3d", json!({"incremental": true})),
    ];
    for (cmd, params) in &setup {
        s.execute(cmd, params).unwrap_or_else(|e| panic!("{cmd} {params}: {e:?}"));
    }
    let room_rows = s.execute("arch.inspect", &json!({"rooms": true})).unwrap()["rooms"].as_array().unwrap().clone();
    let room_id = |x: f64, y: f64| -> String {
        let empty_poly: Vec<Value> = Vec::new();
        room_rows
            .iter()
            .filter_map(|row| row.as_object())
            .find(|row| {
                let poly = row.get("polygon").and_then(Value::as_array).unwrap_or(&empty_poly);
                if poly.is_empty() {
                    return false;
                }
                let pts: Vec<[f64; 2]> = poly.iter().filter_map(|p| p.as_array()).map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]).collect();
                if pts.is_empty() {
                    return false;
                }
                let cx = pts.iter().map(|p| p[0]).sum::<f64>() / pts.len() as f64;
                let cy = pts.iter().map(|p| p[1]).sum::<f64>() / pts.len() as f64;
                (cx - x).hypot(cy - y) < 1.0
            })
            .and_then(|row| row.get("id"))
            .and_then(Value::as_str)
            .unwrap()
            .to_string()
    };
    let s_id = room_id(5.0, 1.5);
    let nw_id = room_id(3.0, 5.5);
    let ne_id = room_id(8.0, 5.5);
    let inspect = s.execute("arch.inspect", &json!({"rooms": true, "trays": true, "soffits": true, "foundations": true})).unwrap();
    let tray_id = inspect["trays"].as_array().unwrap().last().unwrap()["id"].as_str().unwrap().to_string();
    let soffit_id = inspect["soffits"].as_array().unwrap().last().unwrap()["id"].as_str().unwrap().to_string();
    let foundation_id = inspect["foundations"].as_array().unwrap().last().unwrap()["id"].as_str().unwrap().to_string();
    let edits = [
        ("tray.edit", json!({"id": tray_id, "depth": 0.25}), 1),
        ("soffit.edit", json!({"id": soffit_id, "bottom": 2.0}), 1),
        ("room", json!({"id": ne_id, "ceilingStack": "drywall"}), 2),
        ("room", json!({"id": s_id, "flatCeiling": false}), 3),
        ("foundation.edit", json!({"id": foundation_id, "footingDepth": 1.5}), 1),
        ("foundation.edit", json!({"id": foundation_id, "type": "slab"}), 4),
    ];
    for (edit_cmd, params, want) in &edits {
        s.execute(edit_cmd, params).unwrap_or_else(|e| panic!("{edit_cmd} {params}: {e:?}"));
        let inc = s.execute("build3d", &json!({"incremental": true, "digest": true})).unwrap();
        let full = s.execute("build3d", &json!({"digest": true})).unwrap();
        assert_eq!(inc["incremental"], true, "edit {edit_cmd} should be incremental");
        assert_eq!(inc["digest"], full["digest"], "edit {edit_cmd}: incremental digest differs from full digest");
        assert_eq!(inc["rebuilt"], *want, "edit {edit_cmd}: rebuilt {} should be {want}", inc["rebuilt"]);
    }
}

/// The phase-17 scene: the phase-1 house with Level 2, a 0.1 m wall under a 0.9 m stair, the north
/// deck with its railing, a drawn railing and a window with a 2 x 2 grille, all through a Session.
fn phase_17_session() -> Session {
    let mut s = Session::empty();
    s.new_drawing(false);
    let setup: Vec<(&str, Value)> = vec![
        ("wall", json!({"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": true})),
        ("wall", json!({"points": [[0, 3], [10, 3]], "thickness": 0.1, "height": 2.4, "type": "interior"})),
        ("wall", json!({"points": [[6, 3], [6, 8]], "thickness": 0.1, "height": 2.4, "type": "interior"})),
        ("roomdetect", json!({})),
        ("level", json!({"above": 0})),
        ("wall", json!({"points": [[3, 0], [3, 3]], "thickness": 0.1})),
        ("stair", json!({"start": [1, 1], "direction": 0, "width": 0.9, "fromLevel": 0, "toLevel": 1})),
        ("deck", json!({"points": [[2, 8.1], [6, 8.1], [6, 11.1], [2, 11.1], [2, 8.1]], "height": 0.9})),
        ("railing", json!({"path": [[12, 0], [12, 3], [16, 3]]})),
        ("window", json!({"wall": "w2", "offset": 6.0, "width": 0.9, "height": 1.2, "sill": 0.9, "grille": {"across": 2, "down": 2}})),
    ];
    for (cmd, params) in &setup {
        s.execute(cmd, params).unwrap_or_else(|e| panic!("{cmd} {params}: {e:?}"));
    }
    s
}

/// One seeded phase-17 edit of the model of [`phase_17_session`]; returns its label.
fn phase_17_edit(m: &mut ArchModel, state: &mut u64) -> &'static str {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let r = (*state >> 33) as usize;
    match r % 10 {
        0 => {
            m.stairs[0].open_risers = !m.stairs[0].open_risers;
            "stair risers"
        }
        1 => {
            use cadcraft_doc::arch::Stringers;
            m.stairs[0].stringers = match m.stairs[0].stringers {
                Stringers::Two => Stringers::Centre,
                Stringers::Centre => Stringers::None,
                Stringers::None => Stringers::Two,
            };
            "stair stringers"
        }
        2 => {
            m.stairs[0].underside = if m.stairs[0].underside < 0.35 { 0.4 } else { 0.3 };
            "stair underside"
        }
        3 => {
            m.stairs[0].width = if m.stairs[0].width < 0.95 { 1.0 } else { 0.9 };
            "stair width"
        }
        4 => {
            if let Some(room) = m.rooms.iter_mut().find(|r| r.deck_height.is_some()) {
                room.deck_height = Some(if room.deck_height == Some(0.9) { 1.0 } else { 0.9 });
            }
            "deck height"
        }
        5 => {
            if let Some(room) = m.rooms.iter_mut().find(|r| r.deck_height.is_some()) {
                room.deck_boards = if room.deck_boards.is_some() { None } else { Some(90.0) };
            }
            "deck boards"
        }
        6 => {
            if let Some(rail) = m.railings.iter_mut().find(|r| r.follows.is_none()) {
                rail.post_spacing = if rail.post_spacing > 1.5 { 1.2 } else { 1.8 };
            }
            "railing spacing"
        }
        7 => {
            if let Some(rail) = m.railings.iter_mut().find(|r| r.follows.is_none()) {
                rail.inset = if rail.inset > 0.05 { 0.0 } else { 0.1 };
            }
            "railing inset"
        }
        8 => {
            if let Some(o) = m.openings.first_mut() {
                o.grille = if o.grille.is_some() { None } else { Some(cadcraft_doc::arch::Grille { across: 2, down: 2, ..Default::default() }) };
            }
            "grille"
        }
        _ => {
            // The (3,0)-(3,3) wall: its x is 3 or one nudge away.
            if let Some(w) =
                m.walls.iter_mut().find(|w| (w.start.y - 0.0).abs() < 1e-9 && (w.end.y - 3.0).abs() < 1e-9 && (w.start.x - 3.0).abs() < 0.06)
            {
                let dx = if w.start.x > 3.0 { -0.05 } else { 0.05 };
                w.start.x += dx;
                w.end.x += dx;
            }
            "wall nudge"
        }
    }
}

#[test]
fn the_scene_cache_follows_every_phase_17_edit() {
    let s = phase_17_session();
    let m = s.doc().expect("a drawing").arch.clone();
    assert!(
        m.rooms.iter().any(|r| r.deck_height.is_some()) && m.railings.iter().any(|r| r.follows.is_none()),
        "the scene has its deck and drawn railing"
    );
    assert!(m.openings.first().is_some_and(|o| o.grille.is_some()), "the window has its grille");
    let most = cache_follows("phase 17", m, 120, 17, phase_17_edit);
    // Bounds = 1 + the objects whose inputs (incremental.rs) contain the edited data, for this scene:
    //   stair risers, stair stringers: StairIn only                           -> 1
    //   stair underside: the stair + the wall under it (WallIn.flight_cuts)    -> 2
    //   stair width: the stair + the wall under it (its cut follows the width;
    //     the well is stored, RailSurface's FlightLine has no width)           -> 2
    //   deck height: DeckIn + its railing's RailSurface::Deck                  -> 2
    //   deck boards, railing spacing, railing inset: the one object            -> 1
    //   grille: the opening + its host wall (WallIn.openings holds Openings)   -> 2
    //   wall nudge: the wall + the walls joined to it + their openings (none
    //     on w2's neighbours here) + the stair if `against` changes (it does not) -> 3
    let bounds: BTreeMap<&str, usize> = BTreeMap::from([
        ("stair risers", 1),
        ("stair stringers", 1),
        ("stair underside", 2),
        ("stair width", 2),
        ("deck height", 2),
        ("deck boards", 1),
        ("railing spacing", 1),
        ("railing inset", 1),
        ("grille", 2),
        ("wall nudge", 3),
    ]);
    for (label, bound) in bounds {
        let built = most.get(label).copied();
        assert!(built.is_some(), "{label} never ran: {most:?}");
        assert!(built.unwrap_or(0) <= bound, "{label}: rebuilt {built:?} > {bound}; most: {most:?}");
    }
}

#[test]
fn phase_17_edits_through_commands_keep_the_cache_exact() {
    let mut s = phase_17_session();
    let deck = s.doc().expect("a drawing").arch.rooms.iter().find(|r| r.deck_height.is_some()).map(|r| format!("r{}", r.id.0)).expect("a deck room");
    s.execute("build3d", &json!({"incremental": true})).unwrap();
    let edits = [
        ("stair.edit", json!({"id": "s0", "openRisers": true})),
        ("stair.edit", json!({"id": "s0", "stringers": "centre"})),
        ("stair.edit", json!({"id": "s0", "underside": 0.4})),
        ("stair.edit", json!({"id": "s0", "railLeft": "off"})),
        ("deck.edit", json!({"id": deck, "boards": 45})),
        ("deck.edit", json!({"id": deck, "height": 1.1})),
        ("deck.edit", json!({"id": deck, "rails": "off"})),
        ("arch.opening.move", json!({"opening": "o0", "offset": 5.0})),
    ];
    for (cmd, params) in &edits {
        s.execute(cmd, params).unwrap_or_else(|e| panic!("{cmd} {params}: {e:?}"));
        let inc = s.execute("build3d", &json!({"incremental": true, "digest": true})).unwrap();
        let full = s.execute("build3d", &json!({"digest": true})).unwrap();
        assert_eq!(inc["incremental"], true, "{cmd} {params}");
        assert_eq!(inc["digest"], full["digest"], "{cmd} {params}: the cache differs from the full build");
    }
}
// ---------- phase 18: interior finish, kitchen and bath ----------
//
// Proves the phase‑18 incremental sections (p18-scene-plumbing) rebuild exactly the
// full build after any phase‑18 edit. The model is pinned from commands so ids are
// deterministic; the direct edits touch the phase‑18 inputs (room moldings, molding
// lines, countertops, backsplashes, cabinet face/crown/corner/shelf/filler, glass
// splits, niches, shower doors, room type). Bounds come from each section's `In`
// struct: a base cabinet sees the base cabinets of its level, a wall/tall cabinet the
// wall and tall cabinets of its level, its kind group, and the claimed ids of its level.

/// The phase‑18 scene: a two‑room house, the lower room as a KITCHEN with room
/// moldings and a molding line; an L kitchen run (corner cabinet + filler) with a
/// detached, water‑falled countertop and a backsplash; wall cabinets (crown, open‑shelf)
/// and a tall cabinet with a face layout and an oven; a thick partition with a niche;
/// a glass partition with a shower door; and a pony wall (split).
fn phase_18_session() -> Session {
    let mut s = Session::empty();
    s.new_drawing(false);
    let setup: Vec<(&str, Value)> = vec![
        ("wall", json!({"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": true})),
        ("wall", json!({"points": [[0, 4], [10, 4]], "thickness": 0.1, "height": 2.4, "type": "interior"})),
        ("roomdetect", json!({})),
        ("room", json!({"id": "r0", "type": "kitchen"})),
        (
            "room.moldings",
            json!({"room": "r0", "moldings": [{"kind": "crown", "offset": 0.0, "size": [0.08, 0.08]}, {"kind": "base", "offset": 0.0, "size": [0.06, 0.06]}]}),
        ),
        ("moldingline", json!({"points": [[2.0, 2.0], [5.0, 2.0]], "profile": "molding:chair-rail", "inset": 0.0, "flip": false, "elevation": 0.9})),
        ("cabinet", json!({"kind": "base", "wall": "w0", "offset": 1.0, "width": 0.6})),
        ("cabinet", json!({"kind": "base", "wall": "w0", "offset": 1.8, "width": 0.6})),
        ("cabinet.fill", json!({"run": "cb0"})),
        ("cabinet", json!({"kind": "base", "wall": "w0", "offset": 0.3, "corner": "blind"})),
        ("cabinet", json!({"kind": "wall", "wall": "w0", "offset": 1.0, "crown": "molding:crown"})),
        ("cabinet", json!({"kind": "wall", "wall": "w0", "offset": 2.8, "fronts": "open"})),
        ("cabinet", json!({"kind": "tall", "wall": "w0", "offset": 5.0})),
        ("countertop.detach", json!({"run": "cb0"})),
        ("backsplash", json!({"run": "cb0", "height": 0.3, "material": "tile"})),
        ("wall", json!({"points": [[3, 4], [3, 8]], "thickness": 0.4, "height": 2.4, "type": "interior"})),
        ("niche", json!({"wall": "w5", "offset": 1.0, "width": 0.6, "height": 0.4, "depth": 0.2})),
        ("wall", json!({"points": [[8, 4], [8, 6]], "thickness": 0.01, "height": 2.0, "type": "glass"})),
        ("door", json!({"wall": "w6", "offset": 1.0, "style": "shower"})),
        ("wall", json!({"points": [[1, 2], [1, 3]], "thickness": 0.15, "height": 2.4, "type": "interior"})),
        ("wall.split", json!({"id": "w7", "height": 1.4, "upper": "Glass"})),
    ];
    for (cmd, params) in &setup {
        s.execute(cmd, &params).unwrap_or_else(|e| panic!("{cmd} {params}: {e:?}"));
    }
    s
}

/// A labelled direct edit of the phase‑18 model; returns its label. Edits are applied
/// to the `ArchModel` fields the phase‑18 incremental sections read (the `In` structs
/// built by p18-scene-plumbing).
fn phase_18_edit(m: &mut ArchModel, state: &mut u64) -> &'static str {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let r = (*state >> 33) as usize;
    let d = if r & 1 == 0 { 0.05 } else { -0.05 };
    match r % 36 {
        0 => {
            if let Some(room) = m.rooms.iter_mut().find(|r| r.room_type == RoomType::Kitchen) {
                if let Some(md) = room.moldings.first_mut() {
                    md.offset = (md.offset + d).abs();
                }
            }
            "molding offset"
        }
        1 => {
            if let Some(room) = m.rooms.iter_mut().find(|r| r.room_type == RoomType::Kitchen) {
                if let Some(md) = room.moldings.first_mut() {
                    let size = md.profile.size();
                    let scale = (size[1] + d.abs()).max(0.02);
                    md.profile.points.iter_mut().for_each(|p| {
                        p[1] *= scale / size[1].max(0.0001);
                    });
                }
            }
            "molding size"
        }
        2 => {
            if let Some(room) = m.rooms.iter_mut().find(|r| r.room_type == RoomType::Kitchen) {
                room.moldings.push(RoomMolding {
                    kind: MoldingKind::Crown,
                    profile: MoldingProfile { item: "molding-profile".to_string(), points: vec![[0.0, 0.0], [0.02, 0.0], [0.02, 0.05], [0.0, 0.05]] },
                    offset: 0.0,
                });
            }
            "molding add"
        }
        3 => {
            if let Some(room) = m.rooms.iter_mut().find(|r| r.room_type == RoomType::Kitchen) {
                room.moldings.pop();
            }
            "molding remove"
        }
        4 => {
            // A door moved along a wall whose base moldings run: the runs recompute.
            if let Some(o) = m.openings.first_mut() {
                o.offset = (o.offset + d).max(0.3);
            }
            "door move"
        }
        5 => {
            if let Some(ml) = m.molding_lines.first_mut() {
                ml.profile = MoldingProfile { item: format!("molding-line-{r}"), points: vec![[0.0, 0.0], [0.02, 0.0], [0.02, 0.08], [0.0, 0.08]] };
            }
            "moldingline point"
        }
        6 => {
            if let Some(ml) = m.molding_lines.first_mut() {
                ml.inset = (ml.inset + d).abs();
            }
            "moldingline inset"
        }
        7 => {
            if let Some(ml) = m.molding_lines.first_mut() {
                ml.flip = !ml.flip;
            }
            "moldingline flip"
        }
        8 => {
            if let Some(ml) = m.molding_lines.first_mut() {
                ml.elevation = (ml.elevation + d).max(0.1);
            }
            "moldingline elevation"
        }
        9 => {
            if let Some(ct) = m.countertops.first_mut() {
                if ct.edges.is_empty() {
                    ct.edges.push(CounterEdge::default());
                }
                let edge = &mut ct.edges[0];
                edge.profile = if matches!(edge.profile, EdgeProfile::Square) { EdgeProfile::Bevel } else { EdgeProfile::Square };
            }
            "counter edge"
        }
        10 => {
            if let Some(ct) = m.countertops.first_mut() {
                if ct.edges.is_empty() {
                    ct.edges.push(CounterEdge::default());
                }
                ct.edges[0].sagitta = (ct.edges[0].sagitta + d).abs();
            }
            "counter sagitta"
        }
        11 => {
            if let Some(ct) = m.countertops.first_mut() {
                if ct.edges.is_empty() {
                    ct.edges.push(CounterEdge::default());
                }
                ct.edges[0].profile = if matches!(ct.edges[0].profile, EdgeProfile::Square) { EdgeProfile::Bullnose } else { EdgeProfile::Square };
            }
            "counter profile"
        }
        12 => {
            if let Some(ct) = m.countertops.first_mut() {
                if ct.edges.is_empty() {
                    ct.edges.push(CounterEdge::default());
                }
                ct.edges[0].waterfall = !ct.edges[0].waterfall;
            }
            "counter waterfall"
        }
        13 => {
            // Detach/create a countertop object.
            if m.countertops.len() >= 2 {
                m.countertops.pop();
            } else {
                let polygon = vec![Vec2::new(0.0, 0.0), Vec2::new(1.5, 0.0), Vec2::new(1.5, 0.6), Vec2::new(0.0, 0.6)];
                let edges = vec![CounterEdge { sagitta: 0.0, profile: EdgeProfile::Square, waterfall: false }; 4];
                m.countertops.push(Countertop {
                    id: ArchId(m.next_id),
                    level: 0,
                    polygon,
                    edges,
                    top: 0.914,
                    thickness: 0.038,
                    cabinets: vec![],
                    layer: "A-FLOR-CASE".to_string(),
                });
                m.next_id += 1;
            }
            "counter detach"
        }
        14 => {
            if let Some(ct) = m.countertops.first() {
                let id = ct.id;
                m.countertops.retain(|c| c.id != id);
            }
            "counter delete"
        }
        15 => {
            if let Some(bs) = m.backsplashes.first_mut() {
                bs.height = (bs.height + d).max(0.2);
            }
            "backsplash height"
        }
        16 => {
            if let Some(bs) = m.backsplashes.first_mut() {
                bs.to_ceiling = !bs.to_ceiling;
            }
            "backsplash toCeiling"
        }
        17 => {
            if let Some(bs) = m.backsplashes.first_mut() {
                bs.counter = Some(cadcraft_doc::arch::CounterRef::Countertop(ArchId(0)));
            }
            "backsplash material"
        }
        18 => {
            if let Some(cb) = m.cabinets.iter_mut().find(|c| !c.filler) {
                cb.backsplash_off = !cb.backsplash_off;
            }
            "cabinet backsplash off"
        }
        19 => {
            if let Some(cb) = m.cabinets.iter_mut().find(|c| !c.filler) {
                let face = cb.face.get_or_insert_with(FaceLayout::default);
                if face.rows.is_empty() {
                    face.rows.push(FaceRow { kind: FaceKind::Door, ..Default::default() });
                }
                let row = &mut face.rows[0];
                row.kind = if matches!(row.kind, FaceKind::Door) { FaceKind::Drawer } else { FaceKind::Door };
            }
            "cabinet face split"
        }
        20 => {
            if let Some(cb) = m.cabinets.iter_mut().find(|c| !c.filler) {
                cb.kind = if matches!(cb.kind, CabinetKind::Base) { CabinetKind::Wall } else { CabinetKind::Base };
            }
            "cabinet kind"
        }
        21 => {
            if let Some(cb) = m.cabinets.iter_mut().find(|c| c.kind == CabinetKind::Tall) {
                cb.height = (cb.height + d).max(1.8);
            }
            "cabinet height"
        }
        22 => {
            if let Some(cb) = m.cabinets.iter_mut().find(|c| c.kind == CabinetKind::Tall) {
                let face = cb.face.get_or_insert_with(FaceLayout::default);
                face.rows.push(FaceRow { kind: FaceKind::Appliance, appliance: Some("oven".to_string()), ..Default::default() });
                if face.rows.len() > 3 {
                    face.rows.drain(0..1);
                }
            }
            "cabinet appliance"
        }
        23 => {
            if let Some(cb) = m.cabinets.iter_mut().find(|c| c.kind == CabinetKind::Wall) {
                cb.crown = if cb.crown.is_some() { None } else { Some(CabinetMolding::default()) };
            }
            "crown on"
        }
        24 => {
            if let Some(cb) = m.cabinets.iter_mut().find(|c| c.kind == CabinetKind::Wall && c.crown.is_some()) {
                cb.crown = None;
            }
            "crown off"
        }
        25 => {
            if let Some(cb) = m.cabinets.iter_mut().find(|c| c.kind == CabinetKind::Wall) {
                let crown = cb.crown.get_or_insert_with(CabinetMolding::default);
                crown.size = Some([0.06, 0.12]);
            }
            "crown size"
        }
        26 => {
            if let Some(cb) = m.cabinets.iter_mut().find(|c| c.kind == CabinetKind::Wall) {
                cb.corner = match cb.corner {
                    CornerPref::Auto => CornerPref::Blind,
                    CornerPref::Blind => CornerPref::Diagonal,
                    CornerPref::Diagonal => CornerPref::LazySusan,
                    CornerPref::LazySusan => CornerPref::None,
                    CornerPref::None => CornerPref::Auto,
                };
            }
            "corner pref"
        }
        27 => {
            if let Some(cb) = m.cabinets.iter_mut().find(|c| c.kind == CabinetKind::Wall) {
                let shelves = cb.shelves.unwrap_or(0) + 1;
                cb.shelves = Some(shelves % 4);
            }
            "shelves"
        }
        28 => {
            if let Some(cb) = m.cabinets.iter_mut().find(|c| c.filler) {
                cb.width = (cb.width + d).max(0.05);
            }
            "filler width"
        }
        29 => {
            if let Some(w) = m.walls.iter_mut().find(|w| w.wall_type == "glass".to_string()) {
                if let Some(s) = w.split.as_mut() {
                    s.height = (s.height + d).max(0.5);
                }
            }
            "glass split height"
        }
        30 => {
            if let Some(w) = m.walls.iter_mut().find(|w| w.wall_type == "glass".to_string()) {
                w.wall_type = if w.wall_type == "glass".to_string() { "interior".into() } else { "glass".into() };
            }
            "glass split type"
        }
        31 => {
            if let Some(o) = m.openings.iter_mut().find(|o| o.kind == OpeningKind::Niche) {
                o.offset = (o.offset + d).max(0.05);
            }
            "niche offset"
        }
        32 => {
            if let Some(o) = m.openings.iter_mut().find(|o| o.kind == OpeningKind::Niche) {
                o.depth = (o.depth + d).max(0.05);
            }
            "niche depth"
        }
        33 => {
            if let Some(o) = m.openings.iter_mut().find(|o| o.kind == OpeningKind::Niche) {
                o.material = Some(format!("stone-{}", r));
            }
            "niche material"
        }
        34 => {
            if let Some(o) = m.openings.iter_mut().find(|o| o.style == "shower".to_string()) {
                o.offset = (o.offset + d).max(0.05);
            }
            "shower door offset"
        }
        35 => {
            // Room type change: floor finish follows the type.
            if let Some(room) = m.rooms.iter_mut().find(|r| r.room_type == RoomType::Kitchen) {
                room.room_type = RoomType::Living;
            }
            "room type"
        }
        _ => unreachable!("unexpected phase‑18 label: {}", r),
    }
}

#[test]
fn the_scene_cache_follows_every_phase_18_edit() {
    let s = phase_18_session();
    let m0 = s.doc().expect("a drawing").arch.clone();
    assert!(
        m0.rooms.iter().any(|r| r.room_type == RoomType::Kitchen) && !m0.countertops.is_empty() && !m0.backsplashes.is_empty()
            || !m0.molding_lines.is_empty(),
        "the scene has its phase‑18 objects"
    );
    let mut m = m0;
    let (mut cache, full) = SceneCache::new(&m);
    assert!(!full.incremental && full.rebuilt == full.objects, "{full:?}");
    // Fixed per‑label bounds from the dependency rule (what an edit can reach):
    //   molding* 1 (the room's key) · door move <= 2 + rooms with moldings on its wall
    //   moldingline* 1 · counter edge/sagitta/profile/waterfall 1 + its backsplash(es)
    //   counter detach/delete <= 1 + level cabinets + level backsplashes
    //   backsplash* 1 · cabinet face/shelves/crown size 1 · crown on/off <= wall+tall cabs
    //   corner/filler <= kind‑group cabinets + backsplashes · glass split <= wall+openings+moldings
    //   niche <= wall+niche+rooms with moldings on that wall · shower door <= wall+door · room type 1
    let mut most: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut objects = full.objects;
    let mut state = 0u64;
    for k in 0..200 {
        let before = cache.scene().digest();
        let what = phase_18_edit(&mut m, &mut state);
        let stats = cache.update(&m);
        let rebuilt = Scene3d::build(&m);
        assert_eq!(signature(cache.scene()), signature(&rebuilt), "phase‑18 edit {k} ({what}): the cache is not the full build; {stats:?}");
        assert!(stats.incremental && stats.rebuilt <= stats.objects, "phase‑18 edit {k} ({what}): {stats:?}");
        if rebuilt.digest() != before && stats.objects >= objects {
            assert!(stats.rebuilt >= 1, "phase‑18 edit {k} ({what}) changed the scene but rebuilt nothing: {stats:?}");
        }
        objects = stats.objects;
        let e = most.entry(what).or_default();
        *e = (*e).max(stats.rebuilt);
    }
    let bounds: BTreeMap<&str, usize> = BTreeMap::from([
        ("molding offset", 1),
        ("molding size", 1),
        ("molding add", 1),
        ("molding remove", 1),
        ("door move", 4),
        ("moldingline point", 1),
        ("moldingline inset", 1),
        ("moldingline flip", 1),
        ("moldingline elevation", 1),
        ("counter edge", 2),
        ("counter sagitta", 2),
        ("counter profile", 2),
        ("counter waterfall", 2),
        ("counter detach", 8),
        ("counter delete", 8),
        ("backsplash height", 1),
        ("backsplash toCeiling", 1),
        ("backsplash material", 1),
        ("cabinet backsplash off", 2),
        ("cabinet face split", 1),
        ("cabinet kind", 6),
        ("cabinet height", 4),
        ("cabinet appliance", 1),
        ("crown on", 4),
        ("crown off", 4),
        ("crown size", 3),
        ("corner pref", 6),
        ("shelves", 1),
        ("filler width", 6),
        ("glass split height", 4),
        ("glass split type", 4),
        ("niche offset", 3),
        ("niche depth", 3),
        ("niche material", 3),
        ("shower door offset", 3),
        ("room type", 1),
    ]);
    for (label, bound) in bounds {
        assert!(most.get(label).is_some(), "{label} never ran: {most:?}");
        let rebuilt = most[label];
        assert!(rebuilt <= bound, "{label}: rebuilt {rebuilt} > {bound}; most: {most:?}");
    }
}

#[test]
fn phase_18_edits_through_commands_keep_the_cache_exact() {
    let mut s = phase_18_session();
    s.execute("build3d", &json!({"incremental": true})).unwrap();
    // A handful of representative phase‑18 command edits, each checked incremental == full.
    let edits = vec![
        ("room.moldings", json!({"room": "r0", "moldings": [{"kind": "base", "offset": 0.0, "size": [0.1, 0.1]}]})),
        ("moldingline.edit", json!({"id": "ml0", "inset": 0.05, "flip": true})),
        ("countertop.edit", json!({"id": "ct0", "edge": 0, "waterfall": true})),
        ("backsplash.edit", json!({"id": "bs0", "height": 0.4, "material": "tile"})),
        ("cabinet.edit", json!({"id": "cb0", "fronts": "open"})),
        ("cabinet.edit", json!({"id": "cb5", "face": {"rows": [{"kind": "appliance"}]} })),
        ("cabinet.fill", json!({"run": "cb0"})),
        ("cabinet.edit", json!({"id": "cb5", "height": 0.9})),
        ("wall.split", json!({"id": "w7", "height": 1.2, "upper": "Glass"})),
        ("room", json!({"id": "r0", "type": "living"})),
        ("room", json!({"id": "r0", "type": "kitchen"})),
    ];
    for (cmd, params) in &edits {
        s.execute(cmd, &params).unwrap_or_else(|e| panic!("{cmd} {params}: {e:?}"));
        let inc = s.execute("build3d", &json!({"incremental": true, "digest": true})).unwrap();
        let full = s.execute("build3d", &json!({"digest": true})).unwrap();
        assert_eq!(inc["incremental"], true, "{cmd} {params}");
        assert_eq!(inc["digest"], full["digest"], "{cmd} {params}: the cache differs from the full build");
    }
}
