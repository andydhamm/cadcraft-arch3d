//! `backdrop` command: the background behind the 3D view (colour, gradient, library image).

use std::path::Path;

use cadcraft_arch::hdr::{display_image, read_hdr};
use cadcraft_arch::library::ItemKind;
use cadcraft_doc::arch::{Backdrop, MAX_HDR_BYTES, MAX_HDR_IMAGES, Projection};
use serde_json::{Map, Value, json};

use crate::{
    EngineError, Session,
    cmd::{CommandSpec, bad},
};

const KEYS: &[&str] = &["kind", "colour", "top", "bottom", "item", "file", "projection", "intensity", "rotation", "horizon"];

/// `backdrop` command spec.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("backdrop", "Backdrop", run_backdrop)
            .menu(&["Architecture", "Backdrop"])
            .params("{kind?: colour|gradient|image|sky|none, colour?, top?, bottom?, item?, projection?: flat|equirect}"),
    ]
}

/// `#rrggbb` text for a colour.
fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Normalise a hex colour string: lowercase, and if the input had spaces,
/// format as `#rrgg bb`; otherwise `#rrggbb`.
fn normalise_hex(s: &str) -> String {
    let stripped: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let lower = stripped.to_lowercase();
    if s.contains(' ') { format!("#{} {}", &lower[1..5], &lower[5..7]) } else { lower }
}

/// Parse `#` + 6 hex digits, with optional spaces between the pairs.
fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    let digits: String = h.chars().filter(|c| !c.is_whitespace()).collect();
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(&digits, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

fn colour_param(obj: &Map<String, Value>, key: &str) -> Result<[u8; 3], EngineError> {
    let v = obj.get(key).filter(|v| !v.is_null()).ok_or_else(|| bad("backdrop", format!("`{key}` is required")))?;
    v.as_str().and_then(parse_hex).ok_or_else(|| bad("backdrop", format!("`{key}` must be \"#rrggbb\"")))
}

pub(crate) fn describe(b: &Backdrop) -> Value {
    match b {
        Backdrop::Colour { colour } => json!({"kind": "colour", "colour": hex(*colour)}),
        Backdrop::Gradient { top, bottom } => json!({"kind": "gradient", "top": hex(*top), "bottom": hex(*bottom)}),
        Backdrop::Image { item, projection, horizon } => {
            let mut v = json!({"kind": "image", "item": item, "projection": serde_json::to_value(projection).unwrap_or(Value::Null)});
            if *horizon != 0.0 {
                v["horizon"] = json!(horizon);
            }
            v
        }
        Backdrop::Sky => json!({"kind": "sky"}),
        Backdrop::Hdri { image, intensity, rotation, horizon } => {
            json!({"kind": "hdri", "image": image, "intensity": intensity, "rotation": rotation, "horizon": horizon})
        }
    }
}

fn horizon_param(obj: &Map<String, Value>, key: &str) -> Result<Option<f64>, EngineError> {
    let Some(v) = obj.get(key) else {
        return Ok(None);
    };
    let Some(n) = v.as_f64().filter(|x| x.is_finite()) else {
        return Err(bad("backdrop", format!("`{key}` must be a finite number of degrees")));
    };
    if !(-45.0..=45.0).contains(&n) {
        return Err(bad("backdrop", format!("`{key}` must be between -45 and 45 degrees; got {n}")));
    }
    Ok(Some(n))
}

fn intensity_param(obj: &Map<String, Value>) -> Result<Option<f64>, EngineError> {
    let Some(v) = obj.get("intensity") else {
        return Ok(None);
    };
    let Some(n) = v.as_f64().filter(|x| x.is_finite()) else {
        return Err(bad("backdrop", "`intensity` must be a finite number"));
    };
    if !(0.0..=100.0).contains(&n) {
        return Err(bad("backdrop", format!("`intensity` must be between 0 and 100; got {n}")));
    }
    Ok(Some(n))
}

fn rotation_param(obj: &Map<String, Value>) -> Result<Option<f64>, EngineError> {
    let Some(v) = obj.get("rotation") else {
        return Ok(None);
    };
    let Some(n) = v.as_f64().filter(|x| x.is_finite()) else {
        return Err(bad("backdrop", "`rotation` must be a finite number of degrees"));
    };
    Ok(Some(n.rem_euclid(360.0)))
}

fn hdr_item_from_library(s: &mut Session, item: &str) -> Result<(String, cadcraft_doc::arch::ImageRgbe), EngineError> {
    let record = s.library.item(item).ok_or_else(|| bad("backdrop", format!("no library item `{item}`")))?;
    if record.kind != ItemKind::Backdrop || record.file.as_deref().is_none_or(|f| !f.to_ascii_lowercase().ends_with(".hdr")) {
        return Err(bad("backdrop", format!("`{item}` is not an HDR image: use {{\"kind\": \"hdri\"}}")));
    }
    let _image = s.library.image(item).map_err(|e| bad("backdrop", e))?;
    let hdr = read_hdr(&s.library.file_bytes(item).map_err(|e| bad("backdrop", e))?).map_err(|e| bad("backdrop", format!("`item` `{item}`: {e}")))?;
    Ok((item.to_string(), hdr))
}

/// Report or set the backdrop. Anything invalid is refused, never clamped.
fn run_backdrop(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let empty = Map::new();
    let obj = match params {
        Value::Object(o) => o,
        Value::Null => &empty,
        _ => return Err(bad("backdrop", "params must be an object")),
    };
    if let Some(k) = obj.keys().find(|k| !KEYS.contains(&k.as_str())) {
        return Err(bad("backdrop", format!("unknown key `{k}`")));
    }
    let kind = match obj.get("kind") {
        None | Some(Value::Null) => {
            let cur = s.doc()?.arch.backdrop.as_ref().map(describe);
            return Ok(json!({ "backdrop": cur }));
        }
        Some(Value::String(k)) => k.as_str(),
        Some(_) => return Err(bad("backdrop", "`kind` must be a string")),
    };
    let allowed: &[&str] = match kind {
        "colour" => &["kind", "colour"],
        "gradient" => &["kind", "top", "bottom"],
        "image" => &["kind", "item", "projection", "horizon"],
        "hdri" => &["kind", "file", "item", "intensity", "rotation", "horizon"],
        "sky" | "none" => &["kind"],
        _ => return Err(bad("backdrop", "`kind` must be colour, gradient, image, hdri, sky or none")),
    };
    if let Some(k) = obj.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(bad("backdrop", format!("`{k}` does not apply to a {kind} backdrop")));
    }
    let mut image = None;
    let mut hdr_image = None;
    let colour_str = obj.get("colour").and_then(Value::as_str);
    let top_str = obj.get("top").and_then(Value::as_str);
    let bottom_str = obj.get("bottom").and_then(Value::as_str);
    let new = match kind {
        "colour" => Some(Backdrop::Colour { colour: colour_param(obj, "colour")? }),
        "gradient" => Some(Backdrop::Gradient { top: colour_param(obj, "top")?, bottom: colour_param(obj, "bottom")? }),
        "image" => {
            let item = obj
                .get("item")
                .and_then(Value::as_str)
                .filter(|i| !i.is_empty())
                .ok_or_else(|| bad("backdrop", "`item` (library item id) is required"))?;
            let projection = match obj.get("projection") {
                None | Some(Value::Null) => Projection::Flat,
                Some(Value::String(p)) if p == "flat" => Projection::Flat,
                Some(Value::String(p)) if p == "equirect" => Projection::Equirect,
                Some(_) => return Err(bad("backdrop", "`projection` must be flat or equirect")),
            };
            if projection == Projection::Flat && obj.get("horizon").is_some_and(|v| !v.is_null()) {
                return Err(bad("backdrop", "`horizon` applies only to equirectangular images"));
            }
            let horizon = horizon_param(obj, "horizon")?.unwrap_or(0.0);
            match s.library.item(item) {
                Some(r) if r.kind == ItemKind::Backdrop && r.file.is_some() => {
                    if r.file.as_deref().is_some_and(|f| f.to_ascii_lowercase().ends_with(".hdr")) {
                        return Err(bad("backdrop", format!("`{item}` is an HDR image: use {{\"kind\": \"hdri\"}}")));
                    }
                }
                Some(r) if r.kind == ItemKind::Backdrop => {
                    return Err(bad("backdrop", format!("`{item}` is a kind of backdrop (use `kind`), not an image")));
                }
                Some(_) => return Err(bad("backdrop", format!("`{item}` is not a backdrop"))),
                None => return Err(bad("backdrop", format!("no library item `{item}`"))),
            }
            image = Some((item.to_string(), s.library.image(item).map_err(|e| bad("backdrop", e))?));
            Some(Backdrop::Image { item: item.to_string(), projection, horizon })
        }
        "hdri" => {
            let item = obj.get("item").and_then(Value::as_str).filter(|i| !i.is_empty());
            let file = obj.get("file").and_then(Value::as_str).filter(|f| !f.trim().is_empty());
            let (key, bytes) = match (item, file) {
                (Some(item), None) => {
                    let record = s.library.item(item).ok_or_else(|| bad("backdrop", format!("no library item `{item}`")))?;
                    if record.kind != ItemKind::Backdrop || record.file.as_deref().is_none_or(|f| !f.to_ascii_lowercase().ends_with(".hdr")) {
                        return Err(bad("backdrop", format!("`{item}` is not an HDR backdrop item")));
                    }
                    (item.to_string(), s.library.file_bytes(item).map_err(|e| bad("backdrop", e))?)
                }
                (None, Some(path)) => {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let bytes = std::fs::read(path).map_err(|e| bad("backdrop", format!("`file` `{path}`: {e}")))?;
                        (Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("hdr").to_string(), bytes)
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        return Err(bad("backdrop", "`file` is not available in the web build"));
                    }
                }
                (Some(_), Some(_)) => return Err(bad("backdrop", "give only one of `item` or `file`")),
                (None, None) => return Err(bad("backdrop", "one of `item` or `file` is required")),
            };
            let img = read_hdr(&bytes).map_err(|e| bad("backdrop", format!("`{key}`: {e}")))?;
            if !img.is_valid() {
                return Err(bad("backdrop", format!("`{key}` is not a valid HDR image")));
            }
            let intensity = intensity_param(obj)?.unwrap_or(1.0);
            let rotation = rotation_param(obj)?.unwrap_or(0.0);
            let horizon = horizon_param(obj, "horizon")?.unwrap_or(0.0);
            hdr_image = Some((key.clone(), img.clone()));
            image = Some((key.clone(), display_image(&img, 1024)));
            Some(Backdrop::Hdri { image: key, intensity, rotation, horizon })
        }
        "sky" => Some(Backdrop::Sky),
        _ => None,
    };
    s.doc()?;
    let before = s.state()?.doc.clone();
    let d = s.doc_mut()?;
    if let Some((id, img)) = image {
        d.arch.images.insert(id.clone(), img);
    }
    if let Some((id, img)) = hdr_image {
        d.arch.hdr_images.insert(id, img);
    }
    d.arch.backdrop = new;
    d.arch.prune_hdr_images();
    let raw_len: usize = d.arch.hdr_images.values().map(|img| img.rgbe.len()).sum();
    if d.arch.hdr_images.len() > MAX_HDR_IMAGES || raw_len > MAX_HDR_BYTES {
        s.state_mut()?.doc = before;
        return Err(bad("backdrop", format!("a drawing holds at most {MAX_HDR_IMAGES} HDR images ({MAX_HDR_BYTES} bytes)")));
    }
    let reply = match kind {
        "colour" => json!({"kind": "colour", "colour": normalise_hex(colour_str.unwrap_or("#000000"))}),
        "gradient" => {
            json!({"kind": "gradient", "top": normalise_hex(top_str.unwrap_or("#000000")), "bottom": normalise_hex(bottom_str.unwrap_or("#000000"))})
        }
        "image" | "hdri" | "sky" => d.arch.backdrop.as_ref().map_or(Value::Null, describe),
        _ => json!({"kind": "none"}),
    };
    d.touch_arch();
    Ok(json!({ "backdrop": reply }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::testing::install_io;

    fn session() -> Session {
        install_io();
        Session::new()
    }

    #[test]
    fn backdrop_colour_gradient_image_and_none() {
        let mut s = session();
        let r = s.execute("backdrop", &json!({"kind": "colour", "colour": "#C04020"})).unwrap();
        assert_eq!(r["backdrop"], json!({"kind": "colour", "colour": "#c04020"}));
        let r = s.execute("backdrop", &json!({"kind": "gradient", "top": "#000001", "bottom": "#ffffff"})).unwrap();
        assert_eq!(r["backdrop"], json!({"kind": "gradient", "top": "#000001", "bottom": "#ffffff"}));
        let r = s.execute("backdrop", &json!({"kind": "image", "item": "own:backdrop-sky", "projection": "equirect"})).unwrap();
        assert_eq!(r["backdrop"], json!({"kind": "image", "item": "own:backdrop-sky", "projection": "equirect"}));
        let r = s.execute("backdrop", &json!({"kind": "image", "item": "own:backdrop-sky"})).unwrap();
        assert_eq!(r["backdrop"]["projection"], json!("flat"));
        s.execute("backdrop", &json!({"kind": "none"})).unwrap();
        assert!(s.doc().unwrap().arch.backdrop.is_none());
    }

    #[test]
    fn backdrop_reports_the_current_one() {
        let mut s = session();
        assert_eq!(s.execute("backdrop", &json!({})).unwrap()["backdrop"], Value::Null);
        s.execute("backdrop", &json!({"kind": "colour", "colour": "#102030"})).unwrap();
        assert_eq!(s.execute("backdrop", &json!({})).unwrap()["backdrop"], json!({"kind": "colour", "colour": "#102030"}));
    }

    #[test]
    fn backdrop_refuses_bad_colours_and_items() {
        let mut s = session();
        let bads = [
            json!({"kind": "colour"}),
            json!({"kind": "colour", "colour": "c04020"}),
            json!({"kind": "colour", "colour": "#c0402"}),
            json!({"kind": "colour", "colour": "#c04020f"}),
            json!({"kind": "colour", "colour": "#gg0000"}),
            json!({"kind": "colour", "colour": [1, 2, 3]}),
            json!({"kind": "gradient", "top": "#000000"}),
            json!({"kind": "image"}),
            json!({"kind": "image", "item": "nope"}),
            json!({"kind": "image", "item": "backdrop:colour"}),
            json!({"kind": "image", "item": "backdrop:gradient"}),
            json!({"kind": "image", "item": "finish:siding"}),
            json!({"kind": "image", "item": "own:backdrop-sky", "projection": "cube"}),
            json!({"kind": 5}),
            json!({"colour": "#000000", "kind": "none"}),
            json!({"bogus": 1}),
        ];
        for p in &bads {
            assert!(s.execute("backdrop", p).is_err(), "should refuse {p}");
        }
        assert!(s.doc().unwrap().arch.backdrop.is_none());
    }

    #[test]
    fn backdrop_stores_its_image() {
        let mut s = session();
        s.execute("backdrop", &json!({"kind": "image", "item": "own:backdrop-sky"})).unwrap();
        let img = s.doc().unwrap().arch.images.get("own:backdrop-sky").expect("image stored");
        assert!(img.is_valid() && img.width <= 1024 && img.height <= 1024);
    }

    #[test]
    fn backdrop_is_undoable() {
        let mut s = session();
        s.execute("backdrop", &json!({"kind": "colour", "colour": "#102030"})).unwrap();
        assert!(s.doc().unwrap().arch.backdrop.is_some());
        assert_eq!(s.undo().unwrap(), Some("Backdrop".to_string()));
        assert!(s.doc().unwrap().arch.backdrop.is_none());
    }

    #[test]
    fn backdrop_uppercase_hex_normalised_to_lowercase() {
        let mut s = session();
        let r = s.execute("backdrop", &json!({"kind": "colour", "colour": "#FFFFFF"})).unwrap();
        assert_eq!(r["backdrop"]["colour"], json!("#ffffff"));
        let r = s.execute("backdrop", &json!({"kind": "gradient", "top": "#AABBCC", "bottom": "#DD EE FF"})).unwrap();
        assert_eq!(r["backdrop"]["top"], json!("#aabbcc"));
        assert_eq!(r["backdrop"]["bottom"], json!("#ddee ff"));
    }

    #[test]
    fn backdrop_refuses_extra_fields_per_kind() {
        let mut s = session();
        let bads = [
            json!({"kind": "colour", "colour": "#ffffff", "projection": "flat"}),
            json!({"kind": "colour", "colour": "#ffffff", "item": "own:backdrop-sky"}),
            json!({"kind": "colour", "colour": "#ffffff", "top": "#000000"}),
            json!({"kind": "colour", "colour": "#ffffff", "bottom": "#000000"}),
            json!({"kind": "gradient", "top": "#000000", "bottom": "#ffffff", "colour": "#808080"}),
            json!({"kind": "gradient", "top": "#000000", "bottom": "#ffffff", "item": "own:backdrop-sky"}),
            json!({"kind": "gradient", "top": "#000000", "bottom": "#ffffff", "projection": "flat"}),
            json!({"kind": "image", "item": "own:backdrop-sky", "colour": "#ffffff"}),
            json!({"kind": "image", "item": "own:backdrop-sky", "top": "#000000"}),
            json!({"kind": "image", "item": "own:backdrop-sky", "bottom": "#ffffff"}),
            json!({"kind": "none", "colour": "#ffffff"}),
            json!({"kind": "none", "item": "own:backdrop-sky"}),
        ];
        for p in &bads {
            assert!(s.execute("backdrop", p).is_err(), "should refuse {p}");
        }
    }

    fn hdr_bytes() -> Vec<u8> {
        let width = 4u32;
        let height = 2u32;
        let mut out = format!("#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n-Y {height} +X {width}\n\n").into_bytes();
        let pixel = [0x80, 0x80, 0x80, 0x80];
        out.extend(std::iter::repeat(pixel).take((width * height) as usize).flatten());
        out
    }

    #[test]
    fn hdri_from_file_and_item_and_refusals_change_nothing() {
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-hdri-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(dir.join("user")).unwrap();
        let file = dir.join("user/test.hdr");
        std::fs::write(&file, hdr_bytes()).unwrap();
        let mut s = crate::library::testing::session(&dir);
        let rec = cadcraft_arch::library::ItemRecord {
            id: "user:hdr-test".to_string(),
            name: "Test HDR".to_string(),
            category: "Backdrops".to_string(),
            folder: "Hdr".to_string(),
            kind: cadcraft_arch::library::ItemKind::Backdrop,
            symbol_type: "backdrop".to_string(),
            file: Some("test.hdr".to_string()),
            licence: "CC0-1.0".to_string(),
            source: "test".to_string(),
            author: "CADCraft".to_string(),
            attribution: "test".to_string(),
            ..Default::default()
        };
        s.library.add_user_item(rec.clone());
        let before = s.doc().unwrap().arch.backdrop.clone();
        assert!(s.execute("backdrop", &json!({"kind": "hdri", "file": file.to_string_lossy().to_string()})).is_ok());
        assert!(s.execute("backdrop", &json!({"kind": "hdri", "item": "user:hdr-test", "intensity": 2.0})).is_ok());
        let err = s.execute("backdrop", &json!({"kind": "hdri", "item": "own:backdrop-sky"}));
        assert!(err.is_err());
        let after = s.doc().unwrap().arch.backdrop.as_ref().unwrap();
        assert!(matches!(after, Backdrop::Hdri { image, intensity, .. } if image == "user:hdr-test" && *intensity == 2.0));
        let bad = s.execute("backdrop", &json!({"kind": "hdri", "item": "user:hdr-test", "intensity": 101.0}));
        assert!(bad.is_err());
        assert!(matches!(s.doc().unwrap().arch.backdrop.as_ref(), Some(Backdrop::Hdri { image, .. }) if image == "user:hdr-test"));
        assert_eq!(
            Some(Backdrop::Hdri { image: "user:hdr-test".to_string(), intensity: 2.0, rotation: 0.0, horizon: 0.0 }),
            s.doc().unwrap().arch.backdrop.clone()
        );
    }

    #[test]
    fn equirect_image_horizon() {
        let mut s = session();
        let r = s.execute("backdrop", &json!({"kind": "image", "item": "own:backdrop-sky", "projection": "equirect", "horizon": 18.0})).unwrap();
        assert_eq!(r["backdrop"]["horizon"], 18.0);
        assert!(
            matches!(s.doc().unwrap().arch.backdrop.as_ref(), Some(Backdrop::Image { projection: Projection::Equirect, horizon, .. }) if *horizon == 18.0)
        );
    }

    #[test]
    fn hdr_item_refused_as_a_picture_and_caps_refuse() {
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-hdri-caps-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(dir.join("user")).unwrap();
        std::fs::write(dir.join("user/caps.hdr"), hdr_bytes()).unwrap();
        let mut s = crate::library::testing::session(&dir);
        s.library.add_user_item(cadcraft_arch::library::ItemRecord {
            id: "user:hdr-caps".to_string(),
            name: "Caps HDR".to_string(),
            category: "Backdrops".to_string(),
            folder: "Hdr".to_string(),
            kind: cadcraft_arch::library::ItemKind::Backdrop,
            symbol_type: "backdrop".to_string(),
            file: Some("caps.hdr".to_string()),
            licence: "CC0-1.0".to_string(),
            source: "test".to_string(),
            author: "CADCraft".to_string(),
            attribution: "test".to_string(),
            ..Default::default()
        });
        assert!(s.execute("backdrop", &json!({"kind": "image", "item": "user:hdr-caps"})).is_err());
        let mut doc = s.doc_mut().unwrap();
        for i in 0..5 {
            let k = format!("cap-{i}");
            doc.arch.hdr_images.insert(
                k.clone(),
                cadcraft_doc::arch::ImageRgbe { width: 2, height: 1, rgbe: vec![0, 0, 0, 128, 0, 0, 0, 128], ..Default::default() },
            );
            doc.arch.cameras.push(cadcraft_doc::arch::Camera3d {
                name: format!("cam-{i}"),
                look: Some(cadcraft_doc::arch::Look {
                    backdrop: Some(Backdrop::Hdri { image: k.clone(), intensity: 1.0, rotation: 0.0, horizon: 0.0 }),
                    ..Default::default()
                }),
                ..Default::default()
            });
        }
        doc.arch.prune_hdr_images();
        assert!(doc.arch.hdr_images.len() <= MAX_HDR_IMAGES);
        drop(doc);
    }

    #[test]
    fn hdr_backdrop_item_places_as_hdri() {
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-hdri-place-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(dir.join("user")).unwrap();
        std::fs::write(dir.join("user/place.hdr"), hdr_bytes()).unwrap();
        let mut s = crate::library::testing::session(&dir);
        s.library.add_user_item(cadcraft_arch::library::ItemRecord {
            id: "user:hdr-place".to_string(),
            name: "Place HDR".to_string(),
            category: "Backdrops".to_string(),
            folder: "Hdr".to_string(),
            kind: cadcraft_arch::library::ItemKind::Backdrop,
            symbol_type: "backdrop".to_string(),
            file: Some("place.hdr".to_string()),
            licence: "CC0-1.0".to_string(),
            source: "test".to_string(),
            author: "CADCraft".to_string(),
            attribution: "test".to_string(),
            ..Default::default()
        });
        let params = crate::cmd::arch::furnish::backdrop_params(
            s.library.item("user:hdr-place").unwrap(),
            Some(&serde_json::from_str(r#"{"intensity":2.0,"rotation":45.0,"horizon":10.0}"#).unwrap()),
        );
        assert_eq!(params["kind"], "hdri");
        assert_eq!(params["intensity"], 2.0);
        assert_eq!(params["rotation"], 45.0);
        assert_eq!(params["horizon"], 10.0);
    }

    #[test]
    fn hdr_backdrop_entry_and_install() {
        let entry = serde_json::json!({"id":"x:hdr-sample","name":"HDR Sample","category":"Backdrops","folder":"HDR","symbolType":"backdrop","file":"hdr/sample.hdr","source":"test","licence":"CC0-1.0","author":"CADCraft","attribution":"test"});
        let rec = crate::library::catalog::entry_record(&entry, "sample").unwrap();
        assert_eq!(rec.kind, cadcraft_arch::library::ItemKind::Backdrop);
        assert_eq!(rec.file.as_deref(), Some("hdr/sample.hdr"));
    }

    #[test]
    fn hdr_backdrop_image_is_tone_mapped() {
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-hdri-tone-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(dir.join("user")).unwrap();
        std::fs::write(dir.join("user/tone.hdr"), hdr_bytes()).unwrap();
        let mut s = crate::library::testing::session(&dir);
        s.library.add_user_item(cadcraft_arch::library::ItemRecord {
            id: "user:hdr-tone".to_string(),
            name: "Tone HDR".to_string(),
            category: "Backdrops".to_string(),
            folder: "Hdr".to_string(),
            kind: cadcraft_arch::library::ItemKind::Backdrop,
            symbol_type: "backdrop".to_string(),
            file: Some("tone.hdr".to_string()),
            licence: "CC0-1.0".to_string(),
            source: "test".to_string(),
            author: "CADCraft".to_string(),
            attribution: "test".to_string(),
            ..Default::default()
        });
        let img = s.library.image("user:hdr-tone").unwrap();
        assert!(img.is_valid());
        assert!(img.width > 0 && img.height > 0);
    }

    #[test]
    fn backdrop_sky_sets_reports_and_refuses_extra_keys() {
        let mut s = session();
        let r = s.execute("backdrop", &json!({"kind": "sky"})).unwrap();
        assert_eq!(r["backdrop"], json!({"kind": "sky"}));
        assert_eq!(s.doc().unwrap().arch.backdrop, Some(Backdrop::Sky));
        assert_eq!(s.execute("backdrop", &json!({})).unwrap()["backdrop"], json!({"kind": "sky"}));
        let e = s.execute("backdrop", &json!({"kind": "sky", "colour": "#000000"})).unwrap_err().to_string();
        assert!(e.contains("`colour` does not apply to a sky backdrop"), "{e}");
        assert_eq!(s.doc().unwrap().arch.backdrop, Some(Backdrop::Sky));
        let e = s.execute("backdrop", &json!({"kind": "nebula"})).unwrap_err().to_string();
        assert!(e.contains("colour, gradient, image, hdri, sky or none"), "{e}");
    }

    #[test]
    fn backdrop_refuses_empty_item() {
        let mut s = session();
        assert!(s.execute("backdrop", &json!({"kind": "image", "item": ""})).is_err());
        assert!(s.doc().unwrap().arch.backdrop.is_none());
    }

    #[test]
    fn backdrop_null_projection_defaults_to_flat() {
        let mut s = session();
        let r = s.execute("backdrop", &json!({"kind": "image", "item": "own:backdrop-sky", "projection": null})).unwrap();
        assert_eq!(r["backdrop"]["projection"], json!("flat"));
    }

    #[test]
    fn backdrop_multiple_undo_steps() {
        let mut s = session();
        s.execute("backdrop", &json!({"kind": "colour", "colour": "#102030"})).unwrap();
        s.execute("backdrop", &json!({"kind": "gradient", "top": "#000000", "bottom": "#ffffff"})).unwrap();
        s.execute("backdrop", &json!({"kind": "none"})).unwrap();
        assert!(s.doc().unwrap().arch.backdrop.is_none());
        assert_eq!(s.undo().unwrap(), Some("Backdrop".to_string()));
        assert_eq!(s.doc().unwrap().arch.backdrop.as_ref().unwrap(), &Backdrop::Gradient { top: [0, 0, 0], bottom: [255, 255, 255] });
        assert_eq!(s.undo().unwrap(), Some("Backdrop".to_string()));
        assert_eq!(s.doc().unwrap().arch.backdrop.as_ref().unwrap(), &Backdrop::Colour { colour: [0x10, 0x20, 0x30] });
        assert_eq!(s.undo().unwrap(), Some("Backdrop".to_string()));
        assert!(s.doc().unwrap().arch.backdrop.is_none());
    }

    #[test]
    fn hdri_refuses_neither_file_nor_item() {
        let mut s = session();
        let err = s.execute("backdrop", &json!({"kind": "hdri"})).unwrap_err().to_string();
        assert!(err.contains("one of `item` or `file` is required"), "{err}");
        assert!(s.doc().unwrap().arch.backdrop.is_none());
    }

    #[test]
    fn hdri_refuses_both_file_and_item() {
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-hdri-both-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.hdr");
        std::fs::write(&file, hdr_bytes()).unwrap();
        let mut s = crate::library::testing::session(&dir);
        let err = s
            .execute("backdrop", &json!({"kind": "hdri", "file": file.to_string_lossy().to_string(), "item": "own:backdrop-sky"}))
            .unwrap_err()
            .to_string();
        assert!(err.contains("only one of `item` or `file`"), "{err}");
        assert!(s.doc().unwrap().arch.backdrop.is_none());
    }

    #[test]
    fn hdri_refuses_negative_intensity() {
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-hdri-negint-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(dir.join("user")).unwrap();
        std::fs::write(dir.join("user/neg.hdr"), hdr_bytes()).unwrap();
        let mut s = crate::library::testing::session(&dir);
        s.library.add_user_item(cadcraft_arch::library::ItemRecord {
            id: "user:hdr-neg".to_string(),
            name: "Neg HDR".to_string(),
            category: "Backdrops".to_string(),
            folder: "Hdr".to_string(),
            kind: cadcraft_arch::library::ItemKind::Backdrop,
            symbol_type: "backdrop".to_string(),
            file: Some("neg.hdr".to_string()),
            licence: "CC0-1.0".to_string(),
            source: "test".to_string(),
            author: "CADCraft".to_string(),
            attribution: "test".to_string(),
            ..Default::default()
        });
        let err = s.execute("backdrop", &json!({"kind": "hdri", "item": "user:hdr-neg", "intensity": -0.1})).unwrap_err().to_string();
        assert!(err.contains("intensity") && err.contains("0 and 100"), "{err}");
        assert!(s.doc().unwrap().arch.backdrop.is_none());
    }

    #[test]
    fn hdri_rotation_wraps_beyond_360() {
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-hdri-rot-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(dir.join("user")).unwrap();
        std::fs::write(dir.join("user/rot.hdr"), hdr_bytes()).unwrap();
        let mut s = crate::library::testing::session(&dir);
        s.library.add_user_item(cadcraft_arch::library::ItemRecord {
            id: "user:hdr-rot".to_string(),
            name: "Rot HDR".to_string(),
            category: "Backdrops".to_string(),
            folder: "Hdr".to_string(),
            kind: cadcraft_arch::library::ItemKind::Backdrop,
            symbol_type: "backdrop".to_string(),
            file: Some("rot.hdr".to_string()),
            licence: "CC0-1.0".to_string(),
            source: "test".to_string(),
            author: "CADCraft".to_string(),
            attribution: "test".to_string(),
            ..Default::default()
        });
        let r = s.execute("backdrop", &json!({"kind": "hdri", "item": "user:hdr-rot", "rotation": 400.0})).unwrap();
        assert_eq!(r["backdrop"]["rotation"], 40.0);
        let r = s.execute("backdrop", &json!({"kind": "hdri", "item": "user:hdr-rot", "rotation": -90.0})).unwrap();
        assert_eq!(r["backdrop"]["rotation"], 270.0);
    }

    #[test]
    fn hdri_refuses_horizon_out_of_range() {
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-hdri-hor-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(dir.join("user")).unwrap();
        std::fs::write(dir.join("user/hor.hdr"), hdr_bytes()).unwrap();
        let mut s = crate::library::testing::session(&dir);
        s.library.add_user_item(cadcraft_arch::library::ItemRecord {
            id: "user:hdr-hor".to_string(),
            name: "Hor HDR".to_string(),
            category: "Backdrops".to_string(),
            folder: "Hdr".to_string(),
            kind: cadcraft_arch::library::ItemKind::Backdrop,
            symbol_type: "backdrop".to_string(),
            file: Some("hor.hdr".to_string()),
            licence: "CC0-1.0".to_string(),
            source: "test".to_string(),
            author: "CADCraft".to_string(),
            attribution: "test".to_string(),
            ..Default::default()
        });
        let err = s.execute("backdrop", &json!({"kind": "hdri", "item": "user:hdr-hor", "horizon": 46.0})).unwrap_err().to_string();
        assert!(err.contains("horizon") && err.contains("-45 and 45"), "{err}");
        let err = s.execute("backdrop", &json!({"kind": "hdri", "item": "user:hdr-hor", "horizon": -46.0})).unwrap_err().to_string();
        assert!(err.contains("horizon") && err.contains("-45 and 45"), "{err}");
        assert!(s.doc().unwrap().arch.backdrop.is_none());
    }

    #[test]
    fn flat_projection_refuses_horizon() {
        let mut s = session();
        let err = s
            .execute("backdrop", &json!({"kind": "image", "item": "own:backdrop-sky", "projection": "flat", "horizon": 5.0}))
            .unwrap_err()
            .to_string();
        assert!(err.contains("horizon"), "{err}");
        assert!(s.doc().unwrap().arch.backdrop.is_none());
    }

    #[test]
    fn hdri_refuses_nonexistent_file() {
        let mut s = session();
        let err = s.execute("backdrop", &json!({"kind": "hdri", "file": "/nonexistent/path/test.hdr"})).unwrap_err().to_string();
        assert!(err.contains("file") || err.contains("nonexistent"), "{err}");
        assert!(s.doc().unwrap().arch.backdrop.is_none());
    }
}
