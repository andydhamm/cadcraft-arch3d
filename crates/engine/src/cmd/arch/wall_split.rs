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
        let wall = &v["walls"][0];
        assert_eq!(wall["glass"], false);
        assert_eq!(wall["split"]["upper"], "Glass");
    }
}
