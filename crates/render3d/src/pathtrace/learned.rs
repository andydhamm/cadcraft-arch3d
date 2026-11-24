//! The learned denoiser on the GPU (phase 22): the U-Net of `assets/denoise/README.md` in WGSL compute
//! shaders (`shaders/pt/unet.wgsl`). Autoexposure and the input and output transforms run on the CPU
//! (`crate::unet`); the network runs on the device as one compute pass per operation, all in one command
//! buffer per tile. A frame whose activations do not fit the device is processed in overlapping tiles; one
//! whose tiles do not fit either is an `Err`, and the caller (`post::finish_linear`) falls back to the
//! edge-aware filter and says why.

use std::sync::{Arc, Mutex};

use cadcraft_doc::arch::DenoiseModel;

use super::device::ComputeGpu;
use crate::tza::{self, ModelChoice};
use crate::unet::{self, Layer, Net};

/// Pixels two neighbouring tiles share; the output is blended linearly across them.
const OVERLAP: u32 = 32;
/// The largest tile edge (pixels).
const MAX_TILE: u32 = 1024;
/// The smallest tile edge tried when the frame does not fit.
const MIN_TILE: u32 = 256;
/// The most device memory the activations of one pass may take (bytes).
const MEMORY_BUDGET: u64 = 3 << 29;
/// The shader's workgroup edge.
const GROUP: u32 = 8;

/// The model `Auto` stands for on `gpu`: the base model on a GPU, the small one on a CPU adapter.
pub fn model_for(gpu: &ComputeGpu, wanted: DenoiseModel) -> ModelChoice {
    match wanted {
        DenoiseModel::Base => ModelChoice::Base,
        DenoiseModel::Small => ModelChoice::Small,
        DenoiseModel::Auto => {
            if gpu.info.adapter_type.eq_ignore_ascii_case("cpu") {
                ModelChoice::Small
            } else {
                ModelChoice::Base
            }
        }
    }
}

/// Whether the learned denoiser can run on `gpu`; `Err` says why not.
pub fn available(gpu: &ComputeGpu) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = gpu;
        Err("the learned denoiser is native only".to_string())
    }
    #[cfg(not(target_arch = "wasm32"))]
    check_limits(&gpu.device.limits(), super::device::compute_disabled_by_env())
}

/// The device-limit part of [`available`]: compute allowed, four storage buffers per stage, 8 x 8
/// workgroups, and storage bindings large enough for a [`MIN_TILE`] tile of the base model.
pub(super) fn check_limits(limits: &wgpu::Limits, compute_off: bool) -> Result<(), String> {
    if compute_off {
        return Err("the learned denoiser needs compute shaders, which CADCRAFT_RENDER_LIMITS=webgl2 turns off".to_string());
    }
    if limits.max_storage_buffers_per_shader_stage < 4 {
        return Err("the learned denoiser needs four storage buffers per shader stage".to_string());
    }
    if limits.max_compute_invocations_per_workgroup < GROUP * GROUP
        || limits.max_compute_workgroup_size_x < GROUP
        || limits.max_compute_workgroup_size_y < GROUP
    {
        return Err("the device's compute workgroups are too small for the learned denoiser".to_string());
    }
    // The widest activation of the base model is 64 channels (the small model's is 64 as well).
    let tile_bytes = 64 * u64::from(MIN_TILE) * u64::from(MIN_TILE) * 4;
    if u64::from(limits.max_storage_buffer_binding_size) < tile_bytes || limits.max_buffer_size < tile_bytes {
        return Err(format!("the device's storage buffers (under {tile_bytes} bytes) are too small for the learned denoiser"));
    }
    Ok(())
}

/// Denoise `color` (linear RGB mean radiance, w unused) guided by `aov` (the kernel's two entries per
/// pixel: albedo and the hit flag in w, then normal and distance) with the learned model `model`. Returns
/// the colours with w = 1; pixels whose camera ray hit nothing keep their input colour.
pub fn denoise(gpu: &ComputeGpu, model: ModelChoice, color: &[[f32; 4]], aov: &[[f32; 4]], size: (u32, u32)) -> Result<Vec<[f32; 4]>, String> {
    denoise_tiled(gpu, model, color, aov, size, None)
}

/// [`denoise`], with `tile` forcing the tile edge (a multiple of 16, at least 64) instead of choosing it.
pub(super) fn denoise_tiled(
    gpu: &ComputeGpu,
    model: ModelChoice,
    color: &[[f32; 4]],
    aov: &[[f32; 4]],
    size: (u32, u32),
    tile: Option<u32>,
) -> Result<Vec<[f32; 4]>, String> {
    available(gpu)?;
    let (w, h) = size;
    let pixels = u64::from(w) * u64::from(h);
    if w == 0 || h == 0 || pixels > (1 << 28) {
        return Err(format!("the learned denoiser cannot take a {w} x {h} image"));
    }
    let n = pixels as usize;
    if color.len() != n || aov.len() != 2 * n {
        return Err(format!("learned denoiser: {} colours and {} guide entries for a {w} x {h} image", color.len(), aov.len()));
    }
    let net = Net::from_model(tza::model(model)?)?;
    let rgb: Vec<[f32; 3]> = color.iter().map(|p| [p[0], p[1], p[2]]).collect();
    let albedo: Vec<[f32; 3]> = (0..n).map(|i| aov.get(2 * i).map_or([0.0; 3], |p| [p[0], p[1], p[2]])).collect();
    let normal: Vec<[f32; 3]> = (0..n).map(|i| aov.get(2 * i + 1).map_or([0.0; 3], |p| [p[0], p[1], p[2]])).collect();
    let scale = unet::autoexposure(&rgb, size);
    let (input, padded) = unet::encode_input(&rgb, &albedo, &normal, size, scale);

    let pipes = pipelines(gpu)?;
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let (weights, offsets) = upload_weights(gpu, &net)?;
    let candidates: Vec<(u32, u32)> = match tile {
        Some(t) => {
            if t < 64 || t % 16 != 0 {
                return Err(format!("learned denoiser: tile edge {t} is not a multiple of 16 of at least 64"));
            }
            vec![(t.min(padded.0), t.min(padded.1))]
        }
        None => {
            let mut v = vec![padded];
            let mut t = MAX_TILE;
            while t >= MIN_TILE {
                let c = (t.min(padded.0), t.min(padded.1));
                if v.last() != Some(&c) {
                    v.push(c);
                }
                t /= 2;
            }
            v
        }
    };
    let mut why = String::new();
    let mut plan = None;
    for c in candidates {
        match Plan::build(gpu, &pipes, &net, &weights, &offsets, c) {
            Ok(p) => {
                plan = Some(p);
                break;
            }
            Err(e) => why = e,
        }
    }
    let Some(plan) = plan else {
        return Err(format!("the learned denoiser does not fit on this device: {why}"));
    };
    let out = run_tiles(gpu, &plan, &input, padded)?;
    if let Some(e) = futures::executor::block_on(scope.pop()) {
        return Err(format!("learned denoiser: {e}"));
    }
    let decoded = unet::decode_output(&out, padded, size, scale);
    Ok(decoded
        .iter()
        .zip(color)
        .enumerate()
        .map(|(i, (d, c))| {
            let hit = aov.get(2 * i).is_some_and(|a| a[3] > 0.5);
            if hit { [d[0], d[1], d[2], 1.0] } else { [c[0], c[1], c[2], 1.0] }
        })
        .collect())
}

/// The raw network on the device: the 9 CHW channels `input` at `padded` (multiples of 16) to the 3 output
/// channels, in one tile. The counterpart of `unet::forward_cpu`, for the tests.
#[cfg(test)]
pub(super) fn forward_gpu(gpu: &ComputeGpu, net: &Net, input: &[f32], padded: (u32, u32)) -> Result<Vec<f32>, String> {
    let pipes = pipelines(gpu)?;
    let (weights, offsets) = upload_weights(gpu, net)?;
    let plan = Plan::build(gpu, &pipes, net, &weights, &offsets, padded)?;
    run_tiles(gpu, &plan, input, padded)
}

/// The three pipelines and their one bind group layout.
struct Pipes {
    layout: wgpu::BindGroupLayout,
    conv: wgpu::ComputePipeline,
    pool: wgpu::ComputePipeline,
    up: wgpu::ComputePipeline,
}

/// Built once per device and kept for the process (like `PtPipelines::shared`; devices are never
/// destroyed, see `request_compute_with`).
static PIPES: Mutex<Vec<(wgpu::Device, Arc<Pipes>)>> = Mutex::new(Vec::new());

fn pipelines(gpu: &ComputeGpu) -> Result<Arc<Pipes>, String> {
    let mut slot = PIPES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, p)) = slot.iter().find(|(d, _)| *d == gpu.device) {
        return Ok(p.clone());
    }
    let d = &gpu.device;
    let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
    let entry = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    };
    let read = wgpu::BufferBindingType::Storage { read_only: true };
    let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("unet"),
        entries: &[
            entry(0, wgpu::BufferBindingType::Uniform),
            entry(1, read),
            entry(2, read),
            entry(3, read),
            entry(4, wgpu::BufferBindingType::Storage { read_only: false }),
        ],
    });
    let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("unet"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/pt/unet.wgsl").into()),
    });
    let pl =
        d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("unet"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
    let make = |entry_point: &str| {
        d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(entry_point),
            layout: Some(&pl),
            module: &module,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            cache: None,
        })
    };
    let built = Arc::new(Pipes { conv: make("conv_main"), pool: make("pool_main"), up: make("upsample_main"), layout });
    if let Some(e) = futures::executor::block_on(scope.pop()) {
        return Err(format!("the learned denoiser's pipelines do not build: {e}"));
    }
    slot.push((gpu.device.clone(), built.clone()));
    Ok(built)
}

/// Every layer's weights then bias in one buffer; the (weights, bias) offsets in floats per layer.
fn upload_weights(gpu: &ComputeGpu, net: &Net) -> Result<(wgpu::Buffer, Vec<(u32, u32)>), String> {
    let mut flat: Vec<f32> = Vec::new();
    let mut offsets = Vec::with_capacity(net.layers.len());
    for l in &net.layers {
        let w = u32::try_from(flat.len()).map_err(|_| "the weights are too large".to_string())?;
        flat.extend_from_slice(&l.weights);
        let b = u32::try_from(flat.len()).map_err(|_| "the weights are too large".to_string())?;
        flat.extend_from_slice(&l.bias);
        offsets.push((w, b));
    }
    let bytes = (flat.len() * 4) as u64;
    let limits = gpu.device.limits();
    if bytes > u64::from(limits.max_storage_buffer_binding_size) {
        return Err(format!("the weights ({bytes} bytes) do not fit a storage buffer"));
    }
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("unet weights"),
        size: bytes.max(4),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu.queue.write_buffer(&buf, 0, bytemuck::cast_slice(&flat));
    Ok((buf, offsets))
}

/// One compute pass: a pipeline, its bind group and its dispatch size.
struct Step {
    pipeline: wgpu::ComputePipeline,
    bind: wgpu::BindGroup,
    groups: [u32; 3],
}

/// The network for one tile size: its buffers and the passes that run it.
struct Plan {
    tile: (u32, u32),
    input: wgpu::Buffer,
    out: wgpu::Buffer,
    read: wgpu::Buffer,
    steps: Vec<Step>,
}

struct Builder<'a> {
    gpu: &'a ComputeGpu,
    pipes: &'a Pipes,
    weights: &'a wgpu::Buffer,
    steps: Vec<Step>,
    total: u64,
    per_buffer: u64,
    max_groups: u32,
}

impl Builder<'_> {
    /// A storage buffer of `floats` values; `Err` when it is above a limit or the memory budget.
    fn alloc(&mut self, label: &str, floats: u64, usage: wgpu::BufferUsages) -> Result<wgpu::Buffer, String> {
        let bytes = floats.checked_mul(4).ok_or("buffer size overflows")?;
        if bytes > self.per_buffer || floats > u64::from(u32::MAX) {
            return Err(format!("{label} needs {bytes} bytes; the device allows {} per storage buffer", self.per_buffer));
        }
        self.total += bytes;
        if self.total > MEMORY_BUDGET {
            return Err(format!("the activations need more than {} MB", MEMORY_BUDGET >> 20));
        }
        Ok(self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: bytes.max(4),
            usage: wgpu::BufferUsages::STORAGE | usage,
            mapped_at_creation: false,
        }))
    }

    fn step(
        &mut self,
        pipeline: &wgpu::ComputePipeline,
        params: [u32; 8],
        a: &wgpu::Buffer,
        b: &wgpu::Buffer,
        out: &wgpu::Buffer,
        groups: [u32; 3],
    ) -> Result<(), String> {
        if groups.iter().any(|g| *g > self.max_groups) {
            return Err("the tile is more workgroups than the device dispatches".to_string());
        }
        let d = &self.gpu.device;
        let uniform = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("unet params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.gpu.queue.write_buffer(&uniform, 0, bytemuck::cast_slice(&params));
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("unet"),
            layout: &self.pipes.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.weights.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: a.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: b.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: out.as_entire_binding() },
            ],
        });
        self.steps.push(Step { pipeline: pipeline.clone(), bind, groups });
        Ok(())
    }

    /// A convolution of `layer` over `a` (`ca` channels) concatenated with `b` (`cb`), at `w` x `h`.
    #[allow(clippy::too_many_arguments)]
    fn conv(
        &mut self,
        layer: &Layer,
        off: (u32, u32),
        a: (&wgpu::Buffer, u32),
        b: Option<(&wgpu::Buffer, u32)>,
        out: &wgpu::Buffer,
        (w, h): (u32, u32),
    ) -> Result<(), String> {
        let (bb, cb) = b.unwrap_or((a.0, 0));
        if a.1 + cb != layer.cin {
            return Err(format!("layer {} takes {} channels, the wiring gives {}", layer.name, layer.cin, a.1 + cb));
        }
        let params = [a.1, cb, layer.cout, w, h, off.0, off.1, 0];
        let pipeline = self.pipes.conv.clone();
        self.step(&pipeline, params, a.0, bb, out, [w.div_ceil(GROUP), h.div_ceil(GROUP), layer.cout.div_ceil(4)])
    }

    /// Max-pool `c` channels from `a` into `out`, whose size is `w` x `h`.
    fn pool(&mut self, a: &wgpu::Buffer, out: &wgpu::Buffer, c: u32, (w, h): (u32, u32)) -> Result<(), String> {
        let pipeline = self.pipes.pool.clone();
        self.step(&pipeline, [c, 0, c, w, h, 0, 0, 0], a, a, out, [w.div_ceil(GROUP), h.div_ceil(GROUP), c])
    }

    /// Upsample `c` channels from `a` into `out`, whose size is `w` x `h`.
    fn up(&mut self, a: &wgpu::Buffer, out: &wgpu::Buffer, c: u32, (w, h): (u32, u32)) -> Result<(), String> {
        let pipeline = self.pipes.up.clone();
        self.step(&pipeline, [c, 0, c, w, h, 0, 0, 0], a, a, out, [w.div_ceil(GROUP), h.div_ceil(GROUP), c])
    }
}

impl Plan {
    /// Allocate the activations of `net` for tiles of `tile` pixels (multiples of 16) and record its passes.
    fn build(gpu: &ComputeGpu, pipes: &Pipes, net: &Net, weights: &wgpu::Buffer, offsets: &[(u32, u32)], tile: (u32, u32)) -> Result<Plan, String> {
        let limits = gpu.device.limits();
        let mut b = Builder {
            gpu,
            pipes,
            weights,
            steps: Vec::new(),
            total: 0,
            per_buffer: u64::from(limits.max_storage_buffer_binding_size).min(limits.max_buffer_size),
            max_groups: limits.max_compute_workgroups_per_dimension,
        };
        let (tw, th) = tile;
        let lv = |l: u32| (tw >> l, th >> l);
        let px = |l: u32| u64::from(tw >> l) * u64::from(th >> l);
        let layer = |i: usize| -> Result<(&Layer, (u32, u32)), String> {
            let l = net.layers.get(i).ok_or("the net is missing a layer")?;
            let o = offsets.get(i).copied().ok_or("the net is missing a layer")?;
            Ok((l, o))
        };
        let ch = |i: usize| net.layers.get(i).map_or(0, |l| l.cout);
        let store = wgpu::BufferUsages::empty();

        let input = b.alloc("unet input", 9 * px(0), wgpu::BufferUsages::COPY_DST)?;
        let s0_ch = ch(0).max(ch(12)).max(ch(14));
        let s1_ch = ch(1).max(ch(13));
        let s0 = b.alloc("unet full a", u64::from(s0_ch) * px(0), store)?;
        let s1 = b.alloc("unet full b", u64::from(s1_ch) * px(0), store)?;

        // Encoder.
        let (l, o) = layer(0)?;
        b.conv(l, o, (&input, 9), None, &s0, lv(0))?;
        let (l, o) = layer(1)?;
        b.conv(l, o, (&s0, ch(0)), None, &s1, lv(0))?;
        let p1 = b.alloc("unet pool1", u64::from(ch(1)) * px(1), store)?;
        b.pool(&s1, &p1, ch(1), lv(1))?;
        // Levels 1..3: a convolution then a pool; pools[k] is the pool at level k + 1.
        let mut pools: Vec<(wgpu::Buffer, u32)> = vec![(p1, ch(1))];
        for (i, level) in [(2usize, 1u32), (3, 2), (4, 3)] {
            let (src, cs) = pools.last().cloned().ok_or("encoder state is missing")?;
            let (l, o) = layer(i)?;
            let e = b.alloc("unet enc", u64::from(l.cout) * px(level), store)?;
            b.conv(l, o, (&src, cs), None, &e, lv(level))?;
            let p = b.alloc("unet pool", u64::from(l.cout) * px(level + 1), store)?;
            b.pool(&e, &p, l.cout, lv(level + 1))?;
            pools.push((p, l.cout));
        }
        let (p4, c4) = pools.last().cloned().ok_or("encoder state is missing")?;
        let (l, o) = layer(5)?;
        let t5 = b.alloc("unet bottleneck a", u64::from(l.cout) * px(4), store)?;
        b.conv(l, o, (&p4, c4), None, &t5, lv(4))?;
        let (l6, o6) = layer(6)?;
        let mut x = b.alloc("unet bottleneck b", u64::from(l6.cout) * px(4), store)?;
        b.conv(l6, o6, (&t5, l.cout), None, &x, lv(4))?;
        let mut xc = l6.cout;

        // Decoder: levels 3, 2, 1 (first layer 7, 9, 11), skips p3, p2, p1.
        for (first, level) in [(7usize, 3u32), (9, 2), (11, 1)] {
            let (skip, sc) = pools.get(level as usize - 1).cloned().ok_or("encoder state is missing")?;
            let up = b.alloc("unet up", u64::from(xc) * px(level), store)?;
            b.up(&x, &up, xc, lv(level))?;
            let (la, oa) = layer(first)?;
            let t = b.alloc("unet dec a", u64::from(la.cout) * px(level), store)?;
            b.conv(la, oa, (&up, xc), Some((&skip, sc)), &t, lv(level))?;
            let (lb, ob) = layer(first + 1)?;
            let nx = b.alloc("unet dec b", u64::from(lb.cout) * px(level), store)?;
            b.conv(lb, ob, (&t, la.cout), None, &nx, lv(level))?;
            x = nx;
            xc = lb.cout;
        }
        // Full resolution: the two scratch buffers are reused.
        b.up(&x, &s0, xc, lv(0))?;
        let (l, o) = layer(13)?;
        b.conv(l, o, (&s0, xc), Some((&input, 9)), &s1, lv(0))?;
        let c13 = l.cout;
        let (l, o) = layer(14)?;
        b.conv(l, o, (&s1, c13), None, &s0, lv(0))?;
        let c14 = l.cout;
        let (l, o) = layer(15)?;
        if l.cout != 3 {
            return Err("the last layer does not have three outputs".to_string());
        }
        let out = b.alloc("unet output", 3 * px(0), wgpu::BufferUsages::COPY_SRC)?;
        b.conv(l, o, (&s0, c14), None, &out, lv(0))?;
        let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("unet readback"),
            size: 3 * px(0) * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Plan { tile, input, out, read, steps: b.steps })
    }
}

/// Tile origins along a side of `total` pixels with tiles of `t`: overlapping by [`OVERLAP`], the last
/// one flush with the end when there are only two tiles, otherwise at the regular step.
fn starts(total: u32, t: u32) -> Vec<u32> {
    let mut v = vec![0];
    if total <= t {
        return v;
    }
    let step = t.saturating_sub(OVERLAP).max(16);
    let mut s = step;
    while s + t < total {
        v.push(s);
        s += step;
    }
    if v.len() == 1 {
        v.push(total - t);
    } else {
        v.push(s);
    }
    v
}

/// The blend weight of position `i` along a tile of `t` pixels: a linear ramp over the overlap at each
/// edge that has a neighbour tile, 1 elsewhere.
fn ramp(i: u32, t: u32, first: bool, last: bool) -> f32 {
    let mut w = 1.0_f32;
    if !first && i < OVERLAP {
        w = w.min((i as f32 + 0.5) / OVERLAP as f32);
    }
    let from_end = t - 1 - i;
    if !last && from_end < OVERLAP {
        w = w.min((from_end as f32 + 0.5) / OVERLAP as f32);
    }
    w
}

/// Run `plan` over every tile of the encoded `input` (9 CHW channels at `padded`), blending the overlaps;
/// returns the 3 CHW output channels at `padded`.
fn run_tiles(gpu: &ComputeGpu, plan: &Plan, input: &[f32], padded: (u32, u32)) -> Result<Vec<f32>, String> {
    let (pw, ph) = (padded.0 as usize, padded.1 as usize);
    let plane = pw * ph;
    let (tw, th) = plan.tile;
    let mut acc = vec![0.0_f32; 3 * plane];
    let mut weight = vec![0.0_f32; plane];
    let xs = starts(padded.0, tw);
    let ys = starts(padded.1, th);
    let mut crop = vec![0.0_f32; 9 * tw as usize * th as usize];
    for (yi, &y0) in ys.iter().enumerate() {
        for (xi, &x0) in xs.iter().enumerate() {
            for c in 0..9usize {
                for r in 0..th as usize {
                    let src = c * plane + (y0 as usize + r) * pw + x0 as usize;
                    let dst = (c * th as usize + r) * tw as usize;
                    let (Some(s), Some(d)) = (input.get(src..src + tw as usize), crop.get_mut(dst..dst + tw as usize)) else {
                        return Err("the tile is outside the image".to_string());
                    };
                    d.copy_from_slice(s);
                }
            }
            gpu.queue.write_buffer(&plan.input, 0, bytemuck::cast_slice(&crop));
            let mut enc = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("unet") });
            for s in &plan.steps {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("unet"), timestamp_writes: None });
                pass.set_pipeline(&s.pipeline);
                pass.set_bind_group(0, &s.bind, &[]);
                pass.dispatch_workgroups(s.groups[0], s.groups[1], s.groups[2]);
            }
            enc.copy_buffer_to_buffer(&plan.out, 0, &plan.read, 0, 3 * u64::from(tw) * u64::from(th) * 4);
            gpu.queue.submit([enc.finish()]);
            let out = read_back(gpu, &plan.read)?;
            let first = (xi == 0, yi == 0);
            let last = (xi + 1 == xs.len(), yi + 1 == ys.len());
            for r in 0..th {
                let wy = ramp(r, th, first.1, last.1);
                for c in 0..tw {
                    let wgt = wy * ramp(c, tw, first.0, last.0);
                    let at = (y0 + r) as usize * pw + (x0 + c) as usize;
                    let local = r as usize * tw as usize + c as usize;
                    if let Some(w) = weight.get_mut(at) {
                        *w += wgt;
                    }
                    for k in 0..3 {
                        let v = out.get(k * tw as usize * th as usize + local).copied().unwrap_or(0.0);
                        if let Some(a) = acc.get_mut(k * plane + at) {
                            *a += wgt * v;
                        }
                    }
                }
            }
        }
    }
    for k in 0..3 {
        for (i, w) in weight.iter().enumerate() {
            if let Some(a) = acc.get_mut(k * plane + i) {
                *a /= w.max(1e-6);
            }
        }
    }
    Ok(acc)
}

/// The floats of the `MAP_READ` buffer `read`.
fn read_back(gpu: &ComputeGpu, read: &wgpu::Buffer) -> Result<Vec<f32>, String> {
    let slice = read.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    gpu.device
        .poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(300)) })
        .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    let out: Vec<f32> = {
        let view = slice.get_mapped_range().map_err(|e| e.to_string())?;
        bytemuck::cast_slice(&view).to_vec()
    };
    read.unmap();
    Ok(out)
}

#[cfg(test)]
#[path = "learned_tests.rs"]
mod tests;
