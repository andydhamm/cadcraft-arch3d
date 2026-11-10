//! Shaped window tops: outlines and validation.
//!
//! Coordinates are the host wall's profile measured from the wall's bottom: `u` along the wall
//! from its start, `v = z - base`. Outlines are counter-clockwise polygons.

use crate::opening::ROUGH_ALLOWANCE;
use cadcraft_doc::arch::{Opening, OpeningTop};
use cadcraft_geom::Vec2;
use std::f64::consts::PI;

/// Segments over a half circle when an arc top is sampled.
pub const ARC_SEGMENTS: usize = 24;

/// Half the rough allowance: the margin around the frame on each side.
const R2: f64 = ROUGH_ALLOWANCE / 2.0;

/// Smallest rough top above the sill of a roof-top window, and of any vertex of it.
const MIN_ROOF_HEAD: f64 = 0.3;

/// The roof polyline's height at `u`, or `None` when it does not cover `u` (or is unusable).
fn poly_at(pts: &[Vec2], u: f64) -> Option<f64> {
    if pts.len() < 2 || !u.is_finite() || pts.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return None;
    }
    const E: f64 = 1e-9;
    for pair in pts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if b.x < a.x {
            return None;
        }
        if u >= a.x - E && u <= b.x + E {
            let d = b.x - a.x;
            if d < 1e-12 {
                return Some(a.y.max(b.y));
            }
            let t = ((u - a.x) / d).clamp(0.0, 1.0);
            return Some(a.y + (b.y - a.y) * t);
        }
    }
    None
}

/// Roof top height T(u) = polyline minus the roof offset, and the polyline vertices strictly
/// between `ua` and `ub` (as `(u, T)`). `None` when the rough span is not covered.
fn roof_head(o: &Opening, ua: f64, ub: f64, roof_top: Option<&[Vec2]>) -> Option<(f64, f64, Vec<(f64, f64)>)> {
    let pts = roof_top?;
    let ta = poly_at(pts, ua)? - o.roof_offset;
    let tb = poly_at(pts, ub)? - o.roof_offset;
    let mid: Vec<(f64, f64)> = pts.iter().filter(|p| p.x > ua + 1e-9 && p.x < ub - 1e-9).map(|p| (p.x, p.y - o.roof_offset)).collect();
    Some((ta, tb, mid))
}

/// The roof top height T at the opening's centre (roof polyline less the offset), if covered.
fn roof_t_at(o: &Opening, roof_top: Option<&[Vec2]>) -> Option<f64> {
    let half = o.width / 2.0 + R2;
    roof_head(o, o.offset - half, o.offset + half, roof_top)?;
    Some(poly_at(roof_top?, o.offset)? - o.roof_offset)
}

/// Sides at `c +- a` from `v0` up to the arc of radius `rad` about `(c, cy)`, sampled from the
/// right side over the top to the left side. Counter-clockwise.
fn arched(c: f64, v0: f64, a: f64, cy: f64, rad: f64) -> Option<Vec<Vec2>> {
    let dy2 = rad * rad - a * a;
    if !(dy2 >= 0.0) || !(rad > 0.0) {
        return None;
    }
    let side_top = cy + dy2.sqrt();
    let th0 = dy2.sqrt().atan2(a);
    let span = PI - 2.0 * th0;
    let mut n = ((ARC_SEGMENTS as f64 * span / PI).ceil() as usize).max(2);
    if n % 2 == 1 {
        n += 1;
    }
    let mut out = vec![Vec2::new(c - a, v0), Vec2::new(c + a, v0), Vec2::new(c + a, side_top)];
    for i in 1..n {
        let th = th0 + span * (i as f64) / (n as f64);
        out.push(Vec2::new(c + rad * th.cos(), cy + rad * th.sin()));
    }
    out.push(Vec2::new(c - a, side_top));
    Some(out)
}

fn finite(poly: Vec<Vec2>) -> Option<Vec<Vec2>> {
    poly.iter().all(|p| p.x.is_finite() && p.y.is_finite()).then_some(poly)
}

/// The finished (frame) outline of an opening whose lower edge is at `v0`. `None` for a roof top
/// without a covering `roof_top`, or when the numbers do not make a shape.
pub fn finished_outline(o: &Opening, v0: f64, roof_top: Option<&[Vec2]>) -> Option<Vec<Vec2>> {
    let (c, w) = (o.offset, o.width);
    if !(c.is_finite() && w.is_finite() && v0.is_finite() && o.height.is_finite() && w > 0.0) {
        return None;
    }
    let poly = match o.top {
        OpeningTop::Square => {
            let (u0, u1, v1) = (c - w / 2.0, c + w / 2.0, v0 + o.height);
            vec![Vec2::new(u0, v0), Vec2::new(u1, v0), Vec2::new(u1, v1), Vec2::new(u0, v1)]
        }
        OpeningTop::Round => {
            let vs = v0 + o.height - w / 2.0;
            if vs < v0 {
                return None;
            }
            arched(c, v0, w / 2.0, vs, w / 2.0)?
        }
        OpeningTop::Segment => {
            let r = o.top_rise;
            if !(r > 0.0) || !r.is_finite() {
                return None;
            }
            let vs = v0 + o.height - r;
            let big = (w * w / 4.0 + r * r) / (2.0 * r);
            arched(c, v0, w / 2.0, vs + r - big, big)?
        }
        OpeningTop::Roof => roof_outline(o, v0, roof_top, w / 2.0, ROUGH_ALLOWANCE)?,
    };
    finite(poly)
}

/// The rough (cut) outline of an opening whose lower edge is at `v0`. See [`finished_outline`].
pub fn rough_outline(o: &Opening, v0: f64, roof_top: Option<&[Vec2]>) -> Option<Vec<Vec2>> {
    let (c, w) = (o.offset, o.width);
    if !(c.is_finite() && w.is_finite() && v0.is_finite() && o.height.is_finite() && w > 0.0) {
        return None;
    }
    let poly = match o.top {
        OpeningTop::Square => {
            let half = (w + ROUGH_ALLOWANCE) / 2.0;
            let v1 = v0 + o.height + ROUGH_ALLOWANCE;
            vec![Vec2::new(c - half, v0), Vec2::new(c + half, v0), Vec2::new(c + half, v1), Vec2::new(c - half, v1)]
        }
        OpeningTop::Round => {
            let vs = v0 + o.height - w / 2.0;
            if vs < v0 {
                return None;
            }
            arched(c, v0, w / 2.0 + R2, vs, w / 2.0 + R2)?
        }
        OpeningTop::Segment => {
            let r = o.top_rise;
            if !(r > 0.0) || !r.is_finite() {
                return None;
            }
            let vs = v0 + o.height - r;
            let big = (w * w / 4.0 + r * r) / (2.0 * r);
            arched(c, v0, w / 2.0 + R2, vs + r - big, big + R2)?
        }
        OpeningTop::Roof => roof_outline(o, v0, roof_top, w / 2.0 + R2, 0.0)?,
    };
    finite(poly)
}

/// A gable outline: sides at `c +- a` from `v0` up to the roof top T less `drop`, following it.
/// The rough span (`w/2 + R2` each side) must be covered by the polyline.
fn roof_outline(o: &Opening, v0: f64, roof_top: Option<&[Vec2]>, a: f64, drop: f64) -> Option<Vec<Vec2>> {
    let c = o.offset;
    let rough = o.width / 2.0 + R2;
    // Coverage is always checked over the rough span.
    roof_head(o, c - rough, c + rough, roof_top)?;
    let (ta, tb, mid) = roof_head(o, c - a, c + a, roof_top)?;
    let mut out = vec![Vec2::new(c - a, v0), Vec2::new(c + a, v0), Vec2::new(c + a, tb - drop)];
    for &(u, t) in mid.iter().rev() {
        out.push(Vec2::new(u, t - drop));
    }
    out.push(Vec2::new(c - a, ta - drop));
    // The head must stay above the sill.
    out.iter().skip(2).all(|p| p.y > v0).then_some(out)
}

/// Check a shaped top and, for a roof top, set `o.height` and `o.rough_height`. `roof_top` is in
/// the host wall's profile coordinates measured from the wall's bottom (`u` along the wall from
/// its start, `v = z - base`); `o.sill` is measured from that bottom, so it is the outline's lower
/// edge. A segment top with no rise gets the default `width / 6`.
pub fn validate(o: &mut Opening, roof_top: Option<&[Vec2]>) -> Result<(), String> {
    if o.top != OpeningTop::Square && !(o.offset.is_finite() && o.width.is_finite() && o.height.is_finite() && o.sill.is_finite()) {
        return Err("the opening has a non-finite size".into());
    }
    match o.top {
        OpeningTop::Square => Ok(()),
        OpeningTop::Round => {
            if o.height >= o.width / 2.0 + 0.1 {
                Ok(())
            } else {
                Err("a round top needs a height of at least width/2 + 0.1".into())
            }
        }
        OpeningTop::Segment => {
            if !o.top_rise.is_finite() {
                return Err("the segment rise must be finite".into());
            }
            if o.top_rise <= 0.0 {
                o.top_rise = o.width / 6.0;
            }
            let r = o.top_rise;
            if r > 0.0 && r <= o.width / 2.0 && r <= o.height - 0.1 {
                Ok(())
            } else {
                Err("a segment top needs 0 < rise <= width/2 and rise <= height - 0.1".into())
            }
        }
        OpeningTop::Roof => {
            const NEED: &str = "top \"roof\" needs a gable wall under a roof";
            if !o.roof_offset.is_finite() || !(0.0..=1.0).contains(&o.roof_offset) {
                return Err("roofOffset must be between 0 and 1".into());
            }
            let rough = o.width / 2.0 + R2;
            let (ta, tb, mid) = roof_head(o, o.offset - rough, o.offset + rough, roof_top).ok_or(NEED)?;
            let low = std::iter::once(ta).chain(std::iter::once(tb)).chain(mid.iter().map(|m| m.1)).fold(f64::INFINITY, f64::min);
            if !low.is_finite() || low < o.sill + MIN_ROOF_HEAD {
                return Err("the roof is too low above the sill for a roof top".into());
            }
            let t = roof_t_at(o, roof_top).ok_or(NEED)?;
            o.height = t - ROUGH_ALLOWANCE - o.sill;
            o.rough_height = o.height + ROUGH_ALLOWANCE;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_accepts_a_square_opening() {
        let mut o = Opening::default();
        assert!(validate(&mut o, None).is_ok());
    }

    fn win(top: OpeningTop, offset: f64, width: f64, height: f64, sill: f64) -> Opening {
        Opening { offset, width, height, sill, top, ..Opening::default() }
    }

    fn top_at(poly: &[Vec2], u: f64) -> f64 {
        // highest polygon edge height at u (upper chain)
        let mut best = f64::NEG_INFINITY;
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            if (a.x - b.x).abs() < 1e-12 {
                continue;
            }
            let (lo, hi) = (a.x.min(b.x), a.x.max(b.x));
            if u >= lo - 1e-12 && u <= hi + 1e-12 {
                let t = (u - a.x) / (b.x - a.x);
                best = best.max(a.y + (b.y - a.y) * t);
            }
        }
        best
    }

    fn gable() -> Vec<Vec2> {
        // rises 0..3 m at 6/12 → 0.5 slope to a ridge at u=3 (v 3.2 - ... ) keep it simple
        vec![Vec2::new(0.0, 2.4), Vec2::new(4.0, 4.4), Vec2::new(8.0, 2.4)]
    }

    #[test]
    fn square_outline_is_todays_rectangle() {
        let o = win(OpeningTop::Square, 3.0, 1.0, 1.5, 0.9);
        let r = rough_outline(&o, 0.9, None).unwrap_or_default();
        let half = (1.0 + ROUGH_ALLOWANCE) / 2.0;
        assert_eq!(
            r,
            vec![
                Vec2::new(3.0 - half, 0.9),
                Vec2::new(3.0 + half, 0.9),
                Vec2::new(3.0 + half, 0.9 + 1.5 + ROUGH_ALLOWANCE),
                Vec2::new(3.0 - half, 0.9 + 1.5 + ROUGH_ALLOWANCE)
            ]
        );
        let f = finished_outline(&o, 0.9, None).unwrap_or_default();
        assert!((top_at(&f, 3.0) - 2.4).abs() < 1e-12);
    }

    #[test]
    fn round_outline_numbers() {
        let o = win(OpeningTop::Round, 6.5, 0.9, 1.5, 0.6);
        let f = finished_outline(&o, 0.6, None).unwrap_or_default();
        assert!((top_at(&f, 6.5) - 2.1).abs() < 1e-9);
        let r = rough_outline(&o, 0.6, None).unwrap_or_default();
        assert!((top_at(&r, 6.5) - 2.16).abs() < 1e-9);
        assert!((r[2].x - 6.5 - 0.51).abs() < 1e-9 && (r[2].y - 1.65).abs() < 1e-9);
        // every arc vertex sits on the radius-0.51 circle about (offset, 1.65)
        for p in &r[3..r.len() - 1] {
            assert!((((p.x - 6.5).powi(2) + (p.y - 1.65).powi(2)).sqrt() - 0.51).abs() < 1e-9);
        }
        assert_eq!(r.len(), 4 + ARC_SEGMENTS - 1);
        // chord error well inside a millimetre-scale tolerance at du 0.4
        assert!((top_at(&r, 6.9) - 1.9663858).abs() < 2e-3);
    }

    #[test]
    fn segment_outline_numbers() {
        let mut o = win(OpeningTop::Segment, 3.0, 0.9, 1.2, 0.9);
        o.top_rise = 0.15;
        let r = rough_outline(&o, 0.9, None).unwrap_or_default();
        assert!((top_at(&r, 3.0) - 2.16).abs() < 1e-9);
        assert!((r[2].y - 1.9792853).abs() < 1e-6);
        assert!((top_at(&r, 3.45) - 2.0234983).abs() < 2e-3);
        let cy = 1.35;
        for p in &r[3..r.len() - 1] {
            assert!((((p.x - 3.0).powi(2) + (p.y - cy).powi(2)).sqrt() - 0.81).abs() < 1e-9);
        }
        let f = finished_outline(&o, 0.9, None).unwrap_or_default();
        assert!((top_at(&f, 3.0) - 2.1).abs() < 1e-9);
        assert!((f[2].y - 1.95).abs() < 1e-9);
    }

    #[test]
    fn segment_default_rise_is_width_over_6() {
        let mut o = win(OpeningTop::Segment, 3.0, 0.9, 1.2, 0.9);
        assert!(validate(&mut o, None).is_ok());
        assert!((o.top_rise - 0.15).abs() < 1e-12);
        let mut bad = win(OpeningTop::Segment, 3.0, 0.9, 1.2, 0.9);
        bad.top_rise = 0.5;
        assert!(validate(&mut bad, None).is_err());
    }

    #[test]
    fn roof_outline_follows_the_polyline_less_the_offset() {
        let roof = gable();
        let mut o = win(OpeningTop::Roof, 3.0, 1.0, 1.0, 1.2);
        let r = rough_outline(&o, 1.2, Some(&roof)).unwrap_or_default();
        // T = polyline - 0.15; at the right side u = 3.0 + 0.56 the polyline is 2.4 + 0.5 * 3.56
        assert!((r[2].y - (2.4 + 0.5 * 3.56 - 0.15)).abs() < 1e-9);
        assert!(validate(&mut o, Some(&roof)).is_ok());
        let t = 2.4 + 0.5 * 3.0 - 0.15;
        assert!((o.height - (t - ROUGH_ALLOWANCE - 1.2)).abs() < 1e-9);
        assert!((o.rough_height - (o.height + ROUGH_ALLOWANCE)).abs() < 1e-12);
        let f = finished_outline(&o, 1.2, Some(&roof)).unwrap_or_default();
        assert!((top_at(&f, 3.0) - (t - ROUGH_ALLOWANCE)).abs() < 1e-9);
        // a vertex of the polyline inside the span becomes a vertex of the outline
        let mut o2 = win(OpeningTop::Roof, 4.0, 1.0, 1.0, 1.2);
        o2.roof_offset = 0.25;
        let r2 = rough_outline(&o2, 1.2, Some(&roof)).unwrap_or_default();
        assert!(r2.iter().any(|p| (p.x - 4.0).abs() < 1e-9 && (p.y - (4.4 - 0.25)).abs() < 1e-9));
    }

    #[test]
    fn validate_refusals_never_panic() {
        let roof = gable();
        let mut o = win(OpeningTop::Roof, 3.5, 1.0, 1.0, 1.2);
        assert!(validate(&mut o, None).is_err());
        let mut far = win(OpeningTop::Roof, 7.9, 1.0, 1.0, 1.2);
        assert!(validate(&mut far, Some(&roof)).is_err());
        let mut low = win(OpeningTop::Roof, 3.5, 1.0, 1.0, 3.9);
        assert!(validate(&mut low, Some(&roof)).is_err());
        let mut off = win(OpeningTop::Roof, 3.5, 1.0, 1.0, 1.2);
        off.roof_offset = 1.5;
        assert!(validate(&mut off, Some(&roof)).is_err());
        off.roof_offset = f64::NAN;
        assert!(validate(&mut off, Some(&roof)).is_err());
        let mut round = win(OpeningTop::Round, 3.0, 0.9, 0.5, 0.9);
        assert!(validate(&mut round, None).is_err());
        let mut nan = win(OpeningTop::Round, f64::NAN, 0.9, 1.5, 0.9);
        assert!(validate(&mut nan, None).is_err());
        let mut seg = win(OpeningTop::Segment, 3.0, 0.9, 1.2, 0.9);
        seg.top_rise = f64::NAN;
        assert!(validate(&mut seg, None).is_err());
        let bad = [Vec2::new(0.0, f64::NAN), Vec2::new(8.0, 2.0)];
        let mut o2 = win(OpeningTop::Roof, 3.5, 1.0, 1.0, 1.2);
        assert!(validate(&mut o2, Some(&bad)).is_err());
        assert!(rough_outline(&o2, 1.2, Some(&bad)).is_none());
        assert!(validate(&mut Opening::default(), None).is_ok());
    }
}
