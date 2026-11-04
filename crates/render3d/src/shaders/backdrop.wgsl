// Backdrop: a fullscreen triangle drawn first, no depth write. Values are sRGB bytes written as
// they are (the sRGB target encodes the linear value this shader returns).

struct Backdrop {
    inv_view_proj: mat4x4<f32>,
    top: vec4<f32>,      // sRGB 0..255
    bottom: vec4<f32>,   // sRGB 0..255
    size: vec4<f32>,     // target w, h, image w, h
    kind: vec4<u32>,     // 0 colour, 1 gradient, 2 flat image, 3 equirect image
};

@group(0) @binding(0) var<uniform> u: Backdrop;
@group(0) @binding(1) var img: texture_2d<f32>;

const PI: f32 = 3.14159265358979;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    return vec4<f32>(x, y, 1.0, 1.0);
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let s = c / 255.0;
    let lo = s / 12.92;
    let hi = pow((s + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, s <= vec3<f32>(0.04045));
}

fn texel(ix: i32, iy: i32) -> vec3<f32> {
    return textureLoad(img, vec2<i32>(ix, iy), 0).rgb * 255.0;
}

// Bilinear in sRGB bytes; x clamps or wraps, y clamps.
fn sample_img(uv: vec2<f32>, wrap_x: bool) -> vec3<f32> {
    let iw = i32(u.size.z);
    let ih = i32(u.size.w);
    let fx = uv.x * u.size.z - 0.5;
    let fy = uv.y * u.size.w - 0.5;
    let x0f = floor(fx);
    let y0f = floor(fy);
    let tx = fx - x0f;
    let ty = fy - y0f;
    var x0 = i32(x0f);
    var x1 = x0 + 1;
    var y0 = i32(y0f);
    var y1 = y0 + 1;
    if (wrap_x) {
        x0 = ((x0 % iw) + iw) % iw;
        x1 = ((x1 % iw) + iw) % iw;
    } else {
        x0 = clamp(x0, 0, iw - 1);
        x1 = clamp(x1, 0, iw - 1);
    }
    y0 = clamp(y0, 0, ih - 1);
    y1 = clamp(y1, 0, ih - 1);
    let a = mix(texel(x0, y0), texel(x1, y0), tx);
    let b = mix(texel(x0, y1), texel(x1, y1), tx);
    return mix(a, b, ty);
}

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    var c = u.top.rgb;
    let k = u.kind.x;
    if (k == 1u) {
        let t = frag.y / u.size.y;
        c = round(mix(u.top.rgb, u.bottom.rgb, t));
    } else if (k == 2u) {
        c = sample_img(vec2<f32>(frag.x / u.size.x, frag.y / u.size.y), false);
    } else if (k == 3u) {
        let nx = frag.x / u.size.x * 2.0 - 1.0;
        let ny = 1.0 - frag.y / u.size.y * 2.0;
        let a = u.inv_view_proj * vec4<f32>(nx, ny, 0.3, 1.0);
        let b = u.inv_view_proj * vec4<f32>(nx, ny, 0.9, 1.0);
        let d = normalize(b.xyz / b.w - a.xyz / a.w);
        let uu = 0.5 + atan2(d.y, d.x) / (2.0 * PI);
        let vv = 0.5 - asin(clamp(d.z, -1.0, 1.0)) / PI;
        c = sample_img(vec2<f32>(uu, vv), true);
    }
    return vec4<f32>(to_linear(clamp(c, vec3<f32>(0.0), vec3<f32>(255.0))), 1.0);
}
