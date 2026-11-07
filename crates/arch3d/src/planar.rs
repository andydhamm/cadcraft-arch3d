//! The planar kit (phase 16): planes `z = a·x + b·y + c` over plan regions, the cells a region
//! splits into under a set of outlines, and closed solids between two planes.
//!
//! These are the shared tools of vaulted ceilings, wall rises, trays, soffits and foundations. Every
//! function takes hostile input (NaN, 1e300, empty rings) and returns nothing for it rather than
//! panicking.

use cadcraft_doc::arch::RoofPlane;
use cadcraft_geom::Vec2;

use crate::mesh::{Mesh, find_bridge_pair, merge_ring, triangulate_simple};

/// Most triangles [`region_triangles`] returns (and the longest cap ring it triangulates).
pub const MAX_REGION_TRIANGLES: usize = 100_000;
/// Most cells [`split_by_rings`] makes by splitting; the triangles after that come back unsplit.
pub const MAX_CELLS: usize = 50_000;
/// Most edge-against-triangle tests [`split_by_rings`] makes; the triangles after that come back
/// unsplit.
const MAX_SPLIT_TESTS: usize = 20_000_000;
/// A top within this of its bottom has zero height.
const EPS_Z: f64 = 1e-9;
/// A part with less area than this (m²) is empty.
const MIN_AREA: f64 = 1e-12;
/// A signed distance within this of a line is on it.
const EPS_LINE: f64 = 1e-12;

/// The plane `z = a·x + b·y + c`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlaneZ {
    pub a: f64,
    pub b: f64,
    pub c: f64,
}

impl PlaneZ {
    /// The plane `z = a·x + b·y + c`.
    pub const fn new(a: f64, b: f64, c: f64) -> Self {
        PlaneZ { a, b, c }
    }

    /// The level plane `z = z`.
    pub const fn flat(z: f64) -> Self {
        PlaneZ { a: 0.0, b: 0.0, c: z }
    }

    /// The plane's height over `p` (exactly `c` for a level plane).
    pub fn z_at(&self, p: Vec2) -> f64 {
        if self.is_flat() { self.c } else { self.a * p.x + self.b * p.y + self.c }
    }

    /// Whether the plane is level.
    pub fn is_flat(&self) -> bool {
        self.a == 0.0 && self.b == 0.0
    }

    /// The plane moved up by `dz` (down for a negative `dz`).
    pub fn offset(&self, dz: f64) -> Self {
        PlaneZ { c: self.c + dz, ..*self }
    }

    /// Whether `a`, `b` and `c` are all finite.
    pub fn is_finite(&self) -> bool {
        self.a.is_finite() && self.b.is_finite() && self.c.is_finite()
    }
}

/// A plan region (an outer ring and its holes) and the plane over it.
///
/// Build one with [`Piece::new`] outside this file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Piece {
    pub outer: Vec<Vec2>,
    pub holes: Vec<Vec<Vec2>>,
    pub plane: PlaneZ,
}

impl Piece {
    /// The region `outer` minus `holes`, under `plane`.
    pub fn new(outer: Vec<Vec2>, holes: Vec<Vec<Vec2>>, plane: PlaneZ) -> Self {
        Piece { outer, holes, plane }
    }
}

/// The underside of a roof plane as a [`PlaneZ`]: equal to
/// [`crate::roof::underside_z`]`(plane, plane.eave_z, p)` at every `p`. `None` when a number is
/// not finite or the base edge has no length.
pub fn underside_plane(plane: &RoofPlane) -> Option<PlaneZ> {
    let [x0, y0, x1, y1] = plane.base_edge;
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len_sq = dx * dx + dy * dy;
    if !len_sq.is_finite() || len_sq < 1e-12 || !x0.is_finite() || !y0.is_finite() || !plane.slope.is_finite() || !plane.eave_z.is_finite() {
        return None;
    }
    let len = len_sq.sqrt();
    // The base edge's left normal, as `roof::underside_z` measures the distance.
    let (nx, ny) = (-dy / len, dx / len);
    let overhang = if plane.overhang.is_finite() && plane.overhang >= 0.0 { plane.overhang } else { 0.0 };
    let out = PlaneZ::new(plane.slope * nx, plane.slope * ny, plane.eave_z - plane.slope * (x0 * nx + y0 * ny + overhang));
    out.is_finite().then_some(out)
}

/// Signed area of `ring` (positive counter-clockwise); 0 for fewer than three points.
pub fn area(ring: &[Vec2]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let next = ring.iter().cycle().skip(1);
    ring.iter().zip(next).map(|(p, q)| p.cross(*q)).sum::<f64>() / 2.0
}

/// The area centroid of `poly`, or the mean of its points when it has no area; `None` for an
/// empty or non-finite polygon.
pub fn centroid(poly: &[Vec2]) -> Option<Vec2> {
    if poly.is_empty() || !poly.iter().all(|p| p.is_finite()) {
        return None;
    }
    let a = area(poly);
    let mean = poly.iter().fold(Vec2::ZERO, |s, p| s + *p) / poly.len() as f64;
    let c = if a.is_finite() && a.abs() >= MIN_AREA {
        let next = poly.iter().cycle().skip(1);
        let sum = poly.iter().zip(next).fold(Vec2::ZERO, |s, (p, q)| s + (*p + *q) * p.cross(*q));
        sum / (6.0 * a)
    } else {
        mean
    };
    if c.is_finite() {
        Some(c)
    } else if mean.is_finite() {
        Some(mean)
    } else {
        None
    }
}

/// Whether `p` is inside `ring` (even-odd).
fn inside(p: Vec2, ring: &[Vec2]) -> bool {
    let mut odd = false;
    let next = ring.iter().cycle().skip(1);
    for (a, b) in ring.iter().zip(next) {
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y);
            if p.x < x {
                odd = !odd;
            }
        }
    }
    odd
}

/// `(min, max)` corners of `pts`.
fn bbox(pts: &[Vec2]) -> (Vec2, Vec2) {
    let first = pts.first().copied().unwrap_or_default();
    pts.iter().fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)))
}

fn boxes_meet(a: (Vec2, Vec2), b: (Vec2, Vec2)) -> bool {
    a.0.x <= b.1.x && b.0.x <= a.1.x && a.0.y <= b.1.y && b.0.y <= a.1.y
}

/// A ring of at least three finite points with area: as given when `ccw` matches its winding,
/// else reversed.
fn oriented(ring: &[Vec2], ccw: bool) -> Option<Vec<Vec2>> {
    if ring.len() < 3 || !ring.iter().all(|p| p.is_finite()) {
        return None;
    }
    let a = area(ring);
    if !a.is_finite() || a.abs() < MIN_AREA {
        return None;
    }
    Some(if (a > 0.0) == ccw { ring.to_vec() } else { ring.iter().rev().copied().collect() })
}

/// The cap ring of `outer` minus `holes` (counter-clockwise, every hole that bridges spliced in,
/// as [`Mesh::push_prism_with_holes`] splices them) and its triangles as indices into it.
fn cap(outer: &[Vec2], holes: &[Vec<Vec2>]) -> Option<(Vec<Vec2>, Vec<[u32; 3]>)> {
    if outer.len() > MAX_REGION_TRIANGLES {
        return None;
    }
    let mut ring = oriented(outer, true)?;
    for hole in holes.iter().filter_map(|h| oriented(h, false)) {
        if ring.len().saturating_add(hole.len()).saturating_add(2) > MAX_REGION_TRIANGLES {
            break;
        }
        if let Some((p, q)) = find_bridge_pair(&ring, &hole) {
            ring = merge_ring(ring, p, &hole, q);
            ring.dedup();
        }
    }
    let tris = triangulate_simple(&ring);
    (!tris.is_empty()).then_some((ring, tris))
}

/// The triangles of `outer` minus `holes`, counter-clockwise, from the cap triangulation
/// [`Mesh::push_prism_with_holes`] uses (holes that cannot be bridged are left out, as there).
/// At most [`MAX_REGION_TRIANGLES`]; empty for an invalid region.
pub fn region_triangles(outer: &[Vec2], holes: &[Vec<Vec2>]) -> Vec<[Vec2; 3]> {
    let Some((ring, tris)) = cap(outer, holes) else {
        return Vec::new();
    };
    tris.iter()
        .filter_map(|t| {
            let [a, b, c] = t.map(|i| ring.get(i as usize).copied());
            let tri = [a?, b?, c?];
            ((tri[1] - tri[0]).cross(tri[2] - tri[0]) > 0.0).then_some(tri)
        })
        .take(MAX_REGION_TRIANGLES)
        .collect()
}

/// `poly` without repeated consecutive points (first and last included); empty when it has less
/// than [`MIN_AREA`] left.
fn finish_part(mut poly: Vec<Vec2>) -> Vec<Vec2> {
    poly.dedup();
    while poly.len() > 1 && poly.first() == poly.last() {
        poly.pop();
    }
    if poly.len() < 3 || !area(&poly).is_finite() || area(&poly).abs() < MIN_AREA { Vec::new() } else { poly }
}

/// The part of convex `poly` where the signed distances `s` (one per point) are >= 0.
fn clip_half(poly: &[Vec2], s: &[f64]) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(poly.len() + 2);
    let next = poly.iter().zip(s).cycle().skip(1);
    for ((p, sp), (q, sq)) in poly.iter().zip(s).zip(next) {
        if *sp >= 0.0 {
            out.push(*p);
        }
        if (*sp > 0.0 && *sq < 0.0) || (*sp < 0.0 && *sq > 0.0) {
            out.push(*p + (*q - *p) * (*sp / (*sp - *sq)));
        }
    }
    finish_part(out)
}

/// `poly` split where the signed distances `s` change sign: `(s >= 0 part, s <= 0 part)`.
/// Distances within [`EPS_LINE`] of zero count as zero.
fn split_signed(poly: &[Vec2], mut s: Vec<f64>) -> (Vec<Vec2>, Vec<Vec2>) {
    for v in &mut s {
        if v.abs() <= EPS_LINE {
            *v = 0.0;
        }
    }
    let neg: Vec<f64> = s.iter().map(|v| -v).collect();
    (clip_half(poly, &s), clip_half(poly, &neg))
}

/// A polygon worth splitting: at least three finite points with area.
fn valid(poly: &[Vec2]) -> bool {
    poly.len() >= 3 && poly.iter().all(|p| p.is_finite()) && area(poly).is_finite() && area(poly).abs() >= MIN_AREA
}

/// Convex `poly` split by the line through `a` and `b`: `(left of a→b, right of it)`. A part with
/// less than 1e-12 m² comes back empty; a line without a direction leaves `poly` on the left.
/// Both parts keep `poly`'s winding.
pub fn split_by_line(poly: &[Vec2], a: Vec2, b: Vec2) -> (Vec<Vec2>, Vec<Vec2>) {
    if !valid(poly) {
        return (Vec::new(), Vec::new());
    }
    let d = b - a;
    let len = d.len();
    if !a.is_finite() || !len.is_finite() || len < 1e-12 {
        return (finish_part(poly.to_vec()), Vec::new());
    }
    let s: Vec<f64> = poly.iter().map(|p| d.cross(*p - a) / len).collect();
    if !s.iter().all(|v| v.is_finite()) {
        return (Vec::new(), Vec::new());
    }
    split_signed(poly, s)
}

/// Convex `poly` split where `plane` crosses height `z`: `(part where plane < z, part where
/// plane >= z)`. A level plane puts all of `poly` on one side.
pub fn split_by_level(poly: &[Vec2], plane: PlaneZ, z: f64) -> (Vec<Vec2>, Vec<Vec2>) {
    if !valid(poly) || !plane.is_finite() || !z.is_finite() {
        return (Vec::new(), Vec::new());
    }
    let s: Vec<f64> = poly.iter().map(|p| z - plane.z_at(*p)).collect();
    if !s.iter().all(|v| v.is_finite()) {
        return (Vec::new(), Vec::new());
    }
    if s.iter().all(|v| *v <= EPS_LINE) {
        return (Vec::new(), finish_part(poly.to_vec()));
    }
    split_signed(poly, s)
}

/// Convex cells covering `outer` minus `holes`: its triangles, each split by the line of every
/// edge of `rings` whose box meets the triangle's box, so no ring edge crosses a cell. Each cell
/// comes with the indices of the rings holding its centroid. Splitting stops at [`MAX_CELLS`]
/// cells; the triangles after that come back unsplit.
pub fn split_by_rings(outer: &[Vec2], holes: &[Vec<Vec2>], rings: &[Vec<Vec2>]) -> Vec<(Vec<Vec2>, Vec<usize>)> {
    let tris = region_triangles(outer, holes);
    let usable: Vec<(usize, &Vec<Vec2>)> = rings.iter().enumerate().filter(|(_, r)| valid(r)).collect();
    let edges: Vec<(Vec2, Vec2, (Vec2, Vec2))> = usable
        .iter()
        .flat_map(|&(_, r)| r.iter().zip(r.iter().cycle().skip(1)).map(|(a, b)| (*a, *b, bbox(&[*a, *b]))))
        .filter(|(a, b, _)| a != b)
        .collect();
    let mut out: Vec<(Vec<Vec2>, Vec<usize>)> = Vec::new();
    let mut tests = 0usize;
    for tri in tris {
        let mut cells = vec![tri.to_vec()];
        let tb = bbox(&tri);
        for (a, b, eb) in &edges {
            if out.len().saturating_add(cells.len()) >= MAX_CELLS || tests >= MAX_SPLIT_TESTS {
                break;
            }
            tests += 1;
            if !boxes_meet(*eb, tb) {
                continue;
            }
            cells = cells
                .into_iter()
                .flat_map(|c| {
                    let (l, r) = split_by_line(&c, *a, *b);
                    [l, r]
                })
                .filter(|c| !c.is_empty())
                .collect();
        }
        for cell in cells {
            let holders = centroid(&cell).map(|c| usable.iter().filter(|&&(_, r)| inside(c, r)).map(|&(i, _)| i).collect()).unwrap_or_default();
            out.push((cell, holders));
        }
    }
    out
}

/// The cells of `outer` minus `holes` under the given planes (each a plan outline and its plane),
/// as [`split_by_rings`] makes them: each cell with the plane, among those whose outline holds its
/// centroid, lowest at the centroid; `None` where no outline holds it.
pub fn under_planes(outer: &[Vec2], holes: &[Vec<Vec2>], planes: &[(Vec<Vec2>, PlaneZ)]) -> Vec<(Vec<Vec2>, Option<PlaneZ>)> {
    let rings: Vec<Vec<Vec2>> = planes.iter().map(|(r, _)| r.clone()).collect();
    split_by_rings(outer, holes, &rings)
        .into_iter()
        .map(|(cell, holders)| {
            let lowest = centroid(&cell).and_then(|c| {
                holders
                    .iter()
                    .filter_map(|&i| planes.get(i).map(|(_, p)| *p))
                    .filter(|p| p.is_finite() && p.z_at(c).is_finite())
                    .min_by(|p, q| p.z_at(c).total_cmp(&q.z_at(c)))
            });
            (cell, lowest)
        })
        .collect()
}

/// Unit vector of `v`, or `fallback` when it has no length.
fn unit(v: [f64; 3], fallback: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l.is_finite() && l > 1e-15 { [(v[0] / l) as f32, (v[1] / l) as f32, (v[2] / l) as f32] } else { fallback }
}

/// The outward horizontal normal of edge `a`→`b` of a counter-clockwise ring.
fn side_normal(a: Vec2, b: Vec2) -> [f32; 3] {
    unit([b.y - a.y, -(b.x - a.x), 0.0], [0.0, 0.0, 0.0])
}

/// Whether a point's coordinates survive the mesh's `f32` positions.
fn fits_f32(p: Vec2) -> bool {
    (p.x as f32).is_finite() && (p.y as f32).is_finite()
}

/// Quantised plan position, the key `Mesh::is_manifold` welds by.
fn key(p: Vec2) -> (i64, i64) {
    ((p.x * 1e6).round() as i64, (p.y * 1e6).round() as i64)
}

/// For each edge `i → i+1` of the cap ring: whether its side face is left out (a bridge seam,
/// which runs twice between the same two positions, or an edge without length).
fn seams(ring: &[Vec2]) -> Vec<bool> {
    let pair = |(a, b): (&Vec2, &Vec2)| {
        let (ka, kb) = (key(*a), key(*b));
        if ka <= kb { (ka, kb) } else { (kb, ka) }
    };
    let edges = || ring.iter().zip(ring.iter().cycle().skip(1));
    let mut count: std::collections::HashMap<((i64, i64), (i64, i64)), u32> = std::collections::HashMap::new();
    for e in edges().filter(|(a, b)| key(**a) != key(**b)) {
        *count.entry(pair(e)).or_insert(0) += 1;
    }
    edges().map(|e| key(*e.0) == key(*e.1) || count.get(&pair(e)).is_some_and(|&n| n >= 2)).collect()
}

/// Append to `mesh` the closed solid between `bottom` and `top` over `outer` minus `holes`: a cap
/// on each plane and vertical sides.
///
/// Nothing is added when a number is not finite, when the top dips below the bottom at some ring
/// point (by more than 1e-9), or when the top is nowhere above the bottom (no volume). A ring point
/// where the two are within 1e-9 has zero height: its top is its bottom; a side face whose two ends
/// both have zero height is left out and one with a single zero-height end is one triangle, so a
/// region whose top meets its bottom along an edge or at a point is a closed wedge. For two level
/// planes this is [`Mesh::push_prism_with_holes`] from `bottom.c` to `top.c`.
pub fn push_between(mesh: &mut Mesh, outer: &[Vec2], holes: &[Vec<Vec2>], bottom: PlaneZ, top: PlaneZ) {
    if !bottom.is_finite() || !top.is_finite() || !outer.iter().all(|p| fits_f32(*p)) {
        return;
    }
    // A hole the mesh cannot hold is left out.
    let kept: Vec<Vec<Vec2>>;
    let holes = if holes.iter().all(|h| h.iter().all(|p| fits_f32(*p))) {
        holes
    } else {
        kept = holes.iter().filter(|h| h.iter().all(|p| fits_f32(*p))).cloned().collect();
        kept.as_slice()
    };
    if bottom.is_flat() && top.is_flat() {
        if top.c > bottom.c + EPS_Z && (bottom.c as f32).is_finite() && (top.c as f32).is_finite() {
            mesh.push_prism_with_holes(outer, holes, bottom.c, top.c);
        }
        return;
    }
    let Some((ring, tris)) = cap(outer, holes) else {
        return;
    };
    let mut zb = Vec::with_capacity(ring.len());
    let mut zt = Vec::with_capacity(ring.len());
    for p in &ring {
        let (b, t) = (bottom.z_at(*p), top.z_at(*p));
        if !(b as f32).is_finite() || !(t as f32).is_finite() || t < b - EPS_Z {
            return;
        }
        zb.push(b);
        zt.push(if t > b + EPS_Z { t } else { b });
    }
    if zb.iter().zip(&zt).all(|(b, t)| t <= b) {
        return;
    }
    emit_between(mesh, &ring, &tris, &zb, &zt, (bottom, top));
}

/// The vertices and triangles of [`push_between`], from the cap ring, its triangles and the bottom
/// and top height at every ring point.
fn emit_between(mesh: &mut Mesh, ring: &[Vec2], tris: &[[u32; 3]], zb: &[f64], zt: &[f64], (bottom, top): (PlaneZ, PlaneZ)) {
    let m = ring.len();
    let Ok(base) = u32::try_from(mesh.positions.len()) else { return };
    let Ok(m32) = u32::try_from(m) else { return };
    // Room for the caps (2m) and at most four side vertices per edge.
    if u64::from(base) + 6 * u64::from(m32) > u64::from(u32::MAX) {
        return;
    }
    let down = unit([bottom.a, bottom.b, -1.0], [0.0, 0.0, -1.0]);
    let up = unit([-top.a, -top.b, 1.0], [0.0, 0.0, 1.0]);
    for (heights, normal) in [(zb, down), (zt, up)] {
        for (p, z) in ring.iter().zip(heights) {
            mesh.positions.push([p.x as f32, p.y as f32, *z as f32]);
            mesh.uvs.push([p.x as f32, *z as f32]);
            mesh.normals.push(normal);
        }
    }
    let flat_at = |i: u32| zb.get(i as usize).zip(zt.get(i as usize)).is_none_or(|(b, t)| t <= b);
    for t in tris {
        if t.iter().all(|&i| flat_at(i)) {
            continue; // the top lies on the bottom here: no cap on either side
        }
        let [i, j, k] = *t;
        mesh.indices.extend([base + i, base + k, base + j]);
        mesh.indices.extend([base + m32 + i, base + m32 + j, base + m32 + k]);
    }
    let skip = seams(ring);
    let ends = ring.iter().zip(zb.iter().zip(zt)).enumerate();
    let next = ring.iter().zip(zb.iter().zip(zt)).cycle().skip(1);
    for ((i, (a, (a0, a1))), (b, (b0, b1))) in ends.zip(next) {
        if skip.get(i).copied().unwrap_or(true) {
            continue;
        }
        let (flat_a, flat_b) = (a1 <= a0, b1 <= b0);
        let quad = [(a, a0), (b, b0), (b, b1), (a, a1)];
        let corners: &[usize] = match (flat_a, flat_b) {
            (true, true) => continue,
            (true, false) => &[0, 1, 2],
            (false, true) => &[0, 1, 3],
            (false, false) => &[0, 1, 2, 3],
        };
        let Ok(s0) = u32::try_from(mesh.positions.len()) else { return };
        let normal = side_normal(*a, *b);
        for &c in corners {
            let Some((p, z)) = quad.get(c) else { continue };
            mesh.positions.push([p.x as f32, p.y as f32, **z as f32]);
            mesh.uvs.push([p.x as f32, **z as f32]);
            mesh.normals.push(normal);
        }
        if corners.len() == 4 {
            mesh.indices.extend([s0, s0 + 1, s0 + 2, s0, s0 + 2, s0 + 3]);
        } else {
            mesh.indices.extend([s0, s0 + 1, s0 + 2]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::probe_mesh;
    use cadcraft_doc::ArchId;
    use cadcraft_geom::Vec3;
    use std::collections::HashMap;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    fn tri_area(m: &Mesh, t: &[u32]) -> f64 {
        let p = |i: u32| {
            let v = m.positions[i as usize];
            [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])]
        };
        let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let x = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        0.5 * (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt()
    }

    /// Every undirected edge (welded by position) is shared by exactly two triangles.
    fn every_edge_shared_by_two(m: &Mesh) -> bool {
        let k = |i: u32| {
            let v = m.positions[i as usize];
            ((v[0] * 1e5).round() as i64, (v[1] * 1e5).round() as i64, (v[2] * 1e5).round() as i64)
        };
        let mut count: HashMap<_, u32> = HashMap::new();
        for t in m.indices.chunks(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (ka, kb) = (k(a), k(b));
                *count.entry(if ka <= kb { (ka, kb) } else { (kb, ka) }).or_insert(0) += 1;
            }
        }
        count.values().all(|&n| n == 2)
    }

    fn gable_plane(south: bool) -> RoofPlane {
        // A 6/12 gable over the phase-1 house: eaves at z = 2.4 on y = -0.1 and y = 8.1, 0.5 m overhang.
        let base_edge = if south { [0.0, -0.6, 10.0, -0.6] } else { [10.0, 8.6, 0.0, 8.6] };
        RoofPlane { base_edge, slope: 0.5, overhang: 0.5, eave_z: 2.4, ..Default::default() }
    }

    #[test]
    fn underside_plane_matches_roof_underside_z() {
        let mut planes = vec![gable_plane(true), gable_plane(false)];
        planes.push(RoofPlane { base_edge: [1.0, 2.0, 4.0, 6.0], slope: 0.75, overhang: 0.3, eave_z: 3.1, ..Default::default() });
        planes.push(RoofPlane { base_edge: [-2.0, 5.0, -2.0, -1.0], slope: 1.0, overhang: -1.0, eave_z: 0.0, ..Default::default() });
        planes.push(RoofPlane { base_edge: [0.0, 0.0, 3.0, 0.0], slope: 0.4, overhang: f64::NAN, eave_z: 2.0, ..Default::default() });
        for plane in &planes {
            let u = underside_plane(plane).expect("a plane");
            for (x, y) in [(0.0, 0.0), (5.0, 2.0), (-3.5, 7.25), (12.0, -4.0), (3.025, 5.475)] {
                let p = Vec2::new(x, y);
                let want = crate::roof::underside_z(plane, plane.eave_z, p);
                assert!((u.z_at(p) - want).abs() < 1e-9, "{plane:?} at {p:?}: {} vs {want}", u.z_at(p));
            }
        }
        // The brief's numbers: z = 2.4 + 0.5·(y + 0.1) on the south side.
        let s = underside_plane(&gable_plane(true)).expect("south");
        assert!((s.z_at(Vec2::new(5.0, 2.0)) - 3.45).abs() < 1e-12);
        assert!((s.z_at(Vec2::new(5.0, 0.5)) - 2.70).abs() < 1e-12);
        assert!((underside_plane(&gable_plane(false)).expect("north").z_at(Vec2::new(3.0, 7.3)) - 2.8).abs() < 1e-12);
        // Not a plane.
        let bad = [
            RoofPlane { base_edge: [1.0, 1.0, 1.0, 1.0], slope: 0.5, ..Default::default() },
            RoofPlane { base_edge: [0.0, 0.0, 1.0, 0.0], slope: f64::NAN, ..Default::default() },
            RoofPlane { base_edge: [0.0, 0.0, 1.0, 0.0], slope: 0.5, eave_z: f64::INFINITY, ..Default::default() },
            RoofPlane { base_edge: [f64::NAN, 0.0, 1.0, 0.0], slope: 0.5, ..Default::default() },
            RoofPlane { base_edge: [0.0, 0.0, 1e300, 1e300], slope: 0.5, ..Default::default() },
        ];
        for plane in &bad {
            assert_eq!(underside_plane(plane), None, "{plane:?}");
        }
    }

    #[test]
    fn plane_z_basics() {
        let p = PlaneZ::new(0.5, -1.0, 2.0);
        assert_eq!(p.z_at(Vec2::new(2.0, 1.0)), 2.0);
        assert!(!p.is_flat() && PlaneZ::flat(3.0).is_flat());
        assert_eq!(PlaneZ::flat(3.0).z_at(Vec2::new(1e300, -1e300)), 3.0);
        assert_eq!(p.offset(-0.02), PlaneZ::new(0.5, -1.0, 1.98));
        assert!(p.is_finite() && !PlaneZ::new(f64::NAN, 0.0, 0.0).is_finite());
        let piece = Piece::new(rect(0.0, 0.0, 1.0, 1.0), Vec::new(), p);
        assert_eq!(piece.plane, p);
    }

    #[test]
    fn region_triangles_cover_the_region_counter_clockwise() {
        let outer: Vec<Vec2> = rect(0.0, 0.0, 4.0, 3.0).into_iter().rev().collect(); // clockwise in
        let holes = vec![rect(1.0, 1.0, 2.0, 2.0)];
        let tris = region_triangles(&outer, &holes);
        assert!(!tris.is_empty());
        let total: f64 = tris.iter().map(|t| area(&t[..])).sum();
        assert!((total - 11.0).abs() < 1e-9, "{total}");
        assert!(tris.iter().all(|t| area(&t[..]) > 0.0));
    }

    #[test]
    fn push_between_two_flat_planes_equals_push_prism_with_holes() {
        let l_shape =
            vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 1.0), Vec2::new(1.0, 1.0), Vec2::new(1.0, 3.0), Vec2::new(0.0, 3.0)];
        let cases: Vec<(Vec<Vec2>, Vec<Vec<Vec2>>)> =
            vec![(rect(0.0, 0.0, 5.0, 4.0), vec![rect(1.0, 1.0, 2.0, 2.0), rect(3.0, 1.0, 4.0, 3.0)]), (l_shape, Vec::new())];
        for (outer, holes) in cases {
            let mut a = Mesh::new("ceiling", ArchId(1));
            a.push_prism_with_holes(&outer, &holes, 2.38, 2.4);
            let mut b = Mesh::new("ceiling", ArchId(1));
            push_between(&mut b, &outer, &holes, PlaneZ::flat(2.38), PlaneZ::flat(2.4));
            assert!(!a.positions.is_empty());
            assert_eq!((a.positions, a.normals, a.uvs, a.indices), (b.positions, b.normals, b.uvs, b.indices));
        }
    }

    #[test]
    fn push_between_sloped_prism_probes_inside_and_outside() {
        let mut m = Mesh::new("ceiling", ArchId(0));
        push_between(&mut m, &rect(0.0, 0.0, 4.0, 4.0), &[rect(1.0, 1.0, 2.0, 2.0)], PlaneZ::flat(1.0), PlaneZ::new(0.25, 0.0, 2.0));
        assert!(m.is_manifold());
        for (p, want) in [
            (Vec3::new(3.0, 3.0, 2.0), true),
            (Vec3::new(3.0, 3.0, 2.8), false), // top 2.75
            (Vec3::new(3.9, 0.5, 2.9), true),  // top 2.975
            (Vec3::new(0.1, 3.0, 2.1), false), // top 2.025
            (Vec3::new(1.5, 1.5, 1.5), false), // in the hole
            (Vec3::new(3.0, 3.0, 0.9), false), // below the bottom
        ] {
            assert_eq!(probe_mesh(&m, p), want, "{p:?}");
        }
        // A sloped bottom under a level top.
        let mut m = Mesh::new("slab", ArchId(0));
        push_between(&mut m, &rect(0.0, 0.0, 2.0, 2.0), &[], PlaneZ::new(0.0, 0.5, 0.0), PlaneZ::flat(3.0));
        assert!(m.is_manifold());
        assert!(probe_mesh(&m, Vec3::new(1.0, 1.9, 1.0)));
        assert!(!probe_mesh(&m, Vec3::new(1.0, 1.9, 0.9))); // bottom 0.95
    }

    #[test]
    fn push_between_with_a_zero_height_edge_builds_a_wedge() {
        let mut m = Mesh::new("wall", ArchId(0));
        push_between(&mut m, &rect(0.0, 0.0, 1.0, 1.0), &[], PlaneZ::flat(0.0), PlaneZ::new(0.0, 0.5, 0.0));
        assert!(!m.indices.is_empty());
        assert!(probe_mesh(&m, Vec3::new(0.5, 0.5, 0.2)));
        assert!(!probe_mesh(&m, Vec3::new(0.5, 0.5, 0.3)));
        for t in m.indices.chunks(3) {
            assert!(tri_area(&m, t) > 1e-9, "a zero-area triangle {t:?}");
        }
        assert!(every_edge_shared_by_two(&m));
        assert!(m.is_manifold());
        // Two sides of one triangle each and the full high side; nothing along y = 0.
        assert_eq!(m.triangle_count(), 2 + 2 + 1 + 1 + 2);
        // Meeting at a point only: a top tilted to touch the bottom at one corner.
        let mut m = Mesh::new("wall", ArchId(0));
        push_between(&mut m, &rect(0.0, 0.0, 1.0, 1.0), &[], PlaneZ::flat(0.0), PlaneZ::new(0.5, 0.5, 0.0));
        assert!(every_edge_shared_by_two(&m) && m.is_manifold());
        assert!(m.indices.chunks(3).all(|t| tri_area(&m, t) > 1e-9));
    }

    #[test]
    fn push_between_refuses_a_top_below_the_bottom() {
        let sq = rect(0.0, 0.0, 2.0, 2.0);
        let mut m = Mesh::new("x", ArchId(0));
        m.push_prism(&sq, 0.0, 1.0);
        let before = (m.positions.len(), m.indices.len());
        let refused = [
            (PlaneZ::flat(1.0), PlaneZ::new(0.5, 0.0, 0.5)), // dips below at x = 0
            (PlaneZ::flat(1.0), PlaneZ::flat(0.5)),
            (PlaneZ::flat(1.0), PlaneZ::flat(1.0)),
            (PlaneZ::new(0.1, 0.0, 1.0), PlaneZ::new(0.1, 0.0, 1.0)), // no volume anywhere
            (PlaneZ::new(f64::NAN, 0.0, 0.0), PlaneZ::flat(3.0)),
            (PlaneZ::flat(0.0), PlaneZ::new(0.0, f64::INFINITY, 1.0)),
        ];
        for (bottom, top) in refused {
            push_between(&mut m, &sq, &[], bottom, top);
            assert_eq!((m.positions.len(), m.indices.len()), before, "{bottom:?} {top:?}");
        }
    }

    #[test]
    fn split_by_line_keeps_the_area() {
        let sq = rect(0.0, 0.0, 2.0, 2.0);
        let (l, r) = split_by_line(&sq, Vec2::new(0.5, -1.0), Vec2::new(1.5, 3.0));
        assert!(!l.is_empty() && !r.is_empty());
        assert!((area(&l) + area(&r) - 4.0).abs() < 1e-12);
        assert!(area(&l) > 0.0 && area(&r) > 0.0, "both keep the winding");
        // The left part is left of a→b.
        let c = centroid(&l).expect("centroid");
        assert!((Vec2::new(1.5, 3.0) - Vec2::new(0.5, -1.0)).cross(c - Vec2::new(0.5, -1.0)) > 0.0);
        // A line that misses, or runs along an edge, leaves it whole on one side.
        let (l, r) = split_by_line(&sq, Vec2::new(5.0, 0.0), Vec2::new(5.0, 1.0));
        assert!((area(&l) - 4.0).abs() < 1e-12 && r.is_empty());
        let (l, r) = split_by_line(&sq, Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0));
        assert!((area(&l) - 4.0).abs() < 1e-12 && r.is_empty());
        // A sliver under 1e-12 m² is empty.
        let (l, r) = split_by_line(&sq, Vec2::new(0.0, 1e-13), Vec2::new(1.0, 0.0));
        assert!(!l.is_empty() && r.is_empty(), "{r:?}");
    }

    #[test]
    fn split_by_level_on_a_flat_plane_keeps_one_side() {
        let sq = rect(0.0, 0.0, 2.0, 2.0);
        let (lo, hi) = split_by_level(&sq, PlaneZ::flat(2.0), 3.0);
        assert!((area(&lo) - 4.0).abs() < 1e-12 && hi.is_empty());
        for z in [2.0, 1.0] {
            let (lo, hi) = split_by_level(&sq, PlaneZ::flat(2.0), z);
            assert!(lo.is_empty() && (area(&hi) - 4.0).abs() < 1e-12, "{z}");
        }
        // A sloped plane z = x splits at x = 1.
        let (lo, hi) = split_by_level(&sq, PlaneZ::new(1.0, 0.0, 0.0), 1.0);
        assert!((area(&lo) - 2.0).abs() < 1e-12 && (area(&hi) - 2.0).abs() < 1e-12);
        assert!(lo.iter().all(|p| p.x <= 1.0 + 1e-12) && hi.iter().all(|p| p.x >= 1.0 - 1e-12));
    }

    #[test]
    fn split_by_rings_cells_cover_the_region() {
        let outer = rect(0.0, 0.0, 4.0, 4.0);
        let holes = vec![rect(1.5, 1.5, 2.5, 2.5)];
        let rings = vec![rect(0.0, 0.0, 2.0, 4.0), rect(1.0, 1.0, 3.0, 3.0), vec![Vec2::new(f64::NAN, 0.0)]];
        let cells = split_by_rings(&outer, &holes, &rings);
        let total: f64 = cells.iter().map(|(c, _)| area(c)).sum();
        assert!((total - 15.0).abs() < 1e-9, "{total}");
        let held = |i: usize| -> f64 { cells.iter().filter(|(_, h)| h.contains(&i)).map(|(c, _)| area(c)).sum() };
        assert!((held(0) - 7.5).abs() < 1e-9, "{}", held(0));
        assert!((held(1) - 3.0).abs() < 1e-9, "{}", held(1));
        assert_eq!(held(2), 0.0);
        for (cell, holders) in &cells {
            // Convex, counter-clockwise, and on one side of every ring.
            let n = cell.len();
            assert!((0..n).all(|i| (cell[(i + 1) % n] - cell[i]).cross(cell[(i + 2) % n] - cell[(i + 1) % n]) >= -1e-12));
            let c = centroid(cell).expect("centroid");
            for (i, ring) in rings.iter().enumerate().take(2) {
                assert_eq!(holders.contains(&i), inside(c, ring));
                assert!(cell.iter().all(|p| inside(*p + (c - *p) * 1e-6, ring) == holders.contains(&i)), "cell {cell:?} straddles ring {i}");
            }
        }
    }

    #[test]
    fn under_planes_on_a_gable_picks_the_lower_plane() {
        let south = underside_plane(&gable_plane(true)).expect("south");
        let north = underside_plane(&gable_plane(false)).expect("north");
        // The outlines overlap between y = 3.5 and 4.5, and the region runs past both.
        let planes = vec![(rect(-0.6, -0.6, 10.6, 4.5), south), (rect(-0.6, 3.5, 10.6, 8.6), north)];
        let cells = under_planes(&rect(0.0, -1.0, 10.0, 9.0), &[], &planes);
        let total: f64 = cells.iter().map(|(c, _)| area(c)).sum();
        assert!((total - 100.0).abs() < 1e-9, "{total}");
        for (cell, plane) in &cells {
            let c = centroid(cell).expect("centroid");
            let want = if c.y < -0.6 || c.y > 8.6 {
                None
            } else if c.y < 3.5 {
                Some(south)
            } else if c.y > 4.5 {
                Some(north)
            } else if south.z_at(c) <= north.z_at(c) {
                Some(south)
            } else {
                Some(north)
            };
            assert_eq!(*plane, want, "cell at {c:?}");
        }
        assert!(cells.iter().any(|(_, p)| p.is_none()));
    }

    #[test]
    fn planar_helpers_survive_nan_huge_and_empty_input() {
        let nan = Vec2::new(f64::NAN, 0.0);
        let huge = rect(-1e300, -1e300, 1e300, 1e300);
        let bad_rings: Vec<Vec<Vec2>> = vec![Vec::new(), vec![nan, nan, nan], huge.clone(), vec![Vec2::ZERO; 3], rect(0.0, 0.0, 1e-9, 1e-9)];
        let planes = [PlaneZ::new(f64::NAN, 0.0, 0.0), PlaneZ::new(1e300, 1e300, 0.0), PlaneZ::flat(f64::INFINITY), PlaneZ::flat(1.0)];
        for ring in &bad_rings {
            assert!(region_triangles(ring, &bad_rings).len() <= MAX_REGION_TRIANGLES);
            let _ = split_by_line(ring, Vec2::ZERO, Vec2::new(1.0, 1.0));
            let _ = split_by_line(ring, nan, Vec2::new(1e300, 1e300));
            let _ = split_by_rings(ring, &bad_rings, &bad_rings);
            let _ = centroid(ring);
            for plane in planes {
                let _ = split_by_level(ring, plane, 1.0);
                let _ = split_by_level(ring, plane, f64::NAN);
                let _ = under_planes(ring, &bad_rings, &[(ring.clone(), plane)]);
                let mut m = Mesh::new("x", ArchId(0));
                push_between(&mut m, ring, &bad_rings, PlaneZ::flat(0.0), plane);
                push_between(&mut m, ring, &bad_rings, plane, PlaneZ::new(0.0, 1e300, 1e300));
                assert!(m.positions.iter().flatten().all(|v| v.is_finite()), "{ring:?} {plane:?}");
            }
        }
        // A valid region under hostile rings still covers itself.
        let sq = rect(0.0, 0.0, 2.0, 2.0);
        let cells = split_by_rings(&sq, &[], &bad_rings);
        assert!((cells.iter().map(|(c, _)| area(c)).sum::<f64>() - 4.0).abs() < 1e-9);
        assert!(cells.iter().all(|(_, h)| h.is_empty()));
        // A huge plane over a valid region builds nothing that is not finite.
        let mut m = Mesh::new("x", ArchId(0));
        push_between(&mut m, &sq, &[], PlaneZ::flat(0.0), PlaneZ::new(1e300, 0.0, 1.0));
        assert!(m.positions.iter().flatten().all(|v| v.is_finite()));
        assert_eq!(underside_plane(&RoofPlane::default()), None);
    }
}
