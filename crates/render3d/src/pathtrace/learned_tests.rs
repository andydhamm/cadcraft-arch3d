use super::*;

/// A seeded value in 0..1 (an LCG, so the tests need no dependency).
fn rand01(state: &mut u64) -> f32 {
    *state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    ((*state >> 40) as f32) / (1u64 << 24) as f32
}

/// A frame: colour, guides (two entries per pixel) with `hit` deciding the flag per pixel.
fn frame(size: (u32, u32), mut colour: impl FnMut(u32, u32) -> [f32; 3], hit: impl Fn(u32, u32) -> bool) -> (Vec<[f32; 4]>, Vec<[f32; 4]>) {
    let mut color = Vec::new();
    let mut aov = Vec::new();
    for y in 0..size.1 {
        for x in 0..size.0 {
            let c = colour(x, y);
            color.push([c[0], c[1], c[2], 0.0]);
            aov.push([0.5, 0.5, 0.5, if hit(x, y) { 1.0 } else { 0.0 }]);
            aov.push([0.0, 0.0, 1.0, 3.0]);
        }
    }
    (color, aov)
}

#[test]
fn gpu_network_matches_the_cpu_reference() {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("skipped: no compute adapter");
        return;
    };
    for which in [ModelChoice::Small, ModelChoice::Base] {
        let net = Net::from_model(tza::model(which).expect("model parses")).expect("net builds");
        let padded = (48, 32);
        let mut state = 0x5eed_u64;
        let input: Vec<f32> = (0..9 * 48 * 32).map(|_| rand01(&mut state)).collect();
        let want = unet::forward_cpu(&net, &input, padded).expect("cpu forward");
        let got = forward_gpu(&gpu, &net, &input, padded).expect("gpu forward");
        assert_eq!(got.len(), want.len());
        let worst = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0.0_f32, f32::max);
        assert!(worst < 1e-3, "{which:?}: the GPU differs from the CPU by {worst}");
    }
}

#[test]
fn gpu_network_matches_the_cpu_reference_at_160_by_96() {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("SKIP: no compute adapter");
        return;
    };
    let (w, h) = (160usize, 96usize);
    for which in [ModelChoice::Small, ModelChoice::Base] {
        let net = Net::from_model(tza::model(which).expect("model parses")).expect("net builds");
        // Nine planes: colour 0.18, albedo 0.01, normals +x / +y / +z in three regions, then two more planes of 0.
        let mut input = vec![0.0_f32; 9 * w * h];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let n = if x < w / 3 {
                    0
                } else if y < h / 2 {
                    1
                } else {
                    2
                };
                for c in 0..3 {
                    input[c * w * h + i] = 0.18;
                    input[(3 + c) * w * h + i] = 0.01;
                }
                for c in 0..3 {
                    input[(6 + c) * w * h + i] = if c == n { 1.0 } else { 0.0 };
                }
            }
        }
        let padded = (w as u32, h as u32);
        let want = unet::forward_cpu(&net, &input, padded).expect("cpu forward");
        let got = forward_gpu(&gpu, &net, &input, padded).expect("gpu forward");
        assert_eq!(got.len(), want.len());
        let worst = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0.0_f32, f32::max);
        assert!(worst < 2e-3, "{which:?}: the GPU differs from the CPU by {worst} at 160 x 96");
    }
}

// This calls `learned::denoise` directly and keeps a border margin: the network's own answer next to
// the zero-padded border is covered by `flat_frame_through_finish_linear_changes_no_pixel_by_one_percent`
// in post.rs, where `keep_converged` holds a converged pixel to its input.
#[test]
fn flat_grey_frame_changes_under_one_percent() {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("skipped: no compute adapter");
        return;
    };
    let size = (64, 48);
    let (color, aov) = frame(size, |_, _| [0.5; 3], |_, _| true);
    for which in [ModelChoice::Small, ModelChoice::Base] {
        let out = denoise(&gpu, which, &color, &aov, size).expect("denoise");
        // The convolutions see zero padding at the frame's edge, so the outermost pixels differ by design
        // (the CPU reference does the same); the interior and the frame's mean must stay put.
        let (mut interior, mut sum) = (0.0_f32, 0.0_f32);
        for (i, (o, c)) in out.iter().zip(&color).enumerate() {
            let (x, y) = ((i as u32) % size.0, (i as u32) / size.0);
            let rel = (0..3).map(|k| (o[k] - c[k]).abs() / c[k]).fold(0.0_f32, f32::max);
            sum += rel;
            if x >= 16 && y >= 16 && x + 16 < size.0 && y + 16 < size.1 {
                interior = interior.max(rel);
            }
        }
        let mean = sum / out.len() as f32;
        eprintln!("{which:?}: interior {interior:.4}, mean {mean:.4}");
        assert!(interior < 0.01, "{which:?}: the interior of a flat frame changed by {:.2} %", interior * 100.0);
        assert!(mean < 0.01, "{which:?}: a flat frame changed by {:.2} % on average", mean * 100.0);
    }
}

#[test]
fn tiles_match_the_whole_frame() {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("skipped: no compute adapter");
        return;
    };
    let size = (160, 96);
    let tile = 64u32;
    let (color, aov) = frame(size, |x, y| [0.4 + 0.1 * (x as f32 / 160.0), 0.3 + 0.1 * (y as f32 / 96.0), 0.5], |_, _| true);
    let whole = denoise_tiled(&gpu, ModelChoice::Small, &color, &aov, size, None).expect("whole frame");
    let tiled = denoise_tiled(&gpu, ModelChoice::Small, &color, &aov, size, Some(tile)).expect("tiles");
    let seams = |v: u32, total: u32| starts(total, tile).iter().skip(1).any(|s| v + OVERLAP >= *s && v < s + OVERLAP);
    let (mut worst_away, mut worst_seam) = (0.0_f32, 0.0_f32);
    for y in 0..size.1 {
        for x in 0..size.0 {
            let i = (y * size.0 + x) as usize;
            let rel = (0..3).map(|k| (tiled[i][k] - whole[i][k]).abs() / whole[i][k].abs().max(1e-3)).fold(0.0_f32, f32::max);
            if seams(x, size.0) || seams(y, size.1) {
                worst_seam = worst_seam.max(rel);
            } else {
                worst_away = worst_away.max(rel);
            }
        }
    }
    eprintln!("tiles vs whole: away {worst_away:.4}, seams {worst_seam:.4}");
    assert!(worst_away < 0.01, "away from the seams the tiled run differs by {:.2} %", worst_away * 100.0);
    assert!(worst_seam < 0.03, "on the seams the tiled run differs by {:.2} %", worst_seam * 100.0);
}

#[test]
fn miss_pixels_keep_their_colour() {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("skipped: no compute adapter");
        return;
    };
    let size = (40, 24);
    let mut state = 7_u64;
    let (color, aov) = frame(size, |_, _| [0.2 + rand01(&mut state), 0.3, 0.1 + rand01(&mut state)], |x, _| x < 20);
    let out = denoise(&gpu, ModelChoice::Small, &color, &aov, size).expect("denoise");
    let mut changed = false;
    for (i, (o, c)) in out.iter().zip(&color).enumerate() {
        let x = (i as u32) % size.0;
        if x >= 20 {
            assert_eq!(&o[..3], &c[..3], "pixel {i} missed everything and must keep its colour");
        } else if o[..3] != c[..3] {
            changed = true;
        }
    }
    assert!(changed, "the hit pixels were not denoised at all");
}

/// `starts` produces non-overlapping-by-more-than-OVERLAP tile origins that cover the whole side,
/// and `ramp` weights from two adjacent tiles sum to 1 at every overlap pixel.
#[test]
fn starts_and_ramp_are_consistent() {
    // Exact fit: one tile.
    assert_eq!(super::starts(64, 64), vec![0]);
    // One pixel over: two tiles, the second flush with the end.
    assert_eq!(super::starts(65, 64), vec![0, 1]);
    // Three tiles with 32 px overlap each.
    assert_eq!(super::starts(100, 64), vec![0, 32, 64]);
    // Smaller tile.
    assert_eq!(super::starts(128, 64), vec![0, 32, 64]);
    // Tile larger than the side.
    assert_eq!(super::starts(32, 64), vec![0]);

    // Ramp: first tile, no left blend.
    assert!((super::ramp(0, 64, true, false) - 1.0).abs() < 1e-6);
    // Not first: left edge ramps up from 0.5/32.
    assert!((super::ramp(0, 64, false, false) - 0.5 / 32.0).abs() < 1e-6);
    assert!((super::ramp(31, 64, false, false) - 31.5 / 32.0).abs() < 1e-6);
    // Past the overlap: full weight.
    assert!((super::ramp(32, 64, false, false) - 1.0).abs() < 1e-6);
    // Right edge ramps down.
    assert!((super::ramp(63, 64, false, false) - 0.5 / 32.0).abs() < 1e-6);
    // Last tile: no right blend.
    assert!((super::ramp(63, 64, false, true) - 1.0).abs() < 1e-6);

    // Adjacent tiles' weights sum to 1 across the overlap.
    // Tile 0 at [0,63], tile 1 at [32,95]; overlap is global x in [32,63].
    for x in 32..=63 {
        let w0 = super::ramp(x, 64, true, false);
        let w1 = super::ramp(x - 32, 64, false, true);
        let sum = w0 + w1;
        assert!((sum - 1.0).abs() < 1e-6, "at x={x} weights sum to {sum}, not 1");
    }
}

#[test]
fn zero_size_frame_errors() {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("skipped: no compute adapter");
        return;
    };
    let err = super::denoise(&gpu, ModelChoice::Small, &[], &[], (0, 48)).unwrap_err();
    assert!(err.contains("0 x 48"), "zero-width frame must error: {err}");
    let err = super::denoise(&gpu, ModelChoice::Small, &[], &[], (48, 0)).unwrap_err();
    assert!(err.contains("48 x 0"), "zero-height frame must error: {err}");
}

#[test]
fn wrong_input_lengths_error() {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("skipped: no compute adapter");
        return;
    };
    let size = (8, 8);
    let color = vec![[0.5; 4]; 64];
    let aov = vec![[0.5; 4]; 128];
    // Too few colours.
    let err = super::denoise(&gpu, ModelChoice::Small, &color[..32], &aov, size).unwrap_err();
    assert!(err.contains("32"), "wrong colour count must error: {err}");
    // Too few guide entries.
    let err = super::denoise(&gpu, ModelChoice::Small, &color, &aov[..64], size).unwrap_err();
    assert!(err.contains("64"), "wrong guide count must error: {err}");
}

#[test]
fn all_miss_pixels_unchanged() {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("skipped: no compute adapter");
        return;
    };
    let size = (32, 24);
    let mut state = 42_u64;
    // Every pixel misses (hit flag 0).
    let (color, aov) = frame(size, |_, _| [0.3 + rand01(&mut state), 0.4, 0.2], |_, _| false);
    let out = super::denoise(&gpu, ModelChoice::Small, &color, &aov, size).expect("denoise");
    for (i, (o, c)) in out.iter().zip(&color).enumerate() {
        assert_eq!(&o[..3], &c[..3], "pixel {i} missed and must keep its colour");
    }
}

#[test]
fn nan_in_input_survives() {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("skipped: no compute adapter");
        return;
    };
    let size = (16, 16);
    // One pixel has NaN in its red channel; the rest are mid-grey.
    let (color, aov) = frame(
        size,
        |x, y| {
            if x == 8 && y == 8 { [f32::NAN, 0.5, 0.5] } else { [0.5; 3] }
        },
        |_, _| true,
    );
    let out = super::denoise(&gpu, ModelChoice::Small, &color, &aov, size).expect("denoise");
    // The NaN pixel is denoised (its neighbours pull it to a finite value) or stays NaN;
    // either way the call must not panic and the surrounding pixels must be finite.
    let i = (8 * 16 + 8) as usize;
    for (k, v) in out[i][..3].iter().enumerate() {
        if v.is_nan() {
            // Propagated NaN is acceptable.
        } else {
            assert!(v.is_finite(), "pixel 8,8 channel {k} is not finite: {v}");
        }
    }
    // A neighbour must be finite.
    let i2 = (8 * 16 + 7) as usize;
    for (k, v) in out[i2][..3].iter().enumerate() {
        assert!(v.is_finite(), "neighbour pixel 8,7 channel {k} must be finite: {v}");
    }
}

#[test]
fn huge_values_dont_overflow() {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("skipped: no compute adapter");
        return;
    };
    let size = (16, 16);
    // Very bright frame (1e6 nits); autoexposure should scale it down.
    let (color, aov) = frame(size, |_, _| [1e6, 1e6, 1e6], |_, _| true);
    let out = super::denoise(&gpu, ModelChoice::Small, &color, &aov, size).expect("denoise");
    for (i, o) in out.iter().enumerate() {
        for (k, v) in o[..3].iter().enumerate() {
            assert!(v.is_finite(), "pixel {i} channel {k} overflowed: {v}");
        }
    }
}

#[test]
fn tile_edge_validation() {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("skipped: no compute adapter");
        return;
    };
    let size = (64, 48);
    let (color, aov) = frame(size, |_, _| [0.5; 3], |_, _| true);
    // Tile edge must be a multiple of 16 and at least 64.
    let err = super::denoise_tiled(&gpu, ModelChoice::Small, &color, &aov, size, Some(63)).unwrap_err();
    assert!(err.contains("63"), "tile 63 must error: {err}");
    let err = super::denoise_tiled(&gpu, ModelChoice::Small, &color, &aov, size, Some(65)).unwrap_err();
    assert!(err.contains("65"), "tile 65 must error: {err}");
    let err = super::denoise_tiled(&gpu, ModelChoice::Small, &color, &aov, size, Some(16)).unwrap_err();
    assert!(err.contains("16"), "tile 16 must error: {err}");
    // Valid tile edge works.
    super::denoise_tiled(&gpu, ModelChoice::Small, &color, &aov, size, Some(64)).expect("tile 64 works");
}

#[test]
fn learned_is_unavailable_without_compute() {
    // CADCRAFT_RENDER_LIMITS=webgl2 reaches `check_limits` as `compute_off`.
    assert!(check_limits(&wgpu::Limits::default(), true).is_err());
    // WebGL2's limits have no storage buffers.
    assert!(check_limits(&wgpu::Limits::downlevel_webgl2_defaults(), false).is_err());
    assert!(check_limits(&wgpu::Limits::default(), false).is_ok());
    if let Some(gpu) = crate::gputest::shared_compute() {
        let off = super::super::device::compute_disabled_by_env();
        assert_eq!(available(&gpu).is_err(), off);
    }
}

// --- p22g: the rare-path room -------------------------------------------------------------------

mod room {
    use crate::camera::{Camera, FullCamera};
    use crate::lit_frame::Lighting;
    use crate::pathtrace::device::ComputeGpu;
    use crate::pathtrace::tracer::PathTracer;
    use crate::pathtrace::{LinearStage, PathOptions, PathStatus};
    use crate::types::{ArchId, Mesh, Scene3d};
    use cadcraft_arch3d::light::{LightKind, SceneLight, light_axis};
    use cadcraft_doc::arch::DenoiseMode;
    use cadcraft_geom::{Vec2, Vec3};

    const SIZE: (u32, u32) = (96, 64);

    fn slab(m: &mut Mesh, x: (f64, f64), y: (f64, f64), z: (f64, f64)) {
        m.push_prism(&[Vec2::new(x.0, y.0), Vec2::new(x.1, y.0), Vec2::new(x.1, y.1), Vec2::new(x.0, y.1)], z.0, z.1);
    }

    /// A closed 6 m room. The sun (south-west, 45 degrees) comes through a window in the west wall
    /// and lays a patch on the floor; a small emitter sits behind a baffle, so most of the room is
    /// lit by rare bright paths.
    fn scene() -> Scene3d {
        let mut floor = Mesh::new("floor", ArchId(1));
        slab(&mut floor, (-3.2, 3.2), (-3.2, 3.2), (-0.2, 0.0));
        let mut ceiling = Mesh::new("ceiling", ArchId(2));
        slab(&mut ceiling, (-3.2, 3.2), (-3.2, 3.2), (3.0, 3.2));
        let mut wall = Mesh::new("wall", ArchId(3));
        slab(&mut wall, (-3.2, -3.0), (-3.2, -0.15), (0.0, 3.0));
        slab(&mut wall, (-3.2, -3.0), (0.15, 3.2), (0.0, 3.0));
        slab(&mut wall, (-3.2, -3.0), (-0.15, 0.15), (0.0, 1.0));
        slab(&mut wall, (-3.2, -3.0), (-0.15, 0.15), (1.3, 3.0));
        slab(&mut wall, (3.0, 3.2), (-3.2, 3.2), (0.0, 3.0));
        slab(&mut wall, (-3.0, 3.0), (-3.2, -3.0), (0.0, 3.0));
        slab(&mut wall, (-3.0, 3.0), (3.0, 3.2), (0.0, 3.0));
        let mut baffle = Mesh::new("wall", ArchId(4));
        slab(&mut baffle, (0.8, 1.0), (-3.0, 1.0), (0.0, 3.0));
        let light = SceneLight {
            kind: LightKind::Point,
            position: Vec3::new(2.2, -1.5, 2.6),
            direction: Vec3::new(0.0, 0.0, -1.0),
            intensity: 300.0,
            color: [1.0; 3],
            cos_inner: -1.0,
            cos_outer: -1.0,
            axis: light_axis(Vec3::new(0.0, 0.0, -1.0), 0.0),
            ..Default::default()
        };
        let bounds = floor.bounds();
        Scene3d { bounds, meshes: vec![floor, ceiling, wall, baffle], lights: vec![light], ..Default::default() }
    }

    fn render(gpu: &ComputeGpu, opts: &PathOptions) -> (Vec<[f32; 3]>, crate::pathtrace::PathStats) {
        let camera = Camera::Full(FullCamera::new(Vec3::new(2.6, -2.6, 1.4), Vec3::new(-1.5, 0.8, 0.8), 70.0));
        let opts = PathOptions { bounces: 2, keep_linear: Some(LinearStage::Radiance), ..opts.clone() };
        let mut t = match PathTracer::new(gpu, &scene(), &camera, &Lighting::default(), false, SIZE, &opts, None) {
            Ok(t) => t,
            Err(e) => panic!("the tracer did not build: {e}"),
        };
        loop {
            match t.step(gpu) {
                PathStatus::Running { .. } => {}
                PathStatus::Done { .. } => break,
                other => panic!("the render ended {other:?}"),
            }
        }
        let stats = t.stats();
        (stats.linear.clone().unwrap_or_default(), stats)
    }

    fn lum(p: &[f32; 3]) -> f64 {
        0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2])
    }

    /// The check's cross estimator: mean of `(x - r1)(x - r2) / (rbar^2 + eps)` over pixels and channels.
    fn cross_error(x: &[[f32; 3]], r1: &[[f32; 3]], r2: &[[f32; 3]]) -> f64 {
        let mean_ref = r1.iter().zip(r2).map(|(a, b)| 0.5 * (lum(a) + lum(b))).sum::<f64>() / x.len().max(1) as f64;
        let eps = (0.05 * mean_ref).powi(2);
        let mut sum = 0.0;
        for ((p, a), b) in x.iter().zip(r1).zip(r2) {
            for c in 0..3 {
                let (v, a, b) = (f64::from(p[c]), f64::from(a[c]), f64::from(b[c]));
                let rbar = 0.5 * (a + b);
                sum += (v - a) * (v - b) / (rbar * rbar + eps);
            }
        }
        sum / (3 * x.len()).max(1) as f64
    }

    struct Frames {
        input: Vec<[f32; 3]>,
        filter: Vec<[f32; 3]>,
        learned: Vec<[f32; 3]>,
        r1: Vec<[f32; 3]>,
        r2: Vec<[f32; 3]>,
        learned_ran: bool,
    }

    fn frames(gpu: &ComputeGpu) -> Frames {
        let base = PathOptions { spp: 64, spp_per_step: 16, seed: 3, ..PathOptions::default() };
        let (input, _) = render(gpu, &PathOptions { denoise: false, ..base.clone() });
        let (filter, _) = render(gpu, &PathOptions { denoise: true, denoise_mode: DenoiseMode::Filter, ..base.clone() });
        let (learned, stats) = render(gpu, &PathOptions { denoise: true, denoise_mode: DenoiseMode::Learned, ..base });
        let reference = |seed| render(gpu, &PathOptions { spp: 512, spp_per_step: 64, seed, denoise: false, ..PathOptions::default() }).0;
        Frames { input, filter, learned, r1: reference(99), r2: reference(98), learned_ran: stats.denoiser == DenoiseMode::Learned }
    }

    fn gpu_or_skip() -> Option<ComputeGpu> {
        let gpu = crate::gputest::shared_compute();
        if gpu.is_none() {
            eprintln!("SKIP: no compute device");
        }
        gpu
    }

    #[test]
    fn learned_halves_the_filter_error_on_a_rare_path_room() {
        let Some(gpu) = gpu_or_skip() else { return };
        let f = frames(&gpu);
        if !f.learned_ran {
            eprintln!("SKIP: the learned denoiser is not available on this device");
            return;
        }
        let (ef, el) = (cross_error(&f.filter, &f.r1, &f.r2), cross_error(&f.learned, &f.r1, &f.r2));
        eprintln!("room: input {:.4} filter {ef:.4} learned {el:.4}", cross_error(&f.input, &f.r1, &f.r2));
        assert!(el <= 0.5 * ef, "learned {el} is not under half the filter's {ef}");
    }

    #[test]
    fn learned_keeps_the_frame_mean() {
        let Some(gpu) = gpu_or_skip() else { return };
        let f = frames(&gpu);
        if !f.learned_ran {
            eprintln!("SKIP: the learned denoiser is not available on this device");
            return;
        }
        let mean = |v: &[[f32; 3]]| v.iter().map(lum).sum::<f64>() / v.len().max(1) as f64;
        let (m_in, m_out) = (mean(&f.input), mean(&f.learned));
        // Each pixel is held to within 2 sigma of its own unbiased samples, so the network can move
        // the frame mean only by a few percent; 3 % is the bound.
        assert!(m_in > 0.0 && (m_out - m_in).abs() <= 0.03 * m_in, "input mean {m_in}, learned mean {m_out}");
    }
}
