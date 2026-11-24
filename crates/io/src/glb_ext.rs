//! KHR material extensions for glTF: clearcoat, anisotropy, sheen, transmission and volume.
//!
//! The path-tracer-only overrides (`metallic`, `roughness`), `edge_radius` and variation
//! are never exported: rounded edges and variation change nothing in the file.

use cadcraft_arch3d::MaterialExt;
use serde_json::{Map, Value, json};

/// Round a float to 6 decimal places so two exports of the same scene are byte-identical.
fn r6(x: f64) -> f64 {
    (x * 1_000_000.0).round() / 1_000_000.0
}

/// The glTF `extensions` object for a material and the extension names it uses.
///
/// Returns an empty map and an empty name list when no KHR extension applies.
/// Emits `KHR_materials_clearcoat` when `clearcoat > 0`, `KHR_materials_anisotropy`
/// when `anisotropy > 0`, `KHR_materials_sheen` when any sheen channel is `> 0`,
/// `KHR_materials_transmission` when `transmission > 0`, and `KHR_materials_volume`
/// when `transmission > 0` and the material is not thin with `thickness > 0`.
pub(crate) fn material_extensions(ext: &MaterialExt) -> (Map<String, Value>, Vec<&'static str>) {
    let mut map: Map<String, Value> = Map::new();
    let mut used: Vec<&'static str> = Vec::new();

    if ext.clearcoat > 0.0 {
        used.push("KHR_materials_clearcoat");
        map.insert(
            "KHR_materials_clearcoat".to_string(),
            json!({
                "clearcoatFactor": r6(ext.clearcoat.into()),
                "clearcoatRoughnessFactor": r6(ext.clearcoat_roughness.into()),
            }),
        );
    }

    if ext.anisotropy > 0.0 {
        used.push("KHR_materials_anisotropy");
        map.insert(
            "KHR_materials_anisotropy".to_string(),
            json!({
                "anisotropyStrength": r6(ext.anisotropy.into()),
                "anisotropyRotation": r6(ext.anisotropy_rotation.into()),
            }),
        );
    }

    if ext.sheen[0] > 0.0 || ext.sheen[1] > 0.0 || ext.sheen[2] > 0.0 {
        used.push("KHR_materials_sheen");
        map.insert(
            "KHR_materials_sheen".to_string(),
            json!({
                "sheenColorFactor": [
                    r6(ext.sheen[0].into()),
                    r6(ext.sheen[1].into()),
                    r6(ext.sheen[2].into()),
                ],
                "sheenRoughnessFactor": r6(ext.sheen_roughness.into()),
            }),
        );
    }

    if ext.transmission > 0.0 {
        used.push("KHR_materials_transmission");
        map.insert(
            "KHR_materials_transmission".to_string(),
            json!({
                "transmissionFactor": r6(ext.transmission.into()),
            }),
        );
        if !ext.thin && ext.thickness > 0.0 {
            used.push("KHR_materials_volume");
            map.insert(
                "KHR_materials_volume".to_string(),
                json!({
                    "thicknessFactor": r6(ext.thickness.into()),
                }),
            );
        }
    }

    (map, used)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_arch3d::MaterialExt;

    #[test]
    fn zero_fields_emit_no_extensions() {
        let ext = MaterialExt::default();
        let (map, used) = material_extensions(&ext);
        assert!(map.is_empty());
        assert!(used.is_empty());
    }

    #[test]
    fn clearcoat_only_emits_clearcoat() {
        let ext = MaterialExt { clearcoat: 0.5, clearcoat_roughness: 0.2, ..MaterialExt::default() };
        let (map, used) = material_extensions(&ext);
        assert_eq!(used, vec!["KHR_materials_clearcoat"]);
        assert!(map.contains_key("KHR_materials_clearcoat"));
        assert!(!map.contains_key("KHR_materials_anisotropy"));
        assert!(!map.contains_key("KHR_materials_sheen"));
        assert!(!map.contains_key("KHR_materials_transmission"));
        assert!(!map.contains_key("KHR_materials_volume"));
    }

    #[test]
    fn thin_transmission_emits_no_volume() {
        let ext = MaterialExt { transmission: 0.8, thin: true, thickness: 0.01, ..MaterialExt::default() };
        let (map, used) = material_extensions(&ext);
        assert!(used.contains(&"KHR_materials_transmission"));
        assert!(!used.contains(&"KHR_materials_volume"));
        assert!(!map.contains_key("KHR_materials_volume"));
    }

    #[test]
    fn all_fields_emit_all_extensions() {
        let ext = MaterialExt {
            clearcoat: 0.5,
            clearcoat_roughness: 0.2,
            anisotropy: 0.8,
            anisotropy_rotation: 0.5,
            sheen: [0.3, 0.4, 0.5],
            sheen_roughness: 0.6,
            transmission: 0.7,
            thin: false,
            thickness: 0.02,
            ..MaterialExt::default()
        };
        let (map, used) = material_extensions(&ext);
        assert_eq!(used.len(), 5);
        assert!(used.contains(&"KHR_materials_clearcoat"));
        assert!(used.contains(&"KHR_materials_anisotropy"));
        assert!(used.contains(&"KHR_materials_sheen"));
        assert!(used.contains(&"KHR_materials_transmission"));
        assert!(used.contains(&"KHR_materials_volume"));
        assert_eq!(map.len(), 5);
    }

    #[test]
    fn nan_fields_emit_no_extensions() {
        let ext = MaterialExt {
            clearcoat: f64::NAN,
            anisotropy: f64::NAN,
            sheen: [f64::NAN, f64::NAN, f64::NAN],
            transmission: f64::NAN,
            thickness: f64::NAN,
            ..MaterialExt::default()
        };
        let (map, used) = material_extensions(&ext);
        assert!(map.is_empty());
        assert!(used.is_empty());
    }

    #[test]
    fn values_rounded_to_six_decimals() {
        let ext = MaterialExt { clearcoat: 0.1234567, clearcoat_roughness: 0.9876543, ..MaterialExt::default() };
        let (map, _used) = material_extensions(&ext);
        let cc = &map["KHR_materials_clearcoat"];
        assert_eq!(cc["clearcoatFactor"].as_f64().unwrap(), 0.123457);
        assert_eq!(cc["clearcoatRoughnessFactor"].as_f64().unwrap(), 0.987654);
    }
}
