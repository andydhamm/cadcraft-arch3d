// Phase 22: the lens warp. Each output pixel asks `frame_to_dir` (a port of
// `cadcraft_arch3d::lens::frame_to_dir`, in f32) for its view direction and samples the
// rectilinear source frame there. Outside the source it writes the clear colour.

struct Warp {
    // x: tan_h, y: tan_v of the output, z: tan_v of the source, w: shift.
    a: vec4<f32>,
    // x: 1 for Panini, y: compression, z/w: output width/height in pixels.
    b: vec4<f32>,
    // The clear colour (linear).
    clear: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Warp;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

fn frame_to_dir(n: vec2<f32>) -> vec3<f32> {
    let tan_h = u.a.x;
    let tan_v = u.a.y;
    let ny = n.y + 2.0 * u.a.w;
    if (u.b.x > 0.5) {
        let d = clamp(u.b.y, 0.0, 1.0e6);
        let pm = atan(tan_h);
        let k = (d + 1.0) / (d + cos(pm)) * sin(pm);
        let px = n.x * k;
        let py = ny * k * tan_v / tan_h;
        let disc = max((d + 1.0) * ((d + 1.0) - px * px * (d - 1.0)), 0.0);
        let t = px * (d + 1.0) / ((d + 1.0) + sqrt(disc));
        let phi = 2.0 * atan(t);
        let s = (d + 1.0) / (d + cos(phi));
        return vec3<f32>(sin(phi), py / s, cos(phi));
    }
    return vec3<f32>(n.x * tan_h, ny * tan_v, 1.0);
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let n = vec2<f32>(pos.x / u.b.z * 2.0 - 1.0, 1.0 - pos.y / u.b.w * 2.0);
    let v = frame_to_dir(n);
    if (v.z <= 1.0e-4) {
        return u.clear;
    }
    let sx = v.x / v.z / u.a.x;
    let sy = v.y / v.z / u.a.z;
    if (abs(sx) > 1.0 || abs(sy) > 1.0) {
        return u.clear;
    }
    let uv = vec2<f32>(sx * 0.5 + 0.5, 0.5 - sy * 0.5);
    return textureSampleLevel(src, samp, uv, 0.0);
}
