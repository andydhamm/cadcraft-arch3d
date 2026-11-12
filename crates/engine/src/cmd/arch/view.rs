//! Architecture 3D view commands.
//!
//! `view3d` builds and reports the architectural 3D scene. `camera`
//! validates and echoes the orbit camera parameters used by the 3D pane.

use serde_json::{Value, json};

use cadcraft_doc::arch::{Camera3d, CameraKind};

use super::look::{LOOK_KEYS, apply_look_params, effective_look};
use crate::{
    EngineError, Session,
    cmd::{CommandSpec, bad},
};

/// Largest accepted orbit distance, in metres (matches the 3D pane's limit).
const MAX_DISTANCE: f64 = 1_000_000.0;
/// Walkthrough inputs at or beyond this magnitude are refused.
const MAX_WALK: f64 = 1e6;

/// `view3d`, `camera` and `walkthrough` command specs.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("view3d", "3D View", run_view3d).menu(&["Architecture", "3D View"]).params("{}").noundo(),
        CommandSpec::new("camera", "Camera", run_camera).params(
            "{save:name, kind?:orbit|full|overview, target?, azimuth?, elevation?, distance?, eye?, eyeHeight?, fov?, level?: name|index, lowerLevels?: bool, technique?, exposure?, toneMap?, sun?: {angle?…}, sky?, lightSet?, cutaway?, clip?, nearClip?} | {list:true} | {use:name} | {delete:name} | {azimuth?, elevation?, distance?, target?}",
        ),
        CommandSpec::new("camera.reset", "Reset Camera", run_camera_reset).params("{name?: camera (default: the active one)}"),
        CommandSpec::new("camera.orbit", "Orbit Camera", run_camera_orbit)
            .params("{name?, azimuth?: delta degrees, elevation?: delta degrees (result within ±89), distance?: factor > 0}"),
        CommandSpec::new("camera.update", "Update Camera", run_camera_update)
            .params("{name?, target?, azimuth?, elevation?, distance?, eye?, eyeHeight?, fov?, level?, lowerLevels?, + the look keys of camera save}"),
        CommandSpec::new("walkthrough", "Walkthrough", run_walkthrough)
            .menu(&["Architecture", "Walkthrough"])
            .params("{eye,yaw,forward?,strafe?,turn?,step?}")
            .noundo(),
    ]
}

/// Build the 3D scene from the arch model and report its state.
///
/// Returns `{"ok":true,"meshes":N,"revision":rev}`. When the drawing carries
/// no arch walls, `meshes` is 0. The full scene build is deferred to the
/// render3d pane which re-renders when the revision changes.
fn run_view3d(s: &mut Session, _params: &Value) -> Result<Value, EngineError> {
    let doc = s.doc()?;
    let revision = doc.arch_revision;
    // Count meshes by building the scene (empty ArchModel yields 0).
    let meshes: usize = {
        let arch = &doc.arch;
        if arch.walls.is_empty() { 0 } else { cadcraft_arch3d::build(arch).meshes.len() }
    };
    Ok(json!({ "ok": true, "meshes": meshes, "revision": revision }))
}

/// Validate and echo the orbit camera.
///
/// Accepts optional `azimuth`, `elevation`, `distance` (radians / meters)
/// and an optional `target` (`[x,y,z]`). A present, non-null field must be
/// a finite number (`distance` in addition `> 0`), and `target` must be an
/// array of exactly three finite numbers; otherwise a `BadParams` error is
/// returned. Omitted or null fields take defaults (azimuth `0.0`, elevation
/// `0.35`, distance `10.0`, target the origin). Returns
/// `{"ok":true,"azimuth":..,"elevation":..,"distance":..,"target":[x,y,z]}`.
fn run_camera(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let present = |k: &str| params.get(k).is_some();
    if present("save") {
        return camera_save(s, params);
    }
    if present("use") {
        let name = name_param(params, "use")?;
        if s.doc()?.arch.camera_by_name(&name).is_none() {
            return Err(bad("camera", format!("no camera named `{name}`")));
        }
        let doc = s.doc_mut()?;
        doc.arch.active_camera = Some(name);
        doc.touch_arch();
        return cameras_json(s);
    }
    if present("delete") {
        let name = name_param(params, "delete")?;
        if s.doc()?.arch.camera_by_name(&name).is_none() {
            return Err(bad("camera", format!("no camera named `{name}`")));
        }
        let doc = s.doc_mut()?;
        doc.arch.remove_camera(&name);
        doc.touch_arch();
        return cameras_json(s);
    }
    if params.get("list").and_then(Value::as_bool) == Some(true) {
        return cameras_json(s);
    }
    let azimuth = match params.get("azimuth") {
        None | Some(Value::Null) => 0.0,
        Some(v) => v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("camera", "`azimuth` must be a finite number"))?,
    };
    let elevation = match params.get("elevation") {
        None | Some(Value::Null) => 0.35,
        Some(v) => v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("camera", "`elevation` must be a finite number"))?,
    };
    let distance = match params.get("distance") {
        None | Some(Value::Null) => 10.0,
        Some(v) => {
            let d = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("camera", "`distance` must be a finite number"))?;
            if d > 0.0 {
                d
            } else {
                return Err(bad("camera", "`distance` must be greater than 0"));
            }
        }
    };
    let target = match params.get("target") {
        None | Some(Value::Null) => (0.0, 0.0, 0.0),
        Some(v) => {
            let a = v.as_array().ok_or_else(|| bad("camera", "`target` must be an array of three numbers"))?;
            if a.len() != 3 {
                return Err(bad("camera", format!("`target` must have exactly 3 components (got {})", a.len())));
            }
            (
                a.get(0)
                    .and_then(Value::as_f64)
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| bad("camera", "`target` components must be finite numbers"))?,
                a.get(1)
                    .and_then(Value::as_f64)
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| bad("camera", "`target` components must be finite numbers"))?,
                a.get(2)
                    .and_then(Value::as_f64)
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| bad("camera", "`target` components must be finite numbers"))?,
            )
        }
    };

    // The command does not store the camera (the 3D pane owns it); it only
    // validates the parameters and echoes the result so scripts and the CLI
    // can query it.

    Ok(json!({
        "ok": true,
        "azimuth": azimuth,
        "elevation": elevation,
        "distance": distance,
        "target": [target.0, target.1, target.2],
    }))
}

/// A required string parameter (non-empty after trimming).
fn name_param(params: &Value, key: &str) -> Result<String, EngineError> {
    match params.get(key).and_then(Value::as_str) {
        Some(n) if !n.trim().is_empty() => Ok(n.to_string()),
        _ => Err(bad("camera", format!("`{key}` must be a non-empty camera name"))),
    }
}

/// An optional finite number.
fn opt_f64(params: &Value, key: &str) -> Result<Option<f64>, EngineError> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|n| n.is_finite()).map(Some).ok_or_else(|| bad("camera", format!("`{key}` must be a finite number"))),
    }
}

/// An optional array of exactly `N` finite numbers.
fn opt_vec<const N: usize>(params: &Value, key: &str) -> Result<Option<[f64; N]>, EngineError> {
    let Some(v) = params.get(key).filter(|v| !v.is_null()) else { return Ok(None) };
    let msg = || bad("camera", format!("`{key}` must be an array of {N} finite numbers"));
    let a = v.as_array().filter(|a| a.len() == N).ok_or_else(msg)?;
    let mut out = [0.0; N];
    for (o, x) in out.iter_mut().zip(a) {
        *o = x.as_f64().filter(|n| n.is_finite()).ok_or_else(msg)?;
    }
    Ok(Some(out))
}

/// Validate and store a named camera, and make it active.
fn camera_save(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let name = name_param(params, "save")?;
    let kind = match params.get("kind") {
        None | Some(Value::Null) => CameraKind::Orbit,
        Some(Value::String(k)) if k == "orbit" => CameraKind::Orbit,
        Some(Value::String(k)) if k == "full" => CameraKind::Full,
        Some(Value::String(k)) if k == "overview" => CameraKind::Overview,
        Some(v) => return Err(bad("camera", format!("`kind` must be \"orbit\", \"full\" or \"overview\", not {v}"))),
    };
    let mut cam = Camera3d { name, kind, ..Default::default() };
    // Reset optional fields to None to avoid inheriting invalid defaults (e.g. NaN)
    cam.target = None;
    cam.azimuth_deg = None;
    cam.elevation_deg = None;
    cam.distance = None;
    cam.fov_deg = None;
    cam.eye = None;
    cam.eye_height = None;
    cam.light_set = None;

    if let Some(t) = opt_vec::<3>(params, "target")? {
        cam.target = Some(t);
    }
    if let Some(a) = opt_f64(params, "azimuth")? {
        cam.azimuth_deg = Some(a);
    }
    if let Some(e) = opt_f64(params, "elevation")? {
        if !(-89.0..=89.0).contains(&e) {
            return Err(bad("camera", "`elevation` must be within [-89, 89] degrees"));
        }
        cam.elevation_deg = Some(e);
    }
    if let Some(d) = opt_f64(params, "distance")? {
        if !(d > 0.0 && d <= MAX_DISTANCE) {
            return Err(bad("camera", format!("`distance` must be in (0, {MAX_DISTANCE}]")));
        }
        cam.distance = Some(d);
    }
    let eye_param = params.get("eye").filter(|v| !v.is_null());
    let eye_3d: Option<[f64; 3]>;
    let eye_2d: Option<[f64; 2]>;

    if let Some(v) = eye_param {
        let a = v.as_array().ok_or_else(|| bad("camera", "`eye` must be an array"))?;
        if a.len() == 3 {
            let mut e = [0.0; 3];
            for (o, x) in e.iter_mut().zip(a) {
                *o = x.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("camera", "`eye` components must be finite numbers"))?;
            }
            eye_3d = Some(e);
            eye_2d = None;
        } else if a.len() == 2 {
            let mut e = [0.0; 2];
            for (o, x) in e.iter_mut().zip(a) {
                *o = x.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("camera", "`eye` components must be finite numbers"))?;
            }
            eye_2d = Some(e);
            eye_3d = None;
        } else {
            return Err(bad("camera", "`eye` must be an array of 2 or 3 finite numbers"));
        }
    } else {
        eye_3d = None;
        eye_2d = None;
    }

    let eye_height_param = opt_f64(params, "eyeHeight")?;

    if let Some(e) = eye_3d {
        if kind != CameraKind::Full {
            return Err(bad("camera", "`eye` with 3 components requires `kind`: \"full\""));
        }
        if eye_height_param.is_some() {
            return Err(bad("camera", "`eye` with 3 components cannot be combined with `eyeHeight`"));
        }
        cam.eye = Some([e[0], e[1]]);
        cam.eye_height = Some(e[2]);
    } else if let Some(e) = eye_2d {
        cam.eye = Some(e);
        if let Some(h) = eye_height_param {
            cam.eye_height = Some(h);
        }
    } else if let Some(h) = eye_height_param {
        cam.eye_height = Some(h);
    } else if kind != CameraKind::Full {
        // Ensure orbit cameras without explicit eye have no eye stored,
        // overriding any default from Camera3d::default().
        cam.eye = None;
        cam.eye_height = None;
    }

    if let Some(f) = opt_f64(params, "fov")? {
        if !(f > 0.0 && f < 180.0) {
            return Err(bad("camera", "`fov` must be within (0, 180) degrees"));
        }
        cam.fov_deg = Some(f);
    }
    if let Some(v) = params.get("level").filter(|v| !v.is_null()) {
        if kind != CameraKind::Overview {
            return Err(bad("camera", format!("`level` needs `kind`: \"overview\" (got {v})")));
        }
        cam.level = Some(level_param(&s.doc()?.arch, v)?);
    }
    if let Some(v) = params.get("lowerLevels").filter(|v| !v.is_null()) {
        if kind != CameraKind::Overview {
            return Err(bad("camera", format!("`lowerLevels` needs `kind`: \"overview\" (got {v})")));
        }
        cam.lower_levels = Some(v.as_bool().ok_or_else(|| bad("camera", format!("`lowerLevels` must be true or false, not {v}")))?);
    }
    if kind == CameraKind::Overview {
        // The overview pose is an orbit; the target and distance are fitted at render time when absent.
        cam.azimuth_deg.get_or_insert(30.0);
        cam.elevation_deg.get_or_insert(50.0);
    }
    if kind == CameraKind::Orbit {
        if cam.azimuth_deg.is_none() {
            return Err(bad("camera", "an `orbit` camera needs `azimuth`"));
        }
        if cam.elevation_deg.is_none() {
            cam.elevation_deg = Some(0.35);
        }
        if cam.distance.is_none() {
            cam.distance = Some(10.0);
        }
    }
    if kind == CameraKind::Full {
        if eye_3d.is_none() && eye_2d.is_none() {
            return Err(bad("camera", "a `full` camera needs `eye`"));
        }
        let e = cam.eye.ok_or_else(|| bad("camera", "a `full` camera needs `eye`"))?;
        let t = cam.target.unwrap_or([0.0; 3]);
        if e == [t[0], t[1]] {
            return Err(bad("camera", "`eye` must differ from `target`"));
        }
    }
    // The camera keeps its look: the effective global look (with the backdrop) under this call's
    // own look keys, stored complete so later global changes never reach it.
    let mut look = effective_look(&s.doc()?.arch, None);
    apply_look_params("camera", params, &s.doc()?.arch, &mut look, false)?;
    cam.light_set = look.light_set.clone();
    cam.look = Some(look);
    cam.saved = Some(Box::new(cam.clone()));
    let name = cam.name.clone();
    let doc = s.doc_mut()?;
    doc.arch.set_camera(cam).map_err(|e| bad("camera", format!("cannot save camera: {e}")))?;
    doc.arch.active_camera = Some(name);
    doc.touch_arch();
    cameras_json(s)
}

/// The pose keys `camera.update` reads.
const POSE_KEYS: &[&str] = &["target", "azimuth", "elevation", "distance", "eye", "eyeHeight", "fov", "level", "lowerLevels"];

/// A level named by name or index, as the `index` the levels carry.
fn level_param(arch: &cadcraft_doc::ArchModel, v: &Value) -> Result<u32, EngineError> {
    let by_pos = |i: u64| usize::try_from(i).ok().and_then(|i| arch.levels.get(i)).map(|l| l.index).or((i == 0 && arch.levels.is_empty()).then_some(0));
    let found = match v {
        Value::Number(n) => n.as_u64().and_then(by_pos),
        Value::String(n) => match n.trim().parse::<u64>() {
            Ok(i) => by_pos(i),
            Err(_) => arch.levels.iter().find(|l| l.name.eq_ignore_ascii_case(n.trim())).map(|l| l.index),
        },
        _ => None,
    };
    found.ok_or_else(|| bad("camera", format!("`level` must name an existing level (a name or index), not {v}")))
}

/// The camera a `camera.*` command acts on: `name`, else the active camera.
fn target_camera(cmd: &str, s: &Session, params: &Value) -> Result<String, EngineError> {
    let arch = &s.doc()?.arch;
    let name = match params.get("name") {
        None | Some(Value::Null) => arch.active_camera.clone().ok_or_else(|| bad(cmd, "no `name` given and no active camera"))?,
        Some(Value::String(n)) if !n.trim().is_empty() => n.trim().to_string(),
        Some(v) => return Err(bad(cmd, format!("`name` must be a camera name, not {v}"))),
    };
    if arch.camera_by_name(&name).is_none() {
        return Err(bad(cmd, format!("no camera named `{name}`")));
    }
    Ok(name)
}

/// A finite number the command was given, else a refusal naming the key and value.
fn num_key(cmd: &str, params: &Value, key: &str) -> Result<Option<f64>, EngineError> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|n| n.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite number, not {v}"))),
    }
}

/// Write `cam` back over the camera of the same name (undoable; the caller validated it).
fn store_camera(s: &mut Session, cam: Camera3d) -> Result<Value, EngineError> {
    let doc = s.doc_mut()?;
    let Some(slot) = doc.arch.cameras.iter_mut().find(|c| c.name == cam.name) else {
        return Err(bad("camera", format!("no camera named `{}`", cam.name)));
    };
    *slot = cam;
    doc.touch_arch();
    cameras_json(s)
}

/// `camera.reset`: the live camera returns to its saved pose and look.
fn run_camera_reset(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let name = target_camera("camera.reset", s, params)?;
    let Some(cam) = s.doc()?.arch.camera_by_name(&name).cloned() else {
        return Err(bad("camera.reset", format!("no camera named `{name}`")));
    };
    let Some(saved) = cam.saved.clone() else {
        return Err(bad("camera.reset", format!("camera `{name}` has no saved state")));
    };
    let mut live = *saved;
    live.name = cam.name.clone();
    live.saved = cam.saved;
    store_camera(s, live)
}

/// `camera.orbit`: change the live pose by deltas.
fn run_camera_orbit(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const CMD: &str = "camera.orbit";
    let name = target_camera(CMD, s, params)?;
    let (daz, del) = (num_key(CMD, params, "azimuth")?, num_key(CMD, params, "elevation")?);
    let factor = match num_key(CMD, params, "distance")? {
        Some(f) if f <= 0.0 => return Err(bad(CMD, format!("`distance` must be a factor greater than 0, not {f}"))),
        f => f,
    };
    if daz.is_none() && del.is_none() && factor.is_none() {
        return Err(bad(CMD, "nothing to change: give `azimuth`, `elevation` or `distance`"));
    }
    let Some(mut cam) = s.doc()?.arch.camera_by_name(&name).cloned() else {
        return Err(bad(CMD, format!("no camera named `{name}`")));
    };
    let scaled = |d: f64, f: f64| -> Result<f64, EngineError> {
        let n = d * f;
        if n.is_finite() && n > 0.0 && n <= MAX_DISTANCE { Ok(n) } else { Err(bad(CMD, format!("`distance` factor {f} takes the distance outside (0, {MAX_DISTANCE}]"))) }
    };
    let (daz, del) = (daz.unwrap_or(0.0), del.unwrap_or(0.0));
    if cam.kind == CameraKind::Full {
        // Rotate the eye about the target around +Z and pitch it; scale its distance.
        let t = cam.target.unwrap_or([0.0; 3]);
        let e = cam.eye.unwrap_or([0.0, -10.0]);
        let v = [e[0] - t[0], e[1] - t[1], cam.eye_height.unwrap_or(1.6) - t[2]];
        let r = v[0].hypot(v[1]);
        let dist = (r * r + v[2] * v[2]).sqrt();
        if dist <= 0.0 {
            return Err(bad(CMD, format!("camera `{name}` has its eye on its target")));
        }
        let yaw = v[1].atan2(v[0]) + daz.to_radians();
        let pitch = (v[2].atan2(r).to_degrees() + del).clamp(-89.0, 89.0).to_radians();
        let dist = match factor {
            Some(f) => scaled(dist, f)?,
            None => dist,
        };
        let (cp, sp) = (pitch.cos(), pitch.sin());
        let eye = [t[0] + dist * cp * yaw.cos(), t[1] + dist * cp * yaw.sin(), t[2] + dist * sp];
        if !eye.iter().all(|x| x.is_finite()) {
            return Err(bad(CMD, "the new eye is not finite"));
        }
        cam.eye = Some([eye[0], eye[1]]);
        cam.eye_height = Some(eye[2]);
    } else {
        let az = cam.azimuth_deg.unwrap_or(if cam.kind == CameraKind::Overview { 30.0 } else { 0.0 }) + daz;
        let el = cam.elevation_deg.unwrap_or(if cam.kind == CameraKind::Overview { 50.0 } else { 30.0 }) + del;
        if !az.is_finite() || !el.is_finite() {
            return Err(bad(CMD, "the new angles are not finite"));
        }
        cam.azimuth_deg = Some(az);
        cam.elevation_deg = Some(el.clamp(-89.0, 89.0));
        if let Some(f) = factor {
            let Some(d) = cam.distance else {
                return Err(bad(CMD, format!("camera `{name}` has no distance yet (an overview fits it at render time): set one with camera.update")));
            };
            cam.distance = Some(scaled(d, f)?);
        }
    }
    store_camera(s, cam)
}

/// `camera.update`: apply pose and look keys to the live camera, then save the live camera.
fn run_camera_update(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const CMD: &str = "camera.update";
    let name = target_camera(CMD, s, params)?;
    if !POSE_KEYS.iter().chain(LOOK_KEYS).any(|k| params.get(*k).is_some()) {
        return Err(bad(CMD, "nothing to update: give pose keys (target, azimuth, elevation, distance, eye, eyeHeight, fov, level, lowerLevels) or look keys"));
    }
    let doc = s.doc()?;
    let Some(mut cam) = doc.arch.camera_by_name(&name).cloned() else {
        return Err(bad(CMD, format!("no camera named `{name}`")));
    };
    if let Some(t) = opt_vec::<3>(params, "target")? {
        cam.target = Some(t);
    }
    if let Some(a) = num_key(CMD, params, "azimuth")? {
        cam.azimuth_deg = Some(a);
    }
    if let Some(e) = num_key(CMD, params, "elevation")? {
        if !(-89.0..=89.0).contains(&e) {
            return Err(bad(CMD, format!("`elevation` must be within [-89, 89] degrees, not {e}")));
        }
        cam.elevation_deg = Some(e);
    }
    if let Some(d) = num_key(CMD, params, "distance")? {
        if !(d > 0.0 && d <= MAX_DISTANCE) {
            return Err(bad(CMD, format!("`distance` must be in (0, {MAX_DISTANCE}], not {d}")));
        }
        cam.distance = Some(d);
    }
    if let Some(v) = params.get("eye").filter(|v| !v.is_null()) {
        let nums: Option<Vec<f64>> = v.as_array().and_then(|a| a.iter().map(|x| x.as_f64().filter(|n| n.is_finite())).collect());
        match nums.as_deref() {
            Some([x, y]) => cam.eye = Some([*x, *y]),
            Some([x, y, z]) if cam.kind == CameraKind::Full => {
                cam.eye = Some([*x, *y]);
                cam.eye_height = Some(*z);
            }
            _ => return Err(bad(CMD, format!("`eye` must be [x, y] (or [x, y, z] on a full camera), not {v}"))),
        }
    }
    if let Some(h) = num_key(CMD, params, "eyeHeight")? {
        cam.eye_height = Some(h);
    }
    if let Some(f) = num_key(CMD, params, "fov")? {
        if !(f > 0.0 && f < 180.0) {
            return Err(bad(CMD, format!("`fov` must be within (0, 180) degrees, not {f}")));
        }
        cam.fov_deg = Some(f);
    }
    if let Some(v) = params.get("level").filter(|v| !v.is_null()) {
        if cam.kind != CameraKind::Overview {
            return Err(bad(CMD, format!("`level` needs an overview camera (got {v})")));
        }
        cam.level = Some(level_param(&doc.arch, v)?);
    }
    if let Some(v) = params.get("lowerLevels").filter(|v| !v.is_null()) {
        if cam.kind != CameraKind::Overview {
            return Err(bad(CMD, format!("`lowerLevels` needs an overview camera (got {v})")));
        }
        cam.lower_levels = Some(v.as_bool().ok_or_else(|| bad(CMD, format!("`lowerLevels` must be true or false, not {v}")))?);
    }
    if cam.kind == CameraKind::Full {
        let (e, t) = (cam.eye.unwrap_or([0.0; 2]), cam.target.unwrap_or([0.0; 3]));
        if e == [t[0], t[1]] {
            return Err(bad(CMD, "`eye` must differ from `target`"));
        }
    }
    let mut look = effective_look(&doc.arch, Some(&cam));
    apply_look_params(CMD, params, &doc.arch, &mut look, false)?;
    cam.light_set = look.light_set.clone();
    cam.look = Some(look);
    cam.saved = None;
    cam.saved = Some(Box::new(cam.clone()));
    store_camera(s, cam)
}

/// `{"ok":true,"cameras":[…],"active":name|null}` for the current drawing.
fn cameras_json(s: &Session) -> Result<Value, EngineError> {
    let arch = &s.doc()?.arch;
    let cams: Vec<Value> = arch
        .cameras
        .iter()
        .map(|c| {
            let kind = match c.kind {
                CameraKind::Full => "full",
                CameraKind::Overview => "overview",
                _ => "orbit",
            };
            let eye = if c.kind == CameraKind::Full {
                match (c.eye, c.eye_height) {
                    (Some([x, y]), Some(z)) => json!([x, y, z]),
                    (Some([x, y]), None) => json!([x, y, 0.0]),
                    _ => json!(null),
                }
            } else {
                json!(c.eye)
            };
            json!({
                "name": c.name,
                "kind": kind,
                "target": c.target.unwrap_or([0.0, 0.0, 0.0]),
                "azimuth": c.azimuth_deg,
                "elevation": c.elevation_deg,
                "distance": c.distance,
                "eye": eye,
                "eyeHeight": c.eye_height,
                "fov": c.fov_deg,
                "lightSet": c.light_set,
                "level": c.level,
                "lowerLevels": c.lower_levels,
            })
        })
        .collect();
    Ok(json!({ "ok": true, "cameras": cams, "active": arch.active_camera }))
}

/// A finite number with magnitude below [`MAX_WALK`]; `default` when absent.
fn walk_num(params: &Value, key: &str, default: Option<f64>) -> Result<f64, EngineError> {
    let v = match params.get(key) {
        None | Some(Value::Null) => default.ok_or_else(|| bad("walkthrough", format!("`{key}` is required")))?,
        Some(v) => v.as_f64().ok_or_else(|| bad("walkthrough", format!("`{key}` must be a number")))?,
    };
    if !v.is_finite() || v.abs() >= MAX_WALK {
        return Err(bad("walkthrough", format!("`{key}` must be finite with magnitude below {MAX_WALK}")));
    }
    Ok(v)
}

/// Pure walkthrough calculator: move `forward`/`strafe` metres (times `step`)
/// from `eye` facing `yaw` (degrees, 0 = +Y, increasing toward +X), apply
/// `turn`, and return the new `eye` and normalised `yaw` in (-180, 180].
/// Never reads or writes the drawing.
fn run_walkthrough(_s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let eye = match params.get("eye").and_then(Value::as_array) {
        Some(a) if a.len() == 3 => {
            let mut e = [0.0; 3];
            for (o, x) in e.iter_mut().zip(a) {
                *o = x
                    .as_f64()
                    .filter(|n| n.is_finite() && n.abs() < MAX_WALK)
                    .ok_or_else(|| bad("walkthrough", "`eye` components must be finite numbers below 1e6"))?;
            }
            e
        }
        _ => return Err(bad("walkthrough", "`eye` must be an array of three numbers")),
    };
    let yaw = walk_num(params, "yaw", None)?;
    let forward = walk_num(params, "forward", Some(0.0))?;
    let strafe = walk_num(params, "strafe", Some(0.0))?;
    let turn = walk_num(params, "turn", Some(0.0))?;
    let step = walk_num(params, "step", Some(1.0))?;
    if !(step > 0.0 && step <= 100.0) {
        return Err(bad("walkthrough", "`step` must be in (0, 100]"));
    }
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (f, r) = (forward * step, strafe * step);
    let x = eye[0] + f * sy + r * cy;
    let y = eye[1] + f * cy - r * sy;
    let mut n = (yaw + turn) % 360.0;
    if n > 180.0 {
        n -= 360.0;
    } else if n <= -180.0 {
        n += 360.0;
    }
    Ok(json!({ "ok": true, "eye": [x, y, eye[2]], "yaw": n }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EngineError, Session};

    fn cam_count(s: &Session) -> usize {
        s.doc().unwrap().arch.cameras.len()
    }

    #[test]
    fn camera_save_list_use_and_delete() {
        let mut s = Session::new();
        let r = s.execute("camera", &json!({ "save": "front", "azimuth": 10.0, "distance": 20.0 })).unwrap();
        assert_eq!(r["active"], json!("front"));
        assert_eq!(r["cameras"][0]["distance"], json!(20.0));
        s.execute("camera", &json!({ "save": "plan", "kind": "full", "eye": [5.0, 5.0], "fov": 60.0 })).unwrap();
        let l = s.execute("camera", &json!({ "list": true })).unwrap();
        assert_eq!(l["cameras"].as_array().unwrap().len(), 2);
        assert_eq!(l["cameras"][1]["kind"], json!("full"));
        assert_eq!(l["active"], json!("plan"));
        let u = s.execute("camera", &json!({ "use": "front" })).unwrap();
        assert_eq!(u["active"], json!("front"));
        assert!(s.execute("camera", &json!({ "use": "nope" })).is_err());
        let d = s.execute("camera", &json!({ "delete": "front" })).unwrap();
        assert_eq!(d["active"], json!(null));
        assert_eq!(cam_count(&s), 1);
        assert!(s.execute("camera", &json!({ "delete": "front" })).is_err());
        s.execute("u", &json!({})).unwrap();
        assert_eq!(cam_count(&s), 2);
    }

    #[test]
    fn camera_list_does_not_mutate_the_document() {
        let mut s = Session::new();
        s.execute("camera", &json!({ "save": "a", "azimuth": 0.0 })).unwrap();
        let rev = s.doc().unwrap().arch_revision;
        let cams = s.doc().unwrap().arch.cameras.clone();
        s.execute("camera", &json!({ "list": true })).unwrap();
        assert_eq!(s.doc().unwrap().arch_revision, rev);
        assert_eq!(s.doc().unwrap().arch.cameras, cams);
    }

    #[test]
    fn a_cameras_light_set_applies_to_its_renders() {
        let mut s = Session::new();
        let mut doc = s.doc_mut().unwrap();
        doc.arch.lights.push(cadcraft_doc::arch::LightFixture::new(
            cadcraft_doc::arch::ArchId(0),
            "Recessed can",
            cadcraft_geom::Vec2::new(0.0, 0.0),
        ));
        doc.touch_arch();
        s.execute("lightset", &json!({ "name": "night", "lights": { "lf0": { "on": false, "brightness": 0.25 } } })).unwrap();
        s.execute("camera", &json!({ "save": "front", "azimuth": 0.0, "lightSet": "night" })).unwrap();
        let camera = s.doc().unwrap().arch.camera_by_name("front").unwrap();
        assert_eq!(camera.light_set.as_deref(), Some("night"));
        let set = s.doc().unwrap().arch.light_sets.iter().find(|set| set.name == "night").unwrap();
        assert!(!set.lights["lf0"].on);
    }

    #[test]
    fn camera_save_refuses_duplicate_and_non_finite() {
        let mut s = Session::new();
        s.execute("camera", &json!({ "save": "a", "azimuth": 0.0 })).unwrap();
        for p in [
            json!({ "save": "a" }),
            json!({ "save": "  " }),
            json!({ "save": "b", "distance": 0.0 }),
            json!({ "save": "b", "distance": 1e7 }),
            json!({ "save": "b", "elevation": 90.0 }),
            json!({ "save": "b", "fov": 180.0 }),
            json!({ "save": "b", "fov": 0.0 }),
            json!({ "save": "b", "azimuth": "x" }),
            json!({ "save": "b", "target": [1.0, "nan", 0.0] }),
            json!({ "save": "b", "kind": "full" }),
            json!({ "save": "b", "kind": "full", "eye": [0.0, 0.0] }),
            json!({ "save": "b", "kind": "weird" }),
        ] {
            let r = s.execute("camera", &p);
            assert!(matches!(r, Err(EngineError::BadParams { .. })), "{p:?} -> {r:?}");
        }
        assert_eq!(cam_count(&s), 1);
    }

    #[test]
    fn bare_camera_still_echoes_the_orbit() {
        let mut s = Session::new();
        let r = s.execute("camera", &json!({ "azimuth": 2.0 })).unwrap();
        assert_eq!(r["azimuth"], json!(2.0));
        assert_eq!(r["distance"], json!(10.0));
        assert_eq!(cam_count(&s), 0);
    }

    #[test]
    fn walkthrough_forward_and_strafe_are_orthogonal() {
        let mut s = Session::new();
        let r = s.execute("walkthrough", &json!({ "eye": [1.0, 2.0, 1.6], "yaw": 0.0, "forward": 2.0 })).unwrap();
        assert!((r["eye"][0].as_f64().unwrap() - 1.0).abs() < 1e-9);
        assert!((r["eye"][1].as_f64().unwrap() - 4.0).abs() < 1e-9);
        assert_eq!(r["eye"][2], json!(1.6));
        let r = s.execute("walkthrough", &json!({ "eye": [0.0, 0.0, 0.0], "yaw": 0.0, "strafe": 3.0 })).unwrap();
        assert!((r["eye"][0].as_f64().unwrap() - 3.0).abs() < 1e-9);
        assert!(r["eye"][1].as_f64().unwrap().abs() < 1e-9);
        let r = s.execute("walkthrough", &json!({ "eye": [0.0, 0.0, 0.0], "yaw": 90.0, "forward": 1.0, "step": 2.0 })).unwrap();
        assert!((r["eye"][0].as_f64().unwrap() - 2.0).abs() < 1e-9);
        assert!(r["eye"][1].as_f64().unwrap().abs() < 1e-9);
    }

    #[test]
    fn walkthrough_turn_adds_to_yaw_and_normalises() {
        let mut s = Session::new();
        let r = s.execute("walkthrough", &json!({ "eye": [0, 0, 0], "yaw": 170.0, "turn": 20.0 })).unwrap();
        assert!((r["yaw"].as_f64().unwrap() + 170.0).abs() < 1e-9);
        let r = s.execute("walkthrough", &json!({ "eye": [0, 0, 0], "yaw": -170.0, "turn": -10.0 })).unwrap();
        assert!((r["yaw"].as_f64().unwrap() - 180.0).abs() < 1e-9);
    }

    #[test]
    fn walkthrough_refuses_huge_steps() {
        let mut s = Session::new();
        for p in [
            json!({ "eye": [0, 0, 0], "yaw": 0.0, "forward": 1e6 }),
            json!({ "eye": [0, 0, 0], "yaw": 0.0, "strafe": -1e9 }),
            json!({ "eye": [0, 0, 0], "yaw": 0.0, "step": 101.0 }),
            json!({ "eye": [0, 0, 0], "yaw": 0.0, "step": 0.0 }),
            json!({ "eye": [1e6, 0, 0], "yaw": 0.0 }),
            json!({ "eye": [0, 0], "yaw": 0.0 }),
            json!({ "eye": [0, 0, 0] }),
            json!({ "eye": [0, 0, 0], "yaw": 1e6 }),
        ] {
            let r = s.execute("walkthrough", &p);
            assert!(matches!(r, Err(EngineError::BadParams { .. })), "{p:?} -> {r:?}");
        }
    }

    /// `view3d` on a fresh drawing (no walls) reports `ok` with zero meshes
    /// and revision 0.
    #[test]
    fn view3d_command_reports_the_scene() {
        let mut s = Session::new();
        let r = s.execute("view3d", &json!({})).unwrap();
        assert!(r["ok"].as_bool().unwrap());
        assert_eq!(r["meshes"].as_u64().unwrap(), 0);
        assert_eq!(r["revision"].as_u64().unwrap(), 0);
    }

    /// `camera` with explicit parameters echoes them; omitted fields fall
    /// back to the defaults.
    #[test]
    fn camera_command_echoes_the_orbit() {
        let mut s = Session::new();
        let r = s.execute("camera", &json!({ "azimuth": 1.5, "elevation": 0.3, "distance": 25.0 })).unwrap();
        assert!(r["ok"].as_bool().unwrap());
        assert_eq!(r["azimuth"].as_f64().unwrap(), 1.5);
        assert_eq!(r["elevation"].as_f64().unwrap(), 0.3);
        assert_eq!(r["distance"].as_f64().unwrap(), 25.0);

        // Defaults when no parameters are given.
        let r2 = s.execute("camera", &json!({})).unwrap();
        assert_eq!(r2["azimuth"].as_f64().unwrap(), 0.0);
        assert_eq!(r2["elevation"].as_f64().unwrap(), 0.35);
        assert_eq!(r2["distance"].as_f64().unwrap(), 10.0);
    }

    /// The command does not mutate any stored camera state (the 3D pane owns
    /// the orbit); it validates the request and echoes the effective orbit
    /// back to the caller. Supplied fields are returned verbatim; omitted or
    /// null fields fall back to the defaults (azimuth `0.0`, elevation `0.35`,
    /// distance `10.0`, target the origin).
    #[test]
    fn camera_command_updates_the_orbit() {
        let mut s = Session::new();

        // Supplied fields are echoed verbatim, including a three-component target.
        let r = s.execute("camera", &json!({ "azimuth": 0.9, "elevation": 0.5, "distance": 5.0, "target": [1.0, 2.0, 3.0] })).unwrap();
        assert!(r["ok"].as_bool().unwrap());
        assert_eq!(r["azimuth"].as_f64().unwrap(), 0.9);
        assert_eq!(r["elevation"].as_f64().unwrap(), 0.5);
        assert_eq!(r["distance"].as_f64().unwrap(), 5.0);
        assert_eq!(r["target"], json!([1.0, 2.0, 3.0]));

        // Null fields fall back to the defaults too.
        let r2 = s.execute("camera", &json!({ "distance": null })).unwrap();
        assert_eq!(r2["azimuth"].as_f64().unwrap(), 0.0);
        assert_eq!(r2["elevation"].as_f64().unwrap(), 0.35);
        assert_eq!(r2["distance"].as_f64().unwrap(), 10.0);
        assert_eq!(r2["target"], json!([0.0, 0.0, 0.0]));
    }

    /// `camera` rejects unusable values instead of defaulting: a non-positive or
    /// non-numeric `distance`, a non-numeric `azimuth`/`elevation`, and a
    /// `target` that is not an array of exactly three finite numbers each
    /// return `EngineError::BadParams`. A valid three-component `target` is
    /// echoed back.
    #[test]
    fn camera_command_rejects_unusable_values() {
        let mut s = Session::new();

        for p in [
            json!({ "distance": -5.0 }),
            json!({ "distance": 0.0 }),
            json!({ "azimuth": "east" }),
            json!({ "elevation": [1] }),
            json!({ "target": [1, 2] }),
            json!({ "target": [1, 2, "z"] }),
        ] {
            let r = s.execute("camera", &p);
            assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })), "camera({p:?}) should be rejected, got {r:?}");
        }

        let r = s.execute("camera", &json!({ "target": [1.0, 2.0, 3.0] })).unwrap();
        assert!(r["ok"].as_bool().unwrap());
        assert_eq!(r["target"], json!([1.0, 2.0, 3.0]));
    }

    /// Empty or oversized target arrays and non-numeric scalars (booleans,
    /// objects, string numbers) are rejected for every field.
    #[test]
    fn camera_command_rejects_malformed_targets_and_scalars() {
        let mut s = Session::new();

        for p in [
            json!({ "target": [] }),
            json!({ "target": [1.0, 2.0, 3.0, 4.0] }),
            json!({ "azimuth": true }),
            json!({ "elevation": false }),
            json!({ "distance": {} }),
            json!({ "azimuth": "1.5" }),
        ] {
            let r = s.execute("camera", &p);
            assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })), "camera({p:?}) should be rejected, got {r:?}");
        }
    }

    /// Zero is valid for azimuth and elevation (but not distance); very large
    /// finite values are accepted and echoed back.
    #[test]
    fn camera_command_accepts_zero_and_large_values() {
        let mut s = Session::new();

        let r = s.execute("camera", &json!({ "azimuth": 0.0, "elevation": 0.0, "distance": 5.0 })).unwrap();
        assert!(r["ok"].as_bool().unwrap());
        assert_eq!(r["azimuth"].as_f64().unwrap(), 0.0);
        assert_eq!(r["elevation"].as_f64().unwrap(), 0.0);

        let r2 = s.execute("camera", &json!({ "azimuth": 1e308, "elevation": 1e308, "distance": 1e308 })).unwrap();
        assert!(r2["ok"].as_bool().unwrap());
        assert_eq!(r2["azimuth"].as_f64().unwrap(), 1e308);
        assert_eq!(r2["distance"].as_f64().unwrap(), 1e308);
    }

    /// When any field is bad the whole call is rejected, even if the other
    /// fields are valid; a fully valid call with all four fields succeeds.
    #[test]
    fn camera_command_rejects_when_any_field_is_bad() {
        let mut s = Session::new();

        let r = s.execute("camera", &json!({ "azimuth": 0.5, "distance": -1.0 }));
        assert!(matches!(r, Err(EngineError::BadParams { cmd: _, msg: _ })));

        let r2 = s.execute("camera", &json!({ "distance": 5.0, "target": [1.0] }));
        assert!(matches!(r2, Err(EngineError::BadParams { cmd: _, msg: _ })));

        let r3 = s.execute("camera", &json!({ "azimuth": 0.5, "elevation": 0.3, "distance": 5.0, "target": [1.0, 2.0, 3.0] })).unwrap();
        assert!(r3["ok"].as_bool().unwrap());
        assert_eq!(r3["azimuth"].as_f64().unwrap(), 0.5);
        assert_eq!(r3["elevation"].as_f64().unwrap(), 0.3);
        assert_eq!(r3["distance"].as_f64().unwrap(), 5.0);
        assert_eq!(r3["target"], json!([1.0, 2.0, 3.0]));
    }

    #[test]
    fn camera_save_full_accepts_a_3d_eye() {
        let mut s = Session::new();
        let r = s
            .execute(
                "camera",
                &json!({
                    "save": "front",
                    "kind": "full",
                    "eye": [3.0, -8.0, 1.6],
                    "target": [3.0, 0.0, 1.05],
                    "fov": 60.0
                }),
            )
            .unwrap();
        assert_eq!(r["active"], json!("front"));
        assert_eq!(r["cameras"][0]["kind"], json!("full"));
        assert_eq!(r["cameras"][0]["eye"], json!([3.0, -8.0, 1.6]));
        assert_eq!(r["cameras"][0]["eyeHeight"], json!(1.6));
        assert_eq!(r["cameras"][0]["target"], json!([3.0, 0.0, 1.05]));
        assert_eq!(r["cameras"][0]["fov"], json!(60.0));
    }

    #[test]
    fn camera_save_full_still_accepts_eye_xy_with_eye_height() {
        let mut s = Session::new();
        let r = s
            .execute(
                "camera",
                &json!({
                    "save": "front",
                    "kind": "full",
                    "eye": [3.0, -8.0],
                    "eyeHeight": 1.6,
                    "target": [3.0, 0.0, 1.05],
                    "fov": 60.0
                }),
            )
            .unwrap();
        assert_eq!(r["active"], json!("front"));
        assert_eq!(r["cameras"][0]["kind"], json!("full"));
        assert_eq!(r["cameras"][0]["eye"], json!([3.0, -8.0, 1.6]));
        assert_eq!(r["cameras"][0]["eyeHeight"], json!(1.6));
    }

    #[test]
    fn camera_reply_reports_a_full_eye_with_its_height() {
        let mut s = Session::new();
        s.execute(
            "camera",
            &json!({
                "save": "front",
                "kind": "full",
                "eye": [3.0, -8.0, 1.6],
                "target": [3.0, 0.0, 1.05]
            }),
        )
        .unwrap();
        s.execute(
            "camera",
            &json!({
                "save": "overview",
                "kind": "orbit",
                "azimuth": 0.0,
                "distance": 20.0
            }),
        )
        .unwrap();

        let l = s.execute("camera", &json!({ "list": true })).unwrap();
        let cams = l["cameras"].as_array().unwrap();
        assert_eq!(cams.len(), 2);

        // Find front
        let front = cams.iter().find(|c| c["name"] == json!("front")).unwrap();
        assert_eq!(front["kind"], json!("full"));
        assert_eq!(front["eye"], json!([3.0, -8.0, 1.6]));
        assert_eq!(front["eyeHeight"], json!(1.6));

        // Find overview
        let overview = cams.iter().find(|c| c["name"] == json!("overview")).unwrap();
        assert_eq!(overview["kind"], json!("orbit"));
        assert!(overview["eye"].is_null());
    }

    #[test]
    fn camera_save_full_refuses_a_3d_eye_with_eye_height() {
        let mut s = Session::new();
        let r = s.execute(
            "camera",
            &json!({
                "save": "front",
                "kind": "full",
                "eye": [3.0, -8.0, 1.6],
                "eyeHeight": 2.0
            }),
        );
        assert!(matches!(r, Err(EngineError::BadParams { .. })));
        assert_eq!(cam_count(&s), 0);
    }

    /// A 4-component eye is rejected for both full and orbit cameras.
    #[test]
    fn camera_save_full_refuses_4_component_eye() {
        let mut s = Session::new();
        for p in [
            json!({ "save": "a", "kind": "full", "eye": [1.0, 2.0, 3.0, 4.0] }),
            json!({ "save": "b", "kind": "orbit", "eye": [1.0, 2.0, 3.0, 4.0] }),
        ] {
            let r = s.execute("camera", &p);
            assert!(matches!(r, Err(EngineError::BadParams { .. })), "{p:?} -> {r:?}");
        }
        assert_eq!(cam_count(&s), 0);
    }

    /// Null (or NaN) in any eye component is rejected.
    #[test]
    fn camera_save_full_refuses_null_in_eye() {
        let mut s = Session::new();
        for p in [
            json!({ "save": "a", "kind": "full", "eye": [null, 2.0, 3.0] }),
            json!({ "save": "b", "kind": "full", "eye": [1.0, null, 3.0] }),
            json!({ "save": "c", "kind": "full", "eye": [1.0, 2.0, null] }),
            json!({ "save": "d", "kind": "full", "eye": [null, 2.0] }),
        ] {
            let r = s.execute("camera", &p);
            assert!(matches!(r, Err(EngineError::BadParams { .. })), "{p:?} -> {r:?}");
        }
        assert_eq!(cam_count(&s), 0);
    }

    /// An orbit camera with a 3-component eye is rejected.
    #[test]
    fn camera_save_full_refuses_orbit_with_3d_eye() {
        let mut s = Session::new();
        let r = s.execute("camera", &json!({ "save": "a", "kind": "orbit", "eye": [1.0, 2.0, 3.0] }));
        assert!(matches!(r, Err(EngineError::BadParams { .. })), "orbit with 3D eye should be rejected");
        assert_eq!(cam_count(&s), 0);
    }

    /// A full camera without a target defaults to the origin.
    #[test]
    fn camera_save_full_without_target_defaults_to_origin() {
        let mut s = Session::new();
        let r = s.execute("camera", &json!({ "save": "a", "kind": "full", "eye": [3.0, -8.0, 1.6] })).unwrap();
        assert_eq!(r["active"], json!("a"));
        assert_eq!(r["cameras"][0]["target"], json!([0.0, 0.0, 0.0]));
        assert_eq!(r["cameras"][0]["eye"], json!([3.0, -8.0, 1.6]));
    }

    /// Negative and very large finite eye values are accepted.
    #[test]
    fn camera_save_full_accepts_negative_and_large_eye() {
        let mut s = Session::new();
        let r = s.execute("camera", &json!({ "save": "neg", "kind": "full", "eye": [-100.0, -200.0, -50.0] })).unwrap();
        assert_eq!(r["cameras"][0]["eye"], json!([-100.0, -200.0, -50.0]));

        let r2 = s.execute("camera", &json!({ "save": "big", "kind": "full", "eye": [1e308, 1e308, 1e308] })).unwrap();
        assert_eq!(r2["cameras"][1]["eye"], json!([1e308, 1e308, 1e308]));
        assert_eq!(cam_count(&s), 2);
    }

    /// Saving a camera is an undoable drawing mutation.
    #[test]
    fn camera_save_can_be_undone() {
        let mut s = Session::new();
        s.execute("camera", &json!({ "save": "a", "azimuth": 0.0, "distance": 5.0 })).unwrap();
        assert_eq!(cam_count(&s), 1);
        s.execute("u", &json!({})).unwrap();
        assert_eq!(cam_count(&s), 0);
    }

    /// The `step` parameter scales both forward and strafe movement.
    #[test]
    fn walkthrough_step_scales_movement() {
        let mut s = Session::new();
        // step=2 doubles the forward movement (yaw 0 = +Y)
        let r = s.execute("walkthrough", &json!({ "eye": [0.0, 0.0, 0.0], "yaw": 0.0, "forward": 1.0, "step": 2.0 })).unwrap();
        assert!((r["eye"][1].as_f64().unwrap() - 2.0).abs() < 1e-9);
        // step=0.5 halves the strafe (yaw 0, strafe = +X)
        let r2 = s.execute("walkthrough", &json!({ "eye": [0.0, 0.0, 0.0], "yaw": 0.0, "strafe": 1.0, "step": 0.5 })).unwrap();
        assert!((r2["eye"][0].as_f64().unwrap() - 0.5).abs() < 1e-9);
    }

    /// An orbit camera may store an explicit 2D eye position.
    #[test]
    fn orbit_camera_can_have_explicit_eye() {
        let mut s = Session::new();
        let r = s.execute("camera", &json!({ "save": "a", "kind": "orbit", "azimuth": 0.0, "eye": [1.0, 2.0], "distance": 5.0 })).unwrap();
        assert_eq!(r["active"], json!("a"));
        assert_eq!(r["cameras"][0]["eye"], json!([1.0, 2.0]));
        assert_eq!(r["cameras"][0]["kind"], json!("orbit"));
    }

    /// Cameras appear in the list in the order they were saved.
    #[test]
    fn camera_list_preserves_save_order() {
        let mut s = Session::new();
        s.execute("camera", &json!({ "save": "first", "azimuth": 0.0 })).unwrap();
        s.execute("camera", &json!({ "save": "second", "azimuth": 0.0 })).unwrap();
        s.execute("camera", &json!({ "save": "third", "azimuth": 0.0 })).unwrap();
        let l = s.execute("camera", &json!({ "list": true })).unwrap();
        let names: Vec<String> = l["cameras"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap().to_string()).collect();
        assert_eq!(names, vec!["first", "second", "third"]);
    }

    /// `use` changes the active camera and is itself an undo step.
    #[test]
    fn camera_use_sets_active_and_is_undone() {
        let mut s = Session::new();
        s.execute("camera", &json!({ "save": "a", "azimuth": 0.0 })).unwrap();
        s.execute("camera", &json!({ "save": "b", "azimuth": 0.0 })).unwrap();
        // active is now "b"
        let u = s.execute("camera", &json!({ "use": "a" })).unwrap();
        assert_eq!(u["active"], json!("a"));
        s.execute("u", &json!({})).unwrap();
        let l = s.execute("camera", &json!({ "list": true })).unwrap();
        assert_eq!(l["active"], json!("b"));
    }
}
