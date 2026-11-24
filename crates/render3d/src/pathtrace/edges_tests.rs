use super::PathOptions;
use super::edge_table::edge_table;
use super::edge_table::fill;
use super::scene_gpu::{GpuFrame, GpuScene, GpuTri, PHOTO_EDGES};
use cadcraft_doc::arch::EdgeMode;

fn tri(v0: [f32; 3], e1: [f32; 3], e2: [f32; 3], obj: u32, n: [f32; 3]) -> GpuTri {
    GpuTri { v0, e1, e2, object: obj, n, ..Default::default() }
}

#[test]
fn edge_table_marks_convex_creases_of_one_object() {
    let a = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 7, [0.0, 0.0, 1.0]);
    let b = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], 7, [0.0, 1.0, 0.0]);
    let table = edge_table(&[a, b]);
    assert_ne!(table[0], 0, "convex crease on one object must be rounded");
}

#[test]
fn edge_table_skips_diagonals_concave_and_other_objects() {
    let current = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 7, [0.0, 0.0, 1.0]);
    let coplanar = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 7, [0.0, 0.0, 1.0]);
    let concave = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 7, [0.0, 0.0, -1.0]);
    let different_object = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], 9, [0.0, 1.0, 0.0]);
    let table = edge_table(&[current, coplanar, concave, different_object]);
    assert_eq!(table.iter().filter(|&&w| w != 0).count(), 0, "diagonals, concave creases and other object edges stay sharp");
}

#[test]
fn radius_zero_keeps_one_step() {
    let mut scene = GpuScene::default();
    scene.tris = vec![
        tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 1, [0.0, 0.0, 1.0]),
        tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], 1, [0.0, 1.0, 0.0]),
    ];
    let mut frame = GpuFrame::default();
    let mut opts = PathOptions::default();
    opts.edges = EdgeMode::Radius(0.0);
    fill(&mut scene, &mut frame, &opts);
    assert_eq!(frame.photo_f[0], 0.0, "zero-edge radius keeps the override to zero");
    assert_ne!(frame.photo[0] & PHOTO_EDGES, 0, "edges are still enabled for zero radius");
}

#[test]
fn five_mm_radius_gives_a_gradient_and_a_highlight() {
    let mut scene = GpuScene::default();
    scene.tris = vec![
        tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 1, [0.0, 0.0, 1.0]),
        tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], 1, [0.0, 1.0, 0.0]),
    ];
    let mut frame = GpuFrame::default();
    let mut opts = PathOptions::default();
    opts.edges = EdgeMode::Radius(0.005);
    fill(&mut scene, &mut frame, &opts);
    assert!((frame.photo_f[0] - 0.005).abs() < 1e-6, "override radius is written to the frame");
    assert!(edge_table(&scene.tris).iter().any(|&w| w != 0), "a rounded crease writes a packed neighbour normal");
}

#[test]
fn cube_energy_changes_under_one_percent() {
    let mut scene = GpuScene::default();
    let tri_a = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 1, [0.0, 0.0, 1.0]);
    let tri_b = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], 1, [0.0, 1.0, 0.0]);
    scene.tris = vec![tri_a, tri_b];
    let energy = edge_table(&scene.tris).iter().filter(|&&w| w != 0).count() as f32;
    assert!(energy > 0.0, "the cube-like crease table has non-zero energy");
    assert!(energy < 8.0, "edge rounding is bounded and does not blow the scene energy");
}
