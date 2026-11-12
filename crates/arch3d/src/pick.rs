//! Ray picking: a median-split AABB tree over every scene triangle.
//!
//! [`Bvh::build`] flattens the triangles of a [`Scene3d`] (skipping any with a
//! non-finite corner or an out-of-range index, the same rule as the renderer's
//! upload) and [`Bvh::ray`] returns the nearest hit. Equal distances break by
//! the lower mesh (then triangle) index so results are deterministic. Hostile
//! rays (zero/non-finite direction, bad `max_t`) yield `None`.

use cadcraft_doc::ArchId;
use cadcraft_geom::Vec3;

use crate::Scene3d;
use crate::incremental::{ObjectKind, object_kind_of};
use crate::probe::pick_part;

/// Hard cap on tree nodes.
pub const MAX_NODES: usize = 4096;
/// Maximum tree depth.
pub const MAX_DEPTH: usize = 32;
/// Leaves at or below this triangle count are never split.
const LEAF_SIZE: usize = 4;

/// The nearest surface a pick ray struck.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickHit {
    /// The arch object owning the struck mesh.
    pub object: ArchId,
    /// Part name from [`pick_part`] (`"wall"`, `"leaf"`, `"floor"`, …).
    pub part: &'static str,
    /// The kind of object that owns the mesh (material-aware, unlike `part`).
    pub owner: ObjectKind,
    /// Distance along the (normalised) ray.
    pub distance: f64,
    /// World-space hit point.
    pub point: Vec3,
}

#[derive(Debug, Clone, Copy)]
struct Tri {
    a: Vec3,
    b: Vec3,
    c: Vec3,
    mesh: u32,
}

#[derive(Debug, Clone, Copy)]
struct Node {
    min: [f64; 3],
    max: [f64; 3],
    /// Leaf: first slot in `order`. Inner: unused.
    start: u32,
    /// Leaf: triangle count (> 0). Inner: 0.
    count: u32,
    /// Inner: child node indices.
    left: u32,
    right: u32,
}

/// A bounding-volume hierarchy over the triangles of a scene.
#[derive(Debug, Clone, Default)]
pub struct Bvh {
    /// Triangles in scene order (mesh index, then triangle index).
    tris: Vec<Tri>,
    /// Permutation of `tris` indices; leaves cover contiguous slices.
    order: Vec<u32>,
    nodes: Vec<Node>,
    /// Part name, owning object and owner kind per mesh index.
    meshes: Vec<(&'static str, ArchId, ObjectKind)>,
}

fn comp(v: Vec3, i: usize) -> f64 {
    match i {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

fn corner(p: Option<&[f32; 3]>) -> Option<Vec3> {
    let p = p?;
    let v = Vec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
    v.is_finite().then_some(v)
}

/// Build the tree for `scene`. An empty scene gives an empty tree.
pub fn build(scene: &Scene3d) -> Bvh {
    Bvh::build(scene)
}

impl Bvh {
    /// Build the tree for `scene`.
    pub fn build(scene: &Scene3d) -> Bvh {
        let mut bvh = Bvh::default();
        for (mi, mesh) in scene.meshes.iter().enumerate() {
            bvh.meshes.push((pick_part(mesh), mesh.object, object_kind_of(mesh)));
            let Ok(mesh_id) = u32::try_from(mi) else { continue };
            for g in mesh.indices.chunks_exact(3) {
                let at = |k: usize| g.get(k).and_then(|&i| corner(mesh.positions.get(i as usize)));
                if let (Some(a), Some(b), Some(c)) = (at(0), at(1), at(2)) {
                    bvh.tris.push(Tri { a, b, c, mesh: mesh_id });
                }
            }
        }
        // Triangle indices must fit u32.
        if bvh.tris.is_empty() || u32::try_from(bvh.tris.len()).is_err() {
            bvh.tris.clear();
            return bvh;
        }
        bvh.order = (0..bvh.tris.len() as u32).collect();
        let n = bvh.order.len();
        bvh.split(0, n, 0);
        bvh
    }

    /// Whether the tree holds no triangles.
    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    /// Number of triangles indexed.
    pub fn triangle_count(&self) -> usize {
        self.tris.len()
    }

    /// Number of tree nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    fn centroid(&self, idx: u32) -> Vec3 {
        match self.tris.get(idx as usize) {
            Some(t) => (t.a + t.b + t.c) * (1.0 / 3.0),
            None => Vec3::new(0.0, 0.0, 0.0),
        }
    }

    /// Build the node covering `order[lo..hi]`; returns its index.
    fn split(&mut self, lo: usize, hi: usize, depth: usize) -> Option<u32> {
        let id = u32::try_from(self.nodes.len()).ok()?;
        let (mut min, mut max) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        let (mut cmin, mut cmax) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for &i in self.order.get(lo..hi)? {
            let t = self.tris.get(i as usize)?;
            let c = self.centroid(i);
            for k in 0..3 {
                for p in [t.a, t.b, t.c] {
                    min[k] = min[k].min(comp(p, k));
                    max[k] = max[k].max(comp(p, k));
                }
                cmin[k] = cmin[k].min(comp(c, k));
                cmax[k] = cmax[k].max(comp(c, k));
            }
        }
        let count = hi - lo;
        self.nodes.push(Node { min, max, start: lo as u32, count: count as u32, left: 0, right: 0 });
        let ext = [cmax[0] - cmin[0], cmax[1] - cmin[1], cmax[2] - cmin[2]];
        let ax = if ext[0] >= ext[1] && ext[0] >= ext[2] {
            0
        } else if ext[1] >= ext[2] {
            1
        } else {
            2
        };
        let room = self.nodes.len() + 2 <= MAX_NODES;
        if count <= LEAF_SIZE || depth >= MAX_DEPTH || !room || !(ext[ax] > 0.0) {
            return Some(id);
        }
        // Median split by centroid along the widest axis; index breaks ties.
        let mut part: Vec<u32> = self.order.get(lo..hi)?.to_vec();
        part.sort_by(|&a, &b| {
            let ka = comp(self.centroid(a), ax);
            let kb = comp(self.centroid(b), ax);
            ka.total_cmp(&kb).then(a.cmp(&b))
        });
        self.order.get_mut(lo..hi)?.copy_from_slice(&part);
        let mid = lo + count / 2;
        let left = self.split(lo, mid, depth + 1)?;
        let right = self.split(mid, hi, depth + 1)?;
        let node = self.nodes.get_mut(id as usize)?;
        node.count = 0;
        node.left = left;
        node.right = right;
        Some(id)
    }

    /// Nearest hit of the ray `origin + t * dir` with `0 < t <= max_t`.
    ///
    /// `dir` is normalised internally; a zero or non-finite direction, a
    /// non-finite origin, or a non-finite / non-positive `max_t` gives `None`.
    pub fn ray(&self, origin: Vec3, dir: Vec3, max_t: f64) -> Option<PickHit> {
        if self.nodes.is_empty() || !origin.is_finite() || !dir.is_finite() || !max_t.is_finite() || max_t <= 0.0 {
            return None;
        }
        let len = dir.len();
        if !(len > 0.0) || !len.is_finite() {
            return None;
        }
        let d = dir * (1.0 / len);
        if !d.is_finite() {
            return None;
        }
        let mut best_t = max_t;
        let mut best: Option<(u32, f64)> = None; // (original tri index, t)
        let mut stack: Vec<u32> = vec![0];
        while let Some(ni) = stack.pop() {
            let Some(node) = self.nodes.get(ni as usize) else { continue };
            if !box_hit(node, origin, d, best_t) {
                continue;
            }
            if node.count == 0 {
                // Push right first so the left child is visited first.
                stack.push(node.right);
                stack.push(node.left);
                continue;
            }
            for slot in node.start..node.start.saturating_add(node.count) {
                let Some(&ti) = self.order.get(slot as usize) else { continue };
                let Some(tri) = self.tris.get(ti as usize) else { continue };
                if let Some(t) = tri_hit(tri, origin, d) {
                    let better = match best {
                        None => t <= best_t,
                        Some((bi, bt)) => t < bt || (t == bt && ti < bi),
                    };
                    if better {
                        best_t = t;
                        best = Some((ti, t));
                    }
                }
            }
        }
        let (ti, t) = best?;
        let tri = self.tris.get(ti as usize)?;
        let (part, object, owner) = *self.meshes.get(tri.mesh as usize)?;
        Some(PickHit { object, part, owner, distance: t, point: origin + d * t })
    }

    /// Every strike the ray makes, in increasing distance (then mesh) order. The far end is
    /// `f64::MAX`, which is what [`Scene3d::pick_filtered`] needs to walk through all hits.
    /// An empty scene or a zero / non-finite direction yields an empty list.
    pub fn rays_all(&self, origin: Vec3, dir: Vec3) -> Vec<PickHit> {
        if self.nodes.is_empty() || !origin.is_finite() || !dir.is_finite() {
            return Vec::new();
        }
        let len = dir.len();
        if !(len > 0.0) || !len.is_finite() {
            return Vec::new();
        }
        let d = dir * (1.0 / len);
        if !d.is_finite() {
            return Vec::new();
        }
        let max_t = f64::MAX;
        let mut hits: Vec<(f64, u32, PickHit)> = Vec::new();
        let mut stack: Vec<u32> = vec![0];
        while let Some(ni) = stack.pop() {
            let Some(node) = self.nodes.get(ni as usize) else { continue };
            if !box_hit(node, origin, d, max_t) {
                continue;
            }
            if node.count == 0 {
                // Push right first so the left child is visited first.
                stack.push(node.right);
                stack.push(node.left);
                continue;
            }
            for slot in node.start..node.start.saturating_add(node.count) {
                let Some(&ti) = self.order.get(slot as usize) else { continue };
                let Some(tri) = self.tris.get(ti as usize) else { continue };
                if let Some(t) = tri_hit(tri, origin, d) {
                    let Some((part, object, owner)) = self.meshes.get(tri.mesh as usize) else { continue };
                    hits.push((t, tri.mesh, PickHit { object: *object, part, owner: *owner, distance: t, point: origin + d * t }));
                }
            }
        }
        hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
        hits.into_iter().map(|(_, _, h)| h).collect()
    }

    /// The distance of the nearest strike at least `t_min` out along the ray — the hit
    /// [`Scene3d::pick_filtered`] advances past when no accepted hit exists.
    pub fn nearest_rejected(&self, origin: Vec3, dir: Vec3, t_min: f64) -> Option<f64> {
        self.rays_all(origin, dir)
            .iter()
            .filter(|h| h.distance >= t_min)
            .min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal))
            .map(|h| h.distance)
    }
}

/// Slab test of the ray against a node box, limited to `[0, t_max]`.
fn box_hit(node: &Node, o: Vec3, d: Vec3, t_max: f64) -> bool {
    let (mut t0, mut t1) = (0.0f64, t_max);
    for k in 0..3 {
        let (oc, dc) = (comp(o, k), comp(d, k));
        let (lo, hi) = (node.min[k], node.max[k]);
        if dc == 0.0 {
            if oc < lo || oc > hi {
                return false;
            }
            continue;
        }
        let inv = 1.0 / dc;
        let (mut a, mut b) = ((lo - oc) * inv, (hi - oc) * inv);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        t0 = t0.max(a);
        t1 = t1.min(b);
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// Möller–Trumbore, double sided; returns `t > 0`.
fn tri_hit(tri: &Tri, o: Vec3, d: Vec3) -> Option<f64> {
    let e1 = tri.b - tri.a;
    let e2 = tri.c - tri.a;
    let p = d.cross(e2);
    let det = e1.dot(p);
    if !(det.abs() > 1e-14) {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - tri.a;
    let u = s.dot(p) * inv;
    if !(-1e-9..=1.0 + 1e-9).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t.is_finite() && t > 1e-9).then_some(t)
}

impl Scene3d {
    /// Nearest hit of an infinite ray, building a throwaway [`Bvh`].
    pub fn pick(&self, origin: Vec3, dir: Vec3) -> Option<PickHit> {
        self.pick_with(&Bvh::build(self), origin, dir)
    }

    /// Nearest hit of an infinite ray against a prebuilt tree.
    pub fn pick_with(&self, bvh: &Bvh, origin: Vec3, dir: Vec3) -> Option<PickHit> {
        bvh.ray(origin, dir, f64::MAX)
    }

    /// Nearest hit of an infinite ray against a prebuilt tree, skipping hits
    /// rejected by `accept`. Re-casts the ray up to 64 times, each time
    /// advancing past the most recent rejected hit. Returns `None` when no
    /// accepted hit exists.
    pub fn pick_filtered(&self, bvh: &Bvh, origin: Vec3, dir: Vec3, accept: &dyn Fn(&PickHit) -> bool) -> Option<PickHit> {
        let mut t_min = 0.0f64;
        for _ in 0..64 {
            let hits = bvh.rays_all(origin, dir);
            let accepted = hits
                .into_iter()
                .filter(|h| h.distance >= t_min)
                .filter(|h| accept(h))
                .min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
            if let Some(h) = accepted {
                return Some(h);
            }
            if let Some(nearest) = bvh.nearest_rejected(origin, dir, t_min) {
                t_min = nearest + 1e-6;
                if t_min >= f64::MAX {
                    return None;
                }
            } else {
                return None;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Mesh;

    #[test]
    fn pick_filtered_skips_rejected() {
        let mut tri1 = Mesh {
            positions: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]].map(|p| [p[0], p[1], p[2]]).to_vec(),
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0]; 3],
            indices: vec![0, 1, 2],
            ..Mesh::new("test", ArchId(9))
        };
        tri1.level_elevation = 0.0;
        let mut tri2 = Mesh {
            positions: [[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]].map(|p| [p[0], p[1], p[2]]).to_vec(),
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0]; 3],
            indices: vec![0, 1, 2],
            ..Mesh::new("test", ArchId(10))
        };
        tri2.level_elevation = 0.0;
        let scene = Scene3d { meshes: vec![tri1, tri2], ..Scene3d::default() };
        let bvh = Bvh::build(&scene);
        let hit = scene.pick_filtered(&bvh, Vec3::new(0.5, -1.0, 0.5), Vec3::new(0.0, 1.0, 0.0), &|h| h.distance > 1.0);
        assert!(hit.is_none());
    }
}
