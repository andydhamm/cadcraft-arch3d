use cadcraft_doc::ArchId;
use cadcraft_doc::arch::{ArchModel, WallSplit, is_glass_type};
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("wall.split", "Split Wall", run_wall_split)
            .menu(&["Architecture", "Split Wall"])
            .params("{id: \"w0\", height?: number | null, lower?: type, upper?: type, lowerThickness?: metres, upperThickness?: metres}"),
    ]
}

fn wall_type_default(model: &ArchModel, name: &str) -> Option<(String, bool, f64)> {
    let key = name.trim();
    if key.is_empty() {
        return None;
    }
    if let Some(t) = model.wall_type(key) {
        let thickness = t.layers.iter().map(|l| l.thickness).sum::<f64>();
        return Some((t.name.clone(), t.name.eq_ignore_ascii_case("Exterior"), thickness.max(0.0)));
    }
    match key.to_ascii_lowercase().as_str() {
        "exterior" => Some(("Exterior".to_string(), true, 0.2)),
        "interior" => Some(("Interior".to_string(), false, 0.1)),
        "knee" => Some(("Knee".to_string(), false, 0.1)),
        "railing" => Some(("Railing".to_string(), false, 0.1)),
        "retaining" => Some(("Retaining".to_string(), false, 0.2)),
        "glass" => Some(("Glass".to_string(), false, cadcraft_doc::arch::GLASS_THICKNESS)),
        _ => None,
    }
}

fn read_id(raw: &Value) -> Result<ArchId> {
    let id = raw.as_str().ok_or_else(|| bad("wall.split", "`id` (\"w0\") is required"))?;
    id.trim().parse::<ArchId>().map_err(|_| bad("wall.split", format!("unknown wall id `{id}`")))
}

pub(crate) fn inspect_wall_types(model: &ArchModel, out: &mut serde_json::Map<String, Value>) {
    if let Some(Value::Array(rows)) = out.get_mut("walls") {
        for (idx, row) in rows.iter_mut().enumerate() {
            let Some(wall) = model.walls.get(idx) else { continue };
            let split = wall.split.as_ref().map(|s| {
                json!({
                    "height": s.height,
                    "upper": s.upper,
                    "upperThickness": s.upper_thickness,
                })
            });
            let obj = row.as_object_mut().unwrap();
            obj.insert("glass".to_string(), json!(is_glass_type(&wall.wall_type) && wall.split.is_none()));
            obj.insert("split".to_string(), split.unwrap_or(Value::Null));
        }
    }
}

fn run_wall_split(s: &mut Session, p: &Value) -> Result<Value> {
    let id = read_id(p.get("id").ok_or_else(|| bad("wall.split", "`id` (\"w0\") is required"))?)?;
    let doc = s.doc_mut()?;
    let wall_index = doc.arch.walls.iter().position(|w| w.id == id).ok_or_else(|| bad("wall.split", format!("unknown wall id `{id}`")))?;
    let mut wall = doc.arch.walls[wall_index].clone();

    if wall.divider || wall.retaining {
        return Err(bad("wall.split", "split walls must not be divider or retaining walls"));
    }

    match p.get("height") {
        Some(Value::Null) => {
            let thickness = wall.thickness;
            wall.split = None;
            doc.arch.walls[wall_index] = wall;
            doc.touch_arch();
            return Ok(json!({ "wall": id.to_string(), "split": null, "thickness": thickness }));
        }
        Some(v) if v.as_f64().is_some_and(|x| x.is_finite()) => {
            let height = v.as_f64().unwrap();
            let max_span = cadcraft_arch3d::wall_span(&doc.arch, &wall).map_or(wall.height, |(base, top)| top - base);
            if !(height >= 0.1 && height <= max_span - 0.1) {
                return Err(bad("wall.split", format!("`height` must be in [0.1, {}] metres", (max_span - 0.1).max(0.0))));
            }
            wall.split =
                Some(WallSplit { height, upper: "Glass".to_string(), upper_thickness: cadcraft_doc::arch::GLASS_THICKNESS, ..Default::default() });
            if let Some(lower) = p.get("lower").and_then(Value::as_str) {
                if lower.trim().is_empty() {
                    return Err(bad("wall.split", "`lower` cannot be empty"));
                }
            }
            if let Some(upper) = p.get("upper").and_then(Value::as_str) {
                if upper.trim().is_empty() {
                    return Err(bad("wall.split", "`upper` cannot be empty"));
                }
            }
            let lower_name = str_param(p, "lower").unwrap_or(&wall.wall_type).trim().to_string();
            let upper_name = str_param(p, "upper").unwrap_or("Glass").trim().to_string();
            let lower = wall_type_default(&doc.arch, &lower_name).ok_or_else(|| bad("wall.split", format!("unknown wall type `{lower_name}`")))?;
            let upper = wall_type_default(&doc.arch, &upper_name).ok_or_else(|| bad("wall.split", format!("unknown wall type `{upper_name}`")))?;
            let lower_thickness = p.get("lowerThickness").and_then(Value::as_f64).filter(|x| x.is_finite() && *x > 0.0).unwrap_or(lower.2);
            let upper_thickness = p.get("upperThickness").and_then(Value::as_f64).filter(|x| x.is_finite() && *x > 0.0).unwrap_or(upper.2);
            if upper_thickness > lower_thickness + 1e-9 {
                return Err(bad("wall.split", "`upperThickness` must be <= lower thickness"));
            }
            let upper_name = upper.0.clone();
            let thickness = lower_thickness;
            wall.wall_type = lower.0;
            wall.exterior = lower.1;
            wall.thickness = thickness;
            wall.layers = super::wall_layers(wall.exterior);
            if !wall.layers.is_empty() {
                let total: f64 = wall.layers.iter().map(|(_, t)| *t).sum();
                if total > 0.0 && total.is_finite() {
                    let scale = wall.thickness / total;
                    wall.layers = wall.layers.iter().map(|(name, t)| (name.clone(), *t * scale)).collect();
                }
            }
            wall.split = Some(WallSplit { height, upper: upper_name.clone(), upper_thickness, ..Default::default() });
            doc.arch.walls[wall_index] = wall;
            doc.touch_arch();
            return Ok(json!({
                "wall": id.to_string(),
                "split": { "height": height, "upper": upper_name, "upperThickness": upper_thickness },
                "thickness": thickness,
            }));
        }
        Some(_) => return Err(bad("wall.split", "`height` must be a finite number or null")),
        None => return Err(bad("wall.split", "`height` is required unless null")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use cadcraft_doc::{Level, arch::Wall};
    use serde_json::json;

    fn new_session_with_level() -> Session {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level { name: "ground".to_string(), ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        d.touch_arch();
        s
    }

    #[test]
    fn glass_type_defaults() {
        let mut s = new_session_with_level();
        let r = s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Glass"})).unwrap();
        let wall = &s.doc().unwrap().arch.walls[0];
        assert_eq!(wall.wall_type, "Glass");
        assert_eq!(wall.thickness, cadcraft_doc::arch::GLASS_THICKNESS);
        assert_eq!(wall.height, cadcraft_doc::arch::GLASS_WALL_HEIGHT);
        assert_eq!(r["walls"][0]["id"], "w0");
    }

    #[test]
    fn explicit_glass_thickness_and_height_kept() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Glass", "thickness": 0.015, "height": 3.0})).unwrap();
        let wall = &s.doc().unwrap().arch.walls[0];
        assert_eq!(wall.wall_type, "Glass");
        assert_eq!(wall.thickness, 0.015);
        assert_eq!(wall.height, 3.0);
    }

    #[test]
    fn unknown_type_still_refused() {
        let mut s = new_session_with_level();
        let r = s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Glasss"}));
        assert!(r.is_err());
    }

    #[test]
    fn split_glass_with_an_interior_base() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        let res = s.execute("wall.split", &json!({"id": "w0", "height": 1.07, "lower": "Interior", "upper": "Glass"})).unwrap();
        assert_eq!(res["split"]["upper"], "Glass");
        assert_eq!(res["split"]["height"], 1.07);
        let wall = &s.doc().unwrap().arch.walls[0];
        assert_eq!(wall.wall_type, "Interior");
        assert!(!wall.exterior);
        assert_eq!(wall.split.as_ref().unwrap().upper, "Glass");
    }

    #[test]
    fn remove_split() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "upper": "Glass"})).unwrap();
        let res = s.execute("wall.split", &json!({"id": "w0", "height": null})).unwrap();
        assert_eq!(res["split"], Value::Null);
        assert!(s.doc().unwrap().arch.walls[0].split.is_none());
    }

    #[test]
    fn split_refusals_leave_nothing() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        for bad in [
            json!({"id": "w0", "height": 100.0}),
            json!({"id": "w0", "height": 0.05}),
            json!({"id": "w0", "height": 1.0, "upperThickness": 2.0}),
            json!({"id": "oops", "height": 1.0}),
        ] {
            assert!(s.execute("wall.split", &bad).is_err(), "accepted {bad}");
        }
        assert!(s.doc().unwrap().arch.walls[0].split.is_none());
    }

    #[test]
    fn split_is_one_undo_step() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        let before = s.doc().unwrap().arch_revision;
        s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "upper": "Glass"})).unwrap();
        let after = s.doc().unwrap().arch_revision;
        assert!(after > before, "split should mutate the drawing once");
    }

    #[test]
    fn inspect_walls_rows_glass_and_split() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Glass"})).unwrap();
        s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "lower": "Interior", "upper": "Glass"})).unwrap();
        let v = s.execute("arch.inspect", &json!({"walls": true})).unwrap();
        let walls = v["walls"].as_array().unwrap();
        let w0 = walls.iter().find(|w| w.get("id").and_then(Value::as_str) == Some("w0")).expect("w0 row");
        assert_eq!(w0.get("glass").and_then(Value::as_bool), Some(false));
        assert_eq!(w0["split"]["upper"], "Glass");
    }

    #[test]
    fn glass_pony_wall_before_2x4_does_not_break_inspect() {
        let mut s = new_session_with_level();
        // Glass pony wall first (the phase-18 showcase scenario)
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Glass"})).unwrap();
        // Then a 2x4 wall
        s.execute("wall", &json!({"points": [[0.0, 1.0], [2.0, 1.0]], "thickness": 0.2, "wall_type": "2x4"})).unwrap();
        // Split the 2x4 wall (w1)
        s.execute("wall.split", &json!({"id": "w1", "height": 1.0, "upper": "Glass"})).unwrap();
        let v = s.execute("arch.inspect", &json!({"walls": true})).unwrap();
        let walls = v["walls"].as_array().unwrap();
        // Glass pony wall: glass=true, no split
        let w0 = walls.iter().find(|w| w.get("id").and_then(Value::as_str) == Some("w0")).expect("w0 row");
        assert_eq!(w0.get("glass").and_then(Value::as_bool), Some(true));
        assert_eq!(w0.get("split"), Some(&Value::Null));
        // 2x4 wall: glass=false, has split
        let w1 = walls.iter().find(|w| w.get("id").and_then(Value::as_str) == Some("w1")).expect("w1 row");
        assert_eq!(w1.get("glass").and_then(Value::as_bool), Some(false));
        assert_eq!(w1["split"]["upper"], "Glass");
    }

    #[test]
    fn inspect_multiple_walls_mixed_states() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Glass"})).unwrap();
        s.execute("wall", &json!({"points": [[0.0, 1.0], [2.0, 1.0]], "type": "Interior"})).unwrap();
        s.execute("wall", &json!({"points": [[0.0, 2.0], [2.0, 2.0]], "type": "Exterior"})).unwrap();
        // Split only the interior wall
        s.execute("wall.split", &json!({"id": "w1", "height": 1.0, "upper": "Glass"})).unwrap();
        let v = s.execute("arch.inspect", &json!({"walls": true})).unwrap();
        let walls = v["walls"].as_array().unwrap();
        assert_eq!(walls.len(), 3);
        let w0 = walls.iter().find(|w| w.get("id").and_then(Value::as_str) == Some("w0")).expect("w0");
        let w1 = walls.iter().find(|w| w.get("id").and_then(Value::as_str) == Some("w1")).expect("w1");
        let w2 = walls.iter().find(|w| w.get("id").and_then(Value::as_str) == Some("w2")).expect("w2");
        assert_eq!(w0.get("glass").and_then(Value::as_bool), Some(true));
        assert_eq!(w0.get("split"), Some(&Value::Null));
        assert_eq!(w1.get("glass").and_then(Value::as_bool), Some(false));
        assert_eq!(w1["split"]["upper"], "Glass");
        assert_eq!(w2.get("glass").and_then(Value::as_bool), Some(false));
        assert_eq!(w2.get("split"), Some(&Value::Null));
    }

    #[test]
    fn split_upper_thickness_appears_in_inspect() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "upper": "Glass", "upperThickness": 0.015})).unwrap();
        let v = s.execute("arch.inspect", &json!({"walls": true})).unwrap();
        let walls = v["walls"].as_array().unwrap();
        let w0 = walls.iter().find(|w| w.get("id").and_then(Value::as_str) == Some("w0")).expect("w0 row");
        assert_eq!(w0["split"]["upperThickness"], 0.015);
        assert_eq!(w0["split"]["height"], 1.0);
    }

    #[test]
    fn inspect_empty_model_returns_empty_walls() {
        let mut s = new_session_with_level();
        let v = s.execute("arch.inspect", &json!({"walls": true})).unwrap();
        let walls = v["walls"].as_array().unwrap();
        assert!(walls.is_empty());
    }

    #[test]
    fn split_glass_wall_upper_defaults_to_glass() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Glass"})).unwrap();
        let res = s.execute("wall.split", &json!({"id": "w0", "height": 1.0})).unwrap();
        assert_eq!(res["split"]["upper"], "Glass");
        assert_eq!(res["split"]["height"], 1.0);
        let wall = &s.doc().unwrap().arch.walls[0];
        assert_eq!(wall.wall_type, "Glass");
        assert_eq!(wall.split.as_ref().unwrap().upper, "Glass");
        assert_eq!(wall.split.as_ref().unwrap().upper_thickness, cadcraft_doc::arch::GLASS_THICKNESS);
    }

    #[test]
    fn split_height_nan_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        let before = s.doc().unwrap().arch_revision;
        let r = s.execute("wall.split", &json!({"id": "w0", "height": f64::NAN}));
        assert!(r.is_err(), "NaN height should be refused");
        assert_eq!(s.doc().unwrap().arch_revision, before);
        assert!(s.doc().unwrap().arch.walls[0].split.is_none());
    }

    #[test]
    fn split_height_infinity_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        let before = s.doc().unwrap().arch_revision;
        let r = s.execute("wall.split", &json!({"id": "w0", "height": f64::INFINITY}));
        assert!(r.is_err(), "Infinity height should be refused");
        assert_eq!(s.doc().unwrap().arch_revision, before);
        assert!(s.doc().unwrap().arch.walls[0].split.is_none());
    }

    #[test]
    fn split_unknown_lower_type_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        let before = s.doc().unwrap().arch_revision;
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "lower": "Bogus"}));
        assert!(r.is_err(), "unknown lower type should be refused");
        assert_eq!(s.doc().unwrap().arch_revision, before);
        assert!(s.doc().unwrap().arch.walls[0].split.is_none());
    }

    #[test]
    fn split_unknown_upper_type_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        let before = s.doc().unwrap().arch_revision;
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "upper": "Bogus"}));
        assert!(r.is_err(), "unknown upper type should be refused");
        assert_eq!(s.doc().unwrap().arch_revision, before);
        assert!(s.doc().unwrap().arch.walls[0].split.is_none());
    }

    #[test]
    fn split_empty_lower_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        let before = s.doc().unwrap().arch_revision;
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "lower": ""}));
        assert!(r.is_err(), "empty lower should be refused");
        assert_eq!(s.doc().unwrap().arch_revision, before);
        assert!(s.doc().unwrap().arch.walls[0].split.is_none());
    }

    #[test]
    fn split_empty_upper_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        let before = s.doc().unwrap().arch_revision;
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "upper": ""}));
        assert!(r.is_err(), "empty upper should be refused");
        assert_eq!(s.doc().unwrap().arch_revision, before);
        assert!(s.doc().unwrap().arch.walls[0].split.is_none());
    }

    #[test]
    fn split_divider_wall_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "divider": true})).unwrap();
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 1.0}));
        assert!(r.is_err(), "split on divider wall should be refused");
        assert!(s.doc().unwrap().arch.walls[0].split.is_none());
    }

    #[test]
    fn split_retaining_wall_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "retaining": true})).unwrap();
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 1.0}));
        assert!(r.is_err(), "split on retaining wall should be refused");
        assert!(s.doc().unwrap().arch.walls[0].split.is_none());
    }

    #[test]
    fn split_height_at_min_boundary_accepted() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior", "height": 2.7})).unwrap();
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 0.1}));
        assert!(r.is_ok(), "height at min boundary (0.1) should be accepted");
        let res = r.unwrap();
        assert_eq!(res["split"]["height"], 0.1);
    }

    #[test]
    fn split_height_at_max_boundary_accepted() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior", "height": 2.7})).unwrap();
        // max = height - 0.1 = 2.6
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 2.6}));
        assert!(r.is_ok(), "height at max boundary (2.6) should be accepted");
        let res = r.unwrap();
        assert_eq!(res["split"]["height"], 2.6);
    }

    #[test]
    fn split_height_just_below_min_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior", "height": 2.7})).unwrap();
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 0.09}));
        assert!(r.is_err(), "height below min boundary should be refused");
    }

    #[test]
    fn split_height_just_above_max_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior", "height": 2.7})).unwrap();
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 2.61}));
        assert!(r.is_err(), "height above max boundary should be refused");
    }

    #[test]
    fn split_with_top_level_uses_span_for_range() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level { index: 0, name: "Lower".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        d.arch.levels.push(Level { index: 1, name: "Upper".into(), elevation: 2.7, ceiling_height: 2.4, structure: 0.3, ..Default::default() });
        d.touch_arch();
        // Wall spanning both levels: height = 2.7 + 2.4 = 5.1
        s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0]], "level": "Lower", "topLevel": "Upper", "type": "Interior"})).unwrap();
        // Split at 4.0 should be valid (within [0.1, 5.0])
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 4.0}));
        assert!(r.is_ok(), "split at 4.0 within span should be accepted");
        let res = r.unwrap();
        assert_eq!(res["split"]["height"], 4.0);
        // Split at 5.5 should be refused (above span - 0.1 = 5.0)
        let r2 = s.execute("wall.split", &json!({"id": "w0", "height": 5.5}));
        assert!(r2.is_err(), "split at 5.5 above span should be refused");
    }

    #[test]
    fn split_missing_id_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        let r = s.execute("wall.split", &json!({"height": 1.0}));
        assert!(r.is_err(), "missing id should be refused");
    }

    #[test]
    fn split_null_id_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        let r = s.execute("wall.split", &json!({"id": null, "height": 1.0}));
        assert!(r.is_err(), "null id should be refused");
    }

    #[test]
    fn split_non_numeric_height_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        let r = s.execute("wall.split", &json!({"id": "w0", "height": "one"}));
        assert!(r.is_err(), "non-numeric height should be refused");
    }

    #[test]
    fn split_upper_thickness_nan_ignored() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        // NaN upperThickness is ignored (falls back to default)
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "upperThickness": f64::NAN}));
        assert!(r.is_ok(), "NaN upperThickness should be ignored");
        let wall = &s.doc().unwrap().arch.walls[0];
        assert_eq!(wall.split.as_ref().unwrap().upper_thickness, cadcraft_doc::arch::GLASS_THICKNESS);
    }

    #[test]
    fn split_lower_thickness_nan_ignored() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        // NaN lowerThickness is ignored (falls back to default)
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "lowerThickness": f64::NAN}));
        assert!(r.is_ok(), "NaN lowerThickness should be ignored");
    }

    #[test]
    fn split_upper_thickness_equal_to_lower_accepted() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        // upperThickness == lowerThickness should be accepted (not >)
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "lowerThickness": 0.1, "upperThickness": 0.1}));
        assert!(r.is_ok(), "upperThickness equal to lower should be accepted");
    }

    #[test]
    fn split_upper_thickness_slightly_greater_than_lower_refused() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        // upperThickness > lowerThickness + 1e-9 should be refused
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "lowerThickness": 0.1, "upperThickness": 0.100000002}));
        assert!(r.is_err(), "upperThickness slightly greater than lower should be refused");
    }

    #[test]
    fn split_with_model_wall_type() {
        let mut s = new_session_with_level();
        // Add a custom wall type to the model
        {
            let d = s.doc_mut().unwrap();
            d.arch.types.push(cadcraft_doc::arch::WallType {
                name: "Custom".into(),
                layers: vec![cadcraft_doc::arch::WallLayer { name: "custom".into(), thickness: 0.15, material: "custom".into() }],
                ..Default::default()
            });
            d.touch_arch();
        }
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Custom"})).unwrap();
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "lower": "Custom"})).unwrap();
        assert_eq!(r["wall"], "w0");
        let wall = &s.doc().unwrap().arch.walls[0];
        assert_eq!(wall.wall_type, "Custom");
    }

    #[test]
    fn split_rebuilds_layers_for_lower_type() {
        let mut s = new_session_with_level();
        s.execute("wall", &json!({"points": [[0.0, 0.0], [2.0, 0.0]], "type": "Interior"})).unwrap();
        // Split with exterior lower type - layers should change
        let r = s.execute("wall.split", &json!({"id": "w0", "height": 1.0, "lower": "Exterior"})).unwrap();
        assert!(r.is_ok());
        let wall = &s.doc().unwrap().arch.walls[0];
        assert_eq!(wall.wall_type, "Exterior");
        assert!(wall.exterior);
        // Exterior has 4 layers (siding, sheathing, stud, drywall)
        assert_eq!(wall.layers.len(), 4);
    }
}
