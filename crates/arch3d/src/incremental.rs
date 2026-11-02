//! Incremental 3D rebuild: kind-tagged dirty set, scene cache, and a
//! deterministic scene digest.
//!
//! Arch ids are numbered per kind (`w3` and `o3` are both `ArchId(3)`), so a
//! mesh is keyed by [`ObjectKind`] plus [`ArchId`].  The scene cache compares
//! models on that key and rebuilds only the changed objects (or a whole scene
//! when a global value changes).

use std::collections::{BTreeMap, BTreeSet};

use crate::Scene3d;
use crate::mesh::Mesh;
use crate::probe::part_of;
use cadcraft_doc::{ArchId, ArchModel};

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

/// The architectural object a mesh belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

impl Default for ObjectKind {
    fn default() -> Self {
        Self::Wall
    }
}

impl ObjectKind {
    fn from_part(part: &str) -> Self {
        match part {
            "wall" => Self::Wall,
            "floor" | "ceiling" | "deck" => Self::Room,
            "glass" | "frame" | "casing" | "leaf" | "sill" | "header" => Self::Opening,
            "roof" => Self::Roof,
            "stair" => Self::Stair,
            "railing" => Self::Railing,
            "terrain" | "path" => Self::Terrain,
            "plant" => Self::Plant,
            "light" => Self::Light,
            _ => Self::Wall,
        }
    }
}

/// A unique identity for a mesh's owner, disambiguated by kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectKey {
    pub kind: ObjectKind,
    pub id: ArchId,
}

impl Default for ObjectKey {
    fn default() -> Self {
        Self { kind: ObjectKind::default(), id: ArchId::default() }
    }
}

impl ObjectKey {
    /// Build a key from the mesh's material and owning object.
    pub fn from(mesh: &Mesh) -> Self {
        Self { kind: ObjectKind::from_part(part_of(mesh)), id: mesh.object }
    }
}

/// A unique mesh ID for a scene digest.
pub fn object_key(mesh: &Mesh) -> ObjectKey {
    ObjectKey::from(mesh)
}

/// Compare the model by per-object data; `None` means a global change such as a
/// level or wall-type edit, which invalidates the whole scene.
pub fn changed_objects(prev: &ArchModel, next: &ArchModel) -> Option<BTreeSet<ObjectKey>> {
    // Global edits affect the whole scene and cannot be assigned to a single
    // object, so a full rebuild is required.
    if prev.levels != next.levels || prev.types != next.types || prev.site != next.site || prev.plant_species != next.plant_species {
        return None;
    }

    let mut changes = BTreeSet::new();

    let add = |changes: &mut BTreeSet<ObjectKey>, kind: ObjectKind, id: ArchId| {
        changes.insert(ObjectKey { kind, id });
    };

    fn compare<T: PartialEq>(prev: &BTreeMap<ArchId, T>, next: &BTreeMap<ArchId, T>, kind: ObjectKind, set: &mut BTreeSet<ObjectKey>) {
        let ids: BTreeSet<ArchId> = prev.keys().copied().chain(next.keys().copied()).collect();
        for id in ids {
            if let (Some(a), Some(b)) = (prev.get(&id), next.get(&id)) {
                if a == b {
                    continue;
                }
            }
            set.insert(ObjectKey { kind, id });
        }
    }

    let mut prev_walls = BTreeMap::new();
    let mut next_walls = BTreeMap::new();
    for wall in &prev.walls {
        prev_walls.insert(wall.id, wall.clone());
    }
    for wall in &next.walls {
        next_walls.insert(wall.id, wall.clone());
    }
    compare(&prev_walls, &next_walls, ObjectKind::Wall, &mut changes);

    let mut prev_openings = BTreeMap::new();
    let mut next_openings = BTreeMap::new();
    for opening in &prev.openings {
        prev_openings.insert(opening.id, opening.clone());
    }
    for opening in &next.openings {
        next_openings.insert(opening.id, opening.clone());
    }
    compare(&prev_openings, &next_openings, ObjectKind::Opening, &mut changes);

    let mut prev_rooms = BTreeMap::new();
    let mut next_rooms = BTreeMap::new();
    for room in &prev.rooms {
        prev_rooms.insert(room.id, room.clone());
    }
    for room in &next.rooms {
        next_rooms.insert(room.id, room.clone());
    }
    compare(&prev_rooms, &next_rooms, ObjectKind::Room, &mut changes);

    let mut prev_roofs = BTreeMap::new();
    let mut next_roofs = BTreeMap::new();
    for roof in &prev.roofs {
        prev_roofs.insert(roof.id, roof.clone());
    }
    for roof in &next.roofs {
        next_roofs.insert(roof.id, roof.clone());
    }
    compare(&prev_roofs, &next_roofs, ObjectKind::Roof, &mut changes);

    let mut prev_stairs = BTreeMap::new();
    let mut next_stairs = BTreeMap::new();
    for stair in &prev.stairs {
        prev_stairs.insert(stair.id, stair.clone());
    }
    for stair in &next.stairs {
        next_stairs.insert(stair.id, stair.clone());
    }
    compare(&prev_stairs, &next_stairs, ObjectKind::Stair, &mut changes);

    let mut prev_railings = BTreeMap::new();
    let mut next_railings = BTreeMap::new();
    for railing in &prev.railings {
        prev_railings.insert(railing.id, railing.clone());
    }
    for railing in &next.railings {
        next_railings.insert(railing.id, railing.clone());
    }
    compare(&prev_railings, &next_railings, ObjectKind::Railing, &mut changes);

    if prev.terrain != next.terrain {
        add(&mut changes, ObjectKind::Terrain, ArchId(0));
    }
    for plant in &prev.plants {
        let next = next.plants.iter().find(|p| p.id == plant.id);
        if next != Some(plant) {
            add(&mut changes, ObjectKind::Plant, plant.id);
        }
    }
    for plant in &next.plants {
        if prev.plants.iter().all(|p| p.id != plant.id) {
            add(&mut changes, ObjectKind::Plant, plant.id);
        }
    }

    for fixture in &prev.lights {
        let next = next.lights.iter().find(|p| p.id == fixture.id);
        if next != Some(fixture) {
            add(&mut changes, ObjectKind::Light, fixture.id);
        }
    }
    for fixture in &next.lights {
        if prev.lights.iter().all(|p| p.id != fixture.id) {
            add(&mut changes, ObjectKind::Light, fixture.id);
        }
    }

    Some(changes)
}

/// Summary of a rebuild pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RebuildStats {
    pub rebuilt: usize,
    pub objects: usize,
    pub incremental: bool,
}

/// Scene cache that stores the previous `ArchModel` and the last built scene.
pub struct SceneCache {
    model: ArchModel,
    scene: Scene3d,
}

impl SceneCache {
    /// Build a cache from the original model. This is a full build.
    pub fn new(model: &ArchModel) -> (Self, RebuildStats) {
        let scene = Scene3d::build(model);
        let objects = scene_object_keys(&scene).len();
        let stats = RebuildStats { rebuilt: objects, objects, incremental: false };
        (Self { model: model.clone(), scene }, stats)
    }

    /// Update the cache and rebuild only the changed objects, falling back to a
    /// full rebuild when a global field changes.
    pub fn update(&mut self, model: &ArchModel) -> RebuildStats {
        let prev_object_keys = scene_object_keys(&self.scene);
        let changed = changed_objects(&self.model, model);

        match changed {
            None => {
                let scene = Scene3d::build(model);
                self.model = model.clone();
                self.scene = scene;
                let objects = scene_object_keys(&self.scene).len();
                return RebuildStats { rebuilt: objects, objects, incremental: false };
            }
            Some(set) => {
                if set.is_empty() {
                    self.model = model.clone();
                    let objects = prev_object_keys.len();
                    return RebuildStats { rebuilt: 0, objects, incremental: true };
                }

                let dirty_ids: BTreeSet<ArchId> = set.iter().map(|k| k.id).collect();
                let scene = Scene3d::build_dirty(model, &dirty_ids, &self.scene);
                self.model = model.clone();
                self.scene = scene;

                let objects = scene_object_keys(&self.scene).len();
                let rebuilt = set.len().min(objects);
                RebuildStats { rebuilt, objects, incremental: true }
            }
        }
    }

    /// Access the cached scene.
    pub fn scene(&self) -> &Scene3d {
        &self.scene
    }
}

fn scene_object_keys(scene: &Scene3d) -> BTreeSet<ObjectKey> {
    scene.meshes.iter().map(object_key).collect()
}

fn fnv1a(bytes: &[u8], mut hash: u64) -> u64 {
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn digest_bytes(bytes: &[u8]) -> u64 {
    fnv1a(bytes, FNV_OFFSET_BASIS)
}

fn u32_bytes(v: u32) -> [u8; 4] {
    v.to_ne_bytes()
}

fn mesh_digest(mesh: &Mesh) -> u64 {
    let mut hash = digest_bytes(mesh.material.as_bytes());
    hash = fnv1a(
        &[mesh.object.0.to_ne_bytes()[0], mesh.object.0.to_ne_bytes()[1], mesh.object.0.to_ne_bytes()[2], mesh.object.0.to_ne_bytes()[3]],
        hash,
    );
    hash = fnv1a(&[object_key(mesh).kind as u8], hash);
    for p in &mesh.positions {
        for v in p {
            let bits = v.to_bits();
            hash = fnv1a(&bits.to_ne_bytes(), hash);
        }
    }
    for n in &mesh.normals {
        for v in n {
            let bits = v.to_bits();
            hash = fnv1a(&bits.to_ne_bytes(), hash);
        }
    }
    for uv in &mesh.uvs {
        for v in uv {
            let bits = v.to_bits();
            hash = fnv1a(&bits.to_ne_bytes(), hash);
        }
    }
    for index in &mesh.indices {
        hash = fnv1a(&u32_bytes(*index), hash);
    }
    hash
}

impl Scene3d {
    /// FNV-1a 64 digest of the scene, independent of mesh order.
    pub fn digest(&self) -> String {
        let mut digests: Vec<u64> = self.meshes.iter().map(mesh_digest).collect();
        digests.sort_unstable();
        let mut hash = FNV_OFFSET_BASIS;
        for digest in digests {
            hash = fnv1a(&digest.to_ne_bytes(), hash);
        }
        format!("{:016x}", hash)
    }
}
