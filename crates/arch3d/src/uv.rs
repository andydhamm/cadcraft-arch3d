//! Face-aligned texture coordinates (phase 13).

use crate::mesh::Mesh;

/// The same triangles with every triangle given its own three vertices, its face normal on all
/// three, and UVs from the face's own frame divided by `tile` (1.0 in UV = one texture repeat).
///
/// Horizontal faces (`|n.z| > 0.999`) use `u = x, v = y`; any other face uses `t = normalise((-n.y,
/// n.x, 0))` along the face and `b = n x t` flipped so `b.z >= 0`, `u = p.t`, `v = p.b` — on a wall
/// u runs along the wall and v up it; on a roof plane u runs along the eave and v up the slope.
/// A degenerate triangle keeps `n = +z`. Triangles with out-of-range indices are dropped. Material,
/// object and level are kept. A non-positive or non-finite `tile` component is treated as 1.
pub fn face_uvs(mesh: &Mesh, tile: [f64; 2]) -> Mesh {
    let ok = |t: f64| if t.is_finite() && t > 0.0 { t } else { 1.0 };
    let (tu, tv) = (ok(tile[0]), ok(tile[1]));
    let mut out = Mesh { positions: Vec::new(), normals: Vec::new(), uvs: Vec::new(), indices: Vec::new(), ..mesh.clone() };
    let pos = |i: u32| mesh.positions.get(i as usize).map(|p| [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]);
    for tri in mesh.indices.chunks_exact(3) {
        let (Some(&i0), Some(&i1), Some(&i2)) = (tri.first(), tri.get(1), tri.get(2)) else {
            continue;
        };
        let (Some(a), Some(b), Some(c)) = (pos(i0), pos(i1), pos(i2)) else {
            continue;
        };
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let cr = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let len = (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt();
        let n = if len.is_finite() && len > 1e-18 { [cr[0] / len, cr[1] / len, cr[2] / len] } else { [0.0, 0.0, 1.0] };
        let (tdir, bdir) = if n[2].abs() > 0.999 {
            ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
        } else {
            let tl = (n[0] * n[0] + n[1] * n[1]).sqrt();
            let t = [-n[1] / tl, n[0] / tl, 0.0];
            let mut b = [n[1] * t[2] - n[2] * t[1], n[2] * t[0] - n[0] * t[2], n[0] * t[1] - n[1] * t[0]];
            if b[2] < 0.0 {
                b = [-b[0], -b[1], -b[2]];
            }
            (t, b)
        };
        let base = out.positions.len() as u32;
        for (k, p) in [a, b, c].into_iter().enumerate() {
            let u = p[0] * tdir[0] + p[1] * tdir[1] + p[2] * tdir[2];
            let v = p[0] * bdir[0] + p[1] * bdir[1] + p[2] * bdir[2];
            out.positions.push([p[0] as f32, p[1] as f32, p[2] as f32]);
            out.normals.push([n[0] as f32, n[1] as f32, n[2] as f32]);
            out.uvs.push([(u / tu) as f32, (v / tv) as f32]);
            out.indices.push(base + k as u32);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;

    fn quad(pts: [[f32; 3]; 4]) -> Mesh {
        Mesh { positions: pts.to_vec(), indices: vec![0, 1, 2, 0, 2, 3], ..Mesh::new("wall", ArchId(7)) }
    }

    fn span(r: &Mesh, axis: usize) -> f32 {
        let hi = r.uvs.iter().map(|uv| uv[axis]).fold(f32::MIN, f32::max);
        let lo = r.uvs.iter().map(|uv| uv[axis]).fold(f32::MAX, f32::min);
        hi - lo
    }

    #[test]
    fn face_uvs_on_a_y_running_wall_run_along_the_wall() {
        let m = quad([[0.0, 0.0, 0.0], [0.0, 0.0, 3.0], [0.0, 4.0, 3.0], [0.0, 4.0, 0.0]]);
        let r = face_uvs(&m, [1.0, 1.0]);
        assert!((r.normals[0][0].abs() - 1.0).abs() < 1e-5);
        for (p, uv) in r.positions.iter().zip(&r.uvs) {
            assert!((uv[1] - p[2]).abs() < 1e-5, "v {} z {}", uv[1], p[2]);
        }
        assert!((span(&r, 0) - 4.0).abs() < 1e-5, "u span {}", span(&r, 0));
        let scaled = face_uvs(&m, [2.0, 1.5]);
        assert!((span(&scaled, 0) - 2.0).abs() < 1e-5);
        assert!((span(&scaled, 1) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn face_uvs_on_a_roof_plane_run_along_the_eave() {
        let h = 2.0f32;
        let m = quad([[0.0, 0.0, 3.0], [4.0, 0.0, 3.0], [4.0, h, 3.0 + h], [0.0, h, 3.0 + h]]);
        let r = face_uvs(&m, [1.0, 1.0]);
        assert!((span(&r, 0) - 4.0).abs() < 1e-4, "u along eave {}", span(&r, 0));
        assert!((span(&r, 1) - 2.0 * 2f32.sqrt()).abs() < 1e-4, "v along slope {}", span(&r, 1));
        // Positions at the same height along the slope share v.
        assert!((r.uvs[0][1] - r.uvs[1][1]).abs() < 1e-5);
        // A horizontal face uses x, y.
        let flat = face_uvs(&quad([[0.0, 0.0, 1.0], [2.0, 0.0, 1.0], [2.0, 3.0, 1.0], [0.0, 3.0, 1.0]]), [1.0, 1.0]);
        assert_eq!(flat.uvs[2], [2.0, 3.0]);
    }

    #[test]
    fn face_uvs_keep_positions_and_triangles() {
        let m = quad([[0.0, 0.0, 0.0], [0.0, 0.0, 3.0], [0.0, 4.0, 3.0], [0.0, 4.0, 0.0]]);
        let r = face_uvs(&m, [1.0, 1.0]);
        assert_eq!(r.indices.len(), m.indices.len());
        assert_eq!(r.positions.len(), 6, "every triangle owns its vertices");
        assert_eq!(r.normals.len(), 6);
        assert_eq!(r.uvs.len(), 6);
        for (k, &i) in m.indices.iter().enumerate() {
            assert_eq!(r.positions.get(r.indices[k] as usize), m.positions.get(i as usize));
        }
        assert_eq!(r.material, "wall");
        assert_eq!(r.object, ArchId(7));
        // A degenerate triangle keeps +z; a triangle with a bad index is dropped.
        let d = Mesh { positions: vec![[0.0; 3]; 3], indices: vec![0, 1, 2, 0, 1, 9], ..Mesh::new("x", ArchId(1)) };
        let rd = face_uvs(&d, [1.0, 1.0]);
        assert_eq!(rd.indices.len(), 3);
        assert_eq!(rd.normals[0], [0.0, 0.0, 1.0]);
    }

    #[test]
    fn face_uvs_on_empty_mesh_returns_empty() {
        let m = Mesh::new("wall", ArchId(0));
        let r = face_uvs(&m, [1.0, 1.0]);
        assert!(r.positions.is_empty());
        assert!(r.normals.is_empty());
        assert!(r.uvs.is_empty());
        assert!(r.indices.is_empty());
        assert_eq!(r.material, "wall");
        assert_eq!(r.object, ArchId(0));
    }

    #[test]
    fn face_uvs_on_x_running_wall() {
        // Wall running along X at y=0, from x=0 to x=4, z=0 to z=3.
        let m = quad([[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [4.0, 0.0, 3.0], [0.0, 0.0, 3.0]]);
        let r = face_uvs(&m, [1.0, 1.0]);
        // Normal should be (0, -1, 0).
        assert!((r.normals[0][1] + 1.0).abs() < 1e-5, "normal y = {}", r.normals[0][1]);
        // u runs along x, v runs along z.
        for (p, uv) in r.positions.iter().zip(&r.uvs) {
            assert!((uv[0] - p[0]).abs() < 1e-5, "u {} x {}", uv[0], p[0]);
            assert!((uv[1] - p[2]).abs() < 1e-5, "v {} z {}", uv[1], p[2]);
        }
    }

    #[test]
    fn face_uvs_with_zero_and_negative_tile() {
        let m = quad([[0.0, 0.0, 0.0], [0.0, 0.0, 3.0], [0.0, 4.0, 3.0], [0.0, 4.0, 0.0]]);
        let r = face_uvs(&m, [0.0, -1.0]);
        // Zero and negative tile components are treated as 1.0.
        assert!((span(&r, 0) - 4.0).abs() < 1e-5, "u span {}", span(&r, 0));
        assert!((span(&r, 1) - 3.0).abs() < 1e-5, "v span {}", span(&r, 1));
    }

    #[test]
    fn face_uvs_on_downward_facing_horizontal() {
        // Horizontal face at z=1 with normal (0, 0, -1).
        let m = quad([[0.0, 0.0, 1.0], [0.0, 3.0, 1.0], [2.0, 3.0, 1.0], [2.0, 0.0, 1.0]]);
        let r = face_uvs(&m, [1.0, 1.0]);
        assert!((r.normals[0][2] + 1.0).abs() < 1e-5, "normal z = {}", r.normals[0][2]);
        // u = x, v = y for horizontal faces regardless of normal direction.
        for (p, uv) in r.positions.iter().zip(&r.uvs) {
            assert!((uv[0] - p[0]).abs() < 1e-5, "u {} x {}", uv[0], p[0]);
            assert!((uv[1] - p[1]).abs() < 1e-5, "v {} y {}", uv[1], p[1]);
        }
    }

    #[test]
    fn face_uvs_with_nan_positions_does_not_crash() {
        let m =
            Mesh { positions: vec![[0.0, 0.0, 0.0], [f32::NAN, 0.0, 0.0], [0.0, 0.0, 0.0]], indices: vec![0, 1, 2], ..Mesh::new("wall", ArchId(0)) };
        let r = face_uvs(&m, [1.0, 1.0]);
        assert_eq!(r.positions.len(), 3);
        // Degenerate triangle (two vertices coincide) keeps +z normal.
        assert_eq!(r.normals[0], [0.0, 0.0, 1.0]);
    }

    #[test]
    fn face_uvs_single_triangle() {
        let m = Mesh { positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], indices: vec![0, 1, 2], ..Mesh::new("floor", ArchId(5)) };
        let r = face_uvs(&m, [1.0, 1.0]);
        assert_eq!(r.positions.len(), 3);
        assert_eq!(r.indices.len(), 3);
        // Horizontal face: u = x, v = y.
        assert_eq!(r.uvs[0], [0.0, 0.0]);
        assert_eq!(r.uvs[1], [1.0, 0.0]);
        assert_eq!(r.uvs[2], [0.0, 1.0]);
    }
}
