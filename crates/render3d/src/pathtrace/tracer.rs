//! The progressive path tracer (phase 11): uploads a scene, then each [`PathTracer::step`] adds
//! `spp_per_step` samples per pixel (one `trace_main` dispatch, `sample_base` advancing), keeps
//! the requested snapshots, and at the target resolves (and denoises) the image. A fixed seed
//! gives the same image on the same adapter. [`irradiance`] estimates lux at probe points with the
//! same kernel.

use super::device::ComputeGpu;
use super::rt::RqBuild;
use super::scene_gpu::{
    FRAME_FURNACE, FRAME_GROUND, FRAME_SKY, FRAME_SUN, FRAME_SUN_NO_SHADOW, GpuFrame, GpuProbe, GpuScene, MAX_GPU_TRIANGLES, build_gpu_scene,
};
use super::shaders::{PtPipelines, RqPtPipelines, build_rq_pipelines};
use super::{DAY_EXPOSURE, Exposure, MAX_SNAPSHOTS, MAX_SPP, NIGHT_EXPOSURE, SKY_HORIZON_NITS, SKY_ZENITH_NITS};
use super::{PathOptions, PathStats, PathStatus};
use crate::camera::Camera;
use crate::lit_frame::Lighting;
use crate::types::Scene3d;
use bytemuck::Pod;
use cadcraft_doc::arch::ImageRgba;
use cadcraft_doc::arch::backdrop::{Backdrop, Projection};
use std::collections::BTreeMap;
use wgpu::util::DeviceExt;

/// A path-traced still in progress.
pub struct PathTracer {
    size: (u32, u32),
    status: PathStatus,
    stats: PathStats,
    cancelled: bool,
    opts: PathOptions,
    frame: GpuFrame,
    exposure: f32,
    buffers: super::tracer::SceneBuffers,
    accum: wgpu::Buffer,
    aov: wgpu::Buffer,
    probes_buf: wgpu::Buffer,
    pipelines: PtPipelines,
    rq_build: Option<RqBuild>,
    rq_pipelines: Option<RqPtPipelines>,
    spp_done: u32,
    snapshot_idx: usize,
    backdrop: Option<Backdrop>,
    /// The image an image backdrop shows, under its `Scene3d.textures` key (`item:<id>`); empty otherwise.
    backdrop_images: BTreeMap<String, ImageRgba>,
}

impl PathTracer {
    pub(crate) fn variance_of_mean(px: [f32; 4], sum_sq: f32) -> f32 {
        let n = px[3].max(1.0);
        let m = super::luminance([px[0] / n, px[1] / n, px[2] / n]);
        let variance = sum_sq / n - m * m;
        if !variance.is_finite() || variance <= 0.0 { 0.0 } else { variance / n }
    }

    /// Start a still of `scene` from `camera` at `size` pixels, lit by `lighting` (its sun, or
    /// night) and the scene's lights. `Err` when the size is outside 1..=the device's largest
    /// buffer, the options are out of range, the scene has more than `MAX_GPU_TRIANGLES`, or the
    /// pipelines do not build. `cache`: the pipeline cache to build with (see `super::cache`).
    pub fn new(
        gpu: &ComputeGpu,
        scene: &Scene3d,
        camera: &Camera,
        lighting: &Lighting,
        ground: bool,
        size: (u32, u32),
        opts: &PathOptions,
        cache: Option<&wgpu::PipelineCache>,
    ) -> Result<PathTracer, String> {
        let (w, h) = size;
        if w == 0 || h == 0 {
            return Err(format!("size {w}x{h} is empty"));
        }
        let max_dim = gpu.device.limits().max_texture_dimension_2d;
        if w > max_dim || h > max_dim {
            return Err(format!("size {w}x{h} exceeds max_texture_dimension_2d {max_dim}"));
        }
        let pixels = u64::from(w) * u64::from(h);
        let accum_bytes = pixels * 32;
        let max_binding = u64::from(gpu.device.limits().max_storage_buffer_binding_size);
        if accum_bytes > max_binding {
            return Err(format!("accum buffer {accum_bytes} bytes exceeds max_storage_buffer_binding_size {max_binding}"));
        }
        if opts.spp == 0 || opts.spp > MAX_SPP {
            return Err(format!("spp {} outside 1..={MAX_SPP}", opts.spp));
        }
        if opts.bounces > super::MAX_BOUNCES {
            return Err(format!("bounces {} exceeds MAX_BOUNCES {}", opts.bounces, super::MAX_BOUNCES));
        }
        if opts.spp_per_step == 0 {
            return Err("spp_per_step must be at least 1".into());
        }
        if opts.snapshots.len() > MAX_SNAPSHOTS {
            return Err(format!("{} snapshots exceeds MAX_SNAPSHOTS {MAX_SNAPSHOTS}", opts.snapshots.len()));
        }
        let mut prev = 0u32;
        for &s in &opts.snapshots {
            if s == 0 || s > opts.spp || s <= prev {
                return Err(format!("snapshot {s} invalid (must be ascending, 1..=spp, and <= {MAX_SNAPSHOTS} total)"));
            }
            prev = s;
        }
        if let Some(f) = opts.furnace {
            if !(0.0 < f.albedo && f.albedo < 1.0) {
                return Err(format!("furnace albedo {} outside (0, 1)", f.albedo));
            }
            if !(0.0 < f.emission && f.emission <= 1000.0) {
                return Err(format!("furnace emission {} outside (0, 1000]", f.emission));
            }
        }

        let mut gpu_scene = build_gpu_scene(scene, ground, opts.furnace);
        if gpu_scene.tris.len() > MAX_GPU_TRIANGLES {
            return Err(format!("scene has {} triangles, more than MAX_GPU_TRIANGLES {MAX_GPU_TRIANGLES}", gpu_scene.tris.len()));
        }

        let pipelines = PtPipelines::new(gpu, cache)?;
        let rq_build = if opts.rt && gpu.ray_query { RqBuild::build(gpu, &gpu_scene) } else { None };
        // A ray-query kernel that does not build renders the compute path, and says why (rt_error): until
        // 2026-11-03 `.ok()` dropped the error and the kernel had never compiled on any adapter.
        let (rq_pipelines, rt_error) = match rq_build.as_ref().map(|_| build_rq_pipelines(gpu, cache)) {
            Some(Ok(p)) => (Some(p), None),
            Some(Err(e)) => (None, Some(e)),
            None => (None, None),
        };
        let d = &gpu.device;

        let accum = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pt accum"),
            size: accum_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let aov = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pt aov"),
            size: pixels * 32,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let probes_buf = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pt probes"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let vp = crate::camera::view_proj(camera, f64::from(w) / f64::from(h));
        let inv_vp = invert_mat4(&vp);

        let exposure = match opts.exposure {
            Exposure::Fixed(x) => x,
            Exposure::Auto if opts.furnace.is_some() => 1.0,
            Exposure::Auto if lighting.sun.is_some() => DAY_EXPOSURE,
            Exposure::Auto => lighting.exposure,
        };

        let eye = camera.eye();
        let mut frame = GpuFrame {
            inv_view_proj: inv_vp,
            eye: [eye.x as f32, eye.y as f32, eye.z as f32, 0.0],
            size: [w, h],
            sample_base: 0,
            seed: opts.seed,
            bounces: opts.bounces,
            light_count: gpu_scene.lights.len() as u32,
            flags: if ground { FRAME_GROUND } else { 0 },
            probe_count: 0,
            look: [1, 0, 0, 0],
            ..Default::default()
        };
        if let Some(f) = opts.furnace {
            frame.flags |= FRAME_FURNACE;
            frame.furnace = [f.albedo, f.emission, f.emission / (1.0 - f.albedo), 0.0];
        } else {
            frame_lighting(&mut frame, &mut gpu_scene.ies, lighting, Some(camera), opts);
        }
        let buffers = super::tracer::upload_scene(gpu, &gpu_scene);

        Ok(PathTracer {
            size,
            status: PathStatus::Running { progress: 0.0, spp: 0 },
            stats: PathStats { rt: rq_pipelines.is_some(), rt_error, exposure, ..PathStats::default() },
            cancelled: false,
            opts: opts.clone(),
            frame,
            exposure,
            buffers,
            accum,
            aov,
            probes_buf,
            pipelines,
            rq_build,
            rq_pipelines,
            spp_done: 0,
            snapshot_idx: 0,
            backdrop: scene.backdrop.clone(),
            backdrop_images: crate::backdrop::image_textures(scene.backdrop.as_ref(), &scene.textures),
        })
    }

    /// One bounded piece of work: up to `spp_per_step` samples per pixel, or, at the target, the
    /// resolve (and denoise). After `Done`, `Cancelled` or `Failed` it keeps returning that status.
    pub fn step(&mut self, gpu: &ComputeGpu) -> PathStatus {
        if self.cancelled && matches!(self.status, PathStatus::Running { .. }) {
            self.status = PathStatus::Cancelled;
            return self.status.clone();
        }
        if !matches!(self.status, PathStatus::Running { .. }) {
            return self.status.clone();
        }

        let remaining = self.opts.spp - self.spp_done;
        if remaining == 0 {
            return self.finish(gpu);
        }
        let this_pass = remaining.min(self.opts.spp_per_step);

        let mut frame = self.frame;
        frame.sample_base = self.spp_done;
        frame.spp_this_pass = this_pass;

        let d = &gpu.device;
        let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
        let frame_buf = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pt frame"),
            contents: bytemuck::bytes_of(&frame),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let (w, h) = self.size;
        if let (Some(build), Some(rq)) = (self.rq_build.as_ref(), self.rq_pipelines.as_ref()) {
            let kernel_entries = &[
                wgpu::BindGroupEntry { binding: 0, resource: frame_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.buffers.nodes.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.buffers.tris.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.buffers.materials.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: self.buffers.lights.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: self.buffers.ies.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: self.accum.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: self.aov.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 8, resource: self.probes_buf.as_entire_binding() },
            ];
            let bind0 = d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("pt rq trace g0"),
                layout: &rq.kernel_bind_group_layout,
                entries: kernel_entries,
            });
            let acc_entries = &[wgpu::BindGroupEntry { binding: 0, resource: build.tlas.as_binding() }];
            let bind1 =
                d.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("pt rq acc"), layout: &rq.acc_bind_group_layout, entries: acc_entries });
            let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pt rq trace") });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("pt rq trace"), timestamp_writes: None });
                pass.set_pipeline(&rq.rq_trace);
                pass.set_bind_group(0, &bind0, &[]);
                pass.set_bind_group(1, &bind1, &[]);
                pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
            }
            gpu.queue.submit([enc.finish()]);
            if let Some(e) = futures::executor::block_on(scope.pop()) {
                self.status = PathStatus::Failed(format!("rq trace dispatch: {e}"));
                return self.status.clone();
            }
        } else {
            let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("pt trace"),
                layout: &self.pipelines.kernel_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: frame_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.buffers.nodes.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.buffers.tris.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: self.buffers.materials.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: self.buffers.lights.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 5, resource: self.buffers.ies.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 6, resource: self.accum.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 7, resource: self.aov.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 8, resource: self.probes_buf.as_entire_binding() },
                ],
            });
            let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pt trace") });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("pt trace"), timestamp_writes: None });
                pass.set_pipeline(&self.pipelines.trace);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
            }
            gpu.queue.submit([enc.finish()]);
            if let Some(e) = futures::executor::block_on(scope.pop()) {
                self.status = PathStatus::Failed(format!("trace dispatch: {e}"));
                return self.status.clone();
            }
        }

        self.spp_done += this_pass;
        self.stats.spp_done = self.spp_done;

        // Check for a snapshot at this spp.
        if self.snapshot_idx < self.opts.snapshots.len() && self.spp_done >= self.opts.snapshots[self.snapshot_idx] {
            let snap_spp = self.opts.snapshots[self.snapshot_idx];
            if let Some(mut rgba) = self.resolve(gpu) {
                self.backdrop_composite(gpu, &mut rgba);
                self.stats.snapshots.push((snap_spp, rgba));
            }
            self.snapshot_idx += 1;
        }

        let progress = self.spp_done as f32 / self.opts.spp as f32;
        self.status = PathStatus::Running { progress, spp: self.spp_done };

        if self.spp_done >= self.opts.spp {
            // Return Running with progress 1.0. The next call to step() will see remaining == 0
            // and call finish(). This ensures progress rises to 1.0 in a Running state.
            return self.status.clone();
        }

        self.status.clone()
    }

    fn compute_auto_exposure(&self, gpu: &ComputeGpu) -> f32 {
        let pixels = (self.size.0 * self.size.1) as usize;
        if pixels == 0 {
            return DAY_EXPOSURE;
        }
        let accum_bytes = (pixels * 32) as u64;
        let aov_bytes = (pixels * 32) as u64;
        let accum_data = match self.read_buffer(gpu, &self.accum, accum_bytes) {
            Ok(d) => d,
            Err(_) => return DAY_EXPOSURE,
        };
        let aov_data = match self.read_buffer(gpu, &self.aov, aov_bytes) {
            Ok(d) => d,
            Err(_) => return DAY_EXPOSURE,
        };
        let accum: Vec<[f32; 4]> = bytemuck::cast_slice(&accum_data).to_vec();
        let aov: Vec<[f32; 4]> = bytemuck::cast_slice(&aov_data).to_vec();
        let mean_of = |p: &[f32; 4]| {
            let n = p[3].max(1.0);
            [p[0] / n, p[1] / n, p[2] / n]
        };
        let radiance = if self.opts.denoise {
            let colors: Vec<[f32; 4]> = accum[..pixels]
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let [r, g, b] = mean_of(p);
                    let variance = Self::variance_of_mean(*p, accum[pixels + i][0]);
                    [r, g, b, variance]
                })
                .collect();
            match super::denoise::denoise(gpu, &colors, &aov, self.size) {
                Ok(denoised) => denoised,
                Err(_) => accum[..pixels].iter().map(|p| [mean_of(p)[0], mean_of(p)[1], mean_of(p)[2], 0.0]).collect(),
            }
        } else {
            accum[..pixels].iter().map(|p| [mean_of(p)[0], mean_of(p)[1], mean_of(p)[2], 0.0]).collect()
        };
        let mut log_sum = 0.0f64;
        let mut count = 0usize;
        for (i, px) in radiance.iter().enumerate() {
            if aov[2 * i][3] <= 0.5 {
                continue;
            }
            let lum = super::luminance([px[0], px[1], px[2]]).max(0.0);
            log_sum += (1e-4 + lum as f64).ln();
            count += 1;
        }
        if count == 0 {
            return DAY_EXPOSURE;
        }
        let lavg = (log_sum / count as f64).exp() as f32;
        (0.18 / lavg).clamp(DAY_EXPOSURE, NIGHT_EXPOSURE)
    }

    fn finish(&mut self, gpu: &ComputeGpu) -> PathStatus {
        let (w, h) = self.size;
        let pixels = (w * h) as usize;
        let accum_bytes = (pixels * 32) as u64;
        let aov_bytes = (pixels * 32) as u64;

        // Read back the full accumulator: the first half keeps the radiance sum, and the second
        // stores the luminance-squared sum used to estimate the per-pixel variance of the mean.
        let accum_data = match self.read_buffer(gpu, &self.accum, accum_bytes) {
            Ok(d) => d,
            Err(e) => {
                self.status = PathStatus::Failed(format!("read accum: {e}"));
                return self.status.clone();
            }
        };
        let accum: Vec<[f32; 4]> = bytemuck::cast_slice(&accum_data).to_vec();
        let colors_accum = &accum[..pixels];
        // Each pixel holds its radiance summed over its samples, with the sample count in w.
        let mean_of = |p: &[f32; 4]| {
            let n = p[3].max(1.0);
            [p[0] / n, p[1] / n, p[2] / n]
        };

        // Compute radiance stats.
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let mut sum = 0.0f64;
        for px in colors_accum {
            let l = super::luminance(mean_of(px));
            min = min.min(l);
            max = max.max(l);
            sum += l as f64;
        }
        let mean = if pixels > 0 { (sum / pixels as f64) as f32 } else { 0.0 };
        self.stats.radiance = Some(super::RadianceStats { min, max, mean, ..Default::default() });

        // Denoise if requested.
        let final_accum = if self.opts.denoise {
            let aov_data = match self.read_buffer(gpu, &self.aov, aov_bytes) {
                Ok(d) => d,
                Err(e) => {
                    self.status = PathStatus::Failed(format!("read aov: {e}"));
                    return self.status.clone();
                }
            };
            let aov: Vec<[f32; 4]> = bytemuck::cast_slice(&aov_data).to_vec();
            let colors: Vec<[f32; 4]> = colors_accum
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let [r, g, b] = mean_of(p);
                    let variance = Self::variance_of_mean(*p, accum[pixels + i][0]);
                    [r, g, b, variance]
                })
                .collect();
            match super::denoise::denoise(gpu, &colors, &aov, self.size) {
                Ok(denoised) => denoised,
                Err(e) => {
                    self.status = PathStatus::Failed(format!("denoise: {e}"));
                    return self.status.clone();
                }
            }
        } else {
            colors_accum.to_vec()
        };

        // Write denoised result back into the first half of accum so resolve reads it.
        let d = &gpu.device;
        let src = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pt denoised"),
            contents: bytemuck::cast_slice(&final_accum),
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pt write denoised") });
        enc.copy_buffer_to_buffer(&src, 0, &self.accum, 0, (pixels * 16) as u64);
        gpu.queue.submit([enc.finish()]);

        self.stats.denoised = self.opts.denoise;
        self.exposure = match self.opts.exposure {
            Exposure::Fixed(x) => x,
            Exposure::Auto if self.frame.flags & super::scene_gpu::FRAME_FURNACE != 0 => 1.0,
            Exposure::Auto if self.frame.flags & super::scene_gpu::FRAME_SUN != 0 => self.compute_auto_exposure(gpu),
            Exposure::Auto => self.exposure,
        };
        self.stats.exposure = self.exposure;

        // Resolve.
        match self.resolve(gpu) {
            Some(mut rgba) => {
                self.backdrop_composite(gpu, &mut rgba);
                self.status = PathStatus::Done { width: w, height: h, rgba };
            }
            None => {
                self.status = PathStatus::Failed("resolve failed".into());
            }
        }
        self.status.clone()
    }

    /// Paint the backdrop over the pixels whose camera ray hit nothing on every sample (the hit count the kernel keeps
    /// in `accum[w * h + i].y`, kernel.wgsl `trace_main`). The colours are the shared CPU reference
    /// (`crate::backdrop::composite_misses`), with the unjittered ray through each pixel centre for an equirect image.
    /// A pixel any sample hit keeps its shading, and lighting never sees the backdrop.
    ///
    /// 11-04: this painted only the misses darker than 0.9 x the mean miss, so under a sunlit sky (every miss about
    /// as bright as the others) the sky stayed and phase-14 check 09 read (108, 108, 108) for "#204060"; images had
    /// their own sampler, which read the wrong texel (flat) or a fixed grey (equirect), with no image ever passed in.
    /// The first sample's AOV hit flag is not the mask: its jittered ray misses on some edge pixels other samples hit.
    fn backdrop_composite(&self, gpu: &ComputeGpu, rgba: &mut [u8]) {
        let Some(backdrop) = &self.backdrop else { return };
        let (w, h) = self.size;
        let pixels = (w as usize) * (h as usize);
        let Ok(data) = self.read_buffer(gpu, &self.accum, (pixels * 32) as u64) else { return };
        let Ok(accum) = bytemuck::try_cast_slice::<u8, [f32; 4]>(&data) else { return };
        // The first half may hold the denoised colours by now (`finish`); the hit counts are in the second.
        let sampled = self.spp_done > 0;
        let missed: Vec<bool> = (0..pixels).map(|i| sampled && accum.get(pixels + i).is_some_and(|a| a[1] < 0.5)).collect();
        let inv_vp = self.frame.inv_view_proj;
        let ray_of = move |x: u32, y: u32| pixel_ray(&inv_vp, x, y, w, h);
        crate::backdrop::composite_misses(rgba, w, h, &missed, backdrop, &self.backdrop_images, &ray_of);
    }

    fn resolve(&self, gpu: &ComputeGpu) -> Option<Vec<u8>> {
        let (w, h) = self.size;
        let pixels = (w * h) as usize;
        let accum_bytes = (pixels * 16) as u64;
        let rgba_bytes = (pixels * 4) as u64;
        let d = &gpu.device;

        #[repr(C)]
        #[derive(Clone, Copy, Pod, bytemuck::Zeroable)]
        struct ResolveParams {
            size: [u32; 2],
            exposure: f32,
            tonemap: u32,
        }
        let exposure = match self.opts.exposure {
            Exposure::Fixed(x) => x,
            Exposure::Auto if self.frame.flags & super::scene_gpu::FRAME_FURNACE != 0 => 1.0,
            Exposure::Auto if self.frame.flags & super::scene_gpu::FRAME_SUN != 0 => self.compute_auto_exposure(gpu),
            Exposure::Auto => self.exposure,
        };
        let params = ResolveParams { size: [w, h], exposure, tonemap: self.frame.look[0] };

        let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
        let uniform = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pt resolve params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let pixels_buf = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pt resolve out"),
            size: rgba_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pt resolve"),
            layout: &self.pipelines.resolve_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.accum.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: pixels_buf.as_entire_binding() },
            ],
        });
        let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pt resolve") });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("pt resolve"), timestamp_writes: None });
            pass.set_pipeline(&self.pipelines.resolve);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        let read = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pt resolve read"),
            size: rgba_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        enc.copy_buffer_to_buffer(&pixels_buf, 0, &read, 0, rgba_bytes);
        gpu.queue.submit([enc.finish()]);
        if let Some(e) = futures::executor::block_on(scope.pop()) {
            return None;
        }
        let slice = read.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = d.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(300)) });
        let _ = rx.recv();
        let out: Vec<u8> = {
            let view = slice.get_mapped_range().ok()?;
            view.to_vec()
        };
        read.unmap();
        Some(out)
    }

    fn read_buffer(&self, gpu: &ComputeGpu, buf: &wgpu::Buffer, size: u64) -> Result<Vec<u8>, String> {
        let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pt read"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pt read") });
        enc.copy_buffer_to_buffer(buf, 0, &read, 0, size);
        gpu.queue.submit([enc.finish()]);
        let slice = read.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        gpu.device
            .poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(300)) })
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
        let out: Vec<u8> = {
            let view = slice.get_mapped_range().map_err(|e| e.to_string())?;
            view.to_vec()
        };
        read.unmap();
        Ok(out)
    }

    /// The image so far (resolved, not denoised), for the pane while it refines.
    pub fn preview(&self, gpu: &ComputeGpu) -> Option<(u32, u32, Vec<u8>)> {
        if self.spp_done == 0 {
            return None;
        }
        let rgba = self.resolve(gpu)?;
        Some((self.size.0, self.size.1, rgba))
    }

    /// Stop at the next step.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn stats(&self) -> PathStats {
        self.stats.clone()
    }
}

/// Invert a 4x4 column-major matrix (the view-projection). Returns the inverse or an identity if
/// singular.
/// Fills the lighting fields of `frame` (and appends to `aux`, the f32 buffer at binding 5, whose IES
/// tables stay first) from `lighting`, the optional `camera` and `opts`. The one place the sun, sky,
/// environment, moon, view clip and look are set; `new` and `irradiance` both call it.
fn frame_lighting(frame: &mut GpuFrame, aux: &mut Vec<f32>, lighting: &Lighting, camera: Option<&Camera>, opts: &PathOptions) {
    frame_sun(frame, lighting);
    frame_environment(frame, aux, lighting);
    frame_moon(frame, lighting);
    frame_view(frame, lighting, camera);
    frame_look(frame, lighting, opts);
}

/// Sun and sky. The sun (`FRAME_SUN`, `sun_dir`, `sun_rgb` = the look's lux) only when it is on and
/// above the horizon, `FRAME_SUN_NO_SHADOW` when the look has cast shadows off; the gradient sky
/// (`FRAME_SKY`, `sky_zenith`, `sky_horizon`) only when the look's sky is on as well.
fn frame_sun(frame: &mut GpuFrame, lighting: &Lighting) {
    if let Some(sun) = lighting.sun {
        let d = sun.normalized().dir;
        if lighting.look.sun.on && d[2] > 0.0 {
            frame.flags |= FRAME_SUN;
            if !lighting.look.sun.shadows {
                frame.flags |= FRAME_SUN_NO_SHADOW;
            }
            let lux = lighting.look.sun.intensity as f32;
            frame.sun_dir = [d[0] as f32, d[1] as f32, d[2] as f32, 0.0];
            frame.sun_rgb = [lux, lux, lux, 0.0];
        }
        if lighting.look.sky.on {
            frame.flags |= FRAME_SKY;
            frame.sky_zenith = [SKY_ZENITH_NITS; 4];
            frame.sky_horizon = [SKY_HORIZON_NITS; 4];
        }
    }
}

/// Environment map (stub): will append the map and its CDFs to `aux` and record the offsets in
/// `frame.env` / `env_aux` / `env_params`, from `lighting`. Does nothing yet.
fn frame_environment(_frame: &mut GpuFrame, _aux: &mut Vec<f32>, _lighting: &Lighting) {}

/// Moon (stub): will set `frame.moon_dir` / `moon_rgb` from `lighting`. Does nothing yet.
fn frame_moon(_frame: &mut GpuFrame, _lighting: &Lighting) {}

/// View (stub): will set the clip box, near clip and forward from `lighting` and `camera`. Does
/// nothing yet.
fn frame_view(_frame: &mut GpuFrame, _lighting: &Lighting, _camera: Option<&Camera>) {}

/// Look (stub): will set the tone code and the Lambert flag in `frame.look` from `lighting` and `opts`.
/// Does nothing yet (`look[0]` stays 1, ACES).
fn frame_look(_frame: &mut GpuFrame, _lighting: &Lighting, _opts: &PathOptions) {}

fn invert_mat4(m: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    // Standard 4x4 matrix inversion via cofactors.
    let a = m;
    let b00 = a[0][0] * a[1][1] - a[0][1] * a[1][0];
    let b01 = a[0][0] * a[1][2] - a[0][2] * a[1][0];
    let b02 = a[0][0] * a[1][3] - a[0][3] * a[1][0];
    let b03 = a[0][1] * a[1][2] - a[0][2] * a[1][1];
    let b04 = a[0][1] * a[1][3] - a[0][3] * a[1][1];
    let b05 = a[0][2] * a[1][3] - a[0][3] * a[1][2];
    let b06 = a[2][0] * a[3][1] - a[2][1] * a[3][0];
    let b07 = a[2][0] * a[3][2] - a[2][2] * a[3][0];
    let b08 = a[2][0] * a[3][3] - a[2][3] * a[3][0];
    let b09 = a[2][1] * a[3][2] - a[2][2] * a[3][1];
    let b10 = a[2][1] * a[3][3] - a[2][3] * a[3][1];
    let b11 = a[2][2] * a[3][3] - a[2][3] * a[3][2];

    let det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;
    if !det.is_finite() || det.abs() < 1e-12 {
        return [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
    }
    let inv = 1.0 / det;
    let mut out = [[0.0f32; 4]; 4];
    out[0][0] = (a[1][1] * b11 - a[1][2] * b10 + a[1][3] * b09) * inv;
    out[0][1] = (a[0][2] * b10 - a[0][1] * b11 - a[0][3] * b09) * inv;
    out[0][2] = (a[3][1] * b05 - a[3][2] * b04 + a[3][3] * b03) * inv;
    out[0][3] = (a[2][2] * b04 - a[2][1] * b05 - a[2][3] * b03) * inv;
    out[1][0] = (a[1][2] * b08 - a[1][0] * b11 - a[1][3] * b07) * inv;
    out[1][1] = (a[0][0] * b11 - a[0][2] * b08 + a[0][3] * b07) * inv;
    out[1][2] = (a[3][2] * b02 - a[3][0] * b05 - a[3][3] * b01) * inv;
    out[1][3] = (a[2][0] * b05 - a[2][2] * b02 + a[2][3] * b01) * inv;
    out[2][0] = (a[1][0] * b10 - a[1][1] * b08 + a[1][3] * b06) * inv;
    out[2][1] = (a[0][1] * b08 - a[0][0] * b10 - a[0][3] * b06) * inv;
    out[2][2] = (a[3][0] * b04 - a[3][1] * b02 + a[3][3] * b00) * inv;
    out[2][3] = (a[2][1] * b02 - a[2][0] * b04 - a[2][3] * b00) * inv;
    out[3][0] = (a[1][1] * b07 - a[1][0] * b09 - a[1][2] * b06) * inv;
    out[3][1] = (a[0][0] * b09 - a[0][1] * b07 + a[0][2] * b06) * inv;
    out[3][2] = (a[3][1] * b01 - a[3][0] * b03 - a[3][2] * b00) * inv;
    out[3][3] = (a[2][0] * b03 - a[2][1] * b01 + a[2][2] * b00) * inv;
    out
}

/// Irradiance (linear RGB lux; `super::luminance` of it is the illuminance) at each probe
/// (`[point, unit normal]`) from the scene's lights, `lighting`'s sun and sky and, with
/// `opts.bounces > 0`, bounced light, averaged over `opts.spp` samples with `opts.seed`. At most
/// 4096 probes.
pub fn irradiance(
    gpu: &ComputeGpu,
    scene: &Scene3d,
    lighting: &Lighting,
    ground: bool,
    probes: &[[[f64; 3]; 2]],
    opts: &PathOptions,
) -> Result<Vec<[f32; 3]>, String> {
    if probes.len() > 4096 {
        return Err(format!("{} probes exceeds the 4096 limit", probes.len()));
    }
    if opts.spp == 0 {
        return Err("spp must be at least 1".into());
    }

    let mut gpu_scene = build_gpu_scene(scene, ground, opts.furnace);
    // As before, irradiance applies the sun whenever `lighting` has one, furnace or not.
    let mut lit = GpuFrame { look: [1, 0, 0, 0], ..Default::default() };
    frame_lighting(&mut lit, &mut gpu_scene.ies, lighting, None, opts);
    let buffers = super::tracer::upload_scene(gpu, &gpu_scene);
    let pipelines = PtPipelines::new(gpu, None)?;

    let gpu_probes: Vec<GpuProbe> = probes
        .iter()
        .map(|[point, normal]| GpuProbe {
            point: [point[0] as f32, point[1] as f32, point[2] as f32],
            _a: 0.0,
            normal: [normal[0] as f32, normal[1] as f32, normal[2] as f32],
            _b: 0.0,
            ..Default::default()
        })
        .collect();

    let n = gpu_probes.len().max(1);
    let d = &gpu.device;

    // Accumulate over passes of 64 samples.
    let accum_size = (n * 16) as u64;
    let accum = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pt irr accum"),
        size: accum_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let aov = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pt irr aov"),
        size: (n * 16) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let probe_buf = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("pt irr probes"),
        contents: bytemuck::cast_slice(&gpu_probes),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let mut sample_base = 0u32;
    let mut remaining = opts.spp;
    while remaining > 0 {
        let this_pass = remaining.min(64);
        let frame = GpuFrame {
            inv_view_proj: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            size: [1, 1],
            sample_base,
            seed: opts.seed,
            bounces: opts.bounces,
            light_count: gpu_scene.lights.len() as u32,
            spp_this_pass: this_pass,
            probe_count: gpu_probes.len() as u32,
            ..lit
        };

        let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
        let frame_buf = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pt irr frame"),
            contents: bytemuck::bytes_of(&frame),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pt irradiance"),
            layout: &pipelines.kernel_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: frame_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: buffers.nodes.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: buffers.tris.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: buffers.materials.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: buffers.lights.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: buffers.ies.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: accum.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: aov.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 8, resource: probe_buf.as_entire_binding() },
            ],
        });
        let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pt irradiance") });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("pt irradiance"), timestamp_writes: None });
            pass.set_pipeline(&pipelines.irradiance);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups((n as u32).div_ceil(64), 1, 1);
        }
        gpu.queue.submit([enc.finish()]);
        if let Some(e) = futures::executor::block_on(scope.pop()) {
            return Err(format!("irradiance dispatch: {e}"));
        }
        sample_base += this_pass;
        remaining -= this_pass;
    }

    // Read back accum.
    let read = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pt irr read"),
        size: accum_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pt irr read") });
    enc.copy_buffer_to_buffer(&accum, 0, &read, 0, accum_size);
    gpu.queue.submit([enc.finish()]);
    let slice = read.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    d.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(300)) }).map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    let out: Vec<[f32; 4]> = {
        let view = slice.get_mapped_range().map_err(|e| e.to_string())?;
        bytemuck::cast_slice(&view).to_vec()
    };
    read.unmap();

    let spp = opts.spp as f32;
    let result: Vec<[f32; 3]> = out.into_iter().take(probes.len()).map(|v| [v[0] / spp, v[1] / spp, v[2] / spp]).collect();
    Ok(result)
}

/// A `GpuScene` in storage buffers (an empty list becomes one zeroed element: bindings cannot be empty).
pub struct SceneBuffers {
    pub nodes: wgpu::Buffer,
    pub tris: wgpu::Buffer,
    pub materials: wgpu::Buffer,
    pub lights: wgpu::Buffer,
    pub ies: wgpu::Buffer,
}

/// The unit camera ray through the centre of pixel (x, y) of a w x h still: the kernel's primary ray (kernel.wgsl
/// `trace_main`) without its jitter. Straight up when the matrix gives no direction.
fn pixel_ray(inv_vp: &[[f32; 4]; 4], x: u32, y: u32, w: u32, h: u32) -> [f32; 3] {
    let ndc_x = 2.0 * (f64::from(x) + 0.5) / f64::from(w.max(1)) - 1.0;
    let ndc_y = 1.0 - 2.0 * (f64::from(y) + 0.5) / f64::from(h.max(1));
    // Column-major, as WGSL reads it: out[r] = sum over c of m[c][r] * v[c].
    let at = |z: f64| {
        let v = [ndc_x, ndc_y, z, 1.0];
        let mut o = [0.0f64; 4];
        for (c, vc) in v.iter().enumerate() {
            for (r, or) in o.iter_mut().enumerate() {
                *or += f64::from(inv_vp[c][r]) * vc;
            }
        }
        [o[0] / o[3], o[1] / o[3], o[2] / o[3]]
    };
    let (near, far) = (at(0.0), at(1.0));
    let d = [far[0] - near[0], far[1] - near[1], far[2] - near[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if !(len.is_finite() && len > 0.0) {
        return [0.0, 0.0, 1.0];
    }
    [(d[0] / len) as f32, (d[1] / len) as f32, (d[2] / len) as f32]
}

fn storage<T: Copy + Pod + Default>(d: &wgpu::Device, label: &str, items: &[T]) -> wgpu::Buffer {
    let one = [T::default()];
    let data: &[T] = if items.is_empty() { &one } else { items };
    d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(data),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    })
}

/// Upload `scene` to `gpu`.
pub fn upload_scene(gpu: &ComputeGpu, scene: &GpuScene) -> SceneBuffers {
    let d = &gpu.device;
    SceneBuffers {
        nodes: storage(d, "pt nodes", &scene.nodes),
        tris: storage(d, "pt tris", &scene.tris),
        materials: storage(d, "pt materials", &scene.materials),
        lights: storage(d, "pt lights", &scene.lights),
        ies: storage(d, "pt ies", &scene.ies),
    }
}

/// Dispatch the kernel module's `entry` (a `@workgroup_size(64)` entry over `frame.probe_count`
/// probes, e.g. `traverse_test_main`, `irradiance_main`, or a test entry a segment adds) once with
/// `frame` and `probes`, and read back `accum[0..probes.len()]` (zeroed before the dispatch).
/// The pipeline is built for the call (tests; the driver keeps `PtPipelines`).
pub fn run_entry(gpu: &ComputeGpu, buffers: &SceneBuffers, entry: &str, frame: &GpuFrame, probes: &[GpuProbe]) -> Result<Vec<[f32; 4]>, String> {
    run_entry_on(gpu, buffers, entry, frame, probes, None)
}

/// [`run_entry`] on the ray-query kernel module, with `scene`'s acceleration structure bound as group 1
/// (tests of the ray-query traversal against `trace_cpu`). Needs a device with ray queries.
#[cfg(test)]
pub(crate) fn run_entry_rq(
    gpu: &ComputeGpu,
    scene: &super::scene_gpu::GpuScene,
    buffers: &SceneBuffers,
    entry: &str,
    frame: &GpuFrame,
    probes: &[GpuProbe],
) -> Result<Vec<[f32; 4]>, String> {
    run_entry_on(gpu, buffers, entry, frame, probes, Some(scene))
}

fn run_entry_on(
    gpu: &ComputeGpu,
    buffers: &SceneBuffers,
    entry: &str,
    frame: &GpuFrame,
    probes: &[GpuProbe],
    rq_scene: Option<&super::scene_gpu::GpuScene>,
) -> Result<Vec<[f32; 4]>, String> {
    let d = &gpu.device;
    let n = probes.len().max(1);
    let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
    let pipelines = super::shaders::PtPipelines::new(gpu, None)?;
    // With a ray-query scene: the rq module, its two-group layout and the scene's TLAS as group 1.
    let rq = match rq_scene {
        Some(scene) => {
            let build = RqBuild::build(gpu, scene).ok_or("no acceleration structure for this scene (empty, or no ray queries)")?;
            Some((build, build_rq_pipelines(gpu, None)?))
        }
        None => None,
    };
    let source = if rq.is_some() { super::shaders::rq_kernel_source() } else { super::shaders::kernel_source() };
    let module =
        d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("pt test kernel"), source: wgpu::ShaderSource::Wgsl(source.into()) });
    let plain_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pt test"),
        bind_group_layouts: &[Some(&pipelines.kernel_layout)],
        immediate_size: 0,
    });
    let layout = rq.as_ref().map_or(&plain_layout, |(_, p)| &p.rq_pipeline_layout);
    let kernel_layout = rq.as_ref().map_or(&pipelines.kernel_layout, |(_, p)| &p.kernel_bind_group_layout);
    let pipeline = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(entry),
        layout: Some(layout),
        module: &module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    });
    let frame_buf = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("pt frame"),
        contents: bytemuck::bytes_of(frame),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let aov = storage(d, "pt aov", &[[0.0f32; 4]; 2]);
    let probe_buf = storage(d, "pt probes", probes);
    let read = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pt read"),
        size: (n * 32) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    // Buffers start zeroed. The trace kernel appends the luminance-squared moment to a second
    // per-pixel block, so the accumulator covers both the colour sum and the variance guide.
    let accum_src = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pt accum"),
        size: (n * 32) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pt test"),
        layout: kernel_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: frame_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: buffers.nodes.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: buffers.tris.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: buffers.materials.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 4, resource: buffers.lights.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 5, resource: buffers.ies.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 6, resource: accum_src.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 7, resource: aov.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 8, resource: probe_buf.as_entire_binding() },
        ],
    });
    let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pt test") });
    {
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(entry), timestamp_writes: None });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        let acc = rq.as_ref().map(|(build, p)| {
            d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("pt test acc"),
                layout: &p.acc_bind_group_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: build.tlas.as_binding() }],
            })
        });
        if let Some(acc) = acc.as_ref() {
            pass.set_bind_group(1, acc, &[]);
        }
        pass.dispatch_workgroups((n as u32).div_ceil(64), 1, 1);
    }
    enc.copy_buffer_to_buffer(&accum_src, 0, &read, 0, (n * 32) as u64);
    gpu.queue.submit([enc.finish()]);
    if let Some(e) = futures::executor::block_on(scope.pop()) {
        return Err(e.to_string());
    }
    let slice = read.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    d.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(300)) }).map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    let out: Vec<[f32; 4]> = {
        let view = slice.get_mapped_range().map_err(|e| e.to_string())?;
        bytemuck::cast_slice(&view).to_vec()
    };
    read.unmap();
    Ok(out.into_iter().take(probes.len()).collect())
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;
    use crate::camera::{Camera, Orbit};
    use crate::lit_frame::Lighting;
    use crate::types::{ArchId, Mesh, Scene3d};
    use cadcraft_doc::arch::backdrop::Backdrop;
    use cadcraft_geom::{Vec2, Vec3};

    fn floor() -> Mesh {
        let mut m = Mesh::new("floor", ArchId(1));
        let r = [Vec2::new(-8.0, -8.0), Vec2::new(8.0, -8.0), Vec2::new(8.0, 8.0), Vec2::new(-8.0, 8.0)];
        m.push_prism(&r, -0.1, 0.0);
        m
    }

    fn scene() -> Scene3d {
        let m = floor();
        Scene3d { bounds: m.bounds(), meshes: vec![m], lights: Vec::new(), ..Default::default() }
    }

    fn camera() -> Camera {
        Camera::Orbit(Orbit::at_origin(0.0, 30.0, 20.0, 45.0))
    }

    /// The software adapter's device with ray queries asked for (llvmpipe has them); `None` (the test skips)
    /// when there is no compute device, `ray_query` false when the adapter offers none.
    fn compute_rq() -> Option<ComputeGpu> {
        let gpu = crate::gputest::shared_compute_rq();
        if gpu.is_none() {
            eprintln!("SKIP: no compute device");
        }
        gpu
    }

    fn compute() -> Option<ComputeGpu> {
        let gpu = crate::gputest::shared_compute();
        if gpu.is_none() {
            eprintln!("SKIP: no compute device");
        }
        gpu
    }

    #[test]
    fn variance_of_mean_of_four_samples() {
        assert!(
            (PathTracer::variance_of_mean([1.0, 1.0, 1.0, 4.0], 1.0) - 0.046875).abs() <= 1e-6,
            "{:?}",
            PathTracer::variance_of_mean([1.0, 1.0, 1.0, 4.0], 1.0)
        );
        assert!(PathTracer::variance_of_mean([2.0, 2.0, 2.0, 4.0], 1.0).abs() <= 1e-6, "constant pixel is variance 0");
        assert!(PathTracer::variance_of_mean([0.0, 0.0, 0.0, 0.0], 1.0).is_finite(), "w = 0 is treated as one sample");
    }

    #[test]
    fn run_entry_reads_back_a_probe_per_ray() {
        let Some(gpu) = compute() else { return };
        let buffers = upload_scene(&gpu, &GpuScene::default());
        let probes = vec![GpuProbe { normal: [0.0, 0.0, 1.0], ..Default::default() }; 3];
        let frame = GpuFrame { probe_count: 3, ..Default::default() };
        match run_entry(&gpu, &buffers, "traverse_test_main", &frame, &probes) {
            Ok(out) => {
                assert_eq!(out.len(), 3);
                assert!(out.iter().all(|h| h[0] < 0.0), "an empty scene misses every ray: {out:?}");
            }
            Err(e) => assert!(false, "{e}"),
        }
    }

    /// Run `opts` on the floor to the end: (the radiance stats' mean, the resolved image's mean byte).
    fn finished(gpu: &ComputeGpu, opts: &PathOptions) -> (f32, f64) {
        let mut t = PathTracer::new(gpu, &scene(), &camera(), &Lighting::default(), false, (16, 16), opts, None).unwrap();
        loop {
            match t.step(gpu) {
                PathStatus::Running { .. } => {}
                PathStatus::Done { rgba, .. } => {
                    let rgb: Vec<f64> = rgba.chunks_exact(4).flat_map(|p| p[..3].iter().map(|&c| f64::from(c))).collect();
                    let mean = t.stats().radiance.expect("radiance stats").mean;
                    return (mean, rgb.iter().sum::<f64>() / rgb.len() as f64);
                }
                other => panic!("the render ended {other:?}"),
            }
        }
    }

    #[test]
    fn radiance_stats_do_not_grow_with_the_sample_count() {
        let Some(gpu) = compute() else { return };
        let (four, _) = finished(&gpu, &PathOptions { spp: 4, denoise: false, ..Default::default() });
        let (sixteen, _) = finished(&gpu, &PathOptions { spp: 16, denoise: false, ..Default::default() });
        assert!(four > 0.0, "the floor is lit: {four}");
        let ratio = sixteen / four;
        assert!((0.8..=1.25).contains(&ratio), "the stats are per-sample means: spp 4 {four}, spp 16 {sixteen}");
    }

    #[test]
    fn the_denoised_still_keeps_the_raw_brightness() {
        let Some(gpu) = compute() else { return };
        let (_, raw) = finished(&gpu, &PathOptions { spp: 16, denoise: false, ..Default::default() });
        let (_, denoised) = finished(&gpu, &PathOptions { spp: 16, denoise: true, ..Default::default() });
        assert!(raw > 5.0 && raw < 250.0, "the raw floor is neither black nor white: {raw}");
        assert!((denoised - raw).abs() <= 0.15 * raw, "denoising keeps the brightness: raw {raw:.1}, denoised {denoised:.1}");
    }

    #[test]
    fn steps_accumulate_to_the_target_spp() {
        let Some(gpu) = compute() else { return };
        let opts = PathOptions { spp: 10, spp_per_step: 4, ..Default::default() };
        let mut t = PathTracer::new(&gpu, &scene(), &camera(), &Lighting::default(), false, (16, 16), &opts, None).unwrap();
        let mut steps = 0;
        while matches!(t.step(&gpu), PathStatus::Running { .. }) {
            steps += 1;
            assert!(steps < 20, "too many steps");
        }
        assert_eq!(steps, 3, "spp 10, spp_per_step 4 -> 3 steps");
        assert_eq!(t.stats().spp_done, 10);
    }

    #[test]
    fn progress_rises_and_ends_done() {
        let Some(gpu) = compute() else { return };
        let opts = PathOptions { spp: 8, spp_per_step: 2, ..Default::default() };
        let mut t = PathTracer::new(&gpu, &scene(), &camera(), &Lighting::default(), false, (8, 8), &opts, None).unwrap();
        let mut last_progress = 0.0f32;
        loop {
            let s = t.step(&gpu);
            match s {
                PathStatus::Running { progress, .. } => {
                    assert!(progress > last_progress, "progress must rise");
                    last_progress = progress;
                }
                PathStatus::Done { .. } => break,
                other => panic!("unexpected status: {other:?}"),
            }
        }
        assert!((last_progress - 1.0).abs() < 1e-6 || last_progress >= 0.99, "final progress {last_progress}");
    }

    #[test]
    fn cancel_stops_at_the_next_step() {
        let Some(gpu) = compute() else { return };
        let opts = PathOptions { spp: 100, spp_per_step: 1, ..Default::default() };
        let mut t = PathTracer::new(&gpu, &scene(), &camera(), &Lighting::default(), false, (8, 8), &opts, None).unwrap();
        t.step(&gpu);
        t.cancel();
        let s = t.step(&gpu);
        assert!(matches!(s, PathStatus::Cancelled), "expected Cancelled, got {s:?}");
    }

    #[test]
    fn the_same_seed_gives_the_same_bytes() {
        let Some(gpu) = compute() else { return };
        let opts = PathOptions { spp: 4, spp_per_step: 4, seed: 42, denoise: false, ..Default::default() };
        let mut t1 = PathTracer::new(&gpu, &scene(), &camera(), &Lighting::default(), false, (8, 8), &opts, None).unwrap();
        loop {
            match t1.step(&gpu) {
                PathStatus::Done { rgba, .. } => {
                    let mut t2 = PathTracer::new(&gpu, &scene(), &camera(), &Lighting::default(), false, (8, 8), &opts, None).unwrap();
                    loop {
                        match t2.step(&gpu) {
                            PathStatus::Done { rgba: rgba2, .. } => {
                                assert_eq!(rgba, rgba2, "same seed must give same bytes");
                                return;
                            }
                            PathStatus::Running { .. } => {}
                            other => panic!("unexpected: {other:?}"),
                        }
                    }
                }
                PathStatus::Running { .. } => {}
                other => panic!("unexpected: {other:?}"),
            }
        }
    }

    #[test]
    fn snapshots_are_kept_at_their_spp() {
        let Some(gpu) = compute() else { return };
        let opts = PathOptions { spp: 8, spp_per_step: 2, snapshots: vec![2, 6], denoise: false, ..Default::default() };
        let mut t = PathTracer::new(&gpu, &scene(), &camera(), &Lighting::default(), false, (8, 8), &opts, None).unwrap();
        loop {
            match t.step(&gpu) {
                PathStatus::Done { .. } => break,
                PathStatus::Running { .. } => {}
                other => panic!("unexpected: {other:?}"),
            }
        }
        let stats = t.stats();
        assert_eq!(stats.snapshots.len(), 2, "two snapshots kept");
        assert_eq!(stats.snapshots[0].0, 2, "first snapshot at spp 2");
        assert_eq!(stats.snapshots[1].0, 6, "second snapshot at spp 6");
    }

    #[test]
    fn irradiance_answers_per_probe() {
        let Some(gpu) = compute() else { return };
        let probes = vec![[[0.0, 0.0, 0.0], [0.0, 0.0, 1.0]], [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]];
        let opts = PathOptions { spp: 4, ..Default::default() };
        match irradiance(&gpu, &scene(), &Lighting::default(), false, &probes, &opts) {
            Ok(out) => {
                assert_eq!(out.len(), 2, "one value per probe");
            }
            Err(e) => panic!("irradiance failed: {e}"),
        }
    }

    #[test]
    fn options_out_of_range_are_refused() {
        let Some(gpu) = compute() else { return };
        let s = scene();
        let c = camera();
        let l = Lighting::default();
        let size = (8, 8);

        // spp 0
        let opts = PathOptions { spp: 0, ..Default::default() };
        assert!(PathTracer::new(&gpu, &s, &c, &l, false, size, &opts, None).is_err(), "spp 0 refused");

        // spp too large
        let opts = PathOptions { spp: MAX_SPP + 1, ..Default::default() };
        assert!(PathTracer::new(&gpu, &s, &c, &l, false, size, &opts, None).is_err(), "spp > MAX_SPP refused");

        // bounces too large
        let opts = PathOptions { bounces: super::super::MAX_BOUNCES + 1, ..Default::default() };
        assert!(PathTracer::new(&gpu, &s, &c, &l, false, size, &opts, None).is_err(), "bounces > MAX refused");

        // spp_per_step 0
        let opts = PathOptions { spp_per_step: 0, ..Default::default() };
        assert!(PathTracer::new(&gpu, &s, &c, &l, false, size, &opts, None).is_err(), "spp_per_step 0 refused");

        // snapshots not ascending
        let opts = PathOptions { spp: 10, snapshots: vec![5, 3], ..Default::default() };
        assert!(PathTracer::new(&gpu, &s, &c, &l, false, size, &opts, None).is_err(), "non-ascending snapshots refused");

        // snapshot 0
        let opts = PathOptions { spp: 10, snapshots: vec![0], ..Default::default() };
        assert!(PathTracer::new(&gpu, &s, &c, &l, false, size, &opts, None).is_err(), "snapshot 0 refused");

        // snapshot > spp
        let opts = PathOptions { spp: 5, snapshots: vec![10], ..Default::default() };
        assert!(PathTracer::new(&gpu, &s, &c, &l, false, size, &opts, None).is_err(), "snapshot > spp refused");

        // too many snapshots
        let opts = PathOptions { spp: 100, snapshots: vec![1, 2, 3, 4, 5, 6, 7, 8, 9], ..Default::default() };
        assert!(PathTracer::new(&gpu, &s, &c, &l, false, size, &opts, None).is_err(), "too many snapshots refused");

        // furnace albedo out of range
        let opts = PathOptions { furnace: Some(super::super::Furnace { albedo: 1.0, emission: 1.0, ..Default::default() }), ..Default::default() };
        assert!(PathTracer::new(&gpu, &s, &c, &l, false, size, &opts, None).is_err(), "furnace albedo 1.0 refused");

        // furnace emission out of range
        let opts = PathOptions { furnace: Some(super::super::Furnace { albedo: 0.5, emission: 0.0, ..Default::default() }), ..Default::default() };
        assert!(PathTracer::new(&gpu, &s, &c, &l, false, size, &opts, None).is_err(), "furnace emission 0 refused");

        // size 0
        let opts = PathOptions::default();
        assert!(PathTracer::new(&gpu, &s, &c, &l, false, (0, 8), &opts, None).is_err(), "size 0 refused");
    }

    fn render_to_rgba_stats(gpu: &ComputeGpu, opts: &PathOptions) -> (Vec<u8>, PathStats) {
        let s = scene();
        let c = camera();
        let l = Lighting::default();
        let mut t = PathTracer::new(&gpu, &s, &c, &l, false, (16, 16), opts, None).unwrap();
        loop {
            match t.step(gpu) {
                PathStatus::Running { .. } => {}
                PathStatus::Done { rgba, .. } => return (rgba, t.stats()),
                PathStatus::Cancelled => panic!("unexpected cancel"),
                PathStatus::Failed(e) => panic!("failed: {e}"),
            }
        }
    }

    fn mean_luminance(rgba: &[u8], w: u32, h: u32) -> f32 {
        let n = (w * h) as usize;
        (0..n)
            .map(|i| {
                let b = i * 4;
                luminance([rgba[b] as f32 / 255.0, rgba[b + 1] as f32 / 255.0, rgba[b + 2] as f32 / 255.0])
            })
            .sum::<f32>()
            / n as f32
    }

    #[test]
    fn rt_falls_back_to_compute_without_ray_queries() {
        let Some(gpu) = compute() else { return };
        if gpu.ray_query {
            eprintln!("SKIP: device has ray queries (fallback not exercisable)");
            return;
        }
        let opts_rt = PathOptions { spp: 8, spp_per_step: 4, seed: 7, denoise: false, rt: true, ..Default::default() };
        let (rgba_rt, stats_rt) = render_to_rgba_stats(&gpu, &opts_rt);
        let opts_comp = PathOptions { spp: 8, spp_per_step: 4, seed: 7, denoise: false, ..Default::default() };
        let (rgba_comp, _stats_comp) = render_to_rgba_stats(&gpu, &opts_comp);
        assert!(!stats_rt.rt, "stats.rt must be false when the device lacks ray queries");
        assert_eq!(rgba_rt, rgba_comp, "fallback must produce the same bytes as the compute path");
    }

    #[test]
    fn rt_image_matches_the_compute_path_within_noise() {
        let Some(gpu) = compute_rq() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        if !gpu.ray_query {
            eprintln!("SKIP: no ray queries on this device");
            return;
        }
        let opts_rt = PathOptions { spp: 16, spp_per_step: 8, seed: 42, denoise: false, rt: true, ..Default::default() };
        let (rgba_rt, stats_rt) = render_to_rgba_stats(&gpu, &opts_rt);
        assert!(stats_rt.rt, "stats.rt must be true when ray queries are available");
        let opts_comp = PathOptions { spp: 16, spp_per_step: 8, seed: 42, denoise: false, ..Default::default() };
        let (rgba_comp, _stats_comp) = render_to_rgba_stats(&gpu, &opts_comp);
        let l1 = mean_luminance(&rgba_rt, 16, 16);
        let l2 = mean_luminance(&rgba_comp, 16, 16);
        assert!((l1 - l2).abs() < 2.0, "rt mean {l1} vs compute {l2} (diff {:.3} exceeds 2 levels)", (l1 - l2).abs());
    }

    /// 2026-11-03: the ray-query kernel never compiled (its `enable` directive came after common.wgsl, then
    /// attributes after the type and an invented ray-query API) and `.ok()` hid it, so `rt` silently rendered
    /// the compute path on every adapter. The software adapter used by the gates (llvmpipe) offers ray
    /// queries; its pipelines must build.
    #[test]
    fn rq_pipelines_build_on_a_ray_query_device() {
        let Some(gpu) = compute_rq() else { return };
        if !gpu.ray_query {
            eprintln!("SKIP: the software adapter offers no ray queries ({})", gpu.info.name);
            return;
        }
        if let Err(e) = super::super::shaders::build_rq_pipelines(&gpu, None) {
            panic!("the ray-query pipelines must build: {e}");
        }
    }

    #[test]
    fn rt_kernel_source_replaces_only_the_traversal() {
        let compute_src = super::super::shaders::kernel_source();
        let rq_src = super::super::shaders::rq_kernel_source();
        assert_ne!(compute_src, rq_src, "rq source must differ from compute source");
        assert!(rq_src.contains("rayQueryInitialize"), "rq source must use rayQueryInitialize");
        assert!(!compute_src.contains("rayQueryInitialize"), "compute source must not use rayQueryInitialize");
        // The bsdf + lights + kernel modules must be byte-identical.
        let marker = "fn bsdf_sample";
        let c_idx = compute_src.find(marker).expect("fn bsdf_sample not found in compute source");
        let r_idx = rq_src.find(marker).expect("fn bsdf_sample not found in rq source");
        assert_eq!(&compute_src[c_idx..], &rq_src[r_idx..], "everything after the BSDF module must be byte-identical between compute and rq sources");
    }

    #[test]
    fn rq_stats_rt_is_true_when_ray_queries_used() {
        let Some(gpu) = compute_rq() else { return };
        if !gpu.ray_query {
            eprintln!("SKIP: no ray queries on this device");
            return;
        }
        let opts = PathOptions { spp: 4, spp_per_step: 4, rt: true, denoise: false, ..Default::default() };
        let mut t = PathTracer::new(&gpu, &scene(), &camera(), &Lighting::default(), false, (8, 8), &opts, None).unwrap();
        assert_eq!(t.stats().rt_error, None, "the ray-query pipelines must build");
        assert!(t.stats().rt, "stats.rt must be true when ray queries are available");
    }

    #[test]
    fn rq_path_works_with_empty_scene() {
        let Some(gpu) = compute_rq() else { return };
        if !gpu.ray_query {
            eprintln!("SKIP: no ray queries on this device");
            return;
        }
        let empty = Scene3d::default();
        let opts = PathOptions { spp: 4, spp_per_step: 4, rt: true, denoise: false, ..Default::default() };
        let mut t = PathTracer::new(&gpu, &empty, &camera(), &Lighting::default(), false, (8, 8), &opts, None).unwrap();
        loop {
            match t.step(&gpu) {
                PathStatus::Done { .. } => break,
                PathStatus::Running { .. } => {}
                other => panic!("unexpected status: {other:?}"),
            }
        }
    }

    #[test]
    fn rq_path_works_with_single_triangle() {
        let Some(gpu) = compute_rq() else { return };
        if !gpu.ray_query {
            eprintln!("SKIP: no ray queries on this device");
            return;
        }
        let mut m = Mesh::new("tri", ArchId(1));
        m.push_prism(&[Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)], 0.0, 0.1);
        let s = Scene3d { bounds: m.bounds(), meshes: vec![m], lights: Vec::new(), ..Default::default() };
        let opts = PathOptions { spp: 4, spp_per_step: 4, rt: true, denoise: false, ..Default::default() };
        let mut t = PathTracer::new(&gpu, &s, &camera(), &Lighting::default(), false, (8, 8), &opts, None).unwrap();
        loop {
            match t.step(&gpu) {
                PathStatus::Done { .. } => break,
                PathStatus::Running { .. } => {}
                other => panic!("unexpected status: {other:?}"),
            }
        }
    }

    /// Render `scene` with the given options, returning (rgba bytes, stats).
    fn render_to_rgba_scene(gpu: &ComputeGpu, scene: &Scene3d, opts: &PathOptions) -> (Vec<u8>, PathStats) {
        let c = camera();
        let l = Lighting::default();
        let mut t = PathTracer::new(gpu, scene, &c, &l, false, (16, 16), opts, None).unwrap();
        loop {
            match t.step(gpu) {
                PathStatus::Running { .. } => {}
                PathStatus::Done { rgba, .. } => return (rgba, t.stats()),
                PathStatus::Cancelled => panic!("unexpected cancel"),
                PathStatus::Failed(e) => panic!("failed: {e}"),
            }
        }
    }

    #[test]
    fn path_traced_misses_show_the_backdrop() {
        let Some(gpu) = compute() else { return };
        let backdrop = Backdrop::Colour { colour: [255, 0, 0] };
        let m = floor();
        let scene = Scene3d { bounds: m.bounds(), meshes: vec![m], lights: Vec::new(), backdrop: Some(backdrop), ..Default::default() };
        let opts = PathOptions { spp: 4, denoise: false, ..Default::default() };
        let (rgba, _) = render_to_rgba_scene(&gpu, &scene, &opts);
        let n = rgba.len() / 4;
        let mut found_red = false;
        let mut found_not_red = false;
        for i in 0..n {
            let (r, g, b) = (rgba[i * 4] as f32, rgba[i * 4 + 1] as f32, rgba[i * 4 + 2] as f32);
            if (r - 255.0).abs() < 5.0 && (g - 0.0).abs() < 5.0 && (b - 0.0).abs() < 5.0 {
                found_red = true;
            } else {
                found_not_red = true;
            }
        }
        assert!(found_red, "expected some backdrop (red) pixels for missed rays");
        assert!(found_not_red, "expected some floor (non-red) pixels for hit rays");
    }

    #[test]
    fn path_traced_hits_are_unchanged() {
        let Some(gpu) = compute() else { return };
        let m = floor();
        let backdrop = Backdrop::Colour { colour: [255, 0, 0] };
        let scene_no_bg = Scene3d { bounds: m.bounds(), meshes: vec![m.clone()], lights: Vec::new(), ..Default::default() };
        let scene_with_bg = Scene3d { bounds: m.bounds(), meshes: vec![m], lights: Vec::new(), backdrop: Some(backdrop), ..Default::default() };
        let opts = PathOptions { spp: 4, seed: 42, denoise: false, ..Default::default() };
        let (rgba1, _) = render_to_rgba_scene(&gpu, &scene_no_bg, &opts);
        let (rgba2, _) = render_to_rgba_scene(&gpu, &scene_with_bg, &opts);
        let n = rgba1.len() / 4;
        let mut compared = 0;
        let mut mismatch = 0;
        for i in 0..n {
            let lum = luminance([rgba1[i * 4] as f32 / 255.0, rgba1[i * 4 + 1] as f32 / 255.0, rgba1[i * 4 + 2] as f32 / 255.0]);
            if lum > 0.02 {
                let dr = (rgba1[i * 4] as i32 - rgba2[i * 4] as i32).abs() as f32;
                let dg = (rgba1[i * 4 + 1] as i32 - rgba2[i * 4 + 1] as i32).abs() as f32;
                let db = (rgba1[i * 4 + 2] as i32 - rgba2[i * 4 + 2] as i32).abs() as f32;
                if (dr + dg + db) / 3.0 > 2.0 {
                    mismatch += 1;
                }
                compared += 1;
            }
        }
        assert!(compared > 0, "no hit pixels to compare");
        assert!(mismatch == 0, "hit pixels changed: {mismatch} of {compared} differ");
    }

    #[test]
    fn frame_lighting_matches_old_setup() {
        let opts = PathOptions::default();
        let base = GpuFrame { flags: FRAME_GROUND, look: [1, 0, 0, 0], ..Default::default() };

        // (a) a sun sets exactly the old fields; every reserved field stays zero; aux is untouched.
        let lighting = Lighting::default();
        let Some(sun) = lighting.sun else { panic!("default lighting has a sun") };
        let d = sun.normalized().dir;
        let want = GpuFrame {
            flags: FRAME_GROUND | FRAME_SUN | FRAME_SKY,
            sun_dir: [d[0] as f32, d[1] as f32, d[2] as f32, 0.0],
            sun_rgb: [SUN_LUX, SUN_LUX, SUN_LUX, 0.0],
            sky_zenith: [SKY_ZENITH_NITS; 4],
            sky_horizon: [SKY_HORIZON_NITS; 4],
            look: [1, 0, 0, 0],
            ..Default::default()
        };
        let mut f = base;
        let mut aux = vec![1.0f32, 2.0, 3.0];
        frame_lighting(&mut f, &mut aux, &lighting, None, &opts);
        assert_eq!(f, want);
        assert_eq!(aux, [1.0, 2.0, 3.0]);

        // (b) no sun: nothing changes.
        let dark = Lighting { sun: None, ..Lighting::default() };
        let mut f = base;
        let mut aux = vec![1.0f32, 2.0, 3.0];
        frame_lighting(&mut f, &mut aux, &dark, None, &opts);
        assert_eq!(f, base);
        assert_eq!(aux, [1.0, 2.0, 3.0]);

        // (c) a camera makes no difference yet.
        let mut g = base;
        let mut aux = vec![1.0f32, 2.0, 3.0];
        frame_lighting(&mut g, &mut aux, &lighting, Some(&camera()), &opts);
        assert_eq!(g, want);
        assert_eq!(aux, [1.0, 2.0, 3.0]);
    }

    fn frame_for(look: cadcraft_doc::arch::Look) -> GpuFrame {
        let lighting = Lighting { look, ..Lighting::default() };
        let mut f = GpuFrame::default();
        frame_sun(&mut f, &lighting);
        f
    }

    #[test]
    fn frame_sun_follows_intensity_and_no_shadow_flag() {
        let mut look = cadcraft_doc::arch::Look::default();
        let f = frame_for(look.clone());
        assert_eq!(f.flags, FRAME_SUN | FRAME_SKY);
        assert_eq!(f.sun_rgb, [100_000.0, 100_000.0, 100_000.0, 0.0]);
        look.sun.intensity = 50_000.0;
        look.sun.shadows = false;
        let f = frame_for(look.clone());
        assert_eq!(f.flags, FRAME_SUN | FRAME_SKY | FRAME_SUN_NO_SHADOW);
        assert_eq!(f.sun_rgb, [50_000.0, 50_000.0, 50_000.0, 0.0]);
        look.sun.on = false;
        let f = frame_for(look);
        assert_eq!(f.flags & (FRAME_SUN | FRAME_SUN_NO_SHADOW), 0, "sun off: no sun flags");
        assert_eq!(f.sun_rgb, [0.0; 4]);
    }

    #[test]
    fn sky_off_leaves_only_the_sun() {
        let mut look = cadcraft_doc::arch::Look::default();
        look.sky.on = false;
        let f = frame_for(look);
        assert_eq!(f.flags, FRAME_SUN, "no sky flag, the sun stays");
        assert_eq!(f.sky_zenith, [0.0; 4]);
        assert_eq!(f.sky_horizon, [0.0; 4]);
    }

    #[test]
    fn sun_without_shadows_passes_occluders() {
        // The sun's visibility returns 1 without tracing when FRAME_SUN_NO_SHADOW is set.
        let src = super::super::shaders::kernel_source();
        assert!(src.contains("if ((frame.flags & FRAME_SUN_NO_SHADOW) != 0u) { return vec3<f32>(1.0); }"));
        let mut look = cadcraft_doc::arch::Look::default();
        look.sun.shadows = false;
        assert_ne!(frame_for(look).flags & FRAME_SUN_NO_SHADOW, 0);
    }

    /// A path-traced 4-spp still of `scene` from `camera` at `size`, under the default (sunlit) lighting: camera
    /// rays that miss see the sky, so a miss is as bright as the sky there, not black.
    fn sunlit_still(gpu: &ComputeGpu, scene: &Scene3d, camera: &Camera, size: (u32, u32)) -> Vec<u8> {
        let opts = PathOptions { spp: 4, seed: 7, denoise: false, ..Default::default() };
        let mut t = PathTracer::new(gpu, scene, camera, &Lighting::default(), false, size, &opts, None).unwrap();
        loop {
            match t.step(gpu) {
                PathStatus::Running { .. } => {}
                PathStatus::Done { rgba, .. } => return rgba,
                s => panic!("{s:?}"),
            }
        }
    }

    /// From 1.5 m above the floor, looking straight up (40° field of view): every camera ray misses.
    fn looking_up() -> Camera {
        Camera::Full(crate::camera::FullCamera::new(Vec3::new(0.0, 0.0, 1.5), Vec3::new(0.0, 0.001, 30.0), 40.0))
    }

    /// From 1.2 m above the floor, looking level along +y (40° field of view): the floor below the middle, the sky
    /// above it.
    fn looking_level() -> Camera {
        Camera::Full(crate::camera::FullCamera::new(Vec3::new(0.0, -6.0, 1.2), Vec3::new(0.0, 6.0, 1.2), 40.0))
    }

    fn with_backdrop(backdrop: Backdrop, textures: BTreeMap<String, ImageRgba>) -> Scene3d {
        let m = floor();
        Scene3d { bounds: m.bounds(), meshes: vec![m], backdrop: Some(backdrop), textures, ..Default::default() }
    }

    /// Phase-14 check 09 at 11-04 18:16: a "#204060" backdrop behind a sunlit room read back (108, 108, 108), the
    /// sky. Every miss under a sky is about as bright as the others, and the composite only painted misses darker
    /// than 0.9 × their mean.
    #[test]
    fn path_traced_sky_misses_show_the_colour_backdrop() {
        let Some(gpu) = compute() else { return };
        let rgba = sunlit_still(&gpu, &with_backdrop(Backdrop::Colour { colour: [32, 64, 96] }, BTreeMap::new()), &looking_up(), (16, 12));
        let wrong: Vec<usize> = (0..16 * 12).filter(|i| rgba[i * 4..i * 4 + 4] != [32, 64, 96, 255]).collect();
        assert!(wrong.is_empty(), "{} of 192 sky pixels are not the backdrop, e.g. {:?}", wrong.len(), &rgba[wrong[0] * 4..wrong[0] * 4 + 4]);
    }

    /// The same with the denoiser on (the CLI's default): the denoised colours replace the radiance sums before the
    /// composite, and the misses still show the backdrop.
    #[test]
    fn path_traced_sky_misses_show_the_colour_backdrop_when_denoised() {
        let Some(gpu) = compute() else { return };
        let scene = with_backdrop(Backdrop::Colour { colour: [32, 64, 96] }, BTreeMap::new());
        let opts = PathOptions { spp: 4, seed: 7, denoise: true, ..Default::default() };
        let mut t = PathTracer::new(&gpu, &scene, &looking_up(), &Lighting::default(), false, (16, 12), &opts, None).unwrap();
        let rgba = loop {
            match t.step(&gpu) {
                PathStatus::Running { .. } => {}
                PathStatus::Done { rgba, .. } => break rgba,
                s => panic!("{s:?}"),
            }
        };
        assert!(t.stats().denoised, "the denoiser ran");
        let wrong = (0..16 * 12).filter(|i| rgba[i * 4..i * 4 + 4] != [32, 64, 96, 255]).count();
        assert_eq!(wrong, 0, "sky pixels that are not the backdrop, e.g. {:?}", &rgba[0..4]);
    }

    #[test]
    fn path_traced_misses_beside_a_sunlit_floor_show_the_backdrop_and_hits_keep_their_shading() {
        let Some(gpu) = compute() else { return };
        let (w, h) = (32u32, 24u32);
        let with = sunlit_still(&gpu, &with_backdrop(Backdrop::Colour { colour: [32, 64, 96] }, BTreeMap::new()), &looking_level(), (w, h));
        let without = sunlit_still(&gpu, &scene(), &looking_level(), (w, h));
        for x in [0, w / 2, w - 1] {
            let i = x as usize * 4;
            assert_eq!(&with[i..i + 4], &[32, 64, 96, 255], "top row pixel {x}: the sky shows through");
            let j = ((h - 1) * w + x) as usize * 4;
            assert_eq!(&with[j..j + 4], &without[j..j + 4], "bottom row pixel {x} sees the floor: unchanged by the backdrop");
            assert_ne!(&with[j..j + 3], &[32, 64, 96], "bottom row pixel {x} is the floor, not the backdrop");
        }
    }

    /// A flat image is stretched over the view: every pixel of an all-miss still is the shared CPU reference
    /// (`crate::backdrop::backdrop_rgb`), which the raster backdrop pass matches too.
    #[test]
    fn path_traced_flat_image_backdrop_is_the_shared_reference() {
        let Some(gpu) = compute() else { return };
        let img =
            ImageRgba { width: 2, height: 2, rgba: vec![255, 0, 0, 255, 0, 0, 255, 255, 0, 255, 0, 255, 255, 255, 255, 255], ..Default::default() };
        let textures = BTreeMap::from([("item:pic".to_string(), img)]);
        let b = Backdrop::Image { item: "pic".into(), projection: Projection::Flat, horizon: 0.0 };
        let (w, h) = (16u32, 12u32);
        let rgba = sunlit_still(&gpu, &with_backdrop(b.clone(), textures.clone()), &looking_up(), (w, h));
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize * 4;
                let want = crate::backdrop::backdrop_rgb(&b, &textures, x, y, w, h, [0.0, 0.0, 1.0]);
                assert_eq!(&rgba[i..i + 3], &want, "pixel ({x}, {y})");
            }
        }
        assert_eq!(&rgba[0..3], &[255, 0, 0], "top-left is the image's top-left");
    }

    /// An equirect image is looked up by view direction: looking up, the centre is the zenith (the image's top row);
    /// looking level, the top-centre pixel is 20° above the horizon (v = 0.5 - 20/180).
    #[test]
    fn path_traced_equirect_image_backdrop_follows_the_view() {
        let Some(gpu) = compute() else { return };
        // 4 x 4, every row one red level: 240, 160, 80, 0 from the top.
        let rgba: Vec<u8> = [240u8, 160, 80, 0].iter().flat_map(|&r| [r, 0, 0, 255].repeat(4)).collect();
        let textures = BTreeMap::from([("item:sky".to_string(), ImageRgba { width: 4, height: 4, rgba, ..Default::default() })]);
        let b = Backdrop::Image { item: "sky".into(), projection: Projection::Equirect, horizon: 0.0 };
        let (w, h) = (32u32, 24u32);
        let up = sunlit_still(&gpu, &with_backdrop(b.clone(), textures.clone()), &looking_up(), (w, h));
        let c = ((h / 2) * w + w / 2) as usize * 4;
        assert!(up[c] >= 238 && up[c + 1] == 0, "looking up, the centre is the zenith row: {:?}", &up[c..c + 3]);
        let level = sunlit_still(&gpu, &with_backdrop(b, textures), &looking_level(), (w, h));
        let t = (w / 2) as usize * 4;
        // v = 0.5 - 20/180 = 0.3889: texel row 4v - 0.5 = 1.056, between rows 1 (160) and 2 (80).
        let want = 160.0 - 80.0 * (4.0 * (0.5 - 20.0 / 180.0) - 1.5);
        assert!((f64::from(level[t]) - want).abs() <= 3.0, "looking level, top centre: {:?}, expected red {want:.0}", &level[t..t + 3]);
    }
}
