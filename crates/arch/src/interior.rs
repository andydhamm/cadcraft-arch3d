//! Wall faces of a room: which wall faces lie along the room's boundary, for interior elevations.
//!
//! A face is one side of a wall's poché rectangle. It bounds the room when it is collinear (within 1e-6) with a
//! polygon edge of the room and overlaps it by more than 1e-6.

use cadcraft_doc::arch::Side;
use cadcraft_doc::{ArchId, ArchModel, Room};
use cadcraft_geom::Vec2;

use crate::wall::wall_polygon;

/// Collinearity and overlap tolerance (metres).
const TOL: f64 = 1e-6;
/// Smallest dot product with a compass direction for `wall_on_side`.
const MIN_SIDE_DOT: f64 = 0.7;

/// A wall face lying along one of a room polygon's edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WallFace {
    /// The wall.
    pub wall: ArchId,
    /// The side of the wall (left or right of its start to end) the face is on.
    pub side: Side,
    /// Start of the overlap with the room edge.
    pub a: Vec2,
    /// End of the overlap with the room edge.
    pub b: Vec2,
    /// Unit normal from the room toward the wall.
    pub look: Vec2,
}

impl WallFace {
    fn len(&self) -> f64 {
        self.a.dist(self.b)
    }
}

/// Twice the signed area of a ring (positive when counter-clockwise).
fn signed_area2(ring: &[Vec2]) -> f64 {
    let mut s = 0.0;
    for (i, p) in ring.iter().enumerate() {
        if let Some(q) = ring.get((i + 1) % ring.len()) {
            s += p.x * q.y - q.x * p.y;
        }
    }
    s
}

/// Whether `p` is inside `ring` (even-odd) or within `tol` of its boundary.
pub fn ring_contains(ring: &[Vec2], p: Vec2, tol: f64) -> bool {
    if ring.len() < 3 || !p.is_finite() {
        return false;
    }
    for (i, a) in ring.iter().enumerate() {
        let Some(b) = ring.get((i + 1) % ring.len()) else { continue };
        let d = *b - *a;
        let l2 = d.len2();
        let t = if l2 > 0.0 { ((p - *a).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
        if (*a + d * t).dist(p) <= tol {
            return true;
        }
    }
    cadcraft_geom::point_in_polygon(ring, p)
}

/// Every wall face along the room's outer ring, walls on the room's level, sorted by wall id then `a`.
pub fn bounding_faces(model: &ArchModel, room: &Room) -> Vec<WallFace> {
    let ring = &room.polygon;
    let n = ring.len();
    let area2 = signed_area2(ring);
    if n < 3 || !area2.is_finite() || area2.abs() < 1e-12 || ring.iter().any(|p| !p.is_finite()) {
        return Vec::new();
    }
    let ccw = area2 > 0.0;
    let mut out = Vec::new();
    for w in model.walls.iter().filter(|w| w.base_level == room.level) {
        let Some(poly) = wall_polygon(w) else { continue };
        // Corners a, b are on the right of start->end; c, d on the left (see `wall_polygon`).
        let (Some(&p0), Some(&p1), Some(&p2), Some(&p3)) = (poly.first(), poly.get(1), poly.get(2), poly.get(3)) else { continue };
        let dir = (w.end - w.start).normalized();
        let faces = [(Side::Right, p0, p1, dir.perp() * -1.0), (Side::Left, p3, p2, dir.perp())];
        for (side, f0, f1, outward) in faces {
            for (i, e0) in ring.iter().enumerate() {
                let Some(e1) = ring.get((i + 1) % n) else { continue };
                let ed = *e1 - *e0;
                let el = ed.len();
                if !(el >= TOL) {
                    continue;
                }
                let eu = ed / el;
                let dist = |p: Vec2| (p - *e0).dot(eu.perp()).abs();
                if dist(f0) > TOL || dist(f1) > TOL {
                    continue;
                }
                let (t0, t1) = ((f0 - *e0).dot(eu), (f1 - *e0).dot(eu));
                let (lo, hi) = (t0.min(t1).max(0.0), t0.max(t1).min(el));
                if hi - lo <= TOL {
                    continue;
                }
                // Outward normal of the room edge: from the room toward the wall.
                let look = if ccw { Vec2::new(eu.y, -eu.x) } else { Vec2::new(-eu.y, eu.x) };
                // The face's outward normal (out of the wall body) must point back into the room.
                if outward.dot(look) >= 0.0 {
                    continue;
                }
                out.push(WallFace { wall: w.id, side, a: *e0 + eu * lo, b: *e0 + eu * hi, look });
            }
        }
    }
    out.sort_by(|x, y| x.wall.cmp(&y.wall).then(x.a.x.total_cmp(&y.a.x)).then(x.a.y.total_cmp(&y.a.y)));
    out
}

/// The longest face of `wall` along the room; `None` when the wall does not bound it.
pub fn face_of(model: &ArchModel, room: &Room, wall: ArchId) -> Option<WallFace> {
    let mut best: Option<WallFace> = None;
    for f in bounding_faces(model, room).into_iter().filter(|f| f.wall == wall) {
        if best.is_none_or(|b| f.len() > b.len() + 1e-9) {
            best = Some(f);
        }
    }
    best
}

/// The face looking toward a compass `side` ("north" looks (0, 1), "south" (0, -1), "east" (1, 0), "west" (-1, 0)).
/// The largest dot product wins (at least 0.7), ties to the longest face, then the lower wall id.
pub fn wall_on_side(model: &ArchModel, room: &Room, side: &str) -> Option<WallFace> {
    let dir = match side {
        "north" => Vec2::new(0.0, 1.0),
        "south" => Vec2::new(0.0, -1.0),
        "east" => Vec2::new(1.0, 0.0),
        "west" => Vec2::new(-1.0, 0.0),
        _ => return None,
    };
    let mut best: Option<(f64, WallFace)> = None;
    for f in bounding_faces(model, room) {
        let d = f.look.dot(dir);
        if d < MIN_SIDE_DOT {
            continue;
        }
        let better = match best {
            None => true,
            Some((bd, bf)) => {
                if (d - bd).abs() > 1e-9 {
                    d > bd
                } else {
                    f.len() > bf.len() + 1e-9
                }
            }
        };
        if better {
            best = Some((d, f));
        }
    }
    best.map(|(_, f)| f)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::Wall;

    /// A 6 x 4 room whose four 0.1 m walls have their inner faces on its polygon.
    fn house() -> (ArchModel, Room) {
        let mut m = ArchModel::default();
        m.levels.push(cadcraft_doc::Level { index: 0, ..Default::default() });
        let t = 0.1;
        let h = t / 2.0;
        let walls = [((-h, -h), (6.0 + h, -h)), ((6.0 + h, -h), (6.0 + h, 4.0 + h)), ((6.0 + h, 4.0 + h), (-h, 4.0 + h)), ((-h, 4.0 + h), (-h, -h))];
        for ((x0, y0), (x1, y1)) in walls {
            m.push_wall(Wall { start: Vec2::new(x0, y0), end: Vec2::new(x1, y1), thickness: t, height: 2.4, ..Default::default() });
        }
        let room = Room { polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)], ..Default::default() };
        (m, room)
    }

    #[test]
    fn bounding_faces_of_the_phase1_rooms() {
        let (m, room) = house();
        let faces = bounding_faces(&m, &room);
        assert_eq!(faces.len(), 4);
        assert!(faces.windows(2).all(|w| w[0].wall <= w[1].wall));
        let east = faces.iter().find(|f| f.look.x > 0.5).unwrap();
        assert!((east.a.x - 6.0).abs() < 1e-9 && (east.b.x - 6.0).abs() < 1e-9);
        assert!(east.a.y.min(east.b.y).abs() < 1e-9 && (east.a.y.max(east.b.y) - 4.0).abs() < 1e-9);
        // A room on another level has none.
        let other = Room { level: 3, ..room.clone() };
        assert!(bounding_faces(&m, &other).is_empty());
    }

    #[test]
    fn look_from_the_room_at_the_wall() {
        let (m, room) = house();
        for (wall, look) in [(0u32, (0.0, -1.0)), (1, (1.0, 0.0)), (2, (0.0, 1.0)), (3, (-1.0, 0.0))] {
            let f = face_of(&m, &room, ArchId(wall)).unwrap();
            assert!((f.look.x - look.0).abs() < 1e-9 && (f.look.y - look.1).abs() < 1e-9, "w{wall}: {:?}", f.look);
        }
        // Clockwise polygons give the same looks.
        let mut cw = room.clone();
        cw.polygon.reverse();
        let f = face_of(&m, &cw, ArchId(1)).unwrap();
        assert!((f.look.x - 1.0).abs() < 1e-9);
        assert!(face_of(&m, &room, ArchId(99)).is_none());
    }

    #[test]
    fn wall_on_side_picks_the_east_wall() {
        let (m, room) = house();
        assert_eq!(wall_on_side(&m, &room, "east").map(|f| f.wall), Some(ArchId(1)));
        assert_eq!(wall_on_side(&m, &room, "north").map(|f| f.wall), Some(ArchId(2)));
        assert_eq!(wall_on_side(&m, &room, "south").map(|f| f.wall), Some(ArchId(0)));
        assert_eq!(wall_on_side(&m, &room, "west").map(|f| f.wall), Some(ArchId(3)));
        assert!(wall_on_side(&m, &room, "up").is_none());
    }

    #[test]
    fn bounding_faces_zero_area_room() {
        let mut m = ArchModel::default();
        m.levels.push(cadcraft_doc::Level { index: 0, ..Default::default() });
        // Collinear points: zero area.
        let room = Room { polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0)], ..Default::default() };
        assert!(bounding_faces(&m, &room).is_empty());
    }

    #[test]
    fn face_of_wall_not_bounding_room() {
        let (m, room) = house();
        // Add a wall far away that does not bound the room.
        let id = m.push_wall(Wall { start: Vec2::new(10.0, 0.0), end: Vec2::new(10.0, 4.0), thickness: 0.1, height: 2.4, ..Default::default() });
        assert!(face_of(&m, &room, id).is_none());
    }
}
