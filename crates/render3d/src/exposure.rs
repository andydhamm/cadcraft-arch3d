//! Exposure and tone mapping (phase 19), shared by the interactive pane, the raster Finalize and
//! the path tracer.
//!
//! A pixel's linear value is the scene luminance (nits) times an exposure *scale*; for a manual
//! exposure of `EV100` the scale is `1 / (1.2 · 2^EV100)`. Automatic exposure meters the frame's
//! log-average luminance toward middle grey (0.18). Then the tone curve (ACES, Hable or none)
//! maps the exposed value into 0..1. The WGSL twin of the curves is `shaders/tonemap.wgsl`.

use cadcraft_doc::arch::{ExposureSetting, Look, ToneMap};

use crate::lit_frame::Lighting;
use crate::pathtrace::{DAY_EXPOSURE, NIGHT_EXPOSURE};

/// Tone-map code: the ACES curve (the default).
pub const TONE_ACES: u32 = 1;
/// Tone-map code: the Hable (filmic) curve.
pub const TONE_HABLE: u32 = 2;
/// Tone-map code: no curve, the exposed value clamped to 0..1.
pub const TONE_NONE: u32 = 3;

/// The WGSL tone curves (`tone_map_rgb(rgb, code)`), for shaders that concatenate them.
pub const TONEMAP_WGSL: &str = include_str!("shaders/tonemap.wgsl");

/// The brightest scale an automatic exposure reaches when it meters a night frame: EV100 -6.
pub const NIGHT_AUTO_MAX: f32 = 64.0 / 1.2;

/// The exposure scale of a manual exposure of `ev` (EV100): `1 / (1.2 · 2^ev)`.
pub fn ev100_scale(ev: f64) -> f32 {
    (1.0 / (1.2 * ev.exp2())) as f32
}

/// The EV100 of an exposure scale (the inverse of [`ev100_scale`]); a scale that is not positive
/// and finite reads as the dimmest EV100 a scale of [`f32::MIN_POSITIVE`] has.
pub fn scale_ev100(s: f32) -> f64 {
    let s = if s.is_finite() && s > 0.0 { s } else { f32::MIN_POSITIVE };
    -(1.2 * f64::from(s)).log2()
}

/// The code of a look's tone map.
pub fn tone_code(t: ToneMap) -> u32 {
    match t {
        ToneMap::Aces => TONE_ACES,
        ToneMap::Hable => TONE_HABLE,
        ToneMap::None => TONE_NONE,
    }
}

/// The reply name of a tone-map code (`"aces"`, `"hable"`, `"none"`); an unknown code is `"none"`.
pub fn tone_name(code: u32) -> &'static str {
    match code {
        TONE_ACES => "aces",
        TONE_HABLE => "hable",
        _ => "none",
    }
}

fn hable_curve(x: f32) -> f32 {
    (x * (0.15 * x + 0.05) + 0.004) / (x * (0.15 * x + 0.5) + 0.06) - 0.02 / 0.3
}

/// One channel through the tone curve `code`, into 0..=1. NaN reads as 0; any code but
/// [`TONE_ACES`] and [`TONE_HABLE`] is "none" (a clamp).
pub fn tone_map(x: f32, code: u32) -> f32 {
    let x = if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0e6) };
    let y = match code {
        TONE_ACES => x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14),
        TONE_HABLE => hable_curve(2.0 * x) / hable_curve(11.2),
        _ => x,
    };
    y.clamp(0.0, 1.0)
}

/// Auto exposure from a still's hit pixels: `clamp(0.18 / Lavg, DAY_EXPOSURE, NIGHT_EXPOSURE)`.
pub fn auto_exposure_from_luminances(luminance: &[f32], hit: &[bool]) -> f32 {
    if luminance.len() != hit.len() || luminance.is_empty() {
        return DAY_EXPOSURE;
    }
    let mut log_sum = 0.0f64;
    let mut count = 0usize;
    for (&l, &is_hit) in luminance.iter().zip(hit.iter()) {
        if !is_hit {
            continue;
        }
        let lum = l.max(0.0);
        log_sum += (1e-4 + lum as f64).ln();
        count += 1;
    }
    if count == 0 {
        return DAY_EXPOSURE;
    }
    let lavg = (log_sum / count as f64).exp() as f32;
    (0.18 / lavg).clamp(DAY_EXPOSURE, NIGHT_EXPOSURE)
}

/// The automatic exposure scale for a frame whose metered log-average luminance is `lavg` (`None`:
/// nothing was hit).
///
/// With `daylight`: `clamp(0.18 / Lavg, DAY, NIGHT)`, no hits [`DAY_EXPOSURE`]. Without daylight
/// and without `meter_night` (the legacy `sun: false` and hand-built night lightings): exactly
/// [`NIGHT_EXPOSURE`]. Without daylight and with `meter_night`: `clamp(0.18 / Lavg, DAY,
/// NIGHT_AUTO_MAX)`, no hits [`NIGHT_EXPOSURE`].
pub fn auto_scale(lavg: Option<f32>, daylight: bool, meter_night: bool) -> f32 {
    let metered = |hi: f32| lavg.filter(|l| l.is_finite() && *l > 0.0).map(|l| (0.18 / l).clamp(DAY_EXPOSURE, hi));
    if daylight {
        metered(NIGHT_EXPOSURE).unwrap_or(DAY_EXPOSURE)
    } else if !meter_night {
        NIGHT_EXPOSURE
    } else {
        metered(NIGHT_AUTO_MAX).unwrap_or(NIGHT_EXPOSURE)
    }
}

/// Whether the frame has daylight: the sun is on and above the horizon (`p19-env-path` adds the
/// daylight environments).
pub fn daylight(lighting: &Lighting) -> bool {
    lighting.sun.is_some()
}

/// The exposure scale of a frame and how it was chosen (`"fixed"`, `"manual"` or `"auto"`): a
/// `legacy` bare number is used as given; else the look's manual EV100; else the automatic scale
/// of `lavg()`, the metered log-average luminance, which runs only for an automatic exposure that
/// can use it.
pub fn exposure_scale(
    look: &Look,
    legacy: Option<f64>,
    lavg: impl FnOnce() -> Option<f32>,
    daylight: bool,
    meter_night: bool,
) -> (f32, &'static str) {
    if let Some(x) = legacy {
        return (x as f32, "fixed");
    }
    match look.exposure {
        ExposureSetting::Manual { ev100 } => (ev100_scale(ev100), "manual"),
        ExposureSetting::Auto => {
            let l = if daylight || meter_night { lavg() } else { None };
            (auto_scale(l, daylight, meter_night), "auto")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ev100_scale_and_inverse() {
        assert!((ev100_scale(0.0) - 1.0 / 1.2).abs() < 1e-6);
        assert!((ev100_scale(1.0) - 1.0 / 2.4).abs() < 1e-6);
        assert!((ev100_scale(-6.0) - NIGHT_AUTO_MAX).abs() < 1e-3);
        assert!((NIGHT_AUTO_MAX - 53.333).abs() < 1e-3);
        // The furnace numbers: radiance 2.0 at EV100 1 and 2.
        assert!((2.0 * ev100_scale(1.0) - 0.8333).abs() < 1e-3);
        assert!((2.0 * ev100_scale(2.0) - 0.4167).abs() < 1e-3);
        for ev in [-6.0, -1.5, 0.0, 1.0, 6.3808, 14.0, 20.0] {
            assert!((scale_ev100(ev100_scale(ev)) - ev).abs() < 1e-4, "{ev}");
        }
        // The legacy scales: 0.01 per lux is EV100 6.3808, the day scale 14.695.
        assert!((scale_ev100(0.01) - 6.3808).abs() < 1e-3);
        assert!((scale_ev100(DAY_EXPOSURE) - 14.695).abs() < 1e-3);
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(scale_ev100(bad).is_finite());
        }
    }

    #[test]
    fn tone_curves() {
        for code in [TONE_ACES, TONE_HABLE, TONE_NONE] {
            assert!(tone_map(0.0, code) < 1e-6, "{code}");
            assert_eq!(tone_map(f32::NAN, code), tone_map(0.0, code));
            assert!(tone_map(f32::INFINITY, code) <= 1.0 && tone_map(f32::INFINITY, code) >= 0.0);
            assert!(tone_map(-3.0, code) <= tone_map(0.0, code) + 1e-6);
            let (mut last, mut x) = (0.0, 0.0f32);
            while x < 20.0 {
                let y = tone_map(x, code);
                assert!((0.0..=1.0).contains(&y) && y + 1e-6 >= last, "{code} {x} {y} {last}");
                last = y;
                x += 0.05;
            }
        }
        assert_eq!(tone_map(0.5, TONE_NONE), 0.5);
        assert_eq!(tone_map(7.0, TONE_NONE), 1.0);
        // ACES as in lit.wgsl, at a known point: 0.8095 -> 0.8095 (2.51 * .8095 + .03) ...
        let x = 0.8095f32;
        let aces = x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14);
        assert!((tone_map(x, TONE_ACES) - aces).abs() < 1e-6);
        assert!((tone_map(1.0, TONE_ACES) - 0.8037).abs() < 1e-3);
        let f = |x: f32| (x * (0.15 * x + 0.05) + 0.004) / (x * (0.15 * x + 0.5) + 0.06) - 0.02 / 0.3;
        assert!((tone_map(x, TONE_HABLE) - f(2.0 * x) / f(11.2)).abs() < 1e-6);
        // The two names and codes line up.
        assert_eq!([TONE_ACES, TONE_HABLE, TONE_NONE].map(tone_name), ["aces", "hable", "none"]);
        assert_eq!([ToneMap::Aces, ToneMap::Hable, ToneMap::None].map(tone_code), [1, 2, 3]);
    }

    #[test]
    fn wgsl_curves_match_the_mesh_shader_copy() {
        let curves = TONEMAP_WGSL.split_once("\n\n").map_or("", |(_, b)| b).trim_end();
        assert!(include_str!("shaders/mesh.wgsl").contains(curves), "mesh.wgsl must hold an identical copy of tonemap.wgsl");
    }

    #[test]
    fn auto_scale_clamps_legacy_night_and_meters_other_nights() {
        // Daylight: 0.18 / Lavg clamped to DAY..NIGHT, no hits DAY.
        assert_eq!(auto_scale(None, true, true), DAY_EXPOSURE);
        assert_eq!(auto_scale(Some(1.0e6), true, false), DAY_EXPOSURE);
        assert_eq!(auto_scale(Some(1.0e-6), true, false), NIGHT_EXPOSURE);
        assert!((auto_scale(Some(1591.5), true, true) - 1.131e-4).abs() < 1e-6);
        // Legacy night: exactly 0.01, metered or not.
        for l in [None, Some(0.2), Some(1.0e-9)] {
            assert_eq!(auto_scale(l, false, false), NIGHT_EXPOSURE);
        }
        // A metered night goes to EV100 -6 at most, and 0.18 / 0.2 = 0.9 in between.
        assert!((auto_scale(Some(0.2), false, true) - 0.9).abs() < 1e-6);
        assert_eq!(auto_scale(Some(1.0e-9), false, true), NIGHT_AUTO_MAX);
        assert_eq!(auto_scale(None, false, true), NIGHT_EXPOSURE);
        assert_eq!(auto_scale(Some(f32::NAN), false, true), NIGHT_EXPOSURE);

        // exposure_scale: the legacy number, the manual EV100 and the auto meter (run only then).
        let mut look = Look::default();
        let boom = || -> Option<f32> { Some(f32::NAN) };
        assert_eq!(exposure_scale(&look, Some(0.25), boom, true, true), (0.25, "fixed"));
        look.exposure = ExposureSetting::Manual { ev100: 14.0 };
        let (s, mode) = exposure_scale(&look, None, boom, true, true);
        assert_eq!(mode, "manual");
        assert!((s - 5.086e-5).abs() < 1e-8);
        look.exposure = ExposureSetting::Auto;
        let (s, mode) = exposure_scale(&look, None, || Some(0.2), false, true);
        assert!((s - 0.9).abs() < 1e-6 && mode == "auto");
        // Without daylight and without night metering the meter is not even asked.
        let mut asked = false;
        let r = exposure_scale(
            &look,
            None,
            || {
                asked = true;
                Some(0.2)
            },
            false,
            false,
        );
        assert_eq!(r, (NIGHT_EXPOSURE, "auto"));
        assert!(!asked);
        // daylight(): the sun being there.
        assert!(daylight(&Lighting::default()));
        assert!(!daylight(&Lighting { sun: None, ..Lighting::default() }));
    }

    #[test]
    fn night_keeps_the_night_exposure() {
        let lum = [1.0, 2.0, 5.0];
        let e = auto_exposure_from_luminances(&lum, &[true, true, true]);
        assert_eq!(e, NIGHT_EXPOSURE, "bright hits should clamp to the night exposure: {e}");
    }

    #[test]
    fn resolve_tone_maps_by_code() {
        // Each tone-map code resolves to the correct curve.
        let x = 0.5f32;

        // TONE_NONE: clamp to 0..=1.
        assert_eq!(tone_map(x, TONE_NONE), 0.5);
        assert_eq!(tone_map(2.0, TONE_NONE), 1.0);
        assert_eq!(tone_map(-1.0, TONE_NONE), 0.0);

        // TONE_ACES: the ACES curve (as in lit.wgsl).
        let aces = x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14);
        assert!((tone_map(x, TONE_ACES) - aces).abs() < 1e-6, "aces");

        // TONE_HABLE: the Hable (filmic) curve.
        let hable = |t: f32| (t * (0.15 * t + 0.05) + 0.004) / (t * (0.15 * t + 0.5) + 0.06) - 0.02 / 0.3;
        let hable_result = hable(2.0 * x) / hable(11.2);
        assert!((tone_map(x, TONE_HABLE) - hable_result).abs() < 1e-6, "hable");

        // The codes are distinct and in the right order.
        assert_eq!((TONE_ACES, TONE_HABLE, TONE_NONE), (1, 2, 3));

        // tone_code maps the enum to the right codes.
        assert_eq!([ToneMap::Aces, ToneMap::Hable, ToneMap::None].map(tone_code), [TONE_ACES, TONE_HABLE, TONE_NONE],);
    }
}
