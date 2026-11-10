//! What lies over a plan point (phase 16): the floor of the level above, or an Open Below room
//! that opens it. A landing's walls and ceilings stop at that floor; an Open Below room lets
//! them rise to the upper room's own ceiling.

use cadcraft_doc::arch::{ArchId, ArchModel, MAX_ROOMS, Room, RoomType, Wall};
use cadcraft_geom::{Vec2, point_in_polygon};

/// Two elevations closer than this are the same level.
const SAME_LEVEL: f64 = 1e-9;
/// A wall top clip must be this far above the wall's base to count.
const MIN_CLIP_ABOVE_BASE: f64 = 0.01;
/// How far beyond the wall's face the clip samples sit.
const CLIP_SAMPLE_OUT: f64 = 0.05;

/// A room on a level above, with the numbers the overhead queries need.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AboveRoom {
    /// The room.
    pub room: ArchId,
    /// Its level index.
    pub level: u32,
    /// Its level's elevation.
    pub elevation: f64,
    /// The underside of its floor: elevation - structure.
    pub underside: f64,
    /// Its outline.
    pub polygon: Vec<Vec2>,
    /// Its holes.
    pub holes: Vec<Vec<Vec2>>,
    /// Whether it is an Open Below room.
    pub open: bool,
}

/// What lies over a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Overhead {
    /// Nothing: no room above.
    Clear,
    /// The floor of a room above.
    Floor {
        /// The level of that floor.
        level: u32,
        /// Its top.
        elevation: f64,
        /// Its underside.
        underside: f64,
    },
    /// An Open Below room above.
    Open {
        /// The Open Below room.
        room: ArchId,
        /// Its level.
        level: u32,
    },
}

fn holds(polygon: &[Vec2], holes: &[Vec<Vec2>], p: Vec2) -> bool {
    polygon.len() >= 3 && point_in_polygon(polygon, p) && !holes.iter().any(|h| h.len() >= 3 && point_in_polygon(h, p))
}

fn bbox_meets(polygon: &[Vec2], bbox: [f64; 4]) -> bool {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for v in polygon {
        if !v.is_finite() {
            return false;
        }
        lo = [lo[0].min(v.x), lo[1].min(v.y)];
        hi = [hi[0].max(v.x), hi[1].max(v.y)];
    }
    !polygon.is_empty() && lo[0] <= bbox[2] && hi[0] >= bbox[0] && lo[1] <= bbox[3] && hi[1] >= bbox[1]
}

/// The rooms of every level above `level` whose polygon's bounding box meets `bbox`
/// (`[x0, y0, x1, y1]`), sorted by level elevation then model order; empty when `level` is
/// unknown or nothing is above it; at most `MAX_ROOMS`.
pub fn rooms_above(model: &ArchModel, level: u32, bbox: [f64; 4]) -> Vec<AboveRoom> {
    let Some(base) = model.level(level).map(|l| l.elevation).filter(|e| e.is_finite()) else { return Vec::new() };
    if !bbox.iter().all(|v| v.is_finite()) {
        return Vec::new();
    }
    let mut found: Vec<(f64, usize, AboveRoom)> = Vec::new();
    for (k, r) in model.rooms.iter().enumerate() {
        let Some(l) = model.level(r.level) else { continue };
        if !l.elevation.is_finite() || l.elevation <= base + SAME_LEVEL || !bbox_meets(&r.polygon, bbox) {
            continue;
        }
        found.push((
            l.elevation,
            k,
            AboveRoom {
                room: r.id,
                level: r.level,
                elevation: l.elevation,
                underside: l.elevation - l.structure,
                polygon: r.polygon.clone(),
                holes: r.holes.clone(),
                open: r.room_type == RoomType::OpenBelow,
            },
        ));
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    found.into_iter().take(MAX_ROOMS).map(|f| f.2).collect()
}

/// What lies over `p`: the first of `above` (in its order) holding `p` decides.
pub fn overhead_in(above: &[AboveRoom], p: Vec2) -> Overhead {
    if !p.is_finite() {
        return Overhead::Clear;
    }
    match above.iter().find(|a| holds(&a.polygon, &a.holes, p)) {
        None => Overhead::Clear,
        Some(a) if a.open => Overhead::Open { room: a.room, level: a.level },
        Some(a) => Overhead::Floor { level: a.level, elevation: a.elevation, underside: a.underside },
    }
}

/// What lies over plan point `p` on `level`.
pub fn overhead_at(model: &ArchModel, level: u32, p: Vec2) -> Overhead {
    if !p.is_finite() {
        return Overhead::Clear;
    }
    overhead_in(&rooms_above(model, level, [p.x, p.y, p.x, p.y]), p)
}

/// The room with id `id`.
pub(super) fn room_by_id(model: &ArchModel, id: ArchId) -> Option<&Room> {
    model.rooms.iter().find(|r| r.id == id)
}

/// The elevation a wall's top is clipped to by the floor above it, or `None` for no clip: not for
/// a divider, retaining or spanning wall, nor one shorter than 1e-6; not when an Open Below room
/// lies over either face; else the lowest floor top over the two face samples, when above the
/// wall's base by more than 0.01.
pub fn wall_top_clip(model: &ArchModel, wall: &Wall) -> Option<f64> {
    if wall.divider || wall.retaining || wall.top_level.is_some() {
        return None;
    }
    let d = wall.end - wall.start;
    let len = d.len();
    if !(wall.start.is_finite() && wall.end.is_finite() && len.is_finite() && len >= 1e-6 && wall.thickness.is_finite()) {
        return None;
    }
    let mid = Vec2::new((wall.start.x + wall.end.x) * 0.5, (wall.start.y + wall.end.y) * 0.5);
    let n = Vec2::new(-d.y / len, d.x / len);
    let off = wall.thickness / 2.0 + CLIP_SAMPLE_OUT;
    let a = Vec2::new(mid.x + n.x * off, mid.y + n.y * off);
    let b = Vec2::new(mid.x - n.x * off, mid.y - n.y * off);
    let bbox = [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)];
    let above = rooms_above(model, wall.base_level, bbox);
    let (oa, ob) = (overhead_in(&above, a), overhead_in(&above, b));
    if matches!(oa, Overhead::Open { .. }) || matches!(ob, Overhead::Open { .. }) {
        return None;
    }
    let base = wall.bottom_z(&model.levels);
    [oa, ob]
        .iter()
        .filter_map(|o| match o {
            Overhead::Floor { elevation, .. } => Some(*elevation),
            _ => None,
        })
        .fold(None, |best: Option<f64>, e| Some(best.map_or(e, |b| b.min(e))))
        .filter(|e| e.is_finite() && *e > base + MIN_CLIP_ABOVE_BASE)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ceiling::{finished_ceiling_in, rough_ceiling_in};
    use cadcraft_doc::arch::{Level, LevelKind};

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Lower 0, Entry landing 1.35, Main 2.7, structure 0.3; the landing room under Main's room.
    fn split(open: bool) -> ArchModel {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, name: "Lower".into(), ..Level::default() });
        m.levels.push(Level { index: 1, name: "Entry".into(), elevation: 1.35, kind: LevelKind::Landing, ..Level::default() });
        m.levels.push(Level { index: 2, name: "Main".into(), elevation: 2.7, ..Level::default() });
        m.push_room(Room { polygon: rect(0.1, 0.1, 5.95, 2.95), level: 0, ..Room::default() });
        m.push_room(Room { polygon: rect(3.05, 0.1, 5.95, 2.95), level: 1, ..Room::default() });
        if open {
            m.push_room(Room { polygon: rect(3.05, 0.1, 5.95, 2.95), level: 2, room_type: RoomType::OpenBelow, ..Room::default() });
        }
        m.push_room(Room { polygon: rect(0.1, 0.1, 9.9, 7.9), level: 2, ..Room::default() });
        m
    }

    fn landing_wall() -> Wall {
        let mut w = Wall::new(Vec2::new(3.0, 0.0), Vec2::new(3.0, 3.0), 0.2);
        w.base_level = 1;
        w.height = 2.4;
        w
    }

    #[test]
    fn overhead_picks_the_lowest_level_above_by_elevation() {
        let mut m = split(false);
        // Model order puts Main before a level between: elevation order still wins.
        m.levels.push(Level { index: 3, name: "Top".into(), elevation: 5.4, ..Level::default() });
        m.push_room(Room { polygon: rect(0.0, 0.0, 10.0, 8.0), level: 3, ..Room::default() });
        let p = Vec2::new(4.5, 1.5);
        match overhead_at(&m, 0, p) {
            Overhead::Floor { level, elevation, underside } => {
                assert_eq!(level, 1);
                assert!(near(elevation, 1.35) && near(underside, 1.05));
            }
            o => panic!("{o:?}"),
        }
        assert!(matches!(overhead_at(&m, 1, p), Overhead::Floor { level: 2, .. }));
        assert_eq!(overhead_at(&m, 3, p), Overhead::Clear);
        assert_eq!(overhead_at(&m, 9, p), Overhead::Clear);
        assert_eq!(overhead_at(&m, 0, Vec2::new(f64::NAN, 0.0)), Overhead::Clear);
        assert!(rooms_above(&m, 9, [0.0, 0.0, 1.0, 1.0]).is_empty());
        let above = rooms_above(&m, 0, [4.0, 1.0, 5.0, 2.0]);
        assert_eq!(above.iter().map(|a| a.level).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(overhead_in(&above, Vec2::new(1.0, 1.0)), overhead_at(&m, 0, Vec2::new(1.0, 1.0)));
    }

    #[test]
    fn rough_ceiling_is_clipped_by_the_floor_above() {
        let m = split(false);
        let landing = m.rooms[1].clone();
        let lower = m.rooms[0].clone();
        assert!(near(rough_ceiling_in(&m, &landing, Vec2::new(4.5, 1.5)).unwrap(), 2.4));
        assert!(near(rough_ceiling_in(&m, &lower, Vec2::new(4.5, 1.5)).unwrap(), 1.05));
        assert!(near(rough_ceiling_in(&m, &lower, Vec2::new(1.0, 1.5)).unwrap(), 2.4));
    }

    #[test]
    fn an_open_below_room_lifts_the_ceiling_to_its_own() {
        let m = split(true);
        let landing = m.rooms[1].clone();
        let p = Vec2::new(4.5, 1.5);
        assert!(matches!(overhead_at(&m, 1, p), Overhead::Open { level: 2, .. }));
        assert!(near(rough_ceiling_in(&m, &landing, p).unwrap(), 5.1));
        assert!(near(finished_ceiling_in(&m, &landing, p).unwrap(), 5.1));
        assert_eq!(wall_top_clip(&m, &landing_wall()), None);
    }

    #[test]
    fn wall_top_clip_stops_landing_walls_at_the_floor_above() {
        let m = split(false);
        assert!(near(wall_top_clip(&m, &landing_wall()).unwrap(), 2.7));
        let mut w = landing_wall();
        w.divider = true;
        assert_eq!(wall_top_clip(&m, &w), None);
        let mut w = landing_wall();
        w.retaining = true;
        assert_eq!(wall_top_clip(&m, &w), None);
        let mut w = landing_wall();
        w.top_level = Some(2);
        assert_eq!(wall_top_clip(&m, &w), None);
        let mut w = landing_wall();
        w.end = Vec2::new(3.0, 1e-7);
        assert_eq!(wall_top_clip(&m, &w), None);
        let mut w = landing_wall();
        w.thickness = f64::NAN;
        assert_eq!(wall_top_clip(&m, &w), None);
        // Outside the building: nothing above.
        let mut far = Wall::new(Vec2::new(30.0, 0.0), Vec2::new(30.0, 3.0), 0.2);
        far.base_level = 1;
        assert_eq!(wall_top_clip(&m, &far), None);
    }

    #[test]
    fn stacked_levels_keep_their_ceilings_and_walls() {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, ..Level::default() });
        m.levels.push(Level { index: 1, elevation: 2.7, ..Level::default() });
        m.push_room(Room { polygon: rect(0.1, 0.1, 9.9, 7.9), level: 0, ..Room::default() });
        m.push_room(Room { polygon: rect(0.1, 0.1, 9.9, 7.9), level: 1, ..Room::default() });
        let r = m.rooms[0].clone();
        assert!(near(rough_ceiling_in(&m, &r, Vec2::new(5.0, 4.0)).unwrap(), 2.4));
        // A wall as tall as its level reaches the floor above only at its top: no clip below it.
        let mut w = Wall::new(Vec2::new(0.0, 4.0), Vec2::new(10.0, 4.0), 0.2);
        w.height = 2.4;
        assert!(near(wall_top_clip(&m, &w).unwrap(), 2.7));
        assert!(wall_top_clip(&m, &w).unwrap() >= w.top_z(&m.levels));
    }
}
