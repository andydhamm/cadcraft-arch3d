//! The library commands (phase 14): `library` (search, Back and Forward, what fits a surface),
//! `library.item`, `library.tag`, `library.filter`, `library.install` and `library.local`. None
//! of them changes the drawing (all `.noundo()`); they act on [`Session::library`](crate::Session)
//! ([`crate::library::Library`]). Hostile parameters are refused, never repaired.

use std::path::Path;

use cadcraft_arch::library::{Category, ChipKind, ItemRecord, Query, folders, query_for, search};
use cadcraft_doc::arch::{ArchId, ArchModel, Surface};
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, always, bad};
use crate::library::Library;
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("library", "Library", run_library)
            .menu(&["Architecture", "Library"])
            .enabled(always)
            .noundo()
            .params("{text?, chips?: [{kind: \"category\"|\"folder\"|\"symbolType\"|\"placement\"|\"source\"|\"style\"|\"tag\", value}], for?: \"cabinet/cb0/pull\", back?: true, forward?: true}"),
        CommandSpec::new("library.item", "Library Item", run_item).menu(&["Architecture", "Library Item"]).enabled(always).noundo().params("{id}"),
        CommandSpec::new("library.tag", "Tag Library Item", run_tag)
            .menu(&["Architecture", "Tag Library Item"])
            .enabled(always)
            .noundo()
            .params("{item?, tag?, on?: true}"),
        CommandSpec::new("library.filter", "Library Filters", run_filter)
            .menu(&["Architecture", "Library Filters"])
            .enabled(always)
            .noundo()
            .params("{save?: name | apply?: name | delete?: name}"),
        CommandSpec::new("library.install", "Install Catalog", run_install)
            .menu(&["Architecture", "Install Catalog"])
            .enabled(always)
            .noundo()
            .params("{from: folder}"),
        CommandSpec::new("library.local", "Local Catalog", run_local)
            .menu(&["Architecture", "Local Catalog"])
            .enabled(always)
            .noundo()
            .params("{path: folder, licence: \"CC0-1.0\", category?: \"Furniture\"}"),
    ]
}

/// The parameters as an object (`null` is the empty one).
fn object<'a>(cmd: &str, p: &'a Value) -> Result<Option<&'a Map<String, Value>>> {
    match p {
        Value::Null => Ok(None),
        Value::Object(m) => Ok(Some(m)),
        _ => Err(bad(cmd, "the parameters must be an object")),
    }
}

fn get<'a>(m: Option<&'a Map<String, Value>>, key: &str) -> Option<&'a Value> {
    m.and_then(|m| m.get(key)).filter(|v| !v.is_null())
}

/// An optional `true`/`false` flag; anything else is refused.
fn flag(cmd: &str, m: Option<&Map<String, Value>>, key: &str) -> Result<bool> {
    match get(m, key) {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(bad(cmd, format!("`{key}` must be true or false"))),
    }
}

/// A required non-empty string.
fn text<'a>(cmd: &str, m: Option<&'a Map<String, Value>>, key: &str) -> Result<&'a str> {
    match get(m, key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s),
        Some(Value::String(_)) => Err(bad(cmd, format!("`{key}` is empty"))),
        Some(_) => Err(bad(cmd, format!("`{key}` must be a string"))),
        None => Err(bad(cmd, format!("`{key}` is required"))),
    }
}

fn record_json(r: &ItemRecord) -> Value {
    serde_json::to_value(r).unwrap_or_default()
}

/// What `library` replies for query `q`: all nine categories (their counts and icon items over
/// the whole library; the User catalog counts user items only), the matching items, the query and
/// whether Back and Forward can step, plus the folders of the matches when a category chip is on.
pub(crate) fn listing(lib: &Library, q: &Query) -> Value {
    let items = lib.items();
    let categories: Vec<Value> = Category::ALL
        .iter()
        .map(|c| {
            let name = c.name();
            let first = items.iter().filter(|r| r.category == name && (*c != Category::User || r.catalog == "user")).fold(
                (0usize, None::<&ItemRecord>),
                |(n, best), r| {
                    let better = best.is_none_or(|b| (&r.folder, &r.name, &r.id) < (&b.folder, &b.name, &b.id));
                    (n + 1, if better { Some(r) } else { best })
                },
            );
            json!({ "name": name, "items": first.0, "icon": first.1.map_or("", |r| r.id.as_str()) })
        })
        .collect();
    let found = search(items, q);
    let mut out = json!({
        "categories": categories,
        "items": found.iter().map(|r| record_json(r)).collect::<Vec<_>>(),
        "total": found.len(),
        "query": serde_json::to_value(q).unwrap_or_default(),
        "back": lib.history.can_back(),
        "forward": lib.history.can_forward(),
    });
    if q.chips.iter().any(|c| c.kind == ChipKind::Category) {
        out["folders"] = folders(&found).into_iter().map(|(name, n)| json!({ "name": name, "items": n })).collect();
    }
    out
}

/// Whether the surface `s` names an object in `m`.
fn surface_exists(m: &ArchModel, s: &Surface) -> bool {
    match s {
        Surface::WallSide { wall, .. } => m.wall(*wall).is_some(),
        Surface::RoomFloor(r) | Surface::RoomCeiling(r) => m.room(*r).is_some(),
        Surface::Roof(r) => m.roof(*r).is_some(),
        Surface::Cabinet { cabinet, .. } => m.cabinets.iter().any(|c| c.id == *cabinet),
        Surface::Fixture(f) => m.fixtures.iter().any(|x| x.id == *f),
        Surface::Furnishing { furnishing, .. } => m.furnishing(*furnishing).is_some(),
        Surface::Countertop(c) => m.countertop(*c).is_some(),
        Surface::Backsplash(b) => m.backsplash(*b).is_some(),
    }
}

/// For the molding targets (`room/r<n>/molding`, `cabinet/cb<n>/crown`, `cabinet/cb<n>/lightRail`,
/// `moldingLine/ml<n>`, which are not surfaces): whether the object exists; `None` for any other key.
fn molding_target_exists(m: &ArchModel, key: &str) -> Option<bool> {
    let parts: Vec<&str> = key.split('/').collect();
    let id = |p: &str, prefix: &str| -> Option<ArchId> {
        let digits = p.strip_prefix(prefix)?;
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse::<u32>().ok().map(ArchId)
    };
    match parts.as_slice() {
        ["room", r, "molding"] => Some(id(r, "r").is_some_and(|i| m.room(i).is_some())),
        ["cabinet", c, "crown" | "lightRail"] => Some(id(c, "cb").is_some_and(|i| m.cabinets.iter().any(|x| x.id == i))),
        ["moldingLine", l] => Some(id(l, "ml").is_some_and(|i| m.molding_line(i).is_some())),
        _ => None,
    }
}

/// The query for `{"for": surface key}`: the key must name a surface of the active drawing that
/// some library items fit.
fn for_query(s: &Session, v: &Value) -> Result<Query> {
    const CMD: &str = "library";
    let key = v.as_str().ok_or_else(|| bad(CMD, "`for` must be a surface key such as \"cabinet/cb0/pull\""))?;
    let shown: String = key.chars().take(64).collect();
    let d = s.doc().map_err(|_| bad(CMD, "`for` needs an open drawing"))?;
    if let Some(exists) = molding_target_exists(&d.arch, key) {
        if !exists {
            let object = key.split('/').next().unwrap_or("object");
            let object = if object == "moldingLine" { "molding line" } else { object };
            return Err(bad(CMD, format!("the drawing has no {object} `{shown}`")));
        }
        return query_for(key).ok_or_else(|| bad(CMD, format!("no library items fit {shown}")));
    }
    let surface = Surface::parse(key).ok_or_else(|| bad(CMD, format!("`{shown}` is not a surface key")))?;
    if !surface_exists(&d.arch, &surface) {
        return Err(bad(CMD, format!("the drawing has no {}", surface.object_key())));
    }
    query_for(key).ok_or_else(|| bad(CMD, format!("no library items fit {shown}")))
}

/// `library {text?, chips?, for?, back?, forward?}`.
fn run_library(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "library";
    let m = object(CMD, p)?;
    s.library.sync_io();
    let back = flag(CMD, m, "back")?;
    let forward = flag(CMD, m, "forward")?;
    let searching = ["text", "chips", "for"].iter().any(|k| get(m, k).is_some());
    if back && forward {
        return Err(bad(CMD, "`back` and `forward` together"));
    }
    if (back || forward) && searching {
        return Err(bad(CMD, "`back` and `forward` step the history: give no `text`, `chips` or `for` with them"));
    }
    let q = if back {
        s.library.history.back().ok_or_else(|| bad(CMD, "there is no earlier search"))?
    } else if forward {
        s.library.history.forward().ok_or_else(|| bad(CMD, "there is no later search"))?
    } else {
        let mut q = match get(m, "for") {
            Some(v) => for_query(s, v)?,
            None => Query::default(),
        };
        let given = crate::library::parse_query(get(m, "text"), get(m, "chips")).map_err(|e| bad(CMD, e))?;
        if !given.text.is_empty() {
            q.text = given.text;
        }
        q.chips.extend(given.chips);
        if q.chips.len() > crate::library::MAX_CHIPS {
            return Err(bad(CMD, format!("more than {} chips", crate::library::MAX_CHIPS)));
        }
        s.library.history.push(q.clone());
        q
    };
    Ok(listing(&s.library, &q))
}

/// `library.item {id}`.
fn run_item(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "library.item";
    let m = object(CMD, p)?;
    s.library.sync_io();
    let id = text(CMD, m, "id")?;
    let r = s.library.item(id).ok_or_else(|| bad(CMD, format!("no library item `{}`", id.chars().take(256).collect::<String>())))?;
    Ok(json!({ "item": record_json(r) }))
}

/// `library.tag {item, tag, on?}`; `{}` lists the tags (Favorites always).
fn run_tag(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "library.tag";
    let m = object(CMD, p)?;
    s.library.sync_io();
    if get(m, "item").is_none() && get(m, "tag").is_none() && get(m, "on").is_none() {
        return Ok(json!({ "tags": s.library.tags() }));
    }
    let item = text(CMD, m, "item")?;
    let tag = match get(m, "tag") {
        Some(Value::String(t)) => t.as_str(),
        Some(_) => return Err(bad(CMD, "`tag` must be a string")),
        None => return Err(bad(CMD, "`tag` is required")),
    };
    let on = match get(m, "on") {
        None => true,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(bad(CMD, "`on` must be true or false")),
    };
    let tags = s.library.set_tag(item, tag, on).map_err(|e| bad(CMD, e))?;
    Ok(json!({ "item": item, "tag": tag, "on": on, "tags": tags }))
}

fn filters_json(lib: &Library) -> Value {
    lib.filters().iter().map(|f| json!({ "name": f.name, "query": serde_json::to_value(&f.query).unwrap_or_default() })).collect()
}

/// `library.filter {save | apply | delete: name}`; `{}` lists the saved filters.
fn run_filter(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "library.filter";
    let m = object(CMD, p)?;
    s.library.sync_io();
    let actions: Vec<&str> = ["save", "apply", "delete"].into_iter().filter(|k| get(m, k).is_some()).collect();
    if actions.len() > 1 {
        return Err(bad(CMD, "give one of `save`, `apply` and `delete`"));
    }
    let Some(action) = actions.first().copied() else {
        return Ok(json!({ "filters": filters_json(&s.library) }));
    };
    let name = match get(m, action) {
        Some(Value::String(n)) => n.as_str(),
        _ => return Err(bad(CMD, format!("`{action}` must be a filter name"))),
    };
    let shown: String = name.chars().take(64).collect();
    match action {
        "save" => {
            let q = s.library.history.current().cloned().unwrap_or_default();
            s.library.save_filter(name, q).map_err(|e| bad(CMD, e))?;
            Ok(json!({ "saved": name, "filters": filters_json(&s.library) }))
        }
        "apply" => {
            let q = s.library.filter(name).cloned().ok_or_else(|| bad(CMD, format!("no saved filter `{shown}`")))?;
            s.library.history.push(q.clone());
            Ok(listing(&s.library, &q))
        }
        _ => {
            s.library.delete_filter(name).map_err(|_| bad(CMD, format!("no saved filter `{shown}`")))?;
            Ok(json!({ "deleted": name, "filters": filters_json(&s.library) }))
        }
    }
}

/// `library.install {from}`: copy a catalog folder into the user dir (native only).
fn run_install(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "library.install";
    let m = object(CMD, p)?;
    let from = text(CMD, m, "from")?;
    s.library.sync_io();
    let done = s.library.install(Path::new(from)).map_err(|e| bad(CMD, e))?;
    Ok(json!({
        "catalog": done.catalog,
        "items": done.items,
        "skipped": done.skipped.iter().map(|k| k.json()).collect::<Vec<_>>(),
    }))
}

/// `library.local {path, licence, category?}`: a folder of .glb files as the local catalog
/// (native only).
fn run_local(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "library.local";
    let m = object(CMD, p)?;
    let path = text(CMD, m, "path")?;
    let licence = match get(m, "licence") {
        Some(Value::String(l)) if !l.trim().is_empty() => l.as_str(),
        _ => return Err(bad(CMD, "a local catalog needs a licence: `licence`, such as \"CC0-1.0\", for every file in the folder")),
    };
    if licence.chars().count() > 128 || licence.chars().any(char::is_control) || licence.trim() != licence {
        return Err(bad(CMD, "`licence` must be a licence name such as \"CC0-1.0\""));
    }
    let category = match get(m, "category") {
        None => Category::Furniture,
        Some(Value::String(c)) => match Category::parse(c) {
            Some(Category::User) => return Err(bad(CMD, "local items cannot go in the User catalog")),
            Some(c) => c,
            None => return Err(bad(CMD, format!("unknown category `{}`", c.chars().take(64).collect::<String>()))),
        },
        Some(_) => return Err(bad(CMD, "`category` must be a category name")),
    };
    s.library.sync_io();
    let (n, skipped) = s.library.add_local(Path::new(path), licence, category).map_err(|e| bad(CMD, e))?;
    Ok(json!({ "catalog": "local", "items": n, "skipped": skipped.iter().map(|k| k.json()).collect::<Vec<_>>() }))
}

#[cfg(test)]
mod tests {
    use crate::Session;
    use crate::library::testing::{glb, session};
    use crate::library::{FAVORITES, Library};
    use crate::temp_dir::TempDir;
    use cadcraft_arch::library::{Category, ItemKind};
    use serde_json::{Value, json};

    fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
        s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
    }

    fn ids(v: &Value) -> Vec<String> {
        v["items"].as_array().map(|a| a.iter().filter_map(|i| i["id"].as_str().map(String::from)).collect()).unwrap_or_default()
    }

    #[test]
    fn library_lists_nine_categories_with_icons() {
        let dir = TempDir::new("library-cats");
        let mut s = session(&dir);
        let r = run(&mut s, "library", json!({}));
        let cats = r["categories"].as_array().cloned().unwrap_or_default();
        let names: Vec<&str> = cats.iter().filter_map(|c| c["name"].as_str()).collect();
        assert_eq!(names, Category::ALL.map(Category::name));
        // No query lists every item.
        assert_eq!(r["total"], json!(s.library.items().len()));
        assert_eq!(ids(&r).len(), s.library.items().len());
        for c in &cats {
            let name = c["name"].as_str().unwrap_or_default();
            let mut members: Vec<_> =
                s.library.items().iter().filter(|i| i.category == name && (name != "User catalog" || i.catalog == "user")).collect();
            members.sort_by(|a, b| (&a.folder, &a.name, &a.id).cmp(&(&b.folder, &b.name, &b.id)));
            assert_eq!(c["items"], json!(members.len()), "{name}");
            assert_eq!(c["icon"], json!(members.first().map_or("", |i| i.id.as_str())), "{name}");
        }
        // Every category but the (empty) User catalog has items and an icon.
        for c in &cats[..8] {
            assert!(c["items"].as_u64().unwrap_or(0) > 0 && c["icon"] != json!(""), "{c}");
        }
        assert_eq!(cats[8], json!({"name": "User catalog", "items": 0, "icon": ""}));
        assert_eq!(cats[5]["icon"], json!("door:bifold"), "Doors and windows: Doors, Bifold door first");
        // A user item counts in the User catalog.
        s.library.add_user_item(cadcraft_arch::library::ItemRecord {
            id: "user:my-kitchen".into(),
            name: "My kitchen".into(),
            category: "User catalog".into(),
            kind: ItemKind::Group,
            ..Default::default()
        });
        let r = run(&mut s, "library", json!({}));
        assert_eq!(r["categories"][8], json!({"name": "User catalog", "items": 1, "icon": "user:my-kitchen"}));
        assert!(r.get("folders").is_none(), "folders only with a category chip");
        // Deterministic.
        assert_eq!(run(&mut s, "library", json!({})), r);
    }

    #[test]
    fn library_text_and_chips_search() {
        let dir = TempDir::new("library-search");
        let mut s = session(&dir);
        let r = run(&mut s, "library", json!({"text": "chair"}));
        let found = ids(&r);
        assert!(found.contains(&"armchair".to_string()) && found.contains(&"chair-dining-wood".to_string()), "{found:?}");
        assert!(found.iter().all(|id| {
            let i = s.library.item(id).map(|i| (i.name.to_lowercase(), i.folder.to_lowercase(), i.keywords.join(" ").to_lowercase()));
            i.is_some_and(|(n, f, k)| n.contains("chair") || f.contains("chair") || k.contains("chair"))
        }));
        assert_eq!(r["query"], json!({"text": "chair", "chips": []}));
        // Every word must match; chips narrow with AND.
        let r = run(&mut s, "library", json!({"text": "dining chair"}));
        assert!(ids(&r).iter().all(|id| id.starts_with("chair-dining")), "{:?}", ids(&r));
        let r = run(&mut s, "library", json!({"chips": [{"kind": "category", "value": "Furniture"}, {"kind": "folder", "value": "Sofas"}]}));
        assert_eq!(ids(&r), ["sofa-chaise", "sofa-futon"]);
        assert_eq!(r["folders"], json!([{"name": "Sofas", "items": 2}]));
        let r = run(&mut s, "library", json!({"chips": [{"kind": "category", "value": "Doors and windows"}]}));
        assert_eq!(r["folders"], json!([{"name": "Doors", "items": 5}, {"name": "Windows", "items": 10}]));
        // Results sorted by category order, folder, name, id.
        let r = run(&mut s, "library", json!({"chips": [{"kind": "style", "value": "shaker"}]}));
        assert_eq!(ids(&r), ["frontstyle:door-shaker", "frontstyle:drawer-shaker"]);
        let r = run(&mut s, "library", json!({"chips": [{"kind": "source", "value": "builtin"}, {"kind": "placement", "value": "wall"}]}));
        assert_eq!(ids(&r), ["lightfixture:Wall sconce"]);
        let r = run(&mut s, "library", json!({"text": "sofa", "chips": [{"kind": "source", "value": "sample"}]}));
        assert_eq!(ids(&r), ["sofa-chaise", "sofa-futon", "material-fabric"], "a keyword matches too");
        // Zero results is a valid answer.
        let r = run(&mut s, "library", json!({"text": "spaceship"}));
        assert_eq!((r["total"].clone(), ids(&r).len()), (json!(0), 0));
        assert_eq!(r["categories"].as_array().map(Vec::len), Some(9));
    }

    #[test]
    fn library_back_and_forward() {
        let dir = TempDir::new("library-history");
        let mut s = session(&dir);
        assert!(s.execute("library", &json!({"back": true})).is_err(), "nothing to go back to");
        let r = run(&mut s, "library", json!({"text": "chair"}));
        assert_eq!((r["back"].clone(), r["forward"].clone()), (json!(false), json!(false)));
        let r = run(&mut s, "library", json!({"text": "sofa"}));
        assert_eq!((r["back"].clone(), r["forward"].clone()), (json!(true), json!(false)));
        let sofa = r.clone();
        run(&mut s, "library", json!({"chips": [{"kind": "category", "value": "Plants"}]}));
        let r = run(&mut s, "library", json!({"back": true}));
        assert_eq!(
            r,
            sofa.as_object()
                .map(|o| {
                    let mut o = o.clone();
                    o.insert("forward".into(), json!(true));
                    Value::Object(o)
                })
                .unwrap_or_default()
        );
        let r = run(&mut s, "library", json!({"back": true}));
        assert_eq!(r["query"]["text"], json!("chair"));
        assert_eq!((r["back"].clone(), r["forward"].clone()), (json!(false), json!(true)));
        assert!(s.execute("library", &json!({"back": true})).is_err());
        let r = run(&mut s, "library", json!({"forward": true}));
        assert_eq!(r["query"]["text"], json!("sofa"));
        let r = run(&mut s, "library", json!({"forward": true}));
        assert_eq!(r["query"]["chips"], json!([{"kind": "category", "value": "Plants"}]));
        assert_eq!(r["forward"], json!(false));
        assert!(s.execute("library", &json!({"forward": true})).is_err());
        // A new search drops the forward part.
        run(&mut s, "library", json!({"back": true}));
        let r = run(&mut s, "library", json!({"text": "lamp"}));
        assert_eq!((r["back"].clone(), r["forward"].clone()), (json!(true), json!(false)));
        // Stepping with a search, or both ways at once, is refused.
        assert!(s.execute("library", &json!({"back": true, "text": "x"})).is_err());
        assert!(s.execute("library", &json!({"back": true, "forward": true})).is_err());
        assert!(s.execute("library", &json!({"back": "yes"})).is_err());
    }

    #[test]
    fn library_for_a_pull_lists_pulls() {
        let dir = TempDir::new("library-for");
        let mut s = session(&dir);
        run(&mut s, "wall", json!({"points": [[0.0, 0.0], [5.0, 0.0]]}));
        run(&mut s, "cabinet", json!({"kind": "base", "at": [1.0, 1.0]}));
        let r = run(&mut s, "library", json!({"for": "cabinet/cb0/pull"}));
        assert_eq!(ids(&r), ["own:pull-bar-128", "own:pull-knob-32"]);
        assert_eq!(r["query"], json!({"text": "", "chips": [{"kind": "symbolType", "value": "pull"}]}));
        let r = run(&mut s, "library", json!({"for": "cabinet/cb0/front"}));
        assert_eq!(r["total"], json!(6));
        assert!(ids(&r).iter().all(|id| id.starts_with("frontstyle:")));
        let r = run(&mut s, "library", json!({"for": "cabinet/cb0/carcass"}));
        assert!(ids(&r).contains(&"oven-built-in".to_string()), "{:?}", ids(&r));
        let r = run(&mut s, "library", json!({"for": "wall/w0/left"}));
        assert!(r["items"].as_array().is_some_and(|a| !a.is_empty() && a.iter().all(|i| i["category"] == json!("Materials"))));
        // With text, the text narrows what fits.
        let r = run(&mut s, "library", json!({"for": "cabinet/cb0/pull", "text": "knob"}));
        assert_eq!(ids(&r), ["own:pull-knob-32"]);
        // A surface the drawing lacks, a malformed key, one nothing fits: refused.
        for bad in [json!("cabinet/cb9/pull"), json!("wall/w4/left"), json!("cabinet/cb0/handle"), json!("nonsense"), json!(5)] {
            assert!(s.execute("library", &json!({"for": bad})).is_err(), "{bad}");
        }
        run(&mut s, "fixture", json!({"kind": "sink", "at": [3.0, 1.0]}));
        assert!(s.execute("library", &json!({"for": "fixture/fx0"})).is_err(), "nothing in the library fits a fixture");
    }

    fn square_room(s: &mut Session) {
        run(s, "wall", json!({"points": [[0, 0], [4.1, 0], [4.1, 3.1], [0, 3.1], [0, 0]], "thickness": 0.1, "height": 2.4, "exterior": true}));
        run(s, "roomdetect", json!({}));
    }

    #[test]
    fn for_room_molding_is_the_moldings_folder() {
        let dir = TempDir::new("library-for-room-molding");
        let mut s = session(&dir);
        square_room(&mut s);
        let r = run(&mut s, "library", json!({"for": "room/r0/molding"}));
        assert_eq!(r["query"], json!({"text": "", "chips": [{"kind": "folder", "value": "Moldings"}]}));
        let found = ids(&r);
        assert!(found.contains(&"molding:crown".to_string()) && found.len() >= 5, "{found:?}");
        assert!(found.iter().all(|id| id.starts_with("molding:")), "{found:?}");
    }

    #[test]
    fn for_cabinet_crown_light_rail_and_molding_line_targets() {
        let dir = TempDir::new("library-for-molding-targets");
        let mut s = session(&dir);
        run(&mut s, "wall", json!({"points": [[0.0, 0.0], [5.0, 0.0]]}));
        run(&mut s, "cabinet", json!({"kind": "base", "at": [1.0, 1.0]}));
        let line = cadcraft_doc::arch::MoldingLine {
            points: vec![cadcraft_geom::Vec2::new(0.0, 0.0), cadcraft_geom::Vec2::new(2.0, 0.0)],
            ..Default::default()
        };
        let id = s.doc_mut().unwrap().arch.push_molding_line(line);
        let line_key = format!("moldingLine/ml{}", id.0);
        for key in ["cabinet/cb0/crown", "cabinet/cb0/lightRail", line_key.as_str()] {
            let r = run(&mut s, "library", json!({"for": key}));
            assert_eq!(r["query"]["chips"], json!([{"kind": "folder", "value": "Moldings"}]), "{key}");
            assert!(ids(&r).contains(&"molding:base".to_string()), "{key}");
        }
    }

    #[test]
    fn for_a_missing_room_cabinet_or_molding_line_is_refused() {
        let dir = TempDir::new("library-for-molding-missing");
        let mut s = session(&dir);
        square_room(&mut s);
        for key in ["room/r9/molding", "cabinet/cb0/crown", "cabinet/cb0/lightRail", "moldingLine/ml0", "room/rx/molding", "room//molding"] {
            let e = s.execute("library", &json!({"for": key})).unwrap_err().to_string();
            assert!(e.contains("the drawing has no"), "{key}: {e}");
        }
    }

    #[test]
    fn unknown_chip_kinds_are_refused() {
        let dir = TempDir::new("library-chips");
        let mut s = session(&dir);
        run(&mut s, "library", json!({"text": "chair"}));
        for bad in [
            json!({"chips": [{"kind": "colour", "value": "red"}]}),
            json!({"chips": [{"kind": "Category", "value": "Furniture"}]}),
            json!({"chips": [{"kind": 3, "value": "x"}]}),
            json!({"chips": [{"kind": "tag", "value": ""}]}),
            json!({"chips": [{"kind": "tag", "value": 7}]}),
            json!({"chips": [{"kind": "tag"}]}),
            json!({"chips": ["tag"]}),
            json!({"chips": {"kind": "tag", "value": "x"}}),
            json!({"chips": (0..40).map(|_| json!({"kind": "tag", "value": "x"})).collect::<Vec<_>>()}),
            json!({"text": 5}),
            json!({"text": "x".repeat(2000)}),
            json!([]),
            json!("chair"),
        ] {
            let e = s.execute("library", &bad);
            assert!(e.is_err(), "{bad} was taken");
        }
        let e = s.execute("library", &json!({"chips": [{"kind": "colour", "value": "red"}]})).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(e.contains("unknown chip kind `colour`"), "{e}");
        // A refused search leaves the history as it was.
        assert_eq!(s.library.history.current().map(|q| q.text.as_str()), Some("chair"));
        assert!(!s.library.history.can_back());
    }

    #[test]
    fn favorites_tag_and_untag() {
        let dir = TempDir::new("library-tags");
        let mut s = session(&dir);
        // Favorites is always offered.
        assert_eq!(run(&mut s, "library.tag", json!({})), json!({"tags": [FAVORITES]}));
        let r = run(&mut s, "library.tag", json!({"item": "armchair", "tag": "Favorites"}));
        assert_eq!(r["tags"], json!(["Favorites"]));
        run(&mut s, "library.tag", json!({"item": "door:pocket", "tag": "Favorites", "on": true}));
        run(&mut s, "library.tag", json!({"item": "armchair", "tag": "living room"}));
        assert_eq!(run(&mut s, "library.tag", json!({})), json!({"tags": ["Favorites", "living room"]}));
        let r = run(&mut s, "library", json!({"chips": [{"kind": "tag", "value": "Favorites"}]}));
        assert_eq!(ids(&r), ["armchair", "door:pocket"]);
        assert_eq!(run(&mut s, "library.item", json!({"id": "armchair"}))["item"]["tags"], json!(["Favorites", "living room"]));
        // Saved in state.json: a new library sees them.
        let state: Value = serde_json::from_slice(&std::fs::read(dir.join("state.json")).unwrap_or_default()).unwrap_or_default();
        assert_eq!(state["tags"]["armchair"], json!(["Favorites", "living room"]));
        let again = Library::with_user_dir(Some(dir.to_path_buf()));
        assert_eq!(again.item("door:pocket").map(|i| i.tags.clone()), Some(vec!["Favorites".to_string()]));
        // Untag.
        let r = run(&mut s, "library.tag", json!({"item": "armchair", "tag": "Favorites", "on": false}));
        assert_eq!(r["tags"], json!(["living room"]));
        run(&mut s, "library.tag", json!({"item": "door:pocket", "tag": "Favorites", "on": false}));
        let r = run(&mut s, "library", json!({"chips": [{"kind": "tag", "value": "Favorites"}]}));
        assert_eq!(r["total"], json!(0));
        assert_eq!(run(&mut s, "library.tag", json!({})), json!({"tags": ["Favorites", "living room"]}));
        let again = Library::with_user_dir(Some(dir.to_path_buf()));
        assert!(again.item("door:pocket").is_some_and(|i| i.tags.is_empty()));
        // Refused: unknown items, empty, long or untidy tags, bad flags.
        for bad in [
            json!({"item": "no-such-item", "tag": "Favorites"}),
            json!({"item": "armchair", "tag": ""}),
            json!({"item": "armchair", "tag": "x".repeat(65)}),
            json!({"item": "armchair", "tag": " padded"}),
            json!({"item": "armchair", "tag": "a\u{7}b"}),
            json!({"item": "armchair", "tag": 5}),
            json!({"item": "armchair"}),
            json!({"tag": "Favorites"}),
            json!({"item": "armchair", "tag": "x", "on": "no"}),
        ] {
            assert!(s.execute("library.tag", &bad).is_err(), "{bad}");
        }
        assert!(s.execute("library.tag", &json!({"item": "armchair", "tag": "x".repeat(64)})).is_ok(), "64 characters is fine");
    }

    #[test]
    fn saved_filters_persist() {
        let dir = TempDir::new("library-filters");
        let mut s = session(&dir);
        assert_eq!(run(&mut s, "library.filter", json!({})), json!({"filters": []}));
        run(&mut s, "library", json!({"text": "chair", "chips": [{"kind": "category", "value": "Furniture"}]}));
        let r = run(&mut s, "library.filter", json!({"save": "Chairs"}));
        assert_eq!(r["filters"], json!([{"name": "Chairs", "query": {"text": "chair", "chips": [{"kind": "category", "value": "Furniture"}]}}]));
        run(&mut s, "library", json!({"chips": [{"kind": "category", "value": "Plants"}]}));
        run(&mut s, "library.filter", json!({"save": "All plants"}));
        // Persisted: a new session reads them back, by name.
        let mut t = session(&dir);
        let listed = run(&mut t, "library.filter", json!({}));
        let names: Vec<&str> = listed["filters"].as_array().map(|a| a.iter().filter_map(|f| f["name"].as_str()).collect()).unwrap_or_default();
        assert_eq!(names, ["All plants", "Chairs"]);
        let applied = run(&mut t, "library.filter", json!({"apply": "Chairs"}));
        let direct = run(&mut s, "library", json!({"text": "chair", "chips": [{"kind": "category", "value": "Furniture"}]}));
        assert_eq!(applied["items"], direct["items"]);
        assert_eq!(applied["folders"], direct["folders"]);
        assert_eq!(t.library.history.current().map(|q| q.text.as_str()), Some("chair"));
        // Saving again under a name replaces it.
        run(&mut t, "library", json!({"text": "stool"}));
        run(&mut t, "library.filter", json!({"save": "Chairs"}));
        assert_eq!(t.library.filter("Chairs").map(|q| q.text.as_str()), Some("stool"));
        // Delete.
        let r = run(&mut t, "library.filter", json!({"delete": "All plants"}));
        assert_eq!(r["filters"].as_array().map(Vec::len), Some(1));
        let u = Library::with_user_dir(Some(dir.to_path_buf()));
        assert_eq!(u.filters().iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Chairs"]);
        // Unknown names and bad requests are refused.
        for bad in [
            json!({"apply": "Nope"}),
            json!({"delete": "Nope"}),
            json!({"save": ""}),
            json!({"save": "x".repeat(65)}),
            json!({"save": 3}),
            json!({"save": "a", "apply": "Chairs"}),
        ] {
            assert!(t.execute("library.filter", &bad).is_err(), "{bad}");
        }
    }

    /// A catalog folder at `dir/<name>` with `items` (catalog.json entries) and `files`.
    fn catalog_folder(dir: &TempDir, name: &str, items: Value, files: &[(&str, Vec<u8>)]) -> std::path::PathBuf {
        let root = dir.join(name);
        let _ = std::fs::create_dir_all(&root);
        let catalog = json!({"version": 1, "items": items});
        let _ = std::fs::write(root.join("catalog.json"), catalog.to_string());
        for (f, b) in files {
            let _ = std::fs::write(root.join(f), b);
        }
        root
    }

    fn entry(id: &str, file: &str) -> Value {
        json!({"id": id, "name": id, "category": "Furniture", "symbolType": "table", "placement": "floor", "licence": "CC0-1.0",
               "source": "https://example.org/tables", "author": "A. Maker", "file": file})
    }

    #[test]
    fn install_copies_and_updates_a_catalog() {
        let dir = TempDir::new("library-install");
        let user = dir.join("user-dir");
        let mut s = session(&user);
        let src = catalog_folder(
            &dir,
            "Nice Tables!",
            json!([entry("nt:table-a", "table-a.glb"), entry("nt:table-b", "table-b.glb")]),
            &[("table-a.glb", glb([1.2, 0.8, 0.75])), ("table-b.glb", glb([2.0, 1.0, 0.75]))],
        );
        let from = src.to_string_lossy().into_owned();
        let r = run(&mut s, "library.install", json!({"from": from}));
        assert_eq!(r, json!({"catalog": "installed:Nice-Tables-", "items": 2, "skipped": []}));
        let installed = user.join("installed").join("Nice-Tables-");
        assert!(installed.join("catalog.json").is_file() && installed.join("table-a.glb").is_file() && installed.join("table-b.glb").is_file());
        let a = s.library.item("nt:table-a").cloned().unwrap_or_default();
        assert_eq!((a.catalog.as_str(), a.read_only, a.kind, a.folder.as_str()), ("installed:Nice-Tables-", true, ItemKind::Symbol, "Tables"));
        assert_eq!(s.library.file_bytes("nt:table-a").ok(), Some(glb([1.2, 0.8, 0.75])));
        let r = run(&mut s, "library", json!({"chips": [{"kind": "source", "value": "installed"}]}));
        assert_eq!(ids(&r), ["nt:table-a", "nt:table-b"]);
        // Read back by a new session.
        let t = session(&user);
        assert!(t.library.item("nt:table-b").is_some());

        // Installing again updates: table-b goes, table-c comes, table-a changes.
        let _ = std::fs::remove_file(src.join("table-b.glb"));
        catalog_folder(
            &dir,
            "Nice Tables!",
            json!([entry("nt:table-a", "table-a.glb"), entry("nt:table-c", "table-c.glb")]),
            &[("table-a.glb", glb([1.4, 0.8, 0.75])), ("table-c.glb", glb([0.6, 0.6, 0.75]))],
        );
        let r = run(&mut s, "library.install", json!({"from": from}));
        assert_eq!(r, json!({"catalog": "installed:Nice-Tables-", "items": 2, "skipped": []}));
        assert!(s.library.item("nt:table-b").is_none() && s.library.item("nt:table-c").is_some());
        assert!(!installed.join("table-b.glb").exists(), "the old install is replaced");
        assert_eq!(s.library.file_bytes("nt:table-a").ok(), Some(glb([1.4, 0.8, 0.75])));
        let e = s.library.symbol("nt:table-a").map(|m| m.extents()).unwrap_or_default();
        assert!((e[0] - 1.4).abs() < 1e-6 && (e[1] - 0.8).abs() < 1e-6 && (e[2] - 0.75).abs() < 1e-6, "{e:?}");
        // Only the installer's own folders are left in installed/.
        let names: Vec<String> =
            std::fs::read_dir(user.join("installed")).map(|d| d.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect()).unwrap_or_default();
        assert_eq!(names, ["Nice-Tables-"]);

        // Refused: no folder, no catalog.json, a catalog of the wrong version.
        for bad in [json!({}), json!({"from": ""}), json!({"from": 5}), json!({"from": dir.join("nowhere").to_string_lossy()})] {
            assert!(s.execute("library.install", &bad).is_err(), "{bad}");
        }
        let empty = dir.join("empty");
        let _ = std::fs::create_dir_all(&empty);
        assert!(s.execute("library.install", &json!({"from": empty.to_string_lossy()})).is_err(), "no catalog.json");
        let v2 = dir.join("v2");
        let _ = std::fs::create_dir_all(&v2);
        let _ = std::fs::write(v2.join("catalog.json"), r#"{"version": 2, "items": []}"#);
        assert!(s.execute("library.install", &json!({"from": v2.to_string_lossy()})).is_err(), "version 2");
    }

    #[test]
    fn install_copies_and_validates_the_normal_map() {
        let dir = TempDir::new("library-install-normal");
        let user = dir.join("user-dir");
        let mut s = session(&user);

        let mut good = entry("test-mat", "test-mat.png");
        good["category"] = json!("Materials");
        good["normalMap"] = json!("test-mat-normal.png");
        let image = cadcraft_doc::arch::ImageRgba { width: 64, height: 64, rgba: vec![255; 64 * 64 * 4], ..Default::default() };
        let normal = cadcraft_doc::arch::ImageRgba { width: 64, height: 64, rgba: vec![128; 64 * 64 * 4], ..Default::default() };
        let src = catalog_folder(
            &dir,
            "mat",
            json!([good]),
            &[
                ("test-mat.png", cadcraft_io::png::encode_rgba8(image.width, image.height, &image.rgba).unwrap()),
                ("test-mat-normal.png", cadcraft_io::png::encode_rgba8(normal.width, normal.height, &normal.rgba).unwrap()),
            ],
        );
        let r = run(&mut s, "library.install", json!({"from": src.to_string_lossy()}));
        assert_eq!(r["items"], json!(1));
        assert_eq!(r["skipped"], json!([]));
        let installed = user.join("installed").join("mat");
        assert!(installed.join("test-mat.png").is_file());
        assert!(installed.join("test-mat-normal.png").is_file());
        assert_eq!(
            s.library.normal_map_bytes("test-mat").unwrap().unwrap(),
            cadcraft_io::png::encode_rgba8(normal.width, normal.height, &normal.rgba).unwrap()
        );

        let bad = catalog_folder(
            &dir,
            "bad-mat",
            json!([{
                "id": "bad-mat",
                "name": "Bad Mat",
                "category": "Materials",
                "licence": "CC0-1.0",
                "source": "https://example.org",
                "author": "A. Maker",
                "file": "bad-mat.png",
                "normalMap": "../outside.png"
            }]),
            &[("bad-mat.png", cadcraft_io::png::encode_rgba8(8, 8, &vec![255; 8 * 8 * 4]).unwrap())],
        );
        let r = run(&mut s, "library.install", json!({"from": bad.to_string_lossy()}));
        assert_eq!(r["items"], json!(0));
        assert_eq!(r["skipped"].as_array().map(Vec::len), Some(1));
        assert!(r["skipped"][0]["reason"].as_str().unwrap().contains("normal map"));
    }

    #[test]
    fn install_skips_unlicensed_items() {
        let dir = TempDir::new("library-install-skip");
        let user = dir.join("user-dir");
        let mut s = session(&user);
        let mut unlicensed = entry("x:unlicensed", "unlicensed.glb");
        unlicensed["licence"] = json!("");
        let mut no_source = entry("x:nosource", "nosource.glb");
        if let Some(o) = no_source.as_object_mut() {
            o.remove("source");
        }
        let src = catalog_folder(
            &dir,
            "mixed",
            json!([
                entry("x:good", "good.glb"),
                unlicensed,
                no_source,
                entry("x:missing", "missing.glb"),
                entry("x:broken", "broken.glb"),
                entry("armchair", "dup.glb"),
                {"id": "x:nofile", "name": "No file", "category": "Furniture", "licence": "CC0-1.0", "source": "s"},
            ]),
            &[
                ("good.glb", glb([1.0, 1.0, 1.0])),
                ("unlicensed.glb", glb([1.0, 1.0, 1.0])),
                ("nosource.glb", glb([1.0, 1.0, 1.0])),
                ("broken.glb", b"not a glb at all".to_vec()),
                ("dup.glb", glb([1.0, 1.0, 1.0])),
            ],
        );
        let r = run(&mut s, "library.install", json!({"from": src.to_string_lossy()}));
        assert_eq!(r["catalog"], json!("installed:mixed"));
        assert_eq!(r["items"], json!(1));
        assert_eq!(
            r["skipped"],
            json!([
                {"file": "unlicensed.glb", "reason": "no licence"},
                {"file": "nosource.glb", "reason": "no source"},
                {"file": "missing.glb", "reason": "the file is not in the folder"},
                {"file": "broken.glb", "reason": "not a glTF binary file (no glTF header)"},
                {"file": "dup.glb", "reason": "the id `armchair` is already in the library"},
                {"file": "x:nofile", "reason": "no file"},
            ])
        );
        assert!(s.library.item("x:good").is_some());
        for id in ["x:unlicensed", "x:nosource", "x:missing", "x:broken", "x:nofile"] {
            assert!(s.library.item(id).is_none(), "{id} loaded");
        }
        assert_eq!(s.library.item("armchair").map(|i| i.catalog.as_str()), Some("sample"));
        // Only the accepted item's file is copied, and the copied catalog lists only it.
        let inst = user.join("installed").join("mixed");
        assert!(inst.join("good.glb").is_file() && !inst.join("unlicensed.glb").exists() && !inst.join("broken.glb").exists());
        let copied: Value = serde_json::from_slice(&std::fs::read(inst.join("catalog.json")).unwrap_or_default()).unwrap_or_default();
        assert_eq!(copied["items"].as_array().map(Vec::len), Some(1));
        // An unlicensed entry that reaches the folder anyway never loads.
        let _ = std::fs::write(
            inst.join("catalog.json"),
            json!({"version": 1, "items": [entry("x:good", "good.glb"), {
            "id": "x:sneaky", "name": "Sneaky", "category": "Furniture", "source": "s", "file": "good.glb"}]})
            .to_string(),
        );
        s.library.reload();
        assert!(s.library.item("x:good").is_some() && s.library.item("x:sneaky").is_none());
    }

    #[test]
    fn local_reads_glb_folders() {
        let dir = TempDir::new("library-local");
        let user = dir.join("user-dir");
        let root = dir.join("models");
        for (rel, bytes) in [
            ("Furniture/Chairs/chair-a.glb", glb([0.5, 0.5, 0.9])),
            ("Plants/fern.glb", glb([0.4, 0.4, 0.6])),
            ("Chairs/stool.glb", glb([0.4, 0.4, 0.7])),
            ("loose.GLB", glb([1.0, 2.0, 3.0])),
            ("bad.glb", b"junk".to_vec()),
            ("notes.txt", b"not a model".to_vec()),
        ] {
            let p = root.join(rel);
            let _ = std::fs::create_dir_all(p.parent().unwrap_or(&root));
            let _ = std::fs::write(&p, bytes);
        }
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink(root.join("Plants"), root.join("linked"));
            let _ = std::os::unix::fs::symlink(root.join("Plants/fern.glb"), root.join("fern-link.glb"));
        }
        let mut s = session(&user);
        let r = run(
            &mut s,
            "library.local",
            json!({"path": root.to_string_lossy(), "licence": "CC-BY-4.0", "category": "Appliances and plumbing fixtures"}),
        );
        assert_eq!(r["catalog"], json!("local"));
        assert_eq!(r["items"], json!(4));
        assert_eq!(r["skipped"], json!([{"file": "bad.glb", "reason": "not a glTF binary file (no glTF header)"}]));
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        let local: Vec<_> = s.library.items().iter().filter(|i| i.catalog == "local").cloned().collect();
        let summary: Vec<(String, String, String, String)> =
            local.iter().map(|i| (i.id.clone(), i.name.clone(), i.category.clone(), i.folder.clone())).collect();
        let row = |a: &str, b: &str, c: &str, d: &str| (a.to_string(), b.to_string(), c.to_string(), d.to_string());
        assert_eq!(
            summary,
            [
                row("local:Chairs/stool", "stool", "Appliances and plumbing fixtures", "Chairs"),
                row("local:Furniture/Chairs/chair-a", "chair-a", "Furniture", "Chairs"),
                row("local:Plants/fern", "fern", "Plants", "Plants"),
                row("local:loose", "loose", "Appliances and plumbing fixtures", ""),
            ]
        );
        let chair = s.library.item("local:Furniture/Chairs/chair-a").cloned().unwrap_or_default();
        assert_eq!(chair.licence, "CC-BY-4.0");
        assert_eq!(chair.source, root.join("Furniture").join("Chairs").join("chair-a.glb").to_string_lossy());
        assert_eq!(
            (chair.kind, chair.symbol_type.as_str(), chair.placement.as_deref(), chair.read_only),
            (ItemKind::Symbol, "", Some("floor"), true)
        );
        let size = chair.size.unwrap_or_default();
        assert!((size[0] - 0.5).abs() < 1e-6 && (size[1] - 0.5).abs() < 1e-6 && (size[2] - 0.9).abs() < 1e-6, "{size:?}");
        assert_eq!(s.library.symbol("local:loose").map(|m| m.triangles()).ok(), Some(12));
        // Remembered in state.json and read again by a new library.
        let state: Value = serde_json::from_slice(&std::fs::read(user.join("state.json")).unwrap_or_default()).unwrap_or_default();
        assert_eq!(state["local"], json!([{"path": root.to_string_lossy(), "licence": "CC-BY-4.0", "category": "Appliances and plumbing fixtures"}]));
        let t = session(&user);
        let again: Vec<_> = t.library.items().iter().filter(|i| i.catalog == "local").cloned().collect();
        assert_eq!(again, local);
        // Adding the same folder again replaces it (no duplicates).
        let r = run(&mut s, "library.local", json!({"path": root.to_string_lossy(), "licence": "CC0-1.0"}));
        assert_eq!(r["items"], json!(4));
        assert_eq!(s.library.local_catalogs().len(), 1);
        assert_eq!(s.library.item("local:loose").map(|i| (i.category.as_str(), i.licence.as_str())), Some(("Furniture", "CC0-1.0")));
        // Not a folder, an unknown or the User category: refused.
        for bad in [
            json!({"path": dir.join("nowhere").to_string_lossy(), "licence": "CC0-1.0"}),
            json!({"path": root.join("loose.GLB").to_string_lossy(), "licence": "CC0-1.0"}),
            json!({"path": root.to_string_lossy(), "licence": "CC0-1.0", "category": "Spaceships"}),
            json!({"path": root.to_string_lossy(), "licence": "CC0-1.0", "category": "User catalog"}),
            json!({"licence": "CC0-1.0"}),
        ] {
            assert!(s.execute("library.local", &bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn local_needs_a_licence() {
        let dir = TempDir::new("library-local-licence");
        let user = dir.join("user-dir");
        let root = dir.join("models");
        let _ = std::fs::create_dir_all(&root);
        let _ = std::fs::write(root.join("chair.glb"), glb([0.5, 0.5, 0.9]));
        let mut s = session(&user);
        let path = root.to_string_lossy().into_owned();
        for bad in [
            json!({"path": path}),
            json!({"path": path, "licence": ""}),
            json!({"path": path, "licence": "   "}),
            json!({"path": path, "licence": 0}),
            json!({"path": path, "licence": null}),
        ] {
            let e = s.execute("library.local", &bad).err().map(|e| e.to_string()).unwrap_or_default();
            assert!(e.contains("needs a licence"), "{bad}: {e}");
        }
        assert!(s.library.items().iter().all(|i| i.catalog != "local"), "nothing was added");
        assert!(s.library.local_catalogs().is_empty());
        assert!(!user.join("state.json").exists(), "nothing was remembered");
        assert!(s.execute("library.local", &json!({"path": path, "licence": "CC0-1.0"})).is_ok());
        assert!(s.library.item("local:chair").is_some());
    }

    #[test]
    fn library_item_and_hostile_params() {
        let dir = TempDir::new("library-item");
        let mut s = session(&dir);
        let r = run(&mut s, "library.item", json!({"id": "fixture:sink"}));
        assert_eq!(r["item"]["name"], json!("Sink"));
        assert_eq!(r["item"]["folder"], json!("Parametric"));
        assert_eq!(r["item"]["readOnly"], json!(true));
        for bad in [json!({}), json!({"id": "nope"}), json!({"id": 3}), json!([])] {
            assert!(s.execute("library.item", &bad).is_err(), "{bad}");
        }
        // The commands run without a drawing (the Start tab) and change no drawing.
        let mut e = Session::empty();
        assert!(e.execute("library", &json!({"text": "door"})).is_ok());
        assert!(e.execute("library", &json!({"for": "wall/w0/left"})).is_err());
        let before = s.doc().map(|d| d.arch_revision).ok();
        run(&mut s, "library", json!({}));
        assert_eq!(s.doc().map(|d| d.arch_revision).ok(), before);
    }
}
