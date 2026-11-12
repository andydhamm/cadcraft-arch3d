//! Plan callout markers.
//!
//! Marks every placed elevation, section and interior elevation with a callout label
//! (`Callout`), and resolves a point near a placed elevation to its view.
//!
//! Owner: p21-callouts.

use cadcraft_doc::arch::{ArchId, ArchModel};
use cadcraft_geom::Vec2;

use super::PlanAnnotation;

/// Add callout plan lines/labels for the given plan level.
pub fn callouts(_model: &ArchModel, _level: Option<u32>, _out: &mut PlanAnnotation) {}

/// The view id whose callout marker lies near `p` (within `tol`), if any.
pub fn callout_at(_model: &ArchModel, _level: Option<u32>, _p: Vec2, _tol: f64) -> Option<ArchId> {
    None
}
