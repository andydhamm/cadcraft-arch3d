// Path tracer: one à-trous pass of the edge-aware denoiser.
// Pass k reads `src`, writes `dst` with a 5×5 B3-spline kernel at stride `step` (1, 2, 4, 8, 16),
// each tap weighted by its colour, normal and albedo distance to the centre (sigma_c, sigma_n,
// sigma_a); the guides come from the path tracer's AOVs (guide[2i] albedo, guide[2i + 1] normal).
// A pixel whose albedo guide has w = 1 is a hit; taps without a hit are skipped and a pixel
// without a hit is copied.

struct DenoiseParams { size: vec2<u32>, step: u32, _pad: u32, sigma_c: f32, sigma_n: f32, sigma_a: f32, _pad2: f32 };

@group(0) @binding(0) var<uniform> dp: DenoiseParams;
@group(0) @binding(1) var<storage, read> src: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> guide: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> dst: array<vec4<f32>>;

// The B3 spline's 1-D kernel (1/16, 1/4, 3/8, 1/4, 1/16) at tap offset `k` in -2..=2.
fn b3(k: i32) -> f32 {
    let a = abs(k);
    if (a == 0) {
        return 0.375;
    }
    if (a == 1) {
        return 0.25;
    }
    return 0.0625;
}

// Rec. 709 luminance.
fn lum(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn is_hit(i: u32) -> bool {
    return guide[2u * i].w > 0.5;
}

@compute @workgroup_size(8, 8)
fn atrous_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dp.size.x || id.y >= dp.size.y) {
        return;
    }
    let i = id.y * dp.size.x + id.x;
    let c0 = src[i];
    if (!is_hit(i)) {
        dst[i] = c0;
        return;
    }
    let rgb0 = c0.rgb;
    let a0 = guide[2u * i].rgb;
    let n0 = guide[2u * i + 1u].xyz;
    let lum0 = lum(rgb0) + 1.0e-3;
    let denom_c = dp.sigma_c * dp.sigma_c * lum0 * lum0 + 1.0e-8;
    let denom_a = dp.sigma_a * dp.sigma_a + 1.0e-8;
    let w = i32(dp.size.x);
    let h = i32(dp.size.y);
    let stride = i32(dp.step);
    let cx = i32(id.x);
    let cy = i32(id.y);

    var sum = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var dy = -2; dy <= 2; dy = dy + 1) {
        let y = cy + dy * stride;
        if (y < 0 || y >= h) {
            continue;
        }
        for (var dx = -2; dx <= 2; dx = dx + 1) {
            let x = cx + dx * stride;
            if (x < 0 || x >= w) {
                continue;
            }
            let j = u32(y) * dp.size.x + u32(x);
            if (!is_hit(j)) {
                continue;
            }
            let c = src[j].rgb;
            let dc = c - rgb0;
            let da = guide[2u * j].rgb - a0;
            let cosn = dot(guide[2u * j + 1u].xyz, n0);
            let wn = select(0.0, pow(cosn, dp.sigma_n), cosn > 0.0);
            let weight = b3(dx) * b3(dy) * exp(-dot(dc, dc) / denom_c) * wn * exp(-dot(da, da) / denom_a);
            sum = sum + weight * c;
            wsum = wsum + weight;
        }
    }
    // The centre tap alone weighs b3(0)² > 0, but a degenerate (zero or non-finite) normal can
    // zero it; then the pixel has nothing to average with and stays as it is.
    if (wsum > 0.0) {
        dst[i] = vec4<f32>(sum / wsum, 1.0);
    } else {
        dst[i] = vec4<f32>(rgb0, 1.0);
    }
}
