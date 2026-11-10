//! Stair flight geometry shared by every stair consumer (3D builder, railings, walls).
//!
//! Flight coordinates: `u` along `direction` from `start`, `t` across (positive to the LEFT of
//! travel), `v` up from the from-level elevation. `start` is the centre of the bottom nosing line.
//! Tread `k` (1..=treads) has its top at `k * riser`, is [`TREAD_THICKNESS`] thick and spans
//! `u` in `[(k-1) * tread - NOSING, k * tread]`; riser `k` has its face at `u = (k-1) * tread`.

use cadcraft_doc::arch::{DEFAULT_UNDERSIDE, MAX_UNDERSIDE, MIN_UNDERSIDE};
use cadcraft_doc::{ArchModel, Level, Stair};
use cadcraft_geom::Vec2;

/// Thickness of a tread board (metres).
pub const TREAD_THICKNESS: f64 = 0.025;
/// How far a tread overhangs the riser below it.
pub const NOSING: f64 = 0.025;
/// Thickness of a riser board.
pub const RISER_THICKNESS: f64 = 0.019;
/// Thickness of a side stringer.
pub const STRINGER_THICKNESS: f64 = 0.038;
/// How far a stringer stands above the nosing line.
pub const STRINGER_ABOVE: f64 = 0.05;
/// Width of a centre stringer.
pub const CENTRE_STRINGER_WIDTH: f64 = 0.038;
/// Thickness of a skirt board against a wall.
pub const SKIRT_THICKNESS: f64 = 0.019;
/// How far a skirt board hangs below the nosing line.
pub const SKIRT_BELOW: f64 = 0.25;
/// How far a skirt board stands above the nosing line.
pub const SKIRT_ABOVE: f64 = 0.1;
/// How close a wall face must lie to a flight side for the side to be against the wall.
pub const AGAINST_TOL: f64 = 0.03;
/// Cap on the treads of one flight.
pub const MAX_FLIGHT_TREADS: usize = 1024;

/// What a railing on a stair needs of the flight: no width, no underside.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlightLine {
    /// The centre of the bottom nosing line.
    pub origin: Vec2,
    /// Unit direction of travel.
    pub dir: Vec2,
    /// Elevation of the from-level.
    pub base: f64,
    /// Riser height.
    pub riser: f64,
    /// Tread depth.
    pub tread: f64,
    /// Number of treads.
    pub treads: usize,
    /// Horizontal run.
    pub run: f64,
    /// Total rise.
    pub rise: f64,
}

impl FlightLine {
    /// The unit vector to the left of travel.
    pub fn left(&self) -> Vec2 {
        self.dir.perp()
    }

    /// The plan point at `(u, t)`.
    pub fn at(&self, u: f64, t: f64) -> Vec2 {
        self.origin + self.dir * u + self.left() * t
    }

    /// The `(u, t)` of a plan point.
    pub fn local(&self, p: Vec2) -> (f64, f64) {
        let d = p - self.origin;
        (d.dot(self.dir), d.dot(self.left()))
    }

    /// Front `u` of tread `k` (nosing included).
    pub fn nosing_u(&self, k: usize) -> f64 {
        k.saturating_sub(1) as f64 * self.tread - NOSING
    }

    /// The height of the line through the tread nosings at `u`.
    pub fn nosing_v(&self, u: f64) -> f64 {
        self.riser + (u + NOSING) * self.riser / self.tread
    }

    /// The walking surface under `u`: 0 before the flight, the rise past the run, else the highest
    /// tread top over `u`.
    pub fn surface_v(&self, u: f64) -> f64 {
        if u < -NOSING {
            return 0.0;
        }
        if u >= self.run {
            return self.rise;
        }
        let mut best = 0.0f64;
        for k in 1..=self.treads.min(MAX_FLIGHT_TREADS) {
            if self.nosing_u(k) <= u && u <= k as f64 * self.tread {
                best = best.max(k as f64 * self.riser);
            }
        }
        best
    }
}

/// A whole flight: the line plus width and underside.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flight {
    /// The line part.
    pub line: FlightLine,
    /// Flight width.
    pub width: f64,
    /// Depth of the underside below the nosing line.
    pub underside: f64,
}

impl Flight {
    /// The flight of `s` standing on a from-level at elevation `base`; `None` for a stair that
    /// cannot be built (non-finite or non-positive numbers).
    pub fn new(s: &Stair, base: f64) -> Option<Flight> {
        let ok = s.start.is_finite()
            && s.direction.is_finite()
            && s.width.is_finite()
            && s.width > 0.0
            && s.riser_height.is_finite()
            && s.riser_height > 0.0
            && s.tread_depth.is_finite()
            && s.tread_depth > 0.0
            && s.rise.is_finite()
            && s.rise > 0.0
            && s.run.is_finite()
            && s.run >= 0.0
            && base.is_finite();
        if !ok {
            return None;
        }
        let treads = (1..=s.treads.min(MAX_FLIGHT_TREADS)).take_while(|k| (*k as f64 - 1.0) * s.tread_depth < s.run - 1e-9).count();
        if treads < 1 {
            return None;
        }
        let underside =
            if s.underside.is_finite() && (MIN_UNDERSIDE..=MAX_UNDERSIDE).contains(&s.underside) { s.underside } else { DEFAULT_UNDERSIDE };
        let line = FlightLine {
            origin: s.start,
            dir: Vec2::new(s.direction.cos(), s.direction.sin()),
            base,
            riser: s.riser_height,
            tread: s.tread_depth,
            treads,
            run: s.run,
            rise: s.rise,
        };
        Some(Flight { line, width: s.width, underside })
    }

    /// The underside at `u`.
    pub fn underside_v(&self, u: f64) -> f64 {
        self.line.nosing_v(u) - self.underside
    }

    /// Plan footprint, CCW: `u` in `[-NOSING, run]`, `t` in `[-width/2, width/2]`.
    pub fn footprint(&self) -> [Vec2; 4] {
        let h = self.width / 2.0;
        [self.line.at(-NOSING, -h), self.line.at(self.line.run, -h), self.line.at(self.line.run, h), self.line.at(-NOSING, h)]
    }

    /// See [`FlightLine::left`].
    pub fn left(&self) -> Vec2 {
        self.line.left()
    }
    /// See [`FlightLine::at`].
    pub fn at(&self, u: f64, t: f64) -> Vec2 {
        self.line.at(u, t)
    }
    /// See [`FlightLine::local`].
    pub fn local(&self, p: Vec2) -> (f64, f64) {
        self.line.local(p)
    }
    /// See [`FlightLine::nosing_u`].
    pub fn nosing_u(&self, k: usize) -> f64 {
        self.line.nosing_u(k)
    }
    /// See [`FlightLine::nosing_v`].
    pub fn nosing_v(&self, u: f64) -> f64 {
        self.line.nosing_v(u)
    }
    /// See [`FlightLine::surface_v`].
    pub fn surface_v(&self, u: f64) -> f64 {
        self.line.surface_v(u)
    }
}

/// The flight of `s` in `model` (base = the from-level's elevation, 0 when missing).
pub fn flight(model: &ArchModel, s: &Stair) -> Option<Flight> {
    Flight::new(s, model.level(s.from_level).map_or(0.0, |l| l.elevation))
}

/// Which sides of the flight lie against a wall: `[left (+t), right (-t)]`.
pub fn sides_against_walls(model: &ArchModel, s: &Stair) -> [bool; 2] {
    let Some(f) = flight(model, s) else { return [false, false] };
    let level = model.level(s.from_level);
    let h = f.width / 2.0;
    let mut out = [false, false];
    for w in &model.walls {
        if w.divider {
            continue;
        }
        let on_level = w.base_level == s.from_level || level.is_some_and(|l: &Level| w.spans_level(l, &model.levels));
        if !on_level {
            continue;
        }
        let Some(faces) = crate::join::face_lines(w) else { continue };
        for (a, b) in faces {
            let d = b - a;
            let len = d.len();
            if !(len.is_finite() && len > 1e-9) {
                continue;
            }
            // Parallel to the flight within 1e-3 rad.
            if d.cross(f.line.dir).abs() / len > 1e-3 {
                continue;
            }
            let (ua, ta) = f.local(a);
            let (ub, tb) = f.local(b);
            let overlap = ua.max(ub).min(f.line.run) - ua.min(ub).max(-NOSING);
            // At least half of the shorter of the face and the flight must run alongside.
            if overlap < len.min(f.line.run + NOSING) / 2.0 {
                continue;
            }
            for (i, side) in [h, -h].into_iter().enumerate() {
                if (ta - side).abs() <= AGAINST_TOL && (tb - side).abs() <= AGAINST_TOL {
                    if let Some(slot) = out.get_mut(i) {
                        *slot = true;
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::Wall;

    fn stair() -> Stair {
        Stair {
            start: Vec2::new(2.0, 1.0),
            direction: 0.0,
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
        }
    }

    fn house() -> ArchModel {
        let mut m = ArchModel::default();
        m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        m.push_wall(Wall::new(Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), 0.2));
        m.push_wall(Wall::new(Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0), 0.2));
        m.push_wall(Wall::new(Vec2::new(0.0, 8.0), Vec2::new(0.0, 0.0), 0.2));
        m
    }

    #[test]
    fn flight_numbers_match_the_brief() {
        let f = Flight::new(&stair(), 0.0).unwrap();
        assert_eq!(f.line.treads, 15);
        assert!((f.line.nosing_v(2.0) - 1.5141).abs() < 1e-4, "{}", f.line.nosing_v(2.0));
        assert!((f.line.nosing_u(1) + NOSING).abs() < 1e-12);
        assert!((f.underside - 0.3).abs() < 1e-12);
        assert!((f.underside_v(2.0) - 1.2141).abs() < 1e-4);
        let fp = f.footprint();
        assert!((fp[0].x - 1.975).abs() < 1e-9 && (fp[0].y - 0.5).abs() < 1e-9);
        assert!((fp[2].x - 5.81).abs() < 1e-9 && (fp[2].y - 1.5).abs() < 1e-9);
    }

    #[test]
    fn surface_v_steps() {
        let l = Flight::new(&stair(), 0.0).unwrap().line;
        assert_eq!(l.surface_v(-0.1), 0.0);
        assert_eq!(l.surface_v(l.run), l.rise);
        assert!((l.surface_v(0.1) - 0.16875).abs() < 1e-12);
        assert!((l.surface_v(0.3) - 2.0 * 0.16875).abs() < 1e-12);
        assert!((l.surface_v(0.254) - 2.0 * 0.16875).abs() < 1e-12);
        assert!((l.surface_v(0.5) - 3.0 * 0.16875).abs() < 1e-12);
    }

    #[test]
    fn local_and_at_round_trip() {
        let mut s = stair();
        s.direction = 0.7;
        let l = Flight::new(&s, 1.0).unwrap().line;
        let p = l.at(1.3, -0.4);
        let (u, t) = l.local(p);
        assert!((u - 1.3).abs() < 1e-12 && (t + 0.4).abs() < 1e-12);
        assert_eq!(l.base, 1.0);
    }

    #[test]
    fn treads_in_run_respects_run() {
        let mut s = stair();
        s.run = 0.254 * 3.0;
        assert_eq!(Flight::new(&s, 0.0).unwrap().line.treads, 3);
        s.run = 0.0;
        assert!(Flight::new(&s, 0.0).is_none());
        s.run = 3.81;
        s.treads = 100_000;
        assert!(Flight::new(&s, 0.0).unwrap().line.treads <= MAX_FLIGHT_TREADS);
    }

    #[test]
    fn flight_refuses_nan_zero_and_huge() {
        let base = stair();
        let edits: [fn(&mut Stair); 8] = [
            |s| s.start = Vec2::new(f64::NAN, 0.0),
            |s| s.direction = f64::INFINITY,
            |s| s.width = 0.0,
            |s| s.riser_height = f64::NAN,
            |s| s.tread_depth = 0.0,
            |s| s.rise = -1.0,
            |s| s.run = f64::NAN,
            |s| s.treads = 0,
        ];
        for f in edits {
            let mut s = base.clone();
            f(&mut s);
            assert!(Flight::new(&s, 0.0).is_none());
        }
        assert!(Flight::new(&base, f64::NAN).is_none());
        let mut s = base.clone();
        s.width = 1e300;
        s.run = 1e300;
        s.tread_depth = 1e300;
        let _ = Flight::new(&s, 0.0).map(|f| (f.footprint(), f.line.surface_v(1.0)));
        let mut s = base;
        s.underside = f64::NAN;
        assert_eq!(Flight::new(&s, 0.0).unwrap().underside, DEFAULT_UNDERSIDE);
    }

    #[test]
    fn sides_against_walls() {
        let m = house();
        let mut s = stair();
        s.start = Vec2::new(2.0, 0.6);
        assert_eq!(super::sides_against_walls(&m, &s), [false, true]);
        s.start = Vec2::new(2.0, 1.0);
        assert_eq!(super::sides_against_walls(&m, &s), [false, false]);
        s.width = f64::NAN;
        assert_eq!(super::sides_against_walls(&m, &s), [false, false]);
    }
}
