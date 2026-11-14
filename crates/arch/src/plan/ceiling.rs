//! Reflected-ceiling annotation for the plan pipeline.

use std::collections::HashSet;

use cadcraft_doc::arch::{
    ArchModel, Furnishing, Placement, RoofEdgeKind, Room, RoomType, Soffit, Tray, TraySides, furnishing_handle, room_handle, soffit_handle,
    tray_handle,
};
use cadcraft_geom::{Line, Vec2, point_in_polygon};

use crate::ceiling::tray::{outline, top_ring};
use crate::ceiling::{flat_ceiling_z, roof_underside_at, stack};
use crate::furnishing::plan_outline;
use crate::plan::{PlanAnnotation, PlanLabel, PlanLabelKind, PlanLine, PlanLineKind, ring_label_point};

fn ring_edges(ring: &[Vec2]) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
    ring.iter().zip(ring.iter().cycle().skip(1)).map(|(a, b)| (*a, *b))
}

fn vertex_average(ring: &[Vec2]) -> Option<Vec2> {
    if ring.is_empty() || ring.iter().any(|p| !p.is_finite()) {
        return None;
    }
    let n = ring.len() as f64;
    Some(Vec2::new(ring.iter().map(|p| p.x).sum::<f64>() / n, ring.iter().map(|p| p.y).sum::<f64>() / n))
}

fn ring_holds(ring: &[Vec2], p: Vec2) -> bool {
    if !p.is_finite() {
        return false;
    }
    point_in_polygon(ring, p) || ring_edges(ring).any(|(a, b)| Line::new(a, b).dist(p) <= 1e-9)
}

fn segment_crossing(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<f64> {
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
    Some(t)
}

fn clipped_segment(room: &[Vec2], holes: &[Vec<Vec2>], a: Vec2, b: Vec2) -> Vec<(Vec2, Vec2)> {
    let mut ts = vec![0.0, 1.0];
    for (c, d) in room.iter().zip(room.iter().cycle().skip(1)) {
        let c = *c;
        let d = *d;
        if let Some(t) = segment_crossing(a, b, c, d) {
            if (0.0..=1.0).contains(&t) {
                ts.push(t);
            }
        }
    }
    for hole in holes {
        for (c, d) in hole.iter().zip(hole.iter().cycle().skip(1)) {
            let c = *c;
            let d = *d;
            if let Some(t) = segment_crossing(a, b, c, d) {
                if (0.0..=1.0).contains(&t) {
                    ts.push(t);
                }
            }
        }
    }
    ts.sort_by(f64::total_cmp);
    ts.dedup_by(|x, y| (*x - *y).abs() <= 1e-9);
    let mut out = Vec::new();
    for window in ts.windows(2) {
        let t0 = window[0];
        let t1 = window[1];
        let span = t1 - t0;
        if span <= 1e-9 || !span.is_finite() {
            continue;
        }
        let m = (t0 + t1) * 0.5;
        let mid = a + (b - a) * m;
        if !point_in_polygon(room, mid) {
            continue;
        }
        if holes.iter().any(|hole| hole.len() >= 3 && point_in_polygon(hole, mid)) {
            continue;
        }
        let start = a + (b - a) * t0;
        let end = a + (b - a) * t1;
        let len = (end - start).len();
        if len >= 1e-9 {
            out.push((start, end));
        }
    }
    out
}

fn label_point_for_ring(ring: &[Vec2], holes: &[Vec<Vec2>]) -> Vec2 {
    ring_label_point(ring, holes).or_else(|| vertex_average(ring)).unwrap_or(Vec2::ZERO)
}

fn tray_layer(t: &Tray) -> &str {
    if t.layer.trim().is_empty() { "A-CLNG" } else { &t.layer }
}

fn soffit_layer(s: &Soffit) -> &str {
    if s.layer.trim().is_empty() { "A-CLNG" } else { &s.layer }
}

fn tray_label_point(model: &ArchModel, tray: &Tray) -> Vec2 {
    let parent_holes: Vec<Vec<Vec2>> = model
        .trays
        .iter()
        .filter(|child| child.parent == Some(tray.id) && child.room == tray.room && child.level == tray.level)
        .filter_map(|child| outline(model, child).ok())
        .collect();
    let ring = match top_ring(model, tray) {
        Ok(r) if r.len() >= 3 && r.iter().all(|p| p.is_finite()) => r,
        _ => match outline(model, tray) {
            Ok(r) if r.len() >= 3 && r.iter().all(|p| p.is_finite()) => r,
            _ => return Vec2::ZERO,
        },
    };
    label_point_for_ring(&ring, &parent_holes)
}

fn tray_height_text(model: &ArchModel, room: &Room, tray: &Tray, base: f64) -> Option<String> {
    let mut total = base + tray.depth;
    let mut seen = HashSet::new();
    let mut current = tray.parent;
    while let Some(id) = current {
        if !seen.insert(id) {
            break;
        }
        let some = model.trays.iter().find(|t| t.id == id)?;
        total += some.depth;
        current = some.parent;
    }
    total.is_finite().then(|| format!("{total:.2}"))
}

/// The reflected ceiling plan's annotation (tray outlines and heights, vault lines, ceiling
/// items, soffits).
pub fn ceiling_plan(model: &ArchModel, level: Option<u32>, out: &mut PlanAnnotation) {
    for room in &model.rooms {
        if !room.room_type.has_ceiling() || level.is_some_and(|l| room.level != l) {
            continue;
        }
        let Some(flat) = flat_ceiling_z(model, room) else {
            continue;
        };
        let elevation = model.level(room.level).map_or(0.0, |l| l.elevation);
        let base = flat - stack::thickness(&room.ceiling_stack) - elevation;
        if !base.is_finite() {
            continue;
        }

        for tray in &model.trays {
            if tray.room != room.id || tray.level != room.level {
                continue;
            }
            let Ok(ring) = outline(model, tray) else { continue };
            if ring.len() < 3 || !ring.iter().all(|p| p.is_finite()) {
                continue;
            }
            for (a, b) in ring_edges(&ring) {
                if !out.push_line(PlanLine::new(tray_layer(tray), PlanLineKind::TrayOutline, a, b, tray_handle(tray.id))) {
                    return;
                }
            }
            if matches!(tray.sides, TraySides::Pitched(..)) {
                if let Ok(top) = top_ring(model, tray) {
                    if top.len() >= 3 && top.iter().all(|p| p.is_finite()) {
                        for (a, b) in ring_edges(&top) {
                            if !out.push_line(PlanLine::new(tray_layer(tray), PlanLineKind::TrayTop, a, b, tray_handle(tray.id))) {
                                return;
                            }
                        }
                    }
                }
            }
            let Some(text) = tray_height_text(model, room, tray, base) else { continue };
            let at = tray_label_point(model, tray);
            if at == Vec2::ZERO && ring.iter().all(|v| *v != Vec2::ZERO) {
                continue;
            }
            if !out.push_label(PlanLabel::new("A-CLNG-ANNO", PlanLabelKind::TrayHeight, text, at, 0.18, tray_handle(tray.id))) {
                return;
            }
        }

        if !room.flat_ceiling {
            let anchor = ring_label_point(&room.polygon, &room.holes).unwrap_or_else(|| {
                let n = room.polygon.len() as f64;
                Vec2::new(room.polygon.iter().map(|p| p.x).sum::<f64>() / n, room.polygon.iter().map(|p| p.y).sum::<f64>() / n)
            });
            let some = roof_underside_at(model, room.level, anchor).is_some();
            if some {
                let holes = room.holes.clone();
                for roof in &model.roofs {
                    if roof.level != room.level {
                        continue;
                    }
                    for edge in &roof.edges {
                        if !matches!(edge.kind, RoofEdgeKind::Ridge | RoofEdgeKind::Hip | RoofEdgeKind::Valley) {
                            continue;
                        }
                        for (a, b) in clipped_segment(&room.polygon, &holes, edge.a, edge.b) {
                            if !out.push_line(PlanLine::new("A-CLNG", PlanLineKind::VaultLine, a, b, room_handle(room.id))) {
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    // Soffits and ceiling items belong to the level, not to a room: drawn once per call.
    for soffit in &model.soffits {
        if level.is_some_and(|l| soffit.level != l) {
            continue;
        }
        let Some(ring) = crate::ceiling::soffit::soffit_ring(model, soffit) else { continue };
        if ring.len() < 3 || !ring.iter().all(|p| p.is_finite()) {
            continue;
        }
        for (a, b) in ring_edges(&ring) {
            if !out.push_line(PlanLine::new(soffit_layer(soffit), PlanLineKind::Soffit, a, b, soffit_handle(soffit.id))) {
                return;
            }
        }
    }

    for furnishing in &model.furnishings {
        if furnishing.placement != Placement::Ceiling || level.is_some_and(|l| furnishing.level != l) {
            continue;
        }
        let ring = plan_outline(model, furnishing);
        if ring.len() < 3 || !ring.iter().all(|p| p.is_finite()) {
            continue;
        }
        for (a, b) in ring_edges(&ring) {
            if !out.push_line(PlanLine::new("A-CLNG", PlanLineKind::CeilingItem, a, b, furnishing_handle(furnishing.id))) {
                return;
            }
        }
    }
}

/// A vaulted room's pitch text.
pub fn vault_text(model: &ArchModel, room: &Room) -> Option<String> {
    if room.flat_ceiling || !room.room_type.has_ceiling() {
        return None;
    }
    let anchor = ring_label_point(&room.polygon, &room.holes).unwrap_or_else(|| {
        let n = room.polygon.len() as f64;
        Vec2::new(room.polygon.iter().map(|p| p.x).sum::<f64>() / n, room.polygon.iter().map(|p| p.y).sum::<f64>() / n)
    });
    if !anchor.is_finite() || roof_underside_at(model, room.level, anchor).is_none() {
        let any = room.polygon.iter().all(|p| !p.is_finite())
            || !room.polygon.iter().any(|p| {
                model
                    .roofs
                    .iter()
                    .any(|roof| roof.level == room.level && roof.planes.iter().any(|plane| plane.ring.len() >= 3 && ring_holds(&plane.ring, *p)))
            });
        if any {
            return None;
        }
    }
    let mut pitches: Vec<f64> = Vec::new();
    for roof in &model.roofs {
        if roof.level != room.level {
            continue;
        }
        for plane in &roof.planes {
            if plane.ring.len() < 3 || !plane.ring.iter().all(|p| p.is_finite()) {
                continue;
            }
            let holds = ring_holds(&plane.ring, anchor) || room.polygon.iter().any(|p| ring_holds(&plane.ring, *p));
            if !holds {
                continue;
            }
            let slope = plane.slope;
            let Some(_text) = crate::plan::pitch_text(slope) else {
                continue;
            };
            if !pitches.iter().any(|v| (v - slope).abs() <= 1e-9) {
                pitches.push(slope);
            }
        }
    }
    if pitches.is_empty() {
        return None;
    }
    pitches.sort_by(f64::total_cmp);
    pitches.reverse();
    let mut out = Vec::new();
    for slope in pitches {
        if let Some(text) = crate::plan::pitch_text(slope) {
            if !out.iter().any(|t: &String| t == &text) {
                out.push(text);
            }
        }
    }
    let text = out.join(", ");
    (!text.is_empty()).then_some(text)
}

/// The outlines of the room's top-level trays, which the room label block keeps out of when it
/// shows the ceiling height.
pub fn ceiling_label_holes(model: &ArchModel, room: &Room) -> Vec<Vec<Vec2>> {
    model
        .trays
        .iter()
        .filter(|t| t.room == room.id && t.level == room.level && t.parent.is_none())
        .filter_map(|t| outline(model, t).ok())
        .filter(|ring| ring.len() >= 3 && ring.iter().all(|p| p.is_finite()))
        .collect()
}

/// The room's finished flat ceiling height above its level's floor as `"2.40"`: the vault text
/// when the room is vaulted, `None` when the room type has no ceiling or the height is not
/// finite. Trays are excluded (each tray top is labelled separately).
pub fn room_ceiling_text(model: &ArchModel, room: &Room) -> Option<String> {
    if !room.room_type.has_ceiling() {
        return None;
    }
    if let Some(t) = vault_text(model, room) {
        return Some(t);
    }
    let elevation = model.level(room.level).map_or(0.0, |l| l.elevation);
    let z = flat_ceiling_z(model, room)? - stack::thickness(&room.ceiling_stack) - elevation;
    z.is_finite().then(|| format!("{z:.2}"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::arch::{CeilingLayer, Level, Roof, RoofPlane, RoomType, Tray, TraySides};
    use cadcraft_geom::Vec2;

    fn room_model() -> (ArchModel, Room) {
        let mut m = ArchModel::default();
        m.levels = vec![Level { elevation: 0.0, ceiling_height: 2.4, ..Level::default() }];
        let room = Room { room_type: RoomType::Living, ..Room::default() };
        (m, room)
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    fn level_model(rooms: usize) -> ArchModel {
        let mut m = ArchModel::default();
        m.levels = vec![Level { elevation: 0.0, ceiling_height: 2.4, ..Level::default() }];
        for i in 0..rooms {
            let x = 4.0 * i as f64;
            m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(x, 0.0, x + 3.5, 3.5), ..Room::default() });
        }
        m
    }

    fn lines_of(m: &ArchModel, level: Option<u32>, kind: PlanLineKind) -> Vec<PlanLine> {
        let mut out = PlanAnnotation::default();
        ceiling_plan(m, level, &mut out);
        out.lines.into_iter().filter(|l| l.kind == kind).collect()
    }

    fn assert_closed_ring(lines: &[PlanLine]) {
        for (i, l) in lines.iter().enumerate() {
            for end in [l.a, l.b] {
                let meets = lines.iter().enumerate().filter(|(j, o)| *j != i && (o.a == end || o.b == end)).count();
                assert_eq!(meets, 1, "end {end:?} of line {i}");
            }
        }
    }

    #[test]
    fn soffit_outline_is_drawn_once_with_three_rooms() {
        let mut m = level_model(3);
        let id = m.push_soffit(Soffit { level: 0, polygon: rect(7.0, 7.3, 9.0, 7.9), ..Soffit::default() });
        let lines = lines_of(&m, Some(0), PlanLineKind::Soffit);
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().all(|l| l.handle == soffit_handle(id)));
        let total: f64 = lines.iter().map(|l| (l.b - l.a).len()).sum();
        assert!((total - 5.2).abs() < 1e-9, "total {total}");
        assert_closed_ring(&lines);
    }

    #[test]
    fn ceiling_item_outline_is_drawn_once_with_three_rooms() {
        let item = Furnishing { level: 0, placement: Placement::Ceiling, at: [8.0, 7.6], size: [2.0, 0.6, 0.2], ..Furnishing::default() };
        let mut three = level_model(3);
        let id = three.push_furnishing(item.clone());
        let mut one = level_model(1);
        one.push_furnishing(item);
        let lines = lines_of(&three, Some(0), PlanLineKind::CeilingItem);
        assert_eq!(lines.len(), lines_of(&one, Some(0), PlanLineKind::CeilingItem).len());
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().all(|l| l.handle == furnishing_handle(id)));
        assert_closed_ring(&lines);
    }

    #[test]
    fn soffit_draws_without_rooms() {
        let mut m = level_model(0);
        m.push_soffit(Soffit { level: 0, polygon: rect(7.0, 7.3, 9.0, 7.9), ..Soffit::default() });
        assert_eq!(lines_of(&m, Some(0), PlanLineKind::Soffit).len(), 4);
    }

    #[test]
    fn soffit_on_another_level_is_not_drawn() {
        let mut m = level_model(3);
        m.push_soffit(Soffit { level: 0, polygon: rect(7.0, 7.3, 9.0, 7.9), ..Soffit::default() });
        assert!(lines_of(&m, Some(1), PlanLineKind::Soffit).is_empty());
    }

    #[test]
    fn room_ceiling_text_flat() {
        let (mut m, mut room) = room_model();
        assert_eq!(room_ceiling_text(&m, &room).as_deref(), Some("2.40"));
        room.ceiling_stack = vec![
            CeilingLayer::new("drywall", Some("drywall"), 0.0127),
            CeilingLayer::new("air", None, 0.2),
            CeilingLayer::new("ceiling tile", Some("tile"), 0.016),
        ];
        assert_eq!(room_ceiling_text(&m, &room).as_deref(), Some("2.17"));
        room.ceiling_stack.clear();
        m.levels[0].elevation = 2.7;
        assert_eq!(room_ceiling_text(&m, &room).as_deref(), Some("2.40"));
        let none = [RoomType::Garage, RoomType::Other, RoomType::Deck]
            .into_iter()
            .map(|t| Room { room_type: t, ..Room::default() })
            .find(|r| !r.room_type.has_ceiling());
        if let Some(r) = none {
            assert_eq!(room_ceiling_text(&m, &r), None);
        }
    }

    #[test]
    fn tray_outlines_and_heights() {
        let mut m = ArchModel::default();
        let room_id = m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 3.05, 5.95, 7.9), ..Room::default() });
        let t0 = Tray { id: cadcraft_doc::arch::ArchId(0), level: 0, room: room_id, inset: 0.6, depth: 0.3, polygon: Vec::new(), ..Tray::default() };
        let t1 = Tray {
            id: cadcraft_doc::arch::ArchId(1),
            level: 0,
            room: room_id,
            parent: Some(cadcraft_doc::arch::ArchId(0)),
            inset: 0.6,
            depth: 0.15,
            polygon: Vec::new(),
            ..Tray::default()
        };
        m.trays = vec![t0, t1];
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        let outline = out.lines.iter().filter(|l| l.kind == PlanLineKind::TrayOutline).count();
        assert_eq!(outline, 8, "tray outline count: {:?}", out.lines);
        assert!(out.labels.iter().any(|l| l.kind == PlanLabelKind::TrayHeight && l.text == "2.70"));
        assert!(out.labels.iter().any(|l| l.kind == PlanLabelKind::TrayHeight && l.text == "2.85"));
    }

    #[test]
    fn pitched_tray_draws_outline_and_top() {
        let mut m = ArchModel::default();
        let room_id = m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 3.05, 5.95, 7.9), ..Room::default() });
        let tray = Tray {
            id: cadcraft_doc::arch::ArchId(2),
            level: 0,
            room: room_id,
            inset: 0.6,
            depth: 0.3,
            sides: TraySides::Pitched(45.0),
            ..Tray::default()
        };
        m.trays = vec![tray];
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        assert!(out.lines.iter().any(|l| l.kind == PlanLineKind::TrayOutline));
        assert!(out.lines.iter().any(|l| l.kind == PlanLineKind::TrayTop));
    }

    #[test]
    fn tray_label_sits_outside_nested_trays() {
        let mut m = ArchModel::default();
        let room_id = m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 3.05, 5.95, 7.9), ..Room::default() });
        let outer = Tray { id: cadcraft_doc::arch::ArchId(3), level: 0, room: room_id, inset: 0.6, depth: 0.3, ..Tray::default() };
        let inner = Tray {
            id: cadcraft_doc::arch::ArchId(4),
            level: 0,
            room: room_id,
            parent: Some(cadcraft_doc::arch::ArchId(3)),
            inset: 0.6,
            depth: 0.15,
            ..Tray::default()
        };
        m.trays = vec![outer, inner];
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        let outer_handle = tray_handle(cadcraft_doc::arch::ArchId(3));
        let inner_handle = tray_handle(cadcraft_doc::arch::ArchId(4));
        let outer_label = out.labels.iter().find(|l| l.handle == outer_handle).unwrap();
        let inner_label = out.labels.iter().find(|l| l.handle == inner_handle).unwrap();
        assert_eq!(outer_label.text, "2.70");
        assert_eq!(inner_label.text, "2.85");
    }

    #[test]
    fn room_ceiling_label_keeps_out_of_the_trays() {
        let mut m = ArchModel::default();
        let room_id = m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 3.05, 5.95, 7.9), ..Room::default() });
        let outer = Tray { id: cadcraft_doc::arch::ArchId(5), level: 0, room: room_id, inset: 0.6, depth: 0.3, ..Tray::default() };
        let inner = Tray {
            id: cadcraft_doc::arch::ArchId(6),
            level: 0,
            room: room_id,
            parent: Some(cadcraft_doc::arch::ArchId(5)),
            inset: 0.6,
            depth: 0.15,
            ..Tray::default()
        };
        m.trays = vec![outer, inner];
        let room = m.room(room_id).unwrap().clone();
        let holes = ceiling_label_holes(&m, &room);
        assert_eq!(holes.len(), 1);
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        assert!(out.labels.iter().any(|l| l.kind == PlanLabelKind::TrayHeight));
    }

    #[test]
    fn invalid_tray_draws_nothing() {
        let mut m = ArchModel::default();
        let room_id = m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 3.05, 5.95, 7.9), ..Room::default() });
        let bad = Tray { id: cadcraft_doc::arch::ArchId(7), level: 0, room: room_id, inset: f64::NAN, depth: 0.3, ..Tray::default() };
        m.trays = vec![bad];
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        assert!(out.lines.iter().all(|l| l.kind != PlanLineKind::TrayOutline));
    }

    #[test]
    fn tray_label_adds_the_ancestors_depths_and_subtracts_the_stack() {
        let mut m = ArchModel::default();
        let room_id = m.push_room(Room {
            level: 0,
            room_type: RoomType::Living,
            polygon: rect(0.0, 0.0, 10.0, 8.0),
            ceiling_stack: vec![CeilingLayer::new("tile", Some("tile"), 0.1)],
            ..Room::default()
        });
        let parent = Tray { id: cadcraft_doc::arch::ArchId(8), level: 0, room: room_id, inset: 0.5, depth: 0.25, ..Tray::default() };
        let child = Tray {
            id: cadcraft_doc::arch::ArchId(9),
            level: 0,
            room: room_id,
            parent: Some(cadcraft_doc::arch::ArchId(8)),
            inset: 0.5,
            depth: 0.15,
            ..Tray::default()
        };
        m.trays = vec![parent, child];
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        let tray_values: Vec<f64> =
            out.labels.iter().filter(|l| l.kind == PlanLabelKind::TrayHeight).filter_map(|l| l.text.parse::<f64>().ok()).collect();
        assert!(!tray_values.is_empty());
        assert!(tray_values.iter().all(|v| *v > 2.0));
    }

    #[test]
    fn vault_line_is_the_ridge_clipped_to_the_room() {
        let mut m = ArchModel::default();
        let room_id =
            m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 0.1, 5.95, 7.9), flat_ceiling: false, ..Room::default() });
        let roof = Roof {
            level: 0,
            edges: vec![cadcraft_doc::arch::RoofEdge {
                kind: RoofEdgeKind::Ridge,
                a: Vec2::new(0.1, 4.0),
                b: Vec2::new(5.95, 4.0),
                ..Default::default()
            }],
            planes: vec![RoofPlane {
                ring: rect(0.0, 0.0, 10.0, 8.0),
                slope: 0.5,
                base_edge: [0.0, 0.0, 10.0, 0.0],
                eave_z: 2.4,
                ..RoofPlane::default()
            }],
            ..Roof::default()
        };
        m.push_roof(roof);
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        assert!(out.lines.iter().any(|l| l.kind == PlanLineKind::VaultLine));
    }

    #[test]
    fn hip_and_valley_vault_lines_are_clipped_too() {
        let mut m = ArchModel::default();
        let room_id =
            m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 0.1, 5.95, 7.9), flat_ceiling: false, ..Room::default() });
        let _room = m.room(room_id).unwrap().clone();
        let roof = Roof {
            level: 0,
            edges: vec![
                cadcraft_doc::arch::RoofEdge { kind: RoofEdgeKind::Hip, a: Vec2::new(0.1, 1.0), b: Vec2::new(5.95, 7.9), ..Default::default() },
                cadcraft_doc::arch::RoofEdge { kind: RoofEdgeKind::Valley, a: Vec2::new(5.95, 1.0), b: Vec2::new(0.1, 7.9), ..Default::default() },
            ],
            planes: vec![RoofPlane {
                ring: rect(0.0, 0.0, 10.0, 8.0),
                slope: 0.5,
                base_edge: [0.0, 0.0, 10.0, 0.0],
                eave_z: 2.4,
                ..RoofPlane::default()
            }],
            ..Roof::default()
        };
        m.push_roof(roof);
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        assert!(out.lines.iter().any(|l| l.kind == PlanLineKind::VaultLine));
    }

    #[test]
    fn flat_rooms_have_no_vault_lines() {
        let mut m = ArchModel::default();
        let _room_id = m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 0.1, 5.95, 7.9), ..Room::default() });
        let roof = Roof {
            level: 0,
            planes: vec![RoofPlane {
                ring: rect(0.0, 0.0, 10.0, 8.0),
                slope: 0.5,
                base_edge: [0.0, 0.0, 10.0, 0.0],
                eave_z: 2.4,
                ..RoofPlane::default()
            }],
            ..Roof::default()
        };
        m.push_roof(roof);
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        assert!(!out.lines.iter().any(|l| l.kind == PlanLineKind::VaultLine));
    }

    #[test]
    fn vault_text_is_the_pitch_of_the_planes_over_the_room() {
        let mut m = ArchModel::default();
        let room_id =
            m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 0.1, 5.95, 7.9), flat_ceiling: false, ..Room::default() });
        let room = m.room(room_id).unwrap().clone();
        m.push_roof(Roof {
            level: 0,
            planes: vec![RoofPlane {
                ring: rect(0.0, 0.0, 10.0, 8.0),
                slope: 0.5,
                base_edge: [0.0, 0.0, 10.0, 0.0],
                eave_z: 2.4,
                ..RoofPlane::default()
            }],
            ..Roof::default()
        });
        assert_eq!(vault_text(&m, &room), Some("6/12".to_string()));
    }

    #[test]
    fn vault_text_none_for_a_flat_room_or_one_under_no_roof() {
        let mut m = ArchModel::default();
        let room_id = m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 0.1, 5.95, 7.9), ..Room::default() });
        let room = m.room(room_id).unwrap().clone();
        assert_eq!(vault_text(&m, &room), None);
        let mut vaulted = room.clone();
        vaulted.flat_ceiling = false;
        m.rooms[0] = vaulted;
        assert_eq!(vault_text(&m, &room), None);
    }

    #[test]
    fn soffit_outlines_are_solid_with_the_soffit_handle() {
        let mut m = ArchModel::default();
        let room_id = m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 0.1, 5.95, 7.9), ..Room::default() });
        let _ = room_id;
        let soffit =
            cadcraft_doc::arch::Soffit { id: cadcraft_doc::arch::ArchId(15), level: 0, polygon: rect(1.0, 1.0, 2.0, 2.0), ..Default::default() };
        m.soffits = vec![soffit];
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        assert!(out.lines.iter().any(|l| l.kind == PlanLineKind::Soffit && l.handle == soffit_handle(cadcraft_doc::arch::ArchId(15))));
    }

    #[test]
    fn ceiling_items_outline_ceiling_furnishings_only() {
        let mut m = ArchModel::default();
        let room_id = m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 0.1, 5.95, 7.9), ..Room::default() });
        let _ = room_id;
        let item = Furnishing {
            id: cadcraft_doc::arch::ArchId(16),
            placement: Placement::Ceiling,
            level: 0,
            size: [1.0, 1.0, 1.0],
            at: [3.0, 5.0],
            ..Furnishing::default()
        };
        m.furnishings = vec![item];
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        assert!(out.lines.iter().any(|l| l.kind == PlanLineKind::CeilingItem));
    }

    #[test]
    fn ceiling_plan_only_for_the_shown_level() {
        let mut m = ArchModel::default();
        let room0_id = m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 0.1, 5.95, 7.9), ..Room::default() });
        let room1_id = m.push_room(Room { level: 1, room_type: RoomType::Living, polygon: rect(0.1, 0.1, 5.95, 7.9), ..Room::default() });
        let _ = (room0_id, room1_id);
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        assert!(out.lines.iter().all(|l| l.kind == PlanLineKind::VaultLine
            || l.handle == room_handle(cadcraft_doc::arch::ArchId(0))
            || l.handle == room_handle(cadcraft_doc::arch::ArchId(1))
            || l.handle == tray_handle(cadcraft_doc::arch::ArchId(0))
            || l.handle == tray_handle(cadcraft_doc::arch::ArchId(1))
            || l.handle == soffit_handle(cadcraft_doc::arch::ArchId(0))
            || l.handle == soffit_handle(cadcraft_doc::arch::ArchId(1))
            || l.handle == furnishing_handle(cadcraft_doc::arch::ArchId(0))
            || l.handle == furnishing_handle(cadcraft_doc::arch::ArchId(1))));
    }

    #[test]
    fn hostile_trays_soffits_and_roofs_never_panic() {
        let mut m = ArchModel::default();
        let room_id = m.push_room(Room { level: 0, room_type: RoomType::Living, polygon: rect(0.1, 0.1, 5.95, 7.9), ..Room::default() });
        let bad = Tray { id: cadcraft_doc::arch::ArchId(19), level: 0, room: room_id, inset: f64::NAN, depth: f64::NAN, ..Tray::default() };
        m.trays = vec![bad];
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        assert!(out.lines.is_empty());
    }

    #[test]
    fn ceiling_item_on_another_level_is_not_drawn() {
        let item = Furnishing { level: 0, placement: Placement::Ceiling, at: [8.0, 7.6], size: [2.0, 0.6, 0.2], ..Furnishing::default() };
        let mut m = level_model(1);
        m.push_furnishing(item);
        assert!(lines_of(&m, Some(1), PlanLineKind::CeilingItem).is_empty());
    }

    #[test]
    fn ceiling_item_draws_without_rooms() {
        let item = Furnishing { level: 0, placement: Placement::Ceiling, at: [8.0, 7.6], size: [2.0, 0.6, 0.2], ..Furnishing::default() };
        let mut m = level_model(0);
        m.push_furnishing(item);
        assert_eq!(lines_of(&m, Some(0), PlanLineKind::CeilingItem).len(), 4);
    }

    #[test]
    fn multiple_soffits_each_drawn_once() {
        let mut m = level_model(3);
        let id0 = m.push_soffit(Soffit { level: 0, polygon: rect(7.0, 7.3, 9.0, 7.9), ..Soffit::default() });
        let id1 = m.push_soffit(Soffit { level: 0, polygon: rect(1.0, 1.0, 3.0, 2.0), ..Soffit::default() });
        let lines = lines_of(&m, Some(0), PlanLineKind::Soffit);
        assert_eq!(lines.len(), 8);
        let count0 = lines.iter().filter(|l| l.handle == soffit_handle(id0)).count();
        let count1 = lines.iter().filter(|l| l.handle == soffit_handle(id1)).count();
        assert_eq!(count0, 4);
        assert_eq!(count1, 4);
    }

    #[test]
    fn degenerate_soffit_draws_nothing() {
        let mut m = level_model(1);
        let two_points = vec![Vec2::new(1.0, 1.0), Vec2::new(2.0, 2.0)];
        m.push_soffit(Soffit { level: 0, polygon: two_points, ..Soffit::default() });
        let nan_point = vec![Vec2::new(f64::NAN, 1.0), Vec2::new(2.0, 2.0), Vec2::new(3.0, 3.0)];
        m.push_soffit(Soffit { level: 0, polygon: nan_point, ..Soffit::default() });
        let coincident = vec![Vec2::new(1.0, 1.0), Vec2::new(1.0, 1.0), Vec2::new(2.0, 2.0)];
        m.push_soffit(Soffit { level: 0, polygon: coincident, ..Soffit::default() });
        assert!(lines_of(&m, Some(0), PlanLineKind::Soffit).is_empty());
    }

    #[test]
    fn hostile_ceiling_item_never_panics() {
        let item = Furnishing { level: 0, placement: Placement::Ceiling, at: [f64::NAN, 7.6], size: [2.0, 0.6, 0.2], ..Furnishing::default() };
        let mut m = level_model(1);
        m.push_furnishing(item);
        let item2 = Furnishing { level: 0, placement: Placement::Ceiling, at: [8.0, 7.6], size: [f64::INFINITY, 0.6, 0.2], ..Furnishing::default() };
        m.push_furnishing(item2);
        let lines = lines_of(&m, Some(0), PlanLineKind::CeilingItem);
        assert!(lines.iter().all(|l| l.a.is_finite() && l.b.is_finite()));
    }

    #[test]
    fn floor_furnishing_not_drawn_on_ceiling_plan() {
        let floor_item = Furnishing { level: 0, placement: Placement::Floor, at: [8.0, 7.6], size: [2.0, 0.6, 0.2], ..Furnishing::default() };
        let wall_item = Furnishing { level: 0, placement: Placement::Wall, at: [8.0, 7.6], size: [2.0, 0.6, 0.2], ..Furnishing::default() };
        let mut m = level_model(1);
        m.push_furnishing(floor_item);
        m.push_furnishing(wall_item);
        assert!(lines_of(&m, Some(0), PlanLineKind::CeilingItem).is_empty());
    }

    #[test]
    fn empty_model_draws_nothing() {
        let m = level_model(0);
        let mut out = PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        assert!(out.lines.is_empty());
        assert!(out.labels.is_empty());
    }
}
