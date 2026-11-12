//! Plan schedule-number labels.
//!
//! Draws the schedule number (`ScheduleNumber`) on each opening and cabinet in plan, on the
//! kind's tag layer, only when the active plan view's `labels.numbers` is on.
//!
//! Owner: p21-plan-numbers.

use cadcraft_doc::arch::{ArchModel, RoomLabelShow};

use super::{PlanAnnotation, PlanLabelKind};

/// Add schedule-number plan labels for the given plan level and label show settings.
pub fn schedule_labels(_model: &ArchModel, _level: Option<u32>, _show: RoomLabelShow, _out: &mut PlanAnnotation) {}
