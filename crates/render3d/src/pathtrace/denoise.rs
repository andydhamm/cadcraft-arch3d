//! The denoiser (phase 11): an edge-aware à-trous wavelet filter in WGSL (`pt/denoise.wgsl`),
//! [`ITERATIONS`] passes at strides 1, 2, 4, 8, 16, each tap weighted by colour variance (SVGF's
//! sigma_l), normal and albedo similarity to the centre, guided by the path tracer's first-hit
//! AOVs. No ML, no C++.
//!
//! One pass is a 5 x 5 B3-spline kernel (1/16, 1/4, 3/8, 1/4, 1/16 per axis) whose taps sit
//! `step` pixels apart; doubling the step per pass widens the footprint to 61 pixels with 25 taps
//! a pass. The edge-stopping weights keep the filter from averaging across a change of albedo or
//! of surface direction (the guides), or across a colour change much larger than the centre's
//! variance-guided noise. Pixels with no hit (the sky) are copied and never contribute.

use super::device::ComputeGpu;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Passes of the filter.
pub const ITERATIONS: u32 = 5;
/// Edge-stopping width in standard deviations of the two pixels' combined variance (SVGF's
/// `sigma_l`), with normal (cosine power) and albedo (absolute) guides.
pub const SIGMA_COLOR: f32 = 4.0;
pub const SIGMA_NORMAL: f32 = 64.0;
pub const SIGMA_ALBEDO: f32 = 0.1;

/// `DenoiseParams` of `denoise.wgsl` (std140, 32 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
struct DenoiseParams {
    size: [u32; 2],
    step: u32,
    _pad: u32,
    sigma_c: f32,
    sigma_n: f32,
    sigma_a: f32,
    _pad2: f32,
}

/// The compute pipeline of `atrous_main` and the layout of its bind group: params (uniform), src
/// and guide (read-only), dst (read-write) — the same as `PtPipelines::denoise` and
/// `denoise_layout`, built alone so a denoise does not compile the whole kernel module.
struct AtrousPipeline {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
}

fn layout_entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    let min_binding_size = match ty {
        wgpu::BufferBindingType::Uniform => std::num::NonZeroU64::new(32),
        _ => None,
    };
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size },
        count: None,
    }
}

fn build_pipeline(gpu: &ComputeGpu) -> Result<AtrousPipeline, String> {
    let d = &gpu.device;
    let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
    let read = wgpu::BufferBindingType::Storage { read_only: true };
    let write = wgpu::BufferBindingType::Storage { read_only: false };
    let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("pt denoise"),
        entries: &[layout_entry(0, wgpu::BufferBindingType::Uniform), layout_entry(1, read), layout_entry(2, read), layout_entry(3, write)],
    });
    let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("pt denoise"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/pt/denoise.wgsl").into()),
    });
    let pipeline_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pt denoise"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("pt denoise"),
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: Some("atrous_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    match futures::executor::block_on(scope.pop()) {
        Some(e) => Err(format!("the denoise pipeline does not build: {e}")),
        None => Ok(AtrousPipeline { layout, pipeline }),
    }
}

/// The pixel count of `size`, or why the inputs cannot be filtered on `gpu`.
fn check_inputs(gpu: &ComputeGpu, color: usize, aov: usize, size: (u32, u32)) -> Result<usize, String> {
    let (w, h) = size;
    if w == 0 || h == 0 {
        return Err(format!("denoise: the image is {w} x {h} pixels"));
    }
    let pixels = u64::from(w) * u64::from(h);
    if color as u64 != pixels {
        return Err(format!("denoise: {color} colours for a {w} x {h} image"));
    }
    if aov as u64 != 2 * pixels {
        return Err(format!("denoise: {aov} guide entries for a {w} x {h} image (two per pixel)"));
    }
    let limits = gpu.device.limits();
    let guide_bytes = 2 * pixels * 16;
    let largest = u64::from(limits.max_storage_buffer_binding_size).min(limits.max_buffer_size);
    if guide_bytes > largest {
        return Err(format!("denoise: a {w} x {h} image needs a {guide_bytes} byte guide buffer; the device allows {largest}"));
    }
    if w.div_ceil(8) > limits.max_compute_workgroups_per_dimension || h.div_ceil(8) > limits.max_compute_workgroups_per_dimension {
        return Err(format!("denoise: a {w} x {h} image is more workgroups than the device dispatches"));
    }
    Ok(pixels as usize)
}

/// Denoise `color` (`size.0 · size.1` linear RGB values, w is the pixel variance of the mean:
/// luminance-squared, with negative or non-finite values treated as 0) guided by `aov` (two entries
/// per pixel: albedo with w = 1 where something was hit, then normal and distance; the layout the
/// kernel writes). Returns the filtered colours with w = 1. Pixels with no hit are left as they are.
pub fn denoise(gpu: &ComputeGpu, color: &[[f32; 4]], aov: &[[f32; 4]], size: (u32, u32)) -> Result<Vec<[f32; 4]>, String> {
    let pixels = check_inputs(gpu, color.len(), aov.len(), size)?;
    let atrous = build_pipeline(gpu)?;
    let d = &gpu.device;
    let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
    let bytes = (pixels * 16) as u64;
    // The colour's w is the per-pixel variance of the mean; clamp it to finite non-negative values
    // before the pass and restore w = 1 on the readback for the caller's old opaque colour API.
    let opaque: Vec<[f32; 4]> = color.iter().map(|c| [c[0], c[1], c[2], if c[3].is_finite() && c[3] >= 0.0 { c[3] } else { 0.0 }]).collect();
    let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
    let ping = [
        d.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("pt denoise a"), contents: bytemuck::cast_slice(&opaque), usage }),
        d.create_buffer(&wgpu::BufferDescriptor { label: Some("pt denoise b"), size: bytes, usage, mapped_at_creation: false }),
    ];
    let guide = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("pt denoise guide"),
        contents: bytemuck::cast_slice(aov),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pt denoise") });
    for k in 0..ITERATIONS {
        let params = DenoiseParams {
            size: [size.0, size.1],
            step: 1 << k,
            sigma_c: SIGMA_COLOR,
            sigma_n: SIGMA_NORMAL,
            sigma_a: SIGMA_ALBEDO,
            ..Default::default()
        };
        let params_bytes = bytemuck::bytes_of(&params);
        let params_size = ((params_bytes.len() + 255) / 256) * 256;
        let uniform = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pt denoise params"),
            size: params_size as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&uniform, 0, params_bytes);
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pt denoise"),
            layout: &atrous.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: ping[(k % 2) as usize].as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: guide.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: ping[((k + 1) % 2) as usize].as_entire_binding() },
            ],
        });
        // One pass per iteration: the next one reads what this one wrote.
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("pt denoise"), timestamp_writes: None });
        pass.set_pipeline(&atrous.pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(size.0.div_ceil(8), size.1.div_ceil(8), 1);
    }
    let read = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pt denoise read"),
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    enc.copy_buffer_to_buffer(&ping[(ITERATIONS % 2) as usize], 0, &read, 0, bytes);
    gpu.queue.submit([enc.finish()]);
    if let Some(e) = futures::executor::block_on(scope.pop()) {
        return Err(format!("denoise: {e}"));
    }
    read_back(gpu, &read)
}

/// The contents of the `MAP_READ` buffer `read` as colours.
fn read_back(gpu: &ComputeGpu, read: &wgpu::Buffer) -> Result<Vec<[f32; 4]>, String> {
    let slice = read.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    gpu.device
        .poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(300)) })
        .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    let out: Vec<[f32; 4]> = {
        let view = slice.get_mapped_range().map_err(|e| e.to_string())?;
        let mut result: Vec<[f32; 4]> = bytemuck::cast_slice(&view).to_vec();
        for px in &mut result {
            px[3] = 1.0;
        }
        result
    };
    read.unmap();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: u32 = 64;
    const WALL: [f32; 3] = [0.5, 0.5, 0.5];
    const UP: [f32; 3] = [0.0, 0.0, 1.0];
    const SIDE: [f32; 3] = [1.0, 0.0, 0.0];

    /// One pixel of a synthetic frame: the colour the tracer accumulated and its first-hit guides.
    #[derive(Clone, Copy)]
    struct Pixel {
        color: [f32; 3],
        albedo: [f32; 3],
        normal: [f32; 3],
        hit: bool,
    }

    /// A flat wall's worth of guides around `color`.
    fn wall(color: [f32; 3]) -> Pixel {
        Pixel { color, albedo: WALL, normal: UP, hit: true }
    }

    /// xorshift32 in [-1, 1): the same noise on every run.
    struct Noise(u32);

    impl Noise {
        fn next(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            (self.0 >> 8) as f32 / 8_388_608.0 - 1.0
        }
    }

    fn image(mut f: impl FnMut(u32, u32) -> Pixel) -> Vec<Pixel> {
        (0..N * N).map(|i| f(i % N, i / N)).collect()
    }

    /// Run [`denoise`] over `pixels` (row-major `w · h`), failing the test on an `Err`.
    fn run(gpu: &ComputeGpu, pixels: &[Pixel], size: (u32, u32)) -> Vec<[f32; 4]> {
        let color: Vec<[f32; 4]> = pixels.iter().map(|p| [p.color[0], p.color[1], p.color[2], 1.0]).collect();
        let aov: Vec<[f32; 4]> = pixels
            .iter()
            .flat_map(|p| [[p.albedo[0], p.albedo[1], p.albedo[2], if p.hit { 1.0 } else { 0.0 }], [p.normal[0], p.normal[1], p.normal[2], 1.0]])
            .collect();
        match denoise(gpu, &color, &aov, size) {
            Ok(out) => {
                assert_eq!(out.len(), pixels.len(), "one colour per pixel");
                out
            }
            Err(e) => {
                assert!(false, "denoise failed: {e}");
                Vec::new()
            }
        }
    }

    fn compute() -> Option<ComputeGpu> {
        let gpu = crate::gputest::shared_compute();
        if gpu.is_none() {
            eprintln!("SKIP: no compute device");
        }
        gpu
    }

    /// Mean of channel values of the pixels in columns `x0..x1`, all rows.
    fn band_mean(out: &[[f32; 4]], x0: u32, x1: u32) -> f32 {
        let mut sum = 0.0f64;
        let mut count = 0u32;
        for y in 0..N {
            for x in x0..x1 {
                let p = out[(y * N + x) as usize];
                sum += f64::from(p[0] + p[1] + p[2]);
                count += 3;
            }
        }
        (sum / f64::from(count)) as f32
    }

    fn rmse(out: &[[f32; 4]], expected: f32) -> f32 {
        let sum: f64 = out.iter().flat_map(|p| [p[0], p[1], p[2]]).map(|v| f64::from(v - expected).powi(2)).sum();
        (sum / (out.len() * 3) as f64).sqrt() as f32
    }

    /// Left half `left`, right half `right`: the colour steps from 0.40 to 0.50, the guide named by
    /// `guide` carries the other edge. `flat` leaves both guides identical across the step.
    fn halves(guide: &str, flat: bool) -> Vec<Pixel> {
        image(|x, _| {
            let right = x >= N / 2;
            let color = if right { [0.5; 3] } else { [0.4; 3] };
            let mut p = wall(color);
            if right && !flat {
                match guide {
                    "albedo" => p.albedo = [0.8; 3],
                    _ => p.normal = SIDE,
                }
            } else if !right && !flat && guide == "albedo" {
                p.albedo = [0.2; 3];
            }
            p
        })
    }

    /// The step survives: each half keeps its mean (within 2 %) and the two pixels either side of
    /// the edge hold their half's value to within 10 % of the step's height.
    fn assert_step_kept(out: &[[f32; 4]]) {
        let (left, right) = (band_mean(out, 0, N / 2), band_mean(out, N / 2, N));
        assert!((left - 0.4).abs() <= 0.4 * 0.02, "left half mean {left}");
        assert!((right - 0.5).abs() <= 0.5 * 0.02, "right half mean {right}");
        let (near_left, near_right) = (band_mean(out, N / 2 - 2, N / 2), band_mean(out, N / 2, N / 2 + 2));
        assert!((near_left - 0.4).abs() <= 0.1 * 0.1, "left edge band {near_left} mixed with the other side");
        assert!((near_right - 0.5).abs() <= 0.1 * 0.1, "right edge band {near_right} mixed with the other side");
    }

    #[test]
    fn denoise_flattens_noise_on_a_flat_wall() {
        let Some(gpu) = compute() else { return };
        let mut noise = Noise(0x9e37_79b9);
        let pixels = image(|_, _| wall([0.5 * (1.0 + 0.2 * noise.next()), 0.5 * (1.0 + 0.2 * noise.next()), 0.5 * (1.0 + 0.2 * noise.next())]));
        let before: Vec<[f32; 4]> = pixels.iter().map(|p| [p.color[0], p.color[1], p.color[2], 1.0]).collect();
        let (noisy, clean) = (rmse(&before, 0.5), rmse(&run(&gpu, &pixels, (N, N)), 0.5));
        assert!(noisy > 0.04, "the synthetic wall is noisy ({noisy})");
        assert!(clean <= 0.3 * noisy, "RMSE {noisy} -> {clean}: less than 70 % of the noise gone");
    }

    #[test]
    fn denoise_keeps_an_edge_between_albedos() {
        let Some(gpu) = compute() else { return };
        assert_step_kept(&run(&gpu, &halves("albedo", false), (N, N)));
        // Control: the colour step alone is too small for the colour weight to stop (a 0.1 step at
        // a 0.4 centre); with the same guides on both sides the edge does blur. So it is the
        // albedo guide that keeps the step above.
        let blurred = run(&gpu, &halves("albedo", true), (N, N));
        assert!(band_mean(&blurred, N / 2 - 2, N / 2) > 0.4 + 0.1 * 0.1, "without the guide the edge mixes");
    }

    #[test]
    fn denoise_keeps_an_edge_between_normals() {
        let Some(gpu) = compute() else { return };
        assert_step_kept(&run(&gpu, &halves("normal", false), (N, N)));
        let blurred = run(&gpu, &halves("normal", true), (N, N));
        assert!(band_mean(&blurred, N / 2 - 2, N / 2) > 0.4 + 0.1 * 0.1, "without the guide the edge mixes");
    }

    #[test]
    fn pixels_without_a_hit_are_left_alone() {
        let Some(gpu) = compute() else { return };
        let mut noise = Noise(0x2545_f491);
        // The right half is sky: bright, noisy and never part of a hit pixel's average.
        let pixels = image(|x, _| {
            if x < N / 2 {
                wall([0.5; 3])
            } else {
                Pixel { color: [50.0 * (noise.next() + 1.0), 20.0 * (noise.next() + 1.0), 5.0 * (noise.next() + 1.0)], hit: false, ..wall([0.0; 3]) }
            }
        });
        let out = run(&gpu, &pixels, (N, N));
        for (i, (p, o)) in pixels.iter().zip(&out).enumerate() {
            if p.hit {
                assert!(o[..3].iter().all(|v| (v - 0.5).abs() <= 1e-5), "pixel {i} took sky into its average: {o:?}");
            } else {
                assert_eq!(o[..3], p.color[..], "the sky pixel {i} is copied");
            }
        }
        // An image that hits nothing comes back as it went in.
        let all_sky: Vec<Pixel> = pixels.iter().map(|p| Pixel { hit: false, ..*p }).collect();
        let out = run(&gpu, &all_sky, (N, N));
        assert!(all_sky.iter().zip(&out).all(|(p, o)| o[..3] == p.color[..]));
    }

    #[test]
    fn a_constant_image_stays_constant() {
        let Some(gpu) = compute() else { return };
        let pixels = image(|_, _| wall([0.25, 0.5, 0.75]));
        let out = run(&gpu, &pixels, (N, N));
        for (i, o) in out.iter().enumerate() {
            let err = (o[0] - 0.25).abs().max((o[1] - 0.5).abs()).max((o[2] - 0.75).abs());
            assert!(err <= 1e-5, "pixel {i} drifted by {err}: {o:?}");
            assert_eq!(o[3], 1.0, "the result is opaque");
        }
    }

    #[test]
    fn an_image_that_is_not_a_multiple_of_the_workgroup_is_filtered_whole() {
        let Some(gpu) = compute() else { return };
        let (w, h) = (13u32, 7u32);
        let pixels: Vec<Pixel> = (0..w * h).map(|_| wall([0.3, 0.6, 0.9])).collect();
        let out = run(&gpu, &pixels, (w, h));
        assert!(out.iter().all(|o| (o[0] - 0.3).abs() <= 1e-5 && (o[1] - 0.6).abs() <= 1e-5 && (o[2] - 0.9).abs() <= 1e-5 && o[3] == 1.0));
    }

    #[test]
    fn dark_pixels_average_with_their_brighter_noisy_neighbours() {
        let Some(gpu) = compute() else { return };
        let mut noise = Noise(0x3c6e_f375);
        let pixels: Vec<Pixel> = image(|_, _| {
            let mean = (0..4).map(|_| if noise.next() < -0.5 { 1.0 } else { 0.0 }).sum::<f32>() / 4.0;
            wall([mean, mean, mean])
        });
        let color: Vec<[f32; 4]> = pixels.iter().map(|p| [p.color[0], p.color[1], p.color[2], 0.1875 / 4.0]).collect();
        let aov: Vec<[f32; 4]> = pixels
            .iter()
            .flat_map(|p| [[p.albedo[0], p.albedo[1], p.albedo[2], if p.hit { 1.0 } else { 0.0 }], [p.normal[0], p.normal[1], p.normal[2], 1.0]])
            .collect();
        let before = rmse(&color.iter().map(|p| [p[0], p[1], p[2], 1.0]).collect::<Vec<[f32; 4]>>(), 0.25);
        let out = denoise(&gpu, &color, &aov, (N, N)).unwrap();
        let after = rmse(&out, 0.25);
        assert!(after <= 0.3 * before, "RMSE {before} -> {after}: the dark pixels did not average with their brighter neighbours");
    }

    #[test]
    fn a_lighting_edge_with_low_variance_is_kept() {
        let Some(gpu) = compute() else { return };
        let pixels: Vec<Pixel> = image(|x, _| {
            let color = if x < N / 2 { [0.2; 3] } else { [0.8; 3] };
            wall(color)
        });
        let color: Vec<[f32; 4]> = pixels.iter().map(|p| [p.color[0], p.color[1], p.color[2], 1e-4]).collect();
        let aov: Vec<[f32; 4]> = pixels
            .iter()
            .flat_map(|p| [[p.albedo[0], p.albedo[1], p.albedo[2], if p.hit { 1.0 } else { 0.0 }], [p.normal[0], p.normal[1], p.normal[2], 1.0]])
            .collect();
        let out = denoise(&gpu, &color, &aov, (N, N)).unwrap();
        let left = band_mean(&out, N / 2 - 2, N / 2);
        let right = band_mean(&out, N / 2, N / 2 + 2);
        assert!((left - 0.2).abs() <= 0.01, "low-variance left edge band {left}");
        assert!((right - 0.8).abs() <= 0.01, "low-variance right edge band {right}");
    }

    #[test]
    fn a_lighting_edge_inside_the_noise_is_smoothed() {
        let Some(gpu) = compute() else { return };
        let pixels: Vec<Pixel> = image(|x, _| {
            let color = if x < N / 2 { [0.2; 3] } else { [0.8; 3] };
            wall(color)
        });
        let color: Vec<[f32; 4]> = pixels.iter().map(|p| [p.color[0], p.color[1], p.color[2], 0.05]).collect();
        let aov: Vec<[f32; 4]> = pixels
            .iter()
            .flat_map(|p| [[p.albedo[0], p.albedo[1], p.albedo[2], if p.hit { 1.0 } else { 0.0 }], [p.normal[0], p.normal[1], p.normal[2], 1.0]])
            .collect();
        let out = denoise(&gpu, &color, &aov, (N, N)).unwrap();
        let left = band_mean(&out, N / 2 - 2, N / 2);
        assert!(left > 0.21, "noise should smooth the edge, not keep it fixed: {left}");
    }

    #[test]
    fn buffers_that_do_not_fit_the_size_are_refused() {
        let Some(gpu) = compute() else { return };
        let color = vec![[0.5, 0.5, 0.5, 1.0]; 4];
        let aov = vec![[0.5, 0.5, 0.5, 1.0]; 8];
        assert!(denoise(&gpu, &color, &aov, (2, 2)).is_ok());
        assert!(denoise(&gpu, &color, &aov, (2, 3)).is_err(), "the colours do not match the size");
        assert!(denoise(&gpu, &color, &aov[..6], (2, 2)).is_err(), "the guide needs two entries a pixel");
        assert!(denoise(&gpu, &[], &[], (0, 0)).is_err(), "no pixels");
    }

    #[test]
    fn a_single_pixel_image_is_copied() {
        let Some(gpu) = compute() else { return };
        let pixels = vec![wall([0.3, 0.6, 0.9])];
        let out = run(&gpu, &pixels, (1, 1));
        let p = &out[0];
        assert!((p[0] - 0.3).abs() <= 1e-5 && (p[1] - 0.6).abs() <= 1e-5 && (p[2] - 0.9).abs() <= 1e-5, "the single pixel drifted: {p:?}");
    }

    #[test]
    fn hdr_values_are_preserved() {
        let Some(gpu) = compute() else { return };
        // A sunlit pixel at 100 000 nits and a dark pixel at 1 nit, side by side.
        let pixels = image(|x, _| if x < N / 2 { wall([100_000.0, 100_000.0, 100_000.0]) } else { wall([1.0, 1.0, 1.0]) });
        let out = run(&gpu, &pixels, (N, N));
        let bright = band_mean(&out, 0, N / 2);
        let dark = band_mean(&out, N / 2, N);
        assert!(bright > 99_000.0, "the bright side lost its HDR value: {bright}");
        assert!(dark < 2.0, "the dark side was lifted: {dark}");
    }

    #[test]
    fn an_edge_at_the_image_boundary_is_kept() {
        let Some(gpu) = compute() else { return };
        // The first column is a different albedo; the edge sits at the image boundary.
        let pixels = image(|x, _| {
            let color = if x == 0 { [0.3; 3] } else { [0.6; 3] };
            let mut p = wall(color);
            if x == 0 {
                p.albedo = [0.1; 3];
            } else {
                p.albedo = [0.9; 3];
            }
            p
        });
        let out = run(&gpu, &pixels, (N, N));
        let edge = band_mean(&out, 0, 1);
        let rest = band_mean(&out, 1, N);
        assert!((edge - 0.3).abs() <= 0.05, "the edge column was blurred: {edge}");
        assert!((rest - 0.6).abs() <= 0.05, "the rest was blurred: {rest}");
    }

    #[test]
    fn nan_in_the_guide_does_not_crash() {
        let Some(gpu) = compute() else { return };
        // A NaN normal on one pixel: the weight for that tap becomes NaN, but the centre tap
        // (which has a finite normal) keeps wsum > 0, so the pixel is still averaged.
        let mut pixels = image(|_, _| wall([0.5; 3]));
        pixels[(N / 2) as usize * N as usize + (N / 2) as usize].normal = [f32::NAN, 0.0, 1.0];
        let out = run(&gpu, &pixels, (N, N));
        assert!(out.iter().all(|p| p.iter().all(|v| v.is_finite())), "a NaN normal made the output non-finite");
    }
}
