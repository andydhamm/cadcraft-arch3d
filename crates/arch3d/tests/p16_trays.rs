use std::collections::BTreeSet;

use cadcraft_arch::ceiling::rough_ceiling_in;
use cadcraft_arch3d::{Mesh, ObjectKey, ObjectKind, Scene3d, SceneCache, changed_objects, probe};
use cadcraft_doc::arch::{Room, Tray, TraySides};
use cadcraft_doc::{ArchId, ArchModel, Roof, RoofDirective, Wall};
use cadcraft_geom::{Vec2, Vec3, point_in_polygon};

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
    vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
}

fn house(rise: Option<f64>) -> ArchModel {
    let mut m = ArchModel::default();
    let wall = |a: (f64, f64), b: (f64, f64), thickness: f64, exterior: bool| Wall {
        start: Vec2::new(a.0, a.1),
        end: Vec2::new(b.0, b.1),
        thickness,
        height: 2.4,
        exterior,
        ..Default::default()
    };
    for (a, b) in [((0.0, 0.0), (10.0, 0.0)), ((10.0, 0.0), (10.0, 8.0)), ((10.0, 8.0), (0.0, 8.0)), ((0.0, 8.0), (0.0, 0.0))] {
        m.push_wall(wall(a, b, 0.2, true));
    }
    m.push_wall(wall((0.0, 3.0), (10.0, 3.0), 0.1, false));
    m.push_wall(wall((6.0, 3.0), (6.0, 8.0), 0.1, false));
    m.push_room(Room { name: "South".into(), polygon: rect(0.1, 0.1, 9.9, 2.95), ..Default::default() });
    m.push_room(Room { name: "North-west".into(), polygon: rect(0.1, 3.05, 5.95, 7.9), ..Default::default() });
    m.push_room(Room { name: "North-east".into(), polygon: rect(6.05, 3.05, 9.9, 7.9), ..Default::default() });
    m.walls[1].roof_directive = RoofDirective::Gable;
    m.walls[3].roof_directive = RoofDirective::Gable;
    if let Some(rise) = rise {
        let seed = Roof { pitch_rise: rise, pitch_run: 12.0, overhang: 0.5, thickness: 0.2, ..Roof::default() };
        let built = cadcraft_arch::roof::build_roof(&m, 0, &seed).expect("the gable roof builds");
        m.push_roof(built);
    }
    m
}

fn ceiling_of(s: &Scene3d, room: u32) -> &Mesh {
    s.meshes.iter().find(|m| m.material == "ceiling" && m.object == ArchId(room)).expect("the room's ceiling mesh")
}

fn room_meshes<'a>(s: &'a Scene3d, room: u32) -> Vec<&'a Mesh> {
    s.meshes.iter().filter(|m| m.material == "ceiling" && m.object == ArchId(room)).collect()
}

fn room_top_at(s: &Scene3d, room: u32, p: Vec2) -> Option<f64> {
    room_meshes(s, room).into_iter().filter_map(|mesh| top_at(mesh, p)).max_by(|a, b| a.partial_cmp(b).unwrap())
}

fn top_at(mesh: &Mesh, p: Vec2) -> Option<f64> {
    let mut best: Option<f64> = None;
    for tri in mesh.indices.chunks(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| mesh.positions[i as usize].map(f64::from));
        let det = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        if det.abs() < 1e-12 {
            continue;
        }
        let u = ((p.x - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (p.y - a[1])) / det;
        let v = ((b[0] - a[0]) * (p.y - a[1]) - (p.x - a[0]) * (b[1] - a[1])) / det;
        if u < -1e-9 || v < -1e-9 || u + v > 1.0 + 1e-9 {
            continue;
        }
        let z = a[2] + u * (b[2] - a[2]) + v * (c[2] - a[2]);
        best = Some(best.map_or(z, |q| q.max(z)));
    }
    best
}

fn key(kind: ObjectKind, n: u32) -> ObjectKey {
    let mut k = ObjectKey { kind, ..Default::default() };
    k.id = ArchId(n);
    k
}

fn signature(s: &Scene3d) -> (String, Vec<(String, u32)>, [usize; 4]) {
    (s.digest(), s.meshes.iter().map(|m| (m.material.clone(), m.object.0)).collect(), [s.floors, s.ceilings, s.roofs, s.lights.len()])
}

fn grid(r: &Room) -> Vec<Vec2> {
    let mut out = Vec::new();
    for i in 0..25 {
        for j in 0..25 {
            let p = Vec2::new(0.0613 + 0.4013 * i as f64, 0.0719 + 0.3271 * j as f64);
            if point_in_polygon(&r.polygon, p) {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn tray_mesh_matches_the_point_query() {
    let mut m = house(Some(6.0));
    m.rooms[1].flat_ceiling = false;
    let room = m.rooms[1].id;
    let tray0 = m.push_tray(Tray { room, level: 0, inset: 0.6, depth: 0.3, ..Default::default() });
    let _nested = m.push_tray(Tray { room, level: 0, parent: Some(tray0), inset: 0.6, depth: 0.15, ..Default::default() });
    let s = Scene3d::build(&m);
    let room_mesh = ceiling_of(&s, room.0);
    for p in [Vec2::new(3.025, 5.475), Vec2::new(0.4, 5.0)] {
        let want = rough_ceiling_in(&m, &m.rooms[1], p).unwrap();
        let got = top_at(room_mesh, p).unwrap();
        assert!((got - want).abs() < 1e-5, "room at {p:?}: mesh top {got}, point query {want}");
    }
    let inner = Vec2::new(3.025, 5.475);
    let outer = Vec2::new(0.4, 5.0);
    let inner_top = top_at(room_mesh, inner).unwrap();
    let outer_top = top_at(room_mesh, outer).unwrap();
    assert!(inner_top > outer_top, "nested tray lifts the tray centre above the room's plain ceiling");
    assert!(
        rough_ceiling_in(&m, &m.rooms[1], inner).unwrap() > rough_ceiling_in(&m, &m.rooms[1], outer).unwrap(),
        "tray lift is above the plain ceiling"
    );
    assert!(!grid(&m.rooms[1]).is_empty());
}

#[test]
fn vertical_tray_has_side_bands() {
    let mut m = house(Some(6.0));
    m.rooms[1].flat_ceiling = false;
    let room = m.rooms[1].id;
    m.push_tray(Tray { room, level: 0, inset: 0.6, depth: 0.3, ..Default::default() });
    let s = Scene3d::build(&m);
    let room_meshes: Vec<&Mesh> = s.meshes.iter().filter(|mesh| mesh.object == room && mesh.material == "ceiling").collect();
    assert!(room_meshes.len() >= 2, "vertical trays add a side band in addition to the room ceiling");
    let main = room_meshes[0];
    let centre = Vec2::new(3.025, 5.475);
    let centre_top = top_at(main, centre).unwrap();
    assert!(centre_top > 2.4, "tray roof lifts the room surface above the plain ceiling");
    let band = room_meshes.iter().copied().find(|mesh| mesh.bounds().max[2] < 3.0 && top_at(*mesh, Vec2::new(0.71, 5.475)).is_some()).unwrap();
    let band_top = top_at(band, Vec2::new(0.71, 5.475)).unwrap();
    assert!(band_top > 2.4 && band_top < centre_top, "the side band sits below the tray roof and above the parent ceiling");
}

#[test]
fn pitched_tray_has_sloped_sides() {
    let mut m = house(Some(6.0));
    m.rooms[1].flat_ceiling = false;
    let room = m.rooms[1].id;
    m.push_tray(Tray { room, level: 0, inset: 0.6, depth: 0.3, sides: TraySides::Pitched(45.0), ..Default::default() });
    let s = Scene3d::build(&m);
    let room_mesh = ceiling_of(&s, room.0);
    let at_edge = Vec2::new(0.8, 5.475);
    let center = Vec2::new(3.0, 5.0);
    let edge_top = top_at(room_mesh, at_edge).unwrap();
    let center_top = top_at(room_mesh, center).unwrap();
    assert!(center_top > edge_top, "pitched-side ceilings rise inward");
    let ceiling_meshes: Vec<&Mesh> = s.meshes.iter().filter(|mesh| mesh.object == room && mesh.material == "ceiling").collect();
    assert_eq!(ceiling_meshes.len(), 1, "pitched trays add no side band meshes");
}

#[test]
fn tray_edit_rebuilds_only_its_room() {
    let mut m = house(Some(6.0));
    let room = m.rooms[1].id;
    m.rooms[1].flat_ceiling = false;
    m.push_tray(Tray { room, level: 0, inset: 0.6, depth: 0.3, ..Default::default() });
    let prev = m.clone();
    m.trays[0].depth = 0.4;
    assert_eq!(changed_objects(&prev, &m), Some(BTreeSet::from([key(ObjectKind::Room, room.0)])), "changing a tray depth only rebuilds its room");
    let (mut cache, _) = SceneCache::new(&prev);
    let stats = cache.update(&m);
    assert!(stats.incremental, "tray edits should be incremental: {stats:?}");
    assert_eq!(stats.rebuilt, 1, "one room object should be rebuilt: {stats:?}");
}

#[test]
fn cache_equals_full_after_tray_edits() {
    let mut m = house(Some(6.0));
    m.rooms[1].flat_ceiling = false;
    let room = m.rooms[1].id;
    let parent = m.push_tray(Tray { room, level: 0, inset: 0.6, depth: 0.3, ..Default::default() });
    m.push_tray(Tray { room, level: 0, parent: Some(parent), inset: 0.6, depth: 0.15, ..Default::default() });
    let (mut cache, _) = SceneCache::new(&m);
    let edits: Vec<Box<dyn Fn(&mut ArchModel)>> = vec![
        Box::new(|m: &mut ArchModel| m.trays[0].depth = 0.4),
        Box::new(|m: &mut ArchModel| m.trays[0].inset = 0.65),
        Box::new(|m: &mut ArchModel| {
            m.trays.pop();
        }),
    ];
    for edit in edits {
        edit(&mut m);
        let stats = cache.update(&m);
        assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)), "tray cache should match full build after {stats:?}");
        assert!(stats.incremental, "tray edits should remain incremental: {stats:?}");
    }
}

#[test]
fn hostile_trays_build_nothing() {
    let mut m = house(Some(6.0));
    m.rooms[1].flat_ceiling = false;
    let room = m.rooms[1].id;
    let bad_parent = m.push_tray(Tray { room, level: 0, inset: 0.6, depth: 0.3, ..Default::default() });
    m.push_tray(Tray { room, level: 0, inset: f64::NAN, depth: 0.3, ..Default::default() });
    m.push_tray(Tray { room, level: 0, parent: Some(bad_parent), inset: 0.6, depth: 0.15, ..Default::default() });
    m.push_tray(Tray { room, level: 0, inset: 0.6, depth: f64::INFINITY, ..Default::default() });
    let bad = m.push_tray(Tray {
        room,
        level: 0,
        polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(0.0, 2.0), Vec2::new(2.0, 2.0)],
        inset: 0.0,
        depth: 0.1,
        ..Default::default()
    });
    m.trays[bad.0 as usize].polygon = vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(0.0, 2.0), Vec2::new(2.0, 2.0)];
    let s = Scene3d::build(&m);
    let room_mesh = ceiling_of(&s, room.0);
    let p = Vec2::new(0.4, 5.0);
    let want = rough_ceiling_in(&m, &m.rooms[1], p).unwrap();
    let got = top_at(room_mesh, p).unwrap();
    assert!((got - want).abs() < 1e-5, "invalid trays are ignored and the plain room ceiling remains");
    assert!(s.meshes.iter().any(|mesh| mesh.object == room && mesh.material == "ceiling"), "a valid ceiling stays present");
}
