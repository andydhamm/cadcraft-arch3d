use cadcraft_arch::outlets::{Gaps, outlet_gaps};
use cadcraft_arch::room::detect_rooms;
use cadcraft_doc::arch::{Cabinet, CabinetKind, ElectricalDevice, RoomType};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::Vec2;

fn room64() -> ArchModel {
    let mut m = ArchModel::default();
    let c = [(-0.1, -0.1), (6.1, -0.1), (6.1, 4.1), (-0.1, 4.1)];
    for i in 0..4 {
        let (a, b) = (c[i], c[(i + 1) % 4]);
        m.push_wall(Wall { start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness: 0.2, ..Default::default() });
    }
    m.rooms = detect_rooms(&m);
    if let Some(r) = m.rooms.first_mut() {
        r.room_type = RoomType::Bedroom;
        r.name = "Room".into();
    }
    m
}

fn room_id(m: &ArchModel) -> ArchId {
    m.rooms.first().map(|r| r.id).unwrap_or(ArchId(0))
}

#[test]
fn outlet_gaps_matches_the_rule() {
    let m = room64();
    let g = outlet_gaps(&m, room_id(&m));
    assert!(g.max_gap <= 1.8 + 1e-6, "{g:?}");
    assert!(g.max_gap > 0.0, "{g:?}");
    assert!(g.counter.is_none());

    // A room whose only wall run is long: with every wall removed from service, nothing serves it.
    let mut one = room64();
    one.electrical.clear();
    let g = outlet_gaps(&one, room_id(&one));
    assert!(g.max_gap <= 1.8 + 1e-6, "{g:?}");
}

#[test]
fn counter_gaps_report_the_kitchen() {
    let mut m = room64();
    if let Some(r) = m.rooms.first_mut() {
        r.room_type = RoomType::Kitchen;
    }
    for (x, w) in [(1.0, 0.6), (1.75, 0.9)] {
        let id = ArchId(0x50000 + m.cabinets.len() as u32);
        m.cabinets.push(Cabinet {
            id,
            kind: CabinetKind::Base,
            counter: true,
            position: Vec2::new(x, 0.0),
            rotation: 0.0,
            width: w,
            depth: 0.6,
            height: 0.9,
            ..Default::default()
        });
    }
    let g = outlet_gaps(&m, room_id(&m));
    let Some(c) = g.counter else { panic!("no counter gaps: {g:?}") };
    assert!(c.max_spacing <= 1.2 + 1e-6, "{c:?}");
    assert!(c.start_distance <= 0.6 + 1e-6, "{c:?}");
    assert!(c.end_distance <= 0.6 + 1e-6, "{c:?}");
}

#[test]
fn outlet_gaps_on_a_missing_room_is_zero() {
    assert_eq!(outlet_gaps(&ArchModel::default(), ArchId(3)), Gaps::default());
}

#[test]
fn outlet_gaps_kitchen_with_no_counters() {
    let mut m = room64();
    if let Some(r) = m.rooms.first_mut() {
        r.room_type = RoomType::Kitchen;
    }
    let g = outlet_gaps(&m, room_id(&m));
    let Some(c) = g.counter else { panic!("no counter gaps: {g:?}") };
    assert_eq!(c.max_spacing, 0.0);
    assert_eq!(c.start_distance, 0.0);
    assert_eq!(c.end_distance, 0.0);
}

#[test]
fn outlet_gaps_ignores_nan_offsets() {
    let mut m = room64();
    m.electrical.push(ElectricalDevice { offset: f64::NAN, ..Default::default() });
    let g = outlet_gaps(&m, room_id(&m));
    assert!(g.max_gap <= 1.8 + 1e-6, "{g:?}");
}

#[test]
fn outlet_gaps_counter_with_one_gfci() {
    let mut m = room64();
    if let Some(r) = m.rooms.first_mut() {
        r.room_type = RoomType::Kitchen;
    }
    let id = ArchId(0x50000 + m.cabinets.len() as u32);
    m.cabinets.push(Cabinet {
        id,
        kind: CabinetKind::Base,
        counter: true,
        position: Vec2::new(2.0, 0.0),
        rotation: 0.0,
        width: 0.6,
        depth: 0.6,
        height: 0.9,
        ..Default::default()
    });
    let g = outlet_gaps(&m, room_id(&m));
    let Some(c) = g.counter else { panic!("no counter gaps: {g:?}") };
    assert_eq!(c.max_spacing, 0.0);
    assert!(c.start_distance <= 0.6 + 1e-6, "{c:?}");
    assert!(c.end_distance <= 0.6 + 1e-6, "{c:?}");
}

#[test]
fn outlet_gaps_two_counter_runs() {
    let mut m = room64();
    if let Some(r) = m.rooms.first_mut() {
        r.room_type = RoomType::Kitchen;
    }
    for (x, y, w, rot) in [(1.0, 0.0, 0.9, 0.0), (0.0, 1.0, 0.9, 90.0)] {
        let id = ArchId(0x50000 + m.cabinets.len() as u32);
        m.cabinets.push(Cabinet {
            id,
            kind: CabinetKind::Base,
            counter: true,
            position: Vec2::new(x, y),
            rotation: rot,
            width: w,
            depth: 0.6,
            height: 0.9,
            ..Default::default()
        });
    }
    let g = outlet_gaps(&m, room_id(&m));
    let Some(c) = g.counter else { panic!("no counter gaps: {g:?}") };
    assert!(c.max_spacing <= 1.2 + 1e-6, "{c:?}");
    assert!(c.start_distance <= 0.6 + 1e-6, "{c:?}");
    assert!(c.end_distance <= 0.6 + 1e-6, "{c:?}");
}

#[test]
fn outlet_gaps_long_run() {
    let mut m = ArchModel::default();
    let c = [(-0.1, -0.1), (100.1, -0.1), (100.1, 4.1), (-0.1, 4.1)];
    for i in 0..4 {
        let (a, b) = (c[i], c[(i + 1) % 4]);
        m.push_wall(Wall { start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness: 0.2, ..Default::default() });
    }
    m.rooms = detect_rooms(&m);
    if let Some(r) = m.rooms.first_mut() {
        r.room_type = RoomType::Bedroom;
        r.name = "Long Room".into();
    }
    let g = outlet_gaps(&m, room_id(&m));
    assert!(g.max_gap <= 1.8 + 1e-6, "{g:?}");
}

fn room_of_length(len: f64) -> ArchModel {
    let mut m = ArchModel::default();
    let h = 1.5;
    let c = [(-0.1, -h), (-0.1 + len, -h), (-0.1 + len, h), (-0.1, h)];
    for i in 0..4 {
        let (a, b) = (c[i], c[(i + 1) % 4]);
        m.push_wall(Wall { start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness: 0.2, ..Default::default() });
    }
    m.rooms = detect_rooms(&m);
    if let Some(r) = m.rooms.first_mut() {
        r.room_type = RoomType::Bedroom;
        r.name = "Room".into();
    }
    m
}

#[test]
fn a_run_of_3_7_metres_gets_two_outlets() {
    let m = room_of_length(3.9);
    assert!(!m.rooms.is_empty(), "no room detected");
    let g = outlet_gaps(&m, room_id(&m));
    assert!(g.max_gap <= 1.8 + 1e-6, "{g:?}");
}

#[test]
fn duplex_positions_never_leave_a_gap_over_1_8() {
    for len in [0.3, 1.0, 3.6, 3.7, 4.0, 7.3, 12.1, 33.0, 100.2] {
        let m = room_of_length(len);
        let Some(_room) = m.rooms.first() else { continue };
        let g = outlet_gaps(&m, room_id(&m));
        assert!(g.max_gap <= 1.8 + 1e-6, "length {len}: {g:?}");
    }
}

#[test]
fn outlet_gaps_after_a_run_break_stays_under_1_8() {
    let m = room_of_length(1.0);
    assert!(!m.rooms.is_empty(), "no room detected");
    let g = outlet_gaps(&m, room_id(&m));
    assert!(g.max_gap <= 1.8 + 1e-6, "{g:?}");
}
