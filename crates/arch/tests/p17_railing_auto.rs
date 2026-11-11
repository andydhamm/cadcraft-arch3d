use cadcraft_arch::railing_auto::{RailHostRef, auto_railings, regenerate};
use cadcraft_doc::arch::{RailSetting, Railing, RailingHost, Room, RoomType, Stair, StairShape};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::Vec2;

fn make_stair(id: u32, start: Vec2, direction: f64, width: f64, from_level: u32, to_level: u32, rise: f64) -> Stair {
    Stair {
        id: ArchId(id),
        start,
        direction,
        width,
        from_level,
        to_level,
        rise,
        shape: StairShape::Straight,
        risers: 16,
        riser_height: rise / 16.0,
        tread_depth: 0.254,
        treads: 15,
        run: 15.0 * 0.254,
        landing_depth: 1.0,
        well: Vec::new(),
        well_room: None,
        layer: "A-STAIR".into(),
        ..Stair::default()
    }
}

fn make_deck_room(id: u32, level: u32, polygon: Vec<Vec2>, height: f64) -> Room {
    Room {
        id: ArchId(id),
        name: "Deck".into(),
        room_type: RoomType::Deck,
        level,
        polygon,
        holes: Vec::new(),
        ceiling_height: Some(2.4),
        floor_height: Some(0.0),
        interior_area: 0.0,
        standard_area: 0.0,
        deck_height: Some(height),
        deck_rails: Vec::new(),
        ..Room::default()
    }
}

#[test]
fn stair_in_the_open_gets_two_side_railings() {
    let mut m = ArchModel::default();
    let stair = make_stair(0, Vec2::new(0.0, 0.0), 0.0, 1.0, 0, 1, 2.7);
    m.push_stair(stair);

    let rails = auto_railings(&m, RailHostRef::Stair(ArchId(0)));
    assert_eq!(rails.len(), 2);
    assert!(rails.iter().any(|r| r.host == RailingHost::StairLeft));
    assert!(rails.iter().any(|r| r.host == RailingHost::StairRight));
    assert!(rails.iter().all(|r| r.auto));
    assert!(rails.iter().all(|r| r.follows == Some(ArchId(0))));
}

#[test]
fn stair_against_a_wall_gets_one() {
    let mut m = ArchModel::default();
    let stair = make_stair(0, Vec2::new(0.0, 0.0), 0.0, 1.0, 0, 1, 2.7);
    m.push_stair(stair);

    // Add a wall along the right side (y = -0.5)
    // Stair right side is at y = -0.5
    let wall = cadcraft_doc::arch::Wall {
        id: ArchId(0),
        start: Vec2::new(-1.0, -0.5),
        end: Vec2::new(5.0, -0.5),
        thickness: 0.2,
        height: 2.4,
        base_level: 0,
        top_level: None,
        ..cadcraft_doc::arch::Wall::default()
    };
    m.push_wall(wall);

    let rails = auto_railings(&m, RailHostRef::Stair(ArchId(0)));
    // Should only get left railing
    assert_eq!(rails.len(), 1);
    assert_eq!(rails[0].host, RailingHost::StairLeft);
}

#[test]
fn rail_on_forces_a_wall_side() {
    let mut m = ArchModel::default();
    let mut stair = make_stair(0, Vec2::new(0.0, 0.0), 0.0, 1.0, 0, 1, 2.7);
    stair.rail_right = RailSetting::On;
    m.push_stair(stair);

    // Add a wall along the right side
    let wall = cadcraft_doc::arch::Wall {
        id: ArchId(0),
        start: Vec2::new(-1.0, -0.5),
        end: Vec2::new(5.0, -0.5),
        thickness: 0.2,
        height: 2.4,
        base_level: 0,
        top_level: None,
        ..cadcraft_doc::arch::Wall::default()
    };
    m.push_wall(wall);

    let rails = auto_railings(&m, RailHostRef::Stair(ArchId(0)));
    assert_eq!(rails.len(), 2);
    assert!(rails.iter().any(|r| r.host == RailingHost::StairRight));
}

#[test]
fn rail_off_removes_a_side() {
    let mut m = ArchModel::default();
    let mut stair = make_stair(0, Vec2::new(0.0, 0.0), 0.0, 1.0, 0, 1, 2.7);
    stair.rail_right = RailSetting::Off;
    m.push_stair(stair);

    let rails = auto_railings(&m, RailHostRef::Stair(ArchId(0)));
    assert_eq!(rails.len(), 1);
    assert_eq!(rails[0].host, RailingHost::StairLeft);
}

#[test]
fn deck_gets_its_free_edges() {
    let mut m = ArchModel::default();
    // Deck 4x4 at (0,0) to (4,4)
    let poly = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
    let room = make_deck_room(0, 0, poly, 0.5);
    m.push_room(room);

    let rails = auto_railings(&m, RailHostRef::Deck(ArchId(0)));
    // All edges free, so one closed ring
    assert_eq!(rails.len(), 1);
    assert_eq!(rails[0].host, RailingHost::Deck);
    assert_eq!(rails[0].path.len(), 5); // Closed ring
}

#[test]
fn free_standing_deck_gets_a_closed_ring() {
    let mut m = ArchModel::default();
    let poly = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
    let room = make_deck_room(0, 0, poly, 0.5);
    m.push_room(room);

    let rails = auto_railings(&m, RailHostRef::Deck(ArchId(0)));
    assert_eq!(rails.len(), 1);
    assert_eq!(rails[0].path.len(), 5);
}

#[test]
fn deck_rails_on_and_off_per_edge() {
    let mut m = ArchModel::default();
    let poly = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
    let mut room = make_deck_room(0, 0, poly, 0.5);
    // Edge 0 (bottom) off, others auto (free)
    room.deck_rails = vec![RailSetting::Off, RailSetting::Auto, RailSetting::Auto, RailSetting::Auto];
    m.push_room(room);

    let rails = auto_railings(&m, RailHostRef::Deck(ArchId(0)));
    // Should have one open path covering edges 1, 2, 3
    assert_eq!(rails.len(), 1);
    assert_eq!(rails[0].path.len(), 4); // 4 vertices for 3 edges
}

#[test]
fn deck_without_height_or_with_a_bad_polygon_gets_none() {
    let mut m = ArchModel::default();
    let poly = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
    let mut room = make_deck_room(0, 0, poly, 0.5);
    room.deck_height = None;
    m.push_room(room);

    let rails = auto_railings(&m, RailHostRef::Deck(ArchId(0)));
    assert!(rails.is_empty());
}

#[test]
fn stair_without_a_flight_gets_none() {
    let mut m = ArchModel::default();
    // Stair with invalid rise
    let stair = make_stair(0, Vec2::new(0.0, 0.0), 0.0, 1.0, 0, 1, 0.0);
    m.push_stair(stair);

    let rails = auto_railings(&m, RailHostRef::Stair(ArchId(0)));
    assert!(rails.is_empty());
}

#[test]
fn regenerate_keeps_drawn_railings_and_the_well_guard() {
    let mut m = ArchModel::default();
    let stair = make_stair(0, Vec2::new(0.0, 0.0), 0.0, 1.0, 0, 1, 2.7);
    m.push_stair(stair);

    // Add a drawn railing
    let drawn = Railing { host: RailingHost::Well, follows: Some(ArchId(0)), auto: false, ..Railing::default() };
    m.push_railing(drawn);

    let ids = regenerate(&mut m, RailHostRef::Stair(ArchId(0)));
    assert_eq!(ids.len(), 2); // Two auto railings
    assert_eq!(m.railings.len(), 3); // Two auto + one drawn
    assert!(m.railings.iter().any(|r| !r.auto && r.host == RailingHost::Well));
}

#[test]
fn an_old_drawing_keeps_its_railings() {
    let mut m = ArchModel::default();
    // Simulate an old railing (no host, auto false)
    let old = Railing {
        host: RailingHost::Well, // Default for old
        follows: None,
        auto: false,
        ..Railing::default()
    };
    m.push_railing(old);

    // Regenerate for a new stair
    let stair = make_stair(0, Vec2::new(0.0, 0.0), 0.0, 1.0, 0, 1, 2.7);
    m.push_stair(stair);

    let ids = regenerate(&mut m, RailHostRef::Stair(ArchId(0)));
    assert_eq!(ids.len(), 2);
    assert_eq!(m.railings.len(), 3); // Old + 2 new
    assert!(m.railings.iter().any(|r| r.follows.is_none()));
}

#[test]
fn stair_path_is_at_flight_edge_not_inset() {
    let mut m = ArchModel::default();
    // Phase-5 stair: start (2,1), +x direction, width 1.0
    let stair = make_stair(0, Vec2::new(2.0, 1.0), 0.0, 1.0, 0, 1, 2.7);
    m.push_stair(stair);

    let rails = auto_railings(&m, RailHostRef::Stair(ArchId(0)));
    assert_eq!(rails.len(), 2);

    // Left path at t = +0.5: (2, 1.5) to (5.81, 1.5)
    let left = rails.iter().find(|r| r.host == RailingHost::StairLeft).unwrap();
    assert!((left.path[0].x - 2.0).abs() < 1e-9);
    assert!((left.path[0].y - 1.5).abs() < 1e-9);
    assert!((left.path[1].x - 5.81).abs() < 1e-9);
    assert!((left.path[1].y - 1.5).abs() < 1e-9);
    assert_eq!(left.inset, -0.045);

    // Right path at t = -0.5: (2, 0.5) to (5.81, 0.5)
    let right = rails.iter().find(|r| r.host == RailingHost::StairRight).unwrap();
    assert!((right.path[0].x - 2.0).abs() < 1e-9);
    assert!((right.path[0].y - 0.5).abs() < 1e-9);
    assert!((right.path[1].x - 5.81).abs() < 1e-9);
    assert!((right.path[1].y - 0.5).abs() < 1e-9);
    assert_eq!(right.inset, 0.045);
}

#[test]
fn stair_with_both_sides_against_walls_gets_none() {
    let mut m = ArchModel::default();
    let stair = make_stair(0, Vec2::new(0.0, 0.0), 0.0, 1.0, 0, 1, 2.7);
    m.push_stair(stair);

    // Wall along left side (y = +0.5)
    m.push_wall(cadcraft_doc::arch::Wall {
        id: ArchId(0),
        start: Vec2::new(-1.0, 0.5),
        end: Vec2::new(5.0, 0.5),
        thickness: 0.2,
        height: 2.4,
        base_level: 0,
        top_level: None,
        ..cadcraft_doc::arch::Wall::default()
    });
    // Wall along right side (y = -0.5)
    m.push_wall(cadcraft_doc::arch::Wall {
        id: ArchId(1),
        start: Vec2::new(-1.0, -0.5),
        end: Vec2::new(5.0, -0.5),
        thickness: 0.2,
        height: 2.4,
        base_level: 0,
        top_level: None,
        ..cadcraft_doc::arch::Wall::default()
    });

    let rails = auto_railings(&m, RailHostRef::Stair(ArchId(0)));
    assert!(rails.is_empty());
}

#[test]
fn stair_with_both_rail_on_forces_both_sides() {
    let mut m = ArchModel::default();
    let mut stair = make_stair(0, Vec2::new(0.0, 0.0), 0.0, 1.0, 0, 1, 2.7);
    stair.rail_left = RailSetting::On;
    stair.rail_right = RailSetting::On;
    m.push_stair(stair);

    // Walls on both sides
    m.push_wall(cadcraft_doc::arch::Wall {
        id: ArchId(0),
        start: Vec2::new(-1.0, 0.5),
        end: Vec2::new(5.0, 0.5),
        thickness: 0.2,
        height: 2.4,
        base_level: 0,
        top_level: None,
        ..cadcraft_doc::arch::Wall::default()
    });
    m.push_wall(cadcraft_doc::arch::Wall {
        id: ArchId(1),
        start: Vec2::new(-1.0, -0.5),
        end: Vec2::new(5.0, -0.5),
        thickness: 0.2,
        height: 2.4,
        base_level: 0,
        top_level: None,
        ..cadcraft_doc::arch::Wall::default()
    });

    let rails = auto_railings(&m, RailHostRef::Stair(ArchId(0)));
    assert_eq!(rails.len(), 2);
    assert!(rails.iter().any(|r| r.host == RailingHost::StairLeft));
    assert!(rails.iter().any(|r| r.host == RailingHost::StairRight));
}

#[test]
fn deck_with_all_edges_off_gets_none() {
    let mut m = ArchModel::default();
    let poly = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
    let mut room = make_deck_room(0, 0, poly, 0.5);
    room.deck_rails = vec![RailSetting::Off; 4];
    m.push_room(room);

    let rails = auto_railings(&m, RailHostRef::Deck(ArchId(0)));
    assert!(rails.is_empty());
}

#[test]
fn deck_with_single_edge_on_gets_one_open_path() {
    let mut m = ArchModel::default();
    let poly = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
    let mut room = make_deck_room(0, 0, poly, 0.5);
    // Only edge 0 (bottom, from (0,0) to (4,0)) on
    room.deck_rails = vec![RailSetting::On, RailSetting::Off, RailSetting::Off, RailSetting::Off];
    m.push_room(room);

    let rails = auto_railings(&m, RailHostRef::Deck(ArchId(0)));
    assert_eq!(rails.len(), 1);
    // Path should be the bottom edge, pulled back at both ends (0.045 inset)
    assert_eq!(rails[0].path.len(), 2);
    assert!((rails[0].path[0].x - 0.045).abs() < 1e-9);
    assert!((rails[0].path[0].y - 0.0).abs() < 1e-9);
    assert!((rails[0].path[1].x - 3.955).abs() < 1e-9);
    assert!((rails[0].path[1].y - 0.0).abs() < 1e-9);
}

#[test]
fn stair_with_nan_width_gets_none() {
    let mut m = ArchModel::default();
    let mut stair = make_stair(0, Vec2::new(0.0, 0.0), 0.0, f64::NAN, 0, 1, 2.7);
    stair.width = f64::NAN;
    m.push_stair(stair);

    let rails = auto_railings(&m, RailHostRef::Stair(ArchId(0)));
    assert!(rails.is_empty());
}

#[test]
fn deck_with_nan_height_gets_none() {
    let mut m = ArchModel::default();
    let poly = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
    let mut room = make_deck_room(0, 0, poly, 0.5);
    room.deck_height = Some(f64::NAN);
    m.push_room(room);

    let rails = auto_railings(&m, RailHostRef::Deck(ArchId(0)));
    assert!(rails.is_empty());
}
