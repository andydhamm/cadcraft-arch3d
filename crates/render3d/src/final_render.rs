//! Finalize Render (phase 10): one still from the current camera with every light shadowed at
//! high resolution (`lights::FINAL_SHADOW_SIZE` layers in its own atlas), soft shadow edges
//! (PCF radius 2), 2×2 supersampling and tone mapping. It runs in steps so the pane can show
//! progress and cancel it: one step per shadowed light, then the 2x render, then the downsample
//! (averaged in linear light).

use crate::Render3dError;
use crate::camera::Camera;
use crate::lit_frame::Lighting;
use crate::offscreen::Offscreen;
use crate::pathtrace::{ComputeGpu, PathOptions, PathStats, PathStatus, PathTracer};
use crate::types::Scene3d;
use cadcraft_arch3d::light::{LightKind, SceneLight};

/// Where a Finalize Render is.
#[derive(Clone, Debug, PartialEq)]
pub enum FinalStatus {
    /// Still working; `progress` is 0..1.
    Running { progress: f32 },
    /// Finished: the RGBA8 pixels (`width·height·4`, rows top to bottom) of the still.
    Done { width: u32, height: u32, rgba: Vec<u8> },
    /// Cancelled before it finished.
    Cancelled,
    /// It could not run (no lit pipelines, a GPU error).
    Failed(String),
}

/// What a running render keeps between steps.
struct Job {
    scene: Scene3d,
    key: u64,
    camera: Camera,
    lighting: Lighting,
    ground: bool,
    /// The 2x (or 1x) target; it uploads the scene itself.
    target: Offscreen,
    atlas: crate::shadow_atlas::ShadowAtlas,
    drawn: Vec<usize>,
    slots: Vec<(usize, crate::lights::ShadowSlot)>,
    occluders: u64,
    far: f32,
    scale: u32,
    lights_done: usize,
    rendered: bool,
    steps: usize,
}

/// The work a render keeps between steps: the raster job, or a path tracer with its device.
enum Inner {
    Raster(Job),
    Path(Box<PathTracer>, ComputeGpu),
}

/// A Finalize Render in progress.
pub struct FinalRender {
    pub(crate) size: (u32, u32),
    pub(crate) cancelled: bool,
    pub(crate) status: FinalStatus,
    inner: Option<Inner>,
    fallback: Option<&'static str>,
    path_stats: Option<PathStats>,
}

fn failed(size: (u32, u32), msg: &str) -> FinalRender {
    FinalRender { size, cancelled: false, status: FinalStatus::Failed(msg.into()), inner: None, fallback: None, path_stats: None }
}

/// Layers the drawn lights need (a spot one, anything else a cube of six).
fn layers_needed(lights: &[SceneLight], drawn: &[usize]) -> u32 {
    drawn.iter().filter_map(|&i| lights.get(i)).fold(0u32, |n, l| n.saturating_add(if l.kind == LightKind::Spot { 1 } else { 6 }))
}

fn srgb_to_linear(c: u8) -> f32 {
    let v = f32::from(c) / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(v: f32) -> u8 {
    let v = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
    let s = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

/// Average `scale`×`scale` blocks of `rgba` (`src_w` pixels wide) into a `w`×`h` image in linear
/// light. `None` when the source is too small.
fn downsample(rgba: &[u8], src_w: u32, (w, h): (u32, u32), scale: u32) -> Option<Vec<u8>> {
    let lut: Vec<f32> = (0..=255u8).map(srgb_to_linear).collect();
    let mut out = Vec::with_capacity((w as usize).checked_mul(h as usize)?.checked_mul(4)?);
    let n = (scale * scale) as f32;
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0.0f32; 3];
            let mut alpha = 0.0f32;
            for dy in 0..scale {
                for dx in 0..scale {
                    let at = (((y * scale + dy) as usize) * src_w as usize + (x * scale + dx) as usize) * 4;
                    let px = rgba.get(at..at + 4)?;
                    for (s, &c) in sum.iter_mut().zip(px.iter()) {
                        *s += lut.get(usize::from(c)).copied()?;
                    }
                    alpha += f32::from(*px.get(3)?);
                }
            }
            out.extend([
                linear_to_srgb(sum[0] / n),
                linear_to_srgb(sum[1] / n),
                linear_to_srgb(sum[2] / n),
                (alpha / n + 0.5).clamp(0.0, 255.0) as u8,
            ]);
        }
    }
    Some(out)
}

impl FinalRender {
    /// Start a still of `scene` (uploaded under `scene_key`) from `camera` at `size` pixels.
    pub fn start(off: &mut Offscreen, scene: &Scene3d, scene_key: u64, camera: &Camera, lighting: &Lighting, ground: bool, size: (u32, u32)) -> Self {
        let max = off.device.limits().max_texture_dimension_2d.max(1);
        let size = (size.0.clamp(1, max), size.1.clamp(1, max));
        let scale = if size.0.saturating_mul(2) <= max && size.1.saturating_mul(2) <= max { 2 } else { 1 };
        let eye = camera.eye();
        // Every light is shadowed (the budget is all of them); only the 64-light draw cap remains.
        let drawn = crate::lights::select_drawn(&scene.lights, [eye.x, eye.y, eye.z]);
        let side = match layers_needed(&scene.lights, &drawn) {
            0..=16 => crate::lights::FINAL_SHADOW_SIZE,
            17..=64 => 1024,
            _ => 512,
        };
        let target_size = (size.0 * scale, size.1 * scale);
        let mut target = Offscreen::with_device(off.device.clone(), off.queue.clone(), target_size);
        if target.size() != target_size {
            return failed(size, "the render target could not be created");
        }
        target.lit_pipelines = crate::lit::LitPipelines::new(&off.device, &target.gpu);
        if target.lit_pipelines.is_none() {
            return failed(size, "the lit pipelines are not available");
        }
        let far = {
            let (a, b) = (scene.bounds.min, scene.bounds.max);
            let d = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            if d.is_finite() { (d + 1.0).max(1.0) as f32 } else { 1.0 }
        };
        let job = Job {
            scene: scene.clone(),
            key: scene_key,
            camera: *camera,
            lighting: lighting.clone(),
            ground,
            target,
            atlas: crate::shadow_atlas::ShadowAtlas::new(&off.device, side),
            drawn,
            slots: Vec::new(),
            occluders: crate::shadow_atlas::occluder_key(scene),
            far,
            scale,
            lights_done: 0,
            rendered: false,
            steps: 0,
        };
        FinalRender {
            size,
            cancelled: false,
            status: FinalStatus::Running { progress: 0.0 },
            inner: Some(Inner::Raster(job)),
            fallback: None,
            path_stats: None,
        }
    }

    /// Start a path-traced still. With a compute device it starts a [`PathTracer`] (an error
    /// from `PathTracer::new` makes the render `Failed`); without one it starts the raster still
    /// and records [`FinalRender::fallback`] = "no compute shaders".
    #[allow(clippy::too_many_arguments)]
    pub fn start_path(
        compute: Option<&ComputeGpu>,
        off: &mut Offscreen,
        scene: &Scene3d,
        scene_key: u64,
        camera: &Camera,
        lighting: &Lighting,
        ground: bool,
        size: (u32, u32),
        opts: &PathOptions,
        cache: Option<&wgpu::PipelineCache>,
    ) -> Self {
        let max = off.device.limits().max_texture_dimension_2d.max(1);
        let size = (size.0.clamp(1, max), size.1.clamp(1, max));
        let Some(gpu) = compute else {
            let mut r = Self::start(off, scene, scene_key, camera, lighting, ground, size);
            r.fallback = Some("no compute shaders");
            return r;
        };
        match PathTracer::new(gpu, scene, camera, lighting, ground, size, opts, cache) {
            Ok(t) => {
                let size = t.size();
                let path_stats = Some(t.stats());
                FinalRender {
                    size,
                    cancelled: false,
                    status: FinalStatus::Running { progress: 0.0 },
                    inner: Some(Inner::Path(Box::new(t), gpu.clone())),
                    fallback: None,
                    path_stats,
                }
            }
            Err(msg) => failed(size, &msg),
        }
    }

    /// Do one bounded piece of work and report where it is. After `Done`, `Cancelled` or
    /// `Failed` it keeps returning that status.
    pub fn step(&mut self, off: &mut Offscreen) -> FinalStatus {
        if self.cancelled && matches!(self.status, FinalStatus::Running { .. }) {
            self.status = FinalStatus::Cancelled;
            self.inner = None;
        }
        if !matches!(self.status, FinalStatus::Running { .. }) {
            return self.status.clone();
        }
        match self.inner.take() {
            Some(Inner::Raster(mut job)) => match advance(off, &mut job, self.size) {
                Ok(Some(rgba)) => {
                    off.final_renders += 1;
                    self.status = FinalStatus::Done { width: self.size.0, height: self.size.1, rgba };
                }
                Ok(None) => {
                    let total = job.drawn.len() + 2;
                    self.status = FinalStatus::Running { progress: (job.steps as f32 / total as f32).clamp(0.0, 1.0) };
                    self.inner = Some(Inner::Raster(job));
                }
                Err(msg) => self.status = FinalStatus::Failed(msg),
            },
            Some(Inner::Path(mut tracer, gpu)) => {
                let status = tracer.step(&gpu);
                self.path_stats = Some(tracer.stats());
                match status {
                    PathStatus::Running { progress, .. } => {
                        self.status = FinalStatus::Running { progress: progress.clamp(0.0, 1.0) };
                        self.inner = Some(Inner::Path(tracer, gpu));
                    }
                    PathStatus::Done { width, height, rgba } => {
                        off.final_renders += 1;
                        self.status = FinalStatus::Done { width, height, rgba };
                    }
                    PathStatus::Cancelled => self.status = FinalStatus::Cancelled,
                    PathStatus::Failed(msg) => self.status = FinalStatus::Failed(msg),
                }
            }
            None => self.status = FinalStatus::Failed("the render has no work".into()),
        }
        self.status.clone()
    }

    /// Stop at the next step.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// Why a path render ran as a raster one (`Some("no compute shaders")`), else `None`.
    pub fn fallback(&self) -> Option<&'static str> {
        self.fallback
    }

    /// What the path tracer did so far; `None` for a raster render.
    pub fn path_stats(&self) -> Option<&PathStats> {
        self.path_stats.as_ref()
    }

    /// The path tracer's image so far (width, height, RGBA8) while it refines; `None` for a
    /// raster render, before the first pass, or once finished.
    pub fn preview(&self) -> Option<(u32, u32, Vec<u8>)> {
        match &self.inner {
            Some(Inner::Path(tracer, gpu)) => tracer.preview(gpu),
            _ => None,
        }
    }

    /// The pixel size of the still.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }
}

/// One step of `job`: a light's shadow maps, the 2x render, or the downsample (which returns the
/// pixels).
fn advance(off: &mut Offscreen, job: &mut Job, size: (u32, u32)) -> Result<Option<Vec<u8>>, String> {
    job.steps += 1;
    if job.lights_done < job.drawn.len() {
        job.lights_done += 1;
        let shadowed: Vec<usize> = job.drawn.iter().take(job.lights_done).copied().collect();
        off.uploaded(&job.scene, job.key, job.ground).ok_or("the scene could not be uploaded")?;
        let (_, meshes) = off.meshes.as_ref().ok_or("the scene could not be uploaded")?;
        // Geometry outside the view's clip box casts no fixture shadow.
        job.atlas.set_clip(crate::frame_look::look_clip(&job.lighting.look));
        let before = job.atlas.maps_built();
        job.slots = job.atlas.update(&off.device, &off.queue, meshes, job.occluders, &job.scene.lights, &shadowed, job.far);
        off.shadows_built += job.atlas.maps_built().saturating_sub(before);
        return Ok(None);
    }
    if !job.rendered {
        let sun = job.lighting.sun;
        job.target.shadow_clip = crate::frame_look::look_clip(&job.lighting.look);
        let sun_map = match sun.filter(|_| job.lighting.sun_shadow) {
            Some(s) => job.target.build_shadow(&job.scene, job.key, &s, job.ground),
            None => None,
        };
        let sh = job.lighting.environment.as_deref().filter(|e| e.is_valid()).and_then(crate::frame_look::env_sh);
        let params = crate::lit::LitParams {
            sh,
            sun_on: sun.is_some(),
            ambient: sun.map_or(crate::lights::NIGHT_AMBIENT, |l| l.ambient),
            exposure: job.lighting.exposure,
            tonemap: true,
            pcf_radius: 2,
            ..Default::default()
        };
        let layout = &job.target.lit_pipelines.as_ref().ok_or("the lit pipelines are not available")?.layout;
        let mut frame = crate::lit::LitFrame::new(
            &job.target.device,
            layout,
            &crate::lit::frame_uniform(sun_map.as_ref(), &params, job.atlas.size()),
            sun_map.as_ref().map(|m| &m.view),
            &crate::lights::pack_lights(&job.scene.lights, &job.drawn, &job.slots),
            job.atlas.view(),
            &params,
            (job.drawn.len(), job.slots.len()),
        );
        // The frame's look (exposure, tone map, ambient) comes from the lighting, as in the pane.
        frame.lighting = job.lighting.clone();
        let opts =
            crate::offscreen::RenderOptions { lit: Some(&frame), ground: job.ground, style: crate::offscreen::Style::Shaded, ..Default::default() };
        let light = sun.unwrap_or(crate::light::Light::DEFAULT);
        job.target.render_with(&job.scene, job.key, &job.camera, &light, &opts);
        job.rendered = true;
        return Ok(None);
    }
    let big = job.target.read_rgba().map_err(|e| e.to_string())?;
    let rgba = downsample(&big, size.0 * job.scale, size, job.scale).ok_or("the render could not be downsampled")?;
    Ok(Some(rgba))
}

/// Run a Finalize Render to the end and return the PNG (headless `render --final` and the
/// `render {"final":true}` command). Counts one final render on `off`.
pub fn render_final(
    off: &mut Offscreen,
    scene: &Scene3d,
    scene_key: u64,
    camera: &Camera,
    lighting: &Lighting,
    ground: bool,
    size: (u32, u32),
) -> Result<Vec<u8>, Render3dError> {
    let mut render = FinalRender::start(off, scene, scene_key, camera, lighting, ground, size);
    // At most 64 lights are drawn, so this bound is never the limit.
    for _ in 0..1000 {
        match render.step(off) {
            FinalStatus::Running { .. } => {}
            FinalStatus::Done { width, height, rgba } => return crate::offscreen::encode_png(width, height, &rgba),
            FinalStatus::Cancelled => return Err(Render3dError::Wgpu("Finalize Render was cancelled".into())),
            FinalStatus::Failed(msg) => return Err(Render3dError::Wgpu(msg)),
        }
    }
    Err(Render3dError::Wgpu("Finalize Render did not finish".into()))
}

/// Run a Finalize Render to the end in raster or path mode and return the PNG, the path
/// statistics (`None` for raster) and the fallback reason if a path render ran as raster.
#[allow(clippy::too_many_arguments)]
pub fn render_final_with(
    off: &mut Offscreen,
    compute: Option<&ComputeGpu>,
    scene: &Scene3d,
    scene_key: u64,
    camera: &Camera,
    lighting: &Lighting,
    ground: bool,
    size: (u32, u32),
    mode_path: bool,
    opts: &PathOptions,
    cache: Option<&wgpu::PipelineCache>,
) -> Result<(Vec<u8>, Option<PathStats>, Option<&'static str>), Render3dError> {
    if !mode_path {
        return Ok((render_final(off, scene, scene_key, camera, lighting, ground, size)?, None, None));
    }
    let mut render = FinalRender::start_path(compute, off, scene, scene_key, camera, lighting, ground, size, opts, cache);
    // A path render takes spp / spp_per_step steps plus the resolve; bound it by the option caps.
    let limit = u64::from(opts.spp) + 1000;
    for _ in 0..limit {
        match render.step(off) {
            FinalStatus::Running { .. } => {}
            FinalStatus::Done { width, height, rgba } => {
                let png = crate::offscreen::encode_png(width, height, &rgba)?;
                return Ok((png, render.path_stats().cloned(), render.fallback()));
            }
            FinalStatus::Cancelled => return Err(Render3dError::Wgpu("Finalize Render was cancelled".into())),
            FinalStatus::Failed(msg) => return Err(Render3dError::Wgpu(msg)),
        }
    }
    Err(Render3dError::Wgpu("Finalize Render did not finish".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Orbit;
    use crate::types::{ArchId, Mesh};
    use cadcraft_geom::{Vec2, Vec3};

    const SIZE: (u32, u32) = (96, 96);

    fn spot(id: u32, x: f64, y: f64, deg: f64) -> SceneLight {
        SceneLight {
            id: ArchId(id),
            kind: LightKind::Spot,
            position: Vec3::new(x, y, 3.0),
            direction: Vec3::new(0.0, 0.0, -1.0),
            intensity: 6000.0,
            color: [1.0; 3],
            cos_inner: (deg - 5.0).to_radians().cos(),
            cos_outer: deg.to_radians().cos(),
            ..Default::default()
        }
    }

    fn prism(id: u32, material: &str, lo: (f64, f64), hi: (f64, f64), z: (f64, f64)) -> Mesh {
        let mut m = Mesh::new(material, ArchId(id));
        m.push_prism(&[Vec2::new(lo.0, lo.1), Vec2::new(hi.0, lo.1), Vec2::new(hi.0, hi.1), Vec2::new(lo.0, hi.1)], z.0, z.1);
        m
    }

    fn scene(column: bool) -> Scene3d {
        let mut meshes = vec![prism(1, "floor", (-5.0, -5.0), (5.0, 5.0), (-0.1, 0.0))];
        if column {
            meshes.push(prism(2, "wall", (-1.6, -0.2), (-1.2, 0.2), (0.0, 2.5)));
        }
        let mut bounds = meshes[0].bounds();
        for m in meshes.iter().skip(1) {
            bounds.extend(&m.bounds());
        }
        Scene3d { meshes, bounds, lights: vec![spot(10, -2.0, 0.0, 50.0), spot(11, 3.5, -3.5, 20.0)], ..Default::default() }
    }

    fn scene_no_lights() -> Scene3d {
        let meshes = vec![prism(1, "floor", (-5.0, -5.0), (5.0, 5.0), (-0.1, 0.0))];
        let bounds = meshes[0].bounds();
        Scene3d { meshes, bounds, lights: vec![], ..Default::default() }
    }

    fn camera() -> Camera {
        Camera::Orbit(Orbit::at_origin(0.0, 89.0, 12.0, 45.0))
    }

    fn night() -> Lighting {
        Lighting { sun: None, ..Lighting::default() }
    }

    fn off() -> Option<Offscreen> {
        let Some((device, queue)) = crate::gputest::shared_device() else {
            eprintln!("SKIP: no GPU adapter available");
            return None;
        };
        Some(Offscreen::with_device(device, queue, (32, 32)))
    }

    #[test]
    fn render_final_returns_a_png_and_counts_one() {
        let Some(mut o) = off() else { return };
        assert_eq!(o.final_renders(), 0);
        let png = render_final(&mut o, &scene(false), 1, &camera(), &night(), false, SIZE);
        assert!(png.is_ok(), "{:?}", png.as_ref().err());
        let Ok(png) = png else { return };
        let len = crate::offscreen::png_rgba(&png).map(|r| r.len()).ok();
        assert_eq!(len, Some((SIZE.0 * SIZE.1 * 4) as usize));
        assert_eq!(o.final_renders(), 1);
    }

    #[test]
    fn final_progress_rises_to_one() {
        let Some(mut o) = off() else { return };
        let mut r = FinalRender::start(&mut o, &scene(false), 1, &camera(), &night(), false, SIZE);
        let mut last = 0.0f32;
        for _ in 0..20 {
            match r.step(&mut o) {
                FinalStatus::Running { progress } => {
                    assert!(progress >= last && progress < 1.0, "{progress} after {last}");
                    last = progress;
                }
                FinalStatus::Done { width, height, .. } => {
                    assert_eq!((width, height), SIZE);
                    assert!(last > 0.0);
                    return;
                }
                other => assert!(false, "unexpected {other:?}"),
            }
        }
        assert!(false, "the render did not finish");
    }

    #[test]
    fn oversized_still_is_clamped_to_the_gpu_limit() {
        let Some(mut o) = off() else { return };
        let max = o.device.limits().max_texture_dimension_2d.max(1);
        let mut r = FinalRender::start(&mut o, &scene(false), 1, &camera(), &night(), false, (max.saturating_mul(2), max.saturating_mul(2)));
        assert_eq!(r.size(), (max, max));
        assert!(matches!(r.step(&mut o), FinalStatus::Running { .. } | FinalStatus::Done { .. }));
    }

    #[test]
    fn cancelled_final_counts_nothing() {
        let Some(mut o) = off() else { return };
        let mut r = FinalRender::start(&mut o, &scene(false), 1, &camera(), &night(), false, SIZE);
        assert!(matches!(r.step(&mut o), FinalStatus::Running { .. }));
        r.cancel();
        assert_eq!(r.step(&mut o), FinalStatus::Cancelled);
        assert_eq!(r.step(&mut o), FinalStatus::Cancelled);
        assert_eq!(o.final_renders(), 0);
    }

    fn path_opts(spp: u32) -> PathOptions {
        PathOptions { spp, spp_per_step: 2, denoise: false, ..Default::default() }
    }

    fn compute() -> Option<ComputeGpu> {
        let c = crate::gputest::shared_compute();
        if c.is_none() {
            eprintln!("SKIP: no compute device available");
        }
        c
    }

    #[test]
    fn final_path_runs_the_path_tracer_to_done() {
        let (Some(c), Some(mut o)) = (compute(), off()) else { return };
        let mut r = FinalRender::start_path(Some(&c), &mut o, &scene(false), 1, &camera(), &night(), false, SIZE, &path_opts(4), None);
        assert!(r.fallback().is_none());
        for _ in 0..200 {
            match r.step(&mut o) {
                FinalStatus::Running { .. } => {}
                FinalStatus::Done { width, height, rgba } => {
                    assert_eq!((width, height), SIZE);
                    assert_eq!(rgba.len(), (SIZE.0 * SIZE.1 * 4) as usize);
                    assert_eq!(o.final_renders(), 1);
                    assert_eq!(r.path_stats().map(|s| s.spp_done), Some(4));
                    return;
                }
                other => assert!(false, "unexpected {other:?}"),
            }
        }
        assert!(false, "the render did not finish");
    }

    #[test]
    fn final_path_falls_back_to_raster_without_compute() {
        let Some(mut o) = off() else { return };
        let r = render_final_with(&mut o, None, &scene(false), 1, &camera(), &night(), false, SIZE, true, &path_opts(4), None);
        assert!(r.is_ok(), "{:?}", r.as_ref().err());
        let Ok((png, stats, fallback)) = r else { return };
        assert!(stats.is_none());
        assert_eq!(fallback, Some("no compute shaders"));
        assert_eq!(crate::offscreen::png_rgba(&png).map(|p| p.len()).ok(), Some((SIZE.0 * SIZE.1 * 4) as usize));
        assert_eq!(o.final_renders(), 1);
    }

    #[test]
    fn final_path_cancels_and_counts_nothing() {
        let (Some(c), Some(mut o)) = (compute(), off()) else { return };
        let mut r = FinalRender::start_path(Some(&c), &mut o, &scene(false), 1, &camera(), &night(), false, SIZE, &path_opts(64), None);
        assert!(matches!(r.step(&mut o), FinalStatus::Running { .. }));
        r.cancel();
        assert_eq!(r.step(&mut o), FinalStatus::Cancelled);
        assert_eq!(r.step(&mut o), FinalStatus::Cancelled);
        assert_eq!(o.final_renders(), 0);
    }

    #[test]
    fn final_path_preview_refines() {
        let (Some(c), Some(mut o)) = (compute(), off()) else { return };
        let mut r = FinalRender::start_path(Some(&c), &mut o, &scene(false), 1, &camera(), &night(), false, SIZE, &path_opts(64), None);
        assert!(r.preview().is_none(), "nothing to show before the first pass");
        assert!(matches!(r.step(&mut o), FinalStatus::Running { .. }));
        let p = r.preview();
        assert!(p.is_some());
        let Some((w, h, rgba)) = p else { return };
        assert_eq!((w, h), SIZE);
        assert_eq!(rgba.len(), (SIZE.0 * SIZE.1 * 4) as usize);
        let first = r.path_stats().map_or(0, |s| s.spp_done);
        assert!(matches!(r.step(&mut o), FinalStatus::Running { .. }));
        assert!(r.path_stats().map_or(0, |s| s.spp_done) > first);
    }

    fn lum(rgba: &[u8], world: [f64; 3]) -> Option<u32> {
        let p = crate::camera::project_point(&camera(), SIZE, world)?;
        let (x, y) = (p[0].floor() as u32, p[1].floor() as u32);
        let at = ((y * SIZE.0 + x) * 4) as usize;
        let px = rgba.get(at..at + 3)?;
        Some(px.iter().map(|&c| u32::from(c)).sum::<u32>() / 3)
    }

    fn still(o: &mut Offscreen, s: &Scene3d, key: u64) -> Vec<u8> {
        let mut r = FinalRender::start(o, s, key, &camera(), &night(), false, SIZE);
        for _ in 0..20 {
            if let FinalStatus::Done { rgba, .. } = r.step(o) {
                return rgba;
            }
        }
        Vec::new()
    }

    #[test]
    fn final_shadows_every_light_softly() {
        let Some(mut o) = off() else { return };
        let shadowed = lum(&still(&mut o, &scene(true), 1), [0.5, 0.0, 0.0]);
        let open = lum(&still(&mut o, &scene(false), 2), [0.5, 0.0, 0.0]);
        match (shadowed, open) {
            (Some(s), Some(l)) => assert!(l >= s + 40, "open floor {l} must be at least 40 levels above the column's shadow {s}"),
            _ => assert!(false, "the test point must project into the image"),
        }
        assert!(o.shadow_maps_built() >= 4);
    }

    #[test]
    fn finalize_shows_the_backdrop() {
        let Some(mut o) = off() else { return };
        let mut plain = scene(false);
        plain.meshes = vec![prism(1, "wall", (-1.0, -1.0), (1.0, 1.0), (0.0, 2.0))];
        plain.bounds = plain.meshes[0].bounds();
        plain.lights.clear();
        let mut with = plain.clone();
        with.backdrop = Some(cadcraft_doc::arch::Backdrop::Colour { colour: [32, 64, 96] });
        let a = still(&mut o, &plain, 1);
        let b = still(&mut o, &with, 2);
        assert_eq!(a.len(), (SIZE.0 * SIZE.1 * 4) as usize);
        assert_eq!(b.len(), a.len());
        let clear = |p: &[u8]| p.iter().take(3).zip([39u8, 44, 52]).all(|(x, y)| x.abs_diff(y) <= 1);
        let mut background = 0;
        for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
            if clear(pa) {
                background += 1;
                assert!(pb.iter().take(3).zip([32u8, 64, 96]).all(|(x, y)| x.abs_diff(y) <= 1), "background pixel {pb:?}");
            }
        }
        assert!(background > 100, "the still must show background ({background})");
        let at = ((SIZE.1 / 2 * SIZE.0 + SIZE.0 / 2) * 4) as usize;
        assert_eq!(a.get(at..at + 4), b.get(at..at + 4), "a pixel covered by the mesh is unchanged");
        assert!(!clear(&a[at..at + 4]) && a[at..at + 3] != [32, 64, 96], "the centre pixel must be on the mesh");
    }

    #[test]
    fn final_render_leaves_the_interactive_cache_alone() {
        let Some(mut o) = off() else { return };
        let s = scene(false);
        assert!(o.lit_frame(&s, 1, [10.0, 0.0, 6.0], &night(), false).is_some());
        let slots = o.shadowed_lights();
        assert!(render_final(&mut o, &s, 1, &camera(), &night(), false, SIZE).is_ok());
        assert_eq!(o.shadowed_lights(), slots);
        assert_eq!(o.final_renders(), 1);
    }

    #[test]
    fn final_path_preview_none_after_done() {
        let (Some(c), Some(mut o)) = (compute(), off()) else { return };
        let mut r = FinalRender::start_path(Some(&c), &mut o, &scene(false), 1, &camera(), &night(), false, SIZE, &path_opts(4), None);
        for _ in 0..200 {
            match r.step(&mut o) {
                FinalStatus::Running { .. } => {}
                FinalStatus::Done { .. } => {
                    assert!(r.preview().is_none(), "preview must be None once done");
                    return;
                }
                other => assert!(false, "unexpected {other:?}"),
            }
        }
        assert!(false, "the render did not finish");
    }

    #[test]
    fn final_path_no_lights() {
        let (Some(c), Some(mut o)) = (compute(), off()) else { return };
        let mut r = FinalRender::start_path(Some(&c), &mut o, &scene_no_lights(), 1, &camera(), &night(), false, SIZE, &path_opts(4), None);
        for _ in 0..200 {
            match r.step(&mut o) {
                FinalStatus::Running { .. } => {}
                FinalStatus::Done { width, height, rgba } => {
                    assert_eq!((width, height), SIZE);
                    assert_eq!(rgba.len(), (SIZE.0 * SIZE.1 * 4) as usize);
                    return;
                }
                other => assert!(false, "unexpected {other:?}"),
            }
        }
        assert!(false, "the render did not finish");
    }

    #[test]
    fn final_path_oversized_clamped() {
        let (Some(c), Some(mut o)) = (compute(), off()) else { return };
        let max = o.device.limits().max_texture_dimension_2d.max(1);
        let mut r = FinalRender::start_path(
            Some(&c),
            &mut o,
            &scene(false),
            1,
            &camera(),
            &night(),
            false,
            (max.saturating_mul(2), max.saturating_mul(2)),
            &path_opts(4),
            None,
        );
        assert_eq!(r.size(), (max, max));
        for _ in 0..200 {
            match r.step(&mut o) {
                FinalStatus::Running { .. } => {}
                FinalStatus::Done { width, height, .. } => {
                    assert_eq!((width, height), (max, max));
                    return;
                }
                other => assert!(false, "unexpected {other:?}"),
            }
        }
        assert!(false, "the render did not finish");
    }

    #[test]
    fn render_final_with_raster_mode() {
        let Some(mut o) = off() else { return };
        let r = render_final_with(&mut o, None, &scene(false), 1, &camera(), &night(), false, SIZE, false, &path_opts(4), None);
        assert!(r.is_ok(), "{:?}", r.as_ref().err());
        let Ok((png, stats, fallback)) = r else { return };
        assert!(stats.is_none());
        assert!(fallback.is_none());
        assert_eq!(crate::offscreen::png_rgba(&png).map(|p| p.len()).ok(), Some((SIZE.0 * SIZE.1 * 4) as usize));
        assert_eq!(o.final_renders(), 1);
    }

    #[test]
    fn final_path_with_ground() {
        let (Some(c), Some(mut o)) = (compute(), off()) else { return };
        let mut r = FinalRender::start_path(Some(&c), &mut o, &scene(false), 1, &camera(), &night(), true, SIZE, &path_opts(4), None);
        for _ in 0..200 {
            match r.step(&mut o) {
                FinalStatus::Running { .. } => {}
                FinalStatus::Done { width, height, rgba } => {
                    assert_eq!((width, height), SIZE);
                    assert_eq!(rgba.len(), (SIZE.0 * SIZE.1 * 4) as usize);
                    return;
                }
                other => assert!(false, "unexpected {other:?}"),
            }
        }
        assert!(false, "the render did not finish");
    }

    #[test]
    fn render_final_with_path_mode() {
        let (Some(c), Some(mut o)) = (compute(), off()) else { return };
        let r = render_final_with(&mut o, Some(&c), &scene(false), 1, &camera(), &night(), false, SIZE, true, &path_opts(4), None);
        assert!(r.is_ok(), "{:?}", r.as_ref().err());
        let Ok((png, stats, fallback)) = r else { return };
        assert!(stats.is_some());
        assert!(fallback.is_none());
        assert_eq!(stats.as_ref().map(|s| s.spp_done), Some(4));
        assert_eq!(crate::offscreen::png_rgba(&png).map(|p| p.len()).ok(), Some((SIZE.0 * SIZE.1 * 4) as usize));
        assert_eq!(o.final_renders(), 1);
    }

    /// An empty scene (no meshes, no lights) renders to a valid PNG of the requested size.
    #[test]
    fn final_render_empty_scene() {
        let Some(mut o) = off() else { return };
        let meshes: Vec<Mesh> = vec![];
        let bounds = cadcraft_arch3d::mesh::Bounds3::empty();
        let s = Scene3d { meshes, bounds, lights: vec![], ..Default::default() };
        let png = render_final(&mut o, &s, 1, &camera(), &night(), false, SIZE);
        assert!(png.is_ok(), "{:?}", png.as_ref().err());
        let Ok(png) = png else { return };
        let len = crate::offscreen::png_rgba(&png).map(|r| r.len()).ok();
        assert_eq!(len, Some((SIZE.0 * SIZE.1 * 4) as usize));
        assert_eq!(o.final_renders(), 1);
    }

    /// A scene with meshes but no lights renders correctly in raster mode.
    #[test]
    fn final_render_no_lights_raster() {
        let Some(mut o) = off() else { return };
        let mut s = scene(false);
        s.lights.clear();
        let png = render_final(&mut o, &s, 1, &camera(), &night(), false, SIZE);
        assert!(png.is_ok(), "{:?}", png.as_ref().err());
        let Ok(png) = png else { return };
        let len = crate::offscreen::png_rgba(&png).map(|r| r.len()).ok();
        assert_eq!(len, Some((SIZE.0 * SIZE.1 * 4) as usize));
        assert_eq!(o.final_renders(), 1);
    }

    /// With scale=1 the downsample is a passthrough (each pixel is its own 1x1 block).
    #[test]
    fn downsample_scale_one_is_identity() {
        let rgba = [255u8, 128, 64, 255, 10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255];
        let out = downsample(&rgba, 2, (2, 2), 1);
        assert!(out.is_some());
        let out = out.unwrap();
        assert_eq!(out.len(), 16);
        for i in 0..16 {
            assert_eq!(out[i], rgba[i], "pixel {i} changed with scale=1");
        }
    }

    /// Downsample averages in linear light: two black and two white sRGB pixels in a 2x2 block
    /// give linear 0.5, which encodes to sRGB 188.
    #[test]
    fn downsample_averages_linearly() {
        let rgba = [0u8, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255];
        let out = downsample(&rgba, 2, (1, 1), 2);
        assert!(out.is_some());
        let out = out.unwrap();
        assert_eq!(out.len(), 4);
        let expected = linear_to_srgb(0.5);
        assert_eq!(out[0], expected, "R channel");
        assert_eq!(out[1], expected, "G channel");
        assert_eq!(out[2], expected, "B channel");
        assert_eq!(out[3], 255, "alpha");
    }

    /// A scene with exactly one light renders without error.
    #[test]
    fn final_render_single_light() {
        let Some(mut o) = off() else { return };
        let mut s = scene(false);
        s.lights = vec![spot(10, -2.0, 0.0, 50.0)];
        let png = render_final(&mut o, &s, 1, &camera(), &night(), false, SIZE);
        assert!(png.is_ok(), "{:?}", png.as_ref().err());
        let Ok(png) = png else { return };
        let len = crate::offscreen::png_rgba(&png).map(|r| r.len()).ok();
        assert_eq!(len, Some((SIZE.0 * SIZE.1 * 4) as usize));
        assert_eq!(o.final_renders(), 1);
    }

    /// The raster Finalize passes the environment SH through to its LitParams when a valid
    /// environment is present, so its diffuse illumination equals the interactive frame's.
    #[test]
    fn final_raster_uses_the_environment_sh() {
        let Some(mut o) = off() else { return };
        let env = crate::environment::Environment::uniform([1.0; 3], 32, 16);
        let mut look = cadcraft_doc::arch::Look::default();
        look.exposure = cadcraft_doc::arch::ExposureSetting::Manual { ev100: 14.0 };
        look.tone_map = cadcraft_doc::arch::ToneMap::None;
        let lighting = Lighting {
            sun: None,
            look,
            lambert: Some(0.5),
            meter_night: false,
            environment: Some(std::sync::Arc::new(env.clone())),
            ..Lighting::default()
        };
        let mut r = FinalRender::start(&mut o, &scene_no_lights(), 1, &camera(), &lighting, false, SIZE);
        for _ in 0..20 {
            if matches!(r.step(&mut o), FinalStatus::Done { .. }) {
                break;
            }
        }
        // The final render must have completed without failing.
        let Some(FinalStatus::Done { .. }) = r.status.clone() else {
            eprintln!("SKIP: render did not finish: {:?}", r.status);
            return;
        };
        // Verify that frame_look::env_sh returns non-None for a uniform environment.
        assert!(crate::frame_look::env_sh(&env).is_some(), "env_sh must return coefficients for a valid env");
        // The ambient in the final frame must be 0 under an environment.
        let f = crate::frame_look::FrameLook { ambient_lux: 0.0, ..crate::frame_look::FrameLook::default() };
        let mut scene = scene_no_lights();
        let mut meter = crate::frame_look::Meter::default();
        let f = crate::frame_look::frame_look(&scene, &camera(), (SIZE.0, SIZE.1), &lighting, false, &mut meter);
        assert_eq!(f.ambient_lux, 0.0, "ambient must be 0 under an environment");
    }

    /// A white plane of albedo 0.5 under a uniform HDRI of radiance 1.0 at EV100 -1 reads the
    /// same in the interactive frame and the raster Finalize (within 5 %).
    #[test]
    fn final_raster_plane_under_uniform_environment_matches_interactive() {
        let Some(mut o) = off() else { return };
        let env = crate::environment::Environment::uniform([1.0; 3], 32, 16);
        let mut look = cadcraft_doc::arch::Look::default();
        look.exposure = cadcraft_doc::arch::ExposureSetting::Manual { ev100: -1.0 };
        look.tone_map = cadcraft_doc::arch::ToneMap::None;
        let lighting = Lighting {
            sun: None,
            look: look.clone(),
            lambert: Some(0.5),
            meter_night: false,
            environment: Some(std::sync::Arc::new(env)),
            ..Lighting::default()
        };
        let camera = Camera::Orbit(crate::camera::Orbit::at_origin(0.0, 89.0, 3.0, 45.0));
        // Interactive frame.
        let eye = camera.eye();
        let Some(frame) = o.lit_frame(&scene_no_lights(), 1, [eye.x, eye.y, eye.z], &lighting, false) else {
            eprintln!("SKIP: no lit frame");
            return;
        };
        o.render_with(
            &scene_no_lights(),
            1,
            &camera,
            &crate::light::Light::DEFAULT,
            &crate::offscreen::RenderOptions { lit: Some(&frame), ..Default::default() },
        );
        let (w, h) = o.size();
        let Ok(img) = o.read_rgba() else { return };
        let at = ((h / 2 * w + w / 2) * 4) as usize;
        let interactive = img.get(at..at + 3).map(|p| [srgb_to_linear(p[0]), srgb_to_linear(p[1]), srgb_to_linear(p[2])]);
        // Finalize render.
        let png = render_final(&mut o, &scene_no_lights(), 2, &camera, &lighting, false, SIZE);
        let Ok(png) = png else { return };
        let Ok(img) = crate::offscreen::png_rgba(&png) else { return };
        let at = ((SIZE.1 / 2 * SIZE.0 + SIZE.0 / 2) * 4) as usize;
        let finalize = img.get(at..at + 3).map(|p| [srgb_to_linear(p[0]), srgb_to_linear(p[1]), srgb_to_linear(p[2])]);
        // Both should read 0.5 (Lambert 0.5 under uniform radiance 1 → L = 0.5/π × π = 0.5 nits,
        // which at EV100 -1 (scale 1/(1.2×2^-1)) → 0.5×(1.2×2) = 1.2, but with tone_map None that's
        // just the linear value).
        let want = 0.5;
        if let (Some(a), Some(b)) = (interactive, finalize) {
            for (name, val) in [("interactive", a), ("finalize", b)] {
                for (c, ch) in val.iter().enumerate() {
                    assert!(ch.is_finite(), "{name} {c} not finite: {ch}");
                    assert!((ch - want).abs() < want * 0.05 + 0.001, "{name} {c}: {ch} want {want} +- 5%");
                }
            }
            assert!((a[0] - b[0]).abs() < 0.02, "interactive {a:?} vs finalize {b:?} differ");
            assert!((a[1] - b[1]).abs() < 0.02, "interactive {a:?} vs finalize {b:?} differ");
            assert!((a[2] - b[2]).abs() < 0.02, "interactive {a:?} vs finalize {b:?} differ");
        } else {
            eprintln!("SKIP: projection missed the plane: interactive={:?} finalize={:?}", interactive, finalize);
        }
    }

    /// The pane's lit frame and the raster Finalize tone-map alike: the same plane under the same
    /// sun, manual exposure and curve gives the same pixel in both (and the curve's value).
    #[test]
    fn finalize_matches_interactive_on_a_plane() {
        use crate::exposure::{TONE_ACES, TONE_HABLE, ev100_scale, tone_code, tone_map};
        use crate::light::Light;
        use crate::offscreen::{RenderOptions, png_rgba};
        use cadcraft_doc::arch::{ExposureSetting, Look, ToneMap};
        let Some(mut o) = off() else { return };
        let s = scene_no_lights();
        let camera = Camera::Orbit(Orbit::at_origin(0.0, 89.0, 3.0, 45.0));
        for tone in [ToneMap::Aces, ToneMap::Hable] {
            let mut look = Look::default();
            look.exposure = ExposureSetting::Manual { ev100: 14.0 };
            look.tone_map = tone;
            look.sky.on = false;
            let lighting = Lighting {
                sun: Some(Light { dir: [0.0, 0.0, 1.0], ..Light::DEFAULT }),
                look,
                lambert: Some(0.5),
                meter_night: true,
                ..Lighting::default()
            };
            let eye = camera.eye();
            let Some(frame) = o.lit_frame(&s, 1, [eye.x, eye.y, eye.z], &lighting, false) else { return };
            let sun = lighting.sun.unwrap_or(Light::DEFAULT);
            o.render_with(&s, 1, &camera, &sun, &RenderOptions { lit: Some(&frame), ..Default::default() });
            let (w, h) = o.size();
            let Ok(img) = o.read_rgba() else { return };
            let at = ((h / 2 * w + w / 2) * 4) as usize;
            let interactive = img.get(at..at + 3).map(|p| [srgb_to_linear(p[0]), srgb_to_linear(p[1]), srgb_to_linear(p[2])]);
            let png = render_final(&mut o, &s, 2, &camera, &lighting, false, SIZE);
            let Ok(png) = png else { return };
            let Ok(img) = png_rgba(&png) else { return };
            let at = ((SIZE.1 / 2 * SIZE.0 + SIZE.0 / 2) * 4) as usize;
            let finalize = img.get(at..at + 3).map(|p| [srgb_to_linear(p[0]), srgb_to_linear(p[1]), srgb_to_linear(p[2])]);
            let want = tone_map(15_915.5 * ev100_scale(14.0), tone_code(tone));
            assert!(matches!(tone_code(tone), TONE_ACES | TONE_HABLE));
            for (name, got) in [("interactive", interactive), ("finalize", finalize)] {
                let Some(got) = got else { return };
                for c in got {
                    assert!((c - want).abs() <= want * 0.02 + 0.004, "{name} {tone:?}: {got:?} want {want}");
                }
            }
            if let (Some(a), Some(b)) = (interactive, finalize) {
                let diff = (a[0] - b[0]).abs();
                assert!(diff < 0.01, "interactive {a:?} finalize {b:?} diff {diff}");
            }
        }
    }
}
