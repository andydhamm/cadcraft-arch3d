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
    let most = cache_follows("casework", m, 120, 13, casework_edit);
    // A casework edit rebuilds only that cabinet or fixture.
    for kind in ["move a cabinet", "resize a cabinet", "toggle a counter", "change a cabinet's drawers", "move a fixture", "turn a fixture"] {
        assert_eq!(most.get(kind), Some(&1), "{kind}: {most:?}");
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
