//! Interior soffits (phase 16): boxes hung from the ceiling; ceiling items over one hang from
//! its underside.

use crate::ceiling;
use crate::poly;
use crate::wall;
use cadcraft_doc::arch::{ArchId, ArchModel, CabinetKind, LevelKind, MAX_SOFFITS, Soffit};
use cadcraft_geom::Vec2;

/// Snap tolerance for aligning rect sides to wall face lines.
pub const SNAP: f64 = 0.15;

/// Find an interior point of `ring`. Returns the area centroid when it lies inside the
/// polygon; otherwise returns the middle of the widest inside span on the horizontal
/// line through the bbox middle. `None` for fewer than 3 finite points.
pub fn interior_point(ring: &[Vec2]) -> Option<Vec2> {
    if ring.len() < 3 || ring.iter().any(|v| !v.is_finite()) {
        return None;
    }

    let n = ring.len();
    let mut area2 = 0.0_f64;
    let mut cx_sum = 0.0_f64;
    let mut cy_sum = 0.0_f64;
    for i in 0..n {
        let j = (i + 1) % n;
        let a = ring[i];
        let b = ring[j];
        let cross = a.x * b.y - b.x * a.y;
        area2 += cross;
        cx_sum += (a.x + b.x) * cross;
        cy_sum += (a.y + b.y) * cross;
    }
    if area2.abs() <= 1e-15 {
        return None;
    }
    let centroid = Vec2::new(cx_sum / (6.0 * area2), cy_sum / (6.0 * area2));
    if cadcraft_geom::point_in_polygon(ring, centroid) {
        return Some(centroid);
    }

    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;
    for p in ring {
        ymin = ymin.min(p.y);
        ymax = ymax.max(p.y);
    }
    let y = (ymin + ymax) / 2.0 + 1e-7;
    let mut xs = Vec::new();
    for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)) {
        let y1 = a.y;
        let y2 = b.y;
        let x1 = a.x;
        let x2 = b.x;
        if (y1 > y) != (y2 > y) && (y2 - y1).abs() > 0.0 {
            let x = x1 + (y - y1) * (x2 - x1) / (y2 - y1);
            xs.push(x);
        }
    }
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(f64::total_cmp);
    let mut best_width = 0.0;
    let mut best_mid = None;
    for i in (0..xs.len()).step_by(2) {
        if i + 1 >= xs.len() {
            break;
        }
        let x0 = xs[i];
        let x1 = xs[i + 1];
        let span = x1 - x0;
        if !(span.is_finite() && span > 0.0) {
            continue;
        }
        let mid = (x0 + x1) / 2.0;
        let pt = Vec2::new(mid, y);
        if cadcraft_geom::point_in_polygon(ring, pt) && span > best_width {
            best_width = span;
            best_mid = Some(pt);
        }
    }
    if best_width <= 0.0 {
        return None;
    }
    best_mid
}

/// The ring of `s`: when `over` is non-empty every id must be a wall cabinet on `s.level`
/// sharing one host wall and rotation; the ring is the box of the cabinets against that wall.
/// Otherwise `s.polygon` (drop trailing duplicate, keep only finite vertices, reverse if signed
/// area is negative).
pub fn soffit_ring(model: &ArchModel, s: &Soffit) -> Option<Vec<Vec2>> {
    if !s.over.is_empty() {
        let mut host_idx: Option<usize> = None;
        let mut rotation: Option<f64> = None;
        let mut min_t = f64::INFINITY;
        let mut max_t = f64::NEG_INFINITY;
        let mut max_depth = 0.0_f64;
        let mut first_side: Option<f64> = None;

        for cid in &s.over {
            let cab = model.cabinets.iter().find(|c| c.id == *cid)?;
            if cab.kind != CabinetKind::Wall || cab.level != s.level {
                return None;
            }

            let mut found = None;
            for (idx, w) in model.walls.iter().enumerate() {
                if w.base_level != s.level || w.divider || !w.start.is_finite() || !w.end.is_finite() {
                    continue;
                }
                let Some(dir) = crate::wall::wall_dir(w) else { continue };
                let angle = dir.y.atan2(dir.x).to_degrees();
                if (cab.rotation - angle).abs() > 1e-6 && (cab.rotation - (angle + 180.0)).abs() > 1e-6 {
                    continue;
                }
                let left = dir.perp();
                let t = (cab.position - w.start).dot(dir);
                let s0 = t - cab.width / 2.0;
                let s1 = t + cab.width / 2.0;
                if !(s0 >= -1e-6 && s1 <= w.start.dist(w.end) + 1e-6) {
                    continue;
                }
                let side = (cab.position - w.start).dot(left);
                if side.abs() > w.thickness * 1.5 + 1e-6 {
                    continue;
                }
                found = Some((idx, s0, s1, side));
                break;
            }
            let Some((idx, s0, s1, side)) = found else {
                return None;
            };
            if let Some(h) = host_idx {
                if h != idx || rotation.is_some_and(|r| (cab.rotation - r).abs() > 1e-6) {
                    return None;
                }
            } else {
                host_idx = Some(idx);
                rotation = Some(cab.rotation);
            }
            first_side = first_side.or(Some(side));
            min_t = min_t.min(s0);
            max_t = max_t.max(s1);
            max_depth = max_depth.max(cab.depth);
        }

        let host_idx = host_idx?;
        let w = &model.walls[host_idx];
        let dir = crate::wall::wall_dir(w)?;
        let left = dir.perp();
        let side = first_side.unwrap_or(0.0);
        let sgn = if side > 1e-12 {
            1.0
        } else if side < -1e-12 {
            -1.0
        } else {
            1.0
        };
        let p = |t: f64, y: f64| w.start + dir * t + left * y;
        Some(vec![p(min_t, side), p(max_t, side), p(max_t, side + sgn * max_depth), p(min_t, side + sgn * max_depth)])
    } else {
        let mut ring = s.polygon.clone();
        if ring.len() > 1 && ring.first() == ring.last() {
            ring.pop();
        }
        if ring.len() < 3 || ring.iter().any(|v| !v.is_finite()) {
            return None;
        }
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
    let level_z = model.levels.get(s.level as usize).map_or(0.0, |l| l.elevation);
    if !s.over.is_empty() {
        let mut z_max = f64::NEG_INFINITY;
        for cid in &s.over {
            let cab = model.cabinets.iter().find(|c| c.id == *cid)?;
            z_max = z_max.max(cab.elevation + cab.height);
        }
        return Some(level_z + z_max);
    }
    if let Some(bottom) = s.bottom {
        return Some(level_z + bottom);
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

/// The bottom (absolute z) of the first soffit on `level` whose outline holds `p`, else `None`.
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
/// (overlap > 0). For each wall, the face that looks into the rect is used: for the
/// upper side (x1, y1) the smaller coordinate, for the lower side (x0, y0) the larger.
/// A move that degenerates the rect is skipped. Non-finite input returns `r`.
pub fn snap_rect(model: &ArchModel, level: u32, mut r: [f64; 4]) -> [f64; 4] {
    if !r.iter().all(|v| v.is_finite()) {
        return r;
    }

    fn try_side(r: &mut [f64; 4], model: &ArchModel, level: u32, idx: usize) {
        let value = r[idx];
        let axis: u8 = if idx == 0 || idx == 2 { 1 } else { 0 };

        let (span_lo, span_hi) = if axis == 0 { (r[0].min(r[2]), r[0].max(r[2])) } else { (r[1].min(r[3]), r[1].max(r[3])) };

        let mut best: Option<(f64, ArchId, f64)> = None;

        for w in &model.walls {
            if w.base_level != level || w.divider {
                continue;
            }
            let Some(lines) = crate::join::face_lines(w) else {
                continue;
            };

            let mut coords = Vec::new();
            let mut proj_lo = f64::INFINITY;
            let mut proj_hi = f64::NEG_INFINITY;

            for (p0, p1) in &lines {
                if axis == 0 {
                    if (p0.y - p1.y).abs() > 1e-6 || !p0.y.is_finite() || !p1.y.is_finite() {
                        continue;
                    }
                    coords.push(p0.y);
                    if p0.x.is_finite() && p1.x.is_finite() {
                        proj_lo = proj_lo.min(p0.x.min(p1.x));
                        proj_hi = proj_hi.max(p0.x.max(p1.x));
                    }
                } else {
                    if (p0.x - p1.x).abs() > 1e-6 || !p0.x.is_finite() || !p1.x.is_finite() {
                        continue;
                    }
                    coords.push(p0.x);
                    if p0.y.is_finite() && p1.y.is_finite() {
                        proj_lo = proj_lo.min(p0.y.min(p1.y));
                        proj_hi = proj_hi.max(p0.y.max(p1.y));
                    }
                }
            }

            if coords.is_empty() || proj_lo.is_finite() == false || proj_hi.is_finite() == false {
                continue;
            }

            let candidate = if matches!(idx, 2 | 3) {
                coords.iter().copied().fold(f64::INFINITY, f64::min)
            } else {
                coords.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            };

            if !candidate.is_finite() {
                continue;
            }

            let overlap = proj_hi.min(span_hi) - proj_lo.max(span_lo);
            if overlap <= 0.0 {
                continue;
            }

            let dist = (candidate - value).abs();
            if dist > SNAP {
                continue;
            }

            let entry = (dist, w.id, candidate);
            match best {
                None => best = Some(entry),
                Some((best_dist, best_id, _)) => {
                    if dist < best_dist - 1e-12 || (dist <= best_dist + 1e-12 && w.id < best_id) {
                        best = Some(entry);
                    }
                }
            }
        }

        if let Some((_, _, new_val)) = best {
            let old = r[idx];
            r[idx] = new_val;
            if (r[2] - r[0]).abs() <= 1e-9 || (r[3] - r[1]).abs() <= 1e-9 {
                r[idx] = old;
            }
        }
    }

    let r0 = r[0];
    let r1 = r[1];
    let r2 = r[2];
    let r3 = r[3];
    try_side(&mut r, model, level, 1);
    try_side(&mut r, model, level, 3);
    try_side(&mut r, model, level, 0);
    try_side(&mut r, model, level, 2);
    r
}

/// Validate a soffit `s` against the model. Returns an error string when invalid.
pub fn validate(model: &ArchModel, s: &Soffit) -> Result<(), String> {
    let has_bottom = s.bottom.is_some();
    let has_depth = s.depth.is_some();
    let has_over = !s.over.is_empty();
    let count = [has_bottom, has_depth, has_over].iter().filter(|&&v| v).count();
    if count != 1 {
        return Err(format!("exactly one of bottom, depth or over must be set; got {count}"));
    }

    let ring = soffit_ring(model, s).ok_or("ring is invalid")?;
    if ring.len() < 3 || ring.iter().any(|v| !v.is_finite()) {
        return Err("ring must have at least 3 finite points".into());
    }
    if !poly::is_simple(&ring) {
        return Err("ring is not simple".into());
    }
    let mut area2 = 0.0_f64;
    for i in 0..ring.len() {
        let j = (i + 1) % ring.len();
        area2 += ring[i].x * ring[j].y - ring[j].x * ring[i].y;
    }
    if area2.abs() < 0.01 {
        return Err("ring area too small".into());
    }

    let ip = interior_point(&ring).ok_or("no interior point")?;
    if ceiling::room_at_point(model, s.level, ip).is_none() {
        return Err("interior point is not in a room".into());
    }
    if ceiling::finished_ceiling_at(model, s.level, ip).is_none() {
        return Err("interior point has no finished ceiling".into());
    }

    let level_elev = model.levels.get(s.level as usize).map_or(0.0, |l| l.elevation);
    if !(s.bottom.is_some() || s.depth.is_some()) {
        // Over-cabinet soffits are validated by their bottom logic below.
    }
    if let Some(bottom) = s.bottom {
        if !bottom.is_finite() || bottom < 0.0 {
            return Err("bottom must be finite and non-negative".into());
        }
        let z = level_elev + bottom;
        if !z.is_finite() || z < level_elev + 0.5 {
            return Err("bottom must be at least level elevation + 0.5".into());
        }
        for v in &ring {
            if let Some(ceil_z) = ceiling::finished_ceiling_at(model, s.level, *v) {
                if z > ceil_z - 0.01 {
                    return Err("bottom is too close to the finished ceiling".into());
                }
            }
        }
    }
    if let Some(depth) = s.depth {
        if !depth.is_finite() || !(0.01..=3.0).contains(&depth) {
            return Err("depth must be in [0.01, 3.0]".to_string());
        }
    }
    let bottom_z = soffit_bottom(model, s).ok_or("no bottom")?;
    if !bottom_z.is_finite() {
        return Err("bottom must be finite".into());
    }
    for v in &ring {
        if let Some(ceil_z) = ceiling::finished_ceiling_at(model, s.level, *v) {
            if bottom_z > ceil_z - 0.01 {
                return Err("bottom is too close to the finished ceiling".into());
            }
        }
    }
    if let Some(material) = &s.material {
        if material.trim().is_empty() || material.chars().count() > 128 {
            return Err("material must be non-empty and <= 128 chars".to_string());
        }
    }
    if model.soffits.len() >= MAX_SOFFITS {
        return Err("max soffits reached".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchModel, Cabinet, CabinetKind, Level, LevelKind, Room, RoomType, Wall};
    use cadcraft_geom::Vec2;

    fn house() -> ArchModel {
        let mut m = ArchModel::default();
        m.levels.push(Level {
            index: 0,
            name: "Level 1".to_string(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        let t = 0.2;
        m.walls.push(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), t));
        m.walls.push(Wall::new(Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), t));
        m.walls.push(Wall::new(Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0), t));
        m.walls.push(Wall::new(Vec2::new(0.0, 8.0), Vec2::new(0.0, 0.0), t));
        m.walls.push(Wall::new(Vec2::new(0.0, 3.0), Vec2::new(10.0, 3.0), 0.1));
        m.walls.push(Wall::new(Vec2::new(6.0, 3.0), Vec2::new(6.0, 8.0), 0.1));

        m.rooms.push(Room {
            id: ArchId(0),
            name: "S".into(),
            room_type: RoomType::Living,
            level: 0,
            polygon: vec![Vec2::new(0.1, 0.1), Vec2::new(9.9, 0.1), Vec2::new(9.9, 2.95), Vec2::new(0.1, 2.95)],
            ..Room::default()
        });
        m.rooms.push(Room {
            id: ArchId(1),
            name: "NW".into(),
            room_type: RoomType::Other,
            level: 0,
            polygon: vec![Vec2::new(0.1, 3.05), Vec2::new(5.95, 3.05), Vec2::new(5.95, 7.9), Vec2::new(0.1, 7.9)],
            ..Room::default()
        });
        m.rooms.push(Room {
            id: ArchId(2),
            name: "NE".into(),
            room_type: RoomType::Other,
            level: 0,
            polygon: vec![Vec2::new(6.05, 3.05), Vec2::new(9.9, 3.05), Vec2::new(9.9, 7.9), Vec2::new(6.05, 7.9)],
            ..Room::default()
        });
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
    ) -> Cabinet {
        Cabinet { id: ArchId(id), kind, level, position, rotation: 180.0, width, depth, height, elevation, ..Cabinet::default() }
    }

    #[test]
    fn snap_rect_moves_sides_onto_wall_faces_and_into_corners() {
        let m = house();
        let r = snap_rect(&m, 0, [9.0, 7.0, 9.96, 7.95]);
        assert!((r[2] - 9.9).abs() < 1e-6, "x1: {} != 9.9", r[2]);
        assert!((r[3] - 7.9).abs() < 1e-6, "y1: {} != 7.9", r[3]);
        assert!((r[0] - 9.0).abs() < 1e-6, "x0 should not move");
        assert!((r[1] - 7.0).abs() < 1e-6, "y0 should not move");

        let r2 = snap_rect(&m, 0, [6.5, 3.5, 7.3, 4.3]);
        assert!((r2[0] - 6.5).abs() < 1e-9);
        assert!((r2[1] - 3.5).abs() < 1e-9);
        assert!((r2[2] - 7.3).abs() < 1e-9);
        assert!((r2[3] - 4.3).abs() < 1e-9);

        assert!((snap_rect(&m, 0, [9.0, 7.0, 9.86, 7.95])[2] - 9.9).abs() < 1e-6, "0.14 snapped");
        let r3 = snap_rect(&m, 0, [9.0, 7.0, 9.84, 7.95]);
        assert!((r3[2] - 9.9).abs() < 1e-9, "0.06 snaps outward");
    }

    #[test]
    fn bottom_from_depth_below_the_finished_ceiling() {
        let m = house();
        let s_depth = Soffit {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(1.0, 1.0), Vec2::new(5.0, 1.0), Vec2::new(5.0, 5.0), Vec2::new(1.0, 5.0), Vec2::new(1.0, 1.0)],
            depth: Some(0.3),
            ..Soffit::default()
        };
        let b = soffit_bottom(&m, &s_depth).unwrap();
        assert!((b - 2.1).abs() < 1e-6, "depth 0.3 -> {}, expected 2.1", b);

        let s_bottom = Soffit {
            id: ArchId(1),
            level: 0,
            polygon: vec![Vec2::new(1.0, 1.0), Vec2::new(5.0, 1.0), Vec2::new(5.0, 5.0), Vec2::new(1.0, 5.0), Vec2::new(1.0, 1.0)],
            bottom: Some(2.1),
            ..Soffit::default()
        };
        let b2 = soffit_bottom(&m, &s_bottom).unwrap();
        assert!((b2 - 2.1).abs() < 1e-6, "bottom 2.1 -> {}, expected 2.1", b2);
    }

    #[test]
    fn over_cabinets_ring_and_bottom() {
        let mut m = house();
        m.cabinets.push(make_cabinet(0, 2, CabinetKind::Wall, Vec2::new(9.0, 7.9), 0.6, 0.305, 0.762, 1.372, 0));
        m.cabinets.push(make_cabinet(1, 2, CabinetKind::Wall, Vec2::new(8.4, 7.9), 0.6, 0.305, 0.762, 1.372, 0));
        let s = Soffit { id: ArchId(0), level: 0, polygon: vec![], over: vec![ArchId(0), ArchId(1)], ..Soffit::default() };
        let ring = soffit_ring(&m, &s).unwrap();
        assert_eq!(ring.len(), 4);
        let xs: Vec<f64> = ring.iter().map(|v| v.x).collect();
        let ys: Vec<f64> = ring.iter().map(|v| v.y).collect();
        let xmin = xs.iter().copied().fold(f64::INFINITY, f64::min);
        let xmax = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let ymin = ys.iter().copied().fold(f64::INFINITY, f64::min);
        let ymax = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!((xmin - 8.1).abs() < 1e-3, "xmin: {} != 8.1", xmin);
        assert!((xmax - 9.3).abs() < 1e-3, "xmax: {} != 9.3", xmax);
        assert!((ymin - 7.595).abs() < 1e-3, "ymin: {} != 7.595", ymin);
        assert!((ymax - 7.9).abs() < 1e-3, "ymax: {} != 7.9", ymax);
        let b = soffit_bottom(&m, &s).unwrap();
        assert!((b - 2.134).abs() < 1e-6, "bottom: {} != 2.134", b);

        m.cabinets.push(make_cabinet(2, 2, CabinetKind::Base, Vec2::new(9.0, 7.9), 0.6, 0.305, 0.762, 0.0, 0));
        let s_bad = Soffit { id: ArchId(0), level: 0, polygon: vec![], over: vec![ArchId(2)], ..Soffit::default() };
        assert!(soffit_ring(&m, &s_bad).is_none());

        m.cabinets.push(make_cabinet(3, 2, CabinetKind::Wall, Vec2::new(9.0, 7.9), 0.6, 0.305, 0.762, 1.372, 1));
        let s_bad2 = Soffit { id: ArchId(0), level: 0, polygon: vec![], over: vec![ArchId(3)], ..Soffit::default() };
        assert!(soffit_ring(&m, &s_bad2).is_none());

        m.cabinets.push(make_cabinet(4, 0, CabinetKind::Wall, Vec2::new(5.0, 0.1), 0.6, 0.305, 0.762, 1.372, 0));
        let s_bad3 = Soffit { id: ArchId(0), level: 0, polygon: vec![], over: vec![ArchId(0), ArchId(4)], ..Soffit::default() };
        assert!(soffit_ring(&m, &s_bad3).is_none());
    }

    #[test]
    fn bottom_at_inside_and_outside() {
        let mut m = house();
        m.soffits.push(Soffit {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(7.0, 7.3), Vec2::new(9.0, 7.3), Vec2::new(9.0, 7.9), Vec2::new(7.0, 7.9), Vec2::new(7.0, 7.3)],
            bottom: Some(2.1),
            ..Soffit::default()
        });
        let b = bottom_at(&m, 0, Vec2::new(8.0, 7.6)).unwrap();
        assert!((b - 2.1).abs() < 1e-6, "inside: {} != 2.1", b);
        assert!(bottom_at(&m, 0, Vec2::new(8.0, 7.2)).is_none());
        assert!(bottom_at(&m, 1, Vec2::new(8.0, 7.6)).is_none());
    }

    #[test]
    fn interior_point_triangle_and_degenerate() {
        let tri = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(0.0, 3.0)];
        let p = interior_point(&tri).expect("triangle should have interior point");
        assert!(cadcraft_geom::point_in_polygon(&tri, p), "point not in triangle");

        let sq = vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 2.0), Vec2::new(0.0, 2.0)];
        let p2 = interior_point(&sq).expect("square should have interior point");
        assert!(cadcraft_geom::point_in_polygon(&sq, p2), "point not in square");

        let collinear = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0)];
        assert!(interior_point(&collinear).is_none(), "collinear points should have no interior point");

        assert!(interior_point(&vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)]).is_none());
        assert!(interior_point(&[]).is_none());

        let nan = vec![Vec2::new(0.0, 0.0), Vec2::new(f64::NAN, 0.0), Vec2::new(1.0, 1.0)];
        assert!(interior_point(&nan).is_none(), "non-finite point should return None");
    }

    #[test]
    fn soffit_ring_polygon_edge_cases() {
        let m = ArchModel::default();

        let s1 = Soffit {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 2.0), Vec2::new(0.0, 2.0), Vec2::new(0.0, 0.0)],
            ..Soffit::default()
        };
        let ring1 = soffit_ring(&m, &s1).expect("should handle duplicate trailing point");
        assert_eq!(ring1.len(), 4, "trailing duplicate should be dropped");

        let s2 = Soffit {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(0.0, 2.0), Vec2::new(2.0, 2.0), Vec2::new(2.0, 0.0)],
            ..Soffit::default()
        };
        let ring2 = soffit_ring(&m, &s2).expect("should reverse CW polygon");
        let mut area2 = 0.0_f64;
        for i in 0..ring2.len() {
            let j = (i + 1) % ring2.len();
            area2 += ring2[i].x * ring2[j].y - ring2[j].x * ring2[i].y;
        }
        assert!(area2 > 0.0, "ring should be CCW (positive signed area)");

        let s3 = Soffit { id: ArchId(0), level: 0, polygon: vec![], ..Soffit::default() };
        assert!(soffit_ring(&m, &s3).is_none(), "empty polygon should return None");

        let s4 = Soffit { id: ArchId(0), level: 0, polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)], ..Soffit::default() };
        assert!(soffit_ring(&m, &s4).is_none(), "2 points should return None");
    }

    #[test]
    fn snap_rect_snaps_to_the_room_side_face() {
        let m = house();
        // NE room: east inner face x = 9.9, north inner face y = 7.9.
        let r = snap_rect(&m, 0, [9.0, 7.0, 9.96, 7.95]);
        assert!((r[0] - 9.0).abs() < 1e-9, "x0: {}", r[0]);
        assert!((r[1] - 7.0).abs() < 1e-9, "y0: {}", r[1]);
        assert!((r[2] - 9.9).abs() < 1e-9, "x1 should snap to east inner face, got {}", r[2]);
        assert!((r[3] - 7.9).abs() < 1e-9, "y1 should snap to north inner face, got {}", r[3]);

        // A rect already inside the face span snaps outward to the inner face.
        let r = snap_rect(&m, 0, [9.0, 7.0, 9.8, 7.8]);
        assert!((r[2] - 9.9).abs() < 1e-9, "x1 inside-out: {}", r[2]);
        assert!((r[3] - 7.9).abs() < 1e-9, "y1 inside-out: {}", r[3]);
    }

    #[test]
    fn snap_rect_snaps_from_either_side_within_0_15() {
        let m = house();
        // 0.16 m from the face is outside SNAP, so x1 stays put (y1 still snaps).
        let r = snap_rect(&m, 0, [9.0, 7.0, 9.74, 7.95]);
        assert!((r[2] - 9.74).abs() < 1e-9, "x1 out of SNAP (0.16) should stay, got {}", r[2]);
        assert!((r[3] - 7.9).abs() < 1e-9, "y1 in SNAP should snap, got {}", r[3]);

        // Mid-room rect, away from every face: unchanged.
        let r = snap_rect(&m, 0, [6.5, 3.5, 7.3, 4.3]);
        assert!((r[0] - 6.5).abs() < 1e-9);
        assert!((r[1] - 3.5).abs() < 1e-9);
        assert!((r[2] - 7.3).abs() < 1e-9);
        assert!((r[3] - 4.3).abs() < 1e-9);
    }

    #[test]
    fn over_cabinets_ring_starts_at_the_cabinet_backs() {
        let mut m = house();
        m.cabinets.push(make_cabinet(0, 2, CabinetKind::Wall, Vec2::new(9.0, 7.9), 0.6, 0.305, 0.762, 1.372, 0));
        m.cabinets.push(make_cabinet(1, 2, CabinetKind::Wall, Vec2::new(8.4, 7.9), 0.6, 0.305, 0.762, 1.372, 0));
        let s = Soffit { id: ArchId(0), level: 0, polygon: vec![], over: vec![ArchId(0), ArchId(1)], ..Soffit::default() };
        let ring = soffit_ring(&m, &s).expect("over-cabinet ring");
        let xs: Vec<f64> = ring.iter().map(|v| v.x).collect();
        let ys: Vec<f64> = ring.iter().map(|v| v.y).collect();
        let xmin = xs.iter().copied().fold(f64::INFINITY, f64::min);
        let xmax = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let ymin = ys.iter().copied().fold(f64::INFINITY, f64::min);
        let ymax = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        // The ring starts at the cabinets' back line (y = 7.9) and extends into the room
        // by the cabinet depth (0.305), not from the wall centre line.
        assert!((xmin - 8.1).abs() < 1e-3, "xmin: {}", xmin);
        assert!((xmax - 9.3).abs() < 1e-3, "xmax: {}", xmax);
        assert!((ymin - 7.595).abs() < 1e-3, "ymin (back - depth): {}", ymin);
        assert!((ymax - 7.9).abs() < 1e-3, "ymax (cabinet backs): {}", ymax);
    }

    #[test]
    fn validate_exactly_one_of_bottom_depth_over() {
        let m = house();
        let polygon = vec![Vec2::new(1.0, 1.0), Vec2::new(5.0, 1.0), Vec2::new(5.0, 5.0), Vec2::new(1.0, 5.0)];

        let base = Soffit { id: ArchId(0), level: 0, polygon: polygon.clone(), ..Soffit::default() };
        assert!(validate(&m, &base).is_err(), "no bottom/depth/over should error");

        let s_bottom = Soffit { bottom: Some(2.1), ..base.clone() };
        assert!(validate(&m, &s_bottom).is_ok(), "only bottom should be ok");

        let s_depth = Soffit { depth: Some(0.3), ..base.clone() };
        assert!(validate(&m, &s_depth).is_ok(), "only depth should be ok");

        let s_both = Soffit { bottom: Some(2.1), depth: Some(0.3), ..base.clone() };
        assert!(validate(&m, &s_both).is_err(), "both bottom and depth should error");

        let s_bottom_over = Soffit { bottom: Some(2.1), over: vec![ArchId(0)], ..base.clone() };
        assert!(validate(&m, &s_bottom_over).is_err(), "bottom and over should error");
    }

    #[test]
    fn snap_rect_non_finite_and_degenerate() {
        let m = house();

        let r = [0.0, 0.0, f64::NAN, 2.0];
        let result = snap_rect(&m, 0, r);
        assert!(result[2].is_nan(), "non-finite x1 should stay non-finite");

        let r2 = [f64::INFINITY, 0.0, 2.0, 2.0];
        let result2 = snap_rect(&m, 0, r2);
        assert_eq!(result2[0], f64::INFINITY, "infinite x0 should stay infinite");

        let r3 = snap_rect(&m, 0, [9.8, 7.0, 10.0, 7.95]);
        assert!((r3[0] - 9.8).abs() < 1e-9, "x0 should not move");
        assert!((r3[2] - 9.9).abs() < 1e-6, "x1 snaps to inner face 9.9");
    }

    #[test]
    fn bottom_at_first_in_model_order() {
        let mut m = house();
        m.soffits.push(Soffit {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(7.0, 7.0), Vec2::new(9.0, 7.0), Vec2::new(9.0, 7.9), Vec2::new(7.0, 7.9), Vec2::new(7.0, 7.0)],
            bottom: Some(2.1),
            ..Soffit::default()
        });
        m.soffits.push(Soffit {
            id: ArchId(1),
            level: 0,
            polygon: vec![Vec2::new(8.0, 7.5), Vec2::new(9.5, 7.5), Vec2::new(9.5, 8.0), Vec2::new(8.0, 8.0), Vec2::new(8.0, 7.5)],
            bottom: Some(2.0),
            ..Soffit::default()
        });
        let b = bottom_at(&m, 0, Vec2::new(8.5, 7.7)).unwrap();
        assert!((b - 2.1).abs() < 1e-6, "first soffit should win: {} != 2.1", b);
        let b2 = bottom_at(&m, 0, Vec2::new(9.2, 7.7)).unwrap();
        assert!((b2 - 2.0).abs() < 1e-6, "second soffit: {} != 2.0", b2);
    }

    #[test]
    fn validate_material_length() {
        let m = house();
        let polygon = vec![Vec2::new(1.0, 1.0), Vec2::new(5.0, 1.0), Vec2::new(5.0, 5.0), Vec2::new(1.0, 5.0)];

        let s = Soffit { id: ArchId(0), level: 0, polygon: polygon.clone(), bottom: Some(2.1), material: Some("x".repeat(129)), ..Soffit::default() };
        assert!(validate(&m, &s).is_err(), "material > 128 chars should error");

        let s2 = Soffit { id: ArchId(0), level: 0, polygon, bottom: Some(2.1), material: Some("x".repeat(128)), ..Soffit::default() };
        assert!(validate(&m, &s2).is_ok(), "material == 128 chars should be ok");
    }

    #[test]
    fn interior_point_c_shaped_polygon() {
        // C-shape opening to the right; centroid falls in the opening, so the
        // widest-span fallback must find a point inside.
        let ring = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(4.0, 0.0),
            Vec2::new(4.0, 1.0),
            Vec2::new(2.0, 1.0),
            Vec2::new(2.0, 3.0),
            Vec2::new(4.0, 3.0),
            Vec2::new(4.0, 4.0),
            Vec2::new(0.0, 4.0),
        ];
        let p = interior_point(&ring).expect("C-shape should have interior point");
        assert!(cadcraft_geom::point_in_polygon(&ring, p), "point not in C-shape");
    }

    #[test]
    fn soffit_ring_single_cabinet() {
        let mut m = house();
        m.cabinets.push(make_cabinet(0, 2, CabinetKind::Wall, Vec2::new(9.0, 7.9), 0.6, 0.305, 0.762, 1.372, 0));
        let s = Soffit { id: ArchId(0), level: 0, polygon: vec![], over: vec![ArchId(0)], ..Soffit::default() };
        let ring = soffit_ring(&m, &s).expect("single cabinet ring");
        assert_eq!(ring.len(), 4);
        let xs: Vec<f64> = ring.iter().map(|v| v.x).collect();
        let ys: Vec<f64> = ring.iter().map(|v| v.y).collect();
        let xmin = xs.iter().copied().fold(f64::INFINITY, f64::min);
        let xmax = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let ymin = ys.iter().copied().fold(f64::INFINITY, f64::min);
        let ymax = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!((xmin - 8.7).abs() < 1e-3, "xmin: {}", xmin);
        assert!((xmax - 9.3).abs() < 1e-3, "xmax: {}", xmax);
        assert!((ymin - 7.595).abs() < 1e-3, "ymin: {}", ymin);
        assert!((ymax - 7.9).abs() < 1e-3, "ymax: {}", ymax);
    }

    #[test]
    fn soffit_ring_cabinets_different_depths() {
        let mut m = house();
        m.cabinets.push(make_cabinet(0, 2, CabinetKind::Wall, Vec2::new(9.0, 7.9), 0.6, 0.305, 0.762, 1.372, 0));
        m.cabinets.push(make_cabinet(1, 2, CabinetKind::Wall, Vec2::new(8.4, 7.9), 0.6, 0.355, 0.762, 1.372, 0));
        let s = Soffit { id: ArchId(0), level: 0, polygon: vec![], over: vec![ArchId(0), ArchId(1)], ..Soffit::default() };
        let ring = soffit_ring(&m, &s).expect("different depth ring");
        let ys: Vec<f64> = ring.iter().map(|v| v.y).collect();
        let ymin = ys.iter().copied().fold(f64::INFINITY, f64::min);
        // Max depth is 0.355, so ymin should be 7.9 - 0.355 = 7.545
        assert!((ymin - 7.545).abs() < 1e-3, "ymin should use max depth: {}", ymin);
    }

    #[test]
    fn snap_rect_already_on_face() {
        let m = house();
        // Rect already exactly on the inner faces; nothing should move.
        let r = snap_rect(&m, 0, [9.0, 7.0, 9.9, 7.9]);
        assert!((r[0] - 9.0).abs() < 1e-9, "x0: {}", r[0]);
        assert!((r[1] - 7.0).abs() < 1e-9, "y0: {}", r[1]);
        assert!((r[2] - 9.9).abs() < 1e-9, "x1 should stay at 9.9, got {}", r[2]);
        assert!((r[3] - 7.9).abs() < 1e-9, "y1 should stay at 7.9, got {}", r[3]);
    }

    #[test]
    fn validate_bottom_at_boundary() {
        let m = house();
        let polygon = vec![Vec2::new(1.0, 1.0), Vec2::new(5.0, 1.0), Vec2::new(5.0, 5.0), Vec2::new(1.0, 5.0)];
        // Level 0 elevation is 0.0, so bottom must be >= 0.5
        let s = Soffit { id: ArchId(0), level: 0, polygon: polygon.clone(), bottom: Some(0.5), ..Soffit::default() };
        assert!(validate(&m, &s).is_ok(), "bottom exactly at level + 0.5 should be ok");

        let s2 = Soffit { id: ArchId(0), level: 0, polygon, bottom: Some(0.49), ..Soffit::default() };
        assert!(validate(&m, &s2).is_err(), "bottom below level + 0.5 should error");
    }

    #[test]
    fn validate_depth_boundaries() {
        let m = house();
        let polygon = vec![Vec2::new(1.0, 1.0), Vec2::new(5.0, 1.0), Vec2::new(5.0, 5.0), Vec2::new(1.0, 5.0)];

        let s_min = Soffit { id: ArchId(0), level: 0, polygon: polygon.clone(), depth: Some(0.01), ..Soffit::default() };
        assert!(validate(&m, &s_min).is_ok(), "depth 0.01 should be ok");

        let s_max = Soffit { id: ArchId(0), level: 0, polygon: polygon.clone(), depth: Some(3.0), ..Soffit::default() };
        assert!(validate(&m, &s_max).is_ok(), "depth 3.0 should be ok");

        let s_low = Soffit { id: ArchId(0), level: 0, polygon: polygon.clone(), depth: Some(0.009), ..Soffit::default() };
        assert!(validate(&m, &s_low).is_err(), "depth 0.009 should error");

        let s_high = Soffit { id: ArchId(0), level: 0, polygon: polygon.clone(), depth: Some(3.01), ..Soffit::default() };
        assert!(validate(&m, &s_high).is_err(), "depth 3.01 should error");
    }
}
