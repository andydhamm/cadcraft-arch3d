//! `sunstudy`: a series of stills from the current camera with the site's sun (phase 10),
//! rendered through the app's render hook. The engine has no GPU; the app (CLI / desktop)
//! installs a renderer via [`render::set_render_hooks`], which is what the hook calls.
//!
//! One still per time `from … to` inclusive by `step`, at or above the horizon only (below
//! horizon is listed but not rendered). Times are solar times as `sun` takes them.

use std::path::PathBuf;

use cadcraft_arch::site::true_to_drawing;
use cadcraft_arch::sun;
use cadcraft_arch3d::camera::Camera;
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::{EngineError, Session};

use super::pick::parse_camera;
use super::render::{RenderRequest, SunChoice, render_hooks};

/// Largest accepted still size per edge, in pixels (matches the `render`/`pick` caps).
const MAX_SIZE: u64 = 8192;
/// At most this many stills in one study; more is refused.
const MAX_STILLS: usize = 96;
/// Minimum accepted step, in minutes; anything smaller is refused.
const MIN_STEP_MIN: i64 = 5;

/// The `sunstudy` command spec.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("sunstudy", "Sun Study", run)
            .menu(&["Architecture", "Sun Study"])
            .params("{date, from: \"08:00\", to: \"18:00\", step: \"1h\", out: \"dir/prefix\", size?, camera?, final?: bool, ground?: bool}")
            .noundo(),
    ]
}

/// Inclusive list of solar times from `from` to `to` by `step`.
///
/// `from`/`to` are `HH:MM` strings; `step` is `<n>h` or `<n>min` with n whole and the
/// total step at least 5 minutes. Times are capped at `MAX_STILLS`.
///
/// Returns an error string on malformed input, `from > to`, or too many stills.
pub fn study_times(from: &str, to: &str, step: &str) -> Result<Vec<String>, String> {
    let from_mins = parse_time(from)?;
    let to_mins = parse_time(to)?;
    let step_mins = parse_step(step)?;

    if from_mins > to_mins {
        return Err("from must be <= to".to_string());
    }

    let mut times = Vec::new();
    let mut t = from_mins;
    while t <= to_mins {
        times.push(format!("{:02}:{:02}", t / 60, t % 60));
        t += step_mins;
    }

    if times.is_empty() {
        return Err("no times in range".to_string());
    }

    if times.len() > MAX_STILLS {
        return Err(format!("at most {MAX_STILLS} stills in one study; got {}", times.len(),));
    }

    Ok(times)
}

/// Parse an `HH:MM` solar-time string to minutes since midnight.
fn parse_time(s: &str) -> Result<i64, String> {
    let mut parts = s.splitn(2, ':');
    let h: i64 = parts.next().and_then(|p| p.parse().ok()).ok_or_else(|| format!("bad time {s:?}"))?;
    let m: i64 = parts.next().and_then(|p| p.parse().ok()).ok_or_else(|| format!("bad time {s:?}"))?;
    if h < 0 || h > 23 || m < 0 || m > 59 {
        return Err(format!("time {s:?} out of range"));
    }
    Ok(h * 60 + m)
}

/// Parse a step string (`<n>h` or `<n>min`) to minutes, refusing steps
/// smaller than `MIN_STEP_MIN`.
fn parse_step(s: &str) -> Result<i64, String> {
    let n: i64 = if s.ends_with('h') {
        if s.len() <= 1 {
            return Err(format!("bad step {s:?}"));
        }
        s[..s.len().saturating_sub(1)].parse().map_err(|_| format!("bad step {s:?}"))?
    } else if s.ends_with("min") {
        if s.len() <= 3 {
            return Err(format!("bad step {s:?}"));
        }
        s[..s.len().saturating_sub(3)].parse().map_err(|_| format!("bad step {s:?}"))?
    } else {
        return Err(format!("step {s:?} must be <n>h or <n>min"));
    };
    if n < 1 {
        return Err(format!("step {s:?} must be >= 1"));
    }
    let mins = if s.ends_with('h') { n * 60 } else { n };
    if mins < MIN_STEP_MIN {
        return Err(format!("step {s:?} must be >= {} min", MIN_STEP_MIN));
    }
    Ok(mins)
}

/// JSON handler for `sunstudy`.
fn run(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let date = params.get("date").and_then(|v| v.as_str()).ok_or_else(|| bad("sunstudy", "date (YYYY-MM-DD) required"))?;
    let from = params.get("from").and_then(|v| v.as_str()).unwrap_or("08:00");
    let to = params.get("to").and_then(|v| v.as_str()).unwrap_or("18:00");
    let step = params.get("step").and_then(|v| v.as_str()).unwrap_or("1h");
    let out = params.get("out").and_then(|v| v.as_str()).ok_or_else(|| bad("sunstudy", "out (dir/prefix) required"))?;

    let times = study_times(from, to, step).map_err(|e| bad("sunstudy", e))?;

    let drawing = session.doc()?;
    let site = drawing.arch.site.as_ref().ok_or_else(|| bad("sunstudy", "sunstudy needs a site location: run site first"))?;

    let hooks = render_hooks().ok_or_else(|| bad("sunstudy", "no render hook installed"))?;

    let size: Vec<u64> = params
        .get("size")
        .and_then(|v| v.as_array())
        .and_then(|a| a.iter().take(2).map(|x| x.as_u64()).collect::<Option<Vec<_>>>())
        .unwrap_or_else(|| vec![1200, 800]);
    for &w in &size {
        if w > MAX_SIZE {
            return Err(bad("sunstudy", format!("size component {} exceeds {}", w, MAX_SIZE)));
        }
    }
    let (w, h): (u32, u32) = (size[0] as u32, size[1] as u32);

    let camera: Option<Camera> = parse_camera(params).ok();
    let final_render = params.get("final").and_then(|v| v.as_bool()).unwrap_or(false);
    let ground = params.get("ground").and_then(|v| v.as_bool()).unwrap_or(true);

    let mut stills: Vec<Value> = Vec::new();
    let mut count: usize = 0;

    for t in &times {
        let sun_opt = sun::sun_position(site.latitude, date, t);
        match sun_opt {
            Some(s) if s.elevation_deg < 0.0 => {
                stills.push(json!({
                    "time": t,
                    "azimuth": s.azimuth_deg,
                    "elevation": s.elevation_deg,
                    "file": null,
                    "belowHorizon": true,
                }));
            }
            Some(s) => {
                let dir = true_to_drawing(Some(site), sun::sun_dir(&s));
                let hhmm = t.replace(':', "");
                let file_name = format!("{out}-{}.png", hhmm);
                let request = RenderRequest::new(
                    drawing,
                    camera.clone(),
                    (w, h),
                    Some(PathBuf::from(&file_name)),
                    final_render,
                    SunChoice::Dir(dir),
                    "shaded".to_string(),
                    ground,
                    8,
                    Vec::new(),
                );
                let _ = (hooks.render)(&request);
                count += 1;
                stills.push(json!({
                    "time": t,
                    "azimuth": s.azimuth_deg,
                    "elevation": s.elevation_deg,
                    "file": file_name,
                    "belowHorizon": false,
                }));
            }
            None => {
                stills.push(json!({
                    "time": t,
                    "azimuth": 0.0,
                    "elevation": -90.0,
                    "file": null,
                    "belowHorizon": true,
                }));
            }
        }
    }

    Ok(json!({
        "date": date,
        "stills": stills,
        "count": count,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use serde_json::json;

    #[test]
    fn study_times_hourly_from_eight_to_six() {
        let t = study_times("08:00", "18:00", "1h").unwrap();
        assert_eq!(t.len(), 11);
        assert_eq!(t[0], "08:00");
        assert_eq!(t[1], "09:00");
        assert_eq!(t[10], "18:00");
    }

    #[test]
    fn study_times_half_hourly() {
        let t = study_times("08:00", "18:00", "30min").unwrap();
        assert_eq!(t.len(), 21);
    }

    #[test]
    fn sunstudy_needs_a_site() {
        let mut s = Session::new();
        let r = s.execute(
            "sunstudy",
            &json!({
                "date": "2026-06-21",
                "from": "08:00",
                "to": "08:30",
                "step": "1h",
                "out": "test",
            }),
        );
        let msg = r.unwrap_err().to_string();
        assert!(msg.contains("sunstudy needs a site location: run site first"));
    }

    #[test]
    fn sunstudy_refuses_a_tiny_step() {
        let mut s = Session::new();
        let r = s.execute(
            "sunstudy",
            &json!({
                "date": "2026-06-21",
                "from": "08:00",
                "to": "08:30",
                "step": "2min",
                "out": "test",
            }),
        );
        let msg = r.unwrap_err().to_string();
        assert!(msg.contains(">= 5 min"));
    }
}
