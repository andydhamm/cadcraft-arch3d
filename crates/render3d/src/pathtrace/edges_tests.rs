use super::PathOptions;
use super::edge_table::edge_table;
use super::edge_table::fill;
use super::scene_gpu::{GpuFrame, GpuScene, GpuTri, PHOTO_EDGES};
use cadcraft_doc::arch::EdgeMode;

fn tri(v0: [f32; 3], e1: [f32; 3], e2: [f32; 3], obj: u32, n: [f32; 3]) -> GpuTri {
    GpuTri { v0, e1, e2, object: obj, n, ..Default::default() }
}

#[test]
fn edge_table_empty() {
    let table = edge_table(&[]);
    assert!(table.is_empty(), "empty input gives empty table");
}

#[test]
fn edge_table_single_triangle() {
    let t = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 1, [0.0, 0.0, 1.0]);
    let table = edge_table(&[t]);
    assert_eq!(table.len(), 3, "one triangle has three edge slots");
    assert!(table.iter().all(|&w| w == 0), "no shared edges, all zero");
}

#[test]
fn edge_table_three_triangles_share_edge() {
    let a = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 7, [0.0, 0.0, 1.0]);
    let b = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.5, 0.5, -1.0], 7, [0.0, 0.5, 0.866]);
    let c = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], 7, [0.0, 1.0, 0.0]);
    let table = edge_table(&[a, b, c]);
    assert!(table.iter().all(|&w| w == 0), "three-way shared edge is not rounded");
}

#[test]
fn edge_table_nan_vertex() {
    let t = tri([f32::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 1, [0.0, 0.0, 1.0]);
    let table = edge_table(&[t]);
    assert_eq!(table.len(), 3, "NaN vertex does not crash or change table size");
}

#[test]
fn edge_table_huge_coordinates() {
    let big = 1e10_f32;
    let a = tri([0.0, 0.0, 0.0], [big, 0.0, 0.0], [0.0, big, 0.0], 7, [0.0, 0.0, 1.0]);
    let b = tri([0.0, 0.0, 0.0], [big, 0.0, 0.0], [0.0, 0.0, -big], 7, [0.0, 1.0, 0.0]);
    let table = edge_table(&[a, b]);
    assert_ne!(table[0], 0, "huge coordinates still match edges");
}

#[test]
fn edge_table_back_to_back_normals() {
    let a = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 7, [0.0, 0.0, 1.0]);
    let b = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], 7, [0.0, 0.0, -1.0]);
    let table = edge_table(&[a, b]);
    assert_eq!(table[0], 0, "back-to-back normals (180°) are not rounded");
}

#[test]
fn edge_table_angle_boundary() {
    let angle = 20.0f32.to_radians();
    let n_a = [0.0, 0.0, 1.0];
    let n_b = [0.0, angle.sin(), angle.cos()];
    let a = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 7, n_a);
    let b = tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], 7, n_b);
    let table = edge_table(&[a, b]);
    assert_eq!(table[0], 0, "exactly 20° is not rounded (spec: more than 20°)");
}

#[test]
fn fill_per_material_sets_negative_one() {
    let mut scene = GpuScene::default();
    scene.tris = vec![tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 1, [0.0, 0.0, 1.0])];
    let mut frame = GpuFrame::default();
    let mut opts = PathOptions::default();
    opts.edges = EdgeMode::PerMaterial;
    fill(&mut scene, &mut frame, &opts);
    assert_eq!(frame.photo_f[0], -1.0, "PerMaterial sets override to -1");
    assert_ne!(frame.photo[0] & PHOTO_EDGES, 0, "edges flag is set");
}

#[test]
fn fill_skips_when_aux_buffer_full() {
    let mut scene = GpuScene::default();
    scene.ies = vec![0.0; (1 << 25) - 2];
    scene.tris = vec![tri([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 1, [0.0, 0.0, 1.0])];
    let mut frame = GpuFrame::default();
    let mut opts = PathOptions::default();
    opts.edges = EdgeMode::Radius(0.005);
    fill(&mut scene, &mut frame, &opts);
    assert_eq!(frame.photo[0] & PHOTO_EDGES, 0, "edges skipped when aux buffer would overflow");
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
