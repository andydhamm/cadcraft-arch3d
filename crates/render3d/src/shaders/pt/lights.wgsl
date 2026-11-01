// Path tracer: lights, sun and sky (stubs filled by p11-r3d-pt-lights).

// Light arriving at p from one light: along unit dir, from distance dist (PT_INF for the sun),
// radiance-like value li such that the contribution is bsdf_eval(...) * li / pdf; delta = 1 for
// a point, spot or sun (pdf 1, no MIS), 0 for an area light (pdf in solid angle).
struct LightSample { dir: vec3<f32>, dist: f32, li: vec3<f32>, pdf: f32, delta: u32 };

// Sample light i from p with two uniform numbers. Point: li = rgb / d²; spot: times the cone
// (smoothstep between cos_outer and cos_inner, as SceneLight::intensity_toward); IES: the table's
// candela instead (ies_candela); area: a uniform point on the rectangle, li = radiance · cos at
// the light, pdf = d² / (area · cos at the light); nothing behind the face.
fn light_sample(i: u32, p: vec3<f32>, u: vec2<f32>) -> LightSample {
    var s: LightSample;
    s.dir = vec3<f32>(0.0, 0.0, 1.0);
    s.dist = 0.0;
    s.li = vec3<f32>(0.0);
    s.pdf = 1.0;
    s.delta = 1u;
    return s;
}

// The sun toward frame.sun_dir: li = frame.sun_rgb (lux), delta, dist PT_INF; zero when FRAME_SUN
// is off.
fn sun_sample() -> LightSample {
    var s: LightSample;
    s.dir = frame.sun_dir.xyz;
    s.dist = PT_INF;
    s.li = vec3<f32>(0.0);
    s.pdf = 1.0;
    s.delta = 1u;
    return s;
}

// Radiance from the sky (or the furnace environment) along unit d; zero below the horizon and when
// neither FRAME_SKY nor FRAME_FURNACE is set.
fn sky_radiance(d: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(0.0);
}

// Candela of light l's IES table toward the unit direction to_point (from the light), bilinear,
// with the table's symmetry (arch::ies::candela_at); l.ies_v == 0: 0.
fn ies_candela(l: Light, to_point: vec3<f32>) -> f32 {
    return 0.0;
}

// An area light's radiance seen along unit d from a surface, when a BSDF-sampled ray of length t
// hits it (for MIS with light_sample): zero behind its face.
fn area_radiance(i: u32, d: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(0.0);
}
