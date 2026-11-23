//! The learned denoiser (phase 22). Not available yet: [`denoise`] says so, and the caller
//! (`post::finish_linear`) falls back to the edge-aware filter and records why.

use super::device::ComputeGpu;

/// Denoise `color` (linear RGB with the variance of the mean in w) guided by `aov` (the kernel's
/// two entries per pixel) with the learned model; the same shapes as `denoise::denoise`.
pub(super) fn denoise(_gpu: &ComputeGpu, _color: &[[f32; 4]], _aov: &[[f32; 4]], _size: (u32, u32)) -> Result<Vec<[f32; 4]>, String> {
    Err("the learned denoiser is not available yet".to_string())
}
