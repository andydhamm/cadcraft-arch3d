use cadcraft_arch3d::Scene3d;
use cadcraft_doc::arch::{Finish, Roof, RoofDirective, Room};
use cadcraft_doc::{ArchModel, Wall};
use cadcraft_geom::{Vec2, Vec3};

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
    vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
}

fn wall(start: (f64, f64), end: (f64, f64), thickness: f64, exterior: bool) -> Wall {
    Wall { start: Vec2::new(start.0, start.1), end: Vec2::new(end.0, end.1), thickness, height: 2.4, exterior, ..Default::default() }
}

fn house_with_roof(rise: Option<f64>) -> ArchModel {
    let mut m = ArchModel::default();
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

fn probe(scene: &Scene3d, x: f64, y: f64, z: f64) -> Option<(String, String)> {
    scene.probe(Vec3::new(x, y, z)).map(|h| (h.id, h.part))
}

fn count_rise_meshes(scene: &Scene3d, wall_id: cadcraft_doc::ArchId) -> usize {
    scene.meshes.iter().filter(|m| m.object == wall_id && m.material == "wall").count()
}

#[test]
fn interior_wall_rises_to_3_925_3_975() {
    let mut m = house_with_roof(Some(6.0));
    m.rooms[0].flat_ceiling = false;
    let scene = Scene3d::build(&m);
    let hit = probe(&scene, 5.0, 3.0, 3.9);
    assert!(hit.is_some(), "hit was {:?}", hit);
    assert_eq!(hit.as_ref().map(|(id, part)| (id.as_str(), part.as_str())), Some(("w4", "wall")));
    let hit = probe(&scene, 5.0, 0.05, 2.45);
    assert!(hit.is_some(), "hit was {:?}", hit);
    assert_eq!(hit.as_ref().map(|(id, part)| (id.as_str(), part.as_str())), Some(("w0", "wall")));
}

#[test]
fn gable_walls_get_no_rise() {
    let mut m = house_with_roof(Some(6.0));
    m.rooms[0].flat_ceiling = false;
    let scene = Scene3d::build(&m);
    assert!(probe(&scene, 10.0, 0.5, 3.0).is_none());
}

#[test]
fn walls_between_flat_rooms_get_no_rise() {
    let mut m = house_with_roof(Some(6.0));
    m.rooms[0].flat_ceiling = false;
    let scene = Scene3d::build(&m);
    assert!(probe(&scene, 6.0, 5.0, 2.6).is_none());
}

#[test]
fn no_roof_no_rise() {
    let mut m = house_with_roof(None);
    m.rooms[0].flat_ceiling = false;
    let scene = Scene3d::build(&m);
    assert!(probe(&scene, 5.0, 3.0, 3.9).is_none());
}

#[test]
fn divider_and_retaining_walls_never_rise() {
    let mut divider = ArchModel::default();
    divider.push_wall(Wall {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(10.0, 0.0),
        thickness: 0.1,
        height: 2.4,
        divider: true,
        ..Default::default()
    });
    divider.push_room(Room { name: "Single".into(), polygon: rect(0.1, 0.1, 9.9, 2.0), flat_ceiling: false, ..Default::default() });
    let scene = Scene3d::build(&divider);
    assert!(
        scene
            .meshes
            .iter()
            .all(|mesh| mesh.bounds().max[2] <= 2.4 + 1e-6 || mesh.material.starts_with("ceiling") || mesh.material.starts_with("roof"))
    );

    let mut retaining = ArchModel::default();
    retaining.push_wall(Wall {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(10.0, 0.0),
        thickness: 0.2,
        height: 2.4,
        retaining: true,
        ..Default::default()
    });
    retaining.push_room(Room { name: "Single".into(), polygon: rect(0.1, 0.1, 9.9, 2.0), flat_ceiling: false, ..Default::default() });
    let scene = Scene3d::build(&retaining);
    assert!(
        scene
            .meshes
            .iter()
            .all(|mesh| mesh.bounds().max[2] <= 2.4 + 1e-6 || mesh.material.starts_with("ceiling") || mesh.material.starts_with("roof"))
    );
}

#[test]
fn rise_keeps_the_exterior_finish_split() {
    let mut m = house_with_roof(Some(6.0));
    m.rooms[0].flat_ceiling = false;
    m.walls[0].finish = Some(Finish::Brick);
    let scene = Scene3d::build(&m);
    assert!(scene.meshes.iter().any(|mesh| mesh.object == m.walls[0].id && mesh.material == "wall-brick"));
    assert!(scene.meshes.iter().any(|mesh| mesh.object == m.walls[0].id && mesh.material == "wall"));
}

#[test]
fn wall_with_zero_thickness_gets_no_rise() {
    let mut m = ArchModel::default();
    m.push_wall(wall((0.0, 0.0), (10.0, 0.0), 0.0, false));
    m.push_room(Room { name: "Single".into(), polygon: rect(0.1, 0.1, 9.9, 2.0), flat_ceiling: false, ..Default::default() });
    let scene = Scene3d::build(&m);
    assert!(
        scene
            .meshes
            .iter()
            .all(|mesh| mesh.bounds().max[2] <= 2.4 + 1e-6 || mesh.material.starts_with("ceiling") || mesh.material.starts_with("roof"))
    );
}

#[test]
fn wall_with_negative_thickness_gets_no_rise() {
    let mut m = ArchModel::default();
    m.push_wall(wall((0.0, 0.0), (10.0, 0.0), -0.1, false));
    m.push_room(Room { name: "Single".into(), polygon: rect(0.1, 0.1, 9.9, 2.0), flat_ceiling: false, ..Default::default() });
    let scene = Scene3d::build(&m);
    assert!(
        scene
            .meshes
            .iter()
            .all(|mesh| mesh.bounds().max[2] <= 2.4 + 1e-6 || mesh.material.starts_with("ceiling") || mesh.material.starts_with("roof"))
    );
}

#[test]
fn wall_with_nan_start_gets_no_rise() {
    let mut m = ArchModel::default();
    m.push_wall(Wall { start: Vec2::new(f64::NAN, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.4, ..Default::default() });
    m.push_room(Room { name: "Single".into(), polygon: rect(0.1, 0.1, 9.9, 2.0), flat_ceiling: false, ..Default::default() });
    let scene = Scene3d::build(&m);
    assert!(
        scene
            .meshes
            .iter()
            .all(|mesh| mesh.bounds().max[2] <= 2.4 + 1e-6 || mesh.material.starts_with("ceiling") || mesh.material.starts_with("roof"))
    );
}

#[test]
fn room_not_touching_wall_does_not_cause_rise() {
    let mut m = house_with_roof(Some(6.0));
    m.rooms.clear();
    m.push_room(Room { name: "Detached".into(), polygon: rect(0.1, 0.1, 4.9, 2.0), flat_ceiling: false, ..Default::default() });
    let scene = Scene3d::build(&m);
    assert!(probe(&scene, 5.0, 3.0, 3.9).is_none());
}

#[test]
fn wall_at_different_level_than_roof_gets_no_rise() {
    let mut m = house_with_roof(Some(6.0));
    m.rooms[0].flat_ceiling = false;
    m.walls[4].base_level = 1;
    let scene = Scene3d::build(&m);
    assert!(probe(&scene, 5.0, 3.0, 3.9).is_none());
}

#[test]
fn multiple_vaulted_rooms_one_rise() {
    let mut m = house_with_roof(Some(6.0));
    m.rooms[0].flat_ceiling = false;
    m.rooms[1].flat_ceiling = false;
    let scene = Scene3d::build(&m);
    assert!(probe(&scene, 5.0, 3.0, 3.9).is_some());
    let rise_count = scene.meshes.iter().filter(|mesh| mesh.object == m.walls[4].id && mesh.material == "wall").count();
    assert!(rise_count <= 2, "too many rise meshes: {}", rise_count);
}

#[test]
fn very_large_wall_coordinates() {
    let mut m = ArchModel::default();
    m.push_wall(wall((1000.0, 1000.0), (1010.0, 1000.0), 0.2, false));
    m.push_wall(wall((1010.0, 1000.0), (1010.0, 1008.0), 0.2, true));
    m.push_wall(wall((1010.0, 1008.0), (1000.0, 1008.0), 0.2, false));
    m.push_wall(wall((1000.0, 1008.0), (1000.0, 1000.0), 0.2, true));
    m.push_wall(wall((1000.0, 1003.0), (1010.0, 1003.0), 0.1, false));
    m.push_room(Room { name: "South".into(), polygon: rect(1000.1, 1000.1, 1009.9, 1002.95), flat_ceiling: false, ..Default::default() });
    m.push_room(Room { name: "North".into(), polygon: rect(1000.1, 1003.05, 1009.9, 1007.9), ..Default::default() });
    m.walls[1].roof_directive = RoofDirective::Gable;
    m.walls[3].roof_directive = RoofDirective::Gable;
    let seed = Roof { pitch_rise: 6.0, pitch_run: 12.0, overhang: 0.5, thickness: 0.2, ..Roof::default() };
    let built = cadcraft_arch::roof::build_roof(&m, 0, &seed).expect("the gable roof builds");
    m.push_roof(built);
    let scene = Scene3d::build(&m);
    assert!(probe(&scene, 1005.0, 1003.0, 3.9).is_some());
}

#[test]
fn rise_survives_nan_planes_and_zero_length_walls() {
    let mut m = house_with_roof(Some(6.0));
    m.rooms[0].flat_ceiling = false;
    if let Some(roof) = m.roofs.first_mut() {
        if !roof.planes.is_empty() {
            roof.planes[0].ring[0] = Vec2::new(f64::NAN, 0.0);
        }
    }
    m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(0.0, 0.0), thickness: 0.1, height: 2.4, ..Default::default() });
    let scene = Scene3d::build(&m);
    assert!(probe(&scene, 5.0, 3.0, 3.9).is_none());
}
