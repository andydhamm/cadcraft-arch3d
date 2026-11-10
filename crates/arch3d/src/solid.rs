//! A small solid kit for the phase-17 parts: extruded profiles in a flight frame, plan strips,
//! boxes and cylinders, and polygon clipping.
//!
//! Every function takes hostile numbers: anything not finite, or a degenerate shape, adds nothing.
//! Each countable part (a tread, a post, a joist ...) is one `Mesh::new(material, id)` the caller
//! fills with these helpers; never put two overlapping solids in one mesh, or the probe's parity
//! test sees the overlap as outside.

use cadcraft_geom::Vec2;

use crate::mesh::{Mesh, triangulate_simple};

/// The most points a profile ring may have.
const MAX_RING: usize = 4096;

/// A flight frame: `u` runs along `dir` from `origin`, `t` across to the left of travel, `v` up
/// from `base`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    /// Plan position of u = 0, t = 0.
    pub origin: Vec2,
    /// Unit direction of u.
    pub dir: Vec2,
    /// Absolute elevation of v = 0.
    pub base: f64,
}

impl Frame {
    /// A frame; `dir` is normalised. `None` when anything is not finite or `dir` is (nearly) zero.
    pub fn new(origin: Vec2, dir: Vec2, base: f64) -> Option<Frame> {
        if !origin.is_finite() || !dir.is_finite() || !base.is_finite() {
            return None;
        }
        let len = dir.len();
        if !len.is_finite() || len < 1e-12 {
            return None;
        }
        Some(Frame { origin, dir: dir / len, base })
    }

    /// The unit vector across the flight, to the left of travel.
    pub fn left(&self) -> Vec2 {
        self.dir.perp()
    }

    /// The world position of flight coordinates (`u`, `t`, `v`).
    pub fn world(&self, u: f64, t: f64, v: f64) -> [f32; 3] {
        let l = self.left();
        [(self.origin.x + u * self.dir.x + t * l.x) as f32, (self.origin.y + u * self.dir.y + t * l.y) as f32, (self.base + v) as f32]
    }
}

/// Twice the signed area of `ring` (positive when counter-clockwise), halved.
fn area(ring: &[Vec2]) -> f64 {
    let mut a = 0.0;
    for (i, p) in ring.iter().enumerate() {
        let q = ring.get((i + 1) % ring.len()).copied().unwrap_or(*p);
        a += p.x * q.y - q.x * p.y;
    }
    a / 2.0
}

/// Append the closed solid of the (u, v) polygon `ring` (either winding, at least 3 points)
/// extruded across `t` in [`t0`, `t1`] and mapped through `frame`. Faces are wound outward, normals
/// are flat and the uvs are (u, v). Adds nothing when anything is not finite, the ring is
/// degenerate or too long, or `t1 - t0` is not above 1e-9.
pub fn push_profile(mesh: &mut Mesh, ring: &[Vec2], t0: f64, t1: f64, frame: &Frame) {
    if ring.len() < 3 || ring.len() > MAX_RING || !t0.is_finite() || !t1.is_finite() || t1 - t0 <= 1e-9 {
        return;
    }
    if !ring.iter().all(|p| p.is_finite()) || !frame.origin.is_finite() || !frame.dir.is_finite() || !frame.base.is_finite() {
        return;
    }
    let a = area(ring);
    if !a.is_finite() || a.abs() < 1e-12 {
        return;
    }
    // Counter-clockwise in (u, v) is the normal -t (u x v = -t in the right-handed (u, t, v) frame).
    let ring: Vec<Vec2> = if a < 0.0 { ring.iter().rev().copied().collect() } else { ring.to_vec() };
    let tris = triangulate_simple(&ring);
    if tris.is_empty() {
        return;
    }
    // A mirroring frame turns every face inside out: flip the winding then.
    let l = frame.left();
    let mirror = frame.dir.x * l.y - frame.dir.y * l.x < 0.0;
    let n = ring.len() as u32;
    let base = mesh.positions.len() as u32;
    let lf = [l.x as f32, l.y as f32, 0.0];
    let ln = [-lf[0], -lf[1], 0.0];
    let mut tri = |mesh: &mut Mesh, i: [u32; 3]| {
        let i = if mirror { [i[0], i[2], i[1]] } else { i };
        mesh.indices.extend(i);
    };
    // Cap at t0 (normal -t), then cap at t1 (normal +t).
    for (t, normal) in [(t0, ln), (t1, lf)] {
        for p in &ring {
            mesh.positions.push(frame.world(p.x, t, p.y));
            mesh.normals.push(normal);
            mesh.uvs.push([p.x as f32, p.y as f32]);
        }
    }
    for t in &tris {
        tri(mesh, [base + t[0], base + t[1], base + t[2]]);
        tri(mesh, [base + n + t[0], base + n + t[2], base + n + t[1]]);
    }
    // Side faces: four vertices per ring edge with the edge's own flat normal.
    for (i, p) in ring.iter().enumerate() {
        let q = ring.get((i + 1) % ring.len()).copied().unwrap_or(*p);
        let (du, dv) = (q.x - p.x, q.y - p.y);
        let len = (du * du + dv * dv).sqrt();
        if len <= 0.0 || !len.is_finite() {
            continue;
        }
        // Outward in (u, v) for a counter-clockwise ring is to the right of the edge.
        let (nu, nv) = (dv / len, -du / len);
        let normal = [(frame.dir.x * nu) as f32, (frame.dir.y * nu) as f32, nv as f32];
        let s = mesh.positions.len() as u32;
        for (pt, t) in [(p, t0), (&q, t0), (&q, t1), (p, t1)] {
            mesh.positions.push(frame.world(pt.x, t, pt.y));
            mesh.normals.push(normal);
            mesh.uvs.push([pt.x as f32, pt.y as f32]);
        }
        tri(mesh, [s, s + 2, s + 1]);
        tri(mesh, [s, s + 3, s + 2]);
    }
}

/// Append the plan strip of `width` centred on the line `a` -> `b`, from `z0` to `z1`, as a prism.
/// Adds nothing for non-finite numbers, a zero-length line, a width of 1e-9 or less or `z1 - z0`
/// of 1e-9 or less.
pub fn push_member(mesh: &mut Mesh, a: Vec2, b: Vec2, width: f64, z0: f64, z1: f64) {
    if !a.is_finite() || !b.is_finite() || !width.is_finite() || !z0.is_finite() || !z1.is_finite() || width <= 1e-9 || z1 - z0 <= 1e-9 {
        return;
    }
    let d = b - a;
    let len = d.len();
    if !len.is_finite() || len < 1e-9 {
        return;
    }
    let n = d.perp() * (width / 2.0 / len);
    mesh.push_prism(&[a - n, b - n, b + n, a + n], z0, z1);
}

/// Append a box of `length` along `axis` and `width` across it, centred on `centre`, from `z0` to
/// `z1`. Adds nothing for non-finite numbers or a zero `axis`.
pub fn push_box(mesh: &mut Mesh, centre: Vec2, axis: Vec2, length: f64, width: f64, z0: f64, z1: f64) {
    if !centre.is_finite() || !axis.is_finite() || !length.is_finite() || axis.len() < 1e-12 || length <= 1e-9 {
        return;
    }
    let d = axis.normalized() * (length / 2.0);
    push_member(mesh, centre - d, centre + d, width, z0, z1);
}

/// Append an upright cylinder of `radius` on `centre`, from `z0` to `z1`, with `sides` clamped to
/// 6..=64. Adds nothing for non-finite numbers or a radius of 1e-9 or less.
pub fn push_cylinder(mesh: &mut Mesh, centre: Vec2, radius: f64, sides: usize, z0: f64, z1: f64) {
    if !centre.is_finite() || !radius.is_finite() || radius <= 1e-9 || !z0.is_finite() || !z1.is_finite() || z1 - z0 <= 1e-9 {
        return;
    }
    let sides = sides.clamp(6, 64);
    let ring: Vec<Vec2> = (0..sides).map(|i| Vec2::polar(centre, radius, std::f64::consts::TAU * i as f64 / sides as f64)).collect();
    mesh.push_prism(&ring, z0, z1);
}

/// Clip `poly` to the left side of the directed line `a` -> `b` (Sutherland-Hodgman). Empty for
/// non-finite input or a zero-length line.
pub fn clip_left(poly: &[Vec2], a: Vec2, b: Vec2) -> Vec<Vec2> {
    if !a.is_finite() || !b.is_finite() || (b - a).len() < 1e-12 || !poly.iter().all(|p| p.is_finite()) {
        return Vec::new();
    }
    let d = b - a;
    let side = |p: Vec2| d.cross(p - a);
    let mut out = Vec::new();
    for (i, &p) in poly.iter().enumerate() {
        let q = poly.get((i + 1) % poly.len()).copied().unwrap_or(p);
        let (sp, sq) = (side(p), side(q));
        if sp >= 0.0 {
            out.push(p);
        }
        if (sp >= 0.0) != (sq >= 0.0) {
            let k = sp / (sp - sq);
            if k.is_finite() {
                out.push(p.lerp(q, k));
            }
        }
    }
    out
}

/// Clip `poly` to the convex counter-clockwise polygon `clip`. Empty when nothing is left.
pub fn clip_convex(poly: &[Vec2], clip: &[Vec2]) -> Vec<Vec2> {
    let mut out = poly.to_vec();
    for (i, &a) in clip.iter().enumerate() {
        let b = clip.get((i + 1) % clip.len()).copied().unwrap_or(a);
        out = clip_left(&out, a, b);
        if out.is_empty() {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_geom::Vec3;

    fn mesh() -> Mesh {
        Mesh::new("stair-tread", ArchId(0))
    }

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2::new(x, y)
    }

    /// Signed volume of a closed mesh (positive when the faces point outward).
    fn volume(m: &Mesh) -> f64 {
        let mut vol = 0.0;
        for t in m.indices.chunks(3) {
            let p = |i: u32| m.positions.get(i as usize).map_or([0.0; 3], |p| [p[0] as f64, p[1] as f64, p[2] as f64]);
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            vol += a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0]);
        }
        vol / 6.0
    }

    fn frame() -> Frame {
        Frame::new(v(2.0, 1.0), v(0.0, 2.0), 0.5).expect("a valid frame")
    }

    #[test]
    fn push_profile_is_closed_and_outward() {
        let l_shape = [v(0.0, 0.0), v(0.4, 0.0), v(0.4, 0.2), v(0.2, 0.2), v(0.2, 0.5), v(0.0, 0.5)];
        for ring in [l_shape.to_vec(), l_shape.iter().rev().copied().collect::<Vec<_>>()] {
            let mut m = mesh();
            push_profile(&mut m, &ring, -0.5, 0.5, &frame());
            assert!(m.is_manifold());
            // The area 0.4 * 0.2 + 0.2 * 0.3 = 0.14 times the 1.0 width.
            assert!((volume(&m) - 0.14).abs() < 1e-5, "outward and the right size: {}", volume(&m));
            assert_eq!(m.positions.len(), m.normals.len());
            assert_eq!(m.positions.len(), m.uvs.len());
            // Every face normal agrees with its flat normal.
            for t in m.indices.chunks(3) {
                let p = |i: u32| m.positions.get(i as usize).copied().unwrap_or_default();
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                let given = m.normals.get(t[0] as usize).copied().unwrap_or_default();
                assert!(n[0] * given[0] + n[1] * given[1] + n[2] * given[2] >= -1e-9, "winding agrees with the normal");
            }
        }
    }

    #[test]
    fn push_profile_maps_u_v_t() {
        // dir (0, 1): u runs along +y, t to the left (-x), v up from base 0.5.
        let mut m = mesh();
        push_profile(&mut m, &[v(0.0, 0.0), v(1.0, 0.0), v(1.0, 0.1), v(0.0, 0.1)], -0.25, 0.25, &frame());
        let b = m.bounds();
        assert!((b.min[0] - 1.75).abs() < 1e-6 && (b.max[0] - 2.25).abs() < 1e-6, "t across x: {:?}", b);
        assert!((b.min[1] - 1.0).abs() < 1e-6 && (b.max[1] - 2.0).abs() < 1e-6, "u along y: {:?}", b);
        assert!((b.min[2] - 0.5).abs() < 1e-6 && (b.max[2] - 0.6).abs() < 1e-6, "v up from the base: {:?}", b);
        // t > 0 is the left of travel: x decreases.
        let w = frame().world(0.0, 1.0, 0.0);
        assert!((w[0] - 1.0).abs() < 1e-6 && (w[1] - 1.0).abs() < 1e-6);
        // The uvs are (u, v).
        assert!(m.uvs.iter().all(|uv| uv[0] >= -1e-6 && uv[0] <= 1.0 + 1e-6 && uv[1] >= -1e-6 && uv[1] <= 0.1 + 1e-6));
    }

    #[test]
    fn push_member_box_cylinder_probe_inside_and_outside() {
        let inside = |m: &Mesh, p: Vec3| crate::probe::probe_mesh(m, p);
        let mut member = mesh();
        push_member(&mut member, v(0.0, 0.0), v(2.0, 0.0), 0.2, 1.0, 1.5);
        assert!(member.is_manifold());
        assert!(inside(&member, Vec3::new(1.0, 0.05, 1.25)));
        assert!(!inside(&member, Vec3::new(1.0, 0.25, 1.25)));
        assert!(!inside(&member, Vec3::new(1.0, 0.05, 1.75)));
        let mut b = mesh();
        push_box(&mut b, v(1.0, 1.0), v(0.0, 3.0), 0.5, 0.2, 0.0, 1.0);
        assert!(b.is_manifold());
        assert!(inside(&b, Vec3::new(1.05, 1.2, 0.5)));
        assert!(!inside(&b, Vec3::new(1.05, 1.4, 0.5)));
        let mut c = mesh();
        push_cylinder(&mut c, v(0.0, 0.0), 0.15, 2, 0.0, 0.6);
        assert!(c.is_manifold());
        assert_eq!(c.positions.len(), 6 * 2 + 6 * 4, "sides clamp up to 6");
        assert!(inside(&c, Vec3::new(0.05, 0.05, 0.3)));
        assert!(!inside(&c, Vec3::new(0.2, 0.0, 0.3)));
        let mut many = mesh();
        push_cylinder(&mut many, v(0.0, 0.0), 0.15, 1000, 0.0, 0.6);
        assert_eq!(many.positions.len(), 64 * 2 + 64 * 4, "sides clamp down to 64");
    }

    #[test]
    fn clip_left_and_clip_convex() {
        let square = [v(0.0, 0.0), v(2.0, 0.0), v(2.0, 2.0), v(0.0, 2.0)];
        // Left of the line x = 1 going up is x < 1.
        let half = clip_left(&square, v(1.0, 0.0), v(1.0, 1.0));
        assert!((area(&half) - 2.0).abs() < 1e-9 && half.iter().all(|p| p.x <= 1.0 + 1e-9));
        assert_eq!(clip_left(&square, v(5.0, 1.0), v(5.0, 0.0)).len(), 0, "all on the right");
        assert_eq!(clip_left(&square, v(5.0, 0.0), v(5.0, 1.0)).len(), 4, "all on the left");
        let tri = [v(1.0, 1.0), v(3.0, 1.0), v(1.0, 3.0)];
        let cut = clip_convex(&tri, &square);
        // The triangle (1,1) (3,1) (1,3) cut to the square is the unit square [1, 2] x [1, 2].
        assert!((area(&cut) - 1.0).abs() < 1e-9, "{}", area(&cut));
        assert!(clip_convex(&[v(5.0, 5.0), v(6.0, 5.0), v(6.0, 6.0)], &square).is_empty());
    }

    #[test]
    fn solid_helpers_survive_nan_huge_and_empty() {
        let nan = f64::NAN;
        let big = 1e300;
        let ring = [v(0.0, 0.0), v(1.0, 0.0), v(1.0, 1.0)];
        let mut m = mesh();
        let f = frame();
        push_profile(&mut m, &[], 0.0, 1.0, &f);
        push_profile(&mut m, &ring[..2], 0.0, 1.0, &f);
        push_profile(&mut m, &[v(nan, 0.0), v(1.0, 0.0), v(1.0, 1.0)], 0.0, 1.0, &f);
        push_profile(&mut m, &ring, nan, 1.0, &f);
        push_profile(&mut m, &ring, 1.0, 1.0, &f);
        push_profile(&mut m, &[v(0.0, 0.0), v(1.0, 0.0), v(2.0, 0.0)], 0.0, 1.0, &f);
        push_profile(&mut m, &ring, 0.0, 1.0, &Frame { origin: v(nan, 0.0), ..f });
        push_member(&mut m, v(nan, 0.0), v(1.0, 0.0), 0.1, 0.0, 1.0);
        push_member(&mut m, v(0.0, 0.0), v(0.0, 0.0), 0.1, 0.0, 1.0);
        push_member(&mut m, v(0.0, 0.0), v(1.0, 0.0), 0.0, 0.0, 1.0);
        push_member(&mut m, v(0.0, 0.0), v(1.0, 0.0), 0.1, 1.0, 1.0);
        push_box(&mut m, v(0.0, 0.0), v(0.0, 0.0), 1.0, 1.0, 0.0, 1.0);
        push_box(&mut m, v(0.0, 0.0), v(1.0, 0.0), nan, 1.0, 0.0, 1.0);
        push_cylinder(&mut m, v(0.0, 0.0), -1.0, 8, 0.0, 1.0);
        push_cylinder(&mut m, v(0.0, 0.0), 1.0, 8, nan, 1.0);
        assert!(m.positions.is_empty() && m.indices.is_empty());
        assert!(Frame::new(v(0.0, 0.0), v(0.0, 0.0), 0.0).is_none());
        assert!(Frame::new(v(nan, 0.0), v(1.0, 0.0), 0.0).is_none());
        assert!(Frame::new(v(0.0, 0.0), v(1.0, 0.0), f64::INFINITY).is_none());
        assert!(clip_left(&[], v(0.0, 0.0), v(1.0, 0.0)).is_empty());
        assert!(clip_left(&ring, v(nan, 0.0), v(1.0, 0.0)).is_empty());
        assert!(clip_left(&[v(nan, 0.0), v(1.0, 0.0), v(1.0, 1.0)], v(0.0, 0.0), v(1.0, 0.0)).is_empty());
        assert!(clip_convex(&ring, &[]).len() == 3, "no clip edge, nothing to cut");
        // Huge numbers do not panic.
        push_profile(&mut m, &[v(0.0, 0.0), v(big, 0.0), v(big, big)], 0.0, big, &f);
        push_member(&mut m, v(-big, 0.0), v(big, 0.0), big, -big, big);
        push_box(&mut m, v(big, big), v(1.0, 1.0), big, big, 0.0, big);
        push_cylinder(&mut m, v(big, 0.0), big, 12, 0.0, big);
        let _ = clip_convex(&[v(-big, -big), v(big, -big), v(0.0, big)], &[v(-big, -big), v(big, -big), v(big, big), v(-big, big)]);
    }
}
