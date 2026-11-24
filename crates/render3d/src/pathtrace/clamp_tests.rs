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
