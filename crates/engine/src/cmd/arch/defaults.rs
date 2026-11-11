//! Set as Default: the parameters a command uses when a call leaves them out, kept in the
//! drawing's `defaults` by kind ("cabinet.base", "door", "furnishing.<item id>", ...).

use cadcraft_doc::arch::{ArchId, CabinetKind, Fronts, OpeningKind};
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, bad};
use crate::{Result, Session};

/// The commands of this module.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("arch.default", "Set as Default", run_default)
            .menu(&["Architecture", "Set as Default"])
            .params("{id: \"cb0\"|\"o0\"|\"f0\"}"),
    ]
}

/// `p` with the drawing's defaults for `kind` filled in where `p` leaves them out.
pub(crate) fn merged(s: &Session, kind: &str, p: &Value) -> Value {
    let Ok(doc) = s.doc() else { return p.clone() };
    let Some(Value::Object(def)) = doc.arch.defaults.get(kind) else { return p.clone() };
    let mut out: Map<String, Value> = match p {
        Value::Object(m) => m.clone(),
        Value::Null => Map::new(),
        _ => return p.clone(),
    };
    for (k, v) in def {
        if !out.contains_key(k) {
            out.insert(k.clone(), v.clone());
        }
    }
    Value::Object(out)
}

fn parse_id(raw: &str, prefix: &str) -> Option<ArchId> {
    let t = raw.trim().to_ascii_lowercase();
    t.strip_prefix(prefix)?.parse::<u32>().ok().map(ArchId)
}

fn fronts_name(f: Fronts) -> &'static str {
    match f {
        Fronts::Doors => "doors",
        Fronts::Drawers => "drawers",
        Fronts::DrawerOverDoor => "drawerOverDoor",
        Fronts::Open => "open",
    }
}

const REFUSED: &str = "Set as Default works on cabinets, doors, windows and furnishings";

fn run_default(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "arch.default";
    let raw = p.get("id").and_then(Value::as_str).ok_or_else(|| bad(CMD, "`id` is required"))?;
    let doc = s.doc()?;
    let (kind, def): (String, Value) = if let Some(id) = parse_id(raw, "cb") {
        let c = doc.arch.cabinets.iter().find(|c| c.id == id).ok_or_else(|| bad(CMD, format!("unknown cabinet `{raw}`")))?;
        let k = match c.kind {
            CabinetKind::Base => "base",
            CabinetKind::Wall => "wall",
            CabinetKind::Tall => "tall",
        };
        let mut m = json!({
            "depth": c.depth,
            "height": c.height,
            "elevation": c.elevation,
            "fronts": fronts_name(c.fronts),
            "doors": c.doors,
            "drawers": c.drawers,
            "counter": c.counter,
            "doorStyle": c.door_style.name(),
            "drawerStyle": c.drawer_style.name(),
        });
        if let (Some(pull), Some(o)) = (&c.pull, m.as_object_mut()) {
            o.insert("pull".into(), json!(pull));
        }
        (format!("cabinet.{k}"), m)
    } else if let Some(id) = parse_id(raw, "o") {
        let o = doc.arch.openings.iter().find(|o| o.id == id).ok_or_else(|| bad(CMD, format!("unknown opening `{raw}`")))?;
        match o.kind {
            OpeningKind::Door => ("door".to_string(), json!({ "width": o.width, "height": o.height, "style": o.style })),
            OpeningKind::Window => ("window".to_string(), json!({ "width": o.width, "height": o.height, "style": o.style, "sill": o.sill })),
            _ => return Err(bad(CMD, REFUSED)),
        }
    } else if let Some(id) = parse_id(raw, "f") {
        let f = doc.arch.furnishings.iter().find(|f| f.id == id).ok_or_else(|| bad(CMD, format!("unknown furnishing `{raw}`")))?;
        (format!("furnishing.{}", f.item), json!({ "size": f.size, "elevation": f.elevation }))
    } else {
        return Err(bad(CMD, REFUSED));
    };
    let d = s.doc_mut()?;
    d.arch.defaults.insert(kind.clone(), def.clone());
    d.touch_arch();
    Ok(json!({ "kind": kind, "default": def }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{Cabinet, Furnishing, Opening};

    /// A session whose `execute` takes the parameters by value.
    struct S(Session);

    impl std::ops::Deref for S {
        type Target = Session;
        fn deref(&self) -> &Session {
            &self.0
        }
    }

    impl std::ops::DerefMut for S {
        fn deref_mut(&mut self) -> &mut Session {
            &mut self.0
        }
    }

    impl S {
        fn execute(&mut self, cmd: &str, p: Value) -> Result<Value> {
            self.0.execute(cmd, &p)
        }
    }

    fn session() -> S {
        S(Session::new())
    }

    #[test]
    fn merged_fills_only_missing_keys() {
        let mut s = session();
        if let Ok(d) = s.doc_mut() {
            d.arch.defaults.insert("door".into(), json!({ "width": 1.0, "style": "slider" }));
        }
        let out = merged(&s, "door", &json!({ "width": 0.8, "wall": "w0" }));
        assert_eq!(out, json!({ "width": 0.8, "style": "slider", "wall": "w0" }));
        let p = json!({ "a": 1 });
        assert_eq!(merged(&s, "window", &p), p);
    }

    #[test]
    fn set_as_default_on_a_cabinet() {
        let mut s = session();
        let r = s.execute("cabinet", json!({ "kind": "base", "at": [0.0, 0.0], "depth": 0.5, "doorStyle": "shaker" }));
        assert!(r.is_ok(), "{r:?}");
        let r = s.execute("arch.default", json!({ "id": "cb0" })).unwrap_or_default();
        assert_eq!(r["kind"], "cabinet.base");
        assert_eq!(r["default"]["depth"], 0.5);
        assert_eq!(r["default"]["doorStyle"], "shaker");
        assert!(r["default"].get("width").is_none());
        // A later cabinet takes it.
        let _ = s.execute("cabinet", json!({ "kind": "base", "at": [2.0, 0.0] }));
        let doc = s.doc().map(|d| d.arch.cabinets.clone()).unwrap_or_default();
        let second: Option<&Cabinet> = doc.get(1);
        assert_eq!(second.map(|c| c.depth), Some(0.5));
        assert_eq!(second.map(|c| c.door_style.name()), Some("shaker"));
    }

    #[test]
    fn a_wall_cabinet_keeps_its_own_defaults() {
        let mut s = session();
        let _ = s.execute("cabinet", json!({ "kind": "base", "at": [0.0, 0.0], "depth": 0.5 }));
        let _ = s.execute("arch.default", json!({ "id": "cb0" }));
        let _ = s.execute("cabinet", json!({ "kind": "wall", "at": [2.0, 0.0] }));
        let doc = s.doc().map(|d| d.arch.cabinets.clone()).unwrap_or_default();
        let wall = doc.get(1);
        assert_ne!(wall.map(|c| c.depth), Some(0.5));
    }

    #[test]
    fn set_as_default_on_a_door() {
        let mut s = session();
        let _ = s.execute("wall", json!({ "points": [[0.0, 0.0], [6.0, 0.0]] }));
        let _ = s.execute("door", json!({ "wall": "w0", "offset": 1.0, "width": 1.1 }));
        let _ = s.execute("window", json!({ "wall": "w0", "offset": 4.0, "width": 1.5 }));
        let r = s.execute("arch.default", json!({ "id": "o0" })).unwrap_or_default();
        assert_eq!(r["kind"], "door");
        assert_eq!(r["default"]["width"], 1.1);
        let r = s.execute("arch.default", json!({ "id": "o1" })).unwrap_or_default();
        assert_eq!(r["kind"], "window");
        assert!(r["default"].get("sill").is_some());
        let _ = s.execute("door", json!({ "wall": "w0", "offset": 2.5 }));
        let o: Option<Opening> =
            s.doc().ok().and_then(|d| d.arch.openings.iter().find(|o| o.kind == OpeningKind::Door && o.offset > 2.0 && o.offset < 3.0).cloned());
        assert_eq!(o.map(|o| o.width), Some(1.1));
    }

    #[test]
    fn set_as_default_on_a_furnishing() {
        let mut s = session();
        if let Ok(d) = s.doc_mut() {
            d.arch.furnishings.push(Furnishing {
                id: ArchId(0),
                item: "armchair".into(),
                size: [0.9, 0.8, 0.85],
                elevation: 0.1,
                ..Default::default()
            });
        }
        let r = s.execute("arch.default", json!({ "id": "f0" })).unwrap_or_default();
        assert_eq!(r["kind"], "furnishing.armchair");
        assert_eq!(r["default"]["size"], json!([0.9, 0.8, 0.85]));
        assert_eq!(r["default"]["elevation"], 0.1);
        let stored = s.doc().ok().and_then(|d| d.arch.defaults.get("furnishing.armchair").cloned());
        assert_eq!(stored, Some(r["default"].clone()));
    }

    #[test]
    fn set_as_default_refuses_walls() {
        let mut s = session();
        let _ = s.execute("wall", json!({ "points": [[0.0, 0.0], [6.0, 0.0]] }));
        let e = s.execute("arch.default", json!({ "id": "w0" }));
        assert!(e.is_err());
        assert!(format!("{e:?}").contains("Set as Default works on"));
        assert!(s.doc().map(|d| d.arch.defaults.is_empty()).unwrap_or(false));
    }

    #[test]
    fn defaults_survive_a_dxf_round_trip() {
        let mut s = session();
        if let Ok(d) = s.doc_mut() {
            d.arch.defaults.insert("door".into(), json!({ "width": 1.1, "style": "slider" }));
            d.touch_arch();
        }
        let want = s.doc().map(|d| d.arch.defaults.clone()).unwrap_or_default();
        let dxf = s.doc().ok().map(cadcraft_io::write_dxf);
        let back = dxf.and_then(|t| cadcraft_io::read_dxf(t.as_bytes()).ok());
        assert_eq!(back.map(|d| d.arch.defaults), Some(want));
    }
}
