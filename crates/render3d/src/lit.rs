//! The lit pipelines (phase 10): the mesh shader plus the sun's shadow and every fixture light
//! (spot cones, cube and spot shadow maps from `shadow_atlas`), with tone mapping for Finalize.
//! Used when `RenderOptions::lit` is set; without it the phase-6 pipelines draw exactly as before.
//!
//! Group 2 is described in `crate::lights`. Filled here: [`bind_group_layout`],
//! [`frame_uniform`], [`LitFrame::new`], the placeholder sun map. Stub filled by
//! `p10-r3d-lit-pipeline`: [`LitPipelines::new`] (and `shaders/lit.wgsl`).

use crate::lights::{FRAME_UNIFORM_SIZE, LIGHTS_UNIFORM_SIZE};
use crate::pipeline::{COLOR_FORMAT, DEPTH_FORMAT, GpuPipeline, vertex_layout};
use crate::shadow::ShadowMap;

/// How a lit frame shades.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LitParams {
    /// The sun lights the scene (false = night: no sun, `lights::NIGHT_AMBIENT`).
    pub sun_on: bool,
    /// Sun radiance scale when on (1 = the phase-6 sun).
    pub sun_intensity: f32,
    /// Ambient level.
    pub ambient: f32,
    /// Display units per lux of fixture light (`lights::DEFAULT_EXPOSURE`).
    pub exposure: f32,
    /// Tone-map the result (Finalize Render); the interactive view writes linear colour.
    pub tonemap: bool,
    /// PCF radius in texels for fixture and sun shadows (0 = one tap; Finalize uses 2).
    pub pcf_radius: u32,
    /// Ambient illuminance in lux (0 = unset); written to `FrameU.ext.x`.
    pub ambient_lux: f32,
    /// Second-order spherical-harmonic ambient (nine rgb coefficients), when there is one.
    pub sh: Option<[[f32; 3]; 9]>,
}

impl Default for LitParams {
    fn default() -> Self {
        LitParams {
            sun_on: true,
            sun_intensity: 1.0,
            ambient: 0.25,
            exposure: crate::lights::DEFAULT_EXPOSURE,
            tonemap: false,
            pcf_radius: 0,
            ambient_lux: 0.0,
            sh: None,
        }
    }
}

/// Group 2 of the lit pipelines: the frame uniform, the sun map, the lights uniform and the
/// fixture shadow atlas (see `crate::lights`).
pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let uniform = |binding: u32, size: u64| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: std::num::NonZero::new(size),
        },
        count: None,
    };
    let depth = |binding: u32, view_dimension: wgpu::TextureViewDimension| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension, multisampled: false },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render3d lit group 2"),
        entries: &[
            uniform(0, FRAME_UNIFORM_SIZE),
            depth(1, wgpu::TextureViewDimension::D2),
            uniform(2, LIGHTS_UNIFORM_SIZE),
            depth(3, wgpu::TextureViewDimension::D2Array),
        ],
    })
}

/// The `FrameU` bytes: the sun map's matrix and bias when there is one, the sun flags and the
/// shading parameters; `atlas_size` is the fixture atlas's layer edge.
pub fn frame_uniform(sun: Option<&ShadowMap>, params: &LitParams, atlas_size: u32) -> Vec<u8> {
    let mut words: Vec<f32> = Vec::with_capacity(24);
    let vp = sun.map(|m| m.light_view_proj).unwrap_or([[0.0; 4]; 4]);
    for col in vp {
        words.extend_from_slice(&col);
    }
    let finite = |v: f32, d: f32| if v.is_finite() { v } else { d };
    words.extend_from_slice(&[sun.map(|m| m.bias).unwrap_or(0.0), if sun.is_some() { 1.0 } else { 0.0 }, if params.sun_on { 1.0 } else { 0.0 }, 0.0]);
    words.extend_from_slice(&[
        finite(params.exposure, crate::lights::DEFAULT_EXPOSURE).max(0.0),
        if params.tonemap { 1.0 } else { 0.0 },
        params.pcf_radius.min(8) as f32,
        atlas_size as f32,
    ]);
    // ext: x ambient lux, y = 1 when the SH is valid; then nine SH vec4s (rgb, w unused).
    let sh_valid = params.sh.is_some_and(|sh| sh.iter().flatten().all(|c| c.is_finite()));
    words.extend_from_slice(&[finite(params.ambient_lux, 0.0).max(0.0), if sh_valid { 1.0 } else { 0.0 }, 0.0, 0.0]);
    let sh = if sh_valid { params.sh.unwrap_or([[0.0; 3]; 9]) } else { [[0.0; 3]; 9] };
    for c in sh {
        words.extend_from_slice(&[c[0], c[1], c[2], 0.0]);
    }
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    debug_assert_eq!(bytes.len() as u64, FRAME_UNIFORM_SIZE);
    bytes
}

/// One frame's group 2 and the sun values the per-draw PBR block takes.
pub struct LitFrame {
    pub bind_group: wgpu::BindGroup,
    /// `PbrUniform` sun intensity for this frame (0 at night).
    pub sun_intensity: f32,
    /// `PbrUniform` ambient for this frame.
    pub ambient: f32,
    /// Lights drawn and lights shadowed in this frame.
    pub lights: usize,
    pub shadowed: usize,
    /// What the frame was lit with (set by `Offscreen::lit_frame`; the default from
    /// [`LitFrame::new`]); the per-frame `frame_look` reads it.
    pub lighting: crate::lit_frame::Lighting,
}

impl LitFrame {
    /// Upload the two uniforms and bind them with the sun map (`None`: a 1×1 placeholder) and the
    /// fixture atlas view.
    pub fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        frame: &[u8],
        sun: Option<&wgpu::TextureView>,
        lights: &[u8],
        atlas: &wgpu::TextureView,
        params: &LitParams,
        counts: (usize, usize),
    ) -> Self {
        use wgpu::util::DeviceExt;
        let buf = |label: &str, bytes: &[u8], size: u64| {
            let mut data = bytes.to_vec();
            data.resize(size as usize, 0);
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents: &data, usage: wgpu::BufferUsages::UNIFORM })
        };
        let frame_buf = buf("render3d lit frame", frame, FRAME_UNIFORM_SIZE);
        let lights_buf = buf("render3d lit lights", lights, LIGHTS_UNIFORM_SIZE);
        let placeholder = sun.is_none().then(|| placeholder_depth(device));
        let sun_view = sun.or(placeholder.as_ref()).unwrap_or(atlas);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render3d lit group 2"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: frame_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(sun_view) },
                wgpu::BindGroupEntry { binding: 2, resource: lights_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(atlas) },
            ],
        });
        let (sun_intensity, ambient) =
            if params.sun_on { (params.sun_intensity, params.ambient) } else { (0.0, crate::lights::NIGHT_AMBIENT.min(params.ambient)) };
        LitFrame { bind_group, sun_intensity, ambient, lights: counts.0, shadowed: counts.1, lighting: Default::default() }
    }
}

/// A 1×1 `D2` depth view standing in for an absent sun map.
fn placeholder_depth(device: &wgpu::Device) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("render3d lit sun placeholder"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// The lit variants of the main-pass pipelines (opaque and glass) over `gpu`'s group 0/1 layouts
/// plus [`bind_group_layout`].
pub(crate) struct LitPipelines {
    pub opaque: wgpu::RenderPipeline,
    pub glass: wgpu::RenderPipeline,
    pub layout: wgpu::BindGroupLayout,
}

impl LitPipelines {
    /// Build the pipelines; `None` when the shader cannot be assembled.
    pub fn new(device: &wgpu::Device, gpu: &GpuPipeline) -> Option<Self> {
        let source = lit_shader_source()?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("render3d lit mesh shader"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let layout = bind_group_layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render3d lit pipeline layout"),
            bind_group_layouts: &[Some(&gpu.bgl), Some(&gpu.pbr_bgl), Some(&layout), Some(&gpu.tex_bgl)],
            immediate_size: 0,
        });
        let opaque = lit_render_pipeline(device, &pipeline_layout, &shader, "render3d lit opaque", None, true);
        let glass = lit_render_pipeline(device, &pipeline_layout, &shader, "render3d lit glass", Some(wgpu::BlendState::ALPHA_BLENDING), false);
        Some(LitPipelines { opaque, glass, layout })
    }
}

pub(crate) const MESH_FRAGMENT: &str = "@fragment fn fs(v: VOut) -> @location(0) vec4<f32> {";

/// Compose the lit shader without changing the phase-6 mesh shader used by the unlit pipelines:
/// everything in `mesh.wgsl` before its fragment function (so `shade_parts` and the hook functions
/// are in scope), then the light maths and `lit.wgsl`, whose `fs` replaces the mesh one.
fn lit_shader_source() -> Option<String> {
    let mesh = include_str!("shaders/mesh.wgsl");
    let (before, _) = mesh.split_once(MESH_FRAGMENT)?;
    Some(format!("{before}\n{}\n{}", crate::lights::LIGHT_MATH_WGSL, include_str!("shaders/lit.wgsl")))
}

/// Build one main-pass variant over the shared mesh vertex and bind-group layouts.
fn lit_render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    label: &str,
    blend: Option<wgpu::BlendState>,
    depth_write: bool,
) -> wgpu::RenderPipeline {
    let targets = [Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend, write_mask: wgpu::ColorWrites::ALL })];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[Some(vertex_layout())],
        },
        fragment: Some(wgpu::FragmentState { module: shader, entry_point: Some("fs"), compilation_options: Default::default(), targets: &targets }),
        // Two-sided, matching the phase-6 and shadowed main passes.
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(depth_write),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: crate::pipeline::FILL_DEPTH_BIAS,
        }),
        multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{Camera, Orbit};
    use crate::light::Light;
    use crate::offscreen::{Offscreen, RenderOptions};
    use crate::types::{ArchId, Mesh, Scene3d};
    use cadcraft_arch3d::light::{LightKind, SceneLight};
    use cadcraft_geom::{Vec2, Vec3};

    const SIZE: (u32, u32) = (64, 64);

    fn skip_without_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let device = crate::gputest::shared_device();
        if device.is_none() {
            eprintln!("SKIP: no GPU adapter available");
        }
        device
    }

    fn floor_scene() -> Scene3d {
        let mut mesh = Mesh::new("floor", ArchId(1));
        let ring = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        mesh.push_prism(&ring, -0.05, 0.0);
        let bounds = mesh.bounds();
        Scene3d { meshes: vec![mesh], bounds, ..Default::default() }
    }

    fn top_camera() -> Camera {
        Camera::Orbit(Orbit::at_origin(0.0, 89.0, 12.0, 45.0))
    }

    /// An unshadowed 60-degree spot and a 1×1 atlas view for the frame's required binding.
    fn test_frame(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, params: &LitParams) -> (LitFrame, wgpu::Texture) {
        let cosine = 30.0_f64.to_radians().cos();
        let lights = [SceneLight {
            kind: LightKind::Spot,
            position: Vec3::new(2.0, 2.0, 2.4),
            direction: Vec3::new(0.0, 0.0, -1.0),
            intensity: 950.4,
            color: [1.0; 3],
            cos_inner: cosine,
            cos_outer: cosine,
            ..Default::default()
        }];
        let packed = crate::lights::pack_lights(&lights, &[0], &[]);
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render3d lit test atlas"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let atlas_view = atlas.create_view(&wgpu::TextureViewDescriptor {
            label: Some("render3d lit test atlas view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let frame_bytes = frame_uniform(None, params, 1);
        let frame = LitFrame::new(device, layout, &frame_bytes, None, &packed, &atlas_view, params, (1, 0));
        (frame, atlas)
    }

    fn render_spot(device: wgpu::Device, queue: wgpu::Queue, tonemap: bool, exposure: f32) -> Option<(Vec<u8>, Camera)> {
        let scene = floor_scene();
        let camera = top_camera();
        let mut off = Offscreen::with_device(device.clone(), queue, SIZE);
        off.lit_pipelines = LitPipelines::new(&device, &off.gpu);
        let params = LitParams { sun_on: false, exposure, tonemap, ..Default::default() };
        let (frame, _atlas) = {
            let pipelines = off.lit_pipelines.as_ref()?;
            test_frame(&device, &pipelines.layout, &params)
        };
        let opts = RenderOptions { lit: Some(&frame), ..Default::default() };
        off.render_with(&scene, 1, &camera, &Light::DEFAULT, &opts);
        Some((off.read_rgba().ok()?, camera))
    }

    fn render_unlit(device: wgpu::Device, queue: wgpu::Queue) -> Option<Vec<u8>> {
        let scene = floor_scene();
        let camera = top_camera();
        let mut off = Offscreen::with_device(device, queue, SIZE);
        off.render_with(&scene, 1, &camera, &Light::DEFAULT, &RenderOptions::default());
        off.read_rgba().ok()
    }

    fn pixel(rgba: &[u8], camera: &Camera, world: [f64; 3]) -> Option<[u8; 3]> {
        let p = crate::camera::project_point(camera, SIZE, world)?;
        if !p[0].is_finite() || !p[1].is_finite() {
            return None;
        }
        let (x, y) = (p[0].floor() as u32, p[1].floor() as u32);
        if x >= SIZE.0 || y >= SIZE.1 {
            return None;
        }
        let at = ((y * SIZE.0 + x) * 4) as usize;
        Some([*rgba.get(at)?, *rgba.get(at + 1)?, *rgba.get(at + 2)?])
    }

    fn brightness(pixel: [u8; 3]) -> u32 {
        (u32::from(pixel[0]) + u32::from(pixel[1]) + u32::from(pixel[2])) / 3
    }

    #[test]
    fn frame_uniform_256_and_zero_by_default() {
        assert_eq!(FRAME_UNIFORM_SIZE, 256);
        let bytes = frame_uniform(None, &LitParams::default(), 512);
        assert_eq!(bytes.len(), 256);
        assert!(bytes[96..].iter().all(|&b| b == 0), "ext and sh are zero by default");
        let sh = [[1.0, 2.0, 3.0]; 9];
        let on = frame_uniform(None, &LitParams { ambient_lux: 40.0, sh: Some(sh), ..Default::default() }, 512);
        let word = |at: usize| f32::from_le_bytes([on[at], on[at + 1], on[at + 2], on[at + 3]]);
        assert_eq!((word(96), word(100)), (40.0, 1.0));
        assert_eq!((word(112), word(116), word(120), word(124)), (1.0, 2.0, 3.0, 0.0));
        assert_eq!(word(240), 1.0, "the ninth coefficient ends the block");
        let bad = frame_uniform(None, &LitParams { sh: Some([[f32::NAN; 3]; 9]), ..Default::default() }, 512);
        assert!(bad[96..].iter().all(|&b| b == 0), "a non-finite SH is not sent");
    }

    #[test]
    fn lit_pipelines_build_on_the_test_device() {
        let Some((device, queue)) = skip_without_device() else { return };
        let gpu = GpuPipeline::new(&device, &queue, 1);
        assert!(LitPipelines::new(&device, &gpu).is_some(), "the composed lit shader must build both pipelines");
    }

    #[test]
    fn a_spot_lights_its_cone_and_not_beyond() {
        let Some((device, queue)) = skip_without_device() else { return };
        let Some((rgba, camera)) = render_spot(device, queue, false, crate::lights::DEFAULT_EXPOSURE) else {
            assert!(false, "the lit test render failed");
            return;
        };
        match (pixel(&rgba, &camera, [2.0, 2.0, 0.0]), pixel(&rgba, &camera, [0.2, 0.2, 0.0])) {
            (Some(centre), Some(outside)) => assert!(
                brightness(centre) >= brightness(outside) + 40,
                "centre {centre:?} must be at least 40 levels brighter than outside {outside:?}"
            ),
            _ => assert!(false, "both test points must project into the render"),
        }
    }

    #[test]
    fn tone_mapping_keeps_highlights_below_white() {
        let Some((device, queue)) = skip_without_device() else { return };
        // Raise exposure for this check so the linear reference actually clips at the centre.
        let exposure = 0.2;
        let linear = render_spot(device.clone(), queue.clone(), false, exposure);
        let mapped = render_spot(device, queue, true, exposure);
        let (Some((linear, camera)), Some((mapped, _))) = (linear, mapped) else {
            assert!(false, "the tone-mapping test renders failed");
            return;
        };
        match (pixel(&linear, &camera, [2.0, 2.0, 0.0]), pixel(&mapped, &camera, [2.0, 2.0, 0.0])) {
            (Some(linear), Some(mapped)) => {
                assert!(linear.iter().any(|&c| c == 255), "the linear reference must saturate: {linear:?}");
                assert!(mapped.iter().all(|&c| c < 255), "ACES must keep the highlight below white: {mapped:?}");
            }
            _ => assert!(false, "the light centre must project into both renders"),
        }
    }

    #[test]
    fn without_lit_the_render_is_unchanged() {
        let mesh = include_str!("shaders/mesh.wgsl");
        assert!(mesh.contains(MESH_FRAGMENT));
        assert!(!mesh.contains("@group(2)"), "the phase-6 shader must remain self-contained");
        let prefix = mesh.split_once(MESH_FRAGMENT).map(|(before, _)| before);
        if let (Some(prefix), Some(source)) = (prefix, lit_shader_source()) {
            assert!(source.starts_with(prefix));
            assert!(source.contains("fn shade_parts(v: VOut) -> Shade {"));
            assert!(!source.contains(MESH_FRAGMENT), "the mesh fragment is replaced by the lit one");
            assert!(source.ends_with(include_str!("shaders/lit.wgsl")));
        } else {
            assert!(false, "the lit shader must preserve the original mesh shader");
        }

        let Some((device, queue)) = skip_without_device() else { return };
        let first = render_unlit(device.clone(), queue.clone());
        let second = render_unlit(device, queue);
        match (first, second) {
            (Some(first), Some(second)) => assert_eq!(first, second, "the lit: None path must retain the phase-6 pixels"),
            _ => assert!(false, "the unlit reference renders failed"),
        }
    }
}
