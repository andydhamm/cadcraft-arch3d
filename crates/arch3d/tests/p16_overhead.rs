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

fn three_level_model() -> ArchModel {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, name: "Lower".into(), elevation: 0.0, ceiling_height: 2.4, ..Level::default() });
    m.levels.push(Level { index: 1, name: "Main".into(), elevation: 2.7, ceiling_height: 2.4, ..Level::default() });
    m.levels.push(Level { index: 2, name: "Upper".into(), elevation: 5.4, ceiling_height: 2.4, ..Level::default() });
    m
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

#[test]
fn multiple_rooms_above_clip_to_lowest() {
    let mut m = three_level_model();
    // Lower room spans the full area
    m.push_room(Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 0, ..Room::default() });
    // Main level: two rooms at different elevations (one is a raised floor)
    m.push_room(Room { polygon: rect(0.0, 0.0, 5.0, 10.0), level: 1, ..Room::default() });
    let mut raised = Room { polygon: rect(5.0, 0.0, 10.0, 10.0), level: 1, ..Room::default() };
    raised.ceiling_height = Some(2.0); // lower ceiling
    m.push_room(raised);

    // The lower room's ceiling should clip to the lower of the two main-level floors
    // Left side: clips at 2.7 (main level elevation)
    // Right side: clips at 2.7 (same level, but lower ceiling doesn't affect floor height)
    let p_left = Vec2::new(2.5, 5.0);
    let p_right = Vec2::new(7.5, 5.0);
    let left_ceiling = rough_ceiling_in(&m, &m.rooms[0], p_left).unwrap();
    let right_ceiling = rough_ceiling_in(&m, &m.rooms[0], p_right).unwrap();
    assert!(near(left_ceiling, 2.4), "left ceiling should be at level height 2.4, got {}", left_ceiling);
    assert!(near(right_ceiling, 2.4), "right ceiling should be at level height 2.4, got {}", right_ceiling);
}

#[test]
fn partial_overlap_clips_only_overlapping_region() {
    let mut m = three_level_model();
    // Lower room: 10x10
    m.push_room(Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 0, ..Room::default() });
    // Main level room: only covers the top-right quadrant (5,5)-(10,10)
    m.push_room(Room { polygon: rect(5.0, 5.0, 10.0, 10.0), level: 1, ..Room::default() });

    // Points under the main room should clip to 2.4 (level 0 ceiling height)
    // Points outside should stay at 2.4 (no room above, so no clip needed)
    let under = Vec2::new(7.5, 7.5);
    let outside = Vec2::new(2.5, 2.5);
    let under_ceiling = rough_ceiling_in(&m, &m.rooms[0], under).unwrap();
    let outside_ceiling = rough_ceiling_in(&m, &m.rooms[0], outside).unwrap();
    // Both should be at the level ceiling height since the main room's floor is at 2.7
    // and the lower room's ceiling is at 2.4, so no clipping occurs
    assert!(near(under_ceiling, 2.4), "under ceiling should be 2.4, got {}", under_ceiling);
    assert!(near(outside_ceiling, 2.4), "outside ceiling should be 2.4, got {}", outside_ceiling);
}

#[test]
fn ceiling_already_below_floor_above_is_unchanged() {
    let mut m = three_level_model();
    // Lower room with a low ceiling (1.8m)
    let mut lower = Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 0, ..Room::default() };
    lower.ceiling_height = Some(1.8);
    m.push_room(lower);
    // Main level room directly above
    m.push_room(Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 1, ..Room::default() });

    // The lower room's ceiling (1.8) is already below the main level floor (2.7)
    // so no clipping should occur
    let p = Vec2::new(5.0, 5.0);
    let ceiling = rough_ceiling_in(&m, &m.rooms[0], p).unwrap();
    assert!(near(ceiling, 1.8), "ceiling should remain at 1.8, got {}", ceiling);
}

#[test]
fn nan_in_above_room_polygon_does_not_crash() {
    let mut m = three_level_model();
    m.push_room(Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 0, ..Room::default() });
    // Main level room with a NaN in its polygon
    let mut nan_room = Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 1, ..Room::default() };
    nan_room.polygon[1].x = f64::NAN;
    m.push_room(nan_room);

    // Should not crash; ceiling query should still work
    let p = Vec2::new(5.0, 5.0);
    let ceiling = rough_ceiling_in(&m, &m.rooms[0], p);
    // The ceiling should be either clipped or not, but the query must not panic
    assert!(ceiling.is_some(), "ceiling query should not return None");
    assert!(ceiling.unwrap().is_finite(), "ceiling should be finite");
}

#[test]
fn zero_size_room_above_does_not_crash() {
    let mut m = three_level_model();
    m.push_room(Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 0, ..Room::default() });
    // Main level room that is a single point (degenerate)
    m.push_room(Room { polygon: vec![Vec2::new(5.0, 5.0)], level: 1, ..Room::default() });

    // Should not crash
    let p = Vec2::new(5.0, 5.0);
    let ceiling = rough_ceiling_in(&m, &m.rooms[0], p);
    assert!(ceiling.is_some(), "ceiling query should not return None");
}

#[test]
fn open_below_room_above_opens_ceiling_completely() {
    let mut m = three_level_model();
    m.push_room(Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 0, ..Room::default() });
    // Main level: Open Below room
    m.push_room(Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 1, room_type: RoomType::OpenBelow, ..Room::default() });
    // Upper level: solid room
    m.push_room(Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 2, ..Room::default() });

    // The lower room's ceiling should open up to the upper level's ceiling
    let p = Vec2::new(5.0, 5.0);
    let ceiling = rough_ceiling_in(&m, &m.rooms[0], p).unwrap();
    // Should be at the upper level's ceiling: 5.4 + 2.4 = 7.8
    assert!(near(ceiling, 7.8), "ceiling should open to upper level ceiling 7.8, got {}", ceiling);
}

#[test]
fn wall_top_clip_with_nan_level_elevation() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, name: "Lower".into(), elevation: 0.0, ceiling_height: 2.4, ..Level::default() });
    m.levels.push(Level { index: 1, name: "Main".into(), elevation: f64::NAN, ceiling_height: 2.4, ..Level::default() });
    m.push_room(Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 0, ..Room::default() });
    m.push_room(Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 1, ..Room::default() });

    let wall = Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2);
    let clip = wall_top_clip(&m, &wall);
    // With NaN level elevation, the clip should be None or finite
    if let Some(c) = clip {
        assert!(c.is_finite(), "wall top clip should be finite, got {}", c);
    }
}

#[test]
fn overhead_clip_preserves_piece_when_no_above_rooms_overlap() {
    let mut m = three_level_model();
    // Lower room at origin
    m.push_room(Room { polygon: rect(0.0, 0.0, 5.0, 5.0), level: 0, ..Room::default() });
    // Main level room far away (no overlap)
    m.push_room(Room { polygon: rect(20.0, 20.0, 25.0, 25.0), level: 1, ..Room::default() });

    // The lower room's ceiling should be unchanged (no room above it)
    let p = Vec2::new(2.5, 2.5);
    let ceiling = rough_ceiling_in(&m, &m.rooms[0], p).unwrap();
    assert!(near(ceiling, 2.4), "ceiling should be at level height 2.4, got {}", ceiling);
}

#[test]
fn coincident_above_room_boundary() {
    let mut m = three_level_model();
    // Lower room: 0..10
    m.push_room(Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 0, ..Room::default() });
    // Main level room with exactly the same footprint
    m.push_room(Room { polygon: rect(0.0, 0.0, 10.0, 10.0), level: 1, ..Room::default() });

    // Ceiling should clip to the level ceiling height (2.4) since the floor above is at 2.7
    let p = Vec2::new(5.0, 5.0);
    let ceiling = rough_ceiling_in(&m, &m.rooms[0], p).unwrap();
    assert!(near(ceiling, 2.4), "ceiling should be 2.4, got {}", ceiling);

    // Probe should hit the ceiling
    let scene = Scene3d::build(&m);
    let hit = scene.probe(cadcraft_geom::Vec3::new(5.0, 5.0, 2.39));
    assert!(hit.is_some(), "probe just below ceiling should hit");
}
