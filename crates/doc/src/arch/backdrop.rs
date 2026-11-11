//! The backdrop behind the 3D view.

use serde::{Deserialize, Serialize};

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
}
