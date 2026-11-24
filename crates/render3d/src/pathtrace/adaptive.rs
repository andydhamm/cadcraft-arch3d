//! Adaptive sampling (phase 22): at measure points (decided from sample counts only, never the
//! clock) read the accumulator, mark the pixels that have converged (the kernel skips a pixel whose
//! `accum[n + i].w` is 1 under `PHOTO_ADAPTIVE`), report the per-pixel sample statistics and the
//! frame noise, and decide whether the whole still can stop.

use super::device::ComputeGpu;
use super::{PathOptions, PathStats, StopReason};

/// Samples a pixel needs before it can count as converged or the frame can stop on its noise.
const MIN_SAMPLES: u32 = 16;

/// Per-pixel relative error of an accumulator (`accum[i]` = radiance sums and count, `accum[n + i]`
/// = luminance-squared sum and hit count). `None` for a pixel that is not a hit pixel
/// (`accum[n + i].y <= 0`) or whose luminance is not finite; otherwise the standard error of its
/// mean over `max(L, 0.05 * Lbar, 1e-6)`, `Lbar` being the mean luminance of the hit pixels. A
/// slice shorter than `2 * pixels` gives an empty vector.
pub(crate) fn pixel_errors(accum: &[[f32; 4]], pixels: usize) -> Vec<Option<f32>> {
    if pixels == 0 || accum.len() < pixels.saturating_mul(2) {
        return Vec::new();
    }
    // The mean luminance of a hit pixel (`None` for a miss or a non-finite value).
    let lum_of = |i: usize| -> Option<f32> {
        let aux = accum.get(pixels + i)?;
        if aux[1] <= 0.0 {
            return None;
        }
        let px = accum.get(i)?;
        let n = px[3].max(1.0);
        let lum = super::luminance([px[0] / n, px[1] / n, px[2] / n]);
        lum.is_finite().then_some(lum)
    };
    let mut sum = 0.0f32;
    let mut count = 0usize;
    for i in 0..pixels {
        if let Some(l) = lum_of(i) {
            sum += l;
            count += 1;
        }
    }
    if count == 0 {
        return vec![None; pixels];
    }
    let lbar = sum / count as f32;
    (0..pixels)
        .map(|i| {
            let lum = lum_of(i)?;
            let px = accum.get(i)?;
            let sum_sq = accum.get(pixels + i)?[0];
            let variance = super::tracer::PathTracer::variance_of_mean(*px, sum_sq);
            Some(variance.max(0.0).sqrt() / lum.max(0.05 * lbar).max(1e-6))
        })
        .collect()
}

/// Which pixels have converged: true when the pixel has at least `min_samples` samples and either
/// is not a hit pixel or has an error of at most `target`. A slice shorter than `2 * pixels` gives
/// an empty vector.
pub(crate) fn converged_mask(accum: &[[f32; 4]], pixels: usize, target: f32, min_samples: u32) -> Vec<bool> {
    pixel_errors(accum, pixels)
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let count = accum.get(i).map_or(0.0, |p| p[3]);
            count >= min_samples as f32 && e.is_none_or(|e| e <= target)
        })
        .collect()
}

/// The frame noise: the 95th percentile of the per-pixel error over the hit pixels (0 for none).
pub(crate) fn frame_noise_from_accum(accum: &[[f32; 4]], pixels: usize) -> f32 {
    let mut errors: Vec<f32> = pixel_errors(accum, pixels).into_iter().flatten().collect();
    if errors.is_empty() {
        return 0.0;
    }
    errors.sort_by(|a, b| a.total_cmp(b));
    let idx = ((0.95 * errors.len() as f32).ceil() as usize).max(1).min(errors.len()) - 1;
    errors.get(idx).copied().unwrap_or(0.0)
}

/// (least, most, mean, total) samples over the first `pixels` counts `accum[i].w`.
fn counts(accum: &[[f32; 4]], pixels: usize) -> (u32, u32, f32, u64) {
    let mut min = u32::MAX;
    let mut max = 0u32;
    let mut total = 0u64;
    let mut seen = 0usize;
    for p in accum.iter().take(pixels) {
        let c = if p[3].is_finite() && p[3] > 0.0 { p[3].round() as u32 } else { 0 };
        min = min.min(c);
        max = max.max(c);
        total += u64::from(c);
        seen += 1;
    }
    if seen == 0 {
        return (0, 0, 0.0, 0);
    }
    (min, max, total as f32 / pixels as f32, total)
}

/// Set the sample statistics and the noise of `stats` from a whole accumulator (the final image).
pub(crate) fn fill_stats(accum: &[[f32; 4]], pixels: usize, stats: &mut PathStats) {
    let (min, max, mean, total) = counts(accum, pixels);
    stats.spp_min = min;
    stats.spp_max = max;
    stats.spp_mean = mean;
    stats.samples_total = total;
    stats.noise = frame_noise_from_accum(accum, pixels);
}

/// The adaptive sampler of one still.
pub(super) struct Adaptive {
    enabled: bool,
    pixels: u64,
    noise_target: Option<f32>,
    spp_done: u32,
    last_measure_spp: u32,
}

impl Adaptive {
    /// A sampler for a still of `size` pixels under `opts`.
    pub(super) fn new(opts: &PathOptions, size: (u32, u32)) -> Adaptive {
        let pixels = u64::from(size.0) * u64::from(size.1);
        Adaptive { enabled: opts.adaptive, pixels, noise_target: opts.noise_target, spp_done: 0, last_measure_spp: 0 }
    }

    fn read_buffer(&self, gpu: &ComputeGpu, buf: &wgpu::Buffer, size: u64) -> Result<Vec<u8>, String> {
        let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pt adaptive read"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pt adaptive read") });
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
        let out = {
            let view = slice.get_mapped_range().map_err(|e| e.to_string())?;
            view.to_vec()
        };
        read.unmap();
        Ok(out)
    }

    /// Whether the sample counts ask for a measure now: at 16 spp, then every `max(8, last / 4)`.
    fn due(&self) -> bool {
        if self.spp_done < MIN_SAMPLES {
            return false;
        }
        self.last_measure_spp == 0 || self.spp_done >= self.last_measure_spp + 8.max(self.last_measure_spp / 4)
    }

    /// At a measure point read `accum_buf`, put the frame noise (and, under adaptive sampling, the
    /// per-pixel sample statistics) into `stats`, freeze the converged pixels under adaptive
    /// sampling with a noise target, and return `Some(NoiseTarget)` when the still may stop. A
    /// failed readback leaves every pixel active and returns `None`.
    pub(super) fn after_pass(&mut self, gpu: &ComputeGpu, accum_buf: &wgpu::Buffer, stats: &mut PathStats) -> Option<StopReason> {
        if self.noise_target.is_none() && !self.enabled {
            return None;
        }
        let pixels = usize::try_from(self.pixels).ok()?;
        if pixels == 0 || !self.due() {
            return None;
        }

        let data = self.read_buffer(gpu, accum_buf, (pixels * 32) as u64).ok()?;
        let accum: Vec<[f32; 4]> = bytemuck::cast_slice(&data).to_vec();
        if accum.len() < pixels * 2 {
            return None;
        }
        self.last_measure_spp = self.spp_done;
        let noise = frame_noise_from_accum(&accum, pixels);
        stats.noise = noise;
        if self.enabled {
            let (min, max, mean, total) = counts(&accum, pixels);
            stats.spp_min = min;
            stats.spp_max = max;
            stats.spp_mean = mean;
            stats.samples_total = total;
        }
        let target = self.noise_target?;

        let mut all_frozen = false;
        if self.enabled {
            let mask = converged_mask(&accum, pixels, target, MIN_SAMPLES);
            let mut second: Vec<[f32; 4]> = accum.get(pixels..pixels * 2).map(<[_]>::to_vec).unwrap_or_default();
            all_frozen = !second.is_empty() && second.len() == mask.len();
            for (px, done) in second.iter_mut().zip(&mask) {
                // A pixel once frozen stays frozen.
                let frozen = *done || px[3] > 0.5;
                px[3] = if frozen { 1.0 } else { 0.0 };
                all_frozen &= frozen;
            }
            if second.len() == pixels {
                gpu.queue.write_buffer(accum_buf, (pixels * 16) as u64, bytemuck::cast_slice(&second));
            }
        }

        let mut any_hit = false;
        let mut hits_mature = true;
        for i in 0..pixels {
            if accum.get(pixels + i).is_some_and(|p| p[1] > 0.0) {
                any_hit = true;
                hits_mature &= accum.get(i).is_some_and(|p| p[3] >= MIN_SAMPLES as f32);
            }
        }
        if (any_hit && noise <= target && hits_mature) || all_frozen { Some(StopReason::NoiseTarget) } else { None }
    }

    /// Record `spp_done` and, without adaptive sampling, the uniform statistics (every pixel has
    /// `spp_done` samples). Under adaptive sampling the per-pixel counts come from the accumulator.
    pub(super) fn stats(&mut self, spp_done: u32, stats: &mut PathStats) {
        self.spp_done = spp_done;
        stats.spp_done = spp_done;
        if !self.enabled {
            stats.spp_min = spp_done;
            stats.spp_max = spp_done;
            stats.spp_mean = spp_done as f32;
            stats.samples_total = self.pixels.saturating_mul(u64::from(spp_done));
        }
    }
}
