//! The mitred sweep every molding uses: a closed 2D profile pushed along a plan path with a z per
//! path vertex, as a closed volume.

use crate::mesh::{Mesh, triangulate};
use cadcraft_arch::molding_profile::validate_profile;
use cadcraft_doc::ArchId;
use cadcraft_geom::Vec2;

/// The most path points a sweep accepts.
const MAX_PATH_POINTS: usize = 4096;
/// Path points closer than this are one point.
const DUP_TOL: f64 = 1e-9;
/// A mitre whose |m . n_in| is below this is too sharp (a turn of about 174 degrees or more).
const MIN_MITRE: f64 = 0.05;

/// Sweep `profile` (x out from the path, y up) along `path` at heights `z` (one per path point).
///
/// `side >= 0` puts the profile's x to the left of travel, otherwise to the right. Every corner is
/// mitred. An open path gets two end caps; a closed path has none. Each face has its own vertices
/// and a flat normal pointing away from the profile's interior; u is the distance along the path
/// and v the profile's y. `level_elevation` is left NaN for the caller. Returns `None` for an
/// invalid profile, too few or too many path points, a `z` of the wrong length, a non-finite
/// value or a mitre that is too sharp.
pub fn sweep(profile: &[[f64; 2]], path: &[Vec2], z: &[f64], closed: bool, side: f64, material: &str, object: ArchId) -> Option<Mesh> {
    if path.len() > MAX_PATH_POINTS || z.len() != path.len() || !side.is_finite() {
        return None;
    }
    if validate_profile(profile).is_err() {
        return None;
    }
    if path.iter().any(|p| !p.is_finite()) || z.iter().any(|v| !v.is_finite()) {
        return None;
    }
    // Drop consecutive duplicates (and a closing point equal to the first).
    let mut pts: Vec<Vec2> = Vec::with_capacity(path.len());
    let mut zs: Vec<f64> = Vec::with_capacity(path.len());
    for (p, &h) in path.iter().zip(z) {
        if let Some(l) = pts.last() {
            if (l.x - p.x).abs() <= DUP_TOL && (l.y - p.y).abs() <= DUP_TOL {
                continue;
            }
        }
        pts.push(*p);
        zs.push(h);
    }
    if closed && pts.len() > 1 {
        if let (Some(f), Some(l)) = (pts.first().copied(), pts.last().copied()) {
            if (f.x - l.x).abs() <= DUP_TOL && (f.y - l.y).abs() <= DUP_TOL {
                pts.pop();
                zs.pop();
            }
        }
    }
    let n = pts.len();
    if n < if closed { 3 } else { 2 } {
        return None;
    }
    let seg_count = if closed { n } else { n - 1 };

    // Segment directions, left normals, lengths and the distance along the path at each vertex.
    let mut dirs: Vec<[f64; 2]> = Vec::with_capacity(seg_count);
    let mut lens: Vec<f64> = Vec::with_capacity(seg_count);
    for i in 0..seg_count {
        let a = pts.get(i)?;
        let b = pts.get((i + 1) % n)?;
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let l = dx.hypot(dy);
        if !(l.is_finite() && l > DUP_TOL) {
            return None;
        }
        dirs.push([dx / l, dy / l]);
        lens.push(l);
    }
    let normal_of = |i: usize| -> Option<[f64; 2]> {
        let d = dirs.get(i)?;
        Some([-d[1], d[0]])
    };
    let mut cum: Vec<f64> = Vec::with_capacity(n + 1);
    let mut acc = 0.0;
    cum.push(0.0);
    for l in &lens {
        acc += l;
        cum.push(acc);
    }

    // Frames: mitre direction and scale at every vertex.
    let mut frames: Vec<([f64; 2], f64)> = Vec::with_capacity(n);
    for k in 0..n {
        let (n_in, n_out) = if closed {
            (normal_of((k + seg_count - 1) % seg_count)?, normal_of(k % seg_count)?)
        } else if k == 0 {
            let q = normal_of(0)?;
            (q, q)
        } else if k == n - 1 {
            let q = normal_of(k - 1)?;
            (q, q)
        } else {
            (normal_of(k - 1)?, normal_of(k)?)
        };
        let sum = [n_in[0] + n_out[0], n_in[1] + n_out[1]];
        let sl = sum[0].hypot(sum[1]);
        if !(sl.is_finite() && sl > 1e-12) {
            return None;
        }
        let m = [sum[0] / sl, sum[1] / sl];
        let dot = m[0] * n_in[0] + m[1] * n_in[1];
        if dot.abs() < MIN_MITRE {
            return None;
        }
        frames.push((m, 1.0 / dot));
    }

    let sign = if side >= 0.0 { 1.0 } else { -1.0 };
    let pc = profile.len();
    let pos = |k: usize, j: usize| -> Option<[f64; 3]> {
        let p = pts.get(k)?;
        let (m, s) = frames.get(k)?;
        let q = profile.get(j)?;
        let off = sign * q[0] * s;
        Some([p.x + m[0] * off, p.y + m[1] * off, zs.get(k)? + q[1]])
    };

    let mut mesh = Mesh::new(material, object);
    for i in 0..seg_count {
        let (k0, k1) = (i, (i + 1) % n);
        let nseg = normal_of(i)?;
        let (u0, u1) = (*cum.get(i)?, *cum.get(i + 1)?);
        for j in 0..pc {
            let j1 = (j + 1) % pc;
            let (pa, pb) = (profile.get(j)?, profile.get(j1)?);
            let (dx, dy) = (pb[0] - pa[0], pb[1] - pa[1]);
            let el = dx.hypot(dy);
            if !(el > 0.0) {
                continue;
            }
            // Outward from the (counter-clockwise) profile: to the right of the edge.
            let (ox, oy) = (dy / el, -dx / el);
            let nrm = [ox * sign * nseg[0], ox * sign * nseg[1], oy];
            let corners = [(pos(k0, j)?, u0, pa[1]), (pos(k1, j)?, u1, pa[1]), (pos(k1, j1)?, u1, pb[1]), (pos(k0, j1)?, u0, pb[1])];
            let base = mesh.positions.len() as u32;
            for (p, u, v) in corners {
                push_vertex(&mut mesh, p, nrm, u, v);
            }
            push_tri(&mut mesh, [base, base + 1, base + 2], nrm);
            push_tri(&mut mesh, [base, base + 2, base + 3], nrm);
        }
    }

    if !closed {
        let ring: Vec<Vec2> = profile.iter().map(|q| Vec2::new(q[0], q[1])).collect();
        let tris = triangulate(&ring);
        if tris.is_empty() {
            return None;
        }
        let first = dirs.first()?;
        let last = dirs.last()?;
        for (k, nrm) in [(0usize, [-first[0], -first[1], 0.0]), (n - 1, [last[0], last[1], 0.0])] {
            let base = mesh.positions.len() as u32;
            let u = *cum.get(k)?;
            for (j, q) in profile.iter().enumerate() {
                push_vertex(&mut mesh, pos(k, j)?, nrm, u, q[1]);
            }
            for t in &tris {
                push_tri(&mut mesh, [base + t[0], base + t[1], base + t[2]], nrm);
            }
        }
    }
    Some(mesh)
}

/// Append one vertex.
fn push_vertex(mesh: &mut Mesh, p: [f64; 3], normal: [f64; 3], u: f64, v: f64) {
    mesh.positions.push([p[0] as f32, p[1] as f32, p[2] as f32]);
    mesh.normals.push([normal[0] as f32, normal[1] as f32, normal[2] as f32]);
    mesh.uvs.push([u as f32, v as f32]);
}

/// Append a triangle wound so its geometric normal agrees with `normal`.
fn push_tri(mesh: &mut Mesh, t: [u32; 3], normal: [f64; 3]) {
    let at = |i: u32| mesh.positions.get(i as usize).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).unwrap_or([0.0; 3]);
    let (a, b, c) = (at(t[0]), at(t[1]), at(t[2]));
    let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    let cr = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    if cr[0] * normal[0] + cr[1] * normal[1] + cr[2] * normal[2] >= 0.0 {
        mesh.indices.extend([t[0], t[1], t[2]]);
    } else {
        mesh.indices.extend([t[0], t[2], t[1]]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQ: [[f64; 2]; 4] = [[0.0, 0.0], [0.1, 0.0], [0.1, 0.1], [0.0, 0.1]];

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2::new(x, y)
    }

    /// Signed volume by the divergence theorem (positive when the winding is outward).
    fn volume(m: &Mesh) -> f64 {
        let at = |i: u32| m.positions.get(i as usize).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).unwrap_or([0.0; 3]);
        let mut vol = 0.0;
        for t in m.indices.chunks(3) {
            if let [i, j, k] = t {
                let (a, b, c) = (at(*i), at(*j), at(*k));
                let cr = [b[1] * c[2] - b[2] * c[1], b[2] * c[0] - b[0] * c[2], b[0] * c[1] - b[1] * c[0]];
                vol += (a[0] * cr[0] + a[1] * cr[1] + a[2] * cr[2]) / 6.0;
            }
        }
        vol
    }

    #[test]
    fn straight_square_sweep_contains_and_excludes() {
        let m = sweep(&SQ, &[v(0.0, 0.0), v(1.0, 0.0)], &[0.0, 0.0], false, 1.0, "wood", ArchId(1)).unwrap();
        assert!(m.is_manifold());
        assert!((volume(&m) - 0.01).abs() < 1e-6);
        let b = m.bounds();
        // Left of travel (+y), z 0..0.1; nothing at negative y or beyond the ends.
        assert!(b.min[1] > -1e-6 && (b.max[1] - 0.1).abs() < 1e-6);
        assert!(b.min[0].abs() < 1e-6 && (b.max[0] - 1.0).abs() < 1e-6);
        assert!(b.min[2].abs() < 1e-6 && (b.max[2] - 0.1).abs() < 1e-6);
        // Right-hand side flips it to -y.
        let r = sweep(&SQ, &[v(0.0, 0.0), v(1.0, 0.0)], &[0.0, 0.0], false, -1.0, "wood", ArchId(1)).unwrap();
        assert!((r.bounds().min[1] + 0.1).abs() < 1e-6 && r.bounds().max[1] < 1e-6);
        assert!((volume(&r) - 0.01).abs() < 1e-6);
        assert!(m.level_elevation.is_nan());
        assert_eq!(m.normals.len(), m.positions.len());
    }

    #[test]
    fn l_path_is_mitred() {
        let m = sweep(&SQ, &[v(0.0, 0.0), v(1.0, 0.0), v(1.0, 1.0)], &[0.0; 3], false, 1.0, "wood", ArchId(1)).unwrap();
        assert!(m.is_manifold());
        assert!((volume(&m) - 0.019).abs() < 1e-6);
        let inner = m.positions.iter().any(|p| (p[0] - 0.9).abs() < 1e-5 && (p[1] - 0.1).abs() < 1e-5);
        assert!(inner, "inner mitre corner at (0.9, 0.1)");
    }

    #[test]
    fn closed_rectangle_path_is_closed_and_capless() {
        let path = [v(0.0, 0.0), v(2.0, 0.0), v(2.0, 1.0), v(0.0, 1.0)];
        let m = sweep(&SQ, &path, &[0.0; 4], true, 1.0, "wood", ArchId(1)).unwrap();
        assert!(m.is_manifold());
        assert_eq!(m.triangle_count(), 4 * 4 * 2);
        assert!((volume(&m) - 0.056).abs() < 1e-6);
        // A closing point equal to the first is the same path.
        let mut again = path.to_vec();
        again.push(path[0]);
        let m2 = sweep(&SQ, &again, &[0.0; 5], true, 1.0, "wood", ArchId(1)).unwrap();
        assert_eq!(m2.triangle_count(), m.triangle_count());
    }

    #[test]
    fn z_per_vertex_follows_a_slope() {
        let m = sweep(&SQ, &[v(0.0, 0.0), v(1.0, 0.0)], &[0.0, 0.5], false, 1.0, "wood", ArchId(1)).unwrap();
        let b = m.bounds();
        assert!(b.min[2].abs() < 1e-6 && (b.max[2] - 0.6).abs() < 1e-6);
        assert!(m.positions.iter().any(|p| p[0] > 0.99 && (p[2] - 0.5).abs() < 1e-6));
    }

    #[test]
    fn profile_y_above_z() {
        let m = sweep(&SQ, &[v(0.0, 0.0), v(1.0, 0.0)], &[2.0, 2.0], false, 1.0, "wood", ArchId(1)).unwrap();
        let b = m.bounds();
        assert!((b.min[2] - 2.0).abs() < 1e-6 && (b.max[2] - 2.1).abs() < 1e-6);
    }

    #[test]
    fn sweep_refuses_degenerate_input() {
        let two = [v(0.0, 0.0), v(1.0, 0.0)];
        let s = |prof: &[[f64; 2]], path: &[Vec2], z: &[f64], closed: bool| sweep(prof, path, z, closed, 1.0, "m", ArchId(1));
        assert!(s(&SQ, &two, &[0.0, 0.0], false).is_some());
        assert!(s(&SQ, &two, &[0.0], false).is_none());
        assert!(s(&SQ, &two[..1], &[0.0], false).is_none());
        assert!(s(&SQ, &[v(0.0, 0.0), v(0.0, 0.0)], &[0.0, 0.0], false).is_none());
        assert!(s(&SQ, &two, &[0.0, 0.0], true).is_none());
        assert!(s(&SQ, &[v(0.0, 0.0), v(f64::NAN, 0.0)], &[0.0, 0.0], false).is_none());
        assert!(s(&SQ, &two, &[0.0, f64::INFINITY], false).is_none());
        assert!(s(&SQ[..2], &two, &[0.0, 0.0], false).is_none());
        let cw = [[0.0, 0.0], [0.0, 0.1], [0.1, 0.1], [0.1, 0.0]];
        assert!(s(&cw, &two, &[0.0, 0.0], false).is_none());
        // A hairpin turn is too sharp to mitre.
        assert!(s(&SQ, &[v(0.0, 0.0), v(1.0, 0.0), v(0.0, 0.01)], &[0.0; 3], false).is_none());
        let long: Vec<Vec2> = (0..4097).map(|i| v(i as f64, 0.0)).collect();
        assert!(s(&SQ, &long, &vec![0.0; 4097], false).is_none());
    }
}
