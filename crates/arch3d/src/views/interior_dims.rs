//! Dimensions of an interior elevation. Owner: p21-interior-dims (stub: the overall width and height).

use crate::Scene3d;
use crate::hlr::OrthoView;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchView, ViewDim};

/// The dimensions of an interior view whose lines span `extent`. Owner: p21-interior-dims.
pub(crate) fn interior_dims(_model: &ArchModel, view: &ArchView, _sc: &Scene3d, _ov: &OrthoView, extent: [f64; 4]) -> Vec<ViewDim> {
    super::overall_dims(view, Some(extent))
}
