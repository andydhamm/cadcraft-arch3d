//! Material pattern lines over elevation faces. Owner: p21-patterns (stub until then).

use crate::Scene3d;
use crate::hlr::OrthoView;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::ViewPatternLine;

/// Pattern lines for the visible faces of `sc` in `ov`. Owner: p21-patterns.
pub(crate) fn pattern_lines(_model: &ArchModel, _sc: &Scene3d, _ov: &OrthoView) -> Vec<ViewPatternLine> {
    Vec::new()
}
