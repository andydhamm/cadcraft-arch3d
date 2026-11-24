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
