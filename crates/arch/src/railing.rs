//! Railing rules: handrail/guard heights, baluster spacing and path helpers (metres).
//!
//! `stair.rs` is built by a sibling segment, so the code-limit constants are mirrored here
//! with identical values until it lands; swap to `crate::stair::*` then.

use crate::deck::{DECK_BOARD, DECK_JOIST_DEPTH};
use crate::stair_geom::FlightLine;
use crate::{ArchId, ArchModel};
use cadcraft_doc::Railing;
use cadcraft_doc::arch::RailingHost;
use cadcraft_geom::Vec2;

/// Lowest stair handrail height (864 mm).
pub const MIN_HANDRAIL: f64 = 0.864;
/// Highest stair handrail height (965 mm).
pub const MAX_HANDRAIL: f64 = 0.965;
/// Lowest guard height (914 mm).
pub const GUARD_HEIGHT: f64 = 0.914;
/// Largest clear gap between balusters (101.6 mm, the 4-inch sphere rule).
pub const MAX_BALUSTER_GAP: f64 = 0.1016;
/// Highest accepted guard height.
const MAX_GUARD: f64 = 1.2;
/// Largest accepted path size.
const MAX_POINTS: usize = 512;
/// Cap on balusters per railing run.
const MAX_BALUSTERS: usize = 100_000;

/// `stair=true` -> handrail clamped into `MIN_HANDRAIL..MAX_HANDRAIL`; `stair=false` -> guard, must be
/// `GUARD_HEIGHT..=1.2`. `None` outside the band (never silently lowered).
pub fn validate_height(h: f64, stair: bool) -> Option<f64> {
    if !h.is_finite() {
        return None;
    }
    if stair {
        Some(h.clamp(MIN_HANDRAIL, MAX_HANDRAIL))
    } else if (GUARD_HEIGHT..=MAX_GUARD).contains(&h) {
        Some(h)
    } else {
        None
    }
}

/// Evenly spaced balusters with clear gap < `MAX_BALUSTER_GAP` -> `(gap, count)`; `None` if degenerate.
pub fn baluster_spacing(run: f64, baluster_width: f64) -> Option<(f64, usize)> {
    if !run.is_finite() || !baluster_width.is_finite() || run <= 0.0 || baluster_width <= 0.0 {
        return None;
    }
    // n balusters leave n+1 gaps: n*w + (n+1)*g = run. Need g < MAX, so n > (run - MAX)/(w + MAX).
    let min_n = ((run - MAX_BALUSTER_GAP) / (baluster_width + MAX_BALUSTER_GAP)).max(0.0);
    if !min_n.is_finite() || min_n >= MAX_BALUSTERS as f64 {
        return None;
    }
    let mut n = min_n.floor() as usize + 1;
    let gap = |n: usize| (run - n as f64 * baluster_width) / (n as f64 + 1.0);
    while gap(n) >= MAX_BALUSTER_GAP && n < MAX_BALUSTERS {
        n += 1;
    }
    let g = gap(n);
    (g > 0.0 && g < MAX_BALUSTER_GAP).then_some((g, n))
}

fn path_ok(path: &[Vec2]) -> bool {
    path.len() <= MAX_POINTS && path.iter().all(|p| p.x.is_finite() && p.y.is_finite())
}

/// Total polyline length; 0 for a hostile (non-finite or over-long) path.
pub fn path_length(path: &[Vec2]) -> f64 {
    if !path_ok(path) {
        return 0.0;
    }
    let l: f64 = path.windows(2).map(|w| (w[1] - w[0]).len()).sum();
    if l.is_finite() { l } else { 0.0 }
}

/// Offset a polyline sideways by `d` (left of travel when positive; railings are centred on their
/// path). Interior vertices move along the mitre; empty for hostile input.
pub fn offset_path(path: &[Vec2], d: f64) -> Vec<Vec2> {
    if !d.is_finite() || !path_ok(path) {
        return Vec::new();
    }
    let normal = |a: Vec2, b: Vec2| {
        let v = b - a;
        let l = v.len();
        if l > 1e-12 { Some(Vec2::new(-v.y / l, v.x / l)) } else { None }
    };
    // Drop repeated points so every segment has a direction.
    let mut pts: Vec<Vec2> = Vec::with_capacity(path.len());
    for p in path {
        if pts.last().is_none_or(|q| (*p - *q).len() > 1e-12) {
            pts.push(*p);
        }
    }
    let mut out = Vec::with_capacity(pts.len());
    for (i, p) in pts.iter().enumerate() {
        let before = i.checked_sub(1).and_then(|j| pts.get(j)).and_then(|q| normal(*q, *p));
        let after = pts.get(i + 1).and_then(|q| normal(*p, *q));
        let n = match (before, after) {
            (Some(a), Some(b)) => {
                let s = a + b;
                let k = 1.0 + a.x * b.x + a.y * b.y; // 2cos²(θ/2)
                if k > 1e-6 { Vec2::new(s.x / k, s.y / k) } else { a }
            }
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => continue,
        };
        out.push(Vec2::new(p.x + n.x * d, p.y + n.y * d));
    }
    if out.iter().all(|p| p.x.is_finite() && p.y.is_finite()) { out } else { Vec::new() }
}

/// Cap on posts per railing.
pub const MAX_POSTS: usize = 10_000;

/// The path a railing is built along: `path` moved `inset` metres to the left of travel. A closed
/// path (first point equals the last, at least 4 points) is offset as a ring so the closing corner is
/// mitred too and the first and last built points are equal. `inset` 0 returns the path; empty on
/// hostile input.
pub fn built_path(path: &[Vec2], inset: f64) -> Vec<Vec2> {
    if !inset.is_finite() || !path_ok(path) {
        return Vec::new();
    }
    if inset == 0.0 {
        return path.to_vec();
    }
    let closed = path.len() >= 4 && path.first().zip(path.last()).is_some_and(|(a, b)| (*a - *b).len() <= 1e-9);
    if !closed {
        return offset_path(path, inset);
    }
    let mut ring: Vec<Vec2> = Vec::with_capacity(path.len());
    for p in path.iter().take(path.len() - 1) {
        if ring.last().is_none_or(|q| (*p - *q).len() > 1e-12) {
            ring.push(*p);
        }
    }
    let n = ring.len();
    if n < 3 {
        return offset_path(path, inset);
    }
    let normal = |a: Vec2, b: Vec2| {
        let v = b - a;
        let l = v.len();
        if l > 1e-12 { Some(Vec2::new(-v.y / l, v.x / l)) } else { None }
    };
    let mut out = Vec::with_capacity(n + 1);
    for i in 0..n {
        let (Some(prev), Some(p), Some(next)) = (ring.get((i + n - 1) % n), ring.get(i), ring.get((i + 1) % n)) else {
            return Vec::new();
        };
        let (Some(a), Some(b)) = (normal(*prev, *p), normal(*p, *next)) else {
            return Vec::new();
        };
        let k = 1.0 + a.x * b.x + a.y * b.y;
        let m = if k > 1e-6 { Vec2::new((a.x + b.x) / k, (a.y + b.y) / k) } else { a };
        out.push(Vec2::new(p.x + m.x * inset, p.y + m.y * inset));
    }
    if let Some(first) = out.first().copied() {
        out.push(first);
    }
    if out.iter().all(|p| p.x.is_finite() && p.y.is_finite()) { out } else { Vec::new() }
}

/// Post positions along `path`: at every vertex and in equal bays at most `spacing` apart on each
/// segment. A closed path does not repeat its first point. A bad `spacing` gives the vertices only.
/// Capped at [`MAX_POSTS`].
pub fn post_points(path: &[Vec2], spacing: f64) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::new();
    if !path_ok(path) {
        return out;
    }
    let Some(first) = path.first().copied() else {
        return out;
    };
    let closed = path.len() >= 4 && path.last().is_some_and(|l| (*l - first).len() <= 1e-9);
    out.push(first);
    let good = spacing.is_finite() && spacing > 0.0;
    if closed {
        for w in path.windows(2).take(path.len().saturating_sub(1)) {
            let (Some(&a), Some(&b)) = (w.first(), w.get(1)) else {
                continue;
            };
            let l = (b - a).len();
            if good && l.is_finite() && l > 1e-12 {
                let bays = (l / spacing - 1e-9).ceil();
                let n = if bays >= 1.0 { bays.min(MAX_POSTS as f64) as usize } else { 1 };
                for k in 1..n {
                    if out.len() >= MAX_POSTS {
                        return out;
                    }
                    let t = k as f64 / n as f64;
                    out.push(Vec2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t));
                }
            }
            if out.len() >= MAX_POSTS {
                return out;
            }
            out.push(b);
        }
        while out.len() > 1 && out.last().is_some_and(|p| (*p - first).len() <= 1e-9) {
            out.pop();
        }
        return out;
    }
    for w in path.windows(2) {
        let (Some(&a), Some(&b)) = (w.first(), w.get(1)) else {
            continue;
        };
        let l = (b - a).len();
        if good && l.is_finite() && l > 1e-12 {
            let bays = (l / spacing - 1e-9).ceil();
            let n = if bays >= 1.0 { bays.min(MAX_POSTS as f64) as usize } else { 1 };
            for k in 1..n {
                if out.len() >= MAX_POSTS {
                    return out;
                }
                let t = k as f64 / n as f64;
                out.push(Vec2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t));
            }
        }
        if out.len() >= MAX_POSTS {
            return out;
        }
        out.push(b);
    }
    out
}

/// The surface a railing stands on.
#[derive(Clone, Debug, PartialEq)]
pub enum RailSurface {
    /// A flat floor at absolute `z`.
    Flat {
        /// Absolute elevation.
        z: f64,
    },
    /// A deck: boards at `top`, posts reaching down to `post_bottom`.
    Deck {
        /// Absolute top of the boards.
        top: f64,
        /// Absolute bottom of the posts (the joist underside).
        post_bottom: f64,
    },
    /// The side of a stair flight.
    Flight {
        /// The flight the railing climbs.
        line: FlightLine,
    },
}

/// The surface `r` stands on, from its host: a deck room, a side of a stair's flight, else the floor
/// of its level.
pub fn rail_surface(model: &ArchModel, r: &Railing) -> RailSurface {
    let flat = || RailSurface::Flat { z: model.level(r.level).map_or(0.0, |l| l.elevation) };
    match r.host {
        RailingHost::Deck => {
            let room = r.follows.and_then(|id| model.room(id));
            let Some(room) = room else { return flat() };
            match room.deck_height {
                Some(h) if h.is_finite() && h > 0.0 => {
                    let top = model.level(room.level).map_or(0.0, |l| l.elevation) + h;
                    RailSurface::Deck { top, post_bottom: top - DECK_BOARD - DECK_JOIST_DEPTH }
                }
                _ => flat(),
            }
        }
        RailingHost::StairLeft | RailingHost::StairRight => {
            let flight = r.follows.and_then(|id| model.stair(id)).and_then(|s| crate::stair_geom::flight(model, s));
            match flight {
                Some(f) => RailSurface::Flight { line: f.line },
                None => flat(),
            }
        }
        RailingHost::Well => flat(),
    }
}

impl RailSurface {
    /// The height a handrail or post top follows at plan point `p`.
    pub fn walk_z(&self, p: Vec2) -> f64 {
        match self {
            RailSurface::Flat { z } => *z,
            RailSurface::Deck { top, .. } => *top,
            RailSurface::Flight { line } => {
                let u = line.local(p).0.clamp(0.0, line.run.max(0.0));
                line.base + line.nosing_v(u)
            }
        }
    }

    /// The height a post at `p` starts from.
    pub fn post_bottom(&self, p: Vec2) -> f64 {
        match self {
            RailSurface::Flat { z } => *z,
            RailSurface::Deck { post_bottom, .. } => *post_bottom,
            RailSurface::Flight { line } => {
                let u = line.local(p).0.clamp(0.0, line.run.max(0.0));
                line.base + line.surface_v(u)
            }
        }
    }
}

#[cfg(test)]
mod surface_tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::{Level, Room, Stair};

    fn model() -> ArchModel {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, elevation: 0.0, ..Level::default() }, Level { index: 1, elevation: 2.7, ..Level::default() }];
        let id = m.push_stair(Stair {
            start: Vec2::new(2.0, 1.0),
            width: 1.0,
            from_level: 0,
            to_level: 1,
            rise: 2.7,
            risers: 16,
            riser_height: 0.16875,
            tread_depth: 0.254,
            treads: 15,
            run: 3.81,
            ..Stair::default()
        });
        assert_eq!(id, ArchId(0));
        m.push_room(Room {
            polygon: vec![Vec2::new(2.0, 8.1), Vec2::new(6.0, 8.1), Vec2::new(6.0, 11.1), Vec2::new(2.0, 11.1)],
            deck_height: Some(0.9),
            ..Room::default()
        });
        m
    }

    #[test]
    fn rail_surface_flat_for_plain_and_well() {
        let m = model();
        let plain = Railing { level: 1, ..Railing::default() };
        assert_eq!(rail_surface(&m, &plain), RailSurface::Flat { z: 2.7 });
        let well = Railing { level: 1, follows: Some(ArchId(0)), host: RailingHost::Well, ..Railing::default() };
        assert_eq!(rail_surface(&m, &well), RailSurface::Flat { z: 2.7 });
        let off = Railing { level: 9, ..Railing::default() };
        assert_eq!(rail_surface(&m, &off), RailSurface::Flat { z: 0.0 });
    }

    #[test]
    fn rail_surface_deck() {
        let m = model();
        let r = Railing { follows: Some(ArchId(0)), host: RailingHost::Deck, ..Railing::default() };
        let s = rail_surface(&m, &r);
        let RailSurface::Deck { top, post_bottom } = s else { panic!("{s:?}") };
        assert!((top - 0.9).abs() < 1e-12 && (post_bottom - 0.691).abs() < 1e-9);
        assert_eq!(s.walk_z(Vec2::new(3.0, 9.0)), top);
        assert_eq!(s.post_bottom(Vec2::new(3.0, 9.0)), post_bottom);
    }

    #[test]
    fn rail_surface_flight_for_stair_sides() {
        let m = model();
        for host in [RailingHost::StairLeft, RailingHost::StairRight] {
            let r = Railing { follows: Some(ArchId(0)), host, ..Railing::default() };
            let s = rail_surface(&m, &r);
            assert!(matches!(s, RailSurface::Flight { .. }), "{s:?}");
            assert!((s.walk_z(Vec2::new(4.0, 1.455)) - 1.5141).abs() < 1e-4);
            assert!((s.post_bottom(Vec2::new(3.27, 1.455)) - 1.0125).abs() < 1e-9);
            // Clamped to the flight at both ends.
            assert!((s.walk_z(Vec2::new(-50.0, 1.0)) - 0.16875 - 0.025 * 0.16875 / 0.254).abs() < 1e-9);
            assert_eq!(s.post_bottom(Vec2::new(500.0, 1.0)), 2.7);
        }
    }

    #[test]
    fn a_missing_host_falls_back_to_flat() {
        let m = model();
        for host in [RailingHost::Deck, RailingHost::StairLeft, RailingHost::StairRight] {
            let r = Railing { level: 1, follows: Some(ArchId(77)), host, ..Railing::default() };
            assert_eq!(rail_surface(&m, &r), RailSurface::Flat { z: 2.7 });
            let none = Railing { level: 1, follows: None, host, ..Railing::default() };
            assert_eq!(rail_surface(&m, &none), RailSurface::Flat { z: 2.7 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2::new(x, y)
    }

    #[test]
    fn built_path_offsets_left_and_mitres() {
        assert_eq!(built_path(&[v(0.0, 0.0), v(4.0, 0.0)], 0.0), vec![v(0.0, 0.0), v(4.0, 0.0)]);
        let b = built_path(&[v(0.0, 0.0), v(4.0, 0.0), v(4.0, 3.0)], 0.5);
        assert_eq!(b.len(), 3);
        assert!(b[0].x.abs() < 1e-9 && (b[0].y - 0.5).abs() < 1e-9);
        assert!((b[1].x - 3.5).abs() < 1e-9 && (b[1].y - 0.5).abs() < 1e-9);
        assert!((b[2].x - 3.5).abs() < 1e-9 && (b[2].y - 3.0).abs() < 1e-9);
        assert!(built_path(&[v(0.0, 0.0), v(f64::NAN, 1.0)], 0.5).is_empty());
    }

    #[test]
    fn built_path_of_a_closed_path_mitres_the_closing_corner() {
        let sq = [v(0.0, 0.0), v(4.0, 0.0), v(4.0, 3.0), v(0.0, 3.0), v(0.0, 0.0)];
        let b = built_path(&sq, 0.5);
        assert_eq!(b.len(), 5);
        assert!((b[0].x - 0.5).abs() < 1e-9 && (b[0].y - 0.5).abs() < 1e-9);
        assert_eq!(b[0], b[4]);
        assert!((b[2].x - 3.5).abs() < 1e-9 && (b[2].y - 2.5).abs() < 1e-9);
    }

    #[test]
    fn post_points_brief_numbers() {
        let p = post_points(&[v(0.0, 0.0), v(0.0, 3.0), v(4.0, 3.0), v(4.0, 0.0)], 1.8);
        let want = [(0.0, 0.0), (0.0, 1.5), (0.0, 3.0), (1.3333, 3.0), (2.6667, 3.0), (4.0, 3.0), (4.0, 1.5), (4.0, 0.0)];
        assert_eq!(p.len(), want.len());
        for (a, b) in p.iter().zip(want) {
            assert!((a.x - b.0).abs() < 1e-3 && (a.y - b.1).abs() < 1e-3, "{a:?} {b:?}");
        }
    }

    #[test]
    fn post_points_closed_path_has_no_duplicate() {
        let sq = [v(0.0, 0.0), v(2.0, 0.0), v(2.0, 2.0), v(0.0, 2.0), v(0.0, 0.0)];
        assert_eq!(post_points(&sq, 5.0).len(), 4);
    }

    #[test]
    fn post_points_with_bad_spacing_gives_the_vertices() {
        let path = [v(0.0, 0.0), v(10.0, 0.0), v(10.0, 5.0)];
        for s in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(post_points(&path, s), path.to_vec());
        }
    }

    #[test]
    fn post_points_caps_at_10000_and_survives_nan() {
        let p = post_points(&[v(0.0, 0.0), v(500.0, 0.0)], 1e-6);
        assert!(p.len() <= MAX_POSTS && !p.is_empty());
        assert!(post_points(&[v(f64::NAN, 0.0), v(1.0, 0.0)], 1.0).is_empty());
        assert!(post_points(&[], 1.0).is_empty());
    }

    #[test]
    fn stair_handrail_band_is_864_to_965() {
        assert_eq!(validate_height(0.5, true), Some(MIN_HANDRAIL));
        assert_eq!(validate_height(2.0, true), Some(MAX_HANDRAIL));
        assert_eq!(validate_height(0.9, true), Some(0.9));
        assert_eq!(validate_height(f64::NAN, true), None);
    }

    #[test]
    fn guard_below_914_is_refused() {
        assert_eq!(validate_height(0.9, false), None);
        assert_eq!(validate_height(0.914, false), Some(0.914));
        assert_eq!(validate_height(1.2, false), Some(1.2));
        assert_eq!(validate_height(1.3, false), None);
    }

    #[test]
    fn baluster_gap_stays_under_101_6_mm() {
        for run in [0.5, 1.0, 2.4384, 3.0, 10.0] {
            let (g, n) = baluster_spacing(run, 0.019).unwrap_or((f64::NAN, 0));
            assert!(g < MAX_BALUSTER_GAP && g > 0.0, "run {run} gap {g}");
            assert!((n as f64 * 0.019 + (n as f64 + 1.0) * g - run).abs() < 1e-9);
        }
        assert!(baluster_spacing(0.0, 0.02).is_none());
        assert!(baluster_spacing(1.0, -1.0).is_none());
        assert!(baluster_spacing(f64::INFINITY, 0.02).is_none());
        assert!(baluster_spacing(1e12, 0.02).is_none());
        assert!(baluster_spacing(0.05, 0.1).is_none());
    }

    #[test]
    fn path_length_and_offset_are_finite() {
        let p = [Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0)];
        assert!((path_length(&p) - 7.0).abs() < 1e-12);
        let o = offset_path(&p, 0.5);
        assert_eq!(o.len(), 3);
        assert!((o[0].y - 0.5).abs() < 1e-12);
        assert!((o[1].x - 3.5).abs() < 1e-12 && (o[1].y - 0.5).abs() < 1e-12);
        assert!(o.iter().all(|q| q.x.is_finite() && q.y.is_finite()));
        let bad = [Vec2::new(f64::NAN, 0.0), Vec2::new(1.0, 0.0)];
        assert_eq!(path_length(&bad), 0.0);
        assert!(offset_path(&bad, 1.0).is_empty());
        assert!(offset_path(&p, f64::NAN).is_empty());
        let long = vec![Vec2::ZERO; 513];
        assert!(offset_path(&long, 1.0).is_empty());
        assert_eq!(path_length(&long), 0.0);
    }

    #[test]
    fn validate_height_exactly_at_boundaries() {
        // Stair handrail: exactly at MIN and MAX should pass through unchanged.
        assert_eq!(validate_height(MIN_HANDRAIL, true), Some(MIN_HANDRAIL));
        assert_eq!(validate_height(MAX_HANDRAIL, true), Some(MAX_HANDRAIL));
        // Guard: exactly at GUARD_HEIGHT and 1.2 should pass.
        assert_eq!(validate_height(GUARD_HEIGHT, false), Some(GUARD_HEIGHT));
        assert_eq!(validate_height(1.2, false), Some(1.2));
        // Just outside the guard band.
        assert_eq!(validate_height(GUARD_HEIGHT - 0.001, false), None);
        assert_eq!(validate_height(1.2 + 0.001, false), None);
    }

    #[test]
    fn baluster_spacing_narrow_run() {
        // Run just barely large enough for one baluster (width 0.019) with gaps < 101.6 mm.
        // n=1: gap = (run - 0.019) / 2 < 0.1016 => run < 0.2222.
        let (g, n) = baluster_spacing(0.1, 0.019).unwrap();
        assert_eq!(n, 1);
        assert!((g - 0.0405).abs() < 1e-9);
        // Run so small that even zero balusters would need a gap >= 101.6 mm.
        // n=0: gap = run. If run >= 0.1016, n=0 works. But the code always uses at least n=1.
        // Actually the code computes min_n and adds 1, so n is always >= 1.
        // For run = 0.05, w = 0.019: gap = (0.05 - 0.019)/2 = 0.0155 < 0.1016. OK.
        let (g, n) = baluster_spacing(0.05, 0.019).unwrap();
        assert_eq!(n, 1);
        assert!((g - 0.0155).abs() < 1e-9);
        // Baluster width nearly the entire run.
        let (g, n) = baluster_spacing(0.021, 0.02).unwrap();
        assert_eq!(n, 1);
        assert!((g - 0.0005).abs() < 1e-9);
    }

    #[test]
    fn path_length_coincident_points() {
        // Coincident points contribute zero length.
        let p = [Vec2::new(0.0, 0.0), Vec2::new(0.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(3.0, 4.0)];
        assert!((path_length(&p) - 7.0).abs() < 1e-12);
        // All points coincident.
        let q = [Vec2::new(1.0, 2.0), Vec2::new(1.0, 2.0), Vec2::new(1.0, 2.0)];
        assert_eq!(path_length(&q), 0.0);
        // Empty path.
        let empty: [Vec2; 0] = [];
        assert_eq!(path_length(&empty), 0.0);
        // Single point.
        let single = [Vec2::new(5.0, 5.0)];
        assert_eq!(path_length(&single), 0.0);
    }

    #[test]
    fn offset_path_collinear() {
        // All points on a horizontal line; offset should shift all by d in y.
        let p = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(3.0, 0.0)];
        let o = offset_path(&p, 0.5);
        assert_eq!(o.len(), 4);
        for (i, q) in o.iter().enumerate() {
            assert!((q.x - p[i].x).abs() < 1e-12);
            assert!((q.y - 0.5).abs() < 1e-12);
        }
        // Zero offset returns the same points.
        let o0 = offset_path(&p, 0.0);
        assert_eq!(o0.len(), 4);
        for (i, q) in o0.iter().enumerate() {
            assert!((q.x - p[i].x).abs() < 1e-12);
            assert!((q.y - p[i].y).abs() < 1e-12);
        }
    }

    #[test]
    fn path_length_exactly_512_points() {
        // 512 points is allowed (the limit); 513 is rejected.
        let p: Vec<Vec2> = (0..512).map(|i| Vec2::new(i as f64, 0.0)).collect();
        // Length = 511 * 1.0 = 511.
        assert!((path_length(&p) - 511.0).abs() < 1e-9);
        let o = offset_path(&p, 1.0);
        assert_eq!(o.len(), 512);
    }
}
