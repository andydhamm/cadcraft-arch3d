//! Indicator lines such as door swings. Owner: p21-elevation-marks (stub until then).

use crate::Scene3d;
use crate::hlr::OrthoView;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchView, ViewLine};

/// Indicator lines for `view`. Owner: p21-elevation-marks.
pub(crate) fn indicator_lines(_model: &ArchModel, _sc: &Scene3d, _view: &ArchView, _ov: &OrthoView) -> Vec<ViewLine> {
    Vec::new()
}
