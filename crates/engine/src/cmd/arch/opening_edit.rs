use cadcraft_arch::opening::validate_placement;
use cadcraft_doc::ArchId;
use cadcraft_doc::arch::{OpeningKind, opening_label};
use serde_json::{Value, json};

use crate::Session;
use crate::cmd::{CommandSpec, bad, size_param, str_param};

fn parse_opening_id(s: &str) -> Result<ArchId, String> {
    let s = s.trim();
    let bytes = s.as_bytes();
    if bytes.len() < 2 || (bytes[0] != b'o' && bytes[0] != b'O') {
        return Err("opening id must be like `o0`".to_string());
    }
    let digits = &s[1..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("opening id `{s}` is not of the form `o<n>`"));
    }
    let n = digits.parse::<u32>().map_err(|_| format!("opening id `{s}` is not a valid `o<n>`"))?;
    Ok(ArchId(n))
}

fn valid_style_list(kind: OpeningKind) -> &'static [&'static str] {
    crate::cmd::arch::opening::valid_styles(kind)
}

fn run_opening_edit(s: &mut Session, p: &Value) -> Result<Value, crate::EngineError> {
    const CMD: &str = "opening.edit";

    let id_value = p.get("id").and_then(Value::as_str).ok_or_else(|| bad(CMD, "`id` (string opening label) is required"))?;
    let id = parse_opening_id(id_value).map_err(|e| bad(CMD, e))?;

    let any_change = ["width", "height", "sill", "style"].iter().any(|k| p.get(k).is_some_and(|v| !v.is_null()));
    if !any_change {
        return Err(bad(CMD, "`width`, `height`, `sill` or `style` is required"));
    }

    let mut edited = {
        let d = s.doc()?;
        let idx = d.arch.opening_index(id).ok_or_else(|| bad(CMD, format!("unknown opening `{id_value}`")))?;
        let base = d.arch.openings.get(idx).cloned().ok_or_else(|| bad(CMD, format!("unknown opening `{id_value}`")))?;

        let width_before = base.width;
        let height_before = base.height;
        let width = match p.get("width") {
            None | Some(Value::Null) => width_before,
            Some(_) => size_param(CMD, p, "width", false)?.ok_or_else(|| bad(CMD, "`width` must be positive"))?,
        };
        let height = match p.get("height") {
            None | Some(Value::Null) => height_before,
            Some(_) => size_param(CMD, p, "height", false)?.ok_or_else(|| bad(CMD, "`height` must be positive"))?,
        };
        let sill = match p.get("sill") {
            None | Some(Value::Null) => base.sill,
            Some(_) => size_param(CMD, p, "sill", true)?.ok_or_else(|| bad(CMD, "`sill` must be zero or positive"))?,
        };

        let mut edited = base;
        edited.width = width;
        edited.height = height;
        edited.sill = sill;

        if let Some(raw) = str_param(p, "style") {
            let lower = raw.trim().to_ascii_lowercase();
            let valid = valid_style_list(edited.kind);
            if !valid.iter().any(|item| *item == lower) {
                return Err(bad(CMD, format!("`style` must be one of {} for {:?}; got `{raw}`", valid.join(", "), edited.kind)));
            }
            edited.style = lower;
        }

        let width_changed = (edited.width - width_before).abs() > 1e-9;
        let height_changed = (edited.height - height_before).abs() > 1e-9;
        if width_changed || height_changed {
            edited.rough_width = edited.width + 0.12;
            edited.rough_height = edited.height + 0.12;
        }

        let host = edited.host;
        let check_h = cadcraft_arch::opening::check_height(&d.arch, &edited);
        validate_placement(&d.arch, host, edited.offset, edited.width, edited.sill, check_h, edited.kind, &edited.style, Some(id))
            .map_err(|e| bad(CMD, e.to_string()))?;

        edited
    };

    let response = json!({
        "opening": opening_label(id),
        "width": edited.width,
        "height": edited.height,
        "sill": edited.sill,
        "style": edited.style,
    });

    let mut d = s.doc_mut()?;
    let idx = d.arch.opening_index(id).ok_or_else(|| bad(CMD, format!("unknown opening `{id_value}`")))?;
    if let Some(opening) = d.arch.openings.get_mut(idx) {
        *opening = edited;
    }
    d.touch_arch();

    let mut out = response.as_object().cloned().unwrap_or_default();
    out.insert("revision".to_string(), serde_json::Value::from(d.arch_revision));
    Ok(Value::Object(out))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("opening.edit", "Edit Opening", run_opening_edit)
            .menu(&["Architecture", "Edit Opening"])
            .params("{id: \"o0\", width?, height?, sill?, style?}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EngineError;
    use serde_json::json;

    fn with_wall() -> Session {
        let mut s = Session::new();
        let _ = s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0]]})).unwrap();
        s
    }

    fn with_door() -> Session {
        let mut s = with_wall();
        let _ = s.execute("door", &json!({"wall": "w0", "offset": 3.0, "width": 0.9, "height": 2.1})).unwrap();
        s
    }

    fn with_window() -> Session {
        let mut s = with_wall();
        let _ = s.execute("window", &json!({"wall": "w0", "offset": 3.0, "width": 1.2, "height": 1.2, "style": "single-hung"})).unwrap();
        s
    }

    #[test]
    fn edit_width_regrows_the_rough_opening() {
        let mut s = with_window();
        let r = s.execute("opening.edit", &json!({"id": "o0", "width": 1.5})).unwrap();
        assert_eq!(r["opening"], json!("o0"));
        assert_eq!(r["width"], json!(1.5));
        let opening = s.doc().unwrap().arch.opening(ArchId(0)).unwrap();
        assert!((opening.rough_width - 1.62).abs() < 1e-9, "rough width {} != 1.62", opening.rough_width);
        assert!((opening.rough_height - 1.32).abs() < 1e-9, "rough height {} != 1.32", opening.rough_height);
    }

    #[test]
    fn edit_is_revalidated_on_the_wall() {
        let mut s = with_door();
        let before = s.doc().unwrap().arch.openings[0].clone();
        let err = s.execute("opening.edit", &json!({"id": "o0", "width": 8.0})).unwrap_err();
        assert!(matches!(err, EngineError::BadParams { .. }));
        assert_eq!(s.doc().unwrap().arch.openings[0], before);
    }

    #[test]
    fn edit_refusals_change_nothing() {
        let mut s = with_window();
        let before = s.doc().unwrap().arch.openings[0].clone();
        let err = s.execute("opening.edit", &json!({"id": "o0", "style": "hinged"})).unwrap_err();
        assert!(matches!(err, EngineError::BadParams { .. }));
        assert_eq!(s.doc().unwrap().arch.openings[0], before);

        let mut s2 = with_window();
        let err = s2.execute("opening.edit", &json!({"id": "o0", "style": "single-hung"})).unwrap();
        assert_eq!(err["style"], json!("single-hung"));

        let mut s3 = with_door();
        assert!(s3.execute("opening.edit", &json!({"width": 1.2})).is_err());
        assert_eq!(s3.doc().unwrap().arch.openings.len(), 1);
    }

    #[test]
    fn edit_is_one_undo_step() {
        let mut s = with_window();
        let before = s.doc().unwrap().arch.openings[0].clone();
        let _ = s.execute("opening.edit", &json!({"id": "o0", "width": 1.5})).unwrap();
        assert_ne!(s.doc().unwrap().arch.openings[0], before);
        assert_eq!(s.undo().unwrap(), Some("Edit Opening".to_string()));
        assert_eq!(s.doc().unwrap().arch.openings[0], before);
    }

    #[test]
    fn edit_needs_a_key() {
        let mut s = with_window();
        let err = s.execute("opening.edit", &json!({"width": 1.5})).unwrap_err();
        assert!(matches!(err, EngineError::BadParams { .. }));
    }

    #[test]
    fn edit_style_follows_the_kind_whitelist() {
        let mut s = with_window();
        let r = s.execute("opening.edit", &json!({"id": "o0", "style": "Single-Hung"})).unwrap();
        assert_eq!(r["style"], json!("single-hung"));

        let mut s2 = with_door();
        let err = s2.execute("opening.edit", &json!({"id": "o0", "style": "casement"})).unwrap_err();
        assert!(matches!(err, EngineError::BadParams { .. }));
        assert_eq!(s2.doc().unwrap().arch.openings[0].style, "hinged");
    }

    #[test]
    fn edit_unknown_opening_id_is_refused() {
        let mut s = with_window();
        let err = s.execute("opening.edit", &json!({"id": "o99", "width": 1.5})).unwrap_err();
        assert!(matches!(err, EngineError::BadParams { .. }));
        assert_eq!(s.doc().unwrap().arch.openings.len(), 1);
    }

    #[test]
    fn edit_bad_id_format_is_refused() {
        let mut s = with_window();
        for id in ["x0", "o", "o1a", "0", "o-1", ""] {
            let err = s.execute("opening.edit", &json!({"id": id, "width": 1.5})).unwrap_err();
            assert!(matches!(err, EngineError::BadParams { .. }), "id `{id}` should be refused");
        }
        assert_eq!(s.doc().unwrap().arch.openings.len(), 1);
    }

    #[test]
    fn edit_multiple_keys_at_once() {
        let mut s = with_window();
        let r = s.execute("opening.edit", &json!({"id": "o0", "width": 1.5, "height": 1.0, "sill": 1.2, "style": "casement"})).unwrap();
        assert_eq!(r["width"], json!(1.5));
        assert_eq!(r["height"], json!(1.0));
        assert_eq!(r["sill"], json!(1.2));
        assert_eq!(r["style"], json!("casement"));
        let opening = s.doc().unwrap().arch.opening(ArchId(0)).unwrap();
        assert!((opening.width - 1.5).abs() < 1e-9);
        assert!((opening.height - 1.0).abs() < 1e-9);
        assert!((opening.sill - 1.2).abs() < 1e-9);
        assert_eq!(opening.style, "casement");
        assert!((opening.rough_width - 1.62).abs() < 1e-9);
        assert!((opening.rough_height - 1.12).abs() < 1e-9);
    }

    #[test]
    fn edit_null_values_treated_as_not_provided() {
        let mut s = with_window();
        let err = s.execute("opening.edit", &json!({"id": "o0", "width": null})).unwrap_err();
        assert!(matches!(err, EngineError::BadParams { .. }));
        let opening = s.doc().unwrap().arch.opening(ArchId(0)).unwrap();
        assert!((opening.width - 1.2).abs() < 1e-9, "width should be unchanged");
    }

    #[test]
    fn edit_non_numeric_width_is_refused() {
        let mut s = with_window();
        let err = s.execute("opening.edit", &json!({"id": "o0", "width": "abc"})).unwrap_err();
        assert!(matches!(err, EngineError::BadParams { .. }));
        let opening = s.doc().unwrap().arch.opening(ArchId(0)).unwrap();
        assert!((opening.width - 1.2).abs() < 1e-9, "width should be unchanged");
    }

    #[test]
    fn edit_sill_only_keeps_rough_dimensions() {
        let mut s = with_window();
        let before = s.doc().unwrap().arch.opening(ArchId(0)).unwrap().clone();
        let _ = s.execute("opening.edit", &json!({"id": "o0", "sill": 1.5})).unwrap();
        let after = s.doc().unwrap().arch.opening(ArchId(0)).unwrap();
        assert!((after.sill - 1.5).abs() < 1e-9);
        assert!((after.rough_width - before.rough_width).abs() < 1e-9, "rough_width should be unchanged");
        assert!((after.rough_height - before.rough_height).abs() < 1e-9, "rough_height should be unchanged");
    }

    #[test]
    fn edit_style_only_keeps_rough_dimensions() {
        let mut s = with_window();
        let before = s.doc().unwrap().arch.opening(ArchId(0)).unwrap().clone();
        let _ = s.execute("opening.edit", &json!({"id": "o0", "style": "casement"})).unwrap();
        let after = s.doc().unwrap().arch.opening(ArchId(0)).unwrap();
        assert_eq!(after.style, "casement");
        assert!((after.rough_width - before.rough_width).abs() < 1e-9, "rough_width should be unchanged");
        assert!((after.rough_height - before.rough_height).abs() < 1e-9, "rough_height should be unchanged");
    }

    #[test]
    fn edit_zero_width_is_refused() {
        let mut s = with_window();
        let err = s.execute("opening.edit", &json!({"id": "o0", "width": 0.0})).unwrap_err();
        assert!(matches!(err, EngineError::BadParams { .. }));
        let opening = s.doc().unwrap().arch.opening(ArchId(0)).unwrap();
        assert!((opening.width - 1.2).abs() < 1e-9, "width should be unchanged");
    }

    #[test]
    fn edit_negative_sill_is_refused() {
        let mut s = with_window();
        let err = s.execute("opening.edit", &json!({"id": "o0", "sill": -0.1})).unwrap_err();
        assert!(matches!(err, EngineError::BadParams { .. }));
        let opening = s.doc().unwrap().arch.opening(ArchId(0)).unwrap();
        assert!((opening.sill - 0.9).abs() < 1e-9, "sill should be unchanged");
    }

    #[test]
    fn edit_reply_includes_revision() {
        let mut s = with_window();
        let r = s.execute("opening.edit", &json!({"id": "o0", "width": 1.5})).unwrap();
        let rev = r["revision"].as_u64().expect("revision should be a number");
        let d = s.doc().unwrap();
        assert_eq!(rev, d.arch_revision, "revision in response should match doc revision");
    }
}
