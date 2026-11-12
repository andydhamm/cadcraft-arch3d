//! The path tracer's WGSL and compute pipelines (phase 11). The kernel module is
//! `pt/common.wgsl` + `traverse` + `bsdf` + `lights` + `kernel` (one module, entry points
//! `trace_main`, `irradiance_main`, `traverse_test_main`); `resolve` and `denoise` are modules of
//! their own. [`PtPipelines::new`] builds every pipeline against explicit bind-group layouts, with
//! the device's pipeline cache when there is one, and reports WGSL validation errors as `Err`.

use super::device::ComputeGpu;

pub const COMMON_WGSL: &str = include_str!("../shaders/pt/common.wgsl");
pub const TRAVERSE_WGSL: &str = include_str!("../shaders/pt/traverse.wgsl");
pub const BSDF_WGSL: &str = include_str!("../shaders/pt/bsdf.wgsl");
pub const LIGHTS_WGSL: &str = include_str!("../shaders/pt/lights.wgsl");
pub const KERNEL_WGSL: &str = include_str!("../shaders/pt/kernel.wgsl");
pub const RESOLVE_WGSL: &str = include_str!("../shaders/pt/resolve.wgsl");
pub const DENOISE_WGSL: &str = include_str!("../shaders/pt/denoise.wgsl");
pub const RQ_TRAVERSE_WGSL: &str = include_str!("../shaders/pt/traverse_rq.wgsl");

/// The kernel module's source.
pub fn kernel_source() -> String {
    [COMMON_WGSL, TRAVERSE_WGSL, BSDF_WGSL, LIGHTS_WGSL, KERNEL_WGSL].join("\n")
}

/// The ray-query kernel module's source: same modules as [`kernel_source`] but the ray-query
/// traversal replaces the compute traversal. Everything else (bsdf, lights, kernel) is byte-identical.
///
/// WGSL directives (`enable wgpu_ray_query;`) must open a module, before every declaration, so the
/// traversal's directive lines move to the top and the rest of it keeps its place after common.wgsl.
pub fn rq_kernel_source() -> String {
    let (directives, traversal): (Vec<&str>, Vec<&str>) = RQ_TRAVERSE_WGSL.lines().partition(|l| l.trim_start().starts_with("enable "));
    let traversal = traversal.join("\n");
    [directives.join("\n").as_str(), COMMON_WGSL, traversal.as_str(), BSDF_WGSL, LIGHTS_WGSL, KERNEL_WGSL].join("\n")
}

/// Every compute pipeline and the layouts their bind groups follow.
pub struct PtPipelines {
    /// Group 0 of the kernel: frame (uniform), nodes, tris, materials, lights, ies (read-only
    /// storage), accum, aov (read-write), probes (read-only) — bindings 0..=8.
    pub kernel_layout: wgpu::BindGroupLayout,
    pub trace: wgpu::ComputePipeline,
    pub irradiance: wgpu::ComputePipeline,
    pub traverse_test: wgpu::ComputePipeline,
    /// Group 0 of resolve: params (uniform), radiance (read-only), pixels (read-write).
    pub resolve_layout: wgpu::BindGroupLayout,
    pub resolve: wgpu::ComputePipeline,
    /// Group 0 of denoise: params (uniform), src, guide (read-only), dst (read-write).
    pub denoise_layout: wgpu::BindGroupLayout,
    pub denoise: wgpu::ComputePipeline,
}

/// The ray-query kernel pipelines. Built only when the device supports ray queries; the layout
/// takes the same group-0 kernel layout plus group 1 (the TLAS).
pub struct RqPtPipelines {
    /// The two-group layout shared by all rq pipelines (group 0 = kernel buffers, group 1 = TLAS).
    pub rq_pipeline_layout: wgpu::PipelineLayout,
    /// Group 0: the nine kernel buffers.
    pub kernel_bind_group_layout: wgpu::BindGroupLayout,
    /// Group 1: the TLAS.
    pub acc_bind_group_layout: wgpu::BindGroupLayout,
    pub rq_trace: wgpu::ComputePipeline,
    pub rq_irradiance: wgpu::ComputePipeline,
    pub rq_traverse_test: wgpu::ComputePipeline,
}

fn entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

fn layout(device: &wgpu::Device, label: &str, kinds: &[wgpu::BufferBindingType]) -> wgpu::BindGroupLayout {
    let entries: Vec<_> = kinds.iter().enumerate().map(|(k, ty)| entry(k as u32, *ty)).collect();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries: &entries })
}

fn acc_layout_entry() -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::AccelerationStructure { vertex_return: false },
        count: None,
    }
}

const UNIFORM: wgpu::BufferBindingType = wgpu::BufferBindingType::Uniform;
const READ: wgpu::BufferBindingType = wgpu::BufferBindingType::Storage { read_only: true };
const WRITE: wgpu::BufferBindingType = wgpu::BufferBindingType::Storage { read_only: false };

impl PtPipelines {
    /// Build every pipeline on `gpu` (with `cache` when given). `Err` carries the first validation
    /// error (WGSL that does not compile, a layout mismatch).
    pub fn new(gpu: &ComputeGpu, cache: Option<&wgpu::PipelineCache>) -> Result<PtPipelines, String> {
        let device = &gpu.device;
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let kernel_layout = layout(device, "pt kernel", &[UNIFORM, READ, READ, READ, READ, READ, WRITE, WRITE, READ]);
        let resolve_layout = layout(device, "pt resolve", &[UNIFORM, READ, WRITE]);
        let denoise_layout = layout(device, "pt denoise", &[UNIFORM, READ, READ, WRITE]);
        let module = |label: &str, src: String| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(src.into()) })
        };
        let kernel = module("pt kernel", kernel_source());
        let resolve = module("pt resolve", RESOLVE_WGSL.to_string());
        let denoise = module("pt denoise", DENOISE_WGSL.to_string());
        let pipeline = |label: &str, bgl: &wgpu::BindGroupLayout, m: &wgpu::ShaderModule, entry_point: &str| {
            let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(bgl)],
                immediate_size: 0,
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                module: m,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache,
            })
        };
        let out = PtPipelines {
            trace: pipeline("pt trace", &kernel_layout, &kernel, "trace_main"),
            irradiance: pipeline("pt irradiance", &kernel_layout, &kernel, "irradiance_main"),
            traverse_test: pipeline("pt traverse test", &kernel_layout, &kernel, "traverse_test_main"),
            resolve: pipeline("pt resolve", &resolve_layout, &resolve, "resolve_main"),
            denoise: pipeline("pt denoise", &denoise_layout, &denoise, "atrous_main"),
            kernel_layout,
            resolve_layout,
            denoise_layout,
        };
        match futures::executor::block_on(scope.pop()) {
            Some(e) => Err(e.to_string()),
            None => Ok(out),
        }
    }
}

/// Build the ray-query kernel pipelines on `gpu`. `Err` carries the first validation error.
pub fn build_rq_pipelines(gpu: &ComputeGpu, cache: Option<&wgpu::PipelineCache>) -> Result<RqPtPipelines, String> {
    let device = &gpu.device;
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let kernel_layout = layout(device, "pt rq kernel", &[UNIFORM, READ, READ, READ, READ, READ, WRITE, WRITE, READ]);
    let acc_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("pt rq acc"), entries: &[acc_layout_entry()] });
    let rq_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pt rq"),
        bind_group_layouts: &[Some(&kernel_layout), Some(&acc_layout)],
        immediate_size: 0,
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("pt rq kernel"),
        source: wgpu::ShaderSource::Wgsl(rq_kernel_source().into()),
    });
    let rq_pipeline = |label: &str, entry_point: &str| {
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(label),
            layout: Some(&rq_pipeline_layout),
            module: &module,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            cache,
        })
    };
    let out = RqPtPipelines {
        rq_trace: rq_pipeline("pt rq trace", "trace_main"),
        rq_irradiance: rq_pipeline("pt rq irradiance", "irradiance_main"),
        rq_traverse_test: rq_pipeline("pt rq traverse test", "traverse_test_main"),
        rq_pipeline_layout,
        kernel_bind_group_layout: kernel_layout,
        acc_bind_group_layout: acc_layout,
    };
    match futures::executor::block_on(scope.pop()) {
        Some(e) => Err(e.to_string()),
        None => Ok(out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_path_tracer_pipeline_builds() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        if let Err(e) = PtPipelines::new(&gpu, None) {
            assert!(false, "the path tracer's WGSL must compile: {e}");
        }
    }

    fn assert_pt_hooks_are_stubbed(source: &str) {
        let stubs = [
            "fn sun_visibility(o: vec3<f32>, d: vec3<f32>) -> vec3<f32> { if ((frame.flags & FRAME_SUN_NO_SHADOW) != 0u) { return vec3<f32>(1.0); } return visibility(o, d, 1.0e6); }",
            "fn moon_direct(m: Material, p: vec3<f32>, n: vec3<f32>, wo: vec3<f32>) -> vec3<f32> { return vec3<f32>(0.0); }",
            "fn moon_irradiance(o: vec3<f32>, n: vec3<f32>) -> vec3<f32> { return vec3<f32>(0.0); }",
            "fn pt_shading_normal(tri: u32, n: vec3<f32>, u: f32, v: f32) -> vec3<f32> { return n; }",
        ];
        for stub in stubs {
            assert_eq!(source.matches(stub).count(), 1, "missing stub: {stub}");
        }
        assert_eq!(source.matches("sun_visibility(o, sun.dir)").count(), 2, "sun visibility uses");
        assert!(source.contains("env_escape(d, depth, mode, prev_pdf)"));
        assert!(source.contains("pt_shading_normal(h.tri, n, h.u, h.v)"));
        assert!(source.contains("camera_t_min(dir)"));
        assert!(source.contains("env_irradiance(o, n, &r)"));
        assert!(source.contains("moon_irradiance(o, n)"));
        assert!(source.contains("env_direct(m, p, n, wo, r)"));
        assert!(source.contains("moon_direct(m, p, n, wo)"));
        assert!(source.contains("!hit_clipped("));
        assert!(!source.contains("visibility(o, sun.dir, 1.0e6)"));
    }

    #[test]
    fn the_kernel_source_holds_every_entry_point() {
        let src = kernel_source();
        for f in ["fn trace_main", "fn irradiance_main", "fn traverse_test_main", "fn trace(", "fn bsdf_sample", "fn light_sample", "fn sky_radiance"]
        {
            assert!(src.contains(f), "{f}");
        }
    }

    #[test]
    fn pt_hooks_are_stubs_with_todays_behaviour() {
        let src = kernel_source();
        let rq_src = rq_kernel_source();
        for source in [src.as_str(), rq_src.as_str()] {
            assert_pt_hooks_are_stubbed(source);
        }
        assert!(!KERNEL_WGSL.contains("sky_radiance("));
        assert!(!KERNEL_WGSL.contains("visibility(o, sun.dir, 1.0e6)"));
    }
}
