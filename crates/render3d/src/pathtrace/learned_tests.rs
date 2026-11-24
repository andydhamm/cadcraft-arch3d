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
