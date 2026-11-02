//! Incremental 3D rebuild: kind-tagged dirty set, scene cache, and a
//! deterministic scene digest.
//!
//! Wall, opening, room and roof ids all number from zero per kind, so two
//! distinct objects (w3 and o3) share the same `ArchId`.  Every mesh is
//! therefore keyed by [`ObjectKind`] plus its id, and the scene cache diffs
//! models by object kind.  When a field that is not owned by a single object
//! changes (levels, wall types, the site, plant species) nothing is owned by
//! exactly one object, so the whole scene is rebuilt.

use std::collections::BTreeSet;

use crate::probe::part_of;
use cadcraft_doc::ArchId;

use crate::Scene3d;
use cadcraft_doc::{ArchModel, Opening, Room, Railing, Roof, Stair, Wall};
use crate::arch::light::LightFixture;
use crate::plant::Plant;

/// The architectural object a mesh belongs to.
///
/// `ArchId` is counted separately per kind, so `w3` and `o3` are the same
/// `ArchId`.  This kind is what disambiguates them.  New object types added by
/// later phases get a new variant here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[allow(dead_code)]
pub enum ObjectKind {
    Wall,
    Opening,
    Room,
    Roof,
    Stair,
    Railing,
    Terrain,
    Plant,
    Light,
}

impl ObjectKind {
    fn from_part(part: &str) -> ObjectKind {
        // Map a normalized part name to its owning object kind.  The part name
        // comes from [`part_of`]; several part names map to one kind (a room
        // owns its floor, ceiling and deck slab; a terrain owns its paths).
        match part {
            "wall" => ObjectKind::Wall,
            "floor" | "ceiling" | "deck" => ObjectKind::Room,
            "glass" | "frame" | "casing" | "leaf" | "sill" | "header" => ObjectKind::Opening,
            "roof" => ObjectKind::Roof,
            "stair" => ObjectKind::Stair,
            "railing" => ObjectKind::Railing,
            "terrain" | "path" => ObjectKind::Terrain,
            "plant" => ObjectKind::Plant,
            "light" => ObjectKind::Light,
            _ => ObjectKind::Wall,
        }
    }
}

/// A unique identity for a mesh's owner, disambiguated by kind.
///
/// Two meshes with the same id are the *same* object only when their kind
/// agrees, so a `w3` and an `o3` never collide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectKey {
    pub kind: ObjectKind,
    pub id: ArchId,
}

impl ObjectKey {
    /// The kind for the mesh's owner, derived from its part and object id.
    pub fn from(mesh: &Mesh) -> Self {
        ObjectKey {
            kind: ObjectKind::from_part(&part_of(mesh)),
            id: mesh.object,
        }
    }
}
