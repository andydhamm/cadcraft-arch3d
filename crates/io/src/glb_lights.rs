//! `KHR_lights_punctual` for the GLB (phase 10): one glTF light per scene light (spots with their
//! cone, intensity in candela as glTF wants for punctual lights) and one node per light under the
//! root, named `light-lf{n}-source`. Filled by `p10-io-glb-lights`.
//!
//! Phase 11: an area light is a spot of the widest cone (inner 0, outer pi/2) at its candela
//! (lumens / pi) with its size in the node's extras, `{"cadcraft": {"area": [width, length]}}`; an
//! IES fixture carries its peak candela and its profile's name, `{"cadcraft": {"ies": name}}`.

use cadcraft_arch3d::Scene3d;
use cadcraft_doc::arch::LightKind;
use cadcraft_geom::Vec3;
use serde_json::{Value, json};

/// The extension's `lights` array and one node per light (`{"name", "translation",
/// "rotation"?, "extensions": {"KHR_lights_punctual": {"light": i}}}`), both in
/// `scene.lights` order; empty when the scene has no lights.
pub(crate) fn punctual_lights(scene: &Scene3d) -> (Vec<Value>, Vec<Value>) {
    let mut lights = Vec::new();
    let mut nodes = Vec::new();

    for light in &scene.lights {
        // A strip has no KHR light; only its housing node is exported.
        if light.kind == LightKind::Strip || !light_valid(light) {
            continue;
        }

        // An IES fixture's intensity is the peak of its (lumen-scaled) profile; every other light
        // carries its candela.
        let intensity = match &light.ies {
            Some(profile) => cadcraft_arch::ies::peak_candela(profile),
            None => light.intensity,
        };
        if !intensity.is_finite() || intensity < 0.0 {
            continue;
        }

        let name = format!("lf{}", light.id.0);
        let mut def = json!({
            "name": name,
            "type": if matches!(light.kind, LightKind::Spot | LightKind::Area) { "spot" } else { "point" },
            "color": [light.color[0], light.color[1], light.color[2]],
            "intensity": intensity,
        });

        match light.kind {
            LightKind::Spot => {
                let mut inner = cone_angle(light.cos_inner);
                let outer = (cone_angle(light.cos_outer)).min(std::f64::consts::PI / 2.0);
                if !inner.is_finite() || !outer.is_finite() {
                    continue;
                }
                // glTF requires 0 <= innerConeAngle < outerConeAngle <= PI/2.
                // If they are equal (uniform cone) or inner >= outer, shrink inner slightly.
                if inner >= outer {
                    inner = (outer - 0.001).max(0.0);
                }
                def["spot"] = json!({
                    "innerConeAngle": inner,
                    "outerConeAngle": outer,
                });
            }
            // glTF has no area light: the widest spot approximates the hemisphere an area emits
            // into, and the extras keep the size for a reader that knows it.
            LightKind::Area => {
                def["spot"] = json!({
                    "innerConeAngle": 0.0,
                    "outerConeAngle": std::f64::consts::PI / 2.0,
                });
            }
            // A strip is skipped above; this arm only completes the match.
            LightKind::Point | LightKind::Strip => {}
        }

        let light_index = lights.len();
        lights.push(def);

        let mut node = json!({
            "name": format!("light-{}-source", name),
            "translation": [light.position.x, light.position.y, light.position.z],
            "extensions": {
                "KHR_lights_punctual": {
                    "light": light_index
                }
            }
        });

        if let Some(rotation) = quaternion_from_direction(light.direction) {
            node["rotation"] = json!(rotation);
        }
        let mut cadcraft = serde_json::Map::new();
        if light.kind == LightKind::Area && light.size.iter().all(|v| v.is_finite()) {
            cadcraft.insert("area".into(), json!([light.size[0], light.size[1]]));
        }
        if let Some(profile) = &light.ies {
            cadcraft.insert("ies".into(), json!(profile.name));
        }
        if !cadcraft.is_empty() {
            node["extras"] = json!({ "cadcraft": cadcraft });
        }
        nodes.push(node);
    }

    (lights, nodes)
}

fn light_valid(light: &cadcraft_arch3d::light::SceneLight) -> bool {
    if !light.position.is_finite() || !light.direction.is_finite() || !light.color.iter().all(|c| c.is_finite()) {
        return false;
    }
    if !light.intensity.is_finite() || light.intensity < 0.0 {
        return false;
    }
    let direction = light.direction.normalized();
    if !direction.is_finite() || direction.len() < 1e-12 {
        return false;
    }
    match light.kind {
        // An area light's cone is fixed (the whole hemisphere), whatever its cosines say.
        LightKind::Point | LightKind::Area | LightKind::Strip => true,
        LightKind::Spot => {
            light.cos_inner.is_finite()
                && light.cos_outer.is_finite()
                && (-1.0..=1.0).contains(&light.cos_inner)
                && (-1.0..=1.0).contains(&light.cos_outer)
                && light.cos_inner >= light.cos_outer
        }
    }
}

fn cone_angle(cosine: f64) -> f64 {
    let normalized = cosine.clamp(-1.0, 1.0);
    if !normalized.is_finite() {
        return f64::NAN;
    }
    normalized.acos()
}

fn quaternion_from_direction(direction: Vec3) -> Option<[f64; 4]> {
    let dir = direction.normalized();
    if !dir.is_finite() || dir.len() < 1e-12 {
        return None;
    }

    let from = Vec3::new(0.0, 0.0, -1.0);
    if (from - dir).len() < 1e-12 {
        return None;
    }
    if (from + dir).len() < 1e-12 {
        return Some([1.0, 0.0, 0.0, 0.0]);
    }

    let axis = from.cross(dir);
    let axis_len = axis.len();
    if !axis_len.is_finite() || axis_len < 1e-12 {
        return None;
    }

    let axis = axis * (1.0 / axis_len);
    let dot = (from.dot(dir)).clamp(-1.0, 1.0);
    let angle = dot.acos();
    let half = angle / 2.0;
    let s = half.sin();
    let w = half.cos();
    Some([axis.x * s, axis.y * s, axis.z * s, w])
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_arch3d::{Mesh, Scene3d};
    use cadcraft_doc::arch::{ArchId, LightKind};
    use cadcraft_geom::Vec3;
    use serde_json::Value;

    fn scene_light(
        id: u32,
        kind: LightKind,
        position: Vec3,
        direction: Vec3,
        intensity: f64,
        color: [f64; 3],
        cos_inner: f64,
        cos_outer: f64,
    ) -> cadcraft_arch3d::light::SceneLight {
        cadcraft_arch3d::light::SceneLight { id: ArchId(id), kind, position, direction, intensity, color, cos_inner, cos_outer, ..Default::default() }
    }

    fn glb_json_chunk(bytes: &[u8]) -> Value {
        let json_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        let start = 20;
        serde_json::from_slice(&bytes[start..start + json_len]).unwrap_or_default()
    }

    fn rotate_quat(v: Vec3, q: [f64; 4]) -> Vec3 {
        let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
        let norm = (x * x + y * y + z * z + w * w).sqrt();
        let (x, y, z, w) = if norm > 0.0 { (x / norm, y / norm, z / norm, w / norm) } else { (0.0, 0.0, 0.0, 1.0) };
        let qv = Vec3::new(x, y, z);
        let uv = qv.cross(v);
        let uuv = qv.cross(uv);
        let t = uv * (2.0 * w) + uuv * 2.0;
        v + t
    }

    #[test]
    fn punctual_spot_lights_carry_candela_and_cone() {
        let half = 30.0_f64.to_radians();
        let scene = Scene3d {
            lights: vec![
                scene_light(0, LightKind::Spot, Vec3::new(1.0, 1.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 950.36, [1.0, 1.0, 1.0], half.cos(), half.cos()),
                scene_light(1, LightKind::Spot, Vec3::new(3.0, 1.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 950.36, [1.0, 0.8, 0.7], half.cos(), half.cos()),
            ],
            ..Default::default()
        };

        let (lights, _) = punctual_lights(&scene);
        assert_eq!(lights.len(), 2);
        for light in &lights {
            assert_eq!(light["type"], "spot");
            assert!((light["intensity"].as_f64().unwrap_or(f64::NAN) - 950.36).abs() < 0.01);
            let spot = &light["spot"];
            let outer = spot["outerConeAngle"].as_f64().unwrap_or(f64::NAN);
            let inner = spot["innerConeAngle"].as_f64().unwrap_or(f64::NAN);
            assert!((outer - std::f64::consts::PI / 6.0).abs() < 1e-9, "{spot}");
            // glTF requires inner < outer; equal inputs are shrunk by 0.001.
            assert!((inner - (outer - 0.001)).abs() < 1e-9, "{spot}");
            assert!(inner < outer);
        }
    }

    #[test]
    fn punctual_light_nodes_face_their_aim() {
        let dir = Vec3::new(1.0, 0.0, 0.0);
        let scene = Scene3d {
            lights: vec![scene_light(0, LightKind::Spot, Vec3::new(0.0, 0.0, 0.0), dir, 100.0, [1.0, 1.0, 1.0], 0.5, 0.5)],
            ..Default::default()
        };

        let (_, nodes) = punctual_lights(&scene);
        let node = &nodes[0];
        let rotation = node["rotation"].as_array().unwrap();
        let q = [
            rotation[0].as_f64().unwrap_or_default(),
            rotation[1].as_f64().unwrap_or_default(),
            rotation[2].as_f64().unwrap_or_default(),
            rotation[3].as_f64().unwrap_or_default(),
        ];
        let rotated = rotate_quat(Vec3::new(0.0, 0.0, -1.0), q);
        assert!((rotated.x - 1.0).abs() < 1e-9, "rotated={rotated:?}");
        assert!((rotated.y.abs()) < 1e-9);
        assert!((rotated.z.abs()) < 1e-9);
    }

    #[test]
    fn empty_scene_has_no_lights() {
        let scene = Scene3d::default();
        let (lights, nodes) = punctual_lights(&scene);
        assert!(lights.is_empty());
        assert!(nodes.is_empty());
    }

    #[test]
    fn point_light_has_no_spot_section() {
        let scene = Scene3d {
            lights: vec![scene_light(0, LightKind::Point, Vec3::new(0.0, 0.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, 1.0, 1.0], 0.0, 0.0)],
            ..Default::default()
        };
        let (lights, nodes) = punctual_lights(&scene);
        assert_eq!(lights.len(), 1);
        assert_eq!(lights[0]["type"], "point");
        assert!(lights[0].get("spot").is_none());
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn non_finite_light_is_skipped() {
        let scene = Scene3d {
            lights: vec![
                scene_light(0, LightKind::Point, Vec3::new(f64::NAN, 0.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, 1.0, 1.0], 0.0, 0.0),
                scene_light(1, LightKind::Point, Vec3::new(1.0, 1.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, 1.0, 1.0], 0.0, 0.0),
            ],
            ..Default::default()
        };
        let (lights, nodes) = punctual_lights(&scene);
        assert_eq!(lights.len(), 1);
        assert_eq!(lights[0]["name"], "lf1");
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn invalid_spot_cone_is_skipped() {
        let scene = Scene3d {
            lights: vec![
                scene_light(0, LightKind::Spot, Vec3::new(0.0, 0.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, 1.0, 1.0], 0.5, 0.7),
                scene_light(1, LightKind::Spot, Vec3::new(1.0, 1.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, 1.0, 1.0], 0.7, 0.5),
            ],
            ..Default::default()
        };
        let (lights, nodes) = punctual_lights(&scene);
        assert_eq!(lights.len(), 1);
        assert_eq!(lights[0]["name"], "lf1");
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn opposite_direction_gets_180_rotation() {
        let scene = Scene3d {
            lights: vec![scene_light(0, LightKind::Spot, Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), 100.0, [1.0, 1.0, 1.0], 0.5, 0.5)],
            ..Default::default()
        };
        let (_, nodes) = punctual_lights(&scene);
        let node = &nodes[0];
        let rotation = node["rotation"].as_array().unwrap();
        let q = [
            rotation[0].as_f64().unwrap_or_default(),
            rotation[1].as_f64().unwrap_or_default(),
            rotation[2].as_f64().unwrap_or_default(),
            rotation[3].as_f64().unwrap_or_default(),
        ];
        assert!((q[0] - 1.0).abs() < 1e-9);
        assert!(q[1].abs() < 1e-9);
        assert!(q[2].abs() < 1e-9);
        assert!(q[3].abs() < 1e-9);
        let rotated = rotate_quat(Vec3::new(0.0, 0.0, -1.0), q);
        assert!((rotated.z - 1.0).abs() < 1e-9);
    }

    #[test]
    fn outer_cone_capped_at_pi_over_2() {
        let scene = Scene3d {
            lights: vec![scene_light(0, LightKind::Spot, Vec3::new(0.0, 0.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, 1.0, 1.0], 0.0, -0.5)],
            ..Default::default()
        };
        let (lights, _) = punctual_lights(&scene);
        assert_eq!(lights.len(), 1);
        let spot = &lights[0]["spot"];
        let outer = spot["outerConeAngle"].as_f64().unwrap_or(f64::NAN);
        assert!((outer - std::f64::consts::PI / 2.0).abs() < 1e-9);
    }

    #[test]
    fn zero_direction_is_skipped() {
        let scene = Scene3d {
            lights: vec![scene_light(0, LightKind::Point, Vec3::new(0.0, 0.0, 2.4), Vec3::new(0.0, 0.0, 0.0), 100.0, [1.0, 1.0, 1.0], 0.0, 0.0)],
            ..Default::default()
        };
        let (lights, nodes) = punctual_lights(&scene);
        assert!(lights.is_empty());
        assert!(nodes.is_empty());
    }

    #[test]
    fn negative_intensity_is_skipped() {
        let scene = Scene3d {
            lights: vec![scene_light(0, LightKind::Point, Vec3::new(0.0, 0.0, 2.4), Vec3::new(0.0, 0.0, -1.0), -1.0, [1.0, 1.0, 1.0], 0.0, 0.0)],
            ..Default::default()
        };
        let (lights, nodes) = punctual_lights(&scene);
        assert!(lights.is_empty());
        assert!(nodes.is_empty());
    }

    #[test]
    fn glb_declares_the_lights_extension() {
        let mesh = Mesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0]],
            normals: vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
            uvs: vec![[0.0, 0.0], [0.5, 0.0], [0.5, 0.5]],
            indices: vec![0, 1, 2],
            ..Mesh::new("wall", ArchId(1))
        };
        let half = 30.0_f64.to_radians();
        let scene = Scene3d {
            meshes: vec![mesh],
            lights: vec![scene_light(
                0,
                LightKind::Spot,
                Vec3::new(1.0, 1.0, 2.4),
                Vec3::new(0.0, 0.0, -1.0),
                950.36,
                [1.0, 1.0, 1.0],
                half.cos(),
                half.cos(),
            )],
            ..Default::default()
        };

        let bytes = crate::glb::write(&scene);
        let blocks = glb_json_chunk(&bytes);
        let extensions_used = blocks["extensionsUsed"].as_array().unwrap();
        assert!(extensions_used.iter().any(|v| v == "KHR_lights_punctual"));
        let root = blocks["nodes"][0].as_object().unwrap();
        let children = root["children"].as_array().unwrap();
        assert!(children.iter().any(|n| {
            let idx = n.as_u64().unwrap() as usize;
            blocks["nodes"][idx]["name"] == "light-lf0-source"
        }));
    }

    #[test]
    fn glb_exports_an_area_light_as_a_spot_with_its_size_in_extras() {
        let candela = 3600.0 / std::f64::consts::PI;
        let mut panel = scene_light(7, LightKind::Area, Vec3::new(2.0, 1.5, 2.4), Vec3::new(0.0, 0.0, -1.0), candela, [1.0, 0.95, 0.9], 1.0, 0.0);
        panel.size = [0.6, 0.6];
        let mut strip = scene_light(8, LightKind::Area, Vec3::new(4.0, 1.5, 2.4), Vec3::new(0.0, 0.0, -1.0), candela, [1.0, 1.0, 1.0], 1.0, 0.0);
        strip.size = [1.2, 0.3];
        let mesh = Mesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0]],
            normals: vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
            uvs: vec![[0.0, 0.0], [0.5, 0.0], [0.5, 0.5]],
            indices: vec![0, 1, 2],
            ..Mesh::new("wall", ArchId(1))
        };
        let scene = Scene3d { meshes: vec![mesh], lights: vec![panel, strip], ..Default::default() };

        let (lights, nodes) = punctual_lights(&scene);
        assert_eq!(lights.len(), 2);
        assert_eq!(nodes.len(), 2);
        for light in &lights {
            assert_eq!(light["type"], "spot", "{light}");
            assert!((light["intensity"].as_f64().unwrap_or(f64::NAN) - candela).abs() < 1e-9, "{light}");
            let spot = &light["spot"];
            assert_eq!(spot["innerConeAngle"].as_f64(), Some(0.0), "{spot}");
            assert!((spot["outerConeAngle"].as_f64().unwrap_or(f64::NAN) - std::f64::consts::PI / 2.0).abs() < 1e-12, "{spot}");
        }
        assert_eq!(nodes[0]["extras"]["cadcraft"]["area"], json!([0.6, 0.6]));
        assert_eq!(nodes[1]["extras"]["cadcraft"]["area"], json!([1.2, 0.3]));
        assert!(nodes[0]["extras"]["cadcraft"].get("ies").is_none());

        // The same through the whole GLB: the light's node carries the extras.
        let bytes = crate::glb::write(&scene);
        let doc = glb_json_chunk(&bytes);
        let node = doc["nodes"].as_array().and_then(|all| all.iter().find(|n| n["name"] == "light-lf7-source")).expect("the panel's light node");
        assert_eq!(node["extras"]["cadcraft"]["area"], json!([0.6, 0.6]));
        assert_eq!(doc["extensions"]["KHR_lights_punctual"]["lights"][0]["type"], "spot");
    }

    #[test]
    fn glb_exports_an_ies_fixture_with_its_peak_and_name() {
        let profile = cadcraft_doc::arch::IesProfile {
            name: "Downlight 3000K".into(),
            vertical: vec![0.0, 45.0, 90.0],
            horizontal: vec![0.0],
            candela: vec![1200.0, 800.0, 50.0],
            lumens: -1.0,
            ..Default::default()
        };
        let half = 30.0_f64.to_radians();
        let mut pot =
            scene_light(3, LightKind::Spot, Vec3::new(1.0, 1.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 950.36, [1.0, 1.0, 1.0], half.cos(), half.cos());
        pot.ies = Some(std::sync::Arc::new(profile));
        let plain =
            scene_light(4, LightKind::Spot, Vec3::new(3.0, 1.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 950.36, [1.0, 1.0, 1.0], half.cos(), half.cos());
        let scene = Scene3d { lights: vec![pot, plain], ..Default::default() };

        let (lights, nodes) = punctual_lights(&scene);
        assert_eq!(lights.len(), 2);
        assert!(
            (lights[0]["intensity"].as_f64().unwrap_or(f64::NAN) - 1200.0).abs() < 1e-9,
            "the IES fixture exports its peak candela, {}",
            lights[0]
        );
        assert_eq!(nodes[0]["extras"]["cadcraft"]["ies"], "Downlight 3000K");
        assert!(nodes[0]["extras"]["cadcraft"].get("area").is_none());
        // A light without a profile keeps its own candela and has no extras.
        assert!((lights[1]["intensity"].as_f64().unwrap_or(f64::NAN) - 950.36).abs() < 1e-9);
        assert!(nodes[1].get("extras").is_none(), "{}", nodes[1]);
    }

    #[test]
    fn area_light_with_nan_size_omits_area_extras() {
        let candela = 3600.0 / std::f64::consts::PI;
        let mut panel = scene_light(5, LightKind::Area, Vec3::new(2.0, 1.5, 2.4), Vec3::new(0.0, 0.0, -1.0), candela, [1.0, 1.0, 1.0], 1.0, 0.0);
        panel.size = [f64::NAN, 0.6];
        let (_, nodes) = punctual_lights(&Scene3d { lights: vec![panel], ..Default::default() });
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].get("extras").is_none(), "NaN size should omit extras entirely");
    }

    #[test]
    fn ies_profile_with_zero_peak_exports_zero_intensity() {
        let profile = cadcraft_doc::arch::IesProfile {
            name: "Dim fixture".into(),
            vertical: vec![0.0, 90.0],
            horizontal: vec![0.0],
            candela: vec![0.0, 0.0],
            lumens: -1.0,
            ..Default::default()
        };
        let half = 30.0_f64.to_radians();
        let mut pot =
            scene_light(9, LightKind::Spot, Vec3::new(1.0, 1.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 950.36, [1.0, 1.0, 1.0], half.cos(), half.cos());
        pot.ies = Some(std::sync::Arc::new(profile));
        let (lights, _) = punctual_lights(&Scene3d { lights: vec![pot], ..Default::default() });
        assert_eq!(lights.len(), 1);
        assert!((lights[0]["intensity"].as_f64().unwrap_or(f64::NAN) - 0.0).abs() < 1e-9, "zero-peak IES should export intensity 0, {}", lights[0]);
    }

    #[test]
    fn area_light_with_ies_carries_both_extras() {
        let candela = 3600.0 / std::f64::consts::PI;
        let profile = cadcraft_doc::arch::IesProfile {
            name: "Panel IES".into(),
            vertical: vec![0.0, 90.0],
            horizontal: vec![0.0],
            candela: vec![1000.0, 500.0],
            lumens: -1.0,
            ..Default::default()
        };
        let mut panel = scene_light(10, LightKind::Area, Vec3::new(2.0, 1.5, 2.4), Vec3::new(0.0, 0.0, -1.0), candela, [1.0, 1.0, 1.0], 1.0, 0.0);
        panel.size = [0.6, 0.6];
        panel.ies = Some(std::sync::Arc::new(profile));
        let (lights, nodes) = punctual_lights(&Scene3d { lights: vec![panel], ..Default::default() });
        assert_eq!(lights.len(), 1);
        assert_eq!(lights[0]["type"], "spot");
        // IES peak candela should be used as intensity
        assert!((lights[0]["intensity"].as_f64().unwrap_or(f64::NAN) - 1000.0).abs() < 1e-9, "IES peak should override area candela, {}", lights[0]);
        assert_eq!(nodes[0]["extras"]["cadcraft"]["area"], json!([0.6, 0.6]));
        assert_eq!(nodes[0]["extras"]["cadcraft"]["ies"], "Panel IES");
    }

    #[test]
    fn mixed_light_types_export_in_order() {
        let candela = 3600.0 / std::f64::consts::PI;
        let point = scene_light(0, LightKind::Point, Vec3::new(0.0, 0.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, 1.0, 1.0], 0.0, 0.0);
        let half = 30.0_f64.to_radians();
        let spot =
            scene_light(1, LightKind::Spot, Vec3::new(1.0, 0.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 950.36, [1.0, 1.0, 1.0], half.cos(), half.cos());
        let mut area = scene_light(2, LightKind::Area, Vec3::new(2.0, 0.0, 2.4), Vec3::new(0.0, 0.0, -1.0), candela, [1.0, 1.0, 1.0], 1.0, 0.0);
        area.size = [0.6, 0.6];
        let scene = Scene3d { lights: vec![point, spot, area], ..Default::default() };
        let (lights, nodes) = punctual_lights(&scene);
        assert_eq!(lights.len(), 3);
        assert_eq!(nodes.len(), 3);
        assert_eq!(lights[0]["type"], "point");
        assert_eq!(lights[1]["type"], "spot");
        assert_eq!(lights[2]["type"], "spot"); // area exported as spot
        assert_eq!(nodes[0]["name"], "light-lf0-source");
        assert_eq!(nodes[1]["name"], "light-lf1-source");
        assert_eq!(nodes[2]["name"], "light-lf2-source");
        assert!(nodes[0].get("extras").is_none());
        assert!(nodes[1].get("extras").is_none());
        assert_eq!(nodes[2]["extras"]["cadcraft"]["area"], json!([0.6, 0.6]));
    }

    #[test]
    fn a_spot_lights_inner_cone_is_inside_its_outer_cone() {
        // A pot light with uniform intensity (inner == outer cos)
        let half = 30.0_f64.to_radians();
        let scene = Scene3d {
            lights: vec![scene_light(
                0,
                LightKind::Spot,
                Vec3::new(1.0, 1.0, 2.4),
                Vec3::new(0.0, 0.0, -1.0),
                950.36,
                [1.0, 1.0, 1.0],
                half.cos(),
                half.cos(),
            )],
            ..Default::default()
        };

        let (lights, _) = punctual_lights(&scene);
        assert_eq!(lights.len(), 1);
        let spot = &lights[0]["spot"];
        let inner = spot["innerConeAngle"].as_f64().unwrap_or(f64::NAN);
        let outer = spot["outerConeAngle"].as_f64().unwrap_or(f64::NAN);

        assert!(inner < outer, "inner ({inner}) must be strictly less than outer ({outer})");
        assert!(inner >= 0.0);
        assert!(outer <= std::f64::consts::PI / 2.0);
    }

    #[test]
    fn a_spot_with_falloff_beyond_its_cone_keeps_both_angles() {
        // Inner cone 30 deg, outer cone 45 deg
        let inner_rad = 30.0_f64.to_radians();
        let outer_rad = 45.0_f64.to_radians();
        let scene = Scene3d {
            lights: vec![scene_light(
                0,
                LightKind::Spot,
                Vec3::new(1.0, 1.0, 2.4),
                Vec3::new(0.0, 0.0, -1.0),
                950.36,
                [1.0, 1.0, 1.0],
                inner_rad.cos(),
                outer_rad.cos(),
            )],
            ..Default::default()
        };

        let (lights, _) = punctual_lights(&scene);
        assert_eq!(lights.len(), 1);
        let spot = &lights[0]["spot"];
        let inner = spot["innerConeAngle"].as_f64().unwrap_or(f64::NAN);
        let outer = spot["outerConeAngle"].as_f64().unwrap_or(f64::NAN);

        assert!((inner - inner_rad).abs() < 1e-9, "inner should match input cone");
        assert!((outer - outer_rad).abs() < 1e-9, "outer should match input falloff");
        assert!(inner < outer);
    }

    #[test]
    fn spot_light_with_nan_cosines_is_skipped() {
        let scene = Scene3d {
            lights: vec![
                scene_light(0, LightKind::Spot, Vec3::new(0.0, 0.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, 1.0, 1.0], f64::NAN, 0.5),
                scene_light(1, LightKind::Spot, Vec3::new(1.0, 1.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, 1.0, 1.0], 0.5, f64::NAN),
                scene_light(2, LightKind::Spot, Vec3::new(2.0, 1.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, 1.0, 1.0], 0.5, 0.5),
            ],
            ..Default::default()
        };
        let (lights, nodes) = punctual_lights(&scene);
        assert_eq!(lights.len(), 1);
        assert_eq!(lights[0]["name"], "lf2");
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn spot_light_with_boundary_cosines() {
        // cos_inner = 1.0 (0 degrees), cos_outer = -1.0 (180 degrees, clamped to 90)
        let scene = Scene3d {
            lights: vec![scene_light(0, LightKind::Spot, Vec3::new(0.0, 0.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, 1.0, 1.0], 1.0, -1.0)],
            ..Default::default()
        };
        let (lights, _) = punctual_lights(&scene);
        assert_eq!(lights.len(), 1);
        let spot = &lights[0]["spot"];
        let inner = spot["innerConeAngle"].as_f64().unwrap_or(f64::NAN);
        let outer = spot["outerConeAngle"].as_f64().unwrap_or(f64::NAN);
        assert_eq!(inner, 0.0, "zero-degree inner cone");
        assert!((outer - std::f64::consts::PI / 2.0).abs() < 1e-9, "outer clamped to PI/2");
        assert!(inner < outer);
    }

    #[test]
    fn spot_light_with_narrow_inner_wide_outer() {
        // Inner 10 degrees, outer 60 degrees
        let inner_rad = 10.0_f64.to_radians();
        let outer_rad = 60.0_f64.to_radians();
        let scene = Scene3d {
            lights: vec![scene_light(
                0,
                LightKind::Spot,
                Vec3::new(0.0, 0.0, 2.4),
                Vec3::new(0.0, 0.0, -1.0),
                100.0,
                [1.0, 1.0, 1.0],
                inner_rad.cos(),
                outer_rad.cos(),
            )],
            ..Default::default()
        };
        let (lights, _) = punctual_lights(&scene);
        assert_eq!(lights.len(), 1);
        let spot = &lights[0]["spot"];
        let inner = spot["innerConeAngle"].as_f64().unwrap_or(f64::NAN);
        let outer = spot["outerConeAngle"].as_f64().unwrap_or(f64::NAN);
        assert!((inner - inner_rad).abs() < 1e-9, "inner matches input");
        assert!((outer - outer_rad).abs() < 1e-9, "outer matches input");
        assert!(inner < outer);
    }

    #[test]
    fn spot_light_with_wide_inner_and_outer_clamps_outer() {
        // Inner 80 degrees, outer 100 degrees (clamped to 90)
        let inner_rad = 80.0_f64.to_radians();
        let outer_rad = 100.0_f64.to_radians();
        let scene = Scene3d {
            lights: vec![scene_light(
                0,
                LightKind::Spot,
                Vec3::new(0.0, 0.0, 2.4),
                Vec3::new(0.0, 0.0, -1.0),
                100.0,
                [1.0, 1.0, 1.0],
                inner_rad.cos(),
                outer_rad.cos(),
            )],
            ..Default::default()
        };
        let (lights, _) = punctual_lights(&scene);
        assert_eq!(lights.len(), 1);
        let spot = &lights[0]["spot"];
        let inner = spot["innerConeAngle"].as_f64().unwrap_or(f64::NAN);
        let outer = spot["outerConeAngle"].as_f64().unwrap_or(f64::NAN);
        assert!((inner - inner_rad).abs() < 1e-9, "inner matches input");
        assert!((outer - std::f64::consts::PI / 2.0).abs() < 1e-9, "outer clamped to PI/2");
        assert!(inner < outer);
    }

    #[test]
    fn point_light_with_nan_color_is_skipped() {
        let scene = Scene3d {
            lights: vec![
                scene_light(0, LightKind::Point, Vec3::new(0.0, 0.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [f64::NAN, 1.0, 1.0], 0.0, 0.0),
                scene_light(1, LightKind::Point, Vec3::new(1.0, 1.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, f64::NAN, 1.0], 0.0, 0.0),
                scene_light(2, LightKind::Point, Vec3::new(2.0, 1.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, 1.0, f64::NAN], 0.0, 0.0),
                scene_light(3, LightKind::Point, Vec3::new(3.0, 1.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 100.0, [1.0, 1.0, 1.0], 0.0, 0.0),
            ],
            ..Default::default()
        };
        let (lights, nodes) = punctual_lights(&scene);
        assert_eq!(lights.len(), 1);
        assert_eq!(lights[0]["name"], "lf3");
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn spot_light_with_zero_intensity_is_kept() {
        let scene = Scene3d {
            lights: vec![scene_light(0, LightKind::Spot, Vec3::new(0.0, 0.0, 2.4), Vec3::new(0.0, 0.0, -1.0), 0.0, [1.0, 1.0, 1.0], 0.5, 0.5)],
            ..Default::default()
        };
        let (lights, _) = punctual_lights(&scene);
        assert_eq!(lights.len(), 1);
        assert_eq!(lights[0]["intensity"], 0.0);
    }
}
