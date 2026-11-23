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
                "clearcoatFactor": r6(ext.clearcoat),
                "clearcoatRoughnessFactor": r6(ext.clearcoat_roughness),
            }),
        );
    }

    if ext.anisotropy > 0.0 {
        used.push("KHR_materials_anisotropy");
        map.insert(
            "KHR_materials_anisotropy".to_string(),
            json!({
                "anisotropyStrength": r6(ext.anisotropy),
                "anisotropyRotation": r6(ext.anisotropy_rotation),
            }),
        );
    }

    if ext.sheen_color[0] > 0.0 || ext.sheen_color[1] > 0.0 || ext.sheen_color[2] > 0.0 {
        used.push("KHR_materials_sheen");
        map.insert(
            "KHR_materials_sheen".to_string(),
            json!({
                "sheenColorFactor": [
                    r6(ext.sheen_color[0]),
                    r6(ext.sheen_color[1]),
                    r6(ext.sheen_color[2]),
                ],
                "sheenRoughnessFactor": r6(ext.sheen_roughness),
            }),
        );
    }

    if ext.transmission > 0.0 {
        used.push("KHR_materials_transmission");
        map.insert(
            "KHR_materials_transmission".to_string(),
            json!({
                "transmissionFactor": r6(ext.transmission),
            }),
        );
        if !ext.thin && ext.thickness > 0.0 {
            used.push("KHR_materials_volume");
            map.insert(
                "KHR_materials_volume".to_string(),
                json!({
                    "thicknessFactor": r6(ext.thickness),
                }),
            );
        }
    }

    (map, used)
}
