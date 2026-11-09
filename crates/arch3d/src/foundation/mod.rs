//! Foundations (phase 16; owner: p16-foundation-stem): stem walls on footings, or a slab on grade
//! with a thickened edge, under the exterior walls on grade.
//!
//! Meshes: stem walls `foundation`, footings and slab edges `footing`, slab floors `slab` (the
//! room's floor, built in the room's part).

use cadcraft_arch::foundation::{FoundationWall, foundation_walls, is_basement_level};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Foundation, FoundationKind, Level};

use crate::mesh::Mesh;

pub use cadcraft_arch::foundation::FoundationWall;
pub mod slab;
pub mod stem;

/// Everything a foundation's meshes are made from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FoundationIn {
    pub foundation: Foundation,
    pub level: Level,
    /// The walls it stands under (`cadcraft_arch::foundation::foundation_walls` of its level), so a
    /// wall edit rebuilds the foundation.
    pub walls: Vec<FoundationWall>,
    /// Whether its level is a basement.
    pub basement: bool,
}

/// The inputs of foundation `f` in `model`.
pub(crate) fn foundation_input(model: &ArchModel, f: &Foundation) -> FoundationIn {
    let walls = foundation_walls(model, f.level);
    // Every foundation wall carries its level's class; without walls, ask the level.
    let basement = walls.first().map_or_else(|| is_basement_level(model, f.level), |w| w.basement);
    FoundationIn { foundation: f.clone(), level: crate::level_for(model, f.level), walls, basement }
}

/// The meshes of one foundation, by its kind.
pub(crate) fn foundation_meshes(i: &FoundationIn) -> Vec<Mesh> {
    match i.foundation.kind {
        FoundationKind::Stem => stem::meshes(i),
        FoundationKind::Slab => slab::meshes(i),
    }
}
