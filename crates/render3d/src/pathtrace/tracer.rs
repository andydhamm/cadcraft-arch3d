//! The progressive path tracer (phase 11): uploads a scene, then each [`PathTracer::step`] adds
//! `spp_per_step` samples per pixel (one `trace_main` dispatch, `sample_base` advancing), keeps
//! the requested snapshots, and at the target resolves (and denoises) the image. A fixed seed
//! gives the same image on the same adapter. [`irradiance`] estimates lux at probe points with the
//! same kernel.

use super::device::ComputeGpu;
use super::scene_gpu::{FRAME_FURNACE, FRAME_GROUND, FRAME_SKY, FRAME_SUN, GpuFrame, GpuProbe, GpuScene, MAX_GPU_TRIANGLES, build_gpu_scene};
use super::shaders::PtPipelines;
use super::{DAY_EXPOSURE, MAX_SNAPSHOTS, MAX_SPP, SKY_HORIZON_NITS, SKY_ZENITH_NITS, SUN_LUX};
use super::{PathOptions, PathStats, PathStatus};
use crate::camera::Camera;
use crate::lit_frame::Lighting;
use crate::types::Scene3d;
use bytemuck::Pod;
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
    spp_done: u32,
    snapshot_idx: usize,
}

impl PathTracer {
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
        let accum_bytes = pixels * 16;
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

        let gpu_scene = build_gpu_scene(scene, ground, opts.furnace);
        if gpu_scene.tris.len() > MAX_GPU_TRIANGLES {
            return Err(format!("scene has {} triangles, more than MAX_GPU_TRIANGLES {MAX_GPU_TRIANGLES}", gpu_scene.tris.len()));
        }

        let pipelines = PtPipelines::new(gpu, cache)?;
        let buffers = super::tracer::upload_scene(gpu, &gpu_scene);
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

        let mut flags = 0u32;
        let mut sun_dir = [0.0f32; 4];
        let mut sun_rgb = [0.0f32; 4];
        let mut sky_zenith = [0.0f32; 4];
        let mut sky_horizon = [0.0f32; 4];
        let mut furnace = [0.0f32; 4];
        let exposure;

        if let Some(f) = opts.furnace {
            flags |= FRAME_FURNACE;
            let env = f.emission / (1.0 - f.albedo);
            furnace = [f.albedo, f.emission, env, 0.0];
            exposure = 1.0;
        } else if lighting.sun.is_some() {
            flags |= FRAME_SUN | FRAME_SKY;
            let sun = lighting.sun.unwrap();
            let d3 = sun.normalized().dir;
            sun_dir = [d3[0] as f32, d3[1] as f32, d3[2] as f32, 0.0];
            sun_rgb = [SUN_LUX, SUN_LUX, SUN_LUX, 0.0];
            sky_zenith = [SKY_ZENITH_NITS; 4];
            sky_horizon = [SKY_HORIZON_NITS; 4];
            exposure = DAY_EXPOSURE;
        } else {
            exposure = lighting.exposure;
        }
        if ground {
            flags |= FRAME_GROUND;
        }

        let eye = camera.eye();
        let frame = GpuFrame {
            inv_view_proj: inv_vp,
            eye: [eye.x as f32, eye.y as f32, eye.z as f32, 0.0],
            size: [w, h],
            sample_base: 0,
            seed: opts.seed,
            bounces: opts.bounces,
            light_count: gpu_scene.lights.len() as u32,
            flags,
            spp_this_pass: 0,
            sun_dir,
            sun_rgb,
            sky_zenith,
            sky_horizon,
            furnace,
            probe_count: 0,
            ..Default::default()
        };

        Ok(PathTracer {
            size,
            status: PathStatus::Running { progress: 0.0, spp: 0 },
            stats: PathStats::default(),
            cancelled: false,
            opts: opts.clone(),
            frame,
            exposure,
            buffers,
            accum,
            aov,
            probes_buf,
            pipelines,
            spp_done: 0,
            snapshot_idx: 0,
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
            let (w, h) = self.size;
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        gpu.queue.submit([enc.finish()]);
        if let Some(e) = futures::executor::block_on(scope.pop()) {
            self.status = PathStatus::Failed(format!("trace dispatch: {e}"));
            return self.status.clone();
        }

        self.spp_done += this_pass;
        self.stats.spp_done = self.spp_done;

        // Check for a snapshot at this spp.
        if self.snapshot_idx < self.opts.snapshots.len() && self.spp_done >= self.opts.snapshots[self.snapshot_idx] {
            let snap_spp = self.opts.snapshots[self.snapshot_idx];
            if let Some(rgba) = self.resolve(gpu) {
                self.stats.snapshots.push((snap_spp, rgba));
            }
            self.snapshot_idx += 1;
        }

        if self.spp_done >= self.opts.spp {
            return self.finish(gpu);
        }

        let progress = self.spp_done as f32 / self.opts.spp as f32;
        self.status = PathStatus::Running { progress, spp: self.spp_done };
        self.status.clone()
    }

    fn finish(&mut self, gpu: &ComputeGpu) -> PathStatus {
        let (w, h) = self.size;
        let pixels = (w * h) as usize;
        let accum_bytes = (pixels * 16) as u64;
        let aov_bytes = (pixels * 32) as u64;

        // Read back accum.
        let accum_data = match self.read_buffer(gpu, &self.accum, accum_bytes) {
            Ok(d) => d,
            Err(e) => {
                self.status = PathStatus::Failed(format!("read accum: {e}"));
                return self.status.clone();
            }
        };
        let accum: Vec<[f32; 4]> = bytemuck::cast_slice(&accum_data).to_vec();

        // Compute radiance stats.
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let mut sum = 0.0f64;
        for px in &accum {
            let l = super::luminance([px[0], px[1], px[2]]);
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
            let colors: Vec<[f32; 4]> = accum.iter().map(|p| [p[0], p[1], p[2], 1.0]).collect();
            match super::denoise::denoise(gpu, &colors, &aov, self.size) {
                Ok(denoised) => denoised,
                Err(e) => {
                    self.status = PathStatus::Failed(format!("denoise: {e}"));
                    return self.status.clone();
                }
            }
        } else {
            accum
        };

        // Write denoised result back into accum so resolve reads it.
        let d = &gpu.device;
        let src = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pt denoised"),
            contents: bytemuck::cast_slice(&final_accum),
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pt write denoised") });
        enc.copy_buffer_to_buffer(&src, 0, &self.accum, 0, accum_bytes);
        gpu.queue.submit([enc.finish()]);

        self.stats.denoised = self.opts.denoise;

        // Resolve.
        match self.resolve(gpu) {
            Some(rgba) => {
                self.status = PathStatus::Done { width: w, height: h, rgba };
            }
            None => {
                self.status = PathStatus::Failed("resolve failed".into());
            }
        }
        self.status.clone()
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
            spp: u32,
        }
        let params = ResolveParams { size: [w, h], exposure: self.exposure, spp: self.spp_done };

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

    let gpu_scene = build_gpu_scene(scene, ground, opts.furnace);
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
        let mut frame = GpuFrame {
            inv_view_proj: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            eye: [0.0; 4],
            size: [1, 1],
            sample_base,
            seed: opts.seed,
            bounces: opts.bounces,
            light_count: gpu_scene.lights.len() as u32,
            flags: 0,
            spp_this_pass: this_pass,
            sun_dir: [0.0; 4],
            sun_rgb: [0.0; 4],
            sky_zenith: [0.0; 4],
            sky_horizon: [0.0; 4],
            furnace: [0.0; 4],
            probe_count: gpu_probes.len() as u32,
            ..Default::default()
        };
        if let Some(sun) = lighting.sun {
            frame.flags |= FRAME_SUN | FRAME_SKY;
            let d3 = sun.normalized().dir;
            frame.sun_dir = [d3[0] as f32, d3[1] as f32, d3[2] as f32, 0.0];
            frame.sun_rgb = [SUN_LUX, SUN_LUX, SUN_LUX, 0.0];
            frame.sky_zenith = [SKY_ZENITH_NITS; 4];
            frame.sky_horizon = [SKY_HORIZON_NITS; 4];
        }

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

fn storage<T: bytemuck::Pod + Default>(device: &wgpu::Device, label: &str, items: &[T]) -> wgpu::Buffer {
    let one = [T::default()];
    let data: &[T] = if items.is_empty() { &one } else { items };
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
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
    let d = &gpu.device;
    let n = probes.len().max(1);
    let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
    let pipelines = super::shaders::PtPipelines::new(gpu, None)?;
    let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("pt test kernel"),
        source: wgpu::ShaderSource::Wgsl(super::shaders::kernel_source().into()),
    });
    let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pt test"),
        bind_group_layouts: &[Some(&pipelines.kernel_layout)],
        immediate_size: 0,
    });
    let pipeline = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(entry),
        layout: Some(&layout),
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
        size: (n * 16) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    // Buffers start zeroed.
    let accum_src = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pt accum"),
        size: (n * 16) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pt test"),
        layout: &pipelines.kernel_layout,
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
        pass.dispatch_workgroups((n as u32).div_ceil(64), 1, 1);
    }
    enc.copy_buffer_to_buffer(&accum_src, 0, &read, 0, (n * 16) as u64);
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
    use super::*;
    use crate::camera::{Camera, Orbit};
    use crate::lit_frame::Lighting;
    use crate::types::{ArchId, Mesh, Scene3d};
    use cadcraft_geom::Vec2;

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

    fn compute() -> Option<ComputeGpu> {
        let gpu = crate::gputest::shared_compute();
        if gpu.is_none() {
            eprintln!("SKIP: no compute device");
        }
        gpu
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
}
