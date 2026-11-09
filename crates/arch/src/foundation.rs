//! Foundations (phase 16): which walls a foundation stands under, and where its footings go.
//!
//! A foundation supports the exterior walls of one level that have no level below them: the
//! lowest level's, and any on grade higher up (an entry landing, a garage, a bump-out). Each such
//! wall becomes a [`FoundationWall`]: its outer face from outer corner to outer corner and the
//! side the building is on. The 3D (`arch3d::foundation`), the plan (`render::arch_foundation`)
//! and the exports all take the footing's plan outline from [`footing_outline`].

use cadcraft_doc::arch::{
    ArchId, ArchModel, CONCRETE_WALL_TYPE, FOOTING_WIDTH, Level, MIN_WALL_LENGTH, RefSide, STEM_THICKNESS, Wall, WallLayer, WallType,
};
use cadcraft_geom::Vec2;

use crate::ceiling::room_at_point;
use crate::grade::{GradeClass, grade_class};

/// How far past the wall's face a room is looked for, to tell the inside from the outside (m).
const ROOM_PROBE: f64 = 0.05;
/// How close a vertex must be to the outer face line to lie on it (m).
const ON_FACE: f64 = 1e-6;
/// How much lower a level must be to count as below (m).
const BELOW: f64 = 1e-6;
/// How far a footing reaches past the stem's faces and the wall's ends (m): (0.6 - 0.2) / 2.
const FOOTING_OVERHANG: f64 = 0.2;

/// One wall a foundation stands under.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FoundationWall {
    pub wall: ArchId,
    /// The wall's outer face, from `a` to `b` in the wall's direction, reaching the outer corners.
    pub a: Vec2,
    pub b: Vec2,
    /// The unit normal into the building.
    pub inward: Vec2,
    pub thickness: f64,
    /// Whether the wall's level is a basement (its own wall is the foundation wall).
    pub basement: bool,
}

/// The elevation of level `index` (0 when the model has no such level, as `Wall::bottom_z`).
fn elevation_of(model: &ArchModel, index: u32) -> f64 {
    model.level(index).map_or(0.0, |l| l.elevation)
}

/// Whether the wall's centre line, thickness and length are usable.
fn usable(w: &Wall) -> bool {
    w.start.is_finite()
        && w.end.is_finite()
        && w.thickness.is_finite()
        && w.thickness > 0.0
        && w.start.dist(w.end).is_finite()
        && w.start.dist(w.end) >= MIN_WALL_LENGTH
}

/// Whether wall `w` stands on level `level` (its base level, or a level it spans).
fn on_level(model: &ArchModel, w: &Wall, level: u32) -> bool {
    crate::room::wall_on_level(model, w, level)
}

/// The unit normal into the building for `w` on its base level: the side where a room of that
/// level holds the point just past the wall's face, else the side facing the centroid of the
/// level's exterior wall midpoints (left on a tie, the inside of a counter-clockwise ring).
fn inward_of(model: &ArchModel, w: &Wall) -> Option<Vec2> {
    if !usable(w) {
        return None;
    }
    let d = (w.end - w.start).normalized();
    let left = d.perp();
    let mid = (w.start + w.end) / 2.0;
    let off = w.thickness / 2.0 + ROOM_PROBE;
    if room_at_point(model, w.base_level, mid + left * off).is_some() {
        return Some(left);
    }
    if room_at_point(model, w.base_level, mid - left * off).is_some() {
        return Some(-left);
    }
    let mids: Vec<Vec2> = model
        .walls
        .iter()
        .filter(|o| o.exterior && !o.divider && !o.retaining && o.base_level == w.base_level && usable(o))
        .map(|o| (o.start + o.end) / 2.0)
        .collect();
    if mids.is_empty() {
        return Some(left);
    }
    let centroid = mids.iter().fold(Vec2::ZERO, |acc, p| acc + *p) / mids.len() as f64;
    if !centroid.is_finite() || (centroid - mid).dot(left) >= 0.0 { Some(left) } else { Some(-left) }
}

/// Distance from `p` to the segment `a`-`b`.
fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let l2 = ab.dot(ab);
    if !(l2 > 1e-18) {
        return p.dist(a);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    p.dist(a + ab * t)
}

/// Whether some lower level stands under `w`: a room of a level more than 1e-6 m lower holds the
/// point 0.05 m inside the wall's inner face at its midpoint, or a wall standing on such a level
/// (based there or spanning it) has its centre line within half its thickness of `w`'s midpoint.
/// `false` for an unusable wall.
pub fn has_level_below(model: &ArchModel, w: &Wall) -> bool {
    let Some(inward) = inward_of(model, w) else {
        return false;
    };
    let e = elevation_of(model, w.base_level);
    if !e.is_finite() {
        return false;
    }
    let mid = (w.start + w.end) / 2.0;
    let m_in = mid + inward * (w.thickness / 2.0 + ROOM_PROBE);
    model.levels.iter().filter(|l| l.elevation.is_finite() && l.elevation < e - BELOW).any(|l| {
        room_at_point(model, l.index, m_in).is_some()
            || model.walls.iter().any(|o| {
                o.id != w.id
                    && !o.divider
                    && o.start.is_finite()
                    && o.end.is_finite()
                    && o.thickness.is_finite()
                    && o.spans_level(l, &model.levels)
                    && segment_distance(mid, o.start, o.end) <= o.thickness.max(0.0) / 2.0 + 1e-6
            })
    })
}

/// The outer face of `w` (outer side `out`, a unit normal) from the solved join ring `ring`: the
/// least and greatest point along the wall of the ring's vertices on the outer face line, else the
/// raw face from end to end.
fn outer_face(w: &Wall, out: Vec2, ring: Option<&[Vec2]>) -> (Vec2, Vec2) {
    let d = (w.end - w.start).normalized();
    let half = w.thickness / 2.0;
    let base = w.start + out * half;
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for v in ring.unwrap_or(&[]) {
        if !v.is_finite() || ((*v - w.start).dot(out) - half).abs() > ON_FACE {
            continue;
        }
        let t = (*v - w.start).dot(d);
        lo = lo.min(t);
        hi = hi.max(t);
    }
    if lo.is_finite() && hi.is_finite() && hi - lo >= MIN_WALL_LENGTH { (base + d * lo, base + d * hi) } else { (base, w.end + out * half) }
}

/// The walls a foundation under level `level` stands under: the level's exterior walls (not
/// dividers or retaining walls, finite, at least `MIN_WALL_LENGTH` long) with no level below them,
/// in model order. Their outer faces come from the level's own solved joins, so they reach the
/// outer corners.
pub fn foundation_walls(model: &ArchModel, level: u32) -> Vec<FoundationWall> {
    let founded: Vec<(&Wall, Vec2)> = model
        .walls
        .iter()
        .filter(|w| w.exterior && !w.divider && !w.retaining && w.base_level == level && usable(w) && !has_level_below(model, w))
        .filter_map(|w| inward_of(model, w).map(|n| (w, n)))
        .collect();
    if founded.is_empty() {
        return Vec::new();
    }
    // The level's own joins (walls on other levels sharing the corners would disturb them).
    let temp = ArchModel {
        walls: model.walls.iter().filter(|w| !w.divider && on_level(model, w, level)).cloned().collect(),
        types: model.types.clone(),
        levels: model.levels.clone(),
        ..ArchModel::default()
    };
    let shapes = crate::join::solve_joins(&temp);
    let basement = model.level(level).is_some_and(|l| grade_class(model, l) == GradeClass::Basement);
    founded
        .into_iter()
        .filter_map(|(w, inward)| {
            let ring = shapes.iter().find(|s| s.id == w.id).map(|s| s.polygon.as_slice());
            let (a, b) = outer_face(w, -inward, ring);
            (a.is_finite() && b.is_finite() && a.dist(b) >= MIN_WALL_LENGTH).then(|| FoundationWall {
                wall: w.id,
                a,
                b,
                inward,
                thickness: w.thickness,
                basement,
            })
        })
        .collect()
}

/// The levels with at least one foundation wall, ascending by index.
pub fn foundation_levels(model: &ArchModel) -> Vec<u32> {
    let mut levels: Vec<u32> = model.walls.iter().map(|w| w.base_level).collect();
    levels.sort_unstable();
    levels.dedup();
    levels.retain(|l| !foundation_walls(model, *l).is_empty());
    levels
}

/// Whether level `level` is a basement (its floor below the finished grade somewhere along its
/// exterior); `false` for a level the model does not have.
pub fn is_basement_level(model: &ArchModel, level: u32) -> bool {
    model.level(level).is_some_and(|l: &Level| grade_class(model, l) == GradeClass::Basement)
}

/// The footing's outline in plan, counter-clockwise. On grade: 0.6 m wide, centred under the
/// 0.2 m stem (from 0.2 m outside the outer face to 0.4 m inside it), reaching 0.2 m past each
/// end of the face so the corners are covered. Under a basement wall: 0.6 m wide centred on the
/// wall's centre line, over the same length. A degenerate wall gives four copies of `a`.
pub fn footing_outline(w: &FoundationWall) -> [Vec2; 4] {
    let ab = w.b - w.a;
    let len = ab.len();
    if !(len.is_finite() && len >= MIN_WALL_LENGTH) || !w.inward.is_finite() || !w.thickness.is_finite() {
        return [w.a; 4];
    }
    let u = ab / len;
    let n = w.inward;
    let (a, b) = (w.a - u * FOOTING_OVERHANG, w.b + u * FOOTING_OVERHANG);
    let (inner, outer) = if w.basement {
        let c = n * (w.thickness / 2.0);
        (c + n * (FOOTING_WIDTH / 2.0), c - n * (FOOTING_WIDTH / 2.0))
    } else {
        (n * (STEM_THICKNESS + FOOTING_OVERHANG), -(n * FOOTING_OVERHANG))
    };
    let ring = [a + outer, b + outer, b + inner, a + inner];
    if crate::join::polygon_area(&ring) < 0.0 { [ring[3], ring[2], ring[1], ring[0]] } else { ring }
}

/// The "Concrete 0.2" wall type a basement's exterior walls take: one 0.2 m concrete layer,
/// referenced on its centre.
pub fn concrete_wall_type() -> WallType {
    WallType {
        name: CONCRETE_WALL_TYPE.to_string(),
        layers: vec![WallLayer { name: "concrete".to_string(), thickness: STEM_THICKNESS, material: "concrete".to_string(), ..Default::default() }],
        reference: RefSide::Centre,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use cadcraft_doc::arch::{Room, Terrain};
    use cadcraft_geom::Vec3;

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2::new(x, y)
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![v(x0, y0), v(x1, y0), v(x1, y1), v(x0, y1)]
    }

    fn wall(a: (f64, f64), b: (f64, f64), thickness: f64, exterior: bool, level: u32) -> Wall {
        Wall { start: v(a.0, a.1), end: v(b.0, b.1), thickness, height: 2.4, exterior, base_level: level, ..Default::default() }
    }

    fn ring(m: &mut ArchModel, x0: f64, y0: f64, x1: f64, y1: f64, level: u32) -> Vec<ArchId> {
        [((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))]
            .into_iter()
            .map(|(a, b)| m.push_wall(wall(a, b, 0.2, true, level)))
            .collect()
    }

    /// The phase-1 house on level 0: the 0.2 m ring (w0 south, w1 east, w2 north, w3 west), the
    /// interior walls w4 (y = 3) and w5 (x = 6), and the rooms S, NW and NE.
    fn house() -> ArchModel {
        let mut m = ArchModel::default();
        ring(&mut m, 0.0, 0.0, 10.0, 8.0, 0);
        m.push_wall(wall((0.0, 3.0), (10.0, 3.0), 0.1, false, 0));
        m.push_wall(wall((6.0, 3.0), (6.0, 8.0), 0.1, false, 0));
        for poly in [rect(0.1, 0.1, 9.9, 2.95), rect(0.1, 3.05, 5.95, 7.9), rect(6.05, 3.05, 9.9, 7.9)] {
            m.push_room(Room { level: 0, polygon: poly, ..Default::default() });
        }
        m
    }

    fn add_level(m: &mut ArchModel, name: &str, elevation: f64) -> u32 {
        let index = m.levels.len() as u32;
        m.levels.push(Level { index, name: name.into(), elevation, ..Default::default() });
        index
    }

    /// The house with a basement at -2.7: the same ring on the basement level and its room.
    fn with_basement() -> (ArchModel, u32) {
        let mut m = house();
        let b = add_level(&mut m, "Basement", -2.7);
        ring(&mut m, 0.0, 0.0, 10.0, 8.0, b);
        m.push_room(Room { level: b, polygon: rect(0.1, 0.1, 9.9, 7.9), ..Default::default() });
        (m, b)
    }

    fn near(a: Vec2, x: f64, y: f64) -> bool {
        (a.x - x).abs() < 1e-9 && (a.y - y).abs() < 1e-9
    }

    #[test]
    fn phase1_house_founds_its_four_exterior_walls() {
        let m = house();
        let fw = foundation_walls(&m, 0);
        assert_eq!(fw.iter().map(|w| w.wall.0).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        let faces = [((-0.1, -0.1), (10.1, -0.1), (0.0, 1.0)), ((10.1, -0.1), (10.1, 8.1), (-1.0, 0.0))];
        for (w, (a, b, n)) in fw.iter().zip(faces) {
            assert!(near(w.a, a.0, a.1) && near(w.b, b.0, b.1), "{w:?}");
            assert!(near(w.inward, n.0, n.1), "{w:?}");
        }
        assert!(near(fw[2].a, 10.1, 8.1) && near(fw[2].b, -0.1, 8.1), "{:?}", fw[2]);
        assert!(near(fw[3].a, -0.1, 8.1) && near(fw[3].b, -0.1, -0.1), "{:?}", fw[3]);
        assert!(fw.iter().all(|w| !w.basement && (w.thickness - 0.2).abs() < 1e-12));
        assert_eq!(foundation_levels(&m), vec![0]);
        // The footing under the south wall: 0.6 wide centred under the stem (y -0.3..0.3), 0.2 past
        // each outer corner (x -0.3..10.3), counter-clockwise.
        let f = footing_outline(&fw[0]);
        assert!(near(f[0], -0.3, -0.3) && near(f[1], 10.3, -0.3) && near(f[2], 10.3, 0.3) && near(f[3], -0.3, 0.3), "{f:?}");
        // The north wall runs west, so its outline is reordered to stay counter-clockwise.
        let f = footing_outline(&fw[2]);
        assert!(crate::join::polygon_area(&f) > 0.0, "{f:?}");
        let (lo, hi) = f.iter().fold((v(f64::MAX, f64::MAX), v(f64::MIN, f64::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        assert!(near(lo, -0.3, 7.7) && near(hi, 10.3, 8.3), "{f:?}");
    }

    #[test]
    fn interior_walls_are_never_founded() {
        let mut m = house();
        assert!(foundation_walls(&m, 0).iter().all(|w| w.wall.0 < 4));
        // Dividers and retaining walls are never founded either, even marked exterior.
        m.walls[0].divider = true;
        m.walls[1].retaining = true;
        assert_eq!(foundation_walls(&m, 0).iter().map(|w| w.wall.0).collect::<Vec<_>>(), vec![2, 3]);
        // A model with only interior walls has no foundation level.
        let mut m = ArchModel::default();
        m.push_wall(wall((0.0, 0.0), (4.0, 0.0), 0.1, false, 0));
        assert!(foundation_levels(&m).is_empty());
    }

    #[test]
    fn walls_over_a_basement_have_a_level_below() {
        let (m, b) = with_basement();
        for w in m.walls.iter().filter(|w| w.base_level == 0 && w.exterior) {
            assert!(has_level_below(&m, w), "{:?}", w.id);
        }
        assert!(foundation_walls(&m, 0).is_empty());
        assert_eq!(foundation_levels(&m), vec![b]);
        // The basement walls alone (no basement room) still stand under the ring.
        let (mut m, _) = with_basement();
        m.rooms.retain(|r| r.level == 0);
        assert!(foundation_walls(&m, 0).is_empty());
        // A wall beyond the basement's footprint has nothing below it.
        let mut m = m;
        let out = m.push_wall(wall((20.0, 0.0), (24.0, 0.0), 0.2, true, 0));
        assert_eq!(foundation_walls(&m, 0).iter().map(|w| w.wall).collect::<Vec<_>>(), vec![out]);
    }

    #[test]
    fn basement_walls_are_flagged() {
        let (m, b) = with_basement();
        let fw = foundation_walls(&m, b);
        assert_eq!(fw.len(), 4);
        assert!(fw.iter().all(|w| w.basement));
        assert!(is_basement_level(&m, b) && !is_basement_level(&m, 0) && !is_basement_level(&m, 9));
        // Centred on the wall's centre line: y -0.3..0.3 under the south wall, x -0.3..10.3.
        let f = footing_outline(&fw[0]);
        assert!(near(f[0], -0.3, -0.3) && near(f[1], 10.3, -0.3) && near(f[2], 10.3, 0.3) && near(f[3], -0.3, 0.3), "{f:?}");
        // A thicker basement wall keeps its footing centred under it.
        let mut thick = fw[0].clone();
        thick.thickness = 0.3;
        let f = footing_outline(&thick);
        assert!(near(f[0], -0.3, -0.25) && near(f[2], 10.3, 0.35), "{f:?}");
        let t = concrete_wall_type();
        assert_eq!((t.name.as_str(), t.reference), ("Concrete 0.2", RefSide::Centre));
        assert_eq!(
            t.layers.iter().map(|l| (l.name.as_str(), l.thickness, l.material.as_str())).collect::<Vec<_>>(),
            vec![("concrete", 0.2, "concrete")]
        );
    }

    #[test]
    fn studio_on_grade_higher_up_is_founded() {
        let mut m = house();
        let s = add_level(&mut m, "Studio", 1.35);
        let ids = ring(&mut m, 20.0, 0.0, 24.0, 4.0, s);
        assert_eq!(foundation_levels(&m), vec![0, s]);
        let fw = foundation_walls(&m, s);
        assert_eq!(fw.iter().map(|w| w.wall).collect::<Vec<_>>(), ids);
        assert!(near(fw[0].a, 19.9, -0.1) && near(fw[0].b, 24.1, -0.1) && near(fw[0].inward, 0.0, 1.0), "{:?}", fw[0]);
        assert!(fw.iter().all(|w| !w.basement));
        // A level above the house's rooms is not on grade.
        let up = add_level(&mut m, "Level 2", 2.7);
        ring(&mut m, 0.0, 0.0, 10.0, 8.0, up);
        assert!(foundation_walls(&m, up).is_empty());
        assert_eq!(foundation_levels(&m), vec![0, s]);
    }

    #[test]
    fn inward_side_from_rooms_or_the_centroid() {
        // A clockwise ring (inside on the right) with rooms: the rooms decide.
        let mut m = ArchModel::default();
        for (a, b) in [((0.0, 0.0), (0.0, 4.0)), ((0.0, 4.0), (4.0, 4.0)), ((4.0, 4.0), (4.0, 0.0)), ((4.0, 0.0), (0.0, 0.0))] {
            m.push_wall(wall(a, b, 0.2, true, 0));
        }
        m.push_room(Room { level: 0, polygon: rect(0.1, 0.1, 3.9, 3.9), ..Default::default() });
        let fw = foundation_walls(&m, 0);
        assert!(near(fw[0].inward, 1.0, 0.0) && near(fw[0].a, -0.1, -0.1) && near(fw[0].b, -0.1, 4.1), "{:?}", fw[0]);
        assert!(fw.iter().all(|w| crate::join::polygon_area(&footing_outline(w)) > 0.0));
        // Without rooms: towards the centroid of the exterior walls' midpoints.
        m.rooms.clear();
        let fw = foundation_walls(&m, 0);
        assert!(near(fw[0].inward, 1.0, 0.0) && near(fw[1].inward, 0.0, -1.0), "{fw:?}");
        // A lone wall: its left side.
        let mut m = ArchModel::default();
        m.push_wall(wall((0.0, 0.0), (4.0, 0.0), 0.2, true, 0));
        let fw = foundation_walls(&m, 0);
        assert!(near(fw[0].inward, 0.0, 1.0) && near(fw[0].a, 0.0, -0.1) && near(fw[0].b, 4.0, -0.1), "{fw:?}");
    }

    #[test]
    fn footing_depth_ignores_terrain_and_pad() {
        // A terrain under the house with its building pad: the founded walls, their faces and the
        // footings are those of the house on no terrain (depth is measured from the floor).
        let bare = foundation_walls(&house(), 0);
        let mut m = house();
        let mut t = Terrain::default();
        t.boundary = rect(-50.0, -50.0, 50.0, 50.0);
        t.points = vec![Vec3::new(-50.0, -50.0, 0.0), Vec3::new(50.0, -50.0, 0.0), Vec3::new(50.0, 50.0, 0.0), Vec3::new(-50.0, 50.0, 0.0)];
        t.pad = true;
        m.terrain = Some(t);
        assert!(!is_basement_level(&m, 0));
        let fw = foundation_walls(&m, 0);
        assert_eq!(fw, bare);
        assert_eq!(fw.iter().map(footing_outline).collect::<Vec<_>>(), bare.iter().map(footing_outline).collect::<Vec<_>>());
    }

    #[test]
    fn hostile_walls() {
        let mut m = house();
        m.push_wall(wall((f64::NAN, 0.0), (1.0, 1.0), 0.2, true, 0));
        m.push_wall(wall((30.0, 0.0), (30.0, 0.0), 0.2, true, 0));
        m.push_wall(wall((40.0, 0.0), (41.0, 0.0), f64::NAN, true, 0));
        m.push_wall(wall((50.0, 0.0), (51.0, 0.0), -0.2, true, 0));
        m.push_wall(wall((1e300, 0.0), (-1e300, 0.0), 0.2, true, 0));
        // The NaN, zero-length, NaN-thick and negative-thick walls are skipped; the huge one is
        // finite and founded, with finite faces.
        let fw = foundation_walls(&m, 0);
        assert_eq!(fw.iter().map(|w| w.wall.0).collect::<Vec<_>>(), vec![0, 1, 2, 3, 10]);
        assert!(fw.iter().all(|w| w.a.is_finite() && w.b.is_finite() && footing_outline(w).iter().all(|p| p.is_finite())));
        for w in &m.walls {
            let _ = has_level_below(&m, w);
        }
        // No rooms at all, a hostile level and an unknown level.
        let mut m = house();
        m.rooms.clear();
        assert_eq!(foundation_walls(&m, 0).len(), 4);
        assert!(foundation_walls(&m, 7).is_empty());
        m.levels[0].elevation = f64::NAN;
        let _ = foundation_walls(&m, 0);
        let _ = foundation_levels(&m);
        // Hostile foundation walls give finite or repeated outlines, never a panic.
        for w in [
            FoundationWall::default(),
            FoundationWall { a: v(f64::NAN, 0.0), b: v(1.0, 0.0), inward: v(0.0, 1.0), thickness: 0.2, ..Default::default() },
            FoundationWall { a: v(0.0, 0.0), b: v(1.0, 0.0), inward: v(0.0, f64::INFINITY), thickness: 0.2, ..Default::default() },
        ] {
            let f = footing_outline(&w);
            assert!(f.iter().all(|p| p.x.to_bits() == f[0].x.to_bits() && p.y.to_bits() == f[0].y.to_bits()), "{f:?}");
        }
        assert!(foundation_walls(&ArchModel::default(), 0).is_empty());
    }
}
