//! Phase 15: electrical cover plates in the scene, the cache and the probe.

use cadcraft_arch::electrical::place_device;
use cadcraft_arch3d::{Scene3d, SceneCache};
use cadcraft_doc::arch::{ElectricalDevice, Level, Room, WallSide};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::{Vec2, Vec3};

fn wall(id: u32, a: [f64; 2], b: [f64; 2]) -> Wall {
    Wall { id: ArchId(id), start: Vec2::new(a[0], a[1]), end: Vec2::new(b[0], b[1]), thickness: 0.2, height: 2.4, ..Default::default() }
}

/// Four 0.2 m walls round a 6 x 4 m room (inner faces 0..6 x 0..4), one room, one level at 0.
fn room() -> ArchModel {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    m.walls = vec![
        wall(0, [-0.1, -0.1], [6.1, -0.1]),
        wall(1, [6.1, -0.1], [6.1, 4.1]),
        wall(2, [6.1, 4.1], [-0.1, 4.1]),
        wall(3, [-0.1, 4.1], [-0.1, -0.1]),
    ];
    m.next_id = 4;
    m.rooms.push(Room {
        id: ArchId(0),
        polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
        ..Default::default()
    });
    m.next_room_id = 1;
    m
}

fn device(wall: u32, offset: f64) -> ElectricalDevice {
    ElectricalDevice { wall: ArchId(wall), side: WallSide::Left, offset, height: 0.3, ..Default::default() }
}

fn assert_same(cached: &Scene3d, full: &Scene3d, what: &str) {
    assert_eq!(cached.digest(), full.digest(), "{what}: digest");
    let owners = |s: &Scene3d| s.meshes.iter().map(|m| (m.material.clone(), m.object)).collect::<Vec<_>>();
    assert_eq!(owners(cached), owners(full), "{what}: meshes");
    assert_eq!(cached.electrical, full.electrical, "{what}: electrical");
    assert_eq!(cached.electrical_boxes, full.electrical_boxes, "{what}: electrical boxes");
    assert_eq!(cached.materials, full.materials, "{what}: materials");
    let b = |s: &Scene3d| s.meshes.iter().map(|m| m.bounds()).collect::<Vec<_>>();
    assert_eq!(b(cached), b(full), "{what}: bounds");
}

fn plates(s: &Scene3d) -> Vec<&cadcraft_arch3d::Mesh> {
    s.meshes.iter().filter(|m| m.material.starts_with("electrical:")).collect()
}

#[test]
fn plates_probe_as_electrical() {
    let mut model = room();
    let id = model.push_electrical(device(0, 1.0));
    let scene = Scene3d::build(&model);
    assert_eq!(scene.electrical, 1);
    assert_eq!(scene.electrical_boxes.len(), 1);

    let Ok(place) = place_device(&model, &model.electrical[0]) else {
        assert!(false, "the device places");
        return;
    };
    let at = |proud: f64, dz: f64| Vec3::new(place.at.x + place.normal.x * proud, place.at.y + place.normal.y * proud, place.z + dz);

    let hit = scene.probe(at(0.003, 0.0));
    assert!(hit.as_ref().is_some_and(|h| h.part == "electrical" && h.id == "el0"), "{hit:?}");
    assert_eq!(id, ArchId(0));

    let far = scene.probe(at(0.007, 0.0));
    assert!(far.as_ref().is_none_or(|h| h.part != "electrical"), "{far:?}");
    let above = scene.probe(at(0.003, 1.2));
    assert!(above.as_ref().is_none_or(|h| h.part != "electrical"), "{above:?}");

    let inside = scene.probe(at(-0.1, 0.0));
    assert!(inside.as_ref().is_some_and(|h| h.part == "wall"), "{inside:?}");

    // A device on a wall that does not exist adds nothing.
    model.push_electrical(device(99, 1.0));
    let scene = Scene3d::build(&model);
    assert_eq!(scene.electrical, 1);
    assert_eq!(scene.electrical_boxes.len(), 1);
    assert_eq!(plates(&scene).len(), 1);
}

#[test]
fn the_cache_rebuilds_a_moved_plate_only() {
    let mut model = room();
    model.push_electrical(device(0, 1.0));
    model.push_electrical(device(0, 3.0));
    let (mut cache, _) = SceneCache::new(&model);
    assert_same(cache.scene(), &Scene3d::build(&model), "initial");

    let mut next = model.clone();
    next.electrical[1].offset += 0.2;
    let stats = cache.update(&next);
    assert_eq!(stats.rebuilt, 1, "{stats:?}");
    assert_same(cache.scene(), &Scene3d::build(&next), "moved");
}

#[test]
fn a_removed_device_leaves_no_mesh() {
    let mut model = room();
    model.push_electrical(device(0, 1.0));
    model.push_electrical(device(0, 3.0));
    let (mut cache, _) = SceneCache::new(&model);
    assert_eq!(plates(cache.scene()).len(), 2);

    let mut next = model.clone();
    next.electrical.remove(0);
    cache.update(&next);
    let found = plates(cache.scene());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].object, ArchId(1));
    assert_same(cache.scene(), &Scene3d::build(&next), "removed");
}
