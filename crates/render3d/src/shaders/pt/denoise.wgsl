// Path tracer: one à-trous pass of the edge-aware denoiser (stub filled by p11-r3d-denoise).
// Pass k reads `src`, writes `dst` with a 5×5 B3-spline kernel at stride `step` (1, 2, 4, 8, 16),
// each tap weighted by its colour, normal and albedo distance to the centre (sigma_c, sigma_n,
// sigma_a); the guides come from the path tracer's AOVs (aov[2i] albedo, aov[2i + 1] normal).

struct DenoiseParams { size: vec2<u32>, step: u32, _pad: u32, sigma_c: f32, sigma_n: f32, sigma_a: f32, _pad2: f32 };

@group(0) @binding(0) var<uniform> dp: DenoiseParams;
@group(0) @binding(1) var<storage, read> src: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> guide: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> dst: array<vec4<f32>>;

@compute @workgroup_size(8, 8)
fn atrous_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dp.size.x || id.y >= dp.size.y) {
        return;
    }
    let i = id.y * dp.size.x + id.x;
    dst[i] = src[i];
}
