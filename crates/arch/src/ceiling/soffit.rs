//! Interior soffits (phase 16): boxes hung from the ceiling; ceiling items over one hang from
//! its underside.

use crate::ceiling;
use crate::poly;
use crate::wall;
use cadcraft_doc::arch::{ArchId, ArchModel, CabinetKind, MAX_SOFFITS, Soffit};
use cadcraft_geom::Vec2;

/// Snap tolerance for aligning rect sides to wall face lines.
pub const SNAP: f64 = 0.15;

/// Find an interior point of `ring`. Returns the area centroid when it lies inside the
/// polygon; otherwise returns the middle of the widest inside span on the horizontal
/// line through the bbox middle. `None` for fewer than 3 finite points.
pub fn interior_point(ring: &[Vec2]) -> Option<Vec2> {
    if ring.len() < 3 {
        return None;
    }
    for v in ring {
        if !v.is_finite() {
            return None;
        }
    }
    // Area centroid (shoelace formula).
    let n = ring.len();
    let mut area2 = 0.0_f64;
    let mut cx_sum = 0.0_f64;
    let mut cy_sum = 0.0_f64;
    for i in 0..n {
        let j = (i + 1) % n;
        let (xi, yi) = (ring[i].x, ring[i].y);
        let (xj, yj) = (ring[j].x, ring[j].y);
        let cross = xi * yj - xj * yi;
        area2 += cross;
        cx_sum += (xi + xj) * cross;
        cy_sum += (yi + yj) * cross;
    }
    let centroid = if area2.abs() > 1e-15 {
        Vec2::new(cx_sum / (6.0 * area2), cy_sum / (6.0 * area2))
    } else {
        return None;
    };
    if cadcraft_geom::point_in_polygon(ring, centroid) {
        return Some(centroid);
    }
    // Widest inside span on the horizontal line through the bbox middle-y.
    let ymin = ring.iter().map(|v| v.y).fold(f64::INFINITY, f64::min);
    let ymax = ring.iter().map(|v| v.y).fold(f64::NEG_INFINITY, f64::max);
    let midy = (ymin + ymax) / 2.0;
    let xmin = ring.iter().map(|v| v.x).fold(f64::INFINITY, f64::min);
    let xmax = ring.iter().map(|v| v.x).fold(f64::NEG_INFINITY, f64::max);
    // Collect x-coordinates where the line y=midy crosses the polygon edges.
    let mut xs: Vec<f64> = Vec::new();
    xs.push(xmin);
    xs.push(xmax);
    for i in 0..n {
        let j = (i + 1) % n;
        let (y1, y2) = (ring[i].y, ring[j].y);
        let (x1, x2) = (ring[i].x, ring[j].x);
        // Edge crosses the horizontal line y = midy (strictly between y1 and y2,
        // or midy equals one endpoint handled by the >= / <= below).
        if ((y1 <= midy && midy <= y2) || (y2 <= midy && midy <= y1)) && y1 != y2 {
            let t = (midy - y1) / (y2 - y1);
            xs.push(x1 + t * (x2 - x1));
        }
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // Sweep for the longest sub-span whose midpoint is inside the polygon.
    let mut best_len: f64 = 0.0;
    let mut best_mid = f64::NAN;
    for i in 0..xs.len().saturating_sub(1) {
        let a = xs[i];
        let b = xs[i + 1];
        if a >= xmin && b <= xmax {
            let mid = (a + b) / 2.0;
            if mid.is_finite() && cadcraft_geom::point_in_polygon(ring, Vec2::new(mid, midy)) {
                if b - a > best_len + 1e-12 {
                    best_len = b - a;
                    best_mid = mid;
                }
            }
        }
    }
    if best_len > 0.0 && best_mid.is_finite() { Some(Vec2::new(best_mid, midy)) } else { None }
}

/// The ring of `s`: when `over` is non-empty every id must be a wall cabinet on `s.level`
/// sharing one host wall and rotation; the ring is the convex hull box of the cabinets
/// against that wall. Otherwise `s.polygon` (trailing duplicate dropped, >= 3 finite,
/// reversed if its signed area is negative).
pub fn soffit_ring(model: &ArchModel, s: &Soffit) -> Option<Vec<Vec2>> {
    if !s.over.is_empty() {
        // Every id must resolve to a wall cabinet on this level, all on the same wall
        // with the same rotation.
        let mut host_wid: Option<ArchId> = None;
        let mut rotations: Option<f64> = None;
        for cid in &s.over {
            let cab = model.cabinets.iter().find(|c| c.id == *cid)?;
            if cab.kind != CabinetKind::Wall {
                return None;
            }
            if cab.level != s.level {
                return None;
            }
            let (host_id, _, _) = crate::casework::host_wall(model, cab)?;
            if let Some(wid) = host_wid {
                if wid != host_id {
                    return None;
                }
            } else {
                host_wid = Some(host_id);
            }
            if let Some(r0) = rotations {
                if (cab.rotation - r0).abs() > 1e-6 {
                    return None;
                }
            } else {
                rotations = Some(cab.rotation);
            }
        }
        let w = model.walls.iter().find(|w| w.id == host_wid?)?;
        let dir = crate::wall::wall_dir(w)?;
        let left = dir.perp();
        // Compute each cabinet's along-wall position from its back-edge centre.
        let mut s_min = f64::INFINITY;
        let mut s_max = f64::NEG_INFINITY;
        let mut d_max = f64::NEG_INFINITY;
        for cid in &s.over {
            let cab = model.cabinets.iter().find(|c| c.id == *cid)?;
            let t = (cab.position - w.start).dot(dir);
            s_min = s_min.min(t - cab.width / 2.0);
            s_max = s_max.max(t + cab.width / 2.0);
            d_max = d_max.max(cab.depth);
        }
        let p = |t: f64, y: f64| -> Vec2 { w.start + dir * t + left * y };
        Some(vec![p(s_min, 0.0), p(s_max, 0.0), p(s_max, d_max), p(s_min, d_max)])
    } else {
        let mut ring = s.polygon.clone();
        // Drop trailing duplicate.
        if ring.len() > 1 && ring[0] == ring.last().unwrap_or(&ring[0]) {
            ring.pop();
        }
        if ring.len() < 3 {
            return None;
        }
        for v in &ring {
            if !v.is_finite() {
                return None;
            }
        }
        // Reverse if signed area is negative (ensure CCW).
        let mut area2 = 0.0_f64;
        for i in 0..ring.len() {
            let j = (i + 1) % ring.len();
            area2 += ring[i].x * ring[j].y - ring[j].x * ring[i].y;
        }
        if area2 < 0.0 {
            ring.reverse();
        }
        Some(ring)
    }
}

/// The bottom (absolute z) of `s`. Over-cabinets: level elevation + max(cabinet
/// elevation + height). `bottom`: level elevation + bottom. `depth`: min finished
/// ceiling (over ring vertices and the interior point) minus depth.
pub fn soffit_bottom(model: &ArchModel, s: &Soffit) -> Option<f64> {
    let level = model.levels.get(s.level)?.elevation;
    if !s.over.is_empty() {
        let mut z_max = f64::NEG_INFINITY;
        for cid in &s.over {
            if let Some(cab) = model.cabinets.iter().find(|c| &c.id == cid) {
                z_max = z_max.max(cab.elevation + cab.height);
            }
        }
        return level + z_max;
    }
    if let Some(bottom) = s.bottom {
        return level + bottom;
    }
    if let Some(depth) = s.depth {
        let ring = soffit_ring(model, s)?;
        let mut z_min = f64::INFINITY;
        for v in &ring {
            if let Some(ceil_z) = ceiling::finished_ceiling_at(model, s.level, *v) {
                z_min = z_min.min(ceil_z - depth);
            }
        }
        if let Some(ip) = interior_point(&ring) {
            if let Some(ceil_z) = ceiling::finished_ceiling_at(model, s.level, ip) {
                z_min = z_min.min(ceil_z - depth);
            }
        }
        if z_min.is_finite() {
            return Some(z_min);
        }
    }
    None
}

/// The bottom (absolute z) of the first soffit on `level` whose outline holds `p`,
/// else `None`.
pub fn bottom_at(model: &ArchModel, level: u32, p: Vec2) -> Option<f64> {
    for s in &model.soffits {
        if s.level != level {
            continue;
        }
        let ring = soffit_ring(model, s)?;
        if cadcraft_geom::point_in_polygon(&ring, p) {
            return soffit_bottom(model, s);
        }
    }
    None
}

/// Snap each side of `r = [x0, y0, x1, y1]` onto the nearest parallel face line of a
/// non-divider wall on `level` when within `SNAP`. A face line counts when it is
/// axis-parallel (within 1e-6) and its axial projection overlaps the side span
/// (overlap > 0). Ties go to the smaller wall id. A move that degenerates the rect
/// is skipped. Non-finite input returns `r`.
pub fn snap_rect(model: &ArchModel, level: u32, mut r: [f64; 4]) -> [f64; 4] {
    if !r.iter().all(|v| v.is_finite()) {
        return r;
    }
    // Collect all face lines (centre lines) for non-divider walls on `level`.
    // Each: (wall_id, axis, offset, lo, hi) where axis is 0 = horizontal (y = offset,
    // x in [lo, hi]) and 1 = vertical (x = offset, y in [lo, hi]).
    #[derive(Clone, PartialEq, PartialOrd)]
    struct FaceLine {
        wall_id: ArchId,
        axis: u8,
        offset: f64,
        lo: f64,
        hi: f64,
    }
    let lines: Vec<FaceLine> = model
        .walls
        .iter()
        .filter(|w| w.base_level == level)
        .filter(|w| !w.is_divider)
        .flat_map(|w| {
            crate::join::face_lines(w).into_iter().filter_map(|fl| {
                // Only axis-parallel lines.
                if fl.dx.is_finite() && fl.dy.is_finite() {
                    // Horizontal if |dy| < eps, vertical if |dx| < eps.
                    if fl.dy.abs() < 1e-6 {
                        Some((w.id, 0u8, fl.y, fl.x0, fl.x1))
                    } else if fl.dx.abs() < 1e-6 {
                        Some((w.id, 1u8, fl.x, fl.y0, fl.y1))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
        })
        .collect();

    // For each side of the rect, try to snap.
    // Side 0: bottom (y = r[1], x in [r[0], r[2]]) — horizontal
    // Side 1: top (y = r[3], x in [r[0], r[2]]) — horizontal
    // Side 2: left (x = r[0], y in [r[1], r[3]]) — vertical
    // Side 3: right (x = r[2], y in [r[1], r[3]]) — vertical

    // Snap bottom/top to horizontal face lines.
    for (idx, y_val) in [(1, r[1]), (3, r[3])] {
        if !lines.is_empty() {
            // Find the closest horizontal face line whose y is within SNAP of y_val
            // and whose x-range overlaps [r[0], r[2]].
            let mut best: Option<(f64, f64)> = None; // (distance, line.y)
            for fl in &lines {
                if fl.axis != 0 {
                    continue;
                }
                if !fl.offset.is_finite() || fl.lo > fl.hi {
                    continue;
                }
                // Overlap check: [fl.lo, fl.hi] and [r[0], r[2]] must overlap by > 0.
                let overlap = fl.hi.min(r[2]) - fl.lo.max(r[0]);
                if overlap <= 0.0 {
                    continue;
                }
                let d = (fl.offset - y_val).abs();
                if d <= SNAP {
                    match best {
                        None => best = Some((d, fl.offset)),
                        Some((bd, _)) => {
                            if d < bd - 1e-12 {
                                best = Some((d, fl.offset));
                            }
                        }
                    }
                }
            }
            if let Some((_, new_y)) = best {
                // Check that the move does not degenerate the rect.
                if (new_y - y_val).abs() > 0.0 && (new_y - r[3]).abs() > 1e-9 && (new_y - r[1]).abs() > 1e-9 {
                    r[idx] = new_y;
                }
            }
        }
    }

    // Snap left/right to vertical face lines.
    for (idx, x_val) in [(0, r[0]), (2, r[2])] {
        if !lines.is_empty() {
            let mut best: Option<(f64, f64)> = None;
            for fl in &lines {
                if fl.axis != 1 {
                    continue;
                }
                if !fl.offset.is_finite() || fl.lo > fl.hi {
                    continue;
                }
                // Overlap check: [fl.lo, fl.hi] (y-range) and [r[1], r[3]].
                let overlap = fl.hi.min(r[3]) - fl.lo.max(r[1]);
                if overlap <= 0.0 {
                    continue;
                }
                let d = (fl.offset - x_val).abs();
                if d <= SNAP {
                    match best {
                        None => best = Some((d, fl.offset)),
                        Some((bd, _)) => {
                            if d < bd - 1e-12 {
                                best = Some((d, fl.offset));
                            }
                        }
                    }
                }
            }
            if let Some((_, new_x)) = best {
                if (new_x - x_val).abs() > 0.0 && (new_x - r[2]).abs() > 1e-9 && (new_x - r[0]).abs() > 1e-9 {
                    r[idx] = new_x;
                }
            }
        }
    }

    r
}

/// Validate a soffit `s` against the model. Returns an error string when invalid.
pub fn validate(model: &ArchModel, s: &Soffit) -> Result<(), String> {
    // Exactly one of bottom, depth, non-empty over must be set.
    let has_bottom = s.bottom.is_some();
    let has_depth = s.depth.is_some();
    let has_over = !s.over.is_empty();
    let count = has_bottom as u8 + has_depth as u8 + has_over as u8;
    if count != 1 {
        return Err(format!("exactly one of bottom, depth or over must be set; got {}", count));
    }
    // Ring must be Some, >= 3 finite, simple, signed area >= 0.01.
    let ring = soffit_ring(model, s).ok_or("ring is invalid")?;
    if ring.len() < 3 {
        return Err("ring must have at least 3 points".into());
    }
    for v in &ring {
        if !v.is_finite() {
            return Err("ring contains a non-finite point".into());
        }
    }
    if !poly::is_simple(&ring) {
        return Err("ring is not simple".into());
    }
    // Signed area must be >= 0.01 (CCW).
    let mut area2 = 0.0_f64;
    for i in 0..ring.len() {
        let j = (i + 1) % ring.len();
        area2 += ring[i].x * ring[j].y - ring[j].x * ring[i].y;
    }
    let area = area2 / 2.0;
    if area < 0.01 {
        return Err("ring area too small".into());
    }
    // Interior point must be inside a room on the level that has a ceiling.
    let ip = interior_point(&ring).ok_or("no interior point")?;
    let room = ceiling::room_at_point(model, s.level, ip).ok_or("no room at interior point")?;
    let _ = ceiling::finished_ceiling_at(model, s.level, ip).ok_or("no finished ceiling at interior point")?;
    // Bottom must be finite, >= level elevation + 0.5, and >= 0.01 below finished ceiling
    // at every ring vertex that has one.
    if has_bottom || has_depth {
        let level = model.levels.get(s.level).ok_or("bad level")?.elevation;
        let bottom_z = soffit_bottom(model, s).ok_or("no bottom")?;
        if !bottom_z.is_finite() {
            return Err("bottom is non-finite".into());
        }
        if bottom_z < level + 0.5 {
            return Err("bottom too low (must be >= level elevation + 0.5)".into());
        }
        for v in &ring {
            if let Some(ceil_z) = ceiling::finished_ceiling_at(model, s.level, *v) {
                if bottom_z > ceil_z - 0.01 {
                    return Err("bottom too close to ceiling".into());
                }
            }
        }
    }
    if has_depth {
        let d = s.depth.unwrap();
        if !d.is_finite() || d < 0.01 || d > 3.0 {
            return Err("depth must be in [0.01, 3.0]").into();
        }
    }
    // Material must be None or a non-empty string of <= 128 chars.
    if let Some(ref mat) = s.material {
        if mat.is_empty() || mat.len() > 128 {
            return Err("material must be non-empty and <= 128 chars".into());
        }
    }
    // Capacity.
    if model.soffits.len() >= MAX_SOFFITS {
        return Err("max soffits reached".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::casework;
    use cadcraft_doc::arch::ArchModel;
    use cadcraft_geom::Vec2;

    fn house() -> ArchModel {
        let mut m = ArchModel::default();
        // Level 0: elevation 0, ceiling_height 2.4, structure 0.3.
        m.levels.push(cadcraft_doc::arch::Level::new(0, "Level 1", 0.0, 2.4, 0.3));
        // Walls (centre-line coordinates).
        let t = 0.2;
        // w0: south, y = t/2 = 0.1, x from 0.1 to 9.9
        m.walls.push(cadcraft_doc::arch::Wall::new(Vec2::new(0.1, 0.1), Vec2::new(9.9, 0.1), t, 2.4));
        // w1: east, x = 9.9, y from 0.1 to 7.9
        m.walls.push(cadcraft_doc::arch::Wall::new(Vec2::new(9.9, 0.1), Vec2::new(9.9, 7.9), t, 2.4));
        // w2: north, y = 7.9, x from 9.9 to 0.1
        m.walls.push(cadcraft_doc::arch::Wall::new(Vec2::new(9.9, 7.9), Vec2::new(0.1, 7.9), t, 2.4));
        // w3: west, x = 0.1, y from 7.9 to 0.1
        m.walls.push(cadcraft_doc::arch::Wall::new(Vec2::new(0.1, 7.9), Vec2::new(0.1, 0.1), t, 2.4));
        // w4: divider at y=3, x from 0 to 10
        m.walls.push(cadcraft_doc::arch::Wall::new(Vec2::new(0.0, 3.0), Vec2::new(10.0, 3.0), 0.2, 2.4));
        // w5: divider at x=6, y from 3 to 8, at offset 0.1
        m.walls.push(cadcraft_doc::arch::Wall::new(Vec2::new(6.0, 3.0), Vec2::new(6.0, 8.0), 0.2, 2.4));
        // Rooms
        // S: 0.1..9.9 x 0.1..2.95
        let mut room_poly = vec![Vec2::new(0.1, 0.1), Vec2::new(9.9, 0.1), Vec2::new(9.9, 2.95), Vec2::new(0.1, 2.95)];
        m.rooms.push(cadcraft_doc::arch::Room::new(cadcraft_doc::arch::RoomType::Living, 0, room_poly));
        // NW: 0.1..5.95 x 3.05..7.9
        let nw_poly = vec![Vec2::new(0.1, 3.05), Vec2::new(5.95, 3.05), Vec2::new(5.95, 7.9), Vec2::new(0.1, 7.9)];
        m.rooms.push(cadcraft_doc::arch::Room::new(cadcraft_doc::arch::RoomType::Other, 0, nw_poly));
        // NE: 6.05..9.9 x 3.05..7.9
        let ne_poly = vec![Vec2::new(6.05, 3.05), Vec2::new(9.9, 3.05), Vec2::new(9.9, 7.9), Vec2::new(6.05, 7.9)];
        m.rooms.push(cadcraft_doc::arch::Room::new(cadcraft_doc::arch::RoomType::Other, 0, ne_poly));
        m
    }

    fn make_cabinet(
        id: u32,
        wall_id: u32,
        kind: CabinetKind,
        position: Vec2,
        width: f64,
        depth: f64,
        height: f64,
        elevation: f64,
        level: u32,
    ) -> cadcraft_doc::arch::Cabinet {
        cadcraft_doc::arch::Cabinet {
            id,
            kind,
            wall: ArchId(wall_id),
            position,
            width,
            depth,
            height,
            elevation,
            level,
            rotation: 0.0,
            fronts: vec![],
        }
    }

    #[test]
    fn snap_rect_moves_sides_onto_wall_faces_and_into_corners() {
        let m = house();
        // Case 1: [9.0, 7.0, 9.96, 7.95] -> [9.0, 7.0, 9.9, 7.9]
        let r = snap_rect(&m, 0, [9.0, 7.0, 9.96, 7.95]);
        assert!((r[2] - 9.9).abs() < 1e-6, "x1: {} != 9.9", r[2]);
        assert!((r[3] - 7.9).abs() < 1e-6, "y1: {} != 7.9", r[3]);
        assert!((r[0] - 9.0).abs() < 1e-6, "x0 should not move");
        assert!((r[1] - 7.0).abs() < 1e-6, "y0 should not move");
        // Case 2: [6.5, 3.5, 7.3, 4.3] -> unchanged
        let r2 = snap_rect(&m, 0, [6.5, 3.5, 7.3, 4.3]);
        assert!((r2[0] - 6.5).abs() < 1e-9);
        assert!((r2[1] - 3.5).abs() < 1e-9);
        assert!((r2[2] - 7.3).abs() < 1e-9);
        assert!((r2[3] - 4.3).abs() < 1e-9);
    }

    #[test]
    fn bottom_from_depth_below_the_finished_ceiling() {
        let mut m = house();
        // depth 0.3 -> 2.1 (ceiling at 2.4, 2.4 - 0.3 = 2.1)
        let s_depth = Soffit {
            id: crate::arch::ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(1.0, 1.0), Vec2::new(5.0, 1.0), Vec2::new(5.0, 5.0), Vec2::new(1.0, 5.0)],
            bottom: None,
            depth: Some(0.3),
            over: vec![],
            material: None,
            layer: None,
        };
        let b = soffit_bottom(&m, &s_depth).unwrap();
        assert!((b - 2.1).abs() < 1e-6, "depth 0.3 -> {}, expected 2.1", b);
        // bottom 2.1 -> 2.1
        let s_bottom = Soffit {
            id: crate::arch::ArchId(1),
            level: 0,
            polygon: vec![Vec2::new(1.0, 1.0), Vec2::new(5.0, 1.0), Vec2::new(5.0, 5.0), Vec2::new(1.0, 5.0)],
            bottom: Some(2.1),
            depth: None,
            over: vec![],
            material: None,
            layer: None,
        };
        let b2 = soffit_bottom(&m, &s_bottom).unwrap();
        assert!((b2 - 2.1).abs() < 1e-6, "bottom 2.1 -> {}, expected 2.1", b2);
    }

    #[test]
    fn over_cabinets_ring_and_bottom() {
        let mut m = house();
        // Two wall cabinets on w2 (north wall, id=2) at offsets 1.0 and 1.6,
        // 0.6 x 0.305 x 0.762 at elevation 1.372.
        m.cabinets.push(make_cabinet(0, 2, CabinetKind::Wall, Vec2::new(9.0, 7.9), 0.6, 0.305, 0.762, 1.372, 0));
        m.cabinets.push(make_cabinet(1, 2, CabinetKind::Wall, Vec2::new(8.4, 7.9), 0.6, 0.305, 0.762, 1.372, 0));
        let s = Soffit {
            id: crate::arch::ArchId(0),
            level: 0,
            polygon: vec![],
            bottom: None,
            depth: None,
            over: vec![0, 1],
            material: None,
            layer: None,
        };
        let ring = soffit_ring(&m, &s).unwrap();
        assert_eq!(ring.len(), 4);
        // Ring x: 8.1..9.3, y: 7.595..7.9
        let xs: Vec<f64> = ring.iter().map(|v| v.x).collect();
        let ys: Vec<f64> = ring.iter().map(|v| v.y).collect();
        let xmin = xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let xmax = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let ymin = ys.iter().cloned().fold(f64::INFINITY, f64::min);
        let ymax = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!((xmin - 8.1).abs() < 1e-3, "xmin: {} != 8.1", xmin);
        assert!((xmax - 9.3).abs() < 1e-3, "xmax: {} != 9.3", xmax);
        assert!((ymin - 7.595).abs() < 1e-3, "ymin: {} != 7.595", ymin);
        assert!((ymax - 7.9).abs() < 1e-3, "ymax: {} != 7.9", ymax);
        let b = soffit_bottom(&m, &s).unwrap();
        assert!((b - 2.134).abs() < 1e-6, "bottom: {} != 2.134", b);
        // A base cabinet -> None
        m.cabinets.push(make_cabinet(2, 2, CabinetKind::Base, Vec2::new(9.0, 7.9), 0.6, 0.305, 0.762, 0.0, 0));
        let s_bad = Soffit { id: ArchId(0), level: 0, polygon: vec![], bottom: None, depth: None, over: vec![2], material: None, layer: None };
        assert!(soffit_ring(&m, &s_bad).is_none());
        // Other level -> None
        m.cabinets.push(make_cabinet(3, 2, CabinetKind::Wall, Vec2::new(9.0, 7.9), 0.6, 0.305, 0.762, 1.372, 1));
        let s_bad2 = Soffit { id: ArchId(0), level: 0, polygon: vec![], bottom: None, depth: None, over: vec![3], material: None, layer: None };
        assert!(soffit_ring(&m, &s_bad2).is_none());
        // Two different walls -> None
        m.cabinets.push(make_cabinet(4, 0, CabinetKind::Wall, Vec2::new(5.0, 0.1), 0.6, 0.305, 0.762, 1.372, 0));
        let s_bad3 = Soffit { id: ArchId(0), level: 0, polygon: vec![], bottom: None, depth: None, over: vec![0, 4], material: None, layer: None };
        assert!(soffit_ring(&m, &s_bad3).is_none());
    }

    #[test]
    fn bottom_at_inside_and_outside() {
        let mut m = house();
        // Soffit x 7.0..9.0, y 7.3..7.9, bottom 2.1 (level 0).
        m.soffits.push(Soffit {
            id: crate::arch::ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(7.0, 7.3), Vec2::new(9.0, 7.3), Vec2::new(9.0, 7.9), Vec2::new(7.0, 7.9)],
            bottom: Some(2.1),
            depth: None,
            over: vec![],
            material: None,
            layer: None,
        });
        // Inside: (8.0, 7.6) -> 2.1
        let b = bottom_at(&m, 0, Vec2::new(8.0, 7.6)).unwrap();
        assert!((b - 2.1).abs() < 1e-6, "inside: {} != 2.1", b);
        // Outside: (8.0, 7.2) -> None
        assert!(bottom_at(&m, 0, Vec2::new(8.0, 7.2)).is_none());
        // Other level -> None
        assert!(bottom_at(&m, 1, Vec2::new(8.0, 7.6)).is_none());
    }
}
