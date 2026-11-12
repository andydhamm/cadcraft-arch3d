//! The schedule and sheet part of `arch.inspect` (phase 21): `{"schedules": true}` lists the
//! schedules with their rows, `{"sheets": true}` the sheets with their viewports, images and
//! title blocks, and the sheet set. Read-only.

use cadcraft_doc::arch::sheet::{LINK_SCHEDULE, LINK_SHEET_IMAGE, LINK_SHEET_VIEW, LINK_TITLE, read_link, schedule_label};
use cadcraft_doc::{Drawing, Entity, EntityKind, EntityStore};
use serde_json::{Map as JsonMap, Value, json};

fn flag(p: &Value, key: &str) -> bool {
    p.get(key).and_then(Value::as_bool) == Some(true)
}

/// A linked entity: (role, key, number, entity).
type Linked<'a> = (String, String, u32, &'a Entity);

/// Every entity of `store` that carries a link, in handle order.
fn links(store: &EntityStore) -> Vec<Linked<'_>> {
    let mut v: Vec<Linked<'_>> = store.iter().filter_map(|e| read_link(&e.common.xdata).map(|(r, k, n)| (r, k, n, e.as_ref()))).collect();
    v.sort_by_key(|l| l.3.handle.0);
    v
}

/// Add `schedules` when `{"schedules": true}`.
pub(crate) fn inspect_schedules(d: &Drawing, p: &Value, out: &mut JsonMap<String, Value>) {
    if !flag(p, "schedules") {
        return;
    }
    let found = links(&d.model);
    let list: Vec<Value> = d
        .arch
        .schedules
        .iter()
        .map(|s| {
            let label = schedule_label(s.id);
            let table = found
                .iter()
                .find(|(r, k, _, e)| r == LINK_SCHEDULE && *k == label && matches!(e.kind, EntityKind::Table(_)))
                .map(|l| l.3.handle.hex());
            let rows: Vec<Value> = cadcraft_arch::schedule::rows(&d.arch, s)
                .iter()
                .map(|r| json!({"number": r.number, "count": r.count, "members": r.members, "cells": r.cells}))
                .collect();
            json!({
                "id": label,
                "name": s.name,
                "kind": s.kind.key(),
                "columns": s.columns,
                "level": s.level,
                "table": table,
                "rows": rows,
            })
        })
        .collect();
    out.insert("schedules".to_string(), Value::Array(list));
}

/// Add `sheets` and `sheetSet` when `{"sheets": true}`.
pub(crate) fn inspect_sheets(d: &Drawing, p: &Value, out: &mut JsonMap<String, Value>) {
    if !flag(p, "sheets") {
        return;
    }
    let list: Vec<Value> = d
        .arch
        .sheets
        .iter()
        .map(|sh| {
            let found = d.layout(&sh.name).map(|l| links(&l.entities)).unwrap_or_default();
            let for_sheet = |role: &str, n: u32| found.iter().find(|(r, k, num, _)| r == role && *k == sh.name && *num == n);
            let paper = cadcraft_render::paper_size(&sh.size).map(|ps| {
                let (lo, hi) = (ps.width_mm.min(ps.height_mm), ps.width_mm.max(ps.height_mm));
                if sh.landscape { [hi, lo] } else { [lo, hi] }
            });
            let views: Vec<Value> = sh
                .views
                .iter()
                .map(|v| {
                    let viewport =
                        for_sheet(LINK_SHEET_VIEW, v.number).filter(|l| matches!(l.3.kind, EntityKind::Viewport(_))).map(|l| l.3.handle.hex());
                    let image = for_sheet(LINK_SHEET_IMAGE, v.number).and_then(|l| match &l.3.kind {
                        EntityKind::Image(i) => Some(i.path.clone()),
                        _ => None,
                    });
                    json!({
                        "number": v.number,
                        "source": {"kind": v.source.kind_key(&d.arch), "name": v.source.name()},
                        "scale": v.scale,
                        "at": v.at,
                        "size": v.size,
                        "viewport": viewport,
                        "image": image,
                    })
                })
                .collect();
            let mut title = JsonMap::new();
            if let Some(l) = for_sheet(LINK_TITLE, 0)
                && let EntityKind::Insert(ins) = &l.3.kind
            {
                for (key, tag) in
                    [("sheetNumber", "SHEETNO"), ("sheetTitle", "SHEETTITLE"), ("project", "PROJECT"), ("date", "DATE"), ("scale", "SCALE")]
                {
                    if let Some(a) = ins.attribs.iter().find(|a| a.tag.eq_ignore_ascii_case(tag)) {
                        title.insert(key.to_string(), json!(a.text.value));
                    }
                }
            }
            json!({
                "name": sh.name,
                "number": sh.number,
                "title": sh.title,
                "layout": sh.name,
                "size": sh.size,
                "landscape": sh.landscape,
                "paper": paper,
                "views": views,
                "titleBlock": Value::Object(title),
            })
        })
        .collect();
    out.insert("sheets".to_string(), Value::Array(list));
    out.insert("sheetSet".to_string(), json!({"project": d.arch.sheet_set.project, "date": d.arch.sheet_set.date}));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use cadraft_doc::arch::sheet::{Schedule, Sheet, SheetSet};
    use cadraft_doc::arch::{ArchId, Opening, OpeningKind};

    fn inspect(s: &mut Session, p: Value) -> Value {
        s.execute("arch.inspect", &p).unwrap_or(Value::Null)
    }

    #[test]
    fn inspect_reports_schedules_sheets_and_sheet_set() {
        let mut s = Session::new();
        let v = inspect(&mut s, json!({"schedules": true, "sheets": true}));
        assert_eq!(v["schedules"], json!([]), "{v}");
        assert_eq!(v["sheets"], json!([]), "{v}");
        assert_eq!(v["sheetSet"], json!({"project": "", "date": ""}), "{v}");
        {
            if let Ok(d) = s.doc_mut() {
                d.arch.schedules.push(Schedule { name: "Doors".into(), columns: vec!["number".into()], ..Schedule::default() });
                d.arch.sheets.push(Sheet { name: "A101".into(), number: "A101".into(), ..Sheet::default() });
                d.arch.sheet_set = SheetSet { project: "House".into(), date: "2026-11-12".into(), ..SheetSet::default() };
            }
        }
        let v = inspect(&mut s, json!({"schedules": true, "sheets": true}));
        assert_eq!(v["schedules"][0]["id"], "sc0", "{v}");
        assert_eq!(v["schedules"][0]["kind"], "door", "{v}");
        assert!(v["schedules"][0]["table"].is_null(), "{v}");
        assert_eq!(v["sheets"][0]["layout"], "A101", "{v}");
        assert_eq!(v["sheets"][0]["size"], "ARCH D", "{v}");
        assert_eq!(v["sheets"][0]["titleBlock"], json!({}), "{v}");
        assert_eq!(v["sheetSet"]["project"], "House", "{v}");
    }

    #[test]
    fn inspect_numbers_are_null_with_the_stub() {
        let mut s = Session::new();
        if let Ok(d) = s.doc_mut() {
            let wid = d.arch.push_wall(cadcraft_doc::Wall {
                start: cadcraft_geom::Vec2::new(0.0, 0.0),
                end: cadcraft_geom::Vec2::new(5.0, 0.0),
                thickness: 0.2,
                height: 2.7,
                ..cadcraft_doc::Wall::default()
            });
            d.arch.push_opening(Opening::new(wid, 1.0, 0.9, 2.1, OpeningKind::Door));
            d.touch_arch();
        }
        let v = inspect(&mut s, json!({"openings": true, "cabinets": true}));
        let rows = v["openings"].as_array().cloned().unwrap_or_default();
        assert!(!rows.is_empty(), "{v}");
        for r in &rows {
            assert!(r.get("number").is_some_and(Value::is_null), "{r}");
        }
        assert_eq!(v["cabinets"], json!([]), "{v}");
    }
}
