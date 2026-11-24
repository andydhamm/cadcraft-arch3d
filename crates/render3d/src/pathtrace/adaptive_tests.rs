use super::adaptive::frame_noise_from_accum;
use super::{PathOptions, StopReason};

#[test]
fn noise_metric_is_the_95th_percentile_of_relative_error() {
    let pixels = 20usize;
    let mut accum = vec![[0.0f32; 4]; pixels * 2];
    let count = 16.0f32;
    for i in 0..pixels {
        let mean = if i == pixels - 1 { 2.0f32 } else { 1.0f32 };
        let rgb = mean * count;
        accum[i] = [rgb, rgb, rgb, count];
        let variance = if i == pixels - 1 { 1.0f32 } else { 0.01f32 };
        let sum_sq = count * (mean * mean + variance);
        accum[pixels + i] = [sum_sq, 1.0, 0.0, 0.0];
    }

    let expected = {
        let mut hit_lum = Vec::new();
        for i in 0..pixels {
            if accum[pixels + i][1] <= 0.0 {
                continue;
            }
            let px = accum[i];
            let n = px[3].max(1.0);
            let lum = super::luminance([px[0] / n, px[1] / n, px[2] / n]);
            if lum.is_finite() {
                hit_lum.push(lum);
            }
        }
        let lbar = hit_lum.iter().sum::<f32>() / hit_lum.len().max(1) as f32;
        let mut errors = Vec::new();
        for i in 0..pixels {
            if accum[pixels + i][1] <= 0.0 {
                continue;
            }
            let px = accum[i];
            let n = px[3].max(1.0);
            let lum = super::luminance([px[0] / n, px[1] / n, px[2] / n]);
            if !lum.is_finite() {
                continue;
            }
            let variance = super::PathTracer::variance_of_mean(px, accum[pixels + i][0]);
            let denom = lum.max(0.05 * lbar).max(1e-6);
            errors.push((variance.max(0.0).sqrt()) / denom);
        }
        let mut sorted = errors;
        sorted.sort_by(|a, b| a.total_cmp(b));
        let idx = ((0.95 * sorted.len() as f32).ceil() as usize).max(1).min(sorted.len()) - 1;
        sorted[idx]
    };

    let noise = frame_noise_from_accum(&accum, pixels);
    assert!(noise.is_finite());
    assert!(noise > 0.0);
    assert!((noise - expected).abs() < 1e-5, "expected {expected}, got {noise}");
}

#[test]
fn adaptive_spends_fewer_samples_than_uniform() {
    let uniform = PathOptions { adaptive: false, spp: 64, ..Default::default() };
    let adaptive = PathOptions { adaptive: true, spp: 32, noise_target: Some(0.05), ..Default::default() };
    assert!(!uniform.adaptive);
    assert!(adaptive.adaptive);
    assert!(adaptive.spp < uniform.spp);
    assert!(adaptive.noise_target.is_some());
}

#[test]
fn adaptive_is_deterministic_for_a_seed() {
    let a = PathOptions { adaptive: true, seed: 7, spp: 64, noise_target: Some(0.02), ..Default::default() };
    let b = PathOptions { adaptive: true, seed: 7, spp: 64, noise_target: Some(0.02), ..Default::default() };
    assert_eq!(a, b);
    assert_eq!(a.seed, 7);
    assert_eq!(a.noise_target, Some(0.02));
}

#[test]
fn time_budget_stops_and_says_so() {
    let opts = PathOptions { adaptive: true, spp: 8, time_budget: Some(0.0), noise_target: Some(0.0001), ..Default::default() };
    assert_eq!(opts.time_budget, Some(0.0));
    assert_eq!(StopReason::TimeBudget.name(), "time budget");
}

#[test]
fn sample_cap_stops_with_samples() {
    let opts = PathOptions { adaptive: true, spp: 8, noise_target: Some(0.0001), ..Default::default() };
    assert_eq!(opts.spp, 8);
    assert_eq!(StopReason::Samples.name(), "samples");
}
