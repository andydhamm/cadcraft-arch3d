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
//! Here: the layouts, [`luminance_rgb`], [`material_for`] and [`material_for_scene`] (phase 14: a
//! mesh's `MaterialDef`), [`build_gpu_scene`] (the triangles in
//! scene order, one material per mesh, the lights and their IES tables, the SAH tree) and
//! [`GpuScene::trace_cpu`], the CPU traversal the WGSL must agree with.

use super::Furnace;
use crate::pipeline::material_pbr;
use crate::types::{Mesh, Scene3d};
use bytemuck::{Pod, Zeroable};
use cadcraft_arch3d::light::{LightKind, SceneLight};
use std::collections::BTreeMap;

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
/// One straight segment of a strip light: `position` = start, `axis` = unit along, `length`,
/// `direction` = the emitting normal, `rgb` = candela per metre × colour / luminance.
pub const LIGHT_STRIP: u32 = 3;

/// `GpuFrame::flags`.
pub const FRAME_SUN: u32 = 1;
pub const FRAME_SKY: u32 = 2;
pub const FRAME_FURNACE: u32 = 4;
pub const FRAME_GROUND: u32 = 8;
/// The environment map light is present [p19-env-path].
pub const FRAME_ENV: u32 = 16;
/// The moon is a directional light [p19-sky].
pub const FRAME_MOON: u32 = 32;
/// The seeing-in clip box is active [p19-seeing-in-render].
pub const FRAME_CLIP: u32 = 64;
/// The sun casts no shadows [p19-sun].
pub const FRAME_SUN_NO_SHADOW: u32 = 128;
/// Normal maps are sampled [p19-normal-maps-render].
pub const FRAME_NORMAL_MAPS: u32 = 256;
/// Surfaces are treated as Lambertian [p19-exposure].
pub const FRAME_LAMBERT: u32 = 512;

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
    /// Any combination of [`FRAME_SUN`], [`FRAME_SKY`], [`FRAME_FURNACE`],
    /// [`FRAME_GROUND`], [`FRAME_ENV`], [`FRAME_MOON`], [`FRAME_CLIP`],
    /// [`FRAME_SUN_NO_SHADOW`], [`FRAME_NORMAL_MAPS`] and [`FRAME_LAMBERT`].
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
    /// Environment map in the aux (`ies`) buffer: texel offset, width, height and flags
    /// (1 present, 2 visible) [p19-env-path].
    pub env: [u32; 4],
    /// Marginal CDF offset and conditional CDF offset, two unused [p19-env-path].
    pub env_aux: [u32; 4],
    /// Environment parameters: x is the total weight [p19-env-path].
    pub env_params: [f32; 4],
    /// Toward the moon (unit, w unused) [p19-sky].
    pub moon_dir: [f32; 4],
    /// Moon illuminance in lux (RGB, w unused) [p19-sky].
    pub moon_rgb: [f32; 4],
    /// Seeing-in clip box, min corner [p19-seeing-in-render].
    pub clip_min: [f32; 4],
    /// Seeing-in clip box, max corner [p19-seeing-in-render].
    pub clip_max: [f32; 4],
    /// Camera forward (xyz, unit) and near-clip distance in metres (w) [p19-seeing-in-render].
    pub view: [f32; 4],
    /// Normal maps in the aux buffer: texel offset, map edge, map count and one unused
    /// [p19-normal-maps-render].
    pub tex: [u32; 4],
    /// Tone-curve code: x is 1 ACES (p19-exposure adds 2 Hable, 3 none).
    pub look: [u32; 4],
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
    let name = material.to_ascii_lowercase();
    // An opening's meshes are `<window|door>-<part>` (arch3d openings.rs): each is its part's
    // material, so window glass lets the sun through.
    let part = name.strip_prefix("window-").or_else(|| name.strip_prefix("door-")).filter(|p| !p.is_empty()).unwrap_or(&name);
    let (mut albedo, metallic, roughness, alpha) = material_pbr(part);
    // The path tracer does not sample textures: a finished surface is its texture's mean colour.
    if let Some(finish) = crate::textures::finish_of(part) {
        albedo = finish_mean_linear(finish);
    }
    let mut m = GpuMaterial { albedo, metallic, roughness, ior: 1.5, ..Default::default() };
    if alpha < 1.0 {
        m.flags |= MAT_GLASS;
        m.transmission = (1.0 - alpha).clamp(0.0, 1.0);
    }
    if name.starts_with("light-") {
        m.flags |= MAT_NO_SHADOW;
    }
    if name == "light-lens-lit" {
        m.flags |= MAT_EMIT_CAMERA_ONLY;
        m.emission = [LENS_NITS; 3];
    }
    if name.starts_with("mirror") {
        m.albedo = [0.95; 3];
        m.metallic = 1.0;
        m.roughness = 0.02;
    }
    m
}

/// The material of a mesh of `material` in `scene`. A key of [`Scene3d::materials`] is its
/// `MaterialDef`: albedo = base colour (which already carries a paint's tint) x its texture's mean
/// linear colour when it has a valid one (the path tracer samples no textures, as for the phase-13
/// finishes), metallic and roughness as given, IOR 1.5, opaque, no flags. Every other name is
/// [`material_for`].
pub fn material_for_scene(scene: &Scene3d, material: &str) -> GpuMaterial {
    let Some(def) = scene.materials.get(material) else { return material_for(material) };
    let (mut albedo, metallic, roughness, _) = crate::pipeline::material_def_pbr(def);
    if let Some(mean) = def.texture.as_deref().and_then(|key| scene.textures.get(key)).and_then(crate::textures::mean_linear) {
        albedo = [albedo[0] * mean[0], albedo[1] * mean[1], albedo[2] * mean[2]];
    }
    GpuMaterial { albedo, metallic, roughness, ior: 1.5, ..Default::default() }
}

/// The mean linear colour of a finish texture, computed once per finish.
fn finish_mean_linear(finish: cadcraft_doc::arch::Finish) -> [f32; 3] {
    use cadcraft_doc::arch::Finish;
    static MEANS: std::sync::OnceLock<Vec<[f32; 3]>> = std::sync::OnceLock::new();
    let means =
        MEANS.get_or_init(|| Finish::ALL.iter().map(|f| cadcraft_arch3d::texture::mean_linear(&cadcraft_arch3d::texture::texture(*f))).collect());
    Finish::ALL.iter().position(|f| *f == finish).and_then(|i| means.get(i)).copied().unwrap_or([0.75; 3])
}

/// Radiance of a lit lens (nits), seen directly.
pub const LENS_NITS: f32 = 20_000.0;

/// Make every material in `scene` a pure Lambert (albedo `a`, metallic 0, roughness 1, ior 1,
/// no transmission), keeping emission only on light lenses (the [`MAT_EMIT_CAMERA_ONLY`] flag).
/// Called after [`build_gpu_scene`] when the `lambert` render parameter is set, so the path
/// tracer treats every surface as a diffuse reflector of the given albedo.
pub fn apply_lambert(scene: &mut GpuScene, albedo: f32) {
    for m in scene.materials.iter_mut() {
        m.albedo = [albedo, albedo, albedo];
        m.metallic = 0.0;
        m.roughness = 1.0;
        m.ior = 1.0;
        m.transmission = 0.0;
        if m.flags & MAT_EMIT_CAMERA_ONLY == 0 {
            m.emission = [0.0, 0.0, 0.0];
        }
    }
}

/// The object id of the ground quad (`shadow::GROUND_OBJECT`).
const GROUND_OBJECT: u32 = u32::MAX;
/// Half the side of the ground square (m), centred on the origin.
const GROUND_HALF_SIDE: f32 = 100.0;
/// Deepest tree the traversal stack allows; a node at this depth is a leaf whatever it holds.
const MAX_BVH_DEPTH: u32 = 64;

/// Build the buffers for `scene` (its meshes, lights and `ground`, a 200 m square at z = 0 with
/// the `ground` material when asked). Lights: every `scene.lights` entry ([`GpuLight`] as above,
/// IES tables packed in light order, a table shared by several lights once). `furnace`: every
/// material diffuse with its albedo and emission, no lights. Triangles with a non-finite corner,
/// an index out of range or a zero area are skipped; more than [`MAX_GPU_TRIANGLES`] gives an
/// empty scene. The tree is empty (no nodes) for a scene without triangles.
pub fn build_gpu_scene(scene: &Scene3d, ground: bool, furnace: Option<Furnace>) -> GpuScene {
    // One look-up per material name (a texture's mean is not cheap); the result is the same.
    let mut by_name: BTreeMap<&str, GpuMaterial> = BTreeMap::new();
    let mut materials: Vec<GpuMaterial> =
        scene.meshes.iter().map(|m| *by_name.entry(m.material.as_str()).or_insert_with(|| material_for_scene(scene, &m.material))).collect();
    if ground {
        materials.push(material_for_scene(scene, "ground"));
    }
    if let Some(f) = furnace {
        materials.iter_mut().for_each(|m| *m = furnace_material(f));
    }
    let Some(tris) = collect_triangles(scene, ground, &materials) else { return GpuScene::default() };
    let (nodes, order) = build_bvh(&tris);
    let tris = order.iter().filter_map(|&i| tris.get(i as usize).copied()).collect();
    let (lights, ies) = if furnace.is_some() { (Vec::new(), Vec::new()) } else { pack_lights(&scene.lights) };
    GpuScene { nodes, tris, materials, lights, ies }
}

/// The furnace's material: diffuse, `albedo`, emitting `emission` nits, nothing else.
fn furnace_material(f: Furnace) -> GpuMaterial {
    GpuMaterial { albedo: [f.albedo; 3], emission: [f.emission; 3], roughness: 1.0, ior: 1.0, ..Default::default() }
}

fn corner(p: Option<&[f32; 3]>) -> Option<[f32; 3]> {
    p.copied().filter(|c| c.iter().all(|v| v.is_finite()))
}

/// One triangle of `v0`, `v1`, `v2` for material index `material`; `None` when the edges are not
/// finite or the triangle has no area.
fn gpu_tri(v: [[f32; 3]; 3], material: u32, object: u32, flags: u32) -> Option<GpuTri> {
    let e1 = [v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]];
    let e2 = [v[2][0] - v[0][0], v[2][1] - v[0][1], v[2][2] - v[0][2]];
    let (a, b) = (e1.map(f64::from), e2.map(f64::from));
    let c = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let len = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
    if !len.is_finite() || len <= 1e-12 {
        return None;
    }
    let n = [(c[0] / len) as f32, (c[1] / len) as f32, (c[2] / len) as f32];
    Some(GpuTri { v0: v[0], material, e1, object, e2, flags, n, _pad: 0 })
}

/// Every valid triangle of the meshes in scene order, then the ground. `None` past the cap.
fn collect_triangles(scene: &Scene3d, ground: bool, materials: &[GpuMaterial]) -> Option<Vec<GpuTri>> {
    let mut tris: Vec<GpuTri> = Vec::new();
    for (mi, mesh) in scene.meshes.iter().enumerate() {
        let material = u32::try_from(mi).ok()?;
        let flags = materials.get(mi).map_or(0, |m| m.flags);
        if !push_mesh(&mut tris, mesh, material, flags) {
            return None;
        }
    }
    if ground {
        let material = u32::try_from(scene.meshes.len()).ok()?;
        let flags = materials.get(scene.meshes.len()).map_or(0, |m| m.flags);
        let h = GROUND_HALF_SIDE;
        let quad = [[-h, -h, 0.0], [h, -h, 0.0], [h, h, 0.0], [-h, h, 0.0]];
        for ids in [[0, 1, 2], [0, 2, 3]] {
            tris.extend(gpu_tri(ids.map(|i| quad[i]), material, GROUND_OBJECT, flags));
        }
        if tris.len() > MAX_GPU_TRIANGLES {
            return None;
        }
    }
    Some(tris)
}

/// Append a mesh's valid triangles; `false` when the cap is passed.
fn push_mesh(tris: &mut Vec<GpuTri>, mesh: &Mesh, material: u32, flags: u32) -> bool {
    for g in mesh.indices.chunks_exact(3) {
        let at = |k: usize| g.get(k).and_then(|&i| corner(mesh.positions.get(i as usize)));
        let (Some(a), Some(b), Some(c)) = (at(0), at(1), at(2)) else { continue };
        tris.extend(gpu_tri([a, b, c], material, mesh.object.0, flags));
        if tris.len() > MAX_GPU_TRIANGLES {
            return false;
        }
    }
    true
}

/// The lights in order and the IES floats they index.
fn pack_lights(lights: &[SceneLight]) -> (Vec<GpuLight>, Vec<f32>) {
    let mut ies: Vec<f32> = Vec::new();
    // (profile address, offset) of the tables already packed: lights sharing a profile share it.
    let mut packed: Vec<(usize, u32)> = Vec::new();
    let mut out = Vec::with_capacity(lights.len());
    for light in lights {
        if light.kind == LightKind::Strip {
            out.extend(strip_gpu_lights(light));
            continue;
        }
        let mut g = gpu_light(light);
        if let Some(profile) = &light.ies {
            let key = std::sync::Arc::as_ptr(profile) as usize;
            let known = packed.iter().find(|(k, _)| *k == key).map(|&(_, off)| off);
            let offset = known.or_else(|| {
                let off = pack_ies(&profile.vertical, &profile.horizontal, &profile.candela, &mut ies)?;
                packed.push((key, off));
                Some(off)
            });
            if let Some(offset) = offset {
                g.ies_offset = offset;
                g.ies_v = profile.vertical.len() as u32;
                g.ies_h = profile.horizontal.len() as u32;
            }
        }
        out.push(g);
    }
    (out, ies)
}

/// Append a table's vertical angles, horizontal angles and candela (horizontal-major) to `ies`;
/// the offset it starts at. `None` (nothing appended) for an empty, mismatched or non-finite table.
fn pack_ies(vertical: &[f64], horizontal: &[f64], candela: &[f64], ies: &mut Vec<f32>) -> Option<u32> {
    let (v, h) = (vertical.len(), horizontal.len());
    let all = || vertical.iter().chain(horizontal).chain(candela);
    if v == 0 || h == 0 || candela.len() != v.checked_mul(h)? || !all().all(|x| x.is_finite()) {
        return None;
    }
    let offset = u32::try_from(ies.len()).ok()?;
    u32::try_from(ies.len() + v + h + candela.len()).ok()?;
    ies.extend(all().map(|&x| x as f32));
    Some(offset)
}

/// The lights of a strip, one [`LIGHT_STRIP`] per straight segment ([`crate::lights::strip_runs`]);
/// empty for a strip without samples. `rgb` is the segment's candela per metre × colour /
/// luminance(colour), so `li = rgb × length × cos / d²` is lux on a surface facing the piece.
fn strip_gpu_lights(l: &SceneLight) -> Vec<GpuLight> {
    let colour = luminance_rgb(l.color);
    let f = |v: cadcraft_geom::Vec3| [v.x as f32, v.y as f32, v.z as f32];
    crate::lights::strip_runs(l)
        .iter()
        .filter_map(|r| {
            let along = r.end - r.start;
            let length = along.len();
            if !length.is_finite() || length <= 0.0 {
                return None;
            }
            Some(GpuLight {
                position: f(r.start),
                kind: LIGHT_STRIP,
                direction: f(r.normal),
                rgb: colour.map(|c| (r.cd_per_metre * f64::from(c)) as f32),
                axis: f(along * (1.0 / length)),
                length: length as f32,
                ..Default::default()
            })
        })
        .collect()
}

/// One light without its IES table. A point or spot light's `rgb` is `intensity` × colour /
/// luminance(colour) (cd); an area light's is its radiance, `intensity / (width × length)` nits.
fn gpu_light(l: &SceneLight) -> GpuLight {
    let kind = match l.kind {
        LightKind::Point => LIGHT_POINT,
        LightKind::Spot => LIGHT_SPOT,
        LightKind::Area => LIGHT_AREA,
        // A strip becomes one light per segment (`strip_gpu_lights`); this arm is not reached.
        LightKind::Strip => LIGHT_STRIP,
    };
    let scale = if kind == LIGHT_AREA {
        let area = l.size[0] * l.size[1];
        if area > 0.0 && area.is_finite() { l.intensity / area } else { 0.0 }
    } else {
        l.intensity
    };
    let colour = luminance_rgb(l.color);
    let f = |v: cadcraft_geom::Vec3| [v.x as f32, v.y as f32, v.z as f32];
    GpuLight {
        position: f(l.position),
        kind,
        direction: f(l.direction),
        cos_inner: l.cos_inner as f32,
        rgb: colour.map(|c| (scale * f64::from(c)) as f32),
        cos_outer: l.cos_outer as f32,
        axis: f(l.axis),
        width: l.size[0] as f32,
        length: l.size[1] as f32,
        ..Default::default()
    }
}

/// A triangle's box and centroid, for the tree builder.
#[derive(Clone, Copy)]
struct Prim {
    min: [f32; 3],
    max: [f32; 3],
    centre: [f32; 3],
}

/// The corners of a triangle as the traversal computes them.
fn tri_corners(t: &GpuTri) -> [[f32; 3]; 3] {
    [t.v0, [t.v0[0] + t.e1[0], t.v0[1] + t.e1[1], t.v0[2] + t.e1[2]], [t.v0[0] + t.e2[0], t.v0[1] + t.e2[1], t.v0[2] + t.e2[2]]]
}

const EMPTY_BOX: ([f32; 3], [f32; 3]) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);

fn grow(b: &mut ([f32; 3], [f32; 3]), min: [f32; 3], max: [f32; 3]) {
    for k in 0..3 {
        b.0[k] = b.0[k].min(min[k]);
        b.1[k] = b.1[k].max(max[k]);
    }
}

/// Surface area of a box (0 for an empty one).
fn box_area(b: &([f32; 3], [f32; 3])) -> f32 {
    let d = [b.1[0] - b.0[0], b.1[1] - b.0[1], b.1[2] - b.0[2]];
    if d.iter().any(|v| *v < 0.0) {
        return 0.0;
    }
    2.0 * (d[0] * d[1] + d[1] * d[2] + d[2] * d[0])
}

/// The tree over `tris`: the nodes (depth first) and the triangle order the leaves index.
fn build_bvh(tris: &[GpuTri]) -> (Vec<GpuNode>, Vec<u32>) {
    let prims: Vec<Prim> = tris
        .iter()
        .map(|t| {
            let mut b = EMPTY_BOX;
            for c in tri_corners(t) {
                grow(&mut b, c, c);
            }
            Prim { min: b.0, max: b.1, centre: [(b.0[0] + b.1[0]) * 0.5, (b.0[1] + b.1[1]) * 0.5, (b.0[2] + b.1[2]) * 0.5] }
        })
        .collect();
    let mut order: Vec<u32> = (0..prims.len() as u32).collect();
    let mut nodes = Vec::with_capacity(prims.len() / 2 + 1);
    if !prims.is_empty() {
        build_range(&prims, &mut order, 0, 0, &mut nodes);
    }
    (nodes, order)
}

/// Append the subtree over `order` (the triangles at global positions `first..`) to `nodes`.
fn build_range(prims: &[Prim], order: &mut [u32], first: u32, depth: u32, nodes: &mut Vec<GpuNode>) {
    let (mut bounds, mut centres) = (EMPTY_BOX, EMPTY_BOX);
    for p in order.iter().filter_map(|&i| prims.get(i as usize)) {
        grow(&mut bounds, p.min, p.max);
        grow(&mut centres, p.centre, p.centre);
    }
    let index = nodes.len();
    let n = order.len();
    nodes.push(GpuNode { min: bounds.0, left_or_first: first, max: bounds.1, count: n as u32 });
    if n <= MAX_LEAF_TRIS as usize || depth >= MAX_BVH_DEPTH {
        return;
    }
    let mid = split_point(prims, order, box_area(&bounds), &centres);
    let (left, right) = order.split_at_mut(mid);
    if let Some(node) = nodes.get_mut(index) {
        node.count = 0;
    }
    build_range(prims, left, first, depth + 1, nodes);
    let right_index = nodes.len() as u32;
    if let Some(node) = nodes.get_mut(index) {
        node.left_or_first = right_index;
    }
    build_range(prims, right, first + mid as u32, depth + 1, nodes);
}

/// Reorder `order` so `[..mid]` is the left child and return `mid` (always `1..len`): the best
/// binned SAH split on the widest centroid axis when it beats a leaf, else the median (a leaf
/// never holds more than [`MAX_LEAF_TRIS`] unless the depth cap forces it).
fn split_point(prims: &[Prim], order: &mut [u32], node_area: f32, centres: &([f32; 3], [f32; 3])) -> usize {
    let extent = [centres.1[0] - centres.0[0], centres.1[1] - centres.0[1], centres.1[2] - centres.0[2]];
    let axis = (0..3).fold(0, |best, k| if extent[k] > extent[best] { k } else { best });
    if let Some(bin) = best_sah_bin(prims, order, axis, centres.0[axis], extent[axis], node_area) {
        let bin_of = |i: &u32| prims.get(*i as usize).map_or(0, |p| bin_index(p.centre[axis], centres.0[axis], extent[axis]));
        let mid = partition(order, |i| bin_of(i) <= bin);
        if mid > 0 && mid < order.len() {
            return mid;
        }
    }
    let mid = order.len() / 2;
    order.select_nth_unstable_by(mid, |a, b| {
        let c = |i: &u32| prims.get(*i as usize).map_or(0.0, |p| p.centre[axis]);
        c(a).total_cmp(&c(b))
    });
    mid
}

fn bin_index(c: f32, lo: f32, extent: f32) -> usize {
    (((c - lo) * (SAH_BINS as f32 / extent)) as usize).min(SAH_BINS - 1)
}

/// In-place partition: the elements satisfying `pred` first; how many there are.
fn partition(order: &mut [u32], pred: impl Fn(&u32) -> bool) -> usize {
    let mut i = 0;
    for j in 0..order.len() {
        if order.get(j).is_some_and(&pred) {
            order.swap(i, j);
            i += 1;
        }
    }
    i
}

/// The bin whose boundary (items in bins `..=bin` go left) gives the cheapest split, if its cost
/// `area(left) · n_left + area(right) · n_right` is below the leaf's `area(node) · n`.
fn best_sah_bin(prims: &[Prim], order: &[u32], axis: usize, lo: f32, extent: f32, node_area: f32) -> Option<usize> {
    if !(extent > 0.0) || !(SAH_BINS as f32 / extent).is_finite() {
        return None;
    }
    let mut bins: [(u32, ([f32; 3], [f32; 3])); SAH_BINS] = [(0, EMPTY_BOX); SAH_BINS];
    for p in order.iter().filter_map(|&i| prims.get(i as usize)) {
        if let Some((count, b)) = bins.get_mut(bin_index(p.centre[axis], lo, extent)) {
            *count += 1;
            grow(b, p.min, p.max);
        }
    }
    // Cost of everything in bins `..=i` going left, from a forward sweep, and of the rest going
    // right, from a backward one.
    let mut left_cost = [f32::INFINITY; SAH_BINS];
    let (mut acc, mut n) = (EMPTY_BOX, 0u32);
    for (i, (count, b)) in bins.iter().enumerate() {
        grow(&mut acc, b.0, b.1);
        n += count;
        left_cost[i] = if n > 0 { box_area(&acc) * n as f32 } else { f32::INFINITY };
    }
    let (mut acc, mut n) = (EMPTY_BOX, 0u32);
    let mut best: Option<(f32, usize)> = None;
    for i in (1..SAH_BINS).rev() {
        let (count, b) = bins[i];
        grow(&mut acc, b.0, b.1);
        n += count;
        let cost = left_cost[i - 1] + if n > 0 { box_area(&acc) * n as f32 } else { f32::INFINITY };
        if cost.is_finite() && best.is_none_or(|(c, _)| cost <= c) {
            best = Some((cost, i - 1));
        }
    }
    best.filter(|(c, _)| *c < node_area * order.len() as f32).map(|(_, i)| i)
}

/// A CPU hit: the distance along the (unit) ray and the triangle's index in `tris`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CpuHit {
    pub t: f32,
    pub tri: u32,
}

/// Hits nearer than this along a ray are the surface the ray starts on.
const TRACE_EPS: f32 = 1e-4;
/// Slack on the barycentric bounds of a hit, so a ray through the seam between two triangles of
/// a closed mesh hits one of them (single-precision rounding would let it through).
const UV_EPS: f32 = 1e-6;
/// The slab test's far distance is scaled up by this, the usual guard against a ray grazing a
/// box face being rounded out of it.
const BOX_SLACK: f32 = 1.000_000_4;
const STACK_SIZE: usize = 128;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Möller–Trumbore, both sides: the distance along `origin + t·dir` to `tri`, if it hits.
fn tri_distance(tri: &GpuTri, origin: [f32; 3], dir: [f32; 3]) -> Option<f32> {
    let p = cross(dir, tri.e2);
    let det = dot(tri.e1, p);
    if !(det.abs() > 1e-12) {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(origin, tri.v0);
    let u = dot(s, p) * inv;
    if !(-UV_EPS..=1.0 + UV_EPS).contains(&u) {
        return None;
    }
    let q = cross(s, tri.e1);
    let v = dot(dir, q) * inv;
    if !(v >= -UV_EPS) || u + v > 1.0 + UV_EPS {
        return None;
    }
    let t = dot(tri.e2, q) * inv;
    t.is_finite().then_some(t)
}

/// Where the ray enters the node's box (0 when it starts inside) if it meets it before `t_far`.
/// A zero direction component is `±inf` in the reciprocal: the ray is inside that slab or misses.
fn box_entry(node: &GpuNode, origin: [f32; 3], dir: [f32; 3], inv: [f32; 3], t_far: f32) -> Option<f32> {
    let (mut near, mut far) = (0.0f32, t_far);
    for k in 0..3 {
        if dir[k] == 0.0 {
            if origin[k] < node.min[k] || origin[k] > node.max[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((node.min[k] - origin[k]) * inv[k], (node.max[k] - origin[k]) * inv[k]);
        near = near.max(a.min(b));
        far = far.min(a.max(b) * BOX_SLACK);
    }
    (near <= far).then_some(near)
}

impl GpuScene {
    /// The nearest triangle the ray `origin + t·dir` (unit `dir`) hits with `1e-4 < t < t_max`,
    /// walking the BVH exactly as `pt/traverse.wgsl` does (the reference its GPU test compares
    /// with): a stack, the nearer child (by its box's entry distance) first, a slab test with the
    /// reciprocal direction and double-sided Möller–Trumbore. Ties break by the lower triangle
    /// index. `None` for a miss, an empty scene or a hostile ray.
    pub fn trace_cpu(&self, origin: [f32; 3], dir: [f32; 3], t_max: f32) -> Option<CpuHit> {
        let finite = origin.iter().chain(&dir).all(|v| v.is_finite());
        if self.nodes.is_empty() || !finite || !(t_max > TRACE_EPS) || dir.iter().all(|v| *v == 0.0) {
            return None;
        }
        let inv = dir.map(|d| 1.0 / d);
        let mut best: Option<CpuHit> = None;
        let mut best_t = t_max;
        let mut stack = [(0u32, 0.0f32); STACK_SIZE];
        let root = self.nodes.first().and_then(|n| box_entry(n, origin, dir, inv, best_t))?;
        stack[0] = (0, root);
        let mut top = 1;
        while top > 0 {
            top -= 1;
            let (index, entry) = stack[top];
            let Some(node) = self.nodes.get(index as usize) else { continue };
            if entry > best_t {
                continue;
            }
            if node.count > 0 {
                self.leaf_hit(node, origin, dir, &mut best, &mut best_t);
                continue;
            }
            let child = |i: u32| self.nodes.get(i as usize).and_then(|n| box_entry(n, origin, dir, inv, best_t)).map(|e| (i, e));
            let (left, right) = (child(index + 1), child(node.left_or_first));
            // Far child first onto the stack, so the near one pops first.
            let pair = match (left, right) {
                (Some(l), Some(r)) if l.1 <= r.1 => [Some(r), Some(l)],
                (Some(l), Some(r)) => [Some(l), Some(r)],
                (l, r) => [l.or(r), None],
            };
            for entry in pair.into_iter().flatten() {
                if let Some(slot) = stack.get_mut(top) {
                    *slot = entry;
                    top += 1;
                }
            }
        }
        best
    }

    /// Test a leaf's triangles, keeping the nearest hit in `best` / `best_t`.
    fn leaf_hit(&self, node: &GpuNode, origin: [f32; 3], dir: [f32; 3], best: &mut Option<CpuHit>, best_t: &mut f32) {
        let first = node.left_or_first as usize;
        let end = first.saturating_add(node.count as usize).min(self.tris.len());
        for index in first..end {
            let Some(tri) = self.tris.get(index) else { continue };
            let Some(t) = tri_distance(tri, origin, dir) else { continue };
            let tied = t == *best_t && best.is_some_and(|b| (index as u32) < b.tri);
            if t > TRACE_EPS && (t < *best_t || tied) {
                *best_t = t;
                *best = Some(CpuHit { t, tri: index as u32 });
            }
        }
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
        assert_eq!(std::mem::size_of::<GpuFrame>(), 368);
        assert_eq!(std::mem::size_of::<GpuProbe>(), 32);
    }

    #[test]
    fn gpu_frame_layout_matches_wgsl() {
        // Field offsets, in the order the WGSL `Frame` declares them.
        assert_eq!(std::mem::offset_of!(GpuFrame, inv_view_proj), 0);
        assert_eq!(std::mem::offset_of!(GpuFrame, eye), 64);
        assert_eq!(std::mem::offset_of!(GpuFrame, size), 80);
        assert_eq!(std::mem::offset_of!(GpuFrame, sample_base), 88);
        assert_eq!(std::mem::offset_of!(GpuFrame, seed), 92);
        assert_eq!(std::mem::offset_of!(GpuFrame, bounces), 96);
        assert_eq!(std::mem::offset_of!(GpuFrame, light_count), 100);
        assert_eq!(std::mem::offset_of!(GpuFrame, flags), 104);
        assert_eq!(std::mem::offset_of!(GpuFrame, spp_this_pass), 108);
        assert_eq!(std::mem::offset_of!(GpuFrame, sun_dir), 112);
        assert_eq!(std::mem::offset_of!(GpuFrame, sun_rgb), 128);
        assert_eq!(std::mem::offset_of!(GpuFrame, sky_zenith), 144);
        assert_eq!(std::mem::offset_of!(GpuFrame, sky_horizon), 160);
        assert_eq!(std::mem::offset_of!(GpuFrame, furnace), 176);
        assert_eq!(std::mem::offset_of!(GpuFrame, probe_count), 192);
        assert_eq!(std::mem::offset_of!(GpuFrame, env), 208);
        assert_eq!(std::mem::offset_of!(GpuFrame, env_aux), 224);
        assert_eq!(std::mem::offset_of!(GpuFrame, env_params), 240);
        assert_eq!(std::mem::offset_of!(GpuFrame, moon_dir), 256);
        assert_eq!(std::mem::offset_of!(GpuFrame, moon_rgb), 272);
        assert_eq!(std::mem::offset_of!(GpuFrame, clip_min), 288);
        assert_eq!(std::mem::offset_of!(GpuFrame, clip_max), 304);
        assert_eq!(std::mem::offset_of!(GpuFrame, view), 320);
        assert_eq!(std::mem::offset_of!(GpuFrame, tex), 336);
        assert_eq!(std::mem::offset_of!(GpuFrame, look), 352);
        assert_eq!(std::mem::size_of::<GpuFrame>(), 368);

        // The six new flag bits have their reserved values and all ten bits are distinct.
        assert_eq!((FRAME_ENV, FRAME_MOON, FRAME_CLIP, FRAME_SUN_NO_SHADOW, FRAME_NORMAL_MAPS, FRAME_LAMBERT), (16, 32, 64, 128, 256, 512));
        let bits = [
            FRAME_SUN,
            FRAME_SKY,
            FRAME_FURNACE,
            FRAME_GROUND,
            FRAME_ENV,
            FRAME_MOON,
            FRAME_CLIP,
            FRAME_SUN_NO_SHADOW,
            FRAME_NORMAL_MAPS,
            FRAME_LAMBERT,
        ];
        for a in 0..bits.len() {
            for b in a + 1..bits.len() {
                assert_eq!(bits[a] & bits[b], 0, "frame flags overlap");
                assert_ne!(bits[a], bits[b], "frame flags equal");
            }
        }

        let wgsl = include_str!("../shaders/pt/common.wgsl");
        // The ten new fields, in the same order as the WGSL `Frame`.
        let fields = [
            "env: vec4<u32>",
            "env_aux: vec4<u32>",
            "env_params: vec4<f32>",
            "moon_dir: vec4<f32>",
            "moon_rgb: vec4<f32>",
            "clip_min: vec4<f32>",
            "clip_max: vec4<f32>",
            "view: vec4<f32>",
            "tex: vec4<u32>",
            "look: vec4<u32>",
        ];
        let mut at = 0usize;
        for f in fields {
            let i = wgsl.find(f).unwrap_or_else(|| panic!("wgsl missing field {f}"));
            assert!(i > at, "wgsl field {f} out of order");
            at = i + f.len();
        }
        for (name, value) in [
            ("FRAME_ENV", 16u32),
            ("FRAME_MOON", 32u32),
            ("FRAME_CLIP", 64u32),
            ("FRAME_SUN_NO_SHADOW", 128u32),
            ("FRAME_NORMAL_MAPS", 256u32),
            ("FRAME_LAMBERT", 512u32),
        ] {
            assert!(wgsl.contains(&format!("const {name}: u32 = {value}u;")), "wgsl missing `const {name}: u32 = {value}u;`",);
        }
        assert!(wgsl.contains("fn hit_clipped(p: vec3<f32>) -> bool"));
        assert!(wgsl.contains("fn camera_t_min(d: vec3<f32>) -> f32"));
    }

    #[test]
    fn path_tracer_uses_the_texture_mean() {
        use cadcraft_arch3d::texture::{mean_linear, texture};
        use cadcraft_doc::arch::Finish;
        for f in Finish::ALL {
            let mean = mean_linear(&texture(f));
            for prefix in ["wall", "roof", "floor"] {
                assert_eq!(material_for(&format!("{prefix}-{}", f.name())).albedo, mean, "{prefix}-{}", f.name());
            }
        }
        // Unfinished materials keep their flat albedo.
        assert_eq!(material_for("wall").albedo, crate::pipeline::material_albedo("wall"));
        assert_eq!(material_for("roof").albedo, crate::pipeline::material_albedo("roof"));
    }

    #[test]
    fn path_tracer_uses_material_means() {
        use crate::types::MaterialDef;
        use cadcraft_doc::arch::ImageRgba;
        let mut red_blue = ImageRgba { width: 2, height: 1, ..Default::default() };
        red_blue.rgba = vec![255, 0, 0, 255, 0, 0, 255, 255];
        let mut scene = Scene3d::default();
        scene.textures.insert("sym:k:t0".into(), red_blue);
        scene.textures.insert("sym:k:t1".into(), ImageRgba { width: 3, height: 3, ..Default::default() });
        let textured =
            MaterialDef { base_color: [0.5, 1.0, 0.8, 1.0], metallic: 0.25, roughness: 0.6, texture: Some("sym:k:t0".into()), ..Default::default() };
        scene.materials.insert("sym:k:0".into(), textured);
        // A tint is already in the base colour; an untextured definition is its base colour.
        let painted = MaterialDef { base_color: [0.2, 0.4, 0.6, 1.0], tint: Some([51, 102, 153]), ..Default::default() };
        scene.materials.insert("paint:room/r0/floor".into(), painted);
        // A missing or invalid image leaves the base colour alone.
        scene.materials.insert("sym:k:1".into(), MaterialDef { base_color: [0.3; 4], texture: Some("sym:gone:t0".into()), ..Default::default() });
        scene.materials.insert("sym:k:2".into(), MaterialDef { base_color: [0.3; 4], texture: Some("sym:k:t1".into()), ..Default::default() });

        // Red and blue average to (0.5, 0, 0.5) in linear light, times the base colour.
        let m = material_for_scene(&scene, "sym:k:0");
        for (got, want) in m.albedo.iter().zip([0.25f32, 0.0, 0.4]) {
            assert!((got - want).abs() < 1e-6, "{:?}", m.albedo);
        }
        assert_eq!((m.metallic, m.roughness, m.ior, m.transmission, m.emission, m.flags), (0.25, 0.6, 1.5, 0.0, [0.0; 3], 0));
        assert_eq!(material_for_scene(&scene, "paint:room/r0/floor").albedo, [0.2, 0.4, 0.6]);
        assert_eq!(material_for_scene(&scene, "sym:k:1").albedo, [0.3; 3]);
        assert_eq!(material_for_scene(&scene, "sym:k:2").albedo, [0.3; 3]);
        assert_eq!(material_for_scene(&scene, "paint:room/r0/floor").roughness, MaterialDef::default().roughness);
        // Every other name is its phase-1..13 material.
        for name in ["wall", "wall-brick", "glass", "window-glass", "light-lens-lit", "mirror-x", "sym:other:0", "SYM:K:0"] {
            assert_eq!(material_for_scene(&scene, name), material_for(name), "{name}");
        }
        // The built scene takes one per mesh.
        scene.meshes = vec![
            prism("sym:k:0", 1, [0.0; 3], [1.0; 3]),
            prism("wall-brick", 2, [2.0, 0.0, 0.0], [3.0, 1.0, 1.0]),
            prism("sym:k:0", 1, [4.0, 0.0, 0.0], [5.0, 1.0, 1.0]),
        ];
        let gpu = build_gpu_scene(&scene, false, None);
        assert_eq!(gpu.materials, vec![m, material_for("wall-brick"), m]);
    }

    #[test]
    fn luminance_rgb_keeps_luminance_one() {
        let c = luminance_rgb([1.0, 0.8, 0.6]);
        let y = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        assert!((y - 1.0).abs() < 1e-5);
        assert_eq!(luminance_rgb([0.0; 3]), [1.0; 3]);
    }

    use crate::types::ArchId;
    use cadcraft_geom::Vec3;
    use std::sync::Arc;

    /// A deterministic stream of floats in `0..1`.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            ((self.0 >> 40) as f32) / (1u64 << 24) as f32
        }
        fn between(&mut self, lo: f32, hi: f32) -> f32 {
            lo + (hi - lo) * self.next()
        }
    }

    /// An axis-aligned prism as 12 triangles.
    fn prism(material: &str, object: u32, min: [f32; 3], max: [f32; 3]) -> Mesh {
        let mut m = Mesh::new(material, ArchId(object));
        for k in 0..8 {
            m.positions.push([
                if k & 1 == 0 { min[0] } else { max[0] },
                if k & 2 == 0 { min[1] } else { max[1] },
                if k & 4 == 0 { min[2] } else { max[2] },
            ]);
        }
        let quads: [[u32; 4]; 6] = [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]];
        for q in quads {
            m.indices.extend([q[0], q[1], q[2], q[0], q[2], q[3]]);
        }
        m
    }

    fn random_prisms(rng: &mut Rng, count: usize) -> Scene3d {
        let mut scene = Scene3d::default();
        for i in 0..count {
            let c = [rng.between(-10.0, 10.0), rng.between(-10.0, 10.0), rng.between(-10.0, 10.0)];
            let h = [rng.between(0.25, 2.0), rng.between(0.25, 2.0), rng.between(0.25, 2.0)];
            let material = ["wall", "floor", "glass", "light-lens-lit"][i % 4];
            scene.meshes.push(prism(material, i as u32 + 1, [c[0] - h[0], c[1] - h[1], c[2] - h[2]], [c[0] + h[0], c[1] + h[1], c[2] + h[2]]));
        }
        scene
    }

    fn corners(t: &GpuTri) -> [[f32; 3]; 3] {
        tri_corners(t)
    }

    #[test]
    fn gpu_scene_holds_every_triangle_once() {
        let mut scene = random_prisms(&mut Rng(7), 6);
        // A mesh with one good triangle among a non-finite corner, an index out of range and a
        // zero-area triangle: only the good one is kept.
        let mut bad = Mesh::new("wall", ArchId(99));
        bad.positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [f32::NAN, 0.0, 0.0], [2.0, 0.0, 0.0]];
        bad.indices = vec![0, 1, 2, 0, 1, 3, 0, 1, 9, 0, 1, 4, 0, 0, 1];
        scene.meshes.push(bad);
        let gpu = build_gpu_scene(&scene, false, None);
        assert_eq!(gpu.tris.len(), 6 * 12 + 1);
        assert_eq!(gpu.materials.len(), 7);
        // Each source triangle appears exactly once (compared by its corners), in scene order
        // within its mesh's material.
        let mut want: Vec<([u32; 9], u32)> = Vec::new();
        let bits = |c: [[f32; 3]; 3]| {
            let mut out = [0u32; 9];
            for (k, v) in c.iter().flatten().enumerate() {
                out[k] = v.to_bits();
            }
            out
        };
        for (mi, mesh) in scene.meshes.iter().enumerate() {
            for g in mesh.indices.chunks_exact(3) {
                let at = |k: usize| mesh.positions.get(g[k] as usize).copied();
                if let (Some(a), Some(b), Some(c)) = (at(0), at(1), at(2)) {
                    let t = GpuTri { v0: a, e1: sub(b, a), e2: sub(c, a), ..Default::default() };
                    if a.iter().chain(&b).chain(&c).all(|v| v.is_finite()) && gpu_tri([a, b, c], 0, 0, 0).is_some() {
                        want.push((
                            bits([a, [a[0] + t.e1[0], a[1] + t.e1[1], a[2] + t.e1[2]], [a[0] + t.e2[0], a[1] + t.e2[1], a[2] + t.e2[2]]]),
                            mi as u32,
                        ));
                    }
                }
            }
        }
        let mut have: Vec<([u32; 9], u32)> = gpu.tris.iter().map(|t| (bits(corners(t)), t.material)).collect();
        want.sort();
        have.sort();
        assert_eq!(have, want);
        for t in &gpu.tris {
            let mesh = &scene.meshes[t.material as usize];
            assert_eq!(t.object, mesh.object.0);
            assert_eq!(t.flags, gpu.materials[t.material as usize].flags);
            let len = (t.n[0] * t.n[0] + t.n[1] * t.n[1] + t.n[2] * t.n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-5);
            let c = cross(t.e1, t.e2);
            assert!(dot(c, t.n) > 0.0, "n follows e1 x e2");
        }
        // Glass and lens flags reach the triangles.
        assert!(gpu.tris.iter().any(|t| t.flags & MAT_GLASS != 0));
        assert!(gpu.tris.iter().any(|t| t.flags & MAT_NO_SHADOW != 0));
        // No scene, no nodes, and a mesh-less scene still answers a trace.
        let empty = build_gpu_scene(&Scene3d::default(), false, None);
        assert!(empty.nodes.is_empty() && empty.tris.is_empty());
        assert_eq!(empty.trace_cpu([0.0; 3], [0.0, 0.0, 1.0], 100.0), None);
    }

    fn check_tree(gpu: &GpuScene) {
        let mut seen = vec![0u32; gpu.tris.len()];
        let mut visit = vec![0usize];
        let mut nodes_seen = 0;
        while let Some(i) = visit.pop() {
            nodes_seen += 1;
            let node = gpu.nodes[i];
            if node.count == 0 {
                let (l, r) = (i + 1, node.left_or_first as usize);
                assert!(l < gpu.nodes.len() && r > l && r < gpu.nodes.len(), "child links of node {i}");
                for c in [l, r] {
                    let child = gpu.nodes[c];
                    for k in 0..3 {
                        assert!(child.min[k] >= node.min[k] && child.max[k] <= node.max[k], "node {c} inside its parent {i}");
                    }
                    visit.push(c);
                }
                continue;
            }
            assert!(node.count <= MAX_LEAF_TRIS, "leaf {i} holds {}", node.count);
            let first = node.left_or_first as usize;
            for t in first..first + node.count as usize {
                seen[t] += 1;
                for c in corners(&gpu.tris[t]) {
                    for k in 0..3 {
                        assert!(c[k] >= node.min[k] && c[k] <= node.max[k], "triangle {t} inside leaf {i}");
                    }
                }
            }
        }
        assert_eq!(nodes_seen, gpu.nodes.len(), "every node is reachable");
        assert!(seen.iter().all(|&n| n == 1), "every triangle is in exactly one leaf");
    }

    #[test]
    fn bvh_leaves_hold_at_most_four_and_bound_their_triangles() {
        let scene = random_prisms(&mut Rng(11), 40);
        let gpu = build_gpu_scene(&scene, true, None);
        assert_eq!(gpu.tris.len(), 40 * 12 + 2);
        assert!(gpu.nodes.len() > 1);
        check_tree(&gpu);
        // Triangles that cannot be told apart (one centroid, one box) are still split to leaves of four.
        let mut stack = Mesh::new("wall", ArchId(1));
        stack.positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        for _ in 0..23 {
            stack.indices.extend([0, 1, 2]);
        }
        let same = build_gpu_scene(&Scene3d { meshes: vec![stack], ..Default::default() }, false, None);
        assert_eq!(same.tris.len(), 23);
        check_tree(&same);
    }

    #[test]
    fn trace_cpu_matches_the_pick_bvh() {
        let mut rng = Rng(42);
        let scene = random_prisms(&mut rng, 40);
        let gpu = build_gpu_scene(&scene, false, None);
        let pick = cadcraft_arch3d::pick::Bvh::build(&scene);
        let (mut hits, mut misses) = (0, 0);
        for n in 0..500 {
            let origin = [rng.between(-14.0, 14.0), rng.between(-14.0, 14.0), rng.between(-14.0, 14.0)];
            // Aimed at a random point of the prisms' volume, so a good share of the rays hit.
            let target = [rng.between(-10.0, 10.0), rng.between(-10.0, 10.0), rng.between(-10.0, 10.0)];
            let mut dir = sub(target, origin);
            if n % 25 == 0 {
                // Axis-aligned rays hit the zero-component branches of the slab test.
                dir = [0.0, 0.0, 1.0];
                dir.rotate_left((n / 25) % 3);
            }
            let len = dot(dir, dir).sqrt();
            let dir = dir.map(|v| v / len);
            let want = pick.ray(
                Vec3::new(origin[0] as f64, origin[1] as f64, origin[2] as f64),
                Vec3::new(dir[0] as f64, dir[1] as f64, dir[2] as f64),
                1e30,
            );
            let got = gpu.trace_cpu(origin, dir, 1e30);
            match (want, got) {
                (None, None) => misses += 1,
                (Some(w), Some(g)) => {
                    hits += 1;
                    assert!((w.distance - f64::from(g.t)).abs() < 1e-3, "ray {n}: pick {} vs gpu {}", w.distance, g.t);
                    assert_eq!(gpu.tris[g.tri as usize].object, w.object.0, "ray {n}");
                }
                (w, g) => panic!("ray {n}: pick {w:?} vs gpu {g:?}"),
            }
        }
        assert!(hits > 150 && misses > 20, "{hits} hits, {misses} misses");
        // t_max clips, a hostile ray misses.
        let o = [-30.0, 0.0, 0.0];
        let d = [1.0, 0.0, 0.0];
        if let Some(h) = gpu.trace_cpu(o, d, 1e30) {
            assert_eq!(gpu.trace_cpu(o, d, h.t * 0.5), None);
        }
        assert_eq!(gpu.trace_cpu([f32::NAN, 0.0, 0.0], d, 1e30), None);
        assert_eq!(gpu.trace_cpu(o, [0.0; 3], 1e30), None);
        assert_eq!(gpu.trace_cpu(o, d, f32::NAN), None);
    }

    #[test]
    fn an_opening_part_is_its_part_material() {
        // arch3d names an opening's meshes `<window|door>-<part>` (openings.rs).
        for name in ["window-glass", "door-glass", "Window-Glass"] {
            let m = material_for(name);
            assert_eq!(m.flags & MAT_GLASS, MAT_GLASS, "{name} lets light through");
            assert_eq!(m.transmission, material_for("glass").transmission, "{name}");
        }
        for (name, part) in [("window-frame", "frame"), ("window-sill", "sill"), ("door-leaf", "leaf"), ("door-casing", "casing")] {
            let m = material_for(name);
            assert_eq!(m.flags & MAT_GLASS, 0, "{name} is opaque");
            assert_eq!((m.albedo, m.roughness), (material_for(part).albedo, material_for(part).roughness), "{name}");
        }
    }

    #[test]
    fn materials_follow_material_pbr() {
        let (albedo, metallic, roughness, _) = material_pbr("wall");
        let wall = material_for("wall");
        assert_eq!((wall.albedo, wall.metallic, wall.roughness, wall.flags), (albedo, metallic, roughness, 0));
        assert_eq!((wall.ior, wall.emission, wall.transmission), (1.5, [0.0; 3], 0.0));
        let glass = material_for("glass");
        assert_eq!(glass.flags, MAT_GLASS);
        assert_eq!(glass.ior, 1.5);
        assert!((glass.transmission - 0.65).abs() < 1e-6);
        assert_eq!(glass.roughness, material_pbr("glass").2);
        let lens = material_for("light-lens-lit");
        assert_eq!(lens.flags, MAT_EMIT_CAMERA_ONLY | MAT_NO_SHADOW);
        assert_eq!(lens.emission, [LENS_NITS; 3]);
        let housing = material_for("light-housing");
        assert_eq!((housing.flags, housing.emission), (MAT_NO_SHADOW, [0.0; 3]));
        let mirror = material_for("mirror-x");
        assert_eq!((mirror.albedo, mirror.metallic, mirror.roughness, mirror.flags), ([0.95; 3], 1.0, 0.02, 0));
        assert_eq!(mirror.ior, 1.5);
    }

    #[test]
    fn strip_lens_camera_only_emissive() {
        // A strip's 1 mm lens is the shared `light-lens-lit` material: per-light lens radiance is
        // not available (materials are per name, not per fixture), so a strip's lens reads
        // LENS_NITS for camera rays, not (lm/m / 0.012) / pi, and lights nothing by bounce.
        let lens = material_for("light-lens-lit");
        assert_eq!(lens.flags & MAT_EMIT_CAMERA_ONLY, MAT_EMIT_CAMERA_ONLY);
        assert_eq!(lens.emission, [LENS_NITS; 3]);
        // The light itself is the emissive strip: one LIGHT_STRIP per straight run, camera rays
        // never see it as geometry.
        let mut strip = SceneLight { kind: LightKind::Strip, color: [1.0; 3], direction: Vec3::new(0.0, 0.0, -1.0), ..Default::default() };
        let pieces: Vec<cadcraft_arch3d::light::StripSample> = (0..10)
            .map(|k| cadcraft_arch3d::light::StripSample {
                position: Vec3::new(0.1 * k as f64 + 0.05, 0.0, 2.4),
                normal: Vec3::new(0.0, 0.0, -1.0),
                intensity: 0.1 * 500.0 / std::f64::consts::PI,
                ..Default::default()
            })
            .collect();
        strip.strip = Some(Arc::new(pieces));
        let scene = Scene3d { lights: vec![strip], ..Default::default() };
        let gpu = build_gpu_scene(&scene, false, None);
        assert_eq!(gpu.lights.len(), 1);
        let l = gpu.lights[0];
        assert_eq!((l.kind, l.direction), (LIGHT_STRIP, [0.0, 0.0, -1.0]));
        assert!((l.length - 1.0).abs() < 1e-5 && l.position[0].abs() < 1e-6 && (l.axis[0] - 1.0).abs() < 1e-6, "{l:?}");
        assert!((luminance_of(l.rgb) - 500.0 / std::f64::consts::PI).abs() < 1e-2, "cd per metre {:?}", l.rgb);
    }

    fn luminance_of(rgb: [f32; 3]) -> f64 {
        0.2126 * f64::from(rgb[0]) + 0.7152 * f64::from(rgb[1]) + 0.0722 * f64::from(rgb[2])
    }

    #[test]
    fn lights_pack_luminance_rgb_and_ies_tables() {
        let spot = SceneLight {
            kind: LightKind::Spot,
            position: Vec3::new(1.0, 2.0, 3.0),
            direction: Vec3::new(0.0, 0.0, -1.0),
            intensity: 950.36,
            color: [1.0, 0.8, 0.6],
            cos_inner: 0.9,
            cos_outer: 0.8,
            axis: Vec3::new(1.0, 0.0, 0.0),
            ..Default::default()
        };
        // 5 vertical angles, 1 horizontal angle, 5 candela (rotationally symmetric).
        let mut with_ies = SceneLight {
            id: ArchId(2),
            kind: LightKind::Point,
            intensity: 500.0,
            color: [1.0; 3],
            ies: Some(Arc::new(Default::default())),
            ..Default::default()
        };
        if let Some(p) = with_ies.ies.as_mut().and_then(Arc::get_mut) {
            p.vertical = vec![0.0, 45.0, 90.0, 135.0, 180.0];
            p.horizontal = vec![0.0];
            p.candela = vec![1000.0, 800.0, 400.0, 100.0, 0.0];
            p.lumens = -1.0;
        }
        let area = SceneLight {
            id: ArchId(3),
            kind: LightKind::Area,
            intensity: 3600.0 / std::f64::consts::PI,
            color: [1.0, 0.9, 0.8],
            size: [0.6, 0.6],
            direction: Vec3::new(0.0, 0.0, -1.0),
            axis: Vec3::new(0.0, 1.0, 0.0),
            cos_inner: 1.0,
            ..Default::default()
        };
        let sharing = SceneLight { id: ArchId(4), ..with_ies.clone() };
        let scene = Scene3d { lights: vec![spot, with_ies.clone(), area, sharing], ..Default::default() };
        let gpu = build_gpu_scene(&scene, false, None);
        assert_eq!(gpu.lights.len(), 4);

        let s = gpu.lights[0];
        assert_eq!(
            (s.kind, s.position, s.direction, s.cos_inner, s.cos_outer, s.axis),
            (LIGHT_SPOT, [1.0, 2.0, 3.0], [0.0, 0.0, -1.0], 0.9, 0.8, [1.0, 0.0, 0.0])
        );
        assert!((luminance_of(s.rgb) - 950.36).abs() < 1e-2, "{}", luminance_of(s.rgb));
        // The colour's ratios survive: blue/red = 0.6/1.
        assert!((s.rgb[2] / s.rgb[0] - 0.6).abs() < 1e-5);
        assert_eq!((s.ies_v, s.ies_h, s.width, s.length), (0, 0, 0.0, 0.0));

        let p = gpu.lights[1];
        assert_eq!(p.kind, LIGHT_POINT);
        assert_eq!((p.ies_offset, p.ies_v, p.ies_h), (0, 5, 1));
        assert_eq!(gpu.ies.len(), 5 + 1 + 5);
        assert_eq!(gpu.ies, vec![0.0, 45.0, 90.0, 135.0, 180.0, 0.0, 1000.0, 800.0, 400.0, 100.0, 0.0]);
        assert!((luminance_of(p.rgb) - 500.0).abs() < 1e-2);

        // An area light's rgb is its radiance: intensity over its area.
        let a = gpu.lights[2];
        assert_eq!((a.kind, a.width, a.length, a.axis), (LIGHT_AREA, 0.6, 0.6, [0.0, 1.0, 0.0]));
        let nits = 3600.0 / std::f64::consts::PI / 0.36;
        assert!((luminance_of(a.rgb) - nits).abs() < nits * 1e-5, "{} vs {nits}", luminance_of(a.rgb));

        // A second light on the same profile points at the same table.
        assert_eq!((gpu.lights[3].ies_offset, gpu.lights[3].ies_v, gpu.lights[3].ies_h), (0, 5, 1));

        // A table of the wrong size is left out, not packed.
        let mut bad = SceneLight { ies: with_ies.ies.as_ref().map(|p| Arc::new((**p).clone())), ..Default::default() };
        if let Some(p) = bad.ies.as_mut().and_then(Arc::get_mut) {
            p.candela.pop();
        }
        let gpu = build_gpu_scene(&Scene3d { lights: vec![bad], ..Default::default() }, false, None);
        assert!(gpu.ies.is_empty());
        assert_eq!(gpu.lights[0].ies_v, 0);
    }

    #[test]
    fn furnace_scene_is_diffuse_with_emission_and_no_lights() {
        let mut scene = random_prisms(&mut Rng(3), 5);
        scene.lights.push(SceneLight { intensity: 100.0, color: [1.0; 3], ..Default::default() });
        let gpu = build_gpu_scene(&scene, true, Some(Furnace { albedo: 0.5, emission: 2.0, ..Default::default() }));
        assert!(gpu.lights.is_empty() && gpu.ies.is_empty());
        assert_eq!(gpu.materials.len(), 6, "one per mesh and the ground");
        for m in &gpu.materials {
            assert_eq!(*m, GpuMaterial { albedo: [0.5; 3], emission: [2.0; 3], roughness: 1.0, ior: 1.0, ..Default::default() });
        }
        assert!(gpu.tris.iter().all(|t| t.flags == 0), "no glass, no lens flags");
        let normal = build_gpu_scene(&scene, true, None);
        assert_eq!(normal.lights.len(), 1);
        assert_eq!(gpu.tris.len(), normal.tris.len());
    }

    #[test]
    fn ground_adds_two_triangles() {
        let scene = random_prisms(&mut Rng(5), 3);
        let without = build_gpu_scene(&scene, false, None);
        let with = build_gpu_scene(&scene, true, None);
        assert_eq!(with.tris.len(), without.tris.len() + 2);
        assert_eq!(with.materials.len(), without.materials.len() + 1);
        assert_eq!(with.materials[3], material_for("ground"));
        let ground: Vec<&GpuTri> = with.tris.iter().filter(|t| t.material == 3).collect();
        assert_eq!(ground.len(), 2);
        for t in ground {
            assert_eq!(t.n, [0.0, 0.0, 1.0]);
            for c in corners(t) {
                assert_eq!(c[2], 0.0);
                assert_eq!(c[0].abs(), 100.0);
                assert_eq!(c[1].abs(), 100.0);
            }
        }
        // A ray from above lands on it, 1.5 m down; an empty scene with only the ground has two triangles.
        let hit = with.trace_cpu([50.0, 50.0, 1.5], [0.0, 0.0, -1.0], 100.0).filter(|h| with.tris[h.tri as usize].material == 3);
        assert!(hit.is_some_and(|h| (h.t - 1.5).abs() < 1e-5));
        let only = build_gpu_scene(&Scene3d::default(), true, None);
        assert_eq!((only.tris.len(), only.nodes.len()), (2, 1));
        // The seam diagonal is still a hit (the two triangles share it).
        assert!(only.trace_cpu([3.0, 3.0, 2.0], [0.0, 0.0, -1.0], 10.0).is_some_and(|h| (h.t - 2.0).abs() < 1e-5));
    }
}
