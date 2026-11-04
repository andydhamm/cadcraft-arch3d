//! The compute device the path tracer runs on (phase 11): the same adapter choice as the raster
//! renderer, but with the adapter's own limits instead of WebGL2's (compute shaders and storage
//! buffers). Where the adapter has no compute shaders (WebGL2), or `CADCRAFT_RENDER_LIMITS=webgl2`
//! asks to act as if (the checks use it), there is no compute device and Finalize falls back to
//! the raster still.

use crate::Render3dError;
use crate::adapter::{AdapterChoice, DeviceInfo};

/// Storage buffers the path tracer binds in one stage (nodes, triangles, materials, lights, IES,
/// accumulation, AOVs, probes, probe results).
pub const PT_STORAGE_BUFFERS: u32 = 8;

/// A device that runs the path tracer.
#[derive(Clone, Debug)]
pub struct ComputeGpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: DeviceInfo,
    /// The device enabled `Features::EXPERIMENTAL_RAY_QUERY` (asked for and offered).
    pub ray_query: bool,
    /// The device enabled `Features::PIPELINE_CACHE` (Vulkan).
    pub pipeline_cache: bool,
    /// Names this adapter and driver for the pipeline cache file:
    /// `<vendor>-<device>-<driver>-<driver info>-app<version>` (a new build may bring a new wgpu),
    /// lowercase, `[a-z0-9.-]` only.
    pub adapter_key: String,
}

impl ComputeGpu {
    /// Builds a compute device description from the concrete adapter state.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, info: DeviceInfo, ray_query: bool, pipeline_cache: bool, adapter_key: String) -> Self {
        Self { device, queue, info, ray_query, pipeline_cache, adapter_key }
    }
}

/// Whether `CADCRAFT_RENDER_LIMITS=webgl2` asks for no compute device.
pub fn compute_disabled_by_env() -> bool {
    std::env::var("CADCRAFT_RENDER_LIMITS").is_ok_and(|v| v.trim().eq_ignore_ascii_case("webgl2"))
}

/// `[a-z0-9.-]` only, runs of anything else become one `-`, at most 120 characters.
pub fn key_part(s: &str) -> String {
    let mut out = String::new();
    for ch in s.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(120).collect()
}

/// The compute device for `choice` (`Fallback` = the software adapter), with ray queries enabled
/// when `ray_query` and the adapter offers them, and the pipeline cache when the adapter has it.
/// `Ok(None)`: the adapter has no compute shaders or too few storage buffers, or
/// [`compute_disabled_by_env`]. `Err(NoAdapter)`: no adapter at all.
pub fn request_compute(choice: AdapterChoice, ray_query: bool) -> Result<Option<ComputeGpu>, Render3dError> {
    if compute_disabled_by_env() {
        return Ok(None);
    }
    let instance = crate::offscreen::headless_instance();
    futures::executor::block_on(request_compute_with(&instance, choice, ray_query))
}

/// [`request_compute`] on an existing instance.
///
/// One compute device per (adapter choice, ray queries asked for) is made per process and shared, as
/// [`crate::adapter::request_device`] does for render devices (none is destroyed; a device per request ran
/// the T1000 out of memory, 2026-11-04).
pub async fn request_compute_with(instance: &wgpu::Instance, choice: AdapterChoice, ray_query: bool) -> Result<Option<ComputeGpu>, Render3dError> {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(hit) = SHARED_COMPUTE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .find(|(k, _)| *k == (choice, ray_query))
        .map(|(_, g)| g.clone())
    {
        return Ok(Some(hit));
    }
    let options = match choice {
        AdapterChoice::Auto => wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() },
        AdapterChoice::Fallback => wgpu::RequestAdapterOptions { force_fallback_adapter: true, ..Default::default() },
    };
    let adapter = instance.request_adapter(&options).await.map_err(|_| Render3dError::NoAdapter)?;
    let limits = adapter.limits();
    let compute = adapter.get_downlevel_capabilities().flags.contains(wgpu::DownlevelFlags::COMPUTE_SHADERS);
    if !compute || limits.max_storage_buffers_per_shader_stage < PT_STORAGE_BUFFERS || limits.max_compute_invocations_per_workgroup < 64 {
        return Ok(None);
    }
    let offered = adapter.features();
    let mut features = wgpu::Features::empty();
    if offered.contains(wgpu::Features::PIPELINE_CACHE) {
        features |= wgpu::Features::PIPELINE_CACHE;
    }
    let rq = ray_query && offered.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY);
    if rq {
        features |= wgpu::Features::EXPERIMENTAL_RAY_QUERY;
    }
    let mut wanted = wgpu::Limits {
        max_storage_buffers_per_shader_stage: PT_STORAGE_BUFFERS,
        max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
        max_buffer_size: limits.max_buffer_size,
        ..wgpu::Limits::default()
    };
    if rq {
        wanted = wanted.using_minimum_supported_acceleration_structure_values();
    }
    let required_limits = wanted.or_worse_values_from(&limits);
    let desc = wgpu::DeviceDescriptor {
        label: Some("render3d path tracer"),
        required_features: features,
        required_limits,
        experimental_features: cadcraft_gpu_optin::experimental_features(rq, offered),
        ..Default::default()
    };
    let (device, queue) = adapter.request_device(&desc).await.map_err(|e| Render3dError::Wgpu(e.to_string()))?;
    let a = adapter.get_info();
    let adapter_key =
        format!("{:04x}-{:04x}-{}-{}-app{}", a.vendor, a.device, key_part(&a.driver), key_part(&a.driver_info), key_part(env!("CARGO_PKG_VERSION")));
    let info =
        DeviceInfo { name: a.name, backend: format!("{:?}", a.backend), adapter_type: format!("{:?}", a.device_type), ..DeviceInfo::unknown(false) };
    let gpu = ComputeGpu::new(device, queue, info, rq, features.contains(wgpu::Features::PIPELINE_CACHE), adapter_key);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut shared = SHARED_COMPUTE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((_, first)) = shared.iter().find(|(k, _)| *k == (choice, ray_query)) {
            // Another thread made one first: keep ours open (never dropped) and share theirs.
            let _ = crate::offscreen::kept_for_the_process(gpu.device.clone());
            return Ok(Some(first.clone()));
        }
        shared.push(((choice, ray_query), gpu.clone()));
    }
    Ok(Some(gpu))
}

#[cfg(not(target_arch = "wasm32"))]
static SHARED_COMPUTE: std::sync::Mutex<Vec<((AdapterChoice, bool), ComputeGpu)>> = std::sync::Mutex::new(Vec::new());

#[cfg(test)]
mod tests {
    /// As adapter's one_device_per_adapter_choice_is_shared, for the path tracer's compute devices.
    #[test]
    fn one_compute_device_per_choice_and_ray_query_is_shared() {
        let (Ok(Some(a)), Ok(Some(b))) = (request_compute(AdapterChoice::Fallback, false), request_compute(AdapterChoice::Fallback, false)) else {
            return;
        };
        assert!(a.device == b.device, "two compute requests must share one device");
    }

    use super::*;

    #[test]
    fn key_parts_are_lowercase_and_safe() {
        assert_eq!(key_part("Mesa 25.1.4 (LLVM 21)"), "mesa-25.1.4-llvm-21");
        assert_eq!(key_part("../../etc"), "..-..-etc");
        assert_eq!(key_part(""), "");
        assert!(key_part(&"x".repeat(500)).len() <= 120);
    }

    #[test]
    fn the_fallback_adapter_gives_a_compute_device_or_none() {
        // The software adapter (lavapipe) has compute shaders; a machine without one skips.
        if let Ok(Some(gpu)) = request_compute(AdapterChoice::Fallback, false) {
            assert!(gpu.device.limits().max_storage_buffers_per_shader_stage >= PT_STORAGE_BUFFERS);
            assert!(!gpu.ray_query);
            assert!(!gpu.adapter_key.is_empty());
        }
    }
}
