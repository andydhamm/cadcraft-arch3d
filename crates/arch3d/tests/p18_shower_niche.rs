use cadcraft_arch3d::{Scene3d, SceneCache};
use cadcraft_doc::arch::paint::{MaterialRef, Paint, Side, Surface};
use cadcraft_doc::arch::{GLASS_THICKNESS, Opening, OpeningKind, Wall};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::{Vec2, Vec3};

fn probe(scene: &Scene3d, x: f64, y: f64, z: f64) -> Option<(String, String)> {
    scene.probe(Vec3::new(x, y, z)).map(|h| (h.id, h.part))
}

/// The phase-1 north wall w2, (10,8) to (0,8), 0.2 thick, its room side (y = 7.9) its left face.
fn north_wall() -> (ArchModel, ArchId) {
    let mut model = ArchModel::default();
    let id = model.push_wall(Wall { start: Vec2::new(10.0, 8.0), end: Vec2::new(0.0, 8.0), thickness: 0.2, height: 2.7, ..Wall::default() });
    (model, id)
}

fn niche(host: ArchId, offset: f64) -> Opening {
    Opening {
        host,
        offset,
        width: 0.6,
        height: 0.9,
        sill: 1.0,
        depth: 0.1,
        face: Some(Side::Left),
        kind: OpeningKind::Niche,
        style: "niche".into(),
        ..Opening::default()
    }
}

/// A 2.0 m glass wall with a 0.7 x 1.9 shower door at its middle.
fn shower() -> (ArchModel, ArchId, ArchId) {
    let mut model = ArchModel::default();
    let wall = model.push_wall(Wall {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(2.0, 0.0),
        thickness: GLASS_THICKNESS,
        height: 2.0,
        wall_type: "Glass".to_string(),
        ..Wall::default()
    });
    let door = model.push_opening(Opening {
        host: wall,
        offset: 1.0,
        width: 0.7,
        height: 1.9,
        sill: 0.0,
        kind: OpeningKind::Door,
        style: "shower".into(),
        rough_width: 0.7,
        rough_height: 1.9,
        ..Opening::default()
    });
    (model, wall, door)
}

#[test]
fn niche_probes() {
    let (mut model, w) = north_wall();
    model.push_opening(niche(w, 6.0));
    let scene = Scene3d::build(&model);
    let wall = (w.to_string(), "wall".to_string());
    assert_eq!(probe(&scene, 4.0, 7.95, 1.45), None);
    assert_eq!(probe(&scene, 4.0, 8.05, 1.45), Some(wall.clone()));
    assert_eq!(probe(&scene, 4.0, 7.95, 0.95), Some(wall.clone()));
    assert_eq!(probe(&scene, 3.65, 7.95, 1.45), Some(wall.clone()));
    assert_eq!(probe(&scene, 4.33, 7.95, 1.45), Some(wall.clone()));
    assert_eq!(probe(&scene, 4.0, 7.95, 1.93), Some(wall.clone()));
    // The 1 mm skin on the recess bottom is the wall's.
    assert_eq!(probe(&scene, 4.0, 7.9995, 1.45), Some(wall));
}

#[test]
fn back_block_takes_the_wall_finish_and_paint() {
    let (mut model, w) = north_wall();
    model.push_opening(niche(w, 6.0));
    let plain = Scene3d::build(&model);
    // Without paint the back block is part of a "wall" mesh.
    let in_block = |m: &cadcraft_arch3d::Mesh| {
        m.positions
            .iter()
            .any(|p| (f64::from(p[1]) - 8.0).abs() < 1e-4 && (f64::from(p[0]) - 4.0).abs() < 0.3 && f64::from(p[2]) > 1.0 && f64::from(p[2]) < 1.9)
    };
    assert!(plain.meshes.iter().any(|m| m.material == "wall" && in_block(m)), "the block is wall");
    model.paints.insert(Surface::WallSide { wall: w, side: Side::Left }.key(), Paint { material: None, tint: Some([200, 30, 30]) });
    let painted = Scene3d::build(&model);
    // The painted face is split off the wall mesh around the niche; the block stays in the wall's
    // own mesh, which the paint split sees like any other wall solid.
    assert!(painted.meshes.iter().any(|m| m.material == "paint:wall/w0/left"), "the face is painted");
    assert!(painted.meshes.iter().any(|m| m.material == "wall" && m.object == w && in_block(m)), "the block stays wall");
    assert!(painted.meshes.iter().all(|m| m.material != "wall" || m.object == w));
    assert_eq!(probe(&painted, 4.0, 8.05, 1.45), Some((w.to_string(), "wall".to_string())));
    assert_eq!(probe(&painted, 4.0, 7.95, 1.45), None);
}

#[test]
fn back_skin_material_from_the_niche() {
    let (mut model, w) = north_wall();
    let mut n = niche(w, 6.0);
    n.material = Some("#c04020".to_string());
    let id = model.push_opening(n);
    let scene = Scene3d::build(&model);
    let key = format!("niche-back:o{}", id.0);
    let skin: Vec<_> = scene.meshes.iter().filter(|m| m.material == key).collect();
    assert_eq!(skin.len(), 1);
    assert_eq!(skin[0].object, w);
    let def = scene.materials.get(&key).expect("the niche's material definition");
    let want = MaterialRef::parse("#c04020").map(|m| m.key());
    assert_eq!(want.as_deref(), Some("colour:#c04020"));
    assert!(def.base_color[0] > def.base_color[2]);
    // No material: a skin, but no definition.
    model.openings[0].material = None;
    let bare = Scene3d::build(&model);
    assert!(!bare.materials.contains_key(&key));
}

#[test]
fn niche_move_rebuilds_the_wall_and_equals_full() {
    let (mut model, w) = north_wall();
    model.push_opening(niche(w, 6.0));
    let other = model.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
    let _ = other;
    let (mut cache, _) = SceneCache::new(&model);
    let mut edited = model.clone();
    edited.openings[0].offset = 3.0;
    let stats = cache.update(&edited);
    assert!(stats.rebuilt <= 2, "rebuilt too many objects: {stats:?}");
    assert_eq!(cache.scene().digest(), Scene3d::build(&edited).digest());
    let scene = cache.scene();
    assert_eq!(probe(scene, 3.0, 7.95, 1.45), Some((w.to_string(), "wall".to_string())));
    assert_eq!(probe(scene, 7.0, 7.95, 1.45), None);
}

#[test]
fn shower_leaf_is_glass_owned_by_the_opening() {
    let (model, wall, door) = shower();
    let scene = Scene3d::build(&model);
    let leaf: Vec<_> = scene.meshes.iter().filter(|m| m.material == "glass-shower").collect();
    assert_eq!(leaf.len(), 1);
    assert_eq!(leaf[0].object, door);
    let b = leaf[0].bounds();
    assert!((b.min[0] - 0.65).abs() < 1e-5 && (b.max[0] - 1.35).abs() < 1e-5);
    assert!((b.min[2] - 0.0).abs() < 1e-5 && (b.max[2] - 1.9).abs() < 1e-5);
    assert!(((b.max[1] - b.min[1]) - 0.008).abs() < 1e-5);
    assert_eq!(probe(&scene, 1.0, 0.0, 1.0), Some((format!("o{}", door.0), "glass".to_string())));
    assert!(scene.meshes.iter().any(|m| m.object == wall && m.material == "glass-wall"));
}

#[test]
fn shower_door_has_no_frame_or_header() {
    let (model, _, door) = shower();
    let scene = Scene3d::build(&model);
    let mine: Vec<_> = scene.meshes.iter().filter(|m| m.material != "glass-wall").map(|m| m.material.as_str()).collect();
    assert_eq!(mine, vec!["glass-shower", "door-leaf-hardware"]);
    let handle = scene.meshes.iter().find(|m| m.material == "door-leaf-hardware");
    let b = handle.map(|m| m.bounds());
    let b = b.expect("a handle");
    assert!(((b.max[0] - b.min[0]) - 0.02).abs() < 1e-5 && ((b.max[2] - b.min[2]) - 0.3).abs() < 1e-5);
    // Hinge left: the free edge is on the right, 50 mm in.
    assert!(((b.min[0] + b.max[0]) / 2.0 - 1.30).abs() < 1e-5);
    assert!(((b.min[2] + b.max[2]) / 2.0 - 1.0).abs() < 1e-5);
}

#[test]
fn niche_cut_is_exact() {
    let (mut model, w) = north_wall();
    model.push_opening(niche(w, 6.0));
    let scene = Scene3d::build(&model);
    let wall = Some((w.to_string(), "wall".to_string()));
    // Offsets run from the wall's start (10,8): x = 4 is offset 6.
    for (x, z) in [(4.0_f64, 1.45_f64), (3.67, 1.45), (4.33, 1.45), (4.0, 0.97), (4.0, 1.93)] {
        let inside = (x - 4.0).abs() < 0.3 && z > 1.0 && z < 1.9;
        let hit = probe(&scene, x, 7.95, z);
        if inside {
            assert_eq!(hit, None, "({x}, {z}) is in the recess");
        } else {
            assert_eq!(hit, wall, "({x}, {z}) is wall");
        }
    }
    // Just inside the recess edges: empty.
    assert_eq!(probe(&scene, 3.71, 7.95, 1.45), None);
    assert_eq!(probe(&scene, 4.29, 7.95, 1.89), None);
}

#[test]
fn shower_door_cut_is_exact() {
    let (model, wall, door) = shower();
    let scene = Scene3d::build(&model);
    let glass = Some((wall.to_string(), "glass".to_string()));
    assert_eq!(probe(&scene, 1.38, 0.0, 1.0), glass, "3 cm beside the door");
    assert_eq!(probe(&scene, 0.62, 0.0, 1.0), glass);
    assert_eq!(probe(&scene, 1.0, 0.0, 1.95), glass, "5 cm above the door");
    assert_eq!(probe(&scene, 1.0, 0.0, 1.0), Some((format!("o{}", door.0), "glass".to_string())));
    // The wall's profile stays simple: nothing rises above 2.0.
    for m in scene.meshes.iter().filter(|m| m.object == wall) {
        assert!(m.positions.iter().all(|p| f64::from(p[2]) <= 2.0 + 1e-5), "a vertex above the wall top");
    }
    assert!(scene.meshes.iter().any(|m| m.object == wall));
}
