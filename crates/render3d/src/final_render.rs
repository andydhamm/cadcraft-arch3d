//! Finalize Render (phase 10): one still from the current camera with every light shadowed at
//! high resolution (`lights::FINAL_SHADOW_SIZE` layers in its own atlas), soft shadow edges
//! (PCF radius 2), 2×2 supersampling and tone mapping. It runs in steps so the pane can show
//! progress and cancel it: one step per shadowed light, then the 2x render, then the downsample
//! (averaged in linear light).

use crate::Render3dError;
use crate::camera::Camera;
use crate::lit_frame::Lighting;
use crate::offscreen::Offscreen;
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

/// A Finalize Render in progress.
pub struct FinalRender {
    pub(crate) size: (u32, u32),
    pub(crate) cancelled: bool,
    pub(crate) status: FinalStatus,
    job: Option<Job>,
}

fn failed(size: (u32, u32), msg: &str) -> FinalRender {
    FinalRender { size, cancelled: false, status: FinalStatus::Failed(msg.into()), job: None }
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
        if size.0 == 0 || size.1 == 0 || size.0 > max || size.1 > max {
            return failed(size, "the still's size is outside what this GPU can render");
        }
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
            lighting: *lighting,
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
        FinalRender { size, cancelled: false, status: FinalStatus::Running { progress: 0.0 }, job: Some(job) }
    }

    /// Do one bounded piece of work and report where it is. After `Done`, `Cancelled` or
    /// `Failed` it keeps returning that status.
    pub fn step(&mut self, off: &mut Offscreen) -> FinalStatus {
        if self.cancelled && matches!(self.status, FinalStatus::Running { .. }) {
            self.status = FinalStatus::Cancelled;
            self.job = None;
        }
        if !matches!(self.status, FinalStatus::Running { .. }) {
            return self.status.clone();
        }
        let Some(mut job) = self.job.take() else {
            self.status = FinalStatus::Failed("the render has no work".into());
            return self.status.clone();
        };
        match advance(off, &mut job, self.size) {
            Ok(Some(rgba)) => {
                off.final_renders += 1;
                self.status = FinalStatus::Done { width: self.size.0, height: self.size.1, rgba };
            }
            Ok(None) => {
                let total = job.drawn.len() + 2;
                self.status = FinalStatus::Running { progress: (job.steps as f32 / total as f32).clamp(0.0, 1.0) };
                self.job = Some(job);
            }
            Err(msg) => self.status = FinalStatus::Failed(msg),
        }
        self.status.clone()
    }

    /// Stop at the next step.
    pub fn cancel(&mut self) {
        self.cancelled = true;
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
        let before = job.atlas.maps_built();
        job.slots = job.atlas.update(&off.device, &off.queue, meshes, job.occluders, &job.scene.lights, &shadowed, job.far);
        off.shadows_built += job.atlas.maps_built().saturating_sub(before);
        return Ok(None);
    }
    if !job.rendered {
        let sun = job.lighting.sun;
        let sun_map = match sun.filter(|_| job.lighting.sun_shadow) {
            Some(s) => job.target.build_shadow(&job.scene, job.key, &s, job.ground),
            None => None,
        };
        let params = crate::lit::LitParams {
            sun_on: sun.is_some(),
            ambient: sun.map_or(crate::lights::NIGHT_AMBIENT, |l| l.ambient),
            exposure: job.lighting.exposure,
            tonemap: true,
            pcf_radius: 2,
            ..Default::default()
        };
        let layout = &job.target.lit_pipelines.as_ref().ok_or("the lit pipelines are not available")?.layout;
        let frame = crate::lit::LitFrame::new(
            &job.target.device,
            layout,
            &crate::lit::frame_uniform(sun_map.as_ref(), &params, job.atlas.size()),
            sun_map.as_ref().map(|m| &m.view),
            &crate::lights::pack_lights(&job.scene.lights, &job.drawn, &job.slots),
            job.atlas.view(),
            &params,
            (job.drawn.len(), job.slots.len()),
        );
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
    fn cancelled_final_counts_nothing() {
        let Some(mut o) = off() else { return };
        let mut r = FinalRender::start(&mut o, &scene(false), 1, &camera(), &night(), false, SIZE);
        assert!(matches!(r.step(&mut o), FinalStatus::Running { .. }));
        r.cancel();
        assert_eq!(r.step(&mut o), FinalStatus::Cancelled);
        assert_eq!(r.step(&mut o), FinalStatus::Cancelled);
        assert_eq!(o.final_renders(), 0);
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
    fn final_render_leaves_the_interactive_cache_alone() {
        let Some(mut o) = off() else { return };
        let s = scene(false);
        assert!(o.lit_frame(&s, 1, [10.0, 0.0, 6.0], &night(), false).is_some());
        let slots = o.shadowed_lights();
        assert!(render_final(&mut o, &s, 1, &camera(), &night(), false, SIZE).is_ok());
        assert_eq!(o.shadowed_lights(), slots);
        assert_eq!(o.final_renders(), 1);
    }
}
