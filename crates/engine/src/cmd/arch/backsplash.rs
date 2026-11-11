//! Backsplash commands (phase 18): `backsplash`, `backsplash.edit`, `backsplash.delete`, and the
//! `{"backsplashes": true}` inspect rows. A backsplash is data (a counter or wall to follow, a
//! height, a thickness); its segments are recomputed from the model by `cadcraft_arch::backsplash`.

use cadcraft_arch::backsplash::segments;
use cadcraft_arch::countertop;
use cadcraft_arch::paint::effective_material;
use cadcraft_doc::arch::{
    ArchId, ArchModel, Backsplash, CabinetKind, CounterRef, MAX_BACKSPLASHES, MaterialRef, Paint, Surface, backsplash_handle, backsplash_label,
    cabinet_label, countertop_label, handle_json, wall_label,
};
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, bad, str_param};
use crate::{Result, Session};

/// Longest material key accepted.
const MAX_MATERIAL_KEY: usize = 128;
/// The height range of a backsplash above its counter (m).
const HEIGHT_RANGE: std::ops::RangeInclusive<f64> = 0.05..=2.0;

/// The command specs of this module.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("backsplash", "Backsplash", run_backsplash)
            .menu(&["Architecture", "Backsplash"])
            .params("{run: \"cb0\" | \"ct0\"} | {wall: \"w0\", level?: name|index}, plus height?: 0.45 (0.05..2), top?: \"ceiling\", material?: key"),
        CommandSpec::new("backsplash.edit", "Edit Backsplash", run_edit)
            .menu(&["Architecture", "Edit Backsplash"])
            .params("{id: \"bs0\", height?: m, top?: \"ceiling\" | \"auto\", material?: key | null}"),
        CommandSpec::new("backsplash.delete", "Delete Backsplash", run_delete).menu(&["Architecture", "Delete Backsplash"]).params("{id: \"bs0\"}"),
    ]
}

/// `prefix` + digits to an id.
fn label_id(raw: &str, prefix: &str) -> Option<ArchId> {
    let t = raw.trim().to_ascii_lowercase();
    t.strip_prefix(prefix)?.parse::<u32>().ok().map(ArchId)
}

fn given<'a>(p: &'a Value, key: &str) -> Option<&'a Value> {
    p.get(key).filter(|v| !v.is_null())
}

fn text<'a>(cmd: &str, v: &'a Value, key: &str) -> Result<&'a str> {
    v.as_str().ok_or_else(|| bad(cmd, format!("`{key}` must be a string")))
}

fn parse_height(cmd: &str, v: &Value) -> Result<f64> {
    v.as_f64().filter(|h| h.is_finite() && HEIGHT_RANGE.contains(h)).ok_or_else(|| bad(cmd, "`height` must be a number from 0.05 to 2 metres"))
}

fn parse_top(cmd: &str, v: &Value, auto_ok: bool) -> Result<bool> {
    match v.as_str().map(str::trim) {
        Some(s) if s.eq_ignore_ascii_case("ceiling") => Ok(true),
        Some(s) if auto_ok && s.eq_ignore_ascii_case("auto") => Ok(false),
        _ => Err(bad(cmd, if auto_ok { "`top` must be \"ceiling\" or \"auto\"" } else { "`top` must be \"ceiling\"" })),
    }
}

fn parse_material(cmd: &str, v: &Value) -> Result<MaterialRef> {
    let raw = v.as_str().map(str::trim).filter(|s| !s.is_empty() && s.chars().count() <= MAX_MATERIAL_KEY);
    raw.and_then(MaterialRef::parse).ok_or_else(|| bad(cmd, "`material` must be a finish, a library material item or a #rrggbb colour"))
}

fn parse_level(cmd: &str, model: &ArchModel, v: &Value) -> Result<u32> {
    let err = || bad(cmd, "`level` must be a level name or index");
    match v {
        Value::Number(n) => n.as_u64().and_then(|i| u32::try_from(i).ok()).filter(|i| model.level(*i).is_some()).ok_or_else(err),
        Value::String(s) => {
            let t = s.trim();
            if let Some(i) = t.parse::<u32>().ok().filter(|i| model.level(*i).is_some()) {
                return Ok(i);
            }
            model.levels.iter().find(|l| l.name.eq_ignore_ascii_case(t)).map(|l| l.index).ok_or_else(|| bad(cmd, format!("unknown level `{s}`")))
        }
        _ => Err(err()),
    }
}

/// What a `run` names: the counter reference and its level.
fn parse_run(cmd: &str, model: &ArchModel, raw: &str) -> Result<(CounterRef, u32)> {
    if let Some(id) = label_id(raw, "cb") {
        let c = model.cabinets.iter().find(|c| c.id == id).ok_or_else(|| bad(cmd, format!("unknown cabinet `{raw}`")))?;
        if c.kind == CabinetKind::Wall || !countertop::outlines(model).iter().any(|o| o.cabinets.contains(&id)) {
            return Err(bad(cmd, format!("cabinet `{raw}` has no automatic counter")));
        }
        return Ok((CounterRef::Cabinet(id), c.level));
    }
    if let Some(id) = label_id(raw, "ct") {
        let ct = model.countertop(id).ok_or_else(|| bad(cmd, format!("unknown countertop `{raw}`")))?;
        return Ok((CounterRef::Countertop(id), ct.level));
    }
    Err(bad(cmd, format!("`run` must be a cabinet (\"cb0\") or a countertop (\"ct0\"), got `{raw}`")))
}

fn run_backsplash(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "backsplash";
    let model = s.doc()?.arch.clone();
    let mut b = Backsplash { id: ArchId(model.next_backsplash_id), ..Default::default() };
    match (given(p, "run"), given(p, "wall")) {
        (Some(run), None) => {
            let (counter, level) = parse_run(CMD, &model, text(CMD, run, "run")?)?;
            b.counter = Some(counter);
            b.level = level;
        }
        (None, Some(wall)) => {
            let raw = text(CMD, wall, "wall")?;
            let id = label_id(raw, "w").ok_or_else(|| bad(CMD, format!("unknown wall `{raw}`")))?;
            let w = model.wall(id).ok_or_else(|| bad(CMD, format!("unknown wall `{raw}`")))?;
            b.wall = Some(id);
            b.level = match given(p, "level") {
                Some(v) => parse_level(CMD, &model, v)?,
                None => w.base_level,
            };
        }
        _ => return Err(bad(CMD, "exactly one of `run` or `wall` is required")),
    }
    if let Some(v) = given(p, "height") {
        b.height = parse_height(CMD, v)?;
    }
    if let Some(v) = given(p, "top") {
        b.to_ceiling = parse_top(CMD, v, false)?;
    }
    let material = given(p, "material").map(|v| parse_material(CMD, v)).transpose()?;
    if model.backsplashes.len() >= MAX_BACKSPLASHES {
        return Err(bad(CMD, format!("a drawing holds at most {MAX_BACKSPLASHES} backsplashes")));
    }
    let n = segments(&model, &b).len();
    if n == 0 {
        return Err(bad(CMD, "no counter edge lies on a wall face"));
    }
    let doc = s.doc_mut()?;
    let layer = b.layer.clone();
    let id = doc.arch.push_backsplash(b);
    if let Some(m) = material {
        doc.arch.paints.insert(Surface::Backsplash(id).key(), Paint { material: Some(m), ..Default::default() });
    }
    doc.ensure_layer(&layer);
    doc.touch_arch();
    Ok(json!({ "backsplash": backsplash_label(id), "handle": handle_json(backsplash_handle(id)), "segments": n }))
}

fn parse_bs_id(cmd: &str, model: &ArchModel, p: &Value) -> Result<ArchId> {
    let raw = str_param(p, "id").ok_or_else(|| bad(cmd, "`id` (\"bs0\") is required"))?;
    let id = label_id(raw, "bs").ok_or_else(|| bad(cmd, format!("unknown backsplash `{raw}`")))?;
    if model.backsplash(id).is_none() {
        return Err(bad(cmd, format!("unknown backsplash `{raw}`")));
    }
    Ok(id)
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "backsplash.edit";
    let model = s.doc()?.arch.clone();
    let id = parse_bs_id(CMD, &model, p)?;
    let mut b = model.backsplash(id).cloned().ok_or_else(|| bad(CMD, "unknown backsplash"))?;
    if let Some(v) = given(p, "height") {
        b.height = parse_height(CMD, v)?;
    }
    if let Some(v) = given(p, "top") {
        b.to_ceiling = parse_top(CMD, v, true)?;
    }
    // `Some(None)` removes the paint, `Some(Some(m))` sets it.
    let material = match p.get("material") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => Some(Some(parse_material(CMD, v)?)),
    };
    let doc = s.doc_mut()?;
    if let Some(slot) = doc.arch.backsplashes.iter_mut().find(|x| x.id == id) {
        *slot = b;
    }
    match material {
        Some(Some(m)) => {
            doc.arch.paints.insert(Surface::Backsplash(id).key(), Paint { material: Some(m), ..Default::default() });
        }
        Some(None) => {
            doc.arch.paints.remove(&Surface::Backsplash(id).key());
        }
        None => {}
    }
    doc.touch_arch();
    Ok(doc.arch.backsplash(id).map_or(Value::Null, |b| row(&doc.arch, b)))
}

fn run_delete(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "backsplash.delete";
    let model = s.doc()?.arch.clone();
    let id = parse_bs_id(CMD, &model, p)?;
    let doc = s.doc_mut()?;
    doc.arch.backsplashes.retain(|b| b.id != id);
    doc.arch.paints.remove(&Surface::Backsplash(id).key());
    doc.touch_arch();
    Ok(json!({ "deleted": [backsplash_label(id)] }))
}

fn row(model: &ArchModel, b: &Backsplash) -> Value {
    let counter = b.counter.map(|c| match c {
        CounterRef::Cabinet(id) => cabinet_label(id),
        CounterRef::Countertop(id) => countertop_label(id),
    });
    let segs: Vec<Value> = segments(model, b)
        .iter()
        .map(|s| json!({ "from": [s.a.x, s.a.y], "to": [s.b.x, s.b.y], "wall": wall_label(s.wall), "bottom": s.bottom, "top": s.top }))
        .collect();
    json!({
        "id": backsplash_label(b.id), "handle": handle_json(backsplash_handle(b.id)), "level": b.level,
        "counter": counter, "wall": b.wall.map(wall_label), "height": b.height, "toCeiling": b.to_ceiling,
        "thickness": b.thickness, "segments": segs,
        "material": effective_material(model, &Surface::Backsplash(b.id)), "layer": b.layer,
    })
}

/// Add the `backsplashes` rows to `out` when `p` asks for them, and a `backsplash` flag to every
/// `cabinets` row already in `out`. Read-only.
pub(crate) fn inspect_backsplashes(model: &ArchModel, p: &Value, out: &mut Map<String, Value>) {
    if p.get("backsplashes").and_then(Value::as_bool) == Some(true) {
        let rows: Vec<Value> = model.backsplashes.iter().map(|b| row(model, b)).collect();
        out.insert("backsplashes".to_string(), Value::Array(rows));
    }
    if let Some(Value::Array(rows)) = out.get_mut("cabinets") {
        for (r, c) in rows.iter_mut().zip(model.cabinets.iter()) {
            if let Value::Object(obj) = r {
                obj.insert("backsplash".to_string(), Value::Bool(!c.backsplash_off));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use cadcraft_doc::arch::{Cabinet, Level, Wall};
    use cadcraft_geom::Vec2;

    fn cab(kind: CabinetKind, x0: f64, elevation: f64) -> Cabinet {
        let base = kind == CabinetKind::Base;
        Cabinet {
            kind,
            position: Vec2::new(x0 + 0.3, 0.0),
            width: 0.6,
            depth: if base { 0.6 } else { 0.33 },
            height: if base { 0.876 } else { 0.7 },
            elevation,
            doors: 1,
            counter: base,
            counter_thickness: 0.038,
            counter_overhang: 0.025,
            ..Default::default()
        }
    }

    /// A wall whose room face is y = 0, base cabinets cb0..cb2 (x 0.6..2.4) and a wall cabinet cb3
    /// over the first.
    fn kitchen() -> Session {
        let mut s = Session::new();
        {
            let mut d = s.doc_mut().unwrap();
            if d.arch.levels.is_empty() {
                d.arch.levels.push(Level::default());
            }
            d.arch.push_wall(Wall::new(Vec2::new(0.0, -0.1), Vec2::new(6.0, -0.1), 0.2));
            for i in 0..3 {
                d.arch.push_cabinet(cab(CabinetKind::Base, 0.6 + 0.6 * f64::from(i), 0.0));
            }
            d.arch.push_cabinet(cab(CabinetKind::Wall, 0.6, 1.372));
            d.touch_arch();
        }
        s
    }

    fn undo_len(s: &Session) -> usize {
        s.state().map(|st| st.undo.len()).unwrap_or_default()
    }

    fn near(v: &Value, want: f64) -> bool {
        v.as_f64().is_some_and(|x| (x - want).abs() < 1e-6)
    }

    fn inspect(s: &mut Session) -> Value {
        s.execute("arch.inspect", &json!({ "backsplashes": true, "cabinets": true })).unwrap()
    }

    #[test]
    fn create_edit_delete_single_undo_steps() {
        let mut s = kitchen();
        let n = undo_len(&s);
        let r = s.execute("backsplash", &json!({ "run": "cb0" })).unwrap();
        assert_eq!(r["backsplash"], "bs0");
        assert_eq!(r["segments"], 2);
        assert!(r["handle"].is_string());
        assert_eq!(undo_len(&s), n + 1);
        assert!(s.doc().unwrap().layer("A-FLOR-CASE").is_some());
        let r = s.execute("backsplash.edit", &json!({ "id": "bs0", "top": "ceiling" })).unwrap();
        assert_eq!(r["toCeiling"], true);
        assert!(r["segments"].as_array().unwrap().iter().all(|g| near(&g["top"], 2.4)));
        assert_eq!(undo_len(&s), n + 2);
        let r = s.execute("backsplash.edit", &json!({ "id": "bs0", "top": "auto", "height": 0.3 })).unwrap();
        assert_eq!(r["toCeiling"], false);
        let tops: Vec<f64> = r["segments"].as_array().unwrap().iter().filter_map(|g| g["top"].as_f64()).collect();
        assert!((tops[0] - 1.372).abs() < 1e-6 && (tops[1] - 1.214).abs() < 1e-6, "{tops:?}");
        assert_eq!(undo_len(&s), n + 3);
        let r = s.execute("backsplash.delete", &json!({ "id": "bs0" })).unwrap();
        assert_eq!(r["deleted"], json!(["bs0"]));
        assert_eq!(undo_len(&s), n + 4);
        assert!(s.doc().unwrap().arch.backsplashes.is_empty());
        // Undo brings each step back.
        s.execute("undo", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().arch.backsplashes.len(), 1);
        // By wall: a second backsplash on the same face.
        let r = s.execute("backsplash", &json!({ "wall": "w0", "height": 0.3 })).unwrap();
        assert_eq!(r["backsplash"], "bs1");
        assert_eq!(r["segments"], 2);
    }

    #[test]
    fn material_param_writes_the_paint() {
        let mut s = kitchen();
        s.execute("backsplash", &json!({ "run": "cb0", "material": "material-marble" })).unwrap();
        let d = s.doc().unwrap();
        let paint = d.arch.paints.get("backsplash/bs0").unwrap();
        assert_eq!(paint.material, Some(MaterialRef::Item("material-marble".into())));
        assert_eq!(paint.tint, None);
        drop(d);
        let v = inspect(&mut s);
        assert_eq!(v["backsplashes"][0]["material"], "item:material-marble");
        s.execute("backsplash.edit", &json!({ "id": "bs0", "material": "#c04020" })).unwrap();
        assert_eq!(inspect(&mut s)["backsplashes"][0]["material"], "colour:#c04020");
        s.execute("backsplash.edit", &json!({ "id": "bs0", "height": 0.5 })).unwrap();
        assert_eq!(inspect(&mut s)["backsplashes"][0]["material"], "colour:#c04020");
        s.execute("backsplash.edit", &json!({ "id": "bs0", "material": null })).unwrap();
        assert!(s.doc().unwrap().arch.paints.is_empty());
        assert_eq!(inspect(&mut s)["backsplashes"][0]["material"], "default:backsplash");
        s.execute("backsplash.edit", &json!({ "id": "bs0", "material": "siding" })).unwrap();
        s.execute("backsplash.delete", &json!({ "id": "bs0" })).unwrap();
        assert!(s.doc().unwrap().arch.paints.is_empty(), "delete removes the paint too");
    }

    #[test]
    fn backsplash_refusals_leave_nothing() {
        let mut s = kitchen();
        let before = s.doc().unwrap().arch.clone();
        let n = undo_len(&s);
        let bad_calls = [
            json!({}),
            json!({ "run": "cb0", "wall": "w0" }),
            json!({ "run": "cb9" }),
            json!({ "run": "ct0" }),
            json!({ "run": "cb3" }),
            json!({ "run": "nonsense" }),
            json!({ "run": 3 }),
            json!({ "wall": "w9" }),
            json!({ "wall": "w0", "level": "attic" }),
            json!({ "run": "cb0", "height": 0.04 }),
            json!({ "run": "cb0", "height": 2.5 }),
            json!({ "run": "cb0", "height": "tall" }),
            json!({ "run": "cb0", "height": [0.4] }),
            json!({ "run": "cb0", "top": "roof" }),
            json!({ "run": "cb0", "material": "" }),
            json!({ "run": "cb0", "material": "#12" }),
            json!({ "run": "cb0", "material": 5 }),
        ];
        for call in &bad_calls {
            assert!(s.execute("backsplash", call).is_err(), "{call}");
        }
        for call in [json!({}), json!({ "id": "bs0" }), json!({ "id": "w0" })] {
            assert!(s.execute("backsplash.edit", &call).is_err(), "{call}");
            assert!(s.execute("backsplash.delete", &call).is_err(), "{call}");
        }
        assert_eq!(s.doc().unwrap().arch, before);
        assert_eq!(undo_len(&s), n);
        // A counter with no wall behind it: no edge lies on a wall face.
        s.doc_mut().unwrap().arch.walls.clear();
        let err = s.execute("backsplash", &json!({ "run": "cb0" })).unwrap_err().to_string();
        assert!(err.contains("no counter edge lies on a wall face"), "{err}");
        // Bad edits leave an existing backsplash alone.
        let mut s = kitchen();
        s.execute("backsplash", &json!({ "run": "cb0" })).unwrap();
        let before = s.doc().unwrap().arch.clone();
        assert!(s.execute("backsplash.edit", &json!({ "id": "bs0", "height": 9.0 })).is_err());
        assert!(s.execute("backsplash.edit", &json!({ "id": "bs0", "top": "floor" })).is_err());
        assert!(s.execute("backsplash.edit", &json!({ "id": "bs0", "material": "" })).is_err());
        assert_eq!(s.doc().unwrap().arch, before);
        // The cap.
        s.doc_mut().unwrap().arch.backsplashes = vec![Backsplash::default(); MAX_BACKSPLASHES];
        assert!(s.execute("backsplash", &json!({ "run": "cb0" })).is_err());
    }

    #[test]
    fn inspect_backsplash_rows() {
        let mut s = kitchen();
        s.execute("backsplash", &json!({ "run": "cb0", "material": "material-marble" })).unwrap();
        s.execute("cabinet.edit", &json!({ "id": "cb2", "backsplash": false })).unwrap();
        let v = inspect(&mut s);
        let rows = v["backsplashes"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r["id"], "bs0");
        assert!(r["handle"].is_string());
        assert_eq!(r["level"], 0);
        assert_eq!(r["counter"], "cb0");
        assert_eq!(r["wall"], Value::Null);
        assert!(near(&r["height"], 0.45) && near(&r["thickness"], 0.01));
        assert_eq!(r["toCeiling"], false);
        assert_eq!(r["material"], "item:material-marble");
        assert_eq!(r["layer"], "A-FLOR-CASE");
        let segs = r["segments"].as_array().unwrap();
        assert_eq!(segs.len(), 2, "{segs:?}");
        assert!(near(&segs[0]["top"], 1.372) && near(&segs[1]["top"], 1.364));
        assert!(near(&segs[0]["bottom"], 0.914));
        assert_eq!(segs[0]["wall"], "w0");
        assert!(near(&segs[0]["from"][0], 0.6) && near(&segs[1]["to"][0], 1.8), "cb2 is off: {segs:?}");
        assert!(near(&segs[0]["from"][1], 0.0) && near(&segs[0]["to"][1], 0.0));
        let cabs = v["cabinets"].as_array().unwrap();
        let flags: Vec<bool> = cabs.iter().filter_map(|c| c["backsplash"].as_bool()).collect();
        assert_eq!(flags, vec![true, true, false, true]);
        // By wall.
        s.execute("backsplash", &json!({ "wall": "w0" })).unwrap();
        let v = inspect(&mut s);
        assert_eq!(v["backsplashes"][1]["wall"], "w0");
        assert_eq!(v["backsplashes"][1]["counter"], Value::Null);
        // Without the flag there are no backsplash rows.
        let v = s.execute("arch.inspect", &json!({ "cabinets": true })).unwrap();
        assert!(v.get("backsplashes").is_none());
        assert_eq!(v["cabinets"][0]["backsplash"], true);
    }
}
