use super::*;
use crate::camera::Orbit;
use crate::gputest;
use crate::types::{ArchId, Mesh};

/// Build a scene with two rooms (two slabs side by side) so a render has real geometry to light.
fn two_room_scene() -> cadcraft_arch3d::Scene3d {
    let mut s = Scene3d { meshes: Vec::new(), ..Default::default() };
    // Two floor slabs, each a flat box, separated along +Y.
    s.meshes.push(cuboid("floor", [-5.0, -4.0, -0.3], [5.0, 0.0, 0.0]));
    s.meshes.push(cuboid("floor", [-5.0, 0.0, -0.3], [5.0, 4.0, 0.0]));
    // Walls around and between them.
    s.meshes.push(cuboid("wall", [-5.0, -4.0, 0.0], [-4.8, 4.0, 2.7]));
    s.meshes.push(cuboid("wall", [4.8, -4.0, 0.0], [5.0, 4.0, 2.7]));
    s.meshes.push(cuboid("wall", [-5.0, -4.0, 0.0], [5.0, -3.8, 2.7]));
    s.meshes.push(cuboid("wall", [-5.0, 3.8, 0.0], [5.0, 4.0, 2.7]));
    s.meshes.push(cuboid("wall", [-0.1, -4.0, 0.0], [0.1, 0.0, 2.7]));
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in s.meshes.iter().flat_map(|m| m.positions.iter()) {
        for k in 0..3 {
            lo[k] = lo[k].min(f64::from(p[k]));
            hi[k] = hi[k].max(f64::from(p[k]));
        }
    }
    if lo[0].is_finite() {
        s.bounds = crate::types::Bounds3::new(lo, hi);
    }
    s
}

/// An axis-aligned box mesh with outward normals; `material` picks its albedo.
fn cuboid(material: &str, min: [f32; 3], max: [f32; 3]) -> Mesh {
    let mut m = Mesh::new(material, ArchId(1));
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([1.0, 0.0, 0.0], [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]]),
        ([-1.0, 0.0, 0.0], [[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]]),
        ([0.0, 1.0, 0.0], [[x1, y1, z0], [x0, y1, z0], [x0, y1, z1], [x1, y1, z1]]),
        ([0.0, -1.0, 0.0], [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]]),
        ([0.0, 0.0, 1.0], [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]]),
        ([0.0, 0.0, -1.0], [[x0, y1, z0], [x1, y1, z0], [x1, y0, z0], [x0, y0, z0]]),
    ];
    for (n, quad) in faces {
        let base = m.positions.len() as u32;
        for p in quad {
            m.positions.push(p);
            m.normals.push(n);
            m.uvs.push([p[0], p[2]]);
        }
        m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    m
}

fn scene(meshes: Vec<Mesh>) -> Scene3d {
    let mut s = Scene3d { meshes, ..Default::default() };
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in s.meshes.iter().flat_map(|m| m.positions.iter()) {
        for k in 0..3 {
            lo[k] = lo[k].min(f64::from(p[k]));
            hi[k] = hi[k].max(f64::from(p[k]));
        }
    }
    if lo[0].is_finite() {
        s.bounds = crate::types::Bounds3::new(lo, hi);
    }
    s
}

fn renderer(size: (u32, u32)) -> Option<Offscreen> {
    let (device, queue) = gputest::shared_device()?;
    Some(Offscreen::with_device(device, queue, size))
}

fn px(rgba: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

fn clear_srgb() -> [u8; 4] {
    // CLEAR is linear; the sRGB target stores it encoded.
    let enc = |c: f64| {
        let s = if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
        (s * 255.0).round() as u8
    };
    [enc(CLEAR.r), enc(CLEAR.g), enc(CLEAR.b), 255]
}

fn close(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    (0..4).all(|k| a[k].abs_diff(b[k]) <= tol)
}

#[test]
fn headless_png_reports_no_adapter_instead_of_panicking() {
    let scene = cadcraft_arch3d::Scene3d::default();
    let camera = Camera::Orbit(Orbit::at_origin(30.0, 30.0, 10.0, 45.0));
    match render_png(&scene, &camera, &Light::DEFAULT, (64, 64)) {
        Ok(png) => assert_eq!(png_rgba(&png).expect("decode").len(), 64 * 64 * 4),
        Err(Render3dError::NoAdapter) => {}
        Err(e) => panic!("unexpected error: {e}"),
    }
}

#[test]
fn png_roundtrip_keeps_pixels() {
    let mut px = Vec::new();
    for y in 0..4u8 {
        for x in 0..4u8 {
            px.extend_from_slice(&[x * 60, y * 60, 128, 255]);
        }
    }
    let png = encode_png(4, 4, &px).expect("encode");
    assert_eq!(png_rgba(&png).expect("decode"), px);
    assert!(!pixels_are_uniform(&px));
}

#[test]
fn uniform_pixels_are_detected() {
    let px = vec![26u8; 8 * 8 * 4];
    assert!(pixels_are_uniform(&px));
    let mut px2 = px.clone();
    px2[100] = 200;
    assert!(!pixels_are_uniform(&px2));
}

#[test]
fn an_empty_scene_reads_back_the_clear_colour() {
    let Some(mut r) = renderer((32, 24)) else { return };
    r.render(&Scene3d::default(), 0, &Camera::Orbit(Orbit::at_origin(30.0, 30.0, 10.0, 45.0)), &Light::DEFAULT);
    let rgba = r.read_rgba().expect("readback");
    assert_eq!(rgba.len(), 32 * 24 * 4);
    assert!(pixels_are_uniform(&rgba));
    assert!(close(px(&rgba, 32, 0, 0), clear_srgb(), 1), "{:?} vs {:?}", px(&rgba, 32, 0, 0), clear_srgb());
}

#[test]
fn a_box_is_drawn_in_the_middle_and_the_corners_stay_clear() {
    let Some(mut r) = renderer((64, 64)) else { return };
    let s = scene(vec![cuboid("wall", [-1.0, -1.0, -1.0], [1.0, 1.0, 1.0])]);
    r.render(&s, 1, &Camera::Orbit(Orbit::at_origin(30.0, 30.0, 8.0, 45.0)), &Light::DEFAULT);
    let rgba = r.read_rgba().expect("readback");
    assert!(close(px(&rgba, 64, 0, 0), clear_srgb(), 1));
    assert!(close(px(&rgba, 64, 63, 63), clear_srgb(), 1));
    let mid = px(&rgba, 64, 32, 32);
    assert!(!close(mid, clear_srgb(), 8), "the box is missing at the centre: {mid:?}");
}

#[test]
fn faces_turned_differently_to_the_light_get_different_shades() {
    let Some(mut r) = renderer((96, 96)) else { return };
    let s = scene(vec![cuboid("wall", [-1.0, -1.0, -1.0], [1.0, 1.0, 1.0])]);
    // Seen from above the south-west corner: the top and two side faces are visible.
    r.render(&s, 1, &Camera::Orbit(Orbit::at_origin(225.0, 35.0, 7.0, 45.0)), &Light::new([0.0, 0.0, 1.0], 0.2));
    let rgba = r.read_rgba().expect("readback");
    let shades: std::collections::BTreeSet<[u8; 4]> =
        rgba.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).filter(|&p| !close(p, clear_srgb(), 8)).collect();
    // Light straight down: the top is fully lit, the sides only get ambient.
    assert!(shades.len() >= 2, "a lit box shows at least two distinct shades, got {shades:?}");
}

#[test]
fn the_nearer_of_two_boxes_wins_whatever_the_draw_order() {
    let Some(mut r) = renderer((64, 64)) else { return };
    // Camera on +Y looking at the origin (azimuth 0 = north, elevation 0).
    let cam = Camera::Orbit(Orbit::at_origin(0.0, 0.0, 10.0, 45.0));
    let light = Light::new([0.0, 1.0, 0.0], 0.3);
    let near = cuboid("glass", [-1.0, 1.0, -1.0], [1.0, 2.0, 1.0]);
    let far = cuboid("door", [-2.0, -3.0, -2.0], [2.0, -2.0, 2.0]);
    let mut seen = Vec::new();
    for (key, order) in [(1, vec![near.clone(), far.clone()]), (2, vec![far, near])] {
        r.render(&scene(order), key, &cam, &light);
        let rgba = r.read_rgba().expect("readback");
        seen.push(px(&rgba, 64, 32, 32));
    }
    assert_eq!(seen[0], seen[1], "draw order changed the visible surface: depth test missing");
    // Glass albedo has more blue than red; the door's is the other way round.
    assert!(seen[0][2] > seen[0][0], "the far box shows through: {:?}", seen[0]);
}

#[test]
fn a_face_seen_from_behind_is_lit_from_the_camera_side() {
    let Some(mut r) = renderer((48, 48)) else { return };
    // One horizontal quad with its normal up, seen from below and lit from below.
    let mut m = Mesh::new("ceiling", ArchId(2));
    for p in [[-2.0, -2.0, 0.0], [2.0, -2.0, 0.0], [2.0, 2.0, 0.0], [-2.0, 2.0, 0.0]] {
        m.positions.push(p);
        m.normals.push([0.0, 0.0, 1.0]);
        m.uvs.push([0.0, 0.0]);
    }
    m.indices = vec![0, 1, 2, 0, 2, 3];
    let s = scene(vec![m]);
    let cam = Camera::Orbit(Orbit::at_origin(10.0, -60.0, 6.0, 45.0));
    r.render(&s, 1, &cam, &Light::new([0.0, 0.0, -1.0], 0.1));
    let lit = px(&r.read_rgba().expect("readback"), 48, 24, 24);
    r.render(&s, 1, &cam, &Light::new([0.0, 0.0, 1.0], 0.1));
    let unlit = px(&r.read_rgba().expect("readback"), 48, 24, 24);
    assert!(lit[0] > unlit[0] + 40, "back face not flipped toward the camera: lit {lit:?} unlit {unlit:?}");
}

#[test]
fn glass_house_blends_and_differs_from_shaded() {
    let Some(mut r) = renderer((96, 96)) else { return };
    let cam = Camera::Orbit(Orbit::at_origin(25.0, 25.0, 12.0, 45.0));
    let scene = scene(vec![cuboid("door", [-3.0, -3.0, -1.0], [3.0, 0.0, 1.0]), cuboid("glass", [-2.0, 1.0, -1.0], [2.0, 3.0, 1.0])]);
    r.render(&scene, 1, &cam, &Light::DEFAULT);
    let shaded = r.read_rgba().expect("readback");
    r.render_with(&scene, 2, &cam, &Light::DEFAULT, &RenderOptions { style: Style::GlassHouse, ..Default::default() });
    let glass = r.read_rgba().expect("readback");
    let hues = |img: &[u8]| -> usize {
        let mut set = std::collections::BTreeSet::new();
        for p in img.chunks_exact(4) {
            if !close([p[0], p[1], p[2], p[3]], clear_srgb(), 8) {
                set.insert([p[0], p[1], p[2], p[3]]);
            }
        }
        set.len()
    };
    assert_ne!(glass, shaded, "glass-house style should change the image");
    assert!(hues(&glass) > hues(&shaded), "glass-house should produce more blended colours");
}

#[test]
fn glass_house_shows_an_interior_wall_through_an_exterior_one() {
    let Some(mut r) = renderer((96, 96)) else { return };
    let cam = Camera::Orbit(Orbit::at_origin(15.0, 0.0, 8.0, 35.0));
    let exterior = cuboid("wall", [-2.0, -3.0, -1.0], [0.0, 3.0, 1.0]);
    let without = scene(vec![exterior.clone()]);
    let mut with_partition = without.clone();
    with_partition.meshes.push(cuboid("wall", [-0.25, 2.5, -1.0], [1.0, 3.5, 1.0]));
    r.render_with(&without, 1, &cam, &Light::DEFAULT, &RenderOptions { style: Style::GlassHouse, ..Default::default() });
    let without_img = r.read_rgba().expect("readback");
    r.render_with(&with_partition, 2, &cam, &Light::DEFAULT, &RenderOptions { style: Style::GlassHouse, ..Default::default() });
    let with_img = r.read_rgba().expect("readback");
    let diff = without_img
        .chunks_exact(4)
        .zip(with_img.chunks_exact(4))
        .filter(|(a, b)| a[0].abs_diff(b[0]) > 8 || a[1].abs_diff(b[1]) > 8 || a[2].abs_diff(b[2]) > 8)
        .count();
    assert!(diff > 100, "the interior partition should change pixels seen through the exterior wall");
}

#[test]
fn glass_house_pipeline_writes_no_depth() {
    let desc = crate::pipeline::glass_pipeline_descriptor(4);
    assert_eq!(desc.sample_count, 4);
    assert!(!desc.depth_write);
}

#[test]
fn an_unchanged_scene_key_reuses_the_uploaded_meshes() {
    let Some(mut r) = renderer((16, 16)) else { return };
    let s = scene(vec![cuboid("wall", [-1.0, -1.0, -1.0], [1.0, 1.0, 1.0])]);
    let cam = Camera::Orbit(Orbit::at_origin(30.0, 30.0, 8.0, 45.0));
    for az in [0.0, 10.0, 20.0] {
        let cam = Camera::Orbit(Orbit::at_origin(az, 30.0, 8.0, 45.0));
        r.render(&s, 7, &cam, &Light::DEFAULT);
    }
    assert_eq!(r.uploads(), 1);
    r.render(&s, 8, &cam, &Light::DEFAULT);
    assert_eq!(r.uploads(), 2);
}

#[test]
fn resize_changes_the_readback_size_and_the_generation() {
    let Some(mut r) = renderer((16, 16)) else { return };
    let g = r.generation();
    assert!(!r.resize((16, 16)));
    assert!(r.resize((40, 10)));
    assert!(r.generation() > g);
    assert_eq!(r.size(), (40, 10));
    assert!(r.resize((0, 0)));
    assert_eq!(r.size(), (1, 1));
    r.render(&Scene3d::default(), 0, &Camera::Orbit(Orbit::at_origin(0.0, 30.0, 5.0, 45.0)), &Light::DEFAULT);
    assert_eq!(r.read_rgba().expect("readback").len(), 4);
}

#[test]
fn many_meshes_each_get_their_own_uniform_slot() {
    let Some(mut r) = renderer((64, 32)) else { return };
    // 40 boxes in a row with alternating materials: more draws than a single slot holds.
    let meshes: Vec<Mesh> = (0..40)
        .map(|i| {
            let x = i as f32 * 0.5 - 10.0;
            cuboid(if i % 2 == 0 { "glass" } else { "door" }, [x, -0.2, -2.0], [x + 0.4, 0.2, 2.0])
        })
        .collect();
    let s = scene(meshes);
    let cam = Camera::Orbit(Orbit::at_origin(0.0, 0.0, 14.0, 45.0));
    r.render(&s, 3, &cam, &Light::new([0.0, 1.0, 0.0], 0.3));
    let rgba = r.read_rgba().expect("readback");
    let row: Vec<[u8; 4]> = (0..64).map(|x| px(&rgba, 64, x, 16)).filter(|p| !close(*p, clear_srgb(), 8)).collect();
    let bluish = row.iter().filter(|p| p[2] > p[0]).count();
    let reddish = row.iter().filter(|p| p[0] > p[2]).count();
    assert!(bluish > 0 && reddish > 0, "every draw used one albedo: {row:?}");
}

#[test]
fn readback_returns_in_bounded_time() {
    let Some(mut r) = renderer((256, 256)) else { return };
    let start = std::time::Instant::now();
    for _ in 0..5 {
        r.render(&Scene3d::default(), 0, &Camera::Orbit(Orbit::at_origin(0.0, 30.0, 5.0, 45.0)), &Light::DEFAULT);
        r.read_rgba().expect("readback");
    }
    assert!(start.elapsed() < READBACK_TIMEOUT, "five readbacks took {:?}", start.elapsed());
}

/// A box the way arch3d builds walls and slabs: 8 shared corners, each with one averaged normal.
fn shared_corner_box(material: &str, min: [f32; 3], max: [f32; 3]) -> Mesh {
    let mut m = Mesh::new(material, ArchId(3));
    let c = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0, (min[2] + max[2]) / 2.0];
    for i in 0..8u32 {
        let p = [if i & 1 == 0 { min[0] } else { max[0] }, if i & 2 == 0 { min[1] } else { max[1] }, if i & 4 == 0 { min[2] } else { max[2] }];
        let d = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        m.positions.push(p);
        m.normals.push([d[0] / l, d[1] / l, d[2] / l]);
        m.uvs.push([p[0], p[2]]);
    }
    // -y, +y, -x, +x, -z, +z faces as two triangles each.
    m.indices = vec![0, 1, 5, 0, 5, 4, 2, 6, 7, 2, 7, 3, 0, 4, 6, 0, 6, 2, 1, 3, 7, 1, 7, 5, 0, 2, 3, 0, 3, 1, 4, 5, 7, 4, 7, 6];
    m
}

#[test]
fn a_flat_face_is_one_colour_even_with_averaged_corner_normals() {
    let Some(mut r) = renderer((64, 64)) else { return };
    // Looking straight at the box's south face (camera due south, level), lit from the south-west and above.
    let s = scene(vec![shared_corner_box("wall", [-4.0, -1.0, -4.0], [4.0, 1.0, 4.0])]);
    r.render(&s, 1, &Camera::Orbit(Orbit::at_origin(180.0, 0.0, 12.0, 45.0)), &Light::new([-0.5, -0.5, 0.7], 0.25));
    let rgba = r.read_rgba().expect("readback");
    let patch: std::collections::BTreeSet<[u8; 4]> = (24..40).flat_map(|y| (24..40).map(move |x| (x, y))).map(|(x, y)| px(&rgba, 64, x, y)).collect();
    assert_eq!(patch.len(), 1, "one planar face shows {} colours: {patch:?}", patch.len());
}

/// Fraction of pixels in an offscreen render that differ from the clear colour.
/// `None` when no GPU adapter is available (the render can't run).
fn lit_fraction(scene: &Scene3d, key: u64, cam: &Camera, light: &Light, size: (u32, u32)) -> Option<f64> {
    let Some((device, queue)) = gputest::shared_device() else { return None };
    let mut r = Offscreen::with_device(device, queue, size);
    r.render(scene, key, cam, light);
    let rgba = r.read_rgba().ok()?;
    let total = rgba.len() / 4;
    let lit = rgba
        .chunks_exact(4)
        .filter(|p| {
            let px: [u8; 4] = [p[0], p[1], p[2], p[3]];
            !close(px, clear_srgb(), 8)
        })
        .count();
    Some(lit as f64 / total as f64)
}

#[test]
fn offscreen_render_of_a_two_room_model_has_lit_pixels() {
    let scene = two_room_scene();
    let cam = Camera::Orbit(Orbit::at_origin(45.0, 30.0, 30.0, 45.0));
    match lit_fraction(&scene, 1, &cam, &Light::DEFAULT, (320, 200)) {
        None => println!("offscreen_render_of_a_two_room_model_has_lit_pixels: skipped (no GPU adapter)"),
        Some(frac) => assert!(frac > 0.02, "only {frac} of pixels are lit; the scene did not draw"),
    }
}

#[test]
fn projected_scene_bounds_match_the_rendered_silhouette_where_the_bounding_box_does_not() {
    let Some(mut r) = renderer((240, 160)) else { return };
    // A long low slab plus a tall post at one end: most corners of their joint bounding box are empty space.
    let s = scene(vec![cuboid("floor", [0.0, 0.0, -0.3], [10.0, 8.0, 0.0]), cuboid("wall", [9.0, 7.0, 0.0], [10.0, 8.0, 6.0])]);
    let cam = Camera::Orbit(Orbit::at_origin(35.0, 30.0, 28.0, 45.0));
    r.render(&s, 1, &cam, &Light::DEFAULT);
    let rgba = r.read_rgba().expect("readback");
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..160 {
        for x in 0..240 {
            if !close(px(&rgba, 240, x, y), clear_srgb(), 8) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    let tight = crate::camera::projected_scene_bounds(&cam, &s, (240, 160));
    let boxed = crate::camera::projected_bounds(&cam, &s.bounds, (240, 160));
    let near = |a: u32, b: f32| (a as f32 - b).abs() <= 1.5;
    assert!(
        near(x0, tight[0]) && near(y0, tight[1]) && near(x1, tight[2]) && near(y1, tight[3]),
        "silhouette {:?} vs projected vertices {tight:?}",
        (x0, y0, x1, y1)
    );
    let gap = (0..4).map(|k| (tight[k] - boxed[k]).abs()).fold(0.0f32, f32::max);
    assert!(gap > 3.0, "the scene fills its bounding box; this test needs one that does not ({tight:?} vs {boxed:?})");
}

/// A smooth vertical cylinder (radius 1, z -2..2, `sides` faces, one shared vertex per corner with a radial normal).
fn smooth_cylinder(sides: u32) -> Mesh {
    let mut m = Mesh::new("wall", ArchId(1));
    for k in 0..sides {
        let a = std::f32::consts::TAU * k as f32 / sides as f32;
        let (x, y) = (a.cos(), a.sin());
        for z in [-2.0_f32, 2.0] {
            m.positions.push([x, y, z]);
            m.normals.push([x, y, 0.0]);
            m.uvs.push([x, z]);
        }
    }
    for k in 0..sides {
        let n = (k + 1) % sides;
        let (b0, t0, b1, t1) = (2 * k, 2 * k + 1, 2 * n, 2 * n + 1);
        m.indices.extend_from_slice(&[b0, b1, t1, b0, t1, t0]);
    }
    m
}

/// Phase-16 check 18 (technical silhouettes): a smooth surface's outline is drawn in the technical line colour on both
/// sides, its middle stays the flat fill. The silhouette edges lie exactly on the outline, where the adjacent front face
/// is nearer the camera than the edge; without a slope-scaled depth bias on the fill they lose the depth test and no
/// line shows (2026-11-10: 0 of 120 rows of the sample desk lamp's stem were outlined).
#[test]
fn technical_outlines_a_smooth_cylinder() {
    let Some(mut r) = renderer((96, 96)) else { return };
    let s = scene(vec![smooth_cylinder(32)]);
    let cam = Camera::Orbit(Orbit::at_origin(-90.0, 0.0, 12.0, 45.0));
    r.render_with(&s, 1, &cam, &Light::DEFAULT, &RenderOptions { style: Style::Technical, ..Default::default() });
    let img = r.read_rgba().expect("readback");
    let lum = |p: [u8; 4]| 0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2]);
    let bg = clear_srgb();
    let (mut rows, mut lined, mut inside) = (0, 0, 0);
    for y in 30..66 {
        let xs: Vec<u32> = (0..96).filter(|&x| !close(px(&img, 96, x, y), bg, 12)).collect();
        let (Some(&xl), Some(&xr)) = (xs.first(), xs.last()) else { continue };
        if xr - xl < 8 {
            continue;
        }
        rows += 1;
        let dark = |x: u32| lum(px(&img, 96, x, y)) < 60.0;
        lined += usize::from((xl..xl + 2).any(dark) && (xr - 1..=xr).any(dark));
        let third = (xr - xl) / 3;
        inside += usize::from((xl + third..=xr - third).any(dark));
    }
    assert!(rows >= 20, "the cylinder is drawn across the middle rows: {rows}");
    assert!(lined * 10 >= rows * 8, "both sides of the outline carry the line colour on most rows: {lined} of {rows}");
    assert!(inside * 10 <= rows * 2, "the smooth middle draws no lines: {inside} of {rows} rows have a dark pixel there");
}

// ---- phase 19: exposure -------------------------------------------------------------------------

use crate::exposure::tone_map;
use crate::lit_frame::Lighting;
use cadcraft_doc::arch::{ExposureSetting, Look, ToneMap};
use cadcraft_geom::Vec3;

fn srgb_to_linear(c: u8) -> f32 {
    let v = f32::from(c) / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// A sun straight overhead of 100 000 lx, no sky, the calibration albedo 0.5 (L = 15 915.5 nits),
/// a manual exposure and a tone curve.
pub(crate) fn plane_lighting(ev100: f64, tone: ToneMap) -> Lighting {
    let mut look = Look::default();
    look.exposure = ExposureSetting::Manual { ev100 };
    look.tone_map = tone;
    look.sky.on = false;
    Lighting { sun: Some(Light { dir: [0.0, 0.0, 1.0], ..Light::DEFAULT }), look, lambert: Some(0.5), meter_night: true, ..Lighting::default() }
}

/// Looking straight down at `target`, a metre away.
fn down_at(target: [f64; 3]) -> Camera {
    let mut o = Orbit::at_origin(0.0, 89.0, 1.0, 45.0);
    o.target = Vec3::new(target[0], target[1], target[2]);
    Camera::Orbit(o)
}

/// The centre pixel of an interactive lit frame, as linear.
fn lit_centre(r: &mut Offscreen, s: &Scene3d, camera: &Camera, lighting: &Lighting, ground: bool) -> Option<[f32; 3]> {
    lit_centre_keyed(r, s, 1, camera, lighting, ground)
}

/// [`lit_centre`] for a scene uploaded under `key` (distinct scenes need distinct keys).
fn lit_centre_keyed(r: &mut Offscreen, s: &Scene3d, key: u64, camera: &Camera, lighting: &Lighting, ground: bool) -> Option<[f32; 3]> {
    let eye = camera.eye();
    let frame = r.lit_frame(s, key, [eye.x, eye.y, eye.z], lighting, ground)?;
    let light = lighting.sun.unwrap_or(Light::DEFAULT);
    r.render_with(s, key, camera, &light, &RenderOptions { lit: Some(&frame), ground, ..Default::default() });
    let (w, h) = r.size();
    let img = r.read_rgba().ok()?;
    let p = px(&img, w, w / 2, h / 2);
    Some([srgb_to_linear(p[0]), srgb_to_linear(p[1]), srgb_to_linear(p[2])])
}

fn near(got: f32, want: f32, rel: f32) -> bool {
    (got - want).abs() <= want.abs() * rel + 0.004
}

#[test]
fn lambert_albedo_overrides_ground_quad_and_texture() {
    let Some(mut r) = renderer((32, 32)) else { return };
    // The ground quad (albedo 0.75 and a GGX lobe) only: the cube is far from where the camera looks.
    let far = scene(vec![cuboid("wall", [10.0, 10.0, 0.0], [11.0, 11.0, 1.0])]);
    let want = tone_map(15_915.5 * crate::exposure::ev100_scale(14.0), crate::exposure::TONE_NONE);
    let lighting = plane_lighting(14.0, ToneMap::None);
    let Some(g) = lit_centre(&mut r, &far, &down_at([12.0, 12.0, 0.0]), &lighting, true) else { return };
    for c in g {
        assert!(near(c, want, 0.02), "ground quad: {g:?} want {want}");
    }
    // A floor mesh, whose material albedo is brown and which takes a finish texture.
    let floor = scene(vec![cuboid("floor", [-5.0, -5.0, -0.3], [5.0, 5.0, 0.0])]);
    let Some(f) = lit_centre(&mut r, &floor, &down_at([0.0, 0.0, 0.0]), &lighting, false) else { return };
    for c in f {
        assert!(near(c, want, 0.02), "floor: {f:?} want {want}");
    }
    // Without the override the same floor is not grey 0.5.
    let plain = Lighting { lambert: None, ..lighting };
    let Some(p) = lit_centre(&mut r, &floor, &down_at([0.0, 0.0, 0.0]), &plain, false) else { return };
    assert!((p[0] - p[2]).abs() > 0.02, "the floor material is not neutral: {p:?}");
}

#[test]
fn raster_clip_discards_and_near_hides() {
    use cadcraft_doc::arch::ClipBox;
    let Some(mut r) = renderer((32, 32)) else { return };
    // A floor, and a ceiling slab 2 m over it; the camera is 5 m above the floor looking down.
    let both = scene(vec![cuboid("floor", [-8.0, -8.0, -0.3], [8.0, 8.0, 0.0]), cuboid("wall", [-8.0, -8.0, 2.0], [8.0, 8.0, 2.1])]);
    let camera = Camera::Orbit(Orbit::at_origin(0.0, 89.0, 5.0, 45.0));
    let mut lighting = plane_lighting(14.0, ToneMap::None);
    lighting.lambert = None;
    let Some(ceiling) = lit_centre_keyed(&mut r, &both, 10, &camera, &lighting, false) else { return };
    let floor_only = scene(vec![cuboid("floor", [-8.0, -8.0, -0.3], [8.0, 8.0, 0.0])]);
    let Some(floor) = lit_centre_keyed(&mut r, &floor_only, 11, &camera, &lighting, false) else { return };
    assert!((ceiling[0] - floor[0]).abs() + (ceiling[2] - floor[2]).abs() > 0.02, "ceiling {ceiling:?} and floor {floor:?} differ");

    // A clip box below the ceiling discards it (the box is inclusive) and shows the floor.
    lighting.look.clip = Some(ClipBox { min: [-20.0; 3], max: [20.0, 20.0, 1.5], ..Default::default() });
    let Some(clipped) = lit_centre_keyed(&mut r, &both, 10, &camera, &lighting, false) else { return };
    for k in 0..3 {
        assert!(near(clipped[k], floor[k], 0.01), "clipped {clipped:?} want the floor {floor:?}");
    }
    // A box holding everything (the floor's top lies exactly on its lower face) changes nothing.
    lighting.look.clip = Some(ClipBox { min: [-20.0, -20.0, 0.0], max: [20.0, 20.0, 2.1], ..Default::default() });
    let Some(held) = lit_centre_keyed(&mut r, &both, 10, &camera, &lighting, false) else { return };
    for k in 0..3 {
        assert!(near(held[k], ceiling[k], 0.01), "inclusive box {held:?} want the ceiling {ceiling:?}");
    }

    // A near clip of 4 m removes the ceiling (about 2.9 m away) and keeps the floor (5 m).
    lighting.look.clip = None;
    lighting.look.near_clip = Some(4.0);
    let Some(near4) = lit_centre_keyed(&mut r, &both, 10, &camera, &lighting, false) else { return };
    for k in 0..3 {
        assert!(near(near4[k], floor[k], 0.01), "near 4 {near4:?} want the floor {floor:?}");
    }
    // Beyond everything: only the background is left.
    lighting.look.near_clip = Some(8.0);
    let Some(gone) = lit_centre_keyed(&mut r, &both, 10, &camera, &lighting, false) else { return };
    let Some(empty) = lit_centre_keyed(&mut r, &scene(Vec::new()), 12, &camera, &lighting, false) else { return };
    for k in 0..3 {
        assert!(near(gone[k], empty[k], 0.01), "near 8 {gone:?} want the background {empty:?}");
    }
    assert_eq!(r.last_frame_look().near, Some(8.0));
}

#[test]
fn raster_lambert_plane_reads_ev_scaled_luminance() {
    let Some(mut r) = renderer((32, 32)) else { return };
    let floor = scene(vec![cuboid("floor", [-5.0, -5.0, -0.3], [5.0, 5.0, 0.0])]);
    let camera = down_at([0.0, 0.0, 0.0]);
    for (ev, tone) in [(14.0, ToneMap::None), (15.0, ToneMap::None), (14.0, ToneMap::Aces), (14.0, ToneMap::Hable)] {
        let code = crate::exposure::tone_code(tone);
        let want = tone_map(15_915.5 * crate::exposure::ev100_scale(ev), code);
        let Some(got) = lit_centre(&mut r, &floor, &camera, &plane_lighting(ev, tone), false) else { return };
        for c in got {
            assert!(near(c, want, 0.02), "EV100 {ev} {tone:?}: {got:?} want {want}");
        }
    }
    // An automatic exposure with the albedo of the check (c) reads middle grey.
    let mut auto = plane_lighting(14.0, ToneMap::None);
    auto.look.exposure = ExposureSetting::Auto;
    auto.lambert = Some(0.05);
    let Some(got) = lit_centre(&mut r, &floor, &camera, &auto, false) else { return };
    assert!(near(got[0], 0.18, 0.03), "auto: {got:?}");
}

fn strip_raster_light() -> cadcraft_arch3d::light::SceneLight {
    let pieces = 14usize;
    let samples: Vec<cadcraft_arch3d::light::StripSample> = (0..pieces)
        .map(|k| {
            let t = (k as f64 + 0.5) / pieces as f64;
            cadcraft_arch3d::light::StripSample {
                position: Vec3::new(1.0 + (3.0 - 1.0) * t, 2.0, 2.4),
                normal: Vec3::new(0.0, 0.0, -1.0),
                intensity: 500.0 * (3.0 - 1.0) / pieces as f64 / std::f64::consts::PI,
                ..Default::default()
            }
        })
        .collect();
    cadcraft_arch3d::light::SceneLight {
        kind: cadcraft_doc::arch::LightKind::Strip,
        position: Vec3::new(2.0, 2.0, 2.4),
        direction: Vec3::new(0.0, 0.0, -1.0),
        color: [1.0, 0.8, 0.6],
        strip: Some(std::sync::Arc::new(samples)),
        ..Default::default()
    }
}

#[test]
fn raster_strip_lights_the_floor() {
    let Some(mut r) = renderer((32, 32)) else { return };
    let floor = scene(vec![cuboid("floor", [-5.0, -5.0, -0.3], [5.0, 5.0, 0.0])]);
    let camera = down_at([2.0, 2.0, 0.0]);
    let mut plain = floor.clone();
    plain.lights.clear();
    let mut lit = floor.clone();
    lit.lights.push(strip_raster_light());

    let mut lighting = plane_lighting(14.0, ToneMap::None);
    lighting.sun = None;
    let Some(plain_px) = lit_centre_keyed(&mut r, &plain, 21, &camera, &lighting, false) else { return };
    let Some(strip_px) = lit_centre_keyed(&mut r, &lit, 22, &camera, &lighting, false) else { return };

    let plain_sum: f32 = plain_px.iter().sum();
    let strip_sum: f32 = strip_px.iter().sum();
    assert!(strip_sum > plain_sum, "strip: {strip_px:?} plain: {plain_px:?}");
    assert!(strip_px.iter().zip(plain_px.iter()).all(|(a, b)| *a > *b), "strip should brighten the floor under it: {strip_px:?} vs {plain_px:?}");
}

// ---- phase 19: environment light in the raster views ---------------------------------------------

/// A night frame lit only by a uniform environment of radiance 1 (so E = pi), Lambert albedo 0.5.
fn env_lighting(ev100: f64, tone: ToneMap, visible: bool) -> Lighting {
    let mut look = Look::default();
    look.exposure = ExposureSetting::Manual { ev100 };
    look.tone_map = tone;
    let mut env = crate::environment::Environment::uniform([1.0; 3], 32, 16);
    env.visible = visible;
    env.key = env.compute_key();
    Lighting { sun: None, look, lambert: Some(0.5), meter_night: true, environment: Some(std::sync::Arc::new(env)), ..Lighting::default() }
}

#[test]
fn raster_lambert_plane_under_uniform_environment() {
    let Some(mut r) = renderer((32, 32)) else { return };
    let floor = scene(vec![cuboid("floor", [-5.0, -5.0, -0.3], [5.0, 5.0, 0.0])]);
    // L = albedo / pi * E = 0.5; EV100 -1 scales by 2 / 1.2.
    let Some(got) = lit_centre(&mut r, &floor, &down_at([0.0, 0.0, 0.0]), &env_lighting(-1.0, ToneMap::None, false), false) else { return };
    for c in got {
        assert!((c - 0.8333).abs() <= 0.8333 * 0.03, "plane under the environment: {got:?}");
    }
}

#[test]
fn environment_background_exposed_and_tone_mapped() {
    let Some(mut r) = renderer((32, 32)) else { return };
    let empty = scene(Vec::new());
    let camera = down_at([0.0, 0.0, 0.0]);
    let Some(got) = lit_centre(&mut r, &empty, &camera, &env_lighting(0.0, ToneMap::None, true), false) else { return };
    for c in got {
        assert!((c - 0.8333).abs() <= 0.8333 * 0.02, "background: {got:?}");
    }
    let want = tone_map(1.0 / 1.2, crate::exposure::TONE_ACES);
    let Some(aces) = lit_centre(&mut r, &empty, &camera, &env_lighting(0.0, ToneMap::Aces, true), false) else { return };
    for c in aces {
        assert!((c - want).abs() <= want * 0.02, "ACES background: {aces:?} want {want}");
    }
    // A visible environment wins over a picture backdrop.
    let mut pic = scene(Vec::new());
    pic.backdrop = Some(cadcraft_doc::arch::Backdrop::Colour { colour: [32, 64, 96] });
    let Some(over) = lit_centre(&mut r, &pic, &camera, &env_lighting(0.0, ToneMap::None, true), false) else { return };
    assert!((over[0] - 0.8333).abs() <= 0.8333 * 0.02, "environment over picture: {over:?}");
}

#[test]
fn picture_backdrop_unchanged_with_light_only_env() {
    let Some(mut r) = renderer((32, 32)) else { return };
    let mut pic = scene(Vec::new());
    pic.backdrop = Some(cadcraft_doc::arch::Backdrop::Colour { colour: [32, 64, 96] });
    let Some(got) = lit_centre(&mut r, &pic, &down_at([0.0, 0.0, 0.0]), &env_lighting(0.0, ToneMap::Aces, false), false) else { return };
    for (c, byte) in got.iter().zip([32u8, 64, 96]) {
        let want = srgb_to_linear(byte);
        assert!((c - want).abs() <= want * 0.03 + 0.002, "picture colour kept: {got:?}");
    }
}
