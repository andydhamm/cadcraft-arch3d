//! Interior elevations: the scene seen from inside a room. Owner: p21-interior-elevation (stub until then).

use crate::Scene3d;
use cadcraft_doc::arch::ArchView;
use cadcraft_doc::{ArchId, ArchModel};

/// The scene as seen by an interior view, or why it cannot be made. Owner: p21-interior-elevation.
pub(crate) fn restrict(_model: &ArchModel, _scene: &Scene3d, _view: &ArchView) -> Result<Scene3d, String> {
    Err("interior elevations are not available yet".to_string())
}

/// The room an interior view looks from, found again after rooms were renumbered. Owner: p21-interior-elevation.
pub(crate) fn resolve_room(_model: &ArchModel, view: &ArchView) -> Option<ArchId> {
    view.room
}
