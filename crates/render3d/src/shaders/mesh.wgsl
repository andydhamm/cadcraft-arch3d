struct Uniforms {
    view_proj: mat4x4<f32>,   // offset 0,  64 bytes (column-major)
    light_dir: vec3<f32>,     // offset 64
    ambient:   f32,           // offset 76
    albedo:    vec3<f32>,     // offset 80
    _pad:      f32,           // offset 92  -> total 96 bytes
};
@group(0) @binding(0) var<uniform> u: Uniforms;

// Per-draw material and per-frame sun (group 1, dynamic offset), 128 bytes. All zero in the
// reserved fields (tone_map .. normal_strength, clip_min .. _p7) is the pre-phase-19 look.
struct PbrUniforms {
    light_dir:     vec3<f32>,   // offset 0, toward the sun
    sun_intensity: f32,         // offset 12
    ambient:       f32,         // offset 16
    metallic:      f32,         // offset 20
    roughness:     f32,         // offset 24
    alpha:         f32,         // offset 28
    style:         u32,         // offset 32: 0 shaded, 1 shaded-edges, 2 hidden-line, 3 glass-house
    tone_map:      u32,         // offset 36
    normal_layer:  u32,         // offset 40
    flags:         u32,         // offset 44: 1 lambert, 2 clip
    camera_pos:    vec3<f32>,   // offset 48
    shadow_min:    f32,         // offset 60
    shadow_max:    f32,         // offset 64
    exposure:      f32,         // offset 68
    ambient_lux:   f32,         // offset 72
    normal_strength: f32,       // offset 76
    _p6:           vec4<f32>,   // offset 80
    clip_min:      vec3<f32>,   // offset 96
    lambert_albedo: f32,        // offset 108
    clip_max:      vec3<f32>,   // offset 112
    _p7:           f32,         // offset 124 -> total 128 bytes
};
@group(1) @binding(1) var<uniform> p: PbrUniforms;

// The material textures (group 3): layer 0 is white, so an untextured draw is unchanged; 1..=4 the
// finishes; then the scene's textures (phase 14).
@group(3) @binding(0) var mat_tex: texture_2d_array<f32>;
@group(3) @binding(1) var mat_samp: sampler;

// Albedo of the draw at `uv`: the draw colour times its texture layer (`_p6.y`), the UVs divided by
// the draw's tile (`_p6.zw` holds 1 / tile; [1, 1] multiplies them by exactly one, and a zeroed
// block counts as [1, 1]). Samples, so call it from uniform control flow only.
fn base_albedo(uv: vec2<f32>) -> vec3<f32> {
    let per_repeat = select(vec2<f32>(1.0), p._p6.zw, p._p6.zw > vec2<f32>(0.0));
    let textured = u.albedo * textureSample(mat_tex, mat_samp, uv * per_repeat, i32(p._p6.y)).rgb;
    // The calibration look (PBR_FLAG_LAMBERT) ignores the material and its texture; the texture is
    // still sampled above so the control flow stays uniform.
    return select(textured, vec3<f32>(p.lambert_albedo), lambert_on());
}

// A physically scaled frame (phase 19): `tone_map` is non-zero. Zero is the unlit legacy look.
fn physical() -> bool {
    return p.tone_map != 0u;
}

// The plain Lambert calibration look of a physically scaled frame.
fn lambert_on() -> bool {
    return p.tone_map != 0u && (p.flags & 1u) != 0u;
}

const PI: f32 = 3.14159265;

// Tone curves: a copy of tonemap.wgsl (a test keeps the two identical).
fn tone_aces(x: vec3<f32>) -> vec3<f32> {
    let numerator = x * (2.51 * x + vec3<f32>(0.03));
    let denominator = x * (2.43 * x + vec3<f32>(0.59)) + vec3<f32>(0.14);
    return clamp(numerator / denominator, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tone_hable_curve(x: vec3<f32>) -> vec3<f32> {
    return (x * (0.15 * x + vec3<f32>(0.05)) + vec3<f32>(0.004)) / (x * (0.15 * x + vec3<f32>(0.5)) + vec3<f32>(0.06)) - vec3<f32>(0.02 / 0.3);
}

fn tone_hable(x: vec3<f32>) -> vec3<f32> {
    let white = tone_hable_curve(vec3<f32>(11.2));
    return clamp(tone_hable_curve(2.0 * x) / white, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tone_map_rgb(rgb: vec3<f32>, code: u32) -> vec3<f32> {
    let x = clamp(select(vec3<f32>(0.0), rgb, rgb == rgb), vec3<f32>(0.0), vec3<f32>(1.0e6));
    if (code == 1u) {
        return tone_aces(x);
    }
    if (code == 2u) {
        return tone_hable(x);
    }
    return clamp(x, vec3<f32>(0.0), vec3<f32>(1.0));
}
// Matches `offscreen::CLEAR`: hidden-line draws faces in the background colour.
const CLEAR_COLOR: vec3<f32> = vec3<f32>(0.02, 0.025, 0.034);
const GLASS_ALPHA: f32 = 0.35;

struct VIn  { @location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) uv: vec2<f32> };
struct VOut { @builtin(position) clip: vec4<f32>, @location(0) nrm: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) wpos: vec3<f32> };

@vertex fn vs(v: VIn) -> VOut {
    var o: VOut;
    o.clip = u.view_proj * vec4<f32>(v.pos, 1.0);
    o.nrm  = v.nrm;
    o.uv   = v.uv;
    o.wpos = v.pos;
    return o;
}

// GGX normal distribution.
fn distribution_ggx(ndh: f32, a: f32) -> f32 {
    let a2 = a * a;
    let d = ndh * ndh * (a2 - 1.0) + 1.0;
    return a2 / max(PI * d * d, 1e-6);
}

// Smith geometry term (Schlick-GGX form, direct lighting).
fn geometry_smith(ndv: f32, ndl: f32, rough: f32) -> f32 {
    let k = (rough + 1.0) * (rough + 1.0) / 8.0;
    let gv = ndv / (ndv * (1.0 - k) + k);
    let gl = ndl / (ndl * (1.0 - k) + k);
    return gv * gl;
}

// Schlick fresnel.
fn fresnel_schlick(vdh: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - vdh, 0.0, 1.0), 5.0);
}

// What the fragment stage computed before ambient and the final colour step: the sun's share
// (`direct`), the material at the fragment and the shading normal. `keep` = 1 when `direct` is a
// display colour that no lighting changes (hidden-line).
struct Shade {
    direct: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    n: vec3<f32>,
    alpha: f32,
    keep: u32,
};

// True when the draw has a clip box (flag 2) and the fragment lies outside it (the box is inclusive).
fn clip_discard(wpos: vec3<f32>) -> bool {
    if ((p.flags & 2u) == 0u) {
        return false;
    }
    return any(wpos < p.clip_min) || any(wpos > p.clip_max);
}

// Hook [p19-normal-maps-render]: the normal after the draw's normal map.
fn shading_normal(n: vec3<f32>, uv: vec2<f32>, wpos: vec3<f32>) -> vec3<f32> {
    return n;
}

// Irradiance the sun adds on a face square to it; it carries (1 - ambient) of the light so a white
// diffuse face at ndl = 1 is fully lit.
fn sun_irradiance() -> f32 {
    if (physical()) {
        // Lux: the frame's sun illuminance (0 without a sun).
        return p.sun_intensity;
    }
    return p.sun_intensity * (1.0 - p.ambient) * PI;
}

// The ambient term of the phase-6 look.
fn base_ambient(albedo: vec3<f32>, metallic: f32, n: vec3<f32>) -> vec3<f32> {
    if (physical()) {
        // Ambient illuminance in lux on a Lambert surface.
        return albedo / PI * p.ambient_lux * (1.0 - metallic * 0.5);
    }
    return albedo * p.ambient * (1.0 - metallic * 0.5);
}

// The last step on a lit colour: a physically scaled frame multiplies by its exposure and tone maps.
fn finish_color(rgb: vec3<f32>) -> vec3<f32> {
    if (physical()) {
        return tone_map_rgb(rgb * p.exposure, p.tone_map);
    }
    return rgb;
}

fn shade_parts(v: VOut) -> Shade {
    var s: Shade;
    if (clip_discard(v.wpos)) {
        discard;
    }
    // Flat shading from the triangle's own plane: cross(dpdy, dpdx) of the world position is the
    // face normal turned toward the camera. arch3d's boxes share 8 corners with averaged normals,
    // which interpolate to anything across a face (speckled walls, 2026-10-25), and its winding is
    // not consistent, so neither the vertex normal nor front_facing can be trusted here.
    let n = shading_normal(normalize(cross(dpdy(v.wpos), dpdx(v.wpos))), v.uv, v.wpos);

    // Hidden-line: faces in the clear colour, depth written; the edges come from arch3d::edges.
    if (p.style == 2u) {
        s.direct = CLEAR_COLOR;
        s.albedo = vec3<f32>(0.0);
        s.metallic = 0.0;
        s.n = n;
        s.alpha = 1.0;
        s.keep = 1u;
        return s;
    }

    let albedo = base_albedo(v.uv);
    let l = normalize(p.light_dir);
    var vdir = n;
    let to_eye = p.camera_pos - v.wpos;
    // Matte dielectrics keep a per-face constant view direction so a planar face is one colour
    // (the perspective view vector would otherwise shift the faint specular by a level or two).
    if (dot(to_eye, to_eye) > 1e-12 && (p.metallic > 0.01 || p.alpha < 1.0)) {
        vdir = normalize(to_eye);
    }
    let h = normalize(l + vdir);
    let ndl = max(dot(n, l), 0.0);
    let ndv = max(dot(n, vdir), 1e-4);
    let ndh = max(dot(n, h), 0.0);
    let vdh = max(dot(vdir, h), 0.0);

    let lam = lambert_on();
    let metallic = select(clamp(p.metallic, 0.0, 1.0), 0.0, lam);
    let rough = clamp(p.roughness, 0.045, 1.0);
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    let f = fresnel_schlick(vdh, f0);
    let d = distribution_ggx(ndh, rough * rough);
    let g = geometry_smith(ndv, ndl, rough);
    let spec = select(d * g * f / max(4.0 * ndv * ndl, 1e-4), vec3<f32>(0.0), lam);
    // Lambert diffuse for dielectrics; metals have none. The calibration look is pure Lambert.
    let kd = select((vec3<f32>(1.0) - f) * (1.0 - metallic), vec3<f32>(1.0), lam);
    let diffuse = kd * albedo / PI;

    s.direct = (diffuse + spec) * sun_irradiance() * ndl;
    s.albedo = albedo;
    s.metallic = metallic;
    s.n = n;
    s.alpha = clamp(p.alpha, 0.0, 1.0);
    s.keep = 0u;
    return s;
}

@fragment fn fs(v: VOut) -> @location(0) vec4<f32> {
    let s = shade_parts(v);
    if (s.keep != 0u) {
        return vec4<f32>(s.direct, s.alpha);
    }
    return vec4<f32>(finish_color(s.direct + base_ambient(s.albedo, s.metallic, s.n)), s.alpha);
}
