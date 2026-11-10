//! Where a stair flight cuts the underside of a wall above it (filled by the wall-under-flight segment).

use crate::{ArchId, ArchModel, Wall};
use cadcraft_geom::Vec2;

/// Smallest height of wall left standing over a flight cut.
pub const MIN_CUT_HEIGHT: f64 = 0.05;

/// A polyline `(w, z)` along a wall: `w` is the distance along the wall's centre line from
/// `wall.start`, increasing; `z` is absolute. The wall stops at it over `[first w, last w]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlightCut {
    /// The stair that cuts.
    pub stair: ArchId,
    /// The `(w, z)` polyline.
    pub top: Vec<Vec2>,
}

/// The flights that cut `wall` from above. Stub: none yet.
pub fn flight_cuts(_model: &ArchModel, _wall: &Wall) -> Vec<FlightCut> {
    Vec::new()
}
