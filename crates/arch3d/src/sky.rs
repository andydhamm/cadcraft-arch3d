//! An analytic clear sky (Preetham, Shirley and Smits 1999): colour and brightness from the sun's
//! position, twilight from 0 to -6 degrees, a night floor, stars and the moon's direction.
//!
//! Pure and deterministic; every function returns finite numbers for any input.
//! Directions are drawing space (x right, y up the plan, z up), unit or not.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::MoonLook;
use std::cell::Cell;

/// The faint light of a moonless night sky, linear nits, added to every sky sample.
pub const NIGHT_SKY: [f64; 3] = [0.0004, 0.0006, 0.0012];

/// Sun illuminance used for the ground bounce under the sky model, lux.
const SUN_LUX: f64 = 100_000.0;
/// Albedo of the grey ground below the horizon.
const GROUND_ALBEDO: f64 = 0.2;
/// Peak brightness of a star, nits.
const STAR_NITS: f64 = 0.05;

fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    if !v.iter().all(|c| c.is_finite()) {
        return None;
    }
    // Scale first so huge components cannot overflow when squared.
    let m = v[0].abs().max(v[1].abs()).max(v[2].abs());
    if m < 1e-150 {
        return None;
    }
    let w = [v[0] / m, v[1] / m, v[2] / m];
    let l = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
    Some([w[0] / l, w[1] / l, w[2] / l])
}

/// Zenith luminance in kcd/m² for a sun zenith angle `theta_s` (radians) and turbidity `t`.
pub fn zenith_luminance(theta_s: f64, t: f64) -> f64 {
    let chi = (4.0 / 9.0 - t / 120.0) * (std::f64::consts::PI - 2.0 * theta_s);
    let y = (4.0453 * t - 4.9710) * chi.tan() - 0.2155 * t + 2.4192;
    if y.is_finite() { y.max(0.0) } else { 0.0 }
}

/// How much of the day sky remains at a sun elevation in degrees: 1 at or above the horizon,
/// 10^(elev/1.5) below it (1e-4 at -6), 0 under -6 and for non-finite input.
pub fn twilight_factor(elev_deg: f64) -> f64 {
    if !elev_deg.is_finite() || elev_deg < -6.0 {
        0.0
    } else if elev_deg >= 0.0 {
        1.0
    } else {
        10f64.powf(elev_deg / 1.5)
    }
}

/// Brightness of a star-field sample in nits: about 1 % of directions above the horizon are
/// stars (0.05 nits), the rest 0. Unscaled: multiply by `1 - twilight_factor(sun elevation)`
/// (see [`star_radiance`]).
pub fn star(view: [f64; 3]) -> f64 {
    let Some(v) = unit(view) else { return 0.0 };
    if v[2] <= 0.0 {
        return 0.0;
    }
    const K: f64 = 220.0;
    let mut h: u64 = 0x9E37_79B9_7F4A_7C15;
    for c in v {
        h ^= (c * K).floor() as i64 as u64;
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 29;
    }
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 32;
    if h % 100 == 0 { STAR_NITS } else { 0.0 }
}

/// Star brightness for a sun direction: [`star`] scaled by `1 - twilight_factor` of its elevation.
pub fn star_radiance(view: [f64; 3], sun: [f64; 3]) -> f64 {
    let Some(s) = unit(sun) else { return 0.0 };
    star(view) * (1.0 - twilight_factor(s[2].clamp(-1.0, 1.0).asin().to_degrees()))
}

fn perez(c: [f64; 5], cos_t: f64, gamma: f64) -> f64 {
    let cos_t = cos_t.max(0.01);
    (1.0 + c[0] * (c[1] / cos_t).exp()) * (1.0 + c[2] * (c[3] * gamma).exp() + c[4] * gamma.cos().powi(2))
}

/// The day sky (no twilight scaling) in linear nits; `s` has z >= 0, both unit.
/// Everything in the Perez day-sky that is fixed by the sun and turbidity alone.
/// View-dependent work (the `gamma` angle, the cosine of the view's zenith
/// angle) is left to `day_sky`; this struct is what `sky_radiance` and
/// `horizontal_illuminance` used to re-derive per texel.
#[derive(Clone, Copy)]
struct DaySky {
    /// Unit sun, clamped to `z >= 0`.
    sun: [f64; 3],
    /// Luminance base `zenith_luminance(theta_s, t) * 1000.0` (times 1000 to
    /// get cd/m² from kcd/m²).
    lum_base: f64,
    cy: [f64; 5],
    cx: [f64; 5],
    cyy: [f64; 5],
    /// Sun-zenith polynomial coefficients, pre-multiplied.
    xz: f64,
    yz: f64,
    /// `perez(c, 1.0, theta_s)` denominators, one per coefficient set.
    d_cy: f64,
    d_cx: f64,
    d_cyy: f64,
}

impl DaySky {
    fn new(s: [f64; 3], t: f64) -> Self {
        let theta_s = s[2].clamp(-1.0, 1.0).acos();
        let (th2, th3) = (theta_s * theta_s, theta_s * theta_s * theta_s);
        let t2 = t * t;
        let cy = [0.1787 * t - 1.4630, -0.3554 * t + 0.4275, -0.0227 * t + 5.3251, 0.1206 * t - 2.5771, -0.0670 * t + 0.3703];
        let cx = [-0.0193 * t - 0.2592, -0.0665 * t + 0.0008, -0.0004 * t + 0.2125, -0.0641 * t - 0.8989, -0.0033 * t + 0.0452];
        let cyy = [-0.0167 * t - 0.2608, -0.0950 * t + 0.0092, -0.0079 * t + 0.2102, -0.0441 * t - 1.6537, -0.0109 * t + 0.0529];
        Self {
            sun: s,
            lum_base: zenith_luminance(theta_s, t) * 1000.0,
            cy,
            cx,
            cyy,
            xz: t2 * (0.00166 * th3 - 0.00375 * th2 + 0.00209 * theta_s)
                + t * (-0.02903 * th3 + 0.06377 * th2 - 0.03202 * theta_s + 0.00394)
                + (0.11693 * th3 - 0.21196 * theta_s * theta_s + 0.06052 * theta_s + 0.25886),
            yz: t2 * (0.00275 * th3 - 0.00610 * th2 + 0.00317 * theta_s)
                + t * (-0.04214 * th3 + 0.08970 * theta_s * theta_s - 0.04153 * theta_s + 0.00516)
                + (0.15346 * th3 - 0.26756 * theta_s * theta_s + 0.06670 * theta_s + 0.26688),
            d_cy: perez(cy, 1.0, theta_s),
            d_cx: perez(cx, 1.0, theta_s),
            d_cyy: perez(cyy, 1.0, theta_s),
        }
    }
}

/// The day sky (no twilight scaling) in linear nits; `v` is unit, `pre` was
/// precomputed from the sun and turbidity.
fn day_sky(v: [f64; 3], pre: &DaySky) -> [f64; 3] {
    let gamma = (v[0] * pre.sun[0] + v[1] * pre.sun[1] + v[2] * pre.sun[2]).clamp(-1.0, 1.0).acos();
    let cos_t = v[2].max(0.0);
    let ratio = |c: [f64; 5], d: f64| {
        if d.abs() < 1e-9 { 1.0 } else { perez(c, cos_t, gamma) / d }
    };
    let lum = pre.lum_base * ratio(pre.cy, pre.d_cy);
    let x = (pre.xz * ratio(pre.cx, pre.d_cx)).clamp(0.0, 0.8);
    let y = (pre.yz * ratio(pre.cyy, pre.d_cyy)).clamp(0.02, 0.85);
    let lum = if lum.is_finite() { lum.max(0.0) } else { 0.0 };
    let big_x = x / y * lum;
    let big_z = (1.0 - x - y) / y * lum;
    let rgb = [
        3.2406 * big_x - 1.5372 * lum - 0.4986 * big_z,
        -0.9689 * big_x + 1.8758 * lum + 0.0415 * big_z,
        0.0557 * big_x - 0.2040 * lum + 1.0570 * big_z,
    ];
    rgb.map(|c| if c.is_finite() { c.max(0.0) } else { 0.0 })
}

/// Sun direction for the day-sky formulas (clamped up to the horizon) and its twilight factor.
fn sun_for_day(sun: [f64; 3]) -> Option<([f64; 3], f64)> {
    let s = unit(sun)?;
    let tw = twilight_factor(s[2].clamp(-1.0, 1.0).asin().to_degrees());
    if s[2] >= 0.0 {
        return Some((s, tw));
    }
    let h = (s[0] * s[0] + s[1] * s[1]).sqrt();
    let flat = if h > 1e-9 { [s[0] / h, s[1] / h, 0.0] } else { [1.0, 0.0, 0.0] };
    Some((flat, tw))
}

thread_local! {
    static GROUND: Cell<Option<([u64; 4], f64)>> = const { Cell::new(None) };
}

/// Horizontal illuminance of sky plus sun, lux, cached for the last sun and turbidity.
fn horizontal_illuminance(s: [f64; 3], tw: f64, t: f64) -> f64 {
    let key = [s[0].to_bits(), s[1].to_bits(), s[2].to_bits(), t.to_bits()];
    if let Some((k, e)) = GROUND.with(|g| g.get())
        && k == key
    {
        return e;
    }
    let (nt, np) = (6, 12);
    let pre = DaySky::new(s, t);
    let mut sum = 0.0;
    for i in 0..nt {
        let theta = (i as f64 + 0.5) / nt as f64 * std::f64::consts::FRAC_PI_2;
        let (st, ct) = theta.sin_cos();
        for j in 0..np {
            let phi = (j as f64 + 0.5) / np as f64 * std::f64::consts::TAU;
            let c = day_sky([st * phi.cos(), st * phi.sin(), ct], &pre);
            let lum = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
            sum += lum * ct * st;
        }
    }
    let dtheta = std::f64::consts::FRAC_PI_2 / nt as f64;
    let dphi = std::f64::consts::TAU / np as f64;
    let e = (sum * dtheta * dphi + SUN_LUX * s[2].max(0.0)) * tw;
    let e = if e.is_finite() { e } else { 0.0 };
    GROUND.with(|g| g.set(Some((key, e))));
    e
}

/// A sky for one sun and turbidity with the per-sun constants computed once, so a bake of
/// many texels costs one `radiance` call each and nothing more.
#[derive(Clone)]
pub struct SkyModel {
    /// `None` when the sun direction is unusable (the night sky applies everywhere).
    day: Option<(DaySky, f64, f64)>,
    /// Grey-ground radiance below the horizon, computed on first use.
    ground: std::cell::OnceCell<f64>,
}

impl SkyModel {
    /// The sky for `sun` (toward the sun) and `turbidity` (clamped to 1.7..=10, 3 if not finite).
    pub fn new(sun: [f64; 3], turbidity: f64) -> Self {
        let t = if turbidity.is_finite() { turbidity.clamp(1.7, 10.0) } else { 3.0 };
        let day = sun_for_day(sun).map(|(s, tw)| (DaySky::new(s, t), tw, t));
        Self { day, ground: std::cell::OnceCell::new() }
    }

    /// Radiance in linear nits looking along `view`; the value [`sky_radiance`] returns.
    pub fn radiance(&self, view: [f64; 3]) -> [f64; 3] {
        let (Some(v), Some((pre, tw, t))) = (unit(view), self.day.as_ref()) else { return NIGHT_SKY };
        let mut out = if v[2] >= 0.0 {
            day_sky(v, pre).map(|c| c * tw)
        } else {
            let g = *self.ground.get_or_init(|| GROUND_ALBEDO / std::f64::consts::PI * horizontal_illuminance(pre.sun, *tw, *t));
            [g, g, g]
        };
        for (o, n) in out.iter_mut().zip(NIGHT_SKY) {
            *o = if o.is_finite() { *o + n } else { n };
        }
        out
    }
}

/// Sky radiance in linear nits (Rec.709) looking along `view` with the sun toward `sun`.
/// Below the horizon: a grey ground of albedo 0.2 lit by the sky and sun. Never NaN.
pub fn sky_radiance(view: [f64; 3], sun: [f64; 3], turbidity: f64) -> [f64; 3] {
    SkyModel::new(sun, turbidity).radiance(view)
}

/// Unit vector toward the moon in drawing space: compass azimuth (0 north, 90 east) and
/// elevation in degrees, turned by the site's true north. `None` when not finite.
pub fn moon_dir(model: &ArchModel, moon: &MoonLook) -> Option<[f64; 3]> {
    if !moon.azimuth.is_finite() || !moon.elevation.is_finite() {
        return None;
    }
    let (az, el) = (moon.azimuth.to_radians(), moon.elevation.to_radians());
    let v = [az.sin() * el.cos(), az.cos() * el.cos(), el.sin()];
    unit(cadcraft_arch::site::true_to_drawing(model.site.as_ref(), v))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sun_at(elev_deg: f64, az_deg: f64) -> [f64; 3] {
        let (e, a) = (elev_deg.to_radians(), az_deg.to_radians());
        [a.cos() * e.cos(), a.sin() * e.cos(), e.sin()]
    }

    #[test]
    fn zenith_luminance_t3_theta27_61_is_11_08_kcd() {
        let y = zenith_luminance(27.61f64.to_radians(), 3.0);
        assert!((y - 11.08).abs() < 0.05, "{y}");
    }

    #[test]
    fn noon_bluer_low_sun_redder_toward_sun() {
        let noon = sky_radiance([0.0, 0.5, 0.5], sun_at(62.0, 0.0), 3.0);
        assert!(noon[2] > noon[0], "{noon:?}");
        let toward = sky_radiance(sun_at(5.0, 0.0), sun_at(2.0, 0.0), 3.0);
        assert!(toward[0] > toward[2], "{toward:?}");
    }

    #[test]
    fn twilight_fades_by_minus_6_and_night_under_1_percent() {
        assert_eq!(twilight_factor(5.0), 1.0);
        assert!((twilight_factor(-3.0) - 0.01).abs() < 1e-9);
        assert!((twilight_factor(-6.0) - 1e-4).abs() < 1e-12);
        assert_eq!(twilight_factor(-7.0), 0.0);
        let max = |c: [f64; 3]| c[0].max(c[1]).max(c[2]);
        let noon = max(sky_radiance([0.0, 0.5, 0.8], sun_at(62.0, 0.0), 3.0));
        let night = max(sky_radiance([0.0, 0.5, 0.8], sun_at(-10.0, 0.0), 3.0));
        assert!(night < 0.01 * noon, "{night} {noon}");
    }

    #[test]
    fn stars_only_at_night_deterministic() {
        let n = 20_000;
        let mut count = 0;
        let mut found = None;
        for i in 0..n {
            let a = i as f64 * 0.37;
            let v = [a.cos() * 0.6, a.sin() * 0.6, 0.2 + 0.8 * (i as f64 / n as f64)];
            assert_eq!(star(v), star(v));
            if star(v) > 0.0 {
                count += 1;
                found = Some(v);
            }
        }
        assert!(count > 0 && count < n / 20, "{count}");
        let v = found.unwrap_or([0.0, 0.0, 1.0]);
        assert_eq!(star([v[0], v[1], -v[2]]), 0.0);
        assert_eq!(star_radiance(v, sun_at(40.0, 0.0)), 0.0);
        assert!(star_radiance(v, sun_at(-20.0, 0.0)) > 0.0);
    }

    #[test]
    fn no_nan_at_horizon_or_hostile_input() {
        let nan = f64::NAN;
        let inf = f64::INFINITY;
        let cases = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [nan, 0.0, 1.0], [inf, 0.0, 0.0], [1e300, 1e300, 1e300]];
        for v in cases {
            for s in cases.iter().copied().chain([sun_at(0.0, 0.0), sun_at(90.0, 0.0), sun_at(-90.0, 0.0)]) {
                for t in [3.0, nan, inf, -5.0, 0.0] {
                    let c = sky_radiance(v, s, t);
                    assert!(c.iter().all(|x| x.is_finite() && *x >= 0.0), "{v:?} {s:?} {t} {c:?}");
                }
            }
            assert!(star(v).is_finite());
        }
        assert!(zenith_luminance(nan, 3.0).is_finite());
        assert_eq!(twilight_factor(nan), 0.0);
    }

    #[test]
    fn moon_dir_follows_true_north() {
        let m = ArchModel::default();
        let look = MoonLook { azimuth: 90.0, elevation: 0.0, ..Default::default() };
        let d = moon_dir(&m, &look).unwrap_or([0.0; 3]);
        assert!((d[0] - 1.0).abs() < 1e-9 && d[1].abs() < 1e-9 && d[2].abs() < 1e-9, "{d:?}");
        let up = moon_dir(&m, &MoonLook { elevation: 30.0, ..look.clone() }).unwrap_or([0.0; 3]);
        assert!((up[2] - 0.5).abs() < 1e-9);
        let rotated = ArchModel { site: Some(cadcraft_doc::arch::SiteLocation { true_north: 90.0, ..Default::default() }), ..Default::default() };
        let north = MoonLook { azimuth: 0.0, elevation: 0.0, ..Default::default() };
        let d = moon_dir(&rotated, &north).unwrap_or([0.0; 3]);
        assert!((d[0] + 1.0).abs() < 1e-9 && d[1].abs() < 1e-9, "{d:?}");
        assert!(moon_dir(&m, &MoonLook { azimuth: f64::NAN, ..look }).is_none());
    }

    #[test]
    fn twilight_factor_is_monotonic() {
        let mut prev = f64::INFINITY;
        for i in 0..=120 {
            let elev = (i as f64 / 120.0) * -6.0;
            let f = twilight_factor(elev);
            assert!(f <= prev, "twilight_factor not monotonic at elev={}: {} > {}", elev, f, prev);
            prev = f;
        }
    }

    #[test]
    fn sky_is_rotationally_symmetric_around_sun_at_zenith() {
        let sun = [0.0, 0.0, 1.0];
        let v1 = [0.7071, 0.0, 0.7071];
        let v2 = [0.0, 0.7071, 0.7071];
        let c1 = sky_radiance(v1, sun, 3.0);
        let c2 = sky_radiance(v2, sun, 3.0);
        for i in 0..3 {
            assert!((c1[i] - c2[i]).abs() < 1e-9, "sky not rotationally symmetric: {} vs {}", c1[i], c2[i]);
        }
    }

    #[test]
    fn ground_is_darker_than_sky() {
        let sun = sun_at(45.0, 0.0);
        // Compare ground against sky toward the sun, where the sky is brightest.
        let sky = sky_radiance(sun, sun, 3.0);
        let ground = sky_radiance([0.0, -0.5, -0.5], sun, 3.0);
        for i in 0..3 {
            assert!(ground[i] < sky[i], "ground should be darker: ground[{}]={} sky[{}]={}", i, ground[i], i, sky[i]);
        }
    }

    #[test]
    fn high_turbidity_makes_sky_more_diffuse() {
        let sun = sun_at(30.0, 0.0);
        let low_near = sky_radiance(sun_at(30.0, 0.0), sun, 2.0);
        let low_far = sky_radiance([0.0, 0.0, 1.0], sun, 2.0);
        let high_near = sky_radiance(sun_at(30.0, 0.0), sun, 8.0);
        let high_far = sky_radiance([0.0, 0.0, 1.0], sun, 8.0);
        let low_ratio = low_near[1] / low_far[1];
        let high_ratio = high_near[1] / high_far[1];
        assert!(high_ratio < low_ratio, "higher turbidity should be more diffuse: high={:.3} low={:.3}", high_ratio, low_ratio);
    }

    #[test]
    fn moon_at_zenith_points_up() {
        let m = ArchModel::default();
        let look = MoonLook { azimuth: 0.0, elevation: 90.0, ..Default::default() };
        let d = moon_dir(&m, &look).unwrap_or([0.0; 3]);
        assert!((d[2] - 1.0).abs() < 1e-9, "moon at zenith should point up: {:?}", d);
    }

    #[test]
    fn moon_at_horizon_has_zero_z() {
        let m = ArchModel::default();
        let look = MoonLook { azimuth: 0.0, elevation: 0.0, ..Default::default() };
        let d = moon_dir(&m, &look).unwrap_or([0.0; 3]);
        assert!(d[2].abs() < 1e-9, "moon at horizon should have z=0: {:?}", d);
    }

    #[test]
    fn sky_radiance_at_horizon_is_finite() {
        let sun = sun_at(45.0, 0.0);
        let c = sky_radiance([1.0, 0.0, 0.0], sun, 3.0);
        assert!(c.iter().all(|x| x.is_finite() && *x >= 0.0), "horizon sky should be finite and non-negative: {:?}", c);
    }

    #[test]
    fn star_density_is_about_one_percent() {
        let n = 100_000;
        let mut count = 0;
        for i in 0..n {
            let a = i as f64 * 0.37;
            let v = [a.cos() * 0.6, a.sin() * 0.6, 0.2 + 0.8 * (i as f64 / n as f64)];
            if star(v) > 0.0 {
                count += 1;
            }
        }
        let density = count as f64 / n as f64;
        assert!(density > 0.005 && density < 0.02, "star density should be about 1%, got {}", density);
    }

    #[test]
    fn sky_model_radiance_matches_sky_radiance() {
        let sun = [0.3, 0.2, 0.7];
        let view = [0.1, -0.4, 0.5];
        let turbidity = 4.5;
        let single = sky_radiance(view, sun, turbidity);
        let model = SkyModel::new(sun, turbidity);
        let via_model = model.radiance(view);
        for i in 0..3 {
            assert!((single[i] - via_model[i]).abs() < 1e-12, "SkyModel must match sky_radiance: {} vs {}", single[i], via_model[i]);
        }
    }

    #[test]
    fn horizontal_illuminance_cache_returns_correct_values() {
        // Two different suns must give different illuminances;
        // the one-entry cache must not return the wrong value.
        let sun1 = [0.0, 0.0, 1.0]; // Zenith
        let sun2 = [0.7071, 0.0, 0.7071]; // 45 degrees
        let e1 = horizontal_illuminance(sun1, 1.0, 3.0);
        let e2 = horizontal_illuminance(sun2, 1.0, 3.0);
        assert_ne!(e1, e2, "different suns must give different illuminances");
        // Sun at zenith should give more illuminance than sun at 45 degrees
        assert!(e1 > e2, "zenith sun ({e1}) must be brighter than 45-degree sun ({e2})");
        // Re-querying the first sun after the second must still return the correct value
        let e1_again = horizontal_illuminance(sun1, 1.0, 3.0);
        assert_eq!(e1, e1_again, "re-querying must return the same value");
    }

    #[test]
    fn twilight_factor_at_exact_boundaries() {
        assert_eq!(twilight_factor(0.0), 1.0, "at horizon must be 1.0");
        assert!((twilight_factor(-6.0) - 1e-4).abs() < 1e-15, "at -6 degrees must be 1e-4");
        assert_eq!(twilight_factor(-6.0001), 0.0, "just below -6 must be 0.0");
        assert_eq!(twilight_factor(f64::INFINITY), 1.0, "infinite elevation must be 1.0");
        assert_eq!(twilight_factor(f64::NEG_INFINITY), 0.0, "negative infinite elevation must be 0.0");
    }

    #[test]
    fn zenith_luminance_is_non_negative_for_all_inputs() {
        for theta in [0.0, 0.5, 1.0, 1.5, std::f64::consts::FRAC_PI_2] {
            for t in [1.7, 3.0, 5.0, 10.0] {
                let y = zenith_luminance(theta, t);
                assert!(y >= 0.0 && y.is_finite(), "theta={theta} t={t}: {y}");
            }
        }
    }
}
