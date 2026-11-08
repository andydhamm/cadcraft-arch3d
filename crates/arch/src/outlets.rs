//! Usable wall runs of a room (phase 15): continuous stretches of wall face where wall outlets
//! may go. Floor-reaching openings and, in a kitchen, the wall behind a counter break them.
//!
//! Pure and deterministic; hostile geometry yields fewer or no runs, never a panic.

use cadcraft_doc::arch::{RoomType, Side, Surface, WallSide};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::Vec2;

/// Runs shorter than this (metres) are dropped.
const MIN_RUN: f64 = 0.05;
/// Joins within a face (same wall and side).
const TOUCH_SAME: f64 = 1e-9;
/// Joins across a corner.
const TOUCH_CORNER: f64 = 1e-6;
/// How far a counter's back line may stand from a wall face and still count as against it.
const COUNTER_REACH: f64 = 0.15;

/// One continuous stretch of usable wall of a room, in order around the room.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Run {
    /// Consecutive wall faces, in order along the run; s is distance from each wall's start.
    pub wall_spans: Vec<(ArchId, WallSide, f64, f64)>,
    /// Total length along the run (sum of span lengths).
    pub length: f64,
}

/// A wall face of the room. `start` is on the face line at wall parameter 0.
struct Face {
    wall: ArchId,
    side: Side,
    start: Vec2,
    dir: Vec2,
    len: f64,
}

impl Face {
    fn point(&self, s: f64) -> Vec2 {
        self.start + self.dir * s
    }
}

/// A piece of a face, `s0 <= s1`, walked backwards when `rev`.
#[derive(Clone, Copy)]
struct Piece {
    face: usize,
    s0: f64,
    s1: f64,
    rev: bool,
}

impl Piece {
    fn begin(&self, faces: &[Face]) -> Option<Vec2> {
        faces.get(self.face).map(|f| f.point(if self.rev { self.s1 } else { self.s0 }))
    }
    fn finish(&self, faces: &[Face]) -> Option<Vec2> {
        faces.get(self.face).map(|f| f.point(if self.rev { self.s0 } else { self.s1 }))
    }
}

fn side_of(s: Side) -> WallSide {
    match s {
        Side::Left => WallSide::Left,
        Side::Right => WallSide::Right,
    }
}

fn dist(a: Vec2, b: Vec2) -> f64 {
    let d = a - b;
    d.x.hypot(d.y)
}

fn collect_faces(model: &ArchModel, room: ArchId, level: u32) -> Vec<Face> {
    let mut faces = Vec::new();
    for info in crate::paint::surfaces(model) {
        let Surface::WallSide { wall, side } = info.surface else { continue };
        if info.room != Some(room) || info.level != level {
            continue;
        }
        let Some(w) = model.wall(wall) else { continue };
        let d = w.end - w.start;
        let len = d.x.hypot(d.y);
        if !(len.is_finite() && len > 1e-9 && w.thickness.is_finite() && w.start.is_finite()) {
            continue;
        }
        let dir = d * (1.0 / len);
        let n = match side {
            Side::Left => dir.perp(),
            Side::Right => -dir.perp(),
        };
        faces.push(Face { wall, side, start: w.start + n * (w.thickness / 2.0), dir, len });
    }
    faces.sort_by_key(|f| (f.wall, f.side));
    faces.dedup_by_key(|f| (f.wall, f.side));
    faces
}

/// Subtract `cuts` from the ascending interval `piece`; the result is ascending.
fn subtract(piece: (f64, f64), cuts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = vec![piece];
    for &(c0, c1) in cuts {
        let mut next = Vec::with_capacity(out.len() + 1);
        for (a, b) in out {
            if c1 <= a || c0 >= b {
                next.push((a, b));
                continue;
            }
            if c0 > a {
                next.push((a, c0));
            }
            if c1 < b {
                next.push((c1, b));
            }
        }
        out = next;
    }
    out
}

/// Pieces of the faces in order around the room's outer polygon.
fn ordered_pieces(poly: &[Vec2], faces: &[Face]) -> Vec<Piece> {
    let mut matched = vec![false; faces.len()];
    let mut pieces: Vec<Piece> = Vec::new();
    for i in 0..poly.len() {
        let (Some(&p), Some(&q)) = (poly.get(i), poly.get((i + 1) % poly.len())) else { continue };
        if !(p.is_finite() && q.is_finite()) {
            continue;
        }
        let elen = dist(p, q);
        if elen <= 1e-9 {
            continue;
        }
        let ed = (q - p) * (1.0 / elen);
        let mut here: Vec<(f64, Piece)> = Vec::new();
        for (fi, f) in faces.iter().enumerate() {
            if ed.cross(f.dir).abs() > 1e-7 || ed.dot(f.dir).abs() < 1e-9 {
                continue;
            }
            if (p - f.start).cross(f.dir).abs() > 1e-6 {
                continue;
            }
            let sp = (p - f.start).dot(f.dir);
            let sq = (q - f.start).dot(f.dir);
            let (lo, hi) = (sp.min(sq).max(0.0), sp.max(sq).min(f.len));
            if hi - lo <= 1e-9 {
                continue;
            }
            let rev = sq < sp;
            let u = if rev { sp - hi } else { lo - sp };
            if let Some(m) = matched.get_mut(fi) {
                *m = true;
            }
            here.push((u, Piece { face: fi, s0: lo, s1: hi, rev }));
        }
        here.sort_by(|a, b| a.0.total_cmp(&b.0));
        pieces.extend(here.into_iter().map(|(_, p)| p));
    }
    for (fi, f) in faces.iter().enumerate() {
        if !matched.get(fi).copied().unwrap_or(true) {
            pieces.push(Piece { face: fi, s0: 0.0, s1: f.len, rev: false });
        }
    }
    // Consecutive pieces of one face that touch merge.
    let mut merged: Vec<Piece> = Vec::new();
    for p in pieces {
        if let Some(last) = merged.last_mut() {
            if last.face == p.face && last.rev == p.rev {
                if !p.rev && (p.s0 - last.s1).abs() < TOUCH_SAME {
                    last.s1 = p.s1;
                    continue;
                }
                if p.rev && (p.s1 - last.s0).abs() < TOUCH_SAME {
                    last.s0 = p.s0;
                    continue;
                }
            }
        }
        merged.push(p);
    }
    merged
}

/// Wall-parameter intervals of each face that outlets cannot use as a duplex wall.
fn cuts_per_face(model: &ArchModel, faces: &[Face], level: u32, kitchen: bool) -> Vec<Vec<(f64, f64)>> {
    let counters = if kitchen { crate::casework_layout::counter_runs(model).into_iter().filter(|c| c.level == level).collect() } else { Vec::new() };
    let mut cuts: Vec<Vec<(f64, f64)>> = vec![Vec::new(); faces.len()];
    for (f, c) in faces.iter().zip(cuts.iter_mut()) {
        for o in model.openings.iter().filter(|o| o.host == f.wall) {
            if !crate::electrical::floor_reaching_opening(o) || !(o.offset.is_finite() && o.width.is_finite()) {
                continue;
            }
            let h = o.width.abs() / 2.0;
            c.push((o.offset - h, o.offset + h));
        }
        for cr in &counters {
            let rad = cr.rotation.to_radians();
            let cd = Vec2::new(rad.cos(), rad.sin());
            let a = cr.origin + cd * cr.u0;
            let b = cr.origin + cd * cr.u1;
            if !(a.is_finite() && b.is_finite()) || cd.cross(f.dir).abs() > 1e-3 {
                continue;
            }
            if (a - f.start).cross(f.dir).abs() > COUNTER_REACH {
                continue;
            }
            let (sa, sb) = ((a - f.start).dot(f.dir), (b - f.start).dot(f.dir));
            c.push((sa.min(sb), sa.max(sb)));
        }
        c.sort_by(|x, y| x.0.total_cmp(&y.0));
    }
    cuts
}

/// Usable wall runs of `room`, in order around the room. Empty when the room is unknown.
pub fn usable_runs(model: &ArchModel, room: ArchId) -> Vec<Run> {
    let Some(r) = model.room(room) else { return Vec::new() };
    let faces = collect_faces(model, room, r.level);
    if faces.is_empty() {
        return Vec::new();
    }
    let merged = ordered_pieces(&r.polygon, &faces);
    let cuts = cuts_per_face(model, &faces, r.level, r.room_type == RoomType::Kitchen);

    let mut broken: Vec<Piece> = Vec::new();
    for p in merged {
        let cut = cuts.get(p.face).map(Vec::as_slice).unwrap_or(&[]);
        let mut parts = subtract((p.s0, p.s1), cut);
        if p.rev {
            parts.reverse();
        }
        broken.extend(parts.into_iter().filter(|(a, b)| b > a).map(|(s0, s1)| Piece { face: p.face, s0, s1, rev: p.rev }));
    }

    // Group touching pieces into runs.
    let mut groups: Vec<Vec<Piece>> = Vec::new();
    for p in broken {
        let joins = groups.last().and_then(|g| g.last()).is_some_and(|prev| {
            let (Some(e), Some(s)) = (prev.finish(&faces), p.begin(&faces)) else { return false };
            dist(e, s) < if prev.face == p.face { TOUCH_SAME } else { TOUCH_CORNER }
        });
        match groups.last_mut() {
            Some(g) if joins => g.push(p),
            _ => groups.push(vec![p]),
        }
    }

    let mut runs: Vec<Run> = groups
        .into_iter()
        .filter_map(|g| {
            let mut run = Run::default();
            for p in g {
                let f = faces.get(p.face)?;
                run.wall_spans.push((f.wall, side_of(f.side), p.s0, p.s1));
                run.length += p.s1 - p.s0;
            }
            (run.length > MIN_RUN).then_some(run)
        })
        .collect();
    let key = |r: &Run| r.wall_spans.first().map(|&(w, s, a, _)| (w, matches!(s, WallSide::Right), a)).unwrap_or((ArchId(0), false, 0.0));
    runs.sort_by(|a, b| {
        let (ka, kb) = (key(a), key(b));
        ka.0.cmp(&kb.0).then(ka.1.cmp(&kb.1)).then(ka.2.total_cmp(&kb.2))
    });
    runs
}

/// Candidates closer than this (metres along a wall face) are duplicates.
const DEDUPE: f64 = 0.3;
/// How far from a run's ends and internal breaks an outlet stays.
const EDGE: f64 = 0.05;
/// Most outlets one run or counter may get (guards hostile lengths).
const MAX_PER_RUN: usize = 4096;

fn device(
    kind: cadcraft_doc::arch::ElectricalKind,
    wall: ArchId,
    side: WallSide,
    offset: f64,
    counter: bool,
    level: u32,
) -> cadcraft_doc::arch::ElectricalDevice {
    cadcraft_doc::arch::ElectricalDevice { kind, wall, side, offset, height: if counter { 0.15 } else { 0.3 }, counter, level, ..Default::default() }
}

/// Duplex positions along a run of `length` metres.
fn duplex_positions(length: f64) -> Vec<f64> {
    if !(length.is_finite() && length > 0.0) {
        return Vec::new();
    }
    let n = ((length / 3.6).ceil() as usize).clamp(1, MAX_PER_RUN);
    (0..n).map(|i| (i as f64 + 0.5) * length / n as f64).collect()
}

/// Maps a position along `run` to `(wall, side, offset)`.
fn locate(run: &Run, pos: f64) -> Option<(ArchId, WallSide, f64)> {
    let mut cum = 0.0;
    let n = run.wall_spans.len();
    for (i, &(w, s, a, b)) in run.wall_spans.iter().enumerate() {
        let len = b - a;
        if pos <= cum + len || i + 1 == n {
            return Some((w, s, a + (pos - cum).clamp(0.0, len.max(0.0))));
        }
        cum += len;
    }
    None
}

/// Auto-placed outlets for `room` (ids left as `ArchId(0)`; the caller assigns them).
pub fn auto_outlets(model: &ArchModel, room: ArchId) -> Vec<cadcraft_doc::arch::ElectricalDevice> {
    use cadcraft_doc::arch::ElectricalKind;
    let Some(r) = model.room(room) else { return Vec::new() };
    let level = r.level;
    let kitchen = r.room_type == RoomType::Kitchen;
    let mut cands: Vec<cadcraft_doc::arch::ElectricalDevice> = Vec::new();

    if kitchen {
        let faces = collect_faces(model, room, level);
        for cr in crate::casework_layout::counter_runs(model).into_iter().filter(|c| c.level == level) {
            let rad = cr.rotation.to_radians();
            let cd = Vec2::new(rad.cos(), rad.sin());
            let (a, b) = (cr.origin + cd * cr.u0, cr.origin + cd * cr.u1);
            if !(a.is_finite() && b.is_finite()) {
                continue;
            }
            let Some(f) = faces.iter().find(|f| cd.cross(f.dir).abs() <= 1e-6 && (a - f.start).cross(f.dir).abs() <= 1e-6) else { continue };
            let (sa, sb) = ((a - f.start).dot(f.dir), (b - f.start).dot(f.dir));
            let (s0, s1) = (sa.min(sb), sa.max(sb));
            let mut pos = vec![s0 + 0.3];
            let mut p = s0 + 0.3;
            while pos.len() < MAX_PER_RUN && p + 1.2 <= s1 - 0.6 {
                p += 1.2;
                pos.push(p);
            }
            if s1 - 0.3 > p {
                pos.push(s1 - 0.3);
            }
            let cutouts: Vec<(f64, f64)> = cr
                .cabinets
                .iter()
                .filter_map(|id| model.cabinets.iter().find(|c| c.id == *id))
                .filter_map(|c| crate::casework_layout::hosted_cutout(model, c))
                .filter_map(|(_, at, co)| {
                    let c = (Vec2::new(at[0], at[1]) - f.start).dot(f.dir);
                    (c.is_finite() && co[0].is_finite()).then_some((c, co[0].abs()))
                })
                .collect();
            for mut p in pos {
                for &(c, w) in &cutouts {
                    if (p - c).abs() < w / 2.0 {
                        let moved = (c + w / 2.0 + EDGE).clamp(s0, s1);
                        p = if (moved - c).abs() < w / 2.0 { (c - w / 2.0 - EDGE).clamp(s0, s1) } else { moved };
                    }
                }
                cands.push(device(ElectricalKind::Gfci, f.wall, side_of(f.side), p, true, level));
            }
        }
    }

    for run in usable_runs(model, room) {
        let l = run.length;
        let mut breaks = Vec::new();
        let mut cum = 0.0;
        for &(_, _, a, b) in run.wall_spans.iter().take(run.wall_spans.len().saturating_sub(1)) {
            cum += b - a;
            breaks.push(cum);
        }
        let mut positions: Vec<f64> = duplex_positions(l)
            .into_iter()
            .map(|p| p.min(l - EDGE).max(EDGE))
            .map(|p| {
                let mut q = p;
                for &c in &breaks {
                    if (q - c).abs() < EDGE - 1e-12 {
                        q = if c < l / 2.0 { c + EDGE } else { c - EDGE };
                    }
                    q = q.min(l - EDGE).max(EDGE);
                }
                q
            })
            .collect();
        positions.sort_by(|a, b| a.total_cmp(b));
        let mut passes = 0u32;
        while passes < 8 && positions.len() < MAX_PER_RUN {
            let n = positions.len();
            let mut best_gap = f64::NEG_INFINITY;
            let mut best_pos = 0.0f64;
            if n > 0 {
                let g = positions[0] - 0.0;
                if g > 3.6 && g > best_gap {
                    best_gap = g;
                    best_pos = g / 2.0;
                }
            }
            for i in 0..n.saturating_sub(1) {
                let g = positions[i + 1] - positions[i];
                if g > 3.6 && g > best_gap {
                    best_gap = g;
                    best_pos = positions[i] + g / 2.0;
                }
            }
            if n > 0 {
                let g = l - positions[n - 1];
                if g > 3.6 && g > best_gap {
                    best_gap = g;
                    best_pos = positions[n - 1] + g / 2.0;
                }
            }
            if best_gap < 3.6 {
                break;
            }
            for &c in &breaks {
                if (best_pos - c).abs() < EDGE {
                    best_pos = if c < best_pos { c + EDGE } else { c - EDGE };
                }
            }
            best_pos = best_pos.min(l - EDGE).max(EDGE);
            positions.push(best_pos);
            positions.sort_by(|a, b| a.total_cmp(b));
            passes += 1;
        }
        for p in positions {
            if let Some((w, s, o)) = locate(&run, p) {
                cands.push(device(ElectricalKind::Duplex, w, s, o, false, level));
            }
        }
    }

    let mut out: Vec<cadcraft_doc::arch::ElectricalDevice> = Vec::new();
    for c in cands {
        let near = |d: &cadcraft_doc::arch::ElectricalDevice| d.wall == c.wall && d.side == c.side && (d.offset - c.offset).abs() <= DEDUPE;
        if model.electrical.iter().any(near) || out.iter().any(near) {
            continue;
        }
        if crate::electrical::place_device(model, &c).is_ok() {
            out.push(c);
        }
    }
    out.sort_by(|a, b| a.wall.cmp(&b.wall).then((a.side == WallSide::Right).cmp(&(b.side == WallSide::Right))).then(a.offset.total_cmp(&b.offset)));
    out
}

/// Counter-run outlet spacing found in a kitchen room.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CounterGaps {
    /// Largest gap between consecutive counter GFCI outlets (0 when a run has one).
    pub max_spacing: f64,
    /// Largest distance from a counter run's start to its first GFCI.
    pub start_distance: f64,
    /// Largest distance from a counter run's last GFCI to its end.
    pub end_distance: f64,
}

/// How well a room's usable walls are covered by outlets.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Gaps {
    /// Largest distance from a point on a usable run to the nearest serving outlet.
    pub max_gap: f64,
    /// Counter spacing; `Some` only for kitchens.
    pub counter: Option<CounterGaps>,
}

/// Sampling step along runs (metres).
const SAMPLE: f64 = 0.05;
/// Most samples taken along one run.
const MAX_SAMPLES: usize = 100_000;

fn fin(v: f64) -> f64 {
    if v.is_finite() { v } else { 0.0 }
}

/// Measures the outlet spacing of `room` with its existing devices plus what [`auto_outlets`] would add.
pub fn outlet_gaps(model: &ArchModel, room: ArchId) -> Gaps {
    use cadcraft_doc::arch::{ElectricalDevice, ElectricalKind};
    let Some(r) = model.room(room) else { return Gaps::default() };
    let level = r.level;
    let kitchen = r.room_type == RoomType::Kitchen;
    let mut served: Vec<ElectricalDevice> = model.electrical.iter().filter(|d| d.offset.is_finite()).cloned().collect();
    for d in auto_outlets(model, room) {
        let same = |e: &ElectricalDevice| e.wall == d.wall && e.side == d.side && (e.offset - d.offset).abs() <= 1e-9;
        if !served.iter().any(same) {
            served.push(d);
        }
    }

    let mut max_gap = 0.0f64;
    for run in usable_runs(model, room) {
        let mut at: Vec<f64> = Vec::new();
        let mut cum = 0.0;
        for &(w, s, a, b) in &run.wall_spans {
            at.extend(
                served.iter().filter(|d| d.wall == w && d.side == s && d.offset >= a - 1e-9 && d.offset <= b + 1e-9).map(|d| cum + d.offset - a),
            );
            cum += b - a;
        }
        let l = run.length;
        if !(l.is_finite() && l > SAMPLE) {
            continue;
        }
        if at.is_empty() {
            max_gap = f64::INFINITY;
            continue;
        }
        let n = ((l / SAMPLE).floor().min(MAX_SAMPLES as f64)) as usize;
        let step = if n == 0 { 0.0 } else { l / n as f64 };
        let mut worst = 0.0f64;
        for i in 0..=n {
            let p = i as f64 * step;
            worst = worst.max(at.iter().map(|q| (q - p).abs()).fold(f64::INFINITY, f64::min));
        }
        max_gap = max_gap.max(fin(worst));
    }

    let counter = kitchen.then(|| {
        let faces = collect_faces(model, room, level);
        let mut out = CounterGaps::default();
        for cr in crate::casework_layout::counter_runs(model).into_iter().filter(|c| c.level == level) {
            let rad = cr.rotation.to_radians();
            let cd = Vec2::new(rad.cos(), rad.sin());
            let (a, b) = (cr.origin + cd * cr.u0, cr.origin + cd * cr.u1);
            if !(a.is_finite() && b.is_finite()) {
                continue;
            }
            let Some(f) = faces.iter().find(|f| cd.cross(f.dir).abs() <= 1e-6 && (a - f.start).cross(f.dir).abs() <= 1e-6) else { continue };
            let (sa, sb) = ((a - f.start).dot(f.dir), (b - f.start).dot(f.dir));
            let (s0, s1) = (sa.min(sb), sa.max(sb));
            let mut g: Vec<f64> = served
                .iter()
                .filter(|d| {
                    d.kind == ElectricalKind::Gfci && d.wall == f.wall && d.side == side_of(f.side) && d.offset >= s0 - 1e-6 && d.offset <= s1 + 1e-6
                })
                .map(|d| d.offset)
                .collect();
            g.sort_by(|x, y| x.total_cmp(y));
            let (Some(&first), Some(&last)) = (g.first(), g.last()) else { continue };
            let spacing = g.windows(2).map(|w| w[1] - w[0]).fold(0.0, f64::max);
            out.max_spacing = out.max_spacing.max(fin(spacing));
            out.start_distance = out.start_distance.max(fin(first - s0));
            out.end_distance = out.end_distance.max(fin(s1 - last));
        }
        out
    });
    Gaps { max_gap, counter }
}
