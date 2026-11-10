use cadcraft_arch::ceiling::overhead::{Overhead, wall_top_clip};
use cadcraft_arch::ceiling::{finished_ceiling_in, rough_ceiling_in};
use cadcraft_arch3d::{Scene3d, SceneCache};
use cadcraft_doc::arch::{ArchModel, Level, LevelKind, Room, RoomType, Wall};
use cadcraft_geom::Vec2;

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
    vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn split(open: bool) -> ArchModel {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, name: "Lower".into(), ..Level::default() });
    m.levels.push(Level { index: 1, name: "Entry".into(), elevation: 1.35, kind: LevelKind::Landing, ..Level::default() });
    m.levels.push(Level { index: 2, name: "Main".into(), elevation: 2.7, ..Level::default() });
    m.push_room(Room { polygon: rect(0.1, 0.1, 5.95, 2.95), level: 0, ..Room::default() });
    m.push_room(Room { polygon: rect(3.05, 0.1, 5.95, 2.95), level: 1, ..Room::default() });
    if open {
        m.push_room(Room { polygon: rect(3.05, 0.1, 5.95, 2.95), level: 2, room_type: RoomType::OpenBelow, ..Room::default() });
    }
    m.push_room(Room { polygon: rect(0.1, 0.1, 9.9, 7.9), level: 2, ..Room::default() });
    m
}

fn landing_wall() -> Wall {
    let mut w = Wall::new(Vec2::new(3.0, 0.0), Vec2::new(3.0, 3.0), 0.2);
    w.base_level = 1;
    w.height = 2.4;
    w
}

#[test]
fn landing_ceiling_and_walls_stop_at_the_floor_above() {
    let m = split(false);
    let lower = &m.rooms[0];
    let landing = &m.rooms[1];
    let p = Vec2::new(4.5, 1.5);
    assert!(near(rough_ceiling_in(&m, lower, p).unwrap(), 1.05));
    assert!(near(rough_ceiling_in(&m, landing, p).unwrap(), 2.4));
    assert!(near(wall_top_clip(&m, &landing_wall()).unwrap(), 2.7));
}

#[test]
fn open_below_room_opens_the_ceiling_under_it() {
    let m = split(true);
    let landing = &m.rooms[1];
    let p = Vec2::new(4.5, 1.5);
    assert!(matches!(cadcraft_arch::ceiling::overhead::overhead_at(&m, 1, p), Overhead::Open { level: 2, .. }));
    assert!(near(rough_ceiling_in(&m, landing, p).unwrap(), 5.1));
    assert!(near(finished_ceiling_in(&m, landing, p).unwrap(), 5.1));
    assert!(wall_top_clip(&m, &landing_wall()).is_none());
}

#[test]
fn stacked_levels_build_the_same_scene_as_before() {
    let mut base = ArchModel::default();
    base.levels.push(Level { index: 0, name: "Lower".into(), ..Level::default() });
    for poly in [rect(0.1, 0.1, 9.9, 2.95), rect(0.1, 3.05, 5.95, 7.9), rect(6.05, 3.05, 9.9, 7.9)] {
        base.push_room(Room { polygon: poly, level: 0, ..Room::default() });
    }
    let mut stacked = base.clone();
    stacked.levels.push(Level { index: 2, name: "Main".into(), elevation: 2.7, ..Level::default() });
    stacked.push_room(Room { polygon: rect(11.0, 0.1, 15.0, 7.9), level: 2, ..Room::default() });
    let before = Scene3d::build(&stacked).digest();
    let same = stacked.clone();
    assert_eq!(Scene3d::build(&same).digest(), before);
}

#[test]
fn incremental_rebuild_follows_an_upper_room_edit() {
    let model = split(false);
    let (mut cache, _) = SceneCache::new(&model);
    let mut edited = model.clone();
    edited.rooms[2].room_type = RoomType::OpenBelow;
    let stats = cache.update(&edited);
    assert!(stats.incremental, "expected incremental rebuild after an upper-room edit");
    assert_eq!(cache.scene().digest(), Scene3d::build(&edited).digest());
    assert!(matches!(cadcraft_arch::ceiling::overhead::overhead_at(&edited, 1, Vec2::new(4.5, 1.5)), Overhead::Open { .. }));
}
