//! `cabinet.face` (phase 18): edit the face layout of a cabinet, plus the `face` part of the
//! `cabinets` inspect rows.

use cadcraft_arch::casework::validate_cabinet;
use cadcraft_arch::casework_face::{default_face, face_box, face_fronts, row_across, row_heights};
use cadcraft_arch::casework_layout::FrontKind;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchId, Cabinet, FaceKind, FaceLayout, FaceRow, MAX_ACROSS, MAX_FACE_ROWS, cabinet_label};
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::{Result, Session};

const CMD: &str = "cabinet.face";

/// The commands of this module.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new(CMD, "Cabinet Face", run_face).menu(&["Architecture", "Cabinet Face"]).params(
            "{id: \"cb0\", rows?: [{kind: \"door\"|\"drawer\"|\"false\"|\"opening\", height?: m|null, across?: n}] | row?: i, split?: \"down\"|\"across\", count?: 2..6 | row: i, kind?, height?: m|null, across?: n | equalize?: true | doors?: \"auto\"|\"single\"|\"double\" | reset?: true}",
        ),
    ]
}

fn parse_id(raw: &str) -> Option<ArchId> {
    raw.trim().strip_prefix("cb").and_then(|d| d.parse::<u32>().ok()).map(ArchId)
}

fn row_kind(v: &Value) -> Result<FaceKind> {
    match v.as_str().and_then(FaceKind::parse) {
        Some(k) if k != FaceKind::Appliance => Ok(k),
        _ => Err(bad(CMD, "a row `kind` must be \"door\", \"drawer\", \"false\" or \"opening\"")),
    }
}

fn row_height(v: &Value) -> Result<Option<f64>> {
    match v {
        Value::Null => Ok(None),
        _ => v.as_f64().filter(|h| h.is_finite() && *h > 0.0).map(Some).ok_or_else(|| bad(CMD, "a row `height` must be a positive number or null")),
    }
}

fn row_across_param(v: &Value) -> Result<u32> {
    v.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n <= MAX_ACROSS).ok_or_else(|| bad(CMD, format!("`across` must be 0..={MAX_ACROSS}")))
}

fn parse_row(v: &Value) -> Result<FaceRow> {
    let o = v.as_object().ok_or_else(|| bad(CMD, "each row must be an object"))?;
    let mut row = FaceRow { kind: row_kind(o.get("kind").ok_or_else(|| bad(CMD, "each row needs a `kind`"))?)?, ..FaceRow::default() };
    if let Some(h) = o.get("height") {
        row.height = row_height(h)?;
    }
    if let Some(a) = o.get("across") {
        row.across = row_across_param(a)?;
    }
    Ok(row)
}

fn index(p: &Value, face: &FaceLayout) -> Result<usize> {
    let i = p.get("row").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok()).ok_or_else(|| bad(CMD, "`row` must be a row index"))?;
    if i < face.rows.len() { Ok(i) } else { Err(bad(CMD, format!("`row` {i} is past the last row ({})", face.rows.len()))) }
}

/// The new layout `p` asks for, starting from `start`; `None` = back to the legacy layout.
fn edited(c: &Cabinet, start: FaceLayout, p: &Value) -> Result<Option<FaceLayout>> {
    let has = |k: &str| p.get(k).is_some();
    let edit_keys = has("kind") || has("height") || has("across");
    let modes = [has("rows"), has("split"), has("equalize"), has("doors"), has("reset"), !has("split") && has("row")].iter().filter(|b| **b).count();
    if modes != 1 {
        return Err(bad(CMD, "give exactly one of `rows`, `row` with `split`, `row` with `kind`/`height`/`across`, `equalize`, `doors`, `reset`"));
    }
    if has("reset") {
        return if p.get("reset") == Some(&Value::Bool(true)) { Ok(None) } else { Err(bad(CMD, "`reset` must be true")) };
    }
    let mut face = start;
    if let Some(rows) = p.get("rows") {
        let list = rows.as_array().ok_or_else(|| bad(CMD, "`rows` must be a list"))?;
        if list.is_empty() || list.len() > MAX_FACE_ROWS {
            return Err(bad(CMD, format!("`rows` takes 1..={MAX_FACE_ROWS} rows")));
        }
        face.rows = list.iter().map(parse_row).collect::<Result<_>>()?;
    } else if let Some(split) = p.get("split") {
        if edit_keys {
            return Err(bad(CMD, "`split` takes only `row` and `count`"));
        }
        let i = index(p, &face)?;
        let count = p
            .get("count")
            .and_then(Value::as_u64)
            .filter(|n| (2..=6).contains(n))
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| bad(CMD, "`count` must be 2..=6"))?;
        match split.as_str() {
            Some("across") => {
                let across =
                    u32::try_from(count).ok().filter(|n| *n <= MAX_ACROSS).ok_or_else(|| bad(CMD, format!("at most {MAX_ACROSS} fronts across")))?;
                if let Some(r) = face.rows.get_mut(i) {
                    r.across = across;
                }
            }
            Some("down") => {
                let sep = face_box(c).separator;
                let row = face.rows.get(i).cloned().ok_or_else(|| bad(CMD, "unknown row"))?;
                if row.kind == FaceKind::Appliance {
                    return Err(bad(CMD, "an appliance row cannot be split"));
                }
                let part = FaceRow { height: row.height.map(|h| (h - (count as f64 - 1.0) * sep) / count as f64), appliance: None, ..row };
                face.rows.splice(i..=i, std::iter::repeat_n(part, count));
            }
            _ => return Err(bad(CMD, "`split` must be \"down\" or \"across\"")),
        }
    } else if has("row") {
        if !edit_keys {
            return Err(bad(CMD, "`row` needs `split`, `kind`, `height` or `across`"));
        }
        let i = index(p, &face)?;
        let kind = p.get("kind").map(row_kind).transpose()?;
        let height = p.get("height").map(row_height).transpose()?;
        let across = p.get("across").map(row_across_param).transpose()?;
        if let Some(r) = face.rows.get_mut(i) {
            if let Some(k) = kind {
                r.kind = k;
                r.appliance = None;
            }
            if let Some(h) = height {
                r.height = h;
            }
            if let Some(a) = across {
                r.across = a;
            }
        }
    } else if has("equalize") {
        if p.get("equalize") != Some(&Value::Bool(true)) {
            return Err(bad(CMD, "`equalize` must be true"));
        }
        face.rows.iter_mut().for_each(|r| r.height = None);
    } else {
        let across = match p.get("doors").and_then(Value::as_str) {
            Some("auto") => 0,
            Some("single") => 1,
            Some("double") => 2,
            _ => return Err(bad(CMD, "`doors` must be \"auto\", \"single\" or \"double\"")),
        };
        face.rows.iter_mut().filter(|r| r.kind == FaceKind::Door).for_each(|r| r.across = across);
    }
    Ok(Some(face))
}

fn run_face(session: &mut Session, params: &Value) -> Result<Value> {
    let raw = params.get("id").and_then(Value::as_str).ok_or_else(|| bad(CMD, "`id` is required"))?;
    let id = parse_id(raw).ok_or_else(|| bad(CMD, format!("unknown cabinet `{raw}`")))?;
    let doc = session.doc()?;
    let idx = doc.arch.cabinets.iter().position(|c| c.id == id).ok_or_else(|| bad(CMD, format!("unknown cabinet `{raw}`")))?;
    let mut cab = doc.arch.cabinets.get(idx).cloned().ok_or_else(|| bad(CMD, format!("unknown cabinet `{raw}`")))?;
    let start = cab.face.clone().unwrap_or_else(|| default_face(&cab));
    cab.face = edited(&cab, start, params)?;
    if let Some(f) = &cab.face
        && row_heights(&cab, f).is_none()
    {
        return Err(bad(CMD, "the rows do not fit the cabinet face (heights must leave room for the reveals, rows at least 0.05 m)"));
    }
    validate_cabinet(&cab).map_err(|e| bad(CMD, e.to_string()))?;
    let result = face_json(&doc.arch, &cab);
    drop(doc);
    let mut d = session.doc_mut()?;
    if let Some(slot) = d.arch.cabinets.get_mut(idx) {
        *slot = cab;
    }
    d.touch_arch();
    Ok(json!({ "cabinet": cabinet_label(id), "face": result }))
}

fn front_key(k: FrontKind) -> &'static str {
    match k {
        FrontKind::Door => "door",
        FrontKind::Drawer => "drawer",
        FrontKind::False => "false",
        FrontKind::Opening => "opening",
        FrontKind::Appliance => "appliance",
    }
}

/// The inspect object of the face of `c`: `null` for the legacy layout.
pub(crate) fn face_json(model: &ArchModel, c: &Cabinet) -> Value {
    let Some(face) = &c.face else { return Value::Null };
    let b = face_box(c);
    let heights = row_heights(c, face).unwrap_or_default();
    let fronts = face_fronts(model, c, face);
    let mut at = 0usize;
    let rows: Vec<Value> = face
        .rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let n = row_across(c, r) as usize;
            let items: Vec<Value> = fronts.iter().skip(at).take(n).map(|f| json!({"kind": front_key(f.kind), "rect": f.rect})).collect();
            at += n;
            json!({
                "kind": r.kind.key(),
                "height": heights.get(i),
                "fixed": r.height.is_some(),
                "across": n,
                "appliance": r.appliance,
                "items": items,
            })
        })
        .collect();
    json!({"height": b.v1 - b.v0, "reveal": b.separator, "rows": rows})
}

/// Add `construction`, `mount` and `face` to the `cabinets` rows already in `out`. Read-only.
pub(crate) fn inspect_cabinet_faces(model: &ArchModel, out: &mut Map<String, Value>) {
    let Some(Value::Array(rows)) = out.get_mut("cabinets") else { return };
    for (row, c) in rows.iter_mut().zip(model.cabinets.iter()) {
        let Value::Object(obj) = row else { continue };
        obj.insert("construction".to_string(), json!(c.construction.key()));
        obj.insert("mount".to_string(), json!(c.mount.key()));
        obj.insert("face".to_string(), face_json(model, c));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> (Session, String) {
        let mut s = Session::new();
        let r = s.execute("cabinet", &json!({"kind": "base", "at": [1.0, 1.0], "width": 0.6, "height": 0.876, "toeKick": true})).unwrap();
        let id = r["cabinets"][0].as_str().unwrap().to_string();
        (s, id)
    }

    fn face(s: &mut Session) -> Value {
        s.execute("arch.inspect", &json!({"cabinets": true})).unwrap()["cabinets"][0]["face"].clone()
    }

    fn cab(s: &Session) -> Cabinet {
        s.doc().unwrap().arch.cabinets[0].clone()
    }

    #[test]
    fn rows_replace_split_down_split_across_kind_height_across_equalize_doors_reset() {
        let (mut s, id) = session();
        let r = s.execute(CMD, &json!({"id": id, "rows": [{"kind": "drawer", "height": 0.15}, {"kind": "door"}]})).unwrap();
        assert_eq!(r["cabinet"], id);
        assert_eq!(r["face"]["rows"].as_array().unwrap().len(), 2);
        assert_eq!(r["face"]["rows"][1]["across"], 1);
        // split down: a shared row stays shared, three equal rows sum to the face
        s.execute(CMD, &json!({"id": id, "row": 1, "split": "down", "count": 3})).unwrap();
        let f = face(&mut s);
        let rows = f["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 4);
        let hs: Vec<f64> = rows.iter().map(|r| r["height"].as_f64().unwrap()).collect();
        assert!((hs[1] - hs[2]).abs() < 1e-9 && (hs[2] - hs[3]).abs() < 1e-9);
        let sum: f64 = hs.iter().sum::<f64>() + 3.0 * f["reveal"].as_f64().unwrap();
        assert!((sum - f["height"].as_f64().unwrap()).abs() < 1e-9);
        assert_eq!(rows[1]["fixed"], false);
        // split a fixed row down
        s.execute(CMD, &json!({"id": id, "row": 0, "split": "down", "count": 2})).unwrap();
        let f = face(&mut s);
        let h = f["rows"][0]["height"].as_f64().unwrap();
        assert!((2.0 * h + 0.003 - 0.15).abs() < 1e-9);
        assert_eq!(f["rows"][0]["fixed"], true);
        // split across, edits
        s.execute(CMD, &json!({"id": id, "row": 2, "split": "across", "count": 2})).unwrap();
        assert_eq!(face(&mut s)["rows"][2]["across"], 2);
        assert_eq!(face(&mut s)["rows"][2]["items"].as_array().unwrap().len(), 2);
        s.execute(CMD, &json!({"id": id, "row": 2, "kind": "false", "across": 1})).unwrap();
        assert_eq!(face(&mut s)["rows"][2]["kind"], "false");
        s.execute(CMD, &json!({"id": id, "row": 2, "height": 0.1})).unwrap();
        assert_eq!(face(&mut s)["rows"][2]["fixed"], true);
        s.execute(CMD, &json!({"id": id, "row": 2, "height": null})).unwrap();
        assert_eq!(face(&mut s)["rows"][2]["fixed"], false);
        s.execute(CMD, &json!({"id": id, "equalize": true})).unwrap();
        assert!(face(&mut s)["rows"].as_array().unwrap().iter().all(|r| r["fixed"] == false));
        s.execute(CMD, &json!({"id": id, "rows": [{"kind": "door"}]})).unwrap();
        s.execute(CMD, &json!({"id": id, "doors": "double"})).unwrap();
        assert_eq!(face(&mut s)["rows"][0]["across"], 2);
        s.execute(CMD, &json!({"id": id, "doors": "single"})).unwrap();
        assert_eq!(face(&mut s)["rows"][0]["across"], 1);
        s.execute(CMD, &json!({"id": id, "doors": "auto"})).unwrap();
        assert_eq!(cab(&s).face.unwrap().rows[0].across, 0);
        s.execute(CMD, &json!({"id": id, "reset": true})).unwrap();
        assert!(cab(&s).face.is_none());
        assert!(face(&mut s).is_null());
    }

    #[test]
    fn face_refusals_leave_nothing() {
        let (mut s, id) = session();
        s.execute(CMD, &json!({"id": id, "rows": [{"kind": "drawer"}, {"kind": "door"}]})).unwrap();
        let before = cab(&s);
        let nine: Vec<Value> = (0..9).map(|_| json!({"kind": "door"})).collect();
        let bads = [
            json!({"id": id}),
            json!({"id": "cb9", "reset": true}),
            json!({"id": id, "rows": []}),
            json!({"id": id, "rows": nine}),
            json!({"id": id, "rows": [{"kind": "sofa"}]}),
            json!({"id": id, "rows": [{"kind": "appliance"}]}),
            json!({"id": id, "rows": [{"kind": "door", "height": 0.01}]}),
            json!({"id": id, "rows": [{"kind": "door", "height": 5.0}]}),
            json!({"id": id, "rows": [{"kind": "door", "height": -1.0}]}),
            json!({"id": id, "rows": [{"kind": "door", "across": 5}]}),
            json!({"id": id, "rows": [{"kind": "door", "height": "x"}]}),
            json!({"id": id, "row": 5, "split": "down", "count": 2}),
            json!({"id": id, "row": 0, "split": "down", "count": 1}),
            json!({"id": id, "row": 0, "split": "down", "count": 7}),
            json!({"id": id, "row": 0, "split": "across", "count": 5}),
            json!({"id": id, "row": 0, "split": "diagonal", "count": 2}),
            json!({"id": id, "row": 0}),
            json!({"id": id, "row": 0, "kind": "door", "reset": true}),
            json!({"id": id, "equalize": false}),
            json!({"id": id, "doors": "triple"}),
            json!({"id": id, "reset": false}),
            json!({"id": id, "row": 0, "height": 0.75}),
        ];
        for p in bads {
            assert!(s.execute(CMD, &p).is_err(), "{p}");
            assert_eq!(cab(&s), before, "{p}");
        }
        // splitting past the row cap is refused
        s.execute(CMD, &json!({"id": id, "row": 0, "split": "down", "count": 6})).unwrap();
        let before = cab(&s);
        assert!(s.execute(CMD, &json!({"id": id, "row": 0, "split": "down", "count": 6})).is_err());
        assert_eq!(cab(&s), before);
    }

    #[test]
    fn first_edit_starts_from_the_legacy_layout() {
        let (mut s, id) = session();
        assert!(cab(&s).face.is_none());
        let c0 = cab(&s);
        s.execute(CMD, &json!({"id": id, "row": 0, "kind": "drawer"})).unwrap();
        let f = cab(&s).face.unwrap();
        assert_eq!(f.rows.len(), default_face(&c0).rows.len());
        assert_eq!(f.rows[0].kind, FaceKind::Drawer);
        let (mut s, id) = session();
        s.execute(CMD, &json!({"id": id, "doors": "double"})).unwrap();
        let rows = cab(&s).face.unwrap().rows;
        assert!(rows.iter().filter(|r| r.kind == FaceKind::Door).all(|r| r.across == 2));
        assert!(rows.iter().any(|r| r.kind == FaceKind::Door));
    }

    #[test]
    fn inspect_face_rows() {
        let (mut s, id) = session();
        let v = s.execute("arch.inspect", &json!({"cabinets": true})).unwrap();
        assert!(v["cabinets"][0]["face"].is_null());
        assert_eq!(v["cabinets"][0]["construction"], "frameless");
        s.execute(CMD, &json!({"id": id, "rows": [{"kind": "drawer", "height": 0.15}, {"kind": "door", "across": 2}]})).unwrap();
        let v = s.execute("arch.inspect", &json!({"cabinets": true})).unwrap();
        let f = &v["cabinets"][0]["face"];
        assert!((f["height"].as_f64().unwrap() - 0.776).abs() < 1e-9);
        assert_eq!(f["reveal"], 0.003);
        let rows = f["rows"].as_array().unwrap();
        assert_eq!(rows[0]["kind"], "drawer");
        assert_eq!(rows[0]["fixed"], true);
        assert!(rows[0]["appliance"].is_null());
        assert_eq!(rows[1]["across"], 2);
        assert_eq!(rows[1]["items"].as_array().unwrap().len(), 2);
        assert_eq!(rows[1]["items"][0]["rect"].as_array().unwrap().len(), 4);
        assert_eq!(v["cabinets"][0]["frontParts"].as_array().unwrap().len(), 3);
    }
}
