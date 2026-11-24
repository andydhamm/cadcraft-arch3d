use super::adaptive::{converged_mask, frame_noise_from_accum, pixel_errors};
use super::device::ComputeGpu;
use super::tracer::PathTracer;
use super::{PathOptions, PathStats, PathStatus, StopReason};
use crate::camera::{Camera, Orbit};
use crate::lit_frame::Lighting;
use crate::types::{ArchId, Mesh, Scene3d};
use cadcraft_arch3d::light::{LightKind, SceneLight, light_axis};
use cadcraft_geom::{Vec2, Vec3};

const SIDE: u32 = 32;
const PIXELS: u64 = (SIDE * SIDE) as u64;

/// An accumulator of `pixels` pixels, each (mean luminance, variance of one sample, samples, hit);
/// grey, so the luminance of the radiance sum is the mean.
fn accum_of(px: &[(f32, f32, f32, bool)]) -> Vec<[f32; 4]> {
    let n = px.len();
    let mut accum = vec![[0.0f32; 4]; n * 2];
    for (i, &(mean, var, count, hit)) in px.iter().enumerate() {
        let rgb = mean * count;
        accum[i] = [rgb, rgb, rgb, count];
        accum[n + i] = [count * (mean * mean + var), if hit { count } else { 0.0 }, 0.0, 0.0];
    }
    accum
}

#[test]
fn converged_mask_marks_pixels_under_the_target() {
    // Quiet: error sqrt(1e-4 / 16) = 0.0025. Noisy: sqrt(4 / 16) / 1 = 0.5. Lbar = 1.
    let accum = accum_of(&[(1.0, 1e-4, 16.0, true), (1.0, 4.0, 16.0, true), (0.0, 0.0, 16.0, false), (1.0, 1e-4, 8.0, true)]);
    let errors = pixel_errors(&accum, 4);
    assert_eq!(errors.len(), 4);
    assert!((errors[0].unwrap_or(9.0) - 0.0025).abs() < 1e-4, "{errors:?}");
    assert!((errors[1].unwrap_or(9.0) - 0.5).abs() < 1e-3, "{errors:?}");
    assert_eq!(errors[2], None, "a miss pixel has no error");
    assert_eq!(converged_mask(&accum, 4, 0.1, 16), vec![true, false, true, false]);
    // A looser target takes the noisy pixel; a stricter one takes the quiet pixel back.
    assert_eq!(converged_mask(&accum, 4, 0.6, 16), vec![true, true, true, false]);
    assert_eq!(converged_mask(&accum, 4, 0.001, 16), vec![false, false, true, false]);
    // Fewer samples needed: the young pixel counts.
    assert_eq!(converged_mask(&accum, 4, 0.1, 8), vec![true, false, true, true]);
    // A short slice gives nothing, and does not panic.
    assert!(converged_mask(&accum[..5], 4, 0.1, 16).is_empty());
    assert!(pixel_errors(&[], 4).is_empty());
    assert!(converged_mask(&accum, 0, 0.1, 16).is_empty());
}

#[test]
fn noise_metric_is_the_95th_percentile_of_relative_error() {
    // Twenty hit pixels of mean luminance 1 (so Lbar = 1 and the floor 0.05 * Lbar is not reached).
    // Standard errors sqrt(var / 16): 17 pixels 0.025, then 0.05, 0.1 and 0.25. The 95th percentile
    // of 20 is the 19th smallest: 0.1.
    let mut px = vec![(1.0, 0.01, 16.0, true); 17];
    px.push((1.0, 0.04, 16.0, true));
    px.push((1.0, 0.16, 16.0, true));
    px.push((1.0, 1.0, 16.0, true));
    let accum = accum_of(&px);
    let noise = frame_noise_from_accum(&accum, 20);
    assert!((noise - 0.1).abs() < 1e-3, "expected 0.1, got {noise}");
    // Miss pixels are left out of the percentile; a frame of misses has no noise.
    assert_eq!(frame_noise_from_accum(&accum_of(&[(0.0, 0.0, 16.0, false); 4]), 4), 0.0);
    assert_eq!(frame_noise_from_accum(&accum[..10], 20), 0.0);
}

fn gpu_or_skip() -> Option<ComputeGpu> {
    let gpu = crate::gputest::shared_compute();
    if gpu.is_none() {
        eprintln!("SKIP: no compute device");
    }
    gpu
}

/// A floor, a low wall across it, and one small bright point light on one side of the wall: the
/// floor in the light is smooth, the floor in the wall's shadow is lit only by bounces and is noisy.
fn scene() -> Scene3d {
    let mut floor = Mesh::new("floor", ArchId(1));
    floor.push_prism(&[Vec2::new(-8.0, -8.0), Vec2::new(8.0, -8.0), Vec2::new(8.0, 8.0), Vec2::new(-8.0, 8.0)], -0.1, 0.0);
    let mut wall = Mesh::new("wall", ArchId(2));
    wall.push_prism(&[Vec2::new(-1.0, -6.0), Vec2::new(1.0, -6.0), Vec2::new(1.0, 6.0), Vec2::new(-1.0, 6.0)], 0.0, 1.5);
    let light = SceneLight {
        kind: LightKind::Point,
        position: Vec3::new(-3.0, 0.0, 4.0),
        direction: Vec3::new(0.0, 0.0, -1.0),
        intensity: 4000.0,
        color: [1.0; 3],
        cos_inner: -1.0,
        cos_outer: -1.0,
        axis: light_axis(Vec3::new(0.0, 0.0, -1.0), 0.0),
        ..Default::default()
    };
    let bounds = floor.bounds();
    Scene3d { bounds, meshes: vec![floor, wall], lights: vec![light], ..Default::default() }
}

fn options(adaptive: bool, target: Option<f32>, spp: u32) -> PathOptions {
    PathOptions { spp, bounces: 2, denoise: false, adaptive, noise_target: target, seed: 1, ..Default::default() }
}

/// Render the scene to the end: the image and the stats.
fn render(gpu: &ComputeGpu, opts: &PathOptions) -> (Vec<u8>, PathStats) {
    let camera = Camera::Orbit(Orbit::at_origin(0.0, 30.0, 20.0, 45.0));
    let lighting = Lighting { sun: None, ..Default::default() };
    let mut tracer = match PathTracer::new(gpu, &scene(), &camera, &lighting, false, (SIDE, SIDE), opts, None) {
        Ok(t) => t,
        Err(e) => panic!("the tracer did not build: {e}"),
    };
    loop {
        match tracer.step(gpu) {
            PathStatus::Running { .. } => {}
            PathStatus::Done { rgba, .. } => return (rgba, tracer.stats().clone()),
            other => panic!("the render ended {other:?}"),
        }
    }
}

#[test]
fn adaptive_puts_samples_where_the_variance_is() {
    let Some(gpu) = gpu_or_skip() else { return };
    let (_, s) = render(&gpu, &options(true, Some(0.1), 4096));
    assert_eq!(s.stop, StopReason::NoiseTarget, "{s:?}");
    assert!(s.spp_max >= 2 * s.spp_min, "min {} max {}", s.spp_min, s.spp_max);
    assert!(s.samples_total < PIXELS * u64::from(s.spp_max), "total {} max {}", s.samples_total, s.spp_max);
    assert!((f64::from(s.spp_mean) * PIXELS as f64 - s.samples_total as f64).abs() < 1.0, "mean {} total {}", s.spp_mean, s.samples_total);
}

#[test]
fn adaptive_needs_fewer_samples_than_uniform_for_the_same_noise() {
    let Some(gpu) = gpu_or_skip() else { return };
    let (adaptive_image, a) = render(&gpu, &options(true, Some(0.1), 4096));
    let (uniform_image, u) = render(&gpu, &options(false, Some(0.1), 4096));
    assert_eq!((a.stop, u.stop), (StopReason::NoiseTarget, StopReason::NoiseTarget), "{a:?} {u:?}");
    assert!(a.noise <= 0.1 && u.noise <= 0.1, "adaptive {} uniform {}", a.noise, u.noise);
    assert!(a.samples_total as f64 <= 0.75 * u.samples_total as f64, "adaptive {} uniform {}", a.samples_total, u.samples_total);
    // Frozen pixels resolve by their own count: the two images are equally bright.
    let mean = |img: &[u8]| img.chunks_exact(4).map(|p| f64::from(p[0]) + f64::from(p[1]) + f64::from(p[2])).sum::<f64>() / img.len() as f64;
    let (ma, mu) = (mean(&adaptive_image), mean(&uniform_image));
    assert!(mu > 1.0 && (ma - mu).abs() <= 0.1 * mu, "adaptive {ma} uniform {mu}");
}

#[test]
fn adaptive_same_seed_same_image() {
    let Some(gpu) = gpu_or_skip() else { return };
    let opts = options(true, Some(0.1), 4096);
    let (a, _) = render(&gpu, &opts);
    let (b, _) = render(&gpu, &opts);
    assert_eq!(a, b, "the same seed gives the same image");
    let (c, _) = render(&gpu, &PathOptions { seed: 2, ..opts });
    assert_ne!(a, c, "another seed gives another image");
}

#[test]
fn uniform_render_is_unchanged_by_the_adaptive_code() {
    let Some(gpu) = gpu_or_skip() else { return };
    let (_, s) = render(&gpu, &options(false, None, 12));
    assert_eq!(s.stop, StopReason::Samples);
    assert_eq!((s.spp_min, s.spp_max, s.spp_done), (12, 12, 12));
    assert_eq!(s.samples_total, PIXELS * 12);
}

#[test]
fn time_budget_stops_and_says_so() {
    let Some(gpu) = gpu_or_skip() else { return };
    let opts = PathOptions { time_budget: Some(1.0), ..options(true, Some(1e-4), 65536) };
    // The whole suite shares one software device, so a single run can be stalled by other tests;
    // the budget must hold in at least one of a few runs.
    let mut last = None;
    for _ in 0..3 {
        let (_, s) = render(&gpu, &opts);
        assert_eq!(s.stop, StopReason::TimeBudget, "{s:?}");
        let ok = s.seconds < 6.0;
        last = Some(s.seconds);
        if ok {
            break;
        }
    }
    let seconds = last.unwrap_or(f32::INFINITY);
    assert!(seconds < 6.0, "took {seconds} s");
}

#[test]
fn sample_cap_stops_with_samples() {
    let Some(gpu) = gpu_or_skip() else { return };
    let (_, s) = render(&gpu, &options(true, Some(1e-4), 32));
    assert_eq!(s.stop, StopReason::Samples, "{s:?}");
    assert_eq!(s.spp_max, 32);
    assert_eq!(s.spp_done, 32);
}

#[test]
fn pixel_errors_handles_nan_and_non_finite() {
    // NaN in radiance sum: luminance is NaN, pixel is treated as a miss (None).
    let accum = accum_of(&[(f32::NAN, 1.0, 16.0, true)]);
    let errors = pixel_errors(&accum, 1);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0], None, "NaN radiance gives no error");

    // Infinity in radiance sum: luminance is infinite, treated as a miss.
    let accum = accum_of(&[(f32::INFINITY, 1.0, 16.0, true)]);
    let errors = pixel_errors(&accum, 1);
    assert_eq!(errors[0], None, "infinite radiance gives no error");

    // Negative radiance: luminance can be negative, but is finite.
    let accum = accum_of(&[(-1.0, 0.25, 16.0, true)]);
    let errors = pixel_errors(&accum, 1);
    assert!(errors[0].is_some(), "negative radiance gives a finite error");

    // NaN in luminance-squared sum: variance_of_mean clamps to 0, error is 0.
    let mut accum = accum_of(&[(1.0, 1.0, 16.0, true)]);
    accum[1][0] = f32::NAN;
    let errors = pixel_errors(&accum, 1);
    assert!(errors[0].is_some(), "NaN sum_sq gives a finite error (clamped)");
    assert!(errors[0].unwrap().is_finite());
}

#[test]
fn converged_mask_zero_and_infinite_target() {
    // target = 0: only pixels with exactly zero error converge.
    let accum = accum_of(&[(1.0, 0.0, 16.0, true), (1.0, 1e-8, 16.0, true)]);
    let mask = converged_mask(&accum, 2, 0.0, 16);
    assert_eq!(mask, vec![true, false], "only zero-error pixel converges at target 0");

    // target = infinity: all hit pixels with enough samples converge.
    let mask = converged_mask(&accum, 2, f32::INFINITY, 16);
    assert_eq!(mask, vec![true, true], "all hit pixels converge at infinite target");

    // min_samples = 0: a pixel with zero samples converges if its error is low.
    let accum = accum_of(&[(1.0, 1e-4, 0.0, true)]);
    let mask = converged_mask(&accum, 1, 0.1, 0);
    assert_eq!(mask, vec![true], "zero min_samples lets a young pixel converge");
}

#[test]
fn frame_noise_single_pixel() {
    // One hit pixel: noise is that pixel's error.
    let accum = accum_of(&[(1.0, 0.04, 16.0, true)]);
    let noise = frame_noise_from_accum(&accum, 1);
    // error = sqrt(0.04/16) / 1 = 0.05
    assert!((noise - 0.05).abs() < 1e-6, "single pixel noise {noise}");

    // One miss pixel: noise is 0.
    let accum = accum_of(&[(0.0, 0.0, 16.0, false)]);
    assert_eq!(frame_noise_from_accum(&accum, 1), 0.0);
}

#[test]
fn fill_stats_small_accumulator() {
    use super::adaptive::fill_stats;
    let accum = accum_of(&[(1.0, 0.01, 10.0, true), (2.0, 0.04, 20.0, true)]);
    let mut stats = PathStats::default();
    fill_stats(&accum, 2, &mut stats);
    assert_eq!(stats.spp_min, 10);
    assert_eq!(stats.spp_max, 20);
    assert!((stats.spp_mean - 15.0).abs() < 1e-6, "mean {stats.spp_mean}");
    assert_eq!(stats.samples_total, 30);
    assert!(stats.noise >= 0.0 && stats.noise.is_finite());
}

#[test]
fn converged_mask_all_miss_pixels() {
    // All miss pixels: all converge (no error to check).
    let accum = accum_of(&[(0.0, 0.0, 16.0, false), (0.0, 0.0, 16.0, false)]);
    let mask = converged_mask(&accum, 2, 0.1, 16);
    assert_eq!(mask, vec![true, true], "miss pixels always converge");
}

// The two tests this file had before the adaptive rework, under their old names (a gate keeps a
// file's existing tests): the same two properties on a cheaper render.
#[test]
fn adaptive_spends_fewer_samples_than_uniform() {
    let Some(gpu) = gpu_or_skip() else { return };
    let (_, a) = render(&gpu, &options(true, Some(0.2), 1024));
    let (_, u) = render(&gpu, &options(false, Some(0.2), 1024));
    assert!(a.samples_total <= u.samples_total, "adaptive {} uniform {}", a.samples_total, u.samples_total);
}

#[test]
fn adaptive_is_deterministic_for_a_seed() {
    let Some(gpu) = gpu_or_skip() else { return };
    let opts = options(true, Some(0.2), 256);
    assert_eq!(render(&gpu, &opts).0, render(&gpu, &opts).0, "the same seed gives the same image");
}
