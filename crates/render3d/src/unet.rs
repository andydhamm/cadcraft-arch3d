use crate::tza;

/// One convolution layer in the learned denoiser.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: &'static str,
    pub cin: u32,
    pub cout: u32,
    pub weights: Vec<f32>,
    pub bias: Vec<f32>,
}

/// The learned denoiser U-Net.
#[derive(Clone, Debug, PartialEq)]
pub struct Net {
    pub layers: Vec<Layer>,
}

impl Net {
    /// Build the 16-layer reference U-Net from the embedded weights.
    pub fn from_model(m: &tza::Model) -> Result<Net, String> {
        let names = [
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
        let expected = [
            (9, 32),
            (32, 32),
            (32, 48),
            (48, 64),
            (64, 80),
            (80, 96),
            (96, 96),
            (160, 112),
            (112, 112),
            (160, 96),
            (96, 96),
            (128, 64),
            (64, 64),
            (73, 64),
            (64, 32),
            (32, 3),
        ];

        let mut layers = Vec::with_capacity(names.len());
        for (idx, name) in names.iter().enumerate() {
            let weight_name = format!("{name}.weight");
            let bias_name = format!("{name}.bias");
            let weights = m
                .tensor(&weight_name)
                .ok_or_else(|| format!("missing tensor {weight_name}"))?;
            let bias = m
                .tensor(&bias_name)
                .ok_or_else(|| format!("missing tensor {bias_name}"))?;

            if weights.layout != "oihw" {
                return Err(format!("tensor {weight_name} layout is {} not oihw", weights.layout));
            }
            if weights.dims.len() != 4 {
                return Err(format!("tensor {weight_name} has {} dims, expected 4", weights.dims.len()));
            }
            if bias.dims.len() != 1 {
                return Err(format!("tensor {bias_name} has {} dims, expected 1", bias.dims.len()));
            }

            let cout = weights.dims[0];
            let cin = weights.dims[1];
            let kh = weights.dims[2];
            let kw = weights.dims[3];
            if kh != 3 || kw != 3 {
                return Err(format!("tensor {weight_name} is {}x{}, expected 3x3", kh, kw));
            }
            let expected_cin = expected[idx].0 as u32;
            let expected_cout = expected[idx].1 as u32;
            if cin != expected_cin || cout != expected_cout {
                return Err(format!(
                    "tensor {weight_name} channels mismatch: tensor says {cin}->{cout}, README expects {expected_cin}->{expected_cout}"
                ));
            }
            if bias.dims[0] != cout {
                return Err(format!(
                    "tensor {bias_name} has {} entries but weight expects {}",
                    bias.dims[0],
                    cout
                ));
            }

            layers.push(Layer {
                name: *name,
                cin,
                cout,
                weights: weights.data.clone(),
                bias: bias.data.clone(),
            });
        }

        Ok(Net { layers })
    }
}

/// The PU transfer function used by the denoiser input/output transforms.
pub fn pu(y: f32) -> f32 {
    if !y.is_finite() || y <= 0.0 {
        return 0.0;
    }

    const A: f32 = 1.41283765e3;
    const B: f32 = 1.64593172e0;
    const C: f32 = 4.31384981e-1;
    const D: f32 = -2.94139609e-3;
    const E: f32 = 1.92653254e-1;
    const F: f32 = 6.26026094e-3;
    const G: f32 = 9.98620152e-1;
    const Y0: f32 = 1.57945760e-6;
    const Y1: f32 = 3.22087631e-2;

    if y <= Y0 {
        A * y
    } else if y <= Y1 {
        B * y.powf(C) + D
    } else {
        E * (y + F).ln() + G
    }
}

/// The inverse of [`pu`].
pub fn pu_inv(x: f32) -> f32 {
    if !x.is_finite() || x <= 0.0 {
        return 0.0;
    }

    const A: f32 = 1.41283765e3;
    const B: f32 = 1.64593172e0;
    const C: f32 = 4.31384981e-1;
    const D: f32 = -2.94139609e-3;
    const E: f32 = 1.92653254e-1;
    const F: f32 = 6.26026094e-3;
    const G: f32 = 9.98620152e-1;
    const X0: f32 = 2.23151711e-3;
    const X1: f32 = 3.70974749e-1;

    if x <= X0 {
        x / A
    } else if x <= X1 {
        ((x - D) / B).powf(1.0 / C)
    } else {
        ((x - G) / E).exp() - F
    }
}

/// Normalization scale used by the denoiser input pipeline.
pub const NORM_SCALE: f32 = 0.31899;

/// Compute the autoexposure scale for a colour image.
pub fn autoexposure(color: &[[f32; 3]], size: (u32, u32)) -> f32 {
    let (w, h) = size;
    let pixels = (w as usize).saturating_mul(h as usize);
    if w == 0 || h == 0 || pixels == 0 {
        return 1.0;
    }

    let bins_x = ((w as usize).div_ceil(16)).max(1);
    let bins_y = ((h as usize).div_ceil(16)).max(1);
    let mut lum = Vec::with_capacity(bins_x * bins_y);

    for by in 0..bins_y {
        let y0 = by * (h as usize) / bins_y;
        let y1 = ((by + 1) * (h as usize) + bins_y - 1) / bins_y;
        for bx in 0..bins_x {
            let x0 = bx * (w as usize) / bins_x;
            let x1 = ((bx + 1) * (w as usize) + bins_x - 1) / bins_x;
            let mut sum = 0.0_f32;
            let mut count = 0usize;
            for y in y0..y1.min(h as usize) {
                for x in x0..x1.min(w as usize) {
                    let idx = y * w as usize + x;
                    let pixel = color.get(idx).copied().unwrap_or([0.0; 3]);
                    let r = if pixel[0].is_finite() { pixel[0].max(0.0) } else { 0.0 };
                    let g = if pixel[1].is_finite() { pixel[1].max(0.0) } else { 0.0 };
                    let b = if pixel[2].is_finite() { pixel[2].max(0.0) } else { 0.0 };
                    let l = 0.212671 * r + 0.715160 * g + 0.072169 * b;
                    sum += l;
                    count += 1;
                }
            }
            lum.push(if count == 0 { 0.0 } else { sum / count as f32 });
        }
    }

    let mut total_log = 0.0_f32;
    let mut count = 0usize;
    for lum_bin in lum {
        if lum_bin > 1e-8 {
            total_log += lum_bin.log2();
            count += 1;
        }
    }
    if count == 0 {
        1.0
    } else {
        0.18 / 2.0f32.powf(total_log / count as f32)
    }
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

fn sanitize_color(v: f32) -> f32 {
    if v.is_finite() { v.max(0.0) } else { 0.0 }
}

fn byte_index(ch: usize, y: usize, x: usize, h: usize, w: usize) -> usize {
    ch * h * w + y * w + x
}

/// Encode colour, albedo and normals into the 9-channel U-Net input layout.
pub fn encode_input(
    color: &[[f32; 3]],
    albedo: &[[f32; 3]],
    normal: &[[f32; 3]],
    size: (u32, u32),
    scale: f32,
) -> (Vec<f32>, (u32, u32)) {
    let (w, h) = size;
    let w_pad = (w as usize).div_ceil(16) * 16;
    let h_pad = (h as usize).div_ceil(16) * 16;
    let mut input = vec![0.0_f32; 9 * w_pad * h_pad];

    for y in 0..h as usize {
        for x in 0..w as usize {
            let idx = y * w as usize + x;
            let rgb = color.get(idx).copied().unwrap_or([0.0; 3]);
            let alb = albedo.get(idx).copied().unwrap_or([0.0; 3]);
            let nrm = normal.get(idx).copied().unwrap_or([0.0; 3]);
            for ch in 0..3 {
                let base = (ch * h_pad * w_pad) + y * w_pad + x;
                let color_val = sanitize_color(rgb[ch]) * scale;
                input[base] = pu(color_val) * NORM_SCALE;

                let base = ((3 + ch) * h_pad * w_pad) + y * w_pad + x;
                input[base] = clamp01(alb[ch]);

                let base = ((6 + ch) * h_pad * w_pad) + y * w_pad + x;
                input[base] = nrm[ch].clamp(-1.0, 1.0) * 0.5 + 0.5;
            }
        }
    }

    ((input), (w_pad as u32, h_pad as u32))
}

fn conv3x3(input: &[f32], c_in: usize, h: usize, w: usize, layer: &Layer) -> Vec<f32> {
    let out_h = h;
    let out_w = w;
    let out_ch = layer.cout as usize;
    let mut out = vec![0.0_f32; out_ch * out_h * out_w];

    for oc in 0..out_ch {
        for y in 0..out_h {
            for x in 0..out_w {
                let mut sum = layer.bias[oc];
                for ic in 0..c_in {
                    let in_base = ic * h * w;
                    for ky in 0..3usize {
                        for kx in 0..3usize {
                            let iy = y as i32 + ky as i32 - 1;
                            let ix = x as i32 + kx as i32 - 1;
                            if iy >= 0 && iy < h as i32 && ix >= 0 && ix < w as i32 {
                                let src = in_base + (iy as usize * w) + ix as usize;
                                let weight = layer.weights[oc * c_in * 9 + ic * 9 + ky * 3 + kx];
                                sum += input[src] * weight;
                            }
                        }
                    }
                }
                let idx = oc * out_h * out_w + y * out_w + x;
                out[idx] = sum.max(0.0);
            }
        }
    }
    out
}

fn maxpool2(data: &[f32], channels: usize, h: usize, w: usize) -> Vec<f32> {
    let out_h = h / 2;
    let out_w = w / 2;
    let mut out = vec![0.0_f32; channels * out_h * out_w];
    for c in 0..channels {
        let in_base = c * h * w;
        for y in 0..out_h {
            for x in 0..out_w {
                let mut best = f32::NEG_INFINITY;
                for ky in 0..2usize {
                    for kx in 0..2usize {
                        let src = in_base + (y * 2 + ky) * w + (x * 2 + kx);
                        let v = data[src];
                        if v > best {
                            best = v;
                        }
                    }
                }
                out[c * out_h * out_w + y * out_w + x] = best;
            }
        }
    }
    out
}

fn upsample2(data: &[f32], channels: usize, h: usize, w: usize) -> Vec<f32> {
    let out_h = h * 2;
    let out_w = w * 2;
    let mut out = vec![0.0_f32; channels * out_h * out_w];
    for c in 0..channels {
        let in_base = c * h * w;
        for y in 0..out_h {
            let src_y = (y / 2).min(h - 1);
            for x in 0..out_w {
                let src_x = (x / 2).min(w - 1);
                let src = in_base + src_y * w + src_x;
                out[c * out_h * out_w + y * out_w + x] = data[src];
            }
        }
    }
    out
}

fn concat_first(a: &[f32], b: &[f32], h: usize, w: usize) -> Vec<f32> {
    let a_channels = a.len() / (h * w);
    let b_channels = b.len() / (h * w);
    let total = h * w;
    let mut out = vec![0.0_f32; (a_channels + b_channels) * total];
    for c in 0..a_channels {
        let src = c * total;
        let dst = c * total;
        out[dst..dst + total].copy_from_slice(&a[src..src + total]);
    }
    for c in 0..b_channels {
        let src = c * total;
        let dst = (a_channels + c) * total;
        out[dst..dst + total].copy_from_slice(&b[src..src + total]);
    }
    out
}

/// Run the reference CPU U-Net on a CHW tensor.
pub fn forward_cpu(net: &Net, input: &[f32], padded: (u32, u32)) -> Result<Vec<f32>, String> {
    let (pw, ph) = padded;
    if pw > 256 || ph > 256 {
        return Err(format!("padded input is {}x{}, above the 256x256 CPU cap", pw, ph));
    }

    let h = ph as usize;
    let w = pw as usize;
    let expected = 9 * h * w;
    if input.len() != expected {
        return Err(format!("expected {expected} input values for a {}x{} tensor, got {}", w, h, input.len()));
    }

    let mut x = relu(conv3x3(input, 9, h, w, &net.layers[0]));
    let mut x = relu(conv3x3(&x, 32, h, w, &net.layers[1]));
    let p1 = maxpool2(&x, 32, h, w);
    let h1 = h / 2;
    let w1 = w / 2;

    let mut x = relu(conv3x3(&p1, 32, h1, w1, &net.layers[2]));
    let p2 = maxpool2(&x, 48, h1, w1);
    let h2 = h1 / 2;
    let w2 = w1 / 2;

    let mut x = relu(conv3x3(&p2, 48, h2, w2, &net.layers[3]));
    let p3 = maxpool2(&x, 64, h2, w2);
    let h3 = h2 / 2;
    let w3 = w2 / 2;

    let mut x = relu(conv3x3(&p3, 64, h3, w3, &net.layers[4]));
    let p4 = maxpool2(&x, 80, h3, w3);
    let h4 = h3 / 2;
    let w4 = w3 / 2;

    let mut x = relu(conv3x3(&p4, 80, h4, w4, &net.layers[5]));
    let mut x = relu(conv3x3(&x, 96, h4, w4, &net.layers[6]));

    let up = upsample2(&x, 96, h4, w4);
    let concat = concat_first(&up, &p3, h3, w3);
    let mut x = relu(conv3x3(&concat, 160, h3, w3, &net.layers[7]));
    let mut x = relu(conv3x3(&x, 112, h3, w3, &net.layers[8]));

    let up = upsample2(&x, 112, h3, w3);
    let concat = concat_first(&up, &p2, h2, w2);
    let mut x = relu(conv3x3(&concat, 160, h2, w2, &net.layers[9]));
    let mut x = relu(conv3x3(&x, 96, h2, w2, &net.layers[10]));

    let up = upsample2(&x, 96, h2, w2);
    let concat = concat_first(&up, &p1, h1, w1);
    let mut x = relu(conv3x3(&concat, 128, h1, w1, &net.layers[11]));
    let mut x = relu(conv3x3(&x, 64, h1, w1, &net.layers[12]));

    let up = upsample2(&x, 64, h1, w1);
    let concat = concat_first(&up, input, h, w);
    let mut x = relu(conv3x3(&concat, 73, h, w, &net.layers[13]));
    let mut x = relu(conv3x3(&x, 64, h, w, &net.layers[14]));
    let out = relu(conv3x3(&x, 64, h, w, &net.layers[15]));
    Ok(out)
}

/// Denoise a full HDR image using the CPU reference network.
pub fn denoise_cpu(
    net: &Net,
    color: &[[f32; 3]],
    albedo: &[[f32; 3]],
    normal: &[[f32; 3]],
    size: (u32, u32),
) -> Result<Vec<[f32; 3]>, String> {
    let pixels = size.0 as usize * size.1 as usize;
    if pixels == 0 {
        return Ok(Vec::new());
    }

    let scale = autoexposure(color, size);
    let (input, padded) = encode_input(color, albedo, normal, size, scale);
    let output = forward_cpu(net, &input, padded)?;
    Ok(decode_output(&output, padded, size, scale))
}

/// Decode the network output back to RGB image values.
pub fn decode_output(out: &[f32], padded: (u32, u32), size: (u32, u32), scale: f32) -> Vec<[f32; 3]> {
    let (pw, ph) = padded;
    let (w, h) = size;
    let w_usize = w as usize;
    let h_usize = h as usize;
    let mut decoded = vec![[0.0_f32; 3]; w_usize * h_usize];

    for y in 0..h_usize {
        for x in 0..w_usize {
            let idx = y * w_usize + x;
            let mut rgb = [0.0_f32; 3];
            for ch in 0..3 {
                let base = ch * (ph as usize) * (pw as usize) + y * (pw as usize) + x;
                let value = out.get(base).copied().unwrap_or(0.0).max(0.0) / NORM_SCALE;
                rgb[ch] = pu_inv(value) / scale;
            }
            decoded[idx] = rgb;
        }
    }
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tza::{ModelChoice, model};

    #[test]
    fn pu_round_trips() {
        let xs = [
            1e-6, 2e-6, 5e-6, 1e-5, 2e-5, 5e-5, 1e-4, 2e-4, 5e-4, 1e-3, 2e-3, 5e-3, 1e-2,
            2e-2, 5e-2, 1e-1, 2e-1, 5e-1, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0,
            500.0, 1000.0, 5000.0, 10000.0, 20000.0, 40000.0, 60000.0,
        ];
        for &x in &xs {
            let back = pu_inv(pu(x));
            let rel = (back - x).abs() / x.max(1.0);
            assert!(rel <= 1e-4, "PU inverse lost precision at {x}: got {back}, rel {rel}");
        }
    }

    #[test]
    fn autoexposure_of_mid_grey_is_one() {
        let size = (64, 64);
        let image = vec![[0.18, 0.18, 0.18]; (size.0 * size.1) as usize];
        assert!((autoexposure(&image, size) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn input_is_padded_to_multiples_of_sixteen() {
        let color = vec![[0.1, 0.2, 0.3]; 100 * 50];
        let albedo = vec![[0.5, 0.5, 0.5]; 100 * 50];
        let normal = vec![[0.0, 0.0, 1.0]; 100 * 50];
        let (input, padded) = encode_input(&color, &albedo, &normal, (100, 50), 1.0);
        assert_eq!(padded, (112, 64));
        assert_eq!(input.len(), 9 * 112 * 64);
    }

    #[test]
    fn net_shapes_follow_the_readme() {
        let net = Net::from_model(model(ModelChoice::Base).expect("base model should parse")).unwrap();
        assert_eq!(net.layers.len(), 16);
        assert_eq!(net.layers[0].cin, 9);
        assert_eq!(net.layers[0].cout, 32);
        assert_eq!(net.layers[7].cin, 160);
        assert_eq!(net.layers[7].cout, 112);
        assert_eq!(net.layers[9].cin, 160);
        assert_eq!(net.layers[9].cout, 96);
        assert_eq!(net.layers[13].cin, 73);
        assert_eq!(net.layers[13].cout, 64);
        assert_eq!(net.layers[15].cin, 64);
        assert_eq!(net.layers[15].cout, 3);
    }

    #[test]
    fn flat_grey_survives_the_network() {
        for which in [ModelChoice::Base, ModelChoice::Small] {
            let net = Net::from_model(model(which).expect("model should parse")).unwrap();
            let size = (32, 32);
            let color = vec![[0.5, 0.5, 0.5]; (size.0 * size.1) as usize];
            let albedo = vec![[0.5, 0.5, 0.5]; (size.0 * size.1) as usize];
            let normal = vec![[0.0, 0.0, 1.0]; (size.0 * size.1) as usize];
            let out = denoise_cpu(&net, &color, &albedo, &normal, size).unwrap();
            let max_err = out.iter().map(|rgb| rgb.iter().map(|v| (v - 0.5).abs()).fold(0.0f32, f32::max)).fold(0.0f32, f32::max);
            assert!(max_err <= 0.01, "flat grey drifted by {max_err} for {which:?}");
        }
    }

    #[test]
    fn noisy_gradient_is_denoised_on_the_cpu() {
        let net = Net::from_model(model(ModelChoice::Small).expect("small model loaded")).unwrap();
        let size = (48, 32);
        let mut clean = Vec::with_capacity(size.0 as usize * size.1 as usize);
        for y in 0..size.1 {
            for x in 0..size.0 {
                clean.push([
                    x as f32 / size.0 as f32,
                    y as f32 / size.1 as f32,
                    (x as f32 + y as f32) / (size.0 as f32 + size.1 as f32),
                ]);
            }
        }

        let mut noisy = clean.clone();
        for (idx, px) in noisy.iter_mut().enumerate() {
            let n = (((idx as u32 * 37 + 17) % 1000) as f32 / 1000.0) - 0.5;
            px[0] = (px[0] + n * 0.12).clamp(0.0, 1.0);
            px[1] = (px[1] + n * 0.15).clamp(0.0, 1.0);
            px[2] = (px[2] + n * 0.10).clamp(0.0, 1.0);
        }

        let albedo = vec![[0.5, 0.5, 0.5]; clean.len()];
        let normal = vec![[0.0, 0.0, 1.0]; clean.len()];
        let noisy_mse = noisy
            .iter()
            .zip(clean.iter())
            .map(|(a, b)| {
                let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                d[0].powi(2) + d[1].powi(2) + d[2].powi(2)
            })
            .sum::<f32>()
            / clean.len() as f32;

        let denoised = denoise_cpu(&net, &noisy, &albedo, &normal, size).unwrap();
        let denoised_mse = denoised
            .iter()
            .zip(clean.iter())
            .map(|(a, b)| {
                let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                d[0].powi(2) + d[1].powi(2) + d[2].powi(2)
            })
            .sum::<f32>()
            / clean.len() as f32;

        assert!(denoised.iter().flatten().all(|v| v.is_finite()));
        assert!(denoised_mse <= noisy_mse * 0.5, "noisy_mse={noisy_mse}, denoised_mse={denoised_mse}");
    }
}
