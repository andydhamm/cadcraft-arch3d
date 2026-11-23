use serde_json::json;
use std::sync::OnceLock;

const MAX_TENSOR_COUNT: usize = 256;
const MAX_NAME_LEN: usize = 128;
const MAX_DIM_COUNT: usize = 4;
const MAX_ELEMENTS: u64 = 16_000_000;

/// A tensor decoded from a `.tza` weights file.
#[derive(Clone, Debug, PartialEq)]
pub struct Tensor {
    pub name: String,
    pub dims: Vec<u32>,
    pub layout: String,
    pub data: Vec<f32>,
}

/// A learned denoiser model embedded in the native binary.
#[derive(Clone, Debug)]
pub struct Model {
    pub name: &'static str,
    pub file: &'static str,
    pub bytes: usize,
    pub sha256: String,
    pub tensors: Vec<Tensor>,
}

impl Model {
    /// Look up a tensor by its canonical name.
    pub fn tensor(&self, name: &str) -> Option<&Tensor> {
        self.tensors.iter().find(|tensor| tensor.name == name)
    }
}

/// Which native denoiser weights blob to load.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelChoice {
    Base,
    Small,
}

/// Convert a 16-bit IEEE-754 half value to f32.
pub fn half_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exponent = ((h >> 10) & 0x1F) as i32;
    let mantissa = (h & 0x03FF) as u32;

    let bits = match exponent {
        0 if mantissa == 0 => sign << 31,
        0 => {
            let mut mant = mantissa;
            let mut exp = -14;
            while (mant & 0x400) == 0 {
                mant <<= 1;
                exp -= 1;
            }
            let mant = mant & 0x3FF;
            ((sign << 31) | (((exp + 127) as u32) << 23) | (mant << 13))
        }
        0x1F if mantissa == 0 => (sign << 31) | 0x7F800000,
        0x1F => (sign << 31) | 0x7F800000 | (mantissa << 13),
        _ => ((sign << 31) | (((exponent + 127 - 15) as u32) << 23) | (mantissa << 13)),
    };

    f32::from_bits(bits)
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let range = offset..offset + 2;
    if !bytes.get(range.clone()).is_some() {
        return Err(format!("u16 read out of range at offset {offset}"));
    }
    Ok(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let range = offset..offset + 4;
    if !bytes.get(range.clone()).is_some() {
        return Err(format!("u32 read out of range at offset {offset}"));
    }
    Ok(u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]))
}

fn read_u64_le(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let range = offset..offset + 8;
    if !bytes.get(range.clone()).is_some() {
        return Err(format!("u64 read out of range at offset {offset}"));
    }
    Ok(u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ]))
}

fn table_offset(bytes: &[u8]) -> Result<usize, String> {
    let max_probe = bytes.len().min(128);
    for candidate in (4..=max_probe).step_by(4) {
        if bytes.len() < candidate + 8 {
            continue;
        }
        let offset = read_u64_le(bytes, candidate)? as usize;
        if offset >= 64 && offset < bytes.len() {
            return Ok(offset);
        }
    }
    Err("missing tensor table offset".to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be,
        0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa,
        0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85,
        0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f,
        0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    let mut h = [0x6a09e667u32, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];

    let mut padded = bytes.to_vec();
    let bit_len = (bytes.len() as u64) * 8;
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut h_ = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h_.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h_ = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_);
    }

    let mut out = String::with_capacity(64);
    for value in h {
        out.push_str(&format!("{value:08x}"));
    }
    out
}

/// Parse a `.tza` weights blob.
pub fn parse(bytes: &[u8]) -> Result<Vec<Tensor>, String> {
    if bytes.len() < 12 {
        return Err("tza file too short".to_string());
    }

    let magic = read_u16_le(bytes, 0)?;
    if magic != 0x41D7 {
        return Err(format!("bad tza magic: 0x{magic:04x}"));
    }
    let major = bytes[2];
    let minor = bytes[3];
    if major != 2 || minor != 0 {
        return Err(format!("unsupported tza version {major}.{minor}"));
    }

    let table_start = table_offset(bytes)?;
    if table_start + 4 > bytes.len() {
        return Err("tensor table is truncated".to_string());
    }

    let tensor_count = read_u32_le(bytes, table_start)? as usize;
    if tensor_count > MAX_TENSOR_COUNT {
        return Err(format!("tensor count {tensor_count} exceeds the 256 tensor cap"));
    }

    let mut cursor = table_start + 4;
    let mut tensors = Vec::with_capacity(tensor_count);

    for _ in 0..tensor_count {
        if cursor + 2 > bytes.len() {
            return Err("tensor name length is truncated".to_string());
        }
        let name_len = read_u16_le(bytes, cursor)? as usize;
        cursor += 2;
        if name_len == 0 || name_len > MAX_NAME_LEN {
            return Err(format!("tensor name length {name_len} is invalid"));
        }
        if cursor + name_len > bytes.len() {
            return Err("tensor name is truncated".to_string());
        }
        let name_bytes = &bytes[cursor..cursor + name_len];
        cursor += name_len;
        let name = std::str::from_utf8(name_bytes).map_err(|err| format!("tensor name is not valid UTF-8: {err}"))?;
        if !name.is_ascii() {
            return Err(format!("tensor name '{name}' is not ASCII"));
        }

        if cursor + 1 > bytes.len() {
            return Err("tensor dimension count is missing".to_string());
        }
        let dim_count = bytes[cursor] as usize;
        cursor += 1;
        if dim_count == 0 || dim_count > MAX_DIM_COUNT {
            return Err(format!("tensor '{name}' has {dim_count} dimensions, outside the 1..=4 range"));
        }

        let mut dims = Vec::with_capacity(dim_count);
        let mut total_elements = 1u64;
        for _ in 0..dim_count {
            if cursor + 4 > bytes.len() {
                return Err(format!("tensor '{name}' has a truncated dimension"));
            }
            let dim = read_u32_le(bytes, cursor)?;
            cursor += 4;
            if dim == 0 {
                return Err(format!("tensor '{name}' has a zero-sized dimension"));
            }
            total_elements = total_elements.checked_mul(dim as u64).ok_or_else(|| format!("tensor '{name}' exceeds the 16M element cap"))?;
            dims.push(dim);
        }
        if total_elements > MAX_ELEMENTS {
            return Err(format!("tensor '{name}' exceeds the 16M element cap"));
        }

        if cursor + dim_count > bytes.len() {
            return Err(format!("tensor '{name}' is missing its layout string"));
        }
        let layout =
            String::from_utf8(bytes[cursor..cursor + dim_count].to_vec()).map_err(|err| format!("layout for '{name}' is not valid UTF-8: {err}"))?;
        cursor += dim_count;

        if cursor + 1 > bytes.len() {
            return Err(format!("tensor '{name}' is missing its type byte"));
        }
        let type_char = bytes[cursor] as char;
        cursor += 1;
        if type_char != 'h' && type_char != 'f' {
            return Err(format!("tensor '{name}' uses unsupported type '{type_char}'"));
        }

        if cursor + 8 > bytes.len() {
            return Err(format!("tensor '{name}' has a truncated data offset"));
        }
        let data_offset = read_u64_le(bytes, cursor)? as usize;
        cursor += 8;
        if data_offset < 64 || data_offset > bytes.len() {
            return Err(format!("tensor '{name}' data offset {data_offset} is outside the file"));
        }

        let bytes_per_elem = match type_char {
            'h' => 2,
            'f' => 4,
            _ => unreachable!(),
        };
        let total_data_bytes = total_elements.checked_mul(bytes_per_elem as u64).ok_or_else(|| format!("tensor '{name}' data size overflows"))?;
        let end = data_offset.checked_add(total_data_bytes as usize).ok_or_else(|| format!("tensor '{name}' data range overflows the file"))?;
        if end > bytes.len() {
            return Err(format!("tensor '{name}' data runs past the end of the file"));
        }

        let mut data = Vec::with_capacity(total_elements as usize);
        let payload = &bytes[data_offset..end];
        match type_char {
            'h' => {
                if payload.len() % 2 != 0 {
                    return Err(format!("tensor '{name}' data length is not a multiple of 2"));
                }
                for chunk in payload.chunks_exact(2) {
                    let word = u16::from_le_bytes([chunk[0], chunk[1]]);
                    data.push(half_to_f32(word));
                }
            }
            'f' => {
                if payload.len() % 4 != 0 {
                    return Err(format!("tensor '{name}' data length is not a multiple of 4"));
                }
                for chunk in payload.chunks_exact(4) {
                    let word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    data.push(f32::from_bits(word));
                }
            }
            _ => unreachable!(),
        }

        if data.len() != total_elements as usize {
            return Err(format!("tensor '{name}' data size mismatch: expected {total_elements}, got {}", data.len()));
        }

        tensors.push(Tensor { name: name.to_string(), dims, layout, data });
    }

    Ok(tensors)
}

#[cfg(not(target_arch = "wasm32"))]
fn native_models() -> Result<[Model; 2], String> {
    const BASE_BYTES: &[u8] = include_bytes!("../../../assets/denoise/rt_hdr_alb_nrm.tza");
    const SMALL_BYTES: &[u8] = include_bytes!("../../../assets/denoise/rt_hdr_alb_nrm_small.tza");

    let base_name = "base";
    let small_name = "small";
    let base_tensors = parse(BASE_BYTES)?;
    let small_tensors = parse(SMALL_BYTES)?;

    let base = Model { name: base_name, file: "rt_hdr_alb_nrm.tza", bytes: BASE_BYTES.len(), sha256: sha256_hex(BASE_BYTES), tensors: base_tensors };
    let small = Model {
        name: small_name,
        file: "rt_hdr_alb_nrm_small.tza",
        bytes: SMALL_BYTES.len(),
        sha256: sha256_hex(SMALL_BYTES),
        tensors: small_tensors,
    };

    Ok([base, small])
}

#[cfg(not(target_arch = "wasm32"))]
static NATIVE_MODELS: OnceLock<Result<[Model; 2], String>> = OnceLock::new();

/// Return the embedded learned denoiser model.
#[cfg(not(target_arch = "wasm32"))]
pub fn model(which: ModelChoice) -> Result<&'static Model, String> {
    let models = NATIVE_MODELS.get_or_init(native_models);
    match models {
        Ok([base, small]) => match which {
            ModelChoice::Base => Ok(base),
            ModelChoice::Small => Ok(small),
        },
        Err(err) => Err(err.clone()),
    }
}

#[cfg(target_arch = "wasm32")]
pub fn model(_which: ModelChoice) -> Result<&'static Model, String> {
    Err("the learned denoiser is native only".to_string())
}

/// Metadata describing the embedded learned denoiser models.
pub fn info() -> serde_json::Value {
    #[cfg(target_arch = "wasm32")]
    {
        return json!({ "models": [] });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut models = Vec::new();
        for which in [ModelChoice::Base, ModelChoice::Small] {
            if let Ok(model) = model(which) {
                let dims = model.tensor("enc_conv0.weight").map(|tensor| tensor.dims.clone()).unwrap_or_default();
                models.push(json!({
                    "name": model.name,
                    "file": model.file,
                    "bytes": model.bytes,
                    "sha256": model.sha256,
                    "tensors": model.tensors.len(),
                    "encConv0": dims,
                }));
            }
        }
        json!({ "models": models })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic;

    #[test]
    fn both_weight_files_have_32_tensors() {
        let base = model(ModelChoice::Base).expect("base weights should parse");
        let small = model(ModelChoice::Small).expect("small weights should parse");
        assert_eq!(base.tensors.len(), 32, "base model should contain 32 tensors");
        assert_eq!(small.tensors.len(), 32, "small model should contain 32 tensors");
    }

    #[test]
    fn enc_conv0_weight_is_32_9_3_3() {
        let base = model(ModelChoice::Base).expect("base weights should parse");
        let small = model(ModelChoice::Small).expect("small weights should parse");
        let base_weight = base.tensor("enc_conv0.weight").expect("base enc_conv0.weight missing");
        let small_weight = small.tensor("enc_conv0.weight").expect("small enc_conv0.weight missing");
        assert_eq!(base_weight.dims, vec![32, 9, 3, 3]);
        assert_eq!(small_weight.dims, vec![32, 9, 3, 3]);
        assert_eq!(base_weight.layout, "oihw");
        assert_eq!(small_weight.layout, "oihw");
    }

    #[test]
    fn sha256_matches_the_readme() {
        let base = model(ModelChoice::Base).expect("base weights should parse");
        let small = model(ModelChoice::Small).expect("small weights should parse");
        assert_eq!(base.sha256, "e586ef2ff48d7fbb7611986405220ed8fc5c13ca79bfc40be4dc742fbf959e1a");
        assert_eq!(small.sha256, "980ea307e7825cae23ed8b1addf8de62ece84b61094795a5631ebf1c91b2e1a7");
    }

    #[test]
    fn half_to_f32_known_values() {
        assert_eq!(half_to_f32(0x3C00), 1.0);
        assert_eq!(half_to_f32(0xC000), -2.0);
        assert!((half_to_f32(0x0001) - 2f32.powi(-24)).abs() < 1e-8);
    }

    #[test]
    fn hostile_tza_never_panics() {
        let base = include_bytes!("../../../assets/denoise/rt_hdr_alb_nrm.tza");

        let truncated = &base[..base.len() - 7];
        let mut corrupted = base.to_vec();
        corrupted[0] ^= 0xFF;
        corrupted[1] ^= 0xFF;

        for payload in [truncated, &corrupted[..]] {
            let result = panic::catch_unwind(|| parse(payload));
            assert!(result.is_ok(), "parse panicked on hostile input");
            assert!(parse(payload).is_err(), "hostile input should not parse successfully");
        }

        let mut mutated = base.to_vec();
        for i in 0..200 {
            let offset = (i * 137) % mutated.len();
            mutated[offset] = mutated[offset].wrapping_add((i as u8) + 1);
            let result = panic::catch_unwind(|| parse(&mutated));
            assert!(result.is_ok(), "fuzzed input panicked at mutation {i}");
            let _ = result.ok();
        }
    }
}
