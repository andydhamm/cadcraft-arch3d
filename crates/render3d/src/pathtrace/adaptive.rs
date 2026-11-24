//! Adaptive sampling (phase 22): after each pass, decide which pixels have converged (the kernel
//! skips a pixel whose `accum[n + i].w` is 1 under `PHOTO_ADAPTIVE`) and whether the whole still
//! can stop. [`Adaptive`] is the groundwork: it never stops a render early and reports the uniform
//! sample counts; p22-r3d-adaptive replaces `after_pass` and `stats`.

use super::device::ComputeGpu;
use super::{PathOptions, PathStats, StopReason};

/// The adaptive sampler of one still.
pub(super) struct Adaptive {
    enabled: bool,
    pixels: u64,
}

impl Adaptive {
    /// A sampler for a still of `size` pixels under `opts` (it only acts when `opts.adaptive`).
    pub(super) fn new(opts: &PathOptions, size: (u32, u32)) -> Adaptive {
        Adaptive { enabled: opts.adaptive, pixels: u64::from(size.0) * u64::from(size.1) }
    }

    /// Whether the kernel runs with `PHOTO_ADAPTIVE`.
    #[allow(dead_code)]
    pub(super) fn enabled(&self) -> bool {
        self.enabled
    }

    /// Look at the accumulator (`accum`, the per-pixel sums; `aov`, the first-hit guides) after a
    /// pass; `Some(reason)` stops the render before the sample target. Never stops, for now.
    pub(super) fn after_pass(&mut self, _gpu: &ComputeGpu, _accum: &wgpu::Buffer, _aov: &wgpu::Buffer) -> Option<StopReason> {
        None
    }

    /// Fill the samples-per-pixel statistics after `spp_done` samples at every pixel (uniform).
    pub(super) fn stats(&self, spp_done: u32, stats: &mut PathStats) {
        stats.spp_min = spp_done;
        stats.spp_max = spp_done;
        stats.spp_mean = spp_done as f32;
        stats.samples_total = self.pixels.saturating_mul(u64::from(spp_done));
    }
}
