//! Corner cabinets (phase 18): a base or wall cabinet at the end of a run, where another run of the
//! same kind meets it at a right angle, is a corner cabinet (blind, diagonal door or lazy susan).
//! The counter then runs round the corner in one piece. Pure.
//!
//! Frame of a cabinet `c`: origin at its back centre, `u` along the width, `y` out of the front.

use crate::countertop::{CounterKey, CounterOutline};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchId, Cabinet, CabinetKind, CornerPref};
use cadcraft_geom::Vec2;

/// Which width end of the cabinet is in the corner: `u = -w/2` or `+w/2`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CornerEnd {
    /// `u = -w/2`.
    #[default]
    Start,
    /// `u = +w/2`.
    End,
}

/// How a corner cabinet turns the corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CornerStyle {
    /// Blind corner.
    #[default]
    Blind,
    /// Diagonal front.
    Diagonal,
    /// Lazy susan.
    LazySusan,
}

/// A cabinet standing in a corner.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CornerInfo {
    /// The end in the corner.
    pub end: CornerEnd,
    /// The style.
    pub style: CornerStyle,
    /// Distance from this cabinet's back line, along the other wall, to the other run's nearest cabinet.
    pub leg: f64,
    /// That cabinet's depth.
    pub other_depth: f64,
    /// That cabinet.
    pub partner: ArchId,
}

/// Tolerance of the partner's alignment (m).
const ALIGN: f64 = 1e-3;
/// Farthest the other run's near end may stand from the back line (m).
const MAX_LEG: f64 = 1.2;
/// How much room an L style needs beyond a blind corner (m).
const ROOM: f64 = 0.05;
/// Most counter merges of one model.
const MAX_MERGES: usize = 64;
/// How far inside a front edge the fronts' line sits (m).
const FRONT_INSET: f64 = 0.019;

fn rot(p: Vec2, deg: f64) -> Vec2 {
    let (s, c) = deg.to_radians().sin_cos();
    Vec2::new(p.x * c - p.y * s, p.x * s + p.y * c)
}

fn sound(c: &Cabinet) -> bool {
    [c.position.x, c.position.y, c.rotation, c.width, c.depth].iter().all(|v| v.is_finite()) && c.width > 0.0 && c.depth > 0.0
}

/// Whether two angles are equal modulo 360 degrees (within 1e-6).
fn same_angle(a: f64, b: f64) -> bool {
    let d = (a - b).rem_euclid(360.0);
    d < 1e-6 || 360.0 - d < 1e-6
}

/// The corner `c` stands in, if any.
pub fn corner_info(model: &ArchModel, c: &Cabinet) -> Option<CornerInfo> {
    if !matches!(c.kind, CabinetKind::Base | CabinetKind::Wall) || c.filler || c.corner == CornerPref::None || !sound(c) {
        return None;
    }
    let mut found: Option<(CornerEnd, f64, &Cabinet)> = None;
    // End wins over Start when both qualify.
    for (end, u_e, turn) in [(CornerEnd::End, c.width / 2.0, 90.0), (CornerEnd::Start, -c.width / 2.0, -90.0)] {
        let mut best: Option<(f64, &Cabinet)> = None;
        for b in &model.cabinets {
            if b.id == c.id || b.kind != c.kind || b.level != c.level || b.filler || !sound(b) || !same_angle(b.rotation, c.rotation + turn) {
                continue;
            }
            let rel = rot(b.position - c.position, -c.rotation);
            let y_near = rel.y - b.width / 2.0;
            if !((rel.x - u_e).abs() <= ALIGN && y_near >= c.depth - ALIGN && y_near <= MAX_LEG) {
                continue;
            }
            if best.is_none_or(|(y, o)| y_near < y || (y_near == y && b.id < o.id)) {
                best = Some((y_near, b));
            }
        }
        if let Some((leg, b)) = best {
            found = Some((end, leg, b));
            break;
        }
    }
    let (end, leg, b) = found?;
    let wanted = match c.corner {
        CornerPref::Diagonal => CornerStyle::Diagonal,
        CornerPref::LazySusan => CornerStyle::LazySusan,
        CornerPref::Blind => CornerStyle::Blind,
        _ if leg <= c.depth + ALIGN => CornerStyle::Blind,
        _ => CornerStyle::LazySusan,
    };
    let roomy = leg > c.depth + ROOM && c.width > b.depth + ROOM;
    let style = if wanted != CornerStyle::Blind && !roomy { CornerStyle::Blind } else { wanted };
    Some(CornerInfo { end, style, leg, other_depth: b.depth, partner: b.id })
}

/// The footprint of `c` in its own frame, counter-clockwise.
pub fn corner_local_footprint(c: &Cabinet, info: &CornerInfo) -> Vec<Vec2> {
    let (w2, d, db, leg) = (c.width / 2.0, c.depth, info.other_depth, info.leg);
    let p = Vec2::new;
    let end_form = match info.style {
        CornerStyle::Blind => vec![p(-w2, 0.0), p(w2, 0.0), p(w2, d), p(-w2, d)],
        CornerStyle::LazySusan => vec![p(-w2, 0.0), p(w2, 0.0), p(w2, leg), p(w2 - db, leg), p(w2 - db, d), p(-w2, d)],
        CornerStyle::Diagonal => vec![p(-w2, 0.0), p(w2, 0.0), p(w2, leg), p(w2 - db, leg), p(-w2, d)],
    };
    match info.end {
        CornerEnd::End => end_form,
        // Mirrored in u, which reverses the winding.
        CornerEnd::Start => end_form.into_iter().rev().map(|q| p(-q.x, q.y)).collect(),
    }
}

/// The footprint of `c` in the world, counter-clockwise.
pub fn corner_footprint(c: &Cabinet, info: &CornerInfo) -> Vec<Vec2> {
    corner_local_footprint(c, info).into_iter().map(|q| c.position + rot(q, c.rotation)).collect()
}

/// Which edges of the local-frame footprint `ring` are front edges: those that are neither on the
/// back line nor on the corner end's side nor on the free end's side nor (L styles) on the line the
/// other run starts at: a neighbouring cabinet touches those. Edge `i` joins `ring[i]` to `ring[i + 1]`.
pub fn front_edges(c: &Cabinet, info: &CornerInfo, ring: &[Vec2]) -> Vec<bool> {
    let w2 = c.width / 2.0;
    let on = |a: Vec2, b: Vec2, f: &dyn Fn(Vec2) -> f64| f(a).abs() < 1e-9 && f(b).abs() < 1e-9;
    let l_style = info.style != CornerStyle::Blind;
    (0..ring.len())
        .map(|i| match (ring.get(i), ring.get((i + 1) % ring.len())) {
            (Some(&a), Some(&b)) => {
                !(on(a, b, &|q| q.y) || on(a, b, &|q| q.x - w2) || on(a, b, &|q| q.x + w2) || (l_style && on(a, b, &|q| q.y - info.leg)))
            }
            _ => false,
        })
        .collect()
}

/// `ring` (counter-clockwise) with edge `i` moved outward by `dist[i]` (negative = inward), the
/// corners mitred; parallel neighbours keep their vertex, moved along the offset.
pub fn offset_ring(ring: &[Vec2], dist: &[f64]) -> Vec<Vec2> {
    let n = ring.len();
    let edge = |i: usize| -> Option<(Vec2, Vec2)> {
        let (a, b) = (*ring.get(i)?, *ring.get((i + 1) % n)?);
        let dir = (b - a).normalized();
        let normal = Vec2::new(dir.y, -dir.x);
        Some((a + normal * dist.get(i).copied().unwrap_or(0.0), dir))
    };
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let (Some((p0, d0)), Some((p1, d1))) = (edge((k + n - 1) % n), edge(k)) else { return ring.to_vec() };
        let cross = d0.cross(d1);
        let q = if cross.abs() < 1e-9 {
            p1
        } else {
            // p0 + d0 t = p1 + d1 s.
            let t = (p1 - p0).cross(d1) / cross;
            p0 + d0 * t
        };
        out.push(q);
    }
    if out.iter().all(|q| q.is_finite()) { out } else { ring.to_vec() }
}

fn closed(mut r: Vec<Vec2>) -> Vec<Vec2> {
    if let Some(&f) = r.first() {
        r.push(f);
    }
    r
}

fn to_world(c: &Cabinet, ring: Vec<Vec2>) -> Vec<Vec2> {
    ring.into_iter().map(|q| c.position + rot(q, c.rotation)).collect()
}

/// The footprint with its front edges moved out by `by`, in the cabinet's frame.
fn local_offset(c: &Cabinet, info: &CornerInfo, by: f64) -> Vec<Vec2> {
    let ring = corner_local_footprint(c, info);
    let dist: Vec<f64> = front_edges(c, info, &ring).iter().map(|&f| if f { by } else { 0.0 }).collect();
    offset_ring(&ring, &dist)
}

/// Merges the outlines of runs that meet at a corner: the union of both runs' outlines and the
/// corner cabinet's own footprint with its front edges out by the overhang. At most 64 merges.
pub fn merge_corners(model: &ArchModel, mut outlines: Vec<CounterOutline>) -> Vec<CounterOutline> {
    let mut corners: Vec<&Cabinet> = model.cabinets.iter().filter(|c| c.kind == CabinetKind::Base && c.counter).collect();
    corners.sort_by_key(|c| c.id);
    let mut merges = 0;
    for a in corners {
        if merges >= MAX_MERGES {
            break;
        }
        let Some(info) = corner_info(model, a) else { continue };
        let Some(b) = model.cabinets.iter().find(|b| b.id == info.partner) else { continue };
        if b.kind != CabinetKind::Base || !b.counter {
            continue;
        }
        let holds = |id: ArchId, lvl: u32| outlines.iter().position(|o| o.level == lvl && o.cabinets.contains(&id));
        let (Some(i), Some(j)) = (holds(a.id, a.level), holds(b.id, b.level)) else { continue };
        if i == j {
            continue;
        }
        let (Some(oi), Some(oj)) = (outlines.get(i), outlines.get(j)) else { continue };
        if (oi.top - oj.top).abs() > 1e-6 || (oi.thickness - oj.thickness).abs() > 1e-6 {
            continue;
        }
        let mut rings = vec![closed(oi.ring.clone()), closed(oj.ring.clone())];
        if info.style != CornerStyle::Blind {
            rings.push(closed(to_world(a, local_offset(a, &info, a.counter_overhang))));
        }
        let Some(ring) = cadcraft_geom::region::outline(&rings, 1e-9, 0.0) else { continue };
        let ring = tidy(ring);
        if ring.len() < 3 {
            continue;
        }
        let owner = |o: &CounterOutline| match o.key {
            Some(CounterKey::Auto(id)) => Some(id),
            _ => None,
        };
        let key = [owner(oi), owner(oj)].into_iter().flatten().min().map(CounterKey::Auto).or(oi.key);
        let mut cabinets: Vec<ArchId> = oi.cabinets.iter().chain(&oj.cabinets).copied().collect();
        cabinets.sort();
        cabinets.dedup();
        let mut bridged: Vec<ArchId> = oi.bridged.iter().chain(&oj.bridged).copied().collect();
        bridged.sort();
        bridged.dedup();
        let merged = CounterOutline {
            key,
            level: oi.level,
            cabinets,
            sagittas: vec![0.0; ring.len()],
            ring,
            top: oi.top,
            thickness: oi.thickness,
            holes: oi.holes.iter().chain(&oj.holes).cloned().collect(),
            bridged,
        };
        let (lo, hi) = (i.min(j), i.max(j));
        outlines.remove(hi);
        if let Some(slot) = outlines.get_mut(lo) {
            *slot = merged;
        }
        merges += 1;
    }
    outlines
}

/// Drops duplicate and collinear vertices.
fn tidy(ring: Vec<Vec2>) -> Vec<Vec2> {
    let mut r: Vec<Vec2> = Vec::with_capacity(ring.len());
    for p in ring {
        if r.last().is_none_or(|l| l.dist(p) >= 1e-9) {
            r.push(p);
        }
    }
    while r.len() > 1 && r.first().zip(r.last()).is_some_and(|(f, l)| f.dist(*l) < 1e-9) {
        r.pop();
    }
    loop {
        let n = r.len();
        let drop = (0..n).find(|&i| match (r.get((i + n - 1) % n), r.get(i), r.get((i + 1) % n)) {
            (Some(&a), Some(&b), Some(&c)) => (b - a).cross(c - b).abs() < 1e-12,
            _ => false,
        });
        match drop {
            Some(i) if n > 3 => {
                r.remove(i);
            }
            _ => break,
        }
    }
    r
}

/// The merged outline holding `c`, when it joined two or more runs.
pub fn merged_outline_of(model: &ArchModel, c: &Cabinet) -> Option<CounterOutline> {
    if c.kind != CabinetKind::Base || !c.counter {
        return None;
    }
    // Most models have no corner: skip the outlines (a scene build asks for every run). Corners need two
    // directions at a right angle, which one pass tells.
    let mut turns =
        model.cabinets.iter().filter(|x| x.kind == CabinetKind::Base && x.counter && x.rotation.is_finite()).map(|x| x.rotation.rem_euclid(180.0));
    if let Some(first) = turns.next()
        && turns.all(|t| (t - first).abs() < 1e-6 || (t - first).abs() > 180.0 - 1e-6)
    {
        return None;
    }
    if !model.cabinets.iter().any(|x| x.kind == CabinetKind::Base && x.counter && corner_info(model, x).is_some()) {
        return None;
    }
    let outlines = crate::countertop::auto_outlines(model);
    let o = outlines.into_iter().find(|o| o.level == c.level && o.cabinets.contains(&c.id))?;
    let runs = crate::casework_layout::counter_runs(model);
    let joined = runs.iter().filter(|r| o.cabinets.contains(&r.owner)).count();
    (joined >= 2).then_some(o)
}

/// The plan symbol of a corner cabinet: the footprint, the front lines, the counter piece (base
/// with a counter), the style's mark (a circle for a lazy susan, a diagonal across the blind part)
/// and, for a wall cabinet, the footprint's bounding-box diagonals.
pub fn corner_symbol(model: &ArchModel, c: &Cabinet) -> Option<Vec<Vec<Vec2>>> {
    let info = corner_info(model, c)?;
    let ring = corner_local_footprint(c, &info);
    let front = front_edges(c, &info, &ring);
    let (w2, d, db) = (c.width / 2.0, c.depth, info.other_depth);
    let sign = if info.end == CornerEnd::End { 1.0 } else { -1.0 };
    let p = |u: f64, y: f64| Vec2::new(sign * u, y);
    let mut out: Vec<Vec<Vec2>> = vec![to_world(c, closed(ring.clone()))];
    let inset: Vec<f64> = front.iter().map(|&f| if f { -FRONT_INSET } else { 0.0 }).collect();
    let inner = offset_ring(&ring, &inset);
    for (i, &f) in front.iter().enumerate() {
        if let (true, Some(&a), Some(&b)) = (f, inner.get(i), inner.get((i + 1) % inner.len())) {
            out.push(to_world(c, vec![a, b]));
        }
    }
    if c.kind == CabinetKind::Base && c.counter {
        out.push(to_world(c, closed(local_offset(c, &info, c.counter_overhang))));
    }
    match info.style {
        CornerStyle::LazySusan => {
            let (cx, cy, r) = (w2 - db / 2.0, d / 2.0, 0.4 * d.min(db));
            let circle: Vec<Vec2> = (0..24)
                .map(|k| {
                    let a = std::f64::consts::TAU * f64::from(k) / 24.0;
                    p(cx + r * a.cos(), cy + r * a.sin())
                })
                .collect();
            out.push(to_world(c, closed(circle)));
        }
        CornerStyle::Blind => out.push(to_world(c, vec![p(w2 - db, 0.0), p(w2, d)])),
        CornerStyle::Diagonal => {}
    }
    if c.kind == CabinetKind::Wall {
        let top = ring.iter().map(|q| q.y).fold(d, f64::max);
        out.push(to_world(c, vec![Vec2::new(-w2, 0.0), Vec2::new(w2, top)]));
        out.push(to_world(c, vec![Vec2::new(-w2, top), Vec2::new(w2, 0.0)]));
    }
    out.iter().all(|r| r.iter().all(|q| q.is_finite())).then_some(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::countertop::auto_outlines;

    fn base(id: u32, x: f64, y: f64, rotation: f64, width: f64) -> Cabinet {
        Cabinet {
            id: ArchId(id),
            kind: CabinetKind::Base,
            position: Vec2::new(x, y),
            rotation,
            width,
            depth: 0.6,
            height: 0.876,
            counter: true,
            counter_thickness: 0.038,
            counter_overhang: 0.025,
            ..Default::default()
        }
    }

    fn model(cabs: Vec<Cabinet>) -> ArchModel {
        ArchModel { cabinets: cabs, ..Default::default() }
    }

    /// The check's kitchen: a south run 0.9 + 0.6 + 0.6 from the corner, then a west run (rotation
    /// 270, from y = `top` down) of two 0.6 cabinets whose near end is `leg` up the wall.
    fn runs(leg: f64, push: Option<f64>) -> ArchModel {
        let mut v = vec![base(0, 0.45, 0.0, 0.0, 0.9), base(1, 1.2, 0.0, 0.0, 0.6), base(2, 1.8, 0.0, 0.0, 0.6)];
        if let Some(w) = push {
            v = vec![base(0, 1.2, 0.0, 0.0, 0.6), base(1, 1.8, 0.0, 0.0, 0.6)];
            v.push(base(2, w / 2.0, 0.0, 0.0, w));
        }
        let n = v.len() as u32;
        v.push(base(n, 0.0, leg + 0.9, 270.0, 0.6));
        v.push(base(n + 1, 0.0, leg + 0.3, 270.0, 0.6));
        model(v)
    }

    fn near(a: Vec2, x: f64, y: f64) -> bool {
        (a.x - x).abs() < 1e-9 && (a.y - y).abs() < 1e-9
    }

    #[test]
    fn blind_corner_where_the_other_run_butts() {
        let m = runs(0.6, None);
        let info = corner_info(&m, &m.cabinets[0]).unwrap();
        assert_eq!((info.style, info.end, info.partner), (CornerStyle::Blind, CornerEnd::Start, ArchId(4)));
        assert!((info.leg - 0.6).abs() < 1e-9 && (info.other_depth - 0.6).abs() < 1e-12);
        // No other cabinet is a corner cabinet.
        assert!(m.cabinets.iter().skip(1).all(|c| corner_info(&m, c).is_none()));
    }

    #[test]
    fn lazy_susan_when_the_other_run_starts_further() {
        let m = runs(0.9, Some(0.9));
        let c = m.cabinets.iter().find(|c| c.id == ArchId(2)).unwrap();
        let info = corner_info(&m, c).unwrap();
        assert_eq!((info.style, info.end), (CornerStyle::LazySusan, CornerEnd::Start));
        assert!((info.leg - 0.9).abs() < 1e-9);
    }

    #[test]
    fn end_and_start_corners() {
        // A run along the south wall from the corner: the corner is at its start.
        let mut a = base(0, 0.45, 0.0, 0.0, 0.9);
        let b = base(1, 0.0, 0.9, 270.0, 0.6);
        let m = model(vec![a.clone(), b]);
        assert_eq!(corner_info(&m, &a).unwrap().end, CornerEnd::Start);
        // Turned 180 degrees about its back centre, the corner is at its end.
        a.rotation = 180.0;
        let b2 = base(1, 0.0, -0.9, 270.0, 0.6);
        let m2 = model(vec![a.clone(), b2]);
        let i = corner_info(&m2, &a).unwrap();
        assert_eq!(i.end, CornerEnd::End);
        assert!((i.leg - 0.6).abs() < 1e-9);
    }

    #[test]
    fn preference_none_and_forced_styles() {
        let mut m = runs(0.6, None);
        m.cabinets[0].corner = CornerPref::None;
        assert!(corner_info(&m, &m.cabinets[0]).is_none());
        // Too tight for the L styles: a forced lazy susan with the other run butting falls back to blind.
        m.cabinets[0].corner = CornerPref::LazySusan;
        assert_eq!(corner_info(&m, &m.cabinets[0]).unwrap().style, CornerStyle::Blind);
        let mut m = runs(0.9, Some(0.9));
        let ix = m.cabinets.iter().position(|c| c.id == ArchId(2)).unwrap();
        m.cabinets[ix].corner = CornerPref::Diagonal;
        assert_eq!(corner_info(&m, &m.cabinets[ix]).unwrap().style, CornerStyle::Diagonal);
        m.cabinets[ix].corner = CornerPref::Blind;
        assert_eq!(corner_info(&m, &m.cabinets[ix]).unwrap().style, CornerStyle::Blind);
        m.cabinets[ix].corner = CornerPref::Auto;
        assert_eq!(corner_info(&m, &m.cabinets[ix]).unwrap().style, CornerStyle::LazySusan);
    }

    #[test]
    fn l_styles_fall_back_to_blind_when_too_tight() {
        let mut m = runs(0.9, Some(0.9));
        let ix = m.cabinets.iter().position(|c| c.id == ArchId(2)).unwrap();
        m.cabinets[ix].corner = CornerPref::LazySusan;
        // A cabinet barely wider than the other run is deep: no room for the notch.
        m.cabinets[ix].width = 0.64;
        m.cabinets[ix].position = Vec2::new(0.32, 0.0);
        assert_eq!(corner_info(&m, &m.cabinets[ix]).unwrap().style, CornerStyle::Blind);
        m.cabinets[ix].corner = CornerPref::Diagonal;
        assert_eq!(corner_info(&m, &m.cabinets[ix]).unwrap().style, CornerStyle::Blind);
    }

    #[test]
    fn wall_cabinet_corners() {
        let wall = |id: u32, x: f64, y: f64, rot: f64, w: f64| Cabinet {
            kind: CabinetKind::Wall,
            depth: 0.305,
            elevation: 1.372,
            counter: false,
            ..base(id, x, y, rot, w)
        };
        let m = model(vec![wall(0, 0.45, 0.0, 0.0, 0.9), wall(1, 0.0, 0.605, 270.0, 0.6)]);
        let info = corner_info(&m, &m.cabinets[0]).unwrap();
        assert_eq!(info.style, CornerStyle::Blind);
        let sym = corner_symbol(&m, &m.cabinets[0]).unwrap();
        // Footprint, front line, blind diagonal and the two bounding-box diagonals.
        assert_eq!(sym.len(), 5);
        // A base cabinet is no partner of a wall cabinet.
        let mixed = model(vec![wall(0, 0.45, 0.0, 0.0, 0.9), base(1, 0.0, 0.9, 270.0, 0.6)]);
        assert!(corner_info(&mixed, &mixed.cabinets[0]).is_none());
    }

    #[test]
    fn no_partner_no_corner() {
        let m = model(vec![base(0, 0.45, 0.0, 0.0, 0.9), base(1, 1.5, 0.0, 0.0, 0.6)]);
        assert!(m.cabinets.iter().all(|c| corner_info(&m, c).is_none()));
        // Too far up the wall (leg 1.3 > 1.2).
        let far = runs(1.3, None);
        assert!(corner_info(&far, &far.cabinets[0]).is_none());
        // Fillers have no corner.
        let mut f = runs(0.6, None);
        f.cabinets[0].filler = true;
        assert!(corner_info(&f, &f.cabinets[0]).is_none());
    }

    #[test]
    fn partner_must_be_perpendicular_same_kind_same_level() {
        let good = runs(0.6, None);
        assert!(corner_info(&good, &good.cabinets[0]).is_some());
        let mut m = runs(0.6, None);
        for c in m.cabinets.iter_mut().skip(3) {
            c.rotation = 280.0;
        }
        assert!(corner_info(&m, &m.cabinets[0]).is_none(), "not perpendicular");
        let mut m = runs(0.6, None);
        for c in m.cabinets.iter_mut().skip(3) {
            c.level = 1;
        }
        assert!(corner_info(&m, &m.cabinets[0]).is_none(), "other level");
        let mut m = runs(0.6, None);
        for c in m.cabinets.iter_mut().skip(3) {
            c.kind = CabinetKind::Wall;
        }
        assert!(corner_info(&m, &m.cabinets[0]).is_none(), "other kind");
        let mut m = runs(0.6, None);
        for c in m.cabinets.iter_mut().skip(3) {
            c.position.x = 0.1;
        }
        assert!(corner_info(&m, &m.cabinets[0]).is_none(), "not on the end line");
        // A rotation of -90 is a rotation of 270.
        let mut m = runs(0.6, None);
        for c in m.cabinets.iter_mut().skip(3) {
            c.rotation = -90.0;
        }
        assert!(corner_info(&m, &m.cabinets[0]).is_some());
    }

    #[test]
    fn corner_footprints() {
        let m = runs(0.9, Some(0.9));
        let c = m.cabinets.iter().find(|c| c.id == ArchId(2)).unwrap();
        let mut info = corner_info(&m, c).unwrap();
        let fp = corner_footprint(c, &info);
        let want = [(0.0, 0.0), (0.9, 0.0), (0.9, 0.6), (0.6, 0.6), (0.6, 0.9), (0.0, 0.9)];
        assert_eq!(fp.len(), want.len());
        for w in want {
            assert!(fp.iter().any(|p| near(*p, w.0, w.1)), "{w:?} in {fp:?}");
        }
        let area = |r: &[Vec2]| (0..r.len()).map(|i| r[i].cross(r[(i + 1) % r.len()])).sum::<f64>() / 2.0;
        assert!((area(&fp) - 0.72).abs() < 1e-9, "counter-clockwise, 0.9 x 0.6 + 0.6 x 0.3");
        info.style = CornerStyle::Diagonal;
        let fp = corner_footprint(c, &info);
        for w in [(0.0, 0.0), (0.9, 0.0), (0.9, 0.6), (0.6, 0.9), (0.0, 0.9)] {
            assert!(fp.iter().any(|p| near(*p, w.0, w.1)), "{w:?} in {fp:?}");
        }
        assert_eq!(fp.len(), 5);
        assert!(area(&fp) > 0.0);
        info.style = CornerStyle::Blind;
        let fp = corner_footprint(c, &info);
        assert_eq!(fp.len(), 4);
        assert!((area(&fp) - 0.54).abs() < 1e-9);
    }

    #[test]
    fn merge_corners_unions_the_runs() {
        let m = runs(0.6, None);
        let o = auto_outlines(&m);
        assert_eq!(o.len(), 1, "one counter polygon");
        assert_eq!(o[0].key, Some(CounterKey::Auto(ArchId(0))));
        assert_eq!(o[0].cabinets, vec![ArchId(0), ArchId(1), ArchId(2), ArchId(3), ArchId(4)]);
        assert!((crate::countertop::outline_area(&o[0]) - 2.046875).abs() < 1e-9);
        assert_eq!(o[0].sagittas, vec![0.0; o[0].ring.len()]);
        assert_eq!(o[0].ring.len(), 6, "an L");
        assert_eq!(merged_outline_of(&m, &m.cabinets[3]).map(|x| x.key), Some(o[0].key));
        let m = runs(0.9, Some(0.9));
        let o = auto_outlines(&m);
        assert_eq!(o.len(), 1);
        assert!((crate::countertop::outline_area(&o[0]) - 2.234375).abs() < 1e-9);
        // Without a corner there are two counters and nothing merged.
        let mut m = runs(0.6, None);
        m.cabinets[0].corner = CornerPref::None;
        assert_eq!(auto_outlines(&m).len(), 2);
        assert!(merged_outline_of(&m, &m.cabinets[0]).is_none());
    }

    #[test]
    fn different_counter_tops_do_not_merge() {
        let mut m = runs(0.6, None);
        for c in m.cabinets.iter_mut().skip(3) {
            c.counter_thickness = 0.05;
        }
        assert_eq!(auto_outlines(&m).len(), 2);
        let mut m = runs(0.6, None);
        for c in m.cabinets.iter_mut().skip(3) {
            c.height = 0.9;
        }
        assert_eq!(auto_outlines(&m).len(), 2);
        // No counter on the corner cabinet's partner.
        let mut m = runs(0.6, None);
        for c in m.cabinets.iter_mut().skip(3) {
            c.counter = false;
        }
        assert_eq!(auto_outlines(&m).len(), 1);
    }

    #[test]
    fn corner_symbol_shows_the_cut_out() {
        let m = runs(0.9, Some(0.9));
        let c = m.cabinets.iter().find(|c| c.id == ArchId(2)).unwrap();
        let sym = corner_symbol(&m, c).unwrap();
        let fp = &sym[0];
        assert_eq!(fp.len(), 7, "closed L");
        assert_eq!(fp.first(), fp.last());
        // Front lines 0.019 inside the notch faces.
        assert_eq!(sym.iter().filter(|r| r.len() == 2).count(), 2, "one front line per face of the notch");
        assert!(sym.iter().any(|r| r.len() == 2 && r.iter().all(|p| (p.y - 0.581).abs() < 1e-9)));
        // The circle of a lazy susan: 24 points, closed, round the square's centre.
        let circle = sym.iter().find(|r| r.len() == 25).unwrap();
        let (cx, cy) = (0.3, 0.3);
        assert!(circle.iter().all(|p| ((p.x - cx).hypot(p.y - cy) - 0.4 * 0.6).abs() < 1e-9));
        // The counter piece reaches 0.025 beyond the notch faces.
        assert!(sym.iter().any(|r| r.iter().any(|p| near(*p, 0.625, 0.625))));
        let mut d = c.clone();
        d.corner = CornerPref::Diagonal;
        let mut m2 = m.clone();
        let ix = m2.cabinets.iter().position(|x| x.id == c.id).unwrap();
        m2.cabinets[ix] = d;
        let sym = corner_symbol(&m2, &m2.cabinets[ix]).unwrap();
        assert!(sym.iter().all(|r| r.len() != 25));
        assert_eq!(sym[0].len(), 6);
    }

    #[test]
    fn hostile_cabinets_give_no_corner() {
        let good = runs(0.6, None);
        for f in 0..6 {
            for bad in [f64::NAN, f64::INFINITY, -0.6, 0.0] {
                let mut m = good.clone();
                match f {
                    0 => m.cabinets[0].width = bad,
                    1 => m.cabinets[0].depth = bad,
                    2 => m.cabinets[0].position.x = bad,
                    3 => m.cabinets[0].rotation = bad,
                    4 => m.cabinets[4].width = bad,
                    _ => m.cabinets[4].position.y = bad,
                }
                for c in &m.cabinets {
                    let _ = corner_info(&m, c);
                    let _ = corner_symbol(&m, c);
                }
                let _ = auto_outlines(&m);
                if f < 4 && !bad.is_nan() && !(f == 3 && bad == 0.0) {
                    assert!(corner_info(&m, &m.cabinets[0]).is_none(), "field {f} = {bad}");
                }
            }
        }
        assert!(corner_info(&good, &good.cabinets[0]).is_some());
    }
}
