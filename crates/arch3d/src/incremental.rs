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

use std::collections::{BTreeMap, BTreeSet};

use cadcraft_arch::join::{WallShape, solve_joins};
use cadcraft_arch::roof::{Footprint, outer_footprint};
use cadcraft_doc::arch::{
    Cabinet, Fixture, Level, LightFixture, Opening, Plant, PlantSpecies, Railing, Roof, Room, RoomType, Stair, Terrain, WallType,
};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::{Vec2, Vec3};

use crate::mesh::{Bounds3, Mesh};
use crate::probe::part_of;
use crate::walls::{RoofClip, elevation_profile, extrude_profile_slab, extrude_wall_profile, roof_top_polyline_in};
use crate::{
    Scene3d, casework, find_roof_clip, fixture, floors, level_for, light, mesh_is_finite, openings, railing, raw_wall_mesh, site, stairs, wall_span,
    walls, well_holes_for,
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
}

impl ObjectKind {
    /// The kind owning a mesh of the [`part_of`] class `part`.
    fn from_part(part: &str) -> ObjectKind {
        match part {
            "floor" | "ceiling" | "deck" => ObjectKind::Room,
            "glass" | "frame" | "casing" | "leaf" | "sill" | "header" => ObjectKind::Opening,
            "roof" => ObjectKind::Roof,
            "stair" => ObjectKind::Stair,
            "railing" => ObjectKind::Railing,
            "terrain" | "path" => ObjectKind::Terrain,
            "plant" => ObjectKind::Plant,
            "light" => ObjectKind::Light,
            "cabinet" | "counter" => ObjectKind::Cabinet,
            "fixture" => ObjectKind::Fixture,
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

/// The object `mesh` belongs to, from its [`part_of`] class and its `object` id.
pub fn object_key(mesh: &Mesh) -> ObjectKey {
    ObjectKey { kind: ObjectKind::from_part(part_of(mesh)), id: mesh.object }
}

/// The objects whose data differ between `prev` and `next` (added, removed or changed, compared by
/// id per kind); `None` when something not owned by one object differs (levels, wall types, the
/// site location, the plant species), and the caller rebuilds everything.
pub fn changed_objects(prev: &ArchModel, next: &ArchModel) -> Option<BTreeSet<ObjectKey>> {
    if prev.levels != next.levels || prev.types != next.types || prev.site != next.site || prev.plant_species != next.plant_species {
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
    if prev.terrain != next.terrain {
        changes.insert(ObjectKey { kind: ObjectKind::Terrain, id: ArchId(0) });
    }
    Some(changes)
}

/// Add to `out` every id of `kind` whose objects differ between `prev` and `next` (all objects
/// with that id, in order, so duplicate ids compare too).
fn diff<T: PartialEq>(prev: &[T], next: &[T], id: impl Fn(&T) -> ArchId, kind: ObjectKind, out: &mut BTreeSet<ObjectKey>) {
    fn group<'a, T>(list: &'a [T], id: &impl Fn(&T) -> ArchId) -> BTreeMap<ArchId, Vec<&'a T>> {
        let mut by: BTreeMap<ArchId, Vec<&'a T>> = BTreeMap::new();
        for x in list {
            by.entry(id(x)).or_default().push(x);
        }
        by
    }
    let (a, b) = (group(prev, &id), group(next, &id));
    for k in a.keys().chain(b.keys()) {
        if a.get(k) != b.get(k) {
            out.insert(ObjectKey { kind, id: *k });
        }
    }
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
        let scene = sections.scene(model);
        let objects = object_keys(&scene.meshes).len();
        (SceneCache { model: model.clone(), sections, scene }, RebuildStats { rebuilt: objects, objects, incremental: false })
    }

    /// Bring the scene up to `model`: a full build when something global changed (see
    /// [`changed_objects`]), else only the parts whose inputs changed are made again.
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
        self.model = model.clone();
        let present = object_keys(&self.scene.meshes);
        let rebuilt = if changed.is_empty() { 0 } else { self.sections.fresh_keys().intersection(&present).count() };
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

#[derive(Clone, Debug, PartialEq)]
struct WallIn {
    wall: Wall,
    /// `None` skips the wall (and leaves the rest empty).
    span: Option<(f64, f64)>,
    shape: Option<WallShape>,
    openings: Vec<Opening>,
    clip: Option<RoofClip>,
    roof_top: Option<Vec<Vec2>>,
}

#[derive(Clone, Debug, PartialEq)]
struct OpeningIn {
    opening: Opening,
    host: Option<Wall>,
    span: Option<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
struct RoomIn {
    room: Room,
    level: Level,
    /// The room's holes plus the stair wells opening into its level (the floor's holes).
    wells: Vec<Vec<Vec2>>,
    /// The ceiling openings of the recessed fixtures in it.
    cuts: Vec<Vec<Vec2>>,
}

#[derive(Clone, Debug, PartialEq)]
struct RoofIn {
    roof: Roof,
    eave_z: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct StairIn {
    stair: Stair,
    base: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct RailingIn {
    railing: Railing,
    base: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct DeckIn {
    room: Room,
    level: Level,
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

#[derive(Clone, Debug, PartialEq)]
struct CabinetIn {
    cabinet: Cabinet,
    level: Level,
}

#[derive(Clone, Debug, PartialEq)]
struct CaseworkFixtureIn {
    fixture: Fixture,
    level: Level,
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

        let wall_items = model
            .walls
            .iter()
            .map(|wall| {
                let span = wall_span(model, wall);
                let mut input = WallIn { wall: wall.clone(), span, shape: None, openings: Vec::new(), clip: None, roof_top: None };
                if let Some((base, top)) = span {
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
                (wall.id, input)
            })
            .collect();
        let walls = parts(pw, wall_items, |i| wall_mesh(i));

        let opening_items = model
            .openings
            .iter()
            .map(|o| {
                let host = wall_map.get(&o.host).copied();
                let input = OpeningIn { opening: o.clone(), host: host.cloned(), span: host.and_then(|w| wall_span(model, w)) };
                (o.id, input)
            })
            .collect();
        let openings = parts(po, opening_items, |i| {
            let map: BTreeMap<ArchId, &Wall> = i.host.iter().map(|w| (i.opening.host, w)).collect();
            openings::opening_meshes(&map, &i.opening, i.span).into_iter().filter(|m| !m.positions.is_empty()).collect()
        });

        let room_items = model
            .rooms
            .iter()
            .map(|room| {
                let level = level_for(model, room.level);
                let wells = well_holes_for(model, room);
                let cuts = fixture::ceiling_cutouts(model, room, &level);
                (room.id, RoomIn { room: room.clone(), level, wells, cuts })
            })
            .collect();
        let rooms = parts(pr, room_items, |i| {
            let mut out = Vec::new();
            if i.room.room_type != RoomType::Deck {
                let mut floor = i.room.clone();
                floor.holes = i.wells.clone();
                out.extend(floors::floor_slab(&floor, &i.level, "floor").map(|m| crate::finish::finish_floor(m, i.room.floor_finish)));
            }
            let mut ceiling = i.room.clone();
            ceiling.holes.extend(i.cuts.iter().cloned());
            out.extend(floors::ceiling_surface(&ceiling, &i.level, "ceiling"));
            out
        });

        let roof_items = model.roofs.iter().map(|roof| (roof.id, RoofIn { roof: roof.clone(), eave_z: eave_z(model, roof.level) })).collect();
        let roofs = parts(pf, roof_items, |i| crate::roof::roof_meshes(&i.roof, i.eave_z).into_iter().filter(|m| !m.positions.is_empty()).collect());

        let stair_items = model.stairs.iter().map(|s| (s.id, StairIn { stair: s.clone(), base: level_for(model, s.from_level).elevation })).collect();
        let stairs =
            parts(ps, stair_items, |i| stairs::stair_mesh(&i.stair, i.base).into_iter().chain(stairs::landing_mesh(&i.stair, i.base)).collect());

        let railing_items =
            model.railings.iter().map(|r| (r.id, RailingIn { railing: r.clone(), base: level_for(model, r.level).elevation })).collect();
        let railings = parts(pl, railing_items, |i| railing::railing_mesh(&i.railing, i.base).into_iter().collect());

        let deck_items = model
            .rooms
            .iter()
            .filter(|r| r.room_type == RoomType::Deck)
            .map(|r| (r.id, DeckIn { room: r.clone(), level: level_for(model, r.level) }))
            .collect();
        let decks = parts(pd, deck_items, |i| {
            let material = if i.room.deck_height.is_some_and(|h| h > 0.0 && h.is_finite()) { "deck" } else { "floor" };
            floors::floor_slab(&i.room, &i.level, material).into_iter().collect()
        });

        let site_in = SiteIn {
            terrain: model.terrain.clone(),
            plants: model.plants.clone(),
            species: model.plant_species.clone(),
            breaklines: model.walls.iter().filter(|w| w.retaining).map(|w| [w.start, w.end]).collect(),
            pads: if model.terrain.is_some() { cadcraft_arch::terrain::pad::pad_regions(model) } else { Vec::new() },
        };
        let site = parts(pt, vec![(ArchId(0), site_in)], |_| site::site_meshes(model).meshes);

        let fixture_items =
            model.lights.iter().map(|f| (f.id, FixtureIn { light: f.clone(), point: cadcraft_arch::light::light_point(model, f) })).collect();
        let fixtures = parts(px, fixture_items, |i| fixture::fixture_housing(&i.light, i.point));

        let cabinet_items = model.cabinets.iter().map(|c| (c.id, CabinetIn { cabinet: c.clone(), level: level_for(model, c.level) })).collect();
        let cabinets = parts(pc, cabinet_items, |i| casework::cabinet_meshes(model, &i.cabinet));

        let casework_fixture_items =
            model.fixtures.iter().map(|f| (f.id, CaseworkFixtureIn { fixture: f.clone(), level: level_for(model, f.level) })).collect();
        let casework_fixtures = parts(pfx, casework_fixture_items, |i| vec![casework::fixture_mesh(model, &i.fixture)]);

        let joins = Some((model.walls.clone(), model.types.clone(), shapes));
        Sections { walls, openings, rooms, roofs, stairs, railings, decks, site, fixtures, cabinets, casework_fixtures, joins, footprints }
    }

    /// The scene these parts make (a copy; the cache keeps its parts).
    pub(crate) fn scene(&self, model: &ArchModel) -> Scene3d {
        let meshes = self.all_parts().flatten().filter(|m| keep(m)).cloned().collect();
        with_meshes(self.counts(model), meshes)
    }

    /// The scene these parts make, moving their meshes into it.
    pub(crate) fn into_scene(self, model: &ArchModel) -> Scene3d {
        let counts = self.counts(model);
        let Sections { walls, openings, rooms, roofs, stairs, railings, decks, site, fixtures, cabinets, casework_fixtures, .. } = self;
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
            .chain(meshes(casework_fixtures));
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
            floors: room_meshes().filter(|m| m.material.starts_with("floor")).count() + decks,
            ceilings: room_meshes().filter(|m| m.material == "ceiling").count(),
            roofs: made(&self.roofs),
            stairs: made(&self.stairs),
            railings: made(&self.railings),
            decks,
            terrain: site_meshes.clone().filter(|m| m.material == "terrain").count(),
            paths: site_meshes.clone().filter(|m| m.material.starts_with("path-")).count(),
            plants: site_meshes.filter(|m| m.material.starts_with("plant-")).map(|m| m.object).collect::<BTreeSet<_>>().len(),
            lights: light::scene_lights(model),
            fixtures: fixture_ids.len(),
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
        out
    }
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
    let Some(mesh) = wall_base_mesh(i) else {
        return Vec::new();
    };
    let (rest, outer) = crate::finish::split_exterior_face(&i.wall, mesh);
    let mut out = Vec::new();
    if !rest.indices.is_empty() {
        out.push(rest);
    }
    out.extend(outer);
    out
}

/// The wall's whole mesh, before any finish splits its exterior face off.
fn wall_base_mesh(i: &WallIn) -> Option<Mesh> {
    let (base, top) = i.span?;
    let openings: Vec<&Opening> = i.openings.iter().collect();
    let mesh = if !openings.is_empty() || i.clip.is_some() || i.roof_top.is_some() {
        let shape = i.shape.as_ref()?;
        let profile = elevation_profile(shape, &i.wall, base, top, i.clip.as_ref(), i.roof_top.as_deref(), &openings)?;
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
    for wall in model.walls.iter().filter(|w| w.base_level == level) {
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

impl Scene3d {
    /// FNV-1a 64 of every mesh (material, owner, positions, normals, uvs as f32 bits, indices),
    /// taken in a canonical order (meshes sorted by their own digest): 16 lower-case hex digits that
    /// do not depend on mesh order. Equal geometry gives equal digests.
    pub fn digest(&self) -> String {
        let mut digests: Vec<u64> = self.meshes.iter().map(mesh_digest).collect();
        digests.sort_unstable();
        let hash = digests.iter().fold(FNV_OFFSET_BASIS, |h, d| fnv1a(&d.to_le_bytes(), h));
        format!("{hash:016x}")
    }
}
