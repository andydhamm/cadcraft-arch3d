//! GPU tests of the path tracer's rough and thin transmission (`shaders/pt/bsdf_trans.wgsl`):
//! the white furnace, Snell's law, the pdf's integral and a closed rough-glass sphere in a uniform
//! environment. They run the kernel's test entries through [`run_entry`] on the shared compute
//! device and skip (with a SKIP line) without one.

use super::scene_gpu::{
    FRAME_ENV, FRAME_FURNACE, FRAME_SUN, GpuFrame, GpuLight, GpuMaterial, GpuProbe, GpuScene, LIGHT_POINT, MAT_THIN, MAT_TRANS, PHOTO_MATERIALS,
    build_gpu_scene,
};
use super::tracer::{run_entry, upload_scene};
use crate::types::{ArchId, Mesh, Scene3d};

const Z: [f32; 3] = [0.0, 0.0, 1.0];

fn rough_glass(albedo: [f32; 3], roughness: f32, transmission: f32) -> GpuMaterial {
    GpuMaterial { albedo, roughness, ior: 1.5, transmission, flags: MAT_TRANS, ..Default::default() }
}

fn thin_sheet(albedo: [f32; 3], roughness: f32, transmission: f32) -> GpuMaterial {
    GpuMaterial { flags: MAT_TRANS | MAT_THIN, ..rough_glass(albedo, roughness, transmission) }
}

/// The unit direction with cosine `c` to +z, toward +x.
fn with_cos(c: f64) -> [f32; 3] {
    [(1.0 - c * c).max(0.0).sqrt() as f32, 0.0, c as f32]
}

fn probe(material: usize, wo: [f32; 3], samples: u32) -> GpuProbe {
    GpuProbe { point: wo, _a: material as f32, normal: Z, _b: samples as f32, ..Default::default() }
}

/// Run `entry` over `probes` on a scene of `materials` with `frame` (probe count and seed set
/// here); `None` after a SKIP line without a compute device.
fn run_with(entry: &str, materials: Vec<GpuMaterial>, probes: &[GpuProbe], frame: GpuFrame) -> Option<Vec<[f32; 4]>> {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("SKIP: no compute device");
        return None;
    };
    let scene = GpuScene { materials, ..Default::default() };
    let buffers = upload_scene(&gpu, &scene);
    let frame = GpuFrame { probe_count: probes.len() as u32, seed: 11, ..frame };
    match run_entry(&gpu, &buffers, entry, &frame, probes) {
        Ok(out) => {
            assert_eq!(out.len(), probes.len());
            Some(out)
        }
        Err(e) => {
            assert!(false, "{entry}: {e}");
            None
        }
    }
}

fn photo_frame() -> GpuFrame {
    GpuFrame { photo: [PHOTO_MATERIALS, 0, 0, 0], ..Default::default() }
}

const FURNACE_COSINES: [f64; 3] = [0.2, 0.5, 0.9];

fn furnace_means(material: GpuMaterial) -> Option<Vec<[f32; 4]>> {
    let probes: Vec<GpuProbe> = FURNACE_COSINES.iter().map(|c| probe(0, with_cos(*c), 16384)).collect();
    run_with("bsdf_test_main", vec![material], &probes, photo_frame())
}

#[test]
fn trans_inactive_without_the_materials_flag() {
    // Without PHOTO_MATERIALS a MAT_TRANS material is the plain opaque material: the same samples.
    let probes: Vec<GpuProbe> = FURNACE_COSINES.iter().map(|c| probe(0, with_cos(*c), 256)).collect();
    let plain = GpuMaterial { flags: 0, ..thin_sheet([0.8, 0.6, 0.4], 0.5, 0.6) };
    let Some(want) = run_with("bsdf_test_main", vec![plain], &probes, GpuFrame::default()) else {
        return;
    };
    let flagged = thin_sheet([0.8, 0.6, 0.4], 0.5, 0.6);
    let Some(off) = run_with("bsdf_test_main", vec![flagged], &probes, GpuFrame::default()) else {
        return;
    };
    assert_eq!(off, want, "MAT_TRANS without the materials flag changes nothing");
    // With it, a thin sheet sends some samples through (the far side): never below the surface otherwise.
    let dir_probes: Vec<GpuProbe> = (0..64).map(|_| probe(0, with_cos(0.5), 1)).collect();
    let Some(dirs_off) = run_with("bsdf_dir_test_main", vec![flagged], &dir_probes, GpuFrame::default()) else {
        return;
    };
    assert!(dirs_off.iter().all(|d| d[2] > 0.0), "inactive: no sample leaves through the back");
    let Some(dirs_on) = run_with("bsdf_dir_test_main", vec![flagged], &dir_probes, photo_frame()) else {
        return;
    };
    assert!(dirs_on.iter().any(|d| d[2] < 0.0), "active: some samples pass through the sheet");
}

#[test]
fn rough_transmission_furnace_conserves_energy() {
    let Some(out) = furnace_means(rough_glass([1.0; 3], 0.3, 1.0)) else {
        return;
    };
    for (o, c) in out.iter().zip(FURNACE_COSINES) {
        for k in 0..3 {
            assert!((0.95..=1.02).contains(&o[k]), "NoV {c} channel {k}: mean weight {}", o[k]);
        }
    }
    // A mixture with the opaque base keeps it too.
    let Some(out) = furnace_means(rough_glass([1.0; 3], 0.3, 0.6)) else {
        return;
    };
    for (o, c) in out.iter().zip(FURNACE_COSINES) {
        // The opaque base's own grazing loss (bsdf_lobes) shows through here.
        assert!((0.93..=1.02).contains(&o[0]), "NoV {c} (transmission 0.6): mean weight {}", o[0]);
    }
}

#[test]
fn thin_sheet_furnace_conserves_energy() {
    let Some(out) = furnace_means(thin_sheet([1.0; 3], 0.5, 0.6)) else {
        return;
    };
    for (o, c) in out.iter().zip(FURNACE_COSINES) {
        for k in 0..3 {
            assert!((0.95..=1.02).contains(&o[k]), "NoV {c} channel {k}: mean weight {}", o[k]);
        }
    }
}

#[test]
fn refraction_follows_snell() {
    // A near-smooth rough glass; stratified lobe choice (u.y = u.z = 0.5, the middle microfacet).
    let count = 64usize;
    let cos_i = 0.7f64;
    let sin_i = (1.0 - cos_i * cos_i).sqrt();
    let probes: Vec<GpuProbe> = (0..count).map(|_| probe(0, with_cos(cos_i), 1)).collect();
    let Some(out) = run_with("bsdf_dir_test_main", vec![rough_glass([1.0; 3], 0.02, 1.0)], &probes, photo_frame()) else {
        return;
    };
    let (mut refracted, mut reflected) = (0, 0);
    for d in &out {
        let dir = [f64::from(d[0]), f64::from(d[1]), f64::from(d[2])];
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-3, "unit direction, got {dir:?}");
        if dir[2] < 0.0 {
            refracted += 1;
            // Continues on the far side: toward -x, sin_t = sin_i / 1.5, in the plane of incidence.
            assert!((dir[0] + sin_i / 1.5).abs() < 6e-3, "sin theta_t: got {}, want {}", -dir[0], sin_i / 1.5);
            assert!(dir[1].abs() < 6e-3, "stays in the plane of incidence: {}", dir[1]);
        } else {
            reflected += 1;
            assert!((dir[0] + sin_i).abs() < 6e-3 && dir[2] > 0.0, "mirror reflection: {dir:?}");
        }
    }
    assert!(refracted > count / 2 && reflected >= 1, "a split of {refracted} refractions and {reflected} reflections");
}

/// ∫ trans_pdf dω over the sphere for `material`, seen from cosine `cos_o` to the normal, by an
/// equal-area grid over the directions.
fn pdf_integral(material: GpuMaterial, cos_o: f64) -> Option<f64> {
    let (nm, np) = (256usize, 512usize);
    let mut probes = Vec::with_capacity(nm * np);
    for i in 0..nm {
        let mu = -1.0 + 2.0 * (i as f64 + 0.5) / nm as f64;
        let s = (1.0 - mu * mu).sqrt();
        for j in 0..np {
            let phi = 2.0 * std::f64::consts::PI * (j as f64 + 0.5) / np as f64;
            probes.push(probe(0, [(s * phi.cos()) as f32, (s * phi.sin()) as f32, mu as f32], 1));
        }
    }
    let wo = with_cos(cos_o);
    let frame = GpuFrame { sun_dir: [wo[0], wo[1], wo[2], 0.0], ..photo_frame() };
    let out = run_with("trans_pdf_test_main", vec![material], &probes, frame)?;
    let cell = (2.0 / nm as f64) * (2.0 * std::f64::consts::PI / np as f64);
    Some(out.iter().map(|o| f64::from(o[3])).sum::<f64>() * cell)
}

#[test]
fn trans_pdf_integrates_to_one_over_the_sphere() {
    for (name, material) in [
        ("rough glass", rough_glass([1.0; 3], 0.3, 1.0)),
        ("rough glass mixed", rough_glass([1.0; 3], 0.3, 0.6)),
        ("thin sheet", thin_sheet([1.0; 3], 0.5, 0.6)),
    ] {
        let Some(total) = pdf_integral(material, 0.8) else {
            return;
        };
        assert!((total - 1.0).abs() < 0.03, "{name}: the pdf integrates to {total}");
    }
}

/// A sphere of radius `r` at the origin (an octahedron subdivided `levels` times, outward winding).
fn sphere(material: &str, r: f32, levels: u32) -> Mesh {
    let mut tris: Vec<[[f32; 3]; 3]> = Vec::new();
    let (px, nx) = ([1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]);
    let (py, ny) = ([0.0, 1.0, 0.0], [0.0, -1.0, 0.0]);
    let (pz, nz) = ([0.0, 0.0, 1.0], [0.0, 0.0, -1.0]);
    for (a, b, c) in [(px, py, pz), (py, nx, pz), (nx, ny, pz), (ny, px, pz), (py, px, nz), (nx, py, nz), (ny, nx, nz), (px, ny, nz)] {
        tris.push([a, b, c]);
    }
    let mid = |a: [f32; 3], b: [f32; 3]| {
        let m = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5, (a[2] + b[2]) * 0.5];
        let l = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt();
        [m[0] / l, m[1] / l, m[2] / l]
    };
    for _ in 0..levels {
        tris = tris
            .iter()
            .flat_map(|&[a, b, c]| {
                let (ab, bc, ca) = (mid(a, b), mid(b, c), mid(c, a));
                [[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]
            })
            .collect();
    }
    let mut mesh = Mesh::new(material, ArchId(1));
    for [a, b, c] in tris {
        let base = mesh.positions.len() as u32;
        // Every winding is outward by construction (the octants above are counter-clockwise seen
        // from outside); the corner order a, b, c is kept.
        for v in [a, b, c] {
            mesh.positions.push([v[0] * r, v[1] * r, v[2] * r]);
        }
        mesh.indices.extend([base, base + 1, base + 2]);
    }
    mesh
}

/// Light inside glass (n=1.5) at 60 degrees from the normal is beyond the critical angle
/// (arcsin(1/1.5) = 41.8 degrees), so every sample must reflect back into the glass.
#[test]
fn total_internal_reflection_inside_glass() {
    let count = 64usize;
    // wo has z = -0.5: the viewer is on the far side of the surface (inside the glass).
    let probes: Vec<GpuProbe> = (0..count).map(|_| probe(0, with_cos(-0.5), 1)).collect();
    let Some(out) = run_with("bsdf_dir_test_main", vec![rough_glass([1.0; 3], 0.02, 1.0)], &probes, photo_frame()) else {
        return;
    };
    for d in &out {
        let dir = [f64::from(d[0]), f64::from(d[1]), f64::from(d[2])];
        assert!(dir[2] < 0.0, "TIR: sample must reflect, not refract: {dir:?}");
    }
}

#[test]
fn thin_sheet_zero_transmission_is_opaque() {
    let count = 64usize;
    let probes: Vec<GpuProbe> = (0..count).map(|_| probe(0, with_cos(0.5), 1)).collect();
    let sheet = thin_sheet([0.8, 0.6, 0.4], 0.5, 0.0);
    let Some(out) = run_with("bsdf_dir_test_main", vec![sheet], &probes, photo_frame()) else {
        return;
    };
    for d in &out {
        let dir = [f64::from(d[0]), f64::from(d[1]), f64::from(d[2])];
        assert!(dir[2] > 0.0, "transmission=0: no sample may pass through: {dir:?}");
    }
}

#[test]
fn thin_sheet_full_transmission_goes_through() {
    let count = 64usize;
    let probes: Vec<GpuProbe> = (0..count).map(|_| probe(0, with_cos(0.5), 1)).collect();
    let sheet = thin_sheet([0.8, 0.6, 0.4], 0.5, 1.0);
    let Some(out) = run_with("bsdf_dir_test_main", vec![sheet], &probes, photo_frame()) else {
        return;
    };
    for d in &out {
        let dir = [f64::from(d[0]), f64::from(d[1]), f64::from(d[2])];
        assert!(dir[2] < 0.0, "transmission=1: every sample must pass through: {dir:?}");
    }
}

#[test]
fn trans_pdf_is_non_negative() {
    let (nm, np) = (64usize, 128usize);
    let mut probes = Vec::with_capacity(nm * np);
    for i in 0..nm {
        let mu = -1.0 + 2.0 * (i as f64 + 0.5) / nm as f64;
        let s = (1.0 - mu * mu).sqrt();
        for j in 0..np {
            let phi = 2.0 * std::f64::consts::PI * (j as f64 + 0.5) / np as f64;
            probes.push(probe(0, [(s * phi.cos()) as f32, (s * phi.sin()) as f32, mu as f32], 1));
        }
    }
    let wo = with_cos(0.7);
    let frame = GpuFrame { sun_dir: [wo[0], wo[1], wo[2], 0.0], ..photo_frame() };
    let out = run_with("trans_pdf_test_main", vec![rough_glass([1.0; 3], 0.3, 0.8)], &probes, frame).expect("run");
    for o in &out {
        let pdf = f64::from(o[3]);
        assert!(pdf >= 0.0, "PDF must be non-negative: {pdf}");
        assert!(pdf.is_finite(), "PDF must be finite: {pdf}");
    }
}

#[test]
fn rough_glass_sphere_in_a_uniform_environment_is_one() {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("SKIP: no compute device");
        return;
    };
    let scene = Scene3d { meshes: vec![sphere("ball", 3.0, 3)], ..Default::default() };
    let mut gpu_scene = build_gpu_scene(&scene, false, None);
    gpu_scene.materials[0] = rough_glass([1.0; 3], 0.3, 1.0);
    // An 8 x 8 image from (-4, 0, 0) looking along +x (90 degree field): every pixel sees the ball.
    let (eye, f, r, u) = ([-4.0f32, 0.0, 0.0], [1.0f32, 0.0, 0.0], [0.0f32, -1.0, 0.0], [0.0f32, 0.0, 1.0]);
    let a = -0.5f32;
    let spp = 1024u32;
    let frame = GpuFrame {
        inv_view_proj: [
            [r[0], r[1], r[2], 0.0],
            [u[0], u[1], u[2], 0.0],
            [eye[0] * a, eye[1] * a, eye[2] * a, a],
            [eye[0] + f[0], eye[1] + f[1], eye[2] + f[2], 1.0],
        ],
        eye: [eye[0], eye[1], eye[2], 1.0],
        size: [8, 8],
        probe_count: 64,
        bounces: 16,
        spp_this_pass: spp,
        seed: 3,
        flags: FRAME_FURNACE,
        furnace: [0.0, 0.0, 1.0, 0.0],
        photo: [PHOTO_MATERIALS, 0, 0, 0],
        ..Default::default()
    };
    let buffers = upload_scene(&gpu, &gpu_scene);
    let out = match run_entry(&gpu, &buffers, "trace_main", &frame, &vec![GpuProbe::default(); 64]) {
        Ok(o) => o,
        Err(e) => {
            assert!(false, "trace_main: {e}");
            return;
        }
    };
    assert_eq!(out.len(), 64);
    let mean: f64 = out.iter().map(|a| f64::from(a[0]) / f64::from(spp)).sum::<f64>() / 64.0;
    assert!((mean - 1.0).abs() <= 0.02, "the ball renders {mean}, want 1.0 +- 2 %");
}

// ---- Light samples reach the far side (shadow rays leave on the side the sample goes to) ----------

/// An 8 x 8 camera 4 m from a radius-3 sphere along +x (90 degree field), `bounces` and `spp` set.
fn ball_frame(bounces: u32, spp: u32) -> GpuFrame {
    let (eye, f, r, u) = ([-4.0f32, 0.0, 0.0], [1.0f32, 0.0, 0.0], [0.0f32, -1.0, 0.0], [0.0f32, 0.0, 1.0]);
    let a = -0.5f32;
    GpuFrame {
        inv_view_proj: [
            [r[0], r[1], r[2], 0.0],
            [u[0], u[1], u[2], 0.0],
            [eye[0] * a, eye[1] * a, eye[2] * a, a],
            [eye[0] + f[0], eye[1] + f[1], eye[2] + f[2], 1.0],
        ],
        eye: [eye[0], eye[1], eye[2], 1.0],
        size: [8, 8],
        probe_count: 64,
        bounces,
        spp_this_pass: spp,
        seed: 3,
        photo: [PHOTO_MATERIALS, 0, 0, 0],
        ..Default::default()
    }
}

/// Run `trace_main` over the 64 pixels of `frame` on `scene`; `None` after a SKIP line.
fn trace_pixels(scene: &GpuScene, frame: &GpuFrame) -> Option<Vec<[f32; 4]>> {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("SKIP: no compute device");
        return None;
    };
    let buffers = upload_scene(&gpu, scene);
    let frame = GpuFrame { light_count: scene.lights.len() as u32, ..*frame };
    match run_entry(&gpu, &buffers, "trace_main", &frame, &vec![GpuProbe::default(); 64]) {
        Ok(out) => {
            assert_eq!(out.len(), 64);
            Some(out)
        }
        Err(e) => {
            assert!(false, "trace_main: {e}");
            None
        }
    }
}

/// `frame` with a uniform radiance-1.0 environment baked into `scene`, light-sampled.
fn with_uniform_env(scene: &mut GpuScene, frame: GpuFrame) -> GpuFrame {
    let lighting = crate::lit_frame::Lighting {
        sun: None,
        environment: Some(std::sync::Arc::new(crate::environment::Environment::uniform([1.0; 3], 16, 8))),
        ..Default::default()
    };
    let mut frame = frame;
    super::tracer::frame_environment(&mut frame, &mut scene.ies, &lighting);
    frame
}

/// Mean radiance of the camera rays that hit a closed sphere of `material` in the uniform environment.
fn env_furnace_mean(material: GpuMaterial) -> Option<f64> {
    let scene = Scene3d { meshes: vec![sphere("ball", 3.0, 3)], ..Default::default() };
    let mut gpu_scene = build_gpu_scene(&scene, false, None);
    gpu_scene.materials[0] = material;
    let spp = 1024u32;
    let frame = with_uniform_env(&mut gpu_scene, ball_frame(16, spp));
    assert_ne!(frame.flags & FRAME_ENV, 0, "the environment is on (importance sampled)");
    let out = trace_pixels(&gpu_scene, &frame)?;
    Some(out.iter().map(|a| f64::from(a[0]) / f64::from(spp)).sum::<f64>() / 64.0)
}

#[test]
fn thin_sheet_furnace_sphere_keeps_its_energy() {
    let Some(mean) = env_furnace_mean(thin_sheet([1.0; 3], 0.8, 0.6)) else {
        return;
    };
    assert!((0.95..=1.02).contains(&mean), "thin translucent sphere renders {mean}");
}

#[test]
fn rough_glass_furnace_sphere_keeps_its_energy_with_light_samples() {
    let Some(mean) = env_furnace_mean(rough_glass([1.0; 3], 0.3, 1.0)) else {
        return;
    };
    assert!((0.95..=1.02).contains(&mean), "rough glass sphere renders {mean}");
}

/// A 4 x 4 m quad on z = 0 (counter-clockwise seen from +z).
fn quad(material: &str) -> Mesh {
    let mut m = Mesh::new(material, ArchId(1));
    m.positions.extend([[-2.0, -2.0, 0.0], [2.0, -2.0, 0.0], [2.0, 2.0, 0.0], [-2.0, 2.0, 0.0]]);
    m.indices.extend([0, 1, 2, 0, 2, 3]);
    m
}

#[test]
fn far_side_light_sample_is_not_self_shadowed() {
    let scene = Scene3d { meshes: vec![quad("sheet")], ..Default::default() };
    let mut gpu_scene = build_gpu_scene(&scene, false, None);
    gpu_scene.materials[0] = thin_sheet([0.8; 3], 0.5, 1.0);
    let (d, intensity) = (2.0f32, 10.0f32);
    gpu_scene.lights = vec![GpuLight { position: [0.0, 0.0, -d], kind: LIGHT_POINT, rgb: [intensity; 3], ..Default::default() }];
    // From (0, 0, 3) looking down -z with a narrow field (about 0.12 m wide on the sheet).
    let (eye, f, r, u) = ([0.0f32, 0.0, 3.0], [0.0f32, 0.0, -1.0], [0.02f32, 0.0, 0.0], [0.0f32, 0.02, 0.0]);
    let a = -0.5f32;
    let spp = 64u32;
    let frame = GpuFrame {
        inv_view_proj: [
            [r[0], r[1], r[2], 0.0],
            [u[0], u[1], u[2], 0.0],
            [eye[0] * a, eye[1] * a, eye[2] * a, a],
            [eye[0] + f[0], eye[1] + f[1], eye[2] + f[2], 1.0],
        ],
        eye: [eye[0], eye[1], eye[2], 1.0],
        size: [8, 8],
        probe_count: 64,
        bounces: 0,
        spp_this_pass: spp,
        seed: 5,
        photo: [PHOTO_MATERIALS, 0, 0, 0],
        ..Default::default()
    };
    let Some(out) = trace_pixels(&gpu_scene, &frame) else {
        return;
    };
    let mean: f64 = out.iter().map(|p| f64::from(p[0]) / f64::from(spp)).sum::<f64>() / 64.0;
    let want = 0.8 * f64::from(intensity) / (std::f64::consts::PI * f64::from(d * d));
    assert!((mean - want).abs() <= 0.05 * want, "camera-side radiance {mean}, want {want} +- 5 %");
}

#[test]
fn opaque_direct_light_is_unchanged_by_the_shadow_side() {
    let scene = Scene3d { meshes: vec![sphere("ball", 3.0, 2)], ..Default::default() };
    let mut gpu_scene = build_gpu_scene(&scene, false, None);
    // Opaque (no MAT_TRANS): the materials switch must change nothing.
    gpu_scene.materials[0] = GpuMaterial { albedo: [0.7, 0.5, 0.3], roughness: 0.4, ior: 1.5, ..Default::default() };
    gpu_scene.lights = vec![GpuLight { position: [-6.0, 3.0, 4.0], kind: LIGHT_POINT, rgb: [60.0; 3], ..Default::default() }];
    let spp = 4u32;
    let base = GpuFrame { flags: FRAME_SUN, sun_dir: [0.0, 0.6, 0.8, 0.0], sun_rgb: [1.0; 4], ..ball_frame(2, spp) };
    let frame = with_uniform_env(&mut gpu_scene, base);
    let on = GpuFrame { photo: [PHOTO_MATERIALS, 0, 0, 0], ..frame };
    let off = GpuFrame { photo: [0, 0, 0, 0], ..frame };
    let (Some(a), Some(b)) = (trace_pixels(&gpu_scene, &on), trace_pixels(&gpu_scene, &off)) else {
        return;
    };
    assert!(a.iter().any(|p| p[0] > 0.0), "the scene is lit");
    assert_eq!(a, b, "an opaque scene renders identically with PHOTO_MATERIALS on and off");
}
