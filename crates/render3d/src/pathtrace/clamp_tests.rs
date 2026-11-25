use super::luminance;

fn clamp_ref(c: [f32; 3], limit: f32, active: bool, depth: u32) -> ([f32; 3], f32) {
    if !active || depth == 0 || !limit.is_finite() || limit <= 0.0 {
        return (c, 0.0);
    }
    let lum = luminance(c).max(0.0);
    if lum <= limit {
        return (c, 0.0);
    }
    let scale = limit / lum;
    let removed = lum - limit;
    ([c[0] * scale, c[1] * scale, c[2] * scale], removed)
}

#[test]
fn no_clamp_reports_zero_loss() {
    let (after, loss) = clamp_ref([1_000_000.0, 1_000_000.0, 1_000_000.0], 4.0, false, 1);
    assert_eq!(after, [1_000_000.0, 1_000_000.0, 1_000_000.0]);
    assert_eq!(loss, 0.0);
}

#[test]
fn clamp_limits_the_reflected_emitter() {
    let (after, loss) = clamp_ref([1_000_000.0, 1_000_000.0, 1_000_000.0], 4.0, true, 1);
    assert!((luminance(after) - 4.0).abs() < 1e-4, "{after:?}");
    assert!((loss - 999_996.0).abs() < 1e-3, "loss={loss}");
}

#[test]
fn clamp_loss_equals_the_image_difference() {
    let before = [1_000_000.0, 1_000_000.0, 1_000_000.0];
    let limit = 4.0;
    let (after, loss) = clamp_ref(before, limit, true, 1);
    let kept = luminance(after);
    let removed = luminance(before) - kept;
    assert!((loss - removed).abs() < 1e-3, "loss={loss}, removed={removed}");
    assert!(kept <= limit + 1e-4, "kept={kept}");
}

#[test]
fn first_hit_light_is_never_clamped() {
    let before = [1_000_000.0, 1_000_000.0, 1_000_000.0];
    let (after, loss) = clamp_ref(before, 4.0, true, 0);
    assert_eq!(after, before);
    assert_eq!(loss, 0.0);
}

#[test]
fn clamp_at_exact_limit_is_noop() {
    // luminance(c) == limit: no clamp, no loss
    let c = [4.0, 0.0, 0.0]; // luminance = 0.2126 * 4.0 = 0.8504, not 4.0
    // Use a colour whose luminance is exactly the limit
    let limit = 1.0;
    let c = [1.0, 0.0, 0.0]; // luminance = 0.2126
    let (after, loss) = clamp_ref(c, 0.2126, true, 1);
    assert_eq!(after, c, "at exact limit, colour unchanged");
    assert_eq!(loss, 0.0, "at exact limit, no loss");
}

#[test]
fn clamp_with_nan_limit_is_noop() {
    let c = [100.0, 100.0, 100.0];
    let (after, loss) = clamp_ref(c, f32::NAN, true, 1);
    assert_eq!(after, c, "NaN limit: no clamp");
    assert_eq!(loss, 0.0, "NaN limit: no loss");
}

#[test]
fn clamp_with_infinite_limit_is_noop() {
    let c = [100.0, 100.0, 100.0];
    let (after, loss) = clamp_ref(c, f32::INFINITY, true, 1);
    assert_eq!(after, c, "infinite limit: no clamp");
    assert_eq!(loss, 0.0, "infinite limit: no loss");
}

#[test]
fn clamp_with_negative_limit_is_noop() {
    let c = [100.0, 100.0, 100.0];
    let (after, loss) = clamp_ref(c, -1.0, true, 1);
    assert_eq!(after, c, "negative limit: no clamp");
    assert_eq!(loss, 0.0, "negative limit: no loss");
}

#[test]
fn clamp_with_zero_limit_zeroes() {
    // A zero limit means "no clamp" (limit > 0.0 is required), so the colour is unchanged.
    let c = [100.0, 200.0, 50.0];
    let (after, loss) = clamp_ref(c, 0.0, true, 1);
    assert_eq!(after, c, "zero limit: no clamp, colour unchanged");
    assert_eq!(loss, 0.0, "zero limit: no loss");
}

#[test]
fn clamp_preserves_color_ratios() {
    let c = [100.0, 200.0, 50.0];
    let (after, _) = clamp_ref(c, 10.0, true, 1);
    let lum_before = luminance(c);
    let scale = 10.0 / lum_before;
    assert!((after[0] - c[0] * scale).abs() < 1e-4, "red scaled proportionally");
    assert!((after[1] - c[1] * scale).abs() < 1e-4, "green scaled proportionally");
    assert!((after[2] - c[2] * scale).abs() < 1e-4, "blue scaled proportionally");
}

#[test]
fn clamp_with_negative_components() {
    // Negative colour components: luminance can be negative, max(0.0) floors it
    let c = [-10.0, 5.0, 1.0];
    let lum = luminance(c);
    assert!(lum > 0.0, "test colour must have positive luminance");
    let (after, loss) = clamp_ref(c, lum * 0.5, true, 1);
    let after_lum = luminance(after);
    assert!(after_lum <= lum * 0.5 + 1e-6, "clamped luminance within limit");
    assert!(loss > 0.0, "some loss reported");
}

#[test]
fn clamp_with_huge_values_no_overflow() {
    let c = [1e30, 1e30, 1e30];
    let (after, loss) = clamp_ref(c, 4.0, true, 1);
    assert!(after.iter().all(|v| v.is_finite()), "no overflow in clamped colour");
    assert!(loss.is_finite(), "no overflow in loss");
    assert!((luminance(after) - 4.0).abs() < 1e-3, "clamped to limit");
}

#[test]
fn clamp_at_depth_two() {
    let c = [100.0, 100.0, 100.0];
    let (after, loss) = clamp_ref(c, 10.0, true, 2);
    assert!((luminance(after) - 10.0).abs() < 1e-4, "depth 2 clamped");
    assert!(loss > 0.0, "depth 2 reports loss");
}
