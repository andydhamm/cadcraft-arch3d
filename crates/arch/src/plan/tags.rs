//! Plan schedule-number labels.
//!
//! Draws the schedule number (`ScheduleNumber`) on each opening and cabinet in plan, on the
//! kind's tag layer, only when the active plan view's `labels.numbers` is on.
//!
//! Owner: p21-plan-numbers.

use cadcraft_doc::arch::{ArchId, ArchModel, Cabinet, Opening, OpeningKind, RoomLabelShow, Wall, cabinet_handle, opening_handle};
use cadcraft_geom::Vec2;

use super::{PlanAnnotation, PlanLabel, PlanLabelKind};

/// Tag layer for door schedule numbers.
pub const LAYER_DOOR_TAG: &str = "A-DOOR-IDEN";
/// Tag layer for window schedule numbers.
pub const LAYER_WINDOW_TAG: &str = "A-GLAZ-IDEN";
/// Tag layer for cabinet schedule numbers.
pub const LAYER_CASE_TAG: &str = "A-FLOR-CASE-IDEN";

/// Add schedule-number plan labels for the given plan level and label show settings.
pub fn schedule_labels(model: &ArchModel, level: Option<u32>, show: RoomLabelShow, out: &mut PlanAnnotation) {
    if !show.numbers {
        return;
    }

    let numbers = crate::schedule::numbers(model);

    // Openings by index (deterministic order)
    for (i, opening) in model.openings.iter().enumerate() {
        let number = match numbers.openings.get(&i) {
            Some(n) => n,
            None => continue,
        };
        let wall = match model.walls.get(opening.wall as usize) {
            Some(w) => w,
            None => continue,
        };
        if let Some(lvl) = level {
            if wall.base_level != lvl {
                continue;
            }
        }
        let start = wall.start;
        let end = wall.end;
        if !start.is_finite() || !end.is_finite() {
            continue;
        }
        let len = (end - start).len();
        if len < 1e-9 {
            continue;
        }
        let dir = (end - start).normalized();
        let right = -dir.perp();
        let at = start + dir * opening.offset + right * (wall.thickness / 2.0 + 0.35);
        let layer = match opening.kind {
            OpeningKind::Door | OpeningKind::GarageDoor | OpeningKind::CasedOpening => LAYER_DOOR_TAG,
            OpeningKind::Window => LAYER_WINDOW_TAG,
        };
        out.push_label(PlanLabel::new(layer, PlanLabelKind::ScheduleNumber, number.clone(), at, 0.2, opening_handle(ArchId(i))));
    }

    // Cabinets by index (deterministic order)
    for (i, cabinet) in model.cabinets.iter().enumerate() {
        let number = match numbers.cabinets.get(&i) {
            Some(n) => n,
            None => continue,
        };
        let at = cabinet.position + Vec2::new(0.0, cabinet.depth / 2.0).rotate(cabinet.rotation);
        out.push_label(PlanLabel::new(LAYER_CASE_TAG, PlanLabelKind::ScheduleNumber, number.clone(), at, 0.2, cabinet_handle(ArchId(i))));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn test_model() -> ArchModel {
        let mut model = ArchModel::default();
        model.levels = vec![
            cadcraft_doc::arch::Level { index: 0, name: "L0".into(), ..Default::default() },
            cadcraft_doc::arch::Level { index: 1, name: "L1".into(), elevation: 3.0, ..Default::default() },
        ];
        // Wall on level 0 from (0,0) to (10,0)
        model.walls.push(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, base_level: 0, ..Default::default() });
        // Wall on level 1
        model.walls.push(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, base_level: 1, ..Default::default() });
        // Door opening on wall 0 at offset 5
        model.openings.push(Opening { wall: 0, offset: 5.0, kind: OpeningKind::Door, ..Default::default() });
        // Window opening on wall 0 at offset 2
        model.openings.push(Opening { wall: 0, offset: 2.0, kind: OpeningKind::Window, ..Default::default() });
        // Door opening on wall 1 (level 1)
        model.openings.push(Opening { wall: 1, offset: 3.0, kind: OpeningKind::Door, ..Default::default() });
        // Cabinet at (1,1) with depth 0.6, no rotation
        model.cabinets.push(Cabinet { position: Vec2::new(1.0, 1.0), depth: 0.6, rotation: 0.0, ..Default::default() });
        model
    }

    #[test]
    fn openings_label_beyond_the_right_face() {
        let model = test_model();
        let mut out = PlanAnnotation::default();
        let show = RoomLabelShow { name: false, area: false, dimensions: false, ceiling: false, numbers: true };
        schedule_labels(&model, None, show, &mut out);

        // Wall 0: (0,0) to (10,0), dir=(1,0), right normal = -(0,1) = (0,-1)
        // Door at offset 5: at = (5,0) + (0,-1)*(0.1+0.35) = (5, -0.45)
        let door_label = out.labels.iter().find(|l| l.text == "D01").expect("D01 label");
        assert!((door_label.at.x - 5.0).abs() < 1e-9, "door x: {}", door_label.at.x);
        assert!((door_label.at.y - (-0.45)).abs() < 1e-9, "door y: {}", door_label.at.y);

        // Window at offset 2: at = (2,0) + (0,-1)*(0.45) = (2, -0.45)
        let window_label = out.labels.iter().find(|l| l.text == "W01").expect("W01 label");
        assert!((window_label.at.x - 2.0).abs() < 1e-9, "window x: {}", window_label.at.x);
        assert!((window_label.at.y - (-0.45)).abs() < 1e-9, "window y: {}", window_label.at.y);
    }

    #[test]
    fn cabinets_label_at_their_plan_centre() {
        let model = test_model();
        let mut out = PlanAnnotation::default();
        let show = RoomLabelShow { name: false, area: false, dimensions: false, ceiling: false, numbers: true };
        schedule_labels(&model, None, show, &mut out);

        // Cabinet at (1,1), depth 0.6, rotation 0: centre = (1,1) + (0,0.3).rotate(0) = (1, 1.3)
        let cab_label = out.labels.iter().find(|l| l.text == "C01").expect("C01 label");
        assert!((cab_label.at.x - 1.0).abs() < 1e-9, "cabinet x: {}", cab_label.at.x);
        assert!((cab_label.at.y - 1.3).abs() < 1e-9, "cabinet y: {}", cab_label.at.y);
    }

    #[test]
    fn labels_carry_the_object_handle_and_layer() {
        let model = test_model();
        let mut out = PlanAnnotation::default();
        let show = RoomLabelShow { name: false, area: false, dimensions: false, ceiling: false, numbers: true };
        schedule_labels(&model, None, show, &mut out);

        let door_label = out.labels.iter().find(|l| l.text == "D01").expect("D01 label");
        assert_eq!(door_label.layer, LAYER_DOOR_TAG);
        assert_eq!(door_label.handle, opening_handle(ArchId(0)));

        let window_label = out.labels.iter().find(|l| l.text == "W01").expect("W01 label");
        assert_eq!(window_label.layer, LAYER_WINDOW_TAG);
        assert_eq!(window_label.handle, opening_handle(ArchId(1)));

        let cab_label = out.labels.iter().find(|l| l.text == "C01").expect("C01 label");
        assert_eq!(cab_label.layer, LAYER_CASE_TAG);
        assert_eq!(cab_label.handle, cabinet_handle(ArchId(0)));
    }

    #[test]
    fn level_filter() {
        let model = test_model();
        let mut out = PlanAnnotation::default();
        let show = RoomLabelShow { name: false, area: false, dimensions: false, ceiling: false, numbers: true };
        // Only level 0
        schedule_labels(&model, Some(0), show, &mut out);

        // Should have labels for openings 0,1 (on wall 0, level 0) and cabinet 0
        // But not opening 2 (on wall 1, level 1)
        let texts: Vec<&str> = out.labels.iter().map(|l| l.text.as_str()).collect();
        assert!(texts.contains(&"D01"), "D01 missing: {:?}", texts);
        assert!(texts.contains(&"W01"), "W01 missing: {:?}", texts);
        assert!(!texts.contains(&"D02"), "D02 should not be on level 0: {:?}", texts);
    }

    #[test]
    fn numbers_off_draws_nothing() {
        let model = test_model();
        let mut out = PlanAnnotation::default();
        let show = RoomLabelShow { name: false, area: false, dimensions: false, ceiling: false, numbers: false };
        schedule_labels(&model, None, show, &mut out);
        assert!(out.labels.is_empty(), "no labels when numbers off");
    }
}
