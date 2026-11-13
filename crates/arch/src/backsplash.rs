//! Backsplash segments (phase 18): where a backsplash stands on its wall.

use crate::countertop::{self, CounterKey, CounterOutline};
use cadcraft_doc::arch::{ArchId, ArchModel, Backsplash, Cabinet, CabinetKind, CounterRef, Wall};
use cadcraft_geom::Vec2;

/// One straight piece of a backsplash.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BacksplashSegment {
    /// Start along the wall face. `a -> b` keeps the counter outline's direction, so the room
    /// lies to the left of it.
    pub a: Vec2,
    /// End along the wall face.
    pub b: Vec2,
    /// The wall it stands on.
    pub wall: ArchId,
    /// Absolute z of the bottom.
    pub bottom: f64,
    /// Absolute z of the top.
    pub top: f64,
}

/// Pieces shorter than this (m) are dropped.
const MIN_PIECE: f64 = 1e-6;
/// How far a wall cabinet must hang above the counter top to count (m).
const MIN_CLEARANCE: f64 = 0.01;
/// How far into the room the ceiling height is read (m).
const ROOM_PROBE: f64 = 0.05;
/// Most segments of one backsplash.
const MAX_SEGMENTS: usize = 4096;

/// A cabinet's back edge in world plan coordinates.
fn back_edge(c: &Cabinet) -> Option<(Vec2, Vec2)> {
    if ![c.position.x, c.position.y, c.rotation, c.width].iter().all(|v| v.is_finite()) || c.width <= 0.0 {
        return None;
    }
    let (s, k) = c.rotation.to_radians().sin_cos();
    let u = Vec2::new(k, s);
    let h = c.width / 2.0;
    Some((c.position - u * h, c.position + u * h))
}

/// The distance of `p` from the line through `o` with unit direction `d`.
fn line_dist(o: Vec2, d: Vec2, p: Vec2) -> f64 {
    d.cross(p - o).abs()
}

/// The edge-parameter interval of a cabinet's back along the edge `a + t * e` when it lies on the
/// line `(o, d)` within `tol`.
fn back_interval(c: &Cabinet, a: Vec2, e: Vec2, line: (Vec2, Vec2), tol: f64) -> Option<(f64, f64)> {
    let (p, q) = back_edge(c)?;
    if line_dist(line.0, line.1, p) > tol || line_dist(line.0, line.1, q) > tol {
        return None;
    }
    let (t0, t1) = ((p - a).dot(e), (q - a).dot(e));
    Some((t0.min(t1), t0.max(t1)))
}

/// A wall face an edge lies on.
struct Face<'a> {
    wall: &'a Wall,
    /// A point on the face line.
    origin: Vec2,
    /// The unit direction of the face line.
    dir: Vec2,
}

/// The face of `w` the edge `a + t * e` (`0..=len`) lies on and the edge interval the wall covers.
fn face_of<'a>(w: &'a Wall, a: Vec2, e: Vec2, len: f64) -> Option<(Face<'a>, f64, f64)> {
    let d = crate::wall::wall_dir(w)?;
    if !w.thickness.is_finite() || w.thickness <= 0.0 || d.cross(e).abs() > 1e-6 {
        return None;
    }
    let n = d.perp();
    let mid = a + e * (len / 2.0);
    let off = (mid - w.start).dot(n);
    let half = w.thickness / 2.0;
    let side = [half, -half].into_iter().find(|f| (off - f).abs() <= 1e-4)?;
    let origin = w.start + n * side;
    let (t0, t1) = ((w.start - a).dot(e), (w.end - a).dot(e));
    let (lo, hi) = (t0.min(t1).max(0.0), t0.max(t1).min(len));
    (hi - lo > MIN_PIECE).then_some((Face { wall: w, origin, dir: d }, lo, hi))
}

/// Remove `cuts` from the interval `span`.
fn subtract(span: (f64, f64), cuts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut cur = vec![span];
    for &(c0, c1) in cuts {
        let mut next = Vec::new();
        for (lo, hi) in cur {
            if c1 <= lo || c0 >= hi {
                next.push((lo, hi));
                continue;
            }
            if c0 > lo {
                next.push((lo, c0));
            }
            if c1 < hi {
                next.push((c1, hi));
            }
        }
        cur = next;
    }
    cur
}

/// The counters `b` follows.
fn outlines_of<'a>(model: &ArchModel, b: &Backsplash, all: &'a [CounterOutline]) -> Vec<&'a CounterOutline> {
    match (b.counter, b.wall) {
        (Some(CounterRef::Cabinet(id)), _) => all.iter().filter(|o| o.cabinets.contains(&id)).take(1).collect(),
        (Some(CounterRef::Countertop(id)), _) => all.iter().filter(|o| o.key == Some(CounterKey::Custom(id))).collect(),
        (None, Some(w)) if model.wall(w).is_some() => all.iter().filter(|o| o.level == b.level).collect(),
        _ => Vec::new(),
    }
}

/// The ceiling height above the level floor at `p`: the room's when finite and positive, else the level's.
fn ceiling_at(model: &ArchModel, level: u32, p: Vec2) -> Option<f64> {
    let ok = |h: f64| h.is_finite() && h > 0.0;
    let room = crate::room::room_at(model, level, p).and_then(|id| model.room(id)).and_then(|r| r.ceiling_height).filter(|h| ok(*h));
    room.or_else(|| model.level(level).map(|l| l.ceiling_height).filter(|h| ok(*h)))
}

/// The segments of `b`; the thickness goes from the wall face into the room.
///
/// Absolute z. Counters: the counter `b.counter` names, or every counter on `b.level` with an edge
/// on a face of `b.wall`. Only straight edges lying on a wall face get a piece; the stretches
/// behind cabinets with `backsplash_off` are left out. Each piece runs from the counter top to the
/// bottom of a wall cabinet over it, else to the counter top plus `b.height` (or the ceiling).
pub fn segments(model: &ArchModel, b: &Backsplash) -> Vec<BacksplashSegment> {
    let mut out = Vec::new();
    if !b.height.is_finite() || b.height <= 0.0 {
        return out;
    }
    let all = countertop::outlines(model);
    for o in outlines_of(model, b, &all) {
        let floor = model.level(o.level).map_or(0.0, |l| l.elevation);
        if !floor.is_finite() || !o.top.is_finite() {
            continue;
        }
        let bottom = floor + o.top;
        let n = o.ring.len();
        for i in 0..n {
            let (Some(&pa), Some(&pb)) = (o.ring.get(i), o.ring.get((i + 1) % n)) else { continue };
            if o.sagittas.get(i).copied().unwrap_or(0.0).abs() > 1e-12 {
                continue;
            }
            let len = pa.dist(pb);
            if !len.is_finite() || len < MIN_PIECE {
                continue;
            }
            let e = (pb - pa) * (1.0 / len);
            for w in model.walls.iter().filter(|w| w.base_level == o.level && !w.divider && b.wall.is_none_or(|id| id == w.id)) {
                let Some((face, lo, hi)) = face_of(w, pa, e, len) else { continue };
                // Stretches behind cabinets with the backsplash off.
                let cuts: Vec<(f64, f64)> = o
                    .cabinets
                    .iter()
                    .filter_map(|id| model.cabinets.iter().find(|c| c.id == *id))
                    .filter(|c| c.backsplash_off && c.kind != CabinetKind::Wall)
                    .filter_map(|c| back_interval(c, pa, e, (face.origin, face.dir), 1e-4))
                    .collect();
                for span in subtract((lo, hi), &cuts) {
                    for (t0, t1, top) in tops(model, b, o, &face, (pa, e), span, (bottom, floor)) {
                        if out.len() >= MAX_SEGMENTS {
                            return out;
                        }
                        if t1 - t0 < MIN_PIECE || !top.is_finite() || top <= bottom {
                            continue;
                        }
                        let on = |t: f64| {
                            let p = pa + e * t;
                            let nrm = face.dir.perp();
                            p - nrm * (p - face.origin).dot(nrm)
                        };
                        out.push(BacksplashSegment { a: on(t0), b: on(t1), wall: face.wall.id, bottom, top });
                    }
                }
            }
        }
    }
    out
}

/// Split `span` of an edge into `(from, to, top)` pieces; `base` is the counter top and the level floor.
fn tops(
    model: &ArchModel,
    b: &Backsplash,
    o: &CounterOutline,
    face: &Face<'_>,
    (pa, e): (Vec2, Vec2),
    (s0, s1): (f64, f64),
    (bottom, floor): (f64, f64),
) -> Vec<(f64, f64, f64)> {
    if b.to_ceiling {
        let mid = pa + e * ((s0 + s1) / 2.0);
        let probe = mid + e.perp() * ROOM_PROBE;
        return match ceiling_at(model, o.level, probe) {
            Some(h) => vec![(s0, s1, floor + h)],
            None => Vec::new(),
        };
    }
    // (from, to, bottom) of the wall cabinets hung over the counter.
    let over: Vec<(f64, f64, f64)> = model
        .cabinets
        .iter()
        .filter(|c| c.kind == CabinetKind::Wall && c.level == o.level && c.elevation.is_finite())
        .filter(|c| floor + c.elevation >= bottom + MIN_CLEARANCE)
        .filter_map(|c| back_interval(c, pa, e, (face.origin, face.dir), 1e-6).map(|(a, z)| (a, z, floor + c.elevation)))
        .filter(|(a, z, _)| *z > s0 && *a < s1)
        .collect();
    let mut cuts: Vec<f64> = vec![s0, s1];
    cuts.extend(over.iter().flat_map(|(a, z, _)| [*a, *z]).filter(|t| *t > s0 && *t < s1));
    cuts.sort_by(|x, y| x.total_cmp(y));
    cuts.dedup();
    let mut pieces: Vec<(f64, f64, f64)> = Vec::new();
    for w in cuts.windows(2) {
        let (Some(&t0), Some(&t1)) = (w.first(), w.get(1)) else { continue };
        let mid = (t0 + t1) / 2.0;
        let under = over.iter().filter(|(a, z, _)| *a <= mid && mid <= *z).map(|(_, _, bot)| *bot).fold(f64::INFINITY, f64::min);
        let top = if under.is_finite() { under } else { bottom + b.height };
        match pieces.last_mut() {
            Some(last) if (last.2 - top).abs() < 1e-9 && (last.1 - t0).abs() < 1e-9 => last.1 = t1,
            _ => pieces.push((t0, t1, top)),
        }
    }
    pieces
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use cadcraft_doc::arch::{Countertop, Level, Room};

    /// Wall along y = -0.1 (thickness 0.2): its north face is y = 0.
    fn base_model() -> ArchModel {
        let mut m = ArchModel::default();
        m.levels.push(Level::default());
        m.push_wall(Wall::new(Vec2::new(0.0, -0.1), Vec2::new(6.0, -0.1), 0.2));
        m
    }

    fn cab(kind: CabinetKind, x0: f64, elevation: f64) -> Cabinet {
        let height = if kind == CabinetKind::Base { 0.876 } else { 0.7 };
        Cabinet {
            kind,
            position: Vec2::new(x0 + 0.3, 0.0),
            width: 0.6,
            depth: if kind == CabinetKind::Base { 0.6 } else { 0.3 },
            height,
            elevation,
            counter: kind == CabinetKind::Base,
            counter_thickness: 0.038,
            counter_overhang: 0.025,
            ..Default::default()
        }
    }

    /// Three base cabinets (x 0.6..2.4, cb0..cb2) and optionally one wall cabinet (cb3) over x 0.6..1.2.
    fn kitchen(wall_cab: bool) -> ArchModel {
        let mut m = base_model();
        for i in 0..3 {
            m.push_cabinet(cab(CabinetKind::Base, 0.6 + 0.6 * f64::from(i), 0.0));
        }
        if wall_cab {
            m.push_cabinet(cab(CabinetKind::Wall, 0.6, 1.372));
        }
        m
    }

    fn run_bs() -> Backsplash {
        Backsplash { counter: Some(CounterRef::Cabinet(ArchId(0))), ..Default::default() }
    }

    fn tops_at(segs: &[BacksplashSegment], x: f64) -> Vec<f64> {
        segs.iter().filter(|s| s.a.x.min(s.b.x) - 1e-9 <= x && x <= s.a.x.max(s.b.x) + 1e-9).map(|s| s.top).collect()
    }

    #[test]
    fn segments_under_wall_cabinets_stop_at_their_bottom() {
        let m = kitchen(true);
        let segs = segments(&m, &run_bs());
        assert_eq!(segs.len(), 2, "{segs:?}");
        assert_eq!(tops_at(&segs, 0.9), vec![1.372]);
        assert_eq!(tops_at(&segs, 1.5), vec![1.364]);
        for s in &segs {
            assert!((s.bottom - 0.914).abs() < 1e-9 && s.a.y.abs() < 1e-9 && s.b.y.abs() < 1e-9 && s.wall == ArchId(0));
            assert!(s.b.x > s.a.x);
        }
        assert!((segs[0].a.x - 0.6).abs() < 1e-9 && (segs[0].b.x - 1.2).abs() < 1e-9);
        assert!((segs[1].a.x - 1.2).abs() < 1e-9 && (segs[1].b.x - 2.4).abs() < 1e-9);
    }

    #[test]
    fn segments_elsewhere_counter_top_plus_height() {
        let m = kitchen(false);
        let segs = segments(&m, &run_bs());
        assert_eq!(segs.len(), 1);
        assert!((segs[0].top - 1.364).abs() < 1e-9);
        let mut b = run_bs();
        b.height = 0.3;
        assert!((segments(&m, &b)[0].top - 1.214).abs() < 1e-9);
    }

    #[test]
    fn to_ceiling_tops() {
        let mut m = kitchen(true);
        let mut b = run_bs();
        b.to_ceiling = true;
        let segs = segments(&m, &b);
        assert_eq!(segs.len(), 1);
        assert!((segs[0].top - 2.4).abs() < 1e-9);
        // A room's own ceiling height wins when finite and positive.
        m.push_room(Room {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 3.0), Vec2::new(0.0, 3.0)],
            ceiling_height: Some(2.7),
            ..Default::default()
        });
        assert!((segments(&m, &b)[0].top - 2.7).abs() < 1e-9);
        m.rooms[0].ceiling_height = Some(f64::NAN);
        assert!((segments(&m, &b)[0].top - 2.4).abs() < 1e-9);
        // Level elevation is added.
        m.levels[0].elevation = 3.0;
        assert!((segments(&m, &b)[0].top - 5.4).abs() < 1e-9);
    }

    #[test]
    fn cabinets_with_backsplash_off_are_skipped() {
        let mut m = kitchen(false);
        m.cabinets[2].backsplash_off = true;
        let segs = segments(&m, &run_bs());
        assert_eq!(segs.len(), 1);
        assert!((segs[0].a.x - 0.6).abs() < 1e-9 && (segs[0].b.x - 1.8).abs() < 1e-9);
        m.cabinets[0].backsplash_off = true;
        let segs = segments(&m, &run_bs());
        assert!((segs[0].a.x - 1.2).abs() < 1e-9 && (segs[0].b.x - 1.8).abs() < 1e-9);
        m.cabinets[1].backsplash_off = true;
        assert!(segments(&m, &run_bs()).is_empty());
    }

    #[test]
    fn wall_mode_takes_every_counter_on_that_face() {
        let mut m = kitchen(true);
        // A second run far along the same face.
        for i in 0..2 {
            m.push_cabinet(cab(CabinetKind::Base, 4.0 + 0.6 * f64::from(i), 0.0));
        }
        let b = Backsplash { wall: Some(ArchId(0)), height: 0.3, ..Default::default() };
        let segs = segments(&m, &b);
        assert_eq!(segs.len(), 3, "{segs:?}");
        assert_eq!(tops_at(&segs, 0.9), vec![1.372]);
        assert_eq!(tops_at(&segs, 2.0), vec![1.214]);
        assert_eq!(tops_at(&segs, 4.5), vec![1.214]);
        // Another wall's faces do not match.
        m.push_wall(Wall::new(Vec2::new(0.0, 5.0), Vec2::new(6.0, 5.0), 0.2));
        assert!(segments(&m, &Backsplash { wall: Some(ArchId(1)), ..Default::default() }).is_empty());
        // Neither counter nor wall: nothing.
        assert!(segments(&m, &Backsplash::default()).is_empty());
    }

    #[test]
    fn custom_countertop_back_edges() {
        let mut m = base_model();
        let ct = m.push_countertop(Countertop {
            polygon: vec![Vec2::new(1.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(3.0, 0.6), Vec2::new(1.0, 0.6)],
            top: 0.9,
            ..Default::default()
        });
        let b = Backsplash { counter: Some(CounterRef::Countertop(ct)), ..Default::default() };
        let segs = segments(&m, &b);
        assert_eq!(segs.len(), 1, "{segs:?}");
        assert!((segs[0].a.x - 1.0).abs() < 1e-9 && (segs[0].b.x - 3.0).abs() < 1e-9);
        assert!((segs[0].bottom - 0.9).abs() < 1e-9 && (segs[0].top - 1.35).abs() < 1e-9);
        // A clockwise polygon is reversed by the outline: the piece still runs counter-clockwise.
        m.countertops[0].polygon.reverse();
        let segs = segments(&m, &b);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].a.x < segs[0].b.x);
        // An unknown countertop gives none.
        assert!(segments(&m, &Backsplash { counter: Some(CounterRef::Countertop(ArchId(9))), ..Default::default() }).is_empty());
    }

    #[test]
    fn arc_and_free_edges_get_none() {
        let mut m = base_model();
        let ct = m.push_countertop(Countertop {
            polygon: vec![Vec2::new(1.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(3.0, 0.6), Vec2::new(1.0, 0.6)],
            ..Default::default()
        });
        let b = Backsplash { counter: Some(CounterRef::Countertop(ct)), ..Default::default() };
        // The back edge is an arc: no piece; the free edges (x = 1, 3, y = 0.6) never lie on a face.
        m.countertops[0].edges = vec![cadcraft_doc::arch::CounterEdge { sagitta: 0.05, ..Default::default() }];
        assert!(segments(&m, &b).is_empty());
        // A counter away from every wall has none.
        m.countertops[0].edges.clear();
        m.countertops[0].polygon = vec![Vec2::new(1.0, 2.0), Vec2::new(3.0, 2.0), Vec2::new(3.0, 2.6), Vec2::new(1.0, 2.6)];
        assert!(segments(&m, &b).is_empty());
        // A divider wall carries no backsplash.
        m.walls[0].divider = true;
        m.countertops[0].polygon = vec![Vec2::new(1.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(3.0, 0.6), Vec2::new(1.0, 0.6)];
        assert!(segments(&m, &b).is_empty());
    }

    #[test]
    fn low_wall_cabinet_ignored() {
        let mut m = kitchen(false);
        // Bottom 0.005 above the counter top: under the 0.01 clearance, so it does not cut the piece.
        m.push_cabinet(cab(CabinetKind::Wall, 0.6, 0.919));
        let segs = segments(&m, &run_bs());
        assert_eq!(segs.len(), 1);
        assert!((segs[0].top - 1.364).abs() < 1e-9);
        // 0.02 above: it counts.
        m.cabinets[3].elevation = 0.934;
        let segs = segments(&m, &run_bs());
        assert_eq!(tops_at(&segs, 0.9), vec![0.934]);
    }

    #[test]
    fn hostile_input_gives_no_segments() {
        let m = kitchen(true);
        for h in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
            let b = Backsplash { height: h, ..run_bs() };
            assert!(segments(&m, &b).is_empty());
        }
        // Non-finite cabinet and wall data never panics.
        let mut hostile = kitchen(true);
        hostile.cabinets[3].elevation = f64::NAN;
        hostile.cabinets[0].width = f64::NAN;
        hostile.walls[0].thickness = f64::NAN;
        let _ = segments(&hostile, &run_bs());
        let _ = segments(&hostile, &Backsplash { to_ceiling: true, ..run_bs() });
        let mut lvl = kitchen(false);
        lvl.levels[0].elevation = f64::NAN;
        assert!(segments(&lvl, &run_bs()).is_empty());
        lvl.levels.clear();
        assert!(segments(&lvl, &Backsplash { to_ceiling: true, ..run_bs() }).is_empty());
        // No model at all.
        assert!(segments(&ArchModel::default(), &run_bs()).is_empty());
    }
}
