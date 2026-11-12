//! Turning a render look into the lighting a frame is drawn with (phase 19).

use cadcraft_doc::arch::{ExposureSetting, Look};

use crate::exposure::ev100_scale;
use crate::light::{Light, Sun};
use crate::lights::DEFAULT_EXPOSURE;
use crate::lit_frame::Lighting;

/// The lighting for `look` with an optional explicit toward-sun direction (drawing space).
/// No sun when the look has it off or the direction points at or below the horizon; the default
/// light when there is no direction; the shadow map is built only for an explicit direction.
pub fn lighting_for(look: &Look, sun_dir: Option<[f64; 3]>, budget: usize) -> Lighting {
    let sun = if !look.sun.on {
        None
    } else {
        match sun_dir {
            None => Some(Light::DEFAULT),
            Some(d) if d[2] > 0.0 => {
                Some(Light::from_sun(&Sun { dir: [d[0] as f32, d[1] as f32, d[2] as f32], ..Sun::default() }, Light::DEFAULT.ambient))
            }
            Some(_) => None,
        }
    };
    let exposure = match look.exposure {
        ExposureSetting::Manual { ev100 } => ev100_scale(ev100),
        ExposureSetting::Auto => DEFAULT_EXPOSURE,
    };
    Lighting {
        sun_shadow: sun.is_some() && sun_dir.is_some() && look.sun.shadows,
        sun,
        budget,
        exposure,
        look: look.clone(),
        sun_dir,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lighting_for_default_matches_old_hook() {
        let l = lighting_for(&Look::default(), None, 8);
        assert_eq!(l.sun, Some(Light::DEFAULT));
        assert!(!l.sun_shadow);
        assert_eq!(l.budget, 8);
        assert_eq!(l.exposure, DEFAULT_EXPOSURE);
        // An explicit direction: the old SunChoice::Dir meaning, shadows on.
        let d = [0.0, 0.0, 1.0];
        let l = lighting_for(&Look::default(), Some(d), 8);
        let old = Light::from_sun(&Sun { dir: [0.0, 0.0, 1.0], ..Sun::default() }, Light::DEFAULT.ambient);
        assert_eq!(l.sun, Some(old));
        assert!(l.sun_shadow);
        // Night: sun off, or a direction below the horizon.
        let mut off = Look::default();
        off.sun.on = false;
        assert_eq!(lighting_for(&off, Some(d), 8).sun, None);
        assert_eq!(lighting_for(&Look::default(), Some([0.0, 0.0, -1.0]), 8).sun, None);
    }

    #[test]
    fn lighting_for_honours_shadows_and_on() {
        let d = [0.0, 0.0, 1.0];
        let mut look = Look::default();
        look.sun.shadows = false;
        let l = lighting_for(&look, Some(d), 8);
        assert!(l.sun.is_some() && !l.sun_shadow, "sun on, shadows off");
        look.sun.shadows = true;
        assert!(lighting_for(&look, Some(d), 8).sun_shadow);
        look.sun.on = false;
        let l = lighting_for(&look, Some(d), 8);
        assert!(l.sun.is_none() && !l.sun_shadow, "sun off");
    }

    #[test]
    fn lighting_for_honours_manual_exposure() {
        let mut look = Look::default();
        look.exposure = ExposureSetting::Manual { ev100: 4.0 };
        let l = lighting_for(&look, None, 8);
        assert!((l.exposure - ev100_scale(4.0)).abs() < 1e-6);

        look.exposure = ExposureSetting::Auto;
        let l = lighting_for(&look, None, 8);
        assert_eq!(l.exposure, DEFAULT_EXPOSURE);
    }
}
