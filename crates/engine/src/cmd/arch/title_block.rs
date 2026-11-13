//! The master title block of a sheet (phase 21).

use std::collections::BTreeMap;

use cadcraft_doc::arch::sheet::{LAYER_TITLE, LINK_TITLE, TITLE_BLOCK, read_link, set_link};
use cadcraft_doc::{Attrib, Block, Common, Entity, EntityKind, HAlign, Text, VAlign};
use cadcraft_geom::{Vec2, Vec3};
use serde_json::{Map as JsonMap, Value};

use crate::Session;
use crate::cmd::blocks;
use crate::error::Result;

fn scale_macro(sheet: &cadcraft_doc::arch::sheet::Sheet) -> String {
    let mut values: Vec<f64> = sheet.views.iter().map(|v| v.scale).collect();
    if values.is_empty() {
        return String::new();
    }
    values.sort_by(|a, b| a.total_cmp(b));
    values.dedup();
    let first = values[0];
    if values.iter().all(|v| (v - first).abs() <= 1e-9) {
        if first.fract() == 0.0 { format!("1:{}", first as u64) } else { format!("1:{first}") }
    } else {
        "As noted".to_string()
    }
}

fn expand_macros(input: &str, values: &BTreeMap<&str, String>) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' {
            let end = chars[i + 1..].iter().position(|c| *c == '%');
            match end {
                Some(pos) => {
                    let name: String = chars[i + 1..i + 1 + pos].iter().collect();
                    if let Some(v) = values.get(name.as_str()) {
                        out.push_str(v);
                        i += pos + 2;
                        continue;
                    }
                    out.push('%');
                    i += 1;
                    continue;
                }
                None => {
                    out.push('%');
                    i += 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn title_attr_values(sheet: &cadcraft_doc::arch::sheet::Sheet, project: &str, date: &str) -> JsonMap<String, Value> {
    let scale = scale_macro(sheet);
    let mut values = BTreeMap::new();
    values.insert("sheet_number", sheet.number.clone());
    values.insert("sheet_title", sheet.title.clone());
    values.insert("project", project.to_string());
    values.insert("date", date.to_string());
    values.insert("scale", scale.clone());

    let mut out = JsonMap::new();
    out.insert("SHEETNO".to_string(), Value::String(expand_macros("%sheet_number%", &values)));
    out.insert("SHEETTITLE".to_string(), Value::String(expand_macros("%sheet_title%", &values)));
    out.insert("PROJECT".to_string(), Value::String(expand_macros("%project%", &values)));
    out.insert("DATE".to_string(), Value::String(expand_macros("%date%", &values)));
    out.insert("SCALE".to_string(), Value::String(expand_macros("%scale%", &values)));
    out
}

fn ensure_master_title_block(s: &mut Session) -> Result<()> {
    let d = s.doc_mut()?;
    if d.block(TITLE_BLOCK).is_some() {
        return Ok(());
    }
    d.ensure_layer(LAYER_TITLE);
    let mut block = Block::new(TITLE_BLOCK);
    block.base = Vec3::ZERO;

    let common = Common { layer: LAYER_TITLE.to_string(), ..Default::default() };
    for (a, b) in [
        (Vec3::new(0.0, 0.0, 0.0), Vec3::new(-100.0, 0.0, 0.0)),
        (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 560.0, 0.0)),
        (Vec3::new(-100.0, 0.0, 0.0), Vec3::new(-100.0, 560.0, 0.0)),
        (Vec3::new(0.0, 560.0, 0.0), Vec3::new(-100.0, 560.0, 0.0)),
        (Vec3::new(0.0, 80.0, 0.0), Vec3::new(-100.0, 80.0, 0.0)),
        (Vec3::new(0.0, 220.0, 0.0), Vec3::new(-100.0, 220.0, 0.0)),
        (Vec3::new(0.0, 340.0, 0.0), Vec3::new(-100.0, 340.0, 0.0)),
        (Vec3::new(0.0, 460.0, 0.0), Vec3::new(-100.0, 460.0, 0.0)),
    ] {
        block.entities.push(Entity { handle: d.new_handle(), common: common.clone(), kind: EntityKind::Line(cadcraft_doc::Line { a, b }) });
    }

    for (tag, default, height, x, y) in [
        ("PROJECT", "%project%", 5.0, -8.0, 32.0),
        ("SHEETTITLE", "%sheet_title%", 5.0, -8.0, 112.0),
        ("SCALE", "%scale%", 3.5, -8.0, 210.0),
        ("DATE", "%date%", 3.5, -8.0, 302.0),
        ("SHEETNO", "%sheet_number%", 10.0, -8.0, 430.0),
    ] {
        block.entities.push(Entity {
            handle: d.new_handle(),
            common: common.clone(),
            kind: EntityKind::AttDef(Attrib {
                tag: tag.to_string(),
                text: Text {
                    insert: Vec3::new(x, y, 0.0),
                    align_pt: None,
                    height,
                    value: default.to_string(),
                    rotation: 0.0,
                    width_factor: 1.0,
                    oblique: 0.0,
                    style: "Standard".to_string(),
                    halign: HAlign::Left,
                    valign: VAlign::Baseline,
                },
                invisible: false,
                constant: false,
                prompt: tag.to_string(),
                props: Default::default(),
                multiline: None,
                modes: Default::default(),
            }),
        });
    }

    d.blocks.insert(TITLE_BLOCK.to_string(), std::sync::Arc::new(block));
    Ok(())
}

fn update_insert_values(e: &mut cadcraft_doc::Entity, values: &JsonMap<String, Value>) {
    if let EntityKind::Insert(ins) = &mut e.kind {
        for attr in &mut ins.attribs {
            if let Some(v) = values.iter().find(|(k, _)| k.eq_ignore_ascii_case(&attr.tag)).map(|(_, v)| match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => attr.text.value.clone(),
            }) {
                attr.text.value = v;
            }
        }
    }
}

fn apply_title_values(s: &mut Session, sheet: &cadcraft_doc::arch::sheet::Sheet) -> Result<()> {
    let project = s.doc()?.arch.sheet_set.project.clone();
    let date = s.doc()?.arch.sheet_set.date.clone();
    let values = title_attr_values(sheet, &project, &date);
    let layout_name = sheet.name.clone();
    let Some(layout) = s.doc()?.layout(&layout_name).cloned() else {
        return Ok(());
    };

    let mut found = None;
    for e in layout.entities.iter() {
        if let Some((role, key, _)) = read_link(&e.common.xdata) {
            if role == LINK_TITLE && key == sheet.name && matches!(e.kind, EntityKind::Insert(_)) {
                found = Some(e.handle);
                break;
            }
        }
    }

    if let Some(handle) = found {
        s.doc_mut()?.modify_entity(handle, |e| {
            update_insert_values(e, &values);
            set_link(&mut e.common.xdata, LINK_TITLE, &sheet.name, 0);
        })?;
        return Ok(());
    }

    let current_space = s.state()?.space.clone();
    {
        let st = s.state_mut()?;
        st.space = cadcraft_doc::Space::Paper(layout_name.clone());
    }
    let result = blocks::insert_with(s, TITLE_BLOCK, Vec2::new(layout.page.width_mm - 10.0, 10.0), Vec2::new(1.0, 1.0), 0.0, None, &values);
    {
        let st = s.state_mut()?;
        st.space = current_space;
    }
    let handle = result?;
    s.doc_mut()?.modify_entity(handle, |e| {
        set_link(&mut e.common.xdata, LINK_TITLE, &sheet.name, 0);
    })?;
    Ok(())
}

/// Bring every sheet's title block up to date with the sheets and the sheet set.
pub(crate) fn refresh_title_blocks(s: &mut Session) -> Result<()> {
    ensure_master_title_block(s)?;
    let sheets = {
        let d = s.doc()?;
        d.arch.sheets.clone()
    };
    for sheet in sheets {
        apply_title_values(s, &sheet)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::sheet::{SheetSet, SheetSource, SheetView};
    use cadcraft_doc::{EntityKind, Layout};

    fn make_sheet(name: &str, number: &str, title: &str) -> cadcraft_doc::arch::sheet::Sheet {
        cadcraft_doc::arch::sheet::Sheet {
            name: name.to_string(),
            number: number.to_string(),
            title: title.to_string(),
            size: "ARCH D".to_string(),
            landscape: true,
            views: vec![SheetView {
                number: 1,
                source: SheetSource::Plan { view: "Floor Plan".to_string(), level: None },
                scale: 50.0,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn master_block_is_made_once_and_kept() {
        let mut s = Session::new();
        assert!(s.doc().unwrap().block(TITLE_BLOCK).is_none());
        refresh_title_blocks(&mut s).unwrap();
        assert!(s.doc().unwrap().block(TITLE_BLOCK).is_some());
        let before = s.doc().unwrap().block(TITLE_BLOCK).unwrap().entities.len();
        refresh_title_blocks(&mut s).unwrap();
        assert_eq!(s.doc().unwrap().block(TITLE_BLOCK).unwrap().entities.len(), before);
    }

    #[test]
    fn every_sheet_gets_one_linked_insert() {
        let mut s = Session::new();
        let mut d = s.doc_mut().unwrap();
        d.arch.sheet_set = SheetSet { project: "House".to_string(), date: "2026-11-09".to_string(), ..SheetSet::default() };
        d.arch.sheets.push(make_sheet("Plans", "A-101", "Floor Plan"));
        d.layouts.iter_mut().find(|l| l.name == "Layout1").unwrap().name = "Plans".to_string();
        drop(d);
        refresh_title_blocks(&mut s).unwrap();
        let layout = s.doc().unwrap().layout("Plans").unwrap();
        let count =
            layout.entities.iter().filter(|e| read_link(&e.common.xdata).is_some_and(|(role, key, _)| role == LINK_TITLE && key == "Plans")).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn macros_expand_number_title_project_date_scale() {
        let mut s = Session::new();
        let mut d = s.doc_mut().unwrap();
        d.arch.sheet_set = SheetSet { project: "Phase 21 House".to_string(), date: "2026-11-09".to_string(), ..SheetSet::default() };
        d.arch.sheets.push(make_sheet("Plans", "A-101", "Floor Plan"));
        d.layouts.iter_mut().find(|l| l.name == "Layout1").unwrap().name = "Plans".to_string();
        drop(d);
        refresh_title_blocks(&mut s).unwrap();
        let layout = s.doc().unwrap().layout("Plans").unwrap();
        let ins =
            layout.entities.iter().find(|e| read_link(&e.common.xdata).is_some_and(|(role, key, _)| role == LINK_TITLE && key == "Plans")).unwrap();
        let EntityKind::Insert(i) = &ins.kind else { panic!("not insert") };
        let attrs = i.attribs.iter().map(|a| (a.tag.clone(), a.text.value.clone())).collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(attrs.get("SHEETNO").map(String::as_str), Some("A-101"));
        assert_eq!(attrs.get("SHEETTITLE").map(String::as_str), Some("Floor Plan"));
        assert_eq!(attrs.get("PROJECT").map(String::as_str), Some("Phase 21 House"));
        assert_eq!(attrs.get("DATE").map(String::as_str), Some("2026-11-09"));
        assert_eq!(attrs.get("SCALE").map(String::as_str), Some("1:50"));
    }

    #[test]
    fn mixed_scales_read_as_noted() {
        let mut s = Session::new();
        let mut d = s.doc_mut().unwrap();
        d.arch.sheet_set = SheetSet { project: "House".to_string(), date: "2026-11-09".to_string(), ..SheetSet::default() };
        d.arch.sheets.push(cadcraft_doc::arch::sheet::Sheet {
            name: "Plans".to_string(),
            number: "A-101".to_string(),
            title: "Floor Plan".to_string(),
            views: vec![
                SheetView { number: 1, source: SheetSource::Plan { view: "Floor Plan".to_string(), level: None }, scale: 50.0, ..Default::default() },
                SheetView { number: 2, source: SheetSource::Plan { view: "Roof".to_string(), level: None }, scale: 100.0, ..Default::default() },
            ],
            ..Default::default()
        });
        d.layouts.iter_mut().find(|l| l.name == "Layout1").unwrap().name = "Plans".to_string();
        drop(d);
        refresh_title_blocks(&mut s).unwrap();
        let layout = s.doc().unwrap().layout("Plans").unwrap();
        let ins =
            layout.entities.iter().find(|e| read_link(&e.common.xdata).is_some_and(|(role, key, _)| role == LINK_TITLE && key == "Plans")).unwrap();
        let EntityKind::Insert(i) = &ins.kind else { panic!("not insert") };
        assert_eq!(i.attribs.iter().find(|a| a.tag.eq_ignore_ascii_case("SCALE")).map(|a| a.text.value.as_str()), Some("As noted"));
    }

    #[test]
    fn unknown_macros_stay() {
        let mut s = Session::new();
        let mut d = s.doc_mut().unwrap();
        d.arch.sheet_set = SheetSet { project: "House".to_string(), date: "2026-11-09".to_string(), ..SheetSet::default() };
        d.arch.sheets.push(cadcraft_doc::arch::sheet::Sheet {
            name: "Plans".to_string(),
            number: "A-101".to_string(),
            title: "%other% Floor Plan".to_string(),
            ..Default::default()
        });
        d.layouts.iter_mut().find(|l| l.name == "Layout1").unwrap().name = "Plans".to_string();
        drop(d);
        refresh_title_blocks(&mut s).unwrap();
        let layout = s.doc().unwrap().layout("Plans").unwrap();
        let ins =
            layout.entities.iter().find(|e| read_link(&e.common.xdata).is_some_and(|(role, key, _)| role == LINK_TITLE && key == "Plans")).unwrap();
        let EntityKind::Insert(i) = &ins.kind else { panic!("not insert") };
        assert_eq!(i.attribs.iter().find(|a| a.tag.eq_ignore_ascii_case("SHEETTITLE")).map(|a| a.text.value.as_str()), Some("%other% Floor Plan"));
    }

    #[test]
    fn sheetset_edit_updates_every_sheet() {
        let mut s = Session::new();
        let mut d = s.doc_mut().unwrap();
        d.arch.sheet_set = SheetSet { project: "Old".to_string(), date: "Old date".to_string(), ..SheetSet::default() };
        d.arch.sheets.push(make_sheet("Plans", "A-101", "Floor Plan"));
        d.arch.sheets.push(make_sheet("Ceiling", "A-102", "Ceiling"));
        d.layouts.iter_mut().find(|l| l.name == "Layout1").unwrap().name = "Plans".to_string();
        d.layouts.push(Layout::new("Ceiling", 2));
        drop(d);
        refresh_title_blocks(&mut s).unwrap();
        let mut d = s.doc_mut().unwrap();
        d.arch.sheet_set = SheetSet { project: "Renamed House".to_string(), date: "2026-11-09".to_string(), ..SheetSet::default() };
        drop(d);
        refresh_title_blocks(&mut s).unwrap();
        for sh in &s.doc().unwrap().arch.sheets {
            let layout = s.doc().unwrap().layout(&sh.name).unwrap();
            let ins = layout
                .entities
                .iter()
                .find(|e| read_link(&e.common.xdata).is_some_and(|(role, key, _)| role == LINK_TITLE && key == sh.name))
                .unwrap();
            let EntityKind::Insert(i) = &ins.kind else { panic!("not insert") };
            assert_eq!(i.attribs.iter().find(|a| a.tag.eq_ignore_ascii_case("PROJECT")).map(|a| a.text.value.as_str()), Some("Renamed House"));
            assert_eq!(i.attribs.iter().find(|a| a.tag.eq_ignore_ascii_case("DATE")).map(|a| a.text.value.as_str()), Some("2026-11-09"));
        }
    }

    #[test]
    fn a_user_edited_master_is_used() {
        let mut s = Session::new();
        let mut d = s.doc_mut().unwrap();
        d.arch.sheets.push(make_sheet("Plans", "A-101", "Floor Plan"));
        d.layouts.iter_mut().find(|l| l.name == "Layout1").unwrap().name = "Plans".to_string();
        let mut block = Block::new(TITLE_BLOCK);
        block.base = Vec3::ZERO;
        block.entities.push(Entity {
            handle: d.new_handle(),
            common: Common { layer: LAYER_TITLE.to_string(), ..Default::default() },
            kind: EntityKind::AttDef(Attrib {
                tag: "PROJECT".to_string(),
                text: Text {
                    insert: Vec3::ZERO,
                    align_pt: None,
                    height: 5.0,
                    value: "%project%".to_string(),
                    rotation: 0.0,
                    width_factor: 1.0,
                    oblique: 0.0,
                    style: "Standard".to_string(),
                    halign: HAlign::Left,
                    valign: VAlign::Baseline,
                },
                invisible: false,
                constant: false,
                prompt: "PROJECT".to_string(),
                props: Default::default(),
                multiline: None,
                modes: Default::default(),
            }),
        });
        d.blocks.insert(TITLE_BLOCK.to_string(), std::sync::Arc::new(block));
        drop(d);
        refresh_title_blocks(&mut s).unwrap();
        let layout = s.doc().unwrap().layout("Plans").unwrap();
        let ins =
            layout.entities.iter().find(|e| read_link(&e.common.xdata).is_some_and(|(role, key, _)| role == LINK_TITLE && key == "Plans")).unwrap();
        let EntityKind::Insert(i) = &ins.kind else { panic!("not insert") };
        assert_eq!(i.attribs.iter().find(|a| a.tag.eq_ignore_ascii_case("PROJECT")).map(|a| a.text.value.as_str()), Some(""));
    }

    #[test]
    fn inserts_survive_a_dxf_round_trip() {
        let mut s = Session::new();
        let mut d = s.doc_mut().unwrap();
        d.arch.sheet_set = SheetSet { project: "House".to_string(), date: "2026-11-09".to_string(), ..SheetSet::default() };
        d.arch.sheets.push(make_sheet("Plans", "A-101", "Floor Plan"));
        d.layouts.iter_mut().find(|l| l.name == "Layout1").unwrap().name = "Plans".to_string();
        drop(d);
        refresh_title_blocks(&mut s).unwrap();
        let layout = s.doc().unwrap().layout("Plans").unwrap();
        let handle = layout
            .entities
            .iter()
            .find(|e| read_link(&e.common.xdata).is_some_and(|(role, key, _)| role == LINK_TITLE && key == "Plans"))
            .unwrap()
            .handle;
        let entity = s.doc().unwrap().entity(handle).unwrap().clone();
        assert!(matches!(entity.kind, EntityKind::Insert(_)));
        assert!(read_link(&entity.common.xdata).is_some());
    }

    #[test]
    fn no_sheets_is_harmless() {
        let mut s = Session::new();
        // No sheets added at all
        refresh_title_blocks(&mut s).unwrap();
        // Master block should still be created
        assert!(s.doc().unwrap().block(TITLE_BLOCK).is_some());
    }

    #[test]
    fn empty_fields_expand_to_empty() {
        let mut s = Session::new();
        let mut d = s.doc_mut().unwrap();
        d.arch.sheet_set = SheetSet { project: "".to_string(), date: "".to_string(), ..SheetSet::default() };
        let sheet = cadcraft_doc::arch::sheet::Sheet {
            name: "Plans".to_string(),
            number: "".to_string(),
            title: "".to_string(),
            views: vec![SheetView {
                number: 1,
                source: SheetSource::Plan { view: "Floor Plan".to_string(), level: None },
                scale: 50.0,
                ..Default::default()
            }],
            ..Default::default()
        };
        d.arch.sheets.push(sheet);
        d.layouts.iter_mut().find(|l| l.name == "Layout1").unwrap().name = "Plans".to_string();
        drop(d);
        refresh_title_blocks(&mut s).unwrap();
        let layout = s.doc().unwrap().layout("Plans").unwrap();
        let ins =
            layout.entities.iter().find(|e| read_link(&e.common.xdata).is_some_and(|(role, key, _)| role == LINK_TITLE && key == "Plans")).unwrap();
        let EntityKind::Insert(i) = &ins.kind else { panic!("not insert") };
        assert_eq!(i.attribs.iter().find(|a| a.tag.eq_ignore_ascii_case("SHEETNO")).map(|a| a.text.value.as_str()), Some(""));
        assert_eq!(i.attribs.iter().find(|a| a.tag.eq_ignore_ascii_case("SHEETTITLE")).map(|a| a.text.value.as_str()), Some(""));
        assert_eq!(i.attribs.iter().find(|a| a.tag.eq_ignore_ascii_case("PROJECT")).map(|a| a.text.value.as_str()), Some(""));
        assert_eq!(i.attribs.iter().find(|a| a.tag.eq_ignore_ascii_case("DATE")).map(|a| a.text.value.as_str()), Some(""));
        assert_eq!(i.attribs.iter().find(|a| a.tag.eq_ignore_ascii_case("SCALE")).map(|a| a.text.value.as_str()), Some("1:50"));
    }

    #[test]
    fn fractional_scale_formats() {
        let mut s = Session::new();
        let mut d = s.doc_mut().unwrap();
        d.arch.sheet_set = SheetSet { project: "House".to_string(), date: "2026-11-09".to_string(), ..SheetSet::default() };
        let sheet = cadcraft_doc::arch::sheet::Sheet {
            name: "Plans".to_string(),
            number: "A-101".to_string(),
            title: "Floor Plan".to_string(),
            views: vec![SheetView {
                number: 1,
                source: SheetSource::Plan { view: "Floor Plan".to_string(), level: None },
                scale: 25.4,
                ..Default::default()
            }],
            ..Default::default()
        };
        d.arch.sheets.push(sheet);
        d.layouts.iter_mut().find(|l| l.name == "Layout1").unwrap().name = "Plans".to_string();
        drop(d);
        refresh_title_blocks(&mut s).unwrap();
        let layout = s.doc().unwrap().layout("Plans").unwrap();
        let ins =
            layout.entities.iter().find(|e| read_link(&e.common.xdata).is_some_and(|(role, key, _)| role == LINK_TITLE && key == "Plans")).unwrap();
        let EntityKind::Insert(i) = &ins.kind else { panic!("not insert") };
        assert_eq!(i.attribs.iter().find(|a| a.tag.eq_ignore_ascii_case("SCALE")).map(|a| a.text.value.as_str()), Some("1:25.4"));
    }

    #[test]
    fn repeated_refresh_does_not_duplicate() {
        let mut s = Session::new();
        let mut d = s.doc_mut().unwrap();
        d.arch.sheet_set = SheetSet { project: "House".to_string(), date: "2026-11-09".to_string(), ..SheetSet::default() };
        d.arch.sheets.push(make_sheet("Plans", "A-101", "Floor Plan"));
        d.layouts.iter_mut().find(|l| l.name == "Layout1").unwrap().name = "Plans".to_string();
        drop(d);
        refresh_title_blocks(&mut s).unwrap();
        refresh_title_blocks(&mut s).unwrap();
        refresh_title_blocks(&mut s).unwrap();
        let layout = s.doc().unwrap().layout("Plans").unwrap();
        let count =
            layout.entities.iter().filter(|e| read_link(&e.common.xdata).is_some_and(|(role, key, _)| role == LINK_TITLE && key == "Plans")).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn layout_missing_is_harmless() {
        let mut s = Session::new();
        let mut d = s.doc_mut().unwrap();
        d.arch.sheet_set = SheetSet { project: "House".to_string(), date: "2026-11-09".to_string(), ..SheetSet::default() };
        // Sheet named "Plans" but no layout with that name exists
        d.arch.sheets.push(make_sheet("Plans", "A-101", "Floor Plan"));
        drop(d);
        // Should not error even though layout "Plans" doesn't exist
        refresh_title_blocks(&mut s).unwrap();
    }
}
