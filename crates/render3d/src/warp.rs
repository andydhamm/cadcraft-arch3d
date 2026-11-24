//! The lens warp of the raster views (phase 22): Panini and shift as a post-warp.
//!
//! The scene is drawn as usual into a rectilinear source frame that covers every direction the
//! output needs ([`source_extent`]); a fullscreen pass then writes the output, sampling the source
//! where each output pixel's view direction lands. No compute and no storage buffers, so it runs
//! on WebGL2 too.

use cadcraft_arch3d::lens::{LensView, frame_to_dir};

use crate::lens::LensOptions;
use crate::pipeline::COLOR_FORMAT;

const SHADER: &str = include_str!("shaders/warp.wgsl");

/// Bytes of the warp uniform block: three `vec4<f32>`.
const UNIFORM_SIZE: u64 = 48;

/// Largest source height as a multiple of the output height.
const MAX_SOURCE_SCALE: u32 = 4;

/// The lens as `arch3d`'s `f64` math takes it.
fn lens_view(lens: &LensOptions) -> LensView {
    let mut l = LensView::default();
    l.panini = lens.panini;
    l.compression = f64::from(lens.compression);
    l.shift = f64::from(lens.shift);
    l
}

/// The source frame a warped output of `size` needs: `(tan_v of the source, height in pixels)`.
///
/// The source has the output's width and horizontal field (`tan_h`), so a pixel at the centre keeps
/// its size; its height grows until the top and bottom output rows, after undoing shift and Panini,
/// fall inside it. The height is capped at 4x the output; the returned tangent always matches the
/// returned height (`tan_v = tan_h * height / width`). Bad input returns the output's own frame.
pub fn source_extent(lens: &LensOptions, tan_h: f64, tan_v: f64, size: (u32, u32)) -> (f64, u32) {
    let (w, h) = (size.0.max(1), size.1.max(1));
    if !(tan_h.is_finite() && tan_v.is_finite() && tan_h > 0.0 && tan_v > 0.0) {
        return (tan_v, h);
    }
    let l = lens_view(lens);
    let mut need = tan_v;
    const STEPS: u32 = 32;
    for i in 0..=STEPS {
        let t = f64::from(i) / f64::from(STEPS) * 2.0 - 1.0;
        for n in [[t, 1.0], [t, -1.0], [1.0, t], [-1.0, t]] {
            let d = frame_to_dir(&l, tan_h, tan_v, n);
            if d[2] > 1e-6 {
                let r = (d[1] / d[2]).abs();
                if r.is_finite() {
                    need = need.max(r);
                }
            } else {
                // Level with or behind the eye: the source cannot cover it; take the cap.
                need = f64::INFINITY;
            }
        }
    }
    let rows = (f64::from(h) * need / tan_v - 1e-6).ceil();
    let cap = h.saturating_mul(MAX_SOURCE_SCALE);
    let rows = if rows.is_finite() { (rows as u32).clamp(h, cap) } else { cap };
    (tan_h * f64::from(rows) / f64::from(w), rows)
}

/// The warp pipeline, built once per device.
pub struct WarpPass {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl WarpPass {
    /// Build the pipeline (one [`COLOR_FORMAT`] attachment, no depth, one sample).
    pub fn new(device: &wgpu::Device) -> Self {
        let module = device
            .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("render3d warp"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render3d warp bind group layout"),
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
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render3d warp pipeline layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render3d warp pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("render3d warp sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        WarpPass { pipeline, bgl, sampler }
    }

    /// Record the warp of `src` (a rectilinear frame of vertical tangent `tan_v_src`) into `dst`
    /// (`size` pixels, the output camera's tangents `tan_h` and `tan_v`).
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        src: &wgpu::TextureView,
        dst: &wgpu::TextureView,
        lens: &LensOptions,
        tans: (f64, f64, f64),
        size: (u32, u32),
        clear: wgpu::Color,
    ) {
        use wgpu::util::DeviceExt;
        let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
        let words: [f32; 12] = [
            tans.0 as f32,
            tans.1 as f32,
            tans.2 as f32,
            finite(lens.shift),
            if lens.panini { 1.0 } else { 0.0 },
            finite(lens.compression),
            size.0 as f32,
            size.1 as f32,
            clear.r as f32,
            clear.g as f32,
            clear.b as f32,
            clear.a as f32,
        ];
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("render3d warp uniform"),
            contents: &bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render3d warp bind group"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(src) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render3d warp"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_extent_covers_every_output_pixel() {
        let (w, h) = (160u32, 90u32);
        let tan_v = (50.0f64.to_radians() / 2.0).tan();
        let tan_h = tan_v * f64::from(w) / f64::from(h);
        for lens in [
            LensOptions { panini: true, compression: 0.3, ..Default::default() },
            LensOptions { panini: true, compression: 0.7, ..Default::default() },
            LensOptions { panini: true, compression: 1.0, shift: 0.25, ..Default::default() },
            LensOptions { shift: 0.25, ..Default::default() },
            LensOptions { shift: -0.4, ..Default::default() },
        ] {
            let (tv, rows) = source_extent(&lens, tan_h, tan_v, (w, h));
            assert!(rows >= h && rows <= h * MAX_SOURCE_SCALE, "{lens:?}: {rows}");
            assert!((tv - tan_h * f64::from(rows) / f64::from(w)).abs() < 1e-12);
            let l = lens_view(&lens);
            for y in 0..h {
                for x in 0..w {
                    let n = [(f64::from(x) + 0.5) / f64::from(w) * 2.0 - 1.0, 1.0 - (f64::from(y) + 0.5) / f64::from(h) * 2.0];
                    let d = frame_to_dir(&l, tan_h, tan_v, n);
                    assert!(d[2] > 0.0 && (d[1] / d[2]).abs() <= tv + 1e-9, "{lens:?} pixel {x},{y}: {d:?} vs {tv}");
                }
            }
        }
        // A neutral lens needs nothing extra.
        let (tv, rows) = source_extent(&LensOptions::default(), tan_h, tan_v, (w, h));
        assert_eq!(rows, h);
        assert!((tv - tan_v).abs() < 1e-12);
        // Hostile input returns the output's own frame.
        assert_eq!(source_extent(&LensOptions::default(), f64::NAN, tan_v, (w, h)).1, h);
        assert_eq!(source_extent(&LensOptions::default(), tan_h, 0.0, (0, 0)).1, 1);
    }

    /// Large positive and negative shifts still cover every output pixel, even when the
    /// source height hits the 4x cap.
    #[test]
    fn source_extent_with_large_shift() {
        let (w, h) = (160u32, 90u32);
        let tan_v = (50.0f64.to_radians() / 2.0).tan();
        let tan_h = tan_v * f64::from(w) / f64::from(h);
        for shift in [0.9f32, -0.9, 0.49, -0.49] {
            let lens = LensOptions { shift, ..Default::default() };
            let (tv, rows) = source_extent(&lens, tan_h, tan_v, (w, h));
            assert!(rows >= h && rows <= h * MAX_SOURCE_SCALE, "shift {shift}: {rows}");
            let l = lens_view(&lens);
            for y in 0..h {
                for x in 0..w {
                    let n = [(f64::from(x) + 0.5) / f64::from(w) * 2.0 - 1.0, 1.0 - (f64::from(y) + 0.5) / f64::from(h) * 2.0];
                    let d = frame_to_dir(&l, tan_h, tan_v, n);
                    assert!(d[2] > 0.0 && (d[1] / d[2]).abs() <= tv + 1e-9, "shift {shift} pixel {x},{y}: {d:?} vs {tv}");
                }
            }
        }
    }

    /// Panini at the extreme compression 0.0 (full) still covers every output pixel.
    #[test]
    fn source_extent_with_zero_compression() {
        let (w, h) = (160u32, 90u32);
        let tan_v = (50.0f64.to_radians() / 2.0).tan();
        let tan_h = tan_v * f64::from(w) / f64::from(h);
        let lens = LensOptions { panini: true, compression: 0.0, ..Default::default() };
        let (tv, rows) = source_extent(&lens, tan_h, tan_v, (w, h));
        assert!(rows >= h && rows <= h * MAX_SOURCE_SCALE, "{rows}");
        let l = lens_view(&lens);
        for y in 0..h {
            for x in 0..w {
                let n = [(f64::from(x) + 0.5) / f64::from(w) * 2.0 - 1.0, 1.0 - (f64::from(y) + 0.5) / f64::from(h) * 2.0];
                let d = frame_to_dir(&l, tan_h, tan_v, n);
                assert!(d[2] > 0.0 && (d[1] / d[2]).abs() <= tv + 1e-9, "pixel {x},{y}: {d:?} vs {tv}");
            }
        }
    }

    /// NaN in the lens options does not crash and produces a finite, bounded source extent.
    #[test]
    fn source_extent_with_nan_lens_options() {
        let (w, h) = (160u32, 90u32);
        let tan_v = (50.0f64.to_radians() / 2.0).tan();
        let tan_h = tan_v * f64::from(w) / f64::from(h);
        for lens in [
            LensOptions { panini: true, compression: f32::NAN, ..Default::default() },
            LensOptions { shift: f32::NAN, ..Default::default() },
            LensOptions { panini: true, compression: 0.5, shift: f32::NAN, ..Default::default() },
        ] {
            let (tv, rows) = source_extent(&lens, tan_h, tan_v, (w, h));
            assert!(tv.is_finite() && tv > 0.0, "{lens:?}: tan_v {tv}");
            assert!(rows >= h && rows <= h * MAX_SOURCE_SCALE, "{lens:?}: {rows}");
        }
    }

    // ---- GPU tests on the shared test device (skipped without an adapter) ----

    use crate::camera::{Camera, FullCamera};
    use crate::lit_frame::Lighting;
    use crate::offscreen::{CLEAR, Offscreen, RenderOptions};
    use crate::types::{ArchId, Bounds3, Mesh, Scene3d};
    use cadcraft_geom::Vec3;

    fn cuboid(min: [f32; 3], max: [f32; 3]) -> Mesh {
        let mut m = Mesh::new("wall", ArchId(1));
        let [x0, y0, z0] = min;
        let [x1, y1, z1] = max;
        let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
            ([1.0, 0.0, 0.0], [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]]),
            ([-1.0, 0.0, 0.0], [[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]]),
            ([0.0, 1.0, 0.0], [[x1, y1, z0], [x0, y1, z0], [x0, y1, z1], [x1, y1, z1]]),
            ([0.0, -1.0, 0.0], [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]]),
            ([0.0, 0.0, 1.0], [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]]),
            ([0.0, 0.0, -1.0], [[x0, y1, z0], [x1, y1, z0], [x1, y0, z0], [x0, y0, z0]]),
        ];
        for (n, quad) in faces {
            let base = m.positions.len() as u32;
            for p in quad {
                m.positions.push(p);
                m.normals.push(n);
                m.uvs.push([p[0], p[2]]);
            }
            m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        m
    }

    fn sphere(c: [f32; 3], r: f32) -> Mesh {
        let mut m = Mesh::new("wall", ArchId(1));
        let (rings, segs) = (24u32, 48u32);
        for i in 0..=rings {
            let th = std::f32::consts::PI * i as f32 / rings as f32;
            for j in 0..=segs {
                let ph = std::f32::consts::TAU * j as f32 / segs as f32;
                let n = [th.sin() * ph.cos(), th.sin() * ph.sin(), th.cos()];
                m.positions.push([c[0] + r * n[0], c[1] + r * n[1], c[2] + r * n[2]]);
                m.normals.push(n);
                m.uvs.push([0.0, 0.0]);
            }
        }
        for i in 0..rings {
            for j in 0..segs {
                let a = i * (segs + 1) + j;
                let b = a + segs + 1;
                m.indices.extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b]);
            }
        }
        m
    }

    fn scene_of(meshes: Vec<Mesh>) -> Scene3d {
        let mut s = Scene3d { meshes, ..Default::default() };
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for p in s.meshes.iter().flat_map(|m| m.positions.iter()) {
            for k in 0..3 {
                lo[k] = lo[k].min(f64::from(p[k]));
                hi[k] = hi[k].max(f64::from(p[k]));
            }
        }
        s.bounds = Bounds3::new(lo, hi);
        s
    }

    fn renderer(size: (u32, u32)) -> Option<Offscreen> {
        let (device, queue) = crate::gputest::shared_device()?;
        Some(Offscreen::with_device(device, queue, size))
    }

    /// The pixels of `scene` seen from the origin looking along +Y, with `lens`.
    fn shot(o: &mut Offscreen, scene: &Scene3d, fov_deg: f64, lens: LensOptions) -> Option<Vec<u8>> {
        let cam = Camera::Full(FullCamera::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 10.0, 0.0), fov_deg));
        let lighting = Lighting { lens, ..Default::default() };
        let frame = o.lit_frame(scene, 1, [0.0, 0.0, 0.0], &lighting, false)?;
        o.render_with(scene, 1, &cam, &crate::light::Light::DEFAULT, &RenderOptions { lit: Some(&frame), ..Default::default() });
        o.read_rgba().ok()
    }

    /// True where the pixel is not the clear colour.
    fn covered(rgba: &[u8], w: u32, x: u32, y: u32) -> bool {
        let i = ((y * w + x) * 4) as usize;
        let enc = |c: f64| {
            let s = if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
            (s * 255.0).round() as i32
        };
        let clear = [enc(CLEAR.r), enc(CLEAR.g), enc(CLEAR.b)];
        (0..3).any(|k| (i32::from(rgba[i + k]) - clear[k]).abs() > 6)
    }

    /// Bounding box `(x0, y0, x1, y1)` of the covered pixels.
    fn bbox(rgba: &[u8], (w, h): (u32, u32)) -> Option<(u32, u32, u32, u32)> {
        let mut b: Option<(u32, u32, u32, u32)> = None;
        for y in 0..h {
            for x in 0..w {
                if covered(rgba, w, x, y) {
                    b = Some(match b {
                        None => (x, y, x, y),
                        Some((a, c, d, e)) => (a.min(x), c.min(y), d.max(x), e.max(y)),
                    });
                }
            }
        }
        b
    }

    #[test]
    fn neutral_lens_leaves_the_raster_image_identical() {
        let size = (96, 64);
        let Some(mut o) = renderer(size) else { return };
        let scene = scene_of(vec![cuboid([2.0, 8.0, -3.0], [3.0, 9.0, 3.0]), sphere([-2.0, 9.0, 0.0], 1.0)]);
        let Some(plain) = shot(&mut o, &scene, 50.0, LensOptions::default()) else { return };
        assert!(o.warp_src.is_none(), "a neutral lens makes no source target");
        // Compression and focus alone change nothing.
        let Some(same) = shot(&mut o, &scene, 50.0, LensOptions { compression: 0.2, focus: Some(4.0), ..Default::default() }) else { return };
        assert!(o.warp_src.is_none());
        assert_eq!(plain, same);
        // A warping lens does make one, and changes the image.
        let Some(warped) = shot(&mut o, &scene, 50.0, LensOptions { shift: 0.2, ..Default::default() }) else { return };
        assert!(o.warp_src.is_some());
        assert_ne!(plain, warped);
    }

    #[test]
    fn panini_keeps_verticals_in_the_raster() {
        let size = (200u32, 120u32);
        let Some(mut o) = renderer(size) else { return };
        // A tall bar well off the axis, facing the camera.
        let scene = scene_of(vec![cuboid([3.9, 10.0, -40.0], [4.1, 10.2, 40.0])]);
        for d in [0.3f32, 0.7, 1.0] {
            let lens = LensOptions { panini: true, compression: d, ..Default::default() };
            let Some(img) = shot(&mut o, &scene, 50.0, lens) else { return };
            let mut cols = Vec::new();
            for y in 8..size.1 - 8 {
                let xs: Vec<u32> = (0..size.0).filter(|&x| covered(&img, size.0, x, y)).collect();
                assert!(!xs.is_empty(), "d={d}: row {y} has no bar");
                cols.push(xs[0] + xs[xs.len() - 1]);
            }
            // The bar's centre column (doubled) in every row.
            let lo = cols.iter().copied().min().unwrap_or(0) / 2;
            let hi = cols.iter().copied().max().unwrap_or(0) / 2;
            assert!(hi - lo <= 2, "d={d}: the bar wanders from column {lo} to {hi}");
            assert!(lo > size.0 / 2 + 20, "d={d}: the bar should be off axis, at {lo}");
        }
    }

    /// Width / height of the covered area of a small sphere 47.5 degrees off axis at a 100 degree
    /// horizontal view.
    fn sphere_aspect(o: &mut Offscreen, size: (u32, u32), lens: LensOptions) -> Option<f64> {
        let tan_h = 50.0f64.to_radians().tan();
        let fov = 2.0 * (tan_h * f64::from(size.1) / f64::from(size.0)).atan();
        let (az, dist) = (47.5f64.to_radians(), 20.0f64);
        let scene = scene_of(vec![sphere([(dist * az.sin()) as f32, (dist * az.cos()) as f32, 0.0], 0.6)]);
        let img = shot(o, &scene, fov.to_degrees(), lens)?;
        let (x0, y0, x1, y1) = bbox(&img, size)?;
        Some(f64::from(x1 - x0 + 1) / f64::from(y1 - y0 + 1))
    }

    #[test]
    fn panini_rounds_the_off_axis_sphere() {
        let size = (800u32, 450u32);
        let Some(mut o) = renderer(size) else { return };
        let Some(rect) = sphere_aspect(&mut o, size, LensOptions::default()) else { return };
        assert!((1.35..1.62).contains(&rect), "rectilinear aspect {rect}");
        let Some(pan) = sphere_aspect(&mut o, size, LensOptions { panini: true, compression: 1.0, ..Default::default() }) else { return };
        assert!(pan < 1.15, "panini aspect {pan}");
    }

    #[test]
    fn shift_moves_the_raster_horizon() {
        let size = (160u32, 120u32);
        let Some(mut o) = renderer(size) else { return };
        // A thin horizontal bar at eye height.
        let scene = scene_of(vec![cuboid([-60.0, 10.0, -0.1], [60.0, 10.2, 0.1])]);
        let centre = |img: &[u8]| {
            let rows: Vec<u32> = (0..size.1).filter(|&y| (0..size.0).any(|x| covered(img, size.0, x, y))).collect();
            let (a, b) = (rows.first().copied()?, rows.last().copied()?);
            Some(f64::from(a + b) / 2.0 + 0.5)
        };
        let Some(base) = shot(&mut o, &scene, 50.0, LensOptions::default()).and_then(|i| centre(&i)) else { return };
        assert!((base - 60.0).abs() < 2.0, "the horizon starts at the middle: {base}");
        let Some(down) = shot(&mut o, &scene, 50.0, LensOptions { shift: 0.25, ..Default::default() }).and_then(|i| centre(&i)) else { return };
        assert!((down - base - 30.0).abs() < 3.0, "a shift of 0.25 moves the horizon a quarter of 120 rows: {base} -> {down}");
    }

    /// An empty scene with a warping lens produces the clear colour everywhere (no crash, no
    /// artefacts from sampling an empty source).
    #[test]
    fn warp_empty_scene_is_clear_colour() {
        let size = (96u32, 64u32);
        let Some(mut o) = renderer(size) else { return };
        let scene = Scene3d::default();
        let lens = LensOptions { panini: true, compression: 0.7, shift: 0.2, ..Default::default() };
        let Some(img) = shot(&mut o, &scene, 50.0, lens) else { return };
        assert_eq!(img.len(), (size.0 * size.1 * 4) as usize);
        // Every pixel is the clear colour.
        let enc = |c: f64| {
            let s = if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
            (s * 255.0).round() as i32
        };
        let clear = [enc(CLEAR.r), enc(CLEAR.g), enc(CLEAR.b)];
        for px in img.chunks_exact(4) {
            assert!((0..3).all(|k| (i32::from(px[k]) - clear[k]).abs() <= 2), "pixel {:?} is not the clear colour {:?}", &px[..3], clear);
        }
    }

    /// Panini at the extreme compression 0.0 (full) keeps verticals in one column, just as
    /// the intermediate compressions do.
    #[test]
    fn panini_zero_compression_keeps_verticals_in_the_raster() {
        let size = (200u32, 120u32);
        let Some(mut o) = renderer(size) else { return };
        let scene = scene_of(vec![cuboid([3.9, 10.0, -40.0], [4.1, 10.2, 40.0])]);
        let lens = LensOptions { panini: true, compression: 0.0, ..Default::default() };
        let Some(img) = shot(&mut o, &scene, 50.0, lens) else { return };
        let mut cols = Vec::new();
        for y in 8..size.1 - 8 {
            let xs: Vec<u32> = (0..size.0).filter(|&x| covered(&img, size.0, x, y)).collect();
            assert!(!xs.is_empty(), "row {y} has no bar");
            cols.push(xs[0] + xs[xs.len() - 1]);
        }
        let lo = cols.iter().copied().min().unwrap_or(0) / 2;
        let hi = cols.iter().copied().max().unwrap_or(0) / 2;
        assert!(hi - lo <= 2, "the bar wanders from column {lo} to {hi}");
        assert!(lo > size.0 / 2 + 20, "the bar should be off axis, at {lo}");
    }
}
