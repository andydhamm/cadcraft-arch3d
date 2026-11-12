//! Tray ceilings (phase 16; owner: p16-trays): the parts of a room's ceiling recessed up into the
//! attic, and the sides that close them.

use std::collections::HashSet;

use cadcraft_arch::ceiling::tray::{lift_in, outline_in, top_ring_in};
use cadcraft_doc::arch::{Tray, TraySides};
use cadcraft_geom::Vec2;
use cadcraft_geom::skeleton::inset as inset_region;

use super::CeilingIn;
use crate::floors;
use crate::mesh::Mesh;
use crate::planar::{Piece, PlaneZ, split_by_rings};

fn ordered_trays<'a>(i: &'a CeilingIn) -> Vec<&'a Tray> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut active = HashSet::new();

    fn visit<'a>(t: &'a Tray, trays: &'a [Tray], out: &mut Vec<&'a Tray>, seen: &mut HashSet<u32>, active: &mut HashSet<u32>) -> bool {
        if seen.contains(&t.id.0) {
            return true;
        }
        if active.contains(&t.id.0) {
            return false;
        }
        active.insert(t.id.0);
        let ok = if let Some(parent) = t.parent {
            trays.iter().find(|p| p.id == parent).map_or(true, |p| visit(p, trays, out, seen, active))
        } else {
            true
        };
        active.remove(&t.id.0);
        if !ok {
            return false;
        }
        seen.insert(t.id.0);
        out.push(t);
        true
    }

    for tray in &i.trays {
        if !visit(tray, &i.trays, &mut out, &mut seen, &mut active) {
            continue;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchId, Level, Room, Tray, TraySides};
    use cadcraft_geom::Vec2;

    fn test_room() -> Room {
        Room {
            name: "Test".into(),
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0)],
            ..Default::default()
        }
    }

    fn test_level() -> Level {
        Level { index: 0, name: "Ground".into(), elevation: 0.0, ceiling_height: 2.4, ..Default::default() }
    }

    fn ceiling_in(trays: Vec<Tray>) -> CeilingIn {
        CeilingIn { room: test_room(), trays, level: test_level(), ..Default::default() }
    }

    #[test]
    fn ordered_trays_topsorts() {
        let parent = Tray { id: ArchId(0), parent: None, ..Default::default() };
        let child = Tray { id: ArchId(1), parent: Some(ArchId(0)), ..Default::default() };
        let grandchild = Tray { id: ArchId(2), parent: Some(ArchId(1)), ..Default::default() };
        let ci = ceiling_in(vec![grandchild, parent, child]);
        let ordered = ordered_trays(&ci);
        assert_eq!(ordered.len(), 3);
        assert_eq!(ordered[0].id, ArchId(0));
        assert_eq!(ordered[1].id, ArchId(1));
        assert_eq!(ordered[2].id, ArchId(2));
    }

    #[test]
    fn ordered_trays_breaks_cycles() {
        let a = Tray { id: ArchId(0), parent: Some(ArchId(1)), ..Default::default() };
        let b = Tray { id: ArchId(1), parent: Some(ArchId(0)), ..Default::default() };
        let ci = ceiling_in(vec![a, b]);
        let ordered = ordered_trays(&ci);
        // Should not loop; should include at most both trays.
        assert!(ordered.len() <= 2);
    }

    #[test]
    fn side_plane_computes_tilted_plane() {
        let piece = Piece::new(vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)], vec![], PlaneZ::flat(0.0));
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(1.0, 0.0);
        let plane = side_plane(&piece, a, b, 45.0).unwrap();
        // At 45 degrees, the plane should rise 1 unit for every 1 unit inward.
        assert!((plane.z_at(a) - 0.0).abs() < 1e-9);
        assert!((plane.z_at(b) - 0.0).abs() < 1e-9);
        // One unit inward (positive y) should be 1 unit higher.
        assert!((plane.z_at(Vec2::new(0.5, 1.0)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn side_plane_rejects_bad_angles() {
        let piece = Piece::new(vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)], vec![], PlaneZ::flat(0.0));
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(1.0, 0.0);
        assert!(side_plane(&piece, a, b, 0.0).is_none());
        assert!(side_plane(&piece, a, b, 90.0).is_none());
        assert!(side_plane(&piece, a, b, -1.0).is_none());
        assert!(side_plane(&piece, a, b, f64::NAN).is_none());
    }

    #[test]
    fn apply_raises_ceiling_inside_tray() {
        let tray = Tray { id: ArchId(0), room: ArchId(0), level: 0, inset: 0.6, depth: 0.3, ..Default::default() };
        let ci = ceiling_in(vec![tray]);
        // A single piece covering the whole room at z=2.4.
        let piece =
            Piece::new(vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0)], vec![], PlaneZ::flat(2.4));
        let result = apply(&ci, vec![piece]);
        // The result should have multiple pieces: one raised inside the tray, one at the original height outside.
        assert!(result.len() >= 2, "apply should split the piece: got {} pieces", result.len());
        // At least one piece should be at z=2.7 (raised by 0.3).
        let has_raised = result.iter().any(|p| (p.plane.c - 2.7).abs() < 1e-6);
        assert!(has_raised, "one piece should be raised to 2.7");
    }

    #[test]
    fn side_meshes_produces_mesh_for_vertical_tray() {
        let tray = Tray { id: ArchId(0), room: ArchId(0), level: 0, inset: 0.6, depth: 0.3, sides: TraySides::Vertical, ..Default::default() };
        let ci = ceiling_in(vec![tray]);
        let meshes = side_meshes(&ci, 0.02);
        assert!(!meshes.is_empty(), "side_meshes should produce at least one mesh");
    }
}

fn side_plane(piece: &Piece, a: Vec2, b: Vec2, deg: f64) -> Option<PlaneZ> {
    if !deg.is_finite() || !(0.0 < deg && deg < 90.0) {
        return None;
    }
    let d = b - a;
    let len = d.len();
    if !len.is_finite() || len <= 1e-12 || !a.is_finite() || !b.is_finite() {
        return None;
    }
    let tan = deg.to_radians().tan();
    if !tan.is_finite() {
        return None;
    }
    let nx = -d.y / len;
    let ny = d.x / len;
    let pa = piece.plane.z_at(a);
    let pb = piece.plane.z_at(b);
    if !(pa.is_finite() && pb.is_finite()) {
        return None;
    }
    let a0 = piece.plane.a + tan * nx;
    let b0 = piece.plane.b + tan * ny;
    let c = pa - (a0 * a.x + b0 * a.y);
    let plane = PlaneZ::new(a0, b0, c);
    if !plane.is_finite() {
        return None;
    }
    let eps = 1e-6;
    if (plane.z_at(a) - pa).abs() > eps || (plane.z_at(b) - pb).abs() > eps {
        return None;
    }
    Some(plane)
}

/// The ceiling `pieces` of `i`'s room, raised by its trays.
pub(crate) fn apply(i: &CeilingIn, pieces: Vec<Piece>) -> Vec<Piece> {
    let mut work = pieces;
    let ordered = ordered_trays(i);
    if ordered.iter().all(|t| outline_in(&i.room, &i.trays, t).is_err()) {
        return work;
    }

    for tray in ordered {
        let Ok(outline) = outline_in(&i.room, &i.trays, tray) else {
            continue;
        };
        let rings: Vec<Vec<Vec2>> = match tray.sides {
            TraySides::Vertical => vec![outline.clone()],
            TraySides::Pitched(deg) => {
                let Ok(top) = top_ring_in(&i.room, &i.trays, tray) else {
                    continue;
                };
                let mut out = Vec::with_capacity(outline.len() + 1);
                for k in 0..outline.len() {
                    let a = outline[k];
                    let b = outline[(k + 1) % outline.len()];
                    let c = top[(k + 1) % top.len()];
                    let d = top[k];
                    out.push(vec![a, b, c, d]);
                }
                out.push(top);
                out
            }
        };
        let top_idx = match tray.sides {
            TraySides::Pitched(_) => Some(rings.len().saturating_sub(1)),
            _ => None,
        };

        let mut next = Vec::with_capacity(work.len());
        for piece in work {
            let cells = split_by_rings(&piece.outer, &piece.holes, &rings);
            if cells.is_empty() {
                next.push(piece);
                continue;
            }
            let mut kept = false;
            for (cell, holders) in cells {
                let mut plane = piece.plane;
                let mut include = true;
                match tray.sides {
                    TraySides::Vertical => {
                        if holders.iter().any(|&k| k == 0) {
                            plane = piece.plane.offset(tray.depth);
                        }
                    }
                    TraySides::Pitched(deg) => {
                        if let Some(top_i) = top_idx {
                            if holders.iter().any(|&k| k == top_i) {
                                plane = piece.plane.offset(tray.depth);
                            } else if let Some(side_i) = holders.iter().copied().find(|&k| k != top_i) {
                                let edge_a = outline[side_i % outline.len()];
                                let edge_b = outline[(side_i + 1) % outline.len()];
                                match side_plane(&piece, edge_a, edge_b, deg) {
                                    Some(p) => plane = p,
                                    None => include = false,
                                }
                            }
                        }
                    }
                }
                if include {
                    next.push(Piece::new(cell, Vec::new(), plane));
                    kept = true;
                }
            }
            if !kept {
                next.push(piece);
            }
        }
        work = next;
    }
    work
}

/// The sides of `i`'s trays, down to the finished ceiling `drop` below the rough one.
pub(crate) fn side_meshes(i: &CeilingIn, drop: f64) -> Vec<Mesh> {
    let mut out = Vec::new();
    for tray in &i.trays {
        if !matches!(tray.sides, TraySides::Vertical) {
            continue;
        }
        let Ok(outline) = outline_in(&i.room, &i.trays, tray) else {
            continue;
        };
        let Ok(regions) = inset_region(&outline, &[], 0.02) else {
            continue;
        };
        let Some(inner) = regions.into_iter().filter(|r| r.holes.is_empty()).map(|r| r.outer).next() else {
            continue;
        };
        let p = outline.iter().fold(Vec2::ZERO, |acc, &v| acc + v) / outline.len() as f64;
        let ceiling_top = floors::ceiling_top(&i.room, &i.level).unwrap_or(f64::NAN);
        if !ceiling_top.is_finite() {
            continue;
        }
        let other: Vec<Tray> = i.trays.iter().filter(|t| t.id != tray.id).cloned().collect();
        let parent_ceiling = ceiling_top + lift_in(&i.room, &other, p);
        let tray_top = parent_ceiling + tray.depth;
        let z0 = parent_ceiling - drop - 0.02;
        let z1 = tray_top - drop - 0.02;
        if !(z0.is_finite() && z1.is_finite() && z1 > z0) {
            continue;
        }
        let mut m = Mesh::new("ceiling", i.room.id);
        m.level_elevation = i.level.elevation;
        m.push_prism_with_holes(&outline, &[inner], z0, z1);
        if !m.positions.is_empty() {
            out.push(m);
        }
    }
    out
}
