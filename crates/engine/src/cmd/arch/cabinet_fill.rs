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
    let mut doc = s.doc_mut()?;
    let raw = p.get("run").and_then(Value::as_str).ok_or_else(|| bad("cabinet.fill", "`run` is required"))?;
    let id = cabinet_id(&doc, raw)?;
    let run = run_of(&doc.arch, id);
    if run.is_empty() {
        return Err(bad("cabinet.fill", format!("unknown cabinet `{raw}`")));
    }
    let gaps = run_gaps(&doc.arch, id).map_err(|e| bad("cabinet.fill", e))?;
    let mut fillers = Vec::new();
    let mut gap_rows = Vec::new();
    let mut did_any = false;
    for gap in gaps {
        let filled = match filler_for(&doc.arch, &gap) {
            Some(fill) => {
                let inserted = doc.arch.push_cabinet(fill);
                fillers.push(inserted);
                did_any = true;
                true
            }
            None => false,
        };
        gap_rows.push(json!({
            "end": if gap.end == cadcraft_arch::casework_fill::CornerEnd::Start { "start" } else { "end" },
            "gap": gap.gap,
            "filled": filled,
        }));
    }
    if !did_any {
        return Ok(json!({ "fillers": [], "gaps": gap_rows }));
    }
    Ok(json!({ "fillers": fillers, "gaps": gap_rows }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchModel, Cabinet, CabinetKind, Fronts, Level};
    use cadcraft_geom::Vec2;

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
}
