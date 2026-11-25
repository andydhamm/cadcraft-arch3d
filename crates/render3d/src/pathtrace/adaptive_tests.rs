use super::adaptive::{MIN_SAMPLES_HIT, MIN_SAMPLES_MISS, converged_mask, frame_noise_from_accum, freeze_flags, pixel_errors};
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

/// A strip of 22 quiet hit pixels (64 samples, mean 1) with `special` pixels put in at their index.
fn strip(special: &[(usize, (f32, f32, f32, bool))]) -> Vec<[f32; 4]> {
    let mut px = vec![(1.0, 1e-4, 64.0, true); 22];
    for &(i, p) in special {
        px[i] = p;
    }
    accum_of(&px)
}

#[test]
fn converged_mask_marks_pixels_under_the_target() {
    // Far apart so the 5 x 5 pooling of one does not reach another. Quiet: error
    // sqrt(1e-4 / 64) = 0.00125. Noisy (index 7): own sqrt(4 / 64) = 0.25. A miss (14) and a young
    // quiet pixel (21, 32 samples).
    let accum = strip(&[(7, (1.0, 4.0, 64.0, true)), (14, (0.0, 0.0, 16.0, false)), (21, (1.0, 1e-4, 32.0, true))]);
    let size = (22, 1);
    let errors = pixel_errors(&accum, size);
    assert_eq!(errors.len(), 22);
    assert!((errors[0].unwrap_or(9.0) - 0.00125).abs() < 1e-4, "{errors:?}");
    assert!((errors[7].unwrap_or(9.0) - 0.25).abs() < 1e-3, "{errors:?}");
    assert_eq!(errors[14], None, "a miss pixel has no error");
    let at = |m: &[bool], i: usize| m.get(i).copied();
    let m = converged_mask(&accum, size, 0.1, 64);
    assert_eq!([0, 7, 14, 21].map(|i| at(&m, i)), [Some(true), Some(false), Some(true), Some(false)]);
    // A looser target takes the noisy pixel; a stricter one takes the quiet pixel back.
    let m = converged_mask(&accum, size, 0.3, 64);
    assert_eq!(at(&m, 7), Some(true));
    let m = converged_mask(&accum, size, 0.0005, 64);
    assert_eq!([0, 14].map(|i| at(&m, i)), [Some(false), Some(true)]);
    // Fewer samples needed: the young pixel counts.
    assert_eq!(at(&converged_mask(&accum, size, 0.1, 32), 21), Some(true));
    // A short slice gives nothing, and does not panic.
    assert!(converged_mask(&accum[..5], size, 0.1, 64).is_empty());
    assert!(pixel_errors(&[], size).is_empty());
    assert!(converged_mask(&accum, (0, 0), 0.1, 64).is_empty());
}

#[test]
fn noise_metric_is_the_95th_percentile_of_relative_error() {
    // A flat 5 x 4 frame of mean 1 and per-sample variance 0.04 at 16 samples: every pooled error
    // is sqrt(0.04 / 16) = 0.05, so is the percentile.
    let accum = accum_of(&[(1.0, 0.04, 16.0, true); 20]);
    let noise = frame_noise_from_accum(&accum, (5, 4));
    assert!((noise - 0.05).abs() < 1e-4, "expected 0.05, got {noise}");
    // It is the 95th percentile of the per-pixel errors.
    let mut errors: Vec<f32> = pixel_errors(&accum, (5, 4)).into_iter().flatten().collect();
    errors.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(errors.get(18).copied(), Some(noise));
    // Miss pixels are left out of the percentile; a frame of misses has no noise.
    assert_eq!(frame_noise_from_accum(&accum_of(&[(0.0, 0.0, 16.0, false); 4]), (4, 1)), 0.0);
    assert_eq!(frame_noise_from_accum(&accum[..10], (5, 4)), 0.0);
}

/// A `side` x `side` frame of one-sample-variance-`var` pixels (mean 1, 64 samples) with the centre
/// pixel's variance set to `centre`.
fn square(side: usize, var: f32, centre: f32) -> Vec<[f32; 4]> {
    let mut px = vec![(1.0, var, 64.0, true); side * side];
    px[side * (side / 2) + side / 2] = (1.0, centre, 64.0, true);
    accum_of(&px)
}

#[test]
fn a_pixel_with_no_variance_beside_noisy_neighbours_is_not_converged() {
    let size = (5, 5);
    let centre = 12;
    // 24 neighbours of relative variance 100 and a centre of 0: rho = 24 * 100 / 25 = 96.
    let accum = square(5, 100.0, 0.0);
    let e = pixel_errors(&accum, size).get(centre).copied().flatten().unwrap_or(0.0);
    assert!((e - (96.0f32 / 64.0).sqrt()).abs() < 1e-3, "{e}");
    assert!(e > 0.5);
    assert_eq!(converged_mask(&accum, size, 0.25, 64).get(centre), Some(&false));
    // Flat neighbours: the centre is as good as they are.
    let flat = square(5, 0.0, 0.0);
    assert_eq!(converged_mask(&flat, size, 0.25, 64).get(centre), Some(&true));
}

#[test]
fn pooled_error_is_the_ratio_of_sums_over_the_window() {
    // A 7 x 1 strip, 64 samples each; luminance 1, 2, 1, 2, 1, 2, 1 and per-sample variance
    // 0.5, 2, 3, 4, 5, 6, 7. Lbar = 10 / 7, so the floor 0.0714 is never reached.
    let l = [1.0f32, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0];
    let v = [0.5f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
    let px: Vec<_> = l.iter().zip(&v).map(|(&l, &v)| (l, v, 64.0, true)).collect();
    let errors = pixel_errors(&accum_of(&px), (7, 1));
    // Pixel 0 (clipped window 0..=2): sum v = 5.5, sum L^2 = 6; own = 0.5 is smaller.
    let e0 = errors.first().copied().flatten().unwrap_or(0.0);
    assert!((e0 - (5.5f32 / 6.0 / 64.0).sqrt()).abs() < 1e-5, "{e0}");
    // Pixel 3 (window 1..=5): sum v = 20, sum L^2 = 4 + 1 + 4 + 1 + 4 = 14; own = 4 / 4 = 1.
    let e3 = errors.get(3).copied().flatten().unwrap_or(0.0);
    assert!((e3 - (20.0f32 / 14.0 / 64.0).sqrt()).abs() < 1e-5, "{e3}");
}

#[test]
fn hit_pixels_need_sixty_four_samples_and_misses_sixteen() {
    assert_eq!((MIN_SAMPLES_HIT, MIN_SAMPLES_MISS), (64, 16));
    let accum = accum_of(&[(1.0, 0.0, 63.0, true), (1.0, 0.0, 64.0, true), (0.0, 0.0, 15.0, false), (0.0, 0.0, 16.0, false)]);
    assert_eq!(converged_mask(&accum, (4, 1), 0.25, MIN_SAMPLES_HIT), vec![false, true, false, true]);
}

#[test]
fn a_frozen_pixel_thaws_when_its_neighbourhood_gets_noisier() {
    let size = (5, 5);
    let centre = 12;
    let quiet = converged_mask(&square(5, 1e-4, 1e-4), size, 0.25, MIN_SAMPLES_HIT);
    assert_eq!(freeze_flags(&quiet).get(centre), Some(&1.0), "frozen while the neighbourhood is quiet");
    // The same pixel, its own samples unchanged, with noisy neighbours: the flag goes back to 0.
    let noisy = converged_mask(&square(5, 100.0, 1e-4), size, 0.25, MIN_SAMPLES_HIT);
    let flags = freeze_flags(&noisy);
    assert_eq!(flags.get(centre), Some(&0.0), "{flags:?}");
    assert_eq!(freeze_flags(&[true, false]), vec![1.0, 0.0]);
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
fn pixel_errors_empty_and_short_accumulators() {
    // Empty accumulator with non-zero size.
    let errors = pixel_errors(&[], (4, 4));
    assert!(errors.is_empty(), "empty accum gives empty errors");

    // Accumulator shorter than 2 * pixels.
    let short = vec![[1.0, 1.0, 1.0, 16.0], [1.0, 1.0, 1.0, 16.0]];
    let errors = pixel_errors(&short, (4, 4));
    assert!(errors.is_empty(), "short accum gives empty errors");

    // Size (0, 0).
    let errors = pixel_errors(&[], (0, 0));
    assert!(errors.is_empty(), "size (0,0) gives empty errors");

    // Size (0, 5).
    let errors = pixel_errors(&[], (0, 5));
    assert!(errors.is_empty(), "size (0,5) gives empty errors");

    // Exactly 2 * pixels: should work.
    let ok = accum_of(&[(1.0, 0.01, 16.0, true)]);
    let errors = pixel_errors(&ok, (1, 1));
    assert_eq!(errors.len(), 1);
    assert!(errors[0].is_some());
}

#[test]
fn pixel_errors_all_miss_pixels_gives_none() {
    // All miss pixels: no hit pixels, so Lbar is undefined and every error is None.
    let accum = accum_of(&[(0.0, 0.0, 16.0, false), (0.0, 0.0, 32.0, false)]);
    let errors = pixel_errors(&accum, (2, 1));
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0], None, "miss pixel has no error");
    assert_eq!(errors[1], None, "miss pixel has no error");
}

#[test]
fn pixel_errors_single_pixel_image() {
    // A 1 x 1 image with one hit pixel.
    let accum = accum_of(&[(2.0, 0.04, 32.0, true)]);
    let errors = pixel_errors(&accum, (1, 1));
    assert_eq!(errors.len(), 1);
    let e = errors[0].expect("hit pixel has an error");
    // own = 0.04 / 2^2 = 0.01; error = sqrt(0.01 / 32)
    let want = (0.04 / 4.0 / 32.0).sqrt();
    assert!((e - want).abs() < 1e-6, "single pixel error {e} vs {want}");
}

#[test]
fn pixel_errors_large_values_stay_finite() {
    // Huge radiance values should not overflow or produce NaN.
    let accum = accum_of(&[(1e15_f32, 1e10, 16.0, true), (1e15_f32, 1e10, 16.0, true)]);
    let errors = pixel_errors(&accum, (2, 1));
    assert_eq!(errors.len(), 2);
    for e in errors {
        let e = e.expect("hit pixel has an error");
        assert!(e.is_finite(), "error is finite: {e}");
    }
}

#[test]
fn pixel_errors_handles_nan_and_non_finite() {
    // NaN in radiance sum: luminance is NaN, pixel is treated as a miss (None).
    let accum = accum_of(&[(f32::NAN, 1.0, 16.0, true)]);
    let errors = pixel_errors(&accum, (1, 1));
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0], None, "NaN radiance gives no error");

    // Infinity in radiance sum: luminance is infinite, treated as a miss.
    let accum = accum_of(&[(f32::INFINITY, 1.0, 16.0, true)]);
    let errors = pixel_errors(&accum, (1, 1));
    assert_eq!(errors[0], None, "infinite radiance gives no error");

    // Negative radiance: luminance can be negative, but is finite.
    let accum = accum_of(&[(-1.0, 0.25, 16.0, true)]);
    let errors = pixel_errors(&accum, (1, 1));
    assert!(errors[0].is_some(), "negative radiance gives a finite error");

    // NaN in luminance-squared sum: variance_of_mean clamps to 0, error is 0.
    let mut accum = accum_of(&[(1.0, 1.0, 16.0, true)]);
    accum[1][0] = f32::NAN;
    let errors = pixel_errors(&accum, (1, 1));
    assert!(errors[0].is_some(), "NaN sum_sq gives a finite error (clamped)");
    assert!(errors[0].unwrap().is_finite());
}

#[test]
fn converged_mask_empty_and_degenerate() {
    // Empty accumulator.
    let mask = converged_mask(&[], (4, 4), 0.1, 16);
    assert!(mask.is_empty(), "empty accum gives empty mask");

    // Size (0, 0).
    let mask = converged_mask(&[], (0, 0), 0.1, 16);
    assert!(mask.is_empty(), "size (0,0) gives empty mask");

    // Short accumulator.
    let short = vec![[1.0, 1.0, 1.0, 16.0], [1.0, 1.0, 1.0, 16.0]];
    let mask = converged_mask(&short, (4, 4), 0.1, 16);
    assert!(mask.is_empty(), "short accum gives empty mask");
}

#[test]
fn converged_mask_zero_and_infinite_target() {
    // target = 0: only pixels with exactly zero error converge.
    let accum = accum_of(&[(1.0, 0.0, 16.0, true), (1.0, 1e-4, 16.0, true)]);
    let mask = converged_mask(&accum, (2, 1), 0.0, 16);
    assert_eq!(mask, vec![false, false], "at target 0 nothing converges: the quiet pixel is pooled with its noisy neighbour");

    // target = infinity: all hit pixels with enough samples converge.
    let mask = converged_mask(&accum, (2, 1), f32::INFINITY, 16);
    assert_eq!(mask, vec![true, true], "all hit pixels converge at infinite target");

    // min_samples = 0: a pixel with zero samples converges if its error is low.
    let accum = accum_of(&[(1.0, 1e-4, 1.0, true)]);
    let mask = converged_mask(&accum, (1, 1), 0.1, 0);
    assert_eq!(mask, vec![true], "zero min_samples lets a young pixel converge");
}

#[test]
fn frame_noise_empty_and_degenerate() {
    // Empty accumulator.
    assert_eq!(frame_noise_from_accum(&[], (4, 4)), 0.0, "empty accum: noise 0");

    // Size (0, 0).
    assert_eq!(frame_noise_from_accum(&[], (0, 0)), 0.0, "size (0,0): noise 0");

    // Short accumulator.
    let short = vec![[1.0, 1.0, 1.0, 16.0], [1.0, 1.0, 1.0, 16.0]];
    assert_eq!(frame_noise_from_accum(&short, (4, 4)), 0.0, "short accum: noise 0");

    // All miss pixels: no errors, noise 0.
    let accum = accum_of(&[(0.0, 0.0, 16.0, false), (0.0, 0.0, 16.0, false)]);
    assert_eq!(frame_noise_from_accum(&accum, (2, 1)), 0.0, "all miss: noise 0");
}

#[test]
fn frame_noise_single_pixel() {
    // One hit pixel: noise is that pixel's error.
    let accum = accum_of(&[(1.0, 0.04, 16.0, true)]);
    let noise = frame_noise_from_accum(&accum, (1, 1));
    // error = sqrt(0.04/16) / 1 = 0.05
    assert!((noise - 0.05).abs() < 1e-6, "single pixel noise {noise}");

    // One miss pixel: noise is 0.
    let accum = accum_of(&[(0.0, 0.0, 16.0, false)]);
    assert_eq!(frame_noise_from_accum(&accum, (1, 1)), 0.0);
}

#[test]
fn fill_stats_empty_accumulator() {
    use super::adaptive::fill_stats;
    let mut stats = PathStats::default();
    fill_stats(&[], (4, 4), &mut stats);
    assert_eq!(stats.spp_min, 0);
    assert_eq!(stats.spp_max, 0);
    assert_eq!(stats.spp_mean, 0.0);
    assert_eq!(stats.samples_total, 0);
    assert_eq!(stats.noise, 0.0);
}

#[test]
fn fill_stats_small_accumulator() {
    use super::adaptive::fill_stats;
    let accum = accum_of(&[(1.0, 0.01, 10.0, true), (2.0, 0.04, 20.0, true)]);
    let mut stats = PathStats::default();
    fill_stats(&accum, (2, 1), &mut stats);
    assert_eq!(stats.spp_min, 10);
    assert_eq!(stats.spp_max, 20);
    assert!((stats.spp_mean - 15.0).abs() < 1e-6, "mean {}", stats.spp_mean);
    assert_eq!(stats.samples_total, 30);
    assert!(stats.noise >= 0.0 && stats.noise.is_finite());
}

#[test]
fn freeze_flags_empty_slice() {
    let flags = freeze_flags(&[]);
    assert!(flags.is_empty(), "empty mask gives empty flags");
}

#[test]
fn converged_mask_all_miss_pixels() {
    // All miss pixels: all converge (no error to check).
    let accum = accum_of(&[(0.0, 0.0, 16.0, false), (0.0, 0.0, 16.0, false)]);
    let mask = converged_mask(&accum, (2, 1), 0.1, 16);
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

// ---- rare bright paths ---------------------------------------------------------------------------

const ROOM_W: u32 = 48;
const ROOM_H: u32 = 32;

/// A closed 6 m room lit only by a narrow spot hung from the ceiling: the lit patch on the floor is
/// small, so the light reaches most of the room by rare bright paths (a bounce ray hitting the patch).
fn rare_path_room() -> Scene3d {
    let square = |lo: f64, hi: f64| [Vec2::new(lo, lo), Vec2::new(hi, lo), Vec2::new(hi, hi), Vec2::new(lo, hi)];
    let mut floor = Mesh::new("floor", ArchId(1));
    floor.push_prism(&square(-3.1, 3.1), -0.1, 0.0);
    let mut ceiling = Mesh::new("ceiling", ArchId(2));
    ceiling.push_prism(&square(-3.1, 3.1), 3.0, 3.1);
    let slab = |id: u32, x0: f64, y0: f64, x1: f64, y1: f64| {
        let mut m = Mesh::new("wall", ArchId(id));
        m.push_prism(&[Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)], 0.0, 3.0);
        m
    };
    let light = SceneLight {
        kind: LightKind::Spot,
        position: Vec3::new(0.0, 0.0, 2.9),
        direction: Vec3::new(0.0, 0.0, -1.0),
        intensity: 60000.0,
        color: [1.0; 3],
        cos_inner: 5.0f64.to_radians().cos(),
        cos_outer: 6.0f64.to_radians().cos(),
        axis: light_axis(Vec3::new(0.0, 0.0, -1.0), 0.0),
        ..Default::default()
    };
    let meshes = vec![
        floor,
        ceiling,
        slab(3, -3.1, -3.1, 3.1, -3.0),
        slab(4, -3.1, 3.0, 3.1, 3.1),
        slab(5, -3.1, -3.0, -3.0, 3.0),
        slab(6, 3.0, -3.0, 3.1, 3.0),
    ];
    let bounds = meshes[0].bounds();
    Scene3d { bounds, meshes, lights: vec![light], ..Default::default() }
}

/// Render the rare-path room to the end: the stats and the accumulated mean luminance per pixel.
fn render_room(gpu: &ComputeGpu, adaptive: bool, seed: u32) -> (PathStats, Vec<f64>) {
    let camera = Camera::Full(cadcraft_arch3d::camera::FullCamera::new(Vec3::new(-2.6, -2.6, 1.4), Vec3::new(1.5, 1.5, 0.6), 75.0));
    let lighting = Lighting { sun: None, ..Default::default() };
    let opts = PathOptions { seed, ..options(adaptive, Some(0.25), 20000) };
    let mut tracer = match PathTracer::new(gpu, &rare_path_room(), &camera, &lighting, false, (ROOM_W, ROOM_H), &opts, None) {
        Ok(t) => t,
        Err(e) => panic!("the tracer did not build: {e}"),
    };
    loop {
        match tracer.step(gpu) {
            PathStatus::Running { .. } => {}
            PathStatus::Done { .. } => break,
            other => panic!("the render ended {other:?}"),
        }
    }
    let n = (ROOM_W * ROOM_H) as usize;
    let Some(accum) = tracer.accum_snapshot(gpu) else { panic!("no accumulator") };
    let means = accum
        .iter()
        .take(n)
        .map(|p| {
            let c = p[3].max(1.0);
            f64::from(crate::pathtrace::luminance([p[0] / c, p[1] / c, p[2] / c]))
        })
        .collect();
    (tracer.stats().clone(), means)
}

#[test]
fn two_adaptive_seeds_agree_within_the_reported_noise() {
    let Some(gpu) = gpu_or_skip() else { return };
    let (a, xa) = render_room(&gpu, true, 1);
    let (b, xb) = render_room(&gpu, true, 2);
    assert_eq!((a.stop, b.stop), (StopReason::NoiseTarget, StopReason::NoiseTarget), "{a:?} {b:?}");
    let frame_mean = xa.iter().zip(&xb).map(|(x, y)| (x + y) / 2.0).sum::<f64>() / xa.len() as f64;
    let mut d: Vec<f64> =
        xa.iter().zip(&xb).map(|(x, y)| (x - y).abs() / 2.0f64.sqrt() / ((x + y) / 2.0).max(0.05 * frame_mean).max(1e-12)).collect();
    d.sort_by(|p, q| p.total_cmp(q));
    let p95 = d.get(((0.95 * d.len() as f64).ceil() as usize).saturating_sub(1)).copied().unwrap_or(f64::INFINITY);
    assert!(p95 <= 0.75, "p95 of d is {p95} (noise {} and {}, samples {} and {})", a.noise, b.noise, a.samples_total, b.samples_total);
}

#[test]
fn adaptive_still_needs_fewer_samples_than_uniform_on_the_rare_path_room() {
    let Some(gpu) = gpu_or_skip() else { return };
    let (a, _) = render_room(&gpu, true, 1);
    let (u, _) = render_room(&gpu, false, 1);
    assert_eq!((a.stop, u.stop), (StopReason::NoiseTarget, StopReason::NoiseTarget), "{a:?} {u:?}");
    assert!(a.samples_total as f64 <= 0.6 * u.samples_total as f64, "adaptive {} uniform {}", a.samples_total, u.samples_total);
}
