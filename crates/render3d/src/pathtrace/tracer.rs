//! The progressive path tracer (phase 11): uploads a scene, then each [`PathTracer::step`] adds
//! `spp_per_step` samples per pixel (one `trace_main` dispatch, `sample_base` advancing), keeps
//! the requested snapshots, and at the target resolves (and denoises) the image. A fixed seed
//! gives the same image on the same adapter. [`irradiance`] estimates lux at probe points with the
//! same kernel. Stubs filled by `p11-r3d-pt-driver` (signatures final).
//!
//! Complete (groundwork): [`SceneBuffers`] / [`upload_scene`] (a `GpuScene` as storage buffers) and
//! [`run_entry`] (dispatch one kernel entry point over a list of probes and read back `accum`),
//! which the traversal, BSDF, light and kernel tests use; the driver may use them too.

use super::device::ComputeGpu;
use super::scene_gpu::{GpuFrame, GpuProbe, GpuScene};
use super::{PathOptions, PathStats, PathStatus};
use crate::camera::Camera;
use crate::lit_frame::Lighting;
use crate::types::Scene3d;
use wgpu::util::DeviceExt;

/// A path-traced still in progress.
pub struct PathTracer {
    size: (u32, u32),
    status: PathStatus,
    stats: PathStats,
    cancelled: bool,
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
        let _ = (gpu, scene, camera, lighting, ground, opts, cache);
        Ok(PathTracer { size, status: PathStatus::Failed("path tracing is not built yet".into()), stats: PathStats::default(), cancelled: false })
    }

    /// One bounded piece of work: up to `spp_per_step` samples per pixel, or, at the target, the
    /// resolve (and denoise). After `Done`, `Cancelled` or `Failed` it keeps returning that status.
    pub fn step(&mut self, gpu: &ComputeGpu) -> PathStatus {
        let _ = gpu;
        if self.cancelled && matches!(self.status, PathStatus::Running { .. }) {
            self.status = PathStatus::Cancelled;
        }
        self.status.clone()
    }

    /// The image so far (resolved, not denoised), for the pane while it refines.
    pub fn preview(&self, gpu: &ComputeGpu) -> Option<(u32, u32, Vec<u8>)> {
        let _ = gpu;
        None
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
    let _ = (gpu, scene, lighting, ground, probes, opts);
    Err("path-traced illuminance is not built yet".into())
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

    #[test]
    fn run_entry_reads_back_a_probe_per_ray() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
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
}
