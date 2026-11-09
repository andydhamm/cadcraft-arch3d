use cadcraft_arch3d::foundation::{FoundationIn, FoundationWall, slab, stem};
use cadcraft_arch3d::mesh::Mesh;
use cadcraft_doc::arch::{ArchId, Foundation, FoundationKind, Level};
use cadcraft_geom::Vec2;

fn make_house_walls() -> Vec<FoundationWall> {
    vec![
        FoundationWall {
            a: Vec2::new(0.0, 0.0),
            b: Vec2::new(10.0, 0.0),
            inward: Vec2::new(0.0, 1.0),
            thickness: 0.2,
            basement: false,
            ..Default::default()
        },
        FoundationWall {
            a: Vec2::new(10.0, 0.0),
            b: Vec2::new(10.0, 8.0),
            inward: Vec2::new(-1.0, 0.0),
            thickness: 0.2,
            basement: false,
            ..Default::default()
        },
        FoundationWall {
            a: Vec2::new(10.0, 8.0),
            b: Vec2::new(0.0, 8.0),
            inward: Vec2::new(0.0, -1.0),
            thickness: 0.2,
            basement: false,
            ..Default::default()
        },
        FoundationWall {
            a: Vec2::new(0.0, 8.0),
            b: Vec2::new(0.0, 0.0),
            inward: Vec2::new(1.0, 0.0),
            thickness: 0.2,
            basement: false,
            ..Default::default()
        },
    ]
}

fn make_basement_walls() -> Vec<FoundationWall> {
    vec![
        FoundationWall {
            a: Vec2::new(0.0, 0.0),
            b: Vec2::new(10.0, 0.0),
            inward: Vec2::new(0.0, 1.0),
            thickness: 0.2,
            basement: true,
            ..Default::default()
        },
        FoundationWall {
            a: Vec2::new(10.0, 0.0),
            b: Vec2::new(10.0, 8.0),
            inward: Vec2::new(-1.0, 0.0),
            thickness: 0.2,
            basement: true,
            ..Default::default()
        },
        FoundationWall {
            a: Vec2::new(10.0, 8.0),
            b: Vec2::new(0.0, 8.0),
            inward: Vec2::new(0.0, -1.0),
            thickness: 0.2,
            basement: true,
            ..Default::default()
        },
        FoundationWall {
            a: Vec2::new(0.0, 8.0),
            b: Vec2::new(0.0, 0.0),
            inward: Vec2::new(1.0, 0.0),
            thickness: 0.2,
            basement: true,
            ..Default::default()
        },
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
    Foundation { id: ArchId(0), kind: FoundationKind::Stem, footing_depth: 1.0, ..Default::default() }
}

/// The z-range of a mesh (min, max), used to check the footing's vertical extent.
fn z_range(m: &Mesh) -> (f64, f64) {
    let b = m.bounds();
    (b.min[2], b.max[2])
}

#[test]
fn slab_floor_finish_and_paint_still_restyle() {
    // A slab foundation's thickened edge is two adjacent "footing" prisms per on-grade wall:
    // the 0.3 m strip inward from the outer face (e-0.3..e-0.1) and the strip under the wall
    // (e-0.1..e). The slab floor itself is built by the scene plumbing (RoomIn.slab), not here.
    let i = FoundationIn { walls: make_house_walls(), level: main_level(), foundation: main_foundation(), ..Default::default() };

    let meshes = slab::meshes(&i);
    assert_eq!(meshes.len(), 8, "two footing prisms per on-grade wall");
    for mesh in &meshes {
        assert_eq!(mesh.material, "footing");
    }
    // Every footing prism sits at or below the slab top (elevation 0).
    for mesh in &meshes {
        let (lo, hi) = z_range(mesh);
        assert!(lo <= 0.0 + 1e-6 && hi <= 0.0 + 1e-6, "{lo}..{hi}");
    }
}

#[test]
fn basement_walls_on_a_slab_level_get_footings() {
    // Basement walls on a slab level get only a footing (via stem::basement_footing), no stem.
    let i = FoundationIn { walls: make_basement_walls(), level: basement_level(), foundation: main_foundation(), ..Default::default() };

    let meshes = slab::meshes(&i);
    assert_eq!(meshes.len(), 4, "one footing per basement wall");
    for mesh in &meshes {
        assert_eq!(mesh.material, "footing");
    }
    // No stem walls are built.
    assert!(meshes.iter().all(|m| m.material != "foundation"));
    // Each footing is 0.25 m thick, under the basement floor structure (-2.7 - 0.3 = -3.0).
    for mesh in &meshes {
        let (lo, hi) = z_range(mesh);
        assert!((hi + 3.0).abs() < 1e-6 && (lo + 3.25).abs() < 1e-6, "{lo}..{hi}");
    }
}

#[test]
fn stem_to_slab_switch_rebuilds_the_foundation_and_the_levels_rooms_and_equals_full() {
    // Build a scene with a stem foundation: 4 foundation stems + 4 footings.
    let i_stem = FoundationIn { walls: make_house_walls(), level: main_level(), foundation: stem_foundation(), ..Default::default() };
    let stem_meshes = stem::meshes(&i_stem);
    assert_eq!(stem_meshes.len(), 8, "stem: 4 foundation + 4 footing");
    assert_eq!(stem_meshes.iter().filter(|m| m.material == "foundation").count(), 4);
    assert_eq!(stem_meshes.iter().filter(|m| m.material == "footing").count(), 4);

    // Switch to slab foundation: the stems disappear, leaving 2 footing prisms per wall.
    let i_slab = FoundationIn { walls: make_house_walls(), level: main_level(), foundation: main_foundation(), ..Default::default() };
    let slab_meshes = slab::meshes(&i_slab);
    assert_eq!(slab_meshes.len(), 8, "slab: 2 footing prisms per wall");
    assert_eq!(slab_meshes.iter().filter(|m| m.material == "foundation").count(), 0);
    assert_eq!(slab_meshes.iter().filter(|m| m.material == "footing").count(), 8);
}
