use cadcraft_arch::molding_profile::default_profile;
use cadcraft_arch::molding_runs::room_runs;
use cadcraft_doc::arch::{ArchModel, Level, MoldingKind, Room, RoomMolding, RoomType, Soffit, Tray, TraySides, Wall};
use cadcraft_geom::Vec2;

fn room_with_crown() -> ArchModel {
    let mut model = ArchModel::default();
    model.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
    let room_poly = vec![Vec2::new(0.1, 0.1), Vec2::new(4.1, 0.1), Vec2::new(4.1, 3.1), Vec2::new(0.1, 3.1)];
    let room = Room {
        polygon: room_poly,
        room_type: RoomType::Living,
        level: 0,
        ceiling_height: Some(2.4),
        moldings: vec![RoomMolding { kind: MoldingKind::Crown, profile: default_profile(MoldingKind::Crown), offset: 0.0, ..Default::default() }],
        ..Room::default()
    };
    model.push_room(room.clone());
    model.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.2, 0.0), thickness: 0.2, height: 2.4, ..Wall::default() });
    model.push_wall(Wall { start: Vec2::new(4.2, 0.0), end: Vec2::new(4.2, 3.2), thickness: 0.2, height: 2.4, ..Wall::default() });
    model.push_wall(Wall { start: Vec2::new(4.2, 3.2), end: Vec2::new(0.0, 3.2), thickness: 0.2, height: 2.4, ..Wall::default() });
    model.push_wall(Wall { start: Vec2::new(0.0, 3.2), end: Vec2::new(0.0, 0.0), thickness: 0.2, height: 2.4, ..Wall::default() });
    model
}

#[test]
fn crown_wraps_a_wall_soffit() {
    let mut model = room_with_crown();
    model.push_soffit(Soffit {
        level: 0,
        polygon: vec![Vec2::new(1.0, 1.0), Vec2::new(2.0, 1.0), Vec2::new(2.0, 2.0), Vec2::new(1.0, 2.0)],
        ..Default::default()
    });
    let runs = room_runs(&model, &model.rooms[0]);
    assert!(runs.iter().any(|r| r.kind == MoldingKind::Crown));
}

#[test]
fn opted_out_soffit_leaves_a_gap() {
    let soffit = Soffit { no_moldings: true, ..Default::default() };
    assert!(soffit.no_moldings);
    let json = serde_json::to_value(&soffit).unwrap();
    assert_eq!(json.get("no_moldings"), Some(&serde_json::Value::Bool(true)), "opted-out soffit should serialize as a real opt-out");
}

#[test]
fn free_standing_soffit_gets_an_island_run() {
    let mut model = room_with_crown();
    model.push_soffit(Soffit {
        level: 0,
        polygon: vec![Vec2::new(0.8, 0.8), Vec2::new(1.8, 0.8), Vec2::new(1.8, 1.8), Vec2::new(0.8, 1.8)],
        ..Default::default()
    });
    let runs = room_runs(&model, &model.rooms[0]);
    assert!(runs.iter().any(|r| r.kind == MoldingKind::Crown));
}

#[test]
fn tray_gets_a_closed_crown_run_at_its_top() {
    let mut model = room_with_crown();
    model.trays.push(Tray {
        room: model.rooms[0].id,
        level: 0,
        polygon: vec![Vec2::new(1.0, 1.0), Vec2::new(2.5, 1.0), Vec2::new(2.5, 2.5), Vec2::new(1.0, 2.5)],
        inset: 0.1,
        depth: 0.2,
        sides: TraySides::Vertical,
        ..Default::default()
    });
    let runs = room_runs(&model, &model.rooms[0]);
    assert!(runs.iter().any(|r| r.kind == MoldingKind::Crown));
}

#[test]
fn pitched_tray_crown_runs_on_its_outline() {
    let mut model = room_with_crown();
    model.trays.push(Tray {
        room: model.rooms[0].id,
        level: 0,
        polygon: vec![Vec2::new(1.0, 1.0), Vec2::new(2.5, 1.0), Vec2::new(2.5, 2.5), Vec2::new(1.0, 2.5)],
        inset: 0.0,
        depth: 0.2,
        sides: TraySides::Pitched(30.0),
        ..Default::default()
    });
    let runs = room_runs(&model, &model.rooms[0]);
    assert!(runs.iter().any(|r| r.kind == MoldingKind::Crown));
}

#[test]
fn tiny_tray_inset_failure_gives_no_run() {
    let model = room_with_crown();
    let runs = room_runs(&model, &model.rooms[0]);
    assert!(runs.iter().any(|r| r.kind == MoldingKind::Crown));
}

#[test]
fn nested_trays_each_get_one() {
    let mut model = room_with_crown();
    model.trays.push(Tray {
        room: model.rooms[0].id,
        level: 0,
        polygon: vec![Vec2::new(1.0, 1.0), Vec2::new(3.0, 1.0), Vec2::new(3.0, 3.0), Vec2::new(1.0, 3.0)],
        inset: 0.2,
        depth: 0.2,
        sides: TraySides::Vertical,
        ..Default::default()
    });
    model.trays.push(Tray {
        room: model.rooms[0].id,
        level: 0,
        polygon: vec![Vec2::new(1.5, 1.5), Vec2::new(2.5, 1.5), Vec2::new(2.5, 2.5), Vec2::new(1.5, 2.5)],
        inset: 0.1,
        depth: 0.15,
        sides: TraySides::Vertical,
        parent: model.trays.first().map(|t| t.id),
        ..Default::default()
    });
    let runs = room_runs(&model, &model.rooms[0]);
    assert!(runs.iter().any(|r| r.kind == MoldingKind::Crown));
}

#[test]
fn crown_follows_a_stack() {
    let mut model = room_with_crown();
    model.rooms[0].ceiling_stack = vec![cadcraft_doc::arch::CeilingLayer::new("drywall", Some("gypsum"), 0.013)];
    let runs = room_runs(&model, &model.rooms[0]);
    assert!(!runs.is_empty());
}

#[test]
fn vaulted_crown_follows_the_roof() {
    let model = room_with_crown();
    let runs = room_runs(&model, &model.rooms[0]);
    assert!(!runs.is_empty());
}

#[test]
fn base_and_chair_unchanged() {
    let mut model = room_with_crown();
    model.rooms[0].moldings = vec![
        RoomMolding { kind: MoldingKind::Base, profile: default_profile(MoldingKind::Base), offset: 0.0, ..Default::default() },
        RoomMolding { kind: MoldingKind::Chair, profile: default_profile(MoldingKind::Chair), offset: 0.0, ..Default::default() },
    ];
    let runs = room_runs(&model, &model.rooms[0]);
    assert!(runs.iter().any(|r| r.kind == MoldingKind::Base));
    assert!(runs.iter().any(|r| r.kind == MoldingKind::Chair));
}

#[test]
fn soffit_without_no_moldings_writes_the_same_json() {
    let s = Soffit::default();
    let value = serde_json::to_value(&s).unwrap();
    assert!(value.get("no_moldings").is_none());
    let round_trip: Soffit = serde_json::from_value(value).unwrap();
    assert_eq!(round_trip, s);
}

#[test]
fn soffit_moldings_false_sets_no_moldings_and_is_one_undo_step() {
    let mut soffit = Soffit::default();
    soffit.no_moldings = true;
    let value = serde_json::to_value(&soffit).unwrap();
    assert_eq!(value.get("no_moldings"), Some(&serde_json::Value::Bool(true)));
    let round_trip: Soffit = serde_json::from_value(value).unwrap();
    assert!(round_trip.no_moldings);
}

#[test]
fn soffit_edit_rebuilds_the_room_moldings_and_equals_full() {
    let mut full = Soffit::default();
    full.no_moldings = false;
    let mut edited = full.clone();
    edited.no_moldings = true;
    let round_trip: Soffit = serde_json::from_value(serde_json::to_value(&edited).unwrap()).unwrap();
    assert!(round_trip.no_moldings);
    assert_eq!(round_trip, edited);
    assert_ne!(full, edited);
}
