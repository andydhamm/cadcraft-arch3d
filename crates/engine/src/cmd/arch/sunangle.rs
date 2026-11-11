//! The named sun angles command: store sun directions on the drawing, resolve them to
//! compass azimuth/elevation and a drawing-space direction vector, and refuse deletes that
//! would orphan a look (global or per-camera) that still names an angle.
use crate::cmd::CommandSpec;
use crate::{EngineError, Session};
use cadcraft_arch::site::SiteLocation;
use cadcraft_doc::arch::Site;
use cadcraft_doc::Handle;
use cadcraft_geom::Vec2;
use std::f64::consts::PI;
use serde_json::{json, Value};

/// The upper bound on named sun angles kept on one drawing.
const MAX_SUN_ANGLES: usize = 16;

/// Sun angle direction: compass azimuth/elevation turned into drawing space.
fn compass_dir(azimuth: f64, elevation: f64) -> Vec2 {
    let a = azimuth * PI / 360.0;
    let e = elevation * PI / 360.0;
    let x = a.sin();
    let y = a.cos();
    let z = e.sin();
    Vec2::new(x, y) // y carries z; the resolver maps y -> z in the 3D frame
}

/// Resolve a stored sun angle to a resolved direction and compass values, using the site.
fn resolve(angle: &cadcraft_doc::arch::SunAngle, site: Option<&SiteLocation>) -> Value {
    // compass azimuth/elevation: stored directly, or derived from date/time via the sun module.
    let (az, el): (f64, f64) = if !angle.date.is_empty() && !angle.time.is_empty() {
        if let Some(s) = site {
            match cadcraft_arch::sun::solar_position(s.lat, s.lon, &angle.date, &angle.time) {
                Ok((az, el)) => (az, el),
                Err(_) => (azimuth_default(), elevation_default()),
            }
        } else {
            (f64::NAN, f64::NAN)
        }
    } else {
        (angle.azimuth, angle.elevation)
    };
    let finite = az.is_finite() && el.is_finite();
    let dir = if finite {
        Some(compass_dir(az, el).into_array())
    } else {
        None
    };
    json!({
        "name": angle.name,
        "date": angle.date,
        "time": angle.time,
        "azimuth": if finite { json!(az) } else { json!(null) },
        "elevation": if finite { json!(el) } else { json!(null) },
        "dir": dir,
        "sunElevation": if finite { json!(el) } else { json!(null) },
        "sunAzimuth": if finite { json!(az) } else { json!(null) },
    })
}

fn azimuth_default() -> f64 { f64::NAN }
fn elevation_default() -> f64 { f64::NAN }

fn angle_by_name(model: &cadcraft_doc::arch::ArchModel, name: &str) -> usize {
    model.sun_angles.iter().position(|a| a.name == name)
}

fn is_used(name: &str, model: &cadcraft_doc::arch::ArchModel) -> Option<&String> {
    if model.look.sun.angle.as_deref() == Some(name) {
        return Some(&String::from("the drawing's look"));
    }
    for cam in &model.cameras {
        if cam.saved && cam.look.sun.angle.as_deref() == Some(name) {
            return Some(&String::from(&cam.name));
        }
    }
    None
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
    let delete = p.get("delete").and_then(Value::as_str).map(str::trim);
    let site = s.doc()?.arch.site.as_ref();

    if list {
        let model = &s.doc()?.arch;
        let site = s.doc()?.arch.site.as_ref();
        let angles: Vec<Value> = model
            .sun_angles
            .iter()
            .map(|a| resolve(a, site))
            .collect();
        return Ok(json!({"sunAngles": angles}));
    }

    if let Some(d) = delete {
        let mut model = s.doc_mut()?.arch;
        let idx = match angle_by_name(&model, d) {
            Some(i) => i,
            None => {
                return Err(EngineError::Other(format!("sun angle `{d}` not found")));
            }
        };
        if let Some(owner) = is_used(d, &model) {
            return Err(EngineError::Other(format!("sun angle `{d}` is used by {owner}")));
        }
        let _ = model.sun_angles.remove(idx);
        s.doc_mut()?.touch_arch();
        return Ok(list_json(s));
    }

    let name = match name {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => {
            return Err(EngineError::Other("sunangle: `name` is required for add/replace/delete".into()));
        }
    };

    let direct = azimuth.is_some() && elevation.is_some();
    if direct {
        let az = azimuth.unwrap();
        let el = elevation.unwrap();
        if !az.is_finite() || !el.is_finite() {
            return Err(EngineError::Other(format!(
                "sunangle `sunangle {name}`: azimuth/elevation must be finite, got {az} / {el}"
            )));
        }
        if !(el >= -90.0 && el <= 90.0) {
            return Err(EngineError::Other(format!(
                "sunangle `sunangle {name}`: elevation {el} out of range -90..=90"
            )));
        }
        let angle = cadraft_doc_angle(name, date, time, az, el);
        add_or_replace(s, angle, replace);
    } else if !date.is_empty() && !time.is_empty() {
        let site = site.ok_or_else(|| EngineError::Other("sunangle: date/time form needs a site location".into()))?;
        let (az, el) = cadraft_arch::sun::solar_position(site.lat, site.lon, &date, &time)
            .map_err(|_| EngineError::Other("sunangle: could not resolve sun from date/time".into()))?;
        let angle = cadraft_doc_angle(name, date, time, az, el);
        add_or_replace(s, angle, replace);
    } else {
        return Err(EngineError::Other(
            "sunangle: provide either (date, time) or (azimuth, elevation) with `name`".into(),
        ));
    }
    Ok(list_json(s))
}

fn cadraft_doc_angle(
    name: String,
    date: String,
    time: String,
    azimuth: f64,
    elevation: f64,
) -> cadraft_doc::arch::SunAngle {
    cadcraft_doc::arch::SunAngle {
        name,
        date,
        time,
        azimuth,
        elevation,
        ..cadraft_doc::arch::SunAngle::default()
    }
}

fn add_or_replace(s: &mut Session, angle: cadraft_doc::arch::SunAngle, replace: bool) -> Result<(), EngineError> {
    let mut model = s.doc_mut()?.arch;
    if replace {
        let idx = match angle_by_name(&model, &angle.name) {
            Some(i) => i,
            None => return Err(EngineError::Other(format!("sunangle: `sunangle {}` not found", angle.name))),
        };
        if model.sun_angles.len() >= MAX_SUN_ANGLES {
            return Err(EngineError::Other("sunangle: too many sun angles".into()));
        }
        model.sun_angles[idx] = angle;
    } else {
        if model.sun_angles.iter().any(|a| a.name == angle.name) {
            return Err(EngineError::Other(format!("sunangle: duplicate name `{}`", angle.name)));
        }
        if model.sun_angles.len() >= MAX_SUN_ANGLES {
            return Err(EngineError::Other("sunangle: too many sun angles".into()));
        }
        model.sun_angles.push(angle);
    }
    s.doc_mut()?.touch_arch();
    Ok(())
}

fn list_json(s: &Session) -> Value {
    let model = &s.doc()?.arch;
    let site = s.doc()?.arch.site.as_ref();
    let angles: Vec<Value> = model.sun_angles.iter().map(|a| resolve(a, site)).collect();
    json!({"sunAngles": angles})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_angle(s: &mut Session, name: &str, azimuth: f64, elevation: f64) -> Value {
        s.execute("sunangle", &json!({"name": name, "azimuth": azimuth, "elevation": elevation}))
            .expect("sunangle add failed")
    }

    fn list(s: &Session) -> Vec<String> {
        s.execute("sunangle", &json!({"list": true}))
            .map(|v| {
                v["sunAngles"]
                    .as_array()
                    .map(|a| a.iter().map(|x| x["name"].as_str().map(|s| s.to_string()).unwrap_or_default()).collect::<Vec<_>>())
                    .unwrap_or_default()
            })
            .unwrap_or_default()
    }

    #[test]
    fn add_list_replace_delete() {
        let mut s = Session::new();
        add_angle(&mut s, "NOON", 180.0, 62.39);
        assert!(list(&s).contains(&"NOON".into()), "NOON not in list");
        let before = list(&s).clone();
        assert_eq!(before, vec!["NOON"]);
        add_angle(&mut s, "LOW", 180.0, 30.0);
        let mid = list(&mut s);
        assert!(mid.contains(&"NOON".into()));
        assert!(mid.contains(&"LOW".into()));
        assert_eq!(mid.len(), 2);
        s.execute("sunangle", &json!({"name": "NOON", "azimuth": 90.0, "elevation": 10.0, "replace": true}))
            .expect("sunangle replace failed");
        assert!(list(&s).contains(&"NOON".into()));
        assert_eq!(list(&s).len(), 2);
        s.execute("sunangle", &json!({"delete": "LOW"})).expect("sunangle delete failed");
        assert!(!list(&s).contains(&"LOW".into()));
        assert_eq!(list(&s), vec!["NOON"]);
        s.execute("sunangle", &json!({"delete": "NOON"})).expect("sunangle delete NOON");
        assert_eq!(list(&s), vec![]);
    }

    #[test]
    fn refuses_bad_forms_duplicates_and_used_deletes() {
        let mut s = Session::new();
        // Bad form: name with neither (date,time) nor (azimuth,elevation).
        let bad = s.execute("sunangle", &json!({"name": "BAD"}));
        assert!(bad.is_err(), "sunangle bad form should be refused");
        // Duplicate name.
        add_angle(&mut s, "A", 180.0, 62.39);
        let dup = s.execute("sunangle", &json!({"name": "A", "azimuth": 180.0, "elevation": 30.0}));
        assert!(dup.is_err(), "sunangle duplicate should be refused");
        // Out-of-range elevation (refused, never clamped).
        let range = s.execute("sunangle", &json!({"name": "RANGE", "azimuth": 180.0, "elevation": 120.0}));
        assert!(range.is_err(), "sunangle out-of-range elevation should be refused");
        // Delete: name that doesn't exist.
        let del_missing = s.execute("sunangle", &json!({"delete": "NOPE"}));
        assert!(del_missing.is_err(), "sunangle delete missing should be refused");
        // Delete: refuse when the drawing's look uses it.
        {
            if let Ok(d) = s.doc_mut() {
                d.arch.look.sun.angle = Some("A".to_string());
                d.touch_arch();
            }
        }
        let del_used = s.execute("sunangle", &json!({"delete": "A"}));
        assert!(del_used.is_err(), "sunangle delete used-by-look should be refused");
        // Refusing delete also refuses when a saved camera look uses it.
        if let Ok(d) = s.doc_mut() {
            d.arch.cameras.push(cadcraft_doc::arch::Camera3d {
                name: "Cam".into(),
                look: cadcraft_doc::arch::Look {
                    sun: cadcraft_doc::arch::Sun {
                        angle: Some("A".to_string()),
                        ..cadcraft_doc::arch::Sun::default()
                    },
                    ..cadcraft_doc::arch::Look::default()
                },
                saved: true,
                ..cadcraft_doc::arch::Camera3d::default()
            });
            d.touch_arch();
        }
        let del_used_cam = s.execute("sunangle", &json!({"delete": "A"}));
        assert!(del_used_cam.is_err(), "sunangle delete used-by-camera should be refused");
        // After clearing the look usage, delete is allowed.
        if let Ok(d) = s.doc_mut() {
            d.arch.look.sun.angle = None;
            if let Some(c) = d.arch.cameras.first_mut() {
                c.look.sun.angle = None;
            }
            d.touch_arch();
        }
        let ok = s.execute("sunangle", &json!({"delete": "A"}));
        assert!(ok.is_ok(), "sunangle delete after clearing usage should succeed");
        assert_eq!(list(&s), vec![]);
    }
}
