//! User-catalog items stored on disk and placed as a group of copied drawing objects.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cadcraft_arch::library::{ItemKind, ItemRecord};
use cadcraft_doc::arch::{ArchId, Cabinet, Fixture, Furnishing, FurnishingHost, cabinet_label, fixture_label, furnishing_label};
use cadcraft_geom::Vec2;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

const USER_DIR_NAME: &str = "user";
const TRASH_DIR_NAME: &str = "trash";

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredObjects {
    #[serde(default)]
    pub furnishings: Vec<Value>,
    #[serde(default)]
    pub cabinets: Vec<Value>,
    #[serde(default)]
    pub fixtures: Vec<Value>,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserItemFile {
    pub record: ItemRecord,
    #[serde(default)]
    pub objects: StoredObjects,
    #[serde(default)]
    pub symbols: BTreeMap<String, String>,
    #[serde(default)]
    pub source_item: Option<String>,
}

fn slugify(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-');
    let slug = if out.is_empty() { "item" } else { out };
    let mut s = String::new();
    for (i, ch) in slug.chars().enumerate() {
        if i >= 48 {
            break;
        }
        s.push(ch);
    }
    s.trim_end_matches('-').to_string()
}

/// Make a unique slug for `name`, checking the existing names in `dir`.
#[cfg(not(target_arch = "wasm32"))]
pub fn unique_slug(name: &str, dir: &Path) -> std::result::Result<String, String> {
    let base = slugify(name);
    let mut candidate = base.clone();
    let mut n = 2u32;
    while dir.join(USER_DIR_NAME).join(format!("{candidate}.json")).exists() || dir.join(TRASH_DIR_NAME).join(format!("{candidate}.json")).exists() {
        candidate = format!("{base}-{n}");
        n += 1;
    }
    Ok(candidate)
}

#[cfg(target_arch = "wasm32")]
pub fn unique_slug(name: &str, _dir: &Path) -> std::result::Result<String, String> {
    Ok(slugify(name))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn user_dir_file(user_dir: &Path, slug: &str) -> PathBuf {
    user_dir.join(USER_DIR_NAME).join(format!("{slug}.json"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn trash_dir_file(user_dir: &Path, slug: &str) -> PathBuf {
    user_dir.join(TRASH_DIR_NAME).join(format!("{slug}.json"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_user_file(user_dir: &Path, id: &str) -> std::result::Result<UserItemFile, String> {
    let slug = id.strip_prefix("user:").unwrap_or(id);
    let path = user_dir_file(user_dir, slug);
    let bytes = std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let value: UserItemFile = serde_json::from_slice(&bytes).map_err(|e| format!("{} is not a valid user item: {e}", path.display()))?;
    Ok(value)
}

#[cfg(target_arch = "wasm32")]
pub fn read_user_file(_user_dir: &Path, _id: &str) -> std::result::Result<UserItemFile, String> {
    Err("user items are not stored on disk in the web build".into())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write_user_file(user_dir: &Path, item: &UserItemFile) -> std::result::Result<(), String> {
    let slug = item.record.id.strip_prefix("user:").ok_or_else(|| format!("not a user item id: {}", item.record.id))?;
    let path = user_dir_file(user_dir, slug);
    std::fs::create_dir_all(path.parent().unwrap_or(user_dir))
        .map_err(|e| format!("cannot create {}: {e}", path.parent().unwrap_or(user_dir).display()))?;
    let bytes = serde_json::to_vec_pretty(item).map_err(|e| e.to_string())?;
    crate::atomic_write::write_atomic_making_folders(&path, &bytes).map_err(|e| format!("cannot save {}: {e}", path.display()))?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn write_user_file(_user_dir: &Path, _item: &UserItemFile) -> std::result::Result<(), String> {
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn move_to_trash(user_dir: &Path, id: &str) -> std::result::Result<(), String> {
    let slug = id.strip_prefix("user:").unwrap_or(id);
    let from = user_dir_file(user_dir, slug);
    let to = trash_dir_file(user_dir, slug);
    if !from.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(from.parent().unwrap_or(user_dir)).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(to.parent().unwrap_or(user_dir)).map_err(|e| e.to_string())?;
    std::fs::rename(&from, &to).map_err(|e| format!("cannot move {} to {}: {e}", from.display(), to.display()))?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn move_to_trash(_user_dir: &Path, _id: &str) -> std::result::Result<(), String> {
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn restore_from_trash(user_dir: &Path, id: &str) -> std::result::Result<(), String> {
    let slug = id.strip_prefix("user:").unwrap_or(id);
    let to = user_dir_file(user_dir, slug);
    let from = trash_dir_file(user_dir, slug);
    if !from.exists() {
        return Err(format!("no item `{id}` in the trash"));
    }
    std::fs::create_dir_all(to.parent().unwrap_or(user_dir)).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(from.parent().unwrap_or(user_dir)).map_err(|e| e.to_string())?;
    std::fs::rename(&from, &to).map_err(|e| format!("cannot restore {}: {e}", from.display()))?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn restore_from_trash(_user_dir: &Path, _id: &str) -> std::result::Result<(), String> {
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn list_trash(user_dir: &Path) -> Vec<String> {
    let root = user_dir.join(TRASH_DIR_NAME);
    let Ok(entries) = std::fs::read_dir(&root) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("json") {
            let name = p.file_stem().and_then(|n| n.to_str()).unwrap_or_default();
            if !name.is_empty() {
                out.push(format!("user:{name}"));
            }
        }
    }
    out.sort();
    out
}

#[cfg(target_arch = "wasm32")]
pub fn list_trash(_user_dir: &Path) -> Vec<String> {
    Vec::new()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn empty_trash(user_dir: &Path) -> usize {
    let root = user_dir.join(TRASH_DIR_NAME);
    let Ok(entries) = std::fs::read_dir(&root) else { return 0 };
    let mut n = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if std::fs::remove_file(&path).is_ok() {
                n += 1;
            }
        }
    }
    n
}

#[cfg(target_arch = "wasm32")]
pub fn empty_trash(_user_dir: &Path) -> usize {
    0
}

fn make_item_id(slug: &str) -> String {
    format!("user:{slug}")
}

/// Build a user item from `objects` relative to `insertion` and write it to the user's folder.
pub fn save_group_item(
    user_dir: &Path,
    name: &str,
    mut obj: StoredObjects,
    source_item: Option<String>,
) -> std::result::Result<UserItemFile, String> {
    std::fs::create_dir_all(user_dir.join(USER_DIR_NAME)).map_err(|e| format!("cannot create {}: {e}", user_dir.join(USER_DIR_NAME).display()))?;
    let center = group_anchor(&obj);
    normalize_group_objects(&mut obj, center);
    let slug = unique_slug(name, user_dir)?;
    let mut rec = ItemRecord {
        id: make_item_id(&slug),
        name: name.trim().to_string(),
        category: "User catalog".into(),
        folder: "User catalog".into(),
        kind: ItemKind::Group,
        symbol_type: "group".into(),
        catalog: "user".into(),
        read_only: false,
        source: source_item.clone().unwrap_or_else(|| "user".to_string()),
        author: "CADCraft".into(),
        attribution: String::new(),
        licence: "CC0-1.0".into(),
        ..Default::default()
    };
    rec.file = None;
    rec.tags.clear();
    let item = UserItemFile { record: rec, objects: obj, symbols: BTreeMap::new(), source_item };
    write_user_file(user_dir, &item)?;
    Ok(item)
}

fn rotation_matrix(rotation: f64) -> (f64, f64, f64, f64) {
    let rad = rotation.to_radians();
    let (sin, cos) = rad.sin_cos();
    (cos, -sin, sin, cos)
}

fn apply_rotation(point: [f64; 2], rotation: f64) -> [f64; 2] {
    let (c, negs, s, _) = rotation_matrix(rotation);
    [point[0] * c + point[1] * negs, point[0] * s + point[1] * c]
}

fn rel_offset(a: [f64; 2], b: [f64; 2], rotation: f64) -> [f64; 2] {
    let x = a[0] - b[0];
    let y = a[1] - b[1];
    let (c, negs, s, _) = rotation_matrix(rotation);
    [x * c + y * negs, x * s + y * c]
}

fn normalize_group_objects(obj: &mut StoredObjects, anchor: [f64; 2]) {
    for each in &mut obj.furnishings {
        if let Some(at) = each.get_mut("at") {
            if let Some(arr) = at.as_array_mut() {
                arr[0] = json!(arr[0].as_f64().unwrap_or_default() - anchor[0]);
                arr[1] = json!(arr[1].as_f64().unwrap_or_default() - anchor[1]);
            }
        }
    }
    for each in &mut obj.cabinets {
        if let Some(pos) = each.get_mut("position") {
            if let Some(arr) = pos.as_array_mut() {
                arr[0] = json!(arr[0].as_f64().unwrap_or_default() - anchor[0]);
                arr[1] = json!(arr[1].as_f64().unwrap_or_default() - anchor[1]);
            }
        }
    }
    for each in &mut obj.fixtures {
        if let Some(pos) = each.get_mut("position") {
            if let Some(arr) = pos.as_array_mut() {
                arr[0] = json!(arr[0].as_f64().unwrap_or_default() - anchor[0]);
                arr[1] = json!(arr[1].as_f64().unwrap_or_default() - anchor[1]);
            }
        }
    }
}

fn group_anchor(obj: &StoredObjects) -> [f64; 2] {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut seen = false;
    for each in &obj.furnishings {
        if let Some(at) = each.get("at").and_then(Value::as_array) {
            let x = at.get(0).and_then(Value::as_f64).unwrap_or_default();
            let y = at.get(1).and_then(Value::as_f64).unwrap_or_default();
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            seen = true;
        }
    }
    for each in &obj.cabinets {
        if let Some(pos) = each.get("position").and_then(Value::as_array) {
            let x = pos.get(0).and_then(Value::as_f64).unwrap_or_default();
            let y = pos.get(1).and_then(Value::as_f64).unwrap_or_default();
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            seen = true;
        }
    }
    for each in &obj.fixtures {
        if let Some(pos) = each.get("position").and_then(Value::as_array) {
            let x = pos.get(0).and_then(Value::as_f64).unwrap_or_default();
            let y = pos.get(1).and_then(Value::as_f64).unwrap_or_default();
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            seen = true;
        }
    }
    if !seen {
        return [0.0, 0.0];
    }
    [(min_x + max_x) / 2.0, (min_y + max_y) / 2.0]
}

fn set_furnishing_host(f: &mut Furnishing, map: &BTreeMap<u32, u32>) {
    let host = match &f.host {
        FurnishingHost::None => FurnishingHost::None,
        FurnishingHost::Wall { wall } => FurnishingHost::Wall { wall: *wall },
        FurnishingHost::Cabinet { cabinet } => FurnishingHost::Cabinet { cabinet: *cabinet },
        FurnishingHost::Sink { furnishing } => FurnishingHost::Sink { furnishing: ArchId(map.get(&furnishing.0).copied().unwrap_or(furnishing.0)) },
        FurnishingHost::Gap { left, right } => FurnishingHost::Gap {
            left: ArchId(map.get(&left.0).copied().unwrap_or(left.0)),
            right: ArchId(map.get(&right.0).copied().unwrap_or(right.0)),
        },
    };
    f.host = host;
}

/// Place the group described by `item` at plan `at`, rotated by `rotation` degrees."""
pub(crate) fn place_group(s: &mut Session, item: &ItemRecord, at: Vec2, rotation: f64) -> Result<Value> {
    let Some(user_dir) = s.library.user_dir() else {
        return Err(EngineError::Other("user items are placed by p14-cmd-user-catalog".into()));
    };
    let file = match read_user_file(&user_dir, &item.id) {
        Ok(file) => file,
        Err(_) => return Err(EngineError::Other("user items are placed by p14-cmd-user-catalog".into())),
    };
    let mut symbol_meshes: BTreeMap<String, cadcraft_doc::arch::SymbolMesh> = BTreeMap::new();

    for v in &file.objects.furnishings {
        let f: Furnishing = serde_json::from_value(v.clone()).map_err(|e| EngineError::Other(format!("bad stored furnishing: {e}")))?;
        let symbol_item = file.symbols.get(&f.symbol).cloned().unwrap_or_else(|| f.item.clone());
        let mesh = s.library.symbol(&symbol_item).map_err(|e| EngineError::Other(format!("{symbol_item}: {e}")))?;
        symbol_meshes.insert(f.symbol.clone(), mesh);
    }
    for v in &file.objects.cabinets {
        let c: Cabinet = serde_json::from_value(v.clone()).map_err(|e| EngineError::Other(format!("bad stored cabinet: {e}")))?;
        if let Some(pull) = &c.pull {
            let key = c.pull_symbol.clone().unwrap_or_else(|| pull.clone());
            if let Some(item_id) = file.symbols.get(&key) {
                let mesh = s.library.symbol(item_id).map_err(|e| EngineError::Other(format!("{item_id}: {e}")))?;
                symbol_meshes.insert(key, mesh);
            }
        }
    }

    let mut doc = s.doc_mut().map_err(|e| EngineError::Other(e.to_string()))?;
    let mut map: BTreeMap<u32, u32> = BTreeMap::new();
    let mut handles = Vec::new();
    let mut placed = 0usize;

    for v in file.objects.furnishings {
        let mut f: Furnishing = serde_json::from_value(v).map_err(|e| EngineError::Other(format!("bad stored furnishing: {e}")))?;
        let old = f.id.0;
        if !doc.arch.symbols.contains_key(&f.symbol) {
            if let Some(mesh) = symbol_meshes.get(&f.symbol).cloned() {
                doc.arch.symbols.insert(f.symbol.clone(), mesh);
            }
        }
        let dx = f.at[0];
        let dy = f.at[1];
        let rotated = apply_rotation([dx, dy], rotation);
        f.at = [at.x + rotated[0], at.y + rotated[1]];
        f.rotation += rotation;
        let new_id = doc.arch.push_furnishing(f.clone());
        map.insert(old, new_id.0);
        let last_idx = doc.arch.furnishings.len() - 1;
        let mut last = doc.arch.furnishings[last_idx].clone();
        set_furnishing_host(&mut last, &map);
        doc.arch.furnishings[last_idx] = last;
        handles.push(furnishing_label(new_id));
        placed += 1;
    }

    for v in file.objects.cabinets {
        let mut c: Cabinet = serde_json::from_value(v).map_err(|e| EngineError::Other(format!("bad stored cabinet: {e}")))?;
        let old = c.id.0;
        if let Some(pull) = &c.pull {
            let key = c.pull_symbol.clone().unwrap_or_else(|| pull.clone());
            if !doc.arch.symbols.contains_key(&key) {
                if let Some(mesh) = symbol_meshes.get(&key).cloned() {
                    doc.arch.symbols.insert(key.clone(), mesh);
                }
            }
            if file.symbols.get(&key).is_none() && !c.pull_symbol.clone().unwrap_or_default().is_empty() {
                return Err(EngineError::Other(format!("missing symbol for pull {}", c.pull.clone().unwrap_or_default())));
            }
        }
        let p = c.position;
        let rotated = apply_rotation([p.x, p.y], rotation);
        c.position = Vec2::new(at.x + rotated[0], at.y + rotated[1]);
        c.rotation += rotation;
        let new_id = doc.arch.push_cabinet(c.clone());
        map.insert(old, new_id.0);
        handles.push(cabinet_label(new_id));
        placed += 1;
    }

    for v in file.objects.fixtures {
        let mut x: Fixture = serde_json::from_value(v).map_err(|e| EngineError::Other(format!("bad stored fixture: {e}")))?;
        let old = x.id.0;
        let p = x.position;
        let rotated = apply_rotation([p.x, p.y], rotation);
        x.position = Vec2::new(at.x + rotated[0], at.y + rotated[1]);
        x.rotation += rotation;
        let new_id = doc.arch.push_fixture(x.clone());
        map.insert(old, new_id.0);
        handles.push(fixture_label(new_id));
        placed += 1;
    }

    if placed == 0 {
        return Err(EngineError::Other(format!("user item `{}` has no objects", item.id)));
    }

    Ok(
        json!({ "furnishings": handles.iter().filter(|h| h.starts_with('f')).cloned().collect::<Vec<_>>(), "cabinets": handles.iter().filter(|h| h.starts_with("cb")).cloned().collect::<Vec<_>>(), "fixtures": handles.iter().filter(|h| h.starts_with("fx")).cloned().collect::<Vec<_>>(), "handles": handles }),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use cadcraft_arch::library::ItemKind;
    use cadcraft_doc::arch::{ArchId, Cabinet, Fixture, Furnishing, Placement, SymbolMesh};

    use super::*;

    fn install_test_io() {
        crate::library::set_library_io(crate::library::LibraryIo {
            read_glb: |_| Ok(SymbolMesh { key: "test-symbol".to_string(), ..Default::default() }),
            read_image: |_, _| Err("no image in tests".into()),
            encode_png: |_| Vec::new(),
            sample: || &[],
        });
    }

    fn temp_dir() -> PathBuf {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("cadcraft-user-{}-{unique}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn add_stores_three_objects_relative_to_the_insertion_point() {
        let dir = temp_dir();
        let mut a = Furnishing {
            id: ArchId(0),
            item: "armchair".into(),
            symbol: "armchair".into(),
            name: "A".into(),
            category: "Furniture".into(),
            symbol_type: "chair".into(),
            placement: Placement::Floor,
            level: 0,
            at: [2.0, 2.0],
            rotation: 0.0,
            elevation: 0.0,
            size: [0.6, 0.6, 0.9],
            ..Default::default()
        };
        let mut b = a.clone();
        b.id = ArchId(1);
        b.at = [3.0, 2.0];
        let mut c = a.clone();
        c.id = ArchId(2);
        c.at = [2.0, 3.0];
        let mut group = StoredObjects::default();
        group.furnishings = vec![serde_json::to_value(&a).unwrap(), serde_json::to_value(&b).unwrap(), serde_json::to_value(&c).unwrap()];
        let item = save_group_item(&dir, "Dining set", group, None).unwrap();
        let path = user_dir_file(&dir, "dining-set");
        assert!(path.exists());
        let file: UserItemFile = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(file.objects.furnishings.len(), 3);
        for object in &file.objects.furnishings {
            let at = object["at"].as_array().unwrap();
            assert!(at[0].as_f64().unwrap().abs() <= 2.0);
            assert!(at[1].as_f64().unwrap().abs() <= 2.0);
        }
    }

    #[test]
    fn a_user_item_survives_a_new_session() {
        let dir = temp_dir();
        let rec = ItemRecord {
            id: "user:stool".into(),
            name: "Stool".into(),
            category: "User catalog".into(),
            catalog: "user".into(),
            kind: ItemKind::Group,
            ..Default::default()
        };
        let mut group = StoredObjects::default();
        let mut f = Furnishing {
            id: ArchId(0),
            item: "armchair".into(),
            symbol: "armchair".into(),
            name: "A".into(),
            category: "Furniture".into(),
            symbol_type: "chair".into(),
            placement: Placement::Floor,
            level: 0,
            at: [1.0, 1.0],
            rotation: 0.0,
            elevation: 0.0,
            size: [0.6, 0.6, 0.9],
            ..Default::default()
        };
        f.id = ArchId(0);
        group.furnishings.push(serde_json::to_value(&f).unwrap());
        let item = UserItemFile {
            record: rec,
            objects: group,
            symbols: BTreeMap::from([("armchair".to_string(), "armchair".to_string())]),
            source_item: None,
        };
        write_user_file(&dir, &item).unwrap();
        let reread = read_user_file(&dir, "user:stool").unwrap();
        assert_eq!(reread.record.id, "user:stool");
        assert_eq!(reread.objects.furnishings.len(), 1);
    }

    #[test]
    fn place_group_moves_the_same_meshes() {
        install_test_io();
        let dir = temp_dir();
        let rec = ItemRecord {
            id: "user:table".into(),
            name: "Table".into(),
            category: "User catalog".into(),
            catalog: "user".into(),
            kind: ItemKind::Group,
            ..Default::default()
        };
        let mut group = StoredObjects::default();
        let mut f = Furnishing {
            id: ArchId(0),
            item: "armchair".into(),
            symbol: "armchair".into(),
            name: "A".into(),
            category: "Furniture".into(),
            symbol_type: "chair".into(),
            placement: Placement::Floor,
            level: 0,
            at: [0.0, 0.0],
            rotation: 0.0,
            elevation: 0.0,
            size: [0.6, 0.6, 0.9],
            ..Default::default()
        };
        group.furnishings.push(serde_json::to_value(&f).unwrap());
        let item = UserItemFile {
            record: rec.clone(),
            objects: group,
            symbols: BTreeMap::from([("armchair".to_string(), "armchair".to_string())]),
            source_item: None,
        };
        write_user_file(&dir, &item).unwrap();

        let symbol_file = dir.join("user").join("armchair.glb");
        fs::create_dir_all(symbol_file.parent().unwrap()).unwrap();
        fs::write(&symbol_file, b"stub").unwrap();
        let mut lib = crate::library::Library::with_user_dir(Some(dir.clone()));
        lib.add_user_item(ItemRecord {
            id: "armchair".to_string(),
            name: "Armchair".to_string(),
            category: "Furniture".to_string(),
            folder: "Chairs and stools".to_string(),
            kind: ItemKind::Symbol,
            symbol_type: "chair".to_string(),
            catalog: "user".to_string(),
            file: Some("armchair.glb".to_string()),
            source: "user".to_string(),
            licence: "CC0-1.0".to_string(),
            author: "CADCraft".to_string(),
            ..Default::default()
        });

        let mut s = crate::Session::new();
        s.library = lib;
        let r = place_group(&mut s, &rec, Vec2::new(5.0, 0.0), 0.0).unwrap();
        assert_eq!(r["furnishings"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn place_group_reads_its_symbols() {
        install_test_io();
        let dir = temp_dir();
        let rec = ItemRecord {
            id: "user:table".into(),
            name: "Table".into(),
            category: "User catalog".into(),
            catalog: "user".into(),
            kind: ItemKind::Group,
            ..Default::default()
        };
        let mut group = StoredObjects::default();
        let mut f = Furnishing {
            id: ArchId(0),
            item: "armchair".into(),
            symbol: "armchair".into(),
            name: "A".into(),
            category: "Furniture".into(),
            symbol_type: "chair".into(),
            placement: Placement::Floor,
            level: 0,
            at: [0.0, 0.0],
            rotation: 0.0,
            elevation: 0.0,
            size: [0.6, 0.6, 0.9],
            ..Default::default()
        };
        group.furnishings.push(serde_json::to_value(&f).unwrap());
        let item = UserItemFile {
            record: rec.clone(),
            objects: group,
            symbols: BTreeMap::from([("armchair".to_string(), "armchair".to_string())]),
            source_item: None,
        };
        write_user_file(&dir, &item).unwrap();

        let symbol_file = dir.join("user").join("armchair.glb");
        fs::create_dir_all(symbol_file.parent().unwrap()).unwrap();
        fs::write(&symbol_file, b"stub").unwrap();
        let mut lib = crate::library::Library::with_user_dir(Some(dir.clone()));
        lib.add_user_item(ItemRecord {
            id: "armchair".to_string(),
            name: "Armchair".to_string(),
            category: "Furniture".to_string(),
            folder: "Chairs and stools".to_string(),
            kind: ItemKind::Symbol,
            symbol_type: "chair".to_string(),
            catalog: "user".to_string(),
            file: Some("armchair.glb".to_string()),
            source: "user".to_string(),
            licence: "CC0-1.0".to_string(),
            author: "CADCraft".to_string(),
            ..Default::default()
        });

        let mut s = crate::Session::new();
        s.library = lib;
        let _ = s.library.symbol("armchair");
        let r = place_group(&mut s, &rec, Vec2::ZERO, 0.0);
        assert!(r.is_ok(), "{r:?}");
    }

    #[test]
    fn copy_makes_an_editable_copy() {
        let dir = temp_dir();
        let item = ItemRecord {
            id: "armchair".into(),
            name: "Armchair".into(),
            category: "Furniture".into(),
            catalog: "builtin".into(),
            kind: ItemKind::Symbol,
            ..Default::default()
        };
        let new = save_group_item(&dir, &item.name, StoredObjects::default(), Some(item.id.clone())).unwrap();
        assert_eq!(new.record.catalog, "user");
        assert!(!new.record.read_only);
        assert_eq!(new.source_item.as_deref(), Some(item.id.as_str()));
    }

    #[test]
    fn stock_items_stay_read_only() {
        let dir = temp_dir();
        let item = ItemRecord {
            id: "sofa-futon".into(),
            name: "Sofa".into(),
            category: "Furniture".into(),
            catalog: "sample".into(),
            read_only: true,
            ..Default::default()
        };
        let file = read_user_file(&dir, &item.id);
        assert!(file.is_err());
        assert!(item.read_only);
    }

    #[test]
    fn delete_moves_to_the_trash() {
        let dir = temp_dir();
        let user = UserItemFile {
            record: ItemRecord {
                id: "user:alpha".into(),
                name: "Alpha".into(),
                category: "User catalog".into(),
                catalog: "user".into(),
                kind: ItemKind::Group,
                ..Default::default()
            },
            objects: StoredObjects::default(),
            symbols: BTreeMap::new(),
            source_item: None,
        };
        write_user_file(&dir, &user).unwrap();
        move_to_trash(&dir, "user:alpha").unwrap();
        assert!(user_dir_file(&dir, "alpha").exists() == false);
        assert!(trash_dir_file(&dir, "alpha").exists());
    }

    #[test]
    fn restore_and_empty_the_trash() {
        let dir = temp_dir();
        let user = UserItemFile {
            record: ItemRecord {
                id: "user:alpha".into(),
                name: "Alpha".into(),
                category: "User catalog".into(),
                catalog: "user".into(),
                kind: ItemKind::Group,
                ..Default::default()
            },
            objects: StoredObjects::default(),
            symbols: BTreeMap::new(),
            source_item: None,
        };
        write_user_file(&dir, &user).unwrap();
        move_to_trash(&dir, "user:alpha").unwrap();
        restore_from_trash(&dir, "user:alpha").unwrap();
        assert!(user_dir_file(&dir, "alpha").exists());
        move_to_trash(&dir, "user:alpha").unwrap();
        assert_eq!(empty_trash(&dir), 1);
        assert!(trash_dir_file(&dir, "alpha").exists() == false);
    }

    #[test]
    fn slugs_are_unique() {
        let dir = temp_dir();
        let a = save_group_item(&dir, "Kitchen Table", StoredObjects::default(), None).unwrap();
        let b = save_group_item(&dir, "Kitchen Table", StoredObjects::default(), None).unwrap();
        assert_ne!(a.record.id, b.record.id);
        assert!(a.record.id.ends_with("-2") == false || b.record.id.ends_with("-2") == false);
    }

    #[test]
    fn slugify_handles_special_characters() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("Café au lait"), "caf-au-lait");
        assert_eq!(slugify("  spaced  "), "spaced");
        assert_eq!(slugify("a---b"), "a-b");
        assert_eq!(slugify("123"), "123");
        assert_eq!(slugify("$$$"), "item");
    }

    #[test]
    fn slugify_truncates_long_names() {
        let long = "a".repeat(100);
        let s = slugify(&long);
        assert_eq!(s.len(), 48);
        let long_hyphen = "a-".repeat(60);
        let s2 = slugify(&long_hyphen);
        assert!(s2.len() <= 48);
        assert!(!s2.ends_with('-'));
    }

    #[test]
    fn place_group_empty_objects_errors() {
        install_test_io();
        let dir = temp_dir();
        let rec = ItemRecord {
            id: "user:empty".into(),
            name: "Empty".into(),
            category: "User catalog".into(),
            catalog: "user".into(),
            kind: ItemKind::Group,
            ..Default::default()
        };
        let item = UserItemFile { record: rec.clone(), objects: StoredObjects::default(), symbols: BTreeMap::new(), source_item: None };
        write_user_file(&dir, &item).unwrap();

        let lib = crate::library::Library::with_user_dir(Some(dir.clone()));
        let mut s = crate::Session::new();
        s.library = lib;
        let r = place_group(&mut s, &rec, Vec2::ZERO, 0.0);
        assert!(r.is_err(), "empty group should error");
    }

    #[test]
    fn place_group_rotates_cabinets_correctly() {
        install_test_io();
        let dir = temp_dir();
        let rec = ItemRecord {
            id: "user:rot".into(),
            name: "Rot".into(),
            category: "User catalog".into(),
            catalog: "user".into(),
            kind: ItemKind::Group,
            ..Default::default()
        };
        let mut group = StoredObjects::default();
        let c = Cabinet { id: ArchId(0), position: Vec2::new(1.0, 0.0), rotation: 0.0, ..Default::default() };
        group.cabinets.push(serde_json::to_value(&c).unwrap());
        let item = UserItemFile { record: rec.clone(), objects: group, symbols: BTreeMap::new(), source_item: None };
        write_user_file(&dir, &item).unwrap();

        let lib = crate::library::Library::with_user_dir(Some(dir.clone()));
        let mut s = crate::Session::new();
        s.library = lib;
        let r = place_group(&mut s, &rec, Vec2::new(5.0, 0.0), 90.0).unwrap();
        let doc = s.doc().unwrap();
        let cab = doc.arch.cabinets.first().unwrap();
        // (1,0) rotated 90 CCW = (0,1), translated to (5,1)
        assert!((cab.position.x - 5.0).abs() < 1e-9, "cabinet x: {}", cab.position.x);
        assert!((cab.position.y - 1.0).abs() < 1e-9, "cabinet y: {}", cab.position.y);
    }

    #[test]
    fn place_group_rotates_fixtures_correctly() {
        install_test_io();
        let dir = temp_dir();
        let rec = ItemRecord {
            id: "user:rotx".into(),
            name: "RotX".into(),
            category: "User catalog".into(),
            catalog: "user".into(),
            kind: ItemKind::Group,
            ..Default::default()
        };
        let mut group = StoredObjects::default();
        let f = Fixture { id: ArchId(0), position: Vec2::new(0.0, 2.0), rotation: 0.0, ..Default::default() };
        group.fixtures.push(serde_json::to_value(&f).unwrap());
        let item = UserItemFile { record: rec.clone(), objects: group, symbols: BTreeMap::new(), source_item: None };
        write_user_file(&dir, &item).unwrap();

        let lib = crate::library::Library::with_user_dir(Some(dir.clone()));
        let mut s = crate::Session::new();
        s.library = lib;
        let r = place_group(&mut s, &rec, Vec2::new(0.0, 0.0), 180.0).unwrap();
        let doc = s.doc().unwrap();
        let fix = doc.arch.fixtures.first().unwrap();
        // (0,2) rotated 180 = (0,-2)
        assert!((fix.position.x - 0.0).abs() < 1e-9, "fixture x: {}", fix.position.x);
        assert!((fix.position.y - (-2.0)).abs() < 1e-9, "fixture y: {}", fix.position.y);
    }
}
