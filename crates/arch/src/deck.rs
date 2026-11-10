//! Deck geometry helpers: which edges of a deck touch a wall (the ledger edge), the site grade under
//! it, and the frame types the deck-frame segment fills.

use crate::{ArchId, ArchModel, Room, RoomType};
use cadcraft_geom::Vec2;

/// Thickness of a deck board (metres).
pub const DECK_BOARD: f64 = 0.025;
/// Width of a joist.
pub const DECK_JOIST_WIDTH: f64 = 0.038;
/// Depth of a joist.
pub const DECK_JOIST_DEPTH: f64 = 0.184;
/// Joist spacing, centre to centre.
pub const DECK_JOIST_SPACING: f64 = 0.406;
/// Width of a beam.
pub const DECK_BEAM_WIDTH: f64 = 0.076;
/// Depth of a beam.
pub const DECK_BEAM_DEPTH: f64 = 0.235;
/// How far a beam sits in from the free edge.
pub const DECK_BEAM_SETBACK: f64 = 0.3;
/// Side of a square post.
pub const DECK_POST_SIZE: f64 = 0.14;
/// How far a post sits in from the corner.
pub const DECK_POST_INSET: f64 = 0.3;
/// Largest post spacing along a beam.
pub const DECK_POST_SPACING: f64 = 2.4;
/// Diameter of a pier.
pub const DECK_PIER_DIAMETER: f64 = 0.3;
/// Depth of a pier below grade.
pub const DECK_PIER_DEPTH: f64 = 0.6;
/// How far an edge may lie from a wall face and still touch it.
pub const CONTACT_TOL: f64 = 0.02;
/// Shortest overlap that counts as contact.
pub const MIN_CONTACT: f64 = 0.05;

/// One edge of a deck polygon and the wall it touches.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeckEdge {
    /// Index of the polygon vertex the edge starts at.
    pub index: usize,
    /// Start of the edge.
    pub a: Vec2,
    /// End of the edge.
    pub b: Vec2,
    /// The wall the edge touches, if any.
    pub wall: Option<ArchId>,
    /// Length of the contact along the edge (0 when free).
    pub contact: f64,
}

/// The edges of `room`'s polygon (edge `i` runs from vertex `i` to `i + 1`, the closing edge last).
pub fn deck_edges(model: &ArchModel, room: &Room) -> Vec<DeckEdge> {
    let n = room.polygon.len();
    let level = model.level(room.level);
    let mut out = Vec::with_capacity(n.min(4096));
    for (index, a) in room.polygon.iter().enumerate().take(4096) {
        let Some(b) = room.polygon.get((index + 1) % n) else { break };
        let (a, b) = (*a, *b);
        let mut edge = DeckEdge { index, a, b, wall: None, contact: 0.0 };
        let d = b - a;
        let len = d.len();
        if a.is_finite() && b.is_finite() && len.is_finite() && len > 1e-9 {
            for w in &model.walls {
                if w.divider {
                    continue;
                }
                if !(w.base_level == room.level || level.is_some_and(|l| w.spans_level(l, &model.levels))) {
                    continue;
                }
                let Some(faces) = crate::join::face_lines(w) else { continue };
                for (fa, fb) in faces {
                    let fd = fb - fa;
                    let fl = fd.len();
                    if !(fl.is_finite() && fl > 1e-9) || d.cross(fd).abs() / (len * fl) > 1e-3 {
                        continue;
                    }
                    let dir = d / len;
                    let normal = dir.perp();
                    let (da, db) = ((fa - a).dot(normal).abs(), (fb - a).dot(normal).abs());
                    if da > CONTACT_TOL || db > CONTACT_TOL {
                        continue;
                    }
                    let (pa, pb) = ((fa - a).dot(dir), (fb - a).dot(dir));
                    let overlap = pa.max(pb).min(len) - pa.min(pb).max(0.0);
                    if overlap >= MIN_CONTACT && overlap > edge.contact {
                        edge.contact = overlap;
                        edge.wall = Some(w.id);
                    }
                }
            }
        }
        out.push(edge);
    }
    out
}

/// The edge that carries the ledger: the longest contact, ties to the lowest index.
pub fn ledger_edge(edges: &[DeckEdge]) -> Option<usize> {
    let mut best: Option<&DeckEdge> = None;
    for e in edges {
        if e.contact > 0.0 && best.is_none_or(|b| e.contact > b.contact) {
            best = Some(e);
        }
    }
    best.map(|e| e.index)
}

/// A plan centre line with absolute `z` extent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Member {
    /// Start of the centre line.
    pub a: Vec2,
    /// End of the centre line.
    pub b: Vec2,
    /// Plan width.
    pub width: f64,
    /// Absolute bottom z.
    pub bottom: f64,
    /// Absolute top z.
    pub top: f64,
}

/// A square post from grade to `top`, aligned with `axis`, on a pier at grade.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeckPost {
    /// Plan centre.
    pub at: Vec2,
    /// Alignment axis.
    pub axis: Vec2,
    /// Side length.
    pub size: f64,
    /// Ground elevation under the post.
    pub grade: f64,
    /// Absolute top z.
    pub top: f64,
}

/// The structure under a deck (filled by the deck-frame segment).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeckFrame {
    /// Absolute top of the boards.
    pub top: f64,
    /// Direction the boards run.
    pub board_dir: Vec2,
    /// The ledger against the house.
    pub ledger: Option<Member>,
    /// Joists.
    pub joists: Vec<Member>,
    /// Rim joists.
    pub rims: Vec<Member>,
    /// Beams.
    pub beams: Vec<Member>,
    /// Posts.
    pub posts: Vec<DeckPost>,
}

/// The terrain's modified surface at a point; 0 without terrain, outside it, or for a non-finite point.
pub fn site_grade(model: &ArchModel) -> impl Fn(Vec2) -> f64 + '_ {
    let surface = crate::terrain::surface::SiteSurface::new(model).ok();
    move |p: Vec2| {
        if !p.is_finite() {
            return 0.0;
        }
        surface.as_ref().and_then(|s| s.modified_at(p)).filter(|z| z.is_finite()).unwrap_or(0.0)
    }
}

/// Most joists a frame holds; a deck that would need more has no frame.
pub const MAX_JOISTS: usize = 2000;
/// Most posts a frame holds.
pub const MAX_POSTS: usize = 1000;
/// Most polygon vertices a frame is built from.
const MAX_VERTICES: usize = 4096;
/// Shortest post (top minus grade) that is built.
const MIN_POST: f64 = 0.05;
/// Smallest deck area that gets a frame.
const MIN_AREA: f64 = 0.1;

/// One chord of a line through a polygon: the interval `[lo, hi]` along the line and the index of the
/// polygon edge each end lies on.
struct Chord {
    lo: f64,
    lo_edge: usize,
    hi: f64,
    hi_edge: usize,
}

/// The chords of the line `u = c` inside the closed polygon `pts` (points are `(u, v)`): even-odd,
/// half-open at vertices, sorted along `v`. Chords shorter than 1e-9 are dropped.
fn chords(pts: &[(f64, f64)], c: f64) -> Vec<Chord> {
    let mut hits: Vec<(f64, usize)> = Vec::new();
    for (i, &(u0, v0)) in pts.iter().enumerate() {
        let Some(&(u1, v1)) = pts.get((i + 1) % pts.len()) else { continue };
        if (u0 > c) != (u1 > c) {
            let v = v0 + (c - u0) / (u1 - u0) * (v1 - v0);
            if v.is_finite() {
                hits.push((v, i));
            }
        }
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    hits.chunks_exact(2)
        .filter_map(|w| {
            let (lo, hi) = (w.first()?, w.get(1)?);
            Some(Chord { lo: lo.0, lo_edge: lo.1, hi: hi.0, hi_edge: hi.1 })
        })
        .filter(|c| c.hi - c.lo > 1e-9)
        .collect()
}

/// The frame under `room`: `None` unless it is a deck with a finite positive height, a polygon of at
/// least three finite points (at most 4096) and an area of 0.1 m2 or more, and not so large that it
/// needs more than [`MAX_JOISTS`] joists or [`MAX_POSTS`] posts. The grade under each post is `grade`.
pub fn frame(model: &ArchModel, room: &Room, grade: &dyn Fn(Vec2) -> f64) -> Option<DeckFrame> {
    if room.room_type != RoomType::Deck {
        return None;
    }
    let height = room.deck_height.filter(|h| h.is_finite() && *h > 0.0)?;
    let poly = &room.polygon;
    if poly.len() < 3 || poly.len() > MAX_VERTICES || !poly.iter().all(|p| p.is_finite()) {
        return None;
    }
    let twice: f64 = poly.iter().enumerate().map(|(i, p)| p.cross(poly.get((i + 1) % poly.len()).copied().unwrap_or(*p))).sum();
    if !twice.is_finite() || twice.abs() / 2.0 < MIN_AREA {
        return None;
    }
    let ccw = twice > 0.0;
    let top = model.level(room.level).map_or(0.0, |l| l.elevation) + height;
    if !top.is_finite() {
        return None;
    }
    let jt = top - DECK_BOARD;
    let jb = jt - DECK_JOIST_DEPTH;

    let edges = deck_edges(model, room);
    let ledger_ix = ledger_edge(&edges);
    let ledger_e = ledger_ix.and_then(|i| edges.get(i));
    // Axes: d along the ledger (or the longest edge), j into the deck.
    let (a, d, j) = if let Some(e) = ledger_e {
        let d = (e.b - e.a).normalized();
        (e.a, d, if ccw { d.perp() } else { -d.perp() })
    } else {
        let mut best: Option<&DeckEdge> = None;
        for e in &edges {
            if best.is_none_or(|b| (e.b - e.a).len() > (b.b - b.a).len()) {
                best = Some(e);
            }
        }
        let e = best?;
        let d = (e.b - e.a).normalized();
        (e.a, d, d.perp())
    };
    if !d.is_finite() || d.len() < 0.5 {
        return None;
    }
    let from_sr = |s: f64, r: f64| a + d * s + j * r;
    let sr: Vec<(f64, f64)> = poly.iter().map(|p| ((*p - a).dot(d), (*p - a).dot(j))).collect();
    let (mut s0, mut s1, mut r0, mut r1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(s, r) in &sr {
        s0 = s0.min(s);
        s1 = s1.max(s);
        r0 = r0.min(r);
        r1 = r1.max(r);
    }
    if !(s1 - s0).is_finite() || !(r1 - r0).is_finite() {
        return None;
    }
    let bays_f = ((s1 - s0) / DECK_JOIST_SPACING - 1e-9).ceil();
    if bays_f > (MAX_JOISTS + 1) as f64 {
        return None;
    }
    let bays = (bays_f as usize).max(1);
    // The same polygon with the roles of the axes swapped, for lines of constant r.
    let rs_pts: Vec<(f64, f64)> = sr.iter().map(|&(s, r)| (r, s)).collect();

    let member = |p: Vec2, q: Vec2, bottom: f64, top: f64, width: f64| Member { a: p, b: q, width, bottom, top };
    let half = DECK_JOIST_WIDTH / 2.0;

    // The ledger: the centre line half a joist inside the ledger edge, clipped to the polygon.
    let ledger = ledger_e.map(|e| {
        let len = (e.b - e.a).len();
        let mut best: Option<(f64, f64)> = None;
        for c in chords(&rs_pts, half) {
            let (lo, hi) = (c.lo.max(0.0), c.hi.min(len));
            if hi - lo > 1e-9 && best.is_none_or(|b| hi - lo > b.1 - b.0) {
                best = Some((lo, hi));
            }
        }
        let (lo, hi) = best.unwrap_or((0.0, len));
        member(from_sr(lo, half), from_sr(hi, half), jb, jt, DECK_JOIST_WIDTH)
    });

    // Joists run along j at every bay line; their ends stop at a ledger or a rim.
    let mut joists = Vec::new();
    let meets_member = |e: usize| Some(e) == ledger_ix || edges.get(e).is_some_and(|x| x.wall.is_none());
    'bays: for k in 1..bays {
        let s = s0 + k as f64 * (s1 - s0) / bays as f64;
        for c in chords(&sr, s) {
            let lo = if meets_member(c.lo_edge) { c.lo + DECK_JOIST_WIDTH } else { c.lo };
            let hi = if meets_member(c.hi_edge) { c.hi - DECK_JOIST_WIDTH } else { c.hi };
            if hi - lo <= 1e-9 {
                continue;
            }
            if joists.len() >= MAX_JOISTS {
                break 'bays;
            }
            joists.push(member(from_sr(s, lo), from_sr(s, hi), jb, jt, DECK_JOIST_WIDTH));
        }
    }

    // Rims on the free edges, half a joist inside.
    let mut rims = Vec::new();
    for e in edges.iter().filter(|e| e.wall.is_none()) {
        let v = e.b - e.a;
        let len = v.len();
        if !(len.is_finite() && len > 1e-9) {
            continue;
        }
        let n = (v / len).perp();
        let inward = if ccw { n * half } else { -n * half };
        rims.push(member(e.a + inward, e.b + inward, jb, jt, DECK_JOIST_WIDTH));
    }

    // Beams along d, with their posts.
    let lines: Vec<f64> = if ledger.is_some() {
        vec![r1 - DECK_BEAM_SETBACK]
    } else if r1 - r0 < 2.0 * DECK_BEAM_SETBACK + 1e-9 {
        vec![(r0 + r1) / 2.0]
    } else {
        vec![r0 + DECK_BEAM_SETBACK, r1 - DECK_BEAM_SETBACK]
    };
    let (bb, bt) = (jb - DECK_BEAM_DEPTH, jb);
    let mut beams = Vec::new();
    let mut posts = Vec::new();
    for r in lines {
        for c in chords(&rs_pts, r) {
            beams.push(member(from_sr(c.lo, r), from_sr(c.hi, r), bb, bt, DECK_BEAM_WIDTH));
            let len = c.hi - c.lo;
            let mut along = Vec::new();
            if len < 2.0 * DECK_POST_INSET + 1e-9 {
                along.push((c.lo + c.hi) / 2.0);
            } else {
                let m_f = ((len - 2.0 * DECK_POST_INSET) / DECK_POST_SPACING - 1e-9).ceil().max(1.0);
                if m_f > MAX_POSTS as f64 {
                    return None;
                }
                let m = m_f as usize;
                let (p0, p1) = (c.lo + DECK_POST_INSET, c.hi - DECK_POST_INSET);
                for i in 0..=m {
                    along.push(p0 + (p1 - p0) * i as f64 / m as f64);
                }
            }
            for s in along {
                let at = from_sr(s, r);
                let g = grade(at);
                if !g.is_finite() || bb - g < MIN_POST {
                    continue;
                }
                if posts.len() >= MAX_POSTS {
                    break;
                }
                posts.push(DeckPost { at, axis: d, size: DECK_POST_SIZE, grade: g, top: bb });
            }
        }
    }

    let board_dir = match room.deck_boards.filter(|b| b.is_finite()) {
        Some(deg) => {
            let t = deg.to_radians();
            Vec2::new(t.cos(), t.sin())
        }
        None => d,
    };
    Some(DeckFrame { top, board_dir, ledger, joists, rims, beams, posts })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::Wall;
    use cadcraft_doc::arch::Terrain;

    fn house() -> ArchModel {
        let mut m = ArchModel::default();
        m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        m.push_wall(Wall::new(Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), 0.2));
        m.push_wall(Wall::new(Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0), 0.2));
        m.push_wall(Wall::new(Vec2::new(0.0, 8.0), Vec2::new(0.0, 0.0), 0.2));
        m
    }

    fn north_deck() -> Room {
        Room {
            polygon: vec![Vec2::new(2.0, 8.1), Vec2::new(6.0, 8.1), Vec2::new(6.0, 11.1), Vec2::new(2.0, 11.1)],
            deck_height: Some(0.9),
            ..Room::default()
        }
    }

    #[test]
    fn deck_edges_find_the_north_wall() {
        let m = house();
        let edges = deck_edges(&m, &north_deck());
        assert_eq!(edges.len(), 4);
        assert_eq!(edges[0].wall, Some(ArchId(2)));
        assert!((edges[0].contact - 4.0).abs() < 1e-9);
        for e in &edges[1..] {
            assert_eq!((e.wall, e.contact), (None, 0.0));
        }
        assert_eq!(ledger_edge(&edges), Some(0));
    }

    #[test]
    fn ledger_edge_takes_the_longer_contact() {
        let e = |index, contact| DeckEdge { index, contact, ..DeckEdge::default() };
        assert_eq!(ledger_edge(&[e(0, 1.0), e(1, 3.0), e(2, 3.0)]), Some(1));
        assert_eq!(ledger_edge(&[e(0, 0.0), e(1, 0.5)]), Some(1));
        assert_eq!(ledger_edge(&[]), None);
    }

    #[test]
    fn a_free_standing_deck_has_no_ledger() {
        let m = house();
        let room =
            Room { polygon: vec![Vec2::new(20.0, 20.0), Vec2::new(24.0, 20.0), Vec2::new(24.0, 23.0), Vec2::new(20.0, 23.0)], ..Room::default() };
        let edges = deck_edges(&m, &room);
        assert_eq!(ledger_edge(&edges), None);
        assert!(deck_edges(&m, &Room::default()).is_empty());
    }

    #[test]
    fn site_grade_is_zero_without_terrain_and_follows_a_flat_terrain() {
        let mut m = house();
        assert_eq!(site_grade(&m)(Vec2::new(1.0, 1.0)), 0.0);
        let boundary = vec![Vec2::new(-50.0, -50.0), Vec2::new(50.0, -50.0), Vec2::new(50.0, 50.0), Vec2::new(-50.0, 50.0)];
        m.terrain = Some(Terrain { boundary, default_elevation: 1.5, pad: false, ..Terrain::default() });
        let g = site_grade(&m);
        assert!((g(Vec2::new(1.0, 1.0)) - 1.5).abs() < 1e-9);
        assert_eq!(g(Vec2::new(f64::NAN, 0.0)), 0.0);
        assert_eq!(g(Vec2::new(500.0, 0.0)), 0.0);
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn deck_with(m: &ArchModel, room: Room) -> DeckFrame {
        frame(m, &Room { room_type: RoomType::Deck, ..room }, &|_| 0.0).unwrap()
    }

    fn free_room() -> Room {
        Room {
            polygon: vec![Vec2::new(12.0, 0.0), Vec2::new(16.0, 0.0), Vec2::new(16.0, 3.0), Vec2::new(12.0, 3.0)],
            deck_height: Some(0.9),
            ..Room::default()
        }
    }

    #[test]
    fn frame_brief_numbers() {
        let m = house();
        let f = deck_with(&m, north_deck());
        assert!(near(f.top, 0.9));
        let l = f.ledger.as_ref().unwrap();
        assert!(near(l.a.y, 8.119) && near(l.b.y, 8.119) && near(l.a.x, 2.0) && near(l.b.x, 6.0));
        assert_eq!(f.joists.len(), 9);
        for (k, jo) in f.joists.iter().enumerate() {
            assert!(near(jo.a.x, 2.4 + 0.4 * k as f64) && near(jo.b.x, jo.a.x));
            assert!(near(jo.a.y, 8.138) && near(jo.b.y, 11.062), "{jo:?}");
            assert!(near(jo.top, 0.875) && near(jo.bottom, 0.691));
        }
        assert_eq!(f.rims.len(), 3);
        assert_eq!(f.beams.len(), 1);
        let b = &f.beams[0];
        assert!(near(b.a.y, 10.8) && near(b.a.x, 2.0) && near(b.b.x, 6.0));
        assert!(near(b.bottom, 0.456) && near(b.top, 0.691));
        assert_eq!(f.posts.len(), 3);
        for (p, want) in f.posts.iter().zip([2.3, 4.0, 5.7]) {
            assert!(near(p.at.x, want) && near(p.at.y, 10.8));
        }
        assert!(f.posts.iter().all(|p| near(p.top, 0.456) && p.grade == 0.0 && near(p.size, 0.14)));
        assert!(near(f.board_dir.x, 1.0) && near(f.board_dir.y, 0.0));
    }

    #[test]
    fn free_standing_deck_spans_its_short_side() {
        let f = deck_with(&house(), free_room());
        assert!(f.ledger.is_none());
        assert_eq!(f.joists.len(), 9);
        assert!(near(f.joists[0].a.x, 12.4) && near(f.joists[8].a.x, 15.6));
        assert!(near(f.joists[0].a.y, 0.038) && near(f.joists[0].b.y, 2.962));
        assert_eq!(f.rims.len(), 4);
        let mut ys: Vec<f64> = f.beams.iter().map(|b| b.a.y).collect();
        ys.sort_by(f64::total_cmp);
        assert!(ys.len() == 2 && near(ys[0], 0.3) && near(ys[1], 2.7));
        assert_eq!(f.posts.len(), 6);
    }

    #[test]
    fn board_direction_default_is_parallel_to_the_ledger_and_settable() {
        let m = house();
        let base = deck_with(&m, north_deck());
        let turned = deck_with(&m, Room { deck_boards: Some(90.0), ..north_deck() });
        assert!(near(turned.board_dir.x, 0.0) && near(turned.board_dir.y, 1.0));
        assert_eq!(turned.joists, base.joists);
        assert_eq!(turned.posts, base.posts);
        let bad = deck_with(&m, Room { deck_boards: Some(f64::NAN), ..north_deck() });
        assert!(near(bad.board_dir.x, 1.0));
    }

    #[test]
    fn grade_follows_the_terrain() {
        let m = house();
        let room = Room { room_type: RoomType::Deck, ..free_room() };
        let f = frame(&m, &room, &|_| -0.3).unwrap();
        assert_eq!(f.posts.len(), 6);
        assert!(f.posts.iter().all(|p| near(p.grade, -0.3)));
    }

    #[test]
    fn low_deck_skips_posts_shorter_than_0_05() {
        let m = house();
        // Beam bottom = 0.3 - 0.025 - 0.184 - 0.235 = -0.144; grade -0.1 leaves 0.044 < 0.05.
        let room = Room { room_type: RoomType::Deck, deck_height: Some(0.3), ..free_room() };
        let f = frame(&m, &room, &|_| -0.1).unwrap();
        assert!(f.posts.is_empty());
        assert_eq!(f.beams.len(), 2);
        assert_eq!(frame(&m, &room, &|_| -0.2).unwrap().posts.len(), 6);
    }

    #[test]
    fn l_shaped_deck_joists_follow_the_polygon() {
        let m = house();
        let poly = [(0.0, 0.0), (4.0, 0.0), (4.0, 1.0), (1.0, 1.0), (1.0, 3.0), (0.0, 3.0)];
        let room = Room { polygon: poly.iter().map(|p| Vec2::new(p.0 + 20.0, p.1 + 30.0)).collect(), deck_height: Some(0.9), ..Room::default() };
        let f = deck_with(&m, room);
        // Joists run along y at x 20.4 .. 23.6; the two inside the tall leg reach y = 3, the rest y = 1.
        assert_eq!(f.joists.len(), 9);
        let top = |j: &Member| j.a.y.max(j.b.y);
        assert_eq!(f.joists.iter().filter(|j| j.a.x < 21.0).count(), 2);
        assert!(f.joists.iter().filter(|j| j.a.x < 21.0).all(|j| near(top(j), 33.0 - 0.038)));
        assert!(f.joists.iter().filter(|j| j.a.x > 21.0).all(|j| near(top(j), 31.0 - 0.038)));
        assert_eq!(f.rims.len(), 6);
    }

    #[test]
    fn hostile_decks_never_panic_and_are_capped() {
        let m = house();
        let at = |poly: Vec<Vec2>, h: Option<f64>| Room { room_type: RoomType::Deck, polygon: poly, deck_height: h, ..Room::default() };
        let sq = |s: f64| vec![Vec2::new(0.0, 0.0), Vec2::new(s, 0.0), Vec2::new(s, s), Vec2::new(0.0, s)];
        assert!(frame(&m, &at(sq(2.0), None), &|_| 0.0).is_none());
        assert!(frame(&m, &at(sq(2.0), Some(f64::NAN)), &|_| 0.0).is_none());
        assert!(frame(&m, &at(sq(2.0), Some(-1.0)), &|_| 0.0).is_none());
        assert!(frame(&m, &at(sq(0.1), Some(0.5)), &|_| 0.0).is_none());
        assert!(frame(&m, &at(vec![Vec2::new(f64::NAN, 0.0); 4], Some(0.5)), &|_| 0.0).is_none());
        assert!(frame(&m, &at(vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0)], Some(0.5)), &|_| 0.0).is_none());
        assert!(frame(&m, &at(sq(1e300), Some(0.5)), &|_| 0.0).is_none());
        assert!(frame(&m, &at(sq(5000.0), Some(0.5)), &|_| 0.0).is_none());
        assert!(frame(&m, &at(vec![Vec2::new(0.0, 0.0); 10_000], Some(0.5)), &|_| 0.0).is_none());
        let big = frame(&m, &at(sq(800.0), Some(0.5)), &|_| 0.0).unwrap();
        assert!(big.joists.len() <= MAX_JOISTS && big.posts.len() <= MAX_POSTS);
        let not_deck = Room { polygon: sq(3.0), deck_height: Some(0.5), ..Room::default() };
        assert!(frame(&m, &not_deck, &|_| 0.0).is_none());
    }

    #[test]
    fn frame_stub_is_none() {
        // A room that is not a deck (the old stub's answer for everything) has no frame.
        let m = house();
        assert!(frame(&m, &north_deck(), &|_| 0.0).is_none());
    }
}
