//! Room molding meshes.

use cadcraft_arch::molding_runs::MoldingRun;
use cadcraft_doc::ArchId;

use crate::mesh::Mesh;

/// The meshes of one room's moldings, one per run (`molding-room-*` materials, object = `room`),
/// `level` being the room level's elevation.
///
/// Stub: owner p18-room-moldings.
pub fn room_molding_meshes(_room: ArchId, _level: f64, _runs: &[MoldingRun]) -> Vec<Mesh> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_builds_nothing() {
        assert!(room_molding_meshes(ArchId(0), 0.0, &[]).is_empty());
    }
}
