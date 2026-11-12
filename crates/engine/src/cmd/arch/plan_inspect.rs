//! The plan-view part of `arch.inspect` (phase 20): `{"planViews": true}` lists the plan views,
//! the open tabs and the active view; `{"plan": true | "<name>"}` reports what the plan draws
//! (or would draw with that view open) as annotation lines and labels. Read-only.

use cadcraft_arch::plan::{PlanAnnotation, active_view, annotate, opened_level};
use cadcraft_doc::arch::{ArchModel, PlanKind, PlanView, handle_json};
use serde_json::{Map as JsonMap, Value, json};

use crate::EngineError;
use crate::cmd::bad;

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn view_row(v: &PlanView) -> Value {
    let rules: Vec<Value> = v.layers.rules.iter().map(|r| json!({"pattern": r.pattern, "on": r.on, "transparency": r.transparency})).collect();
    json!({
        "name": v.name,
        "level": v.level,
        "kind": v.kind.key(),
        "labels": {"name": v.labels.name, "area": v.labels.area, "dimensions": v.labels.dimensions, "ceiling": v.labels.ceiling},
        "annotation": {"layer": v.annotation.layer, "textStyle": v.annotation.text_style, "dimStyle": v.annotation.dim_style},
        "reference": v.reference.map(|r| json!({"level": r.level, "color": hex(r.color), "transparency": r.transparency})),
        "zoom": v.zoom.map(|z| json!({"center": z.center, "width": z.width, "height": z.height})),
        "rules": rules,
        "snapshot": v.layers.layers.len(),
    })
}

/// Add `planViews`, `openPlanViews` and `activePlanView` when `{"planViews": true}`.
pub(crate) fn inspect_plan_views(m: &ArchModel, p: &Value, out: &mut JsonMap<String, Value>) {
    if p.get("planViews").and_then(Value::as_bool) != Some(true) {
        return;
    }
    out.insert("planViews".to_string(), Value::Array(m.plan_views.iter().map(view_row).collect()));
    out.insert("openPlanViews".to_string(), json!(m.open_plan_views));
    out.insert("activePlanView".to_string(), json!(m.active_plan_view));
}

fn plan_json(view: Option<&PlanView>, level: Option<u32>, a: &PlanAnnotation) -> Value {
    let lines: Vec<Value> = a
        .lines
        .iter()
        .map(|l| {
            let mut o = JsonMap::new();
            o.insert("layer".to_string(), json!(l.layer));
            o.insert("kind".to_string(), json!(l.kind.key()));
            o.insert("a".to_string(), json!([l.a.x, l.a.y]));
            o.insert("b".to_string(), json!([l.b.x, l.b.y]));
            o.insert("length".to_string(), json!(l.length()));
            o.insert("handle".to_string(), json!(handle_json(l.handle)));
            o.insert("dashed".to_string(), json!(l.dashed));
            if let Some(c) = l.color {
                o.insert("color".to_string(), json!(hex(c)));
            }
            if let Some(t) = l.transparency {
                o.insert("transparency".to_string(), json!(t));
            }
            Value::Object(o)
        })
        .collect();
    let labels: Vec<Value> = a
        .labels
        .iter()
        .map(|l| {
            json!({
                "layer": l.layer, "kind": l.kind.key(), "text": l.text, "at": [l.at.x, l.at.y],
                "height": l.height, "rotation": l.rotation, "handle": handle_json(l.handle),
            })
        })
        .collect();
    json!({
        "view": view.map(|v| v.name.clone()),
        "kind": view.map_or(PlanKind::Floor, |v| v.kind).key(),
        "level": level,
        "lines": lines,
        "labels": labels,
    })
}

/// Add `plan` for `{"plan": true}` (what the plan draws now) or `{"plan": "<name>"}` (that view
/// as if open; nothing changes). Absent or `false` adds nothing; anything else is an error.
pub(crate) fn inspect_plan(m: &ArchModel, p: &Value, out: &mut JsonMap<String, Value>) -> Result<(), EngineError> {
    let (view, level) = match p.get("plan") {
        None | Some(Value::Bool(false)) => return Ok(()),
        Some(Value::Bool(true)) => {
            let v = active_view(m);
            (v, m.plan_level)
        }
        Some(Value::String(name)) => {
            let v = m.plan_view(name).ok_or_else(|| bad("arch.inspect", format!("no plan view `{name}`")))?;
            (Some(v), opened_level(m, Some(v)))
        }
        Some(_) => return Err(bad("arch.inspect", "`plan` must be true or the name of a plan view")),
    };
    let a = annotate(m, view, level);
    out.insert("plan".to_string(), plan_json(view, level, &a));
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::Session;
    use cadcraft_doc::arch::{Level, Roof, RoofEdge, RoofEdgeKind, Room, template_plan_views};
    use cadcraft_geom::Vec2;

    fn quad(x: f64, y: f64) -> Vec<Vec2> {
        vec![Vec2::new(x, y), Vec2::new(x + 4.0, y), Vec2::new(x + 4.0, y + 3.0), Vec2::new(x, y + 3.0)]
    }

    fn session() -> Session {
        let mut s = Session::new();
        {
            let d = s.doc_mut().unwrap();
            d.arch.levels = vec![
                Level { index: 0, name: "L0".into(), ..Level::default() },
                Level { index: 1, name: "L1".into(), elevation: 3.0, ..Level::default() },
            ];
            d.arch.push_room(Room { polygon: quad(0.0, 0.0), name: "Down".into(), level: 0, ..Room::default() });
            d.arch.push_room(Room { polygon: quad(0.0, 10.0), name: "Up".into(), level: 1, ..Room::default() });
            d.arch.push_roof(Roof {
                edges: vec![
                    RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::ZERO, b: Vec2::new(4.0, 0.0), ..RoofEdge::default() },
                    RoofEdge { kind: RoofEdgeKind::Eave, a: Vec2::ZERO, b: Vec2::new(0.0, 4.0), ..RoofEdge::default() },
                ],
                ..Roof::default()
            });
        }
        s
    }

    fn texts(plan: &Value) -> Vec<String> {
        plan["labels"].as_array().unwrap().iter().filter_map(|l| l["text"].as_str().map(str::to_string)).collect()
    }

    #[test]
    fn inspect_plan_views_lists_the_templates_and_tabs() {
        let mut s = session();
        let r = s.execute("arch.inspect", &json!({})).unwrap();
        assert!(r.get("planViews").is_none(), "only on request");
        {
            let d = s.doc_mut().unwrap();
            d.arch.open_plan_views = vec!["Roof Plan".into()];
            d.arch.active_plan_view = Some("Roof Plan".into());
        }
        let r = s.execute("arch.inspect", &json!({"planViews": true})).unwrap();
        let rows = r["planViews"].as_array().unwrap();
        let names: Vec<&str> = rows.iter().filter_map(|v| v["name"].as_str()).collect();
        let want: Vec<String> = template_plan_views().into_iter().map(|v| v.name).collect();
        assert_eq!(names, want.iter().map(String::as_str).collect::<Vec<_>>());
        let fp = &rows[1];
        assert_eq!(fp["name"], "Floor Plan");
        assert_eq!(fp["kind"], "floor");
        assert!(fp["level"].is_null() && fp["reference"].is_null() && fp["zoom"].is_null());
        assert_eq!(fp["labels"], json!({"name": true, "area": true, "dimensions": true, "ceiling": false}));
        assert_eq!(fp["annotation"]["layer"], "A-ANNO-NOTE");
        assert_eq!(fp["rules"][0], json!({"pattern": "*", "on": true, "transparency": 0}));
        assert_eq!(fp["rules"][1], json!({"pattern": "E-*", "on": false, "transparency": null}));
        assert_eq!(fp["snapshot"], 0);
        assert_eq!(rows[2]["rules"][0], json!({"pattern": "*", "on": null, "transparency": 70}));
        assert_eq!(rows[3]["kind"], "reflectedCeiling");
        assert_eq!(rows[4]["kind"], "roof");
        assert_eq!(r["openPlanViews"], json!(["Roof Plan"]));
        assert_eq!(r["activePlanView"], "Roof Plan");
    }

    #[test]
    fn inspect_plan_default_is_todays_plan() {
        let mut s = session();
        let r = s.execute("arch.inspect", &json!({})).unwrap();
        assert!(r.get("plan").is_none());
        let r = s.execute("arch.inspect", &json!({"plan": false})).unwrap();
        assert!(r.get("plan").is_none());
        let r = s.execute("arch.inspect", &json!({"plan": true})).unwrap();
        let plan = &r["plan"];
        assert!(plan["view"].is_null() && plan["level"].is_null());
        assert_eq!(plan["kind"], "floor");
        let lines = plan["lines"].as_array().unwrap();
        assert_eq!(lines.len(), 1, "the eave is skipped");
        assert_eq!(lines[0]["kind"], "ridge");
        assert_eq!(lines[0]["layer"], "A-ROOF");
        assert_eq!(lines[0]["length"], 4.0);
        assert_eq!(lines[0]["dashed"], false);
        assert!(lines[0].get("color").is_none());
        assert_eq!(texts(plan), vec!["Down", "Up"]);
        let l = &plan["labels"][0];
        assert_eq!(
            (l["layer"].as_str(), l["kind"].as_str(), l["height"].as_f64(), l["rotation"].as_f64()),
            (Some("A-ROOM"), Some("roomName"), Some(0.24), Some(0.0))
        );
        assert_eq!(l["at"], json!([2.0, 1.5]));
    }

    #[test]
    fn inspect_plan_named_view_uses_its_level_and_changes_nothing() {
        let mut s = session();
        {
            let d = s.doc_mut().unwrap();
            if let Some(v) = d.arch.plan_view_mut("Floor Plan") {
                v.level = Some(1);
            }
        }
        let before = (s.doc().unwrap().arch_revision, s.doc().unwrap().arch.active_plan_view.clone(), s.doc().unwrap().arch.plan_level);
        let r = s.execute("arch.inspect", &json!({"plan": "floor plan"})).unwrap();
        assert_eq!(r["plan"]["view"], "Floor Plan");
        assert_eq!(r["plan"]["level"], 1);
        // Floor Plan shows name, area, dimensions.
        assert_eq!(texts(&r["plan"]), vec!["Up", "0.00 m2", "4.00 x 3.00"]);
        let r = s.execute("arch.inspect", &json!({"plan": "Roof Plan"})).unwrap();
        assert_eq!(r["plan"]["kind"], "roof");
        assert_eq!(r["plan"]["lines"].as_array().map(Vec::len), Some(1));
        let r = s.execute("arch.inspect", &json!({"plan": "Reflected Ceiling"})).unwrap();
        assert_eq!(r["plan"]["kind"], "reflectedCeiling");
        let after = (s.doc().unwrap().arch_revision, s.doc().unwrap().arch.active_plan_view.clone(), s.doc().unwrap().arch.plan_level);
        assert_eq!(before, after);
        assert!(s.doc().unwrap().arch.open_plan_views.is_empty());
    }

    #[test]
    fn inspect_plan_unknown_view_or_bad_value_is_an_error() {
        let mut s = session();
        let e = s.execute("arch.inspect", &json!({"plan": "Nope"})).unwrap_err().to_string();
        assert!(e.contains("Nope"), "{e}");
        assert!(s.execute("arch.inspect", &json!({"plan": 3})).is_err());
        assert!(s.execute("arch.inspect", &json!({"plan": ["Floor Plan"]})).is_err());
        assert!(s.doc().unwrap().arch.open_plan_views.is_empty());
        assert!(s.doc().unwrap().arch.active_plan_view.is_none());
    }

    #[test]
    fn inspect_rooms_report_label_area_and_ceiling_text() {
        let mut s = session();
        {
            let d = s.doc_mut().unwrap();
            d.arch.rooms[1].label_area = cadcraft_doc::arch::LabelArea::Standard;
        }
        let r = s.execute("arch.inspect", &json!({"rooms": true})).unwrap();
        let rows = r["rooms"].as_array().unwrap();
        assert_eq!(rows[0]["labelArea"], "interior");
        assert_eq!(rows[1]["labelArea"], "standard");
        assert_eq!(rows[0]["ceilingText"], "2.40");
    }

    #[test]
    fn inspect_plan_true_annotates_the_plan_level() {
        let mut s = session();
        {
            let d = s.doc_mut().unwrap();
            if let Some(v) = d.arch.plan_view_mut("Floor Plan") {
                v.level = Some(1);
            }
            d.arch.active_plan_view = Some("Floor Plan".into());
            d.arch.plan_level = Some(0);
        }
        let r = s.execute("arch.inspect", &json!({"plan": true})).unwrap();
        assert_eq!(r["plan"]["level"], 0);
        // Floor Plan shows name, area, dimensions.
        assert_eq!(texts(&r["plan"]), vec!["Down", "0.00 m2", "4.00 x 3.00"]);
        let r = s.execute("arch.inspect", &json!({"plan": "Floor Plan"})).unwrap();
        assert_eq!(r["plan"]["level"], 1);
        assert_eq!(texts(&r["plan"]), vec!["Up", "0.00 m2", "4.00 x 3.00"]);
    }
}
