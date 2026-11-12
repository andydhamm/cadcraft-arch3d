//! Reflected-ceiling annotation for the plan pipeline.

use cadcraft_doc::arch::{ArchModel, Room};
use cadcraft_geom::Vec2;

use crate::ceiling::{flat_ceiling_z, stack};

/// The reflected ceiling plan's annotation (tray outlines and heights, vault lines, ceiling
/// items, soffits). Stub: owner p20-rcp.
pub fn ceiling_plan(_model: &ArchModel, _level: Option<u32>, _out: &mut super::PlanAnnotation) {}

/// A vaulted room's pitch text. Stub: owner p20-rcp.
pub fn vault_text(_model: &ArchModel, _room: &Room) -> Option<String> {
    None
}

/// The outlines of the room's top-level trays, which the room label block keeps out of when it
/// shows the ceiling height. Stub: owner p20-rcp.
pub fn ceiling_label_holes(_model: &ArchModel, _room: &Room) -> Vec<Vec<Vec2>> {
    Vec::new()
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
    use cadcraft_doc::arch::{CeilingLayer, Level, RoomType};

    fn room_model() -> (ArchModel, Room) {
        let mut m = ArchModel::default();
        m.levels = vec![Level { elevation: 0.0, ceiling_height: 2.4, ..Level::default() }];
        let room = Room { room_type: RoomType::Living, ..Room::default() };
        (m, room)
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
        // A level at elevation 2.7 still reads 2.40.
        m.levels[0].elevation = 2.7;
        assert_eq!(room_ceiling_text(&m, &room).as_deref(), Some("2.40"));
        // A room type without a ceiling.
        let none = [RoomType::Garage, RoomType::Other, RoomType::Deck]
            .into_iter()
            .map(|t| Room { room_type: t, ..Room::default() })
            .find(|r| !r.room_type.has_ceiling());
        if let Some(r) = none {
            assert_eq!(room_ceiling_text(&m, &r), None);
        }
    }

    #[test]
    fn ceiling_stubs_are_empty() {
        let (m, room) = room_model();
        assert_eq!(vault_text(&m, &room), None);
        assert!(ceiling_label_holes(&m, &room).is_empty());
        let mut out = super::super::PlanAnnotation::default();
        ceiling_plan(&m, Some(0), &mut out);
        assert!(out.lines.is_empty() && out.labels.is_empty());
        super::super::reference::reference_lines(&m, 0, &cadcraft_doc::arch::ReferenceDisplay::default(), &mut out);
        super::super::roof::roof_plan(&m, &mut out);
        assert!(out.lines.is_empty() && out.labels.is_empty());
    }
}
