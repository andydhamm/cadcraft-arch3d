//! Grade classes: whether a level sits above the finished grade or is a basement, and the
//! above-grade floor area (phase 15).

use cadcraft_doc::arch::LevelKind;
use cadcraft_geom::Vec2;

use crate::area::interior_area;
use crate::terrain::surface::SiteSurface;
use crate::{ArchModel, Level, RoomType};

/// Distance sampled outside the exterior face of a wall, in metres.
const SAMPLE_OFFSET: f64 = 0.3;
/// Spacing of the samples along a wall, in metres.
const SAMPLE_STEP: f64 = 0.5;
/// Most samples taken along one wall (bounds hostile lengths).
const MAX_SAMPLES: usize = 4096;
/// Tolerance when comparing a level elevation to the grade.
const EPS: f64 = 1e-6;

/// Whether a level is above grade or a basement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum GradeClass {
    /// The floor is at or above the finished grade all round.
    #[default]
    AboveGrade,
    /// The floor is below the finished grade somewhere along its exterior.
    Basement,
}

impl GradeClass {
    /// The serialized key: `"aboveGrade"` or `"basement"`.
    pub fn key(self) -> &'static str {
        match self {
            GradeClass::AboveGrade => "aboveGrade",
            GradeClass::Basement => "basement",
        }
    }

    /// Parse a key written by [`GradeClass::key`].
    pub fn from_key(key: &str) -> Option<GradeClass> {
        match key {
            "aboveGrade" => Some(GradeClass::AboveGrade),
            "basement" => Some(GradeClass::Basement),
            _ => None,
        }
    }
}

fn grade_with(surface: Option<&SiteSurface>, p: Vec2) -> f64 {
    surface.and_then(|s| s.modified_at(p)).filter(|z| z.is_finite()).unwrap_or(0.0)
}

/// The terrain's finished surface at `p` (modifiers and pads applied), or 0 with no terrain
/// (and outside its boundary).
pub fn finished_grade(model: &ArchModel, p: Vec2) -> f64 {
    let surface = SiteSurface::new(model).ok();
    grade_with(surface.as_ref(), p)
}

/// Classify `level`: sample the grade 0.3 m outside every exterior wall that spans it, every
/// 0.5 m and at both ends; any sample above the level's floor makes it a basement.
pub fn grade_class(model: &ArchModel, level: &Level) -> GradeClass {
    let surface = SiteSurface::new(model).ok();
    class_with(model, level, surface.as_ref())
}

fn class_with(model: &ArchModel, level: &Level, surface: Option<&SiteSurface>) -> GradeClass {
    if !level.elevation.is_finite() {
        return GradeClass::Basement;
    }
    let walls: Vec<_> =
        model.walls.iter().filter(|w| w.exterior && w.start.is_finite() && w.end.is_finite() && w.spans_level(level, &model.levels)).collect();
    let at_or_above = |p: Vec2| level.elevation >= grade_with(surface, p) - EPS;
    if walls.is_empty() {
        let mut ok = true;
        for r in model.rooms.iter().filter(|r| r.level == level.index && !r.polygon.is_empty()) {
            let sum = r.polygon.iter().fold(Vec2::ZERO, |a, p| a + *p);
            ok &= at_or_above(sum / r.polygon.len() as f64);
        }
        return if ok { GradeClass::AboveGrade } else { GradeClass::Basement };
    }
    // The outside is the side facing away from the middle of the exterior walls.
    let mid = walls.iter().fold(Vec2::ZERO, |a, w| a + (w.start + w.end) / 2.0) / walls.len() as f64;
    for w in walls {
        let d = w.end - w.start;
        let len = d.len();
        if !len.is_finite() {
            return GradeClass::Basement;
        }
        let along = if len > 0.0 { d / len } else { Vec2::new(1.0, 0.0) };
        let mut n = Vec2::new(along.y, -along.x);
        if ((w.start + w.end) / 2.0 - mid).dot(n) < 0.0 {
            n = Vec2::new(-n.x, -n.y);
        }
        let off = n * (w.thickness.max(0.0) / 2.0 + SAMPLE_OFFSET);
        let count = ((len / SAMPLE_STEP).floor() as usize).min(MAX_SAMPLES);
        for i in 0..=count {
            let p = w.start + along * (i as f64 * SAMPLE_STEP) + off;
            if !at_or_above(p) {
                return GradeClass::Basement;
            }
        }
        if !at_or_above(w.end + off) {
            return GradeClass::Basement;
        }
    }
    GradeClass::AboveGrade
}

fn level_area_with(model: &ArchModel, level: &Level, surface: Option<&SiteSurface>) -> f64 {
    if level.kind != LevelKind::Floor || class_with(model, level, surface) != GradeClass::AboveGrade {
        return 0.0;
    }
    model.rooms.iter().filter(|r| r.level == level.index && counts_as_living(r.room_type)).map(interior_area).sum()
}

/// Whether a room of this type counts toward above-grade living area: open-below
/// voids, garages, porches and decks do not.
fn counts_as_living(t: RoomType) -> bool {
    !matches!(t, RoomType::OpenBelow | RoomType::Garage | RoomType::Porch | RoomType::Deck)
}

/// Living interior room area (leaving out open-below, garage, porch and deck rooms) on one
/// level when it is a floor classed above grade, else 0.
pub fn level_above_grade_area(model: &ArchModel, level: &Level) -> f64 {
    let surface = SiteSurface::new(model).ok();
    level_area_with(model, level, surface.as_ref())
}

/// The sum of the rooms' living interior area (leaving out open-below, garage, porch and
/// deck rooms) on levels of kind floor classed above grade.
pub fn above_grade_area(model: &ArchModel) -> f64 {
    let surface = SiteSurface::new(model).ok();
    model.levels.iter().map(|l| level_area_with(model, l, surface.as_ref())).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Room, Wall};
    use cadcraft_geom::Vec3;

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2::new(x, y)
    }

    fn house(elev: f64, kind: LevelKind) -> ArchModel {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, elevation: elev, kind, ..Default::default() }];
        let c = [v(0.0, 0.0), v(10.0, 0.0), v(10.0, 8.0), v(0.0, 8.0)];
        for i in 0..4 {
            let mut w = Wall::new(c[i], c[(i + 1) % 4], 0.2);
            w.exterior = true;
            m.push_wall(w);
        }
        m.push_room(Room { level: 0, polygon: c.to_vec(), ..Default::default() });
        m
    }

    fn slope(m: &mut ArchModel, z_west: f64, z_east: f64) {
        let mut t = cadcraft_doc::arch::Terrain::default();
        t.boundary = vec![v(-50.0, -50.0), v(50.0, -50.0), v(50.0, 50.0), v(-50.0, 50.0)];
        t.pad = false;
        t.points =
            vec![Vec3::new(-50.0, -50.0, z_west), Vec3::new(-50.0, 50.0, z_west), Vec3::new(50.0, -50.0, z_east), Vec3::new(50.0, 50.0, z_east)];
        m.terrain = Some(t);
    }

    #[test]
    fn no_terrain_means_grade_zero() {
        let m = house(0.0, LevelKind::Floor);
        assert_eq!(finished_grade(&m, v(3.0, 3.0)), 0.0);
        assert_eq!(grade_class(&m, &m.levels[0]), GradeClass::AboveGrade);
        assert!((above_grade_area(&m) - interior_area(&m.rooms[0])).abs() < 1e-9);
    }

    #[test]
    fn a_level_below_the_slope_is_a_basement() {
        let mut m = house(0.0, LevelKind::Floor);
        slope(&mut m, 0.0, 10.0);
        assert!(finished_grade(&m, v(10.3, 4.0)) > 0.5);
        assert_eq!(grade_class(&m, &m.levels[0]), GradeClass::Basement);
        assert_eq!(above_grade_area(&m), 0.0);
    }

    #[test]
    fn a_floor_exactly_at_grade_is_above_grade() {
        let mut m = house(0.0, LevelKind::Floor);
        slope(&mut m, 0.0, 0.0);
        assert_eq!(grade_class(&m, &m.levels[0]), GradeClass::AboveGrade);
    }

    #[test]
    fn landings_add_no_above_grade_area() {
        let m = house(0.0, LevelKind::Landing);
        assert_eq!(grade_class(&m, &m.levels[0]), GradeClass::AboveGrade);
        assert_eq!(above_grade_area(&m), 0.0);
        assert_eq!(level_above_grade_area(&m, &m.levels[0]), 0.0);
    }

    #[test]
    fn above_grade_area_leaves_out_open_below_garage_porch_and_deck() {
        let mut m = house(0.0, LevelKind::Floor);
        let full = interior_area(&m.rooms[0]);
        assert!((above_grade_area(&m) - full).abs() < 1e-9);
        for t in [RoomType::OpenBelow, RoomType::Garage, RoomType::Porch, RoomType::Deck] {
            m.rooms[0].room_type = t;
            assert_eq!(above_grade_area(&m), 0.0);
            assert_eq!(level_above_grade_area(&m, &m.levels[0]), 0.0);
        }
        m.rooms[0].room_type = RoomType::Kitchen;
        assert!((level_above_grade_area(&m, &m.levels[0]) - full).abs() < 1e-9);
    }

    #[test]
    fn multiple_levels_sum_only_above_grade() {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, elevation: -1.0, kind: LevelKind::Floor, ..Default::default() });
        m.levels.push(Level { index: 1, elevation: 1.0, kind: LevelKind::Floor, ..Default::default() });

        let mut t = cadcraft_doc::arch::Terrain::default();
        t.boundary = vec![v(-50.0, -50.0), v(50.0, -50.0), v(50.0, 50.0), v(-50.0, 50.0)];
        t.pad = false;
        t.points = vec![Vec3::new(-50.0, -50.0, 0.5), Vec3::new(-50.0, 50.0, 0.5), Vec3::new(50.0, -50.0, 0.5), Vec3::new(50.0, 50.0, 0.5)];
        m.terrain = Some(t);

        let c = [v(0.0, 0.0), v(10.0, 0.0), v(10.0, 8.0), v(0.0, 8.0)];
        for i in 0..4 {
            let mut w = Wall::new(c[i], c[(i + 1) % 4], 0.2);
            w.exterior = true;
            m.push_wall(w);
        }
        m.push_room(Room { level: 0, polygon: c.to_vec(), ..Default::default() });
        m.push_room(Room { level: 1, polygon: c.to_vec(), ..Default::default() });

        let room_area = interior_area(&m.rooms[0]);
        assert_eq!(grade_class(&m, &m.levels[0]), GradeClass::Basement);
        assert_eq!(grade_class(&m, &m.levels[1]), GradeClass::AboveGrade);
        assert_eq!(level_above_grade_area(&m, &m.levels[0]), 0.0);
        assert!((level_above_grade_area(&m, &m.levels[1]) - room_area).abs() < 1e-9);
        assert!((above_grade_area(&m) - room_area).abs() < 1e-9);
    }

    #[test]
    fn nan_elevation_is_basement() {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, elevation: f64::NAN, kind: LevelKind::Floor, ..Default::default() });
        assert_eq!(grade_class(&m, &m.levels[0]), GradeClass::Basement);
        assert_eq!(above_grade_area(&m), 0.0);
    }

    #[test]
    fn empty_model_has_zero_above_grade_area() {
        let m = ArchModel::default();
        assert_eq!(above_grade_area(&m), 0.0);
    }

    #[test]
    fn rooms_on_other_levels_are_not_counted() {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, elevation: 0.0, kind: LevelKind::Floor, ..Default::default() });
        m.levels.push(Level { index: 1, elevation: 3.0, kind: LevelKind::Floor, ..Default::default() });

        let c = [v(0.0, 0.0), v(10.0, 0.0), v(10.0, 8.0), v(0.0, 8.0)];
        for i in 0..4 {
            let mut w = Wall::new(c[i], c[(i + 1) % 4], 0.2);
            w.exterior = true;
            m.push_wall(w);
        }
        m.push_room(Room { level: 0, polygon: c.to_vec(), ..Default::default() });
        let c2 = [v(0.0, 0.0), v(5.0, 0.0), v(5.0, 5.0), v(0.0, 5.0)];
        m.push_room(Room { level: 1, polygon: c2.to_vec(), ..Default::default() });

        let area0 = interior_area(&m.rooms[0]);
        let area1 = interior_area(&m.rooms[1]);
        assert!((level_above_grade_area(&m, &m.levels[0]) - area0).abs() < 1e-9);
        assert!((level_above_grade_area(&m, &m.levels[1]) - area1).abs() < 1e-9);
        assert!((above_grade_area(&m) - area0 - area1).abs() < 1e-9);
    }

    #[test]
    fn all_living_room_types_count() {
        let types = [
            RoomType::Living,
            RoomType::Kitchen,
            RoomType::Bedroom,
            RoomType::Dining,
            RoomType::Family,
            RoomType::Entry,
            RoomType::Laundry,
            RoomType::Other,
        ];
        for t in types {
            let mut m = house(0.0, LevelKind::Floor);
            m.rooms[0].room_type = t;
            let full = interior_area(&m.rooms[0]);
            assert!((above_grade_area(&m) - full).abs() < 1e-9, "room type {:?} should count", t);
        }
    }
}
