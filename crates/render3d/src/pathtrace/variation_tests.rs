//! Tests of the path tracer's procedural variation (`shaders/pt/variation.wgsl`): a grey card seen
//! from above under a sun straight overhead, traced through `trace_main` with no bounces, so each
//! pixel's red value is the card's albedo at that place. They skip with a note without an adapter.

use super::device::ComputeGpu;
use super::scene_gpu::{FRAME_SUN, GpuFrame, GpuProbe, GpuScene, PHOTO_VARIATION, build_gpu_scene};
use super::tracer::{run_entry, upload_scene};
use crate::types::{ArchId, Mesh, Scene3d};

/// `run_entry` dispatches one row of 64-wide groups, which covers an 8 x 8 image.
const SIDE: u32 = 8;
const SPP: u32 = 16;

fn gpu_or_skip() -> Option<ComputeGpu> {
    let gpu = crate::gputest::shared_compute();
    if gpu.is_none() {
        eprintln!("SKIP: no compute device");
    }
    gpu
}

/// A 20 m square card at z = 0 facing up, diffuse grey 0.5 with the given variation.
fn card(scale: f32, amount: f32, emission: f32) -> GpuScene {
    let mut m = Mesh::new("wall", ArchId(1));
    m.positions.extend([[-10.0, -10.0, 0.0], [10.0, -10.0, 0.0], [10.0, 10.0, 0.0], [-10.0, 10.0, 0.0]]);
    m.normals.extend([[0.0, 0.0, 1.0]; 4]);
    m.indices.extend([0, 1, 2, 0, 2, 3]);
    let mut gpu = build_gpu_scene(&Scene3d { meshes: vec![m], ..Default::default() }, false, None);
    for mat in gpu.materials.iter_mut() {
        mat.albedo = [0.5; 3];
        mat.metallic = 0.0;
        mat.roughness = 1.0;
        mat.ior = 1.0;
        mat.transmission = 0.0;
        mat.flags = 0;
        mat.emission = [emission; 3];
        mat.var_scale = scale;
        mat.var_amount = amount;
    }
    gpu
}

/// A narrow view (a 1 m square): a pixel is well inside one noise cell, so the sample jitter
/// inside it hardly matters.
fn render(gpu: &ComputeGpu, scene: &GpuScene, photo: u32, seed: u32, sample_base: u32) -> Vec<f64> {
    render_at(gpu, scene, photo, seed, sample_base, 0.25)
}

/// Red radiance per pixel looking straight down from 2 m, the sun overhead; `zoom` 1 sees a 4 m
/// square of the card.
fn render_at(gpu: &ComputeGpu, scene: &GpuScene, photo: u32, seed: u32, sample_base: u32, zoom: f32) -> Vec<f64> {
    let eye = [0.0f32, 0.0, 2.0];
    let (f, r, u) = ([0.0, 0.0, -1.0f32], [zoom, 0.0, 0.0f32], [0.0, zoom, 0.0f32]);
    let a = -0.5;
    let frame = GpuFrame {
        inv_view_proj: [
            [r[0], r[1], r[2], 0.0],
            [u[0], u[1], u[2], 0.0],
            [eye[0] * a, eye[1] * a, eye[2] * a, a],
            [eye[0] + f[0], eye[1] + f[1], eye[2] + f[2], 1.0],
        ],
        eye: [eye[0], eye[1], eye[2], 1.0],
        size: [SIDE, SIDE],
        probe_count: SIDE * SIDE,
        flags: FRAME_SUN,
        sun_dir: [0.0, 0.0, 1.0, 0.0],
        sun_rgb: [1.0; 4],
        spp_this_pass: SPP,
        sample_base,
        bounces: 0,
        seed,
        photo: [photo, 0, 0, 0],
        ..Default::default()
    };
    let buffers = upload_scene(gpu, scene);
    match run_entry(gpu, &buffers, "trace_main", &frame, &vec![GpuProbe::default(); (SIDE * SIDE) as usize]) {
        Ok(out) => out.iter().map(|a| f64::from(a[0]) / f64::from(SPP)).collect(),
        Err(e) => {
            assert!(false, "trace_main: {e}");
            Vec::new()
        }
    }
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len().max(1) as f64
}

/// Standard deviation over the mean.
fn spread(v: &[f64]) -> f64 {
    let m = mean(v);
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / v.len().max(1) as f64;
    var.sqrt() / m.max(1e-12)
}

#[test]
fn variation_off_leaves_the_material() {
    let Some(gpu) = gpu_or_skip() else { return };
    let plain = render(&gpu, &card(0.0, 0.0, 0.0), 0, 1, 0);
    assert!(mean(&plain) > 0.01, "the card is lit: {}", mean(&plain));
    // The option off: the material's fields do nothing.
    assert_eq!(render(&gpu, &card(0.2, 0.5, 0.0), 0, 1, 0), plain);
    // The option on but no amount, or no scale.
    assert_eq!(render(&gpu, &card(0.2, 0.0, 0.0), PHOTO_VARIATION, 1, 0), plain);
    assert_eq!(render(&gpu, &card(0.0, 0.5, 0.0), PHOTO_VARIATION, 1, 0), plain);
    assert!(spread(&plain) < 0.01);
    // An emitter is left alone: its colour (the sun's reflection plus the emission) does not vary.
    let lit = render(&gpu, &card(0.2, 0.5, 0.3), PHOTO_VARIATION, 1, 0);
    let flat = render(&gpu, &card(0.0, 0.0, 0.3), PHOTO_VARIATION, 1, 0);
    assert_eq!(lit, flat);
}

#[test]
fn variation_changes_colour_from_place_to_place() {
    let Some(gpu) = gpu_or_skip() else { return };
    let on = render(&gpu, &card(0.2, 0.5, 0.0), PHOTO_VARIATION, 1, 0);
    let off = render(&gpu, &card(0.2, 0.5, 0.0), 0, 1, 0);
    let (s_on, s_off) = (spread(&on), spread(&off));
    assert!(s_on > 0.02, "with variation: {s_on}");
    assert!(s_off < 0.01, "without: {s_off}");
}

#[test]
fn variation_keeps_the_mean_colour() {
    let Some(gpu) = gpu_or_skip() else { return };
    // A wide view: 8 m square, about 40 noise cells across.
    let on = mean(&render_at(&gpu, &card(0.2, 0.5, 0.0), PHOTO_VARIATION, 1, 0, 2.0));
    let off = mean(&render_at(&gpu, &card(0.2, 0.5, 0.0), 0, 1, 0, 2.0));
    assert!(off > 0.01);
    assert!((on - off).abs() <= 0.02 * off, "means {on} {off}");
}

#[test]
fn variation_is_a_function_of_position_only() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = card(0.2, 0.5, 0.0);
    let a = render(&gpu, &scene, PHOTO_VARIATION, 1, 0);
    assert_eq!(a, render(&gpu, &scene, PHOTO_VARIATION, 1, 0), "the same seed");
    // The noise belongs to the place: another seed or sample index only moves the pixel jitter, a
    // small fraction of the variation itself.
    let m = mean(&a);
    let signal = a.iter().map(|x| (x - m).abs()).sum::<f64>() / a.len() as f64;
    for (seed, base) in [(99, 0), (1, 7)] {
        let b = render(&gpu, &scene, PHOTO_VARIATION, seed, base);
        let diff = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum::<f64>() / a.len() as f64;
        assert!(diff < 0.3 * signal, "seed {seed} base {base}: differs by {diff}, the variation is {signal}");
    }
}
