//! The photoreal parts of a render reply and the calibration scene key: the `photo` echo, the path
//! stills' stop and sample statistics, the denoiser report and the PFM image.

use std::hash::{Hash, Hasher};

use cadcraft_arch3d::calibrate::Calibration;
use cadcraft_arch3d::lens::LensView;
use cadcraft_engine::LinearStageParam;
use cadcraft_engine::doc::arch::{DenoiseMode, EdgeMode, Materials, PhotoSettings};
use cadcraft_render3d::pathtrace::{ComputeGpu, LinearStage, PathStats};
use cadcraft_render3d::{Lighting, lens::encode_pfm};
use serde_json::{Map, Value, json};

/// Mixed into a calibration scene's key so it never equals a drawing revision.
const CALIBRATE_KEY_SALT: u64 = 0xCA11_B8A7_E5CE_4E01;

/// The scene key of a calibration: a hash of its `Debug` text mixed with a constant, top bit set.
pub(crate) fn calibrate_key(c: &Calibration) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{c:?}").hash(&mut h);
    (h.finish() ^ CALIBRATE_KEY_SALT) | (1 << 63)
}

/// The lens the picking and the sample points follow: the lighting's (the look's, after the
/// photo settings were applied).
pub(crate) fn lens_view(lighting: &Lighting) -> LensView {
    let mut v = LensView::default();
    v.panini = lighting.lens.panini;
    v.compression = f64::from(lighting.lens.compression);
    v.shift = f64::from(lighting.lens.shift);
    v
}

/// The renderer's linear stage for a request's.
pub(crate) fn stage_of(s: LinearStageParam) -> LinearStage {
    match s {
        LinearStageParam::Radiance => LinearStage::Radiance,
        LinearStageParam::Film => LinearStage::Film,
    }
}

fn stage_name(s: LinearStageParam) -> &'static str {
    match s {
        LinearStageParam::Radiance => "radiance",
        LinearStageParam::Film => "film",
    }
}

fn denoise_name(d: DenoiseMode) -> &'static str {
    match d {
        DenoiseMode::None => "none",
        DenoiseMode::Filter => "filter",
        DenoiseMode::Learned => "learned",
    }
}

/// Whether a still keeps its requested samples per pixel: a rung or an adaptive render is never
/// lowered on a software adapter.
pub(crate) fn keeps_spp(photo: &PhotoSettings) -> bool {
    photo.rung.is_some() || photo.adaptive
}

/// The `photo` reply key: the settings asked for, with the sun disc and bounces actually used.
pub(crate) fn photo_value(p: &PhotoSettings, bounces: u32, sun_disc_deg: f32) -> Value {
    let edges = match p.edges {
        EdgeMode::Off => json!(false),
        EdgeMode::PerMaterial => json!(true),
        EdgeMode::Radius(r) => json!({ "radius": r }),
    };
    json!({
        "rung": p.rung,
        "edges": edges,
        "materials": if p.materials == Materials::Full { "full" } else { "basic" },
        "variation": p.variation,
        "bounces": bounces,
        "sunDisc": f64::from(sun_disc_deg),
        "clamp": p.clamp,
        "adaptive": p.adaptive,
        "noiseTarget": p.noise_target,
        "timeBudget": p.time_budget,
        "roughnessGuide": p.roughness_guide,
        "denoise": denoise_name(p.denoise),
    })
}

/// The reply keys of a path-traced still: why and how it stopped, the samples taken, the clamp's
/// loss, the denoiser that ran and the compute device.
pub(crate) fn path_keys(st: &PathStats, compute: Option<&ComputeGpu>) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("stop".into(), json!(st.stop.name()));
    m.insert("noise".into(), json!(st.noise));
    m.insert("sppMean".into(), json!(st.spp_mean));
    m.insert("sppMin".into(), json!(st.spp_min));
    m.insert("sppMax".into(), json!(st.spp_max));
    m.insert("samplesTotal".into(), json!(st.samples_total));
    m.insert("seconds".into(), json!(st.seconds));
    m.insert("clampLoss".into(), json!(st.clamp_loss));
    m.insert("denoiser".into(), json!(denoise_name(st.denoiser)));
    if let Some(why) = &st.denoise_fallback {
        m.insert("denoiseFallback".into(), json!(why));
    }
    m.insert("denoiseSeconds".into(), json!(st.denoise_seconds));
    m.insert("computeAdapter".into(), compute.map_or(Value::Null, |c| json!({ "name": c.info.name, "type": c.info.adapter_type })));
    m
}

/// The reply keys of a still that fell back to raster when the learned denoiser was asked for.
pub(crate) fn raster_fallback_keys(asked: DenoiseMode) -> Map<String, Value> {
    let mut m = Map::new();
    if asked == DenoiseMode::Learned {
        m.insert("denoiser".into(), json!("none"));
        m.insert("denoiseFallback".into(), json!("no compute shaders"));
    }
    m
}

/// The PFM bytes of a path still's kept linear image, if it kept one of the right size.
pub(crate) fn pfm_bytes(st: &PathStats, size: (u32, u32)) -> Option<Vec<u8>> {
    let lin = st.linear.as_ref()?;
    let want = (size.0 as usize).checked_mul(size.1 as usize)?;
    (lin.len() == want).then(|| encode_pfm(size.0, size.1, lin))
}

/// The `pfm` and `pfmStage` reply keys: the path written (or null) and the stage asked for.
pub(crate) fn pfm_keys(written: Option<&std::path::Path>, stage: LinearStageParam) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("pfm".into(), written.map_or(Value::Null, |p| json!(p.display().to_string())));
    m.insert("pfmStage".into(), written.map_or(Value::Null, |_| json!(stage_name(stage))));
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_arch3d::calibrate::{CalibLight, CalibObject, Calibration};
    use cadcraft_engine::doc::arch::{DenoiseMode, EdgeMode, Lens, LensProjection, Materials, PhotoSettings};

    #[test]
    fn calibrate_key_is_deterministic_and_distinct() {
        let mut c1 = Calibration::default();
        c1.objects.push(CalibObject::default());
        let mut c2 = Calibration::default();
        c2.objects.push(CalibObject::default());
        assert_eq!(calibrate_key(&c1), calibrate_key(&c2), "identical calibrations must have the same key");

        let mut c3 = Calibration::default();
        c3.objects.push(CalibObject { segments: 64, ..CalibObject::default() });
        assert_ne!(calibrate_key(&c1), calibrate_key(&c3), "different calibrations must have different keys");

        // Key must have the top bit set (distinguish from drawing revisions).
        assert!(calibrate_key(&c1) >> 63 == 1);
    }

    #[test]
    fn keeps_spp_for_rung_and_adaptive() {
        let mut p = PhotoSettings::default();
        assert!(!keeps_spp(&p), "default render does not keep spp");
        p.rung = Some(3);
        assert!(keeps_spp(&p), "rung render keeps spp");
        p.rung = None;
        p.adaptive = true;
        assert!(keeps_spp(&p), "adaptive render keeps spp");
    }

    #[test]
    fn photo_value_echoes_all_settings() {
        let mut p = PhotoSettings::default();
        p.rung = Some(7);
        p.edges = EdgeMode::Radius(0.005);
        p.materials = Materials::Full;
        p.variation = true;
        p.clamp = Some(10.0);
        p.adaptive = true;
        p.noise_target = Some(0.01);
        p.time_budget = Some(30.0);
        p.roughness_guide = true;
        p.denoise = DenoiseMode::Learned;

        let v = photo_value(&p, 8, 0.5);
        assert_eq!(v["rung"], json!(7));
        assert_eq!(v["edges"], json!({ "radius": 0.005 }));
        assert_eq!(v["materials"], "full");
        assert_eq!(v["variation"], true);
        assert_eq!(v["bounces"], 8);
        assert_eq!(v["sunDisc"], json!(0.5));
        assert_eq!(v["clamp"], json!(10.0));
        assert_eq!(v["adaptive"], true);
        assert_eq!(v["noiseTarget"], json!(0.01));
        assert_eq!(v["timeBudget"], json!(30.0));
        assert_eq!(v["roughnessGuide"], true);
        assert_eq!(v["denoise"], "learned");
    }

    #[test]
    fn raster_fallback_keys_only_for_learned() {
        let m = raster_fallback_keys(DenoiseMode::Learned);
        assert_eq!(m["denoiser"], "none");
        assert_eq!(m["denoiseFallback"], "no compute shaders");

        let m2 = raster_fallback_keys(DenoiseMode::Filter);
        assert!(m2.is_empty(), "no fallback keys when filter was asked: {m2:?}");

        let m3 = raster_fallback_keys(DenoiseMode::None);
        assert!(m3.is_empty(), "no fallback keys when none was asked: {m3:?}");
    }

    #[test]
    fn pfm_bytes_requires_correct_size() {
        let size = (10u32, 5u32);
        let mut st = PathStats::default();
        st.linear = Some(vec![1.0; 10 * 5 * 3]);
        assert!(pfm_bytes(&st, size).is_some(), "correct size must produce PFM bytes");

        st.linear = Some(vec![1.0; 100]); // wrong size
        assert!(pfm_bytes(&st, size).is_none(), "wrong size must not produce PFM bytes");

        st.linear = None;
        assert!(pfm_bytes(&st, size).is_none(), "no linear image must not produce PFM bytes");
    }

    #[test]
    fn pfm_keys_written_and_null() {
        let path = std::path::Path::new("/tmp/test.pfm");
        let m = pfm_keys(Some(path), LinearStageParam::Radiance);
        assert_eq!(m["pfm"], json!("/tmp/test.pfm"));
        assert_eq!(m["pfmStage"], "radiance");

        let m2 = pfm_keys(None, LinearStageParam::Film);
        assert!(m2["pfm"].is_null());
        assert!(m2["pfmStage"].is_null());
    }

    #[test]
    fn lens_view_neutral_and_non_neutral() {
        let mut lighting = Lighting::default();
        let v = lens_view(&lighting);
        assert!(v.is_neutral(), "default lighting must produce neutral lens view");

        lighting.lens.panini = true;
        lighting.lens.compression = 0.8;
        lighting.lens.shift = 0.1;
        let v2 = lens_view(&lighting);
        assert!(!v2.is_neutral(), "non-default lens must produce non-neutral lens view");
        assert_eq!(v2.panini, true);
        assert_eq!(v2.compression, 0.8);
        assert_eq!(v2.shift, 0.1);
    }

    #[test]
    fn stage_name_matches_stage_of() {
        assert_eq!(stage_name(LinearStageParam::Radiance), "radiance");
        assert_eq!(stage_name(LinearStageParam::Film), "film");
        assert_eq!(stage_of(LinearStageParam::Radiance), LinearStage::Radiance);
        assert_eq!(stage_of(LinearStageParam::Film), LinearStage::Film);
    }
}
