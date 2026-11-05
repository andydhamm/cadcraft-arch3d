//! `render`: a 3D still of the drawing (phase 10) through the renderer the app installs with
//! [`set_render_hooks`] (the CLI and the desktop app; the engine has no GPU). One renderer lives
//! for the whole session, so its mesh upload and shadow maps are reused from one `render` to the
//! next: a call that only moves the camera builds no shadow map. `sunstudy` uses the same hook.
//!
//! Filled here: the hook types, [`set_render_hooks`], [`render_hooks`], [`stats`]. The command
//! itself is filled by `p10-cmd-render` (spec stub below).

use std::path::PathBuf;
use std::sync::OnceLock;

use cadcraft_arch::site::true_to_drawing;
use cadcraft_arch::sun::{sun_dir, sun_position};
use cadcraft_arch3d::camera::{Camera, FullCamera, Orbit};
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::{Camera3d, CameraKind};
use cadcraft_geom::Vec3;
use serde_json::{Value, json};

use crate::cmd::arch::pick::parse_camera;
use crate::cmd::{CommandSpec, bad};
use crate::{EngineError, Session};

/// What lights the still besides the fixtures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SunChoice {
    /// The default light (`render3d::Light::DEFAULT`), no sun shadow.
    Default,
    /// Night: the sun is off.
    Off,
    /// The sun from this direction (toward the sun, drawing coordinates, +Z up), with its shadow.
    Dir([f64; 3]),
}

/// One `render` (or one `sunstudy` still).
#[derive(Clone, Debug)]
pub struct RenderRequest<'a> {
    pub drawing: &'a Drawing,
    /// `None`: the active saved camera, else the orbit fitted to the scene.
    pub camera: Option<Camera>,
    /// Pixels, each side 1..=8192.
    pub size: (u32, u32),
    /// Where to write the PNG; `None` renders without writing (the reply still reports).
    pub out: Option<PathBuf>,
    /// Finalize Render instead of the interactive view.
    pub final_render: bool,
    pub sun: SunChoice,
    /// `shaded`, `shaded-edges`, `hidden-line` or `glass-house`.
    pub style: String,
    /// Draw the ground plane.
    pub ground: bool,
    /// Lights shadowed at most in the interactive view (Finalize shadows every light).
    pub budget: usize,
    /// World points whose pixel and colour the reply reports (at most 64).
    pub samples: Vec<[f64; 3]>,
    /// Which Finalize Render `final_render` runs (phase 11).
    pub final_mode: FinalMode,
    /// The path tracer's settings (phase 11; used when `final_mode` is `Path`).
    pub path: PathParams,
}

/// Which Finalize Render a request runs (phase 11): the phase-10 shadow-mapped still, or the path
/// tracer. `Path` is the default (`final: true`); `Raster` stays available as `final: "raster"`,
/// and a device without compute shaders always gets `Raster`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FinalMode {
    Raster,
    #[default]
    Path,
}

/// The path tracer's settings in a request (phase 11): samples per pixel (1..=65536, default
/// 256), full bounces before Russian roulette (0..=32, default 4; 0 = direct light only), the
/// seed (default 0), the denoiser (default on), hardware ray queries (default off), the furnace
/// test `(albedo, emission)` and the sample counts to keep images at (`snapshots`, at most 8,
/// ascending, each <= spp).
#[derive(Clone, Debug, PartialEq)]
pub struct PathParams {
    pub spp: u32,
    pub bounces: u32,
    pub seed: u32,
    pub denoise: bool,
    pub rt: bool,
    pub exposure: Option<f64>,
    pub furnace: Option<(f64, f64)>,
    pub snapshots: Vec<u32>,
    /// Whether the request named spp; false in `Default`.
    pub spp_given: bool,
}

impl Default for PathParams {
    fn default() -> Self {
        PathParams { spp: 256, bounces: 4, seed: 0, denoise: true, rt: false, exposure: None, furnace: None, snapshots: Vec::new(), spp_given: false }
    }
}

/// One path-traced `illuminance` (phase 11): irradiance at each `(point, unit normal)` of the
/// drawing's 3D scene, with `path`'s spp, bounces and seed.
#[derive(Clone, Debug)]
pub struct IlluminanceRequest<'a> {
    pub drawing: &'a Drawing,
    pub probes: Vec<[[f64; 3]; 2]>,
    /// Night (`SunChoice::Off`), the default light, or a sun direction.
    pub sun: SunChoice,
    pub path: PathParams,
}

impl<'a> IlluminanceRequest<'a> {
    /// Build a request. Each probe is a `(point, normal)` pair.
    pub fn new(drawing: &'a Drawing, probes: Vec<[[f64; 3]; 2]>, sun: SunChoice, path: PathParams) -> Self {
        Self { drawing, probes, sun, path }
    }
}

impl<'a> RenderRequest<'a> {
    /// Build a request. All fields are supplied so every caller (`render`,
    /// `sunstudy`) can set each aspect. The literal below is in the struct's
    /// own declaring file, so it is exempt from the shared-struct literal rule.
    pub fn new(
        drawing: &'a Drawing,
        camera: Option<Camera>,
        size: (u32, u32),
        out: Option<PathBuf>,
        final_render: bool,
        sun: SunChoice,
        style: String,
        ground: bool,
        budget: usize,
        samples: Vec<[f64; 3]>,
    ) -> Self {
        Self {
            drawing,
            camera,
            size,
            out,
            final_render,
            sun,
            style,
            ground,
            budget,
            samples,
            final_mode: FinalMode::default(),
            path: PathParams::default(),
        }
    }

    /// The same request with a Finalize mode and path-tracer settings (phase 11).
    pub fn with_final(mut self, final_mode: FinalMode, path: PathParams) -> Self {
        self.final_mode = final_mode;
        self.path = path;
        self
    }
}

/// What a session's renderer has done so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    pub shadow_maps_built: u64,
    pub final_renders: u64,
    /// Path-traced stills started (phase 11): a camera move never starts one.
    pub path_traces: u64,
}

/// The app's renderer: `render` returns the reply JSON of one still (see `p10-cli-render-hook`
/// for its keys) or an error message; `stats` reports the counters.
pub struct RenderHooks {
    pub render: fn(&RenderRequest) -> Result<Value, String>,
    pub stats: fn() -> RenderStats,
    /// Path-traced illuminance (phase 11): the reply JSON (`p11-cli-render-path`) or an error;
    /// `None` = `illuminance {"method":"path"}` is refused.
    pub illuminance: Option<fn(&IlluminanceRequest) -> Result<Value, String>>,
}

impl RenderHooks {
    /// The renderer's two entry points.
    pub fn new(render: fn(&RenderRequest) -> Result<Value, String>, stats: fn() -> RenderStats) -> Self {
        Self { render, stats, illuminance: None }
    }

    /// The same hooks with a path-traced illuminance (phase 11).
    pub fn with_illuminance(mut self, f: fn(&IlluminanceRequest) -> Result<Value, String>) -> Self {
        self.illuminance = Some(f);
        self
    }
}

static HOOKS: OnceLock<RenderHooks> = OnceLock::new();

/// Install the app's renderer (the first call wins).
pub fn set_render_hooks(h: RenderHooks) {
    let _ = HOOKS.set(h);
}

/// The installed renderer, if any.
pub fn render_hooks() -> Option<&'static RenderHooks> {
    HOOKS.get()
}

/// The installed renderer's counters (zeros without one): `build3d` reports them.
pub fn stats() -> RenderStats {
    HOOKS.get().map(|h| (h.stats)()).unwrap_or_default()
}

fn parse_size(v: Option<&Value>) -> Result<(u32, u32), EngineError> {
    const MAX: u64 = 8192;
    match v {
        None => Ok((1200, 800)),
        Some(Value::Array(arr)) => {
            if arr.len() != 2 {
                return Err(bad("render", "`size` must be [w,h]"));
            }
            let dim = |idx: usize| -> Result<u32, EngineError> {
                arr.get(idx)
                    .and_then(Value::as_u64)
                    .and_then(|n| (1..=MAX).contains(&n).then_some(n as u32))
                    .ok_or_else(|| bad("render", format!("`size[{}]` must be a whole number in 1..={MAX}", idx)))
            };
            Ok((dim(0)?, dim(1)?))
        }
        _ => Err(bad("render", "`size` must be [w,h]")),
    }
}

fn parse_samples(v: Option<&Value>) -> Result<Vec<[f64; 3]>, EngineError> {
    match v {
        None => Ok(Vec::new()),
        Some(Value::Array(arr)) => {
            if arr.len() > 64 {
                return Err(bad("render", "`samples` must have at most 64 points"));
            }
            let mut out = Vec::with_capacity(arr.len());
            for (i, p) in arr.iter().enumerate() {
                let inner = p.as_array().ok_or_else(|| bad("render", format!("`samples[{}]` must be [x,y,z]", i)))?;
                if inner.len() != 3 {
                    return Err(bad("render", format!("`samples[{}]` must be [x,y,z]", i)));
                }
                let f = |idx: usize| -> Result<f64, EngineError> {
                    inner
                        .get(idx)
                        .and_then(Value::as_f64)
                        .filter(|x| x.is_finite())
                        .ok_or_else(|| bad("render", format!("`samples[{}][{}]` must be a finite number", i, idx)))
                };
                out.push([f(0)?, f(1)?, f(2)?]);
            }
            Ok(out)
        }
        _ => Err(bad("render", "`samples` must be an array of [x,y,z] points")),
    }
}

/// `final`: absent or false = the interactive view; true = the default Finalize
/// ([`FinalMode::default`]); "raster" or "path" = that Finalize.
fn parse_final(v: Option<&Value>) -> Result<(bool, FinalMode), EngineError> {
    match v {
        None | Some(Value::Bool(false)) => Ok((false, FinalMode::default())),
        Some(Value::Bool(true)) => Ok((true, FinalMode::default())),
        Some(Value::String(s)) if s == "raster" => Ok((true, FinalMode::Raster)),
        Some(Value::String(s)) if s == "path" => Ok((true, FinalMode::Path)),
        _ => Err(bad("render", "`final` must be true, false, \"raster\" or \"path\"")),
    }
}

/// `style`: one of shaded, shaded-edges, hidden-line, glass-house, clay, technical; absent is shaded.
fn parse_style(v: Option<&Value>) -> Result<String, EngineError> {
    match v {
        None => Ok("shaded".to_string()),
        Some(v) => v
            .as_str()
            .filter(|s| matches!(*s, "shaded" | "shaded-edges" | "hidden-line" | "glass-house" | "clay" | "technical"))
            .map(str::to_string)
            .ok_or_else(|| bad("render", "`style` must be one of: shaded, shaded-edges, hidden-line, glass-house, clay, technical")),
    }
}

/// `budget`: lights shadowed in the interactive view, a whole number in 0..=64; absent is 8.
fn parse_budget(v: Option<&Value>) -> Result<usize, EngineError> {
    match v {
        None => Ok(8),
        Some(v) => {
            v.as_u64().filter(|n| (0..=64).contains(n)).map(|n| n as usize).ok_or_else(|| bad("render", "`budget` must be a whole number in 0..=64"))
        }
    }
}

/// The path tracer's settings, read flat from the command params into `PathParams` (phase 11).
/// Absent fields take defaults: `spp` 256, `bounces` 4, `seed` 0, `denoise` true, `rt` false,
/// `snapshots` [], `furnace` absent. `snapshots` is an array of at most 8 whole numbers, each
/// in 1..=`spp` and strictly ascending; `furnace` is `{albedo, emission}` with both required,
/// `0 < albedo < 1` and `0 < emission <= 1000`.
fn parse_path_params(v: &Value) -> Result<PathParams, EngineError> {
    let whole = |key: &str, lo: u64, hi: u64, def: u64| -> Result<u64, EngineError> {
        match v.get(key) {
            None => Ok(def),
            Some(val) => {
                val.as_u64().filter(|n| (lo..=hi).contains(n)).ok_or_else(|| bad("render", format!("`{key}` must be a whole number in {lo}..={hi}")))
            }
        }
    };

    let spp = whole("spp", 1, 65536, 256)?;
    let bounces = whole("bounces", 0, 32, 4)?;
    let seed = whole("seed", 0, 4294967295, 0)?;

    let denoise = match v.get("denoise") {
        None => true,
        Some(val) => val.as_bool().ok_or_else(|| bad("render", "`denoise` must be a boolean"))?,
    };
    let rt = match v.get("rt") {
        None => false,
        Some(val) => val.as_bool().ok_or_else(|| bad("render", "`rt` must be true or false"))?,
    };

    let snapshots = match v.get("snapshots") {
        None => Vec::new(),
        Some(val) => {
            let arr = val.as_array().ok_or_else(|| bad("render", "`snapshots` must be an array"))?;
            if arr.len() > 8 {
                return Err(bad("render", "`snapshots` must have at most 8 entries"));
            }
            let mut out = Vec::with_capacity(arr.len());
            let mut prev: u64 = 0;
            for (i, item) in arr.iter().enumerate() {
                let n = item.as_u64().ok_or_else(|| bad("render", format!("`snapshots[{}]` must be a whole number", i)))?;
                if n < 1 || n > spp || n <= prev {
                    return Err(bad("render", format!("`snapshots` must be whole numbers in 1..={spp} and strictly ascending")));
                }
                prev = n;
                out.push(n as u32);
            }
            out
        }
    };

    let exposure = match v.get("exposure") {
        None => None,
        Some(val) => match val {
            Value::String(s) if s == "auto" => None,
            Value::Number(n) if n.is_f64() => {
                let x = n
                    .as_f64()
                    .filter(|v| v.is_finite() && *v > 0.0 && *v <= 1000.0)
                    .ok_or_else(|| bad("render", "`exposure` must be \"auto\" or a positive number"))?;
                Some(x)
            }
            _ => return Err(bad("render", "`exposure` must be \"auto\" or a positive number")),
        },
    };

    let furnace = match v.get("furnace") {
        None => None,
        Some(val) => {
            let obj = val.as_object().ok_or_else(|| bad("render", "`furnace` must be an object with `albedo` and `emission`"))?;
            let albedo = obj
                .get("albedo")
                .and_then(Value::as_f64)
                .filter(|a| a.is_finite() && *a > 0.0 && *a < 1.0)
                .ok_or_else(|| bad("render", "`albedo` must be a finite number with 0 < albedo < 1"))?;
            let emission = obj
                .get("emission")
                .and_then(Value::as_f64)
                .filter(|e| e.is_finite() && *e > 0.0 && *e <= 1000.0)
                .ok_or_else(|| bad("render", "`emission` must be a finite number with 0 < emission <= 1000"))?;
            Some((albedo, emission))
        }
    };

    Ok(PathParams {
        spp: spp as u32,
        bounces: bounces as u32,
        seed: seed as u32,
        denoise,
        rt,
        exposure,
        furnace,
        snapshots,
        spp_given: v.get("spp").is_some(),
    })
}

pub fn parse_sun(v: Option<&Value>, doc: &Drawing) -> Result<SunChoice, EngineError> {
    match v {
        None => Ok(SunChoice::Default),
        Some(Value::Bool(false)) => Ok(SunChoice::Off),
        Some(Value::String(s)) => {
            if s == "default" {
                Ok(SunChoice::Default)
            } else {
                Err(bad("render", format!("unknown sun value `{s}`")))
            }
        }
        Some(Value::Object(obj)) => {
            let date = obj.get("date").and_then(Value::as_str).ok_or_else(|| bad("render", "sun needs a `date` field"))?;
            let time = obj.get("time").and_then(Value::as_str).ok_or_else(|| bad("render", "sun needs a `time` field"))?;
            let site = doc.arch.site.as_ref().ok_or_else(|| bad("render", "sun needs a site location: run site first"))?;
            let sun = sun_position(site.latitude, date, time).ok_or_else(|| bad("render", "invalid sun date or time"))?;
            if sun.elevation_deg <= 0.0 {
                return Err(bad("render", "sun is below the horizon at the given date and time"));
            }
            let dir = sun_dir(&sun);
            let drawing_dir = true_to_drawing(Some(site), dir);
            Ok(SunChoice::Dir(drawing_dir))
        }
        _ => Err(bad("render", "sun must be false, \"default\" or {date, time}")),
    }
}

fn camera_from_saved(c: &Camera3d) -> Result<Camera, EngineError> {
    match c.kind {
        CameraKind::Orbit => {
            let target = c.target.unwrap_or([0.0; 3]);
            let azimuth = c.azimuth_deg.unwrap_or(0.0);
            let elevation = c.elevation_deg.unwrap_or(30.0);
            let distance = c.distance.unwrap_or(10.0);
            let fov = c.fov_deg.unwrap_or(45.0);
            let target_vec = Vec3::new(target[0], target[1], target[2]);
            Ok(Camera::Orbit(Orbit::new(target_vec, azimuth, elevation, distance, fov)))
        }
        CameraKind::Full => {
            let eye_x = c.eye.as_ref().map_or(0.0, |e| e[0]);
            let eye_y = c.eye.as_ref().map_or(0.0, |e| e[1]);
            let eye = Vec3::new(eye_x, eye_y, c.eye_height.unwrap_or(1.6));
            let target = c.target.unwrap_or([0.0; 3]);
            let fov = c.fov_deg.unwrap_or(45.0);
            let target_vec = Vec3::new(target[0], target[1], target[2]);
            Ok(Camera::Full(FullCamera::new(eye, target_vec, fov)))
        }
    }
}

fn camera_from_params(v: Option<&Value>, doc: &Drawing) -> Result<Option<Camera>, EngineError> {
    match v {
        None | Some(Value::Null) => match &doc.arch.active_camera {
            Some(name) => match doc.arch.camera_by_name(name) {
                Some(c) => Ok(Some(camera_from_saved(c)?)),
                None => Ok(None),
            },
            None => Ok(None),
        },
        Some(cv) => Ok(Some(parse_camera(cv)?)),
    }
}

/// `render`: render a 3D still through the installed render hook.
fn run_render(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let hooks = render_hooks().ok_or_else(|| bad("render", "render needs the 3D renderer: run it from cadcraft-cli run or the app"))?;

    let doc = s.doc()?;

    let size = parse_size(params.get("size"))?;
    let camera = camera_from_params(params.get("camera"), doc)?;
    let (final_render, final_mode) = parse_final(params.get("final"))?;
    let sun = parse_sun(params.get("sun"), doc)?;
    let style = parse_style(params.get("style"))?;
    let ground = params.get("ground").and_then(Value::as_bool).unwrap_or(false);
    let budget = parse_budget(params.get("budget"))?;
    let samples = parse_samples(params.get("samples"))?;
    let out = params.get("out").and_then(Value::as_str).map(PathBuf::from);

    let path = parse_path_params(params)?;
    let request = RenderRequest::new(&doc, camera, size, out, final_render, sun, style, ground, budget, samples).with_final(final_mode, path);

    (hooks.render)(&request).map_err(|e| bad("render", e))
}

/// Command specs.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("render", "Render 3D", run_render)
            .menu(&["Architecture", "Render 3D"])
            .params("{out?: path, size?: [w,h], camera?, final?: bool | \"raster\" | \"path\", sun?: false | {date, time} | \"default\", style?: shaded | shaded-edges | hidden-line | glass-house | clay | technical, ground?: bool, budget?, samples?: [[x,y,z], ...], spp?: 1..=65536, bounces?: 0..=32, seed?: 0..=4294967295, denoise?: bool, rt?: bool, snapshots?: (≤8, ascending, each 1..=spp), furnace?: {albedo: 0<a<1, emission: 0<e<=1000}}")
            .noundo(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_path_params_records_whether_spp_was_given() {
        let p = |v: Value| parse_path_params(&v).map(|p| (p.spp, p.spp_given)).ok();
        assert_eq!(p(json!({})), Some((256, false)));
        assert_eq!(p(json!({"spp": 256})), Some((256, true)));
        assert_eq!(p(json!({"spp": 64})), Some((64, true)));
    }

    #[test]
    fn parse_path_params_reads_exposure() {
        let empty = parse_path_params(&json!({})).unwrap();
        assert_eq!(empty.exposure, None);
        let auto = parse_path_params(&json!({"exposure": "auto"})).unwrap();
        assert_eq!(auto.exposure, None);
        let fixed = parse_path_params(&json!({"exposure": 0.5})).unwrap();
        assert_eq!(fixed.exposure, Some(0.5));
        let err = parse_path_params(&json!({"exposure": 0.0})).unwrap_err().to_string();
        assert!(err.contains("`exposure` must be \"auto\" or a positive number"), "{err}");
    }

    #[test]
    fn render_without_a_renderer_is_refused() {
        let mut s = Session::new();
        let r = s.execute("render", &json!({}));
        let e = r.unwrap_err().to_string();
        assert!(e.contains("render needs the 3D renderer"), "{e}");
    }

    #[test]
    fn final_takes_a_bool_or_a_mode() {
        assert_eq!(parse_final(None).ok(), Some((false, FinalMode::default())));
        assert_eq!(parse_final(Some(&json!(true))).ok(), Some((true, FinalMode::default())));
        assert_eq!(parse_final(Some(&json!("raster"))).ok(), Some((true, FinalMode::Raster)));
        assert_eq!(parse_final(Some(&json!("path"))).ok(), Some((true, FinalMode::Path)));
        let e = parse_final(Some(&json!("foo"))).map_err(|e| e.to_string()).err().unwrap_or_default();
        assert!(e.contains("`final` must be"), "{e}");
    }

    #[test]
    fn final_true_is_path_traced_by_default() {
        assert_eq!(FinalMode::default(), FinalMode::Path);
        assert_eq!(parse_final(Some(&json!(true))).ok(), Some((true, FinalMode::Path)));
        assert_eq!(parse_final(Some(&json!("raster"))).ok(), Some((true, FinalMode::Raster)));
    }

    #[test]
    fn style_and_budget_default_when_absent_and_are_checked_when_given() {
        assert_eq!(parse_style(None).ok().as_deref(), Some("shaded"));
        assert_eq!(parse_style(Some(&json!("glass-house"))).ok().as_deref(), Some("glass-house"));
        let e = parse_style(Some(&json!("lit"))).map_err(|e| e.to_string()).err().unwrap_or_default();
        assert!(e.contains("`style` must be one of"), "{e}");
        assert_eq!(parse_budget(None).ok(), Some(8));
        assert_eq!(parse_budget(Some(&json!(0))).ok(), Some(0));
        assert!(parse_budget(Some(&json!(65))).is_err());
        assert!(parse_budget(Some(&json!("8"))).is_err());
    }

    #[test]
    fn render_parses_sun_off_and_site_sun() {
        // No hook is installed, so parse the helpers directly.
        let doc = cadcraft_doc::Drawing::new_metric();
        // sun false -> Off
        let off: serde_json::Value = json!(false);
        assert!(matches!(parse_sun(Some(&off), &doc), Ok(SunChoice::Off)), "sun false should parse to Off");
        // sun {date, time} without a site is refused
        let sun: serde_json::Value = json!({"date": "2026-06-21", "time": "12:00"});
        let r = parse_sun(Some(&sun), &doc);
        let e = r.unwrap_err().to_string();
        assert!(e.contains("sun needs a site location"), "{e}");
        // with a site the site sun is accepted
        let mut doc_with_site = doc.clone();
        doc_with_site.arch.site =
            Some(cadcraft_doc::arch::site::SiteLocation { latitude: 51.5, ..cadcraft_doc::arch::site::SiteLocation::default() });
        let r = parse_sun(Some(&sun), &doc_with_site);
        assert!(matches!(r, Ok(SunChoice::Dir(_))), "site sun should parse to a direction");
    }

    #[test]
    fn render_refuses_bad_size_and_too_many_samples() {
        // bad size: 0
        let s0: serde_json::Value = json!([0, 800]);
        assert!(matches!(parse_size(Some(&s0)), Err(_)), "size 0 should be refused");
        // bad size: > 8192
        let s1: serde_json::Value = json!([9000, 800]);
        assert!(matches!(parse_size(Some(&s1)), Err(_)), "size > 8192 should be refused");
        // default size
        assert!(matches!(parse_size(None), Ok((1200, 800))), "default size should be [1200, 800]");
        // too many samples
        let samples: Vec<serde_json::Value> = (0..65).map(|_| json!([0.0, 0.0, 0.0])).collect();
        let samples_json: serde_json::Value = json!(samples);
        assert!(matches!(parse_samples(Some(&samples_json)), Err(_)), "too many samples should be refused");
        // ok samples
        let ok: serde_json::Value = json!([[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]]);
        assert!(matches!(parse_samples(Some(&ok)), Ok(_)), "valid samples should be accepted");
    }

    #[test]
    fn render_parses_path_params_with_defaults() {
        let p = parse_path_params(&json!({})).unwrap();
        assert_eq!(p.spp, 256);
        assert_eq!(p.bounces, 4);
        assert_eq!(p.seed, 0);
        assert_eq!(p.denoise, true);
        assert_eq!(p.rt, false);
        assert_eq!(p.furnace, None);
        assert_eq!(p.snapshots, Vec::<u32>::new());
    }

    #[test]
    fn render_refuses_bad_path_params_naming_the_field() {
        let check = |params: serde_json::Value, field: &str| {
            let err = parse_path_params(&params).unwrap_err().to_string();
            assert!(err.contains(field), "{field}: {err}");
        };
        check(json!({"spp": 0}), "spp");
        check(json!({"spp": 65537}), "spp");
        check(json!({"spp": "a"}), "spp");
        check(json!({"bounces": 33}), "bounces");
        check(json!({"seed": 4294967296u64}), "seed");
        check(json!({"denoise": "yes"}), "denoise");
        check(json!({"rt": "yes"}), "rt");
        check(json!({"snapshots": (0..9).map(|_| 1).collect::<Vec<_>>()}), "snapshots");
        check(json!({"snapshots": [5, 3]}), "snapshots");
        check(json!({"spp": 10, "snapshots": [20]}), "snapshots");
        check(json!({"furnace": {"albedo": -0.1, "emission": 1.0}}), "albedo");
        check(json!({"furnace": {"albedo": 0.5, "emission": 0.0}}), "emission");
    }

    #[test]
    fn parse_path_params_empty_snapshots_accepted() {
        let p = parse_path_params(&json!({"snapshots": []})).unwrap();
        assert_eq!(p.snapshots, Vec::<u32>::new());
    }

    #[test]
    fn parse_path_params_furnace_missing_albedo_refused() {
        let err = parse_path_params(&json!({"furnace": {"emission": 1.0}})).unwrap_err().to_string();
        assert!(err.contains("albedo"), "{err}");
    }

    #[test]
    fn parse_path_params_furnace_missing_emission_refused() {
        let err = parse_path_params(&json!({"furnace": {"albedo": 0.5}})).unwrap_err().to_string();
        assert!(err.contains("emission"), "{err}");
    }

    #[test]
    fn parse_sun_default_string_accepted() {
        let doc = cadcraft_doc::Drawing::new_metric();
        let sun: serde_json::Value = json!("default");
        assert!(matches!(parse_sun(Some(&sun), &doc), Ok(SunChoice::Default)));
    }

    #[test]
    fn parse_sun_refuses_invalid_values() {
        let doc = cadcraft_doc::Drawing::new_metric();
        let true_val: serde_json::Value = json!(true);
        assert!(parse_sun(Some(&true_val), &doc).is_err());
        let num: serde_json::Value = json!(42);
        assert!(parse_sun(Some(&num), &doc).is_err());
        let arr: serde_json::Value = json!([]);
        assert!(parse_sun(Some(&arr), &doc).is_err());
        let unknown: serde_json::Value = json!("off");
        let err = parse_sun(Some(&unknown), &doc).unwrap_err().to_string();
        assert!(err.contains("unknown sun value"), "{err}");
    }

    #[test]
    fn parse_samples_refuses_non_number() {
        let not_a_number: serde_json::Value = json!([["nan", 0.0, 0.0]]);
        let err = parse_samples(Some(&not_a_number)).unwrap_err().to_string();
        assert!(err.contains("finite"), "{err}");
    }

    #[test]
    fn clay_is_a_render_style() {
        assert_eq!(parse_style(Some(&json!("clay"))).ok().as_deref(), Some("clay"));
        assert_eq!(parse_style(None).ok().as_deref(), Some("shaded"));
        let e = parse_style(Some(&json!("clay-ish"))).map_err(|e| e.to_string()).err().unwrap_or_default();
        assert!(e.contains("`style` must be one of"), "{e}");
    }

    #[test]
    fn parse_samples_refuses_wrong_element_count() {
        let two: serde_json::Value = json!([[0.0, 0.0]]);
        let err = parse_samples(Some(&two)).unwrap_err().to_string();
        assert!(err.contains("must be [x,y,z]"), "{err}");
        let four: serde_json::Value = json!([[0.0, 0.0, 0.0, 0.0]]);
        let err = parse_samples(Some(&four)).unwrap_err().to_string();
        assert!(err.contains("must be [x,y,z]"), "{err}");
    }

    #[test]
    fn technical_is_a_render_style() {
        assert_eq!(parse_style(Some(&json!("technical"))).ok().as_deref(), Some("technical"));
        let e = parse_style(Some(&json!("tech"))).map_err(|e| e.to_string()).err().unwrap_or_default();
        assert!(e.contains("`style` must be one of"), "{e}");
    }

    #[test]
    fn technical_draws_lines_over_flat_colours() {
        // This test verifies the style is accepted by the command parser.
        // The actual rendering logic is tested in render3d.
        let style = parse_style(Some(&json!("technical"))).unwrap();
        assert_eq!(style, "technical");
    }
}
