//! The learned denoiser's U-Net on the CPU: input transforms, autoexposure, the PU transfer function and a
//! slow reference forward pass. The specification is `assets/denoise/README.md`; the GPU version is tested
//! against [`forward_cpu`].

use crate::tza;

/// One 3 x 3 convolution of the denoiser.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// Tensor name prefix, e.g. `enc_conv0`.
    pub name: &'static str,
    /// Input channels.
    pub cin: u32,
    /// Output channels.
    pub cout: u32,
    /// Weights in `oihw` order (`cout * cin * 3 * 3`).
    pub weights: Vec<f32>,
    /// One bias per output channel.
    pub bias: Vec<f32>,
}

/// The denoiser U-Net: 16 convolutions in the README's order.
#[derive(Clone, Debug, PartialEq)]
pub struct Net {
    /// The convolutions, `enc_conv0` first and `dec_conv0` last.
    pub layers: Vec<Layer>,
}

const LAYER_NAMES: [&str; 16] = [
    "enc_conv0",
    "enc_conv1",
    "enc_conv2",
    "enc_conv3",
    "enc_conv4",
    "enc_conv5a",
    "enc_conv5b",
    "dec_conv4a",
    "dec_conv4b",
    "dec_conv3a",
    "dec_conv3b",
    "dec_conv2a",
    "dec_conv2b",
    "dec_conv1a",
    "dec_conv1b",
    "dec_conv0",
];

impl Net {
    /// Build the network from a parsed `.tza` model. Channel counts are read from the tensor dimensions and
    /// cross-checked against the README's wiring (including the concatenations); a mismatch is an error.
    pub fn from_model(m: &tza::Model) -> Result<Net, String> {
        let mut layers = Vec::with_capacity(LAYER_NAMES.len());
        for name in LAYER_NAMES {
            let wname = format!("{name}.weight");
            let bname = format!("{name}.bias");
            let w = m.tensor(&wname).ok_or_else(|| format!("missing tensor {wname}"))?;
            let b = m.tensor(&bname).ok_or_else(|| format!("missing tensor {bname}"))?;
            if w.layout != "oihw" || w.dims.len() != 4 {
                return Err(format!("tensor {wname} is not a 4-d oihw weight"));
            }
            if w.dims.get(2) != Some(&3) || w.dims.get(3) != Some(&3) {
                return Err(format!("tensor {wname} is not 3 x 3"));
            }
            let cout = w.dims.first().copied().unwrap_or(0);
            let cin = w.dims.get(1).copied().unwrap_or(0);
            if b.dims.len() != 1 || b.dims.first() != Some(&cout) {
                return Err(format!("tensor {bname} does not match {wname}"));
            }
            let layer = Layer { name, cin, cout, weights: w.data.clone(), bias: b.data.clone() };
            check_layer(&layer)?;
            layers.push(layer);
        }
        let net = Net { layers };
        net.check_wiring()?;
        Ok(net)
    }

    fn check_wiring(&self) -> Result<(), String> {
        if self.layers.len() != LAYER_NAMES.len() {
            return Err(format!("net has {} layers, expected {}", self.layers.len(), LAYER_NAMES.len()));
        }
        let c = |i: usize| self.layers.get(i).map(|l| l.cout).unwrap_or(0);
        // (layer index, expected input channels).
        let expect: [(usize, u32); 16] = [
            (0, 9),
            (1, c(0)),
            (2, c(1)),
            (3, c(2)),
            (4, c(3)),
            (5, c(4)),
            (6, c(5)),
            (7, c(6) + c(3)),
            (8, c(7)),
            (9, c(8) + c(2)),
            (10, c(9)),
            (11, c(10) + c(1)),
            (12, c(11)),
            (13, c(12) + 9),
            (14, c(13)),
            (15, c(14)),
        ];
        for (i, cin) in expect {
            let l = self.layers.get(i).ok_or("missing layer")?;
            if l.cin != cin {
                return Err(format!("layer {} takes {} channels, the wiring gives {cin}", l.name, l.cin));
            }
        }
        if c(15) != 3 {
            return Err(format!("dec_conv0 has {} outputs, expected 3", c(15)));
        }
        Ok(())
    }
}

const PU_A: f32 = 1.412_837_6e3;
const PU_B: f32 = 1.645_931_7;
const PU_C: f32 = 4.313_849_8e-1;
const PU_D: f32 = -2.941_396_1e-3;
const PU_E: f32 = 1.926_532_5e-1;
const PU_F: f32 = 6.260_261e-3;
const PU_G: f32 = 9.986_201_5e-1;
const PU_Y0: f32 = 1.579_457_6e-6;
const PU_Y1: f32 = 3.220_876_3e-2;
const PU_X0: f32 = 2.231_517_1e-3;
const PU_X1: f32 = 3.709_747_5e-1;

/// The PU transfer function (linear light to perceptual). Non-finite or non-positive input gives 0.
pub fn pu(y: f32) -> f32 {
    if !y.is_finite() || y <= 0.0 {
        0.0
    } else if y <= PU_Y0 {
        PU_A * y
    } else if y <= PU_Y1 {
        PU_B * y.powf(PU_C) + PU_D
    } else {
        PU_E * (y + PU_F).ln() + PU_G
    }
}

/// The inverse of [`pu`]. Non-finite or non-positive input gives 0.
pub fn pu_inv(x: f32) -> f32 {
    if !x.is_finite() || x <= 0.0 {
        0.0
    } else if x <= PU_X0 {
        x / PU_A
    } else if x <= PU_X1 {
        ((x - PU_D) / PU_B).powf(1.0 / PU_C)
    } else {
        ((x - PU_G) / PU_E).exp() - PU_F
    }
}

/// `1 / PU(65504)`, the scale that maps the largest half float to 1 (checked by a test).
pub const NORM_SCALE: f32 = 0.318_967;

fn sanitize(v: f32) -> f32 {
    if v.is_finite() { v.max(0.0) } else { 0.0 }
}

/// Autoexposure scale: bins of at most 16 x 16 pixels, mean luminance per bin, mean log2 over the bins above
/// 1e-8, `0.18 / 2^mean`; 1.0 when no bin qualifies.
pub fn autoexposure(color: &[[f32; 3]], size: (u32, u32)) -> f32 {
    let (w, h) = (size.0 as usize, size.1 as usize);
    if w == 0 || h == 0 || color.len() < w.saturating_mul(h) {
        return 1.0;
    }
    let bins_x = w.div_ceil(16);
    let bins_y = h.div_ceil(16);
    let mut total = 0.0_f64;
    let mut count = 0usize;
    for by in 0..bins_y {
        let (y0, y1) = (by * h / bins_y, (by + 1) * h / bins_y);
        for bx in 0..bins_x {
            let (x0, x1) = (bx * w / bins_x, (bx + 1) * w / bins_x);
            let mut sum = 0.0_f64;
            let mut n = 0usize;
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = color.get(y * w + x).copied().unwrap_or([0.0; 3]);
                    let l = 0.212671 * sanitize(p[0]) + 0.715160 * sanitize(p[1]) + 0.072169 * sanitize(p[2]);
                    sum += f64::from(l);
                    n += 1;
                }
            }
            if n == 0 {
                continue;
            }
            let mean = sum / n as f64;
            if mean > 1e-8 {
                total += mean.log2();
                count += 1;
            }
        }
    }
    if count == 0 {
        return 1.0;
    }
    (0.18 / (total / count as f64).exp2()) as f32
}

/// Encode colour, albedo and normal into the network's 9-channel CHW input, padded with zeros to multiples of
/// 16. Returns the tensor and the padded `(width, height)`.
pub fn encode_input(color: &[[f32; 3]], albedo: &[[f32; 3]], normal: &[[f32; 3]], size: (u32, u32), scale: f32) -> (Vec<f32>, (u32, u32)) {
    let (w, h) = (size.0 as usize, size.1 as usize);
    let (pw, ph) = (w.div_ceil(16) * 16, h.div_ceil(16) * 16);
    let plane = pw * ph;
    let mut out = vec![0.0_f32; 9 * plane];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let c = color.get(i).copied().unwrap_or([0.0; 3]);
            let a = albedo.get(i).copied().unwrap_or([0.0; 3]);
            let n = normal.get(i).copied().unwrap_or([0.0; 3]);
            let o = y * pw + x;
            for k in 0..3 {
                let cv = pu(sanitize(c[k]) * scale) * NORM_SCALE;
                let av = if a[k].is_nan() { 0.0 } else { a[k].clamp(0.0, 1.0) };
                let nv = if n[k].is_nan() { 0.0 } else { n[k].clamp(-1.0, 1.0) } * 0.5 + 0.5;
                for (ch, v) in [(k, cv), (3 + k, av), (6 + k, nv)] {
                    if let Some(slot) = out.get_mut(ch * plane + o) {
                        *slot = v;
                    }
                }
            }
        }
    }
    (out, (pw as u32, ph as u32))
}

/// Decode the network output (3 CHW channels at the padded size) back to linear colour at `size`.
pub fn decode_output(out: &[f32], padded: (u32, u32), size: (u32, u32), scale: f32) -> Vec<[f32; 3]> {
    let (pw, ph) = (padded.0 as usize, padded.1 as usize);
    let (w, h) = (size.0 as usize, size.1 as usize);
    let plane = pw * ph;
    let inv_scale = if scale.is_finite() && scale > 0.0 { 1.0 / scale } else { 1.0 };
    let mut res = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let mut p = [0.0_f32; 3];
            for (k, v) in p.iter_mut().enumerate() {
                let raw = out.get(k * plane + y * pw + x).copied().unwrap_or(0.0);
                let raw = if raw.is_nan() { 0.0 } else { raw.max(0.0) };
                *v = pu_inv(raw / NORM_SCALE) * inv_scale;
            }
            res.push(p);
        }
    }
    res
}

/// Check a layer's buffers against its channel counts.
fn check_layer(layer: &Layer) -> Result<(), String> {
    let (cin, cout) = (layer.cin as usize, layer.cout as usize);
    let want = cout.checked_mul(cin).and_then(|n| n.checked_mul(9)).ok_or("layer too large")?;
    if layer.weights.len() != want || layer.bias.len() != cout {
        return Err(format!("layer {} has inconsistent buffer sizes", layer.name));
    }
    Ok(())
}

/// 3 x 3 convolution, zero padding 1, stride 1, then ReLU.
fn conv3x3(input: &[f32], h: usize, w: usize, layer: &Layer) -> Result<Vec<f32>, String> {
    let (cin, cout) = (layer.cin as usize, layer.cout as usize);
    let plane = h * w;
    if input.len() != cin * plane {
        return Err(format!("layer {} expects {} values, got {}", layer.name, cin * plane, input.len()));
    }
    let mut out = vec![0.0_f32; cout * plane];
    for (oc, o) in out.chunks_exact_mut(plane).enumerate() {
        o.fill(layer.bias.get(oc).copied().unwrap_or(0.0));
        for (ic, src) in input.chunks_exact(plane).enumerate() {
            for ky in 0..3usize {
                for kx in 0..3usize {
                    let wt = layer.weights.get((oc * cin + ic) * 9 + ky * 3 + kx).copied().unwrap_or(0.0);
                    if wt == 0.0 {
                        continue;
                    }
                    // Output pixel (y, x) reads input (y + ky - 1, x + kx - 1).
                    let (y_lo, y_hi) = (1usize.saturating_sub(ky), if ky == 2 { h.saturating_sub(1) } else { h });
                    let (x_lo, x_hi) = (1usize.saturating_sub(kx), if kx == 2 { w.saturating_sub(1) } else { w });
                    for y in y_lo..y_hi {
                        let sy = y + ky - 1;
                        let orow = o.get_mut(y * w + x_lo..y * w + x_hi);
                        let srow = src.get(sy * w + x_lo + kx - 1..sy * w + x_hi + kx - 1);
                        if let (Some(orow), Some(srow)) = (orow, srow) {
                            for (d, s) in orow.iter_mut().zip(srow) {
                                *d += wt * s;
                            }
                        }
                    }
                }
            }
        }
        for v in o.iter_mut() {
            *v = v.max(0.0);
        }
    }
    Ok(out)
}

/// 2 x 2 max pool, stride 2 (dimensions are even).
fn maxpool2(data: &[f32], h: usize, w: usize) -> Vec<f32> {
    let (oh, ow) = (h / 2, w / 2);
    let plane = (h * w).max(1);
    let mut out = Vec::with_capacity(data.len() / 4);
    for chan in data.chunks_exact(plane) {
        for y in 0..oh {
            for x in 0..ow {
                let mut best = f32::NEG_INFINITY;
                for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                    best = best.max(chan.get((2 * y + dy) * w + 2 * x + dx).copied().unwrap_or(f32::NEG_INFINITY));
                }
                out.push(best);
            }
        }
    }
    out
}

/// Nearest-neighbour upsample x 2.
fn upsample2(data: &[f32], h: usize, w: usize) -> Vec<f32> {
    let plane = (h * w).max(1);
    let mut out = Vec::with_capacity(data.len() * 4);
    for chan in data.chunks_exact(plane) {
        for y in 0..h * 2 {
            for x in 0..w * 2 {
                out.push(chan.get((y / 2) * w + x / 2).copied().unwrap_or(0.0));
            }
        }
    }
    out
}

fn concat(a: &[f32], b: &[f32]) -> Vec<f32> {
    let mut v = Vec::with_capacity(a.len() + b.len());
    v.extend_from_slice(a);
    v.extend_from_slice(b);
    v
}

/// Run the reference forward pass on a 9-channel CHW tensor whose sides are multiples of 16 and at most 256.
/// Returns 3 CHW channels at the padded size. ReLU follows every convolution, the last included. It is slow by
/// design: it is the reference the GPU version is tested against.
pub fn forward_cpu(net: &Net, input: &[f32], padded: (u32, u32)) -> Result<Vec<f32>, String> {
    let (pw, ph) = padded;
    if pw > 256 || ph > 256 {
        return Err(format!("padded size {pw}x{ph} is above the 256x256 CPU limit"));
    }
    if pw == 0 || ph == 0 || pw % 16 != 0 || ph % 16 != 0 {
        return Err(format!("padded size {pw}x{ph} is not a positive multiple of 16"));
    }
    let (w, h) = (pw as usize, ph as usize);
    if input.len() != 9 * w * h {
        return Err(format!("expected {} input values, got {}", 9 * w * h, input.len()));
    }
    net.check_wiring()?;
    for l in &net.layers {
        check_layer(l)?;
    }
    let ly = |i: usize| net.layers.get(i).ok_or_else(|| "missing layer".to_string());

    let x = conv3x3(input, h, w, ly(0)?)?;
    let e1 = conv3x3(&x, h, w, ly(1)?)?;
    let p1 = maxpool2(&e1, h, w);
    let e2 = conv3x3(&p1, h / 2, w / 2, ly(2)?)?;
    let p2 = maxpool2(&e2, h / 2, w / 2);
    let e3 = conv3x3(&p2, h / 4, w / 4, ly(3)?)?;
    let p3 = maxpool2(&e3, h / 4, w / 4);
    let e4 = conv3x3(&p3, h / 8, w / 8, ly(4)?)?;
    let p4 = maxpool2(&e4, h / 8, w / 8);
    let x = conv3x3(&p4, h / 16, w / 16, ly(5)?)?;
    let mut x = conv3x3(&x, h / 16, w / 16, ly(6)?)?;

    // (skip tensor, divisor of the stage's resolution, first layer index).
    let stages: [(&[f32], usize, usize); 4] = [(&p3, 8, 7), (&p2, 4, 9), (&p1, 2, 11), (input, 1, 13)];
    for (skip, div, first) in stages {
        let (sh, sw) = (h / div, w / div);
        let cat = concat(&upsample2(&x, sh / 2, sw / 2), skip);
        let t = conv3x3(&cat, sh, sw, ly(first)?)?;
        x = conv3x3(&t, sh, sw, ly(first + 1)?)?;
    }
    conv3x3(&x, h, w, ly(15)?)
}

/// Denoise an HDR image on the CPU: autoexposure, encode, [`forward_cpu`], decode.
pub fn denoise_cpu(net: &Net, color: &[[f32; 3]], albedo: &[[f32; 3]], normal: &[[f32; 3]], size: (u32, u32)) -> Result<Vec<[f32; 3]>, String> {
    let pixels = (size.0 as usize).checked_mul(size.1 as usize).ok_or("image too large")?;
    if pixels == 0 {
        return Ok(Vec::new());
    }
    if color.len() != pixels || albedo.len() != pixels || normal.len() != pixels {
        return Err("colour, albedo and normal must each have width x height pixels".to_string());
    }
    let scale = autoexposure(color, size);
    let (input, padded) = encode_input(color, albedo, normal, size, scale);
    let out = forward_cpu(net, &input, padded)?;
    Ok(decode_output(&out, padded, size, scale))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tza::{ModelChoice, model};

    fn net(which: ModelChoice) -> Net {
        let m = model(which).expect("model parses");
        Net::from_model(m).expect("net builds")
    }

    #[test]
    fn pu_round_trips() {
        let mut y = 1e-6_f32;
        while y < 6e4 {
            let back = pu_inv(pu(y));
            assert!(((back - y) / y).abs() <= 1e-4, "PU round trip at {y}: {back}");
            y *= 1.7;
        }
        let want = 1.0 / pu(65504.0);
        assert!(((NORM_SCALE - want) / want).abs() < 1e-5, "NORM_SCALE {NORM_SCALE} vs {want}");
    }

    #[test]
    fn autoexposure_of_mid_grey_is_one() {
        let img = vec![[0.18_f32; 3]; 40 * 20];
        assert!((autoexposure(&img, (40, 20)) - 1.0).abs() < 1e-4);
        assert_eq!(autoexposure(&vec![[0.0; 3]; 16], (4, 4)), 1.0);
    }

    #[test]
    fn input_is_padded_to_multiples_of_sixteen() {
        let n = 100 * 50;
        let (input, padded) = encode_input(&vec![[0.1; 3]; n], &vec![[0.5; 3]; n], &vec![[0.0, 0.0, 1.0]; n], (100, 50), 1.0);
        assert_eq!(padded, (112, 64));
        assert_eq!(input.len(), 9 * 112 * 64);
        // Padding stays zero; the normal's z channel is 1.0 inside the image.
        assert_eq!(input.get(8 * 112 * 64 + 99), Some(&1.0));
        assert_eq!(input.get(8 * 112 * 64 + 100), Some(&0.0));
    }

    #[test]
    fn net_shapes_follow_the_readme() {
        let base = net(ModelChoice::Base);
        let dims: Vec<(u32, u32)> = base.layers.iter().map(|l| (l.cin, l.cout)).collect();
        assert_eq!(dims[0], (9, 32));
        assert_eq!(dims[7], (160, 112));
        assert_eq!(dims[13], (73, 64));
        assert_eq!(dims[15], (32, 3));
        let small = net(ModelChoice::Small);
        assert_eq!(small.layers.len(), 16);
        assert_eq!(small.layers[7].cin, 64);
        assert_eq!(small.layers[13].cin, 41);
        // A broken wiring is an error.
        let mut bad = base.clone();
        bad.layers[7].cin = 159;
        assert!(bad.check_wiring().is_err());
        // The size cap is an error, not a panic.
        assert!(forward_cpu(&base, &[], (272, 16)).is_err());
    }

    #[test]
    fn flat_grey_survives_the_network() {
        for which in [ModelChoice::Base, ModelChoice::Small] {
            let net = net(which);
            let n = 32 * 32;
            let out = denoise_cpu(&net, &vec![[0.5; 3]; n], &vec![[0.5; 3]; n], &vec![[0.0, 0.0, 1.0]; n], (32, 32)).expect("denoise");
            // Zero padding at the image border legitimately perturbs the outermost pixels, so check the
            // mean over the image and every pixel at least 4 away from the border.
            assert!(out.iter().flatten().all(|v| v.is_finite()));
            let mean = out.iter().flatten().sum::<f32>() / (3 * n) as f32;
            assert!((mean - 0.5).abs() <= 0.005, "{which:?}: mean {mean}");
            for (i, p) in out.iter().enumerate() {
                let (x, y) = (i % 32, i / 32);
                if (4..28).contains(&x) && (4..28).contains(&y) {
                    for v in p {
                        assert!((v - 0.5).abs() <= 0.025, "{which:?}: {v} at {x},{y}");
                    }
                }
            }
        }
    }

    #[test]
    fn noisy_gradient_is_denoised_on_the_cpu() {
        let net = net(ModelChoice::Small);
        let (w, h) = (48usize, 32usize);
        let clean: Vec<[f32; 3]> = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f32, (i / w) as f32);
                let g = 0.1 + 0.8 * x / w as f32;
                [g, g * 0.8 + 0.02 * y / h as f32, g * 0.6]
            })
            .collect();
        let mut seed = 12345u32;
        let mut rnd = move || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5
        };
        let noisy: Vec<[f32; 3]> =
            clean.iter().map(|p| [p[0] + 0.15 * rnd(), p[1] + 0.15 * rnd(), p[2] + 0.15 * rnd()].map(|v| v.max(0.0))).collect();
        let mse = |a: &[[f32; 3]]| {
            a.iter().zip(&clean).map(|(p, q)| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>()).sum::<f32>() / (3 * clean.len()) as f32
        };
        let albedo = vec![[0.5; 3]; w * h];
        let normal = vec![[0.0, 0.0, 1.0]; w * h];
        let out = denoise_cpu(&net, &noisy, &albedo, &normal, (w as u32, h as u32)).expect("denoise");
        assert!(out.iter().flatten().all(|v| v.is_finite()));
        let (before, after) = (mse(&noisy), mse(&out));
        assert!(after <= before * 0.5, "mse before {before}, after {after}");
    }

    #[test]
    fn pu_handles_zero_negative_and_nan() {
        assert_eq!(pu(0.0), 0.0);
        assert_eq!(pu(-1.0), 0.0);
        assert_eq!(pu(f32::NAN), 0.0);
        assert_eq!(pu(f32::INFINITY), 0.0);
        assert_eq!(pu(-f32::INFINITY), 0.0);

        assert_eq!(pu_inv(0.0), 0.0);
        assert_eq!(pu_inv(-1.0), 0.0);
        assert_eq!(pu_inv(f32::NAN), 0.0);
        assert_eq!(pu_inv(f32::INFINITY), 0.0);
        assert_eq!(pu_inv(-f32::INFINITY), 0.0);
    }

    #[test]
    fn autoexposure_all_black_returns_one() {
        let img = vec![[0.0_f32; 3]; 32 * 32];
        assert_eq!(autoexposure(&img, (32, 32)), 1.0);
    }

    #[test]
    fn encode_input_clamps_and_sanitizes() {
        let n = 4 * 4;
        let color: Vec<[f32; 3]> = (0..n).map(|_| [f32::NAN, -0.5, 2.0]).collect();
        let albedo: Vec<[f32; 3]> = (0..n).map(|_| [1.5, 0.5, -0.5]).collect();
        let normal: Vec<[f32; 3]> = (0..n).map(|_| [2.0, 0.0, -2.0]).collect();

        let (input, padded) = encode_input(&color, &albedo, &normal, (4, 4), 1.0);
        assert_eq!(padded, (16, 16));

        let plane = 16 * 16;
        // Color: NaN -> 0, -0.5 -> 0 (clamped at 0), 2.0 -> PU(2.0) * NORM_SCALE
        assert_eq!(input[0], 0.0);
        assert_eq!(input[plane], 0.0);
        assert_eq!(input[2 * plane], pu(2.0) * NORM_SCALE);
        // Albedo: clamp to 0..1
        assert_eq!(input[3 * plane], 1.0);
        assert_eq!(input[4 * plane], 0.5);
        assert_eq!(input[5 * plane], 0.0);
        // Normal: clamp to -1..1, then x 0.5 + 0.5
        assert_eq!(input[6 * plane], 1.0);
        assert_eq!(input[7 * plane], 0.5);
        assert_eq!(input[8 * plane], 0.0);
    }

    #[test]
    fn decode_output_handles_nan_and_zero_scale() {
        let padded = (16, 16);
        let size = (4, 4);
        let plane = 16 * 16;
        let mut out = vec![0.0_f32; 3 * plane];
        out[plane] = f32::NAN;
        out[2 * plane] = NORM_SCALE;

        let decoded = decode_output(&out, padded, size, 0.0);
        assert_eq!(decoded.len(), 16);
        assert_eq!(decoded[0][0], 0.0);
        assert_eq!(decoded[0][1], 0.0);
        assert!((decoded[0][2] - 1.0).abs() < 0.01);
        for p in &decoded {
            assert!(p.iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn denoise_cpu_validates_input_sizes() {
        let net = net(ModelChoice::Small);
        let (w, h) = (4, 4);
        let n = w * h;
        let color = vec![[0.5; 3]; n];
        let albedo = vec![[0.5; 3]; n];
        let normal = vec![[0.0, 0.0, 1.0]; n];

        let result = denoise_cpu(&net, &vec![[0.5; 3]; n + 1], &albedo, &normal, (w as u32, h as u32));
        assert!(result.is_err());

        let result = denoise_cpu(&net, &color, &vec![[0.5; 3]; n + 1], &normal, (w as u32, h as u32));
        assert!(result.is_err());

        let result = denoise_cpu(&net, &color, &albedo, &vec![[0.0, 0.0, 1.0]; n + 1], (w as u32, h as u32));
        assert!(result.is_err());
    }
}
