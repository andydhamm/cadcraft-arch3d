//! The light fixtures' part of `arch.inspect` (phase 10): `{"lights":true}`. Since phase 11 an area
//! light also lists its `width` and `length`, and every light an `ies` object (`{name, lumens,
//! peak}`, or `null` without a profile).

use cadcraft_arch::ies::{peak_candela, scaled};
use cadcraft_arch::light::{intensity_cd, light_point, strip_length};
use cadcraft_arch::room::room_at;
use cadcraft_doc::arch::{ArchModel, LightFixture, LightKind, LightMount, StripFacing, handle_json, light_handle, room_label, tray_label};
use serde_json::{Map, Value, json};

fn kind_name(kind: LightKind) -> &'static str {
    match kind {
        LightKind::Point => "point",
        LightKind::Spot => "spot",
        LightKind::Area => "area",
        LightKind::Strip => "strip",
    }
}

fn mount_name(mount: LightMount) -> &'static str {
    match mount {
        LightMount::Ceiling => "ceiling",
        LightMount::Wall => "wall",
        LightMount::Floor => "floor",
    }
}

/// The fixture's IES profile as `{name, lumens, peak}`, or `null`: `lumens` is the file's rated
/// lumens (-1 = absolute photometry) and `peak` the largest candela value once the profile is
/// fitted to the fixture's own lumens (what the fixture emits).
fn ies_json(fixture: &LightFixture) -> Value {
    match &fixture.ies {
        Some(profile) => json!({ "name": profile.name, "lumens": profile.lumens, "peak": peak_candela(&scaled(profile, fixture.lumens)) }),
        None => Value::Null,
    }
}

/// Peak luminous intensity along the aim (cd): an IES fixture's is the peak of its fitted profile,
/// any other the kind's own (`intensity_cd`). An off fixture emits nothing.
fn candela(fixture: &LightFixture) -> f64 {
    match &fixture.ies {
        Some(profile) if fixture.on => peak_candela(&scaled(profile, fixture.lumens)),
        Some(_) => 0.0,
        None => intensity_cd(fixture),
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
            let mut light = json!({
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
                "candela": candela(fixture),
                "ies": ies_json(fixture),
                "on": fixture.on,
                "brightness": fixture.brightness,
                "room": room_at(model, fixture.level, fixture.position).map(room_label),
            });
            if let (Some(object), LightKind::Area) = (light.as_object_mut(), fixture.kind) {
                object.insert("width".to_string(), json!(fixture.width));
                object.insert("length".to_string(), json!(fixture.length));
            }
            if let (Some(object), LightKind::Strip) = (light.as_object_mut(), fixture.kind) {
                // A strip's height is its own (the level's elevation is not added): `z` is the world value.
                object.insert("path".to_string(), Value::Array(fixture.path.iter().map(|p| json!([p.x, p.y])).collect()));
                object.insert("closed".to_string(), json!(fixture.closed));
                object.insert("height".to_string(), json!(fixture.height));
                object.insert("lumensPerMetre".to_string(), json!(fixture.lumens_per_metre));
                object.insert("facing".to_string(), json!(if fixture.facing == StripFacing::Out { "out" } else { "down" }));
                object.insert("length".to_string(), json!(strip_length(fixture)));
                object.insert("tray".to_string(), fixture.tray.map_or(Value::Null, |id| json!(tray_label(id))));
            }
            light
        })
        .collect::<Vec<_>>();

    out.insert("lights".to_string(), Value::Array(lights));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use crate::cmd::arch::lightfixture::TEST_DOWNLIGHT_IES;
    use crate::temp_dir::TempDir;
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
    fn strip_rows() {
        use cadcraft_doc::arch::{FixtureShape, StripFacing};
        let mut model = ArchModel::default();
        model.lights.push(LightFixture {
            kind: LightKind::Strip,
            shape: FixtureShape::Strip,
            path: vec![Vec2::new(1.0, 2.0), Vec2::new(3.0, 2.0)],
            height: Some(2.4),
            lumens_per_metre: 500.0,
            lumens: 1000.0,
            facing: StripFacing::Out,
            tray: Some(ArchId(2)),
            ..LightFixture::new(ArchId(0), "LED strip", Vec2::new(2.0, 2.0))
        });
        model.lights.push(LightFixture::new(ArchId(1), "Recessed pot light", Vec2::new(1.0, 1.0)));
        let mut out = Map::new();
        inspect_lights(&model, &json!({"lights": true}), &mut out);
        let lights = out["lights"].as_array().unwrap();
        let s = &lights[0];
        assert_eq!(s["kind"], "strip");
        assert_eq!(s["path"], json!([[1.0, 2.0], [3.0, 2.0]]));
        assert_eq!((s["closed"].clone(), s["facing"].clone()), (json!(false), json!("out")));
        assert_eq!((s["height"].as_f64(), s["lumensPerMetre"].as_f64()), (Some(2.4), Some(500.0)));
        assert_eq!((s["length"].as_f64(), s["lumens"].as_f64()), (Some(2.0), Some(1000.0)));
        assert_eq!(s["tray"], "tr2");
        // Without a tray the row says null; other lights have no strip keys.
        model.lights[0].tray = None;
        let mut out = Map::new();
        inspect_lights(&model, &json!({"lights": true}), &mut out);
        assert!(out["lights"][0]["tray"].is_null());
        assert!(out["lights"][1].get("path").is_none() && out["lights"][1].get("lumensPerMetre").is_none());

        // Through the commands: a strip made by `lightfixture` reads back, an edit shows at once.
        let mut session = Session::new();
        session.execute("lightfixture", &json!({"type": "LED strip", "path": [[0, 0], [1, 0], [1, 1]], "closed": true, "height": 2.0})).unwrap();
        let row = session.execute("arch.inspect", &json!({"lights": true})).unwrap()["lights"][0].clone();
        assert_eq!((row["kind"].clone(), row["closed"].clone(), row["facing"].clone()), (json!("strip"), json!(true), json!("down")));
        assert!((row["length"].as_f64().unwrap() - (2.0 + 2.0_f64.sqrt())).abs() < 1e-9, "{row}");
        assert!((row["lumens"].as_f64().unwrap() - 500.0 * (2.0 + 2.0_f64.sqrt())).abs() < 1e-6, "{row}");
        session.execute("lightfixture.edit", &json!({"id": "lf0", "lumensPerMetre": 100, "facing": "out"})).unwrap();
        let row = session.execute("arch.inspect", &json!({"lights": true})).unwrap()["lights"][0].clone();
        assert_eq!((row["lumensPerMetre"].as_f64(), row["facing"].clone()), (Some(100.0), json!("out")));
        assert!((row["lumens"].as_f64().unwrap() - 100.0 * (2.0 + 2.0_f64.sqrt())).abs() < 1e-6, "{row}");
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

    #[test]
    fn lightfixture_point_inspect_no_width_length() {
        let mut model = ArchModel::default();
        model.lights.push(LightFixture { kind: LightKind::Point, ..LightFixture::new(ArchId(0), "Recessed pot light", Vec2::new(1.0, 1.0)) });
        let mut out = Map::new();
        inspect_lights(&model, &json!({"lights": true}), &mut out);
        let lights = out["lights"].as_array().unwrap();
        let light = &lights[0];
        assert!(light.get("width").is_none(), "{light}");
        assert!(light.get("length").is_none(), "{light}");
    }

    #[test]
    fn inspect_lists_area_size_and_ies() {
        let dir = TempDir::new("ies-inspect");
        let file = dir.file("downlight.ies", TEST_DOWNLIGHT_IES);
        let mut session = Session::new();
        session.execute("lightfixture", &json!({"type": "LED panel 600", "at": [2.0, 2.0]})).unwrap();
        session.execute("lightfixture", &json!({"type": "Recessed pot light", "at": [1.0, 1.0]})).unwrap();
        session.execute("lightfixture.edit", &json!({"id": "lf1", "ies": file})).unwrap();
        let inspect = |session: &mut Session| session.execute("arch.inspect", &json!({"lights": true})).unwrap()["lights"].clone();

        // The panel: an area light 0.6 x 0.6 with 3600 / pi cd along its aim and no profile.
        let lights = inspect(&mut session);
        let panel = &lights[0];
        assert_eq!((panel["id"].as_str(), panel["kind"].as_str(), panel["shape"].as_str()), (Some("lf0"), Some("area"), Some("panel")));
        assert_eq!((panel["width"].as_f64(), panel["length"].as_f64()), (Some(0.6), Some(0.6)));
        assert!((panel["candela"].as_f64().unwrap() - 1145.9).abs() < 0.05, "{panel}");
        assert!(panel["ies"].is_null(), "{panel}");

        // The pot light with the IES file: no size, the profile's name, rated lumens and peak, and
        // its candela is that peak.
        let pot = &lights[1];
        assert_eq!(pot["kind"], "spot");
        assert!(pot.get("width").is_none() && pot.get("length").is_none(), "{pot}");
        assert_eq!(pot["ies"], json!({"name": "Test downlight 1000 lm", "lumens": 1000.0, "peak": 1000.0}));
        assert_eq!((pot["lumens"].as_f64(), pot["candela"].as_f64()), (Some(1000.0), Some(1000.0)));

        // The peak follows the fixture's own lumens (the profile is scaled); an off fixture emits nothing.
        session.execute("lightfixture.edit", &json!({"id": "lf1", "lumens": 2000.0})).unwrap();
        let pot = inspect(&mut session)[1].clone();
        assert_eq!(pot["ies"], json!({"name": "Test downlight 1000 lm", "lumens": 1000.0, "peak": 2000.0}));
        assert_eq!(pot["candela"].as_f64(), Some(2000.0));
        session.execute("lightfixture.edit", &json!({"id": "lf1", "on": false})).unwrap();
        assert_eq!(inspect(&mut session)[1]["candela"].as_f64(), Some(0.0));

        // Clearing the profile brings the cone's candela back and `ies` reads null.
        session.execute("lightfixture.edit", &json!({"id": "lf1", "ies": null, "on": true})).unwrap();
        let pot = inspect(&mut session)[1].clone();
        assert!(pot["ies"].is_null(), "{pot}");
        assert!((pot["candela"].as_f64().unwrap() - 2000.0 / cadcraft_arch::light::cone_solid_angle(60.0)).abs() < 1e-6, "{pot}");
    }

    #[test]
    fn inspect_light_rows_report_brightness_and_room() {
        let mut model = ArchModel::default();
        model.rooms.push(Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
            room_type: RoomType::Living,
            ..Room::default()
        });
        model.lights.push(LightFixture { kind: LightKind::Spot, ..LightFixture::new(ArchId(0), "Recessed pot light", Vec2::new(1.0, 1.0)) });
        model.lights.push(LightFixture { kind: LightKind::Spot, ..LightFixture::new(ArchId(1), "Recessed pot light", Vec2::new(9.0, 9.0)) });
        let mut out = Map::new();
        inspect_lights(&model, &json!({"lights": true}), &mut out);
        let lights = out["lights"].as_array().unwrap();
        assert_eq!(lights[0]["brightness"], json!(1.0));
        assert_eq!(lights[0]["room"], json!("r0"));
        assert_eq!(lights[1]["room"], Value::Null);

        // Changing brightness is reflected in the next inspect.
        model.lights[0].brightness = 0.5;
        let mut out2 = Map::new();
        inspect_lights(&model, &json!({"lights": true}), &mut out2);
        let lights2 = out2["lights"].as_array().unwrap();
        assert_eq!(lights2[0]["brightness"], json!(0.5));
    }

    #[test]
    fn inspect_light_in_room_hole_reports_null() {
        let mut model = ArchModel::default();
        model.rooms.push(Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 6.0), Vec2::new(0.0, 6.0)],
            holes: vec![vec![Vec2::new(2.0, 2.0), Vec2::new(4.0, 2.0), Vec2::new(4.0, 4.0), Vec2::new(2.0, 4.0)]],
            room_type: RoomType::Living,
            ..Room::default()
        });
        // Light inside the hole.
        model.lights.push(LightFixture { kind: LightKind::Spot, ..LightFixture::new(ArchId(0), "Recessed pot light", Vec2::new(3.0, 3.0)) });
        // Light in the room but outside the hole.
        model.lights.push(LightFixture { kind: LightKind::Spot, ..LightFixture::new(ArchId(1), "Recessed pot light", Vec2::new(1.0, 1.0)) });
        let mut out = Map::new();
        inspect_lights(&model, &json!({"lights": true}), &mut out);
        let lights = out["lights"].as_array().unwrap();
        assert_eq!(lights[0]["room"], Value::Null, "light in hole should report null room");
        assert_eq!(lights[1]["room"], json!("r0"), "light in room should report r0");
    }

    #[test]
    fn inspect_light_on_wrong_level_reports_null() {
        let mut model = ArchModel::default();
        model.rooms.push(Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
            room_type: RoomType::Living,
            ..Room::default()
        });
        // Light on level 1, room on level 0.
        model.lights.push(LightFixture {
            kind: LightKind::Spot,
            level: 1,
            ..LightFixture::new(ArchId(0), "Recessed pot light", Vec2::new(2.0, 2.0))
        });
        let mut out = Map::new();
        inspect_lights(&model, &json!({"lights": true}), &mut out);
        let lights = out["lights"].as_array().unwrap();
        assert_eq!(lights[0]["room"], Value::Null, "light on different level should report null room");
    }

    #[test]
    fn inspect_light_brightness_extremes() {
        let mut model = ArchModel::default();
        let mut l0 = LightFixture { kind: LightKind::Spot, ..LightFixture::new(ArchId(0), "Recessed pot light", Vec2::new(1.0, 1.0)) };
        l0.brightness = 0.0;
        let mut l1 = LightFixture { kind: LightKind::Spot, ..LightFixture::new(ArchId(1), "Recessed pot light", Vec2::new(2.0, 2.0)) };
        l1.brightness = 1.0;
        model.lights.push(l0);
        model.lights.push(l1);
        let mut out = Map::new();
        inspect_lights(&model, &json!({"lights": true}), &mut out);
        let lights = out["lights"].as_array().unwrap();
        assert_eq!(lights[0]["brightness"], json!(0.0));
        assert_eq!(lights[1]["brightness"], json!(1.0));
    }
}
