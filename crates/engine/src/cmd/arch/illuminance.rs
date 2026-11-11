//! `illuminance`: direct light from the fixtures at a point, with shadows (phase 10).

use cadcraft_doc::arch::{ArchId, light_label};
use cadcraft_geom::Vec3;
use serde_json::{Value, json};

use crate::cmd::arch::look::{apply_look_params, effective_look, effective_sun_dir};
use crate::cmd::arch::render::{IlluminanceRequest, PathParams, SunChoice, render_hooks};
use crate::cmd::{CommandSpec, bad};
use crate::{Result, Session};

fn parse_xyz_array(value: &Value, key: &str, required: bool, cmd: &str) -> Result<Vec3> {
    let raw = match value.get(key) {
        Some(v) if !v.is_null() => v,
        None if required => {
            return Err(bad(cmd, format!("{key} must be [x, y, z] finite numbers")));
        }
        _ => {
            return Err(bad(cmd, format!("{key} must be [x, y, z] finite numbers")));
        }
    };

    let arr = raw.as_array().ok_or_else(|| bad(cmd, format!("{key} must be [x, y, z] finite numbers")))?;
    if arr.len() != 3 {
        return Err(bad(cmd, format!("{key} must be [x, y, z] finite numbers")));
    }

    let mut xyz = [0.0_f64; 3];
    for (i, item) in arr.iter().enumerate() {
        let value = item.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, format!("{key} must be [x, y, z] finite numbers")))?;
        xyz[i] = value;
    }
    Ok(Vec3::new(xyz[0], xyz[1], xyz[2]))
}

fn run(session: &mut Session, params: &Value) -> Result<Value> {
    let doc = session.doc()?;
    let point = parse_xyz_array(params, "point", true, "illuminance")?;
    let normal = match params.get("normal") {
        Some(v) if !v.is_null() => {
            let v = parse_xyz_array(params, "normal", true, "illuminance")?;
            let len = v.len();
            if len <= 0.0 || !len.is_finite() {
                return Err(bad("illuminance", "normal must be [x, y, z] finite numbers with non-zero length"));
            }
            v
        }
        _ => Vec3::new(0.0, 0.0, 1.0),
    };
    if !normal.is_finite() || normal.len() <= 0.0 {
        return Err(bad("illuminance", "normal must be [x, y, z] finite numbers with non-zero length"));
    }

    let method = match params.get("method") {
        None => "direct",
        Some(val) => {
            val.as_str().filter(|s| *s == "direct" || *s == "path").ok_or_else(|| bad("illuminance", "`method` must be \"direct\" or \"path\""))?
        }
    };

    // The look the light is measured under (sun intensity, cast shadows, sky) and the sun it asks
    // for. An absent `sun` stays night, as phases 10 and 11 measured it.
    let mut look = effective_look(&doc.arch, None);
    let explicit = apply_look_params("illuminance", params, &doc.arch, &mut look, true)?;
    let sun_dir = effective_sun_dir(&doc.arch, &look, explicit);
    let sun = if params.get("sun").is_none() || !look.sun.on { SunChoice::Off } else { sun_dir.map_or(SunChoice::Default, SunChoice::Dir) };

    if method == "path" {
        let whole = |key: &str, lo: u64, hi: u64, def: u64| -> Result<u64> {
            match params.get(key) {
                None => Ok(def),
                Some(val) => val
                    .as_u64()
                    .filter(|n| (lo..=hi).contains(n))
                    .ok_or_else(|| bad("illuminance", format!("`{key}` must be a whole number in {lo}..={hi}"))),
            }
        };

        let spp = whole("spp", 1, 65536, 1024)?;
        let bounces = whole("bounces", 0, 32, 4)?;
        let seed = whole("seed", 0, 4294967295, 0)?;

        let path = PathParams {
            spp: spp as u32,
            bounces: bounces as u32,
            seed: seed as u32,
            denoise: params.get("denoise").and_then(Value::as_bool).unwrap_or(true),
            rt: params.get("rt").and_then(Value::as_bool).unwrap_or(false),
            spp_given: params.get("spp").is_some(),
            ..PathParams::default()
        };

        let hooks = render_hooks()
            .ok_or_else(|| bad("illuminance", "path-traced illuminance needs the 3D renderer: run it from cadcraft-cli run or the app"))?;
        let illuminance_fn = hooks
            .illuminance
            .as_ref()
            .ok_or_else(|| bad("illuminance", "path-traced illuminance needs the 3D renderer: run it from cadcraft-cli run or the app"))?;

        let request =
            IlluminanceRequest::new(&doc, vec![[[point.x, point.y, point.z], [normal.x, normal.y, normal.z]]], sun, path).with_look(look, sun_dir);

        (illuminance_fn)(&request).map_err(|e| bad("illuminance", e))
    } else {
        let scene = cadcraft_arch3d::build(&doc.arch);
        let illumination = cadcraft_arch3d::light::illuminance(&scene, point, normal);
        let contributions = doc
            .arch
            .lights
            .iter()
            .map(|light| {
                let lux = illumination.per_light.iter().find(|(id, _)| *id == light.id).map(|(_, v)| *v).unwrap_or(0.0);
                json!({
                    "id": light_label(light.id),
                    "lux": lux,
                })
            })
            .collect::<Vec<_>>();

        // The sun's direct light: 0 when it is off or `sun` is absent; a shadow ray when cast shadows are on.
        let toward_sun = match sun {
            SunChoice::Off => None,
            SunChoice::Dir(d) => Some(Vec3::new(d[0], d[1], d[2])),
            SunChoice::Default => Some(Vec3::new(-0.5, -0.5, std::f64::consts::FRAC_1_SQRT_2)),
        };
        let sun_lux =
            toward_sun.map_or(0.0, |s| cadcraft_arch3d::light::sun_illuminance(&scene, point, normal, s, look.sun.intensity, look.sun.shadows));

        Ok(json!({
            "lux": illumination.lux + sun_lux,
            "sun": sun_lux,
            "lights": scene.lights.len(),
            "direct": true,
            "contributions": contributions,
        }))
    }
}

/// Command specs for direct illuminance at a point.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("illuminance", "Illuminance", run)
            .menu(&["Architecture", "Illuminance"])
            .params("{point: [x,y,z], normal?: [x,y,z], method?: \"direct\" | \"path\", path: spp? 1..=65536, bounces? 0..=32, seed? 0..=4294967295, sun?: false | \"default\" | {on?, intensity?, shadows?, angle? | date, time | azimuth, elevation}, sky?}")
            .noundo(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchId, ArchModel, Level, LightFixture};
    use cadcraft_geom::Vec2;
    use serde_json::json;

    fn room_model() -> ArchModel {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Level::default() });

        let corners = [(-0.1, -0.1), (4.1, -0.1), (4.1, 4.1), (-0.1, 4.1)];
        for k in 0..4 {
            let (x1, y1) = corners[k];
            let (x2, y2) = corners[(k + 1) % 4];
            let mut wall = cadcraft_doc::Wall::new(Vec2::new(x1, y1), Vec2::new(x2, y2), 0.2);
            wall.height = 2.4;
            model.push_wall(wall);
        }

        for (k, x, y) in [(0, 1.0, 1.0), (1, 1.0, 3.0), (2, 3.0, 1.0), (3, 3.0, 3.0)] {
            model.lights.push(LightFixture::new(ArchId(k as u32), "Recessed pot light", Vec2::new(x, y)));
        }
        model.next_light_id = 4;
        model
    }

    fn fresh_session() -> Session {
        let mut s = Session::empty();
        s.new_drawing(false);
        let d = s.doc_mut().unwrap();
        d.arch = room_model();
        d.touch_arch();
        s
    }

    #[test]
    fn illuminance_reports_lux_per_light() {
        let mut s = fresh_session();
        let result = s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0]})).unwrap();
        let lux = result["lux"].as_f64().unwrap();
        assert!((lux - 165.0).abs() < 1.65, "lux={lux}");
        let contribs = result["contributions"].as_array().unwrap();
        assert_eq!(contribs.len(), 4);
        let ids: Vec<_> = contribs.iter().filter_map(|v| v.get("id").and_then(Value::as_str)).collect();
        assert_eq!(ids, vec!["lf0", "lf1", "lf2", "lf3"]);
        assert!(contribs.iter().all(|v| v.get("lux").and_then(Value::as_f64).is_some()));
    }

    fn column_session() -> Session {
        let mut s = Session::empty();
        s.new_drawing(false);
        let mut model = ArchModel::default();
        let mut w = cadcraft_doc::Wall::new(Vec2::new(0.0, 0.0), Vec2::new(0.2, 0.0), 0.2);
        w.height = 3.0;
        model.push_wall(w);
        let d = s.doc_mut().unwrap();
        d.arch = model;
        d.touch_arch();
        s
    }

    #[test]
    fn direct_includes_the_sun_with_shadow_ray() {
        let mut s = column_session();
        let overhead = json!({"azimuth": 0, "elevation": 90});
        let open = s.execute("illuminance", &json!({"point": [5.0, 5.0, 0.0], "sun": overhead, "sky": false})).unwrap();
        assert!((open["sun"].as_f64().unwrap() - 100_000.0).abs() < 1.0, "{open}");
        assert!((open["lux"].as_f64().unwrap() - 100_000.0).abs() < 1.0);
        // Under the column the sun is blocked, unless cast shadows are off.
        let under = json!({"point": [0.1, 0.0, 0.0], "sun": overhead});
        assert_eq!(s.execute("illuminance", &under).unwrap()["sun"].as_f64(), Some(0.0));
        let lit = s.execute("illuminance", &json!({"point": [0.1, 0.0, 0.0], "sun": {"azimuth": 0, "elevation": 90, "shadows": false}})).unwrap();
        assert!((lit["sun"].as_f64().unwrap() - 100_000.0).abs() < 1.0, "{lit}");
    }

    #[test]
    fn intensity_scales_absent_sun_is_night() {
        let mut s = column_session();
        let at = |s: &mut Session, sun: Option<Value>| {
            let mut p = json!({"point": [5.0, 5.0, 0.0]});
            if let Some(v) = sun {
                p["sun"] = v;
            }
            s.execute("illuminance", &p).unwrap()
        };
        let half = at(&mut s, Some(json!({"azimuth": 0, "elevation": 90, "intensity": 50000})));
        assert!((half["sun"].as_f64().unwrap() - 50_000.0).abs() < 1.0, "{half}");
        let low = at(&mut s, Some(json!({"azimuth": 180, "elevation": 30})));
        assert!((low["sun"].as_f64().unwrap() - 50_000.0).abs() < 1.0, "{low}");
        let off = at(&mut s, Some(json!({"azimuth": 0, "elevation": 90, "on": false})));
        assert_eq!((off["sun"].as_f64(), off["lux"].as_f64()), (Some(0.0), Some(0.0)));
        let night = at(&mut s, None);
        assert_eq!((night["sun"].as_f64(), night["lux"].as_f64()), (Some(0.0), Some(0.0)));
        assert!(s.execute("illuminance", &json!({"point": [5.0, 5.0, 0.0], "sun": {"azimuth": 0, "elevation": 91}})).is_err());
    }

    #[test]
    fn illuminance_refuses_a_bad_point() {
        let mut s = fresh_session();
        assert!(s.execute("illuminance", &json!({})).is_err());
        assert!(s.execute("illuminance", &json!({"point": [1.0, 1.0, f64::NAN]})).is_err());
        assert!(s.execute("illuminance", &json!({"point": [1.0, 1.0]})).is_err());
    }

    #[test]
    fn illuminance_without_lights_is_zero() {
        let mut s = Session::empty();
        s.new_drawing(false);
        let result = s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0]})).unwrap();
        assert_eq!(result["lux"].as_f64(), Some(0.0));
        assert_eq!(result["lights"].as_u64(), Some(0));
        assert!(result["contributions"].as_array().unwrap().is_empty());
    }

    #[test]
    fn illuminance_refuses_a_zero_length_normal() {
        let mut s = fresh_session();
        let err = s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "normal": [0.0, 0.0, 0.0]})).unwrap_err();
        assert!(err.to_string().contains("normal"), "error={err}");
    }

    #[test]
    fn illuminance_refuses_a_bad_normal() {
        let mut s = fresh_session();
        // NaN in normal
        assert!(s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "normal": [f64::NAN, 0.0, 1.0]})).is_err());
        // Wrong length
        assert!(s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "normal": [1.0, 0.0]})).is_err());
        // Non-numeric component
        assert!(s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "normal": ["a", 0.0, 1.0]})).is_err());
        // Infinity in normal
        assert!(s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "normal": [f64::INFINITY, 0.0, 1.0]})).is_err());
    }

    #[test]
    fn illuminance_null_normal_uses_default() {
        let mut s = fresh_session();
        let result = s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "normal": null})).unwrap();
        let lux = result["lux"].as_f64().unwrap();
        assert!(lux > 0.0, "null normal should use [0,0,1] default and give positive lux, got {lux}");
    }

    #[test]
    fn illuminance_off_lights_contribute_nothing() {
        let mut model = room_model();
        // Turn off two of the four lights (lf1 at (1,3) and lf3 at (3,3))
        model.lights[1].on = false;
        model.lights[3].on = false;

        let mut s = Session::empty();
        s.new_drawing(false);
        let d = s.doc_mut().unwrap();
        d.arch = model;
        d.touch_arch();

        // Point (2, 1.5, 0) is within the 60° cone of lf0 (1,1) and lf2 (3,1)
        // but outside the cone of lf1 (1,3) and lf3 (3,3) (cone radius at floor ~1.39m)
        let result = s.execute("illuminance", &json!({"point": [2.0, 1.5, 0.0]})).unwrap();
        let contribs = result["contributions"].as_array().unwrap();
        // All four lights are reported, but only the two that are on contribute
        assert_eq!(contribs.len(), 4);
        let active: Vec<_> = contribs.iter().filter(|v| v["lux"].as_f64().unwrap() > 0.0).collect();
        assert_eq!(active.len(), 2, "only two lights should be on");
        // The off lights should have zero contribution
        let lf1 = contribs.iter().find(|v| v["id"] == json!("lf1")).unwrap();
        let lf3 = contribs.iter().find(|v| v["id"] == json!("lf3")).unwrap();
        assert_eq!(lf1["lux"].as_f64().unwrap(), 0.0);
        assert_eq!(lf3["lux"].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn illuminance_normal_direction_matters() {
        let mut s = fresh_session();
        // Normal pointing up (toward the ceiling lights)
        let up = s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "normal": [0.0, 0.0, 1.0]})).unwrap();
        let lux_up = up["lux"].as_f64().unwrap();

        // Normal pointing down (away from the lights)
        let down = s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "normal": [0.0, 0.0, -1.0]})).unwrap();
        let lux_down = down["lux"].as_f64().unwrap();

        // Lights are above, so the upward normal should receive more light
        assert!(lux_up > lux_down, "up={lux_up} down={lux_down}: upward normal should get more light from ceiling fixtures");
    }

    #[test]
    fn illuminance_huge_finite_coordinates() {
        let mut s = fresh_session();
        let result = s.execute("illuminance", &json!({"point": [1e300, 1e300, 1e300]})).unwrap();
        let lux = result["lux"].as_f64().unwrap();
        assert!(lux.is_finite(), "lux at huge finite point must be finite, got {lux}");
        // Very far away from all lights, so illuminance is essentially zero
        assert!(lux < 1.0, "lux should be negligible at 1e300 m away, got {lux}");
    }

    #[test]
    fn illuminance_refuses_infinite_point() {
        let mut s = fresh_session();
        assert!(s.execute("illuminance", &json!({"point": [f64::INFINITY, 0.0, 0.0]})).is_err());
        assert!(s.execute("illuminance", &json!({"point": [0.0, f64::NEG_INFINITY, 0.0]})).is_err());
        assert!(s.execute("illuminance", &json!({"point": [0.0, 0.0, f64::INFINITY]})).is_err());
    }

    #[test]
    fn illuminance_non_unit_normal_works() {
        let mut s = fresh_session();
        // Non-unit normal (twice as long as unit)
        let result = s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "normal": [0.0, 0.0, 2.0]})).unwrap();
        let lux = result["lux"].as_f64().unwrap();
        // Should give the same result as a unit normal (the calculation normalizes)
        let unit = s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "normal": [0.0, 0.0, 1.0]})).unwrap();
        let lux_unit = unit["lux"].as_f64().unwrap();
        assert!((lux - lux_unit).abs() < lux_unit * 0.01, "non-unit normal lux={lux} vs unit lux={lux_unit}");
    }

    #[test]
    fn illuminance_path_needs_the_renderer() {
        let mut s = fresh_session();
        let err = s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "method": "path"})).unwrap_err();
        assert!(err.to_string().contains("path-traced illuminance needs the 3D renderer"), "error={err}");
    }

    #[test]
    fn illuminance_parses_method_spp_bounces_seed_sun() {
        let mut s = fresh_session();

        // Invalid method name
        let err = s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "method": "foo"})).unwrap_err();
        assert!(err.to_string().contains("`method` must be"), "error={err}");

        // Path with valid params → renderer error (not a parse error)
        let err = s
            .execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "method": "path", "spp": 512, "bounces": 4, "seed": 1, "sun": false}))
            .unwrap_err();
        assert!(err.to_string().contains("path-traced illuminance needs the 3D renderer"), "error={err}");

        // Path with bad spp
        let err = s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "method": "path", "spp": 0})).unwrap_err();
        assert!(err.to_string().contains("`spp` must be"), "error={err}");

        // Path with bad bounces
        let err = s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "method": "path", "bounces": 33})).unwrap_err();
        assert!(err.to_string().contains("`bounces` must be"), "error={err}");

        // Path with bad seed
        let err = s.execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "method": "path", "seed": 4294967296u64})).unwrap_err();
        assert!(err.to_string().contains("`seed` must be"), "error={err}");

        // Path with sun object but no site → sun parse error before renderer check
        let err = s
            .execute("illuminance", &json!({"point": [1.0, 1.0, 0.0], "method": "path", "sun": {"date": "2026-06-21", "time": "12:00"}}))
            .unwrap_err();
        assert!(err.to_string().contains("sun needs a site location"), "error={err}");
    }
}
