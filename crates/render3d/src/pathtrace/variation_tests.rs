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

#[test]
fn variation_with_extreme_scale_values() {
    let Some(gpu) = gpu_or_skip() else { return };
    // Very large scale: the 20m card fits inside one noise cell, so variation is nearly flat.
    let large_scale = render(&gpu, &card(100.0, 0.5, 0.0), PHOTO_VARIATION, 1, 0);
    assert!(spread(&large_scale) < 0.02, "large scale should be nearly flat: {}", spread(&large_scale));
    // Very small scale: many noise cells across the card, so variation is strong.
    let small_scale = render(&gpu, &card(0.01, 0.5, 0.0), PHOTO_VARIATION, 1, 0);
    assert!(spread(&small_scale) > 0.02, "small scale should vary a lot: {}", spread(&small_scale));
}

#[test]
fn variation_with_maximum_amount() {
    let Some(gpu) = gpu_or_skip() else { return };
    // Amount 1.0 is the maximum; the variation should be strong but bounded.
    let max_amount = render(&gpu, &card(0.2, 1.0, 0.0), PHOTO_VARIATION, 1, 0);
    let m = mean(&max_amount);
    assert!(m > 0.01, "the card is lit: {}", m);
    // The albedo is clamped to [0, 1], so the radiance should be bounded.
    for &v in &max_amount {
        assert!(v >= 0.0, "radiance should be non-negative: {}", v);
        assert!(v < 10.0, "radiance should be bounded: {}", v);
    }
    // Mean preservation should still hold.
    let plain = render(&gpu, &card(0.2, 0.0, 0.0), 0, 1, 0);
    let m_plain = mean(&plain);
    assert!((m - m_plain).abs() <= 0.02 * m_plain, "means {m} {m_plain}");
}

#[test]
fn variation_with_extreme_albedo() {
    let Some(gpu) = gpu_or_skip() else { return };
    // Black card: albedo 0, variation should keep it black (clamped at 0).
    let mut black = card(0.2, 0.5, 0.0);
    for mat in black.materials.iter_mut() {
        mat.albedo = [0.0; 3];
    }
    let black_render = render(&gpu, &black, PHOTO_VARIATION, 1, 0);
    for &v in &black_render {
        assert!(v < 0.001, "black card should stay black: {}", v);
    }
    // White card: albedo 1, variation should keep it white (clamped at 1).
    let mut white = card(0.2, 0.5, 0.0);
    for mat in white.materials.iter_mut() {
        mat.albedo = [1.0; 3];
    }
    let white_render = render(&gpu, &white, PHOTO_VARIATION, 1, 0);
    let m_white = mean(&white_render);
    assert!(m_white > 0.01, "the white card is lit: {}", m_white);
    // The white card should be brighter than the grey card.
    let grey_render = render(&gpu, &card(0.2, 0.5, 0.0), PHOTO_VARIATION, 1, 0);
    let m_grey = mean(&grey_render);
    assert!(m_white > m_grey, "white should be brighter than grey: {} vs {}", m_white, m_grey);
}

#[test]
fn variation_with_extreme_roughness() {
    let Some(gpu) = gpu_or_skip() else { return };
    // Minimum roughness: 0.02, variation should not go below it.
    let mut min_rough = card(0.2, 0.5, 0.0);
    for mat in min_rough.materials.iter_mut() {
        mat.roughness = 0.02;
    }
    let min_render = render(&gpu, &min_rough, PHOTO_VARIATION, 1, 0);
    assert!(mean(&min_render) > 0.01, "the card is lit: {}", mean(&min_render));
    // Maximum roughness: 1.0, variation should not go above it.
    let mut max_rough = card(0.2, 0.5, 0.0);
    for mat in max_rough.materials.iter_mut() {
        mat.roughness = 1.0;
    }
    let max_render = render(&gpu, &max_rough, PHOTO_VARIATION, 1, 0);
    assert!(mean(&max_render) > 0.01, "the card is lit: {}", mean(&max_render));
}

#[test]
fn variation_preserves_mean_over_wide_area() {
    let Some(gpu) = gpu_or_skip() else { return };
    // A very wide view: 16 m square, about 80 noise cells across at scale 0.2.
    let on = mean(&render_at(&gpu, &card(0.2, 0.5, 0.0), PHOTO_VARIATION, 1, 0, 4.0));
    let off = mean(&render_at(&gpu, &card(0.2, 0.5, 0.0), 0, 1, 0, 4.0));
    assert!(off > 0.01);
    assert!((on - off).abs() <= 0.02 * off, "means {on} {off}");
}
