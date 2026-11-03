//! A minimal PNG encoder (8-bit RGBA, no interlace) for textures embedded in the GLB.
//!
//! Signature, `IHDR`, one `IDAT` of filter-0 scanlines compressed with `miniz_oxide` at a fixed
//! level, `IEND`; the CRC-32 is computed here. The same input gives the same bytes.

use crate::IoError;

/// The deflate level used for every image (fixed, so output is reproducible).
const LEVEL: u8 = 6;

/// Largest width or height accepted (PNG itself allows 2^31 - 1).
const MAX_SIDE: u32 = 16_384;

/// CRC-32 (IEEE, reflected) of `parts` concatenated.
fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for part in parts {
        for &b in *part {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
    }
    !crc
}

/// Append one chunk: length, type, data, CRC over type and data.
fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) -> Result<(), IoError> {
    let len = u32::try_from(data.len()).map_err(|_| IoError::Format("PNG chunk too large".into()))?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
    Ok(())
}

/// Encode `rgba` (4 bytes per pixel, row-major, `width * height` pixels) as a PNG file.
///
/// Errors when a side is zero or above 16384, or the buffer length does not match.
pub fn encode_rgba8(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, IoError> {
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(IoError::Format(format!("PNG size {width}x{height} is out of range")));
    }
    let row = width as usize * 4;
    let expected = row * height as usize;
    if rgba.len() != expected {
        return Err(IoError::Format(format!("PNG pixel buffer has {} bytes, expected {expected}", rgba.len())));
    }
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for line in rgba.chunks_exact(row) {
        raw.push(0);
        raw.extend_from_slice(line);
    }
    let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, LEVEL);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    chunk(&mut out, b"IHDR", &ihdr)?;
    chunk(&mut out, b"IDAT", &z)?;
    chunk(&mut out, b"IEND", &[])?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixels(w: u32, h: u32) -> Vec<u8> {
        (0..w * h * 4).map(|i| (i.wrapping_mul(7) ^ (i >> 3)) as u8).collect()
    }

    #[test]
    fn png_decodes_to_the_same_pixels() {
        let (w, h) = (13u32, 7u32);
        let px = pixels(w, h);
        let png = encode_rgba8(w, h, &px).expect("encode");
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let mut at = 8;
        let mut idat = Vec::new();
        let mut seen = Vec::new();
        while at + 12 <= png.len() {
            let len = u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]) as usize;
            let kind = png[at + 4..at + 8].to_vec();
            let data = &png[at + 8..at + 8 + len];
            let crc = u32::from_be_bytes([png[at + 8 + len], png[at + 9 + len], png[at + 10 + len], png[at + 11 + len]]);
            assert_eq!(crc, crc32(&[&kind, data]), "crc of {kind:?}");
            if kind == b"IHDR" {
                assert_eq!(u32::from_be_bytes([data[0], data[1], data[2], data[3]]), w);
                assert_eq!(u32::from_be_bytes([data[4], data[5], data[6], data[7]]), h);
                assert_eq!(&data[8..], &[8, 6, 0, 0, 0]);
            }
            if kind == b"IDAT" {
                idat.extend_from_slice(data);
            }
            seen.push(String::from_utf8_lossy(&kind).into_owned());
            at += 12 + len;
        }
        assert_eq!(at, png.len());
        assert_eq!(seen, ["IHDR", "IDAT", "IEND"]);
        let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&idat).expect("inflate");
        let row = w as usize * 4;
        assert_eq!(raw.len(), (row + 1) * h as usize);
        let mut got = Vec::new();
        for line in raw.chunks_exact(row + 1) {
            assert_eq!(line[0], 0, "filter 0");
            got.extend_from_slice(&line[1..]);
        }
        assert_eq!(got, px);
    }

    #[test]
    fn png_is_byte_deterministic() {
        let px = pixels(32, 32);
        assert_eq!(encode_rgba8(32, 32, &px).expect("a"), encode_rgba8(32, 32, &px).expect("b"));
    }

    #[test]
    fn png_refuses_bad_sizes() {
        assert!(encode_rgba8(0, 1, &[]).is_err());
        assert!(encode_rgba8(2, 2, &[0; 15]).is_err());
        assert!(encode_rgba8(u32::MAX, 1, &[]).is_err());
    }
}
