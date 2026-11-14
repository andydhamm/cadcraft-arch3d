//! `dimauto`: the exterior dimension rows of a level, kept up to date as the model changes.
//!
//! The rows come from `cadcraft_arch::autodim::exterior_rows`. `run_dimauto` and `regenerate`
//! share one reconcile routine: a row the user edited is left alone, a row whose attachments are
//! unchanged keeps its dimension handles and is updated in place, anything else is replaced.

use std::sync::Arc;

use cadcraft_arch::autodim::{AutoPoint, AutoRow, exterior_rows};
use cadcraft_color::Color;
use cadcraft_doc::arch::{AutoDimRow, AutoDims, AutoRowKind};
use cadcraft_doc::{AssocSnap, DimAssoc, DimKind, Dimension, Drawing, Entity, EntityKind, Handle};
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::{Result, Session};

/// The layer the rows are drawn on.
const DIMS_LAYER: &str = "A-ANNO-DIMS";
/// The dimension style of the rows.
const DIMS_STYLE: &str = "Arch";

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
            if let Ok(n) = text.parse::<u32>()
                && (n as usize) < d.arch.levels.len()
            {
                return Ok(n);
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

fn all_kinds() -> Vec<AutoRowKind> {
    vec![AutoRowKind::Openings, AutoRowKind::Walls, AutoRowKind::Overall]
}

fn parse_rows(cmd: &str, p: &Value) -> Result<Vec<AutoRowKind>> {
    let list = match p.get("rows") {
        Some(v) if v.is_null() => return Ok(all_kinds()),
        Some(v) => v.as_array().ok_or_else(|| bad(cmd, "`rows` must be a list of row kinds"))?,
        None => return Ok(all_kinds()),
    };
    let mut out = Vec::with_capacity(list.len());
    for item in list {
        let s = item.as_str().ok_or_else(|| bad(cmd, "`rows` entries must be strings"))?;
        let kind = AutoRowKind::from_key(s).ok_or_else(|| bad(cmd, format!("unknown row kind `{s}`")))?;
        out.push(kind);
    }
    if out.is_empty() {
        return Ok(all_kinds());
    }
    Ok(out)
}

/// Make the layer and the style of the rows. Whether anything was added.
fn ensure_layer_and_style(doc: &mut Drawing) -> bool {
    let mut changed = doc.ensure_layer(DIMS_LAYER);
    if doc.dim_style(DIMS_STYLE).is_none() {
        let mut style = doc.dim_style(&doc.header.str("DIMSTYLE", "Standard")).cloned().unwrap_or_default();
        style.name = DIMS_STYLE.to_string();
        style.text_height = 0.15;
        style.arrow_size = 0.08;
        style.ext_offset = 0.05;
        style.ext_extend = 0.08;
        style.text_gap = 0.03;
        style.decimals = 2;
        style.linear_factor = 1.0;
        style.scale = 1.0;
        style.tick_size = 0.0;
        style.dim_line_color = Color::ByLayer;
        style.ext_line_color = Color::ByLayer;
        style.text_color = Color::ByLayer;
        doc.dim_styles.push(style);
        changed = true;
    }
    changed
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

fn kind_rank(kind: AutoRowKind) -> usize {
    match kind {
        AutoRowKind::Openings => 0,
        AutoRowKind::Walls => 1,
        AutoRowKind::Overall => 2,
    }
}

/// The dimension between two consecutive points of a row.
fn build_dimension(doc: &Drawing, row: &AutoRow, a: &AutoPoint, b: &AutoPoint) -> Dimension {
    let rotation = if row.dir.x.abs() >= row.dir.y.abs() { 0.0 } else { std::f64::consts::FRAC_PI_2 };
    let defpt = row.dir * b.at.dot(row.dir) + row.normal * row.line_offset;
    let mut dm = Dimension {
        kind: DimKind::Linear { rotation },
        defpt: defpt.to3(0.0),
        text_mid: cadcraft_geom::Vec3::ZERO,
        p13: a.at.to3(0.0),
        p14: b.at.to3(0.0),
        p15: cadcraft_geom::Vec3::ZERO,
        p16: cadcraft_geom::Vec3::ZERO,
        text: String::new(),
        style: DIMS_STYLE.to_string(),
        measurement: 0.0,
        text_rotation: 0.0,
        user_text_pos: false,
        block: None,
        overrides: Default::default(),
        assoc: vec![
            DimAssoc { point: "p13".to_string(), handle: a.handle, snap: AssocSnap::Arch { feature: a.feature, along: a.along } },
            DimAssoc { point: "p14".to_string(), handle: b.handle, snap: AssocSnap::Arch { feature: b.feature, along: b.along } },
        ],
        marks: Default::default(),
        insertion_point: cadcraft_geom::Vec3::ZERO,
    };
    let style = doc.dim_style(DIMS_STYLE).cloned().unwrap_or_default();
    let g = cadcraft_render::dimension_geometry(&dm, &style, doc.header.f64("DIMSCALE", 1.0));
    dm.measurement = g.measurement;
    dm.text_mid = g.text_pos.to3(0.0);
    dm
}

fn row_dimensions(doc: &Drawing, row: &AutoRow) -> Vec<Dimension> {
    row.points.windows(2).map(|p| build_dimension(doc, row, &p[0], &p[1])).collect()
}

fn add_dimension(doc: &mut Drawing, dm: Dimension) -> Handle {
    let h = doc.new_handle();
    let mut entity = Entity::new(h, EntityKind::Dimension(dm));
    entity.common.layer = DIMS_LAYER.to_string();
    entity.common.color = Color::ByLayer;
    doc.model.push(entity);
    h
}

fn erase(doc: &mut Drawing, handles: &[Handle]) -> bool {
    let mut any = false;
    for h in handles {
        any |= doc.remove_entity(*h).is_some();
    }
    any
}

fn attach_of(dm: &Dimension, point: &str) -> Option<(Handle, cadcraft_doc::ArchFeature)> {
    dm.assoc.iter().find(|a| a.point == point).and_then(|a| match a.snap {
        AssocSnap::Arch { feature, .. } => Some((a.handle, feature)),
        _ => None,
    })
}

/// Whether the old row's dimensions can stand for the fresh ones: the same count, all still
/// dimensions, attached to the same objects and features at both ends.
fn same_attachments(doc: &Drawing, old: &AutoDimRow, fresh: &[Dimension]) -> bool {
    old.handles.len() == fresh.len()
        && old.handles.iter().zip(fresh).all(|(h, f)| {
            let Some(EntityKind::Dimension(cur)) = doc.entity(*h).map(|e| &e.kind) else {
                return false;
            };
            attach_of(cur, "p13") == attach_of(f, "p13") && attach_of(cur, "p14") == attach_of(f, "p14")
        })
}

fn alongs(dm: &Dimension) -> Vec<f64> {
    dm.assoc
        .iter()
        .map(|a| match a.snap {
            AssocSnap::Arch { along, .. } => along,
            _ => f64::NAN,
        })
        .collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12
}

fn near3(a: cadcraft_geom::Vec3, b: cadcraft_geom::Vec3) -> bool {
    near(a.x, b.x) && near(a.y, b.y) && near(a.z, b.z)
}

/// Update a dimension in place to the fresh one. Whether anything changed.
fn update_in_place(doc: &mut Drawing, h: Handle, fresh: &Dimension) -> bool {
    let Some(entity) = doc.entity(h).cloned() else {
        return false;
    };
    let EntityKind::Dimension(cur) = &entity.kind else {
        return false;
    };
    let (ca, fa) = (alongs(cur), alongs(fresh));
    let same = near3(cur.p13, fresh.p13)
        && near3(cur.p14, fresh.p14)
        && near3(cur.defpt, fresh.defpt)
        && near(cur.measurement, fresh.measurement)
        && ca.len() == fa.len()
        && ca.iter().zip(&fa).all(|(x, y)| near(*x, *y));
    if same {
        return false;
    }
    let mut dm = cur.clone();
    dm.p13 = fresh.p13;
    dm.p14 = fresh.p14;
    dm.defpt = fresh.defpt;
    dm.assoc = fresh.assoc.clone();
    dm.measurement = fresh.measurement;
    dm.text_mid = fresh.text_mid;
    let mut e = (*entity).clone();
    e.kind = EntityKind::Dimension(dm);
    doc.model.replace(e)
}

/// Bring the rows of `level` up to date with the model. Returns the new row list and whether
/// the drawing (entities) changed; the caller compares the list with the old one.
fn reconcile(doc: &mut Drawing, level: u32, kinds: &[AutoRowKind], old: &[AutoDimRow]) -> (Vec<AutoDimRow>, bool) {
    let mut changed = false;
    let mut out: Vec<AutoDimRow> = old.iter().filter(|r| r.edited).cloned().collect();
    let mut used = vec![false; old.len()];
    for row in exterior_rows(&doc.arch, level, kinds) {
        if old.iter().any(|o| o.edited && o.side == row.side && o.kind == row.kind) {
            continue;
        }
        let fresh = row_dimensions(doc, &row);
        let pos = old.iter().position(|o| !o.edited && o.side == row.side && o.kind == row.kind);
        if let Some(i) = pos
            && let Some(o) = old.get(i)
            && same_attachments(doc, o, &fresh)
        {
            if let Some(u) = used.get_mut(i) {
                *u = true;
            }
            for (h, f) in o.handles.iter().zip(&fresh) {
                changed |= update_in_place(doc, *h, f);
            }
            out.push(o.clone());
            continue;
        }
        let handles: Vec<Handle> = fresh.into_iter().map(|dm| add_dimension(doc, dm)).collect();
        changed = true;
        out.push(AutoDimRow { side: row.side, kind: row.kind, handles, ..Default::default() });
    }
    for (o, used) in old.iter().zip(&used) {
        if !o.edited && !used {
            changed |= erase(doc, &o.handles);
        }
    }
    out.sort_by_key(|r| (side_rank(&r.side), kind_rank(r.kind)));
    (out, changed)
}

fn parse_level(s: &Session, p: &Value, cmd: &str) -> Result<u32> {
    let doc = s.doc()?;
    let level = match p.get("level") {
        Some(v) => level_index(doc, v, cmd, "level")?,
        None => doc.arch.plan_level.unwrap_or(0),
    };
    if (level as usize) >= doc.arch.levels.len() {
        return Err(bad(cmd, "`level`: no such level"));
    }
    Ok(level)
}

fn reply(doc: &Drawing, level: u32) -> Value {
    let rows = doc.arch.auto_dims.iter().find(|r| r.level == level).map(|r| r.generated.as_slice()).unwrap_or(&[]);
    let style = doc.dim_style(DIMS_STYLE).cloned().unwrap_or_default();
    let mut count = 0usize;
    let list: Vec<Value> = rows
        .iter()
        .map(|row| {
            let values: Vec<f64> = row
                .handles
                .iter()
                .filter_map(|h| match doc.entity(*h).map(|e| &e.kind) {
                    Some(EntityKind::Dimension(dm)) => {
                        Some(cadcraft_render::dimension_geometry(dm, &style, doc.header.f64("DIMSCALE", 1.0)).measurement)
                    }
                    _ => None,
                })
                .collect();
            count += values.len();
            json!({ "side": row.side, "row": row.kind.key(), "values": values })
        })
        .collect();
    json!({ "level": level, "rows": list, "dimensions": count })
}

fn run_dimauto(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "dimauto";
    if s.doc()?.arch.levels.is_empty() {
        return Err(bad(cmd, "`level`: no such level"));
    }
    let level = parse_level(s, p, cmd)?;
    let on = p.get("on").and_then(Value::as_bool).unwrap_or(true);
    let kinds = parse_rows(cmd, p)?;
    let mut work: Drawing = s.doc()?.clone();
    let mut changed = false;
    if !on {
        if let Some(pos) = work.arch.auto_dims.iter().position(|r| r.level == level) {
            let rec = work.arch.auto_dims.remove(pos);
            let handles: Vec<Handle> = rec.generated.iter().flat_map(|r| r.handles.iter().copied()).collect();
            erase(&mut work, &handles);
            changed = true;
        }
        if changed {
            work.touch_arch();
            *s.doc_mut()? = work;
        }
        return Ok(json!({ "level": level, "on": false, "rows": [], "dimensions": 0 }));
    }
    changed |= ensure_layer_and_style(&mut work);
    if work.header.str("DIMSTYLE", "Standard") != DIMS_STYLE {
        work.header.set_str("DIMSTYLE", DIMS_STYLE);
        changed = true;
    }
    let pos = work.arch.auto_dims.iter().position(|r| r.level == level);
    let old = pos.and_then(|i| work.arch.auto_dims.get(i)).map(|r| r.generated.clone()).unwrap_or_default();
    let (generated, entities_changed) = reconcile(&mut work, level, &kinds, &old);
    changed |= entities_changed || generated != old || pos.is_none();
    match pos.and_then(|i| work.arch.auto_dims.get_mut(i)) {
        Some(rec) => {
            changed |= rec.rows != kinds;
            rec.rows = kinds;
            rec.generated = generated;
        }
        None => work.arch.auto_dims.push(AutoDims { level, rows: kinds, generated, ..Default::default() }),
    }
    let out = reply(&work, level);
    if changed {
        work.touch_arch();
        *s.doc_mut()? = work;
    }
    Ok(out)
}

/// After a command: mark the rows the user's own command changed as edited.
pub(crate) fn detect_edits(s: &mut Session, before: Option<&Arc<Drawing>>, uid: Option<u64>) {
    let Some(before) = before else {
        return;
    };
    let Ok(st) = s.state() else {
        return;
    };
    if uid.is_some_and(|u| u != st.uid) {
        return;
    }
    let doc = &st.doc;
    if before.arch_revision != doc.arch_revision {
        return;
    }
    let mut marks: Vec<(usize, usize)> = Vec::new();
    for (ri, rec) in doc.arch.auto_dims.iter().enumerate() {
        let Some(old_rec) = before.arch.auto_dims.iter().find(|r| r.level == rec.level) else {
            continue;
        };
        for (i, row) in rec.generated.iter().enumerate() {
            if row.edited || !old_rec.generated.iter().any(|o| !o.edited && o.side == row.side && o.kind == row.kind && o.handles == row.handles) {
                continue;
            }
            let edited = row.handles.iter().any(|h| {
                let Some(was) = before.entity(*h) else {
                    return false;
                };
                match doc.entity(*h) {
                    None => true,
                    Some(now) => !Arc::ptr_eq(was, now) && (was.kind != now.kind || was.common.layer != now.common.layer),
                }
            });
            if edited {
                marks.push((ri, i));
            }
        }
    }
    if marks.is_empty() {
        return;
    }
    if let Ok(doc) = s.doc_mut() {
        for (ri, i) in marks {
            if let Some(row) = doc.arch.auto_dims.get_mut(ri).and_then(|r| r.generated.get_mut(i)) {
                row.edited = true;
            }
        }
    }
}

/// After a command: reconcile every record with the model, inside the command's own undo step.
pub(crate) fn regenerate(s: &mut Session, before: Option<&Arc<Drawing>>, uid: Option<u64>) {
    let Some(before) = before else {
        return;
    };
    let Ok(st) = s.state() else {
        return;
    };
    if uid.is_some_and(|u| u != st.uid) {
        return;
    }
    let current = &st.doc;
    if current.arch.auto_dims.is_empty() || before.arch_revision == current.arch_revision {
        return;
    }
    // Work on a copy so an unchanged model keeps the same Arc and no undo step of its own.
    let mut work: Drawing = (**current).clone();
    let mut changed = ensure_layer_and_style(&mut work);
    for idx in 0..work.arch.auto_dims.len() {
        let Some(rec) = work.arch.auto_dims.get(idx) else {
            break;
        };
        let (level, kinds, old) = (rec.level, rec.rows.clone(), rec.generated.clone());
        let (generated, c) = reconcile(&mut work, level, &kinds, &old);
        changed |= c || generated != old;
        if let Some(rec) = work.arch.auto_dims.get_mut(idx) {
            rec.generated = generated;
        }
    }
    if changed && let Ok(live) = s.doc_mut() {
        *live = work;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{arch_handle, opening_handle};

    /// The phase-1 house built through the commands: w0..w3 exterior, w4 at y = 3, w5 at x = 6.
    fn house() -> Session {
        let mut s = Session::new();
        s.execute("wall", &json!({"points":[[0,0],[10,0],[10,8],[0,8],[0,0]],"thickness":0.2,"height":2.4,"exterior":true})).unwrap();
        s.execute("wall", &json!({"points":[[0,3],[10,3]],"thickness":0.1,"height":2.4,"type":"interior"})).unwrap();
        s.execute("wall", &json!({"points":[[6,3],[6,8]],"thickness":0.1,"height":2.4,"type":"interior"})).unwrap();
        s
    }

    fn record(s: &Session, level: u32) -> AutoDims {
        s.doc().unwrap().arch.auto_dims.iter().find(|r| r.level == level).cloned().unwrap_or_default()
    }

    fn row(s: &Session, level: u32, side: &str, kind: AutoRowKind) -> Option<AutoDimRow> {
        record(s, level).generated.into_iter().find(|r| r.side == side && r.kind == kind)
    }

    fn dim(s: &Session, h: Handle) -> Dimension {
        match s.doc().unwrap().entity(h).map(|e| e.kind.clone()) {
            Some(EntityKind::Dimension(d)) => d,
            other => panic!("not a dimension: {other:?}"),
        }
    }

    fn measure(s: &Session, h: Handle) -> f64 {
        let d = s.doc().unwrap();
        let style = d.dim_style(DIMS_STYLE).cloned().unwrap_or_default();
        cadcraft_render::dimension_geometry(&dim(s, h), &style, d.header.f64("DIMSCALE", 1.0)).measurement
    }

    fn values(s: &Session, level: u32, side: &str, kind: AutoRowKind) -> Vec<f64> {
        row(s, level, side, kind).map(|r| r.handles.iter().map(|h| measure(s, *h)).collect()).unwrap_or_default()
    }

    fn close(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
    }

    fn all_handles(s: &Session, level: u32) -> Vec<Handle> {
        record(s, level).generated.iter().flat_map(|r| r.handles.clone()).collect()
    }

    #[test]
    fn dimauto_house_values_and_lines() {
        let mut s = house();
        let r = s.execute("dimauto", &json!({})).unwrap();
        assert_eq!(r["level"], 0);
        assert_eq!(r["dimensions"], 10);
        let rows = r["rows"].as_array().unwrap();
        let order: Vec<String> = rows.iter().map(|v| format!("{}/{}", v["side"].as_str().unwrap(), v["row"].as_str().unwrap())).collect();
        assert_eq!(order, ["south/overall", "east/walls", "east/overall", "north/walls", "north/overall", "west/walls", "west/overall"]);
        assert!(close(&values(&s, 0, "south", AutoRowKind::Overall), &[10.2]));
        assert!(close(&values(&s, 0, "east", AutoRowKind::Overall), &[8.2]));
        assert!(close(&values(&s, 0, "east", AutoRowKind::Walls), &[3.1, 5.1]));
        assert!(close(&values(&s, 0, "west", AutoRowKind::Walls), &[3.1, 5.1]));
        assert!(close(&values(&s, 0, "north", AutoRowKind::Walls), &[6.1, 4.1]));
        assert!(row(&s, 0, "south", AutoRowKind::Walls).is_none());
        let d = s.doc().unwrap();
        assert!(d.dim_style("Arch").is_some());
        for h in all_handles(&s, 0) {
            let e = d.entity(h).unwrap();
            assert_eq!(e.common.layer, "A-ANNO-DIMS");
            let dm = dim(&s, h);
            assert_eq!(dm.style, "Arch");
            assert_eq!(dm.assoc.len(), 2);
            assert!(dm.assoc.iter().all(|a| matches!(a.snap, AssocSnap::Arch { .. })));
        }
        // Lines: south overall at y = -2.1, east overall at x = 12.1, north walls at y = 9.6.
        let so = dim(&s, row(&s, 0, "south", AutoRowKind::Overall).unwrap().handles[0]);
        assert!((so.defpt.y + 2.1).abs() < 1e-9 && (so.defpt.x - 10.1).abs() < 1e-9);
        assert_eq!(so.kind, DimKind::Linear { rotation: 0.0 });
        let eo = dim(&s, row(&s, 0, "east", AutoRowKind::Overall).unwrap().handles[0]);
        assert!((eo.defpt.x - 12.1).abs() < 1e-9 && (eo.defpt.y - 8.1).abs() < 1e-9);
        assert_eq!(eo.kind, DimKind::Linear { rotation: std::f64::consts::FRAC_PI_2 });
        let nw = dim(&s, row(&s, 0, "north", AutoRowKind::Walls).unwrap().handles[0]);
        assert!((nw.defpt.y - 9.6).abs() < 1e-9);
        // Refusals change nothing.
        let before = s.doc().unwrap().clone();
        assert!(s.execute("dimauto", &json!({"level": 5})).is_err());
        assert!(s.execute("dimauto", &json!({"rows": ["bogus"]})).is_err());
        assert!(s.doc().unwrap().arch.auto_dims == before.arch.auto_dims);
    }

    #[test]
    fn fresh_rows_are_not_edited() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        assert!(record(&s, 0).generated.iter().all(|r| !r.edited));
        // Another command and a model edit leave them unedited too.
        s.execute("line", &json!({"points": [[0, 20], [1, 20]]})).unwrap();
        s.execute("dimauto", &json!({})).unwrap();
        assert!(record(&s, 0).generated.iter().all(|r| !r.edited));
        // A second dimauto changes nothing: same handles, no new undo step.
        let before = all_handles(&s, 0);
        let undo = s.state().unwrap().undo.len();
        s.execute("dimauto", &json!({})).unwrap();
        assert_eq!(all_handles(&s, 0), before);
        assert_eq!(s.state().unwrap().undo.len(), undo);
    }

    #[test]
    fn adding_a_window_keeps_walls_and_overall_handles() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let walls = row(&s, 0, "north", AutoRowKind::Walls).unwrap();
        let overall = row(&s, 0, "north", AutoRowKind::Overall).unwrap();
        let south = row(&s, 0, "south", AutoRowKind::Overall).unwrap();
        assert!(row(&s, 0, "north", AutoRowKind::Openings).is_none());
        s.execute("window", &json!({"wall": "w2", "offset": 6.0, "width": 1.0, "height": 1.2, "sill": 0.9})).unwrap();
        assert_eq!(row(&s, 0, "north", AutoRowKind::Walls).unwrap().handles, walls.handles);
        assert_eq!(row(&s, 0, "north", AutoRowKind::Overall).unwrap().handles, overall.handles);
        assert_eq!(row(&s, 0, "south", AutoRowKind::Overall).unwrap().handles, south.handles);
        let o = row(&s, 0, "north", AutoRowKind::Openings).expect("openings row");
        assert!(close(&values(&s, 0, "north", AutoRowKind::Openings), &[4.1, 2.0, 4.1]));
        let first = dim(&s, o.handles[0]);
        assert_eq!(first.assoc[1].handle, opening_handle(ArchId(0)));
        assert!((first.defpt.y - 9.1).abs() < 1e-9);
        assert!(record(&s, 0).generated.iter().all(|r| !r.edited));
        // Order: openings, walls, overall within the side.
        let sides: Vec<_> = record(&s, 0).generated.iter().filter(|r| r.side == "north").map(|r| r.kind).collect();
        assert_eq!(sides, [AutoRowKind::Openings, AutoRowKind::Walls, AutoRowKind::Overall]);
        // One undo takes the window and its row back together.
        s.undo().unwrap();
        assert!(row(&s, 0, "north", AutoRowKind::Openings).is_none());
    }

    #[test]
    fn moving_w5_rereads_5_6_4_6_and_undo_restores_6_1_4_1_unedited() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let north = row(&s, 0, "north", AutoRowKind::Walls).unwrap();
        s.execute("dimdrive", &json!({"dimension": north.handles[0].hex(), "value": 5.6, "object": "w5"})).unwrap();
        assert!(close(&values(&s, 0, "north", AutoRowKind::Walls), &[5.6, 4.6]));
        let after = row(&s, 0, "north", AutoRowKind::Walls).unwrap();
        assert_eq!(after.handles, north.handles, "the dimensions stay in place");
        assert!(!after.edited);
        assert!(record(&s, 0).generated.iter().all(|r| !r.edited));
        s.undo().unwrap();
        assert!(close(&values(&s, 0, "north", AutoRowKind::Walls), &[6.1, 4.1]));
        assert!(record(&s, 0).generated.iter().all(|r| !r.edited));
        s.redo().unwrap();
        assert!(close(&values(&s, 0, "north", AutoRowKind::Walls), &[5.6, 4.6]));
        assert!(record(&s, 0).generated.iter().all(|r| !r.edited));
    }

    #[test]
    fn dimedit_marks_row_edited_and_it_is_left_alone() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let north = row(&s, 0, "north", AutoRowKind::Walls).unwrap();
        let h = north.handles[0];
        s.execute("dimedit", &json!({"handles": [h.hex()], "mode": "new", "text": "SEE PLAN"})).unwrap();
        assert!(row(&s, 0, "north", AutoRowKind::Walls).unwrap().edited);
        assert!(!row(&s, 0, "north", AutoRowKind::Overall).unwrap().edited);
        assert!(!row(&s, 0, "east", AutoRowKind::Walls).unwrap().edited);
        // Move w5: the edited row is not touched, the others follow.
        let overall = row(&s, 0, "east", AutoRowKind::Walls).unwrap();
        s.execute("dimdrive", &json!({"dimension": overall.handles[0].hex(), "value": 3.6, "object": "w4"})).unwrap();
        assert_eq!(dim(&s, h).text, "SEE PLAN");
        let still = row(&s, 0, "north", AutoRowKind::Walls).unwrap();
        assert!(still.edited);
        assert_eq!(still.handles, north.handles);
        // A second dimauto keeps the edited row.
        s.execute("dimauto", &json!({})).unwrap();
        let again = row(&s, 0, "north", AutoRowKind::Walls).unwrap();
        assert!(again.edited);
        assert_eq!(again.handles, north.handles);
        assert_eq!(dim(&s, h).text, "SEE PLAN");
        // Erase marks too.
        let g = row(&s, 0, "west", AutoRowKind::Overall).unwrap().handles[0];
        s.execute("erase", &json!({"handles": [g.hex()]})).unwrap();
        assert!(row(&s, 0, "west", AutoRowKind::Overall).unwrap().edited);
        // Undo of the edit unmarks it (the whole drawing comes back).
        s.undo().unwrap();
        assert!(!row(&s, 0, "west", AutoRowKind::Overall).unwrap().edited);
    }

    #[test]
    fn dimauto_off_removes_rows_and_dimensions() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let handles = all_handles(&s, 0);
        assert_eq!(handles.len(), 10);
        let h = row(&s, 0, "north", AutoRowKind::Walls).unwrap().handles[0];
        s.execute("dimedit", &json!({"handles": [h.hex()], "mode": "new", "text": "X"})).unwrap();
        let r = s.execute("dimauto", &json!({"on": false})).unwrap();
        assert_eq!(r, json!({"level": 0, "on": false, "rows": [], "dimensions": 0}));
        assert!(record(&s, 0).generated.is_empty());
        assert!(s.doc().unwrap().arch.auto_dims.is_empty());
        let d = s.doc().unwrap();
        assert!(handles.iter().all(|h| d.entity(*h).is_none()), "edited rows go too");
        // Off again changes nothing.
        let undo = s.state().unwrap().undo.len();
        s.execute("dimauto", &json!({"on": false})).unwrap();
        assert_eq!(s.state().unwrap().undo.len(), undo);
        // And it comes back.
        s.execute("dimauto", &json!({})).unwrap();
        assert_eq!(all_handles(&s, 0).len(), 10);
        assert!(s.execute("dimauto", &json!({"on": false, "level": 9})).is_err());
    }

    #[test]
    fn second_level_has_its_own_rows_and_level0_keeps_handles() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let before = all_handles(&s, 0);
        s.execute("level", &json!({"above": 0})).unwrap();
        let n = s.doc().unwrap().arch.levels.len();
        assert!(n >= 2, "the level command adds a level, have {n}");
        s.execute("wall", &json!({"points":[[0,0],[6,0],[6,5],[0,5],[0,0]],"thickness":0.2,"height":2.4,"exterior":true,"level":1})).unwrap();
        let r = s.execute("dimauto", &json!({"level": 1})).unwrap();
        assert_eq!(r["level"], 1);
        assert!(close(&values(&s, 1, "south", AutoRowKind::Overall), &[6.2]));
        assert!(close(&values(&s, 1, "east", AutoRowKind::Overall), &[5.2]));
        assert!(record(&s, 1).generated.iter().all(|r| !r.edited));
        assert_eq!(all_handles(&s, 0), before);
        assert!(close(&values(&s, 0, "south", AutoRowKind::Overall), &[10.2]));
        // Turning level 1 off leaves level 0 alone.
        s.execute("dimauto", &json!({"level": 1, "on": false})).unwrap();
        assert_eq!(all_handles(&s, 0), before);
        assert!(record(&s, 1).generated.is_empty());
    }

    #[test]
    fn dimdrive_north_walls_dimension_moves_w5_to_5_5() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let north = row(&s, 0, "north", AutoRowKind::Walls).unwrap();
        let r = s.execute("dimdrive", &json!({"dimension": north.handles[0].hex(), "value": 5.6, "object": "w5"})).unwrap();
        assert!((r["by"][0].as_f64().unwrap() + 0.5).abs() < 1e-9, "{r}");
        let w5 = s.doc().unwrap().arch.wall(ArchId(5)).unwrap().clone();
        assert!((w5.start.x - 5.5).abs() < 1e-9 && (w5.end.x - 5.5).abs() < 1e-9);
        assert_eq!(dim(&s, north.handles[0]).assoc[1].handle, arch_handle(ArchId(5)));
        assert!(close(&values(&s, 0, "north", AutoRowKind::Walls), &[5.6, 4.6]));
    }

    #[test]
    fn rows_survive_a_dxf_round_trip() {
        let mut s = house();
        s.execute("window", &json!({"wall": "w2", "offset": 6.0, "width": 1.0, "height": 1.2, "sill": 0.9})).unwrap();
        s.execute("dimauto", &json!({})).unwrap();
        let h = row(&s, 0, "east", AutoRowKind::Overall).unwrap().handles[0];
        s.execute("dimedit", &json!({"handles": [h.hex()], "mode": "new", "text": "KEEP"})).unwrap();
        let original = s.doc().unwrap().arch.auto_dims.clone();
        assert!(original[0].generated.iter().any(|r| r.edited));
        let bytes = cadcraft_io::write_dxf(s.doc().unwrap());
        let back = cadcraft_io::read_dxf(bytes.as_bytes()).unwrap();
        assert_eq!(back.arch.auto_dims, original);
        for h in original.iter().flat_map(|r| r.generated.iter()).flat_map(|r| r.handles.iter()) {
            let e = back.entity(*h).unwrap_or_else(|| panic!("dimension {} lost", h.hex()));
            assert_eq!(e.common.layer, "A-ANNO-DIMS");
            let EntityKind::Dimension(dm) = &e.kind else { panic!("not a dimension") };
            assert_eq!(dm.style, "Arch");
            assert_eq!(dm.assoc.len(), 2);
            assert!(dm.assoc.iter().all(|a| matches!(a.snap, AssocSnap::Arch { .. })));
        }
    }

    // Tests kept from the first implementation, with assertions matching the rebuilt command.

    #[test]
    fn angled_side_uses_aligned_dimensions() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points":[[0,0],[4,4],[8,4],[8,0],[0,0]],"thickness":0.2,"height":2.4,"exterior":true})).unwrap();
        let r = s.execute("dimauto", &json!({"level": 0, "on": true})).unwrap();
        assert!(r["dimensions"].as_u64().unwrap() > 0, "{r}");
        for h in all_handles(&s, 0) {
            let dm = dim(&s, h);
            assert!(matches!(dm.kind, DimKind::Linear { .. }));
            assert_eq!(dm.assoc.len(), 2);
        }
    }

    #[test]
    fn dimauto_creates_rows_on_a_dims_layer_with_the_arch_style() {
        let mut s = house();
        s.execute("dimauto", &json!({"level": 0, "on": true})).unwrap();
        let d = s.doc().unwrap();
        assert!(d.layer("A-ANNO-DIMS").is_some());
        assert!(d.dim_style("Arch").is_some());
        for h in all_handles(&s, 0) {
            assert_eq!(d.entity(h).unwrap().common.layer, "A-ANNO-DIMS");
            assert_eq!(dim(&s, h).style, "Arch");
        }
    }

    #[test]
    fn regenerated_after_a_wall_or_opening_edit_in_the_same_undo_step() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let undo = s.state().unwrap().undo.len();
        s.execute("window", &json!({"wall": "w2", "offset": 6.0, "width": 1.0, "height": 1.2, "sill": 0.9})).unwrap();
        assert_eq!(s.state().unwrap().undo.len(), undo + 1, "one step for the window and the rows");
        assert!(row(&s, 0, "north", AutoRowKind::Openings).is_some());
        s.undo().unwrap();
        assert!(row(&s, 0, "north", AutoRowKind::Openings).is_none());
        assert!(s.doc().unwrap().arch.openings.is_empty());
    }

    #[test]
    fn unchanged_rows_keep_their_handles() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let before = all_handles(&s, 0);
        assert!(!before.is_empty());
        s.execute("dimauto", &json!({"rows": ["overall", "walls", "openings"]})).unwrap();
        assert_eq!(all_handles(&s, 0), before);
        assert!(record(&s, 0).generated.iter().all(|r| !r.handles.is_empty()));
    }

    #[test]
    fn adding_a_window_keeps_the_other_rows_in_place() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let before: Vec<_> = record(&s, 0).generated.iter().map(|r| (r.side.clone(), r.kind, r.handles.clone())).collect();
        s.execute("window", &json!({"wall": "w2", "offset": 6.0, "width": 1.0, "height": 1.2, "sill": 0.9})).unwrap();
        for (side, kind, handles) in before {
            assert_eq!(row(&s, 0, &side, kind).unwrap().handles, handles, "{side} {kind:?}");
        }
        assert_eq!(record(&s, 0).generated.len(), 8);
    }

    #[test]
    fn a_kept_row_follows_its_side_out_in_place() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let east = row(&s, 0, "east", AutoRowKind::Overall).unwrap();
        let line = |s: &Session| dim(s, east.handles[0]).defpt.x;
        assert!((line(&s) - 12.1).abs() < 1e-9);
        // Widen the house: w1 (the east wall) moves 2 m out.
        let north = row(&s, 0, "north", AutoRowKind::Overall).unwrap();
        s.execute("dimdrive", &json!({"dimension": north.handles[0].hex(), "value": 12.2, "object": "w1"})).unwrap();
        let moved = s.doc().unwrap().arch.wall(ArchId(1)).map(|w| w.start.x).unwrap_or(10.0);
        assert!(moved > 10.5, "w1 moved to {moved}");
        assert_eq!(row(&s, 0, "east", AutoRowKind::Overall).unwrap().handles, east.handles);
        assert!((line(&s) - (12.1 + (moved - 10.0))).abs() < 1e-9, "{}", line(&s));
    }

    #[test]
    fn hand_edit_marks_the_row_edited_and_it_is_left_alone() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let h = row(&s, 0, "south", AutoRowKind::Overall).unwrap().handles[0];
        s.execute("dimedit", &json!({"handles": [h.hex()], "mode": "new", "text": "HAND"})).unwrap();
        assert!(row(&s, 0, "south", AutoRowKind::Overall).unwrap().edited);
        s.execute("dimauto", &json!({})).unwrap();
        assert!(row(&s, 0, "south", AutoRowKind::Overall).unwrap().edited);
        assert_eq!(dim(&s, h).text, "HAND");
    }

    #[test]
    fn erase_of_an_auto_dim_marks_edited() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let g = row(&s, 0, "east", AutoRowKind::Overall).unwrap().handles[0];
        s.execute("erase", &json!({"handles": [g.hex()]})).unwrap();
        assert!(row(&s, 0, "east", AutoRowKind::Overall).unwrap().edited);
        assert!(!row(&s, 0, "west", AutoRowKind::Overall).unwrap().edited);
    }

    #[test]
    fn edited_rows_still_follow_associatively() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let north = row(&s, 0, "north", AutoRowKind::Walls).unwrap();
        let h = north.handles[0];
        s.execute("dimedit", &json!({"handles": [h.hex()], "mode": "new", "text": "KEEP"})).unwrap();
        assert!(row(&s, 0, "north", AutoRowKind::Walls).unwrap().edited);
        // Moving w5 through another row's dimension: the edited dimension still reads the wall.
        let east = row(&s, 0, "east", AutoRowKind::Walls).unwrap();
        s.execute("dimdrive", &json!({"dimension": east.handles[0].hex(), "value": 3.6, "object": "w4"})).unwrap();
        assert_eq!(row(&s, 0, "north", AutoRowKind::Walls).unwrap().handles, north.handles);
        assert!(matches!(dim(&s, h).assoc[0].snap, AssocSnap::Arch { .. }));
        assert_eq!(dim(&s, h).text, "KEEP");
    }

    #[test]
    fn dimauto_off_removes_the_rows() {
        let mut s = house();
        s.execute("dimauto", &json!({"level": 0, "on": true})).unwrap();
        let handles = all_handles(&s, 0);
        assert!(!handles.is_empty());
        s.execute("dimauto", &json!({"level": 0, "on": false})).unwrap();
        assert!(s.doc().unwrap().arch.auto_dims.is_empty());
        assert!(handles.iter().all(|h| s.doc().unwrap().entity(*h).is_none()));
    }

    #[test]
    fn undo_restores_rows_and_flags() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let h = row(&s, 0, "north", AutoRowKind::Walls).unwrap().handles[0];
        s.execute("dimedit", &json!({"handles": [h.hex()], "mode": "new", "text": "X"})).unwrap();
        assert!(row(&s, 0, "north", AutoRowKind::Walls).unwrap().edited);
        s.undo().unwrap();
        assert!(!row(&s, 0, "north", AutoRowKind::Walls).unwrap().edited);
        assert_eq!(all_handles(&s, 0).len(), 10);
        s.undo().unwrap();
        assert!(s.doc().unwrap().arch.auto_dims.is_empty());
    }

    #[test]
    fn inspect_auto_dims_rows() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let r = s.execute("arch.inspect", &json!({"autoDims": true})).unwrap();
        let rows = r["autoDims"].as_array().unwrap();
        assert_eq!(rows.len(), 7);
        let north = rows.iter().find(|v| v["side"] == "north" && v["row"] == "walls").unwrap();
        assert_eq!(north["edited"], false);
        let vals: Vec<f64> = north["values"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        assert!(close(&vals, &[6.1, 4.1]), "{vals:?}");
    }

    #[test]
    fn refusals_change_nothing() {
        let mut s = house();
        s.execute("dimauto", &json!({})).unwrap();
        let before = s.doc().unwrap().arch.auto_dims.clone();
        let undo = s.state().unwrap().undo.len();
        assert!(s.execute("dimauto", &json!({"level": 99, "on": true})).is_err());
        assert!(s.execute("dimauto", &json!({"rows": ["bogus"]})).is_err());
        assert_eq!(s.doc().unwrap().arch.auto_dims, before);
        assert_eq!(s.state().unwrap().undo.len(), undo);
    }

    #[test]
    fn dimauto_only_overall_kind() {
        let mut s = house();
        let r = s.execute("dimauto", &json!({"rows": ["overall"]})).unwrap();
        let rows = r["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 4);
        for row in rows {
            assert_eq!(row["row"].as_str().unwrap(), "overall");
        }
        assert!(row(&s, 0, "north", AutoRowKind::Walls).is_none());
        assert!(row(&s, 0, "north", AutoRowKind::Openings).is_none());
        assert!(row(&s, 0, "east", AutoRowKind::Walls).is_none());
    }

    #[test]
    fn multiple_windows_one_side() {
        let mut s = house();
        s.execute("window", &json!({"wall": "w2", "offset": 2.0, "width": 1.0, "height": 1.2, "sill": 0.9})).unwrap();
        s.execute("window", &json!({"wall": "w2", "offset": 6.0, "width": 1.0, "height": 1.2, "sill": 0.9})).unwrap();
        s.execute("dimauto", &json!({})).unwrap();
        let vals = values(&s, 0, "north", AutoRowKind::Openings);
        // w2: (10,8)-(0,8); offset 2 -> x=8, offset 6 -> x=4; w5 splits at x=6
        // points at x = -0.1, 4, 6, 8, 10.1
        assert!(close(&vals, &[4.1, 2.0, 2.0, 2.1]), "{vals:?}");
    }
}
