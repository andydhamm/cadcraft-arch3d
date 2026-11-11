use cadcraft_doc::arch::{ImageRgba, ImageRgbe};

const MAX_RADIANCE_DIM: u32 = 8192;
const MAX_HDR_BYTES: usize = 64 * 1024 * 1024;

fn decode_rgbe(pixel: [u8; 4]) -> [f64; 3] {
    let [r, g, b, e] = pixel;
    let factor = if e == 0 { 0.0 } else { 2f64.powi(i32::from(e) - 136) };
    [r as f64 * factor, g as f64 * factor, b as f64 * factor]
}

fn encode_linear_to_rgbe(rgb: [f64; 3]) -> [u8; 4] {
    let max_v = rgb.iter().copied().fold(0.0_f64, f64::max);
    if max_v <= 0.0 {
        return [0, 0, 0, 0];
    }
    let exp = (max_v.log2().floor() as i32 + 136).clamp(0, 255);
    let scale = 2f64.powi(exp - 136);
    let mant = |v: f64| (v / scale).clamp(0.0, 255.0).round() as u8;
    [mant(rgb[0]), mant(rgb[1]), mant(rgb[2]), exp as u8]
}

fn read_until_blank(bytes: &[u8]) -> Result<usize, String> {
    let len = bytes.len();
    if len < 2 {
        return Err("HDR header is truncated".to_string());
    }
    for i in 0..len.saturating_sub(1) {
        if bytes[i] == b'\n' && bytes[i + 1] == b'\n' {
            return Ok(i + 2);
        }
        if i + 3 < len && bytes[i] == b'\r' && bytes[i + 1] == b'\n' && bytes[i + 2] == b'\r' && bytes[i + 3] == b'\n' {
            return Ok(i + 4);
        }
    }
    Err("HDR header is missing the blank separator".to_string())
}

fn parse_header(header: &str) -> Result<(u32, u32), String> {
    let mut width = None;
    let mut height = None;
    let tokens: Vec<_> = header.split_whitespace().collect();
    let mut i = 0usize;
    while i + 1 < tokens.len() {
        match tokens[i] {
            "-Y" => {
                let v = tokens[i + 1].parse::<u32>().map_err(|_| "invalid HDR height".to_string())?;
                height = Some(v);
            }
            "+X" => {
                let v = tokens[i + 1].parse::<u32>().map_err(|_| "invalid HDR width".to_string())?;
                width = Some(v);
            }
            "+Y" | "-X" => return Err("bad HDR header orientation".to_string()),
            _ => {}
        }
        i += 1;
    }
    let width = width.ok_or_else(|| "HDR header is missing +X width".to_string())?;
    let height = height.ok_or_else(|| "HDR header is missing -Y height".to_string())?;
    if !(1..=MAX_RADIANCE_DIM).contains(&width) || !(1..=MAX_RADIANCE_DIM).contains(&height) {
        return Err(format!("HDR dimensions {width}x{height} are out of range"));
    }
    Ok((width, height))
}

fn decode_rle_scanlines(data: &[u8], width: usize, height: usize) -> Result<Vec<u8>, String> {
    let expected = width.checked_mul(height).and_then(|n| n.checked_mul(4)).ok_or_else(|| "HDR image is too large".to_string())?;
    let mut out = vec![0u8; expected];
    let mut pos = 0usize;
    let mut out_pos = 0usize;
    for _ in 0..height {
        if pos + 4 > data.len() {
            return Err("truncated HDR scanline header".to_string());
        }
        let row_hdr = &data[pos..pos + 4];
        pos += 4;
        if row_hdr[0] != 2 || row_hdr[1] != 2 || ((u16::from(row_hdr[2]) << 8) | u16::from(row_hdr[3])) as usize != width {
            return Err("bad HDR scanline header".to_string());
        }
        let mut channels = vec![vec![0u8; width]; 4];
        for ch in 0..4 {
            let mut n = 0usize;
            while n < width {
                if pos >= data.len() {
                    return Err("truncated HDR RLE data".to_string());
                }
                let count = data[pos] as usize;
                pos += 1;
                if count == 0 || count == 128 {
                    return Err("invalid HDR RLE count".to_string());
                }
                if count > 128 {
                    let run = count - 128;
                    if run == 0 || pos >= data.len() {
                        return Err("truncated HDR RLE run".to_string());
                    }
                    let value = data[pos];
                    pos += 1;
                    for _ in 0..run {
                        if n >= width {
                            return Err("HDR RLE overrun".to_string());
                        }
                        channels[ch][n] = value;
                        n += 1;
                    }
                } else {
                    let lit = count;
                    if pos + lit > data.len() {
                        return Err("truncated HDR RLE literal run".to_string());
                    }
                    for _ in 0..lit {
                        if n >= width {
                            return Err("HDR RLE overrun".to_string());
                        }
                        channels[ch][n] = data[pos];
                        pos += 1;
                        n += 1;
                    }
                }
            }
        }
        for x in 0..width {
            let i = out_pos + x * 4;
            out[i] = channels[0][x];
            out[i + 1] = channels[1][x];
            out[i + 2] = channels[2][x];
            out[i + 3] = channels[3][x];
        }
        out_pos += width * 4;
    }
    if pos != data.len() {
        return Err("trailing HDR data".to_string());
    }
    Ok(out)
}

/// Parse a Radiance `.hdr` buffer into a raw RGBE image.
pub fn read_hdr(bytes: &[u8]) -> Result<ImageRgbe, String> {
    if bytes.is_empty() {
        return Err("HDR input is empty".to_string());
    }
    if bytes.len() > MAX_HDR_BYTES {
        return Err(format!("HDR input exceeds {MAX_HDR_BYTES} bytes"));
    }
    if !bytes.starts_with(b"#?RADIANCE") && !bytes.starts_with(b"#?RGBE") {
        return Err("bad HDR magic".to_string());
    }
    let header_end = read_until_blank(bytes)?;
    let header = std::str::from_utf8(&bytes[..header_end]).map_err(|_| "HDR header is not UTF-8".to_string())?;
    let format_line = header.lines().find(|line| line.starts_with("FORMAT="));
    if format_line != Some("FORMAT=32-bit_rle_rgbe") {
        return Err("FORMAT=32-bit_rle_rgbe required".to_string());
    }
    let (width, height) = parse_header(header)?;
    let payload = &bytes[header_end..];
    let expected = width as usize * height as usize * 4;
    let rgbe = if payload.len() == expected {
        payload.to_vec()
    } else if payload.len() >= 4 && payload[0] == 2 && payload[1] == 2 {
        decode_rle_scanlines(payload, width as usize, height as usize)?
    } else {
        return Err(format!("HDR payload length {} does not match {expected} bytes", payload.len()));
    };
    Ok(ImageRgbe { width, height, rgbe })
}

/// Downsample an RGBE image while preserving aspect ratio and linear-light averaging.
pub fn fit(img: &ImageRgbe, max_w: u32, max_h: u32) -> ImageRgbe {
    if img.width == 0 || img.height == 0 || max_w == 0 || max_h == 0 {
        return ImageRgbe::default();
    }
    let max_w = max_w.max(1);
    let max_h = max_h.max(1);
    if img.width <= max_w && img.height <= max_h {
        return img.clone();
    }
    let aspect = img.width as f64 / img.height as f64;
    let target_w = if max_w as f64 / max_h as f64 > aspect { (max_h as f64 * aspect).round() as u32 } else { max_w };
    let target_h = ((target_w as f64 / aspect).round().max(1.0)) as u32;
    let target_h = target_h.min(max_h);
    let target_w = target_w.min(max_w);
    let target_w = target_w.max(1);
    let target_h = target_h.max(1);
    let scale_x = img.width as f64 / target_w as f64;
    let scale_y = img.height as f64 / target_h as f64;
    let mut out = Vec::with_capacity((target_w as usize) * (target_h as usize) * 4);
    for y in 0..target_h {
        for x in 0..target_w {
            let x0 = ((x as f64) * scale_x).floor() as u32;
            let x1 = (((x + 1) as f64) * scale_x).ceil() as u32;
            let y0 = ((y as f64) * scale_y).floor() as u32;
            let y1 = (((y + 1) as f64) * scale_y).ceil() as u32;
            let mut sum = [0.0_f64; 3];
            let mut count = 0u32;
            for yy in y0..y1.min(img.height) {
                for xx in x0..x1.min(img.width) {
                    let rgb = img.pixel_rgb(xx, yy);
                    sum[0] += rgb[0];
                    sum[1] += rgb[1];
                    sum[2] += rgb[2];
                    count += 1;
                }
            }
            let avg = if count == 0 { [0.0; 3] } else { [sum[0] / count as f64, sum[1] / count as f64, sum[2] / count as f64] };
            let px = encode_linear_to_rgbe(avg);
            out.extend_from_slice(&px);
        }
    }
    ImageRgbe { width: target_w, height: target_h, rgbe: out }
}

/// Create a thumbnail from an HDR image, with a bounded, mostly non-uniform display mapping.
pub fn display_image(img: &ImageRgbe, max_side: u32) -> ImageRgba {
    let max_side = max_side.max(1);
    let scaled = fit(img, max_side, max_side);
    let mut rgba = Vec::with_capacity((scaled.width as usize) * (scaled.height as usize) * 4);
    let mut max_lum = 1e-6_f64;
    for px in scaled.rgbe.chunks_exact(4) {
        let linear = decode_rgbe([px[0], px[1], px[2], px[3]]);
        let lum = (0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]).max(0.0);
        if lum > max_lum {
            max_lum = lum;
        }
    }
    let exposed = 0.18 / max_lum.max(1e-6);
    for px in scaled.rgbe.chunks_exact(4) {
        let linear = decode_rgbe([px[0], px[1], px[2], px[3]]);
        let mapped = [(linear[0] * exposed).clamp(0.0, 1.0), (linear[1] * exposed).clamp(0.0, 1.0), (linear[2] * exposed).clamp(0.0, 1.0)];
        let srgb = mapped.map(|v| (v.powf(1.0 / 2.2) * 255.0).round().clamp(0.0, 255.0) as u8);
        rgba.extend_from_slice(&[srgb[0], srgb[1], srgb[2], 255]);
    }
    ImageRgba { width: scaled.width, height: scaled.height, rgba }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hdr(width: u32, height: u32, value: u8) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(format!("#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n-Y {height} +X {width}\n\n").as_bytes());
        for _ in 0..height as usize {
            for _ in 0..width as usize {
                data.extend_from_slice(&[value, value, value, 128]);
            }
        }
        data
    }

    fn make_rle_hdr(width: u32, height: u32, value: u8, exp: u8) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(format!("#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n-Y {height} +X {width}\n\n").as_bytes());
        for _ in 0..height as usize {
            data.push(2);
            data.push(2);
            data.push((width >> 8) as u8);
            data.push((width & 0xff) as u8);
            for _ in 0..width as usize {
                data.push(1);
                data.push(value);
                data.push(1);
                data.push(value);
                data.push(1);
                data.push(value);
                data.push(1);
                data.push(exp);
            }
        }
        data
    }

    #[test]
    fn flat_and_rle_scanlines() {
        let flat = make_hdr(1, 1, 55);
        let flat_img = read_hdr(&flat).unwrap();
        let rle = make_rle_hdr(1, 1, 55, 128);
        let rle_img = read_hdr(&rle).unwrap();
        assert_eq!(flat_img.width, rle_img.width);
        assert_eq!(flat_img.height, rle_img.height);
        assert_eq!(flat_img.rgbe, rle_img.rgbe);
    }

    #[test]
    fn refuses_truncated_overrun_trailing_bad_header_orientation_huge() {
        let mut bad = Vec::from(b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n-Y 2 +X 2\n\n".as_slice());
        for _ in 0..5 {
            bad.push(2);
            bad.push(2);
            bad.push(0);
            bad.push(2);
            bad.push(1);
            bad.push(1);
            bad.push(1);
            bad.push(128);
        }
        assert!(read_hdr(&bad).is_err());
        let over = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n-Y 2 +X 2\n\n".to_vec();
        assert!(read_hdr(&over).is_err());
        let trailing = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n-Y 1 +X 1\n\n\x00\x00\x00\x00\x00".to_vec();
        assert!(read_hdr(&trailing).is_err());
        let bad_orient = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n+Y 2 -X 2\n\n".to_vec();
        assert!(read_hdr(&bad_orient).is_err());
        let huge = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n-Y 9000 +X 9000\n\n".to_vec();
        assert!(read_hdr(&huge).is_err());
    }

    #[test]
    fn fit_downsamples_to_1024_by_512() {
        let mut rgbe = vec![0u8; 2048 * 1024 * 4];
        for px in rgbe.chunks_exact_mut(4) {
            px[3] = 128;
        }
        let img = ImageRgbe { width: 2048, height: 1024, rgbe };
        let fit_img = fit(&img, 1024, 512);
        assert_eq!(fit_img.width, 1024);
        assert_eq!(fit_img.height, 512);
    }

    #[test]
    fn rgbe_one_is_one() {
        let pix = decode_rgbe([1, 1, 1, 136]);
        assert!((pix[0] - 1.0).abs() < 1e-9);
        assert!((pix[1] - 1.0).abs() < 1e-9);
        assert!((pix[2] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn display_image_bounded_and_not_uniform() {
        let img = ImageRgbe {
            width: 4,
            height: 2,
            rgbe: vec![0, 0, 0, 128, 0, 0, 0, 128, 255, 0, 0, 128, 0, 255, 0, 128, 0, 0, 255, 128, 0, 0, 0, 128, 0, 0, 0, 128, 0, 0, 0, 128],
        };
        let thumb = display_image(&img, 16);
        assert!(thumb.width <= 16);
        assert!(thumb.height <= 16);
        assert!(thumb.rgba.iter().all(|&v| v <= 255));
        let rgb = thumb.rgba.chunks_exact(4).map(|px| [px[0], px[1], px[2]]).collect::<Vec<_>>();
        assert!(rgb.iter().any(|px| px[0] != rgb[0][0] || px[1] != rgb[0][1] || px[2] != rgb[0][2]));
    }

    #[test]
    fn hostile_hdr_does_not_panic() {
        let bad = [0u8; 256];
        let res = std::panic::catch_unwind(|| read_hdr(&bad));
        assert!(res.is_ok());
        assert!(read_hdr(&bad).is_err());
    }

    #[test]
    fn hdr_images_pruned_and_capped() {
        let mut model = cadcraft_doc::ArchModel::default();
        model.hdr_images.insert("b".into(), ImageRgbe { width: 2, height: 1, rgbe: vec![0, 0, 0, 128, 0, 0, 0, 128] });
        model.hdr_images.insert("a".into(), ImageRgbe { width: 2, height: 1, rgbe: vec![0, 0, 0, 128, 0, 0, 0, 128] });
        model.hdr_images.insert("c".into(), ImageRgbe { width: 2, height: 1, rgbe: vec![0, 0, 0, 128, 0, 0, 0, 128] });
        model.hdr_images.insert("d".into(), ImageRgbe { width: 2, height: 1, rgbe: vec![0, 0, 0, 128, 0, 0, 0, 128] });
        model.hdr_images.insert("e".into(), ImageRgbe { width: 1, height: 1, rgbe: vec![0, 0, 0, 0] });
        model.backdrop = Some(cadcraft_doc::arch::Backdrop::Hdri { image: "b".to_string(), intensity: 1.0, rotation: 0.0, horizon: 0.0 });
        model.prune_hdr_images();
        assert!(model.hdr_images.contains_key("b"));
        assert!(!model.hdr_images.contains_key("e"));
        assert!(model.hdr_images.len() <= cadcraft_doc::arch::MAX_HDR_IMAGES);
    }
}
