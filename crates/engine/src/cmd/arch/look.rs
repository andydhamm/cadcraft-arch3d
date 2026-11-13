//! `look` command and the one reader/writer of look JSON (phase 19): technique, exposure, tone
//! map, sun, sky, light set, cutaway, clip box and near clip. `render` applies the same keys to
//! one render only ([`apply_look_params`] with `render = true`); `look` writes the drawing's
//! global look. Anything invalid is refused, never clamped, and a refused call changes nothing.

use cadcraft_arch::site::true_to_drawing;
use cadcraft_arch::sun::{Sun, sun_dir, sun_position};
use cadcraft_doc::arch::{ArchModel, Camera3d, ClipBox, ExposureSetting, Look, SunAngle, Technique, ToneMap};
use serde_json::{Map, Value, json};

use super::backdrop::describe as describe_backdrop;
use crate::cmd::{CommandSpec, bad};
use crate::{EngineError, Session};

/// The keys the look reads (shared by `look` and `render`).
pub(crate) const LOOK_KEYS: &[&str] = &["technique", "exposure", "toneMap", "sun", "sky", "lightSet", "cutaway", "clip", "nearClip"];

/// `look` command spec.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("look", "Render Look", run_look)
            .menu(&["Architecture", "Render Look"])
            .params("{technique?: standard|path|clay|technical, exposure?: \"auto\"|{ev100: -10..24}, toneMap?: aces|hable|none, sun?: false|\"default\"|{on?, intensity?: 0..200000, shadows?, angle?: name|null}, sky?: bool|{on?, turbidity?: 1.7..10, stars?, moon?: {lux?: 0..10, azimuth?, elevation?: -90..90}}, lightSet?: name|null, cutaway?: bool, clip?: null|{min: [x,y,z], max: [x,y,z]}, nearClip?: null|metres}"),
    ]
}

fn parse_err(cmd: &str, key: &str, v: &Value, want: &str) -> EngineError {
    bad(cmd, format!("`{key}` must be {want}, not {v}"))
}

fn check_keys(cmd: &str, what: &str, obj: &Map<String, Value>, allowed: &[&str]) -> Result<(), EngineError> {
    match obj.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(k) => Err(bad(cmd, format!("unknown key `{k}` in {what}"))),
        None => Ok(()),
    }
}

fn bool_of(cmd: &str, key: &str, v: &Value) -> Result<bool, EngineError> {
    v.as_bool().ok_or_else(|| parse_err(cmd, key, v, "true or false"))
}

fn num_of(cmd: &str, key: &str, v: &Value) -> Result<f64, EngineError> {
    v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| parse_err(cmd, key, v, "a finite number"))
}

/// Toward-sun direction (drawing space) for a compass azimuth and elevation.
fn compass_dir(model: &ArchModel, azimuth: f64, elevation: f64) -> Option<[f64; 3]> {
    if !azimuth.is_finite() || !elevation.is_finite() || !(-90.0..=90.0).contains(&elevation) {
        return None;
    }
    let d = sun_dir(&Sun { elevation_deg: elevation, azimuth_deg: azimuth, ..Default::default() });
    Some(true_to_drawing(model.site.as_ref(), d))
}

/// Toward-sun direction (drawing space) for a date and time at the drawing's site.
fn clock_dir(model: &ArchModel, date: &str, time: &str) -> Option<[f64; 3]> {
    let site = model.site.as_ref()?;
    let sun = sun_position(site.latitude, date, time)?;
    Some(true_to_drawing(Some(site), sun_dir(&sun)))
}

/// The direction a named sun angle gives: date and time need a site (`None` without one).
pub(crate) fn sun_angle_dir(model: &ArchModel, a: &SunAngle) -> Option<[f64; 3]> {
    match (&a.date, &a.time, a.azimuth, a.elevation) {
        (Some(d), Some(t), None, None) => clock_dir(model, d, t),
        (None, None, Some(az), Some(el)) => compass_dir(model, az, el),
        _ => None,
    }
}

/// The sun direction a look asks for: the explicit one, else its named angle's, else `None`
/// (the default light).
pub fn effective_sun_dir(model: &ArchModel, look: &Look, explicit: Option<[f64; 3]>) -> Option<[f64; 3]> {
    if explicit.is_some() {
        return explicit;
    }
    let name = look.sun.angle.as_ref()?;
    model.sun_angles.iter().find(|a| &a.name == name).and_then(|a| sun_angle_dir(model, a))
}

/// The look a view uses: the camera's own, else the global one with the drawing's backdrop (and a
/// legacy camera's light set).
pub fn effective_look(model: &ArchModel, camera: Option<&Camera3d>) -> Look {
    if let Some(l) = camera.and_then(|c| c.look.as_ref()) {
        return l.clone();
    }
    let mut l = model.look.clone();
    l.backdrop = model.backdrop.clone();
    if let Some(set) = camera.and_then(|c| c.light_set.as_ref()) {
        l.light_set = Some(set.clone());
    }
    l
}

/// The drawing's global look with its backdrop.
fn global_look(model: &ArchModel) -> Look {
    let mut l = model.look.clone();
    l.backdrop = model.backdrop.clone();
    l
}

/// A look as JSON, with exactly the keys the acceptance checks compare.
pub(crate) fn describe_look(l: &Look) -> Value {
    let enum_name = |v: Result<Value, serde_json::Error>| v.unwrap_or(Value::Null);
    json!({
        "technique": enum_name(serde_json::to_value(l.technique)),
        "exposure": match l.exposure {
            ExposureSetting::Auto => json!("auto"),
            ExposureSetting::Manual { ev100 } => json!({"ev100": ev100}),
        },
        "toneMap": enum_name(serde_json::to_value(l.tone_map)),
        "sun": {"on": l.sun.on, "intensity": l.sun.intensity, "shadows": l.sun.shadows, "angle": l.sun.angle},
        "sky": {
            "on": l.sky.on,
            "turbidity": l.sky.turbidity,
            "stars": l.sky.stars,
            "moon": {"lux": l.sky.moon.lux, "azimuth": l.sky.moon.azimuth, "elevation": l.sky.moon.elevation},
        },
        "backdrop": l.backdrop.as_ref().map_or(Value::Null, describe_backdrop),
        "lightSet": l.light_set,
        "cutaway": l.cutaway,
        "clip": l.clip.as_ref().map_or(Value::Null, |c| json!({"min": c.min, "max": c.max})),
        "nearClip": l.near_clip,
    })
}

fn parse_technique(cmd: &str, v: &Value) -> Result<Technique, EngineError> {
    match v.as_str() {
        Some("standard") => Ok(Technique::Standard),
        Some("path") => Ok(Technique::Path),
        Some("clay") => Ok(Technique::Clay),
        Some("technical") => Ok(Technique::Technical),
        _ => Err(parse_err(cmd, "technique", v, "standard, path, clay or technical")),
    }
}

fn parse_tone_map(cmd: &str, v: &Value) -> Result<ToneMap, EngineError> {
    match v.as_str() {
        Some("aces") => Ok(ToneMap::Aces),
        Some("hable") => Ok(ToneMap::Hable),
        Some("none") => Ok(ToneMap::None),
        _ => Err(parse_err(cmd, "toneMap", v, "aces, hable or none")),
    }
}

/// `exposure`: "auto" or {ev100}; with `render`, a bare number is the phase-12 path-tracer
/// multiplier and is left alone.
fn parse_exposure(cmd: &str, v: &Value, render: bool, look: &mut Look) -> Result<(), EngineError> {
    match v {
        Value::String(s) if s == "auto" => look.exposure = ExposureSetting::Auto,
        Value::Object(o) => {
            check_keys(cmd, "`exposure`", o, &["ev100"])?;
            let ev = o.get("ev100").ok_or_else(|| bad(cmd, "`exposure` needs `ev100`"))?;
            look.exposure = ExposureSetting::Manual { ev100: num_of(cmd, "exposure.ev100", ev)? };
        }
        Value::Number(_) if render => {}
        _ => return Err(parse_err(cmd, "exposure", v, "\"auto\" or {ev100: -10..24}")),
    }
    Ok(())
}

fn parse_sun(cmd: &str, model: &ArchModel, v: &Value, render: bool, look: &mut Look) -> Result<Option<[f64; 3]>, EngineError> {
    match v {
        Value::Bool(false) => {
            look.sun.on = false;
            if render {
                look.sky.on = false;
            }
            Ok(None)
        }
        Value::String(s) if s == "default" => Ok(None),
        Value::Object(o) => {
            check_keys(cmd, "`sun`", o, &["on", "intensity", "shadows", "angle", "date", "time", "azimuth", "elevation"])?;
            if let Some(x) = o.get("on") {
                look.sun.on = bool_of(cmd, "sun.on", x)?;
            }
            if let Some(x) = o.get("intensity") {
                look.sun.intensity = num_of(cmd, "sun.intensity", x)?;
            }
            if let Some(x) = o.get("shadows") {
                look.sun.shadows = bool_of(cmd, "sun.shadows", x)?;
            }
            let angle = match o.get("angle") {
                None => None,
                Some(Value::Null) => {
                    look.sun.angle = None;
                    None
                }
                Some(Value::String(name)) => {
                    if !model.sun_angles.iter().any(|a| &a.name == name) {
                        return Err(bad(cmd, format!("no sun angle named `{name}`")));
                    }
                    look.sun.angle = Some(name.clone());
                    Some(name)
                }
                Some(x) => return Err(parse_err(cmd, "sun.angle", x, "a sun angle name or null")),
            };
            let clock = o.contains_key("date") || o.contains_key("time");
            let compass = o.contains_key("azimuth") || o.contains_key("elevation");
            if !clock && !compass {
                return Ok(None);
            }
            if !render {
                return Err(bad(
                    cmd,
                    "a sun direction (date/time or azimuth/elevation) applies to one render, not to the look: save it with sunangle",
                ));
            }
            if (clock && compass) || angle.is_some() {
                return Err(bad(cmd, "give one sun direction: date and time, azimuth and elevation, or `angle`"));
            }
            if clock {
                let (Some(d), Some(t)) = (o.get("date"), o.get("time")) else {
                    return Err(bad(cmd, "`sun` needs both `date` and `time`"));
                };
                let (Some(ds), Some(ts)) = (d.as_str(), t.as_str()) else {
                    return Err(bad(cmd, format!("`sun.date` and `sun.time` must be strings, not {d} and {t}")));
                };
                if model.site.is_none() {
                    return Err(bad(cmd, "sun needs a site location: run site first"));
                }
                clock_dir(model, ds, ts).map(Some).ok_or_else(|| bad(cmd, format!("invalid sun date or time: {ds} {ts}")))
            } else {
                let (Some(a), Some(e)) = (o.get("azimuth"), o.get("elevation")) else {
                    return Err(bad(cmd, "`sun` needs both `azimuth` and `elevation`"));
                };
                let (az, el) = (num_of(cmd, "sun.azimuth", a)?, num_of(cmd, "sun.elevation", e)?);
                compass_dir(model, az, el).map(Some).ok_or_else(|| bad(cmd, format!("`sun.elevation` must be from -90 to 90, not {el}")))
            }
        }
        _ => Err(parse_err(cmd, "sun", v, "false, \"default\" or an object")),
    }
}

fn parse_sky(cmd: &str, v: &Value, look: &mut Look) -> Result<(), EngineError> {
    match v {
        Value::Bool(b) => look.sky.on = *b,
        Value::Object(o) => {
            check_keys(cmd, "`sky`", o, &["on", "turbidity", "stars", "moon"])?;
            if let Some(x) = o.get("on") {
                look.sky.on = bool_of(cmd, "sky.on", x)?;
            }
            if let Some(x) = o.get("turbidity") {
                look.sky.turbidity = num_of(cmd, "sky.turbidity", x)?;
            }
            if let Some(x) = o.get("stars") {
                look.sky.stars = bool_of(cmd, "sky.stars", x)?;
            }
            if let Some(m) = o.get("moon") {
                let Value::Object(mo) = m else { return Err(parse_err(cmd, "sky.moon", m, "an object")) };
                check_keys(cmd, "`sky.moon`", mo, &["lux", "azimuth", "elevation"])?;
                if let Some(x) = mo.get("lux") {
                    look.sky.moon.lux = num_of(cmd, "sky.moon.lux", x)?;
                }
                if let Some(x) = mo.get("azimuth") {
                    look.sky.moon.azimuth = num_of(cmd, "sky.moon.azimuth", x)?;
                }
                if let Some(x) = mo.get("elevation") {
                    look.sky.moon.elevation = num_of(cmd, "sky.moon.elevation", x)?;
                }
            }
        }
        _ => return Err(parse_err(cmd, "sky", v, "true, false or an object")),
    }
    Ok(())
}

fn parse_vec3(cmd: &str, key: &str, v: Option<&Value>) -> Result<[f64; 3], EngineError> {
    let missing = || bad(cmd, format!("`{key}` is required"));
    let v = v.ok_or_else(missing)?;
    let arr = v.as_array().filter(|a| a.len() == 3).ok_or_else(|| parse_err(cmd, key, v, "[x,y,z]"))?;
    let mut out = [0.0; 3];
    for (slot, x) in out.iter_mut().zip(arr) {
        *slot = num_of(cmd, key, x)?;
    }
    Ok(out)
}

fn parse_clip(cmd: &str, v: &Value, look: &mut Look) -> Result<(), EngineError> {
    match v {
        Value::Null => look.clip = None,
        Value::Object(o) => {
            check_keys(cmd, "`clip`", o, &["min", "max"])?;
            look.clip = Some(ClipBox {
                min: parse_vec3(cmd, "clip.min", o.get("min"))?,
                max: parse_vec3(cmd, "clip.max", o.get("max"))?,
                ..Default::default()
            });
        }
        _ => return Err(parse_err(cmd, "clip", v, "null or {min, max}")),
    }
    Ok(())
}

/// Apply the look keys in `params` to `look` (all or nothing: `look` is untouched on an error).
/// With `render` the keys are one render's own: a bare-number `exposure` is skipped (the
/// path tracer's phase-12 multiplier), `sun: false` is the legacy night (sky off too) and a sun
/// direction (`date`+`time`, or compass `azimuth`+`elevation`) is allowed. Returns the explicit
/// toward-sun direction, drawing space, when one was given.
pub(crate) fn apply_look_params(
    cmd: &str,
    params: &Value,
    model: &ArchModel,
    look: &mut Look,
    render: bool,
) -> Result<Option<[f64; 3]>, EngineError> {
    let Some(obj) = params.as_object() else {
        return if params.is_null() { Ok(None) } else { Err(bad(cmd, "params must be an object")) };
    };
    let mut work = look.clone();
    let mut dir = None;
    for key in LOOK_KEYS {
        let Some(v) = obj.get(*key) else { continue };
        match *key {
            "technique" => work.technique = parse_technique(cmd, v)?,
            "exposure" => parse_exposure(cmd, v, render, &mut work)?,
            "toneMap" => work.tone_map = parse_tone_map(cmd, v)?,
            "sun" => dir = parse_sun(cmd, model, v, render, &mut work)?,
            "sky" => parse_sky(cmd, v, &mut work)?,
            "lightSet" => match v {
                Value::Null => work.light_set = None,
                Value::String(n) if model.light_sets.iter().any(|s| s.name == n.trim()) => work.light_set = Some(n.trim().to_string()),
                Value::String(n) => return Err(bad(cmd, format!("unknown light set `{n}`"))),
                _ => return Err(parse_err(cmd, "lightSet", v, "a light set name or null")),
            },
            "cutaway" => work.cutaway = bool_of(cmd, "cutaway", v)?,
            "clip" => parse_clip(cmd, v, &mut work)?,
            "nearClip" => match v {
                Value::Null => work.near_clip = None,
                _ => work.near_clip = Some(num_of(cmd, "nearClip", v)?),
            },
            _ => {}
        }
    }
    work.validate().map_err(|e| bad(cmd, e))?;
    *look = work;
    Ok(dir)
}

/// Report or set the drawing's global look.
fn run_look(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let model = &s.doc()?.arch;
    if let Some(obj) = params.as_object() {
        if let Some(k) = obj.keys().find(|k| !LOOK_KEYS.contains(&k.as_str())) {
            return Err(bad("look", format!("unknown key `{k}`")));
        }
    } else if !params.is_null() {
        return Err(bad("look", "params must be an object"));
    }
    let mut look = model.look.clone();
    let before = look.clone();
    apply_look_params("look", params, model, &mut look, false)?;
    look.backdrop = None;
    if look != before {
        let d = s.doc_mut()?;
        d.arch.look = look;
        d.touch_arch();
    }
    Ok(json!({"look": describe_look(&global_look(&s.doc()?.arch))}))
}

fn resolved_sun(model: &ArchModel, a: &SunAngle) -> Option<(f64, f64, [f64; 3])> {
    match (&a.date, &a.time, a.azimuth, a.elevation) {
        (Some(date), Some(time), None, None) => {
            let site = model.site.as_ref()?;
            let sun = sun_position(site.latitude, date, time)?;
            let dir = true_to_drawing(Some(site), sun_dir(&sun));
            Some((sun.azimuth_deg, sun.elevation_deg, dir))
        }
        (None, None, Some(azimuth), Some(elevation)) => {
            if !azimuth.is_finite() || !elevation.is_finite() || !(-90.0..=90.0).contains(&elevation) {
                return None;
            }
            let dir = true_to_drawing(model.site.as_ref(), sun_dir(&Sun { azimuth_deg: azimuth, elevation_deg: elevation, ..Sun::default() }));
            Some((azimuth, elevation, dir))
        }
        _ => None,
    }
}

/// `arch.inspect` keys `look` (the global look) and `sunAngles`.
pub(crate) fn inspect_look(model: &ArchModel, p: &Value, out: &mut Map<String, Value>) {
    if p.get("look").and_then(Value::as_bool).unwrap_or(false) {
        out.insert("look".to_string(), describe_look(&global_look(model)));
    }
    if p.get("sunAngles").and_then(Value::as_bool).unwrap_or(false) {
        let list: Vec<Value> = model
            .sun_angles
            .iter()
            .map(|a| {
                let resolved = resolved_sun(model, a);
                let (sun_azimuth, sun_elevation, dir) = resolved.unwrap_or((f64::NAN, f64::NAN, [0.0; 3]));
                let ok = sun_azimuth.is_finite() && sun_elevation.is_finite();
                json!({
                    "name": a.name,
                    "date": a.date,
                    "time": a.time,
                    "azimuth": a.azimuth,
                    "elevation": a.elevation,
                    "dir": ok.then_some(dir),
                    "sunElevation": ok.then_some(sun_elevation),
                    "sunAzimuth": ok.then_some(sun_azimuth),
                })
            })
            .collect();
        out.insert("sunAngles".to_string(), Value::Array(list));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::site::SiteLocation;

    fn model() -> ArchModel {
        ArchModel::default()
    }

    fn site_model(true_north: f64) -> ArchModel {
        let mut m = model();
        m.site = Some(SiteLocation { latitude: 51.05, true_north, ..SiteLocation::default() });
        m
    }

    fn apply(m: &ArchModel, p: Value, render: bool) -> Result<(Look, Option<[f64; 3]>), EngineError> {
        let mut l = Look::default();
        let d = apply_look_params("look", &p, m, &mut l, render)?;
        Ok((l, d))
    }

    #[test]
    fn look_command_report_set_undo_refuse() {
        let mut s = Session::new();
        let r = s.execute("look", &json!({})).unwrap();
        assert_eq!(r["look"]["toneMap"], "aces");
        assert_eq!(r["look"]["exposure"], "auto");
        assert_eq!(r["look"]["sun"]["intensity"], 100000.0);
        assert!(r["look"]["backdrop"].is_null() && r["look"]["clip"].is_null());
        let r = s.execute("look", &json!({"exposure": {"ev100": 3}, "toneMap": "hable", "cutaway": true})).unwrap();
        assert_eq!(r["look"]["exposure"], json!({"ev100": 3.0}));
        assert_eq!(s.doc().unwrap().arch.look.tone_map, ToneMap::Hable);
        // Refused: nothing changes, not even the valid key beside the bad one.
        for bad_params in [
            json!({"toneMap": "filmic", "technique": "path"}),
            json!({"exposure": 5}),
            json!({"exposure": {"ev100": 99}}),
            json!({"sun": {"date": "2026-06-21", "time": "12:00"}}),
            json!({"sun": {"azimuth": 90, "elevation": 30}}),
            json!({"sun": {"angle": "Nope"}}),
            json!({"clip": {"min": [0, 0, 0], "max": [0, 1, 1]}}),
            json!({"nearClip": 0}),
            json!({"lightSet": "Nope"}),
            json!({"backdrop": {"kind": "sky"}}),
            json!({"bogus": 1}),
        ] {
            assert!(s.execute("look", &bad_params).is_err(), "should refuse {bad_params}");
        }
        assert_eq!(s.doc().unwrap().arch.look.technique, Technique::Standard);
        assert_eq!(s.doc().unwrap().arch.look.tone_map, ToneMap::Hable);
        assert_eq!(s.undo().unwrap(), Some("Render Look".to_string()));
        assert!(s.doc().unwrap().arch.look.is_default() || s.doc().unwrap().arch.look.tone_map == ToneMap::Aces);
    }

    #[test]
    fn apply_look_params_sun_forms() {
        let m = site_model(0.0);
        let (l, d) = apply(&m, json!({"sun": false}), true).unwrap();
        assert!(!l.sun.on && !l.sky.on && d.is_none());
        let (l, _) = apply(&m, json!({"sun": false}), false).unwrap();
        assert!(!l.sun.on && l.sky.on);
        let (_, d) = apply(&m, json!({"sun": "default"}), true).unwrap();
        assert!(d.is_none());
        let (l, d) = apply(&m, json!({"sun": {"intensity": 5000, "shadows": false, "azimuth": 90, "elevation": 30}}), true).unwrap();
        assert_eq!((l.sun.intensity, l.sun.shadows), (5000.0, false));
        let d = d.unwrap();
        assert!((d[0] - 30f64.to_radians().cos()).abs() < 1e-9 && d[1].abs() < 1e-9 && (d[2] - 0.5).abs() < 1e-9, "{d:?}");
        // A night sun is accepted.
        let (_, d) = apply(&m, json!({"sun": {"date": "2026-06-21", "time": "02:00"}}), true).unwrap();
        assert!(d.unwrap()[2] < 0.0);
        // True north turns the compass direction.
        let (_, d) = apply(&site_model(90.0), json!({"sun": {"azimuth": 0, "elevation": 0}}), true).unwrap();
        assert!((d.unwrap()[0] + 1.0).abs() < 1e-9);
        // No site: compass works, date/time is refused.
        assert!(apply(&model(), json!({"sun": {"azimuth": 90, "elevation": 30}}), true).is_ok());
        assert!(apply(&model(), json!({"sun": {"date": "2026-06-21", "time": "12:00"}}), true).is_err());
        // Refusals.
        for p in [
            json!({"sun": {"azimuth": 90}}),
            json!({"sun": {"azimuth": 90, "elevation": 91}}),
            json!({"sun": {"azimuth": 90, "elevation": 30, "date": "2026-06-21", "time": "12:00"}}),
            json!({"sun": {"azimuth": 90, "elevation": 30, "angle": "x"}}),
            json!({"sun": true}),
            json!({"sun": "off"}),
            json!({"sun": {"bogus": 1}}),
            json!({"sky": {"turbidity": 50}}),
            json!({"sky": {"moon": {"lux": -1}}}),
        ] {
            assert!(apply(&m, p.clone(), true).is_err(), "should refuse {p}");
        }
        // A named angle must exist; one that does is used.
        let mut m2 = site_model(0.0);
        m2.sun_angles.push(SunAngle { name: "noon".into(), azimuth: Some(180.0), elevation: Some(60.0), ..Default::default() });
        let (l, d) = apply(&m2, json!({"sun": {"angle": "noon"}}), false).unwrap();
        assert_eq!(l.sun.angle.as_deref(), Some("noon"));
        assert!(d.is_none());
        assert!(effective_sun_dir(&m2, &l, None).is_some_and(|d| d[2] > 0.8));
        assert!(effective_sun_dir(&m2, &l, Some([0.0, 0.0, 1.0])) == Some([0.0, 0.0, 1.0]));
        // A bare-number exposure is the render's own legacy key.
        let (l, _) = apply(&m, json!({"exposure": 0.5}), true).unwrap();
        assert_eq!(l.exposure, ExposureSetting::Auto);
    }

    #[test]
    fn effective_look_prefers_the_camera() {
        let mut m = model();
        m.look.tone_map = ToneMap::Hable;
        m.backdrop = Some(cadcraft_doc::arch::Backdrop::Sky);
        let l = effective_look(&m, None);
        assert_eq!(l.tone_map, ToneMap::Hable);
        assert_eq!(l.backdrop, Some(cadcraft_doc::arch::Backdrop::Sky));
        let legacy = Camera3d { name: "c".into(), light_set: Some("Evening".into()), ..Default::default() };
        assert_eq!(effective_look(&m, Some(&legacy)).light_set.as_deref(), Some("Evening"));
        assert_eq!(effective_look(&m, Some(&legacy)).tone_map, ToneMap::Hable);
        let own = Look { tone_map: ToneMap::None, ..Look::default() };
        let cam = Camera3d { name: "d".into(), look: Some(own.clone()), light_set: Some("Evening".into()), ..Default::default() };
        assert_eq!(effective_look(&m, Some(&cam)), own);
        let d = describe_look(&own);
        let keys: Vec<&str> = d.as_object().map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
        assert_eq!(keys.len(), 10, "{keys:?}");
    }

    #[test]
    fn view_look_uses_active_camera() {
        let mut d = cadcraft_doc::Drawing::new_imperial();
        d.arch.look.tone_map = ToneMap::Hable;
        let cam = Camera3d {
            name: "active".into(),
            kind: cadcraft_doc::arch::CameraKind::Overview,
            level: Some(2),
            lower_levels: Some(false),
            look: Some(Look { tone_map: ToneMap::None, ..Look::default() }),
            ..Default::default()
        };
        d.arch.cameras.push(cam.clone());
        d.arch.active_camera = Some("active".to_string());

        let v = crate::view_look(&d);
        assert_eq!(v.look.tone_map, ToneMap::None);
        assert_eq!(v.overview, Some((2, false)));
        assert_eq!(v.sun_dir, None);
    }
}
