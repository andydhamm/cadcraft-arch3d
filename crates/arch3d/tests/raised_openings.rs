//! A wall with a window or a door on a level that is not at elevation 0 keeps its solid, with the
//! hole at the opening's own height: the sill is measured from the floor of the wall's level.

use cadcraft_arch3d::{Scene3d, SceneCache};
use cadcraft_doc::arch::{Level, Opening, OpeningKind, wall_label};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::{Vec2, Vec3};

/// The 10 x 8 house on one level at `elevation`; returns the model and wall 0's id (south wall).
fn house(elevation: f64) -> (ArchModel, ArchId) {
    let mut model = ArchModel::default();
    model.levels.push(Level { index: 0, elevation, ceiling_height: 2.4, ..Default::default() });
    let pts = [(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0), (0.0, 0.0)];
    let mut first = None;
    for pair in pts.windows(2) {
        let id = model.push_wall(Wall {
            start: Vec2::new(pair[0].0, pair[0].1),
            end: Vec2::new(pair[1].0, pair[1].1),
            thickness: 0.2,
            height: 2.7,
            exterior: true,
            ..Default::default()
        });
        first.get_or_insert(id);
    }
    (model, first.unwrap_or(ArchId(1)))
}

fn window(wall: ArchId, offset: f64, sill: f64) -> Opening {
    let mut op = Opening::new(wall, offset, 1.2, 1.2, OpeningKind::Window);
    op.sill = sill;
    op
}

fn door(wall: ArchId, offset: f64, height: f64) -> Opening {
    let mut op = Opening::new(wall, offset, 0.9, height, OpeningKind::Door);
    op.sill = 0.0;
    op
}

fn in_wall(scene: &Scene3d, wall: ArchId, x: f64, z: f64) -> bool {
    scene.probe(Vec3::new(x, 0.0, z)).is_some_and(|h| h.part == "wall" && h.id == wall_label(wall))
}

#[test]
fn window_wall_on_a_raised_level_keeps_its_solid() {
    let (mut model, w0) = house(1.5);
    model.push_opening(window(w0, 6.0, 1.0));
    let scene = Scene3d::build(&model);
    for z in [2.5, 1.6, 4.1] {
        assert!(in_wall(&scene, w0, 3.0, z), "(3, 0, {z}) should be solid");
    }
    for z in [4.3, 1.4] {
        assert!(!in_wall(&scene, w0, 3.0, z), "(3, 0, {z}) should be outside the wall");
    }
}

#[test]
fn window_hole_on_a_raised_level_is_at_level_plus_sill() {
    for e in [1.5, 0.3, 2.7] {
        let (mut model, w0) = house(e);
        let op = window(w0, 6.0, 1.0);
        let allowance = cadcraft_arch::opening::cut_allowance(&op);
        model.push_opening(op);
        let scene = Scene3d::build(&model);
        let lo = e + 1.0;
        let hi = e + 2.2;
        assert!(in_wall(&scene, w0, 6.0, lo - 0.3), "E={e}: below the hole is wall");
        assert!(in_wall(&scene, w0, 6.0, hi + allowance + 0.3), "E={e}: above the hole is wall");
        for z in [lo + 0.1 + allowance.min(0.0), lo + 0.6, hi - 0.1] {
            assert!(!in_wall(&scene, w0, 6.0, z), "E={e}: (6, 0, {z}) should be in the hole");
        }
    }
}

#[test]
fn door_notch_on_a_raised_level_is_door_high() {
    let (mut model, w0) = house(1.5);
    let h = 2.1;
    model.push_opening(door(w0, 6.0, h));
    let scene = Scene3d::build(&model);
    assert!(in_wall(&scene, w0, 3.0, 2.0));
    assert!(!in_wall(&scene, w0, 6.0, 1.6));
    assert!(!in_wall(&scene, w0, 6.0, 1.5 + h - 0.1));
    assert!(in_wall(&scene, w0, 6.0, 1.5 + h + 0.4));
}

#[test]
fn window_wall_below_zero_keeps_its_solid() {
    let (mut model, w0) = house(-1.2);
    model.push_opening(window(w0, 6.0, 1.0));
    let scene = Scene3d::build(&model);
    assert!(in_wall(&scene, w0, 3.0, 0.0));
    assert!(!in_wall(&scene, w0, 6.0, 0.4));
    assert!(in_wall(&scene, w0, 6.0, -1.0));
}

fn wall_zs(scene: &Scene3d, wall: ArchId) -> (usize, Vec<f64>) {
    let mesh = scene.mesh_for(&wall).expect("wall mesh");
    let mut zs: Vec<f64> = mesh.positions.iter().map(|p| f64::from(p[2])).collect();
    zs.sort_by(|a, b| a.total_cmp(b));
    (mesh.indices.len() / 3, zs)
}

#[test]
fn level_zero_walls_are_unchanged() {
    let (mut model, w0) = house(0.0);
    model.push_opening(window(w0, 6.0, 1.0));
    let scene = Scene3d::build(&model);
    assert!(in_wall(&scene, w0, 6.0, 0.5));
    assert!(in_wall(&scene, w0, 6.0, 2.5));
    assert!(!in_wall(&scene, w0, 6.0, 1.6));

    let (mut raised, r0) = house(1.5);
    raised.push_opening(window(r0, 6.0, 1.0));
    let raised_scene = Scene3d::build(&raised);
    let (n0, z0) = wall_zs(&scene, w0);
    let (n1, z1) = wall_zs(&raised_scene, r0);
    assert_eq!(n0, n1);
    assert_eq!(z0.len(), z1.len());
    for (a, b) in z0.iter().zip(&z1) {
        assert!((b - a - 1.5).abs() < 1e-6, "z {a} vs {b}");
    }
}

#[test]
fn spanning_wall_door_is_unchanged() {
    let mut model = ArchModel::default();
    model.levels.push(Level { index: 0, elevation: -1.2, ceiling_height: 2.4, ..Default::default() });
    model.levels.push(Level { index: 1, elevation: 1.5, ceiling_height: 2.4, ..Default::default() });
    let w = model.push_wall(Wall {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(10.0, 0.0),
        thickness: 0.2,
        height: 2.7,
        exterior: true,
        top_level: Some(1),
        ..Default::default()
    });
    let mut op = Opening::new(w, 6.0, 0.9, 2.1, OpeningKind::Door);
    op.sill = 1.35;
    model.push_opening(op);
    let scene = Scene3d::build(&model);
    // Absolute hole: -1.2 + 1.35 = 0.15 up to 0.15 + 2.1.
    assert!(!in_wall(&scene, w, 6.0, 0.3));
    assert!(!in_wall(&scene, w, 6.0, 2.0));
    assert!(in_wall(&scene, w, 6.0, -0.5));
}

#[test]
fn scene_cache_matches_full_for_a_raised_window() {
    let (mut model, w0) = house(1.5);
    let op_id = model.push_opening(window(w0, 6.0, 1.0));
    let (mut cache, _) = SceneCache::new(&model);
    assert_eq!(cache.scene().digest(), Scene3d::build(&model).digest());
    let mut next = model.clone();
    if let Some(op) = next.openings.iter_mut().find(|o| o.id == op_id) {
        op.offset += 1.0;
    }
    cache.update(&next);
    assert_eq!(cache.scene().digest(), Scene3d::build(&next).digest());
}
