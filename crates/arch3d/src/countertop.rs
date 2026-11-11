//! Custom countertop meshes.

use cadcraft_arch::countertop::CounterOutline;
use cadcraft_doc::arch::Countertop;

use crate::mesh::Mesh;

/// The meshes of one custom countertop (`countertop*` materials, object = the countertop), built
/// from its resolved `outline` (`None` when it has none), `level` being its level's elevation.
///
/// Stub: owner p18-countertops.
pub fn countertop_meshes(_ct: &Countertop, _outline: Option<&CounterOutline>, _level: f64) -> Vec<Mesh> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_builds_nothing() {
        assert!(countertop_meshes(&Countertop::default(), None, 0.0).is_empty());
    }
}
