//! The on-disk pipeline cache (phase 11): the compiled path-tracer pipelines are kept per adapter
//! and driver so a later Finalize does not compile again. Where the backend has no pipeline cache
//! (anything but Vulkan) everything here reports `Unsupported`. Stub filled by
//! `p11-r3d-pipeline-cache` (signatures final); the one unsafe call is
//! `cadcraft_gpu_optin::pipeline_cache`.

use super::CacheOutcome;
use super::device::ComputeGpu;
use std::path::{Path, PathBuf};

/// The cache directory: `CADCRAFT_PIPELINE_CACHE_DIR` when set (the checks use a temp dir), else
/// `$XDG_CACHE_HOME/cadcraft/pipelines`, else `$HOME/.cache/cadcraft/pipelines`; `None` when none
/// of those is set (the web build).
pub fn cache_dir() -> Option<PathBuf> {
    None
}

/// The cache file for `gpu`: `<dir>/pt-<adapter_key>.bin`.
pub fn cache_file(dir: &Path, gpu: &ComputeGpu) -> PathBuf {
    dir.join(format!("pt-{}.bin", gpu.adapter_key))
}

/// Open the pipeline cache for `gpu` from `dir`: `Hit` when this adapter's file was there and
/// read, `Miss` when not (an empty cache to fill), `Unsupported` without the feature or a `dir`.
pub fn open(gpu: &ComputeGpu, dir: Option<&Path>) -> (Option<wgpu::PipelineCache>, CacheOutcome) {
    let _ = (gpu, dir);
    (None, CacheOutcome::Unsupported)
}

/// Write `cache`'s data to `dir` (atomically: a temp file renamed over the old), after the
/// pipelines were built. Errors are reported, never fatal to a render.
pub fn save(gpu: &ComputeGpu, cache: &wgpu::PipelineCache, dir: &Path) -> Result<(), String> {
    let _ = (gpu, cache, dir);
    Ok(())
}
