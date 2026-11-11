//! Free molding line meshes.

use cadcraft_doc::arch::MoldingLine;

use crate::mesh::Mesh;

/// The meshes of one molding line (`molding-line*` materials, object = the line), `level` being its
/// level's elevation.
///
/// Stub: owner p18-molding-lines.
pub fn line_meshes(_line: &MoldingLine, _level: f64) -> Vec<Mesh> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_builds_nothing() {
        assert!(line_meshes(&MoldingLine::default(), 0.0).is_empty());
    }
}
