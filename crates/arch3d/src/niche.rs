//! Shower niches and shower doors.

use cadcraft_doc::arch::{ArchModel, Opening};
use cadcraft_doc::{ArchId, Wall};

use crate::Scene3d;
use crate::mesh::Mesh;

/// Add the back blocks of the niches in `openings` to the wall `mesh` (so finishes and paints split
/// them like the rest of the wall); `span` is the wall's (base, top).
///
/// Stub: owner p18-shower-niche.
pub fn add_back_blocks(_mesh: &mut Mesh, _wall: &Wall, _openings: &[Opening], _span: (f64, f64)) {}

/// Extra skins of the niches in `openings` (`niche-back*` materials).
///
/// Stub: owner p18-shower-niche.
pub fn back_skins(_wall: &Wall, _openings: &[Opening], _span: (f64, f64)) -> Vec<Mesh> {
    Vec::new()
}

/// The meshes of an opening that builds its own parts (a shower door or a niche), or `None` for
/// every other opening. `walls` are the walls by id, `span` the host wall's (base, top).
///
/// Stub: owner p18-shower-niche.
pub fn special_opening_meshes(_walls: &std::collections::BTreeMap<ArchId, &Wall>, _o: &Opening, _span: Option<(f64, f64)>) -> Option<Vec<Mesh>> {
    None
}

/// Fill the scene's materials for niche parts.
///
/// Stub: owner p18-shower-niche.
pub fn niche_materials(_model: &ArchModel, _scene: &mut Scene3d) {}
