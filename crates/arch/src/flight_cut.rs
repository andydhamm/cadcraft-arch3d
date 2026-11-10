//! Where a stair flight cuts the underside of a wall above it.
//!
//! A wall that runs under a flight stops at the flight's underside (`Stair.underside` below the
//! line through the tread nosings, measured vertically), built like a gable clip: a polyline over
//! the wall's length that the 3D builder takes the minimum with the wall's top.

use crate::stair_geom::{self, NOSING};
use crate::{ArchId, ArchModel, Wall};
use cadcraft_geom::Vec2;

/// Smallest height of wall left standing over a flight cut.
pub const MIN_CUT_HEIGHT: f64 = 0.05;

/// Most cuts one wall carries.
pub const MAX_CUTS_PER_WALL: usize = 64;

/// A polyline `(w, z)` along a wall: `w` is the distance along the wall's centre line from
/// `wall.start`, increasing; `z` is absolute. The wall stops at it over `[first w, last w]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlightCut {
    /// The stair that cuts.
    pub stair: ArchId,
    /// The `(w, z)` polyline.
    pub top: Vec<Vec2>,
}

/// Clip the parameter range `[lo, hi]` of `p(s) = a + s*d` against `-h <= p <= h`.
fn clip_slab(a: f64, d: f64, h: f64, lo: &mut f64, hi: &mut f64) -> bool {
    if d.abs() < 1e-12 {
        return a.abs() <= h;
    }
    let s0 = (-h - a) / d;
    let s1 = (h - a) / d;
    let (s0, s1) = if s0 <= s1 { (s0, s1) } else { (s1, s0) };
    *lo = lo.max(s0);
    *hi = hi.min(s1);
    *lo <= *hi
}

/// The flights that cut `wall` from above, in stair order (at most [`MAX_CUTS_PER_WALL`]).
pub fn flight_cuts(model: &ArchModel, wall: &Wall) -> Vec<FlightCut> {
    let mut out = Vec::new();
    if wall.divider || !(wall.start.is_finite() && wall.end.is_finite()) {
        return out;
    }
    let d = wall.end - wall.start;
    let len = d.len();
    if !(len.is_finite() && len >= 1e-6) {
        return out;
    }
    let top = wall.top_z(&model.levels);
    if !top.is_finite() {
        return out;
    }
    for s in &model.stairs {
        if out.len() >= MAX_CUTS_PER_WALL {
            break;
        }
        let Some(f) = stair_geom::flight(model, s) else { continue };
        let on_level = wall.base_level == s.from_level || model.level(s.from_level).is_some_and(|l| wall.spans_level(l, &model.levels));
        if !on_level {
            continue;
        }
        // The wall's centre line in flight coordinates.
        let (u0, t0) = f.local(wall.start);
        let (u1, t1) = f.local(wall.end);
        let (du, dt) = (u1 - u0, t1 - t0);
        if ![u0, t0, du, dt].iter().all(|v| v.is_finite()) {
            continue;
        }
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        let mid_u = (f.line.run - NOSING) / 2.0;
        let half_u = (f.line.run + NOSING) / 2.0;
        if !(clip_slab(u0 - mid_u, du, half_u, &mut lo, &mut hi) && clip_slab(t0, dt, f.width / 2.0, &mut lo, &mut hi)) {
            continue;
        }
        let (w0, w1) = (lo * len, hi * len);
        if !(w0.is_finite() && w1.is_finite()) || w1 - w0 < 1e-6 {
            continue;
        }
        let uv = |w: f64| f.underside_v(u0 + du * (w / len));
        let (a, b) = (uv(w0), uv(w1));
        if !(a.is_finite() && b.is_finite()) {
            continue;
        }
        let base = f.line.base;
        let z = |v: f64| base + v.max(MIN_CUT_HEIGHT);
        let mut pts = vec![Vec2::new(w0, z(a))];
        if (a - MIN_CUT_HEIGHT) * (b - MIN_CUT_HEIGHT) < 0.0 {
            let wb = w0 + (w1 - w0) * (a - MIN_CUT_HEIGHT) / (a - b);
            if wb > w0 && wb < w1 {
                pts.push(Vec2::new(wb, base + MIN_CUT_HEIGHT));
            }
        }
        pts.push(Vec2::new(w1, z(b)));
        if pts.iter().all(|p| p.y >= top) {
            continue;
        }
        out.push(FlightCut { stair: s.id, top: pts });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{Level, Stair};

    fn model() -> ArchModel {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        m.levels.push(Level { index: 1, elevation: 2.7, ceiling_height: 2.4, ..Default::default() });
        m.stairs.push(Stair {
            id: ArchId(200),
            start: Vec2::new(1.0, 1.0),
            width: 0.9,
            from_level: 0,
            to_level: 1,
            rise: 2.7,
            risers: 16,
            riser_height: 0.16875,
            treads: 15,
            tread_depth: 0.254,
            run: 3.81,
            ..Default::default()
        });
        m
    }

    #[test]
    fn crossing_wall_gets_a_flat_cut() {
        let m = model();
        let w = Wall { start: Vec2::new(3.0, 0.0), end: Vec2::new(3.0, 3.0), thickness: 0.1, height: 2.7, ..Default::default() };
        let c = flight_cuts(&m, &w);
        assert_eq!(c.len(), 1);
        let t = &c[0].top;
        assert!((t[0].x - 0.55).abs() < 1e-9 && (t[1].x - 1.45).abs() < 1e-9);
        assert!((t[0].y - 1.2140994).abs() < 1e-4);
    }
}
