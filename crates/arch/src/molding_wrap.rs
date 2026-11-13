//! Crown molding wrap geometry (phase 18).

use cadcraft_doc::arch::{ArchModel, Room, Soffit, Tray, TraySides};
use cadcraft_geom::Vec2;

/// The inset for vertical tray sides (20 mm).
const TRAY_SIDE: f64 = 0.02;

/// Returns the soffits in `model` that belong to `room`'s level and overlap it.
///
/// Returns a list of `(ring, carries_molding)` pairs.
pub(crate) fn room_soffits(model: &ArchModel, room: &Room) -> Vec<(Vec<Vec2>, bool)> {
    let mut soffits = Vec::new();
    for s in &model.soffits {
        if s.level != room.level {
            continue;
        }
        if soffits.len() >= 256 {
            break;
        }
        let Some(ring) = crate::ceiling::soffit::soffit_ring(model, s) else {
            continue;
        };
        if ring.len() < 3 {
            continue;
        }
        // Check if ring is inside or on room polygon
        let mut inside = false;
        for p in &ring {
            if point_in_poly(p, &room.polygon) || dist_to_poly_boundary(p, &room.polygon) < 1e-6 {
                inside = true;
                break;
            }
        }
        if !inside {
            if let Some(ip) = crate::ceiling::soffit::interior_point(&ring) {
                if point_in_poly(&ip, &room.polygon) {
                    inside = true;
                }
            }
        }
        if inside {
            soffits.push((ring, !s.no_moldings));
        }
    }
    soffits
}

/// Clips segment a->b against rings, returning intervals NOT covered by any ring.
pub(crate) fn clip_outside(a: Vec2, b: Vec2, rings: &[Vec<Vec2>]) -> Vec<(f64, f64)> {
    let mut intervals = Vec::new();
    if rings.is_empty() {
        intervals.push((0.0, a.dist(b)));
        return intervals;
    }

    let mut breaks = vec![0.0, a.dist(b)];
    let l = a.dist(b);
    if l <= 1e-9 {
        return intervals;
    }

    // Find all crossing points with ring edges
    for ring in rings {
        if ring.len() < 3 {
            continue;
        }
        let n = ring.len();
        for i in 0..n {
            let p1 = ring[i];
            let p2 = ring[(i + 1) % n];
            if let Some(t) = seg_seg_intersection_t(a, b, p1, p2) {
                if t > 1e-9 && t < l - 1e-9 {
                    breaks.push(t);
                }
            }
        }
    }

    breaks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    breaks.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    // Keep sub-intervals whose midpoint is outside all rings
    for w in breaks.windows(2) {
        let t0 = w[0];
        let t1 = w[1];
        if t1 - t0 < 1e-9 {
            continue;
        }
        let mid_t = (t0 + t1) * 0.5;
        let mid = a + (b - a) * (mid_t / l);

        let mut covered = false;
        for ring in rings {
            if ring.len() < 3 {
                continue;
            }
            // Check if mid is strictly inside ring
            if point_in_poly(&mid, ring) {
                covered = true;
                break;
            }
            // Check if mid is within 1e-6 of ring boundary
            if dist_to_poly_boundary(&mid, ring) < 1e-6 {
                covered = true;
                break;
            }
        }
        if !covered {
            intervals.push((t0, t1));
        }
    }
    intervals
}

/// Returns directed segments for the free sides of carrying soffits.
pub(crate) fn soffit_segments(room: &Room, soffits: &[(Vec<Vec2>, bool)]) -> Vec<(Vec2, Vec2)> {
    let mut segments = Vec::new();
    let carrying: Vec<&Vec<Vec2>> = soffits.iter().filter(|(_, c)| *c).map(|(r, _)| r).collect();

    for ring in &carrying {
        if ring.len() < 3 {
            continue;
        }
        let n = ring.len();
        // Walk in reverse (clockwise) order
        for i in (0..n).rev() {
            let a = ring[i];
            let b = ring[(i + n - 1) % n];

            // Midpoint of edge
            let mid = (a + b) * 0.5;

            // Must be strictly inside room polygon
            if !point_in_poly(&mid, &room.polygon) {
                continue;
            }
            // Must not be inside a room hole
            let in_hole = room.holes.iter().any(|h| point_in_poly(&mid, h));
            if in_hole {
                continue;
            }

            // Must not be inside or on any OTHER soffit ring
            let in_other = soffits.iter().any(|(other_ring, _)| {
                if other_ring == *ring {
                    return false;
                }
                point_in_poly(&mid, other_ring) || dist_to_poly_boundary(&mid, other_ring) < 1e-6
            });
            if in_other {
                continue;
            }

            segments.push((a, b));
        }
    }
    segments
}

/// Returns the crown molding paths for trays in `room`.
pub(crate) fn tray_paths(model: &ArchModel, room: &Room) -> Vec<(Vec<Vec2>, f64)> {
    let mut paths = Vec::new();
    let Some(base_z) = crate::ceiling::flat_ceiling_z(model, room) else {
        return paths;
    };
    let stack_thick = crate::ceiling::stack::thickness(&room.ceiling_stack);

    let mut trays: Vec<&Tray> = model.trays.iter().filter(|t| t.room == room.id && t.level == room.level).collect();
    trays.sort_by_key(|t| t.id);

    for t in trays {
        if paths.len() >= 64 {
            break;
        }
        let Ok(ring) = crate::ceiling::tray::top_ring_in(room, &model.trays, t) else {
            continue;
        };
        if ring.len() < 3 {
            continue;
        }

        let mut final_ring = ring;
        if t.sides == TraySides::Vertical {
            let regions = cadcraft_geom::skeleton::inset(&final_ring, &[], TRAY_SIDE);
            if let Ok(regions) = regions {
                if regions.len() == 1 {
                    let outer = &regions[0].outer;
                    if outer.len() >= 3 {
                        final_ring = outer.clone();
                    }
                }
            }
        }

        // Orient CCW
        if crate::join::polygon_area(&final_ring) < 0.0 {
            final_ring.reverse();
        }

        // Calculate Z
        let mut depth = t.depth;
        let mut parent_id = t.parent;
        let mut hops = 0;
        while let Some(pid) = parent_id {
            if hops >= 16 {
                break;
            }
            if let Some(parent) = model.trays.iter().find(|pt| pt.id == pid) {
                depth += parent.depth;
                parent_id = parent.parent;
                hops += 1;
            } else {
                break;
            }
        }

        let z = base_z + depth - stack_thick;
        paths.push((final_ring, z));
    }
    paths
}

/// Chains directed segments into paths.
pub(crate) fn chain(segments: &[(Vec2, Vec2)]) -> Vec<(Vec<Vec2>, bool)> {
    let mut used = vec![false; segments.len()];
    let mut chains = Vec::new();

    // Find starts: segments whose start is not another segment's end
    let mut starts = Vec::new();
    for (i, (s, _)) in segments.iter().enumerate() {
        let is_end_of_other = segments.iter().enumerate().any(|(j, (_, e))| i != j && !used[j] && s.dist(*e) < 1e-6);
        if !is_end_of_other {
            starts.push(i);
        }
    }

    // Process starts
    for start_idx in starts {
        if used[start_idx] {
            continue;
        }
        let mut path = Vec::new();
        let mut curr_idx = Some(start_idx);

        while let Some(idx) = curr_idx {
            if used[idx] {
                break;
            }
            used[idx] = true;
            let (s, e) = segments[idx];
            if path.is_empty() {
                path.push(s);
            }
            path.push(e);

            // Find next
            curr_idx = None;
            for (j, (ns, _)) in segments.iter().enumerate() {
                if !used[j] && ns.dist(e) < 1e-6 {
                    curr_idx = Some(j);
                    break;
                }
            }
        }

        if path.len() >= 2 {
            let closed = path.first().map(|p| p.dist(*path.last().unwrap_or(p)) < 1e-6).unwrap_or(false);
            if closed && path.len() > 1 {
                path.pop(); // Remove repeated point
            }
            chains.push((path, closed));
        }
    }

    // Process remaining (closed loops)
    for (i, _) in segments.iter().enumerate() {
        if used[i] {
            continue;
        }
        let mut path = Vec::new();
        let mut curr_idx = Some(i);

        while let Some(idx) = curr_idx {
            if used[idx] {
                break;
            }
            used[idx] = true;
            let (s, e) = segments[idx];
            if path.is_empty() {
                path.push(s);
            }
            path.push(e);

            curr_idx = None;
            for (j, (ns, _)) in segments.iter().enumerate() {
                if !used[j] && ns.dist(e) < 1e-6 {
                    curr_idx = Some(j);
                    break;
                }
            }
        }

        if path.len() >= 2 {
            let closed = path.first().map(|p| p.dist(*path.last().unwrap_or(p)) < 1e-6).unwrap_or(false);
            if closed && path.len() > 1 {
                path.pop();
            }
            chains.push((path, closed));
        }
    }

    chains
}

// Helper functions

fn point_in_poly(p: &Vec2, poly: &[Vec2]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let pi = poly[i];
        let pj = poly[j];
        if (pi.y > p.y) != (pj.y > p.y) {
            let intersect = (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y) + pi.x;
            if p.x < intersect {
                inside = !inside;
            }
        }
    }
    inside
}

fn dist_to_poly_boundary(p: &Vec2, poly: &[Vec2]) -> f64 {
    if poly.len() < 2 {
        return f64::INFINITY;
    }
    let mut min_d = f64::INFINITY;
    let n = poly.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let d = dist_to_segment(p, &poly[i], &poly[j]);
        if d < min_d {
            min_d = d;
        }
    }
    min_d
}

fn dist_to_segment(p: &Vec2, a: &Vec2, b: &Vec2) -> f64 {
    let ab = *b - *a;
    let ap = *p - *a;
    let len_sq = ab.dot(ab);
    if len_sq <= 1e-12 {
        return p.dist(*a);
    }
    let mut t = ap.dot(ab) / len_sq;
    t = t.clamp(0.0, 1.0);
    let proj = *a + ab * t;
    p.dist(proj)
}

fn seg_seg_intersection_t(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<f64> {
    let ab = b - a;
    let cd = d - c;
    let denom = ab.x * cd.y - ab.y * cd.x;
    if denom.abs() < 1e-12 {
        return None;
    }
    let ac = c - a;
    let t = (ac.x * cd.y - ac.y * cd.x) / denom;
    let u = (ac.x * ab.y - ac.y * ab.x) / denom;
    if t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0 { Some(t * ab.len()) } else { None }
}
