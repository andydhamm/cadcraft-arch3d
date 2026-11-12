//! Room labels for the plan pipeline.

use cadcraft_doc::arch::{ArchModel, RoomLabelShow, room_handle};
use cadcraft_geom::Vec2;

use super::{PlanAnnotation, PlanLabel, PlanLabelKind};

/// Room labels on `level` (every room when `None`). Name only for now (the room's name, or
/// `r<id>` when unnamed, at the vertex average of its polygon); area, dimensions and ceiling
/// are p20-room-labels'.
pub fn room_labels(model: &ArchModel, level: Option<u32>, show: RoomLabelShow, out: &mut PlanAnnotation) {
    for r in &model.rooms {
        if r.polygon.len() < 3 || level.is_some_and(|l| l != r.level) || !r.polygon.iter().all(|p| p.is_finite()) {
            continue;
        }
        if show.name {
            let text = if r.name.is_empty() { format!("r{}", r.id.0) } else { r.name.clone() };
            let n = r.polygon.len() as f64;
            let at = Vec2::new(r.polygon.iter().map(|p| p.x).sum::<f64>() / n, r.polygon.iter().map(|p| p.y).sum::<f64>() / n);
            out.push_label(PlanLabel::new("A-ROOM", PlanLabelKind::RoomName, text, at, 0.24, room_handle(r.id)));
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::arch::Room;

    fn quad(x: f64, y: f64) -> Vec<Vec2> {
        vec![Vec2::new(x, y), Vec2::new(x + 4.0, y), Vec2::new(x + 4.0, y + 3.0), Vec2::new(x, y + 3.0)]
    }

    #[test]
    fn room_labels_name_only_matches_todays_render() {
        let mut m = ArchModel::default();
        let a = m.push_room(Room { polygon: quad(0.0, 0.0), name: "Living".into(), ..Room::default() });
        let b = m.push_room(Room { polygon: quad(10.0, 0.0), level: 1, ..Room::default() });
        m.push_room(Room { polygon: vec![Vec2::ZERO, Vec2::new(1.0, 0.0)], ..Room::default() });
        m.push_room(Room { polygon: vec![Vec2::ZERO, Vec2::new(f64::NAN, 0.0), Vec2::new(1.0, 1.0)], ..Room::default() });
        let show = RoomLabelShow::default();
        let mut out = PlanAnnotation::default();
        room_labels(&m, None, show, &mut out);
        assert_eq!(out.labels.len(), 2);
        let l = &out.labels[0];
        assert_eq!((l.text.as_str(), l.layer.as_str(), l.kind), ("Living", "A-ROOM", PlanLabelKind::RoomName));
        assert_eq!((l.at, l.height, l.rotation, l.handle), (Vec2::new(2.0, 1.5), 0.24, 0.0, room_handle(a)));
        assert_eq!(out.labels[1].text, format!("r{}", b.0));
        let mut out = PlanAnnotation::default();
        room_labels(&m, Some(1), show, &mut out);
        assert_eq!(out.labels.len(), 1);
        assert_eq!(out.labels[0].handle, room_handle(b));
        let mut out = PlanAnnotation::default();
        room_labels(&m, None, RoomLabelShow { name: false, ..show }, &mut out);
        assert!(out.labels.is_empty());
    }
}
