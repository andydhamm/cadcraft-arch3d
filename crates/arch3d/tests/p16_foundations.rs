//! Phase 16 stem foundations: a 0.2 m concrete wall flush with the outer face of every exterior
//! wall on grade, from the floor structure's underside down to a 0.6 x 0.25 m footing whose bottom
//! is `footing_depth` below the level's floor; a basement's walls get the footing directly under
//! the basement's floor structure. The numbers are the brief's (phase-1 house, level 0 at 0,
//! structure 0.3).

use cadcraft_arch3d::hlr::OrthoView;
use cadcraft_arch3d::probe::part_of;
use cadcraft_arch3d::section::{Plane, section_cut};
use cadcraft_arch3d::{Mesh, Scene3d, SceneCache};
use cadcraft_doc::arch::{ArchKind, Foundation, FoundationKind, Level, Terrain};
use cadcraft_doc::{ArchId, ArchModel, Room, Wall};
use cadcraft_geom::{Vec2, Vec3};

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
    vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
}

fn wall(a: (f64, f64), b: (f64, f64), thickness: f64, exterior: bool, level: u32) -> Wall {
    Wall { start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness, height: 2.4, exterior, base_level: level, ..Default::default() }
}

fn ring(m: &mut ArchModel, x0: f64, y0: f64, x1: f64, y1: f64, level: u32) {
    for (a, b) in [((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))] {
        m.push_wall(wall(a, b, 0.2, true, level));
    }
}

/// The phase-1 house: the ring w0 (south) w1 (east) w2 (north) w3 (west), interior walls w4 and
/// w5, rooms S, NW and NE on level 0 (at 0, ceiling 2.4, structure 0.3).
fn house() -> ArchModel {
    let mut m = ArchModel::default();
    ring(&mut m, 0.0, 0.0, 10.0, 8.0, 0);
    m.push_wall(wall((0.0, 3.0), (10.0, 3.0), 0.1, false, 0));
    m.push_wall(wall((6.0, 3.0), (6.0, 8.0), 0.1, false, 0));
    for poly in [rect(0.1, 0.1, 9.9, 2.95), rect(0.1, 3.05, 5.95, 7.9), rect(6.05, 3.05, 9.9, 7.9)] {
        m.push_room(Room { level: 0, polygon: poly, ..Default::default() });
    }
    m
}

fn add_level(m: &mut ArchModel, name: &str, elevation: f64) -> u32 {
    let index = m.levels.len() as u32;
    m.levels.push(Level { index, name: name.into(), elevation, ..Default::default() });
    index
}

/// The house with a basement at -2.7 (structure 0.3): the same ring on it and one room.
fn with_basement() -> (ArchModel, u32) {
    let mut m = house();
    let b = add_level(&mut m, "Basement", -2.7);
    ring(&mut m, 0.0, 0.0, 10.0, 8.0, b);
    m.push_room(Room { level: b, polygon: rect(0.1, 0.1, 9.9, 7.9), ..Default::default() });
    (m, b)
}

fn stem(level: u32, depth: f64) -> Foundation {
    Foundation { level, kind: FoundationKind::Stem, footing_depth: depth, ..Default::default() }
}

fn hit(s: &Scene3d, x: f64, y: f64, z: f64) -> Option<(String, String)> {
    s.probe(Vec3::new(x, y, z)).map(|h| (h.id, h.part))
}

fn part_meshes<'a>(s: &'a Scene3d, part: &str) -> Vec<&'a Mesh> {
    s.meshes.iter().filter(|m| part_of(m) == part).collect()
}

/// What a cached scene must match: the digest, every mesh's material and owner in order, and the
/// counts.
fn signature(s: &Scene3d) -> (String, Vec<(String, u32)>, [usize; 4]) {
    (s.digest(), s.meshes.iter().map(|m| (m.material.clone(), m.object.0)).collect(), [s.floors, s.ceilings, s.roofs, s.decks])
}

#[test]
fn stem_and_footing_probes() {
    let mut m = house();
    m.push_foundation(stem(0, 1.2));
    let s = Scene3d::build(&m);
    let fd = Some(("fd0".to_string(), "foundation".to_string()));
    let ft = Some(("fd0".to_string(), "footing".to_string()));
    // The brief's numbers under the south wall.
    assert_eq!(hit(&s, 5.0, 0.0, -0.5), fd, "the stem wall");
    assert_eq!(hit(&s, 5.0, 0.25, -1.0), ft, "the footing");
    assert_eq!(hit(&s, 5.0, 0.25, -0.5), None, "beside the stem, above the footing");
    assert_eq!(hit(&s, 5.0, 0.35, -1.0), None, "past the footing's inner edge");
    // The stem: 0.2 m from the outer face (y -0.1..0.1), from the footing's top (-0.95) up to the
    // floor structure's underside (-0.3); the footing from -1.2 to -0.95, y -0.3..0.3.
    assert_eq!(hit(&s, 5.0, -0.09, -0.32), fd);
    assert_eq!(hit(&s, 5.0, 0.09, -0.93), fd);
    assert_ne!(hit(&s, 5.0, 0.0, -0.28), fd, "the stem stops under the floor structure");
    assert_eq!(hit(&s, 5.0, -0.11, -0.5), None, "flush with the wall's outer face");
    assert_eq!(hit(&s, 5.0, -0.29, -1.19), ft);
    assert_eq!(hit(&s, 5.0, 0.0, -1.21), None, "the footing's bottom is 1.2 below the floor");
    // One stem and one footing per wall, each its own closed mesh on the foundation's level.
    let (stems, footings) = (part_meshes(&s, "foundation"), part_meshes(&s, "footing"));
    assert_eq!((stems.len(), footings.len()), (4, 4));
    for x in stems.iter().chain(footings.iter()) {
        assert!(x.is_manifold(), "{}", x.material);
        assert_eq!((x.object, x.level_elevation), (ArchId(0), 0.0));
    }
    // A deeper footing: (5,0.25,-1.4) is in it and (5,0.25,-1.1) is above it.
    m.foundations[0].footing_depth = 1.5;
    let s = Scene3d::build(&m);
    assert_eq!(hit(&s, 5.0, 0.25, -1.4), ft);
    assert_eq!(hit(&s, 5.0, 0.25, -1.1), None);
    assert_eq!(hit(&s, 5.0, 0.0, -1.1), fd, "the stem reaches down to the deeper footing");

    // A studio on grade at 1.35 gets its own foundation: stem 0.4..1.05, footing 0.15..0.4.
    let mut m = house();
    let st = add_level(&mut m, "Studio", 1.35);
    ring(&mut m, 20.0, 0.0, 24.0, 4.0, st);
    m.push_foundation(stem(0, 1.2));
    m.push_foundation(stem(st, 1.2));
    let s = Scene3d::build(&m);
    assert_eq!(hit(&s, 22.0, 0.0, 0.7), Some(("fd1".to_string(), "foundation".to_string())));
    assert_eq!(hit(&s, 22.0, 0.25, 0.3), Some(("fd1".to_string(), "footing".to_string())));
    assert_eq!(hit(&s, 22.0, 0.0, 1.1), None, "under the studio's floor structure (1.05..1.35) the stem stops");
    assert_eq!(hit(&s, 22.0, 0.25, 0.1), None);
    assert_eq!((part_meshes(&s, "foundation").len(), part_meshes(&s, "footing").len()), (8, 8));
}

#[test]
fn basement_footing_under_the_floor_structure() {
    let (mut m, b) = with_basement();
    m.push_foundation(stem(b, 1.2));
    let s = Scene3d::build(&m);
    // Under the basement's floor structure (-3.0): -3.25..-3.0, centred under the wall.
    assert_eq!(hit(&s, 5.0, 0.25, -3.1), Some(("fd0".to_string(), "footing".to_string())));
    assert_eq!(hit(&s, 5.0, -0.25, -3.2), Some(("fd0".to_string(), "footing".to_string())));
    assert_eq!(hit(&s, 5.0, 0.0, -3.3), None);
    assert_eq!(hit(&s, 5.0, 0.35, -3.1), None);
    assert!(part_meshes(&s, "foundation").is_empty(), "the basement's own walls are its foundation walls");
    let footings = part_meshes(&s, "footing");
    assert_eq!(footings.len(), 4);
    assert!(footings.iter().all(|x| x.is_manifold() && x.level_elevation == -2.7));
    // A foundation under level 0 has nothing to stand under: the basement is below it.
    let (mut m, _) = with_basement();
    m.push_foundation(stem(0, 1.2));
    let s = Scene3d::build(&m);
    assert!(part_meshes(&s, "foundation").is_empty() && part_meshes(&s, "footing").is_empty());
}

#[test]
fn corners_overlap_in_separate_meshes_without_breaking_probes() {
    let mut m = house();
    m.push_foundation(stem(0, 1.2));
    let s = Scene3d::build(&m);
    let fd = Some(("fd0".to_string(), "foundation".to_string()));
    let ft = Some(("fd0".to_string(), "footing".to_string()));
    // The south-west corner is inside two stems and two footings at once.
    assert_eq!(hit(&s, -0.05, -0.05, -0.5), fd);
    assert_eq!(hit(&s, 0.05, 0.05, -0.5), fd);
    assert_eq!(hit(&s, -0.25, -0.25, -1.0), ft);
    assert_eq!(hit(&s, 0.25, 0.25, -1.0), ft);
    assert_eq!(hit(&s, -0.35, -0.25, -1.0), None);
    // Every corner, the same.
    for (x, y) in [(10.05, -0.05), (10.05, 8.05), (-0.05, 8.05)] {
        assert_eq!(hit(&s, x, y, -0.5), fd, "({x}, {y})");
        assert_eq!(hit(&s, x, y, -1.1), ft, "({x}, {y})");
    }
}

#[test]
fn section_cut_has_foundation_rings() {
    let mut m = house();
    m.push_foundation(stem(0, 1.2));
    let plane = Plane::new([5.0, 0.0, 0.0], [1.0, 0.0, 0.0]).expect("a plane at x = 5");
    let view = OrthoView::elevation([1.0, 0.0]).expect("a view looking +x");
    let cut = section_cut(&Scene3d::build(&m), &plane, &view);
    let rings: Vec<f64> = cut.rings.iter().filter(|r| r.kind == ArchKind::Foundation).map(|r| r.signed_area().abs()).collect();
    // Two stems (0.2 x 0.65) and two footings (0.6 x 0.25), under the south and north walls.
    assert_eq!(rings.len(), 4, "{:?}", cut.rings.iter().map(|r| (r.kind, r.material.clone())).collect::<Vec<_>>());
    let mut areas = rings.clone();
    areas.sort_by(f64::total_cmp);
    for (got, want) in areas.iter().zip([0.13, 0.13, 0.15, 0.15]) {
        assert!((got - want).abs() < 1e-6, "{areas:?}");
    }
    // No foundation, no foundation rings.
    let cut = section_cut(&Scene3d::build(&house()), &plane, &view);
    assert!(cut.rings.iter().all(|r| r.kind != ArchKind::Foundation));
}

#[test]
fn cache_equals_full_after_footing_depth_and_wall_edits() {
    let mut m = house();
    m.push_foundation(stem(0, 1.2));
    let (mut cache, _) = SceneCache::new(&m);
    assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)));
    // A level edit is global (a full build); every other edit rebuilds only what it touches.
    let edits: Vec<(&str, bool, Box<dyn Fn(&mut ArchModel)>)> = vec![
        ("deepen the footing", true, Box::new(|m: &mut ArchModel| m.foundations[0].footing_depth = 1.6)),
        ("thicken the south wall", true, Box::new(|m: &mut ArchModel| m.walls[0].thickness = 0.3)),
        ("make the east wall interior", true, Box::new(|m: &mut ArchModel| m.walls[1].exterior = false)),
        (
            "add the studio's level",
            false,
            Box::new(|m: &mut ArchModel| {
                add_level(m, "Studio", 1.35);
            }),
        ),
        (
            "add a studio on grade",
            true,
            Box::new(|m: &mut ArchModel| {
                ring(m, 20.0, 0.0, 24.0, 4.0, 1);
                m.push_foundation(stem(1, 1.2));
            }),
        ),
        ("thicken a studio wall", true, Box::new(|m: &mut ArchModel| m.walls[6].thickness = 0.25)),
        ("lower the studio", false, Box::new(|m: &mut ArchModel| m.levels[1].elevation = 0.9)),
        ("remove the first foundation", true, Box::new(|m: &mut ArchModel| drop(m.foundations.remove(0)))),
    ];
    for (what, incremental, edit) in edits {
        edit(&mut m);
        let stats = cache.update(&m);
        assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)), "{what}: {stats:?}");
        assert_eq!(stats.incremental, incremental, "{what}: {stats:?}");
        if what == "thicken the south wall" {
            // The stem follows the wall's new outer face (y = -0.15).
            let h = cache.scene().probe(Vec3::new(5.0, -0.13, -0.5)).map(|h| h.part);
            assert_eq!(h.as_deref(), Some("foundation"), "{what}");
        }
        if what == "make the east wall interior" {
            assert_eq!(part_meshes(cache.scene(), "foundation").len(), 3, "{what}");
        }
        if what == "add a studio on grade" {
            assert_eq!(part_meshes(cache.scene(), "foundation").len(), 7, "{what}");
        }
    }
    assert_eq!(cache.update(&m).rebuilt, 0, "without an edit nothing is rebuilt");
}

#[test]
fn footing_depth_is_measured_from_the_floor_on_a_terrain_with_a_pad() {
    // The terrain solid (the pad at -0.3, the skirt 1 m below it) encloses the foundation, so the
    // depths are read from the meshes rather than probed: the same as on no terrain.
    let mut m = house();
    let mut t = Terrain::default();
    t.boundary = rect(-50.0, -50.0, 50.0, 50.0);
    t.points = vec![Vec3::new(-50.0, -50.0, 0.0), Vec3::new(50.0, -50.0, 0.0), Vec3::new(50.0, 50.0, 0.0), Vec3::new(-50.0, 50.0, 0.0)];
    t.pad = true;
    m.terrain = Some(t);
    m.push_foundation(stem(0, 1.2));
    let s = Scene3d::build(&m);
    assert_eq!(s.terrain, 1, "the terrain is built");
    let z = |part: &str| -> Vec<(f64, f64)> { part_meshes(&s, part).iter().map(|x| (x.bounds().min[2], x.bounds().max[2])).collect() };
    let near = |got: &[(f64, f64)], lo: f64, hi: f64| got.len() == 4 && got.iter().all(|(a, b)| (a - lo).abs() < 1e-6 && (b - hi).abs() < 1e-6);
    assert!(near(&z("footing"), -1.2, -0.95), "{:?}", z("footing"));
    assert!(near(&z("foundation"), -0.95, -0.3), "{:?}", z("foundation"));
    let mut bare = house();
    bare.push_foundation(stem(0, 1.2));
    let foundation = |s: &Scene3d| -> Vec<String> {
        s.meshes
            .iter()
            .filter(|x| x.object == ArchId(0) && matches!(part_of(x), "foundation" | "footing"))
            .map(|x| format!("{:?}", x.positions))
            .collect()
    };
    assert_eq!(foundation(&s), foundation(&Scene3d::build(&bare)), "the terrain does not move the foundation");
}
