//! Tray ceilings (phase 16): a tray recesses part of a room's ceiling up into the attic.
//!
//! Stub with a fixed signature; `p16-trays` fills it. Until then no ceiling is lifted.

use cadcraft_doc::arch::{ArchModel, Room};
use cadcraft_geom::Vec2;

/// How far the trays of `room` lift its rough ceiling at plan point `p`, in metres.
/// The stub lifts nothing.
#[allow(unused_variables)]
pub fn lift(model: &ArchModel, room: &Room, p: Vec2) -> f64 {
    0.0
}
