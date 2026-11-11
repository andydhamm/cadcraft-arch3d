//! Phase 14: furnishings in the 3D build, the symbol material tables, and the scene cache's new
//! inputs (furnishings, paints, symbols, a cabinet's level).

use cadcraft_arch3d::furnishing::{MISSING_SYMBOL_MATERIAL, furnishing_meshes, symbol_materials};
use cadcraft_arch3d::{MaterialDef, Mesh, ObjectKind, Scene3d, SceneCache, object_key};
use cadcraft_doc::arch::{
    Backdrop, Cabinet, CabinetKind, Fixture, FixtureKind, Furnishing, FurnishingHost, ImageRgba, Level, MaterialRef, Paint, Projection, Roof,
    RoofPlane, Room, SymbolMesh, SymbolPart,
};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::{Vec2, Vec3};

/// The unit cube's key (any 16 hex digits: the tests make their symbols by hand).
const CUBE: &str = "00000000000c0be1";
/// A two-part symbol, the second part textured.
const TWO: &str = "0000000000000002";
/// A pull symbol.
const PULL: &str = "00000000000000a1";

const EPS: f64 = 1e-6;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= EPS
}

fn image(width: u32, height: u32, seed: u8) -> ImageRgba {
    let mut image = ImageRgba { width, height, ..Default::default() };
    image.rgba = (0..width * height * 4).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect();
    image
}

/// A unit cube in symbol space (x and y in [-0.5, 0.5], z in [0, 1]): each face its own four
/// vertices with its outward normal, wound outward.
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

fn cube_symbol() -> SymbolMesh {
    SymbolMesh { key: CUBE.into(), parts: vec![cube_part()], ..Default::default() }
}

/// Part 0 untextured, part 1 textured with the first of two images.
fn two_part_symbol() -> SymbolMesh {
    let first = SymbolPart { base_color: [0.8, 0.2, 0.1, 1.0], metallic: 0.3, roughness: 0.4, texture: None, ..cube_part() };
    let second = SymbolPart { base_color: [0.1, 0.5, 0.9, 0.5], metallic: 1.0, roughness: 0.2, texture: Some(0), ..cube_part() };
    let mut symbol = SymbolMesh { key: TWO.into(), parts: vec![first, second], ..Default::default() };
    symbol.textures = vec![image(2, 2, 7), image(1, 1, 9)];
    symbol
}

/// One part textured with the symbol's second image.
fn pull_symbol() -> SymbolMesh {
    let bar = SymbolPart { base_color: [0.9, 0.9, 0.95, 1.0], metallic: 1.0, roughness: 0.3, texture: Some(1), ..cube_part() };
    let mut symbol = SymbolMesh { key: PULL.into(), parts: vec![bar], ..Default::default() };
    symbol.textures = vec![image(1, 1, 1), image(2, 1, 3)];
    symbol
}

/// Two levels (floors at 0 and 3 m) and the three symbols.
fn base_model() -> ArchModel {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    m.levels.push(Level { index: 1, elevation: 3.0, ..Default::default() });
    m.symbols.insert(CUBE.into(), cube_symbol());
    m.symbols.insert(TWO.into(), two_part_symbol());
    m.symbols.insert(PULL.into(), pull_symbol());
    m
}

fn cube_furnishing() -> Furnishing {
    Furnishing {
        item: "test-cube".into(),
        symbol: CUBE.into(),
        name: "Cube".into(),
        category: "Furniture".into(),
        symbol_type: "table".into(),
        size: [1.0, 1.0, 1.0],
        ..Default::default()
    }
}

fn wall(id: u32, a: [f64; 2], b: [f64; 2]) -> Wall {
    Wall { id: ArchId(id), start: Vec2::new(a[0], a[1]), end: Vec2::new(b[0], b[1]), thickness: 0.2, height: 2.4, ..Default::default() }
}

fn base_cabinet(id: u32, x: f64, width: f64, level: u32) -> Cabinet {
    Cabinet {
        id: ArchId(id),
        kind: CabinetKind::Base,
        level,
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

/// A house on level 0: four walls, a room, a roof, two touching base cabinets and one past a gap
/// (it has a pull), a wall cabinet, a sink, and four furnishings (one in the first cabinet, one in
/// the gap, one on level 1, one whose symbol is missing).
fn house() -> ArchModel {
    let mut m = base_model();
    m.walls =
        vec![wall(0, [0.0, 0.0], [6.0, 0.0]), wall(1, [6.0, 0.0], [6.0, 4.0]), wall(2, [6.0, 4.0], [0.0, 4.0]), wall(3, [0.0, 4.0], [0.0, 0.0])];
    m.next_id = 4;
    m.rooms.push(Room {
        id: ArchId(0),
        polygon: vec![Vec2::new(0.1, 0.1), Vec2::new(5.9, 0.1), Vec2::new(5.9, 3.9), Vec2::new(0.1, 3.9)],
        ..Default::default()
    });
    m.next_room_id = 1;
    let plane = RoofPlane {
        ring: vec![Vec2::new(-0.3, -0.3), Vec2::new(6.3, -0.3), Vec2::new(6.3, 4.3), Vec2::new(-0.3, 4.3)],
        slope: 0.3,
        thickness: 0.2,
        eave_z: 2.4,
        base_edge: [-0.3, -0.3, 6.3, -0.3],
        ..Default::default()
    };
    m.roofs.push(Roof { id: ArchId(0), planes: vec![plane], ..Default::default() });
    m.next_roof_id = 1;
    m.push_cabinet(base_cabinet(0, 1.0, 0.6, 0));
    m.push_cabinet(base_cabinet(0, 1.6, 0.6, 0));
    m.push_cabinet(Cabinet { pull: Some("bar-pull".into()), pull_symbol: Some(PULL.into()), ..base_cabinet(0, 3.0, 0.6, 0) });
    m.push_cabinet(Cabinet { kind: CabinetKind::Wall, elevation: 1.4, height: 0.7, depth: 0.33, counter: false, ..base_cabinet(0, 4.0, 0.6, 0) });
    m.push_fixture(Fixture {
        kind: FixtureKind::Toilet,
        position: Vec2::new(5.0, 3.8),
        rotation: 180.0,
        width: 0.4,
        depth: 0.7,
        height: 0.8,
        ..Default::default()
    });
    m.push_furnishing(Furnishing {
        item: "sink".into(),
        symbol: TWO.into(),
        at: [1.0, 0.8],
        rotation: 180.0,
        elevation: 0.9,
        size: [0.5, 0.4, 0.2],
        placement: cadcraft_doc::arch::Placement::Counter,
        host: FurnishingHost::Cabinet { cabinet: ArchId(0) },
        cutout: Some([0.45, 0.35]),
        ..cube_furnishing()
    });
    m.push_furnishing(Furnishing {
        item: "dishwasher".into(),
        at: [2.3, 0.8],
        size: [0.6, 0.6, 0.85],
        host: FurnishingHost::Gap { left: ArchId(1), right: ArchId(2) },
        ..cube_furnishing()
    });
    m.push_furnishing(Furnishing { at: [2.0, 2.0], rotation: 30.0, level: 1, size: [1.2, 0.8, 0.75], ..cube_furnishing() });
    m.push_furnishing(Furnishing { symbol: "ffffffffffffffff".into(), at: [4.5, 2.5], rotation: 15.0, size: [0.9, 0.5, 1.8], ..cube_furnishing() });
    m
}

/// What the cache must match: the digest, every mesh's material and owner in order, every count,
/// the materials, the textures, the backdrop and the furnishing boxes.
fn assert_same(cached: &Scene3d, full: &Scene3d, what: &str) {
    assert_eq!(cached.digest(), full.digest(), "{what}: digest");
    let owners = |s: &Scene3d| s.meshes.iter().map(|m| (m.material.clone(), m.object)).collect::<Vec<_>>();
    assert_eq!(owners(cached), owners(full), "{what}: meshes");
    let counts = |s: &Scene3d| {
        [
            s.openings_built,
            s.floors,
            s.ceilings,
            s.roofs,
            s.stairs,
            s.railings,
            s.decks,
            s.terrain,
            s.paths,
            s.plants,
            s.fixtures,
            s.lights.len(),
            s.furnishings,
        ]
    };
    assert_eq!(counts(cached), counts(full), "{what}: counts");
    assert_eq!(cached.materials, full.materials, "{what}: materials");
    assert_eq!(cached.textures, full.textures, "{what}: textures");
    assert_eq!(cached.backdrop, full.backdrop, "{what}: backdrop");
    assert_eq!(cached.furnishing_boxes, full.furnishing_boxes, "{what}: furnishing boxes");
}

/// The triangles of a mesh as point triples.
fn triangles(m: &Mesh) -> Vec<[[f64; 3]; 3]> {
    let p = |i: u32| m.positions.get(i as usize).map(|p| [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]).unwrap_or([f64::NAN; 3]);
    m.indices.as_chunks::<3>().0.iter().map(|t| [p(t[0]), p(t[1]), p(t[2])]).collect()
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `p` in the furnishing's frame: (along its width, along its depth, above its bottom).
fn local(f: &Furnishing, floor: f64, p: [f64; 3]) -> [f64; 3] {
    let (s, c) = f.rotation.to_radians().sin_cos();
    let (dx, dy) = (p[0] - f.at[0], p[1] - f.at[1]);
    [dx * c + dy * s, dy * c - dx * s, p[2] - floor - f.elevation]
}

#[test]
fn furnishing_meshes_follow_size_rotation_and_mirror() {
    let model = base_model();
    let part = cube_part();
    let size = [2.0, 1.0, 0.5];
    for (rotation, mirrored, level, elevation) in
        [(0.0, false, 0, 0.0), (90.0, false, 0, 0.25), (37.0, false, 1, 0.4), (37.0, true, 1, 0.4), (0.0, true, 0, 0.0)]
    {
        let f = Furnishing { id: ArchId(4), at: [3.0, -2.0], rotation, size, mirrored, level, elevation, ..cube_furnishing() };
        let floor = if level == 1 { 3.0 } else { 0.0 };
        let what = format!("rotation {rotation}, mirrored {mirrored}");
        let meshes = furnishing_meshes(&model, &f);
        assert_eq!(meshes.len(), 1, "{what}");
        let m = &meshes[0];
        assert_eq!(m.material, format!("sym:{CUBE}:0"));
        assert_eq!(m.object, ArchId(4));
        assert_eq!(m.level_elevation, floor);
        assert_eq!(m.positions.len(), part.positions.len());
        assert_eq!(m.uvs, part.uvs, "uvs are copied");
        // Every vertex is the symbol vertex through local_to_plan, raised by the level's floor.
        for (p, q) in part.positions.iter().zip(&m.positions) {
            let want = f.local_to_plan([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])], [1.0, 1.0, 1.0]);
            assert!(
                near(f64::from(q[0]), want[0]) && near(f64::from(q[1]), want[1]) && near(f64::from(q[2]), want[2] + floor),
                "{what}: {q:?} vs {want:?}"
            );
        }
        // The bounds are the rotated size around `at`, from the elevation up by the height.
        let (s, c) = rotation.to_radians().sin_cos();
        let hx = (c.abs() * size[0] + s.abs() * size[1]) / 2.0;
        let hy = (s.abs() * size[0] + c.abs() * size[1]) / 2.0;
        let b = m.bounds();
        for (got, want) in [
            (b.min[0], 3.0 - hx),
            (b.max[0], 3.0 + hx),
            (b.min[1], -2.0 - hy),
            (b.max[1], -2.0 + hy),
            (b.min[2], floor + elevation),
            (b.max[2], floor + elevation + 0.5),
        ] {
            assert!(near(got, want), "{what}: bounds {b:?}");
        }
        // In the furnishing's own frame every vertex is on the box of its size.
        for q in &m.positions {
            let l = local(&f, floor, [f64::from(q[0]), f64::from(q[1]), f64::from(q[2])]);
            assert!(near(l[0].abs(), 1.0) || near(l[1].abs(), 0.5) || near(l[2], 0.0) || near(l[2], 0.5), "{what}: {l:?}");
            assert!(l[0].abs() <= 1.0 + EPS && l[1].abs() <= 0.5 + EPS && l[2] >= -EPS && l[2] <= 0.5 + EPS, "{what}: {l:?}");
        }
    }
    // Worked by hand: at 90 degrees the symbol's (0.5, -0.5, 0), scaled to (1, -0.5, 0), turns to
    // (0.5, 1); mirrored it is (-1, -0.5, 0) first and turns to (0.5, -1).
    let i = part.positions.iter().position(|p| *p == [0.5, -0.5, 0.0]).unwrap_or(usize::MAX);
    for (mirrored, want) in [(false, [0.5, 1.0]), (true, [0.5, -1.0])] {
        let f = Furnishing { rotation: 90.0, size, mirrored, ..cube_furnishing() };
        let m = &furnishing_meshes(&model, &f)[0];
        let q = m.positions[i];
        assert!(near(f64::from(q[0]), want[0]) && near(f64::from(q[1]), want[1]) && near(f64::from(q[2]), 0.0), "mirrored {mirrored}: {q:?}");
    }
    // A symbol whose native size is not the furnishing's is stretched to it.
    let mut model = base_model();
    let mut big = cube_symbol();
    for p in big.parts[0].positions.iter_mut() {
        *p = [p[0] * 4.0, p[1] * 2.0, p[2] * 3.0];
    }
    model.symbols.insert(CUBE.into(), big);
    let m = &furnishing_meshes(&model, &Furnishing { size, ..cube_furnishing() })[0];
    let b = m.bounds();
    assert!(near(b.max[0] - b.min[0], 2.0) && near(b.max[1] - b.min[1], 1.0) && near(b.max[2] - b.min[2], 0.5), "{b:?}");
}

#[test]
fn mirrored_furnishings_keep_outward_normals() {
    let model = base_model();
    for mirrored in [false, true] {
        for rotation in [0.0, 37.0, 90.0, 211.0] {
            let f = Furnishing { at: [1.0, 2.0], rotation, size: [2.0, 1.0, 0.5], mirrored, elevation: 0.1, ..cube_furnishing() };
            let what = format!("rotation {rotation}, mirrored {mirrored}");
            let m = &furnishing_meshes(&model, &f)[0];
            let centre = [1.0, 2.0, 0.1 + 0.25];
            for t in m.indices.as_chunks::<3>().0 {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| {
                    let p = m.positions[i as usize];
                    [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]
                });
                let g = cross(sub(b, a), sub(c, a));
                let len = dot(g, g).sqrt();
                assert!(len > 1e-9, "{what}: degenerate triangle");
                let g = [g[0] / len, g[1] / len, g[2] / len];
                let mid = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0];
                assert!(dot(g, sub(mid, centre)) > 0.0, "{what}: a triangle faces inward");
                // The vertex normals point where the winding does, and are unit length.
                for i in [t[0], t[1], t[2]] {
                    let n = m.normals[i as usize];
                    let n = [f64::from(n[0]), f64::from(n[1]), f64::from(n[2])];
                    assert!(near(dot(n, n).sqrt(), 1.0), "{what}: |n| = {}", dot(n, n).sqrt());
                    assert!(dot(n, g) > 0.999, "{what}: normal {n:?} against the face {g:?}");
                }
            }
        }
    }
    // A stretched part's normals take the inverse of the scale: a slanted face stays perpendicular.
    let mut model = base_model();
    let wedge = SymbolPart {
        positions: vec![[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.5, 0.5, 1.0]],
        normals: vec![[0.0, -0.707_106_77, 0.707_106_77]; 3],
        indices: vec![0, 1, 2],
        ..Default::default()
    };
    model.symbols.insert(CUBE.into(), SymbolMesh { key: CUBE.into(), parts: vec![wedge], ..Default::default() });
    for mirrored in [false, true] {
        let m = &furnishing_meshes(&model, &Furnishing { size: [1.0, 4.0, 1.0], rotation: 20.0, mirrored, ..cube_furnishing() })[0];
        let tris = triangles(m);
        assert_eq!(tris.len(), 1);
        let [a, b, c] = tris[0];
        let g = cross(sub(b, a), sub(c, a));
        let n = m.normals[0];
        let n = [f64::from(n[0]), f64::from(n[1]), f64::from(n[2])];
        let len = dot(g, g).sqrt();
        assert!(dot(n, [g[0] / len, g[1] / len, g[2] / len]) > 0.999_999, "mirrored {mirrored}: {n:?} against {g:?}");
    }
}

#[test]
fn a_missing_symbol_builds_a_box() {
    let mut model = base_model();
    for symbol in ["ffffffffffffffff", ""] {
        for mirrored in [false, true] {
            let f = Furnishing {
                id: ArchId(6),
                symbol: symbol.into(),
                at: [2.0, 3.0],
                rotation: 30.0,
                elevation: 0.1,
                level: 1,
                size: [0.8, 0.6, 0.9],
                mirrored,
                ..cube_furnishing()
            };
            let meshes = furnishing_meshes(&model, &f);
            assert_eq!(meshes.len(), 1, "one box");
            let m = &meshes[0];
            assert_eq!(m.material, MISSING_SYMBOL_MATERIAL);
            assert_eq!(object_key(m).kind, ObjectKind::Furnishing);
            assert_eq!(m.object, ArchId(6));
            assert_eq!(m.level_elevation, 3.0);
            assert!(m.is_manifold(), "the box is closed");
            assert_eq!(m.uvs.len(), m.positions.len());
            let b = m.bounds();
            assert!(near(b.min[2], 3.1) && near(b.max[2], 4.0), "{b:?}");
            let mut corners = std::collections::BTreeSet::new();
            for q in &m.positions {
                let l = local(&f, 3.0, [f64::from(q[0]), f64::from(q[1]), f64::from(q[2])]);
                assert!(near(l[0].abs(), 0.4) && near(l[1].abs(), 0.3) && (near(l[2], 0.0) || near(l[2], 0.9)), "a corner: {l:?}");
                corners.insert((l[0] > 0.0, l[1] > 0.0, l[2] > 0.45));
            }
            assert_eq!(corners.len(), 8, "all eight corners");
            let centre = [2.0, 3.0, 3.1 + 0.45];
            for [a, b, c] in triangles(m) {
                let mid = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0];
                assert!(dot(cross(sub(b, a), sub(c, a)), sub(mid, centre)) > 0.0, "wound outward");
            }
        }
    }
    // In the scene: counted, probed by its box, and without a material of its own.
    model.furnishings.push(Furnishing { symbol: "ffffffffffffffff".into(), at: [2.0, 3.0], size: [0.8, 0.6, 0.9], ..cube_furnishing() });
    let scene = Scene3d::build(&model);
    assert_eq!(scene.furnishings, 1);
    assert!(scene.meshes.iter().any(|m| m.material == MISSING_SYMBOL_MATERIAL));
    assert!(!scene.materials.contains_key(MISSING_SYMBOL_MATERIAL));
    let hit = scene.probe(Vec3::new(2.0, 3.0, 0.45)).map(|h| (h.id, h.part));
    assert_eq!(hit, Some(("f0".to_string(), "furnishing".to_string())));
}

#[test]
fn symbol_materials_carry_base_colour_and_textures() {
    let mut model = base_model();
    model.symbols.insert("00000000000000ff".into(), SymbolMesh { key: "00000000000000ff".into(), parts: vec![cube_part()], ..Default::default() });
    model.furnishings.push(Furnishing { symbol: TWO.into(), ..cube_furnishing() });
    model.cabinets.push(Cabinet { pull: Some("bar-pull".into()), pull_symbol: Some(PULL.into()), ..base_cabinet(0, 3.0, 0.6, 0) });
    let scene = Scene3d::build(&model);
    let two = two_part_symbol();
    let pull = pull_symbol();
    let keys: Vec<&str> = scene.materials.keys().map(String::as_str).collect();
    assert_eq!(keys, [format!("pull:{PULL}:0"), format!("sym:{TWO}:0"), format!("sym:{TWO}:1")], "the used symbols' parts only");
    assert_eq!(
        scene.materials.get(&format!("sym:{TWO}:0")),
        Some(&MaterialDef { base_color: [0.8, 0.2, 0.1, 1.0], metallic: 0.3, roughness: 0.4, ..Default::default() })
    );
    let textured = scene.materials.get(&format!("sym:{TWO}:1")).cloned().unwrap_or_default();
    assert_eq!(textured.base_color, [0.1, 0.5, 0.9, 0.5]);
    assert_eq!((textured.metallic, textured.roughness), (1.0, 0.2));
    assert_eq!(textured.texture, Some(format!("sym:{TWO}:t0")));
    assert_eq!(textured.tile, [1.0, 1.0]);
    assert_eq!(textured.tint, None);
    let bar = scene.materials.get(&format!("pull:{PULL}:0")).cloned().unwrap_or_default();
    assert_eq!(bar.base_color, [0.9, 0.9, 0.95, 1.0]);
    assert_eq!(bar.texture, Some(format!("sym:{PULL}:t1")), "a pull's texture is its symbol's texture");
    // Only the textures the parts use, with their pixels.
    let texture_keys: Vec<&str> = scene.textures.keys().map(String::as_str).collect();
    assert_eq!(texture_keys, [format!("sym:{TWO}:t0"), format!("sym:{PULL}:t1")]);
    assert_eq!(scene.textures.get(&format!("sym:{TWO}:t0")), Some(&two.textures[0]));
    assert_eq!(scene.textures.get(&format!("sym:{PULL}:t1")), Some(&pull.textures[1]));
    // The meshes name those materials.
    assert!(scene.meshes.iter().any(|m| m.material == format!("sym:{TWO}:1")));
    // symbol_materials on its own adds the same; the default definition is white, rough and flat.
    let mut alone = Scene3d::default();
    symbol_materials(&model, &mut alone);
    assert_eq!(alone.materials, scene.materials);
    assert_eq!(alone.textures, scene.textures);
    let white = MaterialDef::default();
    assert_eq!((white.base_color, white.metallic, white.roughness), ([1.0; 4], 0.0, 0.8));
    assert_eq!((white.texture, white.tile, white.tint), (None, [1.0, 1.0], None));
    // The cache fills the same tables, and refills them when only a symbol's use changes.
    let (mut cache, _) = SceneCache::new(&model);
    assert_same(cache.scene(), &scene, "new");
    let mut next = model.clone();
    next.cabinets[0].pull_symbol = None;
    cache.update(&next);
    assert_same(cache.scene(), &Scene3d::build(&next), "pull removed");
    assert!(!cache.scene().materials.keys().any(|k| k.starts_with("pull:")));
}

#[test]
fn backdrop_and_its_image_reach_the_scene() {
    let mut model = base_model();
    model.images.insert("sky".into(), image(4, 2, 5));
    model.backdrop = Some(Backdrop::Image { item: "sky".into(), projection: Projection::Equirect, horizon: 0.0 });
    let (mut cache, _) = SceneCache::new(&model);
    let scene = Scene3d::build(&model);
    assert_eq!(scene.backdrop, model.backdrop);
    assert_eq!(scene.textures.get("item:sky"), Some(&image(4, 2, 5)));
    assert_same(cache.scene(), &scene, "image backdrop");
    // A colour backdrop needs no texture; an update with no object changed still refreshes it.
    let mut next = model.clone();
    next.backdrop = Some(Backdrop::Gradient { top: [10, 20, 30], bottom: [200, 210, 220] });
    let stats = cache.update(&next);
    assert_eq!(stats.rebuilt, 0);
    assert!(stats.incremental);
    assert_same(cache.scene(), &Scene3d::build(&next), "gradient");
    assert!(cache.scene().textures.is_empty());
    // An image backdrop whose image is missing has no texture.
    next.backdrop = Some(Backdrop::Image { item: "night".into(), projection: Projection::Flat, horizon: 0.0 });
    cache.update(&next);
    assert_same(cache.scene(), &Scene3d::build(&next), "missing image");
    assert!(cache.scene().textures.is_empty());
}

#[test]
fn probe_hits_a_furnishing_by_its_box() {
    let mut model = base_model();
    // An open tray: the cube's bottom face only, not a closed mesh.
    let mut tray = cube_part();
    tray.indices.truncate(6);
    model.symbols.insert(CUBE.into(), SymbolMesh { key: CUBE.into(), parts: vec![tray], ..Default::default() });
    model.walls.push(Wall { base_level: 1, ..wall(0, [4.0, 5.0], [6.0, 5.0]) });
    let f = Furnishing { id: ArchId(3), at: [5.0, 5.0], rotation: 37.0, size: [2.0, 1.0, 0.5], elevation: 0.2, level: 1, ..cube_furnishing() };
    model.furnishings.push(f.clone());
    let scene = Scene3d::build(&model);
    assert_eq!(scene.furnishings, 1);
    assert_eq!(scene.furnishing_boxes.len(), 1);
    // A point in its frame: (along the width, along the depth, height above the level's floor).
    let at = |u: f64, v: f64, z: f64| {
        let (s, c) = 37f64.to_radians().sin_cos();
        Vec3::new(5.0 + u * c - v * s, 5.0 + u * s + v * c, 3.0 + z)
    };
    let furnishing = Some(("f3".to_string(), "furnishing".to_string(), 3.0));
    let probe = |p: Vec3| scene.probe(p).map(|h| (h.id, h.part, h.level));
    // Inside, also in the wall it overlaps: the box comes first.
    assert_eq!(probe(at(0.0, 0.0, 0.45)), furnishing);
    assert_eq!(probe(at(0.95, 0.45, 0.3)), furnishing, "a turned corner");
    assert_eq!(probe(at(-0.99, -0.49, 0.21)), furnishing, "the other corner");
    assert_eq!(probe(at(0.5, 0.2, 0.2)), furnishing, "the bottom face is inside");
    // Outside the turned box though inside its axis-aligned bounds and the tray's.
    assert_eq!(probe(at(0.0, 0.6, 0.3)), None);
    assert_eq!(probe(at(1.05, 0.0, 0.3)), None);
    // Below and above it, and on its top face.
    assert_eq!(probe(at(0.0, 0.3, 0.19)), None);
    assert_eq!(probe(at(0.0, 0.3, 0.71)), None);
    assert_eq!(probe(at(0.0, 0.3, 0.7)), None, "the top face is outside");
    // The wall beside it still probes as the wall.
    assert_eq!(scene.probe(Vec3::new(4.05, 5.0, 4.0)).map(|h| h.part), Some("wall".to_string()));
    // Not a point: nothing.
    assert_eq!(probe(Vec3::new(f64::NAN, 5.0, 3.3)), None);
    // A rotated, mirrored copy on level 0 probes the same way.
    model.furnishings.push(Furnishing { id: ArchId(9), mirrored: true, level: 0, ..f });
    let scene = Scene3d::build(&model);
    assert_eq!(scene.probe(Vec3::new(5.0, 5.0, 0.45)).map(|h| (h.id, h.level)), Some(("f9".to_string(), 0.0)));
    let (cache, _) = SceneCache::new(&model);
    assert_eq!(cache.scene().furnishing_boxes, scene.furnishing_boxes);
}

#[test]
fn one_furnishing_edit_rebuilds_one_object() {
    let mut model = base_model();
    model.walls =
        vec![wall(0, [0.0, 0.0], [6.0, 0.0]), wall(1, [6.0, 0.0], [6.0, 4.0]), wall(2, [6.0, 4.0], [0.0, 4.0]), wall(3, [0.0, 4.0], [0.0, 0.0])];
    model.push_furnishing(Furnishing { at: [1.0, 1.0], ..cube_furnishing() });
    model.push_furnishing(Furnishing { at: [2.0, 1.0], symbol: TWO.into(), ..cube_furnishing() });
    model.push_furnishing(Furnishing { at: [3.0, 1.0], level: 1, ..cube_furnishing() });
    let (mut cache, stats) = SceneCache::new(&model);
    assert!(!stats.incremental);
    let mut step = |next: &ArchModel, what: &str, rebuilt: usize| {
        let stats = cache.update(next);
        assert!(stats.incremental, "{what}");
        assert_eq!(stats.rebuilt, rebuilt, "{what}: {stats:?}");
        assert_same(cache.scene(), &Scene3d::build(next), what);
    };
    // Place.
    let placed = model.push_furnishing(Furnishing { at: [4.0, 2.0], rotation: 45.0, ..cube_furnishing() });
    step(&model, "place", 1);
    assert_eq!(Scene3d::build(&model).furnishings, 4);
    // Edit each field the transform commands change.
    let edits: [(&str, fn(&mut Furnishing)); 6] = [
        ("move", |f| f.at[0] += 0.3),
        ("rotate", |f| f.rotation += 30.0),
        ("resize", |f| f.size = [1.5, 0.5, 0.75]),
        ("mirror", |f| f.mirrored = !f.mirrored),
        ("raise", |f| f.elevation = 0.4),
        ("another level", |f| f.level = 1),
    ];
    for (what, edit) in edits {
        if let Some(f) = model.furnishing_mut(placed) {
            edit(f);
        }
        step(&model, what, 1);
    }
    // Another symbol, then one the model lacks (a box).
    model.furnishings[3].symbol = TWO.into();
    step(&model, "another symbol", 1);
    model.furnishings[3].symbol = "ffffffffffffffff".into();
    step(&model, "a missing symbol", 1);
    // Adding the missing symbol rebuilds the furnishing that uses it.
    let mut shown = cube_symbol();
    shown.key = "ffffffffffffffff".into();
    model.symbols.insert("ffffffffffffffff".into(), shown);
    step(&model, "the symbol arrives", 1);
    // Delete: the deleted object has nothing to make again; the others stay as they were.
    model.furnishings.retain(|f| f.id != placed);
    step(&model, "delete", 0);
    // No change, nothing rebuilt.
    step(&model, "no change", 0);
}

#[test]
fn a_cabinet_edit_rebuilds_the_cabinets_of_its_level() {
    let mut model = house();
    model.push_cabinet(base_cabinet(0, 1.0, 0.6, 1));
    let (mut cache, _) = SceneCache::new(&model);
    let level0 = model.cabinets.iter().filter(|c| c.level == 0).count();
    // A base cabinet's size reaches the base cabinets of its level (counter runs, gaps), not the wall
    // cabinets (they read no base cabinet) and not level 1's.
    let base0 = model.cabinets.iter().filter(|c| c.level == 0 && c.kind == CabinetKind::Base).count();
    model.cabinets[0].width = 0.7;
    let stats = cache.update(&model);
    assert_eq!(stats.rebuilt, base0, "{stats:?}");
    assert_same(cache.scene(), &Scene3d::build(&model), "cabinet width");
    // So does the sink it hosts: the sink plus the level's cabinets.
    model.furnishings[0].at[0] += 0.05;
    let stats = cache.update(&model);
    assert_eq!(stats.rebuilt, level0 + 1, "{stats:?}");
    assert_same(cache.scene(), &Scene3d::build(&model), "hosted sink");
    // Taking the pull's symbol away rebuilds only the cabinet with that pull.
    model.symbols.remove(PULL);
    let stats = cache.update(&model);
    assert_eq!(stats.rebuilt, 1, "{stats:?}");
    assert_same(cache.scene(), &Scene3d::build(&model), "pull symbol gone");
    // A free-standing furnishing touches no cabinet.
    model.furnishings[2].at[1] += 0.1;
    let stats = cache.update(&model);
    assert_eq!(stats.rebuilt, 1, "{stats:?}");
}

#[test]
fn a_paint_change_rebuilds_its_object() {
    let model = house();
    let (mut cache, _) = SceneCache::new(&model);
    let mut model = model;
    let oak = Paint { material: Some(MaterialRef::Item("material-oak".into())), ..Default::default() };
    for key in [
        "wall/w1/left",
        "wall/w1/right",
        "room/r0/floor",
        "room/r0/ceiling",
        "roof/r0",
        "cabinet/cb2/front",
        "cabinet/cb2/pull",
        "cabinet/cb3/carcass",
        "fixture/fx0",
        "furnishing/f2/0",
        "furnishing/f0/1",
    ] {
        model.paints.insert(key.into(), oak.clone());
        let stats = cache.update(&model);
        assert!(stats.incremental, "{key}");
        assert_eq!(stats.rebuilt, 1, "paint {key}: {stats:?}");
        assert_same(cache.scene(), &Scene3d::build(&model), key);
        // Blend: a tint on the same surface.
        if let Some(p) = model.paints.get_mut(key) {
            p.tint = Some([200, 180, 150]);
        }
        assert_eq!(cache.update(&model).rebuilt, 1, "tint {key}");
        assert_same(cache.scene(), &Scene3d::build(&model), key);
    }
    // Unpaint one.
    model.paints.remove("wall/w1/left");
    assert_eq!(cache.update(&model).rebuilt, 1);
    assert_same(cache.scene(), &Scene3d::build(&model), "unpainted");
    // A paint on an object that is not there rebuilds nothing.
    model.paints.insert("wall/w77/left".into(), oak.clone());
    assert_eq!(cache.update(&model).rebuilt, 0);
    // A key that names no surface rebuilds everything, still equal to the full build.
    model.paints.insert("wall/w1/top".into(), oak);
    let stats = cache.update(&model);
    assert!(!stats.incremental);
    assert_same(cache.scene(), &Scene3d::build(&model), "a key naming no surface");
}

#[test]
fn digest_includes_materials() {
    let mut model = base_model();
    model.furnishings.push(Furnishing { symbol: TWO.into(), ..cube_furnishing() });
    let scene = Scene3d::build(&model);
    assert_eq!(scene.materials.len(), 2);
    let digest = scene.digest();
    // Without its materials the scene digests as its meshes alone (as before phase 14).
    let bare = Scene3d { meshes: scene.meshes.clone(), ..Default::default() };
    let mut stripped = scene.clone();
    stripped.materials.clear();
    assert_eq!(stripped.digest(), bare.digest());
    assert_ne!(stripped.digest(), digest);
    // Each of base colour, texture key and tint reaches it.
    let key = format!("sym:{TWO}:1");
    let changed = |change: fn(&mut MaterialDef)| {
        let mut s = scene.clone();
        if let Some(m) = s.materials.get_mut(&key) {
            change(m);
        }
        s.digest()
    };
    assert_ne!(changed(|m| m.base_color[0] = 0.11), digest, "base colour");
    assert_ne!(changed(|m| m.texture = Some("sym:other:t0".into())), digest, "texture key");
    assert_ne!(changed(|m| m.texture = None), digest, "no texture");
    assert_ne!(changed(|m| m.tint = Some([255, 0, 0])), digest, "tint");
    assert_eq!(changed(|m| m.base_color[0] = 0.1), digest, "the same values give the same digest");
    // A renamed material is a different table.
    let mut renamed = scene.clone();
    if let Some(m) = renamed.materials.remove(&key) {
        renamed.materials.insert(format!("sym:{TWO}:9"), m);
    }
    assert_ne!(renamed.digest(), digest);
    // Still independent of mesh order.
    let mut reversed = scene.clone();
    reversed.meshes.reverse();
    assert_eq!(reversed.digest(), digest);
    // Equal meshes, different colours: a symbol re-coloured under its key.
    let mut recoloured = model.clone();
    if let Some(s) = recoloured.symbols.get_mut(TWO) {
        s.parts[0].base_color = [0.0, 1.0, 0.0, 1.0];
    }
    let other = Scene3d::build(&recoloured);
    assert_eq!(bare.digest(), Scene3d { meshes: other.meshes.clone(), ..Default::default() }.digest());
    assert_ne!(other.digest(), digest);
}

/// One seeded edit over furnishings (and what they touch); returns its name and the most objects
/// it may rebuild.
fn edit(m: &mut ArchModel, state: &mut u64) -> (&'static str, usize) {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let r = (*state >> 33) as usize;
    let d = if r & 1 == 0 { 0.15 } else { -0.15 };
    let cabinets = m.cabinets.len();
    let n = m.furnishings.len();
    match (r / 2) % 12 {
        0 if n > 0 => {
            let f = &mut m.furnishings[r % n];
            f.at[(r / 7) % 2] += d;
            ("move", 1 + cabinets)
        }
        1 if n > 0 => {
            m.furnishings[r % n].rotation += 37.0 * d.signum();
            ("rotate", 1 + cabinets)
        }
        2 if n > 0 => {
            let f = &mut m.furnishings[r % n];
            f.size[(r / 7) % 3] = (f.size[(r / 7) % 3] + d).max(0.2);
            ("resize", 1 + cabinets)
        }
        3 if n > 0 => {
            let f = &mut m.furnishings[r % n];
            f.mirrored = !f.mirrored;
            ("mirror", 1 + cabinets)
        }
        4 if n > 0 => {
            let f = &mut m.furnishings[r % n];
            f.level = 1 - f.level.min(1);
            f.elevation = (f.elevation + d).max(0.0);
            ("level and elevation", 1 + cabinets)
        }
        5 if n > 1 => {
            m.furnishings.remove(r % n);
            ("delete", cabinets)
        }
        6 => {
            let symbol = [CUBE, TWO, "fffffffffffffffe"][(r / 7) % 3];
            let at = [((r / 11) % 50) as f64 * 0.1, ((r / 13) % 30) as f64 * 0.1];
            m.push_furnishing(Furnishing { symbol: symbol.into(), at, rotation: ((r / 17) % 8) as f64 * 45.0, ..cube_furnishing() });
            ("place", 1)
        }
        7 if n > 0 => {
            let f = &mut m.furnishings[r % n];
            f.symbol = [CUBE, TWO, "fffffffffffffffe"][(r / 7) % 3].into();
            ("another symbol", 1 + cabinets)
        }
        8 => {
            // The two-part symbol leaves the drawing or comes back: every furnishing using it.
            let users = m.furnishings.iter().filter(|f| f.symbol == TWO).count();
            if m.symbols.remove(TWO).is_none() {
                m.symbols.insert(TWO.into(), two_part_symbol());
            }
            ("symbol presence", users + cabinets)
        }
        9 => {
            let ids: Vec<u32> = m.furnishings.iter().map(|f| f.id.0).collect();
            let keys = [
                ids.first().map(|i| format!("furnishing/f{i}/0")).unwrap_or_else(|| "furnishing/f0/0".into()),
                ids.last().map(|i| format!("furnishing/f{i}/1")).unwrap_or_else(|| "furnishing/f1/1".into()),
                "wall/w2/right".into(),
                "room/r0/floor".into(),
                "cabinet/cb1/counter".into(),
            ];
            let key = keys[(r / 7) % keys.len()].clone();
            if m.paints.remove(&key).is_none() {
                let material = [MaterialRef::Colour([180, 40, 20]), MaterialRef::Item("material-oak".into())][(r / 5) % 2].clone();
                let mut paint = Paint { material: Some(material), ..Default::default() };
                paint.tint = (r / 3).is_multiple_of(2).then_some([250, 240, 230]);
                m.paints.insert(key, paint);
            }
            ("paint", 1)
        }
        10 if n > 0 => {
            let f = &mut m.furnishings[r % n];
            f.host = match f.host {
                FurnishingHost::None => FurnishingHost::Cabinet { cabinet: ArchId(((r / 7) % 3) as u32) },
                _ => FurnishingHost::None,
            };
            f.cutout = if f.cutout.is_some() { None } else { Some([0.4, 0.3]) };
            ("host and cut-out", 1 + cabinets)
        }
        11 => {
            if m.images.is_empty() {
                m.images.insert("sky".into(), image(2, 2, (r % 200) as u8));
            }
            m.backdrop = match (r / 7) % 3 {
                0 => Some(Backdrop::Image { item: "sky".into(), projection: Projection::Equirect, horizon: 0.0 }),
                1 => Some(Backdrop::Colour { colour: [(r % 256) as u8, 30, 60] }),
                _ => None,
            };
            ("backdrop", 0)
        }
        _ => {
            let i = r % m.walls.len().max(1);
            if let Some(w) = m.walls.get_mut(i) {
                w.end.x += d * 0.1;
            }
            // The footprint under the roof changes: any wall on the level.
            ("move a wall end", m.walls.len())
        }
    }
}

#[test]
fn furnishing_edits_keep_the_cache_equal_to_the_full_build() {
    let mut model = house();
    let (mut cache, _) = SceneCache::new(&model);
    assert_same(cache.scene(), &Scene3d::build(&model), "start");
    assert_eq!(cache.scene().furnishings, 4);
    let mut state = 0x5eed_0014_u64;
    let mut kinds = std::collections::BTreeSet::new();
    for round in 0..40 {
        let (what, most) = edit(&mut model, &mut state);
        kinds.insert(what);
        let stats = cache.update(&model);
        let what = format!("edit {round}: {what}");
        assert_same(cache.scene(), &Scene3d::build(&model), &what);
        assert!(stats.incremental, "{what}");
        assert!(stats.rebuilt <= stats.objects, "{what}: {stats:?}");
        assert!(stats.rebuilt <= most, "{what}: rebuilt {} > {most}", stats.rebuilt);
    }
    assert!(kinds.len() >= 9, "the seed covers most kinds of edit: {kinds:?}");
}
