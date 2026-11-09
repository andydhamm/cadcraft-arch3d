//! Interior soffits (phase 16): boxes hung from the ceiling; ceiling items over one hang from
//! its underside.
//!
//! Stub with a fixed signature; `p16-soffits` fills it. Until then there are no soffits.

use cadcraft_doc::arch::ArchModel;
use cadcraft_geom::Vec2;

/// The bottom (absolute z) of the first soffit on `level` whose outline holds `p`, else `None`.
/// The stub finds none.
#[allow(unused_variables)]
pub fn bottom_at(model: &ArchModel, level: u32, p: Vec2) -> Option<f64> {
    None
}
