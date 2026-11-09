//! Phase 16 ceiling build-up: a room's ceiling height is its rough ceiling, its finish stack hangs
//! below it layer by layer (an air gap holds nothing), and the bottom layer is the ceiling people
//! see. An empty stack keeps today's 20 mm ceiling solid.

use cadcraft_arch::ceiling::stack::template;
use cadcraft_arch3d::floors;
use cadcraft_arch3d::probe::part_of;
use cadcraft_arch3d::{Mesh, Scene3d, SceneCache};
use cadcraft_doc::arch::{CeilingLayer, Level, LightFixture, MaterialRef, Paint, Surface};
use cadcraft_doc::{ArchId, ArchModel, Room, Wall};
use cadcraft_geom::{Vec2, Vec3};

const S: usize = 0;
const NW: usize = 1;
const NE: usize = 2;

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
    vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
}

/// The phase-1 house: four exterior walls 2.4 m high on a 10 x 8 m rectangle, interior walls on
/// y = 3 and x = 6, and its three rooms S (r0), NW (r1) and NE (r2) under the level's 2.4 m
/// ceiling. One level, no roof.
fn house() -> ArchModel {
    let mut m = ArchModel::default();
    m.ensure_ground_level();
    let wall = |a: (f64, f64), b: (f64, f64), thickness: f64, exterior: bool| Wall {
        start: Vec2::new(a.0, a.1),
        end: Vec2::new(b.0, b.1),
        thickness,
        height: 2.4,
        exterior,
        ..Default::default()
    };
    for (a, b) in [((0.0, 0.0), (10.0, 0.0)), ((10.0, 0.0), (10.0, 8.0)), ((10.0, 8.0), (0.0, 8.0)), ((0.0, 8.0), (0.0, 0.0))] {
        m.push_wall(wall(a, b, 0.2, true));
    }
    m.push_wall(wall((0.0, 3.0), (10.0, 3.0), 0.1, false));
    m.push_wall(wall((6.0, 3.0), (6.0, 8.0), 0.1, false));
    m.push_room(Room { name: "South".into(), polygon: rect(0.1, 0.1, 9.9, 2.95), ..Default::default() });
    m.push_room(Room { name: "North-west".into(), polygon: rect(0.1, 3.05, 5.95, 7.9), ..Default::default() });
    m.push_room(Room { name: "North-east".into(), polygon: rect(6.05, 3.05, 9.9, 7.9), ..Default::default() });
    m
}

/// The house with two recessed pot lights in the north-west room and a pendant in the north-east.
fn lit_house() -> ArchModel {
    let mut m = house();
    for (n, (name, x, y)) in [("Recessed pot light", 2.0, 5.0), ("Recessed pot light", 4.0, 6.0), ("Pendant", 8.0, 5.0)].into_iter().enumerate() {
        let (id, p) = (ArchId(n as u32), Vec2::new(x, y));
        let t = cadcraft_arch::light::find_type(name);
        m.lights.push(t.as_ref().map(|t| cadcraft_arch::light::fixture_from_type(id, t, p)).unwrap_or_else(|| LightFixture::new(id, name, p)));
    }
    m.next_light_id = 3;
    m
}

fn stack(name: &str) -> Vec<CeilingLayer> {
    template(name).unwrap_or_else(|| panic!("the {name} template"))
}

fn same(a: &Mesh, b: &Mesh) -> bool {
    a.positions == b.positions
        && a.normals == b.normals
        && a.uvs == b.uvs
        && a.indices == b.indices
        && a.material == b.material
        && a.object == b.object
        && a.level_elevation.to_bits() == b.level_elevation.to_bits()
}

/// The meshes of room `r` whose part is `ceiling`, in scene order.
fn ceiling_meshes(s: &Scene3d, r: usize) -> Vec<&Mesh> {
    s.meshes.iter().filter(|m| m.object == ArchId(r as u32) && part_of(m) == "ceiling").collect()
}

fn z_range(m: &Mesh) -> (f64, f64) {
    let b = m.bounds();
    (b.min[2], b.max[2])
}

fn assert_z_range(m: &Mesh, lo: f64, hi: f64, what: &str) {
    let (a, b) = z_range(m);
    assert!((a - lo).abs() < 1e-6 && (b - hi).abs() < 1e-6, "{what}: {a}..{b}, expected {lo}..{hi}");
}

/// What probing the scene at `(x, y, z)` hits: `(object, part)`.
fn hit(s: &Scene3d, x: f64, y: f64, z: f64) -> Option<(String, String)> {
    s.probe(Vec3::new(x, y, z)).map(|h| (h.id, h.part))
}

fn ceiling_of(r: usize) -> Option<(String, String)> {
    Some((format!("r{r}"), "ceiling".to_string()))
}

/// What a cached scene must match: the digest, every mesh's material and owner in order, and the
/// counts.
fn signature(s: &Scene3d) -> (String, Vec<(String, u32)>, [usize; 5]) {
    (
        s.digest(),
        s.meshes.iter().map(|m| (m.material.clone(), m.object.0)).collect(),
        [s.floors, s.ceilings, s.fixtures, s.lights.len(), s.meshes.len()],
    )
}

#[test]
fn empty_stack_keeps_the_20mm_solid() {
    let m = house();
    let s = Scene3d::build(&m);
    let level: Level = m.levels.first().cloned().unwrap_or_default();
    for (r, room) in m.rooms.iter().enumerate() {
        let want = floors::ceiling_surface(room, &level, "ceiling").expect("today's ceiling");
        let got = ceiling_meshes(&s, r);
        assert_eq!(got.len(), 1, "room {r}");
        assert!(same(got[0], &want), "room {r}: the mesh is today's 20 mm solid");
    }
    assert_eq!(hit(&s, 5.0, 1.5, 2.39), ceiling_of(S));
    assert_eq!(hit(&s, 5.0, 1.5, 2.41), None);
    assert_eq!(hit(&s, 5.0, 1.5, 2.37), None);
    // A stack set and cleared again builds exactly the same scene.
    let mut m2 = m.clone();
    m2.rooms[NE].ceiling_stack = stack("suspended");
    m2.rooms[NE].ceiling_stack = stack("none");
    assert_eq!(Scene3d::build(&m2).digest(), s.digest());
}

#[test]
fn suspended_stack_builds_tile_and_drywall_and_leaves_the_gap() {
    let mut m = house();
    m.rooms[NE].ceiling_stack = stack("Suspended");
    let s = Scene3d::build(&m);
    // The tile 2.1713..2.1873, the air gap 2.1873..2.3873, the drywall 2.3873..2.4.
    assert_eq!(hit(&s, 8.0, 5.0, 2.18), ceiling_of(NE), "the tile");
    assert_eq!(hit(&s, 8.0, 5.0, 2.3), None, "the air gap");
    assert_eq!(hit(&s, 8.0, 5.0, 2.39), ceiling_of(NE), "the drywall");
    assert_eq!(hit(&s, 8.0, 5.0, 2.165), None, "below the finished ceiling");
    assert_eq!(hit(&s, 8.0, 5.0, 2.41), None, "above the rough ceiling");
    let meshes = ceiling_meshes(&s, NE);
    let names: Vec<&str> = meshes.iter().map(|m| m.material.as_str()).collect();
    assert_eq!(names, vec!["ceiling", "ceiling-layer"], "the bottom layer first");
    assert_z_range(meshes[0], 2.4 - 0.2287, 2.4 - 0.2127, "the tile");
    assert_z_range(meshes[1], 2.4 - 0.0127, 2.4, "the drywall");
    assert!(meshes.iter().all(|m| m.is_manifold() && m.level_elevation == 0.0));
    // The other rooms keep the 20 mm solid.
    assert_eq!(hit(&s, 3.0, 5.0, 2.39), ceiling_of(NW));
    assert_eq!(hit(&s, 5.0, 1.5, 2.39), ceiling_of(S));
    // The explicit stack of the same layers builds the same solids.
    let mut explicit = house();
    explicit.rooms[NE].ceiling_stack =
        vec![CeilingLayer::new("", Some("drywall"), 0.0127), CeilingLayer::new("", None, 0.2), CeilingLayer::new("", Some("ceiling-tile"), 0.016)];
    let e = Scene3d::build(&explicit);
    let (a, b) = (ceiling_meshes(&e, NE), meshes);
    assert!(a.len() == 2 && a.iter().zip(&b).all(|(x, y)| same(x, y)));
}

#[test]
fn drywall_template_is_one_layer() {
    let mut m = house();
    m.rooms[NW].ceiling_stack = stack("DRYWALL");
    let s = Scene3d::build(&m);
    let meshes = ceiling_meshes(&s, NW);
    assert_eq!(meshes.len(), 1);
    assert_eq!(meshes[0].material, "ceiling");
    assert_z_range(meshes[0], 2.3873, 2.4, "the drywall");
    assert_eq!(hit(&s, 3.0, 5.0, 2.39), ceiling_of(NW));
    assert_eq!(hit(&s, 3.0, 5.0, 2.38), None, "12.7 mm, not today's 20 mm");
}

#[test]
fn stack_meshes_keep_one_ceiling_per_room() {
    let mut m = house();
    m.rooms[NW].ceiling_stack = stack("drywall");
    m.rooms[NE].ceiling_stack = stack("suspended");
    let s = Scene3d::build(&m);
    assert_eq!((s.floors, s.ceilings), (3, 3));
    assert_eq!(s.meshes.iter().filter(|x| part_of(x) == "ceiling").count(), 4, "S 1, NW 1, NE 2");
    // Through the cache too.
    let (cache, _) = SceneCache::new(&m);
    assert_eq!((cache.scene().floors, cache.scene().ceilings), (3, 3));
}

#[test]
fn painted_ceiling_paints_the_bottom_layer() {
    let mut m = house();
    m.rooms[NE].ceiling_stack = stack("suspended");
    m.paints
        .insert(Surface::RoomCeiling(ArchId(NE as u32)).key(), Paint { material: Some(MaterialRef::Colour([200, 180, 160])), ..Default::default() });
    let s = Scene3d::build(&m);
    let meshes = ceiling_meshes(&s, NE);
    let names: Vec<&str> = meshes.iter().map(|m| m.material.as_str()).collect();
    assert_eq!(names, vec!["paint:room/r2/ceiling", "ceiling-layer"]);
    assert_z_range(meshes[0], 2.1713, 2.1873, "the painted tile");
    assert_z_range(meshes[1], 2.3873, 2.4, "the unpainted drywall");
    assert_eq!(hit(&s, 8.0, 5.0, 2.18), ceiling_of(NE));
    assert_eq!(s.ceilings, 3);
}

#[test]
fn cache_equals_full_after_stack_edits() {
    let mut m = lit_house();
    let (mut cache, full) = SceneCache::new(&m);
    assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)));
    assert!(!full.incremental);
    type Edit = Box<dyn Fn(&mut ArchModel)>;
    let edits: Vec<(&str, Edit)> = vec![
        ("suspend NE", Box::new(|m: &mut ArchModel| m.rooms[NE].ceiling_stack = stack("suspended"))),
        ("drywall NW (its pot lights)", Box::new(|m: &mut ArchModel| m.rooms[NW].ceiling_stack = stack("Drywall"))),
        (
            "NE: the explicit layers",
            Box::new(|m: &mut ArchModel| {
                m.rooms[NE].ceiling_stack = vec![
                    CeilingLayer::new("", Some("drywall"), 0.0127),
                    CeilingLayer::new("", None, 0.2),
                    CeilingLayer::new("", Some("material-oak"), 0.016),
                ]
            }),
        ),
        ("NE: a higher rough ceiling", Box::new(|m: &mut ArchModel| m.rooms[NE].ceiling_height = Some(2.7))),
        (
            "paint NE's ceiling",
            Box::new(|m: &mut ArchModel| {
                m.paints.insert(
                    Surface::RoomCeiling(ArchId(NE as u32)).key(),
                    Paint { material: Some(MaterialRef::Colour([90, 90, 200])), ..Default::default() },
                );
            }),
        ),
        ("a thicker plenum", Box::new(|m: &mut ArchModel| m.rooms[NE].ceiling_stack[1].thickness = 0.3)),
        (
            "S: hostile layers",
            Box::new(|m: &mut ArchModel| {
                m.rooms[S].ceiling_stack = vec![CeilingLayer::new("x", Some("drywall"), f64::NAN), CeilingLayer::new("y", Some("drywall"), 0.0)]
            }),
        ),
        ("move a pot light", Box::new(|m: &mut ArchModel| m.lights[0].position.x += 0.3)),
        ("clear NW", Box::new(|m: &mut ArchModel| m.rooms[NW].ceiling_stack.clear())),
        ("clear NE", Box::new(|m: &mut ArchModel| m.rooms[NE].ceiling_stack = stack("none"))),
    ];
    for (what, edit) in edits {
        edit(&mut m);
        let stats = cache.update(&m);
        let want = Scene3d::build(&m);
        assert_eq!(signature(cache.scene()), signature(&want), "{what}: the cache is not the full build; {stats:?}");
        assert!(stats.incremental && stats.rebuilt <= stats.objects, "{what}: {stats:?}");
    }
    // A NaN layer never equals itself, so the cache rebuilds S on every update while it holds
    // one (still equal to the full build). Once S is clean, an update without an edit rebuilds
    // nothing.
    let stats = cache.update(&m);
    assert!(stats.rebuilt <= 1 && signature(cache.scene()) == signature(&Scene3d::build(&m)), "{stats:?}");
    m.rooms[S].ceiling_stack.clear();
    cache.update(&m);
    assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)));
    assert_eq!(cache.update(&m).rebuilt, 0);
}

#[test]
fn a_stack_edit_rebuilds_only_its_room_and_what_hangs_from_it() {
    let mut m = lit_house();
    let (mut cache, _) = SceneCache::new(&m);
    m.rooms[S].ceiling_stack = stack("suspended");
    let stats = cache.update(&m);
    assert!(stats.incremental && stats.rebuilt == 1, "only S: {stats:?}");
    assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)));
}

#[test]
fn templates_in_any_case_build_the_same_scene() {
    let digest = |name: &str| {
        let mut m = lit_house();
        m.rooms[NE].ceiling_stack = stack(name);
        let (mut cache, _) = SceneCache::new(&house());
        cache.update(&m);
        assert_eq!(cache.scene().digest(), Scene3d::build(&m).digest());
        cache.scene().digest()
    };
    let want = digest("suspended");
    for spelled in ["Suspended", "SUSPENDED", " suspended ", "sUsPeNdEd"] {
        assert_eq!(digest(spelled), want, "{spelled}");
    }
    assert_ne!(want, digest("drywall"));
    assert_ne!(want, digest("none"));
}

#[test]
fn hostile_layers_are_skipped() {
    // A loaded file's room with layers no command would store: zero, negative and missing
    // thicknesses, an air gap at the bottom, and an unknown field.
    let mut saved = serde_json::to_value(&house().rooms[NE]).expect("a room saves");
    saved["ceiling_stack"] = serde_json::json!([
        {"name": "zero", "material": "drywall", "thickness": 0.0},
        {"name": "negative", "material": "drywall", "thickness": -0.5},
        {"name": "missing", "material": "drywall"},
        {"name": "tile", "material": "ceiling-tile", "thickness": 0.016},
        {"name": "air", "material": null, "thickness": 0.1, "colour": "blue"}
    ]);
    let room: Room = serde_json::from_value(saved).expect("the room loads");
    assert_eq!(room.ceiling_stack.len(), 5);
    let mut m = house();
    m.rooms[NE] = room;
    m.rooms[NE].ceiling_stack.insert(0, CeilingLayer::new("nan", Some("drywall"), f64::NAN));
    let s = Scene3d::build(&m);
    let meshes = ceiling_meshes(&s, NE);
    assert_eq!(meshes.len(), 1, "only the tile");
    assert_eq!(meshes[0].material, "ceiling");
    assert_z_range(meshes[0], 2.384, 2.4, "the tile, right under the rough ceiling");
    assert_eq!(s.ceilings, 3);
    // Nothing valid left: the empty stack's 20 mm solid.
    let mut m = house();
    m.rooms[NE].ceiling_stack = vec![CeilingLayer::new("nan", Some("drywall"), f64::NAN), CeilingLayer::new("inf", None, f64::INFINITY)];
    let s = Scene3d::build(&m);
    let level: Level = m.levels.first().cloned().unwrap_or_default();
    let want = floors::ceiling_surface(&m.rooms[NE], &level, "ceiling").expect("today's ceiling");
    let got = ceiling_meshes(&s, NE);
    assert!(got.len() == 1 && same(got[0], &want));
    // Huge layers, ten thousand layers, an air-only stack: no panic, every mesh finite, at
    // most MAX_CEILING_LAYERS layer meshes.
    for st in [
        vec![CeilingLayer::new("a", Some("drywall"), 1e300), CeilingLayer::new("b", Some("drywall"), f64::MAX)],
        vec![CeilingLayer::new("a", Some("drywall"), 0.01); 10_000],
        vec![CeilingLayer::new("air", None, 0.2)],
    ] {
        let mut m = lit_house();
        m.rooms[NE].ceiling_stack = st;
        let s = Scene3d::build(&m);
        assert!(s.meshes.iter().all(|x| x.positions.iter().flatten().all(|v| v.is_finite())));
        assert!(ceiling_meshes(&s, NE).len() <= cadcraft_arch::ceiling::stack::MAX_CEILING_LAYERS);
        let (mut cache, _) = SceneCache::new(&house());
        cache.update(&m);
        assert_eq!(signature(cache.scene()), signature(&s));
    }
}
