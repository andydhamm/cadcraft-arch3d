//! Named light sets for a rendering pass. The set is stored on the drawing and
//! applied at render time only; the underlying fixture states are never mutated.

use std::collections::BTreeMap;

use cadcraft_doc::arch::{LightSet, LightState, light_label};
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::{EngineError, Session};

const MAX_LIGHT_SETS: usize = 64;
const MAX_NAME_LEN: usize = 64;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("lightset", "Light Set", run_lightset)
            .menu(&["Architecture", "Light Set"])
            .params("{name, lights: {\"lf0\": {on: bool, brightness?: 0..=1}, ...}} | {delete: name} | {list: true}"),
    ]
}

fn parse_name(value: &Value, cmd: &str, key: &str) -> Result<String, EngineError> {
    let name = value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= MAX_NAME_LEN)
        .ok_or_else(|| bad(cmd, format!("`{key}` must be a non-empty string of at most {MAX_NAME_LEN} chars")))?;
    Ok(name.to_string())
}

fn parse_light_states(doc: &cadcraft_doc::Drawing, value: &Value, cmd: &str) -> Result<BTreeMap<String, LightState>, EngineError> {
    let obj = value.as_object().ok_or_else(|| bad(cmd, "`lights` must be an object keyed by light label"))?;
    let valid: BTreeMap<String, ()> = doc.arch.lights.iter().map(|f| (light_label(f.id), ())).collect();

    let mut out = BTreeMap::new();
    for (light, state) in obj {
        let key = light.trim();
        if !valid.contains_key(key) {
            return Err(bad(cmd, format!("unknown light `{key}`")));
        }
        let entry = state.as_object().ok_or_else(|| bad(cmd, format!("light `{key}` must be an object with `on` and optional `brightness`")))?;
        let on = entry.get("on").and_then(Value::as_bool).ok_or_else(|| bad(cmd, format!("light `{key}` must have a boolean `on` field")))?;
        let brightness = match entry.get("brightness") {
            None | Some(Value::Null) => 1.0,
            Some(v) => v
                .as_f64()
                .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
                .ok_or_else(|| bad(cmd, format!("light `{key}` brightness must be a finite number in [0, 1]")))?,
        };
        out.insert(key.to_string(), LightState { on, brightness, ..Default::default() });
    }
    Ok(out)
}

fn light_set_json(set: &LightSet) -> Value {
    json!({
        "name": set.name,
        "lights": set.lights.iter().map(|(light, state)| (light.clone(), json!({ "on": state.on, "brightness": state.brightness }))).collect::<BTreeMap<_, _>>()
    })
}

fn run_lightset(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    if params.get("list").and_then(Value::as_bool) == Some(true) {
        let doc = s.doc()?;
        return Ok(json!({ "ok": true, "lightSets": doc.arch.light_sets.iter().map(light_set_json).collect::<Vec<_>>() }));
    }

    if let Some(delete_name) = params.get("delete").filter(|v| !v.is_null()) {
        let name = parse_name(delete_name, "lightset", "delete")?;
        let mut doc = s.doc_mut()?;
        let before = doc.arch.light_sets.len();
        doc.arch.light_sets.retain(|set| set.name != name);
        if doc.arch.light_sets.len() == before {
            return Err(bad("lightset", format!("no light set named `{name}`")));
        }
        doc.touch_arch();
        return Ok(json!({ "deleted": name }));
    }

    let name = params.get("name").filter(|v| !v.is_null()).ok_or_else(|| bad("lightset", "`name` is required"))?;
    let name = parse_name(name, "lightset", "name")?;
    let lights = params.get("lights").filter(|v| !v.is_null()).ok_or_else(|| bad("lightset", "`lights` is required"))?;
    let mut doc = s.doc_mut()?;
    let parsed = parse_light_states(&doc, lights, "lightset")?;
    if doc.arch.light_sets.iter().all(|set| set.name != name) && doc.arch.light_sets.len() >= MAX_LIGHT_SETS {
        return Err(bad("lightset", format!("at most {MAX_LIGHT_SETS} light sets are allowed")));
    }
    if let Some(pos) = doc.arch.light_sets.iter().position(|set| set.name == name) {
        doc.arch.light_sets[pos].lights = parsed;
    } else {
        doc.arch.light_sets.push(LightSet { name: name.clone(), lights: parsed, ..Default::default() });
    }
    doc.touch_arch();
    Ok(json!({ "lightSet": name, "lights": doc.arch.light_sets.iter().find(|set| set.name == name).map_or(0, |set| set.lights.len()) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchId, LightFixture, LightKind};
    use cadcraft_geom::Vec2;
    use serde_json::json;

    fn session_with_light() -> Session {
        let mut s = Session::new();
        let mut doc = s.doc_mut().unwrap();
        doc.arch.lights.push(LightFixture::new(ArchId(0), "Recessed can", Vec2::new(0.0, 0.0)));
        doc.touch_arch();
        s
    }

    #[test]
    fn a_light_set_changes_only_the_render() {
        let mut s = session_with_light();
        s.execute("lightset", &json!({ "name": "night", "lights": { "lf0": { "on": false, "brightness": 0.25 } } })).unwrap();
        let doc = s.doc().unwrap();
        assert_eq!(doc.arch.lights[0].brightness, 1.0);
        assert!(!doc.arch.light_sets[0].lights["lf0"].on);
        let overrides = std::collections::BTreeMap::from([(
            "lf0".to_string(),
            cadcraft_doc::arch::LightState { on: false, brightness: 0.25, ..Default::default() },
        )]);
        let scene = cadcraft_arch3d::light::scene_lights_with(&doc.arch, &overrides);
        assert!(scene.is_empty());
    }

    #[test]
    fn unknown_light_sets_are_refused() {
        let mut s = session_with_light();
        let err = s.execute("camera", &json!({ "save": "front", "azimuth": 0.0, "lightSet": "missing" })).unwrap_err();
        assert!(err.to_string().contains("unknown light set"));
        let err = s.execute("lightset", &json!({ "name": "bad", "lights": { "lf9": { "on": true } } })).unwrap_err();
        assert!(err.to_string().contains("unknown light"));
    }
}
