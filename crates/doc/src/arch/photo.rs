//! Photoreal still settings and the rung table (phase 22). Data only.

use serde::{Deserialize, Serialize};

use super::look::Film;

/// Rung names, index = rung number.
pub const RUNG_NAMES: [&str; 8] = ["today", "lens", "edges", "materials", "light", "film", "adaptive", "learned denoiser"];
/// The highest rung, the full photoreal pipeline.
pub const PHOTOREAL_RUNG: u8 = 7;
/// Default edge bevel radius in metres.
pub const DEFAULT_EDGE_RADIUS: f64 = 0.002;
/// Angular diameter of the sun disc in degrees.
pub const SUN_DISC_DEG: f64 = 0.53;
/// Path bounces at the light rung and above.
pub const PHOTOREAL_BOUNCES: u32 = 12;
/// Firefly clamp at the light rung and above.
pub const PHOTOREAL_CLAMP: f64 = 32.0;
/// Default adaptive noise target.
pub const NOISE_TARGET: f64 = 0.02;
/// Default adaptive time budget, seconds.
pub const TIME_BUDGET_S: f64 = 600.0;
/// Per-pixel sample cap for adaptive renders.
pub const ADAPTIVE_MAX_SPP: u32 = 4096;

/// Which denoiser runs after the path tracer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DenoiseMode {
    /// No denoiser.
    None,
    /// The edge-aware filter.
    #[default]
    Filter,
    /// The learned denoiser.
    Learned,
}

/// Which learned denoiser model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DenoiseModel {
    /// Pick by image size.
    #[default]
    Auto,
    /// The base model.
    Base,
    /// The small model.
    Small,
}

/// Edge rounding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EdgeMode {
    /// Sharp edges.
    #[default]
    Off,
    /// Each material's own edge radius.
    PerMaterial,
    /// One radius in metres for everything.
    Radius(f64),
}

/// Material model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Materials {
    /// The basic model.
    #[default]
    Basic,
    /// The full layered model.
    Full,
}

/// Settings of one photoreal still.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PhotoSettings {
    /// The rung these settings came from, if any.
    pub rung: Option<u8>,
    /// Apply the look's lens.
    pub lens: bool,
    /// Edge rounding.
    pub edges: EdgeMode,
    /// Material model.
    pub materials: Materials,
    /// Procedural material variation.
    pub variation: bool,
    /// Path bounces.
    pub bounces: u32,
    /// Sun disc diameter in degrees; `None` = the look's.
    pub sun_disc: Option<f64>,
    /// Firefly clamp in display units; `None` = none.
    pub clamp: Option<f64>,
    /// Film; `None` = the look's, `Some` replaces it.
    pub film: Option<Film>,
    /// Adaptive sampling.
    pub adaptive: bool,
    /// Adaptive noise target.
    pub noise_target: Option<f64>,
    /// Adaptive time budget, seconds.
    pub time_budget: Option<f64>,
    /// Feed a roughness guide to the denoiser.
    pub roughness_guide: bool,
    /// Denoiser.
    pub denoise: DenoiseMode,
    /// Learned denoiser model.
    pub denoise_model: DenoiseModel,
    /// Samples per pixel (the cap when adaptive).
    pub spp: u32,
}

impl Default for PhotoSettings {
    fn default() -> Self {
        PhotoSettings {
            rung: None,
            lens: true,
            edges: EdgeMode::Off,
            materials: Materials::Basic,
            variation: false,
            bounces: 4,
            sun_disc: None,
            clamp: None,
            film: None,
            adaptive: false,
            noise_target: None,
            time_budget: None,
            roughness_guide: false,
            denoise: DenoiseMode::Filter,
            denoise_model: DenoiseModel::Auto,
            spp: 256,
        }
    }
}

/// The settings of rung `n`; rungs are cumulative and `n > 7` behaves as 7.
pub fn rung(n: u8) -> PhotoSettings {
    let n = n.min(PHOTOREAL_RUNG);
    let mut s = PhotoSettings { rung: Some(n), lens: false, film: Some(Film::default()), ..PhotoSettings::default() };
    if n >= 1 {
        s.lens = true;
    }
    if n >= 2 {
        s.edges = EdgeMode::PerMaterial;
    }
    if n >= 3 {
        s.materials = Materials::Full;
        s.variation = true;
    }
    if n >= 4 {
        s.bounces = PHOTOREAL_BOUNCES;
        s.sun_disc = Some(SUN_DISC_DEG);
        s.clamp = Some(PHOTOREAL_CLAMP);
    }
    if n >= 5 {
        s.film = Some(Film::photoreal());
    }
    if n >= 6 {
        s.adaptive = true;
        s.noise_target = Some(NOISE_TARGET);
        s.time_budget = Some(TIME_BUDGET_S);
        s.roughness_guide = true;
        s.spp = ADAPTIVE_MAX_SPP;
    }
    if n >= 7 {
        s.denoise = DenoiseMode::Learned;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rung_table_is_cumulative() {
        let r0 = rung(0);
        assert_eq!(r0.rung, Some(0));
        assert!(!r0.lens);
        assert_eq!(r0.film, Some(Film::default()));
        assert_eq!(r0.bounces, 4);
        assert!(rung(1).lens && rung(1).edges == EdgeMode::Off);
        assert_eq!(rung(2).edges, EdgeMode::PerMaterial);
        assert!(rung(3).variation && rung(3).materials == Materials::Full && rung(3).bounces == 4);
        let r4 = rung(4);
        assert_eq!((r4.bounces, r4.sun_disc, r4.clamp), (12, Some(0.53), Some(32.0)));
        assert_eq!(r4.film, Some(Film::default()));
        assert_eq!(rung(5).film, Some(Film::photoreal()));
        assert!(!rung(5).adaptive);
        let r6 = rung(6);
        assert!(r6.adaptive && r6.roughness_guide);
        assert_eq!((r6.noise_target, r6.time_budget, r6.spp), (Some(0.02), Some(600.0), 4096));
        assert_eq!(r6.denoise, DenoiseMode::Filter);
        assert_eq!(rung(7).denoise, DenoiseMode::Learned);
        let mut r9 = rung(9);
        r9.rung = Some(7);
        assert_eq!(r9, rung(7));
        for n in 0..=7u8 {
            assert_eq!(rung(n).rung, Some(n));
        }
        assert_eq!(RUNG_NAMES.len(), PHOTOREAL_RUNG as usize + 1);
    }

    #[test]
    fn photo_settings_default_has_no_rung() {
        let s = PhotoSettings::default();
        assert_eq!(s.rung, None);
        assert!(s.lens);
        assert_eq!(s.edges, EdgeMode::Off);
        assert_eq!(s.materials, Materials::Basic);
        assert!(!s.variation);
        assert_eq!(s.bounces, 4);
        assert_eq!(s.sun_disc, None);
        assert_eq!(s.clamp, None);
        assert_eq!(s.film, None);
        assert!(!s.adaptive);
        assert_eq!(s.noise_target, None);
        assert_eq!(s.time_budget, None);
        assert!(!s.roughness_guide);
        assert_eq!(s.denoise, DenoiseMode::Filter);
        assert_eq!(s.denoise_model, DenoiseModel::Auto);
        assert_eq!(s.spp, 256);
    }

    #[test]
    fn photo_settings_serde_round_trip() {
        let s = PhotoSettings {
            rung: Some(5),
            lens: true,
            edges: EdgeMode::Radius(0.003),
            materials: Materials::Full,
            variation: true,
            bounces: 12,
            sun_disc: Some(0.53),
            clamp: Some(32.0),
            film: Some(Film::photoreal()),
            adaptive: true,
            noise_target: Some(0.02),
            time_budget: Some(600.0),
            roughness_guide: true,
            denoise: DenoiseMode::Learned,
            denoise_model: DenoiseModel::Small,
            spp: 4096,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: PhotoSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        assert_eq!(back.edges, EdgeMode::Radius(0.003));
        assert_eq!(back.denoise_model, DenoiseModel::Small);
    }

    #[test]
    fn edge_mode_serde_all_variants() {
        assert_eq!(serde_json::to_string(&EdgeMode::Off).unwrap(), "\"off\"");
        assert_eq!(serde_json::to_string(&EdgeMode::PerMaterial).unwrap(), "\"perMaterial\"");
        let radius = serde_json::to_string(&EdgeMode::Radius(0.002)).unwrap();
        assert!(radius.starts_with("{\"radius\":"), "{radius}");
        let back: EdgeMode = serde_json::from_str(&radius).unwrap();
        assert_eq!(back, EdgeMode::Radius(0.002));
        let off: EdgeMode = serde_json::from_str("\"off\"").unwrap();
        assert_eq!(off, EdgeMode::Off);
        let pm: EdgeMode = serde_json::from_str("\"perMaterial\"").unwrap();
        assert_eq!(pm, EdgeMode::PerMaterial);
    }

    #[test]
    fn denoise_mode_and_model_serde() {
        assert_eq!(serde_json::to_string(&DenoiseMode::None).unwrap(), "\"none\"");
        assert_eq!(serde_json::to_string(&DenoiseMode::Filter).unwrap(), "\"filter\"");
        assert_eq!(serde_json::to_string(&DenoiseMode::Learned).unwrap(), "\"learned\"");
        assert_eq!(serde_json::to_string(&DenoiseModel::Auto).unwrap(), "\"auto\"");
        assert_eq!(serde_json::to_string(&DenoiseModel::Base).unwrap(), "\"base\"");
        assert_eq!(serde_json::to_string(&DenoiseModel::Small).unwrap(), "\"small\"");
        let m: DenoiseMode = serde_json::from_str("\"learned\"").unwrap();
        assert_eq!(m, DenoiseMode::Learned);
        let d: DenoiseModel = serde_json::from_str("\"small\"").unwrap();
        assert_eq!(d, DenoiseModel::Small);
    }

    #[test]
    fn materials_serde() {
        assert_eq!(serde_json::to_string(&Materials::Basic).unwrap(), "\"basic\"");
        assert_eq!(serde_json::to_string(&Materials::Full).unwrap(), "\"full\"");
        let f: Materials = serde_json::from_str("\"full\"").unwrap();
        assert_eq!(f, Materials::Full);
    }

    #[test]
    fn photo_settings_old_json_loads_with_defaults() {
        let old = r#"{"lens":true,"bounces":4,"spp":256}"#;
        let s: PhotoSettings = serde_json::from_str(old).unwrap();
        assert_eq!(s.rung, None);
        assert!(s.lens);
        assert_eq!(s.bounces, 4);
        assert_eq!(s.spp, 256);
        assert_eq!(s.edges, EdgeMode::Off);
        assert_eq!(s.materials, Materials::Basic);
        assert!(!s.adaptive);
        assert_eq!(s.denoise, DenoiseMode::Filter);
    }

    #[test]
    fn photo_settings_none_fields_skip_serialization() {
        let s = PhotoSettings::default();
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("rung"), "rung should be skipped when None: {json}");
        assert!(!json.contains("sunDisc"), "sunDisc should be skipped when None: {json}");
        assert!(!json.contains("clamp"), "clamp should be skipped when None: {json}");
        assert!(!json.contains("film"), "film should be skipped when None: {json}");
        assert!(!json.contains("noiseTarget"), "noiseTarget should be skipped when None: {json}");
        assert!(!json.contains("timeBudget"), "timeBudget should be skipped when None: {json}");
    }

    #[test]
    fn rung_above_max_behaves_as_max() {
        let r8 = rung(8);
        let r9 = rung(9);
        let r100 = rung(100);
        let r7 = rung(7);
        assert_eq!(r8, r7);
        assert_eq!(r9, r7);
        assert_eq!(r100, r7);
        assert_eq!(r8.rung, Some(7));
        assert_eq!(r9.rung, Some(7));
        assert_eq!(r100.rung, Some(7));
    }
}
