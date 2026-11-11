//! Library icons (phase 14): deterministic PNG thumbnails for item and category icons.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use cadcraft_arch::library::{Category, ItemKind, ItemRecord};
use cadcraft_arch3d::icon::{IconView, render_sphere, render_symbol, solid, thumbnail};
use cadcraft_doc::arch::{Finish, ImageRgba, fnv1a64_hex};
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, always, bad};
use crate::library::Library;
use crate::{EngineError, Result, Session};

static ICON_CACHE: OnceLock<Mutex<BTreeMap<String, Vec<u8>>>> = OnceLock::new();

fn icon_cache() -> &'static Mutex<BTreeMap<String, Vec<u8>>> {
    ICON_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn icon_hash_for(lib: &Library, item: &ItemRecord, view: &IconView) -> String {
    let mut bytes = lib.file_bytes(&item.id).unwrap_or_else(|_| item.id.as_bytes().to_vec());
    let extra = format!("|{}|{}|{}|{}|v1", view.size, view.yaw, view.pitch, view.zoom);
    bytes.extend_from_slice(extra.as_bytes());
    fnv1a64_hex(&bytes)
}

fn render_item_icon(lib: &mut Library, item: &ItemRecord, view: &IconView) -> Result<ImageRgba> {
    match item.kind {
        ItemKind::Symbol => {
            let mesh = lib.symbol(&item.id).map_err(EngineError::Other)?;
            Ok(render_symbol(&mesh, view))
        }
        ItemKind::Material => {
            let image = lib.image(&item.id).map_err(EngineError::Other)?;
            Ok(render_sphere(Some(&image), [1.0, 1.0, 1.0], view))
        }
        ItemKind::Backdrop => {
            if item.file.is_some() {
                let image = lib.image(&item.id).map_err(EngineError::Other)?;
                Ok(thumbnail(&image, view.size))
            } else if item.id == "backdrop:colour" {
                Ok(solid([128, 160, 200], view.size))
            } else if item.id == "backdrop:gradient" {
                Ok(cadcraft_arch3d::icon::gradient([74, 128, 196], [214, 228, 240], view.size))
            } else {
                let image = lib.image(&item.id).map_err(EngineError::Other)?;
                Ok(thumbnail(&image, view.size))
            }
        }
        ItemKind::Finish => {
            let name = item.id.strip_prefix("finish:").or_else(|| item.symbol_type.strip_prefix("finish:")).unwrap_or(item.symbol_type.as_str());
            let finish = Finish::parse(name).ok_or_else(|| EngineError::Other(format!("`{}` is not a finish", item.id)))?;
            let texture = cadcraft_arch3d::texture::texture(finish);
            let img = ImageRgba { width: texture.width, height: texture.height, rgba: texture.rgba, ..Default::default() };
            Ok(render_sphere(Some(&img), [1.0, 1.0, 1.0], view))
        }
        ItemKind::Molding => {
            let profile = item.profile.as_deref().ok_or_else(|| EngineError::Other(format!("`{}` has no profile", item.id)))?;
            let path = [cadcraft_geom::Vec2::new(0.0, 0.0), cadcraft_geom::Vec2::new(0.3, 0.0)];
            let mesh = cadcraft_arch3d::sweep::sweep(profile, &path, &[0.0, 0.0], false, 1.0, "molding", cadcraft_doc::arch::ArchId(0))
                .ok_or_else(|| EngineError::Other(format!("`{}` has an unusable profile", item.id)))?;
            let part = cadcraft_doc::arch::SymbolPart {
                name: "molding".to_string(),
                positions: mesh.positions,
                normals: mesh.normals,
                indices: mesh.indices,
                base_color: [0.8, 0.78, 0.72, 1.0],
                metallic: 0.0,
                roughness: 0.6,
                ..Default::default()
            };
            Ok(render_symbol(&cadcraft_doc::arch::SymbolMesh { parts: vec![part], ..Default::default() }, view))
        }
        _ => {
            let rgb = colour_for(item);
            Ok(render_sphere(None, [f32::from(rgb[0]) / 255.0, f32::from(rgb[1]) / 255.0, f32::from(rgb[2]) / 255.0], view))
        }
    }
}

fn colour_for(item: &ItemRecord) -> [u8; 3] {
    let hash = fnv1a64_hex(item.id.as_bytes());
    let mut rgb = [0u8; 3];
    for (slot, chunk) in rgb.iter_mut().zip([0, 2, 4].into_iter()) {
        let hi = &hash[chunk..chunk + 2];
        if let Ok(v) = u8::from_str_radix(hi, 16) {
            *slot = v;
        }
    }
    rgb
}

fn item_for_category<'a>(lib: &'a Library, category: &str) -> Result<&'a ItemRecord> {
    let mut items: Vec<&ItemRecord> = lib.items().iter().filter(|r| r.category == category).collect();
    if items.is_empty() {
        if category == Category::User.name() {
            return Err(EngineError::Other("the User catalog has no items yet".to_string()));
        }
        return Err(EngineError::Other(format!("no items in category `{category}`")));
    }
    items.sort_by(|a, b| (&a.folder, &a.name, &a.id).cmp(&(&b.folder, &b.name, &b.id)));
    Ok(items[0])
}

fn empty_category_icon(category: &str, view: &IconView) -> ImageRgba {
    if category == Category::User.name() { solid([200, 200, 200], view.size) } else { solid([180, 180, 180], view.size) }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_disk_cache(dir: &PathBuf, hash: &str) -> Option<Vec<u8>> {
    std::fs::read(dir.join("icons").join(format!("{hash}.png"))).ok()
}

#[cfg(target_arch = "wasm32")]
fn read_disk_cache(_dir: &PathBuf, _hash: &str) -> Option<Vec<u8>> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn write_disk_cache(dir: &PathBuf, hash: &str, png: &[u8]) -> bool {
    let icons = dir.join("icons");
    if std::fs::create_dir_all(&icons).is_err() {
        return false;
    }
    std::fs::write(icons.join(format!("{hash}.png")), png).is_ok()
}

#[cfg(target_arch = "wasm32")]
fn write_disk_cache(_dir: &PathBuf, _hash: &str, _png: &[u8]) -> bool {
    false
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("library.icon", "Library Icon", run_library_icon)
            .menu(&["Architecture", "Library Icon"])
            .enabled(always)
            .noundo()
            .params("{item?: id | category?: \"Furniture\", cache?: true, size?: 128, yaw?: -30, pitch?: 25, zoom?: 1}"),
    ]
}

fn run_library_icon(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "library.icon";
    let Some(obj) = p.as_object() else {
        return Err(bad(CMD, "the parameters must be an object"));
    };

    s.library.sync_io();

    let cache_flag = obj.get("cache").and_then(Value::as_bool).unwrap_or(true);
    if obj.get("item").is_some() && obj.get("category").is_some() {
        return Err(bad(CMD, "give only one of `item` or `category`"));
    }
    let item_id = obj.get("item").and_then(Value::as_str);
    let category_id = obj.get("category").and_then(Value::as_str);
    let size = obj.get("size").and_then(Value::as_u64).unwrap_or(128) as u32;
    let yaw = obj.get("yaw").and_then(Value::as_f64).unwrap_or(-30.0);
    let pitch = obj.get("pitch").and_then(Value::as_f64).unwrap_or(25.0);
    let zoom = obj.get("zoom").and_then(Value::as_f64).unwrap_or(1.0);
    let view = IconView::new(size, yaw, pitch, zoom).map_err(|e| bad(CMD, e))?;

    let source = match (item_id, category_id) {
        (Some(id), None) => s.library.item(id).ok_or_else(|| bad(CMD, format!("no library item `{id}`")))?.clone(),
        // One of the nine categories, or refused: only a real category with no items gets the grey icon.
        (None, Some(name)) if Category::parse(name).is_none() => {
            let names: Vec<&str> = Category::ALL.iter().map(|c| c.name()).collect();
            return Err(bad(CMD, format!("unknown category `{name}`: one of {}", names.join(", "))));
        }
        (None, Some(name)) => match item_for_category(&s.library, name) {
            Ok(item) => item.clone(),
            Err(_) => {
                let grey = empty_category_icon(name, &view);
                let raw = match crate::library::library_io() {
                    Some(reader) => (reader.encode_png)(&grey),
                    None => grey.rgba.clone(),
                };
                let hash = fnv1a64_hex(format!("category:{}|{}|{}|{}|v1", name, view.size, view.yaw, view.pitch).as_bytes());
                return Ok(json!({ "item": name, "hash": hash, "png": crate::cmd::file::base64_encode(&raw), "cached": false, "size": view.size }));
            }
        },
        _ => return Err(bad(CMD, "give one of `item` or `category`")),
    };

    let hash = icon_hash_for(&s.library, &source, &view);
    let png = if cache_flag {
        let key = hash.clone();
        if let Ok(guard) = icon_cache().lock() {
            if let Some(existing) = guard.get(&key) {
                return Ok(
                    json!({ "item": source.id, "hash": key, "png": crate::cmd::file::base64_encode(existing), "cached": true, "size": view.size }),
                );
            }
        }
        let rendered = render_item_icon(&mut s.library, &source, &view).map_err(|e| bad(CMD, e.to_string()))?;
        let raw = match crate::library::library_io() {
            Some(reader) => (reader.encode_png)(&rendered),
            None => rendered.rgba.clone(),
        };
        if let Some(dir) = s.library.user_dir() {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = write_disk_cache(&dir, &hash, &raw.as_slice());
            }
        }
        if let Ok(mut guard) = icon_cache().lock() {
            guard.insert(hash.clone(), raw.clone());
        }
        raw
    } else {
        let rendered = render_item_icon(&mut s.library, &source, &view).map_err(|e| bad(CMD, e.to_string()))?;
        match crate::library::library_io() {
            Some(reader) => (reader.encode_png)(&rendered),
            None => rendered.rgba.clone(),
        }
    };

    Ok(json!({ "item": source.id, "hash": hash, "png": crate::cmd::file::base64_encode(&png), "cached": false, "size": view.size }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;

    fn png_dimensions(png: &[u8]) -> Option<(u32, u32)> {
        if png.len() < 24 || &png[..8] != [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] {
            return None;
        }
        let mut pos = 8usize;
        while pos + 12 <= png.len() {
            let len = u32::from_be_bytes([png[pos], png[pos + 1], png[pos + 2], png[pos + 3]]) as usize;
            let kind = &png[pos + 4..pos + 8];
            let data = &png[pos + 8..pos + 8 + len];
            if kind == b"IHDR" && data.len() >= 8 {
                let w = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                let h = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                return Some((w, h));
            }
            pos += 12 + len;
        }
        None
    }

    #[test]
    fn icons_are_png_of_the_view_size() {
        crate::library::testing::install_io();
        let mut s = crate::Session::new();
        let result = run_library_icon(&mut s, &json!({"item": "door:hinged", "size": 128, "cache": false})).unwrap();
        let png = crate::cmd::file::base64_decode(result["png"].as_str().unwrap()).unwrap();
        let (w, h) = png_dimensions(&png).unwrap();
        assert_eq!((w, h), (128, 128));
    }

    #[test]
    fn two_renders_are_byte_identical() {
        crate::library::testing::install_io();
        let mut s = crate::Session::new();
        let a = run_library_icon(&mut s, &json!({"item": "door:hinged", "cache": false})).unwrap();
        let b = run_library_icon(&mut s, &json!({"item": "door:hinged", "cache": false})).unwrap();
        assert_eq!(a["png"], b["png"]);
    }

    #[test]
    fn cache_returns_the_same_bytes() {
        crate::library::testing::install_io();
        let mut s = crate::Session::new();
        let a = run_library_icon(&mut s, &json!({"item": "finish:brick", "cache": true})).unwrap();
        let b = run_library_icon(&mut s, &json!({"item": "finish:brick", "cache": true})).unwrap();
        assert_eq!(a["png"], b["png"]);
    }

    #[test]
    fn every_kind_has_an_icon() {
        crate::library::testing::install_io();
        let mut s = crate::Session::new();
        let ids = [
            s.library.items().iter().find(|r| r.kind == ItemKind::Door).unwrap().id.clone(),
            s.library.items().iter().find(|r| r.kind == ItemKind::Window).unwrap().id.clone(),
            s.library.items().iter().find(|r| r.kind == ItemKind::Plant).unwrap().id.clone(),
            s.library.items().iter().find(|r| r.kind == ItemKind::LightFixture).unwrap().id.clone(),
            s.library.items().iter().find(|r| r.kind == ItemKind::Finish).unwrap().id.clone(),
            s.library.items().iter().find(|r| r.kind == ItemKind::Cabinet).unwrap().id.clone(),
            s.library.items().iter().find(|r| r.kind == ItemKind::Fixture).unwrap().id.clone(),
            s.library.items().iter().find(|r| r.kind == ItemKind::FrontStyle).unwrap().id.clone(),
            s.library.items().iter().find(|r| r.kind == ItemKind::Backdrop).unwrap().id.clone(),
        ];
        for item in ids {
            let r = run_library_icon(&mut s, &json!({"item": item, "cache": false})).unwrap();
            assert!(r["png"].as_str().is_some_and(|s| !s.is_empty()));
        }
    }

    #[test]
    fn category_icons_use_their_first_item() {
        crate::library::testing::install_io();
        let mut s = crate::Session::new();
        let category = Category::Furniture.name();
        let item_id = item_for_category(&s.library, category).unwrap().id.clone();
        let response = run_library_icon(&mut s, &json!({"category": category, "cache": false})).unwrap();
        assert_eq!(response["item"], item_id);
    }

    #[test]
    fn an_unknown_category_is_refused() {
        // 11-04: phase-14 check 11 sent `library.icon {"category": "Spaceships"}` and got a grey PNG back.
        crate::library::testing::install_io();
        let mut s = crate::Session::new();
        let err = run_library_icon(&mut s, &json!({"category": "Spaceships", "cache": false})).unwrap_err();
        assert!(err.to_string().contains("Spaceships"), "{err}");
    }

    #[test]
    fn every_real_category_gets_an_icon() {
        crate::library::testing::install_io();
        let mut s = crate::Session::new();
        for c in Category::ALL {
            let r = run_library_icon(&mut s, &json!({"category": c.name(), "cache": false})).unwrap_or_else(|e| panic!("{}: {e}", c.name()));
            assert!(r["png"].as_str().is_some_and(|p| !p.is_empty()), "{}", c.name());
        }
    }

    #[test]
    fn bad_views_are_refused() {
        crate::library::testing::install_io();
        let mut s = crate::Session::new();
        let err = run_library_icon(&mut s, &json!({"item": "door:hinged", "size": 0, "cache": false})).unwrap_err();
        assert!(err.to_string().contains("size") || err.to_string().contains("range"));
        let err2 = run_library_icon(&mut s, &json!({"item": "door:hinged", "pitch": 200.0, "cache": false})).unwrap_err();
        assert!(err2.to_string().contains("pitch") || err2.to_string().contains("range"));
    }

    #[test]
    fn unknown_items_are_refused() {
        crate::library::testing::install_io();
        let mut s = crate::Session::new();
        let err = run_library_icon(&mut s, &json!({"item": "no-such-item", "cache": false})).unwrap_err();
        assert!(err.to_string().contains("no library item") || err.to_string().contains("no such"));
    }

    #[test]
    fn molding_icon_is_deterministic_and_not_blank() {
        crate::library::testing::install_io();
        let mut s = crate::Session::new();
        let a = run_library_icon(&mut s, &json!({"item": "molding:crown", "cache": false})).unwrap();
        let b = run_library_icon(&mut s, &json!({"item": "molding:crown", "cache": false})).unwrap();
        assert_eq!(a["png"], b["png"]);
        assert!(a["png"].as_str().is_some_and(|p| !p.is_empty()));
        let casing = run_library_icon(&mut s, &json!({"item": "molding:casing", "cache": false})).unwrap();
        assert_ne!(a["png"], casing["png"]);
        // Not blank: a profile sweep is drawn, so it differs from the flat grey of an empty category.
        let flat = run_library_icon(&mut s, &json!({"category": "User catalog", "cache": false})).unwrap();
        assert_ne!(a["png"], flat["png"]);
    }
}
