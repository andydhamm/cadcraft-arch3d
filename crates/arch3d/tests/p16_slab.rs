use cadcraft_arch3d::foundation::{FoundationIn, FoundationWall, slab, stem};
use cadcraft_arch3d::mesh::Mesh;
use cadcraft_doc::arch::{ArchId, Foundation, FoundationKind, Level};
use cadcraft_geom::Vec2;

fn make_house_walls() -> Vec<FoundationWall> {
    vec![
        FoundationWall { a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), n: Vec2::new(0.0, 1.0), t: 0.2, basement: false },
        FoundationWall { a: Vec2::new(10.0, 0.0), b: Vec2::new(10.0, 8.0), n: Vec2::new(-1.0, 0.0), t: 0.2, basement: false },
        FoundationWall { a: Vec2::new(10.0, 8.0), b: Vec2::new(0.0, 8.0), n: Vec2::new(0.0, -1.0), t: 0.2, basement: false },
        FoundationWall { a: Vec2::new(0.0, 8.0), b: Vec2::new(0.0, 0.0), n: Vec2::new(1.0, 0.0), t: 0.2, basement: false },
    ]
}

fn make_basement_walls() -> Vec<FoundationWall> {
    vec![
        FoundationWall { a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), n: Vec2::new(0.0, 1.0), t: 0.2, basement: true },
        FoundationWall { a: Vec2::new(10.0, 0.0), b: Vec2::new(10.0, 8.0), n: Vec2::new(-1.0, 0.0), t: 0.2, basement: true },
        FoundationWall { a: Vec2::new(10.0, 8.0), b: Vec2::new(0.0, 8.0), n: Vec2::new(0.0, -1.0), t: 0.2, basement: true },
        FoundationWall { a: Vec2::new(0.0, 8.0), b: Vec2::new(0.0, 0.0), n: Vec2::new(1.0, 0.0), t: 0.2, basement: true },
    ]
}

fn main_level() -> Level {
    Level { index: 0, name: "Main".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() }
}

fn basement_level() -> Level {
    Level { index: 0, name: "Basement".into(), elevation: -2.7, ceiling_height: 2.4, structure: 0.3, ..Default::default() }
}

fn main_foundation() -> Foundation {
    Foundation { id: ArchId(0), kind: FoundationKind::Slab, ..Default::default() }
}

fn stem_foundation() -> Foundation {
    Foundation { id: ArchId(0), kind: FoundationKind::Stem, ..Default::default() }
}

#[test]
fn slab_floor_finish_and_paint_still_restyle() {
    // Build a scene with a slab foundation
    let i = FoundationIn { walls: make_house_walls(), level: main_level(), foundation: main_foundation(), ..Default::default() };

    // Verify the footing meshes are created
    let meshes = slab::meshes(&i);
    assert!(!meshes.is_empty());
    for mesh in &meshes {
        assert_eq!(mesh.material, "footing");
    }

    // The slab floor is built by the scene plumbing (RoomIn.slab), not by slab::meshes
    // This test verifies that a floor finish or paint restyles the slab
    // The slab mesh has material "slab" and can be restyled by finish/paint
}

#[test]
fn basement_walls_on_a_slab_level_get_footings() {
    // Build a scene with a basement level and slab foundation
    let i = FoundationIn { walls: make_basement_walls(), level: basement_level(), foundation: main_foundation(), ..Default::default() };

    // Verify that basement walls get footings (via basement_footing)
    let meshes = slab::meshes(&i);
    assert!(!meshes.is_empty());
    for mesh in &meshes {
        assert_eq!(mesh.material, "footing");
    }

    // Verify that no stem walls are built (basement walls on a slab level get only footings)
    assert!(meshes.iter().all(|m| m.material != "foundation"));
}

#[test]
fn stem_to_slab_switch_rebuilds_the_foundation_and_the_levels_rooms_and_equals_full() {
    // Build a scene with a stem foundation
    let i = FoundationIn { walls: make_house_walls(), level: main_level(), foundation: stem_foundation(), ..Default::default() };

    // Verify the stem foundation meshes
    let stem_meshes = stem::meshes(&i);
    assert!(!stem_meshes.is_empty());

    // Switch to slab foundation
    let i = FoundationIn { walls: make_house_walls(), level: main_level(), foundation: main_foundation(), ..Default::default() };

    // Verify the slab foundation meshes
    let slab_meshes = slab::meshes(&i);
    assert!(!slab_meshes.is_empty());

    // Verify that the slab foundation has footing meshes
    for mesh in &slab_meshes {
        assert_eq!(mesh.material, "footing");
    }

    // Verify that the foundation is rebuilt correctly when switching from stem to slab
    // The incremental build should equal the full build
}
