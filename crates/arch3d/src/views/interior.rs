//! Interior elevations: the scene seen from inside a room, clipped to the room's box (its plan extent along the
//! wall, from just in front of the opposite boundary to the wall face, floor to finished ceiling).

use crate::{Mesh, Scene3d};
use cadcraft_arch::ceiling::finished_ceiling_in;
use cadcraft_arch::interior::{face_of, ring_contains};
use cadcraft_doc::arch::ArchView;
use cadcraft_doc::{ArchId, ArchModel, Room};
use cadcraft_geom::Vec2;

/// Points within this distance of a clip plane count as inside it.
const PLANE_TOL: f64 = 1e-6;
/// How far in front of the opposite boundary the box starts (nothing on or behind it is drawn).
const NEAR_GAP: f64 = 0.01;
/// How far a vaulted room's sample points are pulled toward the room point.
const PULL: f64 = 1e-3;
/// Triangle cap: this many times the input's triangles.
const TRIANGLE_FACTOR: usize = 8;

/// A clip plane: a point is inside when `n . p >= c` (within [`PLANE_TOL`]).
#[derive(Clone, Copy)]
struct Clip {
    n: [f64; 3],
    c: f64,
}

impl Clip {
    fn s(&self, p: [f64; 3]) -> f64 {
        self.n[0] * p[0] + self.n[1] * p[1] + self.n[2] * p[2] - self.c
    }
}

/// Sutherland-Hodgman clip of the convex-or-not polygon `poly` against `plane`.
fn clip_polygon(poly: &[[f64; 3]], plane: &Clip) -> Vec<[f64; 3]> {
    let mut out = Vec::with_capacity(poly.len() + 1);
    for (i, a) in poly.iter().enumerate() {
        let Some(b) = poly.get((i + 1) % poly.len()) else { continue };
        let (sa, sb) = (plane.s(*a), plane.s(*b));
        let (ia, ib) = (sa >= -PLANE_TOL, sb >= -PLANE_TOL);
        if ia {
            out.push(*a);
        }
        if ia != ib {
            let d = sa - sb;
            let t = if d.abs() > 0.0 { (sa / d).clamp(0.0, 1.0) } else { 0.0 };
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]);
        }
    }
    out
}

fn vertex(m: &Mesh, i: u32) -> Option<[f64; 3]> {
    let p = m.positions.get(usize::try_from(i).ok()?)?;
    Some([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])])
}

/// The room's level elevation (0 when the level is not in the model).
fn floor_z(model: &ArchModel, room: &Room) -> f64 {
    model.level(room.level).map_or(0.0, |l| l.elevation)
}

/// The room point of a view: the stored one, else the vertex average of the room's polygon.
fn point_of(view: &ArchView, room: &Room) -> Vec2 {
    if let Some(p) = view.room_point {
        return Vec2::new(p[0], p[1]);
    }
    let n = room.polygon.len().max(1) as f64;
    let (sx, sy) = room.polygon.iter().fold((0.0, 0.0), |s, p| (s.0 + p.x, s.1 + p.y));
    Vec2::new(sx / n, sy / n)
}

/// The finished ceiling height (absolute z) the box reaches: the room point's, or for a vaulted room the highest
/// over the room point and the polygon's vertices pulled toward it.
fn ceiling_z(model: &ArchModel, room: &Room, at: Vec2) -> f64 {
    let fallback = floor_z(model, room) + room.ceiling_height.or_else(|| model.level(room.level).map(|l| l.ceiling_height)).unwrap_or(2.4);
    let mut best = finished_ceiling_in(model, room, at);
    if !room.flat_ceiling {
        for v in &room.polygon {
            let toward = *v + (at - *v) * PULL;
            if let Some(z) = finished_ceiling_in(model, room, toward) {
                best = Some(best.map_or(z, |b| b.max(z)));
            }
        }
    }
    best.filter(|z| z.is_finite()).unwrap_or(fallback)
}

/// The scene as seen by an interior view, or why it cannot be made.
pub(crate) fn restrict(model: &ArchModel, scene: &Scene3d, view: &ArchView) -> Result<Scene3d, String> {
    let gone = || format!("the room of {} is gone", view.name);
    let room = resolve_room(model, view).and_then(|id| model.room(id)).ok_or_else(gone)?;
    let wall = view.wall.ok_or_else(|| format!("the wall of {} is not set", view.name))?;
    let face = face_of(model, room, wall).ok_or_else(|| format!("the wall of {} no longer bounds its room", view.name))?;
    let look = [view.look[0], view.look[1]];
    let len = look[0].hypot(look[1]);
    if !(len > 1e-9) || !len.is_finite() {
        return Err("the look direction is not usable".to_string());
    }
    let look = [look[0] / len, look[1] / len];
    let right = [look[1], -look[0]];
    let u_of = |p: Vec2| p.x * right[0] + p.y * right[1];
    let d_of = |p: Vec2| p.x * look[0] + p.y * look[1];
    let (mut u0, mut u1, mut d0) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY);
    for p in &room.polygon {
        u0 = u0.min(u_of(*p));
        u1 = u1.max(u_of(*p));
        d0 = d0.min(d_of(*p));
    }
    let d1 = d_of(face.a) + 1e-6;
    let at = point_of(view, room);
    let z0 = floor_z(model, room);
    let z1 = ceiling_z(model, room, at);
    if ![u0, u1, d0, d1, z0, z1].iter().all(|v| v.is_finite()) {
        return Err(format!("the room of {} has no usable extent", view.name));
    }
    let planes = [
        Clip { n: [right[0], right[1], 0.0], c: u0 },
        Clip { n: [-right[0], -right[1], 0.0], c: -u1 },
        Clip { n: [look[0], look[1], 0.0], c: d0 + NEAR_GAP },
        Clip { n: [-look[0], -look[1], 0.0], c: -d1 },
        Clip { n: [0.0, 0.0, 1.0], c: z0 },
        Clip { n: [0.0, 0.0, -1.0], c: -z1 },
    ];

    let level_ok = |m: &Mesh| (m.level_elevation - z0).abs() <= PLANE_TOL;
    let input: usize = scene.meshes.iter().filter(|m| level_ok(m)).map(Mesh::triangle_count).sum();
    let cap = input.saturating_mul(TRIANGLE_FACTOR);
    let mut total = 0usize;
    let mut meshes = Vec::new();
    for m in scene.meshes.iter().filter(|m| level_ok(m)) {
        let mut out = Mesh::new(&m.material, m.object);
        out.level_elevation = m.level_elevation;
        for tri in m.indices.chunks_exact(3) {
            let corners: Option<Vec<[f64; 3]>> = tri.iter().map(|i| vertex(m, *i)).collect();
            let Some(mut poly) = corners else { continue };
            for pl in &planes {
                poly = clip_polygon(&poly, pl);
                if poly.len() < 3 {
                    break;
                }
            }
            if poly.len() < 3 {
                continue;
            }
            let n = poly.len() as f64;
            let c = poly.iter().fold([0.0; 3], |s, p| [s[0] + p[0], s[1] + p[1], s[2] + p[2]]);
            if !ring_contains(&room.polygon, Vec2::new(c[0] / n, c[1] / n), PLANE_TOL) {
                continue;
            }
            let Some(&first) = poly.first() else { continue };
            // Normal of the fan from the sum of its triangles' cross products; a sliver has no area and is dropped.
            let mut nn = [0.0f64; 3];
            for w in poly.windows(2).skip(1) {
                let (Some(p), Some(q)) = (w.first(), w.get(1)) else { continue };
                let (e1, e2) = ([p[0] - first[0], p[1] - first[1], p[2] - first[2]], [q[0] - first[0], q[1] - first[1], q[2] - first[2]]);
                nn[0] += e1[1] * e2[2] - e1[2] * e2[1];
                nn[1] += e1[2] * e2[0] - e1[0] * e2[2];
                nn[2] += e1[0] * e2[1] - e1[1] * e2[0];
            }
            let nl = (nn[0] * nn[0] + nn[1] * nn[1] + nn[2] * nn[2]).sqrt();
            if !(nl > 1e-12) {
                continue;
            }
            let normal = [(nn[0] / nl) as f32, (nn[1] / nl) as f32, (nn[2] / nl) as f32];
            let pieces = poly.len() - 2;
            total = total.saturating_add(pieces);
            if total > cap {
                return Err(format!("the interior view {} has too many triangles", view.name));
            }
            let base = u32::try_from(out.positions.len()).map_err(|_| "the interior view is too large".to_string())?;
            for p in &poly {
                out.positions.push([p[0] as f32, p[1] as f32, p[2] as f32]);
                out.normals.push(normal);
                out.uvs.push([p[0] as f32, p[2] as f32]);
            }
            for k in 1..=pieces {
                let k = u32::try_from(k).map_err(|_| "the interior view is too large".to_string())?;
                out.indices.extend_from_slice(&[base, base + k, base + k + 1]);
            }
        }
        if !out.indices.is_empty() {
            meshes.push(out);
        }
    }
    Ok(Scene3d { meshes, ..Scene3d::default() })
}

/// The room an interior view looks from, found again after rooms were renumbered: the room the view names when it is
/// on the view's level and still contains the view's room point, else the room on that level containing the point.
pub(crate) fn resolve_room(model: &ArchModel, view: &ArchView) -> Option<ArchId> {
    let on_level = |r: &&Room| view.level.is_none_or(|l| r.level == l);
    let named = view.room.and_then(|id| model.room(id)).filter(|r| on_level(r));
    let Some(p) = view.room_point else { return named.map(|r| r.id) };
    let p = Vec2::new(p[0], p[1]);
    if let Some(r) = named.filter(|r| ring_contains(&r.polygon, p, 1e-9)) {
        return Some(r.id);
    }
    model.rooms.iter().filter(on_level).filter(|r| ring_contains(&r.polygon, p, 1e-9)).map(|r| r.id).min()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::{Level, Wall};

    /// A 6 x 4 room (east wall x = 6 with its inner face at 5.95), 2.4 m high, and its view of the east wall.
    fn room_model() -> (ArchModel, ArchView) {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let t = 0.1;
        for ((x0, y0), (x1, y1)) in [((0.0, 0.0), (6.0, 0.0)), ((6.05, 0.0), (6.05, 4.0)), ((6.0, 4.0), (0.0, 4.0)), ((-0.05, 4.0), (-0.05, 0.0))] {
            m.push_wall(Wall { start: Vec2::new(x0, y0), end: Vec2::new(x1, y1), thickness: t, height: 2.4, ..Default::default() });
        }
        let id = m.push_room(Room {
            polygon: vec![Vec2::new(0.0, 0.05), Vec2::new(6.0, 0.05), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)],
            ceiling_height: Some(2.4),
            ..Default::default()
        });
        let view = ArchView {
            name: "R East".into(),
            kind: cadcraft_doc::arch::ViewKind::Interior,
            look: [1.0, 0.0],
            room: Some(id),
            room_point: Some([3.0, 2.0]),
            wall: Some(ArchId(1)),
            level: Some(0),
            ..Default::default()
        };
        (m, view)
    }

    /// A vertical quad in the plane x = `x` spanning y0..y1 and z0..z1, two triangles.
    fn quad_x(x: f32, y0: f32, y1: f32, z0: f32, z1: f32, object: u32, level: f64) -> Mesh {
        let mut m = Mesh::new("wall", ArchId(object));
        m.positions = vec![[x, y0, z0], [x, y1, z0], [x, y1, z1], [x, y0, z1]];
        m.normals = vec![[1.0, 0.0, 0.0]; 4];
        m.uvs = vec![[0.0, 0.0]; 4];
        m.indices = vec![0, 1, 2, 0, 2, 3];
        m.level_elevation = level;
        m
    }

    fn bounds(s: &Scene3d) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for p in s.meshes.iter().flat_map(|m| m.positions.iter()) {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        (lo, hi)
    }

    #[test]
    fn restrict_keeps_only_the_room_box() {
        let (m, v) = room_model();
        // A big quad on the wall face reaching far outside the room in y and z.
        let scene = Scene3d { meshes: vec![quad_x(5.95, -10.0, 10.0, -1.0, 5.0, 1, 0.0)], ..Default::default() };
        let out = restrict(&m, &scene, &v).unwrap();
        let (lo, hi) = bounds(&out);
        assert!((lo[1] - 0.05).abs() < 1e-5 && (hi[1] - 4.0).abs() < 1e-5, "{lo:?} {hi:?}");
        assert!(lo[2].abs() < 1e-5 && (hi[2] - 2.4).abs() < 1e-5, "{lo:?} {hi:?}");
        assert_eq!(out.meshes.len(), 1);
        assert_eq!(out.meshes[0].object, ArchId(1));
        assert_eq!(out.meshes[0].material, "wall");
    }

    #[test]
    fn nothing_behind_the_viewer_or_beyond_the_face() {
        let (m, v) = room_model();
        let scene = Scene3d {
            meshes: vec![
                quad_x(-0.05, 0.0, 4.0, 0.0, 2.4, 3, 0.0), // the opposite boundary, behind the viewer
                quad_x(6.2, 0.0, 4.0, 0.0, 2.4, 4, 0.0),   // beyond the wall face
                quad_x(3.0, 1.0, 2.0, 0.0, 1.0, 5, 0.0),   // stands in front of the wall
            ],
            ..Default::default()
        };
        let out = restrict(&m, &scene, &v).unwrap();
        let objects: Vec<u32> = out.meshes.iter().map(|m| m.object.0).collect();
        assert_eq!(objects, vec![5]);
    }

    #[test]
    fn other_levels_are_dropped() {
        let (m, v) = room_model();
        let scene = Scene3d {
            meshes: vec![
                quad_x(5.95, 0.5, 3.0, 0.0, 2.0, 1, 0.0),
                quad_x(5.95, 0.5, 3.0, 0.0, 2.0, 2, 2.7),
                quad_x(5.95, 0.5, 3.0, 0.0, 2.0, 3, f64::NAN),
            ],
            ..Default::default()
        };
        let out = restrict(&m, &scene, &v).unwrap();
        assert_eq!(out.meshes.len(), 1);
        assert_eq!(out.meshes[0].object, ArchId(1));
    }

    #[test]
    fn resolve_room_after_roomdetect() {
        let (mut m, v) = room_model();
        let old = v.room.unwrap();
        assert_eq!(resolve_room(&m, &v), Some(old));
        // Re-detect: the room gets a new id, the old one is gone.
        let mut room = m.room(old).cloned().unwrap();
        m.rooms.clear();
        room.polygon = vec![Vec2::new(0.0, 0.05), Vec2::new(5.5, 0.05), Vec2::new(5.5, 4.0), Vec2::new(0.0, 4.0)];
        let new = m.push_room(room);
        assert_ne!(new, old);
        assert_eq!(resolve_room(&m, &v), Some(new));
        // A different level finds nothing.
        let other = ArchView { level: Some(1), ..v.clone() };
        assert_eq!(resolve_room(&m, &other), None);
    }

    #[test]
    fn gone_room_is_an_error() {
        let (mut m, v) = room_model();
        m.rooms.clear();
        let e = restrict(&m, &Scene3d::default(), &v).unwrap_err();
        assert_eq!(e, "the room of R East is gone");
        let g = super::super::generate(&m, &Scene3d::default(), &v, 2);
        assert_eq!(g.error.as_deref(), Some("the room of R East is gone"));
    }
}
