use cadcraft_arch::outlets::{auto_outlets, usable_runs};
use cadcraft_arch::room::detect_rooms;
use cadcraft_doc::arch::{Cabinet, CabinetKind, ElectricalDevice, ElectricalKind, RoomType, WallSide};
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
fn a_rectangular_room_has_no_point_farther_than_1_8_from_an_outlet() {
    let m = room64();
    let devs = auto_outlets(&m, room_id(&m));
    assert!(!devs.is_empty());
    for d in &devs {
        assert_eq!(d.kind, ElectricalKind::Duplex);
        assert_eq!(d.height, 0.3);
        assert!(!d.counter);
    }
    for run in usable_runs(&m, room_id(&m)) {
        let mut at = Vec::new();
        let mut cum = 0.0;
        for &(w, s, a, b) in &run.wall_spans {
            for d in devs.iter().filter(|d| d.wall == w && d.side == s && d.offset >= a - 1e-9 && d.offset <= b + 1e-9) {
                at.push(cum + d.offset - a);
            }
            cum += b - a;
        }
        assert!(!at.is_empty());
        let n = (run.length / 0.05).floor() as usize;
        for i in 0..=n {
            let p = i as f64 * 0.05;
            let nearest = at.iter().map(|q| (q - p).abs()).fold(f64::INFINITY, f64::min);
            assert!(nearest <= 1.8 + 1e-6, "point {p} is {nearest} from an outlet");
        }
    }
}

#[test]
fn existing_outlets_are_not_duplicated() {
    let mut m = room64();
    let first = ElectricalDevice { wall: ArchId(0), side: WallSide::Left, offset: 1.9, ..Default::default() };
    m.electrical.push(first.clone());
    let devs = auto_outlets(&m, room_id(&m));
    assert!(!devs.is_empty());
    for d in &devs {
        assert!(!(d.wall == first.wall && d.side == first.side && (d.offset - first.offset).abs() < 0.3), "{d:?}");
    }
}

#[test]
fn a_kitchen_counter_gets_gfci_at_most_1_2_apart() {
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
    assert_eq!(cadcraft_arch::casework_layout::counter_runs(&m).len(), 1);
    let devs = auto_outlets(&m, room_id(&m));
    let g: Vec<_> = devs.iter().filter(|d| d.kind == ElectricalKind::Gfci && d.wall == ArchId(0)).collect();
    assert!(g.len() >= 2, "{g:?}");
    // The run spans wall offsets 0.8 .. 2.3 on the bottom wall.
    let (s0, s1) = (0.8, 2.3);
    assert!(g.first().is_some_and(|d| (d.offset - s0).abs() <= 0.6 + 1e-6));
    assert!(g.last().is_some_and(|d| (s1 - d.offset).abs() <= 0.6 + 1e-6));
    for w in g.windows(2) {
        assert!(w[1].offset - w[0].offset <= 1.2 + 1e-6);
    }
    assert!(g.iter().all(|d| d.counter && d.kind == ElectricalKind::Gfci));
}

#[test]
fn hostile_unknown_room() {
    assert!(auto_outlets(&ArchModel::default(), ArchId(9)).is_empty());
}
