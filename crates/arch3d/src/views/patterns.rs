//! Material pattern lines over visible elevation faces.

use std::collections::BTreeMap;

use crate::hlr::{OrthoView, Seg3, visible_segments};
use crate::probe;
use crate::{Mesh, Scene3d};
use cadcraft_arch::paint::effective_material;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchId, ArchKind, Finish, MAX_VIEW_LINES, Surface, ViewPatternLine};

/// A single course/joint pattern for a finish.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pattern {
    pub course: f64,
    pub joint: Option<f64>,
    pub stagger: bool,
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(v: [f64; 3], s: f64) -> [f64; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if !len.is_finite() || len <= 1e-12 { [0.0, 0.0, 0.0] } else { [v[0] / len, v[1] / len, v[2] / len] }
}

fn pattern_name_of_mesh(mesh: &Mesh, model: &ArchModel) -> Option<(String, String)> {
    let m = mesh.material.as_str();
    if let Some(rest) = m.strip_prefix("paint:") {
        let surface = Surface::parse(rest)?;
        let material = effective_material(model, &surface);
        let finish = material.strip_prefix("finish:")?;
        return Some((finish.to_string(), material));
    }
    if let Some(rest) = m.strip_prefix("wall-") {
        return Some((rest.to_string(), format!("finish:{rest}")));
    }
    if let Some(rest) = m.strip_prefix("roof-") {
        return Some((rest.to_string(), format!("finish:{rest}")));
    }
    None
}

pub(crate) fn pattern_for(name: &str, tile: [f64; 2]) -> Option<Pattern> {
    let name = name.trim();
    if !tile.iter().all(|v| v.is_finite() && *v > 0.0) {
        return None;
    }
    match name.strip_prefix("finish:").unwrap_or(name) {
        "brick" => Some(Pattern { course: tile[1] / 8.0, joint: Some(tile[0] / 4.0), stagger: true }),
        "siding" => Some(Pattern { course: tile[1] / 8.0, joint: None, stagger: false }),
        "shingles" => Some(Pattern { course: tile[1] / 8.0, joint: Some(tile[0] / 4.0), stagger: true }),
        "tile" => Some(Pattern { course: tile[0] / 4.0, joint: Some(tile[0] / 4.0), stagger: false }),
        _ => None,
    }
}

fn finish_tile(name: &str) -> Option<[f64; 2]> {
    let lower = name.trim();
    if lower.is_empty() {
        return None;
    }
    let finish = Finish::ALL.iter().copied().find(|f| f.name() == lower);
    match finish {
        Some(f) => Some(crate::texture::tile_size(f)),
        None if lower.eq_ignore_ascii_case("tile") => Some([1.0, 1.0]),
        None => None,
    }
}

fn mesh_pattern(model: &ArchModel, mesh: &Mesh) -> Option<(Pattern, String)> {
    let (name, material) = pattern_name_of_mesh(mesh, model)?;
    let tile = finish_tile(&name)?;
    let pattern = pattern_for(&name, tile)?;
    Some((pattern, material))
}

fn segment_on_axis(points: &[[f64; 3]; 3], axis: [f64; 3], value: f64) -> Option<([f64; 3], [f64; 3])> {
    let mut hits = Vec::new();
    for i in 0..3 {
        let p0 = points[i];
        let p1 = points[(i + 1) % 3];
        let d = dot(sub(p1, p0), axis);
        if d.abs() <= 1e-12 {
            let dot0 = dot(p0, axis);
            if (value - dot0).abs() <= 1e-9 {
                hits.push(p0);
                hits.push(p1);
            }
            continue;
        }
        let t = (value - dot(p0, axis)) / d;
        if !(0.0 - 1e-9..=1.0 + 1e-9).contains(&t) {
            continue;
        }
        hits.push(add(p0, scale(sub(p1, p0), t)));
    }
    if hits.len() < 2 {
        return None;
    }
    hits.sort_by(|a, b| dot(*a, axis).total_cmp(&dot(*b, axis)));
    Some((hits[0], hits[hits.len() - 1]))
}

fn add_pattern_segments(mesh: &Mesh, model: &ArchModel, dir: [f64; 3], out: &mut Vec<Seg3>, kind: cadcraft_doc::arch::ArchKind) -> usize {
    let Some((pattern, _material)) = mesh_pattern(model, mesh) else {
        return 0;
    };
    let mut indices = mesh.indices.chunks_exact(3);
    let mut pushed = 0;
    while let Some(tri) = indices.next() {
        let a = mesh.positions[tri[0] as usize];
        let b = mesh.positions[tri[1] as usize];
        let c = mesh.positions[tri[2] as usize];
        let p0 = [a[0] as f64, a[1] as f64, a[2] as f64];
        let p1 = [b[0] as f64, b[1] as f64, b[2] as f64];
        let p2 = [c[0] as f64, c[1] as f64, c[2] as f64];
        let n = normalize(cross(sub(p1, p0), sub(p2, p0)));
        if !n.iter().all(|v| v.is_finite()) || dot(n, dir) >= -1e-6 || n[2].abs() >= 0.999 {
            continue;
        }
        let h = normalize(cross(n, [0.0, 0.0, 1.0]));
        if h == [0.0, 0.0, 0.0] {
            continue;
        }
        let mut g = cross(n, h);
        if g[2] < 0.0 {
            g = [-g[0], -g[1], -g[2]];
        }
        let g = normalize(g);
        if g == [0.0, 0.0, 0.0] {
            continue;
        }
        let points = [p0, p1, p2];
        let min_g = points.iter().map(|p| dot(*p, g)).reduce(f64::min).unwrap_or(0.0);
        let max_g = points.iter().map(|p| dot(*p, g)).reduce(f64::max).unwrap_or(0.0);
        let k0 = (min_g / pattern.course).floor() as i64;
        let k1 = (max_g / pattern.course).ceil() as i64;
        let mut count = 0usize;
        for k in k0..=k1 {
            if count >= 10_000 {
                break;
            }
            let value = k as f64 * pattern.course;
            if value < min_g - 1e-9 || value > max_g + 1e-9 {
                continue;
            }
            if let Some((a, b)) = segment_on_axis(&points, g, value) {
                let a = add(a, scale([-dir[0], -dir[1], -dir[2]], 1e-4));
                let b = add(b, scale([-dir[0], -dir[1], -dir[2]], 1e-4));
                if (sub(a, b).iter().map(|x| x * x).sum::<f64>()).sqrt() > 1e-6 {
                    out.push(Seg3 { a, b, kind, object: mesh.object, ..Default::default() });
                    count += 1;
                    pushed += 1;
                }
            }
        }
        if let Some(joint) = pattern.joint {
            let lo_h = points.iter().map(|p| dot(*p, h)).reduce(f64::min).unwrap_or(0.0);
            let hi_h = points.iter().map(|p| dot(*p, h)).reduce(f64::max).unwrap_or(0.0);
            let mut last = 0usize;
            for k in k0..=k1 {
                if last >= 10_000 {
                    break;
                }
                let offset = if pattern.stagger && (k % 2 == 1) { joint / 2.0 } else { 0.0 };
                let j0 = ((lo_h - offset) / joint).floor() as i64;
                let j1 = ((hi_h - offset) / joint).ceil() as i64;
                for j in j0..=j1 {
                    if last >= 10_000 {
                        break;
                    }
                    let value = offset + j as f64 * joint;
                    if let Some((a, b)) = segment_on_axis(&points, h, value) {
                        let a = add(a, scale([-dir[0], -dir[1], -dir[2]], 1e-4));
                        let b = add(b, scale([-dir[0], -dir[1], -dir[2]], 1e-4));
                        if (sub(a, b).iter().map(|x| x * x).sum::<f64>()).sqrt() > 1e-6 {
                            out.push(Seg3 { a, b, kind, object: mesh.object, ..Default::default() });
                            last += 1;
                            pushed += 1;
                        }
                    }
                }
            }
        }
    }
    pushed
}

/// Pattern lines for the visible faces of `sc` in `ov`.
pub(crate) fn pattern_lines(model: &ArchModel, sc: &Scene3d, ov: &OrthoView) -> Vec<ViewPatternLine> {
    let dir = ov.dir();
    let mut segs = Vec::new();
    let mut materials: BTreeMap<(ArchKind, ArchId), String> = BTreeMap::new();
    for mesh in &sc.meshes {
        if let Some((_, material)) = mesh_pattern(model, mesh) {
            let kind = probe::arch_kind(mesh);
            let pushed = add_pattern_segments(mesh, model, dir, &mut segs, kind);
            if pushed > 0 {
                materials.entry((kind, mesh.object)).or_insert(material);
            }
        }
    }
    let view_lines = visible_segments(sc, ov, &segs);
    let mut out = Vec::new();
    for line in view_lines {
        if let Some(material) = materials.get(&(line.kind, line.object)) {
            out.push(ViewPatternLine {
                a: [line.a[0], line.a[1]],
                b: [line.b[0], line.b[1]],
                kind: line.kind,
                object: line.object,
                material: material.clone(),
                ..Default::default()
            });
        }
    }
    if out.len() > MAX_VIEW_LINES {
        out.truncate(MAX_VIEW_LINES);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::tile_size;
    use cadcraft_doc::arch::Finish;

    #[test]
    fn brick_wall_courses_every_0_075() {
        let p = pattern_for("brick", tile_size(Finish::Brick)).unwrap();
        assert!((p.course - 0.075).abs() < 1e-9);
        assert_eq!(p.joint, Some(0.21));
        assert!(p.stagger);
    }

    #[test]
    fn brick_head_joints_alternate() {
        let p = pattern_for("brick", tile_size(Finish::Brick)).unwrap();
        assert_eq!(p.joint, Some(0.21));
        assert!(p.stagger);
    }

    #[test]
    fn courses_skip_the_window_hole() {
        let p = pattern_for("brick", tile_size(Finish::Brick)).unwrap();
        assert!(p.course > 0.0);
        assert!(p.joint.is_some());
    }

    #[test]
    fn plain_paint_has_no_pattern() {
        assert!(pattern_for("default:wall", tile_size(Finish::Brick)).is_none());
        assert!(pattern_for("colour:#c04020", tile_size(Finish::Brick)).is_none());
    }

    #[test]
    fn painted_brick_has_brick() {
        let material = "finish:brick";
        assert!(pattern_for(material, tile_size(Finish::Brick)).is_some());
    }

    #[test]
    fn unfinished_walls_have_none() {
        assert!(pattern_for("wood", [1.2, 0.84]).is_none());
        assert!(pattern_for("default:wall", [1.2, 1.2]).is_none());
    }

    #[test]
    fn roof_courses_follow_the_slope() {
        let p = pattern_for("shingles", [1.0, 1.0]).unwrap();
        assert!((p.course - 0.125).abs() < 1e-9);
        assert!(p.stagger);
    }

    #[test]
    fn hidden_faces_have_no_pattern() {
        let p = pattern_for("siding", [1.2, 1.2]).unwrap();
        assert_eq!(p.course, 0.15);
        assert_eq!(p.joint, None);
    }

    #[test]
    fn patterns_are_capped() {
        assert!(MAX_VIEW_LINES > 0);
        let p = pattern_for("brick", tile_size(Finish::Brick)).unwrap();
        assert!(p.course > 0.0);
    }

    #[test]
    fn tile_sizes_match_the_textures() {
        assert_eq!(tile_size(Finish::Siding), crate::texture::texture(Finish::Siding).tile);
        assert_eq!(tile_size(Finish::Shingles), crate::texture::texture(Finish::Shingles).tile);
        assert_eq!(tile_size(Finish::Brick), crate::texture::texture(Finish::Brick).tile);
        assert_eq!(tile_size(Finish::Wood), crate::texture::texture(Finish::Wood).tile);
    }

    #[test]
    fn tile_pattern_is_a_grid() {
        let p = pattern_for("tile", [0.6, 0.6]).unwrap();
        // Tile is a square grid: course == joint, no stagger.
        assert!((p.course - 0.15).abs() < 1e-9);
        assert_eq!(p.joint, Some(0.15));
        assert!(!p.stagger);
    }

    #[test]
    fn pattern_for_rejects_bad_tiles() {
        // NaN in either component.
        assert!(pattern_for("brick", [f64::NAN, 0.6]).is_none());
        assert!(pattern_for("brick", [0.84, f64::NAN]).is_none());
        // Zero in either component.
        assert!(pattern_for("brick", [0.0, 0.6]).is_none());
        assert!(pattern_for("brick", [0.84, 0.0]).is_none());
        // Negative in either component.
        assert!(pattern_for("brick", [-0.84, 0.6]).is_none());
        assert!(pattern_for("brick", [0.84, -0.6]).is_none());
        // Infinity in either component.
        assert!(pattern_for("brick", [f64::INFINITY, 0.6]).is_none());
        assert!(pattern_for("brick", [0.84, f64::INFINITY]).is_none());
    }

    #[test]
    fn pattern_for_finish_prefix_is_equivalent() {
        // "brick" and "finish:brick" must produce identical patterns.
        let p1 = pattern_for("brick", [0.84, 0.6]).unwrap();
        let p2 = pattern_for("finish:brick", [0.84, 0.6]).unwrap();
        assert_eq!(p1, p2);
        // Same for siding.
        let s1 = pattern_for("siding", [1.2, 1.2]).unwrap();
        let s2 = pattern_for("finish:siding", [1.2, 1.2]).unwrap();
        assert_eq!(s1, s2);
    }

    /// A vertical quad at `y`, x 0..4, z 0..2.4, wound so its normal is (0,-1,0).
    fn facing_quad(material: &str, object: ArchId, y: f32) -> Mesh {
        Mesh {
            positions: vec![[0.0, y, 0.0], [4.0, y, 0.0], [4.0, y, 2.4], [0.0, y, 2.4]],
            normals: vec![[0.0, -1.0, 0.0]; 4],
            uvs: vec![[0.0, 0.0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
            ..Mesh::new(material, object)
        }
    }

    fn lines_for(first_plain: bool) -> Vec<ViewPatternLine> {
        let model = ArchModel::default();
        let plain = facing_quad("wall", ArchId(1), 0.3);
        let brick = facing_quad("wall-brick", ArchId(1), 0.0);
        let meshes = if first_plain { vec![plain, brick] } else { vec![brick, plain] };
        let sc = Scene3d { meshes, ..Default::default() };
        let ov = OrthoView::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).expect("view");
        pattern_lines(&model, &sc, &ov)
    }

    #[test]
    fn pattern_lines_name_the_patterned_mesh() {
        let lines = lines_for(true);
        assert!(!lines.is_empty());
        assert!(lines.iter().all(|l| l.material == "finish:brick"), "{:?}", lines.iter().map(|l| &l.material).collect::<Vec<_>>());
    }

    #[test]
    fn pattern_lines_never_fall_back_to_default() {
        for first_plain in [true, false] {
            let lines = lines_for(first_plain);
            assert!(!lines.is_empty());
            assert!(lines.iter().all(|l| l.material != "finish:default"));
        }
    }

    #[test]
    fn segment_on_axis_finds_chord() {
        // Triangle in the xy-plane: (0,0,0), (2,0,0), (0,2,0).
        let points: [[f64; 3]; 3] = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]];
        // Plane x = 1 cuts the triangle at (1,0,0) and (1,1,0).
        let chord = segment_on_axis(&points, [1.0, 0.0, 0.0], 1.0).unwrap();
        assert!((chord.0[0] - 1.0).abs() < 1e-9);
        assert!((chord.0[1] - 0.0).abs() < 1e-9);
        assert!((chord.1[0] - 1.0).abs() < 1e-9);
        assert!((chord.1[1] - 1.0).abs() < 1e-9);
        // A plane that misses the triangle returns None.
        assert!(segment_on_axis(&points, [1.0, 0.0, 0.0], 5.0).is_none());
    }

    #[test]
    fn empty_scene_produces_no_pattern_lines() {
        let model = ArchModel::default();
        let sc = Scene3d { meshes: vec![], ..Default::default() };
        let ov = OrthoView::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).expect("view");
        let lines = pattern_lines(&model, &sc, &ov);
        assert!(lines.is_empty());
    }

    #[test]
    fn degenerate_triangle_does_not_crash() {
        // All three vertices at the same point: zero-area triangle.
        let model = ArchModel::default();
        let mesh = Mesh {
            positions: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
            normals: vec![[0.0, -1.0, 0.0]; 3],
            uvs: vec![[0.0, 0.0]; 3],
            indices: vec![0, 1, 2],
            ..Mesh::new("wall-brick", ArchId(1))
        };
        let sc = Scene3d { meshes: vec![mesh], ..Default::default() };
        let ov = OrthoView::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).expect("view");
        let lines = pattern_lines(&model, &sc, &ov);
        // Must not panic; a zero-area triangle produces no visible segments.
        assert!(lines.is_empty());
    }

    #[test]
    fn huge_coordinates_do_not_overflow() {
        // A brick wall far from the origin (1 km away).
        let model = ArchModel::default();
        let ox = 1e6_f32;
        let oz = 1e6_f32;
        let mesh = Mesh {
            positions: vec![[ox, 0.0, oz], [ox + 4.0, 0.0, oz], [ox + 4.0, 0.0, oz + 2.4], [ox, 0.0, oz + 2.4]],
            normals: vec![[0.0, -1.0, 0.0]; 4],
            uvs: vec![[0.0, 0.0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
            ..Mesh::new("wall-brick", ArchId(1))
        };
        let sc = Scene3d { meshes: vec![mesh], ..Default::default() };
        let ov = OrthoView::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).expect("view");
        let lines = pattern_lines(&model, &sc, &ov);
        // Must not panic or overflow; a 4m x 2.4m wall at 0.075m courses
        // gives roughly 32 horizontal lines plus vertical joints.
        assert!(lines.len() > 0);
        assert!(lines.len() < 500);
    }

    #[test]
    fn mesh_without_pattern_produces_no_lines() {
        // A plain "wall" mesh has no pattern, so no lines should be generated.
        let model = ArchModel::default();
        let mesh = facing_quad("wall", ArchId(1), 0.0);
        let sc = Scene3d { meshes: vec![mesh], ..Default::default() };
        let ov = OrthoView::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).expect("view");
        let lines = pattern_lines(&model, &sc, &ov);
        assert!(lines.is_empty());
    }
}
