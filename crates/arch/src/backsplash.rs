//! Backsplash segments (phase 18): where a backsplash stands on its wall.

use cadcraft_doc::arch::{ArchId, ArchModel, Backsplash};
use cadcraft_geom::Vec2;

/// One straight piece of a backsplash.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BacksplashSegment {
    /// Start along the wall face.
    pub a: Vec2,
    /// End along the wall face.
    pub b: Vec2,
    /// The wall it stands on.
    pub wall: ArchId,
    /// Absolute z of the bottom.
    pub bottom: f64,
    /// Absolute z of the top.
    pub top: f64,
}

/// The segments of `b`; the thickness goes from the wall face into the room.
///
/// Stub: owner p18-backsplash.
pub fn segments(_model: &ArchModel, _b: &Backsplash) -> Vec<BacksplashSegment> {
    Vec::new()
}
