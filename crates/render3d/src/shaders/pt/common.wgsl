// Path tracer (phase 11): the shared declarations. Byte layouts mirror
// render3d::pathtrace::scene_gpu (change both together).

const PT_PI: f32 = 3.14159265358979323846;
const PT_INF: f32 = 3.0e38;
const PT_EPS: f32 = 1.0e-4;

fn pt_eps(p: vec3<f32>) -> f32 {
    let m = max(max(abs(p.x), abs(p.y)), abs(p.z));
    return PT_EPS * max(1.0, m);
}

fn pt_offset(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    if (dot(n, n) <= 0.0) {
        return p;
    }
    return p + normalize(n) * pt_eps(p);
}

fn pt_step(p: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    if (dot(d, d) <= 0.0) {
        return p;
    }
    return p + normalize(d) * pt_eps(p);
}

const MAT_GLASS: u32 = 1u;
const MAT_EMIT_CAMERA_ONLY: u32 = 2u;
const MAT_NO_SHADOW: u32 = 4u;

const LIGHT_POINT: u32 = 0u;
const LIGHT_SPOT: u32 = 1u;
const LIGHT_AREA: u32 = 2u;
// One segment of a strip: position = start, axis = unit along, length, direction = emitting normal.
const LIGHT_STRIP: u32 = 3u;

const FRAME_SUN: u32 = 1u;
const FRAME_SKY: u32 = 2u;
const FRAME_FURNACE: u32 = 4u;
const FRAME_GROUND: u32 = 8u;
// [p19-env-path] the environment-map light is present.
const FRAME_ENV: u32 = 16u;
// [p19-sky] the moon is a light source.
const FRAME_MOON: u32 = 32u;
// [p19-seeing-in-render] the seeing-in clip box is active.
const FRAME_CLIP: u32 = 64u;
// [p19-sun] the sun casts no shadows.
const FRAME_SUN_NO_SHADOW: u32 = 128u;
// [p19-normal-maps-render] normal maps are sampled.
const FRAME_NORMAL_MAPS: u32 = 256u;
// [p19-exposure] surfaces are Lambertian.
const FRAME_LAMBERT: u32 = 512u;

const MAX_PATH_DEPTH: u32 = 64u;
const STACK_SIZE: u32 = 64u;

struct Frame {
    inv_view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    size: vec2<u32>,
    sample_base: u32,
    seed: u32,
    bounces: u32,
    light_count: u32,
    flags: u32,
    spp_this_pass: u32,
    sun_dir: vec4<f32>,
    sun_rgb: vec4<f32>,
    sky_zenith: vec4<f32>,
    sky_horizon: vec4<f32>,
    furnace: vec4<f32>,
    probe_count: u32,
    _p0: u32,
    _p1: u32,
    _p2: u32,
    env: vec4<u32>,
    env_aux: vec4<u32>,
    env_params: vec4<f32>,
    moon_dir: vec4<f32>,
    moon_rgb: vec4<f32>,
    clip_min: vec4<f32>,
    clip_max: vec4<f32>,
    view: vec4<f32>,
    tex: vec4<u32>,
    look: vec4<u32>,
};

struct Node { min: vec3<f32>, left_or_first: u32, max: vec3<f32>, count: u32 };
struct Tri { v0: vec3<f32>, material: u32, e1: vec3<f32>, object: u32, e2: vec3<f32>, flags: u32, n: vec3<f32>, _pad: u32 };
struct Material {
    albedo: vec3<f32>,
    metallic: f32,
    emission: vec3<f32>,
    roughness: f32,
    ior: f32,
    transmission: f32,
    flags: u32,
    _pad: u32,
};
struct Light {
    position: vec3<f32>,
    kind: u32,
    direction: vec3<f32>,
    cos_inner: f32,
    rgb: vec3<f32>,
    cos_outer: f32,
    axis: vec3<f32>,
    width: f32,
    length: f32,
    ies_offset: u32,
    ies_v: u32,
    ies_h: u32,
};
struct Probe { point: vec3<f32>, _a: f32, normal: vec3<f32>, _b: f32 };

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> nodes: array<Node>;
@group(0) @binding(2) var<storage, read> tris: array<Tri>;
@group(0) @binding(3) var<storage, read> materials: array<Material>;
@group(0) @binding(4) var<storage, read> lights: array<Light>;
// The per-still aux buffer: IES tables first, then data at the offsets the frame records.
@group(0) @binding(5) var<storage, read> ies: array<f32>;
// Per pixel (or per probe): the radiance sum in rgb, the sample count in w.
@group(0) @binding(6) var<storage, read_write> accum: array<vec4<f32>>;
// Per pixel, two entries: [2i] the first hit's albedo (w = 1 when hit), [2i + 1] its normal and
// distance. Written on a pixel's first sample only (sample_base == 0); the denoiser's guides.
@group(0) @binding(7) var<storage, read_write> aov: array<vec4<f32>>;
@group(0) @binding(8) var<storage, read> probes: array<Probe>;

// PCG hash (O'Neill): one 32-bit state per sample dimension, from the pixel, the sample index,
// the seed and the dimension, so a sample's numbers do not depend on how the passes were cut.
fn pcg(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

struct Rng { state: u32 };

fn rng_new(pixel: u32, sample: u32, seed: u32) -> Rng {
    var r: Rng;
    r.state = pcg(pixel ^ pcg(sample ^ pcg(seed + 0x9e3779b9u)));
    return r;
}

// Uniform in [0, 1).
fn rand(r: ptr<function, Rng>) -> f32 {
    (*r).state = pcg((*r).state);
    return f32((*r).state >> 8u) * (1.0 / 16777216.0);
}

// An orthonormal basis around the unit `n` (Duff et al. 2017); columns: tangent, bitangent, n.
fn onb(n: vec3<f32>) -> mat3x3<f32> {
    let s = select(-1.0, 1.0, n.z >= 0.0);
    let a = -1.0 / (s + n.z);
    let b = n.x * n.y * a;
    let t = vec3<f32>(1.0 + s * n.x * n.x * a, s * b, -s * n.x);
    let bt = vec3<f32>(b, s + n.y * n.y * a, -n.y);
    return mat3x3<f32>(t, bt, n);
}

// [p19-seeing-in-render] true when a hit at world point p is cut away by the clip box.
fn hit_clipped(p: vec3<f32>) -> bool {
    if ((frame.flags & FRAME_CLIP) == 0u) {
        return false;
    }
    return any(p < frame.clip_min.xyz) || any(p > frame.clip_max.xyz);
}
// [p19-seeing-in-render] the distance along a camera ray of direction d at which it starts (near clip).
fn camera_t_min(d: vec3<f32>) -> f32 {
    return frame.view.w / max(dot(d, frame.view.xyz), 1.0e-4);
}
