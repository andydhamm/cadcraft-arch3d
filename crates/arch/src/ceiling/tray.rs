use std::collections::HashSet;

use cadcraft_doc::arch::{ArchId, ArchModel, Level, MAX_TRAY_NESTING, Room, Tray, TraySides};
use cadcraft_geom::skeleton::inset as inset_region;
use cadcraft_geom::{Line, Vec2, point_in_polygon, shoelace};

const ON_EDGE: f64 = 1e-9;

fn ring_edges(ring: &[Vec2]) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
    ring.iter().zip(ring.iter().cycle().skip(1)).map(|(a, b)| (*a, *b))
}

fn ring_holds(ring: &[Vec2], p: Vec2) -> bool {
    if !p.is_finite() {
        return false;
    }
    point_in_polygon(ring, p) || ring_edges(ring).any(|(a, b)| Line::new(a, b).dist(p) <= ON_EDGE)
}

fn crossing(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<Vec2> {
    let r = b - a;
    let s = d - c;
    let denom = r.cross(s);
    if !denom.is_finite() || denom.abs() <= 1e-15 * (r.len() * s.len()).max(f64::MIN_POSITIVE) {
        return None;
    }
    let ac = c - a;
    let t = ac.cross(s) / denom;
    let u = ac.cross(r) / denom;
    const T: f64 = 1e-12;
    if !(t.is_finite() && u.is_finite() && (-T..=1.0 + T).contains(&t) && (-T..=1.0 + T).contains(&u)) {
        return None;
    }
    let x = Vec2::new(a.x + r.x * t, a.y + r.y * t);
    x.is_finite().then_some(x)
}

fn orient_ccw(ring: &[Vec2]) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = ring.iter().copied().collect();
    if out.len() >= 2 && out.first() == out.last() {
        out.pop();
    }
    if out.len() >= 3 && shoelace(&out) < 0.0 {
        out.reverse();
    }
    out
}

fn ring_is_convex(ring: &[Vec2]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut sign = 0i8;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        let c = ring[(i + 2) % ring.len()];
        let cross = (b - a).cross(c - b);
        if !cross.is_finite() {
            return false;
        }
        let s = if cross > 1e-9 {
            1
        } else if cross < -1e-9 {
            -1
        } else {
            0
        };
        if s == 0 {
            continue;
        }
        if sign == 0 {
            sign = s;
        } else if sign != s {
            return false;
        }
    }
    sign != 0
}

fn flat_ceiling_z(model: &ArchModel, room: &Room) -> Option<f64> {
    if !room.room_type.has_ceiling() {
        return None;
    }
    let default_level = Level::default();
    let level = model.level(room.level).unwrap_or(&default_level);
    let h = room.ceiling_height.filter(|v| v.is_finite() && *v > 0.0).unwrap_or(level.ceiling_height);
    if !h.is_finite() || h <= 0.02 {
        return None;
    }
    let z = level.elevation + h;
    z.is_finite().then_some(z)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct UndersideMin {
    pub at: Vec2,
    pub z: f64,
}

fn lowest_underside_over(model: &ArchModel, level: u32, polygon: &[Vec2]) -> (Option<UndersideMin>, bool) {
    let rings: Vec<&[Vec2]> = model
        .roofs
        .iter()
        .filter(|r| r.level == level)
        .flat_map(|r| r.planes.iter())
        .filter(|pl| pl.ring.len() >= 3)
        .map(|pl| pl.ring.as_slice())
        .collect();
    let mut samples: Vec<Vec2> = polygon.iter().copied().collect();
    if polygon.len() >= 2 {
        let mut pairs = 0usize;
        'edges: for (a, b) in ring_edges(polygon) {
            for ring in &rings {
                for (c, d) in ring_edges(ring) {
                    pairs += 1;
                    if samples.len() >= 100_000 || pairs > 4_000_000 {
                        break 'edges;
                    }
                    if let Some(x) = crossing(a, b, c, d) {
                        samples.push(x);
                    }
                }
            }
        }
    }
    if polygon.len() >= 3 {
        'rings: for ring in &rings {
            for &v in ring.iter() {
                if samples.len() >= 100_000 {
                    break 'rings;
                }
                if v.is_finite() && ring_holds(polygon, v) {
                    samples.push(v);
                }
            }
        }
    }

    let mut best: Option<UndersideMin> = None;
    let mut uncovered = false;
    for q in samples {
        match crate::ceiling::roof_underside_at(model, level, q) {
            Some(z) => {
                if best.is_none_or(|b| z < b.z) {
                    best = Some(UndersideMin { at: q, z });
                }
            }
            None => uncovered = true,
        }
    }
    (best, uncovered)
}

fn base_ring_in_inner(room: &Room, trays: &[Tray], t: &Tray, depth: usize, seen: &mut HashSet<ArchId>) -> Option<Vec<Vec2>> {
    if seen.contains(&t.id) {
        return None;
    }
    seen.insert(t.id);

    if !t.polygon.is_empty() {
        return Some(orient_ccw(&t.polygon));
    }
    if let Some(parent_id) = t.parent {
        if depth >= MAX_TRAY_NESTING + 1 {
            return None;
        }
        let parent = trays.iter().find(|p| p.id == parent_id)?;
        return outline_in_inner(room, trays, parent, depth + 1, seen).ok();
    }
    Some(orient_ccw(&room.polygon))
}

fn outline_in_inner(room: &Room, trays: &[Tray], t: &Tray, depth: usize, seen: &mut HashSet<ArchId>) -> Result<Vec<Vec2>, String> {
    let ring = base_ring_in_inner(room, trays, t, depth, seen).ok_or_else(|| "the inset leaves no tray outline".to_string())?;
    let d = t.inset;
    if !d.is_finite() {
        return Err("the inset leaves no tray outline".to_string());
    }
    if d <= 0.0 {
        return Ok(ring);
    }
    let Ok(regions) = inset_region(&ring, &[], d) else {
        return Err("the inset leaves no tray outline".to_string());
    };
    if regions.is_empty() {
        return Err("the inset leaves no tray outline".to_string());
    }
    if regions.len() != 1 || regions.iter().any(|r| !r.holes.is_empty()) {
        return Err("the inset splits the tray outline".to_string());
    }
    Ok(orient_ccw(&regions[0].outer))
}

fn rise_for_tray(room: &Room, trays: &[Tray], t: &Tray, p: Vec2) -> f64 {
    if !p.is_finite() || !t.depth.is_finite() {
        return 0.0;
    }
    match t.sides {
        TraySides::Vertical => t.depth,
        TraySides::Pitched(deg) => {
            if !deg.is_finite() || deg <= 0.0 || deg >= 90.0 {
                return 0.0;
            }
            let outline = match outline_in_inner(room, trays, t, 0, &mut HashSet::new()) {
                Ok(r) => r,
                Err(_) => return 0.0,
            };
            let d = ring_edges(&outline)
                .map(|(a, b)| {
                    let ab = b - a;
                    let len = ab.len();
                    if !len.is_finite() || len <= 1e-12 {
                        return f64::INFINITY;
                    }
                    let cross = (p - a).cross(ab);
                    (cross.abs() / len).abs()
                })
                .fold(f64::INFINITY, f64::min);
            let rise = deg.to_radians().tan();
            if !rise.is_finite() {
                return 0.0;
            }
            t.depth.min(rise * d)
        }
    }
}

fn tray_overlap(a: &[Vec2], b: &[Vec2]) -> bool {
    for &p in a {
        if ring_holds(b, p) {
            return true;
        }
    }
    for &p in b {
        if ring_holds(a, p) {
            return true;
        }
    }
    for (a0, a1) in ring_edges(a) {
        for (b0, b1) in ring_edges(b) {
            if crossing(a0, a1, b0, b1).is_some() {
                return true;
            }
        }
    }
    false
}

/// The base ring of a tray: its explicit polygon (normalized), its parent tray's outline, or the
/// room's polygon, with CCW orientation.
pub fn base_ring_in(room: &Room, trays: &[Tray], t: &Tray) -> Option<Vec<Vec2>> {
    let mut seen = HashSet::new();
    base_ring_in_inner(room, trays, t, 0, &mut seen)
}

/// The outline of a tray's base ring inset by its `inset` distance.
pub fn outline_in(room: &Room, trays: &[Tray], t: &Tray) -> Result<Vec<Vec2>, String> {
    let mut seen = HashSet::new();
    outline_in_inner(room, trays, t, 0, &mut seen)
}

/// The top ring of a tray: same outline for vertical sides, or the outline inset by `depth / tan(deg)`.
pub fn top_ring_in(room: &Room, trays: &[Tray], t: &Tray) -> Result<Vec<Vec2>, String> {
    let outline = outline_in(room, trays, t)?;
    match t.sides {
        TraySides::Vertical => Ok(outline),
        TraySides::Pitched(deg) => {
            if !deg.is_finite() || !(0.0 < deg && deg < 90.0) {
                return Err("the pitched sides cannot be built".to_string());
            }
            let offset = t.depth / deg.to_radians().tan();
            if !offset.is_finite() {
                return Err("the pitched sides cannot be built".to_string());
            }
            let Ok(regions) = inset_region(&outline, &[], offset) else {
                return Err("the pitched sides cannot be built".to_string());
            };
            if regions.len() != 1 || regions.iter().any(|r| !r.holes.is_empty()) || regions[0].outer.len() != outline.len() {
                return Err("the pitched sides cannot be built".to_string());
            }
            let top = orient_ccw(&regions[0].outer);
            if !ring_is_convex(&top) {
                return Err("pitched sides need a convex outline".to_string());
            }
            Ok(top)
        }
    }
}

/// The ring used by `base_ring_in` for a tray in `model`.
pub fn base_ring(model: &ArchModel, t: &Tray) -> Option<Vec<Vec2>> {
    let room = model.room(t.room)?;
    base_ring_in(room, &model.trays, t)
}

/// The tray outline in `model`.
pub fn outline(model: &ArchModel, t: &Tray) -> Result<Vec<Vec2>, String> {
    let room = model.room(t.room).ok_or_else(|| "the tray has no room".to_string())?;
    outline_in(room, &model.trays, t)
}

/// The tray top ring in `model`.
pub fn top_ring(model: &ArchModel, t: &Tray) -> Result<Vec<Vec2>, String> {
    let room = model.room(t.room).ok_or_else(|| "the tray has no room".to_string())?;
    top_ring_in(room, &model.trays, t)
}

/// How far the trays of `room` lift its rough ceiling at plan point `p`, in metres.
pub fn lift_in(room: &Room, trays: &[Tray], p: Vec2) -> f64 {
    if !p.is_finite() {
        return 0.0;
    }
    let mut total = 0.0;
    for t in trays.iter().filter(|t| t.room == room.id && t.level == room.level) {
        let Ok(outline) = outline_in(room, trays, t) else {
            continue;
        };
        if !ring_holds(&outline, p) {
            continue;
        }
        let rise = match t.sides {
            TraySides::Vertical => t.depth,
            TraySides::Pitched(deg) => {
                if !deg.is_finite() || !t.depth.is_finite() || deg <= 0.0 || deg >= 90.0 {
                    0.0
                } else {
                    let d = ring_edges(&outline)
                        .map(|(a, b)| {
                            let ab = b - a;
                            let len = ab.len();
                            if !len.is_finite() || len <= 1e-12 {
                                return f64::INFINITY;
                            }
                            let cross = (p - a).cross(ab);
                            (cross.abs() / len).abs()
                        })
                        .fold(f64::INFINITY, f64::min);
                    let rise = deg.to_radians().tan();
                    if !rise.is_finite() { 0.0 } else { t.depth.min(rise * d) }
                }
            }
        };
        if rise.is_finite() {
            total += rise;
        }
    }
    total
}

/// How far the trays of `room` lift its rough ceiling at plan point `p`, in metres.
pub fn lift(model: &ArchModel, room: &Room, p: Vec2) -> f64 {
    lift_in(room, &model.trays, p)
}

/// Validate a tray against the room and roof conditions for a usable tray.
pub fn validate(model: &ArchModel, t: &Tray) -> Result<(), String> {
    let room = model.room(t.room).ok_or_else(|| "the tray has no room".to_string())?;
    if room.level != t.level || flat_ceiling_z(model, room).is_none() || !room.flat_ceiling {
        return Err("a tray needs a flat ceiling".to_string());
    }

    if !(t.depth.is_finite() && (0.01..=2.0).contains(&t.depth)) || !(t.inset.is_finite() && (0.0..=50.0).contains(&t.inset)) {
        return Err("the tray's depth or inset is not a usable number".to_string());
    }
    match t.sides {
        TraySides::Pitched(deg) if !(deg.is_finite() && (5.0..=85.0).contains(&deg)) => {
            return Err("a pitched tray needs sides between 5 and 85 degrees".to_string());
        }
        _ => {}
    }

    outline_in(room, &model.trays, t)?;
    top_ring_in(room, &model.trays, t)?;

    if !t.polygon.is_empty() {
        let ring = if t.polygon.len() >= 2 && t.polygon.first() == t.polygon.last() { &t.polygon[..t.polygon.len() - 1] } else { &t.polygon };
        for i in 0..ring.len() {
            for j in i + 1..ring.len() {
                let a0 = ring[i];
                let a1 = ring[(i + 1) % ring.len()];
                let b0 = ring[j];
                let b1 = ring[(j + 1) % ring.len()];
                let adjacent = (i + 1) % ring.len() == j || (j + 1) % ring.len() == i;
                if !adjacent && crossing(a0, a1, b0, b1).is_some() {
                    return Err("the drawn tray outline is not inside the room".to_string());
                }
            }
        }
        for &v in ring {
            if !ring_holds(&room.polygon, v) || room.holes.iter().any(|h| ring_holds(h, v)) {
                return Err("the drawn tray outline is not inside the room".to_string());
            }
        }
    }

    if let Some(parent_id) = t.parent {
        let parent = model.trays.iter().find(|p| p.id == parent_id).ok_or_else(|| "the tray is nested too deep".to_string())?;
        if parent.room != t.room || parent.level != t.level {
            return Err("the tray is nested too deep".to_string());
        }
        let mut current = Some(parent_id);
        let mut seen = HashSet::new();
        let mut depth = 0usize;
        while let Some(id) = current {
            if depth > MAX_TRAY_NESTING || !seen.insert(id) {
                return Err("the tray is nested too deep".to_string());
            }
            let tray = model.trays.iter().find(|p| p.id == id).ok_or_else(|| "the tray is nested too deep".to_string())?;
            if tray.room != t.room || tray.level != t.level {
                return Err("the tray is nested too deep".to_string());
            }
            current = tray.parent;
            depth += 1;
        }
        let parent_outline = outline_in(room, &model.trays, parent)?;
        let outline = outline_in(room, &model.trays, t)?;
        for &v in &outline {
            if !ring_holds(&parent_outline, v) {
                return Err("the tray is not inside its parent tray".to_string());
            }
        }
    }

    for sibling in &model.trays {
        if sibling.id == t.id || sibling.room != t.room || sibling.level != t.level || sibling.parent != t.parent {
            continue;
        }
        let a = outline_in(room, &model.trays, t)?;
        let b = outline_in(room, &model.trays, sibling)?;
        if tray_overlap(&a, &b) {
            return Err("the tray overlaps another tray".to_string());
        }
    }

    let default_level = Level::default();
    let room_level = model.level(room.level).unwrap_or(&default_level);
    for other in &model.rooms {
        if other.id == room.id || other.level <= t.level {
            continue;
        }
        let above = model.level(other.level).unwrap_or(&default_level);
        if above.elevation <= room_level.elevation {
            continue;
        }
        let outline = outline_in(room, &model.trays, t)?;
        if other.polygon.iter().any(|&p| ring_holds(&outline, p)) || outline.iter().any(|&p| ring_holds(&other.polygon, p)) {
            return Err(format!("the tray overlaps the room on the level above {}", above.name));
        }
    }

    let outline = outline_in(room, &model.trays, t)?;
    let p = outline.iter().fold(Vec2::ZERO, |acc, &v| acc + v) / outline.len() as f64;
    let mut ancestors_lift = 0.0;
    let mut current = t.parent;
    let mut seen = HashSet::new();
    while let Some(id) = current {
        if !seen.insert(id) {
            break;
        }
        let Some(ancestor) = model.trays.iter().find(|tray| tray.id == id) else {
            break;
        };
        ancestors_lift += rise_for_tray(room, &model.trays, ancestor, p);
        current = ancestor.parent;
    }
    let flat_z = match flat_ceiling_z(model, room) {
        Some(z) => z,
        None => return Err("a tray needs a flat ceiling".to_string()),
    };
    let top = flat_z + ancestors_lift + t.depth;
    let top_ring = top_ring_in(room, &model.trays, t)?;
    let (min, uncovered) = lowest_underside_over(model, t.level, &top_ring);
    if uncovered {
        return Err("no roof above part of the tray".to_string());
    }
    let Some(min) = min else {
        return Err("no roof above part of the tray".to_string());
    };
    if min.z < top - 1e-9 {
        return Err(format!("the tray top at {top:.3} m would rise above the roof underside ({:.3} m at {:.3}, {:.3})", min.z, min.at.x, min.at.y));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchModel, Roof, RoofDirective, Room, Tray, TraySides, Wall};
    use cadcraft_geom::Vec2;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    fn house(rise: Option<f64>) -> ArchModel {
        let mut m = ArchModel::default();
        let wall = |a: (f64, f64), b: (f64, f64), thickness: f64, exterior: bool| Wall {
            start: Vec2::new(a.0, a.1),
            end: Vec2::new(b.0, b.1),
            thickness,
            height: 2.4,
            exterior,
            ..Default::default()
        };
        for (a, b) in [((0.0, 0.0), (10.0, 0.0)), ((10.0, 0.0), (10.0, 8.0)), ((10.0, 8.0), (0.0, 8.0)), ((0.0, 8.0), (0.0, 0.0))] {
            m.push_wall(wall(a, b, 0.2, true));
        }
        m.push_wall(wall((0.0, 3.0), (10.0, 3.0), 0.1, false));
        m.push_wall(wall((6.0, 3.0), (6.0, 8.0), 0.1, false));
        m.push_room(Room { name: "South".into(), polygon: rect(0.1, 0.1, 9.9, 2.95), ..Default::default() });
        m.push_room(Room { name: "North-west".into(), polygon: rect(0.1, 3.05, 5.95, 7.9), ..Default::default() });
        m.push_room(Room { name: "North-east".into(), polygon: rect(6.05, 3.05, 9.9, 7.9), ..Default::default() });
        m.walls[1].roof_directive = RoofDirective::Gable;
        m.walls[3].roof_directive = RoofDirective::Gable;
        if let Some(rise) = rise {
            let seed = Roof { pitch_rise: rise, pitch_run: 12.0, overhang: 0.5, thickness: 0.2, ..Roof::default() };
            let built = crate::roof::build_roof(&m, 0, &seed).expect("the gable roof builds");
            m.push_roof(built);
        }
        m
    }

    #[test]
    fn outline_insets_the_room() {
        let m = house(Some(6.0));
        let room = &m.rooms[1];
        let tray = Tray { room: room.id, level: room.level, inset: 0.6, ..Default::default() };
        let got = outline_in(room, &[], &tray).unwrap();
        let want = vec![Vec2::new(0.7, 3.65), Vec2::new(5.35, 3.65), Vec2::new(5.35, 7.3), Vec2::new(0.7, 7.3)];
        assert_eq!(got.len(), want.len());
        for (a, b) in got.iter().zip(want.iter()) {
            assert!((a.x - b.x).abs() <= 1e-6, "{a:?} vs {b:?}");
            assert!((a.y - b.y).abs() <= 1e-6, "{a:?} vs {b:?}");
        }
        let same = outline_in(room, &[], &Tray { room: room.id, level: room.level, inset: 0.0, ..Default::default() }).unwrap();
        assert_eq!(same, room.polygon);
    }

    #[test]
    fn nested_lift_adds_its_parent() {
        let mut m = house(Some(6.0));
        let room_id = m.rooms[1].id;
        let room_level = m.rooms[1].level;
        let parent = m.push_tray(Tray { room: room_id, level: room_level, inset: 0.6, depth: 0.3, ..Default::default() });
        let child = m.push_tray(Tray { room: room_id, level: room_level, parent: Some(parent), inset: 0.6, depth: 0.15, ..Default::default() });
        let room = &m.rooms[1];
        let child_t = m.tray(child).unwrap();
        assert_eq!(base_ring_in(room, &m.trays, child_t).unwrap().len(), 4);
        assert!((lift_in(room, &m.trays, Vec2::new(3.025, 5.475)) - 0.45).abs() < 1e-6);
        assert!((lift_in(room, &m.trays, Vec2::new(1.0, 5.475)) - 0.3).abs() < 1e-6);
        assert!((lift_in(room, &m.trays, Vec2::new(0.4, 5.0)) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn pitched_lift_ramps_from_the_outline() {
        let mut m = house(Some(6.0));
        let room_id = m.rooms[1].id;
        let room_level = m.rooms[1].level;
        let tray =
            m.push_tray(Tray { room: room_id, level: room_level, inset: 0.6, depth: 0.3, sides: TraySides::Pitched(45.0), ..Default::default() });
        let room = &m.rooms[1];
        let t = m.tray(tray).unwrap();
        assert!((lift_in(room, &m.trays, Vec2::new(0.8, 5.475)) - 0.1).abs() < 1e-6);
        assert!((lift_in(room, &m.trays, Vec2::new(3.025, 5.475)) - 0.3).abs() < 1e-6, "{:?}", t);
    }

    #[test]
    fn validate_refuses_above_the_roof() {
        let mut m = house(Some(6.0));
        let room_id = m.rooms[1].id;
        let room_level = m.rooms[1].level;
        let tray_id = m.push_tray(Tray { room: room_id, level: room_level, inset: 0.6, depth: 0.5, ..Default::default() });
        let err = validate(&m, m.tray(tray_id).unwrap()).unwrap_err();
        assert!(err.contains("roof"));

        let mut okay = house(Some(6.0));
        let okay_id = okay.push_tray(Tray { room: okay.rooms[1].id, level: okay.rooms[1].level, inset: 0.6, depth: 0.3, ..Default::default() });
        assert!(validate(&okay, okay.tray(okay_id).unwrap()).is_ok());

        let mut flat = house(Some(6.0));
        flat.rooms[1].flat_ceiling = false;
        let not_flat_id = flat.push_tray(Tray { room: flat.rooms[1].id, level: flat.rooms[1].level, inset: 0.6, depth: 0.3, ..Default::default() });
        let msg = validate(&flat, flat.tray(not_flat_id).unwrap()).unwrap_err();
        assert!(msg.contains("flat ceiling"));

        let mut above = house(Some(6.0));
        above.levels.push(cadcraft_doc::arch::Level { index: 1, name: "Upper".into(), elevation: 3.0, ceiling_height: 2.4, ..Default::default() });
        above.push_room(Room { name: "Upper room".into(), level: 1, polygon: above.rooms[1].polygon.clone(), ..Default::default() });
        let over_id = above.push_tray(Tray { room: above.rooms[1].id, level: above.rooms[1].level, inset: 0.6, depth: 0.3, ..Default::default() });
        let msg = validate(&above, above.tray(over_id).unwrap()).unwrap_err();
        assert!(msg.contains("level above"));
        assert!(msg.contains("Upper"));
    }

    #[test]
    fn validate_refuses_a_new_tray_above_the_roof() {
        let m = house(Some(6.0));
        let room_id = m.rooms[1].id;
        let room_level = m.rooms[1].level;
        let above = Tray { room: room_id, level: room_level, inset: 0.6, depth: 0.5, ..Default::default() };
        assert!(validate(&m, &above).is_err(), "a new tray above the roof underside should be refused");
        let below = Tray { room: room_id, level: room_level, inset: 0.6, depth: 0.3, ..Default::default() };
        assert!(validate(&m, &below).is_ok(), "a new tray under the roof underside should be accepted");
    }

    #[test]
    fn validate_counts_a_new_nested_trays_parent() {
        let mut m = house(Some(6.0));
        let room_id = m.rooms[1].id;
        let room_level = m.rooms[1].level;
        let parent_id = m.push_tray(Tray { room: room_id, level: room_level, inset: 0.6, depth: 0.3, ..Default::default() });
        let ok = Tray { room: room_id, level: room_level, parent: Some(parent_id), inset: 0.6, depth: 0.15, ..Default::default() };
        assert!(validate(&m, &ok).is_ok(), "a new nested tray with parent lift under the roof should be accepted");
        let over = Tray { room: room_id, level: room_level, parent: Some(parent_id), inset: 0.6, depth: 0.75, ..Default::default() };
        assert!(validate(&m, &over).is_err(), "a new nested tray with parent lift above the roof should be refused");
    }

    #[test]
    fn cyclic_parents_in_a_loaded_file_are_bounded() {
        let mut m = ArchModel::default();
        let room = Room { name: "Test".into(), polygon: rect(0.0, 0.0, 1.0, 1.0), ..Default::default() };
        m.push_room(room);
        let a = m.push_tray(Tray { room: ArchId(0), level: 0, inset: 0.1, ..Default::default() });
        let b = m.push_tray(Tray { room: ArchId(0), level: 0, inset: 0.1, ..Default::default() });
        m.trays[0].parent = Some(b);
        m.trays[1].parent = Some(a);
        assert!(base_ring_in(&m.rooms[0], &m.trays, &m.trays[0]).is_none());
        assert!(outline_in(&m.rooms[0], &m.trays, &m.trays[0]).is_err());
        assert_eq!(lift_in(&m.rooms[0], &m.trays, Vec2::new(0.5, 0.5)), 0.0);
    }
}
