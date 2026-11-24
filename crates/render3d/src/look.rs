//! Turning a render look into the lighting a frame is drawn with (phase 19).

use cadcraft_doc::arch::photo::{Materials, PhotoSettings};
use cadcraft_doc::arch::{Film, Look};

use crate::lens::LensOptions;
use crate::light::{Light, Sun};
use crate::lights::DEFAULT_EXPOSURE;
use crate::lit_frame::Lighting;
use crate::pathtrace::PathOptions;

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
    Lighting {
        sun_shadow: sun.is_some() && sun_dir.is_some() && look.sun.shadows,
        sun,
        budget,
        exposure: DEFAULT_EXPOSURE,
        look: look.clone(),
        sun_dir,
        lens: LensOptions::from_look(&look.lens),
        film: look.film.clone(),
        sun_disc_deg: look.sun.disc as f32,
        ..Default::default()
    }
}

/// Apply a still's photo settings to the path options and the lighting. The CLI hook and the pane
/// both call this, so they cannot disagree.
pub fn apply_photo(opts: &mut PathOptions, lighting: &mut Lighting, p: &PhotoSettings) {
    opts.edges = p.edges;
    opts.materials_full = p.materials == Materials::Full;
    opts.variation = p.variation;
    opts.clamp = p.clamp.map(|c| c as f32);
    opts.adaptive = p.adaptive;
    opts.noise_target = p.noise_target.map(|v| v as f32);
    opts.time_budget = p.time_budget.map(|v| v as f32);
    opts.roughness_guide = p.roughness_guide;
    opts.denoise_mode = p.denoise;
    opts.denoise_model = p.denoise_model;
    opts.bounces = p.bounces;
    opts.spp = p.spp;
    if !p.lens {
        lighting.lens = LensOptions::default();
    }
    if let Some(f) = &p.film {
        lighting.film = f.clone();
    }
    if let Some(d) = p.sun_disc {
        lighting.sun_disc_deg = d as f32;
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
    fn lighting_for_carries_lens_film_and_disc() {
        let n = lighting_for(&Look::default(), None, 8);
        assert!(n.lens.is_neutral() && n.film.is_neutral() && n.sun_disc_deg == 0.0);
        let mut look = Look::default();
        look.lens.shift = 0.2;
        look.film = Film::photoreal();
        look.sun.disc = 0.53;
        let l = lighting_for(&look, None, 8);
        assert_eq!(l.lens.shift, 0.2);
        assert_eq!(l.film, Film::photoreal());
        assert!((l.sun_disc_deg - 0.53).abs() < 1e-6);
    }

    #[test]
    fn apply_photo_maps_every_setting() {
        use cadcraft_doc::arch::photo::{DenoiseMode, DenoiseModel, EdgeMode};
        let mut look = Look::default();
        look.lens.shift = 0.3;
        let mut lighting = lighting_for(&look, None, 8);
        let mut opts = PathOptions::default();
        let p = PhotoSettings {
            lens: false,
            edges: EdgeMode::Radius(0.01),
            materials: Materials::Full,
            variation: true,
            bounces: 12,
            sun_disc: Some(0.53),
            clamp: Some(32.0),
            film: Some(Film::photoreal()),
            adaptive: true,
            noise_target: Some(0.02),
            time_budget: Some(60.0),
            roughness_guide: true,
            denoise: DenoiseMode::Learned,
            denoise_model: DenoiseModel::Small,
            spp: 512,
            ..PhotoSettings::default()
        };
        apply_photo(&mut opts, &mut lighting, &p);
        assert_eq!(opts.edges, EdgeMode::Radius(0.01));
        assert!(opts.materials_full && opts.variation && opts.adaptive && opts.roughness_guide);
        assert_eq!((opts.clamp, opts.noise_target, opts.time_budget), (Some(32.0), Some(0.02), Some(60.0)));
        assert_eq!((opts.denoise_mode, opts.denoise_model), (DenoiseMode::Learned, DenoiseModel::Small));
        assert_eq!((opts.bounces, opts.spp), (12, 512));
        assert!(lighting.lens.is_neutral());
        assert_eq!(lighting.film, Film::photoreal());
        assert!((lighting.sun_disc_deg - 0.53).abs() < 1e-6);
        // Defaults leave the lens, film and disc of the look alone.
        let mut lighting = lighting_for(&look, None, 8);
        apply_photo(&mut PathOptions::default(), &mut lighting, &PhotoSettings::default());
        assert_eq!(lighting.lens.shift, 0.3);
        assert!(lighting.film.is_neutral() && lighting.sun_disc_deg == 0.0);
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
}
