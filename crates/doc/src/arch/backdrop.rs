//! The backdrop behind the 3D view.

use serde::{Deserialize, Serialize};

/// A decoded Radiance RGBE image pixel buffer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageRgbe {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Raw RGBE scanline data: `[r, g, b, e]` per pixel (a lower-case hex string in JSON).
    #[serde(with = "rgbe_hex")]
    pub rgbe: Vec<u8>,
}

impl ImageRgbe {
    /// Decode one pixel to linear float RGB. Returns `[0.0; 3]` when the coordinate is out of range.
    pub fn pixel_rgb(&self, x: u32, y: u32) -> [f64; 3] {
        if x >= self.width || y >= self.height {
            return [0.0; 3];
        }
        let idx = (y as usize).checked_mul(self.width as usize).and_then(|row| row.checked_add(x as usize)).and_then(|i| i.checked_mul(4));
        let Some(idx) = idx else {
            return [0.0; 3];
        };
        let px = match self.rgbe.get(idx..idx + 4) {
            Some(values) => values,
            None => return [0.0; 3],
        };
        let [r, g, b, e] = [px[0], px[1], px[2], px[3]];
        let factor = if e == 0 { 0.0 } else { 2f64.powi(i32::from(e) - 136) };
        [r as f64 * factor, g as f64 * factor, b as f64 * factor]
    }

    /// Decode each pixel to linear float RGB.
    pub fn to_rgb(&self) -> Vec<[f64; 3]> {
        let mut rgb = Vec::with_capacity(self.rgbe.len().saturating_div(4));
        for y in 0..self.height {
            for x in 0..self.width {
                rgb.push(self.pixel_rgb(x, y));
            }
        }
        rgb
    }

    /// Whether the buffer length matches the dimensions and the stored size is within the
    /// model caps used for HDR backdrops.
    pub fn is_valid(&self) -> bool {
        let expected = (self.width as usize).checked_mul(self.height as usize).and_then(|n| n.checked_mul(4));
        matches!(expected, Some(len) if len == self.rgbe.len()) && (1..=1024).contains(&self.width) && (1..=512).contains(&self.height)
    }
}

/// Serde helper: the RGBE bytes as a lower-case hex string. Bad hex or an odd length reads as no bytes, which
/// [`ImageRgbe::is_valid`] refuses.
mod rgbe_hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut hex = String::with_capacity(bytes.len().saturating_mul(2));
        for b in bytes {
            hex.push(char::from(DIGITS[usize::from(b >> 4)]));
            hex.push(char::from(DIGITS[usize::from(b & 15)]));
        }
        serializer.serialize_str(&hex)
    }

    fn nibble(c: u8) -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let hex = String::deserialize(deserializer)?;
        if hex.len() % 2 != 0 {
            return Ok(Vec::new());
        }
        let mut bytes = Vec::with_capacity(hex.len() / 2);
        for pair in hex.as_bytes().chunks_exact(2) {
            match (pair.first().copied().and_then(nibble), pair.get(1).copied().and_then(nibble)) {
                (Some(hi), Some(lo)) => bytes.push((hi << 4) | lo),
                _ => return Ok(Vec::new()),
            }
        }
        Ok(bytes)
    }
}

/// How a backdrop image maps onto the view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Projection {
    /// A flat picture behind the scene.
    #[default]
    Flat,
    /// An equirectangular panorama.
    Equirect,
}

/// The background of a 3D view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Backdrop {
    /// A single colour.
    Colour {
        /// sRGB colour.
        colour: [u8; 3],
    },
    /// A vertical gradient.
    Gradient {
        /// sRGB colour at the top.
        top: [u8; 3],
        /// sRGB colour at the bottom.
        bottom: [u8; 3],
    },
    /// An image from `ArchModel.images`.
    Image {
        /// Library item id.
        item: String,
        /// Mapping.
        projection: Projection,
        /// Degrees the horizon line sits above the picture's middle.
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        horizon: f64,
    },
    /// The procedural sky (drawn by the renderers).
    Sky,
    /// A high-dynamic-range environment image.
    Hdri {
        /// Library item id.
        image: String,
        /// Brightness multiplier.
        #[serde(default = "one")]
        intensity: f64,
        /// Rotation about the vertical axis, degrees.
        #[serde(default)]
        rotation: f64,
        /// Horizon offset, degrees.
        #[serde(default)]
        horizon: f64,
    },
}

fn one() -> f64 {
    1.0
}

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backdrop_serde_names() {
        let c = serde_json::to_value(Backdrop::Colour { colour: [1, 2, 3] }).unwrap();
        assert_eq!(c, serde_json::json!({"kind": "colour", "colour": [1, 2, 3]}));
        let g = serde_json::to_value(Backdrop::Gradient { top: [1, 2, 3], bottom: [4, 5, 6] }).unwrap();
        assert_eq!(g["kind"], "gradient");
        let i = Backdrop::Image { item: "sky".into(), projection: Projection::Equirect, horizon: 0.0 };
        let v = serde_json::to_value(&i).unwrap();
        assert_eq!(v, serde_json::json!({"kind": "image", "item": "sky", "projection": "equirect"}));
        assert_eq!(serde_json::from_value::<Backdrop>(v).unwrap(), i);
        assert_eq!(serde_json::to_string(&Projection::Flat).unwrap(), "\"flat\"");
        assert_eq!(Projection::default(), Projection::Flat);
    }

    #[test]
    fn image_rgbe_hex_round_trip_and_bad_hex_is_invalid() {
        let img = ImageRgbe { width: 2, height: 1, rgbe: vec![0x10, 0x20, 0x30, 0x80, 0x40, 0x50, 0x60, 0x81] };
        let json = serde_json::to_string(&img).unwrap();
        let back: ImageRgbe = serde_json::from_str(&json).unwrap();
        assert_eq!(back.rgbe, img.rgbe);
        assert!(serde_json::from_str::<ImageRgbe>("\"zz\"").is_err());
        assert!(serde_json::from_str::<ImageRgbe>("\"123\"").is_err());
    }
}
