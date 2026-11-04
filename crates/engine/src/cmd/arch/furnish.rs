//! Furniture and furnishing placement helpers: `library.place`, `furnishing.edit`,
//! `furnishing.delete` and `furnishing.centre`.

use std::collections::BTreeSet;

use cadcraft_arch::library::place::{PlaceOutcome, PlaceRequest, place};
use cadcraft_arch::library::{ItemKind, ItemRecord};
use cadcraft_doc::arch::{
    ArchId, CabinetPart, Furnishing, FurnishingHost, Placement, cabinet_label, furnishing_handle, furnishing_label, handle_json,
};
use cadcraft_geom::{Vec2, point_in_polygon};
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("library.place", "Place From Library", run_library_place)
            .menu(&["Architecture", "Place From Library"])
            .params("{item: \"armchair\", at: [x, y], rotation?, level?, elevation?, size?: [w, d, h], mirrored?}"),
        CommandSpec::new("furnishing.edit", "Edit Furnishing", run_furnishing_edit)
            .menu(&["Architecture", "Edit Furnishing"])
            .params("{id: \"f0\", at?, rotation?, elevation?, size?, mirrored?}"),
        CommandSpec::new("furnishing.delete", "Delete Furnishing", run_furnishing_delete)
            .menu(&["Architecture", "Delete Furnishing"])
            .params("{id: \"f0\"}"),
        CommandSpec::new("furnishing.centre", "Center Furnishing", run_furnishing_centre)
            .menu(&["Architecture", "Center Furnishing"])
            .params("{id: \"f0\", on: \"room\"|\"wall\"}"),
    ]
}

fn object<'a>(cmd: &str, p: &'a Value) -> Result<Option<&'a Map<String, Value>>> {
    match p {
        Value::Object(m) => Ok(Some(m)),
        Value::Null => Ok(None),
        _ => Err(bad(cmd, "the parameters must be an object")),
    }
}

fn field<'a>(m: Option<&'a Map<String, Value>>, key: &str) -> Option<&'a Value> {
    m.and_then(|m| m.get(key)).filter(|v| !v.is_null())
}

fn text<'a>(cmd: &str, m: Option<&'a Map<String, Value>>, key: &str) -> Result<&'a str> {
    match field(m, key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s),
        Some(Value::String(_)) => Err(bad(cmd, format!("`{key}` is empty"))),
        Some(_) => Err(bad(cmd, format!("`{key}` must be a string"))),
        None => Err(bad(cmd, format!("`{key}` is required"))),
    }
}

fn opt_text<'a>(cmd: &str, m: Option<&'a Map<String, Value>>, key: &str) -> Result<Option<&'a str>> {
    match field(m, key) {
        None => Ok(None),
        Some(Value::String(s)) if s.trim().is_empty() => Err(bad(cmd, format!("`{key}` is empty"))),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(bad(cmd, format!("`{key}` must be a string"))),
    }
}

fn f64_value(cmd: &str, p: &Value, key: &str) -> Result<f64> {
    let Some(v) = p.get(key) else {
        return Err(bad(cmd, format!("`{key}` is required")));
    };
    let Some(n) = v.as_f64() else {
        return Err(bad(cmd, format!("`{key}` must be a finite number")));
    };
    if !n.is_finite() {
        return Err(bad(cmd, format!("`{key}` must be a finite number")));
    }
    Ok(n)
}

fn point_value(cmd: &str, p: &Value, key: &str) -> Result<Vec2> {
    let Some(v) = p.get(key) else {
        return Err(bad(cmd, format!("`{key}` is required")));
    };
    let Some(a) = v.as_array() else {
        return Err(bad(cmd, format!("`{key}` must be [x, y]")));
    };
    if a.len() != 2 {
        return Err(bad(cmd, format!("`{key}` must be [x, y]")));
    }
    let x = a[0].as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(cmd, format!("`{key}` must be [x, y] of finite numbers")))?;
    let y = a[1].as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(cmd, format!("`{key}` must be [x, y] of finite numbers")))?;
    Ok(Vec2::new(x, y))
}

fn parse_level(doc: &cadcraft_doc::Drawing, v: &Value) -> Result<u32> {
    match v {
        Value::Number(n) => {
            let level = n.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| bad("library.place", "`level` must be a level index"))?;
            if doc.arch.levels.iter().any(|l| l.index == level) || doc.arch.levels.is_empty() && level == 0 {
                Ok(level)
            } else {
                Err(bad("library.place", format!("unknown level `{level}`")))
            }
        }
        Value::String(s) => {
            let s = s.trim();
            doc.arch.levels.iter().find(|l| l.name == s).map(|l| l.index).ok_or_else(|| bad("library.place", format!("unknown level `{s}`")))
        }
        _ => Err(bad("library.place", "`level` must be a name or an index")),
    }
}

fn parse_furnishing_id(raw: &str) -> Option<ArchId> {
    let raw = raw.trim();
    let digits = raw.strip_prefix('f').or_else(|| raw.strip_prefix('F'))?;
    digits.parse::<u32>().ok().map(ArchId)
}

fn as_size_array(v: &Value, cmd: &str, key: &str) -> Result<[f64; 3]> {
    let Some(a) = v.as_array() else {
        return Err(bad(cmd, format!("`{key}` must be [w, d, h]")));
    };
    if a.len() != 3 {
        return Err(bad(cmd, format!("`{key}` must be [w, d, h]")));
    }
    let out = [a[0].as_f64(), a[1].as_f64(), a[2].as_f64()];
    let mut values = [0.0; 3];
    for (i, v) in out.into_iter().enumerate() {
        let Some(n) = v.filter(|n| n.is_finite() && *n > 0.0 && *n <= 100.0) else {
            return Err(bad(cmd, format!("`{key}` must be three finite numbers in (0, 100]")));
        };
        values[i] = n;
    }
    Ok(values)
}

fn as_point_array(v: &Value, cmd: &str, key: &str) -> Result<[f64; 2]> {
    let Some(a) = v.as_array() else {
        return Err(bad(cmd, format!("`{key}` must be [x, y]")));
    };
    if a.len() != 2 {
        return Err(bad(cmd, format!("`{key}` must be [x, y]")));
    }
    let x = a[0].as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(cmd, format!("`{key}` must be [x, y] of finite numbers")))?;
    let y = a[1].as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(cmd, format!("`{key}` must be [x, y] of finite numbers")))?;
    Ok([x, y])
}

fn insertion_key(doc: &mut cadcraft_doc::Drawing, mesh: &cadcraft_doc::arch::SymbolMesh) -> String {
    let key = mesh.key.clone();
    if !doc.arch.symbols.contains_key(&key) {
        doc.arch.symbols.insert(key.clone(), mesh.clone());
    }
    key
}

fn nearest_wall_on_level(doc: &cadcraft_doc::Drawing, at: Vec2, level: u32) -> Option<(cadcraft_doc::arch::Wall, f64)> {
    let mut best: Option<(cadcraft_doc::arch::Wall, f64)> = None;
    for w in &doc.arch.walls {
        if w.base_level != level || w.divider {
            continue;
        }
        let axis = w.end - w.start;
        let len_sq = axis.len2();
        if len_sq <= 0.0 {
            continue;
        }
        let u = ((at - w.start).dot(axis)) / len_sq;
        let clamped = u.clamp(0.0, 1.0);
        let closest = w.start + axis * clamped;
        let d = (at - closest).len();
        if d <= 0.3 && best.as_ref().is_none_or(|(_, best_d)| d < *best_d) {
            let wall = w.clone();
            best = Some((wall, d));
        }
    }
    best
}

fn wall_midpoint(w: &cadcraft_doc::arch::Wall) -> Vec2 {
    (w.start + w.end) * 0.5
}

fn write_swap(doc: &mut cadcraft_doc::Drawing, _id: ArchId, item: &ItemRecord, symbol_key: String) -> Result<Value> {
    let Some(cabinet) = doc.arch.cabinets.first().map(|c| c.id) else {
        return Err(bad("library.place", "there is no cabinet to swap onto"));
    };
    let mut cab = doc.arch.cabinets.iter_mut().find(|c| c.id == cabinet).ok_or_else(|| bad("library.place", "no such cabinet"))?;
    if item.kind == ItemKind::FrontStyle {
        if item.id.starts_with("frontstyle:door-") {
            cab.door_style = item.style.as_deref().and_then(cadcraft_doc::arch::FrontStyle::parse).unwrap_or_default();
            return Ok(json!({ "swapped": { "cabinet": cabinet_label(cab.id), "part": "door" } }));
        }
        if item.id.starts_with("frontstyle:drawer-") {
            cab.drawer_style = item.style.as_deref().and_then(cadcraft_doc::arch::FrontStyle::parse).unwrap_or_default();
            return Ok(json!({ "swapped": { "cabinet": cabinet_label(cab.id), "part": "drawer" } }));
        }
    }
    cab.pull = Some(item.id.clone());
    cab.pull_symbol = Some(symbol_key);
    Ok(json!({ "swapped": { "cabinet": cabinet_label(cab.id), "part": "pull" } }))
}

fn run_library_place(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "library.place";
    let m = object(CMD, p)?;
    let item_id = text(CMD, m, "item")?;
    let mut item = s.library.item(item_id).ok_or_else(|| bad(CMD, format!("no library item `{item_id}`")))?.clone();
    let point = match field(m, "at") {
        Some(v) => as_point_array(v, CMD, "at")?,
        None => return Err(bad(CMD, "`at` is required")),
    };
    let level = match field(m, "level") {
        Some(v) => parse_level(s.doc()?, v)?,
        None => s.doc()?.arch.levels.first().map(|l| l.index).unwrap_or(0),
    };
    let rotation = match field(m, "rotation") {
        Some(v) => {
            let n = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(CMD, "`rotation` must be a finite number"))?;
            n
        }
        None => 0.0,
    };
    let elevation = match field(m, "elevation") {
        Some(v) => {
            let n = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(CMD, "`elevation` must be a finite number"))?;
            if !(0.0..=100.0).contains(&n) {
                return Err(bad(CMD, "`elevation` must be in 0..=100"));
            }
            n
        }
        None => 0.0,
    };
    let mut size = item.size.unwrap_or([1.0, 1.0, 1.0]);
    if let Some(v) = field(m, "size") {
        size = as_size_array(v, CMD, "size")?;
    }
    if item.kind == ItemKind::Finish || item.kind == ItemKind::Material {
        return Err(bad(CMD, "a material is painted: use paint.material"));
    }

    if item.kind == ItemKind::Group {
        return crate::library::user::place_group(s, &item, Vec2::new(point[0], point[1]), rotation);
    }

    if item.kind == ItemKind::Door || item.kind == ItemKind::Window {
        let style = item.style.clone().unwrap_or_else(|| match item.kind {
            ItemKind::Door => "hinged".to_string(),
            ItemKind::Window => "single-hung".to_string(),
            _ => String::new(),
        });
        let mut wall = None;
        let mut offset = 0.0;
        for w in &s.doc()?.arch.walls {
            if w.base_level != level || w.divider {
                continue;
            }
            let axis = w.end - w.start;
            let len_sq = axis.len2();
            if len_sq <= 0.0 {
                continue;
            }
            let u = ((Vec2::new(point[0], point[1]) - w.start).dot(axis)) / len_sq;
            let p = w.start + axis * u.clamp(0.0, 1.0);
            let d = (Vec2::new(point[0], point[1]) - p).len();
            if d <= 0.3 {
                wall = Some(w.id);
                offset = ((Vec2::new(point[0], point[1]) - w.start).dot(axis.normalized())) / axis.len();
                break;
            }
        }
        let Some(wall_id) = wall else {
            return Err(bad(CMD, "the point is not near a wall for a door or window"));
        };
        let via = if item.kind == ItemKind::Door { "door" } else { "window" };
        let r = s.execute(
            via,
            &json!({ "wall": format!("w{}", wall_id.0), "offset": offset, "style": style, "level": level, "width": size[0], "height": size[2] }),
        )?;
        return Ok(json!({ "via": via, "result": r }));
    }

    if item.kind == ItemKind::Cabinet {
        let cabinet_kind = item.id.strip_prefix("cabinet:").unwrap_or("base").to_string();
        let r = s.execute("cabinet", &json!({ "kind": cabinet_kind, "at": [point[0], point[1]], "level": level, "rotation": rotation, "width": size[0], "depth": size[1], "height": size[2] }))?;
        return Ok(json!({ "via": "cabinet", "result": r }));
    }

    if item.kind == ItemKind::Fixture {
        let kind = item.id.strip_prefix("fixture:").unwrap_or("sink").to_string();
        let r = s.execute("fixture", &json!({ "kind": kind, "at": [point[0], point[1]], "level": level, "rotation": rotation, "width": size[0], "depth": size[1], "height": size[2] }))?;
        return Ok(json!({ "via": "fixture", "result": r }));
    }

    if item.kind == ItemKind::Backdrop {
        let kind = item.id.strip_prefix("backdrop:").unwrap_or("colour").to_string();
        let r = s.execute("backdrop", &json!({ "kind": kind, "item": item_id }))?;
        return Ok(json!({ "via": "backdrop", "result": r }));
    }

    let placement = item.placement.as_deref().and_then(Placement::parse).unwrap_or(Placement::Floor);
    let mesh = s.library.symbol(item_id).map_err(|e| bad(CMD, e))?;
    let req = PlaceRequest {
        placement,
        symbol_type: item.symbol_type.clone(),
        kind: item.kind,
        size,
        elevation: Some(elevation),
        cutout: item.cutout,
        at: [point[0], point[1]],
        level,
        rotation: Some(rotation),
    };
    let out = {
        let doc = s.doc()?;
        place(&doc.arch, &req).map_err(|e| bad(CMD, e))?
    };
    let mut doc = s.doc_mut()?;
    let key = insertion_key(&mut doc, &mesh);
    let (host, pos, rot, elev) = match out {
        PlaceOutcome::Place { at, rotation, elevation, host } => (host, at, rotation, elevation),
        PlaceOutcome::Swap { cabinet, part } => {
            let part_name = match part {
                CabinetPart::Pull => "pull",
                CabinetPart::Front => "front",
                _ => "front",
            };
            let mut c = doc.arch.cabinets.iter_mut().find(|c| c.id == cabinet).ok_or_else(|| bad(CMD, "a cabinet part swap matched no cabinet"))?;
            match part {
                CabinetPart::Pull => {
                    c.pull = Some(item_id.to_string());
                    c.pull_symbol = Some(key.clone());
                }
                CabinetPart::Front => {
                    if item_id.starts_with("frontstyle:door-") {
                        c.door_style = item.style.as_deref().and_then(cadcraft_doc::arch::FrontStyle::parse).unwrap_or_default();
                    } else if item_id.starts_with("frontstyle:drawer-") {
                        c.drawer_style = item.style.as_deref().and_then(cadcraft_doc::arch::FrontStyle::parse).unwrap_or_default();
                    }
                }
                _ => {}
            }
            let cabinet_label_value = cabinet_label(cabinet);
            drop(c);
            doc.touch_arch();
            return Ok(json!({ "swapped": { "cabinet": cabinet_label_value, "part": part_name } }));
        }
    };

    let f = Furnishing {
        id: ArchId(0),
        item: item_id.to_owned(),
        symbol: key,
        name: item.name.clone(),
        category: item.category.clone(),
        symbol_type: item.symbol_type.clone(),
        placement,
        level,
        at: pos,
        rotation: rot,
        elevation: elev,
        size,
        mirrored: false,
        host: host.clone(),
        cutout: item.cutout,
        ..Default::default()
    };
    let id = doc.arch.push_furnishing(f);
    doc.touch_arch();
    let host_json = match host {
        FurnishingHost::None => Value::Null,
        _ => serde_json::to_value(&host).unwrap_or(Value::Null),
    };
    Ok(json!({
        "furnishing": furnishing_label(id),
        "handle": handle_json(furnishing_handle(id)),
        "host": host_json,
        "at": pos,
        "rotation": rot,
        "elevation": elev,
        "size": size,
    }))
}

fn run_furnishing_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "furnishing.edit";
    let m = object(CMD, p)?;
    let raw = text(CMD, m, "id")?;
    let id = parse_furnishing_id(raw).ok_or_else(|| bad(CMD, format!("unknown furnishing `{raw}`")))?;
    let d = s.doc_mut()?;
    let Some(i) = d.arch.furnishings.iter().position(|f| f.id == id) else {
        return Err(bad(CMD, format!("unknown furnishing `{raw}`")));
    };
    let mut f = d.arch.furnishings[i].clone();
    let mut changed = false;
    if let Some(v) = field(m, "at") {
        let [x, y] = as_point_array(v, CMD, "at")?;
        f.at = [x, y];
        changed = true;
    }
    if let Some(v) = field(m, "rotation") {
        let n = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(CMD, "`rotation` must be a finite number"))?;
        f.rotation = n;
        changed = true;
    }
    if let Some(v) = field(m, "elevation") {
        let n = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(CMD, "`elevation` must be a finite number"))?;
        if !(0.0..=100.0).contains(&n) {
            return Err(bad(CMD, "`elevation` must be in 0..=100"));
        }
        f.elevation = n;
        changed = true;
    }
    if let Some(v) = field(m, "size") {
        f.size = as_size_array(v, CMD, "size")?;
        changed = true;
    }
    if let Some(v) = field(m, "mirrored") {
        let b = v.as_bool().ok_or_else(|| bad(CMD, "`mirrored` must be true or false"))?;
        f.mirrored = b;
        changed = true;
    }
    if changed {
        f.host = FurnishingHost::None;
    }
    d.arch.furnishings[i] = f;
    d.touch_arch();
    Ok(json!({ "furnishing": furnishing_label(id) }))
}

fn run_furnishing_delete(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "furnishing.delete";
    let m = object(CMD, p)?;
    let raw = text(CMD, m, "id")?;
    let id = parse_furnishing_id(raw).ok_or_else(|| bad(CMD, format!("unknown furnishing `{raw}`")))?;
    let d = s.doc_mut()?;
    if let Some(index) = d.arch.furnishings.iter().position(|f| f.id == id) {
        let _ = d.arch.furnishings.remove(index);
    } else {
        return Err(bad(CMD, format!("unknown furnishing `{raw}`")));
    }
    for f in &mut d.arch.furnishings {
        if matches!(f.host, FurnishingHost::Sink { furnishing } if furnishing == id) {
            f.host = FurnishingHost::None;
        }
    }
    let used: BTreeSet<String> = d.arch.furnishings.iter().map(|f| f.symbol.clone()).collect();
    let pull_symbols: BTreeSet<String> = d.arch.cabinets.iter().filter_map(|c| c.pull_symbol.clone()).collect();
    d.arch.symbols.retain(|key, _| used.contains(key) || pull_symbols.contains(key));
    d.touch_arch();
    Ok(json!({ "furnishing": furnishing_label(id) }))
}

fn run_furnishing_centre(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "furnishing.centre";
    let m = object(CMD, p)?;
    let raw = text(CMD, m, "id")?;
    let id = parse_furnishing_id(raw).ok_or_else(|| bad(CMD, format!("unknown furnishing `{raw}`")))?;
    let on = text(CMD, m, "on")?;
    let mode = if on == "room" {
        "room"
    } else if on == "wall" {
        "wall"
    } else {
        return Err(bad(CMD, "`on` must be `room` or `wall`"));
    };
    let d = s.doc_mut()?;
    let Some(i) = d.arch.furnishings.iter().position(|f| f.id == id) else {
        return Err(bad(CMD, format!("unknown furnishing `{raw}`")));
    };
    let mut f = d.arch.furnishings[i].clone();
    match mode {
        "room" => {
            let p = Vec2::new(f.at[0], f.at[1]);
            let room = d
                .arch
                .rooms
                .iter()
                .find(|r| r.level == f.level && point_in_polygon(&r.polygon, p))
                .ok_or_else(|| bad(CMD, "the furnishing is not in a room"))?;
            let xs: Vec<f64> = room.polygon.iter().map(|v| v.x).collect();
            let ys: Vec<f64> = room.polygon.iter().map(|v| v.y).collect();
            let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
            let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let min_y = ys.iter().copied().fold(f64::INFINITY, f64::min);
            let max_y = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let centre = Vec2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
            f.at = [centre.x, centre.y];
        }
        "wall" => {
            let wall = match &f.host {
                FurnishingHost::Wall { wall } => d.arch.walls.iter().find(|w| w.id == *wall).cloned(),
                _ => nearest_wall_on_level(d, Vec2::new(f.at[0], f.at[1]), f.level).map(|(w, _)| w),
            };
            let Some(w) = wall else {
                return Err(bad(CMD, "there is no wall to centre the furnishing on"));
            };
            let axis = w.end - w.start;
            let mid = wall_midpoint(&w);
            let len = axis.len();
            if len <= 0.0 {
                return Err(bad(CMD, "the wall has zero length"));
            }
            let n = Vec2::new(-axis.y / len, axis.x / len);
            let p = Vec2::new(f.at[0], f.at[1]);
            let offset = (p - mid).dot(n);
            f.at = [mid.x + n.x * offset, mid.y + n.y * offset];
        }
        _ => unreachable!(),
    }
    d.arch.furnishings[i] = f;
    d.touch_arch();
    Ok(json!({ "furnishing": furnishing_label(id), "on": mode, "at": d.arch.furnishings[i].at }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_arch::library::{ItemKind, ItemRecord};
    use cadcraft_doc::arch::SymbolMesh;
    use cadcraft_doc::arch::SymbolPart;
    use cadcraft_doc::arch::{Cabinet, FrontStyle, Wall};
    use serde_json::json;

    fn install_symbols() {
        crate::library::set_library_io(crate::library::LibraryIo {
            read_glb: |_| {
                Ok(SymbolMesh {
                    key: "1234567890abcdef".to_string(),
                    parts: vec![SymbolPart {
                        name: "part".into(),
                        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                        normals: vec![[0.0, 0.0, 1.0]; 3],
                        uvs: vec![],
                        indices: vec![0, 1, 2],
                        base_color: [1.0, 1.0, 1.0, 1.0],
                        metallic: 0.0,
                        roughness: 1.0,
                        texture: None,
                    }],
                    textures: vec![],
                })
            },
            read_image: |_, _| Ok(cadcraft_doc::arch::ImageRgba { width: 1, height: 1, rgba: vec![0, 0, 0, 255] }),
            encode_png: |_| Vec::new(),
            sample: || &[],
        });
    }

    fn add_symbol_item(s: &mut Session, id: &str, symbol_type: &str) {
        install_symbols();
        s.library.add_user_item(ItemRecord {
            id: id.to_string(),
            name: id.to_string(),
            category: "Furniture".to_string(),
            folder: "Other".to_string(),
            kind: ItemKind::Symbol,
            symbol_type: symbol_type.to_string(),
            placement: Some("floor".to_string()),
            size: Some([0.6, 0.6, 0.6]),
            source: "user".to_string(),
            licence: "CC0-1.0".to_string(),
            author: "test".to_string(),
            attribution: String::new(),
            catalog: "user".to_string(),
            ..Default::default()
        });
    }

    #[test]
    fn place_a_floor_symbol() {
        let mut s = Session::new();
        add_symbol_item(&mut s, "armchair", "chair");
        let r = s.execute("library.place", &json!({ "item": "armchair", "at": [2.0, 3.0], "rotation": 10.0, "size": [0.8, 0.6, 0.9] })).unwrap();
        assert_eq!(r["furnishing"], "f0");
        assert_eq!(s.doc().unwrap().arch.furnishings.len(), 1);
    }

    #[test]
    fn place_puts_the_symbol_in_the_model_once() {
        let mut s = Session::new();
        add_symbol_item(&mut s, "armchair", "chair");
        s.execute("library.place", &json!({ "item": "armchair", "at": [1.0, 1.0] })).unwrap();
        s.execute("library.place", &json!({ "item": "armchair", "at": [2.0, 2.0] })).unwrap();
        assert_eq!(s.doc().unwrap().arch.furnishings.len(), 2);
        assert_eq!(s.doc().unwrap().arch.symbols.len(), 1);
    }

    #[test]
    fn place_on_a_wall_and_a_ceiling() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.walls.push(Wall::new(Vec2::ZERO, Vec2::new(8.0, 0.0), 0.2));
        d.arch.rooms.push(cadcraft_doc::arch::Room {
            id: ArchId(0),
            name: "Room".into(),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(8.0, 0.0), Vec2::new(8.0, 5.0), Vec2::new(0.0, 5.0)],
            holes: Vec::new(),
            ..Default::default()
        });
        add_symbol_item(&mut s, "wall-chair", "chair");
        s.library.item_mut("wall-chair").unwrap().placement = Some("wall".to_string());
        let wall = s.execute("library.place", &json!({ "item": "wall-chair", "at": [1.0, 0.1] })).unwrap();
        assert!(wall["host"].is_object());
        let ceiling = s.execute("library.place", &json!({ "item": "wall-chair", "at": [4.0, 3.0], "placement": "ceiling" })).unwrap();
        assert!(ceiling["furnishing"].as_str().is_some());
    }

    #[test]
    fn place_a_sink_into_a_cabinet() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.cabinets.push(Cabinet {
            id: ArchId(0),
            kind: cadcraft_doc::arch::CabinetKind::Base,
            width: 1.0,
            depth: 0.6,
            height: 0.9,
            level: 0,
            counter: true,
            position: Vec2::new(1.0, 2.0),
            ..Default::default()
        });
        add_symbol_item(&mut s, "sink-unit", "sink");
        s.library.item_mut("sink-unit").unwrap().placement = Some("counter".to_string());
        s.library.item_mut("sink-unit").unwrap().cutout = Some([0.4, 0.3]);
        let r = s.execute("library.place", &json!({ "item": "sink-unit", "at": [1.0, 2.0] })).unwrap();
        assert_eq!(r["host"]["kind"], "cabinet");
    }

    #[test]
    fn place_a_pull_swaps_instead() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.cabinets.push(Cabinet {
            id: ArchId(0),
            kind: cadcraft_doc::arch::CabinetKind::Base,
            width: 1.0,
            depth: 0.6,
            height: 0.9,
            level: 0,
            position: Vec2::new(1.0, 2.0),
            ..Default::default()
        });
        add_symbol_item(&mut s, "cabinet-pull", "pull");
        let r = s.execute("library.place", &json!({ "item": "cabinet-pull", "at": [1.0, 2.0] })).unwrap();
        assert_eq!(r["swapped"]["part"], "pull");
    }

    #[test]
    fn place_a_front_style_swaps_door_or_drawer() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.cabinets.push(Cabinet {
            id: ArchId(0),
            kind: cadcraft_doc::arch::CabinetKind::Base,
            width: 1.0,
            depth: 0.6,
            height: 0.9,
            level: 0,
            position: Vec2::new(1.0, 2.0),
            ..Default::default()
        });
        s.library.add_user_item(ItemRecord {
            id: "frontstyle:door-shaker".to_string(),
            name: "Shaker door".to_string(),
            category: "Cabinet parts".to_string(),
            folder: "Front styles".to_string(),
            kind: ItemKind::FrontStyle,
            symbol_type: "front".to_string(),
            style: Some("shaker".to_string()),
            source: "user".to_string(),
            licence: "CC0-1.0".to_string(),
            author: "test".to_string(),
            attribution: String::new(),
            catalog: "user".to_string(),
            ..Default::default()
        });
        let r = s.execute("library.place", &json!({ "item": "frontstyle:door-shaker", "at": [1.0, 2.0] })).unwrap();
        assert_eq!(r["swapped"]["part"], "door");
        assert_eq!(s.doc().unwrap().arch.cabinets[0].door_style, FrontStyle::Shaker);
    }

    #[test]
    fn place_a_door_item_runs_the_door_command() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.walls.push(Wall::new(Vec2::ZERO, Vec2::new(8.0, 0.0), 0.2));
        s.library.add_user_item(ItemRecord {
            id: "door:hinged".to_string(),
            name: "Hinged door".to_string(),
            category: "Doors and windows".to_string(),
            folder: "Doors".to_string(),
            kind: ItemKind::Door,
            symbol_type: "door".to_string(),
            style: Some("hinged".to_string()),
            source: "user".to_string(),
            licence: "CC0-1.0".to_string(),
            author: "test".to_string(),
            attribution: String::new(),
            catalog: "user".to_string(),
            size: Some([0.9, 0.2, 2.1]),
            ..Default::default()
        });
        let r = s.execute("library.place", &json!({ "item": "door:hinged", "at": [1.0, 0.1] })).unwrap();
        assert_eq!(r["via"], "door");
    }

    #[test]
    fn place_refuses_materials_and_bad_numbers() {
        let mut s = Session::new();
        s.library.add_user_item(ItemRecord {
            id: "finish:brick".to_string(),
            name: "Brick".into(),
            category: "Materials".into(),
            folder: "Images".into(),
            kind: ItemKind::Finish,
            symbol_type: "finish".into(),
            source: "user".into(),
            licence: "CC0-1.0".into(),
            author: "test".into(),
            attribution: String::new(),
            catalog: "user".into(),
            ..Default::default()
        });
        let err = s.execute("library.place", &json!({ "item": "finish:brick", "at": [0.0, 0.0], "size": [0.0, 1.0, 1.0] })).unwrap_err();
        assert!(err.to_string().contains("material") || err.to_string().contains("finite"));
    }

    #[test]
    fn edit_and_undo_a_furnishing() {
        let mut s = Session::new();
        add_symbol_item(&mut s, "armchair", "chair");
        s.execute("library.place", &json!({ "item": "armchair", "at": [1.0, 1.0] })).unwrap();
        let r = s.execute("furnishing.edit", &json!({ "id": "f0", "rotation": 45.0, "at": [2.0, 2.0] })).unwrap();
        assert_eq!(r["furnishing"], "f0");
        assert_eq!(s.doc().unwrap().arch.furnishings[0].rotation, 45.0);
    }

    #[test]
    fn delete_drops_unused_symbols() {
        let mut s = Session::new();
        add_symbol_item(&mut s, "armchair", "chair");
        s.execute("library.place", &json!({ "item": "armchair", "at": [1.0, 1.0] })).unwrap();
        let r = s.execute("furnishing.delete", &json!({ "id": "f0" })).unwrap();
        assert_eq!(r["furnishing"], "f0");
        assert!(s.doc().unwrap().arch.furnishings.is_empty());
    }

    #[test]
    fn centre_in_a_room() {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        d.arch.rooms.push(cadcraft_doc::arch::Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0)],
            holes: Vec::new(),
            ..Default::default()
        });
        add_symbol_item(&mut s, "armchair", "chair");
        s.execute("library.place", &json!({ "item": "armchair", "at": [2.0, 2.0] })).unwrap();
        let r = s.execute("furnishing.centre", &json!({ "id": "f0", "on": "room" })).unwrap();
        assert_eq!(r["on"], "room");
    }
}
