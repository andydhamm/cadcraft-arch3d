//! Automatic railings: the railings a stair or a deck should have now, and the regeneration that
//! replaces the host's previous automatic ones. Drawn railings and a stair's well guard are never
//! touched.

use crate::stair::{GUARD_HEIGHT, MAX_RISER, MIN_TREAD, plan_stair};
use cadcraft_doc::arch::{MAX_RAILINGS, RailSetting, Railing, RailingHost, RailingStyle, Room};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::Vec2;

/// What an automatic railing follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailHostRef {
    /// A stair (the railings on the sides of its flight).
    Stair(ArchId),
    /// A deck room.
    Deck(ArchId),
}

impl RailHostRef {
    fn id(self) -> ArchId {
        match self {
            RailHostRef::Stair(i) | RailHostRef::Deck(i) => i,
        }
    }

    fn matches(self, host: RailingHost) -> bool {
        match self {
            RailHostRef::Stair(_) => matches!(host, RailingHost::StairLeft | RailingHost::StairRight),
            RailHostRef::Deck(_) => host == RailingHost::Deck,
        }
    }
}

/// The railings the host should have now, ids unset.
pub fn auto_railings(model: &ArchModel, host: RailHostRef) -> Vec<Railing> {
    match host {
        RailHostRef::Stair(id) => stair_railings(model, id),
        RailHostRef::Deck(id) => deck_railings(model, id),
    }
}

/// Remove every automatic railing of the host (`auto`, following the host, of the host's kind;
/// a stair's well guard and drawn railings stay), then add [`auto_railings`] up to
/// `MAX_RAILINGS`. Returns the new ids in order.
pub fn regenerate(model: &mut ArchModel, host: RailHostRef) -> Vec<ArchId> {
    let id = host.id();
    model.railings.retain(|r| !(r.auto && r.follows == Some(id) && host.matches(r.host)));
    let mut ids = Vec::new();
    for r in auto_railings(model, host) {
        if model.railings.len() >= MAX_RAILINGS {
            break;
        }
        ids.push(model.push_railing(r));
    }
    ids
}

/// Calculate automatic railings for a stair.
fn stair_railings(model: &ArchModel, id: ArchId) -> Vec<Railing> {
    let Some(stair) = model.stairs.iter().find(|s| s.id == id) else {
        return Vec::new();
    };

    // Check if the stair has a flight (geometry exists)
    // We use the flight polygon to verify existence and get run/width
    let plan = plan_stair(stair.rise, stair.width, Some(stair.tread_depth), MAX_RISER, MIN_TREAD);
    let Some(plan) = plan else {
        return Vec::new();
    };

    // Determine which sides are against walls
    let against = stair_sides_against_walls(model, stair);

    let mut rails = Vec::new();

    // Left side (t = +w/2)
    let want_left = match stair.rail_left {
        RailSetting::On => true,
        RailSetting::Off => false,
        RailSetting::Auto => !against[0],
    };

    if want_left {
        let t = stair.width / 2.0;
        let inset = -0.045;
        let path = vec![stair_point(stair, 0.0, t + inset), stair_point(stair, plan.run, t + inset)];
        rails.push(Railing {
            path,
            level: stair.from_level,
            height: GUARD_HEIGHT,
            style: RailingStyle::Panel,
            follows: Some(stair.id),
            host: RailingHost::StairLeft,
            auto: true,
            inset,
            layer: "A-RAIL".into(),
            ..Railing::default()
        });
    }

    // Right side (t = -w/2)
    let want_right = match stair.rail_right {
        RailSetting::On => true,
        RailSetting::Off => false,
        RailSetting::Auto => !against[1],
    };

    if want_right {
        let t = -stair.width / 2.0;
        let inset = 0.045;
        let path = vec![stair_point(stair, 0.0, t + inset), stair_point(stair, plan.run, t + inset)];
        rails.push(Railing {
            path,
            level: stair.from_level,
            height: GUARD_HEIGHT,
            style: RailingStyle::Panel,
            follows: Some(stair.id),
            host: RailingHost::StairRight,
            auto: true,
            inset,
            layer: "A-RAIL".into(),
            ..Railing::default()
        });
    }

    rails
}

/// Calculate automatic railings for a deck.
fn deck_railings(model: &ArchModel, id: ArchId) -> Vec<Railing> {
    let Some(room) = model.rooms.iter().find(|r| r.id == id) else {
        return Vec::new();
    };

    // Deck must have a valid height and polygon
    let Some(height) = room.deck_height else {
        return Vec::new();
    };
    if height <= 0.0 || !height.is_finite() {
        return Vec::new();
    }
    if room.polygon.len() < 3 {
        return Vec::new();
    }

    let edges = deck_edges(model, room);
    if edges.is_empty() {
        return Vec::new();
    }

    let n = edges.len();
    let mut railed = vec![false; n];
    for i in 0..n {
        let setting = room.deck_rails.get(i).copied().unwrap_or(RailSetting::Auto);
        railed[i] = match setting {
            RailSetting::On => true,
            RailSetting::Off => false,
            RailSetting::Auto => edges[i].wall.is_none(),
        };
    }

    let mut paths = Vec::new();
    let all_railed = railed.iter().all(|&r| r);

    if all_railed {
        // One closed path
        let mut path: Vec<Vec2> = room.polygon.clone();
        if let Some(first) = room.polygon.first() {
            path.push(*first);
        }
        paths.push(path);
    } else {
        // Find runs of consecutive railed edges
        let mut runs = Vec::new();
        let mut current_run = Vec::new();
        let mut in_run = false;

        // We iterate twice to handle cyclic nature if needed, but simpler to just find start points
        // A run starts at an edge i where railed[i] is true and railed[i-1] is false.
        for i in 0..n {
            let prev = (i + n - 1) % n;
            if railed[i] && !railed[prev] {
                // Start of a new run
                if !current_run.is_empty() {
                    runs.push(current_run.clone());
                }
                current_run.clear();
                current_run.push(i);
                in_run = true;
            } else if railed[i] && in_run {
                current_run.push(i);
            } else if !railed[i] && in_run {
                runs.push(current_run.clone());
                current_run.clear();
                in_run = false;
            }
        }
        if !current_run.is_empty() {
            runs.push(current_run);
        }

        for run in runs {
            if run.is_empty() {
                continue;
            }
            let start_idx = run[0];
            let end_idx = *run.last().unwrap();

            // Build path from start_idx to end_idx
            // Path vertices are polygon vertices.
            // Edge i connects polygon[i] to polygon[(i+1)%n]
            // Run covers edges start_idx .. end_idx
            // Path vertices: polygon[start_idx], polygon[start_idx+1], ..., polygon[end_idx+1]

            let mut path = Vec::new();
            let mut curr = start_idx;
            loop {
                path.push(room.polygon[curr]);
                if curr == end_idx {
                    path.push(room.polygon[(curr + 1) % n]);
                    break;
                }
                curr = (curr + 1) % n;
            }

            // Pull back ends if they meet an unrailed edge touching a wall
            // Start end: edge start_idx-1 (mod n). If it exists, is unrailed, and has wall, pull back.
            let prev_edge_idx = (start_idx + n - 1) % n;
            if !railed[prev_edge_idx] && edges[prev_edge_idx].wall.is_some() {
                // Pull back start point along the first edge of the run
                let p0 = path[0];
                let p1 = path[1];
                let dir = (p1 - p0).normalized();
                if dir.is_finite() {
                    path[0] = p0 + dir * 0.045;
                }
            }

            // End end: edge end_idx+1 (mod n). If it exists, is unrailed, and has wall, pull back.
            let next_edge_idx = (end_idx + 1) % n;
            if !railed[next_edge_idx] && edges[next_edge_idx].wall.is_some() {
                // Pull back end point along the last edge of the run
                let p_last = *path.last().unwrap();
                let p_prev = path[path.len() - 2];
                let dir = (p_prev - p_last).normalized();
                if dir.is_finite() {
                    let new_last = p_last + dir * 0.045;
                    path.pop();
                    path.push(new_last);
                }
            }

            // Filter short paths
            if path.len() >= 2 {
                let mut valid = true;
                for i in 0..path.len() - 1 {
                    if (path[i + 1] - path[i]).len() < 0.05 {
                        valid = false;
                        break;
                    }
                }
                if valid {
                    paths.push(path);
                }
            }
        }
    }

    let mut rails = Vec::new();
    for path in paths {
        rails.push(Railing {
            path,
            level: room.level,
            height: GUARD_HEIGHT,
            follows: Some(room.id),
            host: RailingHost::Deck,
            auto: true,
            inset: 0.045,
            layer: "A-RAIL".into(),
            ..Railing::default()
        });
    }
    rails
}

/// Helper to get a point on the stair flight.
/// u is distance along direction, t is distance to left.
fn stair_point(stair: &cadcraft_doc::arch::Stair, u: f64, t: f64) -> Vec2 {
    let dir = Vec2::from_angle(stair.direction);
    let left = Vec2::new(-dir.y, dir.x);
    stair.start + dir * u + left * t
}

/// Determine which sides of a stair are against walls.
/// Returns [left_against, right_against].
fn stair_sides_against_walls(model: &ArchModel, stair: &cadcraft_doc::arch::Stair) -> [bool; 2] {
    let dir = Vec2::from_angle(stair.direction);
    let left = Vec2::new(-dir.y, dir.x);
    let w = stair.width;
    let run = stair.run;

    let left_line_start = stair_point(stair, 0.0, w / 2.0);
    let left_line_end = stair_point(stair, run, w / 2.0);
    let right_line_start = stair_point(stair, 0.0, -w / 2.0);
    let right_line_end = stair_point(stair, run, -w / 2.0);

    let mut left_against = false;
    let mut right_against = false;

    // Threshold for "against"
    const THRESHOLD: f64 = 0.1;

    // Check walls
    for wall in &model.walls {
        let w_start = wall.start;
        let w_end = wall.end;

        // Check left side
        if !left_against && segments_close(w_start, w_end, left_line_start, left_line_end, THRESHOLD) {
            left_against = true;
        }
        // Check right side
        if !right_against && segments_close(w_start, w_end, right_line_start, right_line_end, THRESHOLD) {
            right_against = true;
        }
    }

    [left_against, right_against]
}

/// Check if two segments are close to each other.
/// Returns true if the minimum distance between the segments is less than `threshold`.
fn segments_close(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2, threshold: f64) -> bool {
    // Check if endpoints of one segment are close to the other segment
    if dist_to_segment(a1, b1, b2) < threshold || dist_to_segment(a2, b1, b2) < threshold {
        return true;
    }
    if dist_to_segment(b1, a1, a2) < threshold || dist_to_segment(b2, a1, a2) < threshold {
        return true;
    }
    // Check if segments intersect
    if segments_intersect(a1, a2, b1, b2) {
        return true;
    }
    false
}

/// Check if two segments intersect.
fn segments_intersect(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> bool {
    let d1 = a2 - a1;
    let d2 = b2 - b1;
    let cross = d1.x * d2.y - d1.y * d2.x;
    if cross.abs() < 1e-9 {
        // Parallel or collinear
        // Check perpendicular distance to ensure they are collinear
        let n = Vec2::new(-d1.y, d1.x).normalized();
        if n.is_finite() {
            let dist = (b1 - a1).dot(n).abs();
            if dist > 1e-6 {
                return false;
            }
        } else {
            return false;
        }
        // Collinear: check overlap
        let t1 = (b1 - a1).dot(d1) / d1.dot(d1);
        let t2 = (b2 - a1).dot(d1) / d1.dot(d1);
        let min_t = t1.min(t2);
        let max_t = t1.max(t2);
        // Overlap if [min_t, max_t] intersects [0, 1]
        return max_t >= 0.0 && min_t <= 1.0;
    }
    let t = ((b1 - a1).x * d2.y - (b1 - a1).y * d2.x) / cross;
    let u = ((b1 - a1).x * d1.y - (b1 - a1).y * d1.x) / cross;
    t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0
}

fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let ap = p - a;
    let len_sq = ab.dot(ab);
    if len_sq == 0.0 {
        return ap.len();
    }
    let mut t = ap.dot(ab) / len_sq;
    t = t.clamp(0.0, 1.0);
    let proj = a + ab * t;
    (p - proj).len()
}

/// Deck edge info.
struct DeckEdge {
    wall: Option<ArchId>,
}

/// Calculate deck edges and their wall associations.
fn deck_edges(model: &ArchModel, room: &Room) -> Vec<DeckEdge> {
    let n = room.polygon.len();
    if n < 3 {
        return Vec::new();
    }
    let mut edges = Vec::with_capacity(n);
    for i in 0..n {
        let p1 = room.polygon[i];
        let p2 = room.polygon[(i + 1) % n];

        // Check if any wall is close to this edge
        let mut wall_id = None;
        for wall in &model.walls {
            // Check if wall segment overlaps or is close to deck edge
            // Simplified check: midpoint distance
            let mid = (p1 + p2) * 0.5;
            let w_mid = (wall.start + wall.end) * 0.5;
            if (mid - w_mid).len() < 0.1 {
                wall_id = Some(wall.id);
                break;
            }
        }
        edges.push(DeckEdge { wall: wall_id });
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{Room, RoomType, Stair, StairShape};

    fn rail(host: RailingHost, follows: u32, auto: bool) -> Railing {
        Railing { host, follows: Some(ArchId(follows)), auto, ..Railing::default() }
    }

    #[test]
    fn regenerate_replaces_only_the_hosts_auto_railings() {
        let mut m = ArchModel::default();
        m.push_railing(rail(RailingHost::StairLeft, 0, true));
        m.push_railing(rail(RailingHost::StairRight, 0, true));
        m.push_railing(rail(RailingHost::Well, 0, false));
        m.push_railing(rail(RailingHost::StairLeft, 1, true));
        m.push_railing(rail(RailingHost::Deck, 0, true));
        m.push_railing(Railing::default());
        // The stub makes none: only stair 0's two automatic railings go.
        let ids = regenerate(&mut m, RailHostRef::Stair(ArchId(0)));
        assert!(ids.is_empty());
        assert_eq!(m.railings.len(), 4);
        assert!(m.railings.iter().any(|r| r.host == RailingHost::Well));
        assert!(m.railings.iter().any(|r| r.host == RailingHost::Deck && r.auto));
        assert!(m.railings.iter().any(|r| r.follows == Some(ArchId(1))));
        regenerate(&mut m, RailHostRef::Deck(ArchId(0)));
        assert_eq!(m.railings.len(), 3);
    }

    #[test]
    fn regenerate_respects_max_railings() {
        let mut m = ArchModel::default();
        for _ in 0..MAX_RAILINGS {
            m.push_railing(Railing::default());
        }
        let ids = regenerate(&mut m, RailHostRef::Stair(ArchId(0)));
        assert!(ids.is_empty());
        assert_eq!(m.railings.len(), MAX_RAILINGS);
    }
}
