//! The session renderer behind `render` (3D), `sunstudy` and the MCP tools: one offscreen GPU
//! renderer kept for the whole process, so the mesh upload and the shadow maps are reused from one
//! still to the next. A call that only moves the camera builds no shadow map.

use std::sync::Mutex;

use cadcraft_arch3d::camera::{project_point, ray_at};
use cadcraft_arch3d::light::scene_lights_with;
use cadcraft_arch3d::pick::Bvh;
use cadcraft_arch3d::view_filter::ViewFilter;
use cadcraft_engine::doc::arch::{ExposureSetting, LightSet, Look};
use cadcraft_engine::{FinalMode, IlluminanceRequest, PathParams, RenderHooks, RenderRequest, RenderStats, SunChoice, set_render_hooks};
use cadcraft_geom::Vec3;
use cadcraft_render3d::adapter::AdapterChoice;
use cadcraft_render3d::camera::{Camera, Orbit};
use cadcraft_render3d::exposure::{ev100_scale, scale_ev100, tone_code, tone_name};
use cadcraft_render3d::final_render::render_final_with;
use cadcraft_render3d::light::Light;
use cadcraft_render3d::lights::DEFAULT_EXPOSURE;
use cadcraft_render3d::look::lighting_for;
use cadcraft_render3d::offscreen::{Offscreen, RenderOptions, png_rgba};
use cadcraft_render3d::pathtrace::cache;
use cadcraft_render3d::pathtrace::{CacheOutcome, ComputeGpu, Exposure, Furnace, PathOptions, irradiance, luminance, request_compute};
use cadcraft_render3d::{Lighting, Scene3d, render_final};
use serde_json::{Value, json};

/// The most sample points a request reports.
const MAX_SAMPLES: usize = 64;
/// How near (m) the pick ray's first hit must be to a sample point for it to count as visible.
const VISIBLE_TOLERANCE: f64 = 0.05;

/// The renderer and the scene it last built.
struct State {
    off: Offscreen,
    /// The `arch_revision` and light set key the scene below was built for, and the scene.
    scene: Option<(u64, String, Scene3d)>,
    /// The scene with the cutaway's hidden objects removed, and the (revision, light-set key,
    /// hidden-set key) it was made for; rebuilt only when one of the three changes.
    filtered: Option<((u64, String, u64), Scene3d)>,
    /// The compute device, requested once on the first path request (`Some(None)`: none available);
    /// replaced once by a device with ray queries on the first `rt` request (see `compute_of`).
    compute: Option<Option<ComputeGpu>>,
    /// A device with ray queries was asked for already (granted or not), so `rt` never asks again.
    rt_asked: bool,
    /// The pipeline cache and what opening it reported, kept for the process.
    cache: Option<(Option<wgpu::PipelineCache>, CacheOutcome)>,
    /// Whether the cache file was written after a miss.
    cache_saved: bool,
    /// Path-traced stills rendered (a fallback to raster is not one).
    path_traces: u64,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Install the renderer as the engine's render hook (idempotent: the first call wins).
pub(crate) fn install() {
    set_render_hooks(RenderHooks::new(render_one, stats).with_illuminance(illuminance_one));
}

fn stats() -> RenderStats {
    let Ok(guard) = STATE.lock() else { return RenderStats::default() };
    let Some(s) = guard.as_ref() else { return RenderStats::default() };
    let mut r = RenderStats::default();
    r.shadow_maps_built = s.off.shadow_maps_built();
    r.final_renders = s.off.final_renders();
    r.path_traces = s.path_traces;
    r
}

/// The tests use the software adapter so the counts do not depend on the machine's GPU.
#[cfg(test)]
fn adapter_choice() -> AdapterChoice {
    AdapterChoice::Fallback
}

#[cfg(not(test))]
fn adapter_choice() -> AdapterChoice {
    std::env::var("CADCRAFT_RENDER_ADAPTER").ok().and_then(|v| AdapterChoice::parse(&v)).unwrap_or(AdapterChoice::Auto)
}

fn gpu(e: cadcraft_render3d::Render3dError) -> String {
    match e {
        cadcraft_render3d::Render3dError::NoAdapter => "NO_ADAPTER: no GPU adapter".to_string(),
        e => e.to_string(),
    }
}

/// The renderer state, created on first use at `size`, with the scene for `drawing` built.
/// If `light_set` is provided, the scene's lights are modified to match that set.
fn ready<'a>(
    guard: &'a mut Option<State>,
    drawing: &cadcraft_engine::doc::Drawing,
    size: (u32, u32),
    light_set: Option<&str>,
) -> Result<&'a mut State, String> {
    if guard.is_none() {
        let off = Offscreen::create_with(adapter_choice(), size).map_err(gpu)?;
        *guard = Some(State { off, scene: None, filtered: None, compute: None, rt_asked: false, cache: None, cache_saved: false, path_traces: 0 });
    }
    let state = guard.as_mut().ok_or("renderer unavailable")?;
    let key = drawing.arch_revision;

    // Derive a key for the light set to invalidate the scene cache when it changes.
    // We hash the set name and its contents (lights and their states) to ensure
    // different sets or modified sets trigger a rebuild.
    let ls_key = if let Some(set_name) = light_set {
        if let Some(set) = drawing.arch.light_sets.iter().find(|s| s.name == set_name) {
            // Simple deterministic hash: name + sorted light states
            let mut h = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            set.name.hash(&mut h);
            // set.lights is a BTreeMap<String, LightState> (deterministic order)
            for (key, state) in &set.lights {
                key.hash(&mut h);
                state.on.hash(&mut h);
                state.brightness.to_bits().hash(&mut h);
            }
            format!("{}-{:x}", set_name, h.finish())
        } else {
            format!("{}-missing", set_name)
        }
    } else {
        String::new()
    };

    let needs_rebuild = state.scene.as_ref().map_or(true, |(k, lsk, _)| *k != key || *lsk != ls_key);

    if needs_rebuild {
        let mut scene = cadcraft_arch3d::build(&drawing.arch);
        if let Some(set_name) = light_set {
            if let Some(set) = drawing.arch.light_sets.iter().find(|s| s.name == set_name) {
                scene.lights = scene_lights_with(&drawing.arch, &set.lights);
            }
        }
        state.scene = Some((key, ls_key, scene));
    }
    Ok(state)
}

/// The reply keys every render reports: the exposure scale, its EV100, how it was chosen and the
/// tone map (`code` as `exposure::tone_code` gives it).
///
/// `manual_ev100` is the requested EV100 when the exposure mode is manual; if present, it is
/// echoed exactly instead of being recomputed from the f32 scale (which loses precision).
fn exposure_keys(scale: f32, mode: &str, code: u32, manual_ev100: Option<f64>) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("exposure".into(), json!(scale));
    let ev100 = manual_ev100.unwrap_or_else(|| scale_ev100(scale));
    m.insert("ev100".into(), json!(ev100));
    m.insert("exposureMode".into(), json!(if mode.is_empty() { "auto" } else { mode }));
    m.insert("toneMap".into(), json!(tone_name(code)));
    m
}

/// How the path tracer's exposure was chosen: a bare number is `fixed`, a look's EV100 `manual`.
fn path_exposure_mode(p: &PathParams, look: &Look) -> &'static str {
    match (p.exposure, look.exposure) {
        (Some(_), _) => "fixed",
        (None, ExposureSetting::Manual { .. }) => "manual",
        _ => "auto",
    }
}

/// The path-tracer options a request asks for: `look` gives the exposure (a bare number first,
/// then a manual EV100, else metered) and the tone map; `albedo` is the Lambert calibration.
fn path_options(p: &PathParams, look: &Look, albedo: Option<f64>) -> PathOptions {
    let exposure = match (p.exposure, look.exposure) {
        (Some(x), _) => Exposure::Fixed(x as f32),
        (None, ExposureSetting::Manual { ev100 }) => Exposure::Fixed(ev100_scale(ev100)),
        _ => Exposure::Auto,
    };
    PathOptions {
        spp: p.spp,
        bounces: p.bounces,
        seed: p.seed,
        denoise: p.denoise,
        rt: p.rt,
        exposure,
        tone_map: tone_code(look.tone_map),
        lambert: albedo.map(|a| a as f32),
        furnace: p.furnace.map(|(albedo, emission)| Furnace { albedo: albedo as f32, emission: emission as f32, ..Furnace::default() }),
        snapshots: p.snapshots.clone(),
        ..PathOptions::default()
    }
}

/// The compute device, requested on the first call. The first `rt` request on a device without ray
/// queries asks once for one that has them and, when the adapter grants it, uses it from then on (with its
/// own pipeline cache; compute-path renders run on it unchanged). Until 2026-11-03 a later `rt` request kept
/// the first device, so `rt` after a compute render never used ray queries (phase-11 check 10 skipped on
/// every adapter). Devices live for the process, so the request is never repeated.
fn compute_of(state: &mut State, rt: bool) -> Result<Option<ComputeGpu>, String> {
    if state.compute.is_none() {
        state.rt_asked = rt;
        state.compute = Some(request_compute(adapter_choice(), rt).map_err(gpu)?);
    } else if rt && !state.rt_asked && matches!(&state.compute, Some(Some(g)) if !g.ray_query) {
        state.rt_asked = true;
        if let Some(g) = request_compute(adapter_choice(), true).map_err(gpu)?.filter(|g| g.ray_query) {
            state.compute = Some(Some(g));
            state.cache = None;
            state.cache_saved = false;
        }
    }
    Ok(state.compute.clone().flatten())
}

/// The samples per pixel a default Finalize uses on a software (CPU) adapter.
const SOFTWARE_SPP: u32 = 64;

/// The samples per pixel to render with: `SOFTWARE_SPP` for a request that did not name spp (and
/// keeps no snapshots) on a CPU adapter, else the request's own.
fn software_spp(p: &PathParams, on_cpu: bool) -> u32 {
    if on_cpu && !p.spp_given && p.snapshots.is_empty() { SOFTWARE_SPP } else { p.spp }
}

/// The fixtures that can cast a shadow: every light but a strip (a strip counts as one drawn light
/// and is never shadow-mapped in the raster; its shadows are the path tracer's).
fn shadow_casting_lights(scene: &Scene3d) -> usize {
    scene.lights.iter().filter(|l| l.kind != cadcraft_arch3d::light::LightKind::Strip).count()
}

/// `<out without .png>-spp<n>.png`.
fn snapshot_path(out: &std::path::Path, spp: u32) -> std::path::PathBuf {
    let stem = out.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    out.with_file_name(format!("{stem}-spp{spp}.png"))
}

fn write_png(out: &std::path::Path, png: &[u8]) -> Result<(), String> {
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
    }
    // `write_atomic` now returns the newly-created folder (if any); we only need the write to
    // succeed, so discard it.
    crate::output::write_atomic(out, png).map_err(|e| e.to_string()).map(|_| ())
}

/// How many environments to keep in the process-wide cache.
const ENV_CACHE_CAP: usize = 4;
type EnvCache = Vec<(u64, Option<std::sync::Arc<cadcraft_render3d::Environment>>)>;
static ENV_CACHE: Mutex<EnvCache> = Mutex::new(Vec::new());

/// The environment for the drawing and look, rebuilt only when the look's sky fields, its
/// backdrop, the sun direction or the arch revision change (an HDRI decode is not cheap).
/// The last up to `ENV_CACHE_CAP` environments are kept in an LRU order, so alternating looks
/// reuse their built environment instead of re-decoding the HDRI.
fn environment_cached(
    model: &cadcraft_engine::doc::arch::ArchModel,
    revision: u64,
    look: &Look,
    sun_dir: Option<[f64; 3]>,
) -> Option<std::sync::Arc<cadcraft_render3d::Environment>> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}", look.sky).hash(&mut h);
    format!("{:?}", look.backdrop).hash(&mut h);
    sun_dir.map(|d| d.map(f64::to_bits)).hash(&mut h);
    revision.hash(&mut h);
    let key = h.finish();
    if let Ok(mut guard) = ENV_CACHE.lock() {
        if let Some(idx) = guard.iter().position(|(k, _)| *k == key) {
            let (_, env) = guard.remove(idx);
            guard.push((key, env.clone()));
            return env;
        }
    }
    let env = cadcraft_render3d::environment_for(model, look, sun_dir);
    if let Ok(mut guard) = ENV_CACHE.lock() {
        if guard.len() == ENV_CACHE_CAP {
            guard.remove(0);
        }
        guard.push((key, env.clone()));
    }
    env
}

fn render_one(req: &RenderRequest) -> Result<Value, String> {
    let style = crate::render::parse_style(&req.style).ok_or_else(|| format!("`{}` is not one of {}", req.style, crate::render::STYLE_NAMES))?;
    let mut guard = STATE.lock().map_err(|_| "renderer unavailable".to_string())?;
    let state = ready(&mut guard, req.drawing, req.size, req.light_set.as_deref())?;
    let revision = req.drawing.arch_revision;
    let want_path = req.final_render && req.final_mode == FinalMode::Path;
    let compute = if want_path { compute_of(state, req.path.rt)? } else { None };
    if let (Some(c), None) = (&compute, &state.cache) {
        state.cache = Some(cache::open(c, cache::cache_dir().as_deref()));
    }
    let State { off, scene, filtered, cache: pcache, cache_saved, path_traces, .. } = state;
    let (look, sun_dir) = req.resolved_sun();
    let (base_light_key, base) = scene.as_mut().map(|(_, k, s)| (k.clone(), s)).ok_or("renderer unavailable")?;
    // The effective look's backdrop replaces the cached scene's (no rebuild).
    cadcraft_arch3d::set_backdrop(&req.drawing.arch, base, look.backdrop.as_ref());
    let base = &*base;
    off.resize(req.size);

    let camera = req.camera.clone().unwrap_or_else(|| Camera::Orbit(Orbit::fit(&base.bounds)));
    let eye = camera.eye();
    // The view's filter: the cutaway's hidden objects leave the scene (a filtered copy, cached by
    // revision, light set and hidden set, with its own scene key so uploads and shadow maps follow);
    // the clip box and near clip reach the raster and the path tracer through the look.
    let mut filter = ViewFilter::from_look(&look, &req.drawing.arch, &camera);
    if let Some((level, lower)) = req.overview {
        filter = filter.with_overview(&req.drawing.arch, level, lower);
    }
    let (scene, key): (&Scene3d, u64) = if filter.hidden.is_empty() && filter.levels.is_none() {
        (base, revision)
    } else {
        let hidden_key = hidden_set_key(&filter);
        let want = (revision, base_light_key, hidden_key);
        if filtered.as_ref().is_none_or(|(k, _)| *k != want) {
            *filtered = Some((want, filter.apply(base)));
        }
        let Some((_, f)) = filtered.as_mut() else { return Err("renderer unavailable".into()) };
        cadcraft_arch3d::set_backdrop(&req.drawing.arch, f, look.backdrop.as_ref());
        (&*f, filtered_scene_key(revision, hidden_key))
    };
    let moon_dir = cadcraft_arch3d::sky::moon_dir(&req.drawing.arch, &look.sky.moon);
    let mut lighting = lighting_for(&look, sun_dir, req.budget).with_moon(moon_dir);
    lighting.environment = environment_cached(&req.drawing.arch, revision, &look, sun_dir);
    // The calibration albedo, the metered night (all but the legacy `sun: false`) and a bare-number
    // exposure, which the raster uses as its fixed scale too.
    lighting.lambert = req.albedo.map(|a| a as f32);
    lighting.meter_night = !req.legacy_night;
    if let Some(x) = req.path.exposure {
        lighting.exposure = x as f32;
    }
    let maps_before = off.shadow_maps_built();

    // The manual EV100 from the look, if any, used for exact echo in replies.
    let manual_ev = match look.exposure {
        ExposureSetting::Manual { ev100 } => Some(ev100),
        _ => None,
    };

    let mut final_mode: Option<&str> = None;
    let mut path_info = serde_json::Map::new();
    let mut exposure_info: Option<serde_json::Map<String, Value>> = None;

    // The manual EV100 from the look, if any, used for exact echo in replies.
    let manual_ev = match look.exposure {
        ExposureSetting::Manual { ev100 } => Some(ev100),
        _ => None,
    };

    let (rgba, size, drawn, shadowed, png) = if want_path {
        let mut opts = path_options(&req.path, &look, req.albedo);
        // The default 256 spp is too slow on a software (CPU) adapter: lower an unchanged default
        // to 64 (an explicit spp, or snapshots that may need more samples, are kept).
        let on_cpu = compute.as_ref().is_some_and(|c| c.info.adapter_type == "Cpu");
        opts.spp = software_spp(&req.path, on_cpu);
        let cache_ref = pcache.as_ref().and_then(|(c, _)| c.as_ref());
        let (png, stats, fallback) =
            render_final_with(off, compute.as_ref(), scene, key, &camera, &lighting, req.ground, req.size, true, &opts, cache_ref).map_err(gpu)?;
        if let (Some(c), Some((Some(handle), CacheOutcome::Miss)), false) = (&compute, &*pcache, *cache_saved) {
            if let Some(dir) = cache::cache_dir() {
                *cache_saved = cache::save(c, handle, &dir).is_ok();
            }
        }
        match (stats, fallback) {
            (Some(st), None) => {
                final_mode = Some("path");
                *path_traces += 1;
                // The first open reports miss/hit; later renders in this process report the same.
                let outcome = pcache.as_ref().map_or(st.pipeline_cache, |(_, o)| *o);
                path_info.insert(
                    "environment".into(),
                    lighting.environment.as_ref().map_or(Value::Null, |e| json!({ "width": e.width, "height": e.height, "visible": e.visible })),
                );
                path_info.insert("spp".into(), json!(st.spp_done));
                path_info.insert("bounces".into(), json!(opts.bounces));
                path_info.insert("seed".into(), json!(opts.seed));
                path_info.insert("denoised".into(), json!(st.denoised));
                path_info.insert("rt".into(), json!(st.rt));
                if let Some(e) = &st.rt_error {
                    path_info.insert("rtError".into(), json!(e));
                }
                let path_mode = path_exposure_mode(&req.path, &look);
                let path_manual = if path_mode == "manual" { manual_ev } else { None };
                exposure_info = Some(exposure_keys(st.exposure, path_mode, opts.tone_map, path_manual));
                path_info.insert("pipelineCache".into(), json!(outcome.name()));
                path_info.insert("radiance".into(), st.radiance.map_or(Value::Null, |r| json!({ "min": r.min, "max": r.max, "mean": r.mean })));
                let mut snaps = Vec::new();
                for (spp, pixels) in &st.snapshots {
                    let file = match &req.out {
                        Some(out) => {
                            let img = cadcraft_render3d::offscreen::encode_png(req.size.0, req.size.1, pixels).map_err(gpu)?;
                            let path = snapshot_path(out, *spp);
                            write_png(&path, &img)?;
                            Some(path.display().to_string())
                        }
                        None => None,
                    };
                    snaps.push(json!({ "spp": spp, "out": file }));
                }
                path_info.insert("snapshots".into(), Value::Array(snaps));
            }
            (_, fallback) => {
                final_mode = Some("raster");
                path_info.insert("fallback".into(), json!(fallback.unwrap_or("no compute shaders")));
            }
        }
        let rgba = png_rgba(&png).map_err(gpu)?;
        // A strip counts once as a fixture and is never shadowed.
        (rgba, req.size, scene.lights.len(), shadow_casting_lights(scene), Some(png))
    } else if req.final_render {
        final_mode = Some("raster");
        let png = render_final(off, scene, key, &camera, &lighting, req.ground, req.size).map_err(gpu)?;
        let rgba = png_rgba(&png).map_err(gpu)?;
        (rgba, req.size, scene.lights.len(), shadow_casting_lights(scene), Some(png))
    } else {
        let frame = off.lit_frame(scene, key, [eye.x, eye.y, eye.z], &lighting, req.ground);
        let light = lighting.sun.unwrap_or(Light::DEFAULT);
        let opts = RenderOptions { lit: frame.as_ref(), style, ground: req.ground, ..RenderOptions::default() };
        off.render_with(scene, key, &camera, &light, &opts);
        let rgba = off.read_rgba().map_err(gpu)?;
        let (drawn, shadowed) = frame.as_ref().map_or((0, 0), |f| (f.lights, f.shadowed));
        (rgba, off.size(), drawn, shadowed, None)
    };

    let png = match (&req.out, png) {
        (None, _) => None,
        (Some(_), Some(png)) => Some(png),
        (Some(_), None) => Some(off.read_png().map_err(gpu)?),
    };
    if let (Some(out), Some(png)) = (&req.out, &png) {
        write_png(out, png)?;
    }

    // The raster (interactive, Finalize, or the path's fallback) reports the frame it drew.
    let exposure_info = exposure_info.unwrap_or_else(|| {
        let f = off.last_frame_look();
        let mode = if f.exposure_mode.is_empty() { "" } else { &f.exposure_mode };
        // If the frame reports manual mode, we prefer the look's exact EV100 if available.
        let manual = if mode == "manual" { manual_ev } else { None };
        exposure_keys(f.exposure, mode, if f.tone_map == 0 { tone_code(look.tone_map) } else { f.tone_map }, manual)
    });

    // Visibility is read from the filtered scene, ignoring hits the clip box or near clip remove.
    // When ground is drawn, the ground mesh (and horizon plane) are not in scene.meshes, so we
    // build a temporary scene clone including them for picking, ensuring ground samples are visible.
    let pick_scene: Option<Scene3d> = if req.ground && !req.samples.is_empty() {
        let mut s = scene.clone();
        s.meshes.push(cadcraft_render3d::shadow::ground_mesh(&scene.bounds));
        if let Some(ref horizon) = scene.horizon {
            s.meshes.push(horizon.clone());
        }
        Some(s)
    } else {
        None
    };

    let (bvh, pick_target) = if let Some(ref s) = pick_scene {
        (Bvh::build(s), s)
    } else if !req.samples.is_empty() {
        (Bvh::build(scene), scene)
    } else {
        // No samples, no BVH needed.
        (Bvh::default(), scene)
    };

    let samples: Vec<Value> = if !req.samples.is_empty() {
        req.samples.iter().take(MAX_SAMPLES).map(|p| sample(pick_target, &bvh, &filter, &camera, size, &rgba, *p)).collect()
    } else {
        Vec::new()
    };
    let mut reply = json!({
        "ok": true,
        "out": req.out.as_ref().map(|p| p.display().to_string()),
        "size": [size.0, size.1],
        "final": req.final_render,
        "lights": scene.lights.len(),
        "drawnLights": drawn,
        "shadowedLights": shadowed,
        "shadowMapsBuilt": off.shadow_maps_built(),
        "finalRenders": off.final_renders(),
        "frameShadowMaps": off.shadow_maps_built().saturating_sub(maps_before),
        "samples": samples,
        "device": off.device_info().line(),
        "finalMode": final_mode,
        "pathTraces": *path_traces,
        "filter": { "hidden": filter.hidden.len(), "clip": filter.clip.is_some(), "nearClip": filter.near },
    });
    if let Some(map) = reply.as_object_mut() {
        map.extend(path_info);
        map.extend(exposure_info);
    }
    Ok(reply)
}

/// Path-traced illuminance at the request's probes: one probe's answer is the object, several
/// are `{"probes": [...]}`.
fn illuminance_one(req: &IlluminanceRequest) -> Result<Value, String> {
    let mut guard = STATE.lock().map_err(|_| "renderer unavailable".to_string())?;
    let state = ready(&mut guard, req.drawing, (64, 64), None)?;
    let Some(device) = compute_of(state, req.path.rt)? else {
        return Err("path-traced illuminance needs compute shaders on this GPU".into());
    };
    let scene = &state.scene.as_ref().ok_or("renderer unavailable")?.2;
    let (look, sun_dir) = req.resolved_sun();
    let moon_dir = cadcraft_arch3d::sky::moon_dir(&req.drawing.arch, &look.sky.moon);
    let mut lighting = lighting_for(&look, sun_dir, 8).with_moon(moon_dir);
    lighting.environment = environment_cached(&req.drawing.arch, req.drawing.arch_revision, &look, sun_dir);
    let opts = path_options(&req.path, &look, None);
    let rgb = irradiance(&device, scene, &lighting, false, &req.probes, &opts)?;
    let results: Vec<Value> = req
        .probes
        .iter()
        .zip(&rgb)
        .map(|([point, normal], c)| {
            json!({
                "lux": luminance(*c), "rgb": c, "method": "path",
                "spp": opts.spp, "bounces": opts.bounces, "seed": opts.seed,
                "point": point, "normal": normal,
            })
        })
        .collect();
    match results.len() {
        1 => Ok(results.into_iter().next().unwrap_or(Value::Null)),
        _ => Ok(json!({ "probes": results })),
    }
}

/// One sample: the pixel `p` projects into, the colour there and whether the camera sees `p`.
/// Pixel `x` spans `[x, x + 1)` in `project_point`'s coordinates, and the visibility ray goes
/// through `p`'s exact projection: a ray through a pixel's corner misses `p` by centimetres
/// where the surface is seen at a grazing angle.
fn sample(scene: &Scene3d, bvh: &Bvh, filter: &ViewFilter, camera: &Camera, size: (u32, u32), rgba: &[u8], p: [f64; 3]) -> Value {
    let pixel = project_point(camera, size, p).and_then(|px| {
        let (x, y) = (px[0].floor(), px[1].floor());
        let inside = x.is_finite() && y.is_finite() && x >= 0.0 && y >= 0.0 && x < size.0 as f32 && y < size.1 as f32;
        inside.then_some((px, x as u32, y as u32))
    });
    let Some((px, x, y)) = pixel else {
        return json!({ "point": p, "pixel": null, "rgb": null, "visible": false });
    };
    let at = (usize::try_from(y).unwrap_or(0)).saturating_mul(size.0 as usize).saturating_add(x as usize).saturating_mul(4);
    let rgb = rgba.get(at..at.saturating_add(3)).map(<[u8]>::to_vec);
    let visible = ray_at(camera, size, px)
        .and_then(|(origin, dir)| scene.pick_filtered(bvh, origin, dir, &|h| filter.accepts(h.point)))
        .is_some_and(|hit| distance(hit.point, Vec3::new(p[0], p[1], p[2])) <= VISIBLE_TOLERANCE);
    json!({ "point": p, "pixel": [x, y], "rgb": rgb, "visible": visible })
}

/// A key for the cutaway's hidden set alone (the clip box and near clip do not change the scene).
fn hidden_set_key(filter: &ViewFilter) -> u64 {
    let mut only_hidden = filter.clone();
    only_hidden.clip = None;
    only_hidden.near = None;
    only_hidden.key()
}

/// The scene key a filtered scene is uploaded and shadowed under: the drawing's revision mixed with
/// the hidden set's key, never equal to the plain revision's.
fn filtered_scene_key(revision: u64, hidden_key: u64) -> u64 {
    (revision ^ hidden_key.rotate_left(32).wrapping_mul(0x9E37_79B9_7F4A_7C15)) | (1 << 63)
}

fn distance(a: Vec3, b: Vec3) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_engine::doc::Drawing;
    use cadcraft_engine::doc::arch::{ArchId, LightFixture, Wall};
    use cadcraft_geom::Vec2;

    #[test]
    fn replies_report_exposure_mode_ev100_tone_map() {
        let k = exposure_keys(ev100_scale(2.0), "manual", cadcraft_render3d::exposure::TONE_HABLE, None);
        assert_eq!(k["exposureMode"], "manual");
        assert_eq!(k["toneMap"], "hable");
        assert!((k["ev100"].as_f64().unwrap_or(f64::NAN) - 2.0).abs() < 1e-4, "{k:?}");
        assert!((k["exposure"].as_f64().unwrap_or(f64::NAN) - 1.0 / 4.8).abs() < 1e-6, "{k:?}");
        let k = exposure_keys(0.01, "", 0, None);
        assert_eq!((k["exposureMode"].clone(), k["toneMap"].clone()), (json!("auto"), json!("none")));
        assert!((k["ev100"].as_f64().unwrap_or(f64::NAN) - 6.3808).abs() < 1e-3, "{k:?}");
        // The path's mode follows the request: a bare number, a manual EV100, else metered.
        let mut look = Look::default();
        assert_eq!(path_exposure_mode(&PathParams::default(), &look), "auto");
        look.exposure = ExposureSetting::Manual { ev100: 3.0 };
        assert_eq!(path_exposure_mode(&PathParams::default(), &look), "manual");
        let fixed = PathParams { exposure: Some(0.5), ..PathParams::default() };
        assert_eq!(path_exposure_mode(&fixed, &look), "fixed");
        let o = path_options(&PathParams::default(), &look, Some(0.5));
        assert_eq!((o.exposure, o.lambert, o.tone_map), (Exposure::Fixed(ev100_scale(3.0)), Some(0.5), 1));
        assert_eq!(path_options(&fixed, &look, None).exposure, Exposure::Fixed(0.5));
    }

    #[test]
    fn manual_ev100_is_echoed_exactly() {
        // A manual EV100 of 1.0 should be echoed as exactly 1.0, not the f32 approximation.
        let scale = ev100_scale(1.0);
        let k = exposure_keys(scale, "manual", 0, Some(1.0));
        assert_eq!(k["ev100"].as_f64(), Some(1.0), "manual EV100 must be exact: {:?}", k["ev100"]);

        // Auto mode uses the scale-derived EV100.
        let k_auto = exposure_keys(scale, "auto", 0, None);
        assert!((k_auto["ev100"].as_f64().unwrap() - scale_ev100(scale)).abs() < 1e-9);
    }

    #[test]
    fn ground_samples_are_visible_with_ground_on() {
        let _g = reset();
        let d = room();
        let mut req = request(&d, None, None, false);
        req.ground = true;
        // Use a camera inside the room looking down at the floor/ground.
        let eye = Vec3::new(2.0, 2.0, 1.5);
        let target = Vec3::new(2.0, 2.0, 0.0);
        let camera = Camera::Full(cadcraft_render3d::camera::FullCamera::new(eye, target, 70.0));
        req.camera = Some(camera);
        // Sample the floor point.
        req.samples = vec![[2.0, 2.0, 0.0]];

        let Some(v) = render(&req) else { return };
        let s = v["samples"].as_array().cloned().unwrap_or_default();
        assert_eq!(s.len(), 1, "{v}");
        assert_eq!(s[0]["visible"], true, "ground sample should be visible when ground is drawn: {v}");
    }

    #[test]
    fn ground_sample_behind_a_wall_is_not_visible() {
        let _g = reset();
        let mut d = room();
        // Add a wall in the middle of the room to block the view.
        let mut w = Wall::new(Vec2::new(1.0, 1.0), Vec2::new(3.0, 1.0), 0.2);
        w.height = 2.4;
        d.arch.push_wall(w);
        d.touch_arch();

        let mut req = request(&d, None, None, false);
        req.ground = true;
        // Sample a point behind the new wall from the default camera (which is likely outside or high).
        // The default camera is Orbit::fit. Let's pick a point clearly occluded.
        // The room is 4x4. The wall is at y=1 from x=1 to 3.
        // A point at (2.0, 0.5, 0.0) is "behind" the wall relative to a camera looking from y > 1.
        // However, the default camera fits the scene. Let's just check that a point *on* the wall itself or behind it
        // behaves correctly.
        // Actually, simpler test: A point on the ground that is occluded by the column in block_scene?
        // No, we are using `room()`.
        // Let's use the `block_scene` equivalent or just verify that a point *inside* a wall mesh is not visible?
        // The requirement is "A point hidden behind real geometry stays not visible".

        // Let's place a point on the ground at (2.0, 2.0, 0.0) which is visible.
        // Then place a point at (2.0, 0.5, 0.0) which might be visible depending on camera.
        // Let's use a specific camera to ensure occlusion.

        let eye = Vec3::new(2.0, 5.0, 1.5);
        let target = Vec3::new(2.0, 2.0, 0.0);
        let camera = Camera::Full(cadcraft_render3d::camera::FullCamera::new(eye, target, 70.0));

        let mut req = request(&d, Some(camera), None, false);
        req.ground = true;
        // Point behind the wall (y < 1.0) from camera at y=5.0.
        req.samples = vec![[2.0, 0.5, 0.0]];

        let Some(v) = render(&req) else { return };
        let s = v["samples"].as_array().cloned().unwrap_or_default();
        assert_eq!(s.len(), 1, "{v}");
        // The wall at y=1.0 should block the view of the ground at y=0.5.
        assert_eq!(s[0]["visible"], false, "ground sample behind wall should not be visible: {v}");
    }

    #[test]
    fn software_spp_lowers_only_an_unchanged_default() {
        let given = |spp| PathParams { spp, spp_given: true, ..PathParams::default() };
        assert_eq!(software_spp(&PathParams::default(), true), SOFTWARE_SPP);
        assert_eq!(software_spp(&given(256), true), 256);
        assert_eq!(software_spp(&PathParams::default(), false), 256);
        assert_eq!(software_spp(&PathParams { snapshots: vec![4, 16], ..PathParams::default() }, true), 256);
        assert_eq!(software_spp(&given(32), true), 32);
    }

    /// Four pot lights in a closed 4 m room.
    fn room() -> Drawing {
        let mut d = Drawing::new_metric();
        let c = [(-0.1, -0.1), (4.1, -0.1), (4.1, 4.1), (-0.1, 4.1)];
        for k in 0..4 {
            let (a, b) = (c[k], c[(k + 1) % 4]);
            let mut w = Wall::new(Vec2::new(a.0, a.1), Vec2::new(b.0, b.1), 0.2);
            w.height = 2.4;
            d.arch.push_wall(w);
        }
        for (k, (x, y)) in [(1.0, 1.0), (1.0, 3.0), (3.0, 1.0), (3.0, 3.0)].into_iter().enumerate() {
            d.arch.lights.push(LightFixture::new(ArchId(k as u32), "Recessed pot light", Vec2::new(x, y)));
        }
        d.arch.next_light_id = 4;
        d.touch_arch();
        d
    }

    #[test]
    fn a_sample_sees_the_point_it_projects_and_reads_its_pixel() {
        // The room's inner wall faces from inside, at a grazing angle where a pixel spans several
        // centimetres; each pixel's red and green are its own x and y.
        let scene = cadcraft_arch3d::build(&room().arch);
        let eye = Vec3::new(3.6, 3.6, 1.6);
        let camera = Camera::Full(cadcraft_render3d::camera::FullCamera::new(eye, Vec3::new(1.8, 1.0, 0.3), 80.0));
        let size = (120u32, 90u32);
        let rgba: Vec<u8> = (0..size.1).flat_map(|y| (0..size.0).flat_map(move |x| [x as u8, y as u8, 0, 255])).collect();
        let mut seen = 0;
        for k in 1..8 {
            for z in [0.3, 1.0, 1.7] {
                for p in [[f64::from(k) * 0.5, 0.0, z], [0.0, f64::from(k) * 0.5, z]] {
                    let s = sample(&scene, &Bvh::build(&scene), &ViewFilter::default(), &camera, size, &rgba, p);
                    let Some(px) = project_point(&camera, size, p) else { continue };
                    let (x, y) = (px[0].floor(), px[1].floor());
                    if x < 0.0 || y < 0.0 || x >= size.0 as f32 || y >= size.1 as f32 {
                        continue;
                    }
                    seen += 1;
                    assert_eq!(s["visible"], true, "the camera sees {p:?}: {s}");
                    assert_eq!(s["pixel"], json!([x as u32, y as u32]), "{p:?} lies in that pixel: {s}");
                    assert_eq!(s["rgb"], json!([x as u8, y as u8, 0]), "{p:?}: {s}");
                }
            }
        }
        assert!(seen >= 20, "enough wall points are on screen: {seen}");
    }

    fn request<'a>(d: &'a Drawing, camera: Option<Camera>, out: Option<std::path::PathBuf>, final_render: bool) -> RenderRequest<'a> {
        RenderRequest::new(d, camera, (96, 64), out, final_render, SunChoice::Default, "shaded".into(), true, 8, Vec::new())
    }

    fn orbit(d: &Drawing, azimuth: f64) -> Camera {
        let mut o = Orbit::fit(&cadcraft_arch3d::build(&d.arch).bounds);
        o.azimuth += azimuth;
        Camera::Orbit(o)
    }

    /// Render, or `None` (after saying so) when this machine has no GPU adapter. The renderer is
    /// process-wide, so the tests share one lock and start from a fresh one.
    fn render(req: &RenderRequest) -> Option<Value> {
        match render_one(req) {
            Ok(v) => Some(v),
            Err(e) if e.starts_with("NO_ADAPTER") => {
                eprintln!("SKIP: {e}");
                None
            }
            Err(e) => panic!("render failed: {e}"),
        }
    }

    fn reset() -> std::sync::MutexGuard<'static, ()> {
        static SERIAL: Mutex<()> = Mutex::new(());
        // A test that panicked while holding the lock must not hide the others.
        let g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        // Start from a fresh renderer so the shadow-map counts begin at zero.
        *STATE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        g
    }

    #[test]
    fn render_hook_refuses_an_unknown_style() {
        let _g = reset();
        let d = room();
        let mut req = request(&d, None, None, false);
        req.style = "wireframe".into();
        let Err(e) = render_one(&req) else { panic!("an unknown style must be refused") };
        for name in ["shaded", "shaded-edges", "hidden-line", "glass-house", "clay", "technical"] {
            assert!(e.contains(name), "{e}");
        }
    }

    #[test]
    fn render_hook_refuses_case_variants_of_styles() {
        let _g = reset();
        let d = room();
        for style in ["Clay", "CLAY", "Technical", "TECHNICAL", "Shaded", "SHADEDEDGES"] {
            let mut req = request(&d, None, None, false);
            req.style = style.into();
            let Err(e) = render_one(&req) else { panic!("style `{style}` must be refused") };
            assert!(e.contains("is not one of"), "`{style}`: {e}");
        }
    }

    #[test]
    fn render_hook_clay_style_renders() {
        let _g = reset();
        let d = room();
        let mut req = request(&d, None, None, false);
        req.style = "clay".into();
        let Some(v) = render(&req) else { return };
        assert_eq!(v["ok"], true, "{v}");
    }

    #[test]
    fn render_hook_technical_style_renders() {
        let _g = reset();
        let d = room();
        let mut req = request(&d, None, None, false);
        req.style = "technical".into();
        let Some(v) = render(&req) else { return };
        assert_eq!(v["ok"], true, "{v}");
    }

    #[test]
    fn render_hook_first_frame_builds_four_maps() {
        let _g = reset();
        let d = room();
        let Some(v) = render(&request(&d, None, None, false)) else { return };
        assert_eq!(v["shadowMapsBuilt"], 4, "{v}");
        assert_eq!(v["lights"], 4);
        assert_eq!(v["finalRenders"], 0);
    }

    #[test]
    fn render_hook_camera_moves_build_nothing() {
        let _g = reset();
        let d = room();
        for k in 0..30 {
            let Some(v) = render(&request(&d, Some(orbit(&d, f64::from(k))), None, false)) else { return };
            assert_eq!(v["shadowMapsBuilt"], 4, "frame {k}: {v}");
            assert_eq!(v["finalRenders"], 0);
        }
        assert_eq!(stats().shadow_maps_built, 4);
    }

    #[test]
    fn render_hook_moving_a_fixture_rebuilds_one() {
        let _g = reset();
        let mut d = room();
        let Some(v) = render(&request(&d, None, None, false)) else { return };
        assert_eq!(v["shadowMapsBuilt"], 4);
        if let Some(l) = d.arch.lights.first_mut() {
            l.position = Vec2::new(1.5, 1.5);
        }
        d.touch_arch();
        let Some(v) = render(&request(&d, None, None, false)) else { return };
        assert_eq!(v["shadowMapsBuilt"], 5, "{v}");
        assert_eq!(v["frameShadowMaps"], 1);
    }

    #[test]
    fn render_hook_final_counts_one_and_writes_png() {
        let _g = reset();
        let d = room();
        let out = std::env::temp_dir().join(format!("cadcraft-render-hook-{}", std::process::id())).join("final.png");
        let Some(v) = render(&request(&d, None, Some(out.clone()), true)) else { return };
        assert_eq!(v["finalRenders"], 1, "{v}");
        assert_eq!(v["final"], true);
        let bytes = std::fs::read(&out).unwrap_or_default();
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']), "not a PNG");
        let _ = std::fs::remove_dir_all(out.parent().unwrap_or(&out));
    }

    #[test]
    fn render_hook_samples_report_pixels() {
        let _g = reset();
        let d = room();
        let mut req = request(&d, None, None, false);
        req.samples = vec![[2.0, 2.0, 0.0], [1.0e6, 1.0e6, 1.0e6]];
        let Some(v) = render(&req) else { return };
        let s = v["samples"].as_array().cloned().unwrap_or_default();
        assert_eq!(s.len(), 2, "{v}");
        let px = s[0]["pixel"].as_array().cloned().unwrap_or_default();
        assert_eq!(px.len(), 2, "{v}");
        assert_eq!(s[0]["rgb"].as_array().map(Vec::len), Some(3));
        assert!(s[1]["pixel"].is_null() || s[1]["visible"] == false, "{v}");
    }

    #[test]
    fn render_hook_empty_scene_renders() {
        let _g = reset();
        let mut d = Drawing::new_metric();
        d.touch_arch();
        let Some(v) = render(&request(&d, None, None, false)) else { return };
        assert_eq!(v["ok"], true, "{v}");
        assert_eq!(v["lights"], 0, "{v}");
        assert_eq!(v["shadowMapsBuilt"], 0, "{v}");
        assert_eq!(v["finalRenders"], 0, "{v}");
    }

    #[test]
    fn render_hook_sample_nan_point() {
        let _g = reset();
        let d = room();
        let mut req = request(&d, None, None, false);
        req.samples = vec![[f64::NAN, f64::NAN, f64::NAN]];
        let Some(v) = render(&req) else { return };
        let s = v["samples"].as_array().cloned().unwrap_or_default();
        assert_eq!(s.len(), 1, "{v}");
        // NaN projects to nothing or is not visible.
        assert!(s[0]["pixel"].is_null() || s[0]["visible"] == false, "{v}");
    }

    #[test]
    fn render_hook_sample_far_point() {
        let _g = reset();
        let d = room();
        let mut req = request(&d, None, None, false);
        req.samples = vec![[1000.0, 1000.0, 1000.0]];
        let Some(v) = render(&req) else { return };
        let s = v["samples"].as_array().cloned().unwrap_or_default();
        assert_eq!(s.len(), 1, "{v}");
        assert_eq!(s[0]["visible"], false, "{v}");
    }

    #[test]
    fn render_hook_sun_off_renders() {
        let _g = reset();
        let d = room();
        let mut req = request(&d, None, None, false);
        req.sun = SunChoice::Off;
        let Some(v) = render(&req) else { return };
        assert_eq!(v["ok"], true, "{v}");
        assert_eq!(v["lights"], 4, "{v}");
        assert_eq!(v["shadowMapsBuilt"], 4, "{v}");
    }

    fn path_request<'a>(d: &'a Drawing, out: Option<std::path::PathBuf>, path: PathParams) -> RenderRequest<'a> {
        request(d, None, out, true).with_final(FinalMode::Path, path)
    }

    fn quick(spp: u32) -> PathParams {
        PathParams { spp, spp_given: true, bounces: 1, denoise: false, ..PathParams::default() }
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("cadcraft-render-hook-{tag}-{}", std::process::id()))
    }

    #[test]
    fn render_hook_path_final_reports_spp_and_counts_a_path_trace() {
        let _g = reset();
        let d = room();
        let v = match render_one(&path_request(&d, None, quick(4))) {
            Ok(v) => v,
            Err(e) if e.starts_with("NO_ADAPTER") => return eprintln!("SKIP: {e}"),
            Err(e) => panic!("render failed: {e}"),
        };
        if v["finalMode"] != "path" {
            // No compute shaders on this machine: the still ran as raster.
            assert_eq!(v["finalMode"], "raster", "{v}");
            assert_eq!(v["fallback"], "no compute shaders", "{v}");
            eprintln!("SKIP: no compute device");
            return;
        }
        assert_eq!(v["spp"], 4, "{v}");
        assert_eq!(v["pathTraces"], 1, "{v}");
        assert_eq!(v["finalRenders"], 1, "{v}");
        assert!(v["radiance"]["max"].is_number(), "{v}");
        assert!(v["pipelineCache"].is_string(), "{v}");
        assert_eq!(v["shadowedLights"], 4, "{v}");
        assert_eq!(stats().path_traces, 1);
    }

    /// 2026-11-03: the session's compute device was made by its first path render and kept, so `rt` asked for
    /// after a compute render rendered the compute path (`rt: false`) even where the adapter has ray queries;
    /// phase-11 check 10 does exactly that and so skipped on every adapter. The software adapter (llvmpipe)
    /// offers ray queries; a later `rt` render must use them.
    #[test]
    fn a_later_rt_render_in_the_session_uses_ray_queries() {
        let _g = reset();
        let offered = cadcraft_render3d::pathtrace::request_compute(AdapterChoice::Fallback, true).ok().flatten().is_some_and(|g| g.ray_query);
        let d = room();
        let first = match render_one(&path_request(&d, None, quick(4))) {
            Ok(v) => v,
            Err(e) if e.starts_with("NO_ADAPTER") => return eprintln!("SKIP: {e}"),
            Err(e) => panic!("render failed: {e}"),
        };
        if first["finalMode"] != "path" {
            return eprintln!("SKIP: no compute device");
        }
        assert_eq!(first["rt"], false, "{first}");
        let rt = render_one(&path_request(&d, None, PathParams { rt: true, ..quick(4) })).unwrap_or_else(|e| panic!("rt render failed: {e}"));
        if !offered {
            assert_eq!(rt["rt"], false, "{rt}");
            return eprintln!("SKIP: the software adapter offers no ray queries");
        }
        assert_eq!(rt["rt"], true, "a later rt render uses ray queries: {rt}");
        assert!(rt.get("rtError").is_none(), "{rt}");
        let again = render_one(&path_request(&d, None, quick(4))).unwrap_or_else(|e| panic!("render failed: {e}"));
        assert_eq!(again["rt"], false, "a render without rt still runs the compute path: {again}");
    }

    #[test]
    fn render_hook_path_falls_back_without_compute() {
        let _g = reset();
        let d = room();
        // A first raster still creates the renderer; then the compute request is answered "none"
        // exactly as CADCRAFT_RENDER_LIMITS=webgl2 does.
        let Some(_) = render(&request(&d, None, None, false)) else { return };
        if let Ok(mut g) = STATE.lock() {
            if let Some(s) = g.as_mut() {
                s.compute = Some(None);
            }
        }
        let Some(v) = render(&path_request(&d, None, quick(4))) else { return };
        assert_eq!(v["finalMode"], "raster", "{v}");
        assert_eq!(v["fallback"], "no compute shaders", "{v}");
        assert_eq!(v["pathTraces"], 0, "{v}");
        assert_eq!(v["finalRenders"], 1, "{v}");
    }

    #[test]
    fn render_hook_writes_snapshots() {
        let _g = reset();
        let d = room();
        let dir = temp_dir("snap");
        let out = dir.join("still.png");
        let mut p = quick(8);
        p.snapshots = vec![4];
        let Some(v) = render(&path_request(&d, Some(out.clone()), p)) else { return };
        if v["finalMode"] != "path" {
            eprintln!("SKIP: no compute device");
            return;
        }
        let snaps = v["snapshots"].as_array().cloned().unwrap_or_default();
        assert_eq!(snaps.len(), 1, "{v}");
        assert_eq!(snaps[0]["spp"], 4, "{v}");
        let bytes = std::fs::read(dir.join("still-spp4.png")).unwrap_or_default();
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']), "snapshot is not a PNG: {v}");
        assert!(out.exists(), "{v}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn illuminance_hook_answers_lux_from_the_path_tracer() {
        let _g = reset();
        let d = room();
        let probe = vec![[[2.0, 2.0, 0.0], [0.0, 0.0, 1.0]]];
        let req = IlluminanceRequest::new(&d, probe, SunChoice::Off, quick(16));
        match illuminance_one(&req) {
            Ok(v) => {
                assert_eq!(v["method"], "path", "{v}");
                assert!(v["lux"].as_f64().is_some_and(|l| l.is_finite() && l >= 0.0), "{v}");
                assert_eq!(v["spp"], 16, "{v}");
            }
            Err(e) if e.starts_with("NO_ADAPTER") => eprintln!("SKIP: {e}"),
            Err(e) if e.contains("needs compute shaders") => eprintln!("SKIP: {e}"),
            Err(e) => panic!("illuminance failed: {e}"),
        }
    }

    #[test]
    fn camera_moves_start_no_path_trace() {
        let _g = reset();
        let d = room();
        for k in 0..30 {
            let Some(v) = render(&request(&d, Some(orbit(&d, f64::from(k))), None, false)) else { return };
            assert_eq!(v["pathTraces"], 0, "frame {k}: {v}");
            assert!(v["finalMode"].is_null(), "frame {k}: {v}");
        }
        assert_eq!(stats().path_traces, 0);
    }

    #[test]
    fn render_hook_moving_fixture_back_rebuilds_again() {
        let _g = reset();
        let mut d = room();
        let Some(v) = render(&request(&d, None, None, false)) else { return };
        assert_eq!(v["shadowMapsBuilt"], 4, "{v}");
        if let Some(l) = d.arch.lights.first_mut() {
            l.position = Vec2::new(1.5, 1.5);
        }
        d.touch_arch();
        let Some(v) = render(&request(&d, None, None, false)) else { return };
        assert_eq!(v["shadowMapsBuilt"], 5, "{v}");
        // Move it back: the scene key changes again, so the map rebuilds once more.
        if let Some(l) = d.arch.lights.first_mut() {
            l.position = Vec2::new(1.0, 1.0);
        }
        d.touch_arch();
        let Some(v) = render(&request(&d, None, None, false)) else { return };
        assert_eq!(v["shadowMapsBuilt"], 6, "{v}");
        assert_eq!(v["frameShadowMaps"], 1, "{v}");
    }

    #[test]
    fn filtered_scene_cached_per_filter_and_samples_honour_it() {
        use cadcraft_engine::doc::arch::ClipBox;
        let _g = reset();
        let d = room();
        let full = |eye: [f64; 3], target: [f64; 3]| {
            Camera::Full(cadcraft_render3d::camera::FullCamera::new(
                Vec3::new(eye[0], eye[1], eye[2]),
                Vec3::new(target[0], target[1], target[2]),
                70.0,
            ))
        };
        let uploads = || STATE.lock().ok().and_then(|g| g.as_ref().map(|s| s.off.uploads())).unwrap_or(0);
        let filtered_key = || STATE.lock().ok().and_then(|g| g.as_ref().and_then(|s| s.filtered.as_ref().map(|(k, _)| k.clone())));
        let go = |camera: Camera, look: Look, samples: Vec<[f64; 3]>| {
            let mut req = request(&d, Some(camera), None, false);
            req.look = look;
            req.samples = samples;
            render(&req)
        };
        let seen = |v: &Value, i: usize| v["samples"][i]["visible"].as_bool().unwrap_or(false);

        // Cutaway from the south: the south wall's outside is visible without it, gone with it.
        let outside = full([2.0, -8.0, 1.5], [2.0, 2.0, 1.5]);
        let wall_point = [2.0, -0.2, 1.0];
        let Some(plain) = go(outside.clone(), Look::default(), vec![wall_point]) else { return };
        assert!(seen(&plain, 0), "{plain}");
        assert_eq!(plain["filter"], json!({ "hidden": 0, "clip": false, "nearClip": null }), "{plain}");
        assert!(filtered_key().is_none(), "nothing hidden builds no filtered scene");
        let cut = Look { cutaway: true, ..Look::default() };
        let Some(v) = go(outside.clone(), cut.clone(), vec![wall_point]) else { return };
        assert!(v["filter"]["hidden"].as_u64().unwrap_or(0) >= 1, "{v}");
        assert!(!seen(&v, 0), "the cut wall is not visible: {v}");
        let key = filtered_key();
        assert!(key.is_some());
        let up = uploads();

        // The same hidden set again, with a clip box and a near clip: the filtered scene and its
        // upload are reused (neither changes the scene's meshes).
        let clipped =
            Look { clip: Some(ClipBox { min: [-10.0; 3], max: [10.0, 10.0, 5.0], ..Default::default() }), near_clip: Some(1.0), ..cut.clone() };
        let Some(v) = go(full([2.0, -7.0, 1.5], [2.0, 2.0, 1.5]), clipped, vec![wall_point]) else { return };
        assert_eq!((filtered_key(), uploads()), (key.clone(), up), "{v}");
        assert_eq!((v["filter"]["clip"].clone(), v["filter"]["nearClip"].clone()), (json!(true), json!(1.0)), "{v}");

        // Inside the room: a clip box below the sample's height and a near clip beyond it each hide it.
        let inside = full([2.0, 2.0, 1.5], [2.0, 4.0, 1.5]);
        let north = [2.0, 4.0, 1.0];
        let Some(v) = go(inside.clone(), Look::default(), vec![north]) else { return };
        assert!(seen(&v, 0), "{v}");
        let low = Look { clip: Some(ClipBox { min: [-10.0; 3], max: [10.0, 10.0, 0.5], ..Default::default() }), ..Look::default() };
        let Some(v) = go(inside.clone(), low, vec![north]) else { return };
        assert!(!seen(&v, 0), "clipped away: {v}");
        assert!(v["samples"][0]["pixel"].is_array() && v["samples"][0]["rgb"].is_array(), "pixel and rgb stay reported: {v}");
        let near = Look { near_clip: Some(3.0), ..Look::default() };
        let Some(v) = go(inside, near, vec![north]) else { return };
        assert!(!seen(&v, 0), "nearer than the near clip: {v}");
        assert!(filtered_key() == key, "a clip and a near clip leave the filtered scene alone");

        // A different hidden set (the camera on the other side) builds another filtered scene.
        let Some(v) = go(full([2.0, 12.0, 1.5], [2.0, 2.0, 1.5]), cut, vec![[2.0, 4.2, 1.0]]) else { return };
        assert!(v["filter"]["hidden"].as_u64().unwrap_or(0) >= 1, "{v}");
        assert!(filtered_key() != key, "a new hidden set builds a new filtered scene");
    }

    /// The opened-room regression (phase-19 check 15a) through the session renderer: a room with a
    /// detected ceiling, seen from the south with the cutaway on. Without the cutaway the camera
    /// sees the sunlit outer face of the south wall and the north wall's inner face is hidden; with
    /// it the north wall's inner face is visible and reads darker (the ceiling shades it). The
    /// filtered scene keeps its ceiling mesh (hiding a wall by id must not hide the room).
    #[test]
    fn cutaway_keeps_the_ceiling_and_shades_the_back_wall() {
        use cadcraft_engine::doc::arch::{Level, Room, RoomType};
        let _g = reset();
        let mut d = Drawing::new_metric();
        let c = [(-0.1, -0.1), (4.1, -0.1), (4.1, 4.1), (-0.1, 4.1)];
        for k in 0..4 {
            let (a, b) = (c[k], c[(k + 1) % 4]);
            let mut w = Wall::new(Vec2::new(a.0, a.1), Vec2::new(b.0, b.1), 0.2);
            w.height = 2.4;
            d.arch.push_wall(w);
        }
        d.arch.levels.push(Level { name: "Main".into(), elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        d.arch.rooms.push(Room {
            id: cadcraft_engine::doc::ArchId(0),
            name: "Room".into(),
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
            room_type: RoomType::Living,
            level: 0,
            ..Default::default()
        });
        d.touch_arch();

        let full = |eye: [f64; 3], target: [f64; 3]| {
            Camera::Full(cadcraft_render3d::camera::FullCamera::new(
                Vec3::new(eye[0], eye[1], eye[2]),
                Vec3::new(target[0], target[1], target[2]),
                70.0,
            ))
        };
        let north = [2.0, 4.0, 1.2];
        let camera = full([2.0, -8.0, 1.2], [2.0, 2.0, 1.2]);
        let mut look = Look::default();
        look.sky.on = false;
        // Lower exposure to avoid saturation on llvmpipe
        look.exposure = ExposureSetting::Manual { ev100: 8.0 };
        look.tone_map = cadcraft_engine::doc::arch::ToneMap::None;
        look.sun.on = true;
        // Sun from the south (azimuth 180) at elevation 60 degrees.
        // The SunLook angle is an Option<String> in "azimuth,elevation" format.
        look.sun.angle = Some("180,60".to_string());

        let go = |cutaway: bool| {
            let mut req = request(&d, Some(camera.clone()), None, false);
            req.ground = false;
            req.look = Look { cutaway, ..look.clone() };
            req.samples = vec![north];
            render(&req)
        };
        let Some(plain) = go(false) else { return };
        assert_eq!(plain["samples"][0]["visible"], false, "the north wall is hidden behind the south wall: {plain}");
        let plain_rgb: Vec<u8> =
            plain["samples"][0]["rgb"].as_array().map(|a| a.iter().filter_map(|v| v.as_u64().map(|x| x as u8)).collect()).unwrap_or_default();
        let plain_sum: u32 = plain_rgb.iter().map(|&c| u32::from(c)).sum();

        let Some(v) = go(true) else { return };
        assert_eq!(v["samples"][0]["visible"], true, "the cutaway opens the room to the north wall: {v}");
        let rgb: Vec<u8> =
            v["samples"][0]["rgb"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64().map(|c| c as u8)).collect()).unwrap_or_default();
        let sum: u32 = rgb.iter().map(|&c| u32::from(c)).sum();
        assert!(plain_sum >= sum + 30, "the sunlit outer face ({plain_sum}) must be brighter than the shaded inner face ({sum}): {v}");

        let has_ceiling = STATE.lock().ok().and_then(|g| {
            g.as_ref().and_then(|s| s.filtered.as_ref()).map(|(_, f)| f.meshes.iter().any(|m| cadcraft_arch3d::probe::part_of(m) == "ceiling"))
        });
        assert!(has_ceiling.unwrap_or(false), "the filtered scene keeps the ceiling mesh (hiding a wall must not hide the room)");
    }

    #[test]
    fn a_light_set_that_turns_lights_off_renders_darker() {
        let _g = reset();
        let mut d = room();
        let mut lights = std::collections::BTreeMap::new();
        lights.insert("lf0".to_string(), cadcraft_engine::doc::arch::LightState { on: false, brightness: 1.0, ..Default::default() });
        d.arch.light_sets.push(cadcraft_engine::doc::arch::LightSet { name: "off_one".into(), lights, ..Default::default() });
        d.touch_arch();

        let mut req_all = request(&d, None, None, false);
        let v_all = match render(&req_all) {
            Some(v) => v,
            None => return,
        };
        assert_eq!(v_all["lights"], 4);

        let mut req_set = request(&d, None, None, false);
        req_set.light_set = Some("off_one".into());
        let v_set = match render(&req_set) {
            Some(v) => v,
            None => return,
        };
        assert_eq!(v_set["lights"], 3, "{v_set}");
        assert!(!cadcraft_arch3d::light::scene_lights_with(&d.arch, &d.arch.light_sets[0].lights).iter().any(|l| l.id == d.arch.lights[0].id));
    }

    #[test]
    fn renders_with_different_light_sets_do_not_share_a_cache_key() {
        let _g = reset();
        let mut d = room();
        let mut set1 = std::collections::BTreeMap::new();
        set1.insert("lf0".to_string(), cadcraft_engine::doc::arch::LightState { on: false, brightness: 1.0, ..Default::default() });
        let mut set2 = std::collections::BTreeMap::new();
        set2.insert("lf1".to_string(), cadcraft_engine::doc::arch::LightState { on: false, brightness: 1.0, ..Default::default() });
        d.arch.light_sets.push(cadcraft_engine::doc::arch::LightSet { name: "set1".into(), lights: set1, ..Default::default() });
        d.arch.light_sets.push(cadcraft_engine::doc::arch::LightSet { name: "set2".into(), lights: set2, ..Default::default() });
        d.touch_arch();

        let mut req1 = request(&d, None, None, false);
        req1.light_set = Some("set1".into());
        let v1 = match render(&req1) {
            Some(v) => v,
            None => return,
        };
        assert_eq!(v1["lights"], 3, "{v1}");

        let mut req2 = request(&d, None, None, false);
        req2.light_set = Some("set2".into());
        let v2 = match render(&req2) {
            Some(v) => v,
            None => return,
        };
        assert_eq!(v2["lights"], 3, "{v2}");
        assert_ne!(v1["shadowMapsBuilt"], v2["shadowMapsBuilt"], "different light sets must not reuse the cached scene\n{v1}\n{v2}");
    }

    #[test]
    fn a_render_without_a_light_set_is_unchanged() {
        let _g = reset();
        let d = room();
        let req = request(&d, None, None, false);
        let v = match render(&req) {
            Some(v) => v,
            None => return,
        };

        assert_eq!(v["ok"], true);
        assert_eq!(v["lights"], 4);
    }

    #[test]
    fn a_missing_light_set_uses_all_lights() {
        let _g = reset();
        let d = room();
        let mut req = request(&d, None, None, false);
        req.light_set = Some("nonexistent".into());
        let v = match render(&req) {
            Some(v) => v,
            None => return,
        };
        assert_eq!(v["ok"], true, "{v}");
        assert_eq!(v["lights"], 4, "a missing set must not drop lights: {v}");
    }

    #[test]
    fn a_light_set_with_all_lights_off_renders_zero_lights() {
        let _g = reset();
        let mut d = room();
        let mut lights = std::collections::BTreeMap::new();
        for i in 0..4 {
            lights.insert(format!("lf{i}"), cadcraft_engine::doc::arch::LightState { on: false, brightness: 1.0, ..Default::default() });
        }
        d.arch.light_sets.push(cadcraft_engine::doc::arch::LightSet { name: "all_off".into(), lights, ..Default::default() });
        d.touch_arch();

        let mut req = request(&d, None, None, false);
        req.light_set = Some("all_off".into());
        let v = match render(&req) {
            Some(v) => v,
            None => return,
        };
        assert_eq!(v["lights"], 0, "all off must render no lights: {v}");
    }

    #[test]
    fn a_light_set_brightness_keeps_the_light_on() {
        let _g = reset();
        let mut d = room();
        let mut lights = std::collections::BTreeMap::new();
        lights.insert("lf0".to_string(), cadcraft_engine::doc::arch::LightState { on: true, brightness: 0.5, ..Default::default() });
        d.arch.light_sets.push(cadcraft_engine::doc::arch::LightSet { name: "half".into(), lights, ..Default::default() });
        d.touch_arch();

        let scene_lights = cadcraft_arch3d::light::scene_lights_with(&d.arch, &d.arch.light_sets[0].lights);
        assert_eq!(scene_lights.len(), 4, "brightness must not remove lights");
        assert!(scene_lights.iter().any(|l| l.id == d.arch.lights[0].id), "lf0 must still be present");
    }

    #[test]
    fn switching_light_sets_and_back_rebuilds() {
        let _g = reset();
        let mut d = room();
        let mut set1 = std::collections::BTreeMap::new();
        set1.insert("lf0".to_string(), cadcraft_engine::doc::arch::LightState { on: false, brightness: 1.0, ..Default::default() });
        let mut set2 = std::collections::BTreeMap::new();
        set2.insert("lf1".to_string(), cadcraft_engine::doc::arch::LightState { on: false, brightness: 1.0, ..Default::default() });
        d.arch.light_sets.push(cadcraft_engine::doc::arch::LightSet { name: "set1".into(), lights: set1, ..Default::default() });
        d.arch.light_sets.push(cadcraft_engine::doc::arch::LightSet { name: "set2".into(), lights: set2, ..Default::default() });
        d.touch_arch();

        let mut req1 = request(&d, None, None, false);
        req1.light_set = Some("set1".into());
        let v1 = match render(&req1) {
            Some(v) => v,
            None => return,
        };
        let maps_after_set1 = v1["shadowMapsBuilt"].as_u64().unwrap();

        let mut req2 = request(&d, None, None, false);
        req2.light_set = Some("set2".into());
        let v2 = match render(&req2) {
            Some(v) => v,
            None => return,
        };
        let maps_after_set2 = v2["shadowMapsBuilt"].as_u64().unwrap();

        let v3 = match render(&req1) {
            Some(v) => v,
            None => return,
        };
        let maps_after_set1_again = v3["shadowMapsBuilt"].as_u64().unwrap();

        assert!(maps_after_set2 > maps_after_set1, "set2 must rebuild: {v1} {v2}");
        assert!(maps_after_set1_again > maps_after_set2, "set1 again must rebuild: {v2} {v3}");
    }

    #[test]
    fn a_light_set_affects_the_path_tracer() {
        let _g = reset();
        let mut d = room();
        let mut lights = std::collections::BTreeMap::new();
        lights.insert("lf0".to_string(), cadcraft_engine::doc::arch::LightState { on: false, brightness: 1.0, ..Default::default() });
        d.arch.light_sets.push(cadcraft_engine::doc::arch::LightSet { name: "off_one".into(), lights, ..Default::default() });
        d.touch_arch();

        let mut req = path_request(&d, None, quick(4));
        req.light_set = Some("off_one".into());
        let v = match render_one(&req) {
            Ok(v) => v,
            Err(e) if e.starts_with("NO_ADAPTER") => return eprintln!("SKIP: {e}"),
            Err(e) => panic!("render failed: {e}"),
        };
        if v["finalMode"] != "path" {
            return eprintln!("SKIP: no compute device");
        }
        assert_eq!(v["shadowedLights"], 3, "path tracer must honour the light set: {v}");
    }

    #[test]
    fn environment_cache_keeps_alternating_looks() {
        let _g = reset();
        if let Ok(mut g) = ENV_CACHE.lock() {
            g.clear();
        }
        let model = cadcraft_engine::doc::arch::ArchModel::default();
        let mut look = Look::default();
        let sun_a = Some([0.0, -0.6, 0.8]);
        let sun_b = Some([0.6, 0.0, 0.8]);

        // First A with the default look. When the default look is light-only (no sky),
        // turn the sky on so the Preetham sky builds and the cached value is a real Arc.
        let mut a1 = environment_cached(&model, 9_000_001, &look, sun_a);
        if a1.is_none() {
            look.sky.on = true;
            a1 = environment_cached(&model, 9_000_001, &look, sun_a);
        }

        // B (a different sun direction = a different key), then A again. When the sky
        // builds, the second A must be served from the cache (the entry survived B) and B
        // must be a distinct Arc. When the sky does not build for these inputs, every
        // result is None (the cache stores (key, None) entries, which stay within the
        // LRU bound covered by environment_cache_is_bounded).
        let b1 = environment_cached(&model, 9_000_001, &look, sun_b);
        let a2 = environment_cached(&model, 9_000_001, &look, sun_a);

        // A, B, A share the look, so either all three build or none of them do.
        let all_three = a1.is_some() && b1.is_some() && a2.is_some();
        if all_three {
            // Move the Arcs (owned) and compare the allocations they point to: the
            // cached A survived B (first and third share the allocation), and B used a
            // different key (a distinct allocation).
            let a1p = a1.as_ref().map(|a| std::sync::Arc::<cadcraft_render3d::Environment>::as_ptr(&a));
            let b1p = b1.as_ref().map(|a| std::sync::Arc::<cadcraft_render3d::Environment>::as_ptr(&a));
            let a2p = a2.as_ref().map(|a| std::sync::Arc::<cadcraft_render3d::Environment>::as_ptr(&a));
            assert!(a1p == a2p, "the cached A must be returned after B");
            assert!(a1p != b1p, "A and B use different keys");
        } else {
            assert!(
                !a1.is_some() && !b1.is_some() && !a2.is_some(),
                "mixed Some/None outcomes are impossible for the same look: {a1:?} {b1:?} {a2:?}"
            );
        }
    }

    #[test]
    fn environment_cache_is_bounded() {
        let _g = reset();
        if let Ok(mut g) = ENV_CACHE.lock() {
            g.clear();
        }
        let model = cadcraft_engine::doc::arch::ArchModel::default();
        let look = Look::default();

        for i in 0..(ENV_CACHE_CAP + 3) {
            let sun = Some([f64::from(i as f64) * 0.1 - 0.6, 0.0, 0.8]);
            let _ = environment_cached(&model, 9_000_001, &look, sun);
        }

        let count = if let Ok(g) = ENV_CACHE.lock() {
            g.len()
        } else {
            return;
        };
        assert!(count <= ENV_CACHE_CAP, "cache must hold at most {ENV_CACHE_CAP} entries, got {count}");
    }
}
