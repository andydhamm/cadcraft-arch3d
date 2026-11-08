//! `foundation`, `foundation.edit` and `foundation.delete` (phase 16), and the `foundations` rows
//! of `arch.inspect`.
//!
//! A foundation stands under one level: under every exterior wall of that level with no level
//! below it (`cadcraft_arch::foundation::foundation_walls`). FOUNDATION makes one for every such
//! level (or the one named), updating a level's existing foundation in place. It creates no
//! walls: a basement's exterior walls take the "Concrete 0.2" wall type at their drawn thickness.

use cadcraft_arch::foundation::{concrete_wall_type, foundation_levels, foundation_walls, is_basement_level};
use cadcraft_doc::arch::{
    ArchId, ArchModel, CONCRETE_WALL_TYPE, DEFAULT_FOOTING_DEPTH, FOOTING_THICKNESS, FOUNDATION_LAYER, Foundation, FoundationKind, MAX_FOUNDATIONS,
    foundation_handle, foundation_label, handle_json, wall_label,
};
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, bad, str_param};
use crate::{Result, Session};

/// The shallowest footing below the floor structure's underside (m): the footing's thickness and a
/// stem at least this tall.
const MIN_STEM: f64 = 0.05;
/// The deepest footing below the level's floor (m).
const MAX_FOOTING_DEPTH: f64 = 5.0;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("foundation", "Foundation", run_foundation)
            .menu(&["Architecture", "Foundation"])
            .params("{type: \"stem\" | \"slab\", footingDepth?: m (default 1.2), level?: name | index}"),
        CommandSpec::new("foundation.edit", "Edit Foundation", run_foundation_edit)
            .menu(&["Architecture", "Edit Foundation"])
            .params("{id: \"fd0\", type?: \"stem\" | \"slab\", footingDepth?: m}"),
        CommandSpec::new("foundation.delete", "Delete Foundation", run_foundation_delete)
            .menu(&["Architecture", "Delete Foundation"])
            .params("{id: \"fd0\"}"),
    ]
}

fn kind_key(kind: FoundationKind) -> &'static str {
    match kind {
        FoundationKind::Stem => "stem",
        FoundationKind::Slab => "slab",
    }
}

/// `type`: "stem" or "slab" (case-insensitive).
fn parse_kind(cmd: &str, v: &Value) -> Result<FoundationKind> {
    match v.as_str().map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("stem") => Ok(FoundationKind::Stem),
        Some("slab") => Ok(FoundationKind::Slab),
        _ => Err(bad(cmd, format!("`type` must be \"stem\" or \"slab\", not {v}"))),
    }
}

/// `footingDepth`, when given and not null: a finite number.
fn parse_depth(cmd: &str, p: &Value) -> Result<Option<f64>> {
    match p.get("footingDepth") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|d| d.is_finite()).map(Some).ok_or_else(|| bad(cmd, "`footingDepth` must be a finite number of metres")),
    }
}

/// A level named by `raw`: its name (exactly, else ignoring case), or its index as a number or a
/// numeric string.
fn parse_level(cmd: &str, model: &ArchModel, raw: &Value) -> Result<u32> {
    let found = match raw {
        Value::Number(n) => n.as_u64().and_then(|i| u32::try_from(i).ok()).filter(|i| model.level(*i).is_some()),
        Value::String(s) => model
            .levels
            .iter()
            .find(|l| l.name == *s)
            .or_else(|| model.levels.iter().find(|l| l.name.eq_ignore_ascii_case(s.trim())))
            .map(|l| l.index)
            .or_else(|| s.trim().parse::<u32>().ok().filter(|i| model.level(*i).is_some())),
        _ => return Err(bad(cmd, "`level` must be a level name or index")),
    };
    found.ok_or_else(|| bad(cmd, format!("unknown level {raw}")))
}

/// `fd<n>` (or a bare number) to an id.
fn parse_id(cmd: &str, p: &Value) -> Result<ArchId> {
    let raw = str_param(p, "id").ok_or_else(|| bad(cmd, "`id` (\"fd0\") is required"))?;
    let t = raw.trim();
    let digits = t.get(..2).filter(|pre| pre.eq_ignore_ascii_case("fd")).and_then(|_| t.get(2..)).unwrap_or(t);
    digits.parse::<u32>().map(ArchId).map_err(|_| bad(cmd, format!("unknown foundation `{raw}`")))
}

fn level_name(model: &ArchModel, level: u32) -> String {
    model.level(level).map_or_else(|| format!("{level}"), |l| if l.name.is_empty() { format!("{level}") } else { l.name.clone() })
}

/// Refuse a footing depth on `level` unless it leaves room for the footing and a stem under the
/// floor structure, and is at most 5 m (a basement's footing does not use it).
fn check_depth(cmd: &str, model: &ArchModel, level: u32, depth: f64) -> Result<()> {
    if !depth.is_finite() {
        return Err(bad(cmd, "`footingDepth` must be a finite number of metres"));
    }
    if is_basement_level(model, level) {
        return Ok(());
    }
    let structure = model.level(level).map_or(0.3, |l| l.structure);
    let min = structure + FOOTING_THICKNESS + MIN_STEM;
    if !(min.is_finite() && depth >= min - 1e-9 && depth <= MAX_FOOTING_DEPTH) {
        return Err(bad(
            cmd,
            format!(
                "`footingDepth` {depth} on level \"{}\" must be from {min:.2} m (its floor structure, the footing and a stem) to {MAX_FOOTING_DEPTH} m",
                level_name(model, level)
            ),
        ));
    }
    Ok(())
}

fn run_foundation(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "foundation";
    let kind = parse_kind(cmd, p.get("type").ok_or_else(|| bad(cmd, "`type` (\"stem\" or \"slab\") is required"))?)?;
    let depth = parse_depth(cmd, p)?.unwrap_or(DEFAULT_FOOTING_DEPTH);
    let model = s.doc()?.arch.clone();
    let levels = match p.get("level").filter(|v| !v.is_null()) {
        Some(raw) => {
            let level = parse_level(cmd, &model, raw)?;
            if foundation_walls(&model, level).is_empty() {
                return Err(bad(cmd, format!("level \"{}\" has no exterior wall without a level below it", level_name(&model, level))));
            }
            vec![level]
        }
        None => foundation_levels(&model),
    };
    if levels.is_empty() {
        return Err(bad(cmd, "no level has an exterior wall without a level below it"));
    }
    for &level in &levels {
        check_depth(cmd, &model, level, depth)?;
    }
    let new = levels.iter().filter(|l| !model.foundations.iter().any(|f| f.level == **l)).count();
    if model.foundations.len().saturating_add(new) > MAX_FOUNDATIONS {
        return Err(bad(cmd, format!("a drawing holds at most {MAX_FOUNDATIONS} foundations")));
    }

    // Everything is checked: write.
    let mut ids = Vec::new();
    let mut walls = Vec::new();
    let mut concrete = Vec::new();
    let doc = s.doc_mut()?;
    for &level in &levels {
        let id = match doc.arch.foundations.iter_mut().find(|f| f.level == level) {
            Some(f) => {
                f.kind = kind;
                f.footing_depth = depth;
                f.id
            }
            None => {
                doc.arch.push_foundation(Foundation { level, kind, footing_depth: depth, layer: FOUNDATION_LAYER.to_string(), ..Default::default() })
            }
        };
        ids.push(id);
        for fw in foundation_walls(&model, level) {
            walls.push(fw.wall);
            if !fw.basement {
                continue;
            }
            if let Some(w) = doc.arch.walls.iter_mut().find(|w| w.id == fw.wall) {
                w.wall_type = CONCRETE_WALL_TYPE.to_string();
                w.layers = vec![("concrete".to_string(), w.thickness)];
                concrete.push(fw.wall);
            }
        }
    }
    if !concrete.is_empty() && !doc.arch.types.iter().any(|t| t.name == CONCRETE_WALL_TYPE) {
        doc.arch.types.push(concrete_wall_type());
    }
    doc.ensure_layer(FOUNDATION_LAYER);
    doc.touch_arch();
    Ok(json!({
        "foundations": ids.iter().map(|id| foundation_label(*id)).collect::<Vec<_>>(),
        "handles": ids.iter().map(|id| handle_json(foundation_handle(*id))).collect::<Vec<_>>(),
        "walls": walls.iter().map(|id| wall_label(*id)).collect::<Vec<_>>(),
        "concrete": concrete.iter().map(|id| wall_label(*id)).collect::<Vec<_>>(),
    }))
}

fn run_foundation_edit(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "foundation.edit";
    let id = parse_id(cmd, p)?;
    let kind = p.get("type").filter(|v| !v.is_null()).map(|v| parse_kind(cmd, v)).transpose()?;
    let depth = parse_depth(cmd, p)?;
    let model = s.doc()?.arch.clone();
    let current = model.foundation(id).ok_or_else(|| bad(cmd, format!("unknown foundation `{}`", foundation_label(id))))?;
    if let Some(d) = depth {
        check_depth(cmd, &model, current.level, d)?;
    }
    let doc = s.doc_mut()?;
    let Some(f) = doc.arch.foundations.iter_mut().find(|f| f.id == id) else {
        return Err(bad(cmd, format!("unknown foundation `{}`", foundation_label(id))));
    };
    if let Some(k) = kind {
        f.kind = k;
    }
    if let Some(d) = depth {
        f.footing_depth = d;
    }
    let f = f.clone();
    doc.touch_arch();
    Ok(foundation_row(&doc.arch, &f))
}

fn run_foundation_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "foundation.delete";
    let id = parse_id(cmd, p)?;
    if s.doc()?.arch.foundation(id).is_none() {
        return Err(bad(cmd, format!("unknown foundation `{}`", foundation_label(id))));
    }
    let doc = s.doc_mut()?;
    doc.arch.foundations.retain(|f| f.id != id);
    doc.touch_arch();
    Ok(json!({ "deleted": foundation_label(id) }))
}

/// A foundation's `arch.inspect` row.
fn foundation_row(model: &ArchModel, f: &Foundation) -> Value {
    json!({
        "id": foundation_label(f.id),
        "handle": handle_json(foundation_handle(f.id)),
        "level": f.level,
        "type": kind_key(f.kind),
        "footingDepth": f.footing_depth,
        "basement": is_basement_level(model, f.level),
        "walls": foundation_walls(model, f.level).iter().map(|w| wall_label(w.wall)).collect::<Vec<_>>(),
        "layer": f.layer,
    })
}

/// `arch.inspect {"foundations": true}`: one row per foundation, in model order.
pub(crate) fn inspect_foundations(model: &ArchModel, p: &Value, out: &mut Map<String, Value>) {
    if p.get("foundations").and_then(Value::as_bool) != Some(true) {
        return;
    }
    let rows = model.foundations.iter().map(|f| foundation_row(model, f)).collect();
    out.insert("foundations".to_string(), Value::Array(rows));
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// The phase-1 house through the commands (w0..w3 the exterior ring, w4 and w5 interior), with
    /// `level` putting the walls on that level.
    fn house_walls(s: &mut Session, level: Option<&str>) {
        let mut walls = [
            json!({"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": true}),
            json!({"points": [[0, 3], [10, 3]], "thickness": 0.1, "height": 2.4, "type": "interior"}),
            json!({"points": [[6, 3], [6, 8]], "thickness": 0.1, "height": 2.4, "type": "interior"}),
        ];
        for w in walls.iter_mut() {
            if let Some(l) = level {
                w["level"] = json!(l);
            }
            s.execute("wall", w).unwrap();
        }
        let detect = match level {
            Some(l) => json!({"level": l}),
            None => json!({}),
        };
        s.execute("roomdetect", &detect).unwrap();
    }

    fn house() -> Session {
        let mut s = Session::new();
        house_walls(&mut s, None);
        s
    }

    fn strings(v: &Value) -> Vec<String> {
        v.as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
    }

    fn undo_len(s: &Session) -> usize {
        s.state().map(|st| st.undo.len()).unwrap_or_default()
    }

    #[test]
    fn foundation_creates_one_per_level_and_replaces_in_place() {
        let mut s = house();
        let r = s.execute("foundation", &json!({"type": "stem"})).unwrap();
        assert_eq!(strings(&r["foundations"]), vec!["fd0"]);
        assert_eq!(strings(&r["walls"]), vec!["w0", "w1", "w2", "w3"]);
        assert!(strings(&r["concrete"]).is_empty());
        assert_eq!(r["handles"].as_array().map(Vec::len), Some(1));
        {
            let d = s.doc().unwrap();
            assert_eq!(d.arch.foundations.len(), 1);
            let f = &d.arch.foundations[0];
            assert_eq!((f.level, f.kind, f.footing_depth, f.layer.as_str()), (0, FoundationKind::Stem, 1.2, "S-FNDN"));
            assert!(d.layer("S-FNDN").is_some());
        }
        // Again with a deeper footing: the same foundation, updated.
        let r = s.execute("foundation", &json!({"type": "stem", "footingDepth": 1.5})).unwrap();
        assert_eq!(strings(&r["foundations"]), vec!["fd0"]);
        let d = s.doc().unwrap();
        assert_eq!(d.arch.foundations.len(), 1);
        assert_eq!(d.arch.foundations[0].footing_depth, 1.5);
        // A studio on grade higher up gets its own; level 0 keeps its id.
        s.execute("level", &json!({"name": "Studio", "elevation": 1.35})).unwrap();
        s.execute(
            "wall",
            &json!({"points": [[20, 0], [24, 0], [24, 4], [20, 4], [20, 0]], "thickness": 0.2, "height": 2.4, "exterior": true, "level": "Studio"}),
        )
        .unwrap();
        let r = s.execute("foundation", &json!({"type": "slab"})).unwrap();
        assert_eq!(strings(&r["foundations"]), vec!["fd0", "fd1"]);
        assert_eq!(strings(&r["walls"]), vec!["w0", "w1", "w2", "w3", "w6", "w7", "w8", "w9"]);
        let d = s.doc().unwrap();
        assert_eq!(
            d.arch.foundations.iter().map(|f| (f.level, f.kind, f.footing_depth)).collect::<Vec<_>>(),
            vec![(0, FoundationKind::Slab, 1.2), (1, FoundationKind::Slab, 1.2)]
        );
        // Only the studio, by name or index.
        let r = s.execute("foundation", &json!({"type": "stem", "level": "Studio"})).unwrap();
        assert_eq!(strings(&r["foundations"]), vec!["fd1"]);
        let r = s.execute("foundation", &json!({"type": "stem", "level": 0, "footingDepth": 2.0})).unwrap();
        assert_eq!(strings(&r["foundations"]), vec!["fd0"]);
        let d = s.doc().unwrap();
        assert_eq!(
            d.arch.foundations.iter().map(|f| (f.level, f.kind, f.footing_depth)).collect::<Vec<_>>(),
            vec![(0, FoundationKind::Stem, 2.0), (1, FoundationKind::Stem, 1.2)]
        );
    }

    #[test]
    fn edit_and_delete_are_single_undo_steps() {
        let mut s = house();
        let n = undo_len(&s);
        s.execute("foundation", &json!({"type": "stem"})).unwrap();
        assert_eq!(undo_len(&s), n + 1);
        let r = s.execute("foundation.edit", &json!({"id": "fd0", "type": "slab", "footingDepth": 1.4})).unwrap();
        assert_eq!((r["id"].as_str(), r["type"].as_str(), r["footingDepth"].as_f64()), (Some("fd0"), Some("slab"), Some(1.4)));
        assert_eq!(undo_len(&s), n + 2);
        assert_eq!(s.undo().unwrap().as_deref(), Some("Edit Foundation"));
        let f = s.doc().unwrap().arch.foundations[0].clone();
        assert_eq!((f.kind, f.footing_depth), (FoundationKind::Stem, 1.2));
        let r = s.execute("foundation.delete", &json!({"id": "fd0"})).unwrap();
        assert_eq!(r["deleted"], "fd0");
        assert!(s.doc().unwrap().arch.foundations.is_empty());
        assert_eq!(s.undo().unwrap().as_deref(), Some("Delete Foundation"));
        assert_eq!(s.doc().unwrap().arch.foundations.len(), 1);
        assert_eq!(s.undo().unwrap().as_deref(), Some("Foundation"));
        assert!(s.doc().unwrap().arch.foundations.is_empty());
    }

    #[test]
    fn refusals_leave_nothing() {
        let mut s = house();
        s.execute("foundation", &json!({"type": "stem"})).unwrap();
        let before = s.doc().unwrap().clone();
        let n = undo_len(&s);
        for (c, p) in [
            ("foundation", json!({})),
            ("foundation", json!({"type": "pier"})),
            ("foundation", json!({"type": 3})),
            ("foundation", json!({"type": "stem", "footingDepth": 0.2})),
            ("foundation", json!({"type": "stem", "footingDepth": 0.59})),
            ("foundation", json!({"type": "stem", "footingDepth": 5.1})),
            ("foundation", json!({"type": "stem", "footingDepth": "deep"})),
            ("foundation", json!({"type": "stem", "footingDepth": -1.2})),
            ("foundation", json!({"type": "stem", "level": "Moon"})),
            ("foundation", json!({"type": "stem", "level": 7})),
            ("foundation", json!({"type": "stem", "level": -1})),
            ("foundation", json!({"type": "stem", "level": [0]})),
            ("foundation.edit", json!({"id": "fd9", "type": "slab"})),
            ("foundation.edit", json!({"id": "fd0", "type": "pier"})),
            ("foundation.edit", json!({"id": "fd0", "footingDepth": 99})),
            ("foundation.edit", json!({"id": "fd0", "footingDepth": 0.3})),
            ("foundation.edit", json!({"type": "slab"})),
            ("foundation.delete", json!({"id": "sf0"})),
            ("foundation.delete", json!({"id": "fd4294967296"})),
            ("foundation.delete", json!({})),
        ] {
            assert!(s.execute(c, &p).is_err(), "{c} {p} is refused");
        }
        let err = s.execute("foundation", &json!({"type": "stem", "footingDepth": 0.2})).unwrap_err().to_string();
        assert!(err.contains("Level 1") && err.contains("0.60"), "{err}");
        assert_eq!(*s.doc().unwrap(), before, "nothing changed");
        assert_eq!(undo_len(&s), n, "no undo step");
        // The lowest footing depth is allowed.
        s.execute("foundation", &json!({"type": "stem", "footingDepth": 0.6})).unwrap();
        // A level with no wall on grade, and a drawing with none at all.
        s.execute("level", &json!({"name": "Upper", "elevation": 2.7})).unwrap();
        s.execute(
            "wall",
            &json!({"points": [[0, 0], [10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": true, "level": "Upper"}),
        )
        .unwrap();
        let before = s.doc().unwrap().clone();
        assert!(s.execute("foundation", &json!({"type": "stem", "level": "Upper"})).is_err());
        assert_eq!(*s.doc().unwrap(), before);
        let mut empty = Session::new();
        empty.execute("wall", &json!({"points": [[0, 0], [4, 0]], "thickness": 0.1, "type": "interior"})).unwrap();
        let before = empty.doc().unwrap().clone();
        assert!(empty.execute("foundation", &json!({"type": "stem"})).is_err());
        assert_eq!(*empty.doc().unwrap(), before);
    }

    #[test]
    fn foundations_are_capped() {
        let mut s = house();
        {
            let doc = s.doc_mut().unwrap();
            for _ in 0..MAX_FOUNDATIONS {
                doc.arch.push_foundation(Foundation { level: 5, ..Default::default() });
            }
        }
        let before = s.doc().unwrap().clone();
        assert!(s.execute("foundation", &json!({"type": "stem"})).is_err());
        assert_eq!(*s.doc().unwrap(), before);
    }

    #[test]
    fn basement_walls_become_concrete_0_2() {
        let mut s = house();
        s.execute("level", &json!({"name": "Basement", "elevation": -2.7})).unwrap();
        house_walls(&mut s, Some("Basement"));
        let r = s.execute("foundation", &json!({"type": "stem"})).unwrap();
        assert_eq!(strings(&r["foundations"]), vec!["fd0"]);
        assert_eq!(strings(&r["walls"]), vec!["w6", "w7", "w8", "w9"]);
        assert_eq!(strings(&r["concrete"]), vec!["w6", "w7", "w8", "w9"]);
        let d = s.doc().unwrap();
        assert_eq!(d.arch.foundations[0].level, 1);
        for w in d.arch.walls.iter().filter(|w| w.base_level == 1 && w.exterior) {
            assert_eq!(w.wall_type, "Concrete 0.2");
            assert_eq!(w.layers, vec![("concrete".to_string(), 0.2)]);
            assert_eq!(w.thickness, 0.2, "the drawn thickness stays");
        }
        assert!(d.arch.walls.iter().filter(|w| w.base_level == 0).all(|w| w.wall_type != "Concrete 0.2"));
        assert_eq!(d.arch.types.iter().filter(|t| t.name == "Concrete 0.2").count(), 1);
        // Again: the type is not added twice; deleting the foundation keeps the wall types.
        s.execute("foundation", &json!({"type": "stem"})).unwrap();
        s.execute("foundation.delete", &json!({"id": "fd0"})).unwrap();
        let d = s.doc().unwrap();
        assert_eq!(d.arch.types.iter().filter(|t| t.name == "Concrete 0.2").count(), 1);
        assert!(d.arch.walls.iter().filter(|w| w.base_level == 1 && w.exterior).all(|w| w.wall_type == "Concrete 0.2"));
        // Any footing depth suits a basement (its footing sits under the floor structure).
        s.execute("foundation", &json!({"type": "stem", "footingDepth": 0.3})).unwrap();
    }

    #[test]
    fn inspect_foundations_rows() {
        let mut s = house();
        let r = s.execute("arch.inspect", &json!({"walls": true})).unwrap();
        assert!(r.get("foundations").is_none(), "only on request");
        s.execute("foundation", &json!({"type": "stem"})).unwrap();
        let r = s.execute("arch.inspect", &json!({"foundations": true})).unwrap();
        let rows = r["foundations"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row["id"], "fd0");
        assert_eq!(row["handle"], handle_json(foundation_handle(ArchId(0))));
        assert_eq!(row["level"], 0);
        assert_eq!(row["type"], "stem");
        assert_eq!(row["footingDepth"], 1.2);
        assert_eq!(row["basement"], false);
        assert_eq!(strings(&row["walls"]), vec!["w0", "w1", "w2", "w3"]);
        assert_eq!(row["layer"], "S-FNDN");
        // A foundation whose level has lost its walls lists none.
        s.doc_mut().unwrap().arch.walls.clear();
        let r = s.execute("arch.inspect", &json!({"foundations": true})).unwrap();
        assert!(r["foundations"][0]["walls"].as_array().unwrap().is_empty());
    }
}
