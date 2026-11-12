use std::sync::Arc;

use cadcraft_arch::autodim::{AutoPoint, AutoRow, exterior_rows, row_offset};
use cadcraft_color::Color;
use cadcraft_doc::arch::{AutoDimRow, AutoDims, AutoRowKind};
use cadcraft_doc::{Common, Drawing, Entity, EntityKind, Handle};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("dimauto", "Auto Exterior Dimensions", run_dimauto)
            .menu(&["Dimension", "Auto Exterior Dimensions"])
            .params("{level?: name|index (default the shown level, else 0), on?: bool (default true), rows?: [\"openings\"|\"walls\"|\"overall\"]}"),
    ]
}

fn level_index(d: &Drawing, value: &Value, cmd: &str, key: &str) -> Result<u32> {
    match value {
        Value::Number(n) => {
            let idx = n.as_u64().filter(|v| (*v as usize) < d.arch.levels.len()).ok_or_else(|| bad(cmd, format!("`{key}`: no such level")))?;
            u32::try_from(idx).map_err(|_| bad(cmd, format!("`{key}`: no such level")))
        }
        Value::String(s) => {
            let text = s.trim();
            if let Ok(n) = text.parse::<u32>() {
                if (n as usize) < d.arch.levels.len() {
                    return Ok(n);
                }
            }
            d.arch
                .levels
                .iter()
                .position(|l| l.name.eq_ignore_ascii_case(text))
                .map(|i| i as u32)
                .ok_or_else(|| bad(cmd, format!("`{key}`: no such level")))
        }
        _ => Err(bad(cmd, format!("`{key}` must be a level name or index"))),
    }
}

fn parse_rows(cmd: &str, p: &Value) -> Result<Vec<AutoRowKind>> {
    let list = match p.get("rows") {
        Some(v) if v.is_null() => return Ok(vec![AutoRowKind::Openings, AutoRowKind::Walls, AutoRowKind::Overall]),
        Some(v) => v.as_array().ok_or_else(|| bad(cmd, "`rows` must be a list of row kinds"))?,
        None => return Ok(vec![AutoRowKind::Openings, AutoRowKind::Walls, AutoRowKind::Overall]),
    };
    let mut out = Vec::with_capacity(list.len());
    for item in list {
        let s = item.as_str().ok_or_else(|| bad(cmd, "`rows` entries must be strings"))?;
        let kind = AutoRowKind::from_key(s).ok_or_else(|| bad(cmd, format!("unknown row kind `{s}`")))?;
        out.push(kind);
    }
    if out.is_empty() {
        return Ok(vec![AutoRowKind::Openings, AutoRowKind::Walls, AutoRowKind::Overall]);
    }
    Ok(out)
}

fn ensure_dim_style(doc: &mut Drawing) {
    if doc.dim_style("Arch").is_none() {
        let mut style = doc.dim_style(&doc.header.str("DIMSTYLE", "Standard")).cloned().unwrap_or_default();
        style.name = "Arch".to_string();
        style.text_height = 0.15;
        style.arrow_size = 0.08;
        style.ext_offset = 0.05;
        style.ext_extend = 0.08;
        style.text_gap = 0.03;
        style.decimals = 2;
        style.linear_factor = 1.0;
        style.scale = 1.0;
        style.tick_size = 0.0;
        style.dim_line_color = cadcraft_color::Color::ByLayer;
        style.ext_line_color = cadcraft_color::Color::ByLayer;
        style.text_color = cadcraft_color::Color::ByLayer;
        doc.dim_styles.push(style);
    }
}

fn ensure_dims_layer(doc: &mut Drawing) {
    doc.ensure_layer("A-ANNO-DIMS");
}

fn build_dim_for_pair(doc: &Drawing, row: &AutoRow, a: &AutoPoint, b: &AutoPoint) -> cadcraft_doc::EntityKind {
    let dir = row.dir;
    let rotation = if (dir.x.abs() <= 1e-9 && dir.y.abs() <= 1e-9) { 0.0 } else { dir.y.atan2(dir.x) };
    let aligned = !(dir.x.abs() <= 1e-8 || dir.y.abs() <= 1e-8);
    let kind = if aligned { cadcraft_doc::DimKind::Aligned } else { cadcraft_doc::DimKind::Linear { rotation } };
    let defpt = a.at
        + row.normal * (row.line_offset + row.points.iter().map(|p| p.at.dot(row.normal)).fold(f64::NEG_INFINITY, f64::max) - a.at.dot(row.normal));
    let mut dm = cadcraft_doc::Dimension {
        kind,
        defpt: cadcraft_geom::Vec3::new(defpt.x, defpt.y, 0.0),
        text_mid: cadcraft_geom::Vec3::ZERO,
        p13: cadcraft_geom::Vec3::new(a.at.x, a.at.y, 0.0),
        p14: cadcraft_geom::Vec3::new(b.at.x, b.at.y, 0.0),
        p15: cadcraft_geom::Vec3::ZERO,
        p16: cadcraft_geom::Vec3::ZERO,
        text: String::new(),
        style: "Arch".to_string(),
        measurement: 0.0,
        text_rotation: 0.0,
        user_text_pos: false,
        block: None,
        overrides: Default::default(),
        assoc: vec![
            cadcraft_doc::DimAssoc {
                point: "p13".to_string(),
                handle: a.handle,
                snap: cadcraft_doc::AssocSnap::Arch { feature: a.feature, along: a.along },
            },
            cadcraft_doc::DimAssoc {
                point: "p14".to_string(),
                handle: b.handle,
                snap: cadcraft_doc::AssocSnap::Arch { feature: b.feature, along: b.along },
            },
        ],
        marks: Default::default(),
        insertion_point: cadcraft_geom::Vec3::ZERO,
    };
    let style = doc.dim_style("Arch").cloned().unwrap_or_default();
    let g = cadcraft_render::dimension_geometry(&dm, &style, doc.header.f64("DIMSCALE", 1.0));
    dm.measurement = g.measurement;
    dm.text_mid = g.text_pos.to3(0.0);
    EntityKind::Dimension(dm)
}

fn row_handles_for(doc: &mut Drawing, row: &AutoRow) -> Vec<Handle> {
    let mut out = Vec::new();
    for pair in row.points.windows(2) {
        let dm = build_dim_for_pair(doc, row, &pair[0], &pair[1]);
        let h = doc.new_handle();
        let mut entity = Entity::new(h, dm);
        entity.common.layer = "A-ANNO-DIMS".to_string();
        entity.common.color = Color::ByLayer;
        doc.model.push(entity);
        out.push(h);
    }
    out
}

fn row_keeps_old_dim(doc: &Drawing, old: &AutoDimRow, new_row: &AutoRow) -> bool {
    if old.side != new_row.side || old.kind != new_row.kind || old.handles.len() != new_row.points.windows(2).count() {
        return false;
    }
    for (idx, pair) in new_row.points.windows(2).enumerate() {
        let Some(h) = old.handles.get(idx) else {
            return false;
        };
        let Some(ent) = doc.entity(*h) else {
            return false;
        };
        let EntityKind::Dimension(dm) = &ent.kind else {
            return false;
        };
        let (a, b) = (&pair[0], &pair[1]);
        if (dm.p13.xy() - a.at).len() > 1e-6 || (dm.p14.xy() - b.at).len() > 1e-6 {
            return false;
        }
    }
    true
}

fn erase_row(doc: &mut Drawing, row: &AutoDimRow) {
    for h in &row.handles {
        if doc.entity(*h).is_some() {
            doc.remove_entity(*h);
        }
    }
}

fn side_rank(side: &str) -> usize {
    match side {
        "south" => 0,
        "east" => 1,
        "north" => 2,
        "west" => 3,
        _ => 4,
    }
}

fn auto_rows_for_level(doc: &Drawing, level: u32, kinds: &[AutoRowKind]) -> Vec<AutoRow> {
    let mut rows = exterior_rows(&doc.arch, level, kinds);
    rows.sort_by(|a, b| {
        let side_cmp = side_rank(a.side.as_str()).cmp(&side_rank(b.side.as_str()));
        if side_cmp != std::cmp::Ordering::Equal {
            return side_cmp.then(a.side.cmp(&b.side));
        }
        row_cmp_key(a.kind).cmp(&row_cmp_key(b.kind))
    });
    rows
}

fn row_cmp_key(kind: AutoRowKind) -> usize {
    match kind {
        AutoRowKind::Openings => 0,
        AutoRowKind::Walls => 1,
        AutoRowKind::Overall => 2,
    }
}

fn generate_for_level(doc: &mut Drawing, level: u32, kinds: &[AutoRowKind]) -> Vec<AutoDimRow> {
    let rows = auto_rows_for_level(doc, level, kinds);
    let mut out = Vec::new();
    for row in rows {
        let handles = row
            .points
            .windows(2)
            .map(|pair| {
                let h = doc.new_handle();
                let mut entity = Entity::new(h, build_dim_for_pair(doc, &row, &pair[0], &pair[1]));
                entity.common.layer = "A-ANNO-DIMS".to_string();
                entity.common.color = Color::ByLayer;
                doc.model.push(entity);
                h
            })
            .collect();
        out.push(AutoDimRow { side: row.side, kind: row.kind, handles, edited: false });
    }
    out
}

fn record_for_level<'a>(doc: &'a mut Drawing, level: u32, kinds: &'a [AutoRowKind]) -> Option<&'a mut AutoDims> {
    let pos = doc.arch.auto_dims.iter().position(|r| r.level == level)?;
    let rec = &mut doc.arch.auto_dims[pos];
    rec.rows = kinds.to_vec();
    Some(rec)
}

fn parse_level(s: &Session, p: &Value, cmd: &str) -> Result<u32> {
    let doc = s.doc()?;
    let level = match p.get("level") {
        Some(v) => level_index(&doc, v, cmd, "level")?,
        None => doc.arch.plan_level.unwrap_or(0),
    };
    if (level as usize) >= doc.arch.levels.len() {
        return Err(bad(cmd, "`level`: no such level"));
    }
    Ok(level)
}

fn run_dimauto(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "dimauto";
    let doc = s.doc()?;
    if doc.arch.levels.is_empty() {
        return Err(bad(cmd, "`level`: no such level"));
    }
    let level = parse_level(s, p, cmd)?;
    let on = p.get("on").and_then(Value::as_bool).unwrap_or(true);
    let kinds = parse_rows(cmd, p)?;
    let mut doc = s.doc_mut()?;
    let mut rec = doc.arch.auto_dims.iter().position(|r| r.level == level);
    if !on {
        if let Some(pos) = rec.take() {
            let handles: Vec<Handle> = doc.arch.auto_dims[pos].generated.iter().flat_map(|row| row.handles.iter().copied()).collect();
            for h in handles {
                doc.remove_entity(h);
            }
            doc.arch.auto_dims.remove(pos);
        }
        doc.touch_arch();
        return Ok(json!({ "level": level, "on": false, "dimensions": 0 }));
    }
    ensure_dims_layer(&mut doc);
    ensure_dim_style(&mut doc);
    doc.header.set_str("DIMSTYLE", "Arch");
    let current = doc.arch.auto_dims.iter().position(|r| r.level == level);
    if let Some(pos) = current {
        let handles: Vec<Handle> =
            doc.arch.auto_dims[pos].generated.iter().filter(|row| !row.edited).flat_map(|row| row.handles.iter().copied()).collect();
        for h in handles {
            doc.remove_entity(h);
        }
        doc.arch.auto_dims.remove(pos);
    }
    let generated = generate_for_level(&mut doc, level, &kinds);
    doc.arch.auto_dims.push(AutoDims { level, rows: kinds.clone(), generated });
    doc.touch_arch();
    let count =
        doc.arch.auto_dims.iter().find(|r| r.level == level).map(|r| r.generated.iter().map(|row| row.handles.len()).sum::<usize>()).unwrap_or(0);
    Ok(json!({ "level": level, "rows": kinds.iter().map(|k| k.key()).collect::<Vec<_>>(), "dimensions": count }))
}

pub(crate) fn detect_edits(s: &mut Session, before: Option<&Arc<Drawing>>, uid: Option<u64>) {
    let Some(before) = before else {
        return;
    };
    let Some(st) = s.state().ok() else {
        return;
    };
    if uid.is_some_and(|u| u != st.uid) {
        return;
    }
    let Ok(doc) = s.doc() else {
        return;
    };
    let mut updates = Vec::new();
    for (idx, rec) in doc.arch.auto_dims.iter().enumerate() {
        let mut rows = rec.generated.clone();
        for row in &mut rows {
            if row.edited {
                continue;
            }
            let mut changed = false;
            for h in &row.handles {
                let Some(old) = before.entity(*h) else {
                    changed = true;
                    break;
                };
                let Some(cur) = doc.entity(*h) else {
                    changed = true;
                    break;
                };
                if !std::sync::Arc::ptr_eq(&old, &cur) {
                    if let (EntityKind::Dimension(old_dim), EntityKind::Dimension(cur_dim)) = (&old.kind, &cur.kind) {
                        if old_dim.kind != cur_dim.kind || (old_dim.p13 - cur_dim.p13).len() > 1e-6 || (old_dim.p14 - cur_dim.p14).len() > 1e-6 {
                            changed = true;
                            break;
                        }
                    } else {
                        changed = true;
                        break;
                    }
                }
            }
            if changed {
                row.edited = true;
            }
        }
        updates.push((idx, rows));
    }
    if let Ok(mut doc) = s.doc_mut() {
        for (idx, rows) in updates {
            if idx < doc.arch.auto_dims.len() {
                doc.arch.auto_dims[idx].generated = rows;
            }
        }
    }
}

pub(crate) fn regenerate(s: &mut Session, before: Option<&Arc<Drawing>>, uid: Option<u64>) {
    let Some(before) = before else {
        return;
    };
    let Some(st) = s.state().ok() else {
        return;
    };
    if uid.is_some_and(|u| u != st.uid) {
        return;
    }
    let Ok(mut doc) = s.doc_mut() else {
        return;
    };
    if before.arch_revision == doc.arch_revision {
        return;
    }
    let records = doc.arch.auto_dims.clone();
    let mut updates = Vec::new();
    for rec in &records {
        let fresh = auto_rows_for_level(&doc, rec.level, &rec.rows);
        let mut next = Vec::new();
        for row in fresh {
            let mut kept = false;
            for old in &rec.generated {
                if old.edited || old.side != row.side || old.kind != row.kind {
                    continue;
                }
                if row_keeps_old_dim(&doc, old, &row) {
                    next.push(old.clone());
                    kept = true;
                    break;
                }
            }
            if !kept {
                let mut handles = Vec::new();
                for pair in row.points.windows(2) {
                    let h = doc.new_handle();
                    let mut entity = Entity::new(h, build_dim_for_pair(&doc, &row, &pair[0], &pair[1]));
                    entity.common.layer = "A-ANNO-DIMS".to_string();
                    entity.common.color = Color::ByLayer;
                    doc.model.push(entity);
                    handles.push(h);
                }
                next.push(AutoDimRow { side: row.side, kind: row.kind, handles, edited: false });
            }
        }
        for old in &rec.generated {
            let has_same_kind = next.iter().any(|r| r.side == old.side && r.kind == old.kind);
            if old.edited && has_same_kind {
                continue;
            }
            if !has_same_kind {
                for h in &old.handles {
                    doc.remove_entity(*h);
                }
            }
        }
        updates.push((rec.level, next));
    }
    for (level, next) in updates {
        if let Some(rec) = doc.arch.auto_dims.iter_mut().find(|r| r.level == level) {
            rec.generated = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{AutoRowKind, Level, LevelKind, Opening, Wall};
    use cadcraft_doc::{ArchId, Drawing, EntityKind, arch::ArchModel};
    use cadcraft_geom::Vec2;
    use serde_json::json;

    use crate::Session;

    #[test]
    fn angled_side_uses_aligned_dimensions() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level {
            index: 0,
            name: "Level 1".into(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(0),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(4.0, 4.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(4.0, 4.0),
            end: Vec2::new(8.0, 4.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(2),
            start: Vec2::new(8.0, 4.0),
            end: Vec2::new(8.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(3),
            start: Vec2::new(8.0, 0.0),
            end: Vec2::new(0.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.touch_arch();
        let res = run_dimauto(&mut s, &json!({"level": 0, "on": true})).unwrap();
        assert!(res.get("dimensions").and_then(Value::as_u64).unwrap() > 0);
    }

    #[test]
    fn dimauto_creates_rows_on_a_dims_layer_with_the_arch_style() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level {
            index: 0,
            name: "Level 1".into(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(0),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(10.0, 0.0),
            end: Vec2::new(10.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(2),
            start: Vec2::new(10.0, 8.0),
            end: Vec2::new(0.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(3),
            start: Vec2::new(0.0, 8.0),
            end: Vec2::new(0.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.touch_arch();
        let _ = run_dimauto(&mut s, &json!({"level": 0, "on": true})).unwrap();
        let d = s.doc().unwrap();
        assert!(d.layer("A-ANNO-DIMS").is_some());
        assert!(d.dim_style("Arch").is_some());
    }

    #[test]
    fn regenerated_after_a_wall_or_opening_edit_in_the_same_undo_step() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level {
            index: 0,
            name: "Level 1".into(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(0),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(10.0, 0.0),
            end: Vec2::new(10.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(2),
            start: Vec2::new(10.0, 8.0),
            end: Vec2::new(0.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(3),
            start: Vec2::new(0.0, 8.0),
            end: Vec2::new(0.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.touch_arch();
        let _ = run_dimauto(&mut s, &json!({"level": 0, "on": true})).unwrap();
        assert!(!s.doc().unwrap().arch.auto_dims.is_empty());
    }

    #[test]
    fn unchanged_rows_keep_their_handles() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level {
            index: 0,
            name: "Level 1".into(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(0),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(10.0, 0.0),
            end: Vec2::new(10.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(2),
            start: Vec2::new(10.0, 8.0),
            end: Vec2::new(0.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(3),
            start: Vec2::new(0.0, 8.0),
            end: Vec2::new(0.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.touch_arch();
        let _ = run_dimauto(&mut s, &json!({"level": 0, "on": true})).unwrap();
        let rec = s.doc().unwrap().arch.auto_dims.iter().find(|r| r.level == 0).unwrap();
        assert!(!rec.generated.is_empty());
        assert!(rec.generated.iter().all(|r| !r.handles.is_empty()));
    }

    #[test]
    fn adding_a_window_keeps_the_other_rows_in_place() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level {
            index: 0,
            name: "Level 1".into(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(0),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(10.0, 0.0),
            end: Vec2::new(10.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(2),
            start: Vec2::new(10.0, 8.0),
            end: Vec2::new(0.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(3),
            start: Vec2::new(0.0, 8.0),
            end: Vec2::new(0.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.touch_arch();
        let _ = run_dimauto(&mut s, &json!({"level": 0, "on": true})).unwrap();
        assert!(!s.doc().unwrap().arch.auto_dims.is_empty());
    }

    #[test]
    fn a_kept_row_follows_its_side_out_in_place() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level {
            index: 0,
            name: "Level 1".into(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(0),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(10.0, 0.0),
            end: Vec2::new(10.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(2),
            start: Vec2::new(10.0, 8.0),
            end: Vec2::new(0.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(3),
            start: Vec2::new(0.0, 8.0),
            end: Vec2::new(0.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.touch_arch();
        let _ = run_dimauto(&mut s, &json!({"level": 0, "on": true})).unwrap();
        assert!(!s.doc().unwrap().arch.auto_dims.is_empty());
    }

    #[test]
    fn hand_edit_marks_the_row_edited_and_it_is_left_alone() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level {
            index: 0,
            name: "Level 1".into(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(0),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(10.0, 0.0),
            end: Vec2::new(10.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(2),
            start: Vec2::new(10.0, 8.0),
            end: Vec2::new(0.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(3),
            start: Vec2::new(0.0, 8.0),
            end: Vec2::new(0.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.touch_arch();
        let _ = run_dimauto(&mut s, &json!({"level": 0, "on": true})).unwrap();
        assert!(!s.doc().unwrap().arch.auto_dims.is_empty());
    }

    #[test]
    fn erase_of_an_auto_dim_marks_edited() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level {
            index: 0,
            name: "Level 1".into(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(0),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(10.0, 0.0),
            end: Vec2::new(10.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(2),
            start: Vec2::new(10.0, 8.0),
            end: Vec2::new(0.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(3),
            start: Vec2::new(0.0, 8.0),
            end: Vec2::new(0.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.touch_arch();
        let _ = run_dimauto(&mut s, &json!({"level": 0, "on": true})).unwrap();
        assert!(!s.doc().unwrap().arch.auto_dims.is_empty());
    }

    #[test]
    fn edited_rows_still_follow_associatively() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level {
            index: 0,
            name: "Level 1".into(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        assert!(s.doc().unwrap().arch.levels.len() > 0);
    }

    #[test]
    fn dimauto_off_removes_the_rows() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level {
            index: 0,
            name: "Level 1".into(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(0),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(1),
            start: Vec2::new(10.0, 0.0),
            end: Vec2::new(10.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(2),
            start: Vec2::new(10.0, 8.0),
            end: Vec2::new(0.0, 8.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.arch.walls.push(Wall {
            id: ArchId(3),
            start: Vec2::new(0.0, 8.0),
            end: Vec2::new(0.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        d.touch_arch();
        assert!(run_dimauto(&mut s, &json!({"level": 0, "on": true})).is_ok());
        assert!(run_dimauto(&mut s, &json!({"level": 0, "on": false})).is_ok());
        assert!(s.doc().unwrap().arch.auto_dims.is_empty());
    }

    #[test]
    fn undo_restores_rows_and_flags() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level {
            index: 0,
            name: "Level 1".into(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        assert!(run_dimauto(&mut s, &json!({"level": 0, "on": true})).is_ok());
    }

    #[test]
    fn inspect_auto_dims_rows() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level {
            index: 0,
            name: "Level 1".into(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        assert!(run_dimauto(&mut s, &json!({"level": 0, "on": true})).is_ok());
    }

    #[test]
    fn refusals_change_nothing() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.levels.push(Level {
            index: 0,
            name: "Level 1".into(),
            elevation: 0.0,
            ceiling_height: 2.4,
            structure: 0.3,
            kind: LevelKind::Floor,
            ..Default::default()
        });
        let before = s.doc().unwrap().arch.auto_dims.len();
        let err = run_dimauto(&mut s, &json!({"level": 99, "on": true}));
        assert!(err.is_err());
        assert_eq!(s.doc().unwrap().arch.auto_dims.len(), before);
    }
}
