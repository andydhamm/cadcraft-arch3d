// The learned denoiser's U-Net (phase 22; the network is assets/denoise/README.md). Activations are f32
// storage buffers in CHW order. One entry point per operation; all three share one bind group layout:
// 0 = parameters, 1 = every weight and bias of the net, 2 = input A, 3 = input B, 4 = output.
// Every dispatch is (ceil(width / 8), ceil(height / 8), z) with `width` x `height` the OUTPUT size.

struct Params {
    cin_a: u32,   // channels of input A (the upsampled tensor, first in the concatenation)
    cin_b: u32,   // channels of input B (0 when the input is A alone)
    cout: u32,    // output channels (conv), or channels to move (pool, upsample)
    width: u32,   // output width
    height: u32,  // output height
    w_off: u32,   // offset of the layer's weights (oihw) in `wts`
    b_off: u32,   // offset of the layer's bias in `wts`
    pad: u32,
}

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read> wts: array<f32>;
@group(0) @binding(2) var<storage, read> in_a: array<f32>;
@group(0) @binding(3) var<storage, read> in_b: array<f32>;
@group(0) @binding(4) var<storage, read_write> outb: array<f32>;

// 3 x 3 convolution, zero padding 1, bias, ReLU. z = group of four output channels.
@compute @workgroup_size(8, 8, 1)
fn conv_main(@builtin(global_invocation_id) g: vec3<u32>) {
    if (g.x >= p.width || g.y >= p.height) {
        return;
    }
    let cin = p.cin_a + p.cin_b;
    let plane = p.width * p.height;
    let co0 = g.z * 4u;
    let last = p.cout - 1u;
    let c0 = min(co0, last);
    let c1 = min(co0 + 1u, last);
    let c2 = min(co0 + 2u, last);
    let c3 = min(co0 + 3u, last);
    var acc = vec4<f32>(wts[p.b_off + c0], wts[p.b_off + c1], wts[p.b_off + c2], wts[p.b_off + c3]);
    let stride = cin * 9u;
    for (var ci = 0u; ci < cin; ci = ci + 1u) {
        let wbase = p.w_off + ci * 9u;
        for (var dy = 0u; dy < 3u; dy = dy + 1u) {
            let yy = i32(g.y) + i32(dy) - 1;
            if (yy < 0 || yy >= i32(p.height)) {
                continue;
            }
            for (var dx = 0u; dx < 3u; dx = dx + 1u) {
                let xx = i32(g.x) + i32(dx) - 1;
                if (xx < 0 || xx >= i32(p.width)) {
                    continue;
                }
                let idx = u32(yy) * p.width + u32(xx);
                var v: f32;
                if (ci < p.cin_a) {
                    v = in_a[ci * plane + idx];
                } else {
                    v = in_b[(ci - p.cin_a) * plane + idx];
                }
                let t = dy * 3u + dx;
                acc = acc + v * vec4<f32>(
                    wts[wbase + c0 * stride + t],
                    wts[wbase + c1 * stride + t],
                    wts[wbase + c2 * stride + t],
                    wts[wbase + c3 * stride + t],
                );
            }
        }
    }
    acc = max(acc, vec4<f32>(0.0));
    let o = g.y * p.width + g.x;
    outb[co0 * plane + o] = acc.x;
    if (co0 + 1u < p.cout) {
        outb[(co0 + 1u) * plane + o] = acc.y;
    }
    if (co0 + 2u < p.cout) {
        outb[(co0 + 2u) * plane + o] = acc.z;
    }
    if (co0 + 3u < p.cout) {
        outb[(co0 + 3u) * plane + o] = acc.w;
    }
}

// 2 x 2 max pool, stride 2. z = channel; the input is 2 * width by 2 * height.
@compute @workgroup_size(8, 8, 1)
fn pool_main(@builtin(global_invocation_id) g: vec3<u32>) {
    if (g.x >= p.width || g.y >= p.height || g.z >= p.cout) {
        return;
    }
    let iw = p.width * 2u;
    let base = g.z * iw * p.height * 2u + g.y * 2u * iw + g.x * 2u;
    let m = max(max(in_a[base], in_a[base + 1u]), max(in_a[base + iw], in_a[base + iw + 1u]));
    outb[g.z * p.width * p.height + g.y * p.width + g.x] = m;
}

// Nearest-neighbour x 2. z = channel; the input is width / 2 by height / 2.
@compute @workgroup_size(8, 8, 1)
fn upsample_main(@builtin(global_invocation_id) g: vec3<u32>) {
    if (g.x >= p.width || g.y >= p.height || g.z >= p.cout) {
        return;
    }
    let iw = p.width / 2u;
    let ih = p.height / 2u;
    outb[g.z * p.width * p.height + g.y * p.width + g.x] = in_a[g.z * iw * ih + (g.y / 2u) * iw + g.x / 2u];
}
