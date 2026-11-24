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
