// Phase-10 fixture lighting. Appended after mesh.wgsl (cut before its fragment function, so
// shade_parts and the hook functions are in scope) and lights::LIGHT_MATH_WGSL.

struct FrameU {
    sun_vp: mat4x4<f32>,
    sun: vec4<f32>,
    params: vec4<f32>,
    ext: vec4<f32>,             // x ambient lux, y = 1 when `sh` is valid
    sh: array<vec4<f32>, 9>,    // second-order spherical harmonics, rgb in xyz
};
@group(2) @binding(0) var<uniform> fr: FrameU;
@group(2) @binding(1) var sun_map: texture_depth_2d;

struct GpuLight {
    pos_range: vec4<f32>,
    dir_cos_outer: vec4<f32>,
    color_cos_inner: vec4<f32>,
    shadow: vec4<f32>,
};
// One straight run of a strip light: a = start (w = candela per metre along n), b = end
// (w = number of equal pieces, each at most 0.15 m), n = the emitting normal.
struct StripSeg {
    a: vec4<f32>,
    b: vec4<f32>,
    n: vec4<f32>,
};
struct LightsU {
    count: vec4<u32>,
    lights: array<GpuLight, 64>,
    spot_vp: array<mat4x4<f32>, 64>,
    strip_segs: array<StripSeg, 128>,
};
@group(2) @binding(2) var<uniform> lu: LightsU;
@group(2) @binding(3) var atlas: texture_depth_2d_array;

const LIT_PI: f32 = 3.14159265358979323846;

// One when the sun reaches the fragment, zero when the stored sun depth occludes it.
fn lit_sun_visibility(wpos: vec3<f32>) -> f32 {
    let clip = fr.sun_vp * vec4<f32>(wpos, 1.0);
    if (clip.w <= 0.0) {
        return 1.0;
    }
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }
    let dimensions = vec2<i32>(textureDimensions(sun_map, 0));
    let maximum = max(dimensions - vec2<i32>(1), vec2<i32>(0));
    let texel = clamp(vec2<i32>(floor(uv * vec2<f32>(dimensions))), vec2<i32>(0), maximum);
    let stored = textureLoad(sun_map, texel, 0);
    return select(1.0, 0.0, ndc.z > stored + fr.sun.x);
}

// Spot maps store non-linear depth, but their comparison is made in forward metres.
fn lit_spot_shadow(i: u32, wpos: vec3<f32>) -> f32 {
    let light = lu.lights[i];
    let clip = lu.spot_vp[i] * vec4<f32>(wpos, 1.0);
    if (clip.w <= 0.0) {
        return 0.0;
    }
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return 0.0;
    }

    let size = max(i32(fr.params.w), 1);
    let centre = vec2<i32>(floor(uv * f32(size)));
    let layer = i32(light.shadow.x);
    let forward = dot(wpos - light.pos_range.xyz, light.dir_cos_outer.xyz);
    let radius = i32(clamp(fr.params.z, 0.0, 8.0));
    var visibility = 0.0;
    var samples = 0.0;
    for (var dy = -radius; dy <= radius; dy = dy + 1) {
        for (var dx = -radius; dx <= radius; dx = dx + 1) {
            let texel = clamp(
                centre + vec2<i32>(dx, dy),
                vec2<i32>(0),
                vec2<i32>(size - 1),
            );
            let stored = textureLoad(atlas, texel, layer, 0);
            let occluder = linear_depth(stored, light.shadow.z, light.pos_range.w);
            visibility = visibility + select(0.0, 1.0, forward <= occluder + light.shadow.w);
            samples = samples + 1.0;
        }
    }
    return visibility / max(samples, 1.0);
}

// A point light uses six consecutive atlas layers, selected by cube_uv_depth.
fn lit_cube_shadow(i: u32, delta: vec3<f32>) -> f32 {
    let light = lu.lights[i];
    let hit = cube_uv_depth(delta);
    let size = max(i32(fr.params.w), 1);
    let centre = vec2<i32>(floor(hit.uv * f32(size)));
    let layer = i32(light.shadow.x) + i32(hit.face);
    let radius = i32(clamp(fr.params.z, 0.0, 8.0));
    var visibility = 0.0;
    var samples = 0.0;
    for (var dy = -radius; dy <= radius; dy = dy + 1) {
        for (var dx = -radius; dx <= radius; dx = dx + 1) {
            let texel = clamp(
                centre + vec2<i32>(dx, dy),
                vec2<i32>(0),
                vec2<i32>(size - 1),
            );
            let stored = textureLoad(atlas, texel, layer, 0);
            let occluder = linear_depth(stored, light.shadow.z, light.pos_range.w);
            visibility = visibility + select(0.0, 1.0, hit.z <= occluder + light.shadow.w);
            samples = samples + 1.0;
        }
    }
    return visibility / max(samples, 1.0);
}

// Hook [p19-env-raster]: the ambient term of the lit pipelines.
fn lit_ambient(albedo: vec3<f32>, metallic: f32, n: vec3<f32>) -> vec3<f32> {
    return base_ambient(albedo, metallic, n);
}

// Hook [p19-exposure]: display units per lux of fixture light.
fn fixture_scale() -> f32 {
    if (physical()) {
        // Radiance in nits; the frame's exposure scales it in finish_color.
        return 1.0;
    }
    return fr.params.x;
}

// Hook [p19-strip-render]: the illuminance (lux) fixture `i` casts on a face with normal `n` at
// `wpos`: its cone, the cosine, the inverse square and its shadow.
fn fixture_light(i: u32, wpos: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let light = lu.lights[i];
    if (light.dir_cos_outer.w < -2.5) {
        // A strip light (marker -3): the sum of its pieces' midpoints, I x piece length x
        // max(0, n_e.-l) x max(0, n.l) / d^2, unshadowed (shadows are the path tracer's).
        var lux = 0.0;
        let first = u32(max(light.shadow.z, 0.0));
        let last = min(first + u32(max(light.shadow.w, 0.0)), 128u);
        for (var s = first; s < last; s = s + 1u) {
            let seg = lu.strip_segs[s];
            let pieces = clamp(u32(seg.b.w + 0.5), 1u, 1024u);
            let step = (seg.b.xyz - seg.a.xyz) / f32(pieces);
            let piece_len = length(step);
            for (var k = 0u; k < pieces; k = k + 1u) {
                let to_p = seg.a.xyz + step * (f32(k) + 0.5) - wpos;
                let d2 = max(dot(to_p, to_p), 1e-8);
                let l = to_p * inverseSqrt(d2);
                lux = lux + seg.a.w * piece_len * max(0.0, -dot(seg.n.xyz, l)) * max(0.0, dot(n, l)) / d2;
            }
        }
        return light.color_cos_inner.xyz * lux;
    }
    let to_l = light.pos_range.xyz - wpos;
    let d2 = max(dot(to_l, to_l), 1e-8);
    let l = to_l / sqrt(d2);
    let ndl = max(dot(n, l), 0.0);

    var cone = 1.0;
    if (light.dir_cos_outer.w >= -1.5) {
        let toward = dot(-l, light.dir_cos_outer.xyz);
        if (light.color_cos_inner.w <= light.dir_cos_outer.w + 1e-7) {
            cone = step(light.dir_cos_outer.w, toward);
        } else {
            cone = smoothstep(light.dir_cos_outer.w, light.color_cos_inner.w, toward);
        }
    }
    let illuminance = light.color_cos_inner.xyz * cone * ndl / d2;

    var shadow = 1.0;
    if (light.shadow.x >= 0.0) {
        if (light.shadow.y < 0.5) {
            shadow = lit_spot_shadow(i, wpos);
        } else {
            shadow = lit_cube_shadow(i, wpos - light.pos_range.xyz);
        }
    }
    return illuminance * shadow;
}

@fragment
fn fs(v: VOut) -> @location(0) vec4<f32> {
    let s = shade_parts(v);
    if (s.keep != 0u) {
        return vec4<f32>(s.direct, s.alpha);
    }
    let albedo = s.albedo;
    var direct = s.direct;

    if (fr.sun.y > 0.5 && fr.sun.z > 0.5 && lit_sun_visibility(v.wpos) < 0.5) {
        direct = vec3<f32>(0.0);
    }
    var rgb = direct + lit_ambient(albedo, s.metallic, s.n);

    let n = s.n;
    let n_l = select(n, -n, dot(n, p.camera_pos - v.wpos) < 0.0);
    let diffuse_albedo = albedo * (1.0 - s.metallic * 0.5);
    let light_count = min(lu.count.x, 64u);
    for (var i = 0u; i < light_count; i = i + 1u) {
        rgb = rgb + (diffuse_albedo / LIT_PI) * fixture_light(i, v.wpos, n_l) * fixture_scale();
    }

    // Emissive is a display value; in a physically scaled frame it is 100 nits per unit (what the
    // legacy exposure of 0.01 turns back into the same display value).
    rgb = rgb + albedo * p._p6.x * select(1.0, 100.0, physical());
    if (fr.params.y > 0.5 && !physical()) {
        let x = rgb;
        let numerator = x * (2.51 * x + vec3<f32>(0.03));
        let denominator = x * (2.43 * x + vec3<f32>(0.59)) + vec3<f32>(0.14);
        rgb = clamp(numerator / denominator, vec3<f32>(0.0), vec3<f32>(1.0));
    }
    return vec4<f32>(finish_color(rgb), s.alpha);
}
