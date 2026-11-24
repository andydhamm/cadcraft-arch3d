//! Adaptive sampling (phase 22): after each pass, decide which pixels have converged (the kernel
//! skips a pixel whose `accum[n + i].w` is 1 under `PHOTO_ADAPTIVE`) and whether the whole still
//! can stop. The CPU readback keeps the adaptive state and the frame's noise estimate, while the
//! kernel still owns the actual sample schedule.

use super::device::ComputeGpu;
use super::{PathOptions, PathStats, StopReason};

pub(crate) fn frame_noise_from_accum(accum: &[[f32; 4]], pixels: usize) -> f32 {
    if pixels == 0 || accum.len() < pixels * 2 {
        return 0.0;
    }

    let mut hit_lum = Vec::new();
    for i in 0..pixels {
        let hit = accum[pixels + i][1] > 0.0;
        if !hit {
            continue;
        }
        let px = accum[i];
        let n = px[3].max(1.0);
        let lum = super::luminance([px[0] / n, px[1] / n, px[2] / n]);
        if lum.is_finite() {
            hit_lum.push(lum);
        }
    }
    if hit_lum.is_empty() {
        return 0.0;
    }
    let lbar = hit_lum.iter().sum::<f32>() / hit_lum.len() as f32;

    let mut errors = Vec::new();
    for i in 0..pixels {
        let hit = accum[pixels + i][1] > 0.0;
        if !hit {
            continue;
        }
        let px = accum[i];
        let n = px[3].max(1.0);
        let lum = super::luminance([px[0] / n, px[1] / n, px[2] / n]);
        if !lum.is_finite() {
            continue;
        }
        let variance = super::tracer::PathTracer::variance_of_mean(px, accum[pixels + i][0]);
        let denom = lum.max(0.05 * lbar).max(1e-6);
        errors.push((variance.max(0.0).sqrt()) / denom);
    }
    if errors.is_empty() {
        return 0.0;
    }

    let mut sorted = errors;
    sorted.sort_by(|a, b| a.total_cmp(b));
    let idx = ((0.95 * sorted.len() as f32).ceil() as usize).max(1).min(sorted.len()) - 1;
    sorted[idx]
}

/// The adaptive sampler of one still.
pub(super) struct Adaptive {
    enabled: bool,
    pixels: u64,
    noise_target: Option<f32>,
    spp_done: u32,
    last_measure_spp: u32,
    last_measure_at: std::time::Instant,
    last_noise: f32,
    spp_min: u32,
    spp_max: u32,
    spp_mean: f32,
    samples_total: u64,
}

impl Adaptive {
    /// A sampler for a still of `size` pixels under `opts`.
    pub(super) fn new(opts: &PathOptions, size: (u32, u32)) -> Adaptive {
        let pixels = u64::from(size.0) * u64::from(size.1);
        Adaptive {
            enabled: opts.adaptive,
            pixels,
            noise_target: opts.noise_target,
            spp_done: 0,
            last_measure_spp: 0,
            last_measure_at: std::time::Instant::now(),
            last_noise: 0.0,
            spp_min: 0,
            spp_max: 0,
            spp_mean: 0.0,
            samples_total: 0,
        }
    }

    /// Whether the kernel runs with `PHOTO_ADAPTIVE`.
    #[allow(dead_code)]
    pub(super) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(super) fn last_noise(&self) -> f32 {
        self.last_noise
    }

    pub(super) fn apply_stats(&self, stats: &mut PathStats) {
        stats.noise = self.last_noise;
        stats.spp_min = self.spp_min;
        stats.spp_max = self.spp_max;
        stats.spp_mean = self.spp_mean;
        stats.samples_total = self.samples_total;
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

    fn refresh_summary_from_accum(&mut self, accum: &[[f32; 4]]) {
        let pixels = self.pixels as usize;
        let mut min = u32::MAX;
        let mut max = 0u32;
        let mut total = 0u64;
        for i in 0..pixels {
            let count = accum[i][3].round() as u32;
            if count > 0 {
                min = min.min(count);
                max = max.max(count);
            }
            total += count as u64;
        }
        self.spp_min = if pixels == 0 || min == u32::MAX { 0 } else { min };
        self.spp_max = if pixels == 0 { 0 } else { max };
        self.spp_mean = if pixels == 0 { 0.0 } else { total as f32 / pixels as f32 };
        self.samples_total = total;
    }

    /// Read `accum`, measure the frame noise, and mark converged pixels under adaptive mode. Returns
    /// `Some(reason)` when the stop rule is met.
    pub(super) fn after_pass(&mut self, gpu: &ComputeGpu, accum_buf: &wgpu::Buffer, _aov: &wgpu::Buffer) -> Option<StopReason> {
        if self.noise_target.is_none() && !self.enabled {
            return None;
        }
        let pixels = self.pixels as usize;
        if pixels == 0 {
            self.last_noise = 0.0;
            return None;
        }

        if self.spp_done < 16 {
            return None;
        }
        if self.last_measure_spp != 0 && self.spp_done < self.last_measure_spp + 8 {
            return None;
        }
        if self.last_measure_spp != 0 && self.last_measure_at.elapsed() < std::time::Duration::from_millis(500) {
            return None;
        }

        let data = self.read_buffer(gpu, accum_buf, (pixels * 32) as u64).ok()?;
        let accum: Vec<[f32; 4]> = bytemuck::cast_slice(&data).to_vec();
        self.refresh_summary_from_accum(&accum);
        self.last_noise = frame_noise_from_accum(&accum, pixels);
        self.last_measure_spp = self.spp_done;
        self.last_measure_at = std::time::Instant::now();

        if let Some(target) = self.noise_target {
            let hit_pixels = (0..pixels).filter(|&i| accum[pixels + i][1] > 0.0).collect::<Vec<_>>();
            if !hit_pixels.is_empty() {
                let all_samples_mature = hit_pixels.iter().all(|&i| accum[i][3].round() as u32 >= 16);
                if self.last_noise <= target && all_samples_mature {
                    return Some(StopReason::NoiseTarget);
                }
            }
        }

        None
    }

    /// Fill the samples-per-pixel statistics after `spp_done` samples at every pixel (uniform).
    pub(super) fn stats(&mut self, spp_done: u32, stats: &mut PathStats) {
        self.spp_done = spp_done;
        stats.spp_done = spp_done;
        stats.spp_min = spp_done;
        stats.spp_max = spp_done;
        stats.spp_mean = spp_done as f32;
        stats.samples_total = self.pixels.saturating_mul(u64::from(spp_done));
        self.spp_min = spp_done;
        self.spp_max = spp_done;
        self.spp_mean = spp_done as f32;
        self.samples_total = self.pixels.saturating_mul(u64::from(spp_done));
    }
}
