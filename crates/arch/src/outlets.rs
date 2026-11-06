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
