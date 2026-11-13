//! `countertop`, `countertop.detach`, `countertop.edit` and `countertop.delete` (phase 18).
//!
//! A custom countertop owns its outline: `countertop.detach` turns an automatic counter into one
//! that keeps its outline, `countertop` draws one from scratch, `countertop.edit` moves an edge
//! by a typed distance (or a vertex, the top, the thickness) and `countertop.delete` hands its
//! cabinets back to the automatic counter. Cabinets a countertop claims lose their automatic
//! counter.

use cadcraft_arch::countertop::{self, CounterKey};
use cadcraft_arch::countertop_edit::{detach, move_edge, validate};
use cadcraft_doc::arch::{
    ArchId, ArchModel, COUNTER_LAYER, CabinetKind, Countertop, MAX_COUNTER_VERTICES, MAX_COUNTERTOPS, cabinet_label, countertop_handle,
    countertop_label, handle_json,
};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::{Result, Session};

/// The command specs of this module.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("countertop", "Countertop", run_countertop)
            .menu(&["Architecture", "Countertop"])
            .params("{polygon: [[x,y],...], elevation: m (the top above the level floor), thickness: m, level?: name | index}"),
        CommandSpec::new("countertop.detach", "Detach Countertop", run_detach)
            .menu(&["Architecture", "Detach Countertop"])
            .params("{run: \"cb3\" (any cabinet of the automatic counter)}"),
        CommandSpec::new("countertop.edit", "Edit Countertop", run_edit)
            .menu(&["Architecture", "Edit Countertop"])
            .params("{id: \"ct0\", edge?: i, move?: m | vertex?: i, to?: [x,y], top?: m, thickness?: m}"),
        CommandSpec::new("countertop.delete", "Delete Countertop", run_delete).menu(&["Architecture", "Delete Countertop"]).params("{id: \"ct0\"}"),
    ]
}

/// A level named by `raw`: its name (exactly, else ignoring case), or its index.
fn parse_level(cmd: &str, model: &ArchModel, raw: &Value) -> Result<u32> {
    let found = match raw {
        Value::Number(n) => n.as_u64().and_then(|i| u32::try_from(i).ok()).filter(|i| model.level(*i).is_some()),
        Value::String(s) => model
            .levels
            .iter()
            .find(|l| l.name == *s)
            .or_else(|| model.levels.iter().find(|l| l.name.eq_ignore_ascii_case(s.trim())))
            .map(|l| l.index)
            .or_else(|| s.trim().parse::<u32>().ok().filter(|i| model.level(*i).is_some())),
        _ => return Err(bad(cmd, "`level` must be a level name or index")),
    };
    found.ok_or_else(|| bad(cmd, format!("unknown level {raw}")))
}

/// `<prefix><n>` (or a bare number) to an id.
fn parse_label(cmd: &str, key: &str, prefix: &str, v: Option<&Value>, what: &str) -> Result<ArchId> {
    let raw = match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => return Err(bad(cmd, format!("`{key}` (\"{prefix}0\") is required"))),
    };
    let t = raw.trim();
    let digits = t.get(..prefix.len()).filter(|p| p.eq_ignore_ascii_case(prefix)).and_then(|_| t.get(prefix.len()..)).unwrap_or(t);
    digits.parse::<u32>().map(ArchId).map_err(|_| bad(cmd, format!("unknown {what} `{raw}`")))
}

fn parse_point(cmd: &str, key: &str, v: &Value) -> Result<Vec2> {
    let pair = v.as_array().filter(|a| a.len() == 2);
    let x = pair.and_then(|a| a.first()).and_then(Value::as_f64).filter(|n| n.is_finite());
    let y = pair.and_then(|a| a.get(1)).and_then(Value::as_f64).filter(|n| n.is_finite());
    match (x, y) {
        (Some(x), Some(y)) => Ok(Vec2::new(x, y)),
        _ => Err(bad(cmd, format!("`{key}` holds a point that is not [x, y] of finite numbers"))),
    }
}

/// `polygon`: at most 256 points; a trailing repeat of the first is dropped.
fn parse_polygon(cmd: &str, v: Option<&Value>) -> Result<Vec<Vec2>> {
    let items = v
        .and_then(Value::as_array)
        .filter(|a| a.len() <= MAX_COUNTER_VERTICES + 1)
        .ok_or_else(|| bad(cmd, format!("`polygon` must be a list of 3 to {MAX_COUNTER_VERTICES} points")))?;
    let mut ring = items.iter().map(|p| parse_point(cmd, "polygon", p)).collect::<Result<Vec<_>>>()?;
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    Ok(ring)
}

/// A finite number under `key`; `None` when absent.
fn metres(cmd: &str, p: &Value, key: &str) -> Result<Option<f64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|n| n.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite number"))),
    }
}

/// The area of countertop `id` as its outline reads.
fn area_of(model: &ArchModel, id: ArchId) -> f64 {
    model.countertop(id).and_then(|c| countertop::custom_outline(model, c)).map_or(0.0, |o| countertop::outline_area(&o))
}

fn reply(model: &ArchModel, id: ArchId) -> Value {
    let cabinets: Vec<String> = model.countertop(id).map(|c| c.cabinets.iter().map(|b| cabinet_label(*b)).collect()).unwrap_or_default();
    json!({
        "countertop": countertop_label(id),
        "handle": handle_json(countertop_handle(id)),
        "cabinets": cabinets,
        "area": area_of(model, id),
    })
}

fn run_detach(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "countertop.detach";
    let cab = parse_label(cmd, "run", "cb", p.get("run"), "cabinet")?;
    let model = &s.doc()?.arch;
    let Some(c) = model.cabinets.iter().find(|c| c.id == cab) else {
        return Err(bad(cmd, format!("unknown cabinet `{}`", cabinet_label(cab))));
    };
    if c.kind == CabinetKind::Wall {
        return Err(bad(cmd, format!("{} is a wall cabinet; it has no counter", cabinet_label(cab))));
    }
    if countertop::claimed_cabinets(model).contains(&cab) {
        return Err(bad(cmd, format!("{} already belongs to a custom countertop", cabinet_label(cab))));
    }
    if model.countertops.len() >= MAX_COUNTERTOPS {
        return Err(bad(cmd, format!("a drawing holds at most {MAX_COUNTERTOPS} countertops")));
    }
    let owner = countertop::auto_outlines(model).into_iter().find(|o| o.cabinets.contains(&cab)).and_then(|o| match o.key {
        Some(CounterKey::Auto(owner)) => Some(owner),
        _ => None,
    });
    let ct = detach(model, cab).map_err(|e| bad(cmd, e))?;
    validate(&ct).map_err(|e| bad(cmd, e))?;
    let paint = owner.and_then(|o| model.paints.get(&format!("cabinet/{}/counter", cabinet_label(o))).cloned());

    let doc = s.doc_mut()?;
    let id = doc.arch.push_countertop(ct);
    if let Some(paint) = paint {
        doc.arch.paints.insert(format!("countertop/{}", countertop_label(id)), paint);
    }
    doc.ensure_layer(COUNTER_LAYER);
    doc.touch_arch();
    Ok(reply(&doc.arch, id))
}

fn run_countertop(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "countertop";
    let model = &s.doc()?.arch;
    let level = match p.get("level").filter(|v| !v.is_null()) {
        Some(raw) => parse_level(cmd, model, raw)?,
        None => 0,
    };
    let polygon = parse_polygon(cmd, p.get("polygon"))?;
    let top = metres(cmd, p, "elevation")?.ok_or_else(|| bad(cmd, "`elevation` (the top above the floor, in metres) is required"))?;
    let thickness = metres(cmd, p, "thickness")?.ok_or_else(|| bad(cmd, "`thickness` (in metres) is required"))?;
    if model.countertops.len() >= MAX_COUNTERTOPS {
        return Err(bad(cmd, format!("a drawing holds at most {MAX_COUNTERTOPS} countertops")));
    }
    let claimed = countertop::claimed_cabinets(model);
    let cabinets: Vec<ArchId> = countertop::counter_cabinets_under(model, level, &polygon).into_iter().filter(|c| !claimed.contains(c)).collect();
    let ct = Countertop { level, polygon, top, thickness, cabinets, layer: COUNTER_LAYER.to_string(), ..Default::default() };
    validate(&ct).map_err(|e| bad(cmd, e))?;

    let doc = s.doc_mut()?;
    let id = doc.arch.push_countertop(ct);
    doc.ensure_layer(COUNTER_LAYER);
    doc.touch_arch();
    Ok(reply(&doc.arch, id))
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "countertop.edit";
    let id = parse_label(cmd, "id", "ct", p.get("id"), "countertop")?;
    let model = &s.doc()?.arch;
    let mut ct = model.countertop(id).ok_or_else(|| bad(cmd, format!("unknown countertop `{}`", countertop_label(id))))?.clone();
    let given = |k: &str| p.get(k).is_some_and(|v| !v.is_null());
    let mut edited = false;
    if given("edge") || given("move") {
        if !(given("edge") && given("move")) || given("vertex") || given("to") {
            return Err(bad(cmd, "`edge` and `move` go together, and not with `vertex`/`to`"));
        }
        let edge =
            p.get("edge").and_then(Value::as_u64).and_then(|e| usize::try_from(e).ok()).ok_or_else(|| bad(cmd, "`edge` must be an edge index"))?;
        let d = metres(cmd, p, "move")?.ok_or_else(|| bad(cmd, "`move` must be a finite number of metres"))?;
        ct = move_edge(&ct, edge, d).map_err(|e| bad(cmd, e))?;
        edited = true;
    } else if given("vertex") || given("to") {
        if !(given("vertex") && given("to")) {
            return Err(bad(cmd, "`vertex` and `to` go together"));
        }
        let i = p
            .get("vertex")
            .and_then(Value::as_u64)
            .and_then(|e| usize::try_from(e).ok())
            .ok_or_else(|| bad(cmd, "`vertex` must be a vertex index"))?;
        let to = parse_point(cmd, "to", p.get("to").unwrap_or(&Value::Null))?;
        let slot = ct.polygon.get_mut(i).ok_or_else(|| bad(cmd, format!("vertex {i} does not exist")))?;
        *slot = to;
        edited = true;
    }
    if let Some(t) = metres(cmd, p, "top")? {
        ct.top = t;
        edited = true;
    }
    if let Some(t) = metres(cmd, p, "thickness")? {
        ct.thickness = t;
        edited = true;
    }
    if !edited {
        return Err(bad(cmd, "nothing to edit: give `edge` + `move`, `vertex` + `to`, `top` or `thickness`"));
    }
    validate(&ct).map_err(|e| bad(cmd, e))?;

    let doc = s.doc_mut()?;
    if let Some(slot) = doc.arch.countertops.iter_mut().find(|c| c.id == id) {
        *slot = ct;
    }
    doc.touch_arch();
    Ok(json!({ "countertop": countertop_label(id), "area": area_of(&doc.arch, id) }))
}

fn run_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "countertop.delete";
    let id = parse_label(cmd, "id", "ct", p.get("id"), "countertop")?;
    if s.doc()?.arch.countertop(id).is_none() {
        return Err(bad(cmd, format!("unknown countertop `{}`", countertop_label(id))));
    }
    let doc = s.doc_mut()?;
    doc.arch.countertops.retain(|c| c.id != id);
    doc.arch.paints.remove(&format!("countertop/{}", countertop_label(id)));
    doc.touch_arch();
    Ok(json!({ "deleted": [countertop_label(id)] }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::arch::{Cabinet, Furnishing, FurnishingHost, Paint};

    fn island() -> Session {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        for x in [2.0, 2.9] {
            d.arch.push_cabinet(Cabinet {
                kind: CabinetKind::Base,
                position: Vec2::new(x, 2.0),
                width: 0.9,
                depth: 0.6,
                height: 0.876,
                counter: true,
                counter_thickness: 0.038,
                counter_overhang: 0.025,
                ..Default::default()
            });
        }
        d.touch_arch();
        s
    }

    fn undo_len(s: &Session) -> usize {
        s.state().map(|st| st.undo.len()).unwrap_or_default()
    }

    fn rows(s: &mut Session) -> Vec<Value> {
        let r = s.execute("arch.inspect", &json!({"countertops": true})).unwrap();
        r["countertops"].as_array().cloned().unwrap_or_default()
    }

    #[test]
    fn detach_create_edit_delete_are_single_undo_steps() {
        let mut s = island();
        let n = undo_len(&s);
        let r = s.execute("countertop.detach", &json!({"run": "cb1"})).unwrap();
        assert_eq!(r["countertop"], "ct0");
        assert_eq!(r["cabinets"], json!(["cb0", "cb1"]));
        assert!((r["area"].as_f64().unwrap() - 1.8 * 0.625).abs() < 1e-9, "{r}");
        assert_eq!(undo_len(&s), n + 1);
        let r = s.execute("countertop.edit", &json!({"id": "ct0", "edge": 2, "move": 0.3})).unwrap();
        assert!((r["area"].as_f64().unwrap() - (1.125 + 0.54)).abs() < 1e-9, "{r}");
        assert_eq!(undo_len(&s), n + 2);
        let r =
            s.execute("countertop", &json!({"polygon": [[5, 0], [6, 0], [6, 1], [5, 1], [5, 0]], "elevation": 0.914, "thickness": 0.03})).unwrap();
        assert_eq!(r["countertop"], "ct1");
        assert!((r["area"].as_f64().unwrap() - 1.0).abs() < 1e-9);
        assert_eq!(undo_len(&s), n + 3);
        let r = s.execute("countertop.delete", &json!({"id": "ct0"})).unwrap();
        assert_eq!(r["deleted"], json!(["ct0"]));
        assert_eq!(undo_len(&s), n + 4);
        assert_eq!(s.doc().unwrap().arch.countertops.len(), 1);
        assert_eq!(s.undo().unwrap().as_deref(), Some("Delete Countertop"));
        assert_eq!(s.doc().unwrap().arch.countertops.len(), 2);
        assert_eq!(s.undo().unwrap().as_deref(), Some("Countertop"));
        assert_eq!(s.undo().unwrap().as_deref(), Some("Edit Countertop"));
        assert_eq!(s.undo().unwrap().as_deref(), Some("Detach Countertop"));
        assert!(s.doc().unwrap().arch.countertops.is_empty());
    }

    #[test]
    fn detach_copies_the_counter_paint() {
        let mut s = island();
        s.doc_mut().unwrap().arch.paints.insert("cabinet/cb0/counter".into(), Paint::default());
        s.execute("countertop.detach", &json!({"run": "cb1"})).unwrap();
        assert!(s.doc().unwrap().arch.paints.contains_key("countertop/ct0"));
        s.doc_mut().unwrap().arch.paints.insert("countertop/ct0".into(), Paint::default());
        s.execute("countertop.delete", &json!({"id": "ct0"})).unwrap();
        assert!(!s.doc().unwrap().arch.paints.contains_key("countertop/ct0"), "the paint entry goes with it");
    }

    #[test]
    fn detach_refusals_leave_nothing() {
        let mut s = island();
        s.doc_mut().unwrap().arch.push_cabinet(Cabinet { kind: CabinetKind::Wall, position: Vec2::new(9.0, 9.0), ..Default::default() });
        s.execute("countertop.detach", &json!({"run": "cb0"})).unwrap();
        let before = s.doc().unwrap().clone();
        let n = undo_len(&s);
        for (c, p) in [
            ("countertop.detach", json!({"run": "cb1"})),
            ("countertop.detach", json!({"run": "cb9"})),
            ("countertop.detach", json!({"run": "cb2"})),
            ("countertop.detach", json!({})),
            ("countertop.detach", json!({"run": "x"})),
            ("countertop.edit", json!({"id": "ct9", "top": 0.9})),
            ("countertop.edit", json!({"id": "ct0"})),
            ("countertop.edit", json!({"id": "ct0", "edge": 2})),
            ("countertop.edit", json!({"id": "ct0", "edge": 9, "move": 0.1})),
            ("countertop.edit", json!({"id": "ct0", "edge": 2, "move": 5.0})),
            ("countertop.edit", json!({"id": "ct0", "edge": 2, "move": -2.0})),
            ("countertop.edit", json!({"id": "ct0", "edge": 2, "move": null, "top": 1})),
            ("countertop.edit", json!({"id": "ct0", "top": 7})),
            ("countertop.edit", json!({"id": "ct0", "thickness": 0.5})),
            ("countertop.edit", json!({"id": "ct0", "vertex": 0, "to": [3.0, 2.7]})),
            ("countertop.edit", json!({"id": "ct0", "vertex": 9, "to": [0, 0]})),
            ("countertop.delete", json!({"id": "ct9"})),
            ("countertop", json!({"polygon": [[0, 0], [1, 0]], "elevation": 0.9, "thickness": 0.03})),
            ("countertop", json!({"polygon": [[0, 0], [1, 0], [1, 1]], "thickness": 0.03})),
            ("countertop", json!({"polygon": [[0, 0], [1, 1], [1, 0], [0, 1]], "elevation": 0.9, "thickness": 0.03})),
            ("countertop", json!({"polygon": [[0, 0], [1, 0], [1, 1]], "elevation": 0.9, "thickness": 0.03, "level": "Moon"})),
        ] {
            assert!(s.execute(c, &p).is_err(), "{c} {p} is refused");
        }
        assert_eq!(*s.doc().unwrap(), before, "nothing changed");
        assert_eq!(undo_len(&s), n, "no undo step");
    }

    #[test]
    fn countertop_from_scratch_claims_cabinets_under_it() {
        let mut s = island();
        assert_eq!(rows(&mut s).len(), 1);
        let r = s
            .execute("countertop", &json!({"polygon": [[1.0, 1.9], [4.0, 1.9], [4.0, 2.8], [1.0, 2.8]], "elevation": 0.914, "thickness": 0.038}))
            .unwrap();
        assert_eq!(r["cabinets"], json!(["cb0", "cb1"]));
        let rs = rows(&mut s);
        assert_eq!(rs.len(), 1, "the claimed cabinets lose their automatic counter: {rs:?}");
        assert_eq!(rs[0]["id"], "ct0");
        s.execute("countertop.delete", &json!({"id": "ct0"})).unwrap();
        let rs = rows(&mut s);
        assert_eq!((rs.len(), &rs[0]["id"]), (1, &json!("cb0")));
    }

    #[test]
    fn pick3d_on_a_countertop_names_it() {
        let mut s = Session::new();
        let r = s.execute("countertop", &json!({"polygon": [[0, 0], [1, 0], [1, 1], [0, 1]], "elevation": 0.9, "thickness": 0.04})).unwrap();
        let cam = json!({ "kind": "full", "eye": [0.5, -2.0, 3.0], "target": [0.5, 0.5, 0.9], "fov": 45 });
        let p = json!({ "camera": cam, "size": [100, 100], "pixel": [50, 50] });
        let hit = s.execute("pick3d", &p).unwrap()["hit"].clone();
        assert_eq!(hit["id"], "ct0", "{hit}");
        assert_eq!(hit["handle"], r["handle"]);
        assert_eq!(hit["part"], "counter");
    }

    #[test]
    fn inspect_rows_follow_detach_and_move() {
        let mut s = island();
        s.doc_mut().unwrap().arch.push_furnishing(Furnishing {
            item: "sink".into(),
            at: [2.2, 2.3],
            cutout: Some([0.5, 0.4]),
            host: FurnishingHost::Cabinet { cabinet: ArchId(0) },
            ..Default::default()
        });
        let r = rows(&mut s);
        assert_eq!((r[0]["id"].clone(), r[0]["auto"].clone()), (json!("cb0"), json!(true)));
        assert_eq!(r[0]["holes"].as_array().map(Vec::len), Some(1));
        s.execute("countertop.detach", &json!({"run": "cb1"})).unwrap();
        let r = rows(&mut s);
        assert_eq!(r.len(), 1);
        assert_eq!((r[0]["id"].clone(), r[0]["auto"].clone()), (json!("ct0"), json!(false)));
        assert!((r[0]["area"].as_f64().unwrap() - 1.125).abs() < 1e-9);
        assert_eq!(r[0]["holes"].as_array().map(Vec::len), Some(1), "the cut-out carries over");
        let e = r[0]["edges"].as_array().unwrap().iter().position(|e| (e["from"][1].as_f64().unwrap() - 2.625).abs() < 1e-9).unwrap();
        s.execute("countertop.edit", &json!({"id": "ct0", "edge": e, "move": 0.3})).unwrap();
        let r = rows(&mut s);
        assert!((r[0]["area"].as_f64().unwrap() - 1.665).abs() < 1e-9, "{}", r[0]["area"]);
    }
}
