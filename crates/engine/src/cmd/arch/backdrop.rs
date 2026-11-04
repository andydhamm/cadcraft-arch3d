//! `backdrop` command: the background behind the 3D view (colour, gradient, library image).

use cadcraft_arch::library::ItemKind;
use cadcraft_doc::arch::{Backdrop, Projection};
use serde_json::{Map, Value, json};

use crate::{
    EngineError, Session,
    cmd::{CommandSpec, bad},
};

const KEYS: &[&str] = &["kind", "colour", "top", "bottom", "item", "projection"];

/// `backdrop` command spec.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("backdrop", "Backdrop", run_backdrop)
            .menu(&["Architecture", "Backdrop"])
            .params("{kind?: colour|gradient|image|none, colour?, top?, bottom?, item?, projection?: flat|equirect}"),
    ]
}

/// `#rrggbb` text for a colour.
fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Parse exactly `#` + 6 hex digits.
fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

fn colour_param(obj: &Map<String, Value>, key: &str) -> Result<[u8; 3], EngineError> {
    let v = obj.get(key).filter(|v| !v.is_null()).ok_or_else(|| bad("backdrop", format!("`{key}` is required")))?;
    v.as_str().and_then(parse_hex).ok_or_else(|| bad("backdrop", format!("`{key}` must be \"#rrggbb\"")))
}

fn describe(b: &Backdrop) -> Value {
    match b {
        Backdrop::Colour { colour } => json!({"kind": "colour", "colour": hex(*colour)}),
        Backdrop::Gradient { top, bottom } => json!({"kind": "gradient", "top": hex(*top), "bottom": hex(*bottom)}),
        Backdrop::Image { item, projection } => {
            json!({"kind": "image", "item": item, "projection": serde_json::to_value(projection).unwrap_or(Value::Null)})
        }
    }
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
        "image" => &["kind", "item", "projection"],
        "none" => &["kind"],
        _ => return Err(bad("backdrop", "`kind` must be colour, gradient, image or none")),
    };
    if let Some(k) = obj.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(bad("backdrop", format!("`{k}` does not apply to a {kind} backdrop")));
    }
    let mut image = None;
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
            match s.library.item(item) {
                Some(r) if r.kind == ItemKind::Backdrop && r.file.is_some() => {}
                Some(r) if r.kind == ItemKind::Backdrop => {
                    return Err(bad("backdrop", format!("`{item}` is a kind of backdrop (use `kind`), not an image")));
                }
                Some(_) => return Err(bad("backdrop", format!("`{item}` is not a backdrop"))),
                None => return Err(bad("backdrop", format!("no library item `{item}`"))),
            }
            image = Some((item.to_string(), s.library.image(item).map_err(|e| bad("backdrop", e))?));
            Some(Backdrop::Image { item: item.to_string(), projection })
        }
        _ => None,
    };
    s.doc()?;
    let d = s.doc_mut()?;
    if let Some((id, img)) = image {
        d.arch.images.insert(id, img);
    }
    d.arch.backdrop = new;
    let reply = d.arch.backdrop.as_ref().map_or_else(|| json!({"kind": "none"}), describe);
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
            json!({"kind": "sky"}),
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
}
