//! The photoreal params of `render` (phase 22): the rung preset and the explicit keys over it,
//! the lens/film/sun-disc look hand-over, the focus pick, `pfm` and `calibrate`. `render.rs`
//! only calls [`parse_photo`], [`parse_pfm`] and [`parse_calibrate`]. Everything invalid is
//! refused, never clamped.

use std::path::PathBuf;

use cadcraft_arch3d::Scene3d;
use cadcraft_arch3d::calibrate::{self, CalibLight, CalibMaterial, CalibObject, CalibShape, Calibration};
use cadcraft_arch3d::camera::Camera;
use cadcraft_arch3d::lens::{LensView, camera_basis, ray_at_lens};
use cadcraft_arch3d::pick::Bvh;
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::photo::{self, DenoiseMode, DenoiseModel, EdgeMode, Materials, PhotoSettings};
use cadcraft_doc::arch::{Lens, LensProjection, Look, Technique};
use serde_json::{Map, Value};

use super::look::focus_pick;
use super::render::{LinearStageParam, PathParams};
use crate::EngineError;
use crate::cmd::bad;

const CMD: &str = "render";

fn err(msg: impl Into<String>) -> EngineError {
    bad(CMD, msg.into())
}

fn boolean(key: &str, v: &Value) -> Result<bool, EngineError> {
    v.as_bool().ok_or_else(|| err(format!("`{key}` must be true or false, not {v}")))
}

fn number(key: &str, v: &Value, lo: f64, hi: f64) -> Result<f64, EngineError> {
    v.as_f64().filter(|x| x.is_finite() && *x >= lo && *x <= hi).ok_or_else(|| err(format!("`{key}` must be a number in {lo}..={hi}, not {v}")))
}

fn check_keys(what: &str, obj: &Map<String, Value>, allowed: &[&str]) -> Result<(), EngineError> {
    match obj.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(k) => Err(err(format!("unknown key `{k}` in {what}"))),
        None => Ok(()),
    }
}

/// Read the photoreal keys of a `render` into settings, adjusting `look` and `path` to match.
/// Returns the settings and whether the final must be path-traced (a `rung`, or technique
/// photoreal unless `final` names raster, i.e. `final_given`).
pub(crate) fn parse_photo(
    params: &Value,
    look: &mut Look,
    doc: &Drawing,
    camera: Option<&Camera>,
    size: (u32, u32),
    path: &mut PathParams,
    final_given: bool,
) -> Result<(PhotoSettings, bool), EngineError> {
    // (1) the base.
    let (mut photo, force) = match params.get("rung") {
        Some(r) => {
            let n = r.as_u64().filter(|n| *n <= 7).ok_or_else(|| err(format!("`rung` must be a whole number in 0..=7, not {r}")))?;
            (photo::rung(n as u8), true)
        }
        None if look.technique == Technique::Photoreal => (photo::rung(photo::PHOTOREAL_RUNG), !final_given),
        None => (PhotoSettings::default(), false),
    };
    let is_rung = photo.rung.is_some();

    // (2) explicit keys.
    if let Some(v) = params.get("edges") {
        photo.edges = match v {
            Value::Bool(false) => EdgeMode::Off,
            Value::Bool(true) => EdgeMode::PerMaterial,
            Value::Object(o) => {
                check_keys("`edges`", o, &["radius"])?;
                let r = o.get("radius").ok_or_else(|| err("`edges` object needs `radius`"))?;
                EdgeMode::Radius(number("edges.radius", r, 0.0, 0.05)?)
            }
            _ => return Err(err(format!("`edges` must be false, true or an object with radius 0..=0.05, not {v}"))),
        };
    }
    if let Some(v) = params.get("materials") {
        photo.materials = match v.as_str() {
            Some("basic") => Materials::Basic,
            Some("full") => Materials::Full,
            _ => return Err(err(format!("`materials` must be \"basic\" or \"full\", not {v}"))),
        };
    }
    if let Some(v) = params.get("variation") {
        photo.variation = boolean("variation", v)?;
    }
    if let Some(v) = params.get("clamp") {
        photo.clamp = if v.is_null() {
            None
        } else {
            let c = number("clamp", v, 0.0, 1e6)?;
            if c <= 0.0 {
                return Err(err(format!("`clamp` must be null or a number in (0, 1e6], not {v}")));
            }
            Some(c)
        };
    }
    if let Some(v) = params.get("adaptive") {
        photo.adaptive = boolean("adaptive", v)?;
    }
    if let Some(v) = params.get("noiseTarget") {
        photo.noise_target = Some(number("noiseTarget", v, 0.001, 1.0)?);
    }
    if let Some(v) = params.get("timeBudget") {
        photo.time_budget = Some(number("timeBudget", v, 1.0, 86400.0)?);
    }
    if let Some(v) = params.get("denoise") {
        photo.denoise = match v {
            Value::Bool(false) => DenoiseMode::None,
            Value::Bool(true) => DenoiseMode::Filter,
            Value::String(s) if s == "filter" => DenoiseMode::Filter,
            Value::String(s) if s == "learned" => DenoiseMode::Learned,
            _ => return Err(err(format!("`denoise` must be false, true, \"filter\" or \"learned\", not {v}"))),
        };
        if photo.denoise == DenoiseMode::None {
            path.denoise = false;
        }
    }
    if let Some(v) = params.get("denoiseModel") {
        photo.denoise_model = match v.as_str() {
            Some("base") => DenoiseModel::Base,
            Some("small") => DenoiseModel::Small,
            _ => return Err(err(format!("`denoiseModel` must be \"base\" or \"small\", not {v}"))),
        };
    }
    if let Some(v) = params.get("roughnessGuide") {
        photo.roughness_guide = boolean("roughnessGuide", v)?;
    }
    // bounces and spp: given wins, else the base's.
    if is_rung {
        if params.get("bounces").is_none() {
            path.bounces = photo.bounces;
        }
        if params.get("spp").is_none() {
            path.spp = photo.spp;
        }
        path.spp_given = true;
    }
    photo.bounces = path.bounces;
    photo.spp = path.spp;

    // (3) the look.
    if !photo.lens {
        look.lens = Lens::default();
    }
    if let Some(f) = photo.film.clone() {
        if look.film.is_neutral() {
            look.film = f;
        } else {
            photo.film = None;
        }
    }
    if let Some(d) = photo.sun_disc {
        if !(look.sun.disc > 0.0) {
            look.sun.disc = d;
        }
    }

    // (4) the focus pick.
    if let Some(pixel) = params.get("focus").map(focus_pick).transpose()?.flatten() {
        let calibration = parse_calibrate(params)?;
        let cam = camera.ok_or_else(|| err("`focus: {pick}` needs a camera"))?;
        look.lens.focus = Some(resolve_focus_pick(pixel, cam, look, doc, size, calibration.as_ref())?);
    }
    Ok((photo, force))
}

/// The distance along the camera's forward axis to the surface under `pixel` (of a `size`
/// image), seen through the look's lens, in the drawing's scene or the calibration scene.
/// Nothing under the pixel, or a pixel outside the image, is refused.
pub(crate) fn resolve_focus_pick(
    pixel: [f64; 2],
    camera: &Camera,
    look: &Look,
    doc: &Drawing,
    size: (u32, u32),
    calibration: Option<&Calibration>,
) -> Result<f64, EngineError> {
    let (w, h) = (f64::from(size.0), f64::from(size.1));
    if !(pixel[0].is_finite() && pixel[1].is_finite()) || pixel[0] < 0.0 || pixel[1] < 0.0 || pixel[0] > w || pixel[1] > h {
        return Err(err(format!("`focus.pick` must be a pixel inside the {}x{} image", size.0, size.1)));
    }
    let lens = LensView { panini: look.lens.projection == LensProjection::Panini, compression: look.lens.compression, shift: look.lens.shift };
    let (origin, dir) =
        ray_at_lens(camera, &lens, size, [pixel[0] as f32, pixel[1] as f32]).ok_or_else(|| err("`focus.pick` has no ray through that pixel"))?;
    let scene = match calibration {
        Some(c) => calibrate::scene(c),
        None => Scene3d::build(&doc.arch),
    };
    let bvh = Bvh::build(&scene);
    let hit = scene.pick_with(&bvh, origin, dir).ok_or_else(|| err("`focus.pick`: nothing is under that pixel"))?;
    let (_, _, f) = camera_basis(camera);
    let d = (hit.point.x - origin.x) * f[0] + (hit.point.y - origin.y) * f[1] + (hit.point.z - origin.z) * f[2];
    if d.is_finite() && d > 0.0 && d <= 10_000.0 { Ok(d) } else { Err(err("`focus.pick`: the surface under that pixel is out of focus range")) }
}

/// `pfm` and `pfmStage`.
pub(crate) fn parse_pfm(params: &Value) -> Result<Option<(PathBuf, LinearStageParam)>, EngineError> {
    let stage = match params.get("pfmStage") {
        None => LinearStageParam::Radiance,
        Some(v) => match v.as_str() {
            Some("radiance") => LinearStageParam::Radiance,
            Some("film") => LinearStageParam::Film,
            _ => return Err(err(format!("`pfmStage` must be \"radiance\" or \"film\", not {v}"))),
        },
    };
    match params.get("pfm") {
        None if params.get("pfmStage").is_some() => Err(err("`pfmStage` needs `pfm`")),
        None => Ok(None),
        Some(v) => match v.as_str() {
            Some(s) if !s.trim().is_empty() && s.len() <= 4096 && !s.contains('\0') => Ok(Some((PathBuf::from(s), stage))),
            _ => Err(err("`pfm` must be a non-empty path string")),
        },
    }
}

fn vec3(key: &str, v: &Value) -> Result<[f64; 3], EngineError> {
    let a = v.as_array().filter(|a| a.len() == 3).ok_or_else(|| err(format!("`{key}` must be [x, y, z], not {v}")))?;
    let mut out = [0.0; 3];
    for (o, x) in out.iter_mut().zip(a) {
        *o = x.as_f64().filter(|x| x.is_finite()).ok_or_else(|| err(format!("`{key}` must hold finite numbers, not {v}")))?;
    }
    Ok(out)
}

fn finite(key: &str, v: &Value) -> Result<f64, EngineError> {
    v.as_f64().filter(|x| x.is_finite()).ok_or_else(|| err(format!("`{key}` must be a finite number, not {v}")))
}

fn parse_material(v: &Value) -> Result<CalibMaterial, EngineError> {
    let o = v.as_object().ok_or_else(|| err("`material` must be an object"))?;
    check_keys(
        "`material`",
        o,
        &[
            "color",
            "metallic",
            "roughness",
            "emission",
            "clearcoat",
            "clearcoatRoughness",
            "anisotropy",
            "anisotropyRotation",
            "sheen",
            "sheenRoughness",
            "transmission",
            "thin",
            "ior",
            "thickness",
            "edgeRadius",
            "variation",
        ],
    )?;
    let mut m = CalibMaterial::default();
    let f = |key: &str| -> Result<Option<f32>, EngineError> { o.get(key).map(|v| finite(key, v).map(|x| x as f32)).transpose() };
    if let Some(c) = o.get("color") {
        let c = vec3("color", c)?;
        m.color = [c[0] as f32, c[1] as f32, c[2] as f32];
    }
    if let Some(x) = f("metallic")? {
        m.metallic = x;
    }
    if let Some(x) = f("roughness")? {
        m.roughness = x;
    }
    if let Some(x) = f("emission")? {
        m.emission = x;
    }
    if let Some(x) = f("clearcoat")? {
        m.ext.clearcoat = x;
    }
    if let Some(x) = f("clearcoatRoughness")? {
        m.ext.clearcoat_roughness = x;
    }
    if let Some(x) = f("anisotropy")? {
        m.ext.anisotropy = x;
    }
    if let Some(x) = f("anisotropyRotation")? {
        m.ext.anisotropy_rotation = x.to_radians();
    }
    if let Some(x) = f("sheen")? {
        m.ext.sheen = [x; 3];
    }
    if let Some(x) = f("sheenRoughness")? {
        m.ext.sheen_roughness = x;
    }
    if let Some(x) = f("transmission")? {
        m.ext.transmission = x;
    }
    if let Some(t) = o.get("thin") {
        m.ext.thin = boolean("thin", t)?;
    }
    if let Some(x) = f("ior")? {
        m.ext.ior = x;
    }
    if let Some(x) = f("thickness")? {
        m.ext.thickness = x;
    }
    if let Some(x) = f("edgeRadius")? {
        m.ext.edge_radius = Some(x);
    }
    if let Some(var) = o.get("variation") {
        let vo = var.as_object().ok_or_else(|| err("`variation` must be an object with scale and amount"))?;
        check_keys("`variation`", vo, &["scale", "amount"])?;
        if let Some(s) = vo.get("scale") {
            m.ext.variation_scale = finite("variation.scale", s)? as f32;
        }
        if let Some(a) = vo.get("amount") {
            m.ext.variation_amount = finite("variation.amount", a)? as f32;
        }
    }
    Ok(m)
}

fn parse_object(v: &Value) -> Result<CalibObject, EngineError> {
    let o = v.as_object().ok_or_else(|| err("a calibrate object must be an object"))?;
    check_keys("a calibrate object", o, &["shape", "at", "size", "segments", "material"])?;
    let mut obj = CalibObject {
        shape: match o.get("shape").and_then(Value::as_str) {
            Some("sphere") => CalibShape::Sphere,
            Some("box") => CalibShape::Box,
            _ => return Err(err("`shape` must be \"sphere\" or \"box\"")),
        },
        ..CalibObject::default()
    };
    if let Some(at) = o.get("at") {
        obj.at = vec3("at", at)?;
    }
    match o.get("size") {
        None => {}
        Some(s @ Value::Array(_)) => obj.size = vec3("size", s)?,
        Some(s) => obj.size = [finite("size", s)?; 3],
    }
    if let Some(s) = o.get("segments") {
        obj.segments = s.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| err(format!("`segments` must be a whole number, not {s}")))?;
    }
    if let Some(m) = o.get("material") {
        obj.material = parse_material(m)?;
    }
    Ok(obj)
}

fn parse_light(v: &Value) -> Result<CalibLight, EngineError> {
    let o = v.as_object().ok_or_else(|| err("a calibrate light must be an object"))?;
    check_keys("a calibrate light", o, &["at", "candela", "kelvin"])?;
    let mut l = CalibLight::default();
    if let Some(at) = o.get("at") {
        l.at = vec3("at", at)?;
    }
    if let Some(c) = o.get("candela") {
        l.candela = finite("candela", c)?;
    }
    if let Some(k) = o.get("kelvin") {
        l.kelvin = finite("kelvin", k)?;
    }
    Ok(l)
}

/// `calibrate`: the objects and lights that replace the model, validated against the caps.
pub(crate) fn parse_calibrate(params: &Value) -> Result<Option<Calibration>, EngineError> {
    let Some(v) = params.get("calibrate") else { return Ok(None) };
    let o = v.as_object().ok_or_else(|| err("`calibrate` must be an object with objects and lights"))?;
    check_keys("`calibrate`", o, &["objects", "lights"])?;
    let objs = o.get("objects").and_then(Value::as_array).ok_or_else(|| err("`calibrate.objects` must be an array"))?;
    if objs.is_empty() || objs.len() > calibrate::MAX_OBJECTS {
        return Err(err(format!("`calibrate.objects` needs 1..={} entries, got {}", calibrate::MAX_OBJECTS, objs.len())));
    }
    let lights: &[Value] = match o.get("lights") {
        None => &[],
        Some(l) => l.as_array().ok_or_else(|| err("`calibrate.lights` must be an array"))?,
    };
    if lights.len() > calibrate::MAX_LIGHTS {
        return Err(err(format!("`calibrate.lights` takes at most {} entries, got {}", calibrate::MAX_LIGHTS, lights.len())));
    }
    let c = Calibration {
        objects: objs.iter().map(parse_object).collect::<Result<_, _>>()?,
        lights: lights.iter().map(parse_light).collect::<Result<_, _>>()?,
    };
    calibrate::validate(&c).map_err(|e| err(format!("`calibrate`: {e}")))?;
    Ok(Some(c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_arch3d::camera::FullCamera;
    use cadcraft_doc::arch::Film;
    use cadcraft_doc::arch::photo::PHOTOREAL_RUNG;
    use cadcraft_geom::Vec3;
    use serde_json::json;

    use crate::cmd::arch::render::{FinalMode, build_request};

    fn parse(p: Value) -> Result<(PhotoSettings, bool, Look, PathParams), EngineError> {
        let d = Drawing::new_metric();
        let mut look = Look::default();
        if p.get("technique").and_then(Value::as_str) == Some("photoreal") {
            look.technique = Technique::Photoreal;
        }
        let mut path = PathParams::default();
        let (ph, force) = parse_photo(&p, &mut look, &d, None, (100, 100), &mut path, p.get("final").is_some())?;
        Ok((ph, force, look, path))
    }

    #[test]
    fn plain_render_has_default_photo_settings() {
        let d = Drawing::new_metric();
        let r = build_request(&d, &json!({})).unwrap();
        assert_eq!(r.photo, PhotoSettings::default());
        assert!(r.linear.is_none() && r.calibrate.is_none());
        let (ph, force, look, path) = parse(json!({})).unwrap();
        assert_eq!((ph, force), (PhotoSettings::default(), false));
        assert_eq!(look, Look::default());
        assert_eq!(path, PathParams::default());
    }

    #[test]
    fn technique_photoreal_is_rung_seven_and_path_final() {
        let d = Drawing::new_metric();
        let r = build_request(&d, &json!({"technique": "photoreal"})).unwrap();
        assert_eq!(r.photo.rung, Some(PHOTOREAL_RUNG));
        assert!(r.final_render && r.final_mode == FinalMode::Path);
        assert_eq!(r.path.spp, 4096);
        assert!(r.path.spp_given);
        assert_eq!(r.path.bounces, 12);
        let r = build_request(&d, &json!({"technique": "photoreal", "final": "raster"})).unwrap();
        assert_eq!(r.final_mode, FinalMode::Raster);
        let (_, force, _, _) = parse(json!({"technique": "photoreal", "final": "raster"})).unwrap();
        assert!(!force);
    }

    #[test]
    fn explicit_keys_override_the_preset() {
        let p = json!({
            "rung": 7, "edges": {"radius": 0.01}, "materials": "basic", "variation": false, "clamp": null, "adaptive": false,
            "noiseTarget": 0.1, "timeBudget": 30, "denoise": false, "denoiseModel": "small", "roughnessGuide": false,
            "bounces": 3, "spp": 64
        });
        let d = Drawing::new_metric();
        let r = build_request(&d, &p).unwrap();
        let (ph, path) = (r.photo, r.path);
        assert!(r.final_render && r.final_mode == FinalMode::Path);
        assert_eq!(ph.edges, EdgeMode::Radius(0.01));
        assert_eq!(ph.materials, Materials::Basic);
        assert!(!ph.variation && !ph.adaptive && !ph.roughness_guide);
        assert_eq!(ph.clamp, None);
        assert_eq!((ph.noise_target, ph.time_budget), (Some(0.1), Some(30.0)));
        assert_eq!((ph.denoise, ph.denoise_model), (DenoiseMode::None, DenoiseModel::Small));
        assert_eq!((path.bounces, path.spp, path.spp_given, path.denoise), (3, 64, true, false));
        // noiseTarget without adaptive keeps uniform sampling.
        let (ph, ..) = parse(json!({"noiseTarget": 0.05})).unwrap();
        assert!(!ph.adaptive && ph.noise_target == Some(0.05));
        let (ph, ..) = parse(json!({"denoise": "learned", "edges": true})).unwrap();
        assert_eq!((ph.denoise, ph.edges), (DenoiseMode::Learned, EdgeMode::PerMaterial));
    }

    #[test]
    fn rung_zero_neutralises_the_lens_and_film() {
        let d = Drawing::new_metric();
        let mut look = Look::default();
        look.lens.shift = 0.25;
        look.lens.aperture = Some(2.8);
        let mut path = PathParams::default();
        let (ph, force) = parse_photo(&json!({"rung": 0}), &mut look, &d, None, (10, 10), &mut path, false).unwrap();
        assert!(force && ph.rung == Some(0));
        assert!(look.lens.is_neutral() && look.film.is_neutral());
        assert_eq!(path.spp, 256);
        let (ph, _, look, _) = parse(json!({"rung": 5})).unwrap();
        assert_eq!(ph.film, Some(Film::photoreal()));
        assert_eq!(look.film, Film::photoreal());
        let (_, _, look, _) = parse(json!({"rung": 4})).unwrap();
        assert_eq!(look.sun.disc, 0.53);
    }

    #[test]
    fn camera_film_wins_over_the_preset_film() {
        let d = Drawing::new_metric();
        let mut look = Look::default();
        look.film.vignette = 0.5;
        look.sun.disc = 1.0;
        let mut path = PathParams::default();
        let (ph, _) = parse_photo(&json!({"rung": 7}), &mut look, &d, None, (10, 10), &mut path, false).unwrap();
        assert_eq!(ph.film, None);
        assert_eq!(look.film.vignette, 0.5);
        assert_eq!(look.sun.disc, 1.0);
    }

    fn sphere_params(extra: Value) -> Value {
        let mut p = json!({"calibrate": {"objects": [{"shape": "sphere", "at": [0, 5, 0], "size": 2}]}});
        if let (Some(o), Some(e)) = (p.as_object_mut(), extra.as_object()) {
            o.extend(e.clone());
        }
        p
    }

    #[test]
    fn focus_pick_resolves_the_axial_distance() {
        let d = Drawing::new_metric();
        let cam = Camera::Full(FullCamera::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 5.0, 0.0), 40.0));
        let mut look = Look::default();
        let mut path = PathParams::default();
        let p = sphere_params(json!({"focus": {"pick": [50, 50]}}));
        parse_photo(&p, &mut look, &d, Some(&cam), (100, 100), &mut path, false).unwrap();
        let f = look.lens.focus.unwrap_or(0.0);
        assert!((f - 4.0).abs() < 0.01, "{f}");
        // Off the sphere, outside the image, and without a camera are refused.
        for (pix, c) in [(json!([2, 2]), Some(&cam)), (json!([500, 5]), Some(&cam)), (json!([50, 50]), None)] {
            let p = sphere_params(json!({"focus": {"pick": pix}}));
            let mut l = Look::default();
            assert!(parse_photo(&p, &mut l, &d, c, (100, 100), &mut path, false).is_err());
        }
    }

    #[test]
    fn hostile_photo_params_are_refused() {
        for p in [
            json!({"rung": 8}),
            json!({"rung": 1.5}),
            json!({"rung": -1}),
            json!({"edges": 1}),
            json!({"edges": {"radius": 0.06}}),
            json!({"edges": {"radius": 0.01, "x": 1}}),
            json!({"materials": "pbr"}),
            json!({"variation": "yes"}),
            json!({"clamp": 0}),
            json!({"clamp": 1e7}),
            json!({"clamp": "a"}),
            json!({"adaptive": 1}),
            json!({"noiseTarget": 0.0001}),
            json!({"noiseTarget": 2}),
            json!({"timeBudget": 0}),
            json!({"timeBudget": 90000}),
            json!({"denoise": "ai"}),
            json!({"denoiseModel": "big"}),
            json!({"roughnessGuide": 0}),
            json!({"focus": {"pick": [1]}}),
            json!({"focus": {"pick": [-1, 2]}}),
            json!({"focus": {"nope": 1}}),
        ] {
            assert!(parse(p.clone()).is_err(), "{p}");
        }
        for p in [json!({"pfm": 3}), json!({"pfm": ""}), json!({"pfm": "a.pfm", "pfmStage": "x"}), json!({"pfmStage": "film"})] {
            assert!(parse_pfm(&p).is_err(), "{p}");
        }
        assert_eq!(parse_pfm(&json!({"pfm": "a.pfm", "pfmStage": "film"})).unwrap(), Some((PathBuf::from("a.pfm"), LinearStageParam::Film)));
        assert_eq!(parse_pfm(&json!({"pfm": "a.pfm"})).unwrap().map(|x| x.1), Some(LinearStageParam::Radiance));
    }

    #[test]
    fn calibrate_parses_and_caps() {
        let c = parse_calibrate(&json!({"calibrate": {
            "objects": [{"shape": "box", "at": [1, 2, 3], "size": [1, 2, 3], "material": {
                "color": [1, 0.5, 0.25], "metallic": 1, "roughness": 0.2, "emission": 10, "clearcoat": 1, "clearcoatRoughness": 0.1,
                "anisotropy": 0.5, "anisotropyRotation": 90, "sheen": 0.5, "sheenRoughness": 0.3, "transmission": 0.5, "thin": true,
                "ior": 1.4, "thickness": 0.1, "edgeRadius": 0.01, "variation": {"scale": 2, "amount": 0.5}}}],
            "lights": [{"at": [0, 0, 3], "candela": 50, "kelvin": 3000}]
        }}))
        .unwrap()
        .unwrap();
        let o = &c.objects[0];
        assert_eq!((o.shape, o.size), (CalibShape::Box, [1.0, 2.0, 3.0]));
        assert_eq!(o.material.color, [1.0, 0.5, 0.25]);
        assert!((o.material.ext.anisotropy_rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert_eq!(o.material.ext.sheen, [0.5; 3]);
        assert!(o.material.ext.thin && o.material.ext.edge_radius == Some(0.01));
        assert_eq!((o.material.ext.variation_scale, o.material.ext.variation_amount), (2.0, 0.5));
        assert_eq!(c.lights[0].kelvin, 3000.0);
        assert_eq!(parse_calibrate(&json!({})).unwrap(), None);
        let obj = json!({"shape": "sphere"});
        let many = |n: usize| json!({"calibrate": {"objects": vec![obj.clone(); n]}});
        assert!(parse_calibrate(&many(16)).is_ok());
        assert!(parse_calibrate(&many(17)).is_err());
        assert!(parse_calibrate(&many(0)).is_err());
        let lights = |n: usize| json!({"calibrate": {"objects": [obj.clone()], "lights": vec![json!({"candela": 1}); n]}});
        assert!(parse_calibrate(&lights(8)).is_ok());
        assert!(parse_calibrate(&lights(9)).is_err());
        for bad_c in [
            json!({"calibrate": {"objects": [{"shape": "cone"}]}}),
            json!({"calibrate": {"objects": [{"shape": "box", "size": 0}]}}),
            json!({"calibrate": {"objects": [{"shape": "box", "size": 5000}]}}),
            json!({"calibrate": {"objects": [{"shape": "sphere", "segments": 4}]}}),
            json!({"calibrate": {"objects": [{"shape": "sphere", "at": [1e9, 0, 0]}]}}),
            json!({"calibrate": {"objects": [{"shape": "sphere", "bogus": 1}]}}),
            json!({"calibrate": {"objects": [{"shape": "sphere", "material": {"bogus": 1}}]}}),
            json!({"calibrate": {"objects": [{"shape": "sphere", "material": {"roughness": 2}}]}}),
            json!({"calibrate": {"objects": [{"shape": "sphere", "material": {"color": [1, 1]}}]}}),
            json!({"calibrate": {"objects": [{"shape": "sphere"}], "lights": [{"candela": 0}]}}),
            json!({"calibrate": {"objects": [{"shape": "sphere"}], "extra": 1}}),
            json!({"calibrate": 5}),
        ] {
            assert!(parse_calibrate(&bad_c).is_err(), "{bad_c}");
        }
    }
}
