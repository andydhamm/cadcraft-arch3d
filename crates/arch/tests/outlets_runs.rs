use cadcraft_arch::outlets::usable_runs;
use cadcraft_arch::room::detect_rooms;
use cadcraft_doc::arch::{OpeningKind, RoomType};
use cadcraft_doc::{ArchId, ArchModel, Opening, Wall};
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

fn add(m: &mut ArchModel, host: ArchId, offset: f64, width: f64, kind: OpeningKind, sill: f64) {
    m.push_opening(Opening { host, offset, width, sill, kind, ..Default::default() });
}

fn room_id(m: &ArchModel) -> ArchId {
    m.rooms.first().map(|r| r.id).unwrap_or(ArchId(0))
}

#[test]
fn a_doorway_breaks_the_usable_wall() {
    let mut m = room64();
    add(&mut m, ArchId(0), 3.0, 0.9, OpeningKind::Door, 0.0);
    let runs = usable_runs(&m, room_id(&m));
    assert!(!runs.is_empty());
    let mut bottom = Vec::new();
    for (ri, r) in runs.iter().enumerate() {
        for &(w, _, a, b) in &r.wall_spans {
            if w == ArchId(0) {
                assert!(b <= 2.55 + 1e-9 || a >= 3.35 - 1e-9, "span {a}..{b} covers the doorway");
                bottom.push(ri);
            }
        }
    }
    bottom.dedup();
    assert!(bottom.len() >= 2, "bottom wall pieces must be separate runs: {bottom:?}");
}

#[test]
fn windows_do_not_break_the_wall() {
    let mut m = room64();
    add(&mut m, ArchId(2), 3.0, 1.2, OpeningKind::Window, 0.9);
    let runs = usable_runs(&m, room_id(&m));
    let spans: Vec<_> = runs.iter().flat_map(|r| r.wall_spans.iter()).filter(|s| s.0 == ArchId(2)).collect();
    assert_eq!(spans.len(), 1);
    assert!((spans[0].3 - spans[0].2 - 6.0).abs() < 1e-6, "{:?}", spans[0]);
}

#[test]
fn unknown_room_and_hostile_walls() {
    assert!(usable_runs(&ArchModel::default(), ArchId(7)).is_empty());
    let mut m = room64();
    m.push_wall(Wall { start: Vec2::new(f64::NAN, 0.0), end: Vec2::new(1.0, 1.0), thickness: f64::NAN, ..Default::default() });
    let _ = usable_runs(&m, room_id(&m));
}

#[test]
fn a_zero_length_wall_is_skipped() {
    let mut m = room64();
    m.push_wall(Wall { start: Vec2::new(3.0, 2.0), end: Vec2::new(3.0, 2.0), thickness: 0.2, ..Default::default() });
    let runs = usable_runs(&m, room_id(&m));
    for r in &runs {
        assert!(r.length > 0.0);
    }
}

#[test]
fn two_doors_create_multiple_runs() {
    let mut m = room64();
    add(&mut m, ArchId(0), 1.5, 0.9, OpeningKind::Door, 0.0);
    add(&mut m, ArchId(0), 4.5, 0.9, OpeningKind::Door, 0.0);
    let runs = usable_runs(&m, room_id(&m));
    let bottom_runs: Vec<_> = runs.iter().filter(|r| r.wall_spans.iter().any(|s| s.0 == ArchId(0))).collect();
    assert!(bottom_runs.len() >= 3, "two doors should create at least 3 runs: {}", bottom_runs.len());
}

#[test]
fn a_door_at_the_wall_edge() {
    let mut m = room64();
    add(&mut m, ArchId(0), 0.0, 0.9, OpeningKind::Door, 0.0);
    let runs = usable_runs(&m, room_id(&m));
    for r in &runs {
        assert!(r.length > 0.0);
    }
}

#[test]
fn a_door_larger_than_the_wall() {
    let mut m = room64();
    add(&mut m, ArchId(0), 3.0, 10.0, OpeningKind::Door, 0.0);
    let runs = usable_runs(&m, room_id(&m));
    let has_wall0 = runs.iter().any(|r| r.wall_spans.iter().any(|s| s.0 == ArchId(0)));
    assert!(!has_wall0, "oversized door should eliminate all usable wall on wall 0");
}

#[test]
fn a_room_with_no_openings_has_one_run() {
    let m = room64();
    let runs = usable_runs(&m, room_id(&m));
    assert_eq!(runs.len(), 1, "a closed room with no openings should have one continuous run");
}
