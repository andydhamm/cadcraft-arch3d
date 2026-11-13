//! The backdrop behind the 3D view: a colour, a vertical gradient, or an image (flat or an
//! equirectangular panorama), shown only where no surface is hit. [`backdrop_rgb`] is the CPU
//! reference of the colour of one pixel; [`BackdropPass`] is the GPU pass with the same formulas
//! (`shaders/backdrop.wgsl`). sRGB values are written as they are, so a `#204060` backdrop reads
//! back as (32, 64, 96) ±1.

use std::collections::BTreeMap;

use cadcraft_doc::arch::{Backdrop, ImageRgba, Projection};

use crate::environment::Environment;
use crate::pipeline::{COLOR_FORMAT, DEPTH_FORMAT};

/// WGSL source of the backdrop pipeline.
const SHADER: &str = include_str!("shaders/backdrop.wgsl");

/// The sRGB bytes of [`crate::offscreen::CLEAR`]: what a pixel shows when there is no backdrop or its
/// image is missing.
pub const CLEAR_RGB: [u8; 3] = [39, 44, 52];

/// The resident texture key of the baked environment.
const ENV_KEY: &str = "environment";

/// Byte size of [`BackdropUniform`].
const UNIFORM_SIZE: u64 = 128;

/// Uniform block of the backdrop shader (WGSL `Backdrop`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BackdropUniform {
    inv_view_proj: [[f32; 4]; 4],
    top: [f32; 4],
    bottom: [f32; 4],
    size: [f32; 4],
    kind: [u32; 4],
}

/// The texture key of a backdrop image item in `Scene3d.textures`.
fn image_key(item: &str) -> String {
    format!("item:{item}")
}

/// The one texture an image backdrop `b` needs out of `textures` (`Scene3d.textures`), under its own key, for a
/// renderer that keeps a copy (the path tracer); empty for a colour, a gradient, none or a missing image.
pub fn image_textures(b: Option<&Backdrop>, textures: &BTreeMap<String, ImageRgba>) -> BTreeMap<String, ImageRgba> {
    match b {
        Some(Backdrop::Image { item, .. }) => {
            let key = image_key(item);
            textures.get(&key).map(|img| BTreeMap::from([(key, img.clone())])).unwrap_or_default()
        }
        _ => BTreeMap::new(),
    }
}

/// The usable image of `b` in `textures`, with its projection.
fn image_of<'a>(b: &Backdrop, textures: &'a BTreeMap<String, ImageRgba>) -> Option<(&'a ImageRgba, Projection)> {
    match b {
        Backdrop::Image { item, projection, .. } => textures.get(&image_key(item)).filter(|i| i.is_valid()).map(|i| (i, *projection)),
        _ => None,
    }
}

/// One texel in sRGB bytes; indices are already in range.
fn texel(img: &ImageRgba, x: i64, y: i64) -> [f32; 3] {
    let at = (y.max(0) as usize).saturating_mul(img.width as usize).saturating_add(x.max(0) as usize).saturating_mul(4);
    match img.rgba.get(at..at + 3) {
        Some(p) => [f32::from(p[0]), f32::from(p[1]), f32::from(p[2])],
        None => [0.0; 3],
    }
}

/// `img` sampled bilinearly at `(u, v)` in 0..1; x clamps, or wraps when `wrap_x`; y clamps.
fn sample(img: &ImageRgba, u: f64, v: f64, wrap_x: bool) -> [f32; 3] {
    let (iw, ih) = (i64::from(img.width), i64::from(img.height));
    let fx = u * iw as f64 - 0.5;
    let fy = v * ih as f64 - 0.5;
    if !fx.is_finite() || !fy.is_finite() {
        return [0.0; 3];
    }
    let (x0f, y0f) = (fx.floor(), fy.floor());
    let (tx, ty) = ((fx - x0f) as f32, (fy - y0f) as f32);
    let fix_x = |x: i64| if wrap_x { x.rem_euclid(iw) } else { x.clamp(0, iw - 1) };
    let fix_y = |y: i64| y.clamp(0, ih - 1);
    let (x0, x1) = (fix_x(x0f as i64), fix_x(x0f as i64 + 1));
    let (y0, y1) = (fix_y(y0f as i64), fix_y(y0f as i64 + 1));
    let mut out = [0.0f32; 3];
    let (a, b, c, d) = (texel(img, x0, y0), texel(img, x1, y0), texel(img, x0, y1), texel(img, x1, y1));
    for k in 0..3 {
        let top = a[k] + (b[k] - a[k]) * tx;
        let bottom = c[k] + (d[k] - c[k]) * tx;
        out[k] = top + (bottom - top) * ty;
    }
    out
}

fn to_byte(v: f32) -> u8 {
    (v + 0.5).clamp(0.0, 255.0) as u8
}

/// The colour (sRGB bytes) of pixel `(x, y)` of a `w` × `h` image: the CPU reference of the GPU
/// pass. `ray` is the unit camera ray through the pixel centre (used by equirect images only);
/// `textures` is `Scene3d.textures`, where an image backdrop's pixels are `item:<id>`. A missing or
/// invalid image gives [`CLEAR_RGB`].
pub fn backdrop_rgb(b: &Backdrop, textures: &BTreeMap<String, ImageRgba>, x: u32, y: u32, w: u32, h: u32, ray: [f32; 3]) -> [u8; 3] {
    let (wf, hf) = (f64::from(w.max(1)), f64::from(h.max(1)));
    match b {
        Backdrop::Colour { colour } => *colour,
        Backdrop::Gradient { top, bottom } => {
            let t = (f64::from(y) + 0.5) / hf;
            let mut out = [0u8; 3];
            for k in 0..3 {
                let (a, c) = (f64::from(top[k]), f64::from(bottom[k]));
                out[k] = (a + (c - a) * t).round().clamp(0.0, 255.0) as u8;
            }
            out
        }
        Backdrop::Sky | Backdrop::Hdri { .. } => CLEAR_RGB,
        Backdrop::Image { .. } => {
            let Some((img, projection)) = image_of(b, textures) else { return CLEAR_RGB };
            let horizon = if let Backdrop::Image { horizon, .. } = b { *horizon } else { 0.0 };
            let rgb = match projection {
                Projection::Flat => sample(img, (f64::from(x) + 0.5) / wf, (f64::from(y) + 0.5) / hf, false),
                Projection::Equirect => {
                    let (dx, dy, dz) = (f64::from(ray[0]), f64::from(ray[1]), f64::from(ray[2]));
                    let u = 0.5 + dy.atan2(dx) / std::f64::consts::TAU;
                    // A positive horizon raises the picture's horizon: sample lower in it.
                    let half = std::f64::consts::FRAC_PI_2;
                    let el = (dz.clamp(-1.0, 1.0).asin() - horizon.to_radians()).clamp(-half, half);
                    let v = 0.5 - el / std::f64::consts::PI;
                    sample(img, u, v, true)
                }
            };
            [to_byte(rgb[0]), to_byte(rgb[1]), to_byte(rgb[2])]
        }
    }
}

/// Paint the backdrop into the pixels of `rgba` (tight RGBA8, `w` × `h`, rows top to bottom) where
/// `missed[y * w + x]` is true. `ray_of(x, y)` gives the unit camera ray through a pixel centre.
/// A buffer of the wrong length is left unchanged.
pub fn composite_misses(
    rgba: &mut [u8],
    w: u32,
    h: u32,
    missed: &[bool],
    b: &Backdrop,
    textures: &BTreeMap<String, ImageRgba>,
    ray_of: &dyn Fn(u32, u32) -> [f32; 3],
) {
    // A sky or HDRI backdrop paints nothing: the path tracer's own miss radiance is the environment,
    // and camera rays that read it still count as misses.
    if matches!(b, Backdrop::Sky | Backdrop::Hdri { .. }) {
        return;
    }
    let n = (w as usize).saturating_mul(h as usize);
    if w == 0 || h == 0 || rgba.len() != n.saturating_mul(4) || missed.len() != n {
        return;
    }
    let equirect = matches!(b, Backdrop::Image { projection: Projection::Equirect, .. });
    for y in 0..h {
        for x in 0..w {
            let i = (y as usize) * (w as usize) + x as usize;
            if !missed.get(i).copied().unwrap_or(false) {
                continue;
            }
            let ray = if equirect { ray_of(x, y) } else { [0.0, 0.0, 1.0] };
            let c = backdrop_rgb(b, textures, x, y, w, h, ray);
            if let Some(px) = rgba.get_mut(i * 4..i * 4 + 4) {
                px.copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
    }
}

/// Invert a column-major 4×4 matrix in f64; the identity when it is singular or not finite.
fn invert(m: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut a = [[0.0f64; 8]; 4];
    for r in 0..4 {
        for c in 0..4 {
            a[r][c] = f64::from(m[c][r]);
        }
        a[r][4 + r] = 1.0;
    }
    for col in 0..4 {
        let mut piv = col;
        for r in col + 1..4 {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        if !(a[piv][col].abs() > 1e-18) {
            return [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        }
        a.swap(col, piv);
        let p = a[col][col];
        for v in a[col].iter_mut() {
            *v /= p;
        }
        let pivot_row = a[col];
        for (r, row) in a.iter_mut().enumerate() {
            if r != col {
                let f = row[col];
                for (v, pv) in row.iter_mut().zip(pivot_row.iter()) {
                    *v -= f * pv;
                }
            }
        }
    }
    let mut out = [[0.0f32; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            out[c][r] = a[r][4 + c] as f32;
        }
    }
    out
}

/// Cheap identity of an image's pixels: size, buffer address and a sample of its bytes.
fn fingerprint(img: &ImageRgba) -> (u32, u32, usize, u64) {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let step = (img.rgba.len() / 512).max(1);
    for b in img.rgba.iter().step_by(step) {
        h = (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3);
    }
    (img.width, img.height, img.rgba.as_ptr() as usize, h)
}

/// Shrink `img` by an integer box filter so both sides fit `max` (sRGB bytes averaged).
fn fit(img: &ImageRgba, max: u32) -> ImageRgba {
    let f = img.width.max(img.height).div_ceil(max.max(1)).max(1);
    if f == 1 {
        return img.clone();
    }
    let (w, h) = ((img.width / f).max(1), (img.height / f).max(1));
    let mut rgba = Vec::with_capacity((w as usize).saturating_mul(h as usize).saturating_mul(4));
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0u32; 4];
            for dy in 0..f {
                for dx in 0..f {
                    let at = (((y * f + dy) as usize) * img.width as usize + (x * f + dx) as usize) * 4;
                    if let Some(p) = img.rgba.get(at..at + 4) {
                        for (s, &c) in sum.iter_mut().zip(p) {
                            *s += u32::from(c);
                        }
                    }
                }
            }
            rgba.extend(sum.iter().map(|s| ((s + f * f / 2) / (f * f)) as u8));
        }
    }
    ImageRgba { width: w, height: h, rgba, ..Default::default() }
}

struct Resident {
    key: String,
    print: (u32, u32, usize, u64),
    size: (u32, u32),
    view: wgpu::TextureView,
}

/// The backdrop render pipeline: a fullscreen triangle, no depth test or write.
pub struct BackdropPass {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    dummy: wgpu::TextureView,
    image: Option<Resident>,
    bind_group: Option<wgpu::BindGroup>,
    uploads: u64,
}

fn make_texture(device: &wgpu::Device, queue: &wgpu::Queue, img: &ImageRgba) -> wgpu::TextureView {
    let size = wgpu::Extent3d { width: img.width, height: img.height, depth_or_array_layers: 1 };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("render3d backdrop image"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        &img.rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(img.width * 4), rows_per_image: Some(img.height) },
        size,
    );
    tex.create_view(&Default::default())
}

impl BackdropPass {
    /// Build the pipeline for the main pass (1 sample, [`COLOR_FORMAT`], [`DEPTH_FORMAT`]).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        // The tone curves go in front of the backdrop code (kind 4 tone maps the environment).
        let source = format!("{}\n{}", crate::exposure::TONEMAP_WGSL, SHADER);
        let module = device
            .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("render3d backdrop"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render3d backdrop bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZero::new(UNIFORM_SIZE),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render3d backdrop pipeline layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render3d backdrop pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render3d backdrop uniform"),
            size: UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dummy = make_texture(device, queue, &ImageRgba { width: 1, height: 1, rgba: vec![0, 0, 0, 255], ..Default::default() });
        BackdropPass { pipeline, bgl, uniform, dummy, image: None, bind_group: None, uploads: 0 }
    }

    /// How many image textures were uploaded (an image is uploaded once per key).
    pub fn uploads(&self) -> u64 {
        self.uploads
    }

    /// Write this frame's uniform for a `size` target seen through `view_proj` (column-major, as
    /// [`crate::camera::view_proj`] gives it) and make the image resident. `false` when there is
    /// nothing to draw (no backdrop, or an image that is missing or invalid: the clear colour
    /// shows).
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        b: &Backdrop,
        textures: &BTreeMap<String, ImageRgba>,
        size: (u32, u32),
        view_proj: &[[f32; 4]; 4],
    ) -> bool {
        let mut u = BackdropUniform {
            inv_view_proj: invert(view_proj),
            top: [0.0; 4],
            bottom: [0.0; 4],
            size: [size.0 as f32, size.1 as f32, 1.0, 1.0],
            kind: [0; 4],
        };
        let f = |c: &[u8; 3]| [f32::from(c[0]), f32::from(c[1]), f32::from(c[2]), 255.0];
        match b {
            Backdrop::Colour { colour } => u.top = f(colour),
            Backdrop::Gradient { top, bottom } => {
                u.top = f(top);
                u.bottom = f(bottom);
                u.kind[0] = 1;
            }
            // The sky and an HDRI are not pictures here: the clear colour (p19-env-raster draws them later).
            Backdrop::Sky | Backdrop::Hdri { .. } => u.top = f(&CLEAR_RGB),
            Backdrop::Image { item, projection, .. } => {
                let Some((img, _)) = image_of(b, textures) else { return false };
                let key = image_key(item);
                let print = fingerprint(img);
                if self.image.as_ref().is_none_or(|r| r.key != key || r.print != print) {
                    let max = device.limits().max_texture_dimension_2d.max(1);
                    let fitted = fit(img, max);
                    let view = make_texture(device, queue, &fitted);
                    self.image = Some(Resident { key, print, size: (fitted.width, fitted.height), view });
                    self.uploads += 1;
                }
                let Some(res) = &self.image else { return false };
                u.size[2] = res.size.0 as f32;
                u.size[3] = res.size.1 as f32;
                u.kind[0] = if *projection == Projection::Equirect { 3 } else { 2 };
                if let Backdrop::Image { horizon, .. } = b {
                    u.top[3] = *horizon as f32;
                }
            }
        }
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let view = if u.kind[0] >= 2 { self.image.as_ref().map(|r| &r.view).unwrap_or(&self.dummy) } else { &self.dummy };
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render3d backdrop"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
            ],
        }));
        true
    }

    /// Like [`BackdropPass::prepare`] for a visible environment (kind 4): a world-space equirect
    /// copy (at most 1024 x 512, baked with [`Environment::radiance`], re-uploaded when `env.key`
    /// changes) drawn with the frame's `exposure` scale and `tone` code. `false` for an invalid
    /// environment.
    pub fn prepare_environment(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        env: &Environment,
        exposure: f32,
        tone: u32,
        size: (u32, u32),
        view_proj: &[[f32; 4]; 4],
    ) -> bool {
        if !env.is_valid() {
            return false;
        }
        let (w, h) = (env.width.clamp(256, 1024), env.height.clamp(128, 512));
        let print = (w, h, 0usize, env.key);
        if self.image.as_ref().is_none_or(|r| r.key != ENV_KEY || r.print != print) {
            let pi = std::f32::consts::PI;
            let mut texels: Vec<f32> = Vec::with_capacity((w as usize).saturating_mul(h as usize).saturating_mul(4));
            for j in 0..h {
                let el = (0.5 - (j as f32 + 0.5) / h as f32) * pi;
                for i in 0..w {
                    let az = ((i as f32 + 0.5) / w as f32 - 0.5) * 2.0 * pi;
                    let c = env.radiance([el.cos() * az.cos(), el.cos() * az.sin(), el.sin()]);
                    texels.extend_from_slice(&[c[0], c[1], c[2], 1.0]);
                }
            }
            let extent = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("render3d backdrop environment"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                bytemuck::cast_slice(&texels),
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 16), rows_per_image: Some(h) },
                extent,
            );
            self.image = Some(Resident { key: ENV_KEY.to_string(), print, size: (w, h), view: tex.create_view(&Default::default()) });
            self.uploads += 1;
        }
        let Some(res) = &self.image else { return false };
        let scale = if exposure.is_finite() { exposure.max(0.0) } else { 0.0 };
        let u = BackdropUniform {
            inv_view_proj: invert(view_proj),
            top: [0.0; 4],
            bottom: [scale, 0.0, 0.0, 0.0],
            size: [size.0 as f32, size.1 as f32, res.size.0 as f32, res.size.1 as f32],
            kind: [4, tone, 0, 0],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render3d backdrop environment"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&res.view) },
            ],
        }));
        true
    }

    /// Draw the backdrop (call after a `true` [`BackdropPass::prepare`], before the meshes).
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if let Some(bg) = &self.bind_group {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bg, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> BTreeMap<String, ImageRgba> {
        BTreeMap::new()
    }

    fn with(item: &str, img: ImageRgba) -> BTreeMap<String, ImageRgba> {
        BTreeMap::from([(image_key(item), img)])
    }

    #[test]
    fn equirect_picture_horizon_offset() {
        // Bright above the middle row, dark below.
        let mut rgba = Vec::new();
        for y in 0..32u32 {
            for _ in 0..64u32 {
                let v = if y < 16 { 250 } else { 10 };
                rgba.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let img = ImageRgba { width: 64, height: 32, rgba, ..Default::default() };
        let t = BTreeMap::from([("item:sky".to_string(), img)]);
        let at = |horizon: f64| {
            let b = Backdrop::Image { item: "sky".into(), projection: Projection::Equirect, horizon };
            let e = 5f32.to_radians();
            backdrop_rgb(&b, &t, 0, 0, 1, 1, [e.cos(), 0.0, e.sin()])[0]
        };
        assert!(at(0.0) > 200, "5 degrees up is bright at horizon 0");
        assert!(at(10.0) < 60, "raising the horizon by 10 puts 5 degrees up below it");
        assert!(at(-10.0) > 200);
    }

    #[test]
    fn backdrop_rgb_colour_gradient_flat_equirect() {
        let up = [0.0, 0.0, 1.0];
        // Colour.
        let c = Backdrop::Colour { colour: [32, 64, 96] };
        assert_eq!(backdrop_rgb(&c, &none(), 3, 4, 10, 10, up), [32, 64, 96]);
        // Gradient: t = (y + 0.5) / h, rounded per channel.
        let g = Backdrop::Gradient { top: [0, 100, 200], bottom: [100, 100, 0] };
        assert_eq!(backdrop_rgb(&g, &none(), 0, 0, 1, 2, up), [25, 100, 150]);
        assert_eq!(backdrop_rgb(&g, &none(), 0, 1, 1, 2, up), [75, 100, 50]);
        // Flat: a 2x1 black/white image over 4 columns, bilinear and clamped.
        let img = ImageRgba { width: 2, height: 1, rgba: vec![0, 0, 0, 255, 255, 255, 255, 255], ..Default::default() };
        let flat = Backdrop::Image { item: "pic".into(), projection: Projection::Flat, horizon: 0.0 };
        let t = with("pic", img);
        let row: Vec<u8> = (0..4).map(|x| backdrop_rgb(&flat, &t, x, 0, 4, 1, up)[0]).collect();
        assert_eq!(row, vec![0, 64, 191, 255]);
        // Equirect: a 4x2 image whose red is col * 50 + row * 10.
        let mut rgba = Vec::new();
        for row in 0..2u8 {
            for col in 0..4u8 {
                rgba.extend_from_slice(&[col * 50 + row * 10, 0, 0, 255]);
            }
        }
        let eq = Backdrop::Image { item: "sky".into(), projection: Projection::Equirect, horizon: 0.0 };
        let t = with("sky", ImageRgba { width: 4, height: 2, rgba, ..Default::default() });
        let red = |d: [f32; 3]| backdrop_rgb(&eq, &t, 0, 0, 1, 1, d)[0];
        assert_eq!(red([1.0, 0.0, 0.0]), 80);
        assert_eq!(red([-1.0, 0.0, 0.0]), 80, "u wraps across the seam");
        assert_eq!(red([0.0, 1.0, 0.0]), 130);
        assert_eq!(red([0.0, 0.0, 1.0]), 75, "the zenith is the top row");
        // A missing image gives the clear colour.
        assert_eq!(backdrop_rgb(&flat, &none(), 0, 0, 4, 1, up), CLEAR_RGB);
    }

    use crate::camera::{Camera, Orbit, ray_at};
    use crate::light::Light;
    use crate::offscreen::{Offscreen, RenderOptions, Style};
    use crate::types::{ArchId, Mesh, Scene3d};
    use cadcraft_geom::Vec2;

    const SIZE: (u32, u32) = (96, 72);

    fn scene(backdrop: Option<Backdrop>, textures: BTreeMap<String, ImageRgba>) -> Scene3d {
        let mut m = Mesh::new("concrete", ArchId(1));
        m.push_prism(&[Vec2::new(-1.0, -1.0), Vec2::new(1.0, -1.0), Vec2::new(1.0, 1.0), Vec2::new(-1.0, 1.0)], 0.0, 2.0);
        let bounds = m.bounds();
        Scene3d { meshes: vec![m], bounds, backdrop, textures, ..Default::default() }
    }

    fn camera() -> Camera {
        Camera::Orbit(Orbit::at_origin(30.0, 25.0, 9.0, 45.0))
    }

    fn shot(off: &mut Offscreen, s: &Scene3d, style: Style) -> Vec<u8> {
        off.render_with(s, 1, &camera(), &Light::DEFAULT, &RenderOptions { style, ..Default::default() });
        off.read_rgba().unwrap_or_default()
    }

    fn offscreen() -> Option<Offscreen> {
        let Some((device, queue)) = crate::gputest::shared_device() else {
            eprintln!("SKIP: no GPU adapter available");
            return None;
        };
        Some(Offscreen::with_device(device, queue, SIZE))
    }

    fn is_clear(p: &[u8]) -> bool {
        p.iter().take(3).zip(CLEAR_RGB).all(|(a, b)| a.abs_diff(b) <= 1)
    }

    fn ray(x: u32, y: u32) -> [f32; 3] {
        match ray_at(&camera(), SIZE, [x as f32 + 0.5, y as f32 + 0.5]) {
            Some((_, d)) => [d.x as f32, d.y as f32, d.z as f32],
            None => [0.0, 0.0, 1.0],
        }
    }

    /// A smooth 64x32 panorama so that the GPU and CPU rays may differ by float noise only.
    fn sky() -> ImageRgba {
        let mut rgba = Vec::new();
        for y in 0..32u32 {
            for x in 0..64u32 {
                rgba.extend_from_slice(&[(x * 3) as u8, (y * 6) as u8, 120, 255]);
            }
        }
        ImageRgba { width: 64, height: 32, rgba, ..Default::default() }
    }

    #[test]
    fn offscreen_draws_the_backdrop_behind_meshes() {
        let Some(mut off) = offscreen() else { return };
        let plain = shot(&mut off, &scene(None, none()), Style::Shaded);
        let missed: Vec<bool> = plain.chunks_exact(4).map(|p| plain_is_background(p)).collect();
        assert!(missed.iter().any(|m| *m) && missed.iter().any(|m| !*m), "the test scene needs both background and mesh");
        let flat_img =
            ImageRgba { width: 2, height: 2, rgba: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255], ..Default::default() };
        let cases = [
            (Backdrop::Colour { colour: [32, 64, 96] }, none()),
            (Backdrop::Gradient { top: [250, 200, 20], bottom: [10, 20, 240] }, none()),
            (Backdrop::Image { item: "pic".into(), projection: Projection::Flat, horizon: 0.0 }, with("pic", flat_img)),
            (Backdrop::Image { item: "sky".into(), projection: Projection::Equirect, horizon: 0.0 }, with("sky", sky())),
        ];
        for (b, textures) in cases {
            let got = shot(&mut off, &scene(Some(b.clone()), textures.clone()), Style::Shaded);
            assert_eq!(got.len(), plain.len());
            for y in 0..SIZE.1 {
                for x in 0..SIZE.0 {
                    let i = (y * SIZE.0 + x) as usize;
                    let (g, p) = (&got[i * 4..i * 4 + 4], &plain[i * 4..i * 4 + 4]);
                    if missed[i] {
                        let want = backdrop_rgb(&b, &textures, x, y, SIZE.0, SIZE.1, ray(x, y));
                        assert!(g.iter().zip(want).all(|(a, w)| a.abs_diff(w) <= 1), "{b:?} at ({x},{y}): {g:?} vs {want:?}");
                    } else if !near_edge(&missed, x, y) {
                        assert_eq!(g, p, "{b:?}: the mesh pixel ({x},{y}) changed");
                    }
                }
            }
        }
        // Hidden-line keeps the clear colour.
        let hidden_plain = shot(&mut off, &scene(None, none()), Style::HiddenLine);
        let hidden = shot(&mut off, &scene(Some(Backdrop::Colour { colour: [200, 0, 0] }), none()), Style::HiddenLine);
        assert_eq!(hidden, hidden_plain);
    }

    fn plain_is_background(p: &[u8]) -> bool {
        p.iter().take(3).zip(CLEAR_RGB).all(|(a, b)| a == &b)
    }

    /// True when a pixel next to `(x, y)` is background (edge pixels are anti-aliased by nothing, but
    /// stay out of the way of the comparison).
    fn near_edge(missed: &[bool], x: u32, y: u32) -> bool {
        let at = |dx: i64, dy: i64| {
            let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
            nx >= 0 && ny >= 0 && nx < i64::from(SIZE.0) && ny < i64::from(SIZE.1) && missed[(ny as usize) * SIZE.0 as usize + nx as usize]
        };
        [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| at(*dx, *dy))
    }

    #[test]
    fn no_backdrop_renders_as_before() {
        let Some(mut off) = offscreen() else { return };
        let a = shot(&mut off, &scene(None, none()), Style::Shaded);
        let b = shot(&mut off, &scene(None, none()), Style::Shaded);
        assert_eq!(a, b);
        assert!(a.chunks_exact(4).any(is_clear), "the background is the clear colour");
        assert!(off.backdrop.is_none(), "no backdrop pass is built without a backdrop");
    }

    #[test]
    fn a_missing_image_draws_the_clear_colour() {
        let Some(mut off) = offscreen() else { return };
        let plain = shot(&mut off, &scene(None, none()), Style::Shaded);
        for projection in [Projection::Flat, Projection::Equirect] {
            let b = Backdrop::Image { item: "nowhere".into(), projection, horizon: 0.0 };
            assert_eq!(shot(&mut off, &scene(Some(b), none()), Style::Shaded), plain);
        }
        let bad = ImageRgba { width: 4, height: 4, rgba: vec![1, 2, 3], ..Default::default() };
        let b = Backdrop::Image { item: "bad".into(), projection: Projection::Flat, horizon: 0.0 };
        assert_eq!(shot(&mut off, &scene(Some(b), with("bad", bad)), Style::Shaded), plain);
    }

    #[test]
    fn composite_misses_touches_only_missed_pixels() {
        let mut px = vec![9u8; 2 * 4];
        let b = Backdrop::Colour { colour: [1, 2, 3] };
        composite_misses(&mut px, 2, 1, &[false, true], &b, &none(), &|_, _| [0.0, 0.0, 1.0]);
        assert_eq!(px, vec![9, 9, 9, 9, 1, 2, 3, 255]);
        composite_misses(&mut px, 3, 1, &[true; 3], &b, &none(), &|_, _| [0.0, 0.0, 1.0]);
        assert_eq!(px, vec![9, 9, 9, 9, 1, 2, 3, 255], "a wrong-sized buffer is left alone");
    }

    #[test]
    fn sky_and_hdri_are_not_pictures() {
        let hdri = Backdrop::Hdri { image: "pic".into(), intensity: 1.0, rotation: 0.0, horizon: 0.0 };
        let img = ImageRgba { width: 1, height: 1, rgba: vec![1, 2, 3, 255], ..Default::default() };
        let textures = with("pic", img);
        for b in [Backdrop::Sky, hdri] {
            assert!(image_of(&b, &textures).is_none());
            assert!(image_textures(Some(&b), &textures).is_empty());
            // The path tracer's own miss radiance stays: nothing is painted over a miss.
            let mut px = vec![9u8; 2 * 4];
            composite_misses(&mut px, 2, 1, &[true, true], &b, &textures, &|_, _| [0.0, 0.0, 1.0]);
            assert_eq!(px, vec![9u8; 8]);
        }
    }

    #[test]
    fn invert_roundtrips() {
        let m = [[2.0, 0.0, 0.0, 0.0], [0.0, 4.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [1.0, 2.0, 3.0, 1.0]];
        let inv = invert(&m);
        assert!((inv[0][0] - 0.5).abs() < 1e-6 && (inv[1][1] - 0.25).abs() < 1e-6);
        assert!((inv[3][0] + 0.5).abs() < 1e-6);
    }
}
