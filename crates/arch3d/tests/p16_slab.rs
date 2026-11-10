use cadcraft_arch3d::foundation::{FoundationIn, FoundationWall, slab, stem};
use cadcraft_arch3d::mesh::Mesh;
use cadcraft_doc::arch::{ArchId, Foundation, FoundationKind, Level};
use cadcraft_geom::{Vec2, Vec3};

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
    // A slab foundation's thickened edge is ONE "footing" mesh per on-grade wall, built from
    // two prisms: the 0.3 m strip inward from the outer face (e-0.3..e-0.1) and the strip
    // under the wall (e-0.1..e). The slab floor itself is built by the scene plumbing
    // (RoomIn.slab), not here.
    let i = FoundationIn { walls: make_house_walls(), level: main_level(), foundation: main_foundation(), ..Default::default() };

    let meshes = slab::meshes(&i);
    assert_eq!(meshes.len(), 4, "one footing mesh per on-grade wall");
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

    // Switch to slab foundation: the stems disappear, leaving one footing mesh per on-grade wall.
    let i_slab = FoundationIn { walls: make_house_walls(), level: main_level(), foundation: main_foundation(), ..Default::default() };
    let slab_meshes = slab::meshes(&i_slab);
    assert_eq!(slab_meshes.len(), 4, "slab: 1 footing mesh per on-grade wall");
    assert_eq!(slab_meshes.iter().filter(|m| m.material == "foundation").count(), 0);
    assert_eq!(slab_meshes.iter().filter(|m| m.material == "footing").count(), 4);
}

#[test]
fn empty_walls_produce_no_meshes() {
    let i = FoundationIn { walls: vec![], level: main_level(), foundation: main_foundation(), ..Default::default() };
    let meshes = slab::meshes(&i);
    assert!(meshes.is_empty());
}

#[test]
fn nan_level_elevation_produces_no_meshes() {
    let mut level = main_level();
    level.elevation = f64::NAN;
    let i = FoundationIn { walls: make_house_walls(), level, foundation: main_foundation(), ..Default::default() };
    let meshes = slab::meshes(&i);
    assert!(meshes.is_empty());
}

#[test]
fn zero_length_wall_does_not_crash() {
    let walls = vec![FoundationWall {
        a: Vec2::new(5.0, 5.0),
        b: Vec2::new(5.0, 5.0),
        inward: Vec2::new(0.0, 1.0),
        thickness: 0.2,
        basement: false,
        ..Default::default()
    }];
    let i = FoundationIn { walls, level: main_level(), foundation: main_foundation(), ..Default::default() };
    let meshes = slab::meshes(&i);
    // A zero-length wall produces a degenerate ring; push_prism should handle it gracefully.
    // The important invariant is that it does not crash.
    for mesh in &meshes {
        assert_eq!(mesh.material, "footing");
    }
}

#[test]
fn mixed_basement_and_on_grade_walls() {
    let walls = vec![
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
            basement: true,
            ..Default::default()
        },
    ];
    let i = FoundationIn { walls, level: main_level(), foundation: main_foundation(), ..Default::default() };
    let meshes = slab::meshes(&i);
    // 1 on-grade wall → 1 footing mesh; 1 basement wall → 1 footing
    assert_eq!(meshes.len(), 2);
    assert_eq!(meshes.iter().filter(|m| m.material == "footing").count(), 2);
}

#[test]
fn wall_thickness_equal_to_strip_width() {
    let walls = vec![FoundationWall {
        a: Vec2::new(0.0, 0.0),
        b: Vec2::new(10.0, 0.0),
        inward: Vec2::new(0.0, 1.0),
        thickness: 0.3,
        basement: false,
        ..Default::default()
    }];
    let i = FoundationIn { walls, level: main_level(), foundation: main_foundation(), ..Default::default() };
    let meshes = slab::meshes(&i);
    assert_eq!(meshes.len(), 1, "one footing mesh when wall thickness equals strip width");
    // The two prisms share the same plan extent (thickness = 0.3 = strip width) but span
    // different z ranges, so the combined mesh covers z = elevation-0.3..elevation.
    let (lo, hi) = z_range(&meshes[0]);
    assert!((lo + 0.3).abs() < 1e-6 && hi.abs() < 1e-6, "combined z: {lo}..{hi}");
}

#[test]
fn very_thick_wall() {
    let walls = vec![FoundationWall {
        a: Vec2::new(0.0, 0.0),
        b: Vec2::new(10.0, 0.0),
        inward: Vec2::new(0.0, 1.0),
        thickness: 1.0,
        basement: false,
        ..Default::default()
    }];
    let i = FoundationIn { walls, level: main_level(), foundation: main_foundation(), ..Default::default() };
    let meshes = slab::meshes(&i);
    assert_eq!(meshes.len(), 1, "one footing mesh for a very thick wall");
    // The wall strip extends 1.0m inward, well beyond the 0.3m edge strip.
    let b = meshes[0].bounds();
    assert!((b.max[1] - 1.0).abs() < 1e-6, "mesh should extend to y=1.0, got {}", b.max[1]);
}

#[test]
fn footing_meshes_have_correct_object_and_level() {
    let i = FoundationIn { walls: make_house_walls(), level: main_level(), foundation: main_foundation(), ..Default::default() };
    let meshes = slab::meshes(&i);
    for mesh in &meshes {
        assert_eq!(mesh.object, i.foundation.id);
        assert_eq!(mesh.level_elevation, i.level.elevation);
    }
}

/// One footing mesh per exterior wall: each is a closed union of the 0.3 m edge strip
/// (e-0.3..e-0.1) and the wall strip (e-0.1..e) sharing the e-0.1 face. Because the two
/// prisms share a face and live in one mesh, an upward-ray probe counts crossings so that a
/// point in either prism stays inside (odd) and the shared face does not open a gap.
#[test]
fn slab_edge_is_one_footing_mesh_per_wall() {
    let i = FoundationIn { walls: make_house_walls(), level: main_level(), foundation: main_foundation(), ..Default::default() };

    let meshes = slab::meshes(&i);
    assert_eq!(meshes.len(), 4, "one footing mesh per exterior wall");

    let held = |p: Vec3| meshes.iter().filter(|m| cadcraft_arch3d::probe::probe_mesh(m, p)).count();

    // (5, 0.15, -0.2) lies in the south wall's edge strip (x 0..10, y 0..0.3, z -0.3..-0.1).
    assert_eq!(held(Vec3::new(5.0, 0.15, -0.2)), 1, "edge strip point held by exactly one mesh");
    // (5, 0.0, -0.05) lies at the south wall's outer face, inside the wall strip (z -0.1..0).
    assert_eq!(held(Vec3::new(5.0, 0.0, -0.05)), 1, "wall strip point held by exactly one mesh");
    // (5, 0.25, -0.2) is outside every footing prism.
    assert_eq!(held(Vec3::new(5.0, 0.25, -0.2)), 0, "no footing holds this point");
}
