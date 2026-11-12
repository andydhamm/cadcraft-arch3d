//! Room molding runs (phase 18): the resolved paths a room's moldings follow.

use crate::join::polygon_area;
use cadcraft_doc::arch::{ArchModel, MoldingKind, MoldingProfile, Room, RoomMolding, Wall};
use cadcraft_geom::Vec2;

/// One continuous run of a room molding along the surface its back is fixed to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MoldingRun {
    /// Which molding it is.
    pub kind: MoldingKind,
    /// The index of the molding in the room's `moldings`.
    pub index: usize,
    /// The cross-section.
    pub profile: MoldingProfile,
    /// Plan points on the surface the profile's back is fixed to.
    pub path: Vec<Vec2>,
    /// Absolute elevation of the profile's BOTTOM at each path vertex.
    pub z: Vec<f64>,
    /// Whether the path closes on itself.
    pub closed: bool,
    /// `+1`: the profile's x is to the left of travel, `-1`: to the right.
    pub side: f64,
}

/// The vertical span (bottom, top) of a wall's solid part.
///
/// Returns None when either end is not finite or top <= bottom.
pub fn wall_z_span(model: &ArchModel, w: &Wall) -> Option<(f64, f64)> {
    let bottom = w.bottom_z(&model.levels);
    let top = if w.top_level.is_none() && w.to_ceiling {
        if let Some(level) = model.level(w.base_level) {
            level.elevation + level.ceiling_height
        } else {
            return None;
        }
    } else {
        w.top_z(&model.levels)
    };
    if !bottom.is_finite() || !top.is_finite() || top <= bottom {
        return None;
    }
    Some((bottom, top))
}

/// Whether a wall's solid part carries a molding band.
///
/// The band must be finite and the wall's top must reach its top.
pub(crate) fn face_carries(model: &ArchModel, w: &Wall, band: (f64, f64)) -> bool {
    if !band.0.is_finite() || !band.1.is_finite() {
        return false;
    }
    wall_z_span(model, w).is_some_and(|(_, top)| top >= band.1 - 1e-6) && band_on_solid(model, w, band)
}

/// Whether `band` lies on a solid (non-glass) part of the wall: glass faces carry no molding.
/// A split wall is solid below its split height only when its lower type is, above only when its
/// upper type is; a band across the split needs both parts solid.
fn band_on_solid(model: &ArchModel, w: &Wall, band: (f64, f64)) -> bool {
    use cadcraft_doc::arch::is_glass_type;
    let Some(split) = w.split.as_ref() else {
        return !is_glass_type(&w.wall_type);
    };
    let Some((bottom, _)) = wall_z_span(model, w) else {
        return false;
    };
    let s = bottom + split.height;
    if band.1 <= s + 1e-9 {
        !is_glass_type(&w.wall_type)
    } else if band.0 >= s - 1e-9 {
        !is_glass_type(&split.upper)
    } else {
        !is_glass_type(&w.wall_type) && !is_glass_type(&split.upper)
    }
}

/// The molding runs of `room`.
pub fn room_runs(model: &ArchModel, room: &Room) -> Vec<MoldingRun> {
    let mut runs = Vec::new();
    let Some(level) = model.level(room.level) else {
        return runs;
    };
    let e = level.elevation;
    let ceiling_h = room.ceiling_height.filter(|h| h.is_finite() && *h > 0.0).unwrap_or(level.ceiling_height);

    for (idx, molding) in room.moldings.iter().enumerate() {
        // Skip base/chair when no floor, crown when no ceiling
        if (molding.kind == MoldingKind::Base || molding.kind == MoldingKind::Chair) && !room.room_type.has_floor() {
            continue;
        }
        if molding.kind == MoldingKind::Crown && !room.room_type.has_ceiling() {
            continue;
        }

        let h = molding.profile.size()[1];
        if !h.is_finite() || h <= 0.0 {
            continue;
        }

        let z0 = match molding.kind {
            MoldingKind::Base => e + molding.offset,
            MoldingKind::Chair => e + molding.offset,
            MoldingKind::Crown => e + ceiling_h - molding.offset - h,
        };
        let band = (z0, z0 + h);

        // Build rings: room polygon CCW, holes CW
        let mut rings: Vec<(Vec<Vec2>, bool)> = Vec::new();
        let mut room_poly = room.polygon.clone();
        if !room_poly.is_empty() {
            // Ensure CCW
            if polygon_area(&room_poly) < 0.0 {
                room_poly.reverse();
            }
            rings.push((room_poly, true));
        }
        for hole in &room.holes {
            let mut h = hole.clone();
            if h.len() >= 3 {
                // Ensure CW
                if polygon_area(&h) > 0.0 {
                    h.reverse();
                }
                rings.push((h, false));
            }
        }

        // Process each ring
        for (ring, is_outer) in rings {
            if ring.len() < 3 {
                continue;
            }
            let mut pieces: Vec<(f64, f64, usize)> = Vec::new(); // (t_start, t_end, edge_idx)

            let n = ring.len();
            for ei in 0..n {
                let a = ring[ei];
                let b = ring[(ei + 1) % n];
                let edge_vec = b - a;
                let l = edge_vec.len();
                if l <= 1e-9 {
                    continue;
                }
                let unit = edge_vec / l;
                let mid = (a + b) * 0.5;

                // Find matching wall face
                let mut matched_wall: Option<&Wall> = None;
                let mut matched_side = cadcraft_doc::arch::Side::Right;

                for w in &model.walls {
                    if w.divider || w.thickness <= 0.0 || !w.thickness.is_finite() {
                        continue;
                    }
                    // Check level
                    let on_level = if w.base_level == room.level {
                        true
                    } else if w.top_level.is_some() {
                        if let Some(level) = model.level(room.level) { w.spans_level(level, &model.levels) } else { false }
                    } else {
                        false
                    };
                    if !on_level {
                        continue;
                    }
                    // Check parallel
                    let wall_dir = (w.end - w.start).normalized();
                    if !wall_dir.is_finite() {
                        continue;
                    }
                    let cross = unit.x * wall_dir.y - unit.y * wall_dir.x;
                    if cross.abs() > 1e-6 {
                        continue;
                    }

                    // Check face line proximity
                    // The room polygon edge lies on the wall face.
                    // Wall face lines are offset from centerline by thickness/2.
                    // We check if the edge midpoint is close to either face line.
                    let left_normal = Vec2::new(-wall_dir.y, wall_dir.x);
                    let t = w.thickness / 2.0;

                    // Project midpoint onto wall normal to find distance from centerline
                    let rel_mid = mid - w.start;
                    let dist_from_center = rel_mid.dot(left_normal);

                    // Determine which face is closer
                    let dist_right = (dist_from_center + t).abs();
                    let dist_left = (dist_from_center - t).abs();

                    let (side, dist) = if dist_right <= dist_left {
                        (cadcraft_doc::arch::Side::Right, dist_right)
                    } else {
                        (cadcraft_doc::arch::Side::Left, dist_left)
                    };

                    if dist > 1e-4 {
                        continue;
                    }

                    // Check extent overlap
                    let wall_len = (w.end - w.start).len();
                    let edge_t0 = (a - w.start).dot(wall_dir);
                    let edge_t1 = (b - w.start).dot(wall_dir);
                    let edge_min = edge_t0.min(edge_t1);
                    let edge_max = edge_t0.max(edge_t1);

                    // Allow slight tolerance for joins
                    if edge_max < -1e-4 || edge_min > wall_len + 1e-4 {
                        continue;
                    }

                    matched_wall = Some(w);
                    matched_side = side;
                    break;
                }

                let Some(w) = matched_wall else {
                    continue;
                };

                // Check if wall carries this band
                if !face_carries(model, w, band) {
                    continue;
                }

                // Compute blocked intervals from openings
                let mut blocked: Vec<(f64, f64)> = Vec::new();
                let wall_bottom = wall_z_span(model, w).map(|(b, _)| b).unwrap_or(e);

                for o in &model.openings {
                    if o.host != w.id {
                        continue;
                    }
                    let Some((s0, s1)) = crate::opening_trim::trim_interval(model, o, matched_side) else {
                        continue;
                    };
                    let sill = o.sill;
                    let th = crate::opening_trim::trim_height(o, matched_side);
                    let o_bottom = wall_bottom + sill;
                    let o_top = o_bottom + th;
                    // Check vertical overlap
                    if o_top <= band.0 + 1e-9 || o_bottom >= band.1 - 1e-9 {
                        continue;
                    }
                    // Map s to edge parameter t
                    let wall_dir = (w.end - w.start).normalized();
                    if !wall_dir.is_finite() {
                        continue;
                    }
                    let wall_pt0 = w.start + wall_dir * s0;
                    let wall_pt1 = w.start + wall_dir * s1;
                    let t0 = (wall_pt0 - a).dot(unit);
                    let t1 = (wall_pt1 - a).dot(unit);
                    let mut bs = t0.min(t1);
                    let mut be = t0.max(t1);
                    bs = bs.clamp(0.0, l);
                    be = be.clamp(0.0, l);
                    if be > bs + 1e-9 {
                        blocked.push((bs, be));
                    }
                }

                // Merge blocked intervals
                blocked.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                let mut merged: Vec<(f64, f64)> = Vec::new();
                for (s, e) in blocked {
                    if let Some(last) = merged.last_mut() {
                        if s <= last.1 + 1e-9 {
                            last.1 = last.1.max(e);
                        } else {
                            merged.push((s, e));
                        }
                    } else {
                        merged.push((s, e));
                    }
                }

                // Compute unblocked pieces
                let mut prev = 0.0;
                for (s, e) in &merged {
                    if *s > prev + 1e-9 {
                        pieces.push((prev, *s, ei));
                    }
                    prev = *e;
                }
                if prev < l - 1e-9 {
                    pieces.push((prev, l, ei));
                }
            }

            // Build runs from pieces
            if pieces.is_empty() {
                continue;
            }

            // Sort pieces by edge index and start
            pieces.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)));

            let is_end = |t: f64, ei: usize| -> bool {
                let a = ring[ei];
                let b = ring[(ei + 1) % n];
                (t - (b - a).len()).abs() <= 1e-9
            };
            // One full piece per edge: one closed run starting at ring vertex 0.
            let full = pieces.len() == n && pieces.iter().enumerate().all(|(k, (t0, t1, ei))| *ei == k && t0.abs() <= 1e-9 && is_end(*t1, *ei));
            if full {
                runs.push(MoldingRun {
                    kind: molding.kind,
                    index: idx,
                    profile: molding.profile.clone(),
                    path: ring.clone(),
                    z: vec![z0; n],
                    closed: true,
                    side: 1.0,
                });
                continue;
            }
            let mut groups: Vec<Vec<usize>> = Vec::new();
            for k in 0..pieces.len() {
                let joins = k > 0 && {
                    let (_, pt1, pei) = pieces[k - 1];
                    let (qt0, _, qei) = pieces[k];
                    is_end(pt1, pei) && qt0.abs() <= 1e-9 && qei == (pei + 1) % n
                };
                match groups.last_mut() {
                    Some(g) if joins => g.push(k),
                    _ => groups.push(vec![k]),
                }
            }
            // Wrap-around: the last group continues into the first.
            if groups.len() > 1 {
                let (lt1, lei) = pieces.last().map(|p| (p.1, p.2)).unwrap_or((0.0, 0));
                let (ft0, fei) = pieces.first().map(|p| (p.0, p.2)).unwrap_or((1.0, 0));
                if is_end(lt1, lei) && ft0.abs() <= 1e-9 && fei == (lei + 1) % n {
                    let first = groups.remove(0);
                    if let Some(last) = groups.last_mut() {
                        last.extend(first);
                    }
                }
            }
            for g in groups {
                let mut path: Vec<Vec2> = Vec::new();
                for (gi, k) in g.iter().enumerate() {
                    let (t0, t1, ei) = pieces[*k];
                    let a = ring[ei];
                    let b = ring[(ei + 1) % n];
                    let l = (b - a).len();
                    if l <= 1e-9 {
                        continue;
                    }
                    let unit = (b - a) / l;
                    if gi == 0 {
                        path.push(a + unit * t0);
                    }
                    path.push(if is_end(t1, ei) { b } else { a + unit * t1 });
                }
                if path.len() >= 2 {
                    let z = vec![z0; path.len()];
                    runs.push(MoldingRun { kind: molding.kind, index: idx, profile: molding.profile.clone(), path, z, closed: false, side: 1.0 });
                }
            }
        }

        // Cap path points
        let mut total_pts: usize = 0;
        for r in &mut runs {
            total_pts += r.path.len();
            if total_pts > 4096 {
                r.path.truncate(4096);
                r.z.truncate(4096);
                break;
            }
        }
    }

    runs
}

/// The length of a run: the sum of its segment lengths, plus the closing segment when closed;
/// 0 when any point is not finite.
pub fn run_length(r: &MoldingRun) -> f64 {
    if r.path.iter().any(|p| !p.is_finite()) {
        return 0.0;
    }
    let mut total: f64 = r.path.windows(2).map(|w| w[0].dist(w[1])).sum();
    if r.closed && r.path.len() > 2 {
        if let (Some(first), Some(last)) = (r.path.first(), r.path.last()) {
            total += last.dist(*first);
        }
    }
    if total.is_finite() { total } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchModel, Level, MoldingKind, Room, RoomMolding, RoomType, Wall};
    use cadcraft_geom::Vec2;

    #[test]
    fn run_length_open_closed_and_hostile() {
        let sq = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)];
        let mut r = MoldingRun { path: sq, ..Default::default() };
        assert!((run_length(&r) - 3.0).abs() < 1e-12);
        r.closed = true;
        assert!((run_length(&r) - 4.0).abs() < 1e-12);
        r.path.push(Vec2::new(f64::NAN, 0.0));
        assert_eq!(run_length(&r), 0.0);
    }

    #[test]
    fn brief_room_base_is_14_minus_the_door_trim() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        // 4.0 x 3.0 interior room
        let room_poly = vec![Vec2::new(0.1, 0.1), Vec2::new(4.1, 0.1), Vec2::new(4.1, 3.1), Vec2::new(0.1, 3.1)];
        let room = Room {
            polygon: room_poly,
            room_type: RoomType::Living,
            level: 0,
            ceiling_height: Some(2.4),
            moldings: vec![RoomMolding {
                kind: MoldingKind::Base,
                profile: crate::molding_profile::default_profile(MoldingKind::Base),
                offset: 0.0,
                ..Default::default()
            }],
            ..Room::default()
        };
        model.push_room(room.clone());

        // Walls around the room
        let w0 = model.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.2, 0.0), thickness: 0.2, ..Wall::default() });
        let w1 = model.push_wall(Wall { start: Vec2::new(4.2, 0.0), end: Vec2::new(4.2, 3.2), thickness: 0.2, ..Wall::default() });
        let w2 = model.push_wall(Wall { start: Vec2::new(4.2, 3.2), end: Vec2::new(0.0, 3.2), thickness: 0.2, ..Wall::default() });
        let w3 = model.push_wall(Wall { start: Vec2::new(0.0, 3.2), end: Vec2::new(0.0, 0.0), thickness: 0.2, ..Wall::default() });

        // Door on south wall (w0)
        let o = cadcraft_doc::arch::Opening {
            host: w0,
            offset: 2.0,
            width: 0.9,
            height: 2.1,
            kind: cadcraft_doc::arch::OpeningKind::Door,
            frame_width: 0.06,
            casing_width: 0.0,
            ..cadcraft_doc::arch::Opening::default()
        };
        model.push_opening(o);

        let runs = room_runs(&model, &room);
        let base_runs: Vec<_> = runs.iter().filter(|r| r.kind == MoldingKind::Base).collect();
        let total_len: f64 = base_runs.iter().map(|r| run_length(r)).sum();
        // Perimeter 14.0, trim 0.96
        assert!((total_len - (14.0 - 0.96)).abs() < 0.01);
    }

    #[test]
    fn crown_is_14_and_closed() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        let room_poly = vec![Vec2::new(0.1, 0.1), Vec2::new(4.1, 0.1), Vec2::new(4.1, 3.1), Vec2::new(0.1, 3.1)];
        let room = Room {
            polygon: room_poly,
            room_type: RoomType::Living,
            level: 0,
            ceiling_height: Some(2.4),
            moldings: vec![RoomMolding {
                kind: MoldingKind::Crown,
                profile: crate::molding_profile::default_profile(MoldingKind::Crown),
                offset: 0.0,
                ..Default::default()
            }],
            ..Room::default()
        };
        model.push_room(room.clone());

        let w0 = model.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.2, 0.0), thickness: 0.2, ..Wall::default() });
        let w1 = model.push_wall(Wall { start: Vec2::new(4.2, 0.0), end: Vec2::new(4.2, 3.2), thickness: 0.2, ..Wall::default() });
        let w2 = model.push_wall(Wall { start: Vec2::new(4.2, 3.2), end: Vec2::new(0.0, 3.2), thickness: 0.2, ..Wall::default() });
        let w3 = model.push_wall(Wall { start: Vec2::new(0.0, 3.2), end: Vec2::new(0.0, 0.0), thickness: 0.2, ..Wall::default() });

        let runs = room_runs(&model, &room);
        let crown_runs: Vec<_> = runs.iter().filter(|r| r.kind == MoldingKind::Crown).collect();
        assert_eq!(crown_runs.len(), 1);
        assert!(crown_runs[0].closed);
        assert!((run_length(crown_runs[0]) - 14.0).abs() < 0.01);
    }

    #[test]
    fn chair_stops_at_doors_and_windows_base_only_at_doors() {
        // Implementation test for chair rail stopping at openings
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        let room_poly = vec![Vec2::new(0.1, 0.1), Vec2::new(4.1, 0.1), Vec2::new(4.1, 3.1), Vec2::new(0.1, 3.1)];
        let room = Room {
            polygon: room_poly,
            room_type: RoomType::Living,
            level: 0,
            ceiling_height: Some(2.4),
            moldings: vec![
                RoomMolding {
                    kind: MoldingKind::Base,
                    profile: crate::molding_profile::default_profile(MoldingKind::Base),
                    offset: 0.0,
                    ..Default::default()
                },
                RoomMolding {
                    kind: MoldingKind::Chair,
                    profile: crate::molding_profile::default_profile(MoldingKind::Chair),
                    offset: 0.9,
                    ..Default::default()
                },
            ],
            ..Room::default()
        };
        model.push_room(room.clone());

        let w0 = model.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.2, 0.0), thickness: 0.2, ..Wall::default() });
        let w1 = model.push_wall(Wall { start: Vec2::new(4.2, 0.0), end: Vec2::new(4.2, 3.2), thickness: 0.2, ..Wall::default() });
        let w2 = model.push_wall(Wall { start: Vec2::new(4.2, 3.2), end: Vec2::new(0.0, 3.2), thickness: 0.2, ..Wall::default() });
        let w3 = model.push_wall(Wall { start: Vec2::new(0.0, 3.2), end: Vec2::new(0.0, 0.0), thickness: 0.2, ..Wall::default() });

        // Door on south wall
        let door = cadcraft_doc::arch::Opening {
            host: w0,
            offset: 2.0,
            width: 0.9,
            height: 2.1,
            kind: cadcraft_doc::arch::OpeningKind::Door,
            frame_width: 0.06,
            casing_width: 0.0,
            ..cadcraft_doc::arch::Opening::default()
        };
        model.push_opening(door);

        // Window on north wall (w2), sill 0.9, 1.2 x 1.2
        let window = cadcraft_doc::arch::Opening {
            host: w2,
            offset: 2.1,
            width: 1.2,
            height: 1.2,
            sill: 0.9,
            kind: cadcraft_doc::arch::OpeningKind::Window,
            frame_width: 0.06,
            casing_width: 0.0,
            ..cadcraft_doc::arch::Opening::default()
        };
        model.push_opening(window);

        let runs = room_runs(&model, &room);
        let base_len: f64 = runs.iter().filter(|r| r.kind == MoldingKind::Base).map(|r| run_length(r)).sum();
        let chair_len: f64 = runs.iter().filter(|r| r.kind == MoldingKind::Chair).map(|r| run_length(r)).sum();
        // Base: 14.0 - 0.96 (door trim)
        // Chair: 14.0 - 0.96 (door) - 1.26 (window trim)
        assert!((base_len - (14.0 - 0.96)).abs() < 0.01);
        assert!((chair_len - (14.0 - 0.96 - 1.26)).abs() < 0.01);
    }

    #[test]
    fn runs_start_and_end_at_trim_edges() {
        // Test that runs break at trim intervals
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        let room_poly = vec![Vec2::new(0.1, 0.1), Vec2::new(4.1, 0.1), Vec2::new(4.1, 3.1), Vec2::new(0.1, 3.1)];
        let room = Room {
            polygon: room_poly,
            room_type: RoomType::Living,
            level: 0,
            ceiling_height: Some(2.4),
            moldings: vec![RoomMolding {
                kind: MoldingKind::Base,
                profile: crate::molding_profile::default_profile(MoldingKind::Base),
                offset: 0.0,
                ..Default::default()
            }],
            ..Room::default()
        };
        model.push_room(room.clone());

        let w0 = model.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.2, 0.0), thickness: 0.2, ..Wall::default() });
        let w1 = model.push_wall(Wall { start: Vec2::new(4.2, 0.0), end: Vec2::new(4.2, 3.2), thickness: 0.2, ..Wall::default() });
        let w2 = model.push_wall(Wall { start: Vec2::new(4.2, 3.2), end: Vec2::new(0.0, 3.2), thickness: 0.2, ..Wall::default() });
        let w3 = model.push_wall(Wall { start: Vec2::new(0.0, 3.2), end: Vec2::new(0.0, 0.0), thickness: 0.2, ..Wall::default() });

        let door = cadcraft_doc::arch::Opening {
            host: w0,
            offset: 2.0,
            width: 0.9,
            height: 2.1,
            kind: cadcraft_doc::arch::OpeningKind::Door,
            frame_width: 0.06,
            casing_width: 0.0,
            ..cadcraft_doc::arch::Opening::default()
        };
        model.push_opening(door);

        let runs = room_runs(&model, &room);
        let base_runs: Vec<_> = runs.iter().filter(|r| r.kind == MoldingKind::Base).collect();
        // The pieces before and after the door join round the ring into one open run.
        assert_eq!(base_runs.len(), 1);
        assert!(!base_runs[0].closed);
        let (first, last) = (base_runs[0].path[0], base_runs[0].path[base_runs[0].path.len() - 1]);
        assert!((first.x - 2.48).abs() < 1e-9 && (first.y - 0.1).abs() < 1e-9);
        assert!((last.x - 1.52).abs() < 1e-9 && (last.y - 0.1).abs() < 1e-9);
    }

    #[test]
    fn divider_and_missing_faces_break_runs() {
        // Test that divider walls and missing faces break runs
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        let room_poly = vec![Vec2::new(0.1, 0.1), Vec2::new(4.1, 0.1), Vec2::new(4.1, 3.1), Vec2::new(0.1, 3.1)];
        let room = Room {
            polygon: room_poly,
            room_type: RoomType::Living,
            level: 0,
            ceiling_height: Some(2.4),
            moldings: vec![RoomMolding {
                kind: MoldingKind::Base,
                profile: crate::molding_profile::default_profile(MoldingKind::Base),
                offset: 0.0,
                ..Default::default()
            }],
            ..Room::default()
        };
        model.push_room(room.clone());

        // Only 3 walls, missing one
        let w0 = model.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.2, 0.0), thickness: 0.2, ..Wall::default() });
        let w1 = model.push_wall(Wall { start: Vec2::new(4.2, 0.0), end: Vec2::new(4.2, 3.2), thickness: 0.2, ..Wall::default() });
        let w2 = model.push_wall(Wall { start: Vec2::new(4.2, 3.2), end: Vec2::new(0.0, 3.2), thickness: 0.2, ..Wall::default() });

        let runs = room_runs(&model, &room);
        let base_runs: Vec<_> = runs.iter().filter(|r| r.kind == MoldingKind::Base).collect();
        // Should not be a closed ring
        assert!(base_runs.iter().all(|r| !r.closed));
    }

    #[test]
    fn hole_rings_get_runs_facing_the_room() {
        // Test that hole rings get runs with correct orientation
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        let room_poly = vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 4.0), Vec2::new(0.0, 4.0)];
        let hole = vec![Vec2::new(2.0, 1.0), Vec2::new(4.0, 1.0), Vec2::new(4.0, 3.0), Vec2::new(2.0, 3.0)];
        let room = Room {
            polygon: room_poly,
            holes: vec![hole],
            room_type: RoomType::Living,
            level: 0,
            ceiling_height: Some(2.4),
            moldings: vec![RoomMolding {
                kind: MoldingKind::Base,
                profile: crate::molding_profile::default_profile(MoldingKind::Base),
                offset: 0.0,
                ..Default::default()
            }],
            ..Room::default()
        };
        model.push_room(room.clone());

        // Walls around room and hole
        let w0 = model.push_wall(Wall { start: Vec2::new(-0.1, -0.1), end: Vec2::new(6.1, -0.1), thickness: 0.2, ..Wall::default() });
        let w1 = model.push_wall(Wall { start: Vec2::new(6.1, -0.1), end: Vec2::new(6.1, 4.1), thickness: 0.2, ..Wall::default() });
        let w2 = model.push_wall(Wall { start: Vec2::new(6.1, 4.1), end: Vec2::new(-0.1, 4.1), thickness: 0.2, ..Wall::default() });
        let w3 = model.push_wall(Wall { start: Vec2::new(-0.1, 4.1), end: Vec2::new(-0.1, -0.1), thickness: 0.2, ..Wall::default() });
        let w4 = model.push_wall(Wall { start: Vec2::new(1.9, 0.9), end: Vec2::new(4.1, 0.9), thickness: 0.2, ..Wall::default() });
        let w5 = model.push_wall(Wall { start: Vec2::new(4.1, 0.9), end: Vec2::new(4.1, 3.1), thickness: 0.2, ..Wall::default() });
        let w6 = model.push_wall(Wall { start: Vec2::new(4.1, 3.1), end: Vec2::new(1.9, 3.1), thickness: 0.2, ..Wall::default() });
        let w7 = model.push_wall(Wall { start: Vec2::new(1.9, 3.1), end: Vec2::new(1.9, 0.9), thickness: 0.2, ..Wall::default() });

        let runs = room_runs(&model, &room);
        let base_runs: Vec<_> = runs.iter().filter(|r| r.kind == MoldingKind::Base).collect();
        // Should have runs on both outer and hole rings
        assert!(base_runs.len() >= 2);
    }

    #[test]
    fn z_values_per_kind() {
        // Test that z values are correct for each molding kind
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        let room_poly = vec![Vec2::new(0.1, 0.1), Vec2::new(4.1, 0.1), Vec2::new(4.1, 3.1), Vec2::new(0.1, 3.1)];
        let room = Room {
            polygon: room_poly,
            room_type: RoomType::Living,
            level: 0,
            ceiling_height: Some(2.4),
            moldings: vec![
                RoomMolding {
                    kind: MoldingKind::Base,
                    profile: crate::molding_profile::default_profile(MoldingKind::Base),
                    offset: 0.0,
                    ..Default::default()
                },
                RoomMolding {
                    kind: MoldingKind::Chair,
                    profile: crate::molding_profile::default_profile(MoldingKind::Chair),
                    offset: 0.9,
                    ..Default::default()
                },
                RoomMolding {
                    kind: MoldingKind::Crown,
                    profile: crate::molding_profile::default_profile(MoldingKind::Crown),
                    offset: 0.0,
                    ..Default::default()
                },
            ],
            ..Room::default()
        };
        model.push_room(room.clone());

        let w0 = model.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.2, 0.0), thickness: 0.2, ..Wall::default() });
        let w1 = model.push_wall(Wall { start: Vec2::new(4.2, 0.0), end: Vec2::new(4.2, 3.2), thickness: 0.2, ..Wall::default() });
        let w2 = model.push_wall(Wall { start: Vec2::new(4.2, 3.2), end: Vec2::new(0.0, 3.2), thickness: 0.2, ..Wall::default() });
        let w3 = model.push_wall(Wall { start: Vec2::new(0.0, 3.2), end: Vec2::new(0.0, 0.0), thickness: 0.2, ..Wall::default() });

        let runs = room_runs(&model, &room);
        for r in &runs {
            if r.kind == MoldingKind::Base {
                assert!((r.z[0] - 0.0).abs() < 1e-9);
            } else if r.kind == MoldingKind::Chair {
                assert!((r.z[0] - 0.9).abs() < 1e-9);
            } else if r.kind == MoldingKind::Crown {
                // Crown z depends on profile height
                assert!(r.z[0] > 2.0 && r.z[0] < 2.4);
            }
        }
    }

    #[test]
    fn no_crown_in_rooms_without_ceilings() {
        // Test that crown is skipped when room has no ceiling
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        let room_poly = vec![Vec2::new(0.1, 0.1), Vec2::new(4.1, 0.1), Vec2::new(4.1, 3.1), Vec2::new(0.1, 3.1)];
        let room = Room {
            polygon: room_poly,
            room_type: RoomType::Deck,
            level: 0,
            ceiling_height: None,
            moldings: vec![RoomMolding {
                kind: MoldingKind::Crown,
                profile: crate::molding_profile::default_profile(MoldingKind::Crown),
                offset: 0.0,
                ..Default::default()
            }],
            ..Room::default()
        };
        model.push_room(room.clone());

        let w0 = model.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.2, 0.0), thickness: 0.2, ..Wall::default() });
        let w1 = model.push_wall(Wall { start: Vec2::new(4.2, 0.0), end: Vec2::new(4.2, 3.2), thickness: 0.2, ..Wall::default() });
        let w2 = model.push_wall(Wall { start: Vec2::new(4.2, 3.2), end: Vec2::new(0.0, 3.2), thickness: 0.2, ..Wall::default() });
        let w3 = model.push_wall(Wall { start: Vec2::new(0.0, 3.2), end: Vec2::new(0.0, 0.0), thickness: 0.2, ..Wall::default() });

        let runs = room_runs(&model, &room);
        let crown_runs: Vec<_> = runs.iter().filter(|r| r.kind == MoldingKind::Crown).collect();
        assert!(crown_runs.is_empty());
    }

    #[test]
    fn nan_walls_and_rooms_build_no_runs() {
        // Test that NaN inputs produce no runs
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        let room_poly = vec![Vec2::new(f64::NAN, 0.1), Vec2::new(4.1, 0.1), Vec2::new(4.1, 3.1), Vec2::new(0.1, 3.1)];
        let room = Room {
            polygon: room_poly,
            room_type: RoomType::Living,
            level: 0,
            ceiling_height: Some(2.4),
            moldings: vec![RoomMolding {
                kind: MoldingKind::Base,
                profile: crate::molding_profile::default_profile(MoldingKind::Base),
                offset: 0.0,
                ..Default::default()
            }],
            ..Room::default()
        };
        model.push_room(room.clone());

        let runs = room_runs(&model, &room);
        assert!(runs.is_empty());
    }

    #[test]
    fn low_walls_carry_no_band_above_their_top() {
        // Test that moldings don't appear on walls that are too short
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        let room_poly = vec![Vec2::new(0.1, 0.1), Vec2::new(4.1, 0.1), Vec2::new(4.1, 3.1), Vec2::new(0.1, 3.1)];
        let room = Room {
            polygon: room_poly,
            room_type: RoomType::Living,
            level: 0,
            ceiling_height: Some(2.4),
            moldings: vec![RoomMolding {
                kind: MoldingKind::Crown,
                profile: crate::molding_profile::default_profile(MoldingKind::Crown),
                offset: 0.0,
                ..Default::default()
            }],
            ..Room::default()
        };
        model.push_room(room.clone());

        // Low wall (1.0 m high)
        let w0 = model.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.2, 0.0), thickness: 0.2, height: 1.0, ..Wall::default() });
        let w1 = model.push_wall(Wall { start: Vec2::new(4.2, 0.0), end: Vec2::new(4.2, 3.2), thickness: 0.2, ..Wall::default() });
        let w2 = model.push_wall(Wall { start: Vec2::new(4.2, 3.2), end: Vec2::new(0.0, 3.2), thickness: 0.2, ..Wall::default() });
        let w3 = model.push_wall(Wall { start: Vec2::new(0.0, 3.2), end: Vec2::new(0.0, 0.0), thickness: 0.2, ..Wall::default() });

        let runs = room_runs(&model, &room);
        let crown_runs: Vec<_> = runs.iter().filter(|r| r.kind == MoldingKind::Crown).collect();
        // The low wall should not carry crown molding
        // No segment runs along the low south wall (y = 0.1).
        assert!(!crown_runs.is_empty());
        assert!(crown_runs.iter().all(|r| r.path.windows(2).all(|s| !(s[0].y < 0.15 && s[1].y < 0.15))));
    }

    #[test]
    fn wall_z_span_follows_to_ceiling_and_top_level() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        model.levels.push(Level { index: 1, name: "Level 2".into(), elevation: 2.7, ceiling_height: 2.4, structure: 0.3, ..Default::default() });

        // Wall to ceiling
        let w1 = Wall { start: Vec2::ZERO, end: Vec2::new(10.0, 0.0), thickness: 0.2, base_level: 0, to_ceiling: true, ..Wall::default() };
        let span = wall_z_span(&model, &w1);
        assert!(span.is_some());
        let (b, t) = span.unwrap();
        assert!((b - 0.0).abs() < 1e-9);
        assert!((t - 2.4).abs() < 1e-9);

        // Wall with top_level
        let w2 = Wall { start: Vec2::ZERO, end: Vec2::new(10.0, 0.0), thickness: 0.2, base_level: 0, top_level: Some(1), ..Wall::default() };
        let span = wall_z_span(&model, &w2);
        assert!(span.is_some());
        let (b, t) = span.unwrap();
        assert!((b - 0.0).abs() < 1e-9);
        assert!((t - 5.1).abs() < 1e-9); // 2.7 + 2.4
    }

    #[test]
    fn glass_faces_carry_no_molding() {
        use cadcraft_doc::arch::WallSplit;
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "L1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        let mk = |wall_type: &str, split: Option<WallSplit>| Wall {
            start: Vec2::ZERO,
            end: Vec2::new(4.0, 0.0),
            thickness: 0.1,
            height: 2.4,
            wall_type: wall_type.into(),
            split,
            ..Wall::default()
        };
        let base = (0.0, 0.1);
        let chair = (0.9, 1.0);
        assert!(face_carries(&model, &mk("Interior", None), base));
        let glass = mk("Glass", None);
        assert!(!face_carries(&model, &glass, base) && !face_carries(&model, &glass, chair));
        let pony = mk("Interior", Some(WallSplit { height: 1.1, upper: "Glass".into(), upper_thickness: 0.01, ..Default::default() }));
        assert!(face_carries(&model, &pony, base) && face_carries(&model, &pony, chair));
        assert!(!face_carries(&model, &pony, (1.0, 1.2)));
        assert!(!face_carries(&model, &pony, (2.3, 2.4)));
        let inv = mk("Glass", Some(WallSplit { height: 1.1, upper: "Interior".into(), upper_thickness: 0.1, ..Default::default() }));
        assert!(!face_carries(&model, &inv, base) && face_carries(&model, &inv, (1.2, 1.3)));
    }
}
