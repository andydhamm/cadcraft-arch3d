//! Notes stored with linked views: `view.note`, `view.dim` and `view.note.delete`.
//!
//! A note lives in its view (so it undoes, saves and audits with the drawing), either in view coordinates or attached
//! to a corner of an object's lines in the view, and is kept when the view regenerates
//! (`cadcraft_arch3d::views::notes::resolve`). Notes change no 3D, so none of these commands touches the arch revision.

use serde_json::{Value, json};

use cadcraft_doc::arch::{
    ArchId, ArchKind, ArchView, Corner, MAX_NOTE_TEXT, MAX_VIEW_NOTES, NoteAttach, NoteKind, ViewNote, note_label, validate_note, view_label,
};

use crate::cmd::{CommandSpec, bad};
use crate::{EngineError, Result, Session};

/// Largest accepted view coordinate or offset magnitude.
const MAX_COORD: f64 = 1e6;
/// Shortest accepted dimension.
const MIN_DIM: f64 = 1e-6;

/// Command specs for the view-note commands.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("view.note", "View Note", run_note)
            .menu(&["Architecture", "View Note"])
            .params("{view, text, at?: [u,v] | attach?: {object, corner}, offset?: [du,dv], height?, kind?: text|marker, layer?}"),
        CommandSpec::new("view.dim", "View Dimension", run_dim)
            .menu(&["Architecture", "View Dimension"])
            .params("{view, a: [u,v] | {object, corner}, b: [u,v] | {object, corner}, offset?, text?}"),
        CommandSpec::new("view.note.delete", "Delete View Note", run_delete)
            .menu(&["Architecture", "Delete View Note"])
            .params("{view, note: \"n0\"}"),
    ]
}

fn present<'a>(p: &'a Value, key: &str) -> Option<&'a Value> {
    match p.get(key) {
        None | Some(Value::Null) => None,
        Some(v) => Some(v),
    }
}

fn coord(cmd: &str, key: &str, v: &Value) -> Result<[f64; 2]> {
    let err = || bad(cmd, format!("`{key}` must be [u, v] with two finite numbers within 1e6"));
    let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(err)?;
    let mut out = [0.0; 2];
    for (o, x) in out.iter_mut().zip(a) {
        *o = x.as_f64().filter(|n| n.is_finite() && n.abs() <= MAX_COORD).ok_or_else(err)?;
    }
    Ok(out)
}

fn finite(cmd: &str, key: &str, v: &Value) -> Result<f64> {
    v.as_f64().filter(|n| n.is_finite() && n.abs() <= MAX_COORD).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite number within 1e6")))
}

/// Parse a label such as "cb0" into its kind and id (walls, openings, rooms, cabinets and fixtures only).
fn parse_object(label: &str) -> Option<(ArchKind, ArchId)> {
    let (kind, rest) = if let Some(r) = label.strip_prefix("cb") {
        (ArchKind::Cabinet, r)
    } else if let Some(r) = label.strip_prefix("fx") {
        (ArchKind::Fixture, r)
    } else if let Some(r) = label.strip_prefix('w') {
        (ArchKind::Wall, r)
    } else if let Some(r) = label.strip_prefix('o') {
        (ArchKind::Opening, r)
    } else if let Some(r) = label.strip_prefix('r') {
        (ArchKind::Room, r)
    } else {
        return None;
    };
    if rest.is_empty() || rest.len() > 9 || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.parse::<u32>().ok().map(|n| (kind, ArchId(n)))
}

fn attach_of(cmd: &str, key: &str, v: &Value) -> Result<NoteAttach> {
    let err = || {
        bad(
            cmd,
            format!(
                "`{key}` must be {{object: a wall, opening, room, cabinet or fixture label, corner: topLeft|topRight|bottomLeft|bottomRight|center}}"
            ),
        )
    };
    let o = v.as_object().ok_or_else(err)?;
    if o.keys().any(|k| k != "object" && k != "corner") {
        return Err(err());
    }
    let label = o.get("object").and_then(Value::as_str).ok_or_else(err)?;
    let (kind, object) =
        parse_object(label).ok_or_else(|| bad(cmd, format!("`{key}.object` `{label}` is not a wall, opening, room, cabinet or fixture label")))?;
    let corner = o.get("corner").and_then(Value::as_str).and_then(Corner::from_key).ok_or_else(err)?;
    Ok(NoteAttach { kind, object, corner })
}

/// A point given as view coordinates or as an attachment.
enum Spot {
    Free([f64; 2]),
    Attached(NoteAttach),
}

fn spot(cmd: &str, key: &str, v: &Value) -> Result<Spot> {
    if v.is_array() { coord(cmd, key, v).map(Spot::Free) } else { attach_of(cmd, key, v).map(Spot::Attached) }
}

/// The view named by `view` ("v0" or its unique name) with geometry fresh for the current revision. A stale view is
/// regenerated on a copy, so a refusal leaves the document untouched.
fn fresh_view(s: &Session, cmd: &str, p: &Value) -> Result<ArchView> {
    let r = present(p, "view").and_then(Value::as_str).ok_or_else(|| bad(cmd, "`view` (such as \"v0\" or the view's name) is required"))?;
    let d = s.doc()?;
    let id = d.arch.view_by_ref(r).map(|v| v.id).ok_or_else(|| bad(cmd, format!("no view `{r}`")))?;
    let found = if d.arch.view(id).is_some_and(|v| v.is_stale(d.arch_revision)) {
        let mut m = d.arch.clone();
        cadcraft_arch3d::views::refresh(&mut m, d.arch_revision);
        m.view(id).cloned()
    } else {
        d.arch.view(id).cloned()
    };
    found.ok_or_else(|| EngineError::Other("the view vanished".into()))
}

/// The point of an attachment in the view's geometry; refused when the object has no lines there.
fn point_of(cmd: &str, key: &str, v: &ArchView, a: &NoteAttach) -> Result<[f64; 2]> {
    let g = v.generated.as_ref().ok_or_else(|| bad(cmd, "the view has no geometry"))?;
    cadcraft_arch3d::views::notes::attach_point(g, a)
        .ok_or_else(|| bad(cmd, format!("`{key}`: {} has no lines in view {}", a.kind.label(a.object), view_label(v.id))))
}

/// Store `note` in the view `id` with one document edit; returns the note's label.
fn commit(s: &mut Session, cmd: &str, id: ArchId, mut note: ViewNote, layer: &str) -> Result<ViewNote> {
    let doc = s.doc_mut()?;
    let view = doc.arch.view_mut(id).ok_or_else(|| bad(cmd, "the view vanished"))?;
    if view.notes.len() >= MAX_VIEW_NOTES {
        return Err(bad(cmd, format!("a view holds at most {MAX_VIEW_NOTES} notes")));
    }
    note.id = view.next_note_id;
    view.next_note_id = view.next_note_id.saturating_add(1);
    view.notes.push(note.clone());
    doc.ensure_layer(layer);
    Ok(note)
}

fn run_note(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "view.note";
    let view = fresh_view(s, CMD, p)?;
    let text = present(p, "text").and_then(Value::as_str).ok_or_else(|| bad(CMD, "`text` is required"))?;
    if text.is_empty() || text.chars().count() > MAX_NOTE_TEXT || text.chars().any(char::is_control) {
        return Err(bad(CMD, format!("`text` must be 1..={MAX_NOTE_TEXT} characters without control characters")));
    }
    let (at, attach) = match (present(p, "at"), present(p, "attach")) {
        (Some(_), Some(_)) => return Err(bad(CMD, "give `at` or `attach`, not both")),
        (None, None) => return Err(bad(CMD, "`at` ([u, v]) or `attach` ({object, corner}) is required")),
        (Some(a), None) => (coord(CMD, "at", a)?, None),
        (None, Some(a)) => {
            let a = attach_of(CMD, "attach", a)?;
            (point_of(CMD, "attach", &view, &a)?, Some(a))
        }
    };
    let offset = match present(p, "offset") {
        None => [0.0, 0.0],
        Some(_) if attach.is_none() => return Err(bad(CMD, "`offset` goes with `attach`")),
        Some(v) => coord(CMD, "offset", v)?,
    };
    let height = match present(p, "height") {
        None => 0.15,
        Some(v) => v.as_f64().filter(|h| h.is_finite() && *h > 0.0 && *h <= 10.0).ok_or_else(|| bad(CMD, "`height` must be in (0, 10]"))?,
    };
    let kind = match present(p, "kind").map(|v| v.as_str()) {
        None | Some(Some("text")) => NoteKind::Text,
        Some(Some("marker")) => NoteKind::Marker,
        Some(_) => return Err(bad(CMD, "`kind` must be text or marker")),
    };
    let layer = match present(p, "layer") {
        None => "A-ANNO-NOTE".to_string(),
        Some(v) => v
            .as_str()
            .filter(|l| !l.trim().is_empty() && l.chars().count() <= 255 && !l.chars().any(char::is_control))
            .ok_or_else(|| bad(CMD, "`layer` must be a non-empty name without control characters"))?
            .to_string(),
    };
    let at = [at[0] + offset[0], at[1] + offset[1]];
    let note = ViewNote { kind, text: text.to_string(), at, attach, offset, height, layer: layer.clone(), ..ViewNote::default() };
    validate_note(&note).map_err(|e| bad(CMD, e))?;
    let note = commit(s, CMD, view.id, note, &layer)?;
    let placed = s.doc()?.arch.view(view.id).map_or(at, |v| v.placed(at));
    Ok(json!({ "view": view_label(view.id), "note": note_label(note.id), "at": [at[0], at[1]], "placed": [placed[0], placed[1]] }))
}

fn run_dim(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "view.dim";
    let view = fresh_view(s, CMD, p)?;
    let mut ends: Vec<([f64; 2], Option<NoteAttach>)> = Vec::new();
    for key in ["a", "b"] {
        let v = present(p, key).ok_or_else(|| bad(CMD, format!("`{key}` ([u, v] or {{object, corner}}) is required")))?;
        ends.push(match spot(CMD, key, v)? {
            Spot::Free(c) => (c, None),
            Spot::Attached(a) => (point_of(CMD, key, &view, &a)?, Some(a)),
        });
    }
    let (Some(&(a, attach)), Some(&(b, attach_b))) = (ends.first(), ends.get(1)) else {
        return Err(bad(CMD, "`a` and `b` are required"));
    };
    let value = (b[0] - a[0]).hypot(b[1] - a[1]);
    if !value.is_finite() || value < MIN_DIM {
        return Err(bad(CMD, "`a` and `b` must be at least 1e-6 apart"));
    }
    let offset = match present(p, "offset") {
        None => 0.5,
        Some(v) => finite(CMD, "offset", v)?,
    };
    let text = match present(p, "text") {
        None => format!("{value:.3}").trim_end_matches('0').trim_end_matches('.').to_string(),
        Some(v) => v.as_str().ok_or_else(|| bad(CMD, "`text` must be a string"))?.to_string(),
    };
    let note = ViewNote { kind: NoteKind::Dimension, text, at: a, b: Some(b), attach, attach_b, offset: [0.0, offset], ..ViewNote::default() };
    validate_note(&note).map_err(|e| bad(CMD, e))?;
    let layer = note.layer.clone();
    let note = commit(s, CMD, view.id, note, &layer)?;
    Ok(json!({ "view": view_label(view.id), "note": note_label(note.id), "value": value }))
}

fn run_delete(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "view.note.delete";
    let view = fresh_view(s, CMD, p)?;
    let label = present(p, "note").and_then(Value::as_str).ok_or_else(|| bad(CMD, "`note` (such as \"n0\") is required"))?;
    let note = view
        .notes
        .iter()
        .find(|n| note_label(n.id) == label)
        .map(|n| n.id)
        .ok_or_else(|| bad(CMD, format!("no note `{label}` in view {}", view_label(view.id))))?;
    let doc = s.doc_mut()?;
    let v = doc.arch.view_mut(view.id).ok_or_else(|| bad(CMD, "the view vanished"))?;
    v.notes.retain(|n| n.id != note);
    Ok(json!({ "deleted": label }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ViewGeometry, ViewLine};

    /// A session with view v0 whose geometry (already fresh) holds a cabinet cb0 spanning u 1..1.6, v 0..0.9.
    fn session() -> Session {
        let mut s = Session::new();
        let d = s.doc_mut().unwrap();
        let id = d.arch.push_view(ArchView { name: "E".into(), insert: [10.0, 20.0], ..ArchView::default() }).unwrap();
        let line = |a: [f64; 2], b: [f64; 2]| ViewLine { a, b, kind: ArchKind::Cabinet, object: ArchId(0), ..Default::default() };
        let g = ViewGeometry {
            revision: d.arch_revision,
            extent: [0.0, 0.0, 2.0, 2.0],
            lines: vec![line([1.0, 0.0], [1.6, 0.0]), line([1.6, 0.0], [1.6, 0.9])],
            ..Default::default()
        };
        if let Some(v) = d.arch.view_mut(id) {
            v.generated = Some(g);
        }
        s
    }

    fn notes(s: &Session) -> Vec<ViewNote> {
        s.doc().unwrap().arch.views[0].notes.clone()
    }

    #[test]
    fn note_command_refusals() {
        let mut s = session();
        let before = s.doc().unwrap().clone();
        let cases = [
            json!({}),
            json!({"view": "v9", "text": "x", "at": [0, 0]}),
            json!({"view": "v0", "at": [0, 0]}),
            json!({"view": "v0", "text": "", "at": [0, 0]}),
            json!({"view": "v0", "text": "a\nb", "at": [0, 0]}),
            json!({"view": "v0", "text": "x"}),
            json!({"view": "v0", "text": "x", "at": [0, 0], "attach": {"object": "cb0", "corner": "center"}}),
            json!({"view": "v0", "text": "x", "at": [2e6, 0]}),
            json!({"view": "v0", "text": "x", "at": [0]}),
            json!({"view": "v0", "text": "x", "at": [0, 0], "offset": [1, 1]}),
            json!({"view": "v0", "text": "x", "at": [0, 0], "height": 0}),
            json!({"view": "v0", "text": "x", "at": [0, 0], "height": 11}),
            json!({"view": "v0", "text": "x", "at": [0, 0], "kind": "dimension"}),
            json!({"view": "v0", "text": "x", "at": [0, 0], "layer": ""}),
            json!({"view": "v0", "text": "x", "attach": {"object": "cb7", "corner": "center"}}),
            json!({"view": "v0", "text": "x", "attach": {"object": "s0", "corner": "center"}}),
            json!({"view": "v0", "text": "x", "attach": {"object": "cb0", "corner": "middle"}}),
            json!({"view": "v0", "text": "x", "attach": "cb0"}),
        ];
        for c in &cases {
            assert!(s.execute("view.note", c).is_err(), "{c}");
        }
        let dims = [
            json!({"view": "v0", "a": [0, 0]}),
            json!({"view": "v0", "a": [0, 0], "b": [0, 0]}),
            json!({"view": "v0", "a": [0, 0], "b": [1, 0], "offset": "x"}),
            json!({"view": "v0", "a": [0, 0], "b": [f64::NAN, 0]}),
            json!({"view": "v0", "a": [0, 0], "b": {"object": "cb4", "corner": "center"}}),
        ];
        for c in &dims {
            assert!(s.execute("view.dim", c).is_err(), "{c}");
        }
        assert!(s.execute("view.note.delete", &json!({"view": "v0", "note": "n3"})).is_err());
        assert!(s.execute("view.note.delete", &json!({"view": "v0"})).is_err());
        assert_eq!(*s.doc().unwrap(), before);
        assert!(s.undo().unwrap().is_none(), "refusals push no undo step");
    }

    #[test]
    fn notes_are_one_undo_step_and_do_not_touch_arch_revision() {
        let mut s = session();
        let rev = s.doc().unwrap().arch_revision;
        let r = s.execute("view.note", &json!({"view": "v0", "text": "HI", "at": [1.0, 1.5]})).unwrap();
        assert_eq!(r["note"], "n0");
        assert_eq!(r["placed"], json!([11.0, 21.5]));
        assert_eq!(s.doc().unwrap().arch_revision, rev);
        s.execute("view.note", &json!({"view": "v0", "text": "TOP", "attach": {"object": "cb0", "corner": "topLeft"}, "offset": [0, 0.1]})).unwrap();
        assert_eq!(notes(&s)[1].at, [1.0, 0.9 + 0.1]);
        assert_eq!(s.doc().unwrap().arch_revision, rev);
        assert_eq!(s.undo().unwrap().as_deref(), Some("View Note"));
        assert_eq!(notes(&s).len(), 1);
        assert_eq!(s.undo().unwrap().as_deref(), Some("View Note"));
        assert!(notes(&s).is_empty());
        assert_eq!(s.doc().unwrap().arch_revision, rev);
    }

    #[test]
    fn delete_removes_one_note() {
        let mut s = session();
        s.execute("view.note", &json!({"view": "E", "text": "A", "at": [0, 0]})).unwrap();
        s.execute("view.note", &json!({"view": "v0", "text": "B", "at": [1, 1]})).unwrap();
        let d = s
            .execute(
                "view.dim",
                &json!({"view": "v0", "a": {"object": "cb0", "corner": "bottomLeft"}, "b": {"object": "cb0", "corner": "bottomRight"}}),
            )
            .unwrap();
        assert_eq!(d["note"], "n2");
        assert!((d["value"].as_f64().unwrap() - 0.6).abs() < 1e-9);
        assert_eq!(s.execute("view.note.delete", &json!({"view": "v0", "note": "n0"})).unwrap()["deleted"], "n0");
        let left: Vec<String> = notes(&s).iter().map(|n| n.text.clone()).collect();
        assert_eq!(left, vec!["B".to_string(), "0.6".to_string()]);
        // Ids are never reused.
        let r = s.execute("view.note", &json!({"view": "v0", "text": "C", "at": [0, 0]})).unwrap();
        assert_eq!(r["note"], "n3");
    }

    #[test]
    fn notes_survive_a_dxf_round_trip() {
        let mut s = session();
        s.execute("view.note", &json!({"view": "v0", "text": "KEEP", "at": [-6.5, 1.5]})).unwrap();
        s.execute("view.dim", &json!({"view": "v0", "a": {"object": "cb0", "corner": "bottomLeft"}, "b": [3.0, 0.0], "offset": 0.7})).unwrap();
        let text = cadcraft_io::write_dxf(s.doc().unwrap());
        let back = cadcraft_io::read_dxf(text.as_bytes()).unwrap();
        let v = &back.arch.views[0];
        assert_eq!(v.notes.len(), 2);
        assert_eq!(v.notes[0].text, "KEEP");
        assert_eq!(v.notes[0].at, [-6.5, 1.5]);
        assert_eq!(v.notes[1].kind, NoteKind::Dimension);
        assert_eq!(v.notes[1].b, Some([3.0, 0.0]));
        assert_eq!(v.notes[1].offset, [0.0, 0.7]);
        assert_eq!(v.notes[1].attach.map(|a| a.corner), Some(Corner::BottomLeft));
        assert_eq!(v.next_note_id, 2);
    }
}
