//! The session renderer behind `render` (3D), `sunstudy` and the MCP tools: one offscreen GPU
//! renderer kept for the whole process, so the mesh upload and the shadow maps are reused from one
//! still to the next. A call that only moves the camera builds no shadow map.

use std::sync::Mutex;

use cadcraft_arch3d::camera::{project_point, ray_at};
use cadcraft_engine::{RenderHooks, RenderRequest, RenderStats, SunChoice, set_render_hooks};
use cadcraft_geom::Vec3;
use cadcraft_render3d::adapter::AdapterChoice;
use cadcraft_render3d::camera::{Camera, Orbit};
use cadcraft_render3d::light::{Light, Sun};
use cadcraft_render3d::lights::DEFAULT_EXPOSURE;
use cadcraft_render3d::offscreen::{Offscreen, RenderOptions, Style, png_rgba};
use cadcraft_render3d::{Lighting, Scene3d, render_final};
use serde_json::{Value, json};

/// The most sample points a request reports.
const MAX_SAMPLES: usize = 64;
/// How near (m) the pick ray's first hit must be to a sample point for it to count as visible.
const VISIBLE_TOLERANCE: f64 = 0.02;

/// The renderer and the scene it last built.
struct State {
    off: Offscreen,
    /// The `arch_revision` the scene below was built for, and the scene.
    scene: Option<(u64, Scene3d)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Install the renderer as the engine's render hook (idempotent: the first call wins).
pub(crate) fn install() {
    set_render_hooks(RenderHooks::new(render_one, stats));
}

fn stats() -> RenderStats {
    let Ok(guard) = STATE.lock() else { return RenderStats::default() };
    let Some(s) = guard.as_ref() else { return RenderStats::default() };
    let mut r = RenderStats::default();
    r.shadow_maps_built = s.off.shadow_maps_built();
    r.final_renders = s.off.final_renders();
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

fn render_one(req: &RenderRequest) -> Result<Value, String> {
    let mut guard = STATE.lock().map_err(|_| "renderer unavailable".to_string())?;
    if guard.is_none() {
        let off = Offscreen::create_with(adapter_choice(), req.size).map_err(gpu)?;
        *guard = Some(State { off, scene: None });
    }
    let state = guard.as_mut().ok_or("renderer unavailable")?;
    let key = req.drawing.arch_revision;
    if state.scene.as_ref().is_none_or(|(k, _)| *k != key) {
        state.scene = Some((key, cadcraft_arch3d::build(&req.drawing.arch)));
    }
    let State { off, scene } = state;
    let scene = &scene.as_ref().ok_or("renderer unavailable")?.1;
    off.resize(req.size);

    let camera = req.camera.clone().unwrap_or_else(|| Camera::Orbit(Orbit::fit(&scene.bounds)));
    let eye = camera.eye();
    let sun = match req.sun {
        SunChoice::Default => Some(Light::DEFAULT),
        SunChoice::Off => None,
        SunChoice::Dir(d) => Some(Light::from_sun(&Sun { dir: [d[0] as f32, d[1] as f32, d[2] as f32], ..Sun::default() }, Light::DEFAULT.ambient)),
    };
    let lighting =
        Lighting { sun, sun_shadow: matches!(req.sun, SunChoice::Dir(_)), budget: req.budget, exposure: DEFAULT_EXPOSURE, ..Lighting::default() };
    let maps_before = off.shadow_maps_built();

    let (rgba, size, drawn, shadowed, png) = if req.final_render {
        let png = render_final(off, scene, key, &camera, &lighting, req.ground, req.size).map_err(gpu)?;
        let rgba = png_rgba(&png).map_err(gpu)?;
        (rgba, req.size, scene.lights.len(), scene.lights.len(), Some(png))
    } else {
        let frame = off.lit_frame(scene, key, [eye.x, eye.y, eye.z], &lighting, req.ground);
        let style = crate::render::parse_style(&req.style).unwrap_or(Style::Shaded);
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
        if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
        }
        crate::output::write_atomic(out, png).map_err(|e| e.to_string())?;
    }

    let samples: Vec<Value> = req.samples.iter().take(MAX_SAMPLES).map(|p| sample(scene, &camera, size, &rgba, *p)).collect();
    Ok(json!({
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
    }))
}

/// One sample: the pixel `p` projects to, the colour there and whether the camera sees `p`.
fn sample(scene: &Scene3d, camera: &Camera, size: (u32, u32), rgba: &[u8], p: [f64; 3]) -> Value {
    let pixel = project_point(camera, size, p).and_then(|px| {
        let (x, y) = (px[0].round(), px[1].round());
        let inside = x.is_finite() && y.is_finite() && x >= 0.0 && y >= 0.0 && x < size.0 as f32 && y < size.1 as f32;
        inside.then_some((x as u32, y as u32))
    });
    let Some((x, y)) = pixel else {
        return json!({ "point": p, "pixel": null, "rgb": null, "visible": false });
    };
    let at = (usize::try_from(y).unwrap_or(0)).saturating_mul(size.0 as usize).saturating_add(x as usize).saturating_mul(4);
    let rgb = rgba.get(at..at.saturating_add(3)).map(<[u8]>::to_vec);
    let visible = ray_at(camera, size, [x as f32, y as f32])
        .and_then(|(origin, dir)| scene.pick(origin, dir))
        .is_some_and(|hit| distance(hit.point, Vec3::new(p[0], p[1], p[2])) <= VISIBLE_TOLERANCE);
    json!({ "point": p, "pixel": [x, y], "rgb": rgb, "visible": visible })
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
}
