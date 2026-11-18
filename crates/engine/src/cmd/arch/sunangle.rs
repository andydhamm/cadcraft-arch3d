//! The named sun angles command: store sun directions on the drawing, resolve them to
//! compass azimuth/elevation and a drawing-space direction vector, and refuse deletes that
//! would orphan a look (global or per-camera) still naming an angle.
use crate::cmd::CommandSpec;
use crate::{EngineError, Session};
use cadcraft_arch::site::true_to_drawing;
use cadcraft_arch::sun::{Sun, sun_dir, sun_position};
use cadcraft_doc::arch::site::SiteLocation;
use cadcraft_doc::arch::{ArchModel, Camera3d, Look, SunAngle, SunLook};
use serde_json::{Value, json};

/// The upper bound on named sun angles kept on one drawing.
const MAX_SUN_ANGLES: usize = 16;

/// Resolve a stored sun angle to its resolved compass azimuth/elevation and drawing-space direction.
fn resolved_sun(angle: &SunAngle, site: Option<&SiteLocation>) -> Option<(f64, f64, [f64; 3])> {
    match (&angle.date, &angle.time, angle.azimuth, angle.elevation) {
        (Some(date), Some(time), None, None) => {
            let site = site?;
            let sun = sun_position(site.latitude, date, time)?;
            let dir = true_to_drawing(Some(site), sun_dir(&sun));
            Some((sun.azimuth_deg, sun.elevation_deg, dir))
        }
        (None, None, Some(azimuth), Some(elevation)) => {
            if !azimuth.is_finite() || !elevation.is_finite() || !(-90.0..=90.0).contains(&elevation) {
                return None;
            }
            let dir = true_to_drawing(site, sun_dir(&Sun { azimuth_deg: azimuth, elevation_deg: elevation, ..Sun::default() }));
            Some((azimuth, elevation, dir))
        }
        _ => None,
    }
}

/// Resolve a stored sun angle to a JSON row using the site's orientation when needed.
fn resolve(angle: &SunAngle, site: Option<&SiteLocation>) -> Value {
    let (az, el, dir) = resolved_sun(angle, site).unwrap_or((f64::NAN, f64::NAN, [0.0; 3]));
    let ok = az.is_finite() && el.is_finite();
    json!({
        "name": angle.name,
        "date": angle.date,
        "time": angle.time,
        "azimuth": angle.azimuth,
        "elevation": angle.elevation,
        "dir": ok.then_some(dir),
        "sunElevation": ok.then_some(el),
        "sunAzimuth": ok.then_some(az),
    })
}

fn angle_by_name(model: &ArchModel, name: &str) -> Option<usize> {
    model.sun_angles.iter().position(|a| a.name == name)
}

fn is_used<'a>(name: &str, model: &'a ArchModel) -> Option<&'a str> {
    if model.look.sun.angle.as_deref() == Some(name) {
        return Some("the drawing's look");
    }
    for cam in &model.cameras {
        if cam.look.as_ref().and_then(|look| look.sun.angle.as_deref()) == Some(name) {
            return Some(cam.name.as_str());
        }
    }
    None
}

fn add_or_replace(s: &mut Session, angle: SunAngle, replace: bool) -> Result<(), EngineError> {
    let mut d = s.doc_mut()?;
    let model = &mut d.arch;
    if let Some(i) = angle_by_name(model, &angle.name) {
        if replace {
            model.sun_angles[i] = angle;
            d.touch_arch();
            return Ok(());
        }
        return Err(EngineError::Other(format!("sun angle `{}` already exists", angle.name)));
    }
    if model.sun_angles.len() >= MAX_SUN_ANGLES {
        return Err(EngineError::Other(format!("sun angle limit reached: at most {MAX_SUN_ANGLES}")));
    }
    model.sun_angles.push(angle);
    d.touch_arch();
    Ok(())
}

fn list_json(s: &Session) -> Result<Value, EngineError> {
    let model = &s.doc()?.arch;
    let site = model.site.as_ref();
    Ok(json!({ "sunAngles": model.sun_angles.iter().map(|a| resolve(a, site)).collect::<Vec<_>>() }))
}

/// Command specs for the sun angle module.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("sunangle", "Sun Angle", run_sunangle)
            .menu(&["Architecture", "Sun Angle"])
            .params("{name?, date?, time?, azimuth?, elevation?, replace?, delete?, list?}"),
    ]
}

fn run_sunangle(s: &mut Session, p: &Value) -> Result<Value, EngineError> {
    let name = p.get("name").and_then(Value::as_str).map(str::trim);
    let date = p.get("date").and_then(Value::as_str).map(|v| v.trim().to_string());
    let time = p.get("time").and_then(Value::as_str).map(|v| v.trim().to_string());
    let azimuth = p.get("azimuth").and_then(Value::as_f64);
    let elevation = p.get("elevation").and_then(Value::as_f64);
    let replace = p.get("replace").and_then(Value::as_bool).unwrap_or(false);
    let list = p.get("list").and_then(Value::as_bool).unwrap_or(false);
    let delete_name = p.get("delete").and_then(Value::as_str).map(str::trim);

    if list {
        return list_json(s);
    }

    if let Some(delete_name) = delete_name {
        let mut d = s.doc_mut()?;
        let idx = match d.arch.sun_angles.iter().position(|a| a.name == delete_name) {
            Some(i) => i,
            None => return Err(EngineError::Other(format!("sun angle `{delete_name}` not found"))),
        };
        if let Some(owner) = is_used(delete_name, &d.arch) {
            return Err(EngineError::Other(format!("sun angle `{delete_name}` is used by {owner}")));
        }
        d.arch.sun_angles.remove(idx);
        d.touch_arch();
        return list_json(s);
    }

    let name = match name {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => return Err(EngineError::Other("sunangle: `name` is required for add/replace/delete".into())),
    };

    let has_date_time = date.is_some() || time.is_some();
    let has_compass = azimuth.is_some() || elevation.is_some();
    if has_date_time && has_compass {
        return Err(EngineError::Other(format!("sunangle: `name` may use date/time or azimuth/elevation, not both for `{name}`")));
    }
    if !has_date_time && !has_compass {
        return Err(EngineError::Other("sunangle: provide either (date, time) or (azimuth, elevation) with `name`".into()));
    }
    if has_date_time {
        let date = date.ok_or_else(|| EngineError::Other("sunangle: `date` is required with `time`".into()))?;
        let time = time.ok_or_else(|| EngineError::Other("sunangle: `time` is required with `date`".into()))?;
        let site = s.doc()?.arch.site.as_ref().ok_or_else(|| EngineError::Other("sunangle: date/time form needs a site location".into()))?;
        // The date/time form stores only the date and time. The `sun_position` call is kept
        // purely as the check that they resolve at the site's latitude; the resolved
        // azimuth/elevation are derived on use by `resolved_sun`, never stored on the angle.
        let _ = sun_position(site.latitude, &date, &time)
            .ok_or_else(|| EngineError::Other(format!("sunangle `{name}`: could not resolve sun for {date} {time}")))?;
        let angle = SunAngle { name, date: Some(date), time: Some(time), azimuth: None, elevation: None, ..Default::default() };
        angle.validate().map_err(EngineError::Other)?;
        add_or_replace(s, angle, replace)?;
        return list_json(s);
    }

    let az = azimuth.ok_or_else(|| EngineError::Other(format!("sunangle `{name}`: `azimuth` is required")))?;
    let el = elevation.ok_or_else(|| EngineError::Other(format!("sunangle `{name}`: `elevation` is required")))?;
    if !az.is_finite() || !el.is_finite() {
        return Err(EngineError::Other(format!("sunangle `{name}`: azimuth/elevation must be finite, got {az} / {el}")));
    }
    if !(-90.0..=90.0).contains(&el) {
        return Err(EngineError::Other(format!("sunangle `{name}`: elevation {el} out of range -90..=90")));
    }
    let angle = SunAngle { name, date: None, time: None, azimuth: Some(az), elevation: Some(el), ..Default::default() };
    angle.validate().map_err(EngineError::Other)?;
    add_or_replace(s, angle, replace)?;
    list_json(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_angle(s: &mut Session, name: &str, azimuth: f64, elevation: f64) -> Value {
        s.execute("sunangle", &json!({"name": name, "azimuth": azimuth, "elevation": elevation})).expect("sunangle add failed")
    }

    fn list(s: &mut Session) -> Vec<String> {
        s.execute("sunangle", &json!({"list": true}))
            .map(|v| {
                v["sunAngles"]
                    .as_array()
                    .map(|a| a.iter().map(|x| x["name"].as_str().map(str::to_owned).unwrap_or_default()).collect())
                    .unwrap_or_default()
            })
            .unwrap_or_default()
    }

    #[test]
    fn add_list_replace_delete() {
        let mut s = Session::new();
        add_angle(&mut s, "NOON", 180.0, 62.39);
        assert!(list(&mut s).contains(&"NOON".to_string()), "NOON not in list");
        assert_eq!(list(&mut s), vec!["NOON"]);

        add_angle(&mut s, "LOW", 180.0, 30.0);
        let mid = list(&mut s);
        assert!(mid.contains(&"NOON".to_string()));
        assert!(mid.contains(&"LOW".to_string()));
        assert_eq!(mid.len(), 2);

        s.execute("sunangle", &json!({"name": "NOON", "azimuth": 90.0, "elevation": 10.0, "replace": true})).expect("sunangle replace failed");
        assert_eq!(list(&mut s).len(), 2);

        s.execute("sunangle", &json!({"delete": "LOW"})).expect("sunangle delete failed");
        assert!(!list(&mut s).contains(&"LOW".to_string()));
        assert_eq!(list(&mut s), vec!["NOON"]);

        s.execute("sunangle", &json!({"delete": "NOON"})).expect("sunangle delete NOON");
        assert!(list(&mut s).is_empty());
    }

    #[test]
    fn date_time_angle_is_accepted_and_resolves() {
        let mut s = Session::new();
        let mut d = s.doc_mut().unwrap();
        d.arch.site = Some(SiteLocation { latitude: 51.05, ..SiteLocation::default() });
        d.touch_arch();
        let r = s.execute("sunangle", &json!({"name": "noon", "date": "2026-06-21", "time": "solar-noon"}));
        assert!(r.is_ok(), "sunangle add should succeed");
        let list: Value = s.execute("sunangle", &json!({"list": true})).expect("sunangle list");
        let row = &list["sunAngles"][0];
        let el: f64 = row["sunElevation"].as_f64().expect("sunElevation");
        let az: f64 = row["sunAzimuth"].as_f64().expect("sunAzimuth");
        assert!((el - 62.39).abs() < 0.1, "sunElevation {} not near 62.39", el);
        assert!((az - 180.0).abs() < 0.1, "sunAzimuth {} not near 180", az);
        let dir = row["dir"].as_array().expect("dir array");
        let x = dir[0].as_f64().unwrap();
        let y = dir[1].as_f64().unwrap();
        let z = dir[2].as_f64().unwrap();
        assert!((x * x + y * y + z * z - 1.0).abs() < 1e-6, "dir not unit: ({}, {}, {})", x, y, z);
        assert!(z > 0.0, "dir z should be positive: {}", z);
        assert!(y < 0.0, "dir y should be negative (south): {}", y);
    }

    #[test]
    fn date_time_angle_lists_null_azimuth_and_elevation() {
        let mut s = Session::new();
        let mut d = s.doc_mut().unwrap();
        d.arch.site = Some(SiteLocation { latitude: 51.05, ..SiteLocation::default() });
        d.touch_arch();
        let _ = s.execute("sunangle", &json!({"name": "noon", "date": "2026-06-21", "time": "solar-noon"})).expect("sunangle add");
        let v: Value = s.execute("sunangle", &json!({"list": true})).expect("sunangle list");
        let row = &v["sunAngles"][0];
        assert_eq!(row["date"], json!("2026-06-21"));
        assert_eq!(row["time"], json!("solar-noon"));
        assert_eq!(row["azimuth"], json!(null));
        assert_eq!(row["elevation"], json!(null));
        assert!(row["sunElevation"].is_number());
        assert!(row["sunAzimuth"].is_number());
        assert!(row["dir"].is_array());
    }

    #[test]
    fn look_uses_a_date_time_angle() {
        let mut s = Session::new();
        // Set the site first so the mutable borrow ends before the first `s.execute`.
        {
            let mut d = s.doc_mut().unwrap();
            d.arch.site = Some(SiteLocation { latitude: 51.05, ..SiteLocation::default() });
            d.touch_arch();
        }
        s.execute("sunangle", &json!({"name": "noon", "date": "2026-06-21", "time": "solar-noon"})).expect("sunangle add");
        {
            let mut d = s.doc_mut().unwrap();
            d.arch.look.sun.angle = Some("noon".to_string());
            d.touch_arch();
        }
        {
            let model = &s.doc().unwrap().arch;
            let angle = &model.sun_angles[0];
            let site = model.site.as_ref();
            let (az, el, dir) = resolved_sun(angle, site).expect("resolved_sun");
            assert!((az - 180.0).abs() < 0.1, "sunAzimuth {} not near 180", az);
            assert!((el - 62.39).abs() < 0.1, "sunElevation {} not near 62.39", el);
            assert!((dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2] - 1.0).abs() < 1e-6, "dir not unit");
            assert!(dir[2] > 0.0, "dir z positive");
        }
        // Deleting the angle used by the look is refused.
        let del = s.execute("sunangle", &json!({"delete": "noon"}));
        assert!(del.is_err(), "deleting an angle used by the look should be refused");
        assert_eq!(s.doc().unwrap().arch.sun_angles.len(), 1, "angle still present after refused delete",);
    }

    #[test]
    fn date_time_without_site_is_refused() {
        let mut s = Session::new();
        let r = s.execute("sunangle", &json!({"name": "noon", "date": "2026-06-21", "time": "solar-noon"}));
        assert!(r.is_err(), "date/time form without site should be refused");
        let err = r.unwrap_err().to_string();
        assert!(err.contains("site"), "error should mention site: {err}");
    }

    #[test]
    fn compass_angle_with_nan_values_is_refused() {
        let mut s = Session::new();
        let r = s.execute("sunangle", &json!({"name": "NAN", "azimuth": f64::NAN, "elevation": 45.0}));
        assert!(r.is_err(), "NaN azimuth should be refused");
        let r = s.execute("sunangle", &json!({"name": "NAN", "azimuth": 180.0, "elevation": f64::NAN}));
        assert!(r.is_err(), "NaN elevation should be refused");
        let r = s.execute("sunangle", &json!({"name": "INF", "azimuth": f64::INFINITY, "elevation": 45.0}));
        assert!(r.is_err(), "inf azimuth should be refused");
    }

    #[test]
    fn replace_date_time_with_compass_and_back() {
        let mut s = Session::new();
        {
            let mut d = s.doc_mut().unwrap();
            d.arch.site = Some(SiteLocation { latitude: 51.05, ..SiteLocation::default() });
            d.touch_arch();
        }
        s.execute("sunangle", &json!({"name": "noon", "date": "2026-06-21", "time": "solar-noon"})).expect("add date/time");
        let v: Value = s.execute("sunangle", &json!({"list": true})).expect("list");
        assert_eq!(v["sunAngles"][0]["date"], json!("2026-06-21"));
        assert_eq!(v["sunAngles"][0]["azimuth"], json!(null));

        s.execute("sunangle", &json!({"name": "noon", "azimuth": 90.0, "elevation": 30.0, "replace": true})).expect("replace with compass");
        let v: Value = s.execute("sunangle", &json!({"list": true})).expect("list");
        assert_eq!(v["sunAngles"][0]["date"], json!(null));
        assert_eq!(v["sunAngles"][0]["azimuth"], json!(90.0));

        s.execute("sunangle", &json!({"name": "noon", "date": "2026-12-21", "time": "solar-noon", "replace": true})).expect("replace with date/time");
        let v: Value = s.execute("sunangle", &json!({"list": true})).expect("list");
        assert_eq!(v["sunAngles"][0]["date"], json!("2026-12-21"));
        assert_eq!(v["sunAngles"][0]["azimuth"], json!(null));
    }

    #[test]
    fn list_with_no_angles_returns_empty_array() {
        let mut s = Session::new();
        let v: Value = s.execute("sunangle", &json!({"list": true})).expect("list");
        let arr = v["sunAngles"].as_array().expect("sunAngles should be an array");
        assert!(arr.is_empty(), "list should be empty");
    }

    #[test]
    fn elevation_boundary_values_accepted() {
        let mut s = Session::new();
        s.execute("sunangle", &json!({"name": "ZENITH", "azimuth": 0.0, "elevation": 90.0})).expect("elevation 90 should be accepted");
        s.execute("sunangle", &json!({"name": "NADIR", "azimuth": 0.0, "elevation": -90.0})).expect("elevation -90 should be accepted");
        let v: Value = s.execute("sunangle", &json!({"list": true})).expect("list");
        assert_eq!(v["sunAngles"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn refuses_bad_forms_duplicates_and_used_deletes() {
        let mut s = Session::new();

        let bad = s.execute("sunangle", &json!({"name": "BAD"}));
        assert!(bad.is_err(), "sunangle bad form should be refused");

        add_angle(&mut s, "A", 180.0, 62.39);
        let dup = s.execute("sunangle", &json!({"name": "A", "azimuth": 180.0, "elevation": 30.0}));
        assert!(dup.is_err(), "sunangle duplicate should be refused");

        let range = s.execute("sunangle", &json!({"name": "RANGE", "azimuth": 180.0, "elevation": 120.0}));
        assert!(range.is_err(), "sunangle out-of-range elevation should be refused");

        let del_missing = s.execute("sunangle", &json!({"delete": "NOPE"}));
        assert!(del_missing.is_err(), "sunangle delete missing should be refused");

        {
            let mut d = s.doc_mut().unwrap();
            d.arch.look.sun.angle = Some("A".to_string());
            d.touch_arch();
        }
        let del_used = s.execute("sunangle", &json!({"delete": "A"}));
        assert!(del_used.is_err(), "sunangle delete used-by-look should be refused");

        {
            let mut d = s.doc_mut().unwrap();
            d.arch.cameras.push(Camera3d {
                name: "Cam".into(),
                look: Some(Look { sun: SunLook { angle: Some("A".to_string()), ..SunLook::default() }, ..Look::default() }),
                ..Camera3d::default()
            });
            d.touch_arch();
        }
        let del_used_cam = s.execute("sunangle", &json!({"delete": "A"}));
        assert!(del_used_cam.is_err(), "sunangle delete used-by-camera should be refused");

        {
            let mut d = s.doc_mut().unwrap();
            d.arch.look.sun.angle = None;
            if let Some(c) = d.arch.cameras.first_mut() {
                c.look = None;
            }
            d.touch_arch();
        }
        let ok = s.execute("sunangle", &json!({"delete": "A"}));
        assert!(ok.is_ok(), "sunangle delete after clearing usage should succeed");
        assert!(list(&mut s).is_empty());
    }

    #[test]
    fn max_sun_angles_limit_enforced() {
        let mut s = Session::new();
        for i in 0..16 {
            let name = format!("A{}", i);
            add_angle(&mut s, &name, 180.0, 45.0);
        }
        assert_eq!(list(&mut s).len(), 16);
        let r = s.execute("sunangle", &json!({"name": "A16", "azimuth": 180.0, "elevation": 45.0}));
        assert!(r.is_err(), "17th angle should be refused");
        assert_eq!(list(&mut s).len(), 16, "list unchanged after refused add");
    }

    #[test]
    fn replace_nonexistent_angle_adds_it() {
        let mut s = Session::new();
        let r = s.execute("sunangle", &json!({"name": "NEW", "azimuth": 90.0, "elevation": 30.0, "replace": true}));
        assert!(r.is_ok(), "replace of nonexistent should add");
        assert!(list(&mut s).contains(&"NEW".to_string()));
    }

    #[test]
    fn mixed_date_time_and_compass_forms_refused() {
        let mut s = Session::new();
        let r = s.execute("sunangle", &json!({"name": "MIXED", "date": "2026-06-21", "time": "solar-noon", "azimuth": 180.0, "elevation": 45.0}));
        assert!(r.is_err(), "mixed forms should be refused");
    }

    #[test]
    fn invalid_date_format_refused() {
        let mut s = Session::new();
        {
            let mut d = s.doc_mut().unwrap();
            d.arch.site = Some(SiteLocation { latitude: 51.05, ..SiteLocation::default() });
            d.touch_arch();
        }
        let r = s.execute("sunangle", &json!({"name": "BADDATE", "date": "not-a-date", "time": "solar-noon"}));
        assert!(r.is_err(), "invalid date should be refused");
    }

    #[test]
    fn empty_and_whitespace_names_refused() {
        let mut s = Session::new();
        let r = s.execute("sunangle", &json!({"name": "", "azimuth": 180.0, "elevation": 45.0}));
        assert!(r.is_err(), "empty name should be refused");
        let r = s.execute("sunangle", &json!({"name": "   ", "azimuth": 180.0, "elevation": 45.0}));
        assert!(r.is_err(), "whitespace-only name should be refused");
    }

    #[test]
    fn delete_angle_used_by_multiple_cameras_refused() {
        let mut s = Session::new();
        add_angle(&mut s, "SHARED", 180.0, 45.0);
        {
            let mut d = s.doc_mut().unwrap();
            d.arch.cameras.push(Camera3d {
                name: "Cam1".into(),
                look: Some(Look { sun: SunLook { angle: Some("SHARED".to_string()), ..SunLook::default() }, ..Look::default() }),
                ..Camera3d::default()
            });
            d.arch.cameras.push(Camera3d {
                name: "Cam2".into(),
                look: Some(Look { sun: SunLook { angle: Some("SHARED".to_string()), ..SunLook::default() }, ..Look::default() }),
                ..Camera3d::default()
            });
            d.touch_arch();
        }
        let r = s.execute("sunangle", &json!({"delete": "SHARED"}));
        assert!(r.is_err(), "delete should be refused when used by multiple cameras");
    }

    #[test]
    fn delete_angle_with_camera_null_look_allowed() {
        let mut s = Session::new();
        add_angle(&mut s, "FREE", 180.0, 45.0);
        {
            let mut d = s.doc_mut().unwrap();
            d.arch.cameras.push(Camera3d { name: "CamNoLook".into(), look: None, ..Camera3d::default() });
            d.touch_arch();
        }
        let r = s.execute("sunangle", &json!({"delete": "FREE"}));
        assert!(r.is_ok(), "delete should succeed when camera has null look");
    }

    #[test]
    fn azimuth_accepts_any_finite_value() {
        let mut s = Session::new();
        let r = s.execute("sunangle", &json!({"name": "FULL", "azimuth": 360.0, "elevation": 45.0}));
        assert!(r.is_ok(), "azimuth 360 should be accepted");
        let r = s.execute("sunangle", &json!({"name": "DOUBLE", "azimuth": 720.0, "elevation": 45.0}));
        assert!(r.is_ok(), "azimuth 720 should be accepted");
        let r = s.execute("sunangle", &json!({"name": "NEG", "azimuth": -45.0, "elevation": 45.0}));
        assert!(r.is_ok(), "negative azimuth should be accepted");
    }

    #[test]
    fn elevation_just_out_of_range_refused() {
        let mut s = Session::new();
        let r = s.execute("sunangle", &json!({"name": "HIGH", "azimuth": 180.0, "elevation": 90.1}));
        assert!(r.is_err(), "elevation 90.1 should be refused");
        let r = s.execute("sunangle", &json!({"name": "LOW", "azimuth": 180.0, "elevation": -90.1}));
        assert!(r.is_err(), "elevation -90.1 should be refused");
    }
}
