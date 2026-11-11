use cadcraft_arch3d::{Scene3d, SceneCache};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{GLASS_THICKNESS, GLASS_WALL_HEIGHT, Opening, OpeningKind, Wall, WallSplit};
use cadcraft_geom::{Vec2, Vec3};

fn probe(scene: &Scene3d, x: f64, y: f64, z: f64) -> Option<(String, String)> {
    scene.probe(Vec3::new(x, y, z)).map(|h| (h.id, h.part))
}

#[test]
fn glass_wall_probes() {
    let mut model = ArchModel::default();
    let wall_id = model.push_wall(Wall {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(2.0, 0.0),
        thickness: GLASS_THICKNESS,
        height: GLASS_WALL_HEIGHT,
        wall_type: "Glass".to_string(),
        ..Wall::default()
    });
    let scene = Scene3d::build(&model);
    assert_eq!(probe(&scene, 1.0, 0.0, 1.0), Some((wall_id.to_string(), "glass".to_string())));
    assert_eq!(probe(&scene, 1.0, 0.0, 2.05), None);
    assert_eq!(probe(&scene, 1.0, 0.0, 2.2), None);
}

#[test]
fn pony_probes() {
    let mut model = ArchModel::default();
    let wall_id = model.push_wall(Wall {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(2.0, 0.0),
        thickness: 0.1,
        height: 2.0,
        wall_type: "Interior".to_string(),
        split: Some(WallSplit { height: 1.07, upper: "Glass".to_string(), upper_thickness: 0.01 }),
        ..Wall::default()
    });
    let scene = Scene3d::build(&model);
    assert_eq!(probe(&scene, 1.0, 0.0, 0.5), Some((wall_id.to_string(), "wall".to_string())));
    assert_eq!(probe(&scene, 1.0, 0.0, 1.5), Some((wall_id.to_string(), "glass".to_string())));
    assert_eq!(probe(&scene, 1.0, 0.0, 2.2), None);
    assert_eq!(probe(&scene, 1.0, 0.03, 0.5), Some((wall_id.to_string(), "wall".to_string())));
    assert_eq!(probe(&scene, 1.0, 0.03, 1.5), None);
}

#[test]
fn glass_mesh_not_split_by_finishes_or_paints() {
    let mut model = ArchModel::default();
    let wall_id = model.push_wall(Wall {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(2.0, 0.0),
        thickness: GLASS_THICKNESS,
        height: GLASS_WALL_HEIGHT,
        wall_type: "Glass".to_string(),
        ..Wall::default()
    });
    let scene = Scene3d::build(&model);
    let meshes: Vec<_> = scene.meshes.iter().filter(|m| m.object == wall_id && !m.material.is_empty()).collect();
    assert!(!meshes.is_empty());
    assert!(meshes.iter().all(|m| m.material == "glass-wall"));
    assert!(meshes.iter().all(|m| !m.material.starts_with("wall-")));
}

#[test]
fn openings_cut_both_parts() {
    let mut model = ArchModel::default();
    let wall_id = model.push_wall(Wall {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(2.0, 0.0),
        thickness: 0.1,
        height: 2.0,
        wall_type: "Interior".to_string(),
        split: Some(WallSplit { height: 1.0, upper: "Glass".to_string(), upper_thickness: 0.01 }),
        ..Wall::default()
    });
    let mut opening = Opening::new(wall_id, 0.5, 0.8, 1.6, OpeningKind::Window);
    opening.sill = 0.2;
    model.push_opening(opening);
    let scene = Scene3d::build(&model);
    assert_eq!(probe(&scene, 0.8, 0.02, 0.5), None);
    assert_eq!(probe(&scene, 0.8, 0.02, 1.3), None);
}

#[test]
fn split_edit_rebuilds_only_the_wall_and_its_openings_and_equals_full() {
    let mut model = ArchModel::default();
    let wall_id = model.push_wall(Wall {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(2.0, 0.0),
        thickness: 0.1,
        height: 2.0,
        wall_type: "Interior".to_string(),
        split: Some(WallSplit { height: 1.0, upper: "Glass".to_string(), upper_thickness: 0.01 }),
        ..Wall::default()
    });
    let mut opening = Opening::new(wall_id, 0.5, 0.8, 1.2, OpeningKind::Window);
    opening.sill = 0.3;
    model.push_opening(opening);

    let (mut cache, _) = SceneCache::new(&model);
    let mut edited = model.clone();
    edited.walls[0].split.as_mut().unwrap().height = 1.2;
    edited.openings[0].width = 0.7;
    let stats = cache.update(&edited);
    assert!(stats.rebuilt <= 2, "rebuilt too many objects: {stats:?}");
    assert_eq!(cache.scene().digest(), Scene3d::build(&edited).digest());
}
