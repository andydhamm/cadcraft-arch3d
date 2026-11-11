// Tone curves of phase 19 (render3d::exposure::tone_map is the CPU twin). `tone_map_rgb` takes
// the exposed linear colour and the frame's tone code: 1 ACES, 2 Hable, 3 none (clamp 0..1);
// anything else is none too. This text is also inside mesh.wgsl, which the unlit and shadowed
// pipelines compile on their own; a test keeps the two identical.

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
