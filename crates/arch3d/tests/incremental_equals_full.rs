use std::collections::BTreeSet;
use std::time::Instant;

use cadcraft_arch3d::{ObjectKey, ObjectKind, Scene3d, SceneCache, changed_objects, object_key};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::Vec2;

#[test]
fn object_keys_tell_a_wall_from_an_opening_with_the_same_id() {
    let wall = cadcraft_arch3d::Mesh::new("wall", ArchId(3));
    let opening = cadcraft_arch3d::Mesh::new("window-glass", ArchId(3));
    let wall_key = object_key(&wall);
    let opening_key = object_key(&opening);
    assert_eq!(wall_key.kind, ObjectKind::Wall);
    assert_eq!(opening_key.kind, ObjectKind::Opening);
    assert_ne!(wall_key, opening_key);
}

#[test]
fn changed_objects_finds_one_moved_wall() {
    let mut prev = ArchModel::default();
    prev.walls.push(Wall { id: ArchId(0), start: Vec2::new(0.0, 0.0), end: Vec2::new(4.0, 0.0), thickness: 0.2, height: 2.4, ..Default::default() });

    let mut next = prev.clone();
    next.walls[0].start.x += 0.5;
    next.walls[0].end.x += 0.5;

    let changed = changed_objects(&prev, &next).expect("wall move should be object-local");
    assert_eq!(changed, BTreeSet::from([ObjectKey { kind: ObjectKind::Wall, id: ArchId(0) }]));
}

#[test]
fn changed_objects_is_none_when_a_level_changes() {
    let mut prev = ArchModel::default();
    prev.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
    let mut next = prev.clone();
    next.levels[0].elevation = 1.0;

    assert!(changed_objects(&prev, &next).is_none());
}

#[test]
fn update_without_edits_rebuilds_nothing() {
    let model = ArchModel::default();
    let (mut cache, _) = SceneCache::new(&model);
    let stats = cache.update(&model);
    assert_eq!(stats.rebuilt, 0);
    assert!(stats.incremental);
    assert_eq!(cache.scene().digest(), Scene3d::build(&model).digest());
}

#[test]
fn incremental_digest_equals_full_after_random_edits() {
    let mut model = ArchModel::default();
    for i in 0..60 {
        let x0 = i as f64;
        let x1 = x0 + 1.0;
        model.walls.push(Wall {
            id: ArchId(i),
            start: Vec2::new(x0, 0.0),
            end: Vec2::new(x1, 0.0),
            thickness: 0.2,
            height: 2.4,
            ..Default::default()
        });
    }

    let mut cache = SceneCache::new(&model).0;
    let mut state = 1u64;
    for _ in 0..60 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let idx = (state % model.walls.len() as u64) as usize;
        let mut next = model.clone();
        let delta = if (state & 1) == 0 { 0.15 } else { -0.15 };
        next.walls[idx].start.x += delta;
        next.walls[idx].end.x += delta;
        if idx < 30 {
            if let Some(opening) = next.openings.first_mut() {
                opening.width = (opening.width + 0.05).max(0.4);
            }
        }
        let stats = cache.update(&next);
        assert!(stats.rebuilt <= stats.objects, "rebuilt overcount: {stats:?}");
        assert_eq!(cache.scene().digest(), Scene3d::build(&next).digest(), "digest mismatch after random edit");
        model = next;
    }
}

#[test]
fn digest_ignores_mesh_order() {
    let mut model = ArchModel::default();
    model.walls.push(Wall { id: ArchId(0), start: Vec2::new(0.0, 0.0), end: Vec2::new(2.0, 0.0), thickness: 0.2, height: 2.4, ..Default::default() });
    let scene = Scene3d::build(&model);
    let mut swapped = scene.clone();
    swapped.meshes.reverse();
    assert_eq!(scene.digest(), swapped.digest());
}

#[test]
fn one_wall_edit_rebuilds_its_neighbours_and_their_openings_only() {
    let mut model = ArchModel::default();
    model.walls.push(Wall { id: ArchId(0), start: Vec2::new(0.0, 0.0), end: Vec2::new(4.0, 0.0), thickness: 0.2, height: 2.4, ..Default::default() });
    model.walls.push(Wall { id: ArchId(1), start: Vec2::new(4.0, 0.0), end: Vec2::new(4.0, 3.0), thickness: 0.2, height: 2.4, ..Default::default() });
    model.walls.push(Wall { id: ArchId(2), start: Vec2::new(0.0, 3.0), end: Vec2::new(4.0, 3.0), thickness: 0.2, height: 2.4, ..Default::default() });
    let mut opening = cadcraft_doc::arch::Opening::default();
    opening.id = ArchId(3);
    opening.host = ArchId(1);
    opening.offset = 1.0;
    opening.width = 0.8;
    opening.height = 1.8;
    model.openings.push(opening);

    let mut cache = SceneCache::new(&model).0;
    let mut edited = model.clone();
    edited.walls[1].start.x += 0.2;
    edited.walls[1].end.x += 0.2;
    let stats = cache.update(&edited);
    assert!(stats.rebuilt <= 6, "rebuilt too many objects: {stats:?}");
    assert_eq!(cache.scene().digest(), Scene3d::build(&edited).digest());
}

#[test]
fn three_hundred_walls_incremental_edit_under_10ms() {
    if cfg!(debug_assertions) {
        return;
    }
    let mut model = ArchModel::default();
    for r in 0..10 {
        for c in 0..15 {
            let x0 = c as f64;
            let x1 = c as f64 + 1.0;
            let y0 = r as f64;
            let y1 = r as f64 + 1.0;
            model.walls.push(Wall {
                id: ArchId((r * 15 + c) as u32),
                start: Vec2::new(x0, y0),
                end: Vec2::new(x1, y0),
                thickness: 0.15,
                height: 2.4,
                ..Default::default()
            });
            model.walls.push(Wall {
                id: ArchId((300 + r * 15 + c) as u32),
                start: Vec2::new(x0, y0),
                end: Vec2::new(x0, y1),
                thickness: 0.15,
                height: 2.4,
                ..Default::default()
            });
        }
    }
    let (mut cache, _) = SceneCache::new(&model);
    let mut edited = model.clone();
    edited.walls[0].end.x += 0.25;
    let t0 = Instant::now();
    let stats = cache.update(&edited);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    assert!(ms < 10.0, "incremental update took {ms:.2} ms; stats={stats:?}");
    assert_eq!(cache.scene().digest(), Scene3d::build(&edited).digest());
}
