//! `cabinet.fill` command: close a cabinet run against a wall with one or more fillers.

use crate::cmd::{CommandSpec, bad};
use crate::{Result, Session};
use cadcraft_arch::casework_fill::{filler_for, run_gaps, run_of};
use cadcraft_doc::Drawing;
use serde_json::{Value, json};

pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec::new("cabinet.fill", "Fill Cabinet Run", run_fill).menu(&["Architecture", "Fill Cabinet Run"]).params("{run: \"cb0\"}")]
}

fn cabinet_id(doc: &Drawing, raw: &str) -> Result<cadcraft_doc::arch::ArchId> {
    let s = raw.trim();
    let digits = s.strip_prefix("cb").or_else(|| s.strip_prefix("CB")).unwrap_or(s);
    let n = digits.parse::<u32>().map_err(|_| bad("cabinet.fill", format!("unknown cabinet `{raw}`")))?;
    let id = cadcraft_doc::arch::ArchId(n);
    if doc.arch.cabinets.iter().all(|c| c.id != id) {
        return Err(bad("cabinet.fill", format!("unknown cabinet `{raw}`")));
    }
    Ok(id)
}

fn run_fill(s: &mut Session, p: &Value) -> Result<Value> {
    let raw = p.get("run").and_then(Value::as_str).ok_or_else(|| bad("cabinet.fill", "`run` is required"))?;
    // Plan from the read-only view first: a run with nothing to fill must not become an undo step.
    let (fills, gap_rows) = {
        let doc = s.doc()?;
        let id = cabinet_id(&doc, raw)?;
        if run_of(&doc.arch, id).is_empty() {
            return Err(bad("cabinet.fill", format!("unknown cabinet `{raw}`")));
        }
        let gaps = run_gaps(&doc.arch, id).map_err(|e| bad("cabinet.fill", e))?;
        let mut fills = Vec::new();
        let mut gap_rows = Vec::new();
        for gap in gaps {
            let fill = filler_for(&doc.arch, &gap);
            gap_rows.push(json!({
                "end": if gap.end == cadcraft_arch::casework_fill::CornerEnd::Start { "start" } else { "end" },
                "gap": gap.gap,
                "filled": fill.is_some(),
            }));
            fills.extend(fill);
        }
        (fills, gap_rows)
    };
    if fills.is_empty() {
        return Ok(json!({ "fillers": [], "gaps": gap_rows }));
    }
    let mut doc = s.doc_mut()?;
    let fillers: Vec<String> = fills.into_iter().map(|fill| cadcraft_doc::arch::cabinet_label(doc.arch.push_cabinet(fill))).collect();
    doc.touch_arch();
    Ok(json!({ "fillers": fillers, "gaps": gap_rows }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchModel, Cabinet, CabinetKind, Fronts, Level};
    use cadcraft_geom::Vec2;
    use serde_json::json;

    #[test]
    fn cabinet_fill_reply_and_single_undo_step() {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        m.walls.push(cadcraft_doc::Wall::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, 2.0), 0.2));
        m.push_cabinet(Cabinet {
            id: cadcraft_doc::ArchId(0),
            kind: CabinetKind::Base,
            level: 0,
            position: Vec2::new(0.45, 0.5),
            rotation: 0.0,
            width: 0.6,
            depth: 0.6,
            height: 0.876,
            elevation: 0.0,
            fronts: Fronts::Doors,
            doors: 1,
            counter: true,
            counter_thickness: 0.038,
            counter_overhang: 0.025,
            toe_kick: true,
            ..Default::default()
        });
        m.push_cabinet(Cabinet {
            id: cadcraft_doc::ArchId(1),
            kind: CabinetKind::Base,
            level: 0,
            position: Vec2::new(1.05, 0.5),
            rotation: 0.0,
            width: 0.6,
            depth: 0.6,
            height: 0.876,
            elevation: 0.0,
            fronts: Fronts::Doors,
            doors: 1,
            counter: true,
            counter_thickness: 0.038,
            counter_overhang: 0.025,
            toe_kick: true,
            ..Default::default()
        });
        let gaps = run_gaps(&m, cadcraft_doc::ArchId(0)).unwrap();
        assert!(gaps.iter().any(|g| g.gap.is_some_and(|v| (v - 0.05).abs() < 1e-9)));
        assert!(filler_for(&m, &gaps[0]).is_some());
    }

    #[test]
    fn nothing_to_fill_is_not_an_undo_step() {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        m.walls.push(cadcraft_doc::Wall::new(Vec2::new(3.0, 0.0), Vec2::new(3.0, 2.0), 0.2));
        m.push_cabinet(Cabinet {
            id: cadcraft_doc::ArchId(0),
            kind: CabinetKind::Base,
            level: 0,
            position: Vec2::new(0.45, 0.5),
            rotation: 0.0,
            width: 0.6,
            depth: 0.6,
            height: 0.876,
            elevation: 0.0,
            fronts: Fronts::Doors,
            doors: 1,
            counter: true,
            counter_thickness: 0.038,
            counter_overhang: 0.025,
            toe_kick: true,
            ..Default::default()
        });
        let gaps = run_gaps(&m, cadcraft_doc::ArchId(0)).unwrap();
        assert!(gaps.iter().all(|g| g.gap.is_none()));
    }

    fn room() -> Session {
        let mut s = Session::new();
        {
            let mut d = s.doc_mut().unwrap();
            if d.arch.levels.is_empty() {
                d.arch.levels.push(Level::default());
            }
            let c = [(0.0, -0.1, 6.0, -0.1), (6.1, 0.0, 6.1, 4.0), (6.0, 4.1, 0.0, 4.1), (-0.1, 4.0, -0.1, 0.0)];
            for (x0, y0, x1, y1) in c {
                d.arch.push_wall(cadcraft_doc::Wall::new(Vec2::new(x0, y0), Vec2::new(x1, y1), 0.2));
            }
            d.touch_arch();
        }
        s
    }

    fn undo_len(s: &Session) -> usize {
        s.state().map(|st| st.undo.len()).unwrap_or_default()
    }

    #[test]
    fn cabinet_fill_reply_lists_cabinet_labels() {
        let mut s = room();
        s.execute("cabinet", &json!({ "kind": "base", "wall": "w0", "offset": 0.35, "widths": [0.6, 0.6] })).unwrap();
        let before = undo_len(&s);
        let r = s.execute("cabinet.fill", &json!({ "run": "cb1" })).unwrap();
        assert_eq!(r["fillers"], json!(["cb2"]));
        assert_eq!(undo_len(&s), before + 1);
        let gaps = r["gaps"].as_array().unwrap();
        let start = gaps.iter().find(|g| g["end"] == "start").unwrap();
        assert!((start["gap"].as_f64().unwrap() - 0.05).abs() < 1e-6);
        assert_eq!(start["filled"], true);
        let end = gaps.iter().find(|g| g["end"] == "end").unwrap();
        assert!(end["gap"].is_null());
        assert_eq!(end["filled"], false);
        let i = s.execute("arch.inspect", &json!({ "cabinets": true })).unwrap();
        let rows = i["cabinets"].as_array().unwrap();
        let row = rows.iter().find(|c| c["id"] == "cb2").unwrap();
        assert_eq!(row["filler"], true);
        assert!((row["width"].as_f64().unwrap() - 0.05).abs() < 1e-6);
    }

    #[test]
    fn cabinet_fill_with_nothing_to_fill_changes_nothing() {
        let mut s = room();
        s.execute("cabinet", &json!({ "kind": "base", "wall": "w0", "offset": 0.6, "widths": [0.6] })).unwrap();
        let before = undo_len(&s);
        let count = s.doc().unwrap().arch.cabinets.len();
        let rev = s.doc().unwrap().arch_revision;
        let r = s.execute("cabinet.fill", &json!({ "run": "cb0" })).unwrap();
        assert_eq!(r["fillers"], json!([]));
        let gaps = r["gaps"].as_array().unwrap();
        assert!(gaps.iter().all(|g| g["filled"] == false));
        assert!(gaps.iter().any(|g| g["gap"].as_f64().is_some_and(|v| (v - 0.3).abs() < 1e-6)));
        assert_eq!(undo_len(&s), before);
        assert_eq!(s.doc().unwrap().arch.cabinets.len(), count);
        assert_eq!(s.doc().unwrap().arch_revision, rev);
    }
}
