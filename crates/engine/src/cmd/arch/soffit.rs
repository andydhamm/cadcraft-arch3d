//! `soffit`, `soffit.edit` and `soffit.delete` (phase 16), and the `soffits` rows of `arch.inspect`.
//!
//! A soffit is a box hung from the ceiling over a plan outline: a `rect` (two corners, snapped to
//! nearby wall faces), a closed `polygon`, or the wall cabinets it fills `over`. Its bottom is given
//! as a height above the level's floor (`bottom`), a depth below the finished ceiling (`depth`), or
//! comes from the cabinets' tops. Every change reseats the ceiling items on the level
//! (`seated_ceiling_items` before, `reseat` after) so lights and furnishings follow the underside.

use cadcraft_arch::ceiling::soffit::{interior_point, snap_rect, soffit_bottom, soffit_ring, validate};
use cadcraft_arch::ceiling::{finished_ceiling_at, reseat, room_at_point, seated_ceiling_items};
use cadcraft_doc::arch::{ArchId, ArchModel, CEILING_LAYER, Soffit, cabinet_label, handle_json, room_label, soffit_handle, soffit_label};
use cadcraft_geom::Vec2;
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, bad, str_param};
use crate::{Result, Session};

/// Most vertices in a `polygon` and most cabinets in `over` a command accepts.
const MAX_LIST: usize = 256;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("soffit", "Soffit", run_soffit).menu(&["Architecture", "Soffit"]).params(
            "{rect: [[x0,y0],[x1,y1]] | polygon: [[x,y],...] | over: [\"cb3\",...], bottom?: m | depth?: m, material?: key, level?: name | index}",
        ),
        CommandSpec::new("soffit.edit", "Edit Soffit", run_soffit_edit)
            .menu(&["Architecture", "Edit Soffit"])
            .params("{id: \"sf0\", rect?: [[x0,y0],[x1,y1]] | polygon?: [[x,y],...], bottom?: m | depth?: m, material?: key | null}"),
        CommandSpec::new("soffit.delete", "Delete Soffit", run_soffit_delete).menu(&["Architecture", "Delete Soffit"]).params("{id: \"sf0\"}"),
    ]
}

/// A level named by `raw`: its name (exactly, else ignoring case), or its index as a number or a
/// numeric string.
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

/// `sf<n>` (or a bare number) to an id.
fn parse_id(cmd: &str, p: &Value) -> Result<ArchId> {
    let raw = match p.get("id") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => return Err(bad(cmd, "`id` (\"sf0\") is required")),
    };
    let t = raw.trim();
    let digits = t.get(..2).filter(|pre| pre.eq_ignore_ascii_case("sf")).and_then(|_| t.get(2..)).unwrap_or(t);
    digits.parse::<u32>().map(ArchId).map_err(|_| bad(cmd, format!("unknown soffit `{raw}`")))
}

/// `[x, y]` of two finite numbers.
fn parse_point(cmd: &str, key: &str, v: &Value) -> Result<Vec2> {
    let pair = v.as_array().filter(|a| a.len() == 2);
    let x = pair.and_then(|a| a.first()).and_then(Value::as_f64).filter(|n| n.is_finite());
    let y = pair.and_then(|a| a.get(1)).and_then(Value::as_f64).filter(|n| n.is_finite());
    match (x, y) {
        (Some(x), Some(y)) => Ok(Vec2::new(x, y)),
        _ => Err(bad(cmd, format!("`{key}` holds a point that is not [x, y] of finite numbers"))),
    }
}

/// `rect`: two corners in any order, as the stored 4-point CCW outline snapped to wall faces.
fn parse_rect(cmd: &str, model: &ArchModel, level: u32, v: &Value) -> Result<Vec<Vec2>> {
    let corners = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| bad(cmd, "`rect` must be [[x0, y0], [x1, y1]]"))?;
    let a = parse_point(cmd, "rect", &corners[0])?;
    let b = parse_point(cmd, "rect", &corners[1])?;
    let r = snap_rect(model, level, [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)]);
    Ok(vec![Vec2::new(r[0], r[1]), Vec2::new(r[2], r[1]), Vec2::new(r[2], r[3]), Vec2::new(r[0], r[3])])
}

/// `polygon`: a closed ring of at least 3 finite points; a trailing duplicate of the first is dropped.
fn parse_polygon(cmd: &str, v: &Value) -> Result<Vec<Vec2>> {
    let items =
        v.as_array().filter(|a| a.len() <= MAX_LIST).ok_or_else(|| bad(cmd, format!("`polygon` must be a list of at most {MAX_LIST} points")))?;
    let mut ring = items.iter().map(|pt| parse_point(cmd, "polygon", pt)).collect::<Result<Vec<_>>>()?;
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    if ring.len() < 3 {
        return Err(bad(cmd, "`polygon` needs at least 3 distinct points"));
    }
    Ok(ring)
}

/// `over`: a non-empty array of `cb<n>`.
fn parse_over(cmd: &str, v: &Value) -> Result<Vec<ArchId>> {
    let items = v
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= MAX_LIST)
        .ok_or_else(|| bad(cmd, format!("`over` must list 1 to {MAX_LIST} cabinets (\"cb3\")")))?;
    items
        .iter()
        .map(|item| {
            let s = item.as_str().map(str::trim).unwrap_or_default();
            s.get(..2)
                .filter(|pre| pre.eq_ignore_ascii_case("cb"))
                .and_then(|_| s.get(2..))
                .and_then(|d| d.parse::<u32>().ok())
                .map(ArchId)
                .ok_or_else(|| bad(cmd, format!("`over` holds {item}, not a cabinet id like \"cb3\"")))
        })
        .collect()
}

/// A finite number under `key` (not null).
fn parse_metres(cmd: &str, p: &Value, key: &str) -> Result<Option<f64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|n| n.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite number of metres"))),
    }
}

/// `material`: a non-empty key of at most 128 characters.
fn parse_material(cmd: &str, v: &Value) -> Result<String> {
    match v.as_str() {
        Some(s) if !s.trim().is_empty() && s.chars().count() <= 128 => Ok(s.to_string()),
        _ => Err(bad(cmd, "`material` must be a non-empty key of at most 128 characters")),
    }
}

fn point_json(p: Vec2) -> Value {
    json!([p.x, p.y])
}

fn ring_json(model: &ArchModel, s: &Soffit) -> Value {
    Value::Array(soffit_ring(model, s).unwrap_or_default().into_iter().map(point_json).collect())
}

/// The reply of `soffit` and `soffit.edit`.
fn reply(model: &ArchModel, s: &Soffit) -> Value {
    let ip = soffit_ring(model, s).and_then(|r| interior_point(&r));
    json!({
        "soffit": soffit_label(s.id),
        "handle": handle_json(soffit_handle(s.id)),
        "polygon": ring_json(model, s),
        "bottom": soffit_bottom(model, s),
        "top": ip.and_then(|p| finished_ceiling_at(model, s.level, p)),
        "moldings": !s.no_moldings,
        "room": ip.and_then(|p| room_at_point(model, s.level, p)).map(|r| room_label(r.id)),
    })
}

fn run_soffit(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "soffit";
    let model = s.doc()?.arch.clone();
    let level = match p.get("level").filter(|v| !v.is_null()) {
        Some(raw) => parse_level(cmd, &model, raw)?,
        None => 0,
    };
    let given = |k: &str| p.get(k).is_some_and(|v| !v.is_null());
    if [given("rect"), given("polygon"), given("over")].iter().filter(|g| **g).count() != 1 {
        return Err(bad(cmd, "exactly one of `rect`, `polygon` or `over` is required"));
    }
    let mut cand = Soffit { id: ArchId(model.next_soffit_id), level, layer: CEILING_LAYER.to_string(), ..Default::default() };
    if let Some(v) = p.get("moldings").filter(|v| !v.is_null()) {
        cand.no_moldings = v.as_bool().map(|b| !b).ok_or_else(|| bad(cmd, "`moldings` must be a boolean"))?;
    }
    if let Some(v) = p.get("rect").filter(|v| !v.is_null()) {
        cand.polygon = parse_rect(cmd, &model, level, v)?;
    } else if let Some(v) = p.get("polygon").filter(|v| !v.is_null()) {
        cand.polygon = parse_polygon(cmd, v)?;
    } else if let Some(v) = p.get("over") {
        cand.over = parse_over(cmd, v)?;
        if given("bottom") || given("depth") {
            return Err(bad(cmd, "`over` takes neither `bottom` nor `depth`"));
        }
    }
    cand.bottom = parse_metres(cmd, p, "bottom")?;
    cand.depth = parse_metres(cmd, p, "depth")?;
    if let Some(v) = p.get("material").filter(|v| !v.is_null()) {
        cand.material = Some(parse_material(cmd, v)?);
    }
    validate(&model, &cand).map_err(|e| bad(cmd, e))?;

    let doc = s.doc_mut()?;
    let seated = seated_ceiling_items(&doc.arch, level);
    let id = doc.arch.push_soffit(cand);
    reseat(&mut doc.arch, &seated);
    doc.ensure_layer(CEILING_LAYER);
    doc.touch_arch();
    let row = doc.arch.soffit(id).map_or(Value::Null, |s| reply(&doc.arch, s));
    Ok(row)
}

fn run_soffit_edit(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "soffit.edit";
    let id = parse_id(cmd, p)?;
    let model = s.doc()?.arch.clone();
    let current = model.soffit(id).ok_or_else(|| bad(cmd, format!("unknown soffit `{}`", soffit_label(id))))?.clone();
    let given = |k: &str| p.get(k).is_some_and(|v| !v.is_null());
    if given("rect") && given("polygon") {
        return Err(bad(cmd, "give `rect` or `polygon`, not both"));
    }
    if given("bottom") as u8 + given("depth") as u8 + given("over") as u8 > 1 {
        return Err(bad(cmd, "give at most one of `bottom`, `depth` or `over`"));
    }
    let mut cand = current.clone();
    if let Some(v) = p.get("rect").filter(|v| !v.is_null()) {
        cand.polygon = parse_rect(cmd, &model, cand.level, v)?;
    } else if let Some(v) = p.get("polygon").filter(|v| !v.is_null()) {
        cand.polygon = parse_polygon(cmd, v)?;
    }
    if let Some(v) = p.get("moldings").filter(|v| !v.is_null()) {
        cand.no_moldings = v.as_bool().map(|b| !b).ok_or_else(|| bad(cmd, "`moldings` must be a boolean"))?;
    }
    if let Some(v) = p.get("over").filter(|v| !v.is_null()) {
        cand.over = parse_over(cmd, v)?;
        cand.bottom = None;
        cand.depth = None;
    } else if let Some(b) = parse_metres(cmd, p, "bottom")? {
        cand.bottom = Some(b);
        cand.depth = None;
        cand.over.clear();
    } else if let Some(d) = parse_metres(cmd, p, "depth")? {
        cand.depth = Some(d);
        cand.bottom = None;
        cand.over.clear();
    }
    if (given("rect") || given("polygon")) && !cand.over.is_empty() {
        return Err(bad(cmd, "a soffit over cabinets takes its outline from them: give `bottom` or `depth` to replace them first"));
    }
    match p.get("material") {
        None => {}
        Some(Value::Null) => cand.material = None,
        Some(v) => cand.material = Some(parse_material(cmd, v)?),
    }
    // The soffit being edited does not count against the cap.
    let mut others = model.clone();
    others.soffits.retain(|o| o.id != id);
    validate(&others, &cand).map_err(|e| bad(cmd, e))?;

    let doc = s.doc_mut()?;
    let seated = seated_ceiling_items(&doc.arch, cand.level);
    let Some(slot) = doc.arch.soffits.iter_mut().find(|o| o.id == id) else {
        return Err(bad(cmd, format!("unknown soffit `{}`", soffit_label(id))));
    };
    *slot = cand.clone();
    reseat(&mut doc.arch, &seated);
    doc.touch_arch();
    Ok(reply(&doc.arch, &cand))
}

fn run_soffit_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "soffit.delete";
    let id = parse_id(cmd, p)?;
    let level = s.doc()?.arch.soffit(id).map(|o| o.level).ok_or_else(|| bad(cmd, format!("unknown soffit `{}`", soffit_label(id))))?;
    let doc = s.doc_mut()?;
    let seated = seated_ceiling_items(&doc.arch, level);
    doc.arch.soffits.retain(|o| o.id != id);
    reseat(&mut doc.arch, &seated);
    doc.touch_arch();
    Ok(json!({ "deleted": soffit_label(id) }))
}

/// A soffit's `arch.inspect` row.
fn soffit_row(model: &ArchModel, s: &Soffit) -> Value {
    let ring = soffit_ring(model, s).unwrap_or_default();
    let tops = ring.iter().filter(|v| v.is_finite()).filter_map(|v| finished_ceiling_at(model, s.level, *v));
    let (lo, hi) = tops.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), z| (lo.min(z), hi.max(z)));
    let top = if lo.is_finite() && hi.is_finite() { json!([lo, hi]) } else { Value::Null };
    let room = interior_point(&ring).and_then(|p| room_at_point(model, s.level, p)).map(|r| room_label(r.id));
    json!({
        "id": soffit_label(s.id),
        "handle": handle_json(soffit_handle(s.id)),
        "level": s.level,
        "polygon": Value::Array(ring.into_iter().map(point_json).collect()),
        "bottom": soffit_bottom(model, s),
        "top": top,
        "depth": s.depth,
        "over": s.over.iter().map(|c| cabinet_label(*c)).collect::<Vec<_>>(),
        "moldings": !s.no_moldings,
        "material": s.material,
        "room": room,
        "layer": s.layer,
    })
}

/// `arch.inspect {"soffits": true}`: one row per soffit, in model order.
pub(crate) fn inspect_soffits(model: &ArchModel, p: &Value, out: &mut Map<String, Value>) {
    if p.get("soffits").and_then(Value::as_bool) != Some(true) {
        return;
    }
    let rows = model.soffits.iter().map(|s| soffit_row(model, s)).collect();
    out.insert("soffits".to_string(), Value::Array(rows));
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// The phase-1 house through the commands (exterior ring, one interior wall along y = 3 and
    /// one along x = 6 above it), rooms detected.
    fn house() -> Session {
        let mut s = Session::new();
        let walls = [
            json!({"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": true}),
            json!({"points": [[0, 3], [10, 3]], "thickness": 0.1, "height": 2.4, "type": "interior"}),
            json!({"points": [[6, 3], [6, 8]], "thickness": 0.1, "height": 2.4, "type": "interior"}),
        ];
        for w in &walls {
            s.execute("wall", w).unwrap();
        }
        s.execute("roomdetect", &json!({})).unwrap();
        s
    }

    fn undo_len(s: &Session) -> usize {
        s.state().map(|st| st.undo.len()).unwrap_or_default()
    }

    fn near(a: &Value, b: f64) -> bool {
        a.as_f64().is_some_and(|x| (x - b).abs() < 1e-6)
    }

    #[test]
    fn soffit_create_edit_delete_are_single_undo_steps() {
        let mut s = house();
        let n = undo_len(&s);
        let r = s.execute("soffit", &json!({"rect": [[7.0, 7.3], [9.0, 7.9]], "bottom": 2.1})).unwrap();
        assert_eq!(r["soffit"], "sf0");
        assert_eq!(undo_len(&s), n + 1);
        assert!(near(&r["bottom"], 2.1) && near(&r["top"], 2.4), "{r}");
        {
            let d = s.doc().unwrap();
            assert_eq!(d.arch.soffits.len(), 1);
            assert!(d.layer("A-CLNG").is_some());
        }
        let r = s.execute("soffit.edit", &json!({"id": "sf0", "depth": 0.4})).unwrap();
        assert!(near(&r["bottom"], 2.0), "{r}");
        assert_eq!(undo_len(&s), n + 2);
        {
            let d = s.doc().unwrap();
            assert_eq!((d.arch.soffits[0].bottom, d.arch.soffits[0].depth), (None, Some(0.4)));
        }
        assert_eq!(s.undo().unwrap().as_deref(), Some("Edit Soffit"));
        assert_eq!(s.doc().unwrap().arch.soffits[0].bottom, Some(2.1));
        let r = s.execute("soffit.delete", &json!({"id": "sf0"})).unwrap();
        assert_eq!(r["deleted"], "sf0");
        assert_eq!(undo_len(&s), n + 2);
        assert!(s.doc().unwrap().arch.soffits.is_empty());
        assert_eq!(s.undo().unwrap().as_deref(), Some("Delete Soffit"));
        assert_eq!(s.doc().unwrap().arch.soffits.len(), 1);
        assert_eq!(s.undo().unwrap().as_deref(), Some("Soffit"));
        assert!(s.doc().unwrap().arch.soffits.is_empty());
    }

    #[test]
    fn soffit_refusals_leave_nothing() {
        let mut s = house();
        s.execute("soffit", &json!({"rect": [[7.0, 7.3], [9.0, 7.9]], "bottom": 2.1})).unwrap();
        let before = s.doc().unwrap().clone();
        let n = undo_len(&s);
        let rect = json!([[6.5, 3.5], [7.3, 4.3]]);
        let long = "m".repeat(129);
        for (c, p) in [
            ("soffit", json!({})),
            ("soffit", json!({"rect": rect, "bottom": 2.1, "depth": 0.3})),
            ("soffit", json!({"over": ["cb0"], "bottom": 2.1})),
            ("soffit", json!({"rect": rect})),
            ("soffit", json!({"rect": [[6.5, 3.5], [6.55, 3.55]], "bottom": 2.1})),
            ("soffit", json!({"rect": [[20.0, 20.0], [21.0, 21.0]], "bottom": 2.1})),
            ("soffit", json!({"rect": rect, "bottom": 0.2})),
            ("soffit", json!({"rect": rect, "bottom": 2.5})),
            ("soffit", json!({"rect": rect, "depth": 0.005})),
            ("soffit", json!({"rect": rect, "depth": 3.5})),
            ("soffit", json!({"rect": rect, "bottom": 2.1, "material": ""})),
            ("soffit", json!({"rect": rect, "bottom": 2.1, "material": long})),
            ("soffit", json!({"polygon": [[6.5, 3.5], [7.3, 4.3]], "bottom": 2.1})),
            ("soffit", json!({"rect": [[6.5, 3.5], [null, 4.3]], "bottom": 2.1})),
            ("soffit", json!({"rect": [[6.5, 3.5], [f64::INFINITY, 4.3]], "bottom": 2.1})),
            ("soffit", json!({"over": ["cb9"]})),
            ("soffit", json!({"rect": rect, "bottom": 2.1, "level": "Moon"})),
            ("soffit.edit", json!({"id": "sf9", "bottom": 2.1})),
            ("soffit.edit", json!({"id": "sf0", "bottom": 0.2})),
            ("soffit.edit", json!({"id": "sf0", "bottom": 2.1, "depth": 0.3})),
            ("soffit.edit", json!({"id": "sf0", "rect": rect, "polygon": [[0, 0], [1, 0], [1, 1]]})),
            ("soffit.edit", json!({"id": "sf0", "material": ""})),
            ("soffit.edit", json!({"bottom": 2.1})),
            ("soffit.delete", json!({"id": "sf9"})),
            ("soffit.delete", json!({})),
        ] {
            assert!(s.execute(c, &p).is_err(), "{c} {p} is refused");
        }
        assert_eq!(*s.doc().unwrap(), before, "nothing changed");
        assert_eq!(undo_len(&s), n, "no undo step");
    }

    #[test]
    fn inspect_soffits_rows() {
        let mut s = house();
        s.execute("soffit", &json!({"rect": [[7.0, 7.3], [9.0, 7.9]], "depth": 0.3, "material": "plaster"})).unwrap();
        let r = s.execute("arch.inspect", &json!({"walls": true})).unwrap();
        assert!(r.get("soffits").is_none(), "only on request");
        let r = s.execute("arch.inspect", &json!({"soffits": true})).unwrap();
        let rows = r["soffits"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row["id"], "sf0");
        assert_eq!(row["handle"], handle_json(soffit_handle(ArchId(0))));
        assert_eq!(row["level"], 0);
        assert_eq!(row["polygon"].as_array().map(Vec::len), Some(4));
        assert!(near(&row["bottom"], 2.1), "{row}");
        assert!(near(&row["top"][0], 2.4) && near(&row["top"][1], 2.4), "{row}");
        assert!(near(&row["depth"], 0.3));
        assert!(row["over"].as_array().unwrap().is_empty());
        assert_eq!(row["material"], "plaster");
        assert!(row["room"].as_str().is_some_and(|r| r.starts_with('r')), "{row}");
        assert_eq!(row["layer"], "A-CLNG");
    }

    #[test]
    fn moving_a_soffit_reseats_ceiling_items_and_lights() {
        crate::library::testing::install_io();
        let mut s = house();
        s.execute("library.place", &json!({"item": "light-pendant", "at": [8.0, 7.6]})).unwrap();
        s.execute("lightfixture", &json!({"type": "Surface ceiling fixture", "at": [8.0, 7.6]})).unwrap();
        let top = |s: &Session| {
            let m = &s.doc().unwrap().arch;
            m.furnishings.first().map_or(f64::NAN, |f| f.elevation + f.size[2])
        };
        let light_z = |s: &mut Session| s.execute("arch.inspect", &json!({"lights": true})).unwrap()["lights"][0]["z"].as_f64().unwrap_or(f64::NAN);
        assert!((top(&s) - 2.4).abs() < 1e-6 && (light_z(&mut s) - 2.4).abs() < 0.06);
        s.execute("soffit", &json!({"rect": [[7.0, 7.3], [9.0, 7.9]], "bottom": 2.1})).unwrap();
        assert!((top(&s) - 2.1).abs() < 1e-6, "{}", top(&s));
        assert!((light_z(&mut s) - 2.05).abs() < 1e-6, "{}", light_z(&mut s));
        s.execute("soffit.edit", &json!({"id": "sf0", "rect": [[6.5, 3.5], [7.3, 4.3]]})).unwrap();
        assert!((top(&s) - 2.4).abs() < 1e-6, "{}", top(&s));
        assert!((light_z(&mut s) - 2.35).abs() < 1e-6, "{}", light_z(&mut s));
        s.execute("soffit.edit", &json!({"id": "sf0", "rect": [[7.0, 7.3], [9.0, 7.9]]})).unwrap();
        assert!((top(&s) - 2.1).abs() < 1e-6 && (light_z(&mut s) - 2.05).abs() < 1e-6);
    }
}
