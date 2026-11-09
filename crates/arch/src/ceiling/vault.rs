//! Ceilings that follow the roof (phase 16): a room with `flat_ceiling` off takes the underside
//! of the roof planes over it, and a flat ceiling never rises above that underside.
//!
//! Stubs with fixed signatures; `p16-vault-ceiling` fills them. Until then every ceiling is flat.

use cadcraft_doc::arch::{ArchModel, Room};
use cadcraft_geom::Vec2;

/// The rough ceiling of `room` at plan point `p`, given its flat rough ceiling `flat`
/// (absolute). The stub keeps the flat ceiling.
#[allow(unused_variables)]
pub fn adjust(model: &ArchModel, room: &Room, flat: f64, p: Vec2) -> f64 {
    flat
}

/// Refuse a flat ceiling at `flat_z` (absolute) that would rise above the roof underside over
/// `room`; the error names the room. The stub accepts everything.
#[allow(unused_variables)]
pub fn check_flat_ceiling(model: &ArchModel, room: &Room, flat_z: f64) -> Result<(), String> {
    Ok(())
}

/// A warning for a flat ceiling a roof edit left above the roof underside, else `None`.
/// The stub never warns.
#[allow(unused_variables)]
pub fn ceiling_warning(model: &ArchModel, room: &Room) -> Option<String> {
    None
}
