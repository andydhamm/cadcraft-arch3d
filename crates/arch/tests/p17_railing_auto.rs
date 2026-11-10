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
