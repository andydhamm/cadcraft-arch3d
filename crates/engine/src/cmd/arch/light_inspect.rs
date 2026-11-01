//! The light fixtures' part of `arch.inspect` (phase 10): `{"lights":true}`. Filled in by
//! `p10-cmd-lightfixture`.

use cadcraft_arch::light::{intensity_cd, light_point};
use cadcraft_doc::arch::{ArchModel, LightKind, LightMount, handle_json, light_handle};
use serde_json::{Map, Value, json};

fn kind_name(kind: LightKind) -> &'static str {
    match kind {
        LightKind::Point => "point",
        LightKind::Spot => "spot",
        LightKind::Area => "area",
    }
}

fn mount_name(mount: LightMount) -> &'static str {
    match mount {
        LightMount::Ceiling => "ceiling",
        LightMount::Wall => "wall",
        LightMount::Floor => "floor",
    }
}

/// Add the `lights` key to `out` when `p` asks for it (`{"lights":true}`). Read-only.
pub(crate) fn inspect_lights(model: &ArchModel, p: &Value, out: &mut Map<String, Value>) {
    if p.get("lights").and_then(Value::as_bool) != Some(true) {
        return;
    }

    let lights = model
        .lights
        .iter()
        .map(|fixture| {
            let z = light_point(model, fixture).map(|point| point.z);
            json!({
                "id": format!("lf{}", fixture.id.0),
                "handle": handle_json(light_handle(fixture.id)),
                "fixture": fixture.fixture,
                "shape": fixture.shape.name(),
                "kind": kind_name(fixture.kind),
                "mount": mount_name(fixture.mount),
                "level": fixture.level,
                "at": [fixture.position.x, fixture.position.y],
                "z": z,
                "rotation": fixture.rotation,
                "tilt": fixture.tilt,
                "lumens": fixture.lumens,
                "kelvin": fixture.kelvin,
                "cone": fixture.cone,
                "falloff": fixture.falloff,
                "candela": intensity_cd(fixture),
                "on": fixture.on,
            })
        })
        .collect::<Vec<_>>();

    out.insert("lights".to_string(), Value::Array(lights));
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchId, LightFixture, Room, RoomType};
    use cadcraft_geom::Vec2;

    #[test]
    fn inspect_lights_reports_z_and_candela() {
        let mut model = ArchModel::default();
        model.rooms.push(Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
            room_type: RoomType::Living,
            ..Room::default()
        });
        model.lights.push(LightFixture { kind: LightKind::Spot, ..LightFixture::new(ArchId(0), "Recessed pot light", Vec2::new(1.0, 1.0)) });
        let mut out = Map::new();
        inspect_lights(&model, &json!({"lights": true}), &mut out);
        let lights = out["lights"].as_array().unwrap();
        assert_eq!(lights[0]["id"], "lf0");
        assert_eq!(lights[0]["handle"], "a0048000");
        assert_eq!(lights[0]["z"], 2.4);
        assert!((lights[0]["candela"].as_f64().unwrap() - 950.36).abs() < 0.02);
    }

    #[test]
    fn inspect_lights_empty() {
        let model = ArchModel::default();
        let mut out = Map::new();
        inspect_lights(&model, &json!({"lights": true}), &mut out);
        let lights = out["lights"].as_array().unwrap();
        assert!(lights.is_empty());
    }

    #[test]
    fn inspect_lights_not_requested() {
        let model = ArchModel::default();
        let mut out = Map::new();
        inspect_lights(&model, &json!({}), &mut out);
        assert!(!out.contains_key("lights"));
    }
}
