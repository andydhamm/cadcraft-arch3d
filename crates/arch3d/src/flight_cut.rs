//! Where a stair's flight cuts a wall's top: the wall profile polygon is lowered under the flight's
//! underside, the way a gable clip shapes it (no 3D boolean).

use cadcraft_arch::flight_cut::FlightCut;
use cadcraft_geom::Vec2;

use crate::walls::WallProfile;

/// Holes must stay this far inside the outer ring (the same inset `elevation_profile` uses).
const HOLE_INSET: f64 = 1e-3;
/// Smallest wall left over a door notch.
const NOTCH_MARGIN: f64 = 1e-3;
const EPS: f64 = 1e-9;

/// Value of the cut polyline `g` at `x`, `None` outside its range.
fn cut_at(g: &[Vec2], x: f64) -> Option<f64> {
    for pair in g.windows(2) {
        let (p, q) = (pair[0], pair[1]);
        if x >= p.x - EPS && x <= q.x + EPS {
            let dx = q.x - p.x;
            if dx < EPS {
                return Some(p.y.min(q.y));
            }
            return Some(p.y + (q.y - p.y) * ((x - p.x) / dx).clamp(0.0, 1.0));
        }
    }
    None
}

/// Push `p` unless it repeats the last point.
fn push(out: &mut Vec<Vec2>, p: Vec2) {
    if out.last().is_some_and(|l| (l.x - p.x).abs() < EPS && (l.y - p.y).abs() < EPS) {
        return;
    }
    out.push(p);
}

/// The chain (ascending `x`) with `g` taken as a minimum; `changed` is set if any value dropped.
fn min_with(chain: &[Vec2], g: &[Vec2], changed: &mut bool) -> Vec<Vec2> {
    let (Some(g0), Some(g1)) = (g.first(), g.last()) else { return chain.to_vec() };
    let mut out = Vec::new();
    for pair in chain.windows(2) {
        let (p, q) = (pair[0], pair[1]);
        let mut xs = vec![p.x];
        if q.x - p.x > EPS {
            let mut inner: Vec<f64> = g.iter().map(|c| c.x).filter(|&x| x > p.x + EPS && x < q.x - EPS).collect();
            inner.sort_by(|a, b| a.total_cmp(b));
            xs.extend(inner);
        }
        xs.push(q.x);
        let f = |x: f64| if q.x - p.x > EPS { p.y + (q.y - p.y) * (x - p.x) / (q.x - p.x) } else { p.y };
        if q.x - p.x <= EPS {
            // A vertical step: both ends are lowered to the cut.
            let cap = cut_at(g, p.x).filter(|_| p.x >= g0.x - EPS && p.x <= g1.x + EPS);
            for e in [p, q] {
                let y = cap.map_or(e.y, |c| e.y.min(c));
                *changed |= y < e.y - EPS;
                push(&mut out, Vec2::new(e.x, y));
            }
            continue;
        }
        for w in xs.windows(2) {
            let (a, b) = (w[0], w[1]);
            let inside = (a + b) / 2.0 >= g0.x && (a + b) / 2.0 <= g1.x;
            let (fa, fb) = (f(a), f(b));
            if !inside {
                push(&mut out, Vec2::new(a, fa));
                push(&mut out, Vec2::new(b, fb));
                continue;
            }
            let ga = cut_at_in(g, a, b, a);
            let gb = cut_at_in(g, a, b, b);
            let (da, db) = (fa - ga, fb - gb);
            push(&mut out, Vec2::new(a, fa.min(ga)));
            if da * db < 0.0 {
                let x = a + (b - a) * da / (da - db);
                push(&mut out, Vec2::new(x, f(x)));
            }
            push(&mut out, Vec2::new(b, fb.min(gb)));
            *changed |= da > EPS || db > EPS;
        }
    }
    out
}

/// The cut's value at `x` on the sub-interval `[a, b]`, read from the cut segment holding its middle.
fn cut_at_in(g: &[Vec2], a: f64, b: f64, x: f64) -> f64 {
    let mid = (a + b) / 2.0;
    for pair in g.windows(2) {
        let (p, q) = (pair[0], pair[1]);
        if mid >= p.x && mid <= q.x && q.x - p.x > EPS {
            return p.y + (q.y - p.y) * (x - p.x) / (q.x - p.x);
        }
    }
    cut_at(g, x).unwrap_or(f64::INFINITY)
}

/// Drop middle points that lie on the line through their neighbours.
fn drop_collinear(pts: Vec<Vec2>) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::with_capacity(pts.len());
    for p in pts {
        while out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            if cross.abs() < 1e-12 && (b.x - a.x) * (p.x - b.x) + (b.y - a.y) * (p.y - b.y) > 0.0 {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    out
}

/// The chain with a vertex at `x` (interpolated); unchanged if `x` is outside it or already a vertex.
fn split_at(chain: &[Vec2], x: f64) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(chain.len() + 1);
    for (i, p) in chain.iter().enumerate() {
        out.push(*p);
        if let Some(q) = chain.get(i + 1) {
            if x > p.x + EPS && x < q.x - EPS {
                out.push(Vec2::new(x, p.y + (q.y - p.y) * (x - p.x) / (q.x - p.x)));
            }
        }
    }
    out
}

/// The chain lifted to at least `floor` over `[x0, x1]`, with vertical steps at both ends.
fn raise_over(chain: &[Vec2], x0: f64, x1: f64, floor: f64) -> Vec<Vec2> {
    let chain = split_at(&split_at(chain, x0), x1);
    let mut out = Vec::with_capacity(chain.len() + 2);
    for (i, &p) in chain.iter().enumerate() {
        if !(p.x >= x0 - EPS && p.x <= x1 + EPS && p.y < floor) {
            push(&mut out, p);
            continue;
        }
        let lifted = Vec2::new(p.x, floor);
        let same_x = |q: Option<&Vec2>| q.is_some_and(|q| (q.x - p.x).abs() < EPS);
        if (p.x - x0).abs() < EPS {
            if !same_x(i.checked_sub(1).and_then(|j| chain.get(j))) {
                push(&mut out, p);
            }
            push(&mut out, lifted);
        } else if (p.x - x1).abs() < EPS {
            push(&mut out, lifted);
            if !same_x(chain.get(i + 1)) {
                push(&mut out, p);
            }
        } else {
            push(&mut out, lifted);
        }
    }
    out
}

/// Whether `p` lies inside `ring` at least `inset` from every edge.
fn inside_by(ring: &[Vec2], p: Vec2, inset: f64) -> bool {
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let (Some(&a), Some(&b)) = (ring.get(i), ring.get((i + 1) % n)) else { return false };
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) {
            inside = !inside;
        }
        let d = b - a;
        let l2 = d.dot(d);
        let t = if l2 > 0.0 { ((p - a).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let q = a + d * t;
        if (p - q).dot(p - q) < inset * inset {
            return false;
        }
    }
    inside
}

/// Lower `profile` (a wall standing on a level at `base`) under the `cuts`: the top chain of the
/// outer ring becomes its pointwise minimum with each cut's `(w, z - base)` polyline, and holes no
/// longer inside the outer ring are dropped. Non-finite input leaves the profile as it was.
pub(crate) fn apply(profile: &mut WallProfile, cuts: &[FlightCut], base: f64) {
    if cuts.is_empty() || !base.is_finite() || profile.outer.len() < 3 || !profile.outer.iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
        return;
    }
    let len = profile.outer.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    // The bottom chain ends at the first vertex (len, 0) after the start.
    let Some(split) = profile.outer.iter().enumerate().skip(1).find(|(_, p)| (p.x - len).abs() < EPS && p.y.abs() < EPS).map(|(i, _)| i) else {
        return;
    };
    let bottom: Vec<Vec2> = profile.outer.iter().take(split + 1).copied().collect();
    let mut top: Vec<Vec2> = profile.outer.iter().skip(split + 1).rev().copied().collect();
    if top.len() < 2 {
        return;
    }
    let mut changed = false;
    for c in cuts {
        let g: Vec<Vec2> = c.top.iter().map(|p| Vec2::new(p.x, (p.y - base).max(NOTCH_MARGIN))).collect();
        if g.len() < 2 || !g.iter().all(|p| p.x.is_finite() && p.y.is_finite()) || g.windows(2).any(|w| w[1].x < w[0].x) {
            return;
        }
        top = min_with(&top, &g, &mut changed);
    }
    if !changed {
        return;
    }
    // Keep a sliver of wall above a door notch the cut dips under.
    for w in bottom.windows(2) {
        let (a, b) = (w[0], w[1]);
        if (a.y - b.y).abs() < EPS && a.y > EPS && (a.x - b.x).abs() > EPS {
            top = raise_over(&top, a.x.min(b.x), a.x.max(b.x), a.y + NOTCH_MARGIN);
        }
    }
    let top = drop_collinear(top);
    let mut outer = bottom;
    outer.extend(top.into_iter().rev());
    if !outer.iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
        return;
    }
    let holes: Vec<Vec<Vec2>> = profile.holes.iter().filter(|h| h.iter().all(|&p| inside_by(&outer, p, HOLE_INSET))).cloned().collect();
    profile.outer = outer;
    profile.holes = holes;
}
