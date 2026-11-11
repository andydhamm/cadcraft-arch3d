//! Incremental 3D rebuild: kind-tagged object keys, a scene cache that rebuilds only what an edit
//! changed, and an order-independent scene digest.
//!
//! [`Scene3d::build`] is made of *parts*: the meshes of one object in one section of the build
//! (a wall, an opening, a room's floor and ceiling, a roof, …), each made by a pure function of
//! its *inputs* (the object, plus everything else that function reads: the wall's join shape, its
//! hosted openings, the roof above it, the light points under a ceiling, …). A full build makes
//! every part; [`SceneCache::update`] makes again only the parts whose inputs changed and moves
//! the others over from the cached build, so the cached scene is the full build by construction:
//! equal meshes, in the full build's order.
//!
//! Arch ids are numbered per kind (`w3`, `o3`, room 3 and roof 3 are all `ArchId(3)`), so a mesh's
//! owner is an [`ObjectKey`]: its [`ObjectKind`] plus its id.
//!
//! Phase 14: every object's inputs include its paints (the `ArchModel.paints` entries of its
//! surfaces), and every part ends with [`crate::paint::apply`]. Symbols are content-addressed (a
//! key is the hash of the file), so a furnishing's inputs name its symbol by key, plus whether the
//! model has it. A cabinet's inputs include the base cabinets of its level, the furnishings they
//! host and its pull symbol, since counter runs and cut-outs reach across cabinets. The materials,
//! textures and backdrop are filled from the model after every build and update.

use std::collections::{BTreeMap, BTreeSet};

use cadcraft_arch::casework_layout::counter_runs;
use cadcraft_arch::join::{WallShape, solve_joins};
use cadcraft_arch::roof::{Footprint, outer_footprint};
use cadcraft_doc::arch::{
    Backsplash, Cabinet, CabinetKind, CabinetPart, Countertop, ElectricalDevice, Fixture, FoundationKind, Furnishing, FurnishingHost, Level,
    LightFixture, MoldingLine, Opening, OpeningTop, Paint, Plant, PlantSpecies, Railing, Roof, Room, RoomType, SLAB_THICKNESS, Side, Stair, Surface,
    Terrain, WallType,
};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::{Vec2, Vec3};

use crate::mesh::{Bounds3, Mesh};
use crate::probe::part_of;
use crate::walls::{RoofClip, elevation_profile, extrude_profile_slab, extrude_wall_profile, roof_top_polyline_in};
use crate::{
    Scene3d, casework, fill_materials, find_roof_clip, fixture, floors, furnishing, level_for, light, mesh_is_finite, openings, railing,
    raw_wall_mesh, site, stairs, wall_span, walls, well_holes_for,
};

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

/// The architectural object a mesh belongs to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ObjectKind {
    #[default]
    Wall,
    Opening,
    Room,
    Roof,
    Stair,
    Railing,
    Terrain,
    Plant,
    Light,
    Cabinet,
    Fixture,
    /// Phase 14: a placed library item (`f<n>`).
    Furnishing,
    /// Phase 15: an outlet or switch cover plate (`el{n}`).
    Electrical,
    /// Phase 16: an interior soffit (`sf{n}`).
    Soffit,
    /// Phase 16: a foundation's stem walls, footings and slab edges (`fd{n}`).
    Foundation,
    /// Phase 18: a countertop (`ct{n}`).
    Countertop,
    /// Phase 18: a backsplash (`bs{n}`).
    Backsplash,
    /// Phase 18: a molding line (`ml{n}`).
    MoldingLine,
}

impl ObjectKind {
    /// The label of the object of this kind with id `id`.
    pub fn label(self, id: ArchId) -> String {
        use cadcraft_doc::arch as a;
        match self {
            ObjectKind::Wall => a::wall_label(id),
            ObjectKind::Opening => a::opening_label(id),
            ObjectKind::Room => a::room_label(id),
            ObjectKind::Roof => a::roof_label(id),
            ObjectKind::Stair => a::stair_label(id),
            ObjectKind::Railing => a::railing_label(id),
            ObjectKind::Terrain => a::terrain_label(Some(id.0)),
            ObjectKind::Plant => a::plant_label(id),
            ObjectKind::Light => a::light_label(id),
            ObjectKind::Cabinet => a::cabinet_label(id),
            ObjectKind::Fixture => a::fixture_label(id),
            ObjectKind::Furnishing => a::furnishing_label(id),
            ObjectKind::Electrical => a::electrical_label(id),
            ObjectKind::Soffit => a::soffit_label(id),
            ObjectKind::Foundation => a::foundation_label(id),
            ObjectKind::Countertop => a::countertop_label(id),
            ObjectKind::Backsplash => a::backsplash_label(id),
            ObjectKind::MoldingLine => a::molding_line_label(id),
        }
    }

    /// The proxy handle of the object of this kind with id `id`.
    pub fn handle(self, id: ArchId) -> cadcraft_doc::Handle {
        use cadcraft_doc::arch as a;
        match self {
            ObjectKind::Wall | ObjectKind::Terrain => a::arch_handle(id),
            ObjectKind::Opening => a::opening_handle(id),
            ObjectKind::Room => a::room_handle(id),
            ObjectKind::Roof => a::roof_handle(id),
            ObjectKind::Stair => a::stair_handle(id),
            ObjectKind::Railing => a::railing_handle(id),
            ObjectKind::Plant => a::plant_handle(id),
            ObjectKind::Light => a::light_handle(id),
            ObjectKind::Cabinet => a::cabinet_handle(id),
            ObjectKind::Fixture => a::fixture_handle(id),
            ObjectKind::Furnishing => a::furnishing_handle(id),
            ObjectKind::Electrical => a::electrical_handle(id),
            ObjectKind::Soffit => a::soffit_handle(id),
            ObjectKind::Foundation => a::foundation_handle(id),
            ObjectKind::Countertop => a::countertop_handle(id),
            ObjectKind::Backsplash => a::backsplash_handle(id),
            ObjectKind::MoldingLine => a::molding_line_handle(id),
        }
    }

    /// The kind owning a mesh of the [`part_of`] class `part`.
    fn from_part(part: &str) -> ObjectKind {
        match part {
            "floor" | "ceiling" | "deck" | "slab" | "ledger" | "joist" | "rim" | "beam" => ObjectKind::Room,
            "glass" | "frame" | "casing" | "leaf" | "sill" | "header" | "grille" => ObjectKind::Opening,
            "roof" => ObjectKind::Roof,
            "stair" | "tread" | "riser" | "stringer" | "skirt" => ObjectKind::Stair,
            "railing" | "post" => ObjectKind::Railing,
            "terrain" | "path" => ObjectKind::Terrain,
            "plant" => ObjectKind::Plant,
            "light" => ObjectKind::Light,
            "cabinet" | "counter" | "pull" => ObjectKind::Cabinet,
            "fixture" => ObjectKind::Fixture,
            "furnishing" => ObjectKind::Furnishing,
            "electrical" => ObjectKind::Electrical,
            "soffit" => ObjectKind::Soffit,
            "foundation" | "footing" => ObjectKind::Foundation,
            "backsplash" => ObjectKind::Backsplash,
            _ => ObjectKind::Wall,
        }
    }
}

/// A mesh's owner: its kind and its id (ids number per kind).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectKey {
    pub kind: ObjectKind,
    pub id: ArchId,
}

/// The kind of object `mesh` belongs to: material-aware, so a painted countertop or backsplash is
/// its own object's, deck structure and room moldings belong to their room, and a molding line to
/// its line; else the kind of its [`part_of`] class.
pub fn object_kind_of(mesh: &Mesh) -> ObjectKind {
    let m = mesh.material.to_lowercase();
    // A painted surface is named by its key first (the part `counter` would read as a cabinet's).
    if let Some(s) = m.strip_prefix("paint:").and_then(Surface::parse) {
        match s {
            Surface::Countertop(_) => return ObjectKind::Countertop,
            Surface::Backsplash(_) => return ObjectKind::Backsplash,
            _ => {}
        }
    }
    // Phase 17: deck structure (posts and piers too) belongs to its deck room, while phase 16's
    // "footing" part stays a foundation's.
    if m.starts_with("deck-") || m.starts_with("molding-room-") {
        ObjectKind::Room
    } else if m.starts_with("glass-wall") || m.starts_with("niche-back") {
        ObjectKind::Wall
    } else if m.starts_with("molding-line") {
        ObjectKind::MoldingLine
    } else if m == "countertop" || m.starts_with("countertop-") {
        ObjectKind::Countertop
    } else if m == "backsplash" {
        ObjectKind::Backsplash
    } else {
        ObjectKind::from_part(part_of(mesh))
    }
}

/// The object `mesh` belongs to, from its [`object_kind_of`] class and its `object` id.
pub fn object_key(mesh: &Mesh) -> ObjectKey {
    ObjectKey { kind: object_kind_of(mesh), id: mesh.object }
}

/// The objects whose data differ between `prev` and `next` (added, removed or changed, compared by
/// id per kind; a changed paint counts for the object it paints, an added or removed symbol for the
/// furnishings and cabinets that use it, a changed tray for its room before and after the edit);
/// `None` when something not owned by one object differs (levels, wall types, the site location,
/// the plant species, a paint key that names no surface, a symbol whose content changed under the
/// same key), and the caller rebuilds everything.
///
/// Images, the backdrop and the defaults belong to no object: they change only the materials,
/// textures and backdrop, which every update fills again.
pub fn changed_objects(prev: &ArchModel, next: &ArchModel) -> Option<BTreeSet<ObjectKey>> {
    if prev.levels != next.levels || prev.types != next.types || prev.site != next.site || prev.plant_species != next.plant_species {
        return None;
    }
    // A key is the hash of its symbol's file, so new content under an old key is a corrupt model.
    if next.symbols.iter().any(|(key, symbol)| prev.symbols.get(key).is_some_and(|old| old != symbol)) {
        return None;
    }
    let mut changes = BTreeSet::new();
    diff(&prev.walls, &next.walls, |w| w.id, ObjectKind::Wall, &mut changes);
    diff(&prev.openings, &next.openings, |o| o.id, ObjectKind::Opening, &mut changes);
    diff(&prev.rooms, &next.rooms, |r| r.id, ObjectKind::Room, &mut changes);
    diff(&prev.roofs, &next.roofs, |r| r.id, ObjectKind::Roof, &mut changes);
    diff(&prev.stairs, &next.stairs, |s| s.id, ObjectKind::Stair, &mut changes);
    diff(&prev.railings, &next.railings, |r| r.id, ObjectKind::Railing, &mut changes);
    diff(&prev.plants, &next.plants, |p| p.id, ObjectKind::Plant, &mut changes);
    diff(&prev.lights, &next.lights, |l| l.id, ObjectKind::Light, &mut changes);
    diff(&prev.cabinets, &next.cabinets, |c| c.id, ObjectKind::Cabinet, &mut changes);
    diff(&prev.fixtures, &next.fixtures, |f| f.id, ObjectKind::Fixture, &mut changes);
    diff(&prev.furnishings, &next.furnishings, |f| f.id, ObjectKind::Furnishing, &mut changes);
    diff(&prev.electrical, &next.electrical, |d| d.id, ObjectKind::Electrical, &mut changes);
    diff(&prev.soffits, &next.soffits, |s| s.id, ObjectKind::Soffit, &mut changes);
    diff(&prev.foundations, &next.foundations, |f| f.id, ObjectKind::Foundation, &mut changes);
    diff(&prev.countertops, &next.countertops, |c| c.id, ObjectKind::Countertop, &mut changes);
    diff(&prev.backsplashes, &next.backsplashes, |b| b.id, ObjectKind::Backsplash, &mut changes);
    diff(&prev.molding_lines, &next.molding_lines, |l| l.id, ObjectKind::MoldingLine, &mut changes);
    // A tray is part of its room's ceiling.
    for id in changed_ids(&prev.trays, &next.trays, |t| t.id) {
        for t in prev.trays.iter().chain(&next.trays).filter(|t| t.id == id) {
            changes.insert(ObjectKey { kind: ObjectKind::Room, id: t.room });
        }
    }
    if prev.terrain != next.terrain {
        changes.insert(ObjectKey { kind: ObjectKind::Terrain, id: ArchId(0) });
    }
    for key in prev.paints.keys().chain(next.paints.keys()) {
        if prev.paints.get(key) != next.paints.get(key) {
            changes.insert(surface_owner(&Surface::parse(key)?));
        }
    }
    for key in prev.symbols.keys().chain(next.symbols.keys()) {
        if prev.symbols.contains_key(key) != next.symbols.contains_key(key) {
            for f in prev.furnishings.iter().chain(&next.furnishings).filter(|f| f.symbol == *key) {
                changes.insert(ObjectKey { kind: ObjectKind::Furnishing, id: f.id });
            }
            for c in prev.cabinets.iter().chain(&next.cabinets).filter(|c| c.pull_symbol.as_deref() == Some(key.as_str())) {
                changes.insert(ObjectKey { kind: ObjectKind::Cabinet, id: c.id });
            }
        }
    }
    Some(changes)
}

/// The object a surface belongs to.
fn surface_owner(surface: &Surface) -> ObjectKey {
    let (kind, id) = match surface {
        Surface::WallSide { wall, .. } => (ObjectKind::Wall, *wall),
        Surface::RoomFloor(r) | Surface::RoomCeiling(r) => (ObjectKind::Room, *r),
        Surface::Roof(r) => (ObjectKind::Roof, *r),
        Surface::Cabinet { cabinet, .. } => (ObjectKind::Cabinet, *cabinet),
        Surface::Fixture(f) => (ObjectKind::Fixture, *f),
        Surface::Furnishing { furnishing, .. } => (ObjectKind::Furnishing, *furnishing),
        Surface::Countertop(c) => (ObjectKind::Countertop, *c),
        Surface::Backsplash(b) => (ObjectKind::Backsplash, *b),
    };
    ObjectKey { kind, id }
}

/// The paints of the object whose [`Surface::object_key`] is `object`: every entry of
/// `model.paints` whose key starts with `object` + "/".
fn paints_under(model: &ArchModel, object: &str) -> Vec<(String, Paint)> {
    let prefix = format!("{object}/");
    model.paints.range(prefix.clone()..).take_while(|(k, _)| k.starts_with(&prefix)).map(|(k, p)| (k.clone(), p.clone())).collect()
}

/// The paint of the object whose whole surface is `object` (roofs and casework fixtures).
fn paint_of(model: &ArchModel, object: &str) -> Vec<(String, Paint)> {
    model.paints.get(object).map(|p| vec![(object.to_string(), p.clone())]).unwrap_or_default()
}

/// `meshes` after [`crate::paint::apply`]: how every part builder ends.
fn painted(model: &ArchModel, mut meshes: Vec<Mesh>) -> Vec<Mesh> {
    crate::paint::apply(model, &mut meshes);
    meshes
}

/// Add to `out` every id of `kind` whose objects differ between `prev` and `next` (all objects
/// with that id, in order, so duplicate ids compare too).
fn diff<T: PartialEq>(prev: &[T], next: &[T], id: impl Fn(&T) -> ArchId, kind: ObjectKind, out: &mut BTreeSet<ObjectKey>) {
    out.extend(changed_ids(prev, next, id).into_iter().map(|id| ObjectKey { kind, id }));
}

/// The ids whose objects differ between `prev` and `next` (all objects with that id, in order).
fn changed_ids<T: PartialEq>(prev: &[T], next: &[T], id: impl Fn(&T) -> ArchId) -> BTreeSet<ArchId> {
    fn group<'a, T>(list: &'a [T], id: &impl Fn(&T) -> ArchId) -> BTreeMap<ArchId, Vec<&'a T>> {
        let mut by: BTreeMap<ArchId, Vec<&'a T>> = BTreeMap::new();
        for x in list {
            by.entry(id(x)).or_default().push(x);
        }
        by
    }
    let (a, b) = (group(prev, &id), group(next, &id));
    a.keys().chain(b.keys()).filter(|k| a.get(k) != b.get(k)).copied().collect()
}

/// What a rebuild did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RebuildStats {
    /// Distinct objects whose meshes were made again (all of them on a full build).
    pub rebuilt: usize,
    /// Distinct objects ([`ObjectKey`]s) among the scene's meshes.
    pub objects: usize,
    /// False for a full build.
    pub incremental: bool,
}

/// A model, its built scene, and the parts the scene was assembled from.
#[derive(Clone, Debug)]
pub struct SceneCache {
    model: ArchModel,
    sections: Sections,
    scene: Scene3d,
}

impl SceneCache {
    /// A full build of `model`.
    pub fn new(model: &ArchModel) -> (SceneCache, RebuildStats) {
        let sections = Sections::make(model, None);
        let mut scene = sections.scene(model);
        fill_materials(model, &mut scene);
        let objects = object_keys(&scene.meshes).len();
        (SceneCache { model: model.clone(), sections, scene }, RebuildStats { rebuilt: objects, objects, incremental: false })
    }

    /// Bring the scene up to `model`: a full build when something global changed (see
    /// [`changed_objects`]), else only the parts whose inputs changed are made again. Either way the
    /// materials, textures and backdrop are filled again from `model`.
    pub fn update(&mut self, model: &ArchModel) -> RebuildStats {
        let Some(changed) = changed_objects(&self.model, model) else {
            let (cache, stats) = SceneCache::new(model);
            *self = cache;
            return stats;
        };
        if !changed.is_empty() {
            self.sections = Sections::make(model, Some(std::mem::take(&mut self.sections)));
            self.scene = self.sections.scene(model);
        }
        fill_materials(model, &mut self.scene);
        self.model = model.clone();
        let present = object_keys(&self.scene.meshes);
        let fresh = self.sections.fresh_keys();
        let rebuilt = if changed.is_empty() { 0 } else { fresh.intersection(&present).count() };
        if changed == BTreeSet::from([ObjectKey { kind: ObjectKind::Foundation, id: ArchId(0) }]) {
            eprintln!("changed={changed:?} fresh={fresh:?} present={present:?} rebuilt={rebuilt}");
        }
        RebuildStats { rebuilt, objects: present.len(), incremental: true }
    }

    /// The cached scene: equal to `Scene3d::build` of the last model given.
    pub fn scene(&self) -> &Scene3d {
        &self.scene
    }
}

fn object_keys(meshes: &[Mesh]) -> BTreeSet<ObjectKey> {
    meshes.iter().map(object_key).collect()
}

/// One object's meshes in one section, the inputs they were made from, and whether they were made
/// in the last build (else moved over from the one before).
#[derive(Clone, Debug)]
struct Part<I> {
    id: ArchId,
    input: I,
    meshes: Vec<Mesh>,
    fresh: bool,
}

/// Parts for `items` (id, inputs), in their order: a part of `prev` with the same id (the n-th
/// part with that id for its n-th item) and equal inputs is moved over, any other is made.
fn parts<I: PartialEq>(prev: Vec<Part<I>>, items: Vec<(ArchId, I)>, make: impl Fn(&I) -> Vec<Mesh>) -> Vec<Part<I>> {
    let mut old: BTreeMap<(ArchId, usize), Part<I>> = BTreeMap::new();
    let mut seen: BTreeMap<ArchId, usize> = BTreeMap::new();
    for p in prev {
        let n = seen.entry(p.id).or_insert(0);
        old.insert((p.id, *n), p);
        *n += 1;
    }
    seen.clear();
    items
        .into_iter()
        .map(|(id, input)| {
            let n = seen.entry(id).or_insert(0);
            let key = (id, *n);
            *n += 1;
            match old.remove(&key) {
                Some(p) if p.input == input => Part { id, input, meshes: p.meshes, fresh: false },
                _ => {
                    let meshes = make(&input);
                    Part { id, input, meshes, fresh: true }
                }
            }
        })
        .collect()
}

/// Everything one wall's meshes are made from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WallIn {
    pub(crate) wall: Wall,
    /// `None` skips the wall (and leaves the rest empty).
    pub(crate) span: Option<(f64, f64)>,
    pub(crate) shape: Option<WallShape>,
    pub(crate) openings: Vec<Opening>,
    pub(crate) clip: Option<RoofClip>,
    pub(crate) roof_top: Option<Vec<Vec2>>,
    /// Phase 16: the part of the wall rising to a vaulted ceiling over its room, if any.
    pub(crate) rise: Option<crate::wall_rise::RiseIn>,
    pub(crate) paints: Vec<(String, Paint)>,
    /// Phase 17: where a stair's flight cuts the wall's top (empty for most walls).
    pub(crate) flight_cuts: Vec<cadcraft_arch::flight_cut::FlightCut>,
}

#[derive(Clone, Debug, PartialEq)]
struct OpeningIn {
    opening: Opening,
    host: Option<Wall>,
    span: Option<(f64, f64)>,
    /// Phase 17: the host wall's roof top polyline, for an opening whose top follows the roof.
    roof_top: Option<Vec<Vec2>>,
}

#[derive(Clone, Debug, PartialEq)]
struct RoomIn {
    room: Room,
    level: Level,
    /// The room's holes plus the stair wells opening into its level (the floor's holes).
    wells: Vec<Vec<Vec2>>,
    /// Phase 16: everything its ceiling is made from (the recessed fixtures' openings, the roofs
    /// on its level, its trays).
    ceiling: crate::ceiling::CeilingIn,
    /// Phase 16: whether its level stands on a slab foundation (its floor is the slab).
    slab: bool,
    paints: Vec<(String, Paint)>,
}

#[derive(Clone, Debug, PartialEq)]
struct RoofIn {
    roof: Roof,
    eave_z: f64,
    paints: Vec<(String, Paint)>,
}

#[derive(Clone, Debug, PartialEq)]
struct StairIn {
    stair: Stair,
    base: f64,
    /// Phase 17: which sides ([left, right]) stand against a wall.
    against: [bool; 2],
}

#[derive(Clone, Debug, PartialEq)]
struct RailingIn {
    railing: Railing,
    /// Phase 17: what the railing stands on (a floor, a deck, a stair flight).
    surface: cadcraft_arch::railing::RailSurface,
}

#[derive(Clone, Debug, PartialEq)]
struct DeckIn {
    room: Room,
    level: Level,
    /// Phase 17: the deck's substructure, if it has one.
    frame: Option<cadcraft_arch::deck::DeckFrame>,
    /// A deck is a room: its floor takes the room's paints.
    paints: Vec<(String, Paint)>,
}

/// Everything [`site::site_meshes`] reads.
#[derive(Clone, Debug, PartialEq)]
struct SiteIn {
    terrain: Option<Terrain>,
    plants: Vec<Plant>,
    species: Vec<PlantSpecies>,
    breaklines: Vec<[Vec2; 2]>,
    pads: Vec<cadcraft_arch::terrain::pad::PadRegion>,
}

#[derive(Clone, Debug, PartialEq)]
struct FixtureIn {
    light: LightFixture,
    point: Option<Vec3>,
}

/// The fields of a cabinet that another cabinet's meshes read: counter runs and their corner
/// merges, corner partners, fillers joining runs, a crown's exposed ends. A projection, not a
/// clone, so an edit of any other field (face, style, shelves, ...) rebuilds that cabinet alone.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CabinetNeighbour {
    pub(crate) id: ArchId,
    pub(crate) kind: CabinetKind,
    pub(crate) level: u32,
    pub(crate) position: Vec2,
    pub(crate) rotation: f64,
    pub(crate) width: f64,
    pub(crate) depth: f64,
    pub(crate) height: f64,
    pub(crate) elevation: f64,
    pub(crate) counter: bool,
    pub(crate) counter_thickness: f64,
    pub(crate) counter_overhang: f64,
    pub(crate) filler: bool,
    pub(crate) corner: cadcraft_doc::arch::CornerPref,
    /// Whether it has a crown molding.
    pub(crate) crown: bool,
    /// Whether it has a light rail.
    pub(crate) light_rail: bool,
}

impl CabinetNeighbour {
    fn of(c: &Cabinet) -> CabinetNeighbour {
        CabinetNeighbour {
            id: c.id,
            kind: c.kind,
            level: c.level,
            position: c.position,
            rotation: c.rotation,
            width: c.width,
            depth: c.depth,
            height: c.height,
            elevation: c.elevation,
            counter: c.counter,
            counter_thickness: c.counter_thickness,
            counter_overhang: c.counter_overhang,
            filler: c.filler,
            corner: c.corner,
            crown: c.crown.is_some(),
            light_rail: c.light_rail.is_some(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CabinetIn {
    cabinet: Cabinet,
    level: Level,
    paints: Vec<(String, Paint)>,
    /// Projections of the other cabinets of its kind group on its level (base cabinets for a base
    /// cabinet, wall and tall cabinets for a wall or tall one): counter runs, corners, fillers and
    /// crown ends reach across them.
    neighbours: Vec<CabinetNeighbour>,
    /// The furnishings hosted by a cabinet on that level or by a gap beside one (cut-outs, bridged
    /// gaps).
    hosted: Vec<Furnishing>,
    /// The cabinets on its level that custom countertops claim (sorted).
    claimed: Vec<ArchId>,
    /// The pull symbol's key, when the model has that symbol.
    pull_symbol: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct CaseworkFixtureIn {
    fixture: Fixture,
    level: Level,
    paints: Vec<(String, Paint)>,
}

#[derive(Clone, Debug, PartialEq)]
struct FurnishingIn {
    furnishing: Furnishing,
    level: Level,
    paints: Vec<(String, Paint)>,
    /// Whether the model has the symbol (`furnishing.symbol` names its content): a missing one
    /// builds a box.
    has_symbol: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct ElectricalIn {
    device: ElectricalDevice,
    /// Where the device sits on its wall (`None` when the placement fails).
    place: Option<cadcraft_arch::electrical::DevicePlace>,
    /// The elevation of the device's level.
    level: f64,
}

/// Phase 18: one room's resolved molding runs.
#[derive(Clone, Debug, PartialEq)]
struct RoomMoldingIn {
    room: ArchId,
    /// The elevation of the room's level.
    level: f64,
    runs: Vec<cadcraft_arch::molding_runs::MoldingRun>,
}

/// Phase 18: one molding line.
#[derive(Clone, Debug, PartialEq)]
struct MoldingLineIn {
    line: MoldingLine,
    level: f64,
}

/// Phase 18: one countertop with its custom outline.
#[derive(Clone, Debug, PartialEq)]
struct CountertopIn {
    outline: Option<cadcraft_arch::countertop::CounterOutline>,
    countertop: Countertop,
    level: f64,
    paints: Vec<(String, Paint)>,
}

/// Phase 18: one backsplash with its resolved segments.
#[derive(Clone, Debug, PartialEq)]
struct BacksplashIn {
    backsplash: Backsplash,
    segments: Vec<cadcraft_arch::backsplash::BacksplashSegment>,
    level: f64,
    paints: Vec<(String, Paint)>,
}

/// The parts of a scene, section by section in the order [`Scene3d::build`] emits them.
#[derive(Clone, Debug, Default)]
pub(crate) struct Sections {
    walls: Vec<Part<WallIn>>,
    openings: Vec<Part<OpeningIn>>,
    rooms: Vec<Part<RoomIn>>,
    roofs: Vec<Part<RoofIn>>,
    stairs: Vec<Part<StairIn>>,
    railings: Vec<Part<RailingIn>>,
    decks: Vec<Part<DeckIn>>,
    site: Vec<Part<SiteIn>>,
    fixtures: Vec<Part<FixtureIn>>,
    cabinets: Vec<Part<CabinetIn>>,
    casework_fixtures: Vec<Part<CaseworkFixtureIn>>,
    furnishings: Vec<Part<FurnishingIn>>,
    electrical: Vec<Part<ElectricalIn>>,
    soffits: Vec<Part<crate::soffit::SoffitIn>>,
    foundations: Vec<Part<crate::foundation::FoundationIn>>,
    room_moldings: Vec<Part<RoomMoldingIn>>,
    molding_lines: Vec<Part<MoldingLineIn>>,
    countertops: Vec<Part<CountertopIn>>,
    backsplashes: Vec<Part<BacksplashIn>>,
    /// The join shapes, for the walls and wall types they were solved for.
    joins: Option<(Vec<Wall>, Vec<WallType>, Vec<WallShape>)>,
    /// Per level: its walls, and the outer footprint they make (it joins every wall on the level).
    footprints: BTreeMap<u32, (Vec<Wall>, Option<Footprint>)>,
}

impl Sections {
    /// The parts of `model`'s scene, moving over every part of `prev` whose inputs are unchanged.
    pub(crate) fn make(model: &ArchModel, prev: Option<Sections>) -> Sections {
        let Sections {
            walls: pw,
            openings: po,
            rooms: pr,
            roofs: pf,
            stairs: ps,
            railings: pl,
            decks: pd,
            site: pt,
            fixtures: px,
            cabinets: pc,
            casework_fixtures: pfx,
            furnishings: pfu,
            electrical: pel,
            soffits: psf,
            foundations: pfd,
            room_moldings: prm,
            molding_lines: pml,
            countertops: pct,
            backsplashes: pbs,
            joins,
            footprints: mut old_footprints,
        } = prev.unwrap_or_default();

        // The joins and the footprints read only walls and wall types: reuse them while those are unchanged.
        let same_types = joins.as_ref().is_some_and(|(_, types, _)| *types == model.types);
        if !same_types {
            old_footprints.clear();
        }
        let shapes = match joins {
            Some((walls, _, shapes)) if same_types && walls == model.walls => shapes,
            _ => solve_joins(model),
        };
        let shape_by_id: BTreeMap<ArchId, usize> = shapes.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
        let wall_map: BTreeMap<ArchId, &Wall> = model.walls.iter().map(|w| (w.id, w)).collect();
        let mut hosted: BTreeMap<ArchId, Vec<Opening>> = BTreeMap::new();
        for o in &model.openings {
            hosted.entry(o.host).or_default().push(o.clone());
        }
        let mut footprints: BTreeMap<u32, (Vec<Wall>, Option<Footprint>)> = BTreeMap::new();

        // Each wall's roof top, for the openings whose top follows the roof.
        let mut roof_tops: BTreeMap<ArchId, Option<Vec<Vec2>>> = BTreeMap::new();
        let wall_items = model
            .walls
            .iter()
            .map(|wall| {
                let span = wall_span(model, wall);
                let paints = paints_under(model, &Surface::WallSide { wall: wall.id, side: Side::Left }.object_key());
                let mut input = WallIn {
                    wall: wall.clone(),
                    span,
                    shape: None,
                    openings: Vec::new(),
                    clip: None,
                    roof_top: None,
                    rise: None,
                    paints,
                    flight_cuts: Vec::new(),
                };
                if let Some((base, top)) = span {
                    input.flight_cuts = cadcraft_arch::flight_cut::flight_cuts(model, wall);
                    input.shape = shape_by_id.get(&wall.id).and_then(|&i| shapes.get(i)).cloned();
                    input.openings = hosted.get(&wall.id).cloned().unwrap_or_default();
                    input.clip = find_roof_clip(model, wall);
                    if !model.roofs.is_empty() {
                        let level = wall.base_level;
                        let (_, fp) = footprints.entry(level).or_insert_with(|| {
                            let on_level: Vec<Wall> = model.walls.iter().filter(|w| w.base_level == level).cloned().collect();
                            match old_footprints.remove(&level) {
                                Some((walls, fp)) if walls == on_level => (walls, fp),
                                _ => (on_level, outer_footprint(model, level)),
                            }
                        });
                        input.roof_top = roof_top_polyline_in(model, fp.as_ref(), wall, base, top);
                    }
                }
                input.rise = crate::wall_rise::rise_input(model, wall, input.shape.as_ref(), span, input.roof_top.is_some());
                roof_tops.insert(wall.id, input.roof_top.clone());
                (wall.id, input)
            })
            .collect();
        let walls = parts(pw, wall_items, |i| {
            let mut out = wall_mesh(i);
            if let Some(rise) = &i.rise {
                out.extend(crate::wall_rise::rise_meshes(&i.wall, rise));
            }
            painted(model, out)
        });

        let opening_items = model
            .openings
            .iter()
            .map(|o| {
                let host = wall_map.get(&o.host).copied();
                let roof_top = if o.top == OpeningTop::Roof { roof_tops.get(&o.host).cloned().flatten() } else { None };
                let input = OpeningIn { opening: o.clone(), host: host.cloned(), span: host.and_then(|w| wall_span(model, w)), roof_top };
                (o.id, input)
            })
            .collect();
        let openings = parts(po, opening_items, |i| {
            let map: BTreeMap<ArchId, &Wall> = i.host.iter().map(|w| (i.opening.host, w)).collect();
            let made = openings::opening_meshes_with(&map, &i.opening, i.span, i.roof_top.as_deref());
            painted(model, made.into_iter().filter(|m| !m.positions.is_empty()).collect())
        });

        let room_items: Vec<(ArchId, RoomIn)> = model
            .rooms
            .iter()
            .map(|room| {
                let level = level_for(model, room.level);
                let wells = well_holes_for(model, room);
                let cuts = fixture::ceiling_cutouts(model, room, &level);
                let ceiling = crate::ceiling::ceiling_input(model, room, &level, cuts);
                let slab = model.foundations.iter().any(|f| f.kind == FoundationKind::Slab && f.level == room.level);
                let paints = paints_under(model, &Surface::RoomFloor(room.id).object_key());
                (room.id, RoomIn { room: Room { moldings: Vec::new(), ..room.clone() }, level, wells, ceiling, slab, paints })
            })
            .collect();
        for p in &pr {
            if let Some(i) = room_items.iter().find(|(id, _)| *id == p.id) {
                if p.input != i.1 {
                    eprintln!("room input changed for {}: prev={:?} next={:?}", p.id, p.input, i.1);
                }
            }
        }
        let rooms = parts(pr, room_items, |i| {
            let mut out = Vec::new();
            if i.room.room_type != RoomType::Deck {
                let mut floor = i.room.clone();
                floor.holes = i.wells.clone();
                // On a slab foundation the floor structure is the slab.
                let slab = if i.slab {
                    floors::floor_slab(&floor, &Level { structure: SLAB_THICKNESS, ..i.level.clone() }, "slab")
                } else {
                    floors::floor_slab(&floor, &i.level, "floor")
                };
                out.extend(slab.map(|m| crate::finish::finish_floor(m, i.room.floor_finish)));
            }
            out.extend(crate::ceiling::ceiling_meshes(&i.ceiling));
            painted(model, out)
        });

        let roof_items = model
            .roofs
            .iter()
            .map(|roof| {
                let paints = paint_of(model, &Surface::Roof(roof.id).object_key());
                (roof.id, RoofIn { roof: roof.clone(), eave_z: eave_z(model, roof.level), paints })
            })
            .collect();
        let roofs = parts(pf, roof_items, |i| {
            painted(model, crate::roof::roof_meshes(&i.roof, i.eave_z).into_iter().filter(|m| !m.positions.is_empty()).collect())
        });

        let stair_items = model
            .stairs
            .iter()
            .map(|s| {
                let against = cadcraft_arch::stair_geom::sides_against_walls(model, s);
                (s.id, StairIn { stair: s.clone(), base: level_for(model, s.from_level).elevation, against })
            })
            .collect();
        let stairs = parts(ps, stair_items, |i| painted(model, crate::stair_parts::stair_meshes(&i.stair, i.base, i.against)));

        let railing_items = model
            .railings
            .iter()
            .map(|r| (r.id, RailingIn { railing: r.clone(), surface: cadcraft_arch::railing::rail_surface(model, r) }))
            .collect();
        let railings = parts(pl, railing_items, |i| painted(model, crate::railing_parts::railing_meshes(&i.railing, &i.surface)));

        // The site grade under the decks, worked out once and only when a deck exists.
        let grade = std::cell::OnceCell::new();
        let deck_items = model
            .rooms
            .iter()
            .filter(|r| r.room_type == RoomType::Deck)
            .map(|r| {
                let paints = paints_under(model, &Surface::RoomFloor(r.id).object_key());
                let grade = grade.get_or_init(|| cadcraft_arch::deck::site_grade(model));
                let frame = cadcraft_arch::deck::frame(model, r, grade);
                (r.id, DeckIn { room: r.clone(), level: level_for(model, r.level), frame, paints })
            })
            .collect();
        let decks = parts(pd, deck_items, |i| painted(model, crate::deck_frame::deck_meshes(&i.room, &i.level, i.frame.as_ref())));

        let site_in = SiteIn {
            terrain: model.terrain.clone(),
            plants: model.plants.clone(),
            species: model.plant_species.clone(),
            breaklines: model.walls.iter().filter(|w| w.retaining).map(|w| [w.start, w.end]).collect(),
            pads: if model.terrain.is_some() { cadcraft_arch::terrain::pad::pad_regions(model) } else { Vec::new() },
        };
        let site = parts(pt, vec![(ArchId(0), site_in)], |_| painted(model, site::site_meshes(model).meshes));

        let fixture_items =
            model.lights.iter().map(|f| (f.id, FixtureIn { light: f.clone(), point: cadcraft_arch::light::light_point(model, f) })).collect();
        let fixtures = parts(px, fixture_items, |i| painted(model, fixture::fixture_housing(&i.light, i.point)));

        let casework = casework_levels(model);
        let cabinet_items = model
            .cabinets
            .iter()
            .map(|c| {
                let (base, upper, hosted, claimed) = casework.get(&c.level).cloned().unwrap_or_default();
                let neighbours = if c.kind == CabinetKind::Base { base } else { upper };
                let input = CabinetIn {
                    claimed,
                    cabinet: c.clone(),
                    level: level_for(model, c.level),
                    paints: paints_under(model, &Surface::Cabinet { cabinet: c.id, part: CabinetPart::Carcass }.object_key()),
                    neighbours,
                    hosted,
                    pull_symbol: c.pull_symbol.clone().filter(|k| model.symbols.contains_key(k)),
                };
                (c.id, input)
            })
            .collect();
        // The counter runs, worked out once (and only when a cabinet is rebuilt) for every cabinet.
        let runs = std::cell::OnceCell::new();
        let cabinets =
            parts(pc, cabinet_items, |i| painted(model, casework::cabinet_meshes_in(model, &i.cabinet, runs.get_or_init(|| counter_runs(model)))));

        let casework_fixture_items = model
            .fixtures
            .iter()
            .map(|f| {
                let paints = paint_of(model, &Surface::Fixture(f.id).object_key());
                (f.id, CaseworkFixtureIn { fixture: f.clone(), level: level_for(model, f.level), paints })
            })
            .collect();
        let casework_fixtures = parts(pfx, casework_fixture_items, |i| painted(model, vec![casework::fixture_mesh(model, &i.fixture)]));

        let furnishing_items = model
            .furnishings
            .iter()
            .map(|f| {
                let input = FurnishingIn {
                    furnishing: f.clone(),
                    level: level_for(model, f.level),
                    paints: paints_under(model, &Surface::Furnishing { furnishing: f.id, part: 0 }.object_key()),
                    has_symbol: model.symbols.contains_key(&f.symbol),
                };
                (f.id, input)
            })
            .collect();
        let furnishings = parts(pfu, furnishing_items, |i| painted(model, furnishing::furnishing_meshes(model, &i.furnishing)));

        let electrical_items = model
            .electrical
            .iter()
            .map(|d| {
                let input = ElectricalIn {
                    device: d.clone(),
                    place: cadcraft_arch::electrical::place_device(model, d).ok(),
                    level: level_for(model, d.level).elevation,
                };
                (d.id, input)
            })
            .collect();
        let electrical = parts(pel, electrical_items, |i| crate::electrical::plate_meshes(&i.device, i.place.as_ref()));

        let soffit_items = model.soffits.iter().map(|s| (s.id, crate::soffit::soffit_input(model, s))).collect();
        let soffits = parts(psf, soffit_items, |i| painted(model, crate::soffit::soffit_meshes(i)));

        let foundation_items = model.foundations.iter().map(|f| (f.id, crate::foundation::foundation_input(model, f))).collect();
        let foundations = parts(pfd, foundation_items, |i| painted(model, crate::foundation::foundation_meshes(i)));

        let room_molding_items = model
            .rooms
            .iter()
            .filter(|r| !r.moldings.is_empty())
            .map(|r| {
                let input =
                    RoomMoldingIn { room: r.id, level: level_for(model, r.level).elevation, runs: cadcraft_arch::molding_runs::room_runs(model, r) };
                (r.id, input)
            })
            .collect();
        let room_moldings = parts(prm, room_molding_items, |i| crate::molding::room::room_molding_meshes(i.room, i.level, &i.runs));

        let molding_line_items =
            model.molding_lines.iter().map(|l| (l.id, MoldingLineIn { line: l.clone(), level: level_for(model, l.level).elevation })).collect();
        let molding_lines = parts(pml, molding_line_items, |i| crate::molding::line::line_meshes(&i.line, i.level));

        let countertop_items = model
            .countertops
            .iter()
            .map(|ct| {
                let input = CountertopIn {
                    outline: cadcraft_arch::countertop::custom_outline(model, ct),
                    countertop: ct.clone(),
                    level: level_for(model, ct.level).elevation,
                    paints: paint_of(model, &Surface::Countertop(ct.id).object_key()),
                };
                (ct.id, input)
            })
            .collect();
        let countertops =
            parts(pct, countertop_items, |i| painted(model, crate::countertop::countertop_meshes(&i.countertop, i.outline.as_ref(), i.level)));

        let backsplash_items = model
            .backsplashes
            .iter()
            .map(|b| {
                let input = BacksplashIn {
                    backsplash: b.clone(),
                    segments: cadcraft_arch::backsplash::segments(model, b),
                    level: level_for(model, b.level).elevation,
                    paints: paint_of(model, &Surface::Backsplash(b.id).object_key()),
                };
                (b.id, input)
            })
            .collect();
        let backsplashes =
            parts(pbs, backsplash_items, |i| painted(model, crate::backsplash::backsplash_meshes(&i.backsplash, &i.segments, i.level)));

        let joins = Some((model.walls.clone(), model.types.clone(), shapes));
        Sections {
            walls,
            openings,
            rooms,
            roofs,
            stairs,
            railings,
            decks,
            site,
            fixtures,
            cabinets,
            casework_fixtures,
            furnishings,
            electrical,
            soffits,
            foundations,
            room_moldings,
            molding_lines,
            countertops,
            backsplashes,
            joins,
            footprints,
        }
    }

    /// The scene these parts make (a copy; the cache keeps its parts).
    pub(crate) fn scene(&self, model: &ArchModel) -> Scene3d {
        let meshes = self.all_parts().flatten().filter(|m| keep(m)).cloned().collect();
        with_meshes(self.counts(model), meshes)
    }

    /// The scene these parts make, moving their meshes into it.
    pub(crate) fn into_scene(self, model: &ArchModel) -> Scene3d {
        let counts = self.counts(model);
        let Sections {
            walls,
            openings,
            rooms,
            roofs,
            stairs,
            railings,
            decks,
            site,
            fixtures,
            cabinets,
            casework_fixtures,
            furnishings,
            electrical,
            soffits,
            foundations,
            room_moldings,
            molding_lines,
            countertops,
            backsplashes,
            ..
        } = self;
        fn meshes<I>(list: Vec<Part<I>>) -> impl Iterator<Item = Mesh> {
            list.into_iter().flat_map(|p| p.meshes)
        }
        let all = meshes(walls)
            .chain(meshes(openings))
            .chain(meshes(rooms))
            .chain(meshes(roofs))
            .chain(meshes(stairs))
            .chain(meshes(railings))
            .chain(meshes(decks))
            .chain(meshes(site))
            .chain(meshes(fixtures))
            .chain(meshes(cabinets))
            .chain(meshes(casework_fixtures))
            .chain(meshes(furnishings))
            .chain(meshes(electrical))
            .chain(meshes(soffits))
            .chain(meshes(foundations))
            .chain(meshes(room_moldings))
            .chain(meshes(molding_lines))
            .chain(meshes(countertops))
            .chain(meshes(backsplashes));
        with_meshes(counts, all.filter(keep).collect())
    }

    /// A scene without meshes, counted the way [`Scene3d::build`] counts: an object counts when its
    /// part made meshes, before non-finite ones are dropped.
    fn counts(&self, model: &ArchModel) -> Scene3d {
        let site_meshes = self.site.iter().flat_map(|p| &p.meshes);
        let fixture_ids: BTreeSet<ArchId> = self.fixtures.iter().flat_map(|p| &p.meshes).map(|m| m.object).collect();
        let room_meshes = || self.rooms.iter().flat_map(|p| &p.meshes);
        let decks = made(&self.decks);
        let scene = Scene3d {
            openings_built: made(&self.openings),
            floors: room_meshes().filter(|m| m.material.starts_with("floor") || m.material == "slab").count() + decks,
            // One per room whose part has a ceiling mesh: stack layers and tray sides do not add any.
            ceilings: self.rooms.iter().filter(|p| p.meshes.iter().any(|m| part_of(m) == "ceiling")).count(),
            roofs: made(&self.roofs),
            stairs: made(&self.stairs),
            railings: made(&self.railings),
            decks,
            terrain: site_meshes.clone().filter(|m| m.material == "terrain").count(),
            paths: site_meshes.clone().filter(|m| m.material.starts_with("path-")).count(),
            plants: site_meshes.filter(|m| m.material.starts_with("plant-")).map(|m| m.object).collect::<BTreeSet<_>>().len(),
            lights: light::scene_lights(model),
            fixtures: fixture_ids.len(),
            furnishings: made(&self.furnishings),
            furnishing_boxes: self
                .furnishings
                .iter()
                .filter(|p| !p.meshes.is_empty())
                .filter_map(|p| furnishing::placed_box(&p.input.furnishing, p.input.level.elevation))
                .collect(),
            electrical: made(&self.electrical),
            electrical_boxes: self
                .electrical
                .iter()
                .filter(|p| !p.meshes.is_empty())
                .filter_map(|p| Some(crate::electrical::plate_box(&p.input.device, p.input.place.as_ref()?, p.input.level)))
                .collect(),
            ..Default::default()
        };
        scene
    }

    /// Every part's meshes, section by section.
    fn all_parts(&self) -> impl Iterator<Item = &Vec<Mesh>> {
        self.walls
            .iter()
            .map(|p| &p.meshes)
            .chain(self.openings.iter().map(|p| &p.meshes))
            .chain(self.rooms.iter().map(|p| &p.meshes))
            .chain(self.roofs.iter().map(|p| &p.meshes))
            .chain(self.stairs.iter().map(|p| &p.meshes))
            .chain(self.railings.iter().map(|p| &p.meshes))
            .chain(self.decks.iter().map(|p| &p.meshes))
            .chain(self.site.iter().map(|p| &p.meshes))
            .chain(self.fixtures.iter().map(|p| &p.meshes))
            .chain(self.cabinets.iter().map(|p| &p.meshes))
            .chain(self.casework_fixtures.iter().map(|p| &p.meshes))
            .chain(self.furnishings.iter().map(|p| &p.meshes))
            .chain(self.electrical.iter().map(|p| &p.meshes))
            .chain(self.soffits.iter().map(|p| &p.meshes))
            .chain(self.foundations.iter().map(|p| &p.meshes))
            .chain(self.room_moldings.iter().map(|p| &p.meshes))
            .chain(self.molding_lines.iter().map(|p| &p.meshes))
            .chain(self.countertops.iter().map(|p| &p.meshes))
            .chain(self.backsplashes.iter().map(|p| &p.meshes))
    }

    /// The objects of the parts made in the last build.
    fn fresh_keys(&self) -> BTreeSet<ObjectKey> {
        let mut out = BTreeSet::new();
        let mut add = |fresh: bool, meshes: &[Mesh]| {
            if fresh {
                out.extend(meshes.iter().map(object_key));
            }
        };
        self.walls.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.openings.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.rooms.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.roofs.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.stairs.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.railings.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.decks.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.site.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.fixtures.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.cabinets.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.casework_fixtures.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.furnishings.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.electrical.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.soffits.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.foundations.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.room_moldings.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.molding_lines.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.countertops.iter().for_each(|p| add(p.fresh, &p.meshes));
        self.backsplashes.iter().for_each(|p| add(p.fresh, &p.meshes));
        out
    }
}

/// The casework inputs of one level: the projections of its base cabinets, those of its wall and
/// tall cabinets, the furnishings hosted by any cabinet on it, and the cabinet ids its custom
/// countertops claim (sorted).
type LevelCasework = (Vec<CabinetNeighbour>, Vec<CabinetNeighbour>, Vec<Furnishing>, Vec<ArchId>);

/// Per level index: the projections of its base cabinets, those of its wall and tall cabinets (all
/// in model order), the furnishings hosted by any cabinet on it or by a gap beside one, and the
/// cabinet ids on it that custom countertops claim. The casework inputs every cabinet on that level
/// shares; projections, not clones, so an edit of a field no other cabinet reads rebuilds one cabinet.
fn casework_levels(model: &ArchModel) -> BTreeMap<u32, LevelCasework> {
    let mut on_level: BTreeMap<u32, BTreeSet<ArchId>> = BTreeMap::new();
    for c in &model.cabinets {
        on_level.entry(c.level).or_default().insert(c.id);
    }
    let claimed_all = cadcraft_arch::countertop::claimed_cabinets(model);
    on_level
        .into_iter()
        .map(|(level, ids)| {
            let project = |base: bool| -> Vec<CabinetNeighbour> {
                model.cabinets.iter().filter(|c| c.level == level && (c.kind == CabinetKind::Base) == base).map(CabinetNeighbour::of).collect()
            };
            let hosted = model
                .furnishings
                .iter()
                .filter(|f| match f.host {
                    FurnishingHost::Cabinet { cabinet } => ids.contains(&cabinet),
                    FurnishingHost::Gap { left, right } => ids.contains(&left) || ids.contains(&right),
                    FurnishingHost::None | FurnishingHost::Wall { .. } | FurnishingHost::Sink { .. } => false,
                })
                .cloned()
                .collect();
            let claimed = ids.iter().copied().filter(|id| claimed_all.contains(id)).collect();
            (level, (project(true), project(false), hosted, claimed))
        })
        .collect()
}

/// Whether a mesh goes into the scene: [`Scene3d::build`] drops empty and non-finite meshes.
fn keep(m: &Mesh) -> bool {
    !m.positions.is_empty() && mesh_is_finite(m)
}

/// `scene` with `meshes` and their bounds.
fn with_meshes(mut scene: Scene3d, meshes: Vec<Mesh>) -> Scene3d {
    scene.bounds = match meshes.split_first() {
        None => Bounds3::empty(),
        Some((first, rest)) => rest.iter().fold(first.bounds(), |mut b, m| {
            b.extend(&m.bounds());
            b
        }),
    };
    scene.meshes = meshes;
    scene
}

/// How many of `list`'s objects made meshes.
fn made<I>(list: &[Part<I>]) -> usize {
    list.iter().filter(|p| !p.meshes.is_empty()).count()
}

/// One wall's mesh: its join shape extruded, cut by its openings and clipped by the roof over it.
fn wall_mesh(i: &WallIn) -> Vec<Mesh> {
    if let Some(m) = crate::wall_glass::wall_meshes(i) {
        return m;
    }
    let Some(mut mesh) = wall_base_mesh(i) else {
        return Vec::new();
    };
    // The niche's back block becomes part of the wall mesh, so finishes and paints split it too.
    if let Some(span) = i.span {
        crate::niche::add_back_blocks(&mut mesh, &i.wall, &i.openings, span);
    }
    let (rest, outer) = crate::finish::split_exterior_face(&i.wall, mesh);
    let mut out = Vec::new();
    if !rest.indices.is_empty() {
        out.push(rest);
    }
    out.extend(outer);
    if let Some(span) = i.span {
        out.extend(crate::niche::back_skins(&i.wall, &i.openings, span));
    }
    out
}

/// The wall's whole mesh, before any finish splits its exterior face off.
fn wall_base_mesh(i: &WallIn) -> Option<Mesh> {
    wall_base_mesh_span(i, i.span?)
}

/// The wall's whole mesh between `span` = (base, top), flight cuts included.
pub(crate) fn wall_base_mesh_span(i: &WallIn, span: (f64, f64)) -> Option<Mesh> {
    let (base, top) = span;
    let openings: Vec<&Opening> = i.openings.iter().collect();
    let mesh = if !openings.is_empty() || i.clip.is_some() || i.roof_top.is_some() || !i.flight_cuts.is_empty() {
        let shape = i.shape.as_ref()?;
        let mut profile = elevation_profile(shape, &i.wall, base, top, i.clip.as_ref(), i.roof_top.as_deref(), &openings)?;
        crate::flight_cut::apply(&mut profile, &i.flight_cuts, base);
        if i.roof_top.is_some() {
            extrude_profile_slab(shape, &profile, base, &i.wall, "wall")
        } else {
            extrude_wall_profile(shape, profile, base, "wall")
        }
    } else {
        match &i.shape {
            Some(shape) => walls::extrude_wall(shape, top - base, base, "wall"),
            None => raw_wall_mesh(&i.wall, base, top, i.wall.id),
        }
    };
    mesh.map(|mut m| {
        m.level_elevation = base;
        m
    })
}

/// The eave height of a roof on `level`: the level's elevation plus its tallest wall.
fn eave_z(model: &ArchModel, level: u32) -> f64 {
    let lv = level_for(model, level);
    let mut max_wall_height = 0.0;
    for wall in model.walls.iter().filter(|w| crate::wall_reaches_level(w, level)) {
        if let Some((_, top)) = wall_span(model, wall) {
            let h = top - lv.elevation;
            if h > max_wall_height {
                max_wall_height = h;
            }
        }
    }
    lv.elevation + max_wall_height
}

fn fnv1a(bytes: &[u8], mut hash: u64) -> u64 {
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn mesh_digest(mesh: &Mesh) -> u64 {
    let mut hash = fnv1a(mesh.material.as_bytes(), FNV_OFFSET_BASIS);
    hash = fnv1a(&mesh.object.0.to_le_bytes(), hash);
    hash = fnv1a(&[object_key(mesh).kind as u8], hash);
    for v in mesh.positions.iter().chain(&mesh.normals).flatten() {
        hash = fnv1a(&v.to_bits().to_le_bytes(), hash);
    }
    for v in mesh.uvs.iter().flatten() {
        hash = fnv1a(&v.to_bits().to_le_bytes(), hash);
    }
    for index in &mesh.indices {
        hash = fnv1a(&index.to_le_bytes(), hash);
    }
    hash
}

/// FNV-1a 64 of `bytes` with their length in front, so consecutive strings cannot run together.
fn fnv1a_str(bytes: &[u8], hash: u64) -> u64 {
    fnv1a(bytes, fnv1a(&(bytes.len() as u64).to_le_bytes(), hash))
}

impl Scene3d {
    /// FNV-1a 64 of every mesh (material, owner, positions, normals, uvs as f32 bits, indices),
    /// taken in a canonical order (meshes sorted by their own digest), then of every material by
    /// name (its name, base colour, metallic and roughness as f32 bits, texture key, tile and tint):
    /// 16 lower-case hex digits that do not depend on mesh order. Equal geometry and materials give
    /// equal digests; a scene without materials digests as it did before phase 14.
    pub fn digest(&self) -> String {
        let mut digests: Vec<u64> = self.meshes.iter().map(mesh_digest).collect();
        digests.sort_unstable();
        let mut hash = digests.iter().fold(FNV_OFFSET_BASIS, |h, d| fnv1a(&d.to_le_bytes(), h));
        // `materials` is a BTreeMap: its iteration is sorted by name.
        for (name, m) in &self.materials {
            hash = fnv1a_str(name.as_bytes(), hash);
            for v in m.base_color.iter().chain([&m.metallic, &m.roughness]).chain(&m.tile) {
                hash = fnv1a(&v.to_bits().to_le_bytes(), hash);
            }
            hash = match &m.texture {
                Some(key) => fnv1a_str(key.as_bytes(), fnv1a(&[1], hash)),
                None => fnv1a(&[0], hash),
            };
            hash = match m.tint {
                Some(t) => fnv1a(&t, fnv1a(&[1], hash)),
                None => fnv1a(&[0], hash),
            };
        }
        format!("{hash:016x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The phase-1 house: four 0.2 m exterior walls on 10 x 8 m, 2.4 m high, and a gable roof with
    /// gables on x = 0 and x = 10.
    fn p17_house(roof: bool) -> ArchModel {
        use cadcraft_doc::RoofDirective;
        let mut m = ArchModel::default();
        for (a, b) in [((0.0, 0.0), (10.0, 0.0)), ((10.0, 0.0), (10.0, 8.0)), ((10.0, 8.0), (0.0, 8.0)), ((0.0, 8.0), (0.0, 0.0))] {
            m.push_wall(Wall {
                start: Vec2::new(a.0, a.1),
                end: Vec2::new(b.0, b.1),
                thickness: 0.2,
                height: 2.4,
                exterior: true,
                ..Default::default()
            });
        }
        if roof {
            m.walls[1].roof_directive = RoofDirective::Gable;
            m.walls[3].roof_directive = RoofDirective::Gable;
            let r = Roof { pitch_rise: 6.0, pitch_run: 12.0, overhang: 0.5, thickness: 0.2, ..Roof::default() };
            if let Ok(built) = cadcraft_arch::roof::build_roof(&m, 0, &r) {
                m.push_roof(built);
            }
        }
        m
    }

    /// A stair on the phase-1 house: start (2, 0.6), +x, width 1.0, level 0 to 1 at 2.7.
    fn p17_stair() -> Stair {
        Stair {
            start: Vec2::new(2.0, 0.6),
            width: 1.0,
            from_level: 0,
            to_level: 1,
            rise: 2.7,
            risers: 16,
            riser_height: 0.16875,
            treads: 15,
            tread_depth: 0.254,
            run: 3.81,
            ..Default::default()
        }
    }

    #[test]
    fn object_key_of_deck_structure_is_the_room() {
        for material in ["deck", "deck-ledger", "deck-joist", "deck-rim", "deck-beam", "deck-post", "deck-footing"] {
            let key = object_key(&Mesh::new(material, ArchId(6)));
            assert_eq!(key, ObjectKey { kind: ObjectKind::Room, id: ArchId(6) }, "{material}");
        }
        // Phase 16's footing stays a foundation's, and the new parts find their owners.
        let owner = |material: &str| object_key(&Mesh::new(material, ArchId(1))).kind;
        assert_eq!(owner("footing"), ObjectKind::Foundation);
        for (material, kind) in [
            ("stair-tread", ObjectKind::Stair),
            ("stair-riser", ObjectKind::Stair),
            ("stair-stringer", ObjectKind::Stair),
            ("stair-skirt", ObjectKind::Stair),
            ("stair-landing", ObjectKind::Stair),
            ("railing-rail", ObjectKind::Railing),
            ("window-grille", ObjectKind::Opening),
            ("door-grille", ObjectKind::Opening),
        ] {
            assert_eq!(owner(material), kind, "{material}");
        }
    }

    #[test]
    fn object_kind_of_phase18_meshes() {
        let kind = |material: &str| object_kind_of(&Mesh::new(material, ArchId(1)));
        for (material, want) in [
            ("countertop", ObjectKind::Countertop),
            ("countertop-waterfall", ObjectKind::Countertop),
            ("backsplash", ObjectKind::Backsplash),
            ("molding-line", ObjectKind::MoldingLine),
            ("molding-room-base-0", ObjectKind::Room),
            ("glass-wall", ObjectKind::Wall),
            ("niche-back:o3", ObjectKind::Wall),
            ("deck-joist", ObjectKind::Room),
            ("counter", ObjectKind::Cabinet),
            ("paint:countertop/ct0", ObjectKind::Countertop),
            ("paint:backsplash/bs1", ObjectKind::Backsplash),
            ("paint:cabinet/cb0/counter", ObjectKind::Cabinet),
        ] {
            assert_eq!(kind(material), want, "{material}");
        }
        assert_eq!(object_key(&Mesh::new("backsplash", ArchId(2))), ObjectKey { kind: ObjectKind::Backsplash, id: ArchId(2) });
        assert_eq!(ObjectKind::from_part("backsplash"), ObjectKind::Backsplash);
    }

    #[test]
    fn changed_objects_reports_countertops_backsplashes_molding_lines_and_room_moldings() {
        use cadcraft_doc::arch::{Backsplash as Bs, Countertop as Ct, RoomMolding};
        let mut prev = ArchModel::default();
        prev.rooms.push(Room { id: ArchId(0), ..Default::default() });
        let mut next = prev.clone();
        assert_eq!(changed_objects(&prev, &next), Some(BTreeSet::new()));
        next.push_countertop(Ct::default());
        next.push_backsplash(Bs::default());
        next.push_molding_line(MoldingLine::default());
        next.rooms[0].moldings.push(RoomMolding::default());
        let got = changed_objects(&prev, &next).unwrap_or_default();
        let key = |kind| ObjectKey { kind, id: ArchId(0) };
        assert_eq!(
            got,
            BTreeSet::from([key(ObjectKind::Countertop), key(ObjectKind::Backsplash), key(ObjectKind::MoldingLine), key(ObjectKind::Room)])
        );
        // A paint on a countertop counts for the countertop.
        let mut painted = next.clone();
        painted.paints.insert("countertop/ct0".into(), Paint::default());
        let got = changed_objects(&next, &painted).unwrap_or_default();
        assert!(got.contains(&key(ObjectKind::Countertop)), "{got:?}");
    }

    #[test]
    fn object_kind_label_and_handle() {
        use cadcraft_doc::arch as a;
        let id = ArchId(4);
        assert_eq!(ObjectKind::Wall.label(id), a::wall_label(id));
        assert_eq!(ObjectKind::Wall.handle(id), a::arch_handle(id));
        assert_eq!(ObjectKind::Terrain.label(id), a::terrain_label(Some(4)));
        assert_eq!(ObjectKind::Terrain.handle(id), a::arch_handle(id));
        assert_eq!(ObjectKind::Opening.handle(id), a::opening_handle(id));
        assert_eq!(ObjectKind::Cabinet.label(id), a::cabinet_label(id));
        assert_eq!(ObjectKind::Cabinet.handle(id), a::cabinet_handle(id));
        assert_eq!(ObjectKind::Countertop.label(id), "ct4");
        assert_eq!(ObjectKind::Countertop.handle(id), a::countertop_handle(id));
        assert_eq!(ObjectKind::Backsplash.label(id), "bs4");
        assert_eq!(ObjectKind::Backsplash.handle(id), a::backsplash_handle(id));
        assert_eq!(ObjectKind::MoldingLine.label(id), "ml4");
        assert_eq!(ObjectKind::MoldingLine.handle(id), a::molding_line_handle(id));
        assert_eq!(ObjectKind::Room.handle(id), a::room_handle(id));
        // Every kind's handle is distinct for one id.
        let kinds = [
            ObjectKind::Wall,
            ObjectKind::Opening,
            ObjectKind::Room,
            ObjectKind::Roof,
            ObjectKind::Stair,
            ObjectKind::Railing,
            ObjectKind::Plant,
            ObjectKind::Light,
            ObjectKind::Cabinet,
            ObjectKind::Fixture,
            ObjectKind::Furnishing,
            ObjectKind::Electrical,
            ObjectKind::Soffit,
            ObjectKind::Foundation,
            ObjectKind::Countertop,
            ObjectKind::Backsplash,
            ObjectKind::MoldingLine,
        ];
        let handles: BTreeSet<_> = kinds.iter().map(|k| k.handle(id).0).collect();
        assert_eq!(handles.len(), kinds.len());
    }

    #[test]
    fn deck_post_and_railing_post_are_different_owners() {
        let deck = object_key(&Mesh::new("deck-post", ArchId(2)));
        let rail = object_key(&Mesh::new("railing-post", ArchId(2)));
        assert_eq!(deck.kind, ObjectKind::Room);
        assert_eq!(rail.kind, ObjectKind::Railing);
        assert_ne!(deck, rail);
        let newel = object_key(&Mesh::new("railing-newel", ArchId(2)));
        assert_eq!(newel, rail);
    }

    fn p17_deck() -> Room {
        let polygon = vec![Vec2::new(2.0, 8.1), Vec2::new(6.0, 8.1), Vec2::new(6.0, 11.1), Vec2::new(2.0, 11.1)];
        Room { polygon, room_type: RoomType::Deck, deck_height: Some(0.9), ..Default::default() }
    }

    #[test]
    fn stair_inputs_carry_against() {
        let mut m = p17_house(false);
        m.push_stair(p17_stair());
        m.push_stair(Stair { start: Vec2::new(2.0, 1.0), ..p17_stair() });
        let s = Sections::make(&m, None);
        let got: Vec<[bool; 2]> = s.stairs.iter().map(|p| p.input.against).collect();
        assert_eq!(got, vec![[false, true], [false, false]]);
        for p in &s.stairs {
            assert_eq!(p.input.against, cadcraft_arch::stair_geom::sides_against_walls(&m, &p.input.stair));
        }
    }

    #[test]
    fn railing_inputs_carry_the_surface() {
        use cadcraft_arch::railing::RailSurface;
        use cadcraft_doc::arch::RailingHost;
        let mut m = p17_house(false);
        m.levels.clear();
        m.levels.push(Level { index: 0, elevation: 0.3, ceiling_height: 2.4, ..Default::default() });
        let deck = m.push_room(p17_deck());
        let flat = m.push_railing(Railing { path: vec![Vec2::new(0.0, 0.5), Vec2::new(4.0, 0.5)], ..Default::default() });
        let on_deck = m.push_railing(Railing {
            path: vec![Vec2::new(2.0, 11.1), Vec2::new(6.0, 11.1)],
            follows: Some(deck),
            host: RailingHost::Deck,
            ..Default::default()
        });
        let s = Sections::make(&m, None);
        let surface = |id: ArchId| s.railings.iter().find(|p| p.id == id).map(|p| p.input.surface.clone());
        assert_eq!(surface(flat), Some(RailSurface::Flat { z: 0.3 }));
        match surface(on_deck) {
            Some(RailSurface::Deck { top, post_bottom }) => {
                assert!((top - 1.2).abs() < 1e-9 && (post_bottom - (1.2 - 0.025 - 0.184)).abs() < 1e-9, "{top} {post_bottom}");
            }
            other => panic!("a deck surface, got {other:?}"),
        }
        // A deck height change moves the surface, so the railing is made again.
        let mut next = m.clone();
        if let Some(r) = next.rooms.iter_mut().find(|r| r.id == deck) {
            r.deck_height = Some(0.5);
        }
        let again = Sections::make(&next, Some(s));
        let moved = again.railings.iter().find(|p| p.id == on_deck);
        assert!(moved.is_some_and(|p| p.fresh), "the deck railing is rebuilt");
        assert!(again.railings.iter().find(|p| p.id == flat).is_some_and(|p| !p.fresh), "the flat one is kept");
    }

    #[test]
    fn wall_inputs_carry_flight_cuts() {
        let mut m = p17_house(false);
        m.push_stair(p17_stair());
        let s = Sections::make(&m, None);
        assert_eq!(s.walls.len(), 4);
        for p in &s.walls {
            assert_eq!(p.input.flight_cuts, cadcraft_arch::flight_cut::flight_cuts(&m, &p.input.wall), "wall {}", p.id);
        }
        // A wall without a span (a missing level) carries none.
        let mut skipped = p17_house(false);
        skipped.walls[0].base_level = 7;
        let s = Sections::make(&skipped, None);
        assert!(s.walls.iter().all(|p| p.input.span.is_some() || p.input.flight_cuts.is_empty()));
    }

    #[test]
    fn roof_window_openings_carry_the_host_roof_top() {
        let mut m = p17_house(true);
        assert!(!m.roofs.is_empty(), "the gable roof builds");
        let gable = m.walls[1].id;
        let south = m.walls[0].id;
        let mut roof = Opening::new(gable, 4.0, 1.0, 1.0, cadcraft_doc::arch::OpeningKind::Window);
        roof.top = OpeningTop::Roof;
        let roof = m.push_opening(roof);
        let square = m.push_opening(Opening::new(gable, 2.0, 0.6, 1.0, cadcraft_doc::arch::OpeningKind::Window));
        let other = m.push_opening({
            let mut o = Opening::new(south, 4.0, 1.0, 1.0, cadcraft_doc::arch::OpeningKind::Window);
            o.top = OpeningTop::Roof;
            o
        });
        let s = Sections::make(&m, None);
        let host_top = s.walls.iter().find(|p| p.id == gable).and_then(|p| p.input.roof_top.clone());
        assert!(host_top.is_some(), "the gable wall has a roof top");
        let input = |id: ArchId| s.openings.iter().find(|p| p.id == id).map(|p| p.input.roof_top.clone());
        assert_eq!(input(roof), Some(host_top), "a roof-topped opening carries its host's roof top");
        assert_eq!(input(square), Some(None), "a square one carries none");
        let south_top = s.walls.iter().find(|p| p.id == south).and_then(|p| p.input.roof_top.clone());
        assert_eq!(input(other), Some(south_top), "whatever the host has, even nothing");
    }

    #[test]
    fn cache_equals_full_with_stairs_decks_railings_and_cuts() {
        use cadcraft_doc::arch::RailingHost;
        let mut model = p17_house(false);
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        model.levels.push(Level { index: 1, elevation: 2.7, ceiling_height: 2.4, ..Default::default() });
        let stair = model.push_stair(p17_stair());
        let deck = model.push_room(p17_deck());
        let rail = model.push_railing(Railing { path: vec![Vec2::new(0.0, 0.5), Vec2::new(4.0, 0.5)], ..Default::default() });
        model.push_railing(Railing {
            path: vec![Vec2::new(2.0, 11.1), Vec2::new(6.0, 11.1)],
            follows: Some(deck),
            host: RailingHost::Deck,
            ..Default::default()
        });
        let (mut cache, _) = SceneCache::new(&model);
        assert_eq!(cache.scene().digest(), Scene3d::build(&model).digest());
        // Seeded edits: stair width, deck height, railing path, a wall end.
        let mut state = 7u64;
        for step in 0..24 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let pick = (state >> 33) % 4;
            let amount = 0.05 + ((state >> 20) % 10) as f64 * 0.02;
            let mut next = model.clone();
            match pick {
                0 => {
                    if let Some(s) = next.stairs.iter_mut().find(|s| s.id == stair) {
                        s.width = 0.9 + amount;
                    }
                }
                1 => {
                    if let Some(r) = next.rooms.iter_mut().find(|r| r.id == deck) {
                        r.deck_height = Some(0.3 + amount);
                    }
                }
                2 => {
                    if let Some(r) = next.railings.iter_mut().find(|r| r.id == rail) {
                        if let Some(p) = r.path.get_mut(1) {
                            p.x = 3.0 + amount * 10.0;
                        }
                    }
                }
                _ => {
                    if let Some(w) = next.walls.get_mut(2) {
                        w.end.x = 0.5 * amount;
                    }
                }
            }
            cache.update(&next);
            assert_eq!(cache.scene().digest(), Scene3d::build(&next).digest(), "step {step} (edit {pick})");
            model = next;
        }
    }

    fn square_room() -> ArchModel {
        let mut m = ArchModel::default();
        let ring = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)];
        m.push_room(Room { polygon: ring, ..Default::default() });
        m
    }

    #[test]
    fn stack_layers_and_tray_sides_do_not_add_ceilings_and_slabs_are_floors() {
        let m = square_room();
        let mut sections = Sections::make(&m, None);
        let before = sections.counts(&m);
        assert_eq!((before.floors, before.ceilings), (1, 1));
        let room = sections.rooms.first_mut().expect("the room's part");
        // An upper stack layer and a tray side are more ceiling meshes of the same room.
        room.meshes.push(Mesh::new("ceiling-layer", room.id));
        room.meshes.push(Mesh::new("ceiling", room.id));
        room.meshes.push(Mesh::new("slab", room.id));
        let after = sections.counts(&m);
        assert_eq!((after.floors, after.ceilings), (2, 1));
    }

    #[test]
    fn a_room_on_a_slab_level_knows_it() {
        let mut m = square_room();
        assert!(Sections::make(&m, None).rooms.iter().all(|p| !p.input.slab));
        m.push_foundation(cadcraft_doc::arch::Foundation { kind: FoundationKind::Slab, ..Default::default() });
        let sections = Sections::make(&m, None);
        assert!(sections.rooms.iter().all(|p| p.input.slab));
        assert_eq!(sections.foundations.len(), 1);
        m.foundations[0].kind = FoundationKind::Stem;
        assert!(Sections::make(&m, None).rooms.iter().all(|p| !p.input.slab));
    }
}
