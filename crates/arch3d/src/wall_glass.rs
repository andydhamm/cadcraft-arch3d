//! Glass walls.

use crate::incremental::WallIn;
use crate::mesh::Mesh;

/// The meshes of a glass (or glass-split) wall, or `None` for an ordinary wall, which then builds
/// as before. When it returns `Some` for a split wall it must call `niche::add_back_blocks` and
/// `niche::back_skins` on its lower part itself.
///
/// Stub: owner p18-glass-walls.
pub(crate) fn wall_meshes(_i: &WallIn) -> Option<Vec<Mesh>> {
    None
}
