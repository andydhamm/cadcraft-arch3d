//! Tag labels on elevation objects. Owner: p21-elevation-marks (stub until then).

use crate::hlr::OrthoView;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchView, ViewLabel, ViewLine};

/// Tag labels for `view`, given its final `lines`. Owner: p21-elevation-marks.
pub(crate) fn tag_labels(_model: &ArchModel, _view: &ArchView, _ov: &OrthoView, _lines: &[ViewLine]) -> Vec<ViewLabel> {
    Vec::new()
}
