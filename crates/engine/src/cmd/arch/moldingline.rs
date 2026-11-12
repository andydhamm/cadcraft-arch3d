//! Molding line commands (phase 18): a profile swept along a drawn 2D path at an elevation.

use cadcraft_arch::molding_line::{line_run, validate};
use cadcraft_arch::molding_profile::{builtin_profile, scaled};
use cadcraft_arch::molding_runs::run_length;
use cadcraft_doc::arch::{ArchId, ArchModel, MAX_MOLDING_LINES, MOLDING_LAYER, MoldingLine, handle_json, molding_line_handle, molding_line_label};
use cadcraft_geom::Vec2;
use serde_json::{Map, Value, json};

use crate::cmd::{CommandSpec, bad, str_param};
use crate::{Result, Session};

/// Most points accepted in one command, before consecutive duplicates are dropped.
const MAX_INPUT_POINTS: usize = 4_096;
/// The profile used when none is named.
const DEFAULT_PROFILE: &str = "molding:chair-rail";

/// The molding line command specs.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("moldingline", "Molding Line", run_create)
            .menu(&["Architecture", "Molding Line"])
            .params("{points: [[x,y],...], closed?: false, elevation: m, profile?: \"molding:chair-rail\", size?: [w,h], inset?: 0, flip?: false, level?: name|index}"),
        CommandSpec::new("moldingline.edit", "Edit Molding Line", run_edit)
            .menu(&["Architecture", "Edit Molding Line"])
            .params("{id: \"ml0\", points?, closed?, elevation?, profile?, size?, inset?, flip?, level?}"),
        CommandSpec::new("moldingline.delete", "Delete Molding Line", run_delete)
            .menu(&["Architecture", "Delete Molding Line"])
            .params("{id: \"ml0\"}"),
    ]
}

fn parse_id(cmd: &str, p: &Value) -> Result<ArchId> {
    let raw = str_param(p, "id").ok_or_else(|| bad(cmd, "`id` (\"ml0\") is required"))?;
    let text = raw.trim();
    let digits = text.strip_prefix("ml").unwrap_or(text);
    digits.parse::<u32>().map(ArchId).map_err(|_| bad(cmd, format!("unknown molding line `{raw}`")))
}

fn present<'a>(p: &'a Value, key: &str) -> Option<&'a Value> {
    p.get(key).filter(|v| !v.is_null())
}

fn finite(cmd: &str, v: &Value, key: &str) -> Result<f64> {
    v.as_f64().filter(|x| x.is_finite()).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite number")))
}

fn parse_points(cmd: &str, v: &Value) -> Result<Vec<Vec2>> {
    let err = || bad(cmd, "`points` must be a list of finite [x, y] points");
    let arr = v.as_array().ok_or_else(err)?;
    if arr.len() > MAX_INPUT_POINTS {
        return Err(bad(cmd, format!("`points` holds at most {MAX_INPUT_POINTS} points")));
    }
    let mut out = Vec::with_capacity(arr.len());
    for pt in arr {
        let pair = pt.as_array().filter(|a| a.len() == 2).ok_or_else(err)?;
        let x = pair.first().and_then(Value::as_f64).filter(|x| x.is_finite()).ok_or_else(err)?;
        let y = pair.get(1).and_then(Value::as_f64).filter(|y| y.is_finite()).ok_or_else(err)?;
        out.push(Vec2::new(x, y));
    }
    Ok(out)
}

fn parse_level(cmd: &str, model: &ArchModel, v: &Value) -> Result<u32> {
    let err = || bad(cmd, "`level` must be a level name or index");
    match v {
        Value::Number(n) => n.as_u64().and_then(|i| u32::try_from(i).ok()).filter(|i| model.level(*i).is_some()).ok_or_else(err),
        Value::String(s) => {
            let t = s.trim();
            if let Some(i) = t.parse::<u32>().ok().filter(|i| model.level(*i).is_some()) {
                return Ok(i);
            }
            model.levels.iter().find(|l| l.name.eq_ignore_ascii_case(t)).map(|l| l.index).ok_or_else(|| bad(cmd, format!("unknown level `{s}`")))
        }
        _ => Err(err()),
    }
}

fn parse_size(cmd: &str, v: &Value) -> Result<[f64; 2]> {
    let err = || bad(cmd, "`size` must be [width, height] in metres");
    let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(err)?;
    let w = a.first().and_then(Value::as_f64).ok_or_else(err)?;
    let h = a.get(1).and_then(Value::as_f64).ok_or_else(err)?;
    Ok([w, h])
}

/// Apply the keys present in `p` to `l`.
fn apply(cmd: &str, model: &ArchModel, p: &Value, l: &mut MoldingLine) -> Result<()> {
    if let Some(v) = present(p, "points") {
        l.points = parse_points(cmd, v)?;
    }
    if let Some(v) = present(p, "closed") {
        l.closed = v.as_bool().ok_or_else(|| bad(cmd, "`closed` must be true or false"))?;
    }
    if let Some(v) = present(p, "elevation") {
        l.elevation = finite(cmd, v, "elevation")?;
    }
    if let Some(v) = present(p, "inset") {
        l.inset = finite(cmd, v, "inset")?;
    }
    if let Some(v) = present(p, "flip") {
        l.flip = v.as_bool().ok_or_else(|| bad(cmd, "`flip` must be true or false"))?;
    }
    if let Some(v) = present(p, "level") {
        l.level = parse_level(cmd, model, v)?;
    }
    if let Some(v) = present(p, "profile") {
        let item = v.as_str().ok_or_else(|| bad(cmd, "`profile` must be a catalogue item such as \"molding:chair-rail\""))?;
        l.profile = builtin_profile(item.trim()).ok_or_else(|| bad(cmd, format!("unknown molding profile `{item}`")))?;
    }
    if let Some(v) = present(p, "size") {
        let size = parse_size(cmd, v)?;
        l.profile = scaled(&l.profile, size).map_err(|e| bad(cmd, e))?;
    }
    Ok(())
}

fn reply(model: &ArchModel, l: &MoldingLine) -> Value {
    let length = line_run(model, l).map_or(0.0, |r| run_length(&r));
    json!({ "moldingLine": molding_line_label(l.id), "handle": handle_json(molding_line_handle(l.id)), "length": length })
}

fn run_create(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "moldingline";
    let model = s.doc()?.arch.clone();
    if present(p, "points").is_none() {
        return Err(bad(cmd, "`points` is required"));
    }
    if present(p, "elevation").is_none() {
        return Err(bad(cmd, "`elevation` is required"));
    }
    let mut line = MoldingLine { profile: builtin_profile(DEFAULT_PROFILE).unwrap_or_default(), ..Default::default() };
    apply(cmd, &model, p, &mut line)?;
    validate(&line).map_err(|e| bad(cmd, e))?;
    if model.molding_lines.len() >= MAX_MOLDING_LINES {
        return Err(bad(cmd, format!("a drawing holds at most {MAX_MOLDING_LINES} molding lines")));
    }
    line.layer = MOLDING_LAYER.to_string();
    let doc = s.doc_mut()?;
    let id = doc.arch.push_molding_line(line);
    doc.ensure_layer(MOLDING_LAYER);
    doc.touch_arch();
    let made = doc.arch.molding_line(id).ok_or_else(|| bad(cmd, "molding line not found after insert"))?;
    Ok(reply(&doc.arch, made))
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "moldingline.edit";
    let id = parse_id(cmd, p)?;
    let model = s.doc()?.arch.clone();
    let current = model.molding_line(id).ok_or_else(|| bad(cmd, format!("unknown molding line `{}`", molding_line_label(id))))?.clone();
    let keys = ["points", "closed", "elevation", "profile", "size", "inset", "flip", "level"];
    if !keys.iter().any(|k| present(p, k).is_some()) {
        return Err(bad(cmd, "give at least one key to change"));
    }
    let mut edited = current;
    apply(cmd, &model, p, &mut edited)?;
    validate(&edited).map_err(|e| bad(cmd, e))?;
    let doc = s.doc_mut()?;
    if let Some(slot) = doc.arch.molding_lines.iter_mut().find(|m| m.id == id) {
        *slot = edited.clone();
    }
    doc.ensure_layer(MOLDING_LAYER);
    doc.touch_arch();
    Ok(reply(&doc.arch, &edited))
}

fn run_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "moldingline.delete";
    let id = parse_id(cmd, p)?;
    if s.doc()?.arch.molding_line(id).is_none() {
        return Err(bad(cmd, format!("unknown molding line `{}`", molding_line_label(id))));
    }
    let doc = s.doc_mut()?;
    doc.arch.molding_lines.retain(|m| m.id != id);
    doc.touch_arch();
    Ok(json!({ "deleted": [molding_line_label(id)] }))
}

/// `{"moldingLines": true}`: one row per molding line.
pub(crate) fn inspect_molding_lines(model: &ArchModel, p: &Value, out: &mut Map<String, Value>) {
    if p.get("moldingLines").and_then(Value::as_bool) != Some(true) {
        return;
    }
    let rows: Vec<Value> = model
        .molding_lines
        .iter()
        .map(|l| {
            json!({
                "id": molding_line_label(l.id),
                "handle": handle_json(molding_line_handle(l.id)),
                "level": l.level,
                "points": l.points.iter().map(|q| json!([q.x, q.y])).collect::<Vec<_>>(),
                "closed": l.closed,
                "elevation": l.elevation,
                "inset": l.inset,
                "flip": l.flip,
                "profile": l.profile.item,
                "size": l.profile.size(),
                "length": line_run(model, l).map_or(0.0, |r| run_length(&r)),
                "layer": l.layer,
            })
        })
        .collect();
    out.insert("moldingLines".to_string(), Value::Array(rows));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l_shape() -> Value {
        json!({"points": [[1.0, 1.0], [3.0, 1.0], [3.0, 2.0]], "elevation": 1.0})
    }

    fn count(s: &Session) -> usize {
        s.doc().map(|d| d.arch.molding_lines.len()).unwrap_or(usize::MAX)
    }

    #[test]
    fn create_edit_delete_single_undo_steps() {
        let mut s = Session::new();
        let r = s.execute("moldingline", &l_shape()).unwrap();
        assert_eq!(r["moldingLine"], "ml0");
        assert!((r["length"].as_f64().unwrap() - 3.0).abs() < 1e-9);
        assert_eq!(s.doc().unwrap().layers.iter().filter(|l| l.name == MOLDING_LAYER).count(), 1);
        s.execute("moldingline.edit", &json!({"id": "ml0", "inset": 0.1})).unwrap();
        assert!((s.doc().unwrap().arch.molding_lines[0].inset - 0.1).abs() < 1e-12);
        let r = s.execute("moldingline.delete", &json!({"id": "ml0"})).unwrap();
        assert_eq!(r["deleted"], json!(["ml0"]));
        assert_eq!(count(&s), 0);
        s.undo().unwrap();
        assert_eq!(count(&s), 1);
        s.undo().unwrap();
        assert!(s.doc().unwrap().arch.molding_lines[0].inset.abs() < 1e-12);
        s.undo().unwrap();
        assert_eq!(count(&s), 0);
    }

    #[test]
    fn refusals_leave_nothing() {
        let mut s = Session::new();
        let rev = s.doc().unwrap().arch_revision;
        let bad_calls = [
            json!({"points": [[0.0, 0.0]], "elevation": 1.0}),
            json!({"points": [[0.0, 0.0], [1.0, 0.0]]}),
            json!({"elevation": 1.0}),
            json!({"points": [[0.0, 0.0], [1.0, 0.0]], "elevation": 50.0}),
            json!({"points": [[0.0, 0.0], [1.0, 0.0]], "elevation": 1.0, "profile": "molding:nope"}),
            json!({"points": [[0.0, 0.0], [1.0, 0.0]], "elevation": 1.0, "size": [0.0, 0.1]}),
            json!({"points": [[0.0, 0.0], [1.0, 0.0]], "elevation": 1.0, "inset": 2.0}),
            json!({"points": [[0.0, 0.0], [1.0, 0.0]], "elevation": 1.0, "level": "attic"}),
            json!({"points": [[0.0, 0.0], [1.0, 0.0], [0.0, 0.01]], "elevation": 1.0}),
            json!({"points": [[0.0, 0.0], [1.0, "x"]], "elevation": 1.0}),
        ];
        for c in bad_calls {
            assert!(s.execute("moldingline", &c).is_err(), "{c}");
        }
        assert_eq!(count(&s), 0);
        assert_eq!(s.doc().unwrap().arch_revision, rev);
        s.execute("moldingline", &l_shape()).unwrap();
        for c in [
            json!({"id": "ml0"}),
            json!({"id": "ml9", "flip": true}),
            json!({"id": "ml0", "elevation": 99.0}),
            json!({"id": "ml0", "points": [[0.0, 0.0]]}),
        ] {
            assert!(s.execute("moldingline.edit", &c).is_err(), "{c}");
        }
        assert!(s.execute("moldingline.delete", &json!({"id": "ml9"})).is_err());
        let line = &s.doc().unwrap().arch.molding_lines[0];
        assert_eq!((line.points.len(), line.elevation, line.flip), (3, 1.0, false));
    }

    #[test]
    fn inspect_molding_lines_rows() {
        let mut s = Session::new();
        s.execute("moldingline", &json!({"points": [[1.0, 1.0], [3.0, 1.0], [3.0, 2.0]], "elevation": 1.0, "inset": 0.1})).unwrap();
        let r = s.execute("arch.inspect", &json!({"moldingLines": true})).unwrap();
        let row = &r["moldingLines"][0];
        assert_eq!(row["id"], "ml0");
        assert_eq!(row["profile"], "molding:chair-rail");
        assert_eq!(row["layer"], "A-FLOR-TRIM");
        assert_eq!(row["closed"], false);
        assert!((row["length"].as_f64().unwrap() - 2.8).abs() < 1e-9);
        assert!((row["size"][0].as_f64().unwrap() - 0.019).abs() < 1e-9);
        assert!((row["size"][1].as_f64().unwrap() - 0.064).abs() < 1e-9);
        let none = s.execute("arch.inspect", &json!({})).unwrap();
        assert!(none.get("moldingLines").is_none());
    }

    #[test]
    fn default_profile_is_chair_rail() {
        let mut s = Session::new();
        s.execute("moldingline", &l_shape()).unwrap();
        s.execute("moldingline", &json!({"points": [[0.0, 0.0], [1.0, 0.0]], "elevation": 0.5, "profile": "molding:base", "size": [0.02, 0.1]}))
            .unwrap();
        let d = s.doc().unwrap();
        assert_eq!(d.arch.molding_lines[0].profile, builtin_profile("molding:chair-rail").unwrap());
        assert_eq!(d.arch.molding_lines[1].profile.item, "molding:base");
        let size = d.arch.molding_lines[1].profile.size();
        assert!((size[0] - 0.02).abs() < 1e-9 && (size[1] - 0.1).abs() < 1e-9, "{size:?}");
    }
}
