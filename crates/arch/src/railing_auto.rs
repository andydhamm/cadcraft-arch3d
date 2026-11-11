//! Automatic railings: the railings a stair or a deck should have now, and the regeneration that
//! replaces the host's previous automatic ones. Drawn railings and a stair's well guard are never
//! touched.

use crate::stair::{GUARD_HEIGHT, MAX_RISER, MIN_TREAD, plan_stair};
use cadcraft_doc::arch::{MAX_RAILINGS, RailSetting, Railing, RailingHost, RailingStyle};
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
        let path = vec![stair_point(stair, 0.0, t), stair_point(stair, plan.run, t)];
        rails.push(Railing {
            path,
            level: stair.from_level,
            height: GUARD_HEIGHT,
            style: RailingStyle::Panel,
            follows: Some(stair.id),
            host: RailingHost::StairLeft,
            auto: true,
            inset: -0.045,
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
        let path = vec![stair_point(stair, 0.0, t), stair_point(stair, plan.run, t)];
        rails.push(Railing {
            path,
            level: stair.from_level,
            height: GUARD_HEIGHT,
            style: RailingStyle::Panel,
            follows: Some(stair.id),
            host: RailingHost::StairRight,
            auto: true,
            inset: 0.045,
            layer: "A-RAIL".into(),
            ..Railing::default()
        });
    }

    rails
}

/// Calculate automatic railings for a deck.
fn deck_railings(model: &ArchModel, id: ArchId) -> Vec<Railing> {
    const INSET: f64 = 0.045;
    let Some(room) = model.rooms.iter().find(|r| r.id == id) else {
        return Vec::new();
    };
    let Some(height) = room.deck_height else {
        return Vec::new();
    };
    if height <= 0.0 || !height.is_finite() || room.polygon.len() < 3 {
        return Vec::new();
    }
    let edges = crate::deck::deck_edges(model, room);
    let n = edges.len();
    if n == 0 || room.polygon.iter().any(|p| !p.is_finite()) {
        return Vec::new();
    }

    let railed: Vec<bool> = edges
        .iter()
        .enumerate()
        .map(|(i, e)| match room.deck_rails.get(i).copied().unwrap_or(RailSetting::Auto) {
            RailSetting::On => true,
            RailSetting::Off => false,
            RailSetting::Auto => e.wall.is_none(),
        })
        .collect();
    let at = |i: usize| railed.get(i).copied().unwrap_or(false);
    let touches_wall = |i: usize| edges.get(i).is_some_and(|e| e.wall.is_some());

    let mut paths: Vec<Vec<Vec2>> = Vec::new();
    if railed.iter().all(|&r| r) {
        let mut path = room.polygon.clone();
        if let Some(first) = room.polygon.first() {
            path.push(*first);
        }
        paths.push(path);
    } else {
        for start in 0..n {
            if !(at(start) && !at((start + n - 1) % n)) {
                continue;
            }
            // Walk the run cyclically.
            let mut last = start;
            for k in 1..n {
                if at((start + k) % n) {
                    last = (start + k) % n;
                } else {
                    break;
                }
            }
            let count = (last + n - start) % n + 1;
            let mut path: Vec<Vec2> = Vec::with_capacity(count + 1);
            for k in 0..=count {
                if let Some(p) = room.polygon.get((start + k) % n) {
                    path.push(*p);
                }
            }
            if path.len() < 2 {
                continue;
            }
            // Pull an end back from a corner where the neighbouring edge is a wall.
            if touches_wall((start + n - 1) % n)
                && let (Some(&p0), Some(&p1)) = (path.first(), path.get(1))
            {
                let dir = (p1 - p0).normalized();
                if dir.is_finite()
                    && let Some(f) = path.first_mut()
                {
                    *f = p0 + dir * INSET;
                }
            }
            if touches_wall((last + 1) % n) {
                let prev = path.len().checked_sub(2).and_then(|i| path.get(i)).copied();
                if let (Some(&pl), Some(pp)) = (path.last(), prev) {
                    let dir = (pp - pl).normalized();
                    if dir.is_finite()
                        && let Some(l) = path.last_mut()
                    {
                        *l = pl + dir * INSET;
                    }
                }
            }
            if path.windows(2).all(|w| match w {
                [a, b] => (*b - *a).len() >= 0.05,
                _ => true,
            }) {
                paths.push(path);
            }
        }
    }

    // Signed area: counter-clockwise means the left of travel is inside.
    let mut area2 = 0.0;
    for (i, a) in room.polygon.iter().enumerate() {
        if let Some(b) = room.polygon.get((i + 1) % room.polygon.len()) {
            area2 += a.cross(*b);
        }
    }
    let inset = if area2 >= 0.0 { INSET } else { -INSET };

    paths
        .into_iter()
        .map(|path| Railing {
            path,
            level: room.level,
            height: GUARD_HEIGHT,
            follows: Some(room.id),
            host: RailingHost::Deck,
            auto: true,
            inset,
            layer: "A-RAIL".into(),
            ..Railing::default()
        })
        .collect()
}

/// Helper to get a point on the stair flight.
/// u is distance along direction, t is distance to left.
fn stair_point(stair: &cadcraft_doc::arch::Stair, u: f64, t: f64) -> Vec2 {
    let dir = Vec2::from_angle(stair.direction);
    let left = Vec2::new(-dir.y, dir.x);
    stair.start + dir * u + left * t
}

/// Determine which sides of a stair are against walls: a wall face parallel to the side line,
/// within 0.05 m of it, and overlapping at least half of its length.
/// Returns [left_against, right_against].
fn stair_sides_against_walls(model: &ArchModel, stair: &cadcraft_doc::arch::Stair) -> [bool; 2] {
    let half = stair.width / 2.0;
    [side_against(model, stair, half), side_against(model, stair, -half)]
}

/// Whether the side line of the flight at offset `t` lies against a wall face.
fn side_against(model: &ArchModel, stair: &cadcraft_doc::arch::Stair, t: f64) -> bool {
    let s0 = stair_point(stair, 0.0, t);
    let s1 = stair_point(stair, stair.run, t);
    let d = s1 - s0;
    let len = d.len();
    if !(s0.is_finite() && s1.is_finite() && len.is_finite() && len > 1e-9) {
        return false;
    }
    let dir = d / len;
    let normal = dir.perp();
    let level = model.level(stair.from_level);
    for w in &model.walls {
        if w.divider {
            continue;
        }
        if !(w.base_level == stair.from_level || level.is_some_and(|l| w.spans_level(l, &model.levels))) {
            continue;
        }
        let Some(faces) = crate::join::face_lines(w) else { continue };
        for (fa, fb) in faces {
            let fd = fb - fa;
            let fl = fd.len();
            if !(fl.is_finite() && fl > 1e-9) || d.cross(fd).abs() / (len * fl) > 1e-3 {
                continue;
            }
            if (fa - s0).dot(normal).abs() > 0.05 || (fb - s0).dot(normal).abs() > 0.05 {
                continue;
            }
            let (pa, pb) = ((fa - s0).dot(dir), (fb - s0).dot(dir));
            let overlap = pa.max(pb).min(len) - pa.min(pb).max(0.0);
            if overlap >= len / 2.0 {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;

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
