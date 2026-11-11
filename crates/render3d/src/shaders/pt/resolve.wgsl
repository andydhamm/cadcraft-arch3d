// Path tracer: accumulation -> RGBA8 (complete). Average, exposure, the raster view's ACES tone
// map (lit.wgsl), sRGB encode, alpha 255; packed little-endian RGBA in one u32 per pixel.

struct ResolveParams { size: vec2<u32>, exposure: f32, tonemap: u32 };

@group(0) @binding(0) var<uniform> rp: ResolveParams;
@group(0) @binding(1) var<storage, read> radiance: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> pixels: array<u32>;

fn srgb(v: f32) -> f32 {
    let c = clamp(v, 0.0, 1.0);
    return select(1.055 * pow(c, 1.0 / 2.4) - 0.055, c * 12.92, c <= 0.0031308);
}

@compute @workgroup_size(8, 8)
fn resolve_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= rp.size.x || id.y >= rp.size.y) {
        return;
    }
    let i = id.y * rp.size.x + id.x;
    let a = radiance[i];
    var rgb = a.rgb / max(a.w, 1.0) * rp.exposure;
    rgb = select(vec3<f32>(0.0), rgb, rgb == rgb);
    // `tonemap` is the frame's tone code: 1 = ACES, anything else no curve (p19-exposure adds 2 Hable, 3 none).
    if (rp.tonemap == 1u) {
        rgb = clamp(rgb * (2.51 * rgb + vec3<f32>(0.03)) / (rgb * (2.43 * rgb + vec3<f32>(0.59)) + vec3<f32>(0.14)), vec3<f32>(0.0), vec3<f32>(1.0));
    }
    let r = u32(srgb(rgb.x) * 255.0 + 0.5);
    let g = u32(srgb(rgb.y) * 255.0 + 0.5);
    let b = u32(srgb(rgb.z) * 255.0 + 0.5);
    pixels[i] = r | (g << 8u) | (b << 16u) | (255u << 24u);
}
