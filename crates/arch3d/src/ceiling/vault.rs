//! Vaulted ceilings (phase 16; owner: p16-vault-ceiling): a room's ceiling following the roof
//! underside over it.
//!
//! The level ceiling is split into convex cells along the outlines of the roof planes of the room's
//! level ([`planar::split_by_rings`]), and each cell takes the lowest of the planes it may follow
//! (the lower envelope, split where two planes cross): in a vaulted room the undersides of the roof
//! planes over the cell (the level ceiling where no roof covers it); in a flat room the level
//! ceiling and those undersides, so a flat ceiling a roof edit left above the underside follows the
//! underside there and stays level elsewhere. This is `cadcraft_arch::ceiling::vault::adjust`
//! cell by cell. A piece no cell of which changes comes back as it was, so a flat room under a
//! higher roof keeps phase 3's ceiling exactly. The cells come from the piece's own triangles, so
//! its holes (stair wells, recessed fixture cuts) stay open.

use cadcraft_geom::Vec2;

use super::CeilingIn;
use crate::planar::{self, Piece, PlaneZ};

/// How far a roof underside may dip below a flat ceiling before the ceiling follows it (m), as
/// `cadcraft_arch::ceiling::vault`.
const BELOW: f64 = 1e-9;
/// Most plane-against-plane splits one room's vault makes; past them a part keeps the plane it
/// has (bounds the work on a hostile model with many overlapping roof planes).
const MAX_SPLITS: usize = 200_000;

/// The level ceiling `pieces` of `i`'s room, following the roof underside where the room vaults.
pub(crate) fn apply(i: &CeilingIn, pieces: Vec<Piece>) -> Vec<Piece> {
    let planes: Vec<(Vec<Vec2>, PlaneZ)> = i
        .roofs
        .iter()
        .flat_map(|r| r.planes.iter())
        .filter(|pl| pl.ring.len() >= 3)
        .filter_map(|pl| Some((pl.ring.clone(), planar::underside_plane(pl)?)))
        .collect();
    if planes.is_empty() {
        return pieces;
    }
    let rings: Vec<Vec<Vec2>> = planes.iter().map(|(r, _)| r.clone()).collect();
    let vaulted = !i.room.flat_ceiling;
    let mut budget = MAX_SPLITS;
    let mut out = Vec::with_capacity(pieces.len());
    for piece in pieces {
        let cells: Vec<(Vec<Vec2>, Vec<PlaneZ>)> = planar::split_by_rings(&piece.outer, &piece.holes, &rings)
            .into_iter()
            .map(|(cell, holders)| {
                let over: Vec<PlaneZ> = holders.iter().filter_map(|&k| planes.get(k).map(|(_, pl)| *pl)).collect();
                (cell, over)
            })
            .collect();
        let keeps = |(cell, over): &(Vec<Vec2>, Vec<PlaneZ>)| {
            if vaulted { over.is_empty() } else { over.iter().all(|u| cell.iter().all(|v| u.z_at(*v) >= piece.plane.z_at(*v) - BELOW)) }
        };
        if cells.iter().all(keeps) {
            out.push(piece);
            continue;
        }
        for (cell, over) in cells {
            // At a tie the earlier plane wins: the level ceiling first in a flat room.
            let candidates: Vec<PlaneZ> = if vaulted && !over.is_empty() { over } else { std::iter::once(piece.plane).chain(over).collect() };
            for (part, plane) in lowest(cell, &candidates, &mut budget) {
                out.push(Piece::new(part, Vec::new(), plane));
            }
        }
    }
    out
}

/// Convex `cell` split into the parts where each of `planes` is the lowest: the lower envelope,
/// cut along the line where two planes cross. At a tie the earlier plane wins. A split that cannot
/// be made (a number that is not finite, or no `budget` left) keeps the earlier plane over the
/// whole part; each split made takes one from `budget`.
fn lowest(cell: Vec<Vec2>, planes: &[PlaneZ], budget: &mut usize) -> Vec<(Vec<Vec2>, PlaneZ)> {
    let Some((first, rest)) = planes.split_first() else {
        return Vec::new();
    };
    let mut parts = vec![(cell, *first)];
    for q in rest {
        let mut next = Vec::with_capacity(parts.len() + 1);
        for (part, p) in parts {
            // Where q - p < 0, q is lower.
            let diff = PlaneZ::new(q.a - p.a, q.b - p.b, q.c - p.c);
            if !diff.is_finite() || *budget == 0 {
                next.push((part, p));
                continue;
            }
            *budget -= 1;
            let (lower, kept) = planar::split_by_level(&part, diff, 0.0);
            if lower.is_empty() && kept.is_empty() {
                next.push((part, p));
                continue;
            }
            if !lower.is_empty() {
                next.push((lower, *q));
            }
            if !kept.is_empty() {
                next.push((kept, p));
            }
        }
        parts = next;
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::super::{base_pieces, ceiling_input, rough_pieces};
    use super::*;
    use cadcraft_doc::ArchModel;
    use cadcraft_doc::arch::{Level, Roof, RoofDirective, RoofPlane, Room, Wall};
    use cadcraft_geom::point_in_polygon;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    /// The phase-1 house: rooms S = rooms[0] (0.1..9.9 x 0.1..2.95), NW = rooms[1] (0.1..5.95 x
    /// 3.05..7.9) and NE = rooms[2] (6.05..9.9 x 3.05..7.9) on level 0 (ceiling 2.4), under a gable
    /// of `rise` over 12 with gables on x = 0 and x = 10 (no roof for `None`).
    fn house(rise: Option<f64>) -> ArchModel {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, name: "Level 1".into(), ..Level::default() });
        let pts = [(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)];
        for k in 0..4 {
            let (a, b) = (pts[k], pts[(k + 1) % 4]);
            let mut w = Wall::new(Vec2::new(a.0, a.1), Vec2::new(b.0, b.1), 0.2);
            w.height = 2.4;
            w.exterior = true;
            if k == 1 || k == 3 {
                w.roof_directive = RoofDirective::Gable;
            }
            m.push_wall(w);
        }
        for poly in [rect(0.1, 0.1, 9.9, 2.95), rect(0.1, 3.05, 5.95, 7.9), rect(6.05, 3.05, 9.9, 7.9)] {
            m.push_room(Room { polygon: poly, level: 0, ..Room::default() });
        }
        if let Some(rise) = rise {
            let seed = Roof { pitch_rise: rise, pitch_run: 12.0, overhang: 0.5, thickness: 0.2, ..Roof::default() };
            let roof = cadcraft_arch::roof::build_roof(&m, 0, &seed).expect("the gable builds");
            m.push_roof(roof);
        }
        m
    }

    fn input(m: &ArchModel, room: usize) -> CeilingIn {
        let level = m.levels.first().cloned().unwrap_or_default();
        ceiling_input(m, &m.rooms[room], &level, Vec::new())
    }

    /// The plane of the piece holding `p` (inside its outer ring and outside its holes).
    fn plane_at(pieces: &[Piece], p: Vec2) -> Option<PlaneZ> {
        pieces.iter().find(|q| point_in_polygon(&q.outer, p) && !q.holes.iter().any(|h| point_in_polygon(h, p))).map(|q| q.plane)
    }

    /// Every grid point of `room` (off the cell edges): the pieces' plane there equals the arch
    /// point query `rough_ceiling_in` to 1e-9.
    fn matches_the_point_query(m: &ArchModel, room: usize) -> usize {
        let pieces = rough_pieces(&input(m, room));
        let r = &m.rooms[room];
        let mut checked = 0;
        for i in 0..40 {
            for j in 0..40 {
                let p = Vec2::new(0.0137 + 0.2513 * i as f64, 0.0219 + 0.2027 * j as f64);
                if !point_in_polygon(&r.polygon, p) {
                    continue;
                }
                let want = cadcraft_arch::ceiling::rough_ceiling_in(m, r, p).expect("a ceiling");
                let got = plane_at(&pieces, p).unwrap_or_else(|| panic!("no piece over {p:?}")).z_at(p);
                assert!((got - want).abs() < 1e-9, "room {room} at {p:?}: pieces {got}, point query {want}");
                checked += 1;
            }
        }
        checked
    }

    #[test]
    fn vault_pieces_equal_the_point_query() {
        // Every room vaulted under the 6/12 and 8/12 gables (NW and NE straddle the ridge).
        for rise in [6.0, 8.0] {
            let mut m = house(Some(rise));
            for r in &mut m.rooms {
                r.flat_ceiling = false;
            }
            for room in 0..3 {
                assert!(matches_the_point_query(&m, room) > 50);
            }
        }
        let mut m = house(Some(6.0));
        m.rooms[0].flat_ceiling = false;
        let pieces = rough_pieces(&input(&m, 0));
        assert!((plane_at(&pieces, Vec2::new(5.0, 2.0)).unwrap().z_at(Vec2::new(5.0, 2.0)) - 3.45).abs() < 1e-9);
        assert!((plane_at(&pieces, Vec2::new(5.0, 0.5)).unwrap().z_at(Vec2::new(5.0, 0.5)) - 2.70).abs() < 1e-9);
    }

    #[test]
    fn clipped_flat_pieces_equal_the_point_query() {
        // A 2/12 gable under S's own 2.45 m ceiling: clipped along y < 0.2.
        let mut m = house(Some(2.0));
        m.rooms[0].ceiling_height = Some(2.45);
        assert!(matches_the_point_query(&m, 0) > 50);
        let pieces = rough_pieces(&input(&m, 0));
        assert!(pieces.iter().any(|p| !p.plane.is_flat()), "a sloped part along the south side");
        assert!((plane_at(&pieces, Vec2::new(5.0, 0.15)).unwrap().z_at(Vec2::new(5.0, 0.15)) - (2.4 + 0.25 / 6.0)).abs() < 1e-9);
        // The rooms whose ceiling stays under the roof keep the one level piece.
        for room in [1, 2] {
            let i = input(&m, room);
            assert_eq!(rough_pieces(&i), base_pieces(&i), "room {room}");
        }
    }

    #[test]
    fn overlapping_roof_planes_take_the_lowest() {
        // A second roof on the same level whose plane crosses the gable's south plane inside S:
        // the vault follows the lower of the two everywhere, the flat ceiling the lowest of all.
        for flat in [false, true] {
            let mut m = house(Some(6.0));
            m.rooms[0].flat_ceiling = flat;
            m.rooms[0].ceiling_height = Some(3.0);
            m.push_roof(Roof {
                planes: vec![RoofPlane {
                    ring: rect(2.0, -1.0, 12.0, 2.5),
                    slope: 0.25,
                    base_edge: [12.0, -1.0, 12.0, 2.5],
                    eave_z: 2.6,
                    ..RoofPlane::default()
                }],
                ..Roof::default()
            });
            assert!(matches_the_point_query(&m, 0) > 50, "flat {flat}");
        }
    }

    #[test]
    fn unchanged_rooms_keep_their_pieces() {
        // No roof, a roof on another level, or a flat ceiling below the roof: the level piece.
        let mut bare = house(None);
        bare.rooms[0].flat_ceiling = false;
        let i = input(&bare, 0);
        assert_eq!(apply(&i, base_pieces(&i)), base_pieces(&i));
        let mut other = house(Some(6.0));
        other.roofs[0].level = 1;
        other.rooms[0].flat_ceiling = false;
        let i = input(&other, 0);
        assert!(i.roofs.is_empty());
        assert_eq!(apply(&i, base_pieces(&i)), base_pieces(&i));
        let m = house(Some(6.0));
        for room in 0..3 {
            let i = input(&m, room);
            assert_eq!(apply(&i, base_pieces(&i)), base_pieces(&i), "room {room}");
        }
        // Nothing in, nothing out.
        let mut vault = house(Some(6.0));
        vault.rooms[0].flat_ceiling = false;
        assert!(apply(&input(&vault, 0), Vec::new()).is_empty());
    }

    #[test]
    fn vault_survives_hostile_roofs() {
        let mut m = house(Some(6.0));
        m.rooms[0].flat_ceiling = false;
        m.roofs[0].planes[0].eave_z = f64::NAN;
        m.roofs[0].planes[1].ring = vec![Vec2::new(f64::NAN, 0.0), Vec2::new(1e300, 0.0), Vec2::new(0.0, f64::INFINITY)];
        m.push_roof(Roof {
            planes: vec![
                RoofPlane { ring: rect(0.0, 0.0, 10.0, 8.0), slope: f64::INFINITY, base_edge: [0.0, 0.0, 10.0, 0.0], ..RoofPlane::default() },
                RoofPlane { ring: rect(0.0, 0.0, 10.0, 8.0), slope: 0.5, base_edge: [0.0, 0.0, 0.0, 0.0], ..RoofPlane::default() },
                RoofPlane {
                    ring: rect(-1e300, -1e300, 1e300, 1e300),
                    slope: 1e300,
                    base_edge: [0.0, 0.0, 1.0, 0.0],
                    eave_z: 1e300,
                    ..RoofPlane::default()
                },
                RoofPlane { ring: vec![Vec2::ZERO; 2], slope: 0.5, base_edge: [0.0, 0.0, 1.0, 0.0], ..RoofPlane::default() },
                RoofPlane {
                    ring: rect(1e300, 1e300, 1e300, 1e300),
                    slope: 0.5,
                    base_edge: [0.0, 0.0, 1.0, 0.0],
                    eave_z: 2.0,
                    ..RoofPlane::default()
                },
            ],
            ..Roof::default()
        });
        for flat in [false, true] {
            m.rooms[0].flat_ceiling = flat;
            let i = input(&m, 0);
            for p in apply(&i, base_pieces(&i)) {
                assert!(p.outer.iter().all(|v| v.is_finite()), "{p:?}");
            }
            let _ = super::super::ceiling_meshes(&i);
        }
        // A NaN room polygon builds nothing and does not panic.
        m.rooms[0].polygon = vec![Vec2::new(f64::NAN, 0.0); 4];
        let i = input(&m, 0);
        let _ = apply(&i, base_pieces(&i));
    }
}
