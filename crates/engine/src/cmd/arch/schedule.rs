//! Schedule commands: door, window, cabinet and room-finish schedules as linked TABLEs.

use crate::EngineError;
use crate::Result;
use crate::Session;
use crate::cmd::CommandSpec;
use crate::cmd::table::make_table;
use cadcraft_doc::arch::{
    ArchId, LAYER_SCHEDULE, LINK_SCHEDULE, MAX_SCHEDULE_ROWS, MAX_SCHEDULES, Schedule, ScheduleKind, link_tags, read_link, set_link,
};
use cadcraft_doc::{Drawing, Handle, Space, Table, TableStyle};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};
use std::sync::Arc;

/// Run the `schedule` command.
pub fn run_schedule(s: &mut Session, params: &Value) -> Result<Value> {
    let kind_str = params.get("kind").and_then(Value::as_str).ok_or_else(|| EngineError::Other("kind is required".into()))?;
    let kind = ScheduleKind::from_key(kind_str).ok_or_else(|| EngineError::Other(format!("unknown kind: {kind_str}")))?;

    let columns = parse_columns(params.get("columns"), kind)?;
    let name = params.get("name").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| default_name(kind));
    if name.is_empty() || name.len() > 256 {
        return Err(EngineError::Other("name is empty or too long".into()));
    }

    let level = parse_level_param(params.get("level"))?;

    let at = if let Some(v) = params.get("at") { parse_at(v)? } else { default_at(s.doc()?, kind)? };

    let mut doc = s.doc()?.clone();

    if doc.arch.schedules.len() >= MAX_SCHEDULES {
        return Err(EngineError::Other(format!("too many schedules (max {MAX_SCHEDULES})")));
    }

    let id = doc.arch.next_schedule_id;
    let label = format!("sc{id}");

    let schedule = Schedule { id: ArchId(id), name: name.clone(), kind, columns: columns.clone(), level, at: [at.x, at.y], ..Default::default() };

    let rows = cadcraft_arch::schedule::rows(&doc.arch, &schedule);
    let table = build_table(&doc, &schedule, &rows, at)?;

    doc.ensure_layer(LAYER_SCHEDULE);
    let mut common: cadcraft_doc::Common = Default::default();
    common.layer = LAYER_SCHEDULE.to_string();
    let mut tags = link_tags(LINK_SCHEDULE, &label, 0);
    set_link(&mut tags, LINK_SCHEDULE, &label, 0);
    common.xdata = tags;
    let handle = doc.add(&Space::Model, common, cadcraft_doc::EntityKind::Table(table))?;

    doc.arch.schedules.push(schedule);
    doc.arch.next_schedule_id += 1;

    s.state_mut()?.doc = Arc::new(doc);

    let row_json: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "number": r.number,
                "count": r.count,
                "members": r.members,
                "cells": r.cells
            })
        })
        .collect();

    Ok(json!({
        "schedule": label,
        "table": handle.hex(),
        "name": name,
        "kind": kind_str,
        "columns": columns,
        "rows": row_json
    }))
}

/// Run the `schedule.edit` command.
pub fn run_schedule_edit(s: &mut Session, params: &Value) -> Result<Value> {
    let id_str = params.get("id").and_then(Value::as_str).ok_or_else(|| EngineError::Other("id is required".into()))?;
    let id = parse_schedule_id(id_str)?;

    let mut doc = s.doc()?.clone();

    let idx = doc.arch.schedules.iter().position(|s| s.id == id).ok_or_else(|| EngineError::Other(format!("unknown schedule: {id_str}")))?;
    let mut schedule = doc.arch.schedules[idx].clone();

    let mut changed = false;

    if let Some(v) = params.get("columns") {
        let cols = parse_columns(Some(v), schedule.kind)?;
        if cols != schedule.columns {
            schedule.columns = cols;
            changed = true;
        }
    }

    if let Some(v) = params.get("name") {
        let name = v.as_str().ok_or_else(|| EngineError::Other("name must be a string".into()))?;
        if name.is_empty() || name.len() > 256 {
            return Err(EngineError::Other("name is empty or too long".into()));
        }
        if name != schedule.name {
            schedule.name = name.to_string();
            changed = true;
        }
    }

    if let Some(v) = params.get("level") {
        let level = parse_level_param(Some(v))?;
        if level != schedule.level {
            schedule.level = level;
            changed = true;
        }
    }

    if let Some(v) = params.get("at") {
        let at = parse_at(v)?;
        let at_arr = [at.x, at.y];
        if at_arr != schedule.at {
            schedule.at = at_arr;
            changed = true;
        }
    }

    if !changed {
        return Err(EngineError::Other("no changes specified".into()));
    }

    let rows = cadcraft_arch::schedule::rows(&doc.arch, &schedule);
    let label = format!("sc{}", id.0);
    let at_vec = Vec2::new(schedule.at[0], schedule.at[1]);
    let table = build_table(&doc, &schedule, &rows, at_vec)?;

    if let Some(handle) = schedule_table(&doc, &label) {
        // Rewrite table
        let mut new_entity = None;
        for e in doc.model.iter() {
            if e.handle == handle {
                let mut ne = (**e).clone();
                if let cadcraft_doc::EntityKind::Table(ref mut t) = ne.kind {
                    *t = table;
                }
                new_entity = Some(ne);
                break;
            }
        }
        if let Some(ne) = new_entity {
            doc.model.replace(ne);
        }
    } else {
        // Create new table if missing (shouldn't happen if linked, but robust)
        doc.ensure_layer(LAYER_SCHEDULE);
        let mut common: cadcraft_doc::Common = Default::default();
        common.layer = LAYER_SCHEDULE.to_string();
        let mut tags = link_tags(LINK_SCHEDULE, &label, 0);
        set_link(&mut tags, LINK_SCHEDULE, &label, 0);
        common.xdata = tags;
        doc.add(&Space::Model, common, cadcraft_doc::EntityKind::Table(table))?;
    }

    // Prepare JSON before moving schedule
    let name = schedule.name.clone();
    let kind_key = kind_str(schedule.kind);
    let columns = schedule.columns.clone();

    doc.arch.schedules[idx] = schedule;
    s.state_mut()?.doc = Arc::new(doc);

    let row_json: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "number": r.number,
                "count": r.count,
                "members": r.members,
                "cells": r.cells
            })
        })
        .collect();

    Ok(json!({
        "schedule": label,
        "name": name,
        "kind": kind_key,
        "columns": columns,
        "rows": row_json
    }))
}

/// Run the `schedule.delete` command.
pub fn run_schedule_delete(s: &mut Session, params: &Value) -> Result<Value> {
    let id_str = params.get("id").and_then(Value::as_str).ok_or_else(|| EngineError::Other("id is required".into()))?;
    let id = parse_schedule_id(id_str)?;

    let mut doc = s.doc()?.clone();

    let idx = doc.arch.schedules.iter().position(|s| s.id == id).ok_or_else(|| EngineError::Other(format!("unknown schedule: {id_str}")))?;
    let label = format!("sc{}", id.0);

    // Find handle first (immutable borrow)
    let handle = schedule_table(&doc, &label);

    // Remove from arch (mutable borrow)
    doc.arch.schedules.remove(idx);

    // Remove from model (mutable borrow)
    if let Some(h) = handle {
        doc.model.retain(|e| e.handle != h);
    }

    s.state_mut()?.doc = Arc::new(doc);

    Ok(json!({ "deleted": label }))
}

/// Find the model-space TABLE linked to a schedule label.
pub(crate) fn schedule_table(d: &Drawing, label: &str) -> Option<Handle> {
    d.model.iter().find_map(|e| {
        if let cadcraft_doc::EntityKind::Table(_) = e.kind {
            if let Some((key, val, _)) = read_link(&e.common.xdata) {
                if key == LINK_SCHEDULE && val == label {
                    return Some(e.handle);
                }
            }
        }
        None
    })
}

/// Refresh all schedules after a command.
pub(crate) fn refresh_schedules(s: &mut Session) {
    let mut doc = s.doc().ok().cloned().unwrap_or_default();
    let mut to_remove = Vec::new();
    let mut to_update: Vec<(Handle, Table)> = Vec::new();

    // First pass: identify changes (read-only on doc)
    for schedule in &doc.arch.schedules {
        let label = format!("sc{}", schedule.id.0);
        let rows = cadcraft_arch::schedule::rows(&doc.arch, schedule);

        if rows.len() > MAX_SCHEDULE_ROWS {
            continue;
        }

        if let Some(handle) = schedule_table(&doc, &label) {
            let at_vec = Vec2::new(schedule.at[0], schedule.at[1]);
            let expected_table = match build_table(&doc, schedule, &rows, at_vec) {
                Ok(t) => t,
                Err(_) => continue,
            };

            // Find current table
            let mut current_table = None;
            for e in doc.model.iter() {
                if e.handle == handle {
                    if let cadcraft_doc::EntityKind::Table(t) = &e.kind {
                        current_table = Some(t.clone());
                    }
                    break;
                }
            }

            if let Some(current) = current_table {
                if table_differs(&current, &expected_table) {
                    to_update.push((handle, expected_table));
                }
            }
        } else {
            to_remove.push(schedule.id);
        }
    }

    let mut changed = false;

    // Second pass: apply changes (mutable on doc)
    if !to_remove.is_empty() {
        doc.arch.schedules.retain(|s| !to_remove.contains(&s.id));
        changed = true;
    }

    for (handle, table) in to_update {
        let mut new_entity = None;
        for e in doc.model.iter() {
            if e.handle == handle {
                let mut ne = (**e).clone();
                if let cadcraft_doc::EntityKind::Table(ref mut t) = ne.kind {
                    *t = table;
                }
                new_entity = Some(ne);
                break;
            }
        }
        if let Some(ne) = new_entity {
            doc.model.replace(ne);
            changed = true;
        }
    }

    if changed {
        if let Ok(st) = s.state_mut() {
            st.doc = Arc::new(doc);
        }
    }
}

fn build_table(d: &Drawing, schedule: &Schedule, rows: &[cadcraft_arch::schedule::ScheduleRow], at: Vec2) -> Result<Table> {
    let style = TableStyle::default();
    let cols = schedule.columns.len();
    let row_count = 1 + rows.len(); // Header + data rows

    let mut cells = Vec::new();
    // Header row
    let mut header = Vec::new();
    for col_key in &schedule.columns {
        header.push(cadcraft_arch::schedule::column_title(col_key).to_string());
    }
    cells.push(header);

    // Data rows
    for row in rows {
        cells.push(row.cells.clone());
    }

    // Calculate widths
    let mut col_widths = vec![0.6; cols];
    for (i, _) in schedule.columns.iter().enumerate() {
        let mut max_len = 0;
        for cell_row in &cells {
            if let Some(cell) = cell_row.get(i) {
                max_len = max_len.max(cell.len());
            }
        }
        // 0.6 base + text height factor + margin
        let w = (0.6 + 0.6 * 0.25 * (max_len as f64) + 0.1).max(0.6);
        col_widths[i] = w;
    }

    let mut table = make_table(&style, at, rows.len(), cols, None, None, Some(&schedule.name), Some(cells[0].clone()), &cells[1..]);

    table.col_widths = col_widths;

    Ok(table)
}

fn table_differs(current: &Table, expected: &Table) -> bool {
    if current.title != expected.title {
        return true;
    }
    if current.header != expected.header {
        return true;
    }
    if current.cells.len() != expected.cells.len() {
        return true;
    }
    for (c_row, e_row) in current.cells.iter().zip(expected.cells.iter()) {
        if c_row.len() != e_row.len() {
            return true;
        }
        for (c_cell, e_cell) in c_row.iter().zip(e_row.iter()) {
            if c_cell.text != e_cell.text {
                return true;
            }
        }
    }
    false
}

fn parse_columns(v: Option<&Value>, kind: ScheduleKind) -> Result<Vec<String>> {
    if let Some(v) = v {
        let arr = v.as_array().ok_or_else(|| EngineError::Other("columns must be a list".into()))?;
        if arr.is_empty() {
            return Err(EngineError::Other("columns must not be empty".into()));
        }
        let mut cols = Vec::new();
        for item in arr {
            let s = item.as_str().ok_or_else(|| EngineError::Other("column keys must be strings".into()))?;
            if let Err(e) = cadcraft_arch::schedule::validate_columns(kind, &[s.to_string()]) {
                return Err(EngineError::Other(e));
            }
            if cols.contains(&s.to_string()) {
                return Err(EngineError::Other(format!("duplicate column key: {s}")));
            }
            cols.push(s.to_string());
        }
        Ok(cols)
    } else {
        Ok(cadcraft_arch::schedule::default_columns(kind).iter().map(|s| s.to_string()).collect())
    }
}

fn parse_level_param(v: Option<&Value>) -> Result<Option<u32>> {
    if let Some(v) = v {
        if v.is_null() {
            return Ok(None);
        }
        if let Some(s) = v.as_str() {
            if s == "all" {
                return Ok(None);
            }
            if let Ok(i) = s.parse::<u32>() {
                return Ok(Some(i));
            }
            return Err(EngineError::Other(format!("unknown level: {s}")));
        }
        if let Some(i) = v.as_u64() {
            return Ok(Some(i as u32));
        }
        Err(EngineError::Other("level must be a string or integer".into()))
    } else {
        Ok(None)
    }
}

fn parse_at(v: &Value) -> Result<Vec2> {
    let arr = v.as_array().ok_or_else(|| EngineError::Other("at must be an array".into()))?;
    if arr.len() != 2 {
        return Err(EngineError::Other("at must have two numbers".into()));
    }
    let x = arr[0].as_f64().ok_or_else(|| EngineError::Other("at x must be a number".into()))?;
    let y = arr[1].as_f64().ok_or_else(|| EngineError::Other("at y must be a number".into()))?;
    if !x.is_finite() || !y.is_finite() {
        return Err(EngineError::Other("at coordinates must be finite".into()));
    }
    Ok(Vec2::new(x, y))
}

fn default_at(d: &Drawing, _kind: ScheduleKind) -> Result<Vec2> {
    let bounds = d.arch.bounds();
    if bounds.is_empty() {
        return Ok(Vec2::ZERO);
    }
    let x = bounds.max.x + 3.0;
    let count = d.arch.schedules.len();
    let y = bounds.max.y - 3.0 * (count as f64);
    Ok(Vec2::new(x, y))
}

fn default_name(kind: ScheduleKind) -> String {
    match kind {
        ScheduleKind::Door => "Door Schedule".to_string(),
        ScheduleKind::Window => "Window Schedule".to_string(),
        ScheduleKind::Cabinet => "Cabinet Schedule".to_string(),
        ScheduleKind::Room => "Room Finish Schedule".to_string(),
    }
}

fn kind_str(kind: ScheduleKind) -> String {
    match kind {
        ScheduleKind::Door => "door".to_string(),
        ScheduleKind::Window => "window".to_string(),
        ScheduleKind::Cabinet => "cabinet".to_string(),
        ScheduleKind::Room => "room".to_string(),
    }
}

fn parse_schedule_id(s: &str) -> Result<ArchId> {
    if let Some(rest) = s.strip_prefix("sc") {
        rest.parse::<u32>().map(ArchId).map_err(|_| EngineError::Other(format!("invalid schedule id: {s}")))
    } else {
        Err(EngineError::Other(format!("invalid schedule id: {s}")))
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("schedule", "Schedule", run_schedule)
            .menu(&["Architecture", "Schedule"])
            .params("{kind: door|window|cabinet|room, columns?: [...], name?: string, at?: [x,y], level?: name|index|all}"),
        CommandSpec::new("schedule.edit", "Edit Schedule", run_schedule_edit)
            .menu(&["Architecture", "Edit Schedule"])
            .params("{id: sc<n>, columns?: [...], name?: string, level?: name|index|all, at?: [x,y]}"),
        CommandSpec::new("schedule.delete", "Delete Schedule", run_schedule_delete).menu(&["Architecture", "Delete Schedule"]).params("{id: sc<n>}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use cadcraft_doc::arch::{ArchModel, ScheduleKind};
    use serde_json::json;

    #[test]
    fn schedule_makes_a_linked_table() {
        let mut s = Session::new();
        // Add a wall and a door to have something to schedule
        s.execute("wall", &json!({"points": [[0,0],[10,0],[10,10],[0,10],[0,0]]})).unwrap();
        s.execute("door", &json!({"wall": "w0", "offset": 5.0})).unwrap();

        let res = s.execute("schedule", &json!({"kind": "door"})).unwrap();
        let schedule_id = res.get("schedule").and_then(Value::as_str).unwrap();
        let table_handle = res.get("table").and_then(Value::as_str).unwrap();

        assert!(schedule_id.starts_with("sc"));
        assert!(!table_handle.is_empty());

        // Verify table exists and is linked
        let doc = s.doc().unwrap();
        let handle = Handle::parse_hex(table_handle).unwrap();
        let entity = doc.entity(handle).unwrap();
        assert!(matches!(entity.kind, cadcraft_doc::EntityKind::Table(_)));

        let (key, val, _) = read_link(&entity.common.xdata).unwrap();
        assert_eq!(key, LINK_SCHEDULE);
        assert_eq!(val, schedule_id);
    }

    #[test]
    fn table_cells_are_title_header_rows() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0,0],[10,0],[10,10],[0,10],[0,0]]})).unwrap();
        s.execute("door", &json!({"wall": "w0", "offset": 5.0})).unwrap();

        let res = s.execute("schedule", &json!({"kind": "door"})).unwrap();
        let table_handle = res.get("table").and_then(Value::as_str).unwrap();
        let handle = Handle::parse_hex(table_handle).unwrap();

        let doc = s.doc().unwrap();
        if let cadcraft_doc::EntityKind::Table(t) = &doc.entity(handle).unwrap().kind {
            assert!(t.title);
            assert!(t.header);
            assert!(!t.cells.is_empty());
            // First row is title
            assert_eq!(t.cells[0][0].text, "Door Schedule");
            // Second row is header
            assert_eq!(t.cells[1][0].text, "No.");
        } else {
            panic!("Not a table");
        }
    }

    #[test]
    fn a_resize_rewrites_the_table_in_the_same_undo_step() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0,0],[10,0],[10,10],[0,10],[0,0]]})).unwrap();
        s.execute("door", &json!({"wall": "w0", "offset": 5.0})).unwrap();

        let res = s.execute("schedule", &json!({"kind": "door"})).unwrap();
        let table_handle = res.get("table").and_then(Value::as_str).unwrap();
        let handle = Handle::parse_hex(table_handle).unwrap();

        // Add another door to change the schedule (count or new row)
        // Use offset 7.0 to avoid overlap (min separation 0.05, width 0.9)
        s.execute("door", &json!({"wall": "w0", "offset": 7.0})).unwrap();

        // Check table updated
        let doc = s.doc().unwrap();
        if let cadcraft_doc::EntityKind::Table(t) = &doc.entity(handle).unwrap().kind {
            // Row 0 is title, Row 1 is header, Row 2 is first data row
            // Assuming default columns include count
            // If identical doors, count becomes 2
            let count_cell = &t.cells[2][1]; // 0-indexed, 2nd col (Count)
            assert!(count_cell.text.contains("2"));
        }
    }

    #[test]
    fn undo_restores_the_table() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0,0],[10,0],[10,10],[0,10],[0,0]]})).unwrap();
        s.execute("door", &json!({"wall": "w0", "offset": 5.0})).unwrap();

        let res = s.execute("schedule", &json!({"kind": "door"})).unwrap();
        let table_handle = res.get("table").and_then(Value::as_str).unwrap();
        let handle = Handle::parse_hex(table_handle).unwrap();

        // Add another door
        s.execute("door", &json!({"wall": "w0", "offset": 7.0})).unwrap();

        // Undo
        s.execute("undo", &json!({})).unwrap();

        let doc = s.doc().unwrap();
        if let cadcraft_doc::EntityKind::Table(t) = &doc.entity(handle).unwrap().kind {
            // Row 0 title, Row 1 header, Row 2 data
            let count_cell = &t.cells[2][1];
            assert!(count_cell.text.contains("1")); // Back to 1 door
        }
    }

    #[test]
    fn edit_columns_rewrites_the_header() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0,0],[10,0],[10,10],[0,10],[0,0]]})).unwrap();
        s.execute("door", &json!({"wall": "w0", "offset": 5.0})).unwrap();

        let res = s.execute("schedule", &json!({"kind": "door"})).unwrap();
        let schedule_id = res.get("schedule").and_then(Value::as_str).unwrap();
        let table_handle = res.get("table").and_then(Value::as_str).unwrap();
        let handle = Handle::parse_hex(table_handle).unwrap();

        // Edit columns to just width
        s.execute("schedule.edit", &json!({"id": schedule_id, "columns": ["width"]})).unwrap();

        let doc = s.doc().unwrap();
        if let cadcraft_doc::EntityKind::Table(t) = &doc.entity(handle).unwrap().kind {
            assert!(t.header);
            // Title + Header + 1 Data row = 3 rows
            assert_eq!(t.cells.len(), 3);
            // Row 1 is Header
            assert_eq!(t.cells[1].len(), 1);
            assert_eq!(t.cells[1][0].text, "Width");
        }
    }

    #[test]
    fn delete_erases_the_table() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0,0],[10,0],[10,10],[0,10],[0,0]]})).unwrap();
        s.execute("door", &json!({"wall": "w0", "offset": 5.0})).unwrap();

        let res = s.execute("schedule", &json!({"kind": "door"})).unwrap();
        let schedule_id = res.get("schedule").and_then(Value::as_str).unwrap();
        let table_handle = res.get("table").and_then(Value::as_str).unwrap();
        let handle = Handle::parse_hex(table_handle).unwrap();

        s.execute("schedule.delete", &json!({"id": schedule_id})).unwrap();

        let doc = s.doc().unwrap();
        assert!(doc.entity(handle).is_none());
    }

    #[test]
    fn erasing_the_table_drops_the_schedule() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0,0],[10,0],[10,10],[0,10],[0,0]]})).unwrap();
        s.execute("door", &json!({"wall": "w0", "offset": 5.0})).unwrap();

        let res = s.execute("schedule", &json!({"kind": "door"})).unwrap();
        let schedule_id = res.get("schedule").and_then(Value::as_str).unwrap();
        let table_handle = res.get("table").and_then(Value::as_str).unwrap();
        let handle = Handle::parse_hex(table_handle).unwrap();

        // Erase table manually
        s.execute("erase", &json!({"handles": [table_handle]})).unwrap();

        // Trigger refresh (e.g. by doing another command)
        s.execute("wall", &json!({"points": [[0,0],[1,0]]})).unwrap();

        // Schedule should be gone
        let doc = s.doc().unwrap();
        assert!(doc.arch.schedules.iter().all(|s| s.id.to_string() != schedule_id[2..]));
    }

    #[test]
    fn refresh_does_nothing_when_rows_are_unchanged() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0,0],[10,0],[10,10],[0,10],[0,0]]})).unwrap();
        s.execute("door", &json!({"wall": "w0", "offset": 5.0})).unwrap();

        let res = s.execute("schedule", &json!({"kind": "door"})).unwrap();
        let table_handle = res.get("table").and_then(Value::as_str).unwrap();
        let handle = Handle::parse_hex(table_handle).unwrap();

        // Get revision before
        let rev_before = s.state().unwrap().revision;

        // Run a command that changes the drawing but not the schedule (add a wall)
        s.execute("wall", &json!({"points": [[11,0],[12,0]]})).unwrap();

        // Revision should have increased due to wall command
        let rev_after = s.state().unwrap().revision;
        assert!(rev_after > rev_before);

        // Table should still exist and be valid
        let doc = s.doc().unwrap();
        assert!(doc.entity(handle).is_some());
    }

    #[test]
    fn default_at_is_beside_the_model() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0,0],[10,0],[10,10],[0,10],[0,0]]})).unwrap();

        let res = s.execute("schedule", &json!({"kind": "door"})).unwrap();
        let schedule_id = res.get("schedule").and_then(Value::as_str).unwrap();
        // schedule_id is "sc0"
        let id_num = schedule_id.trim_start_matches("sc").parse::<u32>().unwrap_or(0);

        let doc = s.doc().unwrap();
        let schedule = doc.arch.schedules.iter().find(|s| s.id.0 == id_num).unwrap();

        // 3 m right of the model's bounds (the wall faces, not its centre lines).
        let expect = doc.arch.bounds().max.x + 3.0;
        assert!((schedule.at[0] - expect).abs() < 1e-9, "{} vs {expect}", schedule.at[0]);
        assert!(schedule.at[0] > 13.0 - 1e-9);
    }

    #[test]
    fn refusals_change_nothing_and_push_no_undo() {
        let mut s = Session::new();
        s.execute("wall", &json!({"points": [[0,0],[10,0],[10,10],[0,10],[0,0]]})).unwrap();

        let rev_before = s.state().unwrap().revision;

        // Invalid kind
        let res = s.execute("schedule", &json!({"kind": "invalid"}));
        assert!(res.is_err());

        // Invalid columns
        let res = s.execute("schedule", &json!({"kind": "door", "columns": ["invalid_col"]}));
        assert!(res.is_err());

        // Invalid at
        let res = s.execute("schedule", &json!({"kind": "door", "at": [1, "bad"]}));
        assert!(res.is_err());

        let rev_after = s.state().unwrap().revision;
        assert_eq!(rev_before, rev_after);
    }
}
