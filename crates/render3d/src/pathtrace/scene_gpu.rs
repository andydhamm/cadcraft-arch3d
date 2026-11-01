//! The scene as the path tracer's GPU buffers (phase 11): a flattened BVH over every triangle
//! (fixture housings included, so lenses can be seen), one material per mesh, the lights and the
//! IES tables. The structs are the byte layouts of `shaders/pt/common.wgsl` (std430, 16-byte
//! aligned, sizes checked by the tests); change both together.
//!
//! The BVH: nodes in depth-first order. An inner node (`count == 0`) has its left child right
//! after it (`index + 1`) and its right child at `left_or_first`; a leaf (`count > 0`) covers
//! `tris[left_or_first .. left_or_first + count]`. Leaves hold at most [`MAX_LEAF_TRIS`]
//! triangles; the tree is a binned SAH split ([`SAH_BINS`] bins along the widest centroid axis).
//!
//! Filled here: the layouts and [`luminance_rgb`]. Stubs filled by `p11-r3d-gpu-scene`:
//! [`build_gpu_scene`], [`material_for`], [`GpuScene::trace_cpu`].

use crate::types::Scene3d;
use bytemuck::{Pod, Zeroable};

/// Most triangles a leaf holds, and the bins of the SAH split.
pub const MAX_LEAF_TRIS: u32 = 4;
pub const SAH_BINS: usize = 16;
/// Most triangles the path tracer takes (a bigger scene is refused, not truncated).
pub const MAX_GPU_TRIANGLES: usize = 8_000_000;

/// `GpuMaterial::flags`.
pub const MAT_GLASS: u32 = 1;
/// Emits only toward the camera (a lit fixture lens: its light is the fixture's, not a second one).
pub const MAT_EMIT_CAMERA_ONLY: u32 = 2;
/// A fixture housing: never in a shadow ray's way (as in the raster maps and the CPU illuminance).
pub const MAT_NO_SHADOW: u32 = 4;

/// `GpuLight::kind`.
pub const LIGHT_POINT: u32 = 0;
pub const LIGHT_SPOT: u32 = 1;
pub const LIGHT_AREA: u32 = 2;

/// `GpuFrame::flags`.
pub const FRAME_SUN: u32 = 1;
pub const FRAME_SKY: u32 = 2;
pub const FRAME_FURNACE: u32 = 4;
pub const FRAME_GROUND: u32 = 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct GpuNode {
    pub min: [f32; 3],
    pub left_or_first: u32,
    pub max: [f32; 3],
    pub count: u32,
}

/// A triangle as `v0` and the edges `e1 = v1 − v0`, `e2 = v2 − v0`, with its geometric normal
/// `n` (unit, `e1 × e2`), its mesh's material and the mesh's object id.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct GpuTri {
    pub v0: [f32; 3],
    pub material: u32,
    pub e1: [f32; 3],
    pub object: u32,
    pub e2: [f32; 3],
    pub flags: u32,
    pub n: [f32; 3],
    pub _pad: u32,
}

/// Linear albedo, metalness, roughness (perceptual, `α = roughness²`), emission (nits, RGB),
/// index of refraction and transmission (glass), [`MAT_GLASS`] / [`MAT_EMIT_CAMERA_ONLY`] /
/// [`MAT_NO_SHADOW`] flags.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct GpuMaterial {
    pub albedo: [f32; 3],
    pub metallic: f32,
    pub emission: [f32; 3],
    pub roughness: f32,
    pub ior: f32,
    pub transmission: f32,
    pub flags: u32,
    pub _pad: u32,
}

/// A light: `rgb` is intensity (cd) × colour / luminance(colour) (luminance = candela); an area
/// light's `rgb` is its radiance (nits) × colour / luminance(colour), `axis` its width direction;
/// `ies_offset`, `ies_v`, `ies_h` locate its IES table in the `ies` buffer (`ies_v == 0`: none):
/// `ies_v` vertical angles, then `ies_h` horizontal angles, then `ies_h × ies_v` candela,
/// horizontal-major, already scaled to the fixture.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct GpuLight {
    pub position: [f32; 3],
    pub kind: u32,
    pub direction: [f32; 3],
    pub cos_inner: f32,
    pub rgb: [f32; 3],
    pub cos_outer: f32,
    pub axis: [f32; 3],
    pub width: f32,
    pub length: f32,
    pub ies_offset: u32,
    pub ies_v: u32,
    pub ies_h: u32,
}

/// One frame's uniform (`frame` in the WGSL).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct GpuFrame {
    /// Clip → world, column-major (pixel centres map through NDC).
    pub inv_view_proj: [[f32; 4]; 4],
    pub eye: [f32; 4],
    pub size: [u32; 2],
    /// Index of the first sample this pass adds (the sample count so far).
    pub sample_base: u32,
    pub seed: u32,
    pub bounces: u32,
    pub light_count: u32,
    /// [`FRAME_SUN`] | [`FRAME_SKY`] | [`FRAME_FURNACE`] | [`FRAME_GROUND`].
    pub flags: u32,
    /// Samples per pixel this pass adds.
    pub spp_this_pass: u32,
    /// Toward the sun (unit, w unused), and its illuminance as RGB lux (w unused).
    pub sun_dir: [f32; 4],
    pub sun_rgb: [f32; 4],
    /// The sky's radiance overhead and at the horizon (nits, RGB).
    pub sky_zenith: [f32; 4],
    pub sky_horizon: [f32; 4],
    /// Furnace: albedo, emission, environment radiance, unused.
    pub furnace: [f32; 4],
    pub probe_count: u32,
    pub _pad: [u32; 3],
}

/// One irradiance probe: a point and the unit normal of the surface it lies on.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct GpuProbe {
    pub point: [f32; 3],
    pub _a: f32,
    pub normal: [f32; 3],
    pub _b: f32,
}

/// The scene as buffers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuScene {
    pub nodes: Vec<GpuNode>,
    pub tris: Vec<GpuTri>,
    pub materials: Vec<GpuMaterial>,
    pub lights: Vec<GpuLight>,
    pub ies: Vec<f32>,
}

/// `rgb / luminance(rgb)`, or white for a black or non-finite colour.
pub fn luminance_rgb(rgb: [f64; 3]) -> [f32; 3] {
    let y = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    if !y.is_finite() || y <= 1e-9 {
        return [1.0; 3];
    }
    [(rgb[0] / y) as f32, (rgb[1] / y) as f32, (rgb[2] / y) as f32]
}

/// The material of an arch3d mesh material name: albedo, metalness and roughness from
/// `pipeline::material_pbr`; glass (`alpha < 1`) gets [`MAT_GLASS`], IOR 1.5 and transmission
/// 1 − alpha; `light-lens-lit` emits [`LENS_NITS`] in its fixture's white with
/// [`MAT_EMIT_CAMERA_ONLY`]; every `light-…` material gets [`MAT_NO_SHADOW`]; `mirror…` is
/// metallic 1, roughness 0.02, albedo 0.95. Everything else IOR 1.5, no emission.
pub fn material_for(material: &str) -> GpuMaterial {
    let _ = material;
    GpuMaterial { albedo: [0.75; 3], roughness: 0.5, ior: 1.5, ..Default::default() }
}

/// Radiance of a lit lens (nits), seen directly.
pub const LENS_NITS: f32 = 20_000.0;

/// Build the buffers for `scene` (its meshes, lights and `ground`, a 200 m square at z = 0 with
/// the `ground` material when asked). Lights: every `scene.lights` entry ([`GpuLight`] as above,
/// IES tables packed in light order). `furnace`: every material diffuse with its albedo and
/// emission, no lights. Triangles with a non-finite corner or a zero area are skipped; more than
/// [`MAX_GPU_TRIANGLES`] gives an empty scene.
pub fn build_gpu_scene(scene: &Scene3d, ground: bool, furnace: Option<super::Furnace>) -> GpuScene {
    let _ = (scene, ground, furnace);
    GpuScene::default()
}

/// A CPU hit: the distance along the (unit) ray and the triangle's index in `tris`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CpuHit {
    pub t: f32,
    pub tri: u32,
}

impl GpuScene {
    /// The nearest triangle the ray `origin + t·dir` (unit `dir`) hits with `1e-4 < t < t_max`,
    /// walking the BVH exactly as `pt/traverse.wgsl` does (the reference its GPU test compares
    /// with). Ties break by the lower triangle index. `None` for a miss or a hostile ray.
    pub fn trace_cpu(&self, origin: [f32; 3], dir: [f32; 3], t_max: f32) -> Option<CpuHit> {
        let _ = (origin, dir, t_max);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_match_the_wgsl() {
        assert_eq!(std::mem::size_of::<GpuNode>(), 32);
        assert_eq!(std::mem::size_of::<GpuTri>(), 64);
        assert_eq!(std::mem::size_of::<GpuMaterial>(), 48);
        assert_eq!(std::mem::size_of::<GpuLight>(), 80);
        assert_eq!(std::mem::size_of::<GpuFrame>(), 208);
        assert_eq!(std::mem::size_of::<GpuProbe>(), 32);
    }

    #[test]
    fn luminance_rgb_keeps_luminance_one() {
        let c = luminance_rgb([1.0, 0.8, 0.6]);
        let y = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        assert!((y - 1.0).abs() < 1e-5);
        assert_eq!(luminance_rgb([0.0; 3]), [1.0; 3]);
    }
}
