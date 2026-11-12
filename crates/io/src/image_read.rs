//! PNG and JPEG decoding to [`ImageRgba`], with deterministic down-scaling.

use cadcraft_doc::arch::ImageRgba;
use std::io::Cursor;

/// Largest accepted source side, checked from the header before decoding.
const MAX_SOURCE_SIDE: u32 = 16384;
/// Largest accepted source pixel count (decoded RGBA is four bytes each).
const MAX_SOURCE_PIXELS: u64 = 64 * 1024 * 1024;

/// Decode a PNG or JPEG into RGBA8. A side above `max_side` is reduced (integer box
/// filter, then exact area averaging) keeping the aspect ratio. Deterministic.
pub fn read_image(bytes: &[u8], max_side: u32) -> Result<ImageRgba, String> {
    if bytes.is_empty() {
        return Err("image is empty".into());
    }
    if max_side == 0 {
        return Err("image size limit is zero".into());
    }
    let format = image::guess_format(bytes).map_err(|e| format!("unknown image format: {e}"))?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return Err("only PNG and JPEG images are supported".into());
    }
    let reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| format!("cannot read image: {e}"))?;
    let (w, h) = reader.into_dimensions().map_err(|e| format!("cannot read image header: {e}"))?;
    if w == 0 || h == 0 {
        return Err("image has no pixels".into());
    }
    if w > MAX_SOURCE_SIDE || h > MAX_SOURCE_SIDE || u64::from(w) * u64::from(h) > MAX_SOURCE_PIXELS {
        return Err(format!("image is too large ({w} x {h})"));
    }
    let img = image::load_from_memory_with_format(bytes, format).map_err(|e| format!("cannot decode image: {e}"))?.into_rgba8();
    let (w, h) = (img.width(), img.height());
    let mut cur = ImageRgba { width: w, height: h, rgba: img.into_raw(), ..Default::default() };
    let long = w.max(h);
    if long > max_side {
        let f = long / max_side;
        if f >= 2 {
            cur = box_reduce(&cur, f);
        }
        let long = cur.width.max(cur.height);
        if long > max_side {
            let scale = f64::from(max_side) / f64::from(long);
            let nw = ((f64::from(cur.width) * scale).round() as u32).clamp(1, max_side);
            let nh = ((f64::from(cur.height) * scale).round() as u32).clamp(1, max_side);
            cur = area_resize(&cur, nw, nh);
        }
    }
    Ok(cur)
}

/// Average `f` x `f` blocks (partial blocks at the edges average what exists).
fn box_reduce(src: &ImageRgba, f: u32) -> ImageRgba {
    let (sw, sh) = (src.width as usize, src.height as usize);
    let f = f as usize;
    let (nw, nh) = (sw.div_ceil(f), sh.div_ceil(f));
    let mut out = vec![0u8; nw * nh * 4];
    for y in 0..nh {
        for x in 0..nw {
            let mut sum = [0u64; 4];
            let mut n = 0u64;
            for sy in y * f..((y + 1) * f).min(sh) {
                for sx in x * f..((x + 1) * f).min(sw) {
                    if let Some(px) = src.rgba.get((sy * sw + sx) * 4..(sy * sw + sx) * 4 + 4) {
                        for (s, v) in sum.iter_mut().zip(px) {
                            *s += u64::from(*v);
                        }
                        n += 1;
                    }
                }
            }
            if let Some(dst) = out.get_mut((y * nw + x) * 4..(y * nw + x) * 4 + 4) {
                for (d, s) in dst.iter_mut().zip(sum) {
                    *d = if n == 0 { 0 } else { ((s + n / 2) / n).min(255) as u8 };
                }
            }
        }
    }
    ImageRgba { width: nw as u32, height: nh as u32, rgba: out, ..Default::default() }
}

/// First source cell and weights of the cells covering destination cell `i` of `n` over `len` cells.
fn spans(i: usize, n: usize, len: usize) -> (usize, Vec<f32>) {
    let a = i as f64 * len as f64 / n as f64;
    let b = (i + 1) as f64 * len as f64 / n as f64;
    let lo = (a.floor() as usize).min(len.saturating_sub(1));
    let hi = (b.ceil() as usize).min(len).max(lo + 1);
    let mut w = Vec::with_capacity(hi - lo);
    for k in lo..hi {
        let cover = (b.min((k + 1) as f64) - a.max(k as f64)).max(0.0);
        w.push((cover / (b - a)) as f32);
    }
    (lo, w)
}

/// Exact area averaging to `nw` x `nh` (separable).
fn area_resize(src: &ImageRgba, nw: u32, nh: u32) -> ImageRgba {
    let (sw, sh, nw, nh) = (src.width as usize, src.height as usize, nw as usize, nh as usize);
    let mut tmp = vec![0f32; nw * sh * 4];
    for x in 0..nw {
        let (lo, ws) = spans(x, nw, sw);
        for y in 0..sh {
            let mut acc = [0f32; 4];
            for (k, w) in ws.iter().enumerate() {
                let i = (y * sw + lo + k) * 4;
                if let Some(px) = src.rgba.get(i..i + 4) {
                    for (a, v) in acc.iter_mut().zip(px) {
                        *a += f32::from(*v) * w;
                    }
                }
            }
            if let Some(dst) = tmp.get_mut((y * nw + x) * 4..(y * nw + x) * 4 + 4) {
                dst.copy_from_slice(&acc);
            }
        }
    }
    let mut out = vec![0u8; nw * nh * 4];
    for y in 0..nh {
        let (lo, ws) = spans(y, nh, sh);
        for x in 0..nw {
            let mut acc = [0f32; 4];
            for (k, w) in ws.iter().enumerate() {
                let i = ((lo + k) * nw + x) * 4;
                if let Some(px) = tmp.get(i..i + 4) {
                    for (a, v) in acc.iter_mut().zip(px) {
                        *a += v * w;
                    }
                }
            }
            if let Some(dst) = out.get_mut((y * nw + x) * 4..(y * nw + x) * 4 + 4) {
                for (d, a) in dst.iter_mut().zip(acc) {
                    *d = a.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
    ImageRgba { width: nw as u32, height: nh as u32, rgba: out, ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(img: &image::RgbaImage, f: image::ImageFormat) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        match f {
            image::ImageFormat::Png => {
                img.write_to(&mut out, f).expect("PNG encode");
            }
            image::ImageFormat::Jpeg => {
                let rgb = img.to_rgb8();
                rgb.write_to(&mut out, f).expect("JPEG encode");
            }
            _ => panic!("unsupported format in test"),
        }
        out.into_inner()
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255]));
        encode(&img, image::ImageFormat::Png)
    }

    #[test]
    fn read_image_fits_within_max_side() {
        let small = read_image(&png(4, 2), 256).expect("decodes");
        assert_eq!((small.width, small.height), (4, 2));
        for (w, h) in [(1000, 500), (300, 300), (513, 77), (64, 700)] {
            let bytes = png(w, h);
            let r = read_image(&bytes, 256).expect("decodes");
            assert!(r.is_valid());
            assert!(r.width <= 256 && r.height <= 256, "{w}x{h} -> {}x{}", r.width, r.height);
            assert!(r.width == 256 || r.height == 256);
            assert_eq!(r, read_image(&bytes, 256).expect("decodes"));
        }
        // A flat colour stays flat.
        let flat = image::RgbaImage::from_pixel(600, 400, image::Rgba([10, 200, 30, 255]));
        let r = read_image(&encode(&flat, image::ImageFormat::Png), 100).expect("decodes");
        assert_eq!((r.width, r.height), (100, 67));
        assert!(r.rgba.chunks_exact(4).all(|p| p == [10, 200, 30, 255]));
    }

    #[test]
    fn read_image_exactly_max_side_not_resized() {
        let img = image::RgbaImage::from_fn(256, 256, |x, y| image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255]));
        let bytes = encode(&img, image::ImageFormat::Png);
        let r = read_image(&bytes, 256).expect("decodes");
        assert_eq!((r.width, r.height), (256, 256));
    }

    #[test]
    fn read_image_handles_1x1() {
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([10, 20, 30, 255]));
        let bytes = encode(&img, image::ImageFormat::Png);
        let r = read_image(&bytes, 256).expect("decodes");
        assert_eq!((r.width, r.height), (1, 1));
        assert_eq!(r.rgba, vec![10, 20, 30, 255]);
    }

    #[test]
    fn read_image_handles_jpeg() {
        let img = image::RgbaImage::from_fn(100, 100, |x, y| image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255]));
        let bytes = encode(&img, image::ImageFormat::Jpeg);
        let r = read_image(&bytes, 256).expect("decodes");
        assert_eq!((r.width, r.height), (100, 100));
    }

    #[test]
    fn read_image_refuses_other_formats() {
        assert!(read_image(&[], 256).is_err());
        assert!(read_image(b"GIF89a\x01\x00\x01\x00\x00\x00\x00;", 256).is_err());
        assert!(read_image(b"not an image at all", 256).is_err());
        let mut bmp = b"BM".to_vec();
        bmp.extend_from_slice(&[0u8; 60]);
        assert!(read_image(&bmp, 256).is_err());
        assert!(read_image(&png(2, 2), 0).is_err());
        let p = png(8, 8);
        assert!(read_image(&p[..p.len() / 2], 256).is_err());
    }
}
