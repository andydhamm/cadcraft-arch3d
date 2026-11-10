//! Deck geometry helpers: which edges of a deck touch a wall (the ledger edge), the site grade under
//! it, and the frame types the deck-frame segment fills.

use crate::{ArchId, ArchModel, Room};
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

/// The frame under `room`. Stub: none yet.
pub fn frame(_model: &ArchModel, _room: &Room, _grade: &dyn Fn(Vec2) -> f64) -> Option<DeckFrame> {
    None
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

    #[test]
    fn frame_stub_is_none() {
        let m = house();
        assert!(frame(&m, &north_deck(), &|_| 0.0).is_none());
    }
}
