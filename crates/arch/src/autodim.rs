//! Auto exterior dimension rows: the points of the four sides of a level's exterior walls.
//!
//! Each side gets up to three rows (openings, walls, overall). The sides come from the bounding box
//! of the exterior walls (centre-line ends widened by half the thickness), so the rows do not change
//! with the winding or the order of the walls.

use cadcraft_doc::arch::{AutoRowKind, MAX_AUTO_DIMS_PER_LEVEL, OpeningKind, arch_handle, opening_handle};
use cadcraft_doc::{ArchFeature, ArchModel, Handle, Wall};
use cadcraft_geom::Vec2;

/// One point of a row: a place on an arch object that a dimension attaches to.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoPoint {
    /// The point in the plan.
    pub at: Vec2,
    /// The wall or opening it attaches to.
    pub handle: Handle,
    /// The feature of that object.
    pub feature: ArchFeature,
    /// Distance from the feature's origin along its direction.
    pub along: f64,
}

/// One row of dimensions along a side.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoRow {
    /// `south`, `east`, `north` or `west`.
    pub side: String,
    /// The outward direction of the side.
    pub normal: Vec2,
    /// The direction the row runs in.
    pub dir: Vec2,
    /// Which row of the side this is.
    pub kind: AutoRowKind,
    /// The points, sorted along `dir`.
    pub points: Vec<AutoPoint>,
    /// The signed coordinate of the dimension line along `normal`.
    pub line_offset: f64,
}

/// How far outside the building the row of this kind runs, in metres.
pub fn row_offset(kind: AutoRowKind) -> f64 {
    match kind {
        AutoRowKind::Openings => 1.0,
        AutoRowKind::Walls => 1.5,
        AutoRowKind::Overall => 2.0,
    }
}

const EPS: f64 = 1e-6;

/// A wall that can be dimensioned: finite and with a direction.
struct W<'a> {
    wall: &'a Wall,
    dir: Vec2,
    t: f64,
}

fn usable(wall: &Wall) -> Option<W<'_>> {
    if !wall.start.is_finite() || !wall.end.is_finite() || !wall.thickness.is_finite() {
        return None;
    }
    let d = wall.end - wall.start;
    let len = d.len();
    if !len.is_finite() || len <= 1e-9 {
        return None;
    }
    Some(W { wall, dir: d / len, t: wall.thickness })
}

fn cross(a: Vec2, b: Vec2) -> f64 {
    a.x * b.y - a.y * b.x
}

fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let l2 = ab.dot(ab);
    if l2 <= 0.0 {
        return p.dist(a);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    p.dist(a + ab * t)
}

fn point(model: &ArchModel, w: &W, feature: ArchFeature, at: Vec2) -> AutoPoint {
    let handle = arch_handle(w.wall.id);
    AutoPoint { at, handle, feature, along: crate::dimension::along_of(model, handle, feature, at).unwrap_or(0.0) }
}

/// An end of a wall: 0 = start, 1 = end.
fn end_of(w: &W, i: usize) -> Vec2 {
    if i == 0 { w.wall.start } else { w.wall.end }
}

/// The extreme point of a side: `max` false = the one with the smallest coordinate along `dir`.
fn extreme(model: &ArchModel, ws: &[W], dir: Vec2, normal: Vec2, bound: f64, max: bool) -> Option<AutoPoint> {
    // (not perpendicular, wall id, index into ws, end)
    let mut best: Option<(bool, u32, usize, usize)> = None;
    for (wi, w) in ws.iter().enumerate() {
        for i in 0..2 {
            let c = end_of(w, i).dot(dir) + if max { w.t / 2.0 } else { -w.t / 2.0 };
            if (c - bound).abs() > 1e-9 {
                continue;
            }
            let key = (w.dir.dot(dir).abs() > EPS, w.wall.id.0);
            let better = match best {
                None => true,
                Some((np, id, _, bi)) => {
                    if key != (np, id) {
                        key < (np, id)
                    } else {
                        // The same wall: the end nearer the side.
                        end_of(w, i).dot(normal) > end_of(w, bi).dot(normal)
                    }
                }
            };
            if better {
                best = Some((key.0, key.1, wi, i));
            }
        }
    }
    let (not_perp, _, wi, i) = best?;
    let w = ws.get(wi)?;
    let e = end_of(w, i);
    if !not_perp {
        let perp = w.dir.perp();
        let half = w.t / 2.0;
        let left = (e + perp * half).dot(dir);
        let right = (e - perp * half).dot(dir);
        let use_left = if max { left >= right } else { left <= right };
        return Some(if use_left {
            point(model, w, ArchFeature::LeftFace, e + perp * half)
        } else {
            point(model, w, ArchFeature::RightFace, e - perp * half)
        });
    }
    if cross(w.dir, dir).abs() <= EPS {
        let out = if i == 0 { -w.dir } else { w.dir };
        return Some(point(model, w, ArchFeature::Centre, e + out * (w.t / 2.0)));
    }
    Some(point(model, w, ArchFeature::Centre, e))
}

/// A point with its merge rank: extremes (0) over walls (1) over openings (2).
#[derive(Clone)]
struct Ranked {
    p: AutoPoint,
    rank: u8,
}

fn merge(mut pts: Vec<Ranked>, dir: Vec2) -> Vec<AutoPoint> {
    pts.sort_by(|a, b| a.p.at.dot(dir).total_cmp(&b.p.at.dot(dir)).then(a.rank.cmp(&b.rank)));
    let mut out: Vec<Ranked> = Vec::new();
    for r in pts {
        if let Some(last) = out.last_mut()
            && (r.p.at.dot(dir) - last.p.at.dot(dir)).abs() <= EPS
        {
            if r.rank < last.rank {
                *last = r;
            }
            continue;
        }
        out.push(r);
    }
    out.into_iter().map(|r| r.p).collect()
}

/// The rows of the exterior of `level`, in the order south, east, north, west; within a side
/// openings, walls, overall. `kinds` empty means all three.
pub fn exterior_rows(model: &ArchModel, level: u32, kinds: &[AutoRowKind]) -> Vec<AutoRow> {
    let all: Vec<W> = model.walls.iter().filter(|w| w.base_level == level).filter_map(usable).collect();
    let ext: Vec<W> = all.iter().filter(|w| w.wall.exterior).map(|w| W { wall: w.wall, dir: w.dir, t: w.t }).collect();
    if ext.is_empty() {
        return Vec::new();
    }
    let (mut x0, mut x1, mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for w in &ext {
        let h = w.t / 2.0;
        for i in 0..2 {
            let e = end_of(w, i);
            x0 = x0.min(e.x - h);
            x1 = x1.max(e.x + h);
            y0 = y0.min(e.y - h);
            y1 = y1.max(e.y + h);
        }
    }
    if ![x0, x1, y0, y1].iter().all(|v| v.is_finite()) {
        return Vec::new();
    }
    let want = |k: AutoRowKind| kinds.is_empty() || kinds.contains(&k);

    // (name, normal, dir, min along dir, max along dir, box edge along normal, opposite edge, tie wins)
    let sides = [
        ("south", Vec2::new(0.0, -1.0), Vec2::new(1.0, 0.0), x0, x1, -y0, -y1, true),
        ("east", Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0), y0, y1, x1, x0, false),
        ("north", Vec2::new(0.0, 1.0), Vec2::new(1.0, 0.0), x0, x1, y1, y0, false),
        ("west", Vec2::new(-1.0, 0.0), Vec2::new(0.0, 1.0), y0, y1, -x0, -x1, true),
    ];
    let mut rows = Vec::new();
    for (name, normal, dir, lo, hi, edge, opposite, tie_wins) in sides {
        if hi - lo <= EPS {
            continue;
        }
        let (Some(a), Some(b)) = (extreme(model, &ext, dir, normal, lo, false), extreme(model, &ext, dir, normal, hi, true)) else {
            continue;
        };
        let (c_lo, c_hi) = (a.at.dot(dir), b.at.dot(dir));
        let mut pts: Vec<Ranked> = vec![Ranked { p: a, rank: 0 }, Ranked { p: b, rank: 0 }];

        let side_walls: Vec<&W> = ext
            .iter()
            .filter(|w| w.dir.dot(normal).abs() <= EPS)
            .filter(|w| {
                let m = ((w.wall.start + w.wall.end) * 0.5).dot(normal);
                let (da, db) = ((m - edge).abs(), (m - opposite).abs());
                if tie_wins { da <= db } else { da < db }
            })
            .collect();

        // Interior walls that end on a side wall split the side.
        for iw in all.iter().filter(|w| !w.wall.exterior && !w.wall.divider && w.t > 0.0) {
            if cross(iw.dir, dir).abs() <= EPS {
                continue;
            }
            for i in 0..2 {
                let e = end_of(iw, i);
                if side_walls.iter().any(|sw| dist_to_segment(e, sw.wall.start, sw.wall.end) <= sw.t / 2.0 + EPS) {
                    pts.push(Ranked { p: point(model, iw, ArchFeature::Centre, e), rank: 1 });
                }
            }
        }

        // Jogs: ends of side walls that are not at the extremes.
        for sw in &side_walls {
            let mid = ((sw.wall.start + sw.wall.end) * 0.5).dot(dir);
            for i in 0..2 {
                let e = end_of(sw, i);
                let c = e.dot(dir);
                if !(c > c_lo + EPS && c < c_hi - EPS) {
                    continue;
                }
                let mut found = None;
                for pw in ext.iter().filter(|w| w.wall.id != sw.wall.id && w.dir.dot(dir).abs() <= EPS) {
                    let near = (0..2)
                        .filter(|&j| end_of(pw, j).dist(e) <= EPS + pw.t / 2.0)
                        .min_by(|&j, &k| end_of(pw, j).dist(e).total_cmp(&end_of(pw, k).dist(e)));
                    if let Some(j) = near {
                        found = Some((pw, end_of(pw, j)));
                        break;
                    }
                }
                let p = match found {
                    Some((pw, pe)) => {
                        let perp = pw.dir.perp();
                        let half = pw.t / 2.0;
                        let l = pe + perp * half;
                        let r = pe - perp * half;
                        if (l.dot(dir) - mid).abs() >= (r.dot(dir) - mid).abs() {
                            point(model, pw, ArchFeature::LeftFace, l)
                        } else {
                            point(model, pw, ArchFeature::RightFace, r)
                        }
                    }
                    None => point(model, sw, ArchFeature::Centre, e),
                };
                pts.push(Ranked { p, rank: 1 });
            }
        }

        // Openings of the side walls.
        let mut opening_pts: Vec<Ranked> = Vec::new();
        for o in &model.openings {
            if o.kind == OpeningKind::Niche || !o.offset.is_finite() {
                continue;
            }
            let Some(sw) = side_walls.iter().find(|w| w.wall.id == o.host) else {
                continue;
            };
            opening_pts.push(Ranked {
                p: AutoPoint { at: sw.wall.start + sw.dir * o.offset, handle: opening_handle(o.id), feature: ArchFeature::Centre, along: 0.0 },
                rank: 2,
            });
        }

        let overall = merge(pts.iter().take(2).cloned().collect(), dir);
        let walls_row = merge(pts.clone(), dir);
        let has_openings = !opening_pts.is_empty();
        pts.extend(opening_pts);
        let openings_row = merge(pts, dir);

        let mut push = |kind: AutoRowKind, mut points: Vec<AutoPoint>| {
            points.truncate(MAX_AUTO_DIMS_PER_LEVEL);
            rows.push(AutoRow { side: name.to_string(), normal, dir, kind, points, line_offset: edge + row_offset(kind) });
        };
        if has_openings && want(AutoRowKind::Openings) {
            push(AutoRowKind::Openings, openings_row);
        }
        if walls_row.len() >= 3 && want(AutoRowKind::Walls) {
            push(AutoRowKind::Walls, walls_row);
        }
        if want(AutoRowKind::Overall) {
            push(AutoRowKind::Overall, overall);
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::Opening;

    fn wall(id: u32, a: (f64, f64), b: (f64, f64), t: f64, exterior: bool) -> Wall {
        Wall { id: ArchId(id), start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness: t, exterior, base_level: 0, ..Default::default() }
    }

    /// The phase-1 house: 10 x 8, exterior 0.2, interior w4 at y = 3 and w5 at x = 6.
    fn house() -> ArchModel {
        let mut m = ArchModel::default();
        m.walls = vec![
            wall(0, (0.0, 0.0), (10.0, 0.0), 0.2, true),
            wall(1, (10.0, 0.0), (10.0, 8.0), 0.2, true),
            wall(2, (10.0, 8.0), (0.0, 8.0), 0.2, true),
            wall(3, (0.0, 8.0), (0.0, 0.0), 0.2, true),
            wall(4, (0.0, 3.0), (10.0, 3.0), 0.1, false),
            wall(5, (6.0, 3.0), (6.0, 8.0), 0.1, false),
        ];
        m.next_id = 6;
        m
    }

    fn row(rows: &[AutoRow], side: &str, kind: AutoRowKind) -> Option<AutoRow> {
        rows.iter().find(|r| r.side == side && r.kind == kind).cloned()
    }

    fn values(r: &AutoRow) -> Vec<f64> {
        r.points.windows(2).map(|p| (p[1].at - p[0].at).dot(r.dir)).collect()
    }

    fn close(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
    }

    fn window(id: u32, host: u32, offset: f64) -> Opening {
        Opening { id: ArchId(id), host: ArchId(host), offset, width: 1.0, kind: OpeningKind::Window, ..Default::default() }
    }

    #[test]
    fn phase1_house_overall_rows_are_10_2_and_8_2() {
        let rows = exterior_rows(&house(), 0, &[AutoRowKind::Overall]);
        assert_eq!(rows.iter().map(|r| r.side.as_str()).collect::<Vec<_>>(), ["south", "east", "north", "west"]);
        for r in &rows {
            let want = if r.dir.x == 1.0 { 10.2 } else { 8.2 };
            assert!(close(&values(r), &[want]), "{} {:?}", r.side, values(r));
            assert_eq!(r.points.len(), 2);
        }
        let south = &rows[0];
        assert_eq!(south.points[0].at, Vec2::new(-0.1, 0.0));
        assert_eq!(south.points[0].handle, arch_handle(ArchId(3)));
        assert_eq!(south.points[0].feature, ArchFeature::RightFace);
    }

    #[test]
    fn phase1_house_north_walls_row_splits_at_w5_centre() {
        let rows = exterior_rows(&house(), 0, &[]);
        let r = row(&rows, "north", AutoRowKind::Walls).expect("north walls row");
        assert!(close(&values(&r), &[6.1, 4.1]), "{:?}", values(&r));
        let mid = &r.points[1];
        assert_eq!(mid.at, Vec2::new(6.0, 8.0));
        assert_eq!(mid.handle, arch_handle(ArchId(5)));
        assert_eq!(mid.feature, ArchFeature::Centre);
        assert!((mid.along - 5.0).abs() < 1e-9);
    }

    #[test]
    fn phase1_house_has_no_south_walls_row_and_no_openings_rows() {
        let rows = exterior_rows(&house(), 0, &[]);
        assert!(row(&rows, "south", AutoRowKind::Walls).is_none());
        assert!(rows.iter().all(|r| r.kind != AutoRowKind::Openings));
        assert_eq!(rows.len(), 4 + 3);
    }

    #[test]
    fn east_and_west_walls_rows_read_3_1_and_5_1() {
        let rows = exterior_rows(&house(), 0, &[AutoRowKind::Walls]);
        for side in ["east", "west"] {
            let r = row(&rows, side, AutoRowKind::Walls).expect(side);
            assert!(close(&values(&r), &[3.1, 5.1]), "{side} {:?}", values(&r));
            assert_eq!(r.points[1].handle, arch_handle(ArchId(4)));
            assert_eq!(r.points[1].feature, ArchFeature::Centre);
        }
    }

    #[test]
    fn window_adds_north_openings_row_4_1_2_0_4_1() {
        let mut m = house();
        m.openings.push(window(0, 2, 6.0));
        let rows = exterior_rows(&m, 0, &[]);
        let r = row(&rows, "north", AutoRowKind::Openings).expect("openings row");
        assert!(close(&values(&r), &[4.1, 2.0, 4.1]), "{:?}", values(&r));
        assert_eq!(r.points[1].at, Vec2::new(4.0, 8.0));
        assert_eq!(r.points[1].handle, opening_handle(ArchId(0)));
        assert_eq!(r.points[2].handle, arch_handle(ArchId(5)));
        assert!(row(&rows, "south", AutoRowKind::Openings).is_none());
        let mut niche = house();
        niche.openings.push(Opening { kind: OpeningKind::Niche, ..window(0, 2, 6.0) });
        assert!(row(&exterior_rows(&niche, 0, &[]), "north", AutoRowKind::Openings).is_none());
    }

    #[test]
    fn interior_wall_at_x4_adds_south_walls_row_4_1_6_1() {
        let mut m = house();
        m.walls.push(wall(6, (4.0, 0.0), (4.0, 3.0), 0.1, false));
        let rows = exterior_rows(&m, 0, &[]);
        let r = row(&rows, "south", AutoRowKind::Walls).expect("south walls row");
        assert!(close(&values(&r), &[4.1, 6.1]), "{:?}", values(&r));
        assert_eq!(r.points[1].handle, arch_handle(ArchId(6)));
        assert_eq!(r.points[1].at, Vec2::new(4.0, 0.0));
        assert_eq!(r.points[1].along, 0.0);
    }

    #[test]
    fn extremes_attach_to_outer_faces() {
        let m = house();
        let rows = exterior_rows(&m, 0, &[AutoRowKind::Overall]);
        let want = [
            ("south", Vec2::new(-0.1, 0.0), Vec2::new(10.1, 0.0)),
            ("east", Vec2::new(10.0, -0.1), Vec2::new(10.0, 8.1)),
            ("north", Vec2::new(-0.1, 8.0), Vec2::new(10.1, 8.0)),
            ("west", Vec2::new(0.0, -0.1), Vec2::new(0.0, 8.1)),
        ];
        for (side, lo, hi) in want {
            let r = row(&rows, side, AutoRowKind::Overall).expect(side);
            assert!(r.points[0].at.dist(lo) < 1e-9 && r.points[1].at.dist(hi) < 1e-9, "{side}: {:?}", r.points);
            for p in &r.points {
                assert!(matches!(p.feature, ArchFeature::LeftFace | ArchFeature::RightFace), "{side}");
                // The attachment reproduces the point.
                let g = crate::dimension::feature_geom(&m, p.handle, p.feature).expect("feature");
                if let crate::dimension::FeatureGeom::Line { origin, dir } = g {
                    assert!((origin + dir * p.along).dist(p.at) < 1e-9, "{side}");
                }
            }
        }
        // A wall parallel to the side that sticks out: the point is the end pushed half a thickness.
        let mut ps = ArchModel::default();
        ps.walls = vec![wall(0, (0.0, 0.0), (10.0, 0.0), 0.2, true), wall(1, (0.0, 5.0), (10.0, 5.0), 0.2, true)];
        let rows = exterior_rows(&ps, 0, &[AutoRowKind::Overall]);
        let s = row(&rows, "south", AutoRowKind::Overall).expect("south");
        assert_eq!(s.points[0].feature, ArchFeature::Centre);
        assert!(s.points[0].at.dist(Vec2::new(-0.1, 0.0)) < 1e-9);
        assert!((s.points[0].along + 0.1).abs() < 1e-9);
        assert!((s.points[1].along - 10.1).abs() < 1e-9);
    }

    #[test]
    fn row_lines_are_fixed_per_kind() {
        let mut m = house();
        m.openings.push(window(0, 2, 6.0));
        let rows = exterior_rows(&m, 0, &[]);
        let line = |s: &str, k| row(&rows, s, k).expect(s).line_offset;
        assert!((line("south", AutoRowKind::Overall) - 2.1).abs() < 1e-9);
        assert!((line("north", AutoRowKind::Openings) - 9.1).abs() < 1e-9);
        assert!((line("north", AutoRowKind::Walls) - 9.6).abs() < 1e-9);
        assert!((line("east", AutoRowKind::Overall) - 12.1).abs() < 1e-9);
        assert!((line("east", AutoRowKind::Walls) - 11.6).abs() < 1e-9);
        assert!((line("west", AutoRowKind::Overall) - 2.1).abs() < 1e-9);
        // South walls (with a wall at x = 4): the line y = -1.6.
        m.walls.push(wall(6, (4.0, 0.0), (4.0, 3.0), 0.1, false));
        let rows = exterior_rows(&m, 0, &[]);
        let s = row(&rows, "south", AutoRowKind::Walls).expect("south walls");
        assert!((s.line_offset - 1.6).abs() < 1e-9);
        assert!((s.normal.y + 1.0).abs() < 1e-12);
        // Asking for one kind gives only that kind.
        let only = exterior_rows(&m, 0, &[AutoRowKind::Overall]);
        assert!(only.iter().all(|r| r.kind == AutoRowKind::Overall));
    }

    #[test]
    fn hostile_models_give_no_rows() {
        assert!(exterior_rows(&ArchModel::default(), 0, &[]).is_empty());
        let mut m = house();
        assert!(exterior_rows(&m, 7, &[]).is_empty());
        for w in m.walls.iter_mut() {
            w.exterior = false;
        }
        assert!(exterior_rows(&m, 0, &[]).is_empty());
        let mut m = ArchModel::default();
        m.walls = vec![wall(0, (f64::NAN, 0.0), (1.0, 0.0), 0.2, true), wall(1, (0.0, 0.0), (0.0, 0.0), 0.2, true)];
        assert!(exterior_rows(&m, 0, &[]).is_empty());
        let mut m = house();
        m.walls[0].thickness = f64::NAN;
        m.walls[1].end = Vec2::new(f64::INFINITY, 0.0);
        let _ = exterior_rows(&m, 0, &[]);
    }
}
