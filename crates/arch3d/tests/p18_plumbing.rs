//! Phase 18 scene plumbing: the new sections, the cabinet hooks and the cabinet neighbour
//! projections. Every stub builds nothing, so a model without phase-18 objects builds exactly what
//! it built before.

use cadcraft_arch3d::{Scene3d, SceneCache};
use cadcraft_doc::arch::{
    Backsplash, Cabinet, CabinetKind, CabinetMolding, Countertop, FrontStyle, Fronts, Furnishing, FurnishingHost, Level, MoldingLine, Room,
    RoomMolding, SymbolMesh, SymbolPart,
};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::Vec2;

const CUBE: &str = "00000000000c0be1";

/// The digest of [`kitchen`] built by `Scene3d::build`. Every phase-18 builder is a stub that
/// builds nothing, so a drift of this value means the plumbing changed an existing mesh.
const KITCHEN_DIGEST: &str = "7f17dd19db30bc9a";

fn cube_part() -> SymbolPart {
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([0.0, 0.0, -1.0], [[-0.5, -0.5, 0.0], [-0.5, 0.5, 0.0], [0.5, 0.5, 0.0], [0.5, -0.5, 0.0]]),
        ([0.0, 0.0, 1.0], [[-0.5, -0.5, 1.0], [0.5, -0.5, 1.0], [0.5, 0.5, 1.0], [-0.5, 0.5, 1.0]]),
        ([0.0, -1.0, 0.0], [[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.5, -0.5, 1.0], [-0.5, -0.5, 1.0]]),
        ([0.0, 1.0, 0.0], [[0.5, 0.5, 0.0], [-0.5, 0.5, 0.0], [-0.5, 0.5, 1.0], [0.5, 0.5, 1.0]]),
        ([-1.0, 0.0, 0.0], [[-0.5, 0.5, 0.0], [-0.5, -0.5, 0.0], [-0.5, -0.5, 1.0], [-0.5, 0.5, 1.0]]),
        ([1.0, 0.0, 0.0], [[0.5, -0.5, 0.0], [0.5, 0.5, 0.0], [0.5, 0.5, 1.0], [0.5, -0.5, 1.0]]),
    ];
    let mut part = SymbolPart { name: "cube".into(), base_color: [0.6, 0.6, 0.6, 1.0], roughness: 0.5, ..Default::default() };
    for (normal, corners) in faces {
        let base = part.positions.len() as u32;
        for (corner, uv) in corners.iter().zip([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]) {
            part.positions.push(*corner);
            part.normals.push(normal);
            part.uvs.push(uv);
        }
        part.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    part
}

fn wall(id: u32, a: [f64; 2], b: [f64; 2]) -> Wall {
    Wall { id: ArchId(id), start: Vec2::new(a[0], a[1]), end: Vec2::new(b[0], b[1]), thickness: 0.2, height: 2.4, ..Default::default() }
}

fn base_cabinet(x: f64, width: f64) -> Cabinet {
    Cabinet {
        kind: CabinetKind::Base,
        position: Vec2::new(x, 0.5),
        width,
        depth: 0.6,
        height: 0.876,
        doors: 1,
        counter: true,
        counter_thickness: 0.038,
        counter_overhang: 0.025,
        toe_kick: true,
        ..Default::default()
    }
}

fn wall_cabinet(x: f64) -> Cabinet {
    Cabinet {
        kind: CabinetKind::Wall,
        elevation: 1.4,
        height: 0.7,
        depth: 0.33,
        counter: false,
        pull: Some("bar-pull".into()),
        pull_symbol: Some(CUBE.into()),
        ..base_cabinet(x, 0.6)
    }
}

fn furnishing(item: &str, at: [f64; 2], size: [f64; 3], host: FurnishingHost, cutout: Option<[f64; 2]>) -> Furnishing {
    Furnishing { item: item.into(), symbol: CUBE.into(), at, size, host, cutout, elevation: 0.9, ..Default::default() }
}

/// A phase-14 kitchen: walls only (no rooms), a base run with a hosted sink cut-out, a dishwasher in
/// a bridged gap, wall cabinets with pulls and a tall cabinet with a counter.
fn kitchen() -> ArchModel {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    m.symbols.insert(CUBE.into(), SymbolMesh { key: CUBE.into(), parts: vec![cube_part()], ..Default::default() });
    m.walls =
        vec![wall(0, [0.0, 0.0], [6.0, 0.0]), wall(1, [6.0, 0.0], [6.0, 4.0]), wall(2, [6.0, 4.0], [0.0, 4.0]), wall(3, [0.0, 4.0], [0.0, 0.0])];
    m.next_id = 4;
    m.push_cabinet(base_cabinet(1.0, 0.6));
    m.push_cabinet(base_cabinet(1.6, 0.6));
    m.push_cabinet(base_cabinet(3.0, 0.6));
    m.push_cabinet(wall_cabinet(1.0));
    m.push_cabinet(wall_cabinet(1.6));
    m.push_cabinet(Cabinet { kind: CabinetKind::Tall, height: 2.1, counter: true, ..base_cabinet(4.0, 0.6) });
    m.push_furnishing(furnishing("sink", [1.0, 0.8], [0.5, 0.4, 0.2], FurnishingHost::Cabinet { cabinet: ArchId(0) }, Some([0.45, 0.35])));
    m.push_furnishing(furnishing("dishwasher", [2.3, 0.8], [0.6, 0.6, 0.85], FurnishingHost::Gap { left: ArchId(1), right: ArchId(2) }, None));
    m
}

/// Every phase-18 object, on top of [`kitchen`].
fn with_phase18(mut m: ArchModel) -> ArchModel {
    m.rooms.push(Room {
        id: ArchId(0),
        polygon: vec![Vec2::new(0.1, 0.1), Vec2::new(5.9, 0.1), Vec2::new(5.9, 3.9), Vec2::new(0.1, 3.9)],
        moldings: vec![RoomMolding::default()],
        ..Default::default()
    });
    m.next_room_id = 1;
    m.push_molding_line(MoldingLine { points: vec![Vec2::new(1.0, 1.0), Vec2::new(3.0, 1.0)], ..Default::default() });
    m.push_countertop(Countertop {
        polygon: vec![Vec2::new(2.0, 2.0), Vec2::new(3.0, 2.0), Vec2::new(3.0, 3.0), Vec2::new(2.0, 3.0)],
        ..Default::default()
    });
    m.push_backsplash(Backsplash { wall: Some(ArchId(0)), ..Default::default() });
    m
}

fn owners(s: &Scene3d) -> Vec<(String, ArchId)> {
    s.meshes.iter().map(|m| (m.material.clone(), m.object)).collect()
}

#[test]
fn a_model_without_phase18_objects_builds_the_same_digest() {
    assert_eq!(Scene3d::build(&kitchen()).digest(), KITCHEN_DIGEST);
}

#[test]
fn new_sections_are_empty_with_stubs() {
    let plain = kitchen();
    let full = with_phase18(kitchen());
    let (a, b) = (Scene3d::build(&plain), Scene3d::build(&full));
    // The room (for its moldings) adds a floor and a ceiling; nothing else.
    let without_room: Vec<_> =
        owners(&b).into_iter().filter(|(material, _)| material != "floor" && material != "ceiling" && material != "countertop").collect();
    assert_eq!(owners(&a), without_room);
    assert!(b.meshes.iter().all(|m| !m.material.starts_with("molding-") && m.material != "backsplash"), "the stubs build nothing");
}

#[test]
fn cache_equals_full_with_countertops_backsplashes_and_molding_lines_present() {
    let mut model = kitchen();
    let (mut cache, _) = SceneCache::new(&model);
    model = with_phase18(model);
    let stats = cache.update(&model);
    assert!(stats.incremental);
    assert_eq!(cache.scene().digest(), Scene3d::build(&model).digest());
    assert_eq!(owners(cache.scene()), owners(&Scene3d::build(&model)));
    // Ids and diffs still flow: an edit of each new object is an incremental update that matches.
    model.countertops[0].top = 0.95;
    model.backsplashes[0].height = 0.5;
    model.molding_lines[0].elevation = 0.1;
    model.rooms[0].moldings[0].offset = 0.1;
    let stats = cache.update(&model);
    assert!(stats.incremental);
    assert_eq!(cache.scene().digest(), Scene3d::build(&model).digest());
    // And removing them again.
    model.countertops.clear();
    model.backsplashes.clear();
    model.molding_lines.clear();
    model.rooms[0].moldings.clear();
    cache.update(&model);
    assert_eq!(cache.scene().digest(), Scene3d::build(&model).digest());
}

#[test]
fn cabinet_meshes_unchanged_through_the_hooks() {
    let model = kitchen();
    let scene = Scene3d::build(&model);
    let materials_of = |id: u32| -> Vec<String> {
        scene
            .meshes
            .iter()
            .filter(|m| m.object == ArchId(id) && (m.material.starts_with("cabinet-") || m.material == "counter" || m.material.starts_with("pull:")))
            .map(|m| m.material.clone())
            .collect()
    };
    // A base cabinet: carcass and front; the run owner (cb0) adds the run's counter, the cut-out sink's.
    assert_eq!(materials_of(0), ["cabinet-box", "cabinet-front", "counter"]);
    assert_eq!(materials_of(1), ["cabinet-box", "cabinet-front"]);
    // A wall cabinet with a pull; a tall cabinet with its own counter.
    assert_eq!(materials_of(3), ["cabinet-box", "cabinet-front", "pull:00000000000c0be1:0"]);
    assert_eq!(materials_of(5), ["cabinet-box", "cabinet-front", "counter"]);
    // The same through the public entry point.
    for c in &model.cabinets {
        let own: Vec<_> =
            scene.meshes.iter().filter(|m| m.object == c.id && (m.material.starts_with("cabinet-") || m.material == "counter")).collect();
        let direct = cadcraft_arch3d::casework::cabinet_meshes(&model, c);
        assert_eq!(own.len(), direct.iter().filter(|m| m.material.starts_with("cabinet-") || m.material == "counter").count());
    }
}

#[test]
fn a_filler_that_owns_its_run_builds_the_counter() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    // A 0.6 base cabinet (u 0.7..1.3) and a 0.05 filler touching its start end (u 0.65..0.7): the
    // filler has the lowest u, so it owns the run.
    m.push_cabinet(base_cabinet(1.0, 0.6));
    m.push_cabinet(Cabinet { filler: true, fronts: Fronts::Open, ..base_cabinet(0.675, 0.05) });
    let scene = Scene3d::build(&m);
    let counters: Vec<_> = scene.meshes.iter().filter(|mesh| mesh.material == "counter").collect();
    assert_eq!(counters.len(), 1, "{:?}", owners(&scene));
    let counter = counters[0];
    assert_eq!(counter.object, ArchId(1), "the filler owns the run");
    let b = counter.bounds();
    assert!((b.max[0] - b.min[0] - 0.65).abs() < 1e-4, "{b:?}");
    assert!((b.max[1] - b.min[1] - 0.625).abs() < 1e-4, "{b:?}");
}

#[test]
fn cabinet_neighbours_are_projections() {
    let mut m = kitchen();
    // Three base and two wall cabinets (and no tall one).
    m.cabinets.retain(|c| c.kind != CabinetKind::Tall);
    m.furnishings.clear();
    m.push_countertop(Countertop {
        polygon: vec![Vec2::new(2.0, 2.0), Vec2::new(3.0, 2.0), Vec2::new(3.0, 3.0), Vec2::new(2.0, 3.0)],
        ..Default::default()
    });
    let (mut cache, _) = SceneCache::new(&m);
    let check = |cache: &SceneCache, m: &ArchModel, what: &str| {
        assert_eq!(cache.scene().digest(), Scene3d::build(m).digest(), "{what}");
        assert_eq!(owners(cache.scene()), owners(&Scene3d::build(m)), "{what}");
    };

    // A base cabinet's door style is no projected field: it rebuilds itself alone.
    m.cabinets[0].door_style = FrontStyle::Shaker;
    assert_eq!(cache.update(&m).rebuilt, 1);
    check(&cache, &m, "door style");

    // A wall cabinet's crown (projected as a flag) rebuilds the wall and tall cabinets only.
    m.cabinets[3].crown = Some(CabinetMolding::default());
    assert_eq!(cache.update(&m).rebuilt, 2);
    check(&cache, &m, "crown toggle");

    // A countertop's polygon is read by no cabinet mesh: only the countertop rebuilds.
    m.countertops[0].polygon[2] = Vec2::new(3.5, 3.2);
    assert_eq!(cache.update(&m).rebuilt, 1);
    check(&cache, &m, "countertop polygon");

    // Claiming a cabinet changes the claimed ids of the level: every cabinet on it is rebuilt.
    m.countertops[0].cabinets.push(ArchId(0));
    let level = m.cabinets.len();
    assert_eq!(cache.update(&m).rebuilt, level);
    check(&cache, &m, "claimed cabinet");
}
