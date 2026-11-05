//! Commands that add, copy, edit and delete user-catalog items, and manage the trash.

use std::path::Path;

use cadcraft_arch::library::{ItemKind, ItemRecord};
use cadcraft_doc::arch::{ArchId, Cabinet, Fixture, Furnishing};
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, always, bad};
use crate::library::user::{
    StoredObjects, UserItemFile, empty_trash, list_trash, move_to_trash, read_user_file, restore_from_trash, save_group_item, unique_slug,
    write_user_file,
};
use crate::{Result, Session};

const ADD: &str = "library.add";
const COPY: &str = "library.copy";
const EDIT: &str = "library.edit";
const DELETE: &str = "library.delete";
const TRASH: &str = "library.trash";

fn object(p: &Value) -> Result<&serde_json::Map<String, Value>> {
    match p {
        Value::Object(m) => Ok(m),
        _ => Err(bad(ADD, "the parameters must be an object")),
    }
}

fn field<'a>(m: Option<&'a serde_json::Map<String, Value>>, key: &str) -> Option<&'a Value> {
    m.and_then(|m| m.get(key)).filter(|v| !v.is_null())
}

fn text<'a>(cmd: &str, m: Option<&'a serde_json::Map<String, Value>>, key: &str) -> Result<&'a str> {
    match field(m, key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim()),
        Some(Value::String(_)) => Err(bad(cmd, format!("`{key}` is empty"))),
        Some(_) => Err(bad(cmd, format!("`{key}` must be a string"))),
        None => Err(bad(cmd, format!("`{key}` is required"))),
    }
}

fn parse_id(label: &str) -> Option<(char, u32)> {
    let s = label.trim();
    if let Some(n) = s.strip_prefix('f') {
        return n.parse::<u32>().ok().map(|v| ('f', v));
    }
    if let Some(n) = s.strip_prefix("cb") {
        return n.parse::<u32>().ok().map(|v| ('c', v));
    }
    if let Some(n) = s.strip_prefix("fx") {
        return n.parse::<u32>().ok().map(|v| ('x', v));
    }
    None
}

fn bounding_center(doc: &cadcraft_doc::Drawing, refs: &[String]) -> Option<[f64; 2]> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut seen = false;
    for ref_name in refs {
        if let Some((kind, id)) = parse_id(ref_name) {
            match kind {
                'f' => {
                    if let Some(f) = doc.arch.furnishing(ArchId(id)) {
                        seen = true;
                        min_x = min_x.min(f.at[0]);
                        min_y = min_y.min(f.at[1]);
                        max_x = max_x.max(f.at[0]);
                        max_y = max_y.max(f.at[1]);
                    }
                }
                'c' => {
                    if let Some(c) = doc.arch.cabinets.iter().find(|c| c.id == ArchId(id)) {
                        seen = true;
                        let x = c.position.x;
                        let y = c.position.y;
                        min_x = min_x.min(x);
                        min_y = min_y.min(y);
                        max_x = max_x.max(x);
                        max_y = max_y.max(y);
                    }
                }
                'x' => {
                    if let Some(f) = doc.arch.fixtures.iter().find(|f| f.id == ArchId(id)) {
                        seen = true;
                        let x = f.position.x;
                        let y = f.position.y;
                        min_x = min_x.min(x);
                        min_y = min_y.min(y);
                        max_x = max_x.max(x);
                        max_y = max_y.max(y);
                    }
                }
                _ => {}
            }
        }
    }
    if !seen {
        return None;
    }
    Some([(min_x + max_x) / 2.0, (min_y + max_y) / 2.0])
}

fn relative_value<T: serde::Serialize>(value: &T, point: [f64; 2]) -> Value {
    let mut out = serde_json::to_value(value).unwrap_or(Value::Null);
    if let Some(obj) = out.as_object_mut() {
        if let Some(at) = obj.get_mut("at") {
            let arr = at.as_array_mut().unwrap();
            arr[0] = json!(arr[0].as_f64().unwrap_or_default() - point[0]);
            arr[1] = json!(arr[1].as_f64().unwrap_or_default() - point[1]);
        }
        if let Some(pos) = obj.get_mut("position") {
            let arr = pos.as_array_mut().unwrap();
            arr[0] = json!(arr[0].as_f64().unwrap_or_default() - point[0]);
            arr[1] = json!(arr[1].as_f64().unwrap_or_default() - point[1]);
        }
    }
    out
}

fn save_user_item_file(s: &mut Session, rec: ItemRecord, objects: StoredObjects, source_item: Option<String>) -> Result<Value> {
    let Some(dir) = s.library.user_dir() else {
        return Err(bad(ADD, "there is no library folder"));
    };
    let slug = unique_slug(&rec.name, &dir).map_err(|e| bad(ADD, e))?;
    let id = format!("user:{slug}");
    let mut record = rec;
    record.id = id.clone();
    record.catalog = "user".to_string();
    record.read_only = false;
    record.kind = ItemKind::Group;
    let item = UserItemFile { record, objects, symbols: Default::default(), source_item };
    write_user_file(&dir, &item).map_err(|e| bad(ADD, e))?;
    s.library.reload();
    Ok(json!({ "item": item.record.id, "objects": item.objects.furnishings.len() + item.objects.cabinets.len() + item.objects.fixtures.len() }))
}

fn store_group_objects(s: &Session, refs: &[String], insertion: Option<[f64; 2]>) -> Result<StoredObjects> {
    let doc = s.doc()?;
    let point = insertion.or_else(|| bounding_center(doc, refs)).ok_or_else(|| bad(ADD, "the objects do not fit in the active drawing"))?;
    let mut out = StoredObjects::default();
    for label in refs {
        let Some((kind, id)) = parse_id(label) else {
            return Err(bad(ADD, format!("unknown object `{label}`")));
        };
        match kind {
            'f' => {
                let f = doc.arch.furnishing(ArchId(id)).ok_or_else(|| bad(ADD, format!("unknown furnishing `{label}`")))?;
                let v = relative_value(f, point);
                out.furnishings.push(v);
            }
            'c' => {
                let c = doc.arch.cabinets.iter().find(|c| c.id == ArchId(id)).ok_or_else(|| bad(ADD, format!("unknown cabinet `{label}`")))?;
                let v = relative_value(c, point);
                out.cabinets.push(v);
            }
            'x' => {
                let x = doc.arch.fixtures.iter().find(|x| x.id == ArchId(id)).ok_or_else(|| bad(ADD, format!("unknown fixture `{label}`")))?;
                let v = relative_value(x, point);
                out.fixtures.push(v);
            }
            _ => {}
        }
    }
    Ok(out)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new(ADD, "Add to User Catalog", run_add)
            .menu(&["Architecture", "Add to User Catalog"])
            .enabled(always)
            .noundo()
            .params("{objects: [\"f0\", \"cb1\"], name, insertionPoint?: [x, y]}"),
        CommandSpec::new(COPY, "Copy to User Catalog", run_copy)
            .menu(&["Architecture", "Copy to User Catalog"])
            .enabled(always)
            .noundo()
            .params("{item: \"armchair\"}"),
        CommandSpec::new(EDIT, "Edit Library Item", run_edit)
            .menu(&["Architecture", "Edit Library Item"])
            .enabled(always)
            .noundo()
            .params("{item, name?, keywords?: [..], notes?, size?}"),
        CommandSpec::new(DELETE, "Delete Library Item", run_delete)
            .menu(&["Architecture", "Delete Library Item"])
            .enabled(always)
            .noundo()
            .params("{item}"),
        CommandSpec::new(TRASH, "Library Trash", run_trash)
            .menu(&["Architecture", "Library Trash"])
            .enabled(always)
            .noundo()
            .params("{action: \"list\" | \"restore\" | \"empty\", item?}"),
    ]
}

fn run_add(s: &mut Session, p: &Value) -> Result<Value> {
    let m = object(p)?;
    let names: Vec<String> = match field(Some(m), "objects") {
        Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        _ => return Err(bad(ADD, "`objects` must be a non-empty array of object labels")),
    };
    if names.is_empty() {
        return Err(bad(ADD, "`objects` is empty"));
    }
    let name = text(ADD, Some(m), "name")?;
    let insertion = match field(Some(m), "insertionPoint") {
        Some(Value::Array(a)) if a.len() == 2 => Some([
            a[0].as_f64().ok_or_else(|| bad(ADD, "`insertionPoint` must be [x, y]"))?,
            a[1].as_f64().ok_or_else(|| bad(ADD, "`insertionPoint` must be [x, y]"))?,
        ]),
        None => None,
        _ => return Err(bad(ADD, "`insertionPoint` must be [x, y]")),
    };
    let mut objects = store_group_objects(s, &names, insertion)?;
    let record = ItemRecord {
        id: "user:temp".to_string(),
        name: name.to_string(),
        category: "User catalog".to_string(),
        folder: "User catalog".to_string(),
        kind: ItemKind::Group,
        symbol_type: "group".to_string(),
        catalog: "user".to_string(),
        source: "built in".to_string(),
        licence: "CC0-1.0".to_string(),
        author: "CADCraft".to_string(),
        attribution: String::new(),
        keywords: vec![],
        tags: vec![],
        notes: String::new(),
        ..Default::default()
    };
    save_user_item_file(s, record, objects, None)
}

fn run_copy(s: &mut Session, p: &Value) -> Result<Value> {
    let m = object(p)?;
    let item = text(COPY, Some(m), "item")?;
    let Some(rec) = s.library.item(item).cloned() else {
        return Err(bad(COPY, format!("no library item `{item}`")));
    };
    // Copy to User Catalog is how a stock (read-only) item becomes editable, so a stock item is exactly what it takes
    // (11-04 it refused one). The copy is named after the item it copies (`sofa-futon` -> `user:sofa-futon`, then -2,
    // -3...), not after its display name.
    let Some(dir) = s.library.user_dir() else {
        return Err(bad(COPY, "there is no library folder"));
    };
    let slug = unique_slug(item.strip_prefix("user:").unwrap_or(item), &dir).map_err(|e| bad(COPY, e))?;
    let mut copy = rec.clone();
    copy.id = format!("user:{slug}");
    copy.catalog = "user".to_string();
    copy.read_only = false;
    // A copy belongs to the User catalog, filed under its source's category (as library.add's groups belong to it).
    copy.folder = rec.category.clone();
    copy.category = cadcraft_arch::library::Category::User.name().to_string();
    copy.kind = ItemKind::Group;
    let file = UserItemFile { record: copy, objects: StoredObjects::default(), symbols: Default::default(), source_item: Some(item.to_string()) };
    write_user_file(&dir, &file).map_err(|e| bad(COPY, e))?;
    s.library.reload();
    Ok(json!({ "item": file.record.id }))
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    let m = object(p)?;
    let item = text(EDIT, Some(m), "item")?;
    let Some(dir) = s.library.user_dir() else {
        return Err(bad(EDIT, "there is no library folder"));
    };
    // Only a user item (`user:<slug>`) is edited or deleted: read_user_file maps a bare stock id to the same slug, so
    // `sofa-futon` would reach the user's copy `user:sofa-futon` (11-05).
    if !item.starts_with("user:") {
        return Err(bad(EDIT, "a stock item is read-only: Copy to User Catalog first"));
    }
    let mut file = read_user_file(&dir, item).map_err(|e| bad(EDIT, e))?;
    if file.record.read_only || !file.record.id.starts_with("user:") {
        return Err(bad(EDIT, "a stock item is read-only: Copy to User Catalog first"));
    }
    if let Some(name) = field(Some(m), "name") {
        if let Some(v) = name.as_str() {
            file.record.name = v.to_string();
        }
    }
    if let Some(keywords) = field(Some(m), "keywords") {
        if let Some(v) = keywords.as_array() {
            file.record.keywords = v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
        }
    }
    if let Some(notes) = field(Some(m), "notes") {
        if let Some(v) = notes.as_str() {
            file.record.notes = v.to_string();
        }
    }
    if let Some(size) = field(Some(m), "size") {
        if let Some(a) = size.as_array() {
            let values: Vec<f64> = a.iter().filter_map(|v| v.as_f64()).collect();
            if values.len() == 3 {
                file.record.size = Some([values[0], values[1], values[2]]);
            }
        }
    }
    write_user_file(&dir, &file).map_err(|e| bad(EDIT, e))?;
    s.library.reload();
    Ok(json!({ "item": file.record.id, "name": file.record.name, "keywords": file.record.keywords, "notes": file.record.notes }))
}

fn run_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let m = object(p)?;
    let item = text(DELETE, Some(m), "item")?;
    let Some(dir) = s.library.user_dir() else {
        return Err(bad(DELETE, "there is no library folder"));
    };
    // Only a user item (`user:<slug>`) is edited or deleted: read_user_file maps a bare stock id to the same slug, so
    // `sofa-futon` would reach the user's copy `user:sofa-futon` (11-05).
    if !item.starts_with("user:") {
        return Err(bad(DELETE, "a stock item is read-only: Copy to User Catalog first"));
    }
    let file = read_user_file(&dir, item).map_err(|e| bad(DELETE, e))?;
    if file.record.read_only || !file.record.id.starts_with("user:") {
        return Err(bad(DELETE, "a stock item is read-only: Copy to User Catalog first"));
    }
    move_to_trash(&dir, &file.record.id).map_err(|e| bad(DELETE, e))?;
    s.library.reload();
    Ok(json!({ "trashed": file.record.id }))
}

fn run_trash(s: &mut Session, p: &Value) -> Result<Value> {
    let m = object(p)?;
    let action = text(TRASH, Some(m), "action")?;
    let Some(dir) = s.library.user_dir() else {
        return Err(bad(TRASH, "there is no library folder"));
    };
    match action {
        "list" => Ok(json!({ "trash": list_trash(&dir) })),
        "restore" => {
            let item = text(TRASH, Some(m), "item")?;
            restore_from_trash(&dir, item).map_err(|e| bad(TRASH, e))?;
            s.library.reload();
            Ok(json!({ "restored": item }))
        }
        "empty" => {
            let n = empty_trash(&dir);
            s.library.reload();
            Ok(json!({ "emptied": n }))
        }
        _ => Err(bad(TRASH, "`action` must be \"list\", \"restore\" or \"empty\"")),
    }
}
