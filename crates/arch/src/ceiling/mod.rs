//! Ceiling point queries (phase 16): where a room's rough and finished ceiling is at a plan
//! point, the roof underside over a level, and where ceiling items hang. Every elevation here is
//! absolute (the level's elevation is already added).
//!
//! A room's ceiling height (its own, else its level's) is its ROUGH ceiling: the underside of the
//! structure above. [`rough_ceiling_in`] adjusts that flat height for vaults ([`vault`]) and
//! trays ([`tray`]); [`finished_ceiling_in`] subtracts the room's finish stack ([`stack`]); and
//! [`hang_z`] is where ceiling items hang: the underside of a soffit ([`soffit`]) over the point,
//! else the finished ceiling. "A room under a roof" means, throughout phase 16, a point that
//! [`roof_underside_at`] answers for: a plane of a roof whose `level` is the room's level holds it.

pub mod overhead;
pub mod soffit;
pub mod stack;
pub mod tray;
pub mod vault;

use cadcraft_doc::arch::{ArchId, ArchModel, Level, Placement, RoofPlane, Room};
use cadcraft_geom::{Line, Vec2, point_in_polygon};

/// Most sample points [`lowest_underside_over`] evaluates.
pub const MAX_UNDERSIDE_SAMPLES: usize = 100_000;
/// Most edge pairs [`lowest_underside_over`] intersects (bounds the work on a hostile model).
const MAX_EDGE_PAIRS: usize = 4_000_000;
/// A point this close to a roof plane's ring edge is under that plane.
const ON_EDGE: f64 = 1e-9;
/// Thinnest ceiling height that still builds a ceiling (the 20 mm ceiling solid, as
/// `arch3d::floors::ceiling_top`).
const MIN_CEILING_HEIGHT: f64 = 0.02;
/// How close a ceiling item's top must be to [`hang_z`] to count as seated.
const SEATED: f64 = 1e-6;

/// The first room on `level` whose polygon holds `p` and none of whose holes does (the rule of
/// `room::room_at`). `None` for a non-finite `p`.
pub fn room_at_point(model: &ArchModel, level: u32, p: Vec2) -> Option<&Room> {
    if !p.is_finite() {
        return None;
    }
    model.rooms.iter().find(|r| {
        r.level == level && r.polygon.len() >= 3 && point_in_polygon(&r.polygon, p) && !r.holes.iter().any(|h| h.len() >= 3 && point_in_polygon(h, p))
    })
}

/// The flat rough ceiling of `room` (absolute z) with `level` as its level: the level's elevation
/// (0 without a level) plus the room's ceiling height when finite and positive, else the level's
/// (2.4 m without a level). `None` when the room type has no ceiling, or the height is not finite
/// or too thin to build a ceiling: the numbers of `arch3d::floors::ceiling_top`.
pub fn flat_ceiling_z_with(room: &Room, level: Option<&Level>) -> Option<f64> {
    if !room.room_type.has_ceiling() {
        return None;
    }
    let level_ceiling = level.map_or(Level::default().ceiling_height, |l| l.ceiling_height);
    let h = room.ceiling_height.filter(|h| h.is_finite() && *h > 0.0).unwrap_or(level_ceiling);
    if !h.is_finite() || h <= MIN_CEILING_HEIGHT {
        return None;
    }
    let z = level.map_or(0.0, |l| l.elevation) + h;
    z.is_finite().then_some(z)
}

/// [`flat_ceiling_z_with`] with the room's own level from `model`.
pub fn flat_ceiling_z(model: &ArchModel, room: &Room) -> Option<f64> {
    flat_ceiling_z_with(room, model.level(room.level))
}

/// The underside of roof plane `plane` at plan point `p`: `eave_z + slope * (d - overhang)`, with
/// `d` the distance from the base edge's line along its left normal and a non-finite or negative
/// overhang taken as 0 (exactly `arch3d::roof::underside_z(plane, plane.eave_z, p)`). `None` when
/// the eave, the slope or the base edge is not finite, the base edge is shorter than 1e-12, or
/// the result is not finite.
pub fn underside_z_of(plane: &RoofPlane, p: Vec2) -> Option<f64> {
    let [x0, y0, x1, y1] = plane.base_edge;
    if !(plane.eave_z.is_finite() && plane.slope.is_finite() && plane.base_edge.iter().all(|v| v.is_finite())) {
        return None;
    }
    let a = Vec2::new(x0, y0);
    let ab = Vec2::new(x1, y1) - a;
    let len = (ab.x * ab.x + ab.y * ab.y).sqrt();
    if !len.is_finite() || len < 1e-12 {
        return None;
    }
    let dir = Vec2::new(ab.x / len, ab.y / len);
    let normal = Vec2::new(-dir.y, dir.x);
    let v = p - a;
    let overhang = if plane.overhang.is_finite() && plane.overhang >= 0.0 { plane.overhang } else { 0.0 };
    let u = plane.eave_z + plane.slope * ((v.x * normal.x + v.y * normal.y) - overhang);
    u.is_finite().then_some(u)
}

/// The roof planes over `level`: every plane (ring of 3 or more points) of every roof whose
/// `level` is `level`, in model order.
fn level_planes(model: &ArchModel, level: u32) -> impl Iterator<Item = &RoofPlane> {
    model.roofs.iter().filter(move |r| r.level == level).flat_map(|r| r.planes.iter()).filter(|pl| pl.ring.len() >= 3)
}

/// The closing edges of a ring, `(ring[k], ring[k + 1])` and the last back to the first.
fn ring_edges(ring: &[Vec2]) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
    ring.iter().zip(ring.iter().cycle().skip(1)).map(|(a, b)| (*a, *b))
}

/// Whether `ring` holds `p`: inside it, or within [`ON_EDGE`] of one of its edges.
fn ring_holds(ring: &[Vec2], p: Vec2) -> bool {
    point_in_polygon(ring, p) || ring_edges(ring).any(|(a, b)| Line::new(a, b).dist(p) <= ON_EDGE)
}

/// The roof underside over plan point `p` on `level` (absolute z): the lowest
/// [`underside_z_of`] over the planes of [`level_planes`] whose ring holds `p` (inside it, or
/// within 1e-9 of an edge). `None` where no roof of that level covers `p`.
pub fn roof_underside_at(model: &ArchModel, level: u32, p: Vec2) -> Option<f64> {
    if !p.is_finite() {
        return None;
    }
    level_planes(model, level)
        .filter(|pl| ring_holds(&pl.ring, p))
        .filter_map(|pl| underside_z_of(pl, p))
        .fold(None, |best: Option<f64>, u| Some(best.map_or(u, |b| b.min(u))))
}

/// The lowest roof underside found over a polygon, and where.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UndersideMin {
    /// The plan point.
    pub at: Vec2,
    /// The underside there (absolute z).
    pub z: f64,
}

/// Where segments `a`-`b` and `c`-`d` cross, or `None` (parallel, apart, or not finite).
fn crossing(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<Vec2> {
    let r = b - a;
    let s = d - c;
    let denom = r.cross(s);
    if !denom.is_finite() || denom.abs() <= 1e-15 * (r.len() * s.len()).max(f64::MIN_POSITIVE) {
        return None;
    }
    let ac = c - a;
    let t = ac.cross(s) / denom;
    let u = ac.cross(r) / denom;
    const T: f64 = 1e-12;
    if !(t.is_finite() && u.is_finite() && (-T..=1.0 + T).contains(&t) && (-T..=1.0 + T).contains(&u)) {
        return None;
    }
    let x = Vec2::new(a.x + r.x * t, a.y + r.y * t);
    x.is_finite().then_some(x)
}

/// The lowest roof underside over `polygon` on `level`, and whether some part of it has no roof
/// over it. The samples are the polygon's vertices, every crossing of a polygon edge with a ring
/// edge of the level's roof planes, and every ring vertex inside the polygon (or on its
/// boundary): the underside is planar on each plane, so its minimum over the polygon is at one
/// of them. Returns the covered sample with the lowest [`roof_underside_at`] (`None` when no
/// sample is covered) and whether any sample is uncovered (a non-finite vertex counts as
/// uncovered). At most [`MAX_UNDERSIDE_SAMPLES`] samples.
pub fn lowest_underside_over(model: &ArchModel, level: u32, polygon: &[Vec2]) -> (Option<UndersideMin>, bool) {
    let rings: Vec<&[Vec2]> = level_planes(model, level).map(|pl| pl.ring.as_slice()).collect();
    let mut samples: Vec<Vec2> = polygon.iter().copied().take(MAX_UNDERSIDE_SAMPLES).collect();
    if polygon.len() >= 2 {
        let mut pairs = 0usize;
        'edges: for (a, b) in ring_edges(polygon) {
            for ring in &rings {
                for (c, d) in ring_edges(ring) {
                    pairs += 1;
                    if pairs > MAX_EDGE_PAIRS || samples.len() >= MAX_UNDERSIDE_SAMPLES {
                        break 'edges;
                    }
                    if let Some(x) = crossing(a, b, c, d) {
                        samples.push(x);
                    }
                }
            }
        }
    }
    if polygon.len() >= 3 {
        'rings: for ring in &rings {
            for &v in ring.iter() {
                if samples.len() >= MAX_UNDERSIDE_SAMPLES {
                    break 'rings;
                }
                if v.is_finite() && ring_holds(polygon, v) {
                    samples.push(v);
                }
            }
        }
    }
    let mut best: Option<UndersideMin> = None;
    let mut uncovered = false;
    for q in samples {
        match roof_underside_at(model, level, q) {
            Some(z) => {
                if best.is_none_or(|b| z < b.z) {
                    best = Some(UndersideMin { at: q, z });
                }
            }
            None => uncovered = true,
        }
    }
    (best, uncovered)
}

/// The rough ceiling of `room` at plan point `p` (absolute z): its flat ceiling, adjusted where
/// it follows the roof ([`vault::adjust`]) and lifted by its trays ([`tray::lift`]).
///
/// What lies over `p` ([`overhead::overhead_at`]) clips it: the floor above caps it at that
/// floor's underside; an Open Below room above replaces it with the upper room's own rough ceiling.
pub fn rough_ceiling_in(model: &ArchModel, room: &Room, p: Vec2) -> Option<f64> {
    rough_ceiling_depth(model, room, p, model.levels.len())
}

fn rough_ceiling_depth(model: &ArchModel, room: &Room, p: Vec2, depth: usize) -> Option<f64> {
    use overhead::Overhead;
    let over = overhead::overhead_at(model, room.level, p);
    if let Overhead::Open { room: up, .. } = over {
        let upper = overhead::room_by_id(model, up)?;
        return rough_ceiling_depth(model, upper, p, depth.checked_sub(1)?);
    }
    let flat = flat_ceiling_z(model, room)?;
    let mut z = vault::adjust(model, room, flat, p) + tray::lift(model, room, p);
    if let Overhead::Floor { underside, .. } = over
        && underside.is_finite()
    {
        z = z.min(underside);
    }
    z.is_finite().then_some(z)
}

/// The finished ceiling of `room` at `p` (absolute z): the rough ceiling minus the finish stack.
/// Under an Open Below room it is the upper room's finished ceiling (its own stack).
pub fn finished_ceiling_in(model: &ArchModel, room: &Room, p: Vec2) -> Option<f64> {
    finished_ceiling_depth(model, room, p, model.levels.len())
}

fn finished_ceiling_depth(model: &ArchModel, room: &Room, p: Vec2, depth: usize) -> Option<f64> {
    if let overhead::Overhead::Open { room: up, .. } = overhead::overhead_at(model, room.level, p) {
        let upper = overhead::room_by_id(model, up)?;
        return finished_ceiling_depth(model, upper, p, depth.checked_sub(1)?);
    }
    let z = rough_ceiling_depth(model, room, p, depth)? - stack::thickness(&room.ceiling_stack);
    z.is_finite().then_some(z)
}

/// The finished ceiling over plan point `p` on `level` (absolute z), `None` outside every room
/// (or in a room without a ceiling).
pub fn finished_ceiling_at(model: &ArchModel, level: u32, p: Vec2) -> Option<f64> {
    finished_ceiling_in(model, room_at_point(model, level, p)?, p)
}

/// Where a ceiling item at plan point `p` on `level` hangs from (absolute z): the underside of a
/// soffit over `p`, else the finished ceiling.
pub fn hang_z(model: &ArchModel, level: u32, p: Vec2) -> Option<f64> {
    soffit::bottom_at(model, level, p).or_else(|| finished_ceiling_at(model, level, p))
}

/// The elevation of level `level` (0 when the model has no such level).
fn level_elevation(model: &ArchModel, level: u32) -> f64 {
    model.level(level).map_or(0.0, |l| l.elevation)
}

/// The ceiling-placed furnishings on `level` whose top (level elevation + elevation + height)
/// is within 1e-6 of [`hang_z`] at their point: the items to [`reseat`] after a ceiling change.
/// Items someone moved off the ceiling by hand are not listed, so they stay where they are.
pub fn seated_ceiling_items(model: &ArchModel, level: u32) -> Vec<ArchId> {
    let base = level_elevation(model, level);
    model
        .furnishings
        .iter()
        .filter(|f| f.placement == Placement::Ceiling && f.level == level)
        .filter(|f| {
            let top = base + f.elevation + f.size[2];
            let at = Vec2::new(f.at[0], f.at[1]);
            top.is_finite() && hang_z(model, level, at).is_some_and(|h| (top - h).abs() <= SEATED)
        })
        .map(|f| f.id)
        .collect()
}

/// Hang every listed furnishing from [`hang_z`] at its point again: elevation = hang_z - level
/// elevation - height. Items with no ceiling over them stay. Returns how many moved.
///
/// Every command that changes a ceiling lists the seated items before it changes the model
/// (`let seated = seated_ceiling_items(&d.arch, level);`) and calls `reseat(&mut d.arch, &seated)`
/// after.
pub fn reseat(model: &mut ArchModel, items: &[ArchId]) -> usize {
    let mut moved = 0;
    for id in items {
        let Some(f) = model.furnishings.iter().find(|f| f.id == *id) else { continue };
        let (level, at, height) = (f.level, Vec2::new(f.at[0], f.at[1]), f.size[2]);
        let Some(h) = hang_z(model, level, at) else { continue };
        let elevation = h - level_elevation(model, level) - height;
        if !elevation.is_finite() {
            continue;
        }
        if let Some(f) = model.furnishings.iter_mut().find(|f| f.id == *id)
            && (f.elevation - elevation).abs() > 1e-12
        {
            f.elevation = elevation;
            moved += 1;
        }
    }
    moved
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{CeilingLayer, Furnishing, Roof, RoofDirective, RoomType, Wall};

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// The phase-1 house: a 10 x 8 ring of 0.2 m walls 2.4 m high on level 0 (ceiling 2.4), with
    /// the rooms S (0.1..9.9 x 0.1..2.95), NW (0.1..5.95 x 3.05..7.9) and NE (6.05..9.9 x
    /// 3.05..7.9) pushed by hand; with `gable`, the phase-4 6/12 roof with gables on x = 0 and
    /// x = 10 (underside 2.4 + 0.5 (y + 0.1) for y <= 4, 2.4 + 0.5 (8.1 - y) above).
    fn house(gable: bool) -> ArchModel {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, name: "Level 1".into(), ..Level::default() });
        let pts = [(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)];
        for k in 0..4 {
            let (a, b) = (pts[k], pts[(k + 1) % 4]);
            let mut w = Wall::new(Vec2::new(a.0, a.1), Vec2::new(b.0, b.1), 0.2);
            w.height = 2.4;
            w.exterior = true;
            if k == 1 || k == 3 {
                w.roof_directive = RoofDirective::Gable;
            }
            m.push_wall(w);
        }
        for poly in [rect(0.1, 0.1, 9.9, 2.95), rect(0.1, 3.05, 5.95, 7.9), rect(6.05, 3.05, 9.9, 7.9)] {
            m.push_room(Room { polygon: poly, level: 0, ..Room::default() });
        }
        if gable {
            let seed = Roof { pitch_rise: 6.0, pitch_run: 12.0, overhang: 0.5, thickness: 0.2, ..Roof::default() };
            let roof = crate::roof::build_roof(&m, 0, &seed).unwrap();
            m.push_roof(roof);
        }
        m
    }

    fn gable_u(y: f64) -> f64 {
        if y <= 4.0 { 2.4 + 0.5 * (y + 0.1) } else { 2.4 + 0.5 * (8.1 - y) }
    }

    fn pendant(level: u32, at: [f64; 2], elevation: f64) -> Furnishing {
        Furnishing { placement: Placement::Ceiling, level, at, elevation, size: [0.4, 0.4, 0.86], ..Default::default() }
    }

    #[test]
    fn flat_ceiling_z_matches_floors_ceiling_top() {
        let level = Level { elevation: 2.7, ceiling_height: 2.4, ..Level::default() };
        let room = Room { polygon: rect(0.0, 0.0, 4.0, 4.0), ..Room::default() };
        // The level's ceiling, its own, and a hostile own height that falls back.
        assert!(near(flat_ceiling_z_with(&room, Some(&level)).unwrap(), 5.1));
        let own = Room { ceiling_height: Some(3.0), ..room.clone() };
        assert!(near(flat_ceiling_z_with(&own, Some(&level)).unwrap(), 5.7));
        for h in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            let bad = Room { ceiling_height: Some(h), ..room.clone() };
            assert!(near(flat_ceiling_z_with(&bad, Some(&level)).unwrap(), 5.1), "{h}");
        }
        // No level: elevation 0, ceiling 2.4.
        assert!(near(flat_ceiling_z_with(&room, None).unwrap(), 2.4));
        // Decks and porches have no ceiling; a level ceiling that would build nothing gives none.
        for t in [RoomType::Deck, RoomType::Porch] {
            assert_eq!(flat_ceiling_z_with(&Room { room_type: t, ..room.clone() }, Some(&level)), None);
        }
        let thin = Level { ceiling_height: 0.02, ..level.clone() };
        assert_eq!(flat_ceiling_z_with(&room, Some(&thin)), None);
        let nan = Level { elevation: f64::NAN, ..level.clone() };
        assert_eq!(flat_ceiling_z_with(&room, Some(&nan)), None);
        // flat_ceiling_z reads the room's level from the model.
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, ..Level::default() });
        m.levels.push(Level { index: 1, elevation: 2.7, ..Level::default() });
        let r1 = Room { level: 1, ..room.clone() };
        assert!(near(flat_ceiling_z(&m, &r1).unwrap(), 5.1));
        assert!(near(flat_ceiling_z(&m, &room).unwrap(), 2.4));
    }

    #[test]
    fn roof_underside_at_gable_numbers() {
        let m = house(true);
        assert!(near(roof_underside_at(&m, 0, Vec2::new(5.0, 2.0)).unwrap(), 3.45));
        assert!(near(roof_underside_at(&m, 0, Vec2::new(5.0, 7.3)).unwrap(), 2.8));
        assert!(near(roof_underside_at(&m, 0, Vec2::new(5.0, -0.1)).unwrap(), 2.4));
        assert!(near(roof_underside_at(&m, 0, Vec2::new(5.0, 4.0)).unwrap(), 4.45), "the ridge");
        for y in [0.1, 1.0, 3.9, 4.1, 6.0, 7.9] {
            assert!(near(roof_underside_at(&m, 0, Vec2::new(3.0, y)).unwrap(), gable_u(y)), "{y}");
        }
        assert_eq!(roof_underside_at(&m, 0, Vec2::new(20.0, 20.0)), None, "outside the roof");
        assert_eq!(roof_underside_at(&m, 1, Vec2::new(5.0, 2.0)), None, "another level's roof");
        assert_eq!(roof_underside_at(&house(false), 0, Vec2::new(5.0, 2.0)), None, "no roof");
    }

    #[test]
    fn underside_matches_the_arch3d_formula() {
        // A sloped base edge from (1, 2) to (4, 6) (length 5), its left normal (-0.8, 0.6).
        let mut plane = RoofPlane {
            ring: rect(0.0, 0.0, 9.0, 9.0),
            slope: 0.75,
            base_edge: [1.0, 2.0, 4.0, 6.0],
            overhang: 0.3,
            eave_z: 2.5,
            ..RoofPlane::default()
        };
        let formula = |plane: &RoofPlane, p: Vec2| {
            let o = if plane.overhang.is_finite() && plane.overhang >= 0.0 { plane.overhang } else { 0.0 };
            let d = (p.x - 1.0) * -0.8 + (p.y - 2.0) * 0.6;
            plane.eave_z + plane.slope * (d - o)
        };
        for p in [Vec2::new(0.0, 5.0), Vec2::new(3.0, 3.0), Vec2::new(-2.0, 8.0), Vec2::new(1.0, 2.0)] {
            assert!((underside_z_of(&plane, p).unwrap() - formula(&plane, p)).abs() < 1e-12, "{p:?}");
        }
        for o in [-1.0, f64::NAN, f64::INFINITY] {
            plane.overhang = o;
            let p = Vec2::new(0.0, 5.0);
            assert!((underside_z_of(&plane, p).unwrap() - formula(&plane, p)).abs() < 1e-12, "{o}");
        }
        // Degenerate planes and points answer None.
        assert_eq!(underside_z_of(&RoofPlane { base_edge: [1.0, 1.0, 1.0, 1.0], ..plane.clone() }, Vec2::ZERO), None);
        assert_eq!(underside_z_of(&RoofPlane { eave_z: f64::NAN, ..plane.clone() }, Vec2::ZERO), None);
        assert_eq!(underside_z_of(&RoofPlane { slope: f64::INFINITY, ..plane.clone() }, Vec2::ZERO), None);
        assert_eq!(underside_z_of(&RoofPlane { base_edge: [f64::NAN, 0.0, 1.0, 0.0], ..plane.clone() }, Vec2::ZERO), None);
        assert_eq!(underside_z_of(&plane, Vec2::new(f64::NAN, 0.0)), None);
    }

    #[test]
    fn lowest_underside_over_finds_the_eave_side() {
        let m = house(true);
        // S: lowest along its south side, y = 0.1 (2.5); fully under the roof.
        let (min, uncovered) = lowest_underside_over(&m, 0, &rect(0.1, 0.1, 9.9, 2.95));
        let min = min.unwrap();
        assert!(near(min.z, 2.5) && near(min.at.y, 0.1), "{min:?}");
        assert!(!uncovered);
        // NE: lowest along its north side, y = 7.9 (2.5).
        let (min, _) = lowest_underside_over(&m, 0, &rect(6.05, 3.05, 9.9, 7.9));
        assert!(near(min.unwrap().z, 2.5) && near(min.unwrap().at.y, 7.9), "{min:?}");
        // Partly outside the roof: the corners outside are uncovered and the lowest point is
        // where the polygon crosses the eave outline (y = -0.6, overhang 0.5): 2.15.
        let (min, uncovered) = lowest_underside_over(&m, 0, &rect(2.0, -1.0, 4.0, 1.0));
        assert!(uncovered);
        let min = min.unwrap();
        assert!(near(min.z, 2.15) && near(min.at.y, -0.6), "{min:?}");
        // A plane whose ring lies inside the polygon: its lowest vertex is the answer.
        let mut tri = ArchModel::default();
        tri.push_roof(Roof {
            planes: vec![RoofPlane {
                ring: vec![Vec2::new(4.0, 1.0), Vec2::new(6.0, 1.0), Vec2::new(5.0, 3.0)],
                slope: 1.0,
                base_edge: [0.0, 0.0, 10.0, 0.0],
                eave_z: 2.0,
                ..RoofPlane::default()
            }],
            ..Roof::default()
        });
        let (min, uncovered) = lowest_underside_over(&tri, 0, &rect(0.0, 0.0, 10.0, 10.0));
        assert!(uncovered);
        assert!(near(min.unwrap().z, 3.0) && near(min.unwrap().at.y, 1.0), "{min:?}");
        // No roof: nothing covered.
        let (min, uncovered) = lowest_underside_over(&house(false), 0, &rect(0.1, 0.1, 9.9, 2.95));
        assert_eq!(min, None);
        assert!(uncovered);
        assert_eq!(lowest_underside_over(&m, 0, &[]), (None, false));
    }

    #[test]
    fn finished_ceiling_subtracts_the_stack() {
        let mut m = house(false);
        let stack = vec![
            CeilingLayer::new("drywall", Some("drywall"), 0.0127),
            CeilingLayer::new("air", None, 0.2),
            CeilingLayer::new("ceiling tile", Some("ceiling-tile"), 0.016),
        ];
        m.rooms[2].ceiling_stack = stack;
        let ne = m.rooms[2].clone();
        let p = Vec2::new(8.0, 5.0);
        assert!(near(rough_ceiling_in(&m, &ne, p).unwrap(), 2.4));
        assert!(near(finished_ceiling_in(&m, &ne, p).unwrap(), 2.1713));
        assert!(near(finished_ceiling_at(&m, 0, p).unwrap(), 2.1713));
        // The other rooms keep 2.4; a room's own ceiling height moves its finished ceiling.
        assert!(near(finished_ceiling_at(&m, 0, Vec2::new(3.0, 5.0)).unwrap(), 2.4));
        m.rooms[0].ceiling_height = Some(2.7);
        assert!(near(finished_ceiling_at(&m, 0, Vec2::new(5.0, 1.5)).unwrap(), 2.7));
    }

    #[test]
    fn finished_ceiling_outside_rooms_is_none() {
        let mut m = house(false);
        assert_eq!(finished_ceiling_at(&m, 0, Vec2::new(20.0, 20.0)), None);
        assert_eq!(finished_ceiling_at(&m, 0, Vec2::new(6.0, 5.0)), None, "inside the partition");
        assert_eq!(finished_ceiling_at(&m, 1, Vec2::new(5.0, 1.5)), None, "no room on level 1");
        m.rooms[0].holes = vec![rect(4.0, 1.0, 6.0, 2.0)];
        assert_eq!(finished_ceiling_at(&m, 0, Vec2::new(5.0, 1.5)), None, "in a hole");
        assert!(finished_ceiling_at(&m, 0, Vec2::new(2.0, 1.5)).is_some());
        m.rooms[1].room_type = RoomType::Deck;
        assert_eq!(finished_ceiling_at(&m, 0, Vec2::new(3.0, 5.0)), None, "a deck has no ceiling");
        assert_eq!(room_at_point(&m, 0, Vec2::new(3.0, 5.0)).map(|r| r.id), Some(m.rooms[1].id));
    }

    #[test]
    fn hang_z_is_the_finished_ceiling_without_soffits() {
        let mut m = house(true);
        m.rooms[2].ceiling_stack = vec![CeilingLayer { material: Some("drywall".into()), thickness: 0.0127, ..Default::default() }];
        for (p, want) in [((5.0, 1.5), Some(2.4)), ((3.0, 5.0), Some(2.4)), ((8.0, 5.0), Some(2.3873)), ((20.0, 20.0), None)] {
            let p = Vec2::new(p.0, p.1);
            assert_eq!(hang_z(&m, 0, p).map(|z| (z * 1e9).round()), want.map(|z: f64| (z * 1e9).round()), "{p:?}");
            assert_eq!(hang_z(&m, 0, p), finished_ceiling_at(&m, 0, p));
        }
    }

    #[test]
    fn reseat_moves_only_seated_items() {
        let mut m = house(false);
        // No structure: the floor above (underside 2.7) does not clip NE's raised 2.7 ceiling.
        m.levels.push(Level { index: 1, name: "Level 2".into(), elevation: 2.7, structure: 0.0, ..Level::default() });
        m.push_room(Room { polygon: rect(0.1, 0.1, 9.9, 7.9), level: 1, ..Room::default() });
        let seated = m.push_furnishing(pendant(0, [8.0, 5.0], 2.4 - 0.86));
        let offset = m.push_furnishing(pendant(0, [9.0, 6.0], 1.0));
        let upstairs = m.push_furnishing(pendant(1, [8.0, 5.0], 2.4 - 0.86));
        let floor = m.push_furnishing(Furnishing { placement: Placement::Floor, at: [8.0, 4.0], size: [1.0, 1.0, 2.4], ..Default::default() });
        assert_eq!(seated_ceiling_items(&m, 0), vec![seated]);
        assert_eq!(seated_ceiling_items(&m, 1), vec![upstairs], "tops measured from the level's elevation");
        // Raise NE's ceiling: only the seated item follows.
        let list = seated_ceiling_items(&m, 0);
        m.rooms[2].ceiling_height = Some(2.7);
        assert_eq!(reseat(&mut m, &list), 1);
        let top = |m: &ArchModel, id: ArchId| m.furnishing(id).map(|f| f.elevation + f.size[2]).unwrap();
        assert!(near(top(&m, seated), 2.7));
        assert!(near(top(&m, offset), 1.86));
        assert!(near(m.furnishing(floor).unwrap().elevation, 0.0));
        // Nothing to do the second time; an unknown id or an item outside every room is skipped.
        assert_eq!(reseat(&mut m, &list), 0);
        let lost = m.push_furnishing(pendant(0, [30.0, 30.0], 1.0));
        assert_eq!(reseat(&mut m, &[ArchId(999), lost]), 0);
    }

    #[test]
    fn ceiling_queries_survive_nan_and_huge_input() {
        let mut m = house(true);
        for p in [Vec2::new(f64::NAN, 1.0), Vec2::new(1e300, -1e300), Vec2::new(f64::INFINITY, 0.0)] {
            assert_eq!(room_at_point(&m, 0, p).map(|r| r.id), None);
            assert_eq!(finished_ceiling_at(&m, 0, p), None);
            assert_eq!(hang_z(&m, 0, p), None);
            assert_eq!(roof_underside_at(&m, 0, p), None);
        }
        let _ = lowest_underside_over(&m, 0, &[Vec2::new(f64::NAN, 0.0), Vec2::new(1e300, 1e300), Vec2::new(-1e300, 5.0)]);
        let (_, uncovered) = lowest_underside_over(&m, 0, &[Vec2::new(f64::NAN, 0.0), Vec2::new(5.0, 2.0), Vec2::new(5.0, 3.0)]);
        assert!(uncovered, "a non-finite vertex is uncovered");
        // Hostile roofs, rooms, levels and furnishings in a loaded file.
        m.roofs[0].planes[0].eave_z = f64::NAN;
        m.roofs[0].planes[1].ring = vec![Vec2::new(f64::NAN, 0.0), Vec2::new(1e300, 0.0), Vec2::new(0.0, f64::INFINITY)];
        let _ = roof_underside_at(&m, 0, Vec2::new(5.0, 2.0));
        let _ = lowest_underside_over(&m, 0, &rect(0.1, 0.1, 9.9, 2.95));
        m.rooms[0].polygon = vec![Vec2::new(f64::NAN, 0.0); 4];
        m.rooms[1].ceiling_height = Some(1e308);
        m.rooms[1].ceiling_stack = vec![CeilingLayer { thickness: f64::NAN, ..Default::default() }];
        m.levels[0].elevation = 1e308;
        let _ = finished_ceiling_at(&m, 0, Vec2::new(3.0, 5.0));
        let id = m.push_furnishing(pendant(0, [3.0, 5.0], f64::NAN));
        let _ = seated_ceiling_items(&m, 0);
        let _ = reseat(&mut m, &[id]);
        m.furnishings[0].size = [f64::NAN; 3];
        let _ = reseat(&mut m, &[id]);
        // A large ring stays within the sample cap.
        let big: Vec<Vec2> = (0..50_000).map(|k| Vec2::new((k as f64).cos() * 3.0 + 5.0, (k as f64).sin() * 1.0 + 2.0)).collect();
        let _ = lowest_underside_over(&house(true), 0, &big);
    }
}
