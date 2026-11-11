//! Backsplash meshes.

use cadcraft_arch::backsplash::BacksplashSegment;
use cadcraft_doc::arch::Backsplash;

use crate::mesh::Mesh;

/// The meshes of one backsplash (`backsplash` material, object = the backsplash) over its resolved
/// `segments`, `level` being its level's elevation.
///
/// Stub: owner p18-backsplash.
pub fn backsplash_meshes(_b: &Backsplash, _segments: &[BacksplashSegment], _level: f64) -> Vec<Mesh> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_builds_nothing() {
        assert!(backsplash_meshes(&Backsplash::default(), &[], 0.0).is_empty());
    }
}
