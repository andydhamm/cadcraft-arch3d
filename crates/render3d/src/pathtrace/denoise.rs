//! The denoiser (phase 11): an edge-aware à-trous wavelet filter in WGSL (`pt/denoise.wgsl`),
//! [`ITERATIONS`] passes at strides 1, 2, 4, 8, 16, each tap weighted by colour, normal and albedo
//! similarity to the centre, guided by the path tracer's first-hit AOVs. No ML, no C++.
//! Stub filled by `p11-r3d-denoise` (signature final).

use super::device::ComputeGpu;

/// Passes of the filter.
pub const ITERATIONS: u32 = 5;
/// Edge-stopping widths: colour (relative to the centre's luminance), normal (cosine power),
/// albedo (absolute).
pub const SIGMA_COLOR: f32 = 0.5;
pub const SIGMA_NORMAL: f32 = 64.0;
pub const SIGMA_ALBEDO: f32 = 0.1;

/// Denoise `color` (`size.0 · size.1` linear RGB values, w ignored) guided by `aov` (two entries
/// per pixel: albedo with w = 1 where something was hit, then normal and distance; the layout the
/// kernel writes). Returns the filtered colours (w = 1). Pixels with no hit are left as they are.
pub fn denoise(gpu: &ComputeGpu, color: &[[f32; 4]], aov: &[[f32; 4]], size: (u32, u32)) -> Result<Vec<[f32; 4]>, String> {
    let _ = (gpu, aov, size);
    Ok(color.to_vec())
}
