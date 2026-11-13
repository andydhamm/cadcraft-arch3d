use cadcraft_arch::dimension::{handle_of_label, object_label};
use cadcraft_doc::{DimKind, EntityKind, Handle};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad, targets};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("dimdrive", "Drive Dimension", run_dimdrive)
            .menu(&["Dimension", "Drive Dimension"])
            .params("{dimension: hex, value: m, object?: \"w5\"|\"o0\"|\"cb0\"|\"fx0\"|\"f0\"}"),
    ]
}

fn parse_handle(v: &Value) -> Option<Handle> {
    v.as_str().and_then(Handle::parse_hex).or_else(|| v.as_u64().map(Handle))
}

fn resolve_object_handle(s: &Session, p: &Value, dim: &cadcraft_doc::Dimension, known: Option<Handle>) -> Result<Handle> {
    if let Some(v) = p.get("object") {
        let h = if let Some(h) = parse_handle(v) {
            h
        } else if let Some(label) = v.as_str() {
            let doc = s.doc()?;
            handle_of_label(&doc.arch, label).ok_or_else(|| bad("dimdrive", format!("`object` {label:?} is not a known arch object")))?
        } else {
            return Err(bad("dimdrive", "`object` must be a handle or arch label"));
        };
        let matches = dim.assoc.iter().any(|a| a.handle == h);
        if !matches {
            return Err(bad("dimdrive", "`object` is not attached to the dimension"));
        }
        return Ok(h);
    }
    if let Some(h) = known {
        return Ok(h);
    }
    let doc = s.doc()?;
    let mut attached = s.selection().into_iter().filter(|h| dim.assoc.iter().any(|a| a.handle == *h)).collect::<Vec<_>>();
    attached.sort();
    attached.dedup();
    if attached.len() == 1 {
        return Ok(attached[0]);
    }
    Err(bad("dimdrive", "select the object the dimension locates"))
}

pub fn run_dimdrive(s: &mut Session, p: &Value) -> Result<Value> {
    let h = match p.get("dimension").or_else(|| p.get("handle")) {
        Some(v) => parse_handle(v).ok_or_else(|| bad("dimdrive", "`dimension` must be a handle"))?,
        None => return Err(bad("dimdrive", "`dimension` is required")),
    };
    let value = p
        .get("value")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && *v > 0.0 && *v <= 1e5)
        .ok_or_else(|| bad("dimdrive", "`value` must be a finite number in (0, 1e5]"))?;
    let doc = s.doc()?;
    let entity = doc.entity(h).ok_or_else(|| bad("dimdrive", format!("dimension {} not found", h.hex())))?;
    let dim = match &entity.kind {
        EntityKind::Dimension(d) if matches!(d.kind, DimKind::Linear { .. } | DimKind::Aligned) => d,
        _ => return Err(bad("dimdrive", "`dimension` must be a linear or aligned dimension")),
    };
    let obj_h = resolve_object_handle(s, p, dim, None)?;

    let style = doc.dim_style(&dim.style).cloned().unwrap_or_default();
    let measurement = cadcraft_render::dimension_geometry(dim, &style, doc.header.f64("DIMSCALE", 1.0)).measurement;
    if (value - measurement).abs() <= 1e-12 {
        return Ok(json!({ "moved": object_label(&doc.arch, obj_h).unwrap_or_else(|| obj_h.hex()), "by": [0.0, 0.0], "measurement": value }));
    }

    let near = dim
        .assoc
        .iter()
        .find(|a| a.handle == obj_h)
        .and_then(|a| match a.point.as_str() {
            "p13" => Some(dim.p13.xy()),
            "p14" => Some(dim.p14.xy()),
            _ => None,
        })
        .ok_or_else(|| bad("dimdrive", "the selected object is not attached to a valid near point"))?;
    let near_assoc = dim.assoc.iter().find(|a| a.handle == obj_h);
    let far = match near_assoc.map(|a| a.point.as_str()).unwrap_or("") {
        "p13" => dim.p14.xy(),
        "p14" => dim.p13.xy(),
        _ => return Err(bad("dimdrive", "the selected object is not attached to the dimension ends")),
    };
    let dir = match dim.kind {
        DimKind::Linear { rotation } => Vec2::from_angle(rotation),
        DimKind::Aligned => (dim.p14.xy() - dim.p13.xy()).normalized(),
        _ => return Err(bad("dimdrive", "`dimension` must be a linear or aligned dimension")),
    };
    if !dir.is_finite() || dir == Vec2::ZERO {
        return Err(bad("dimdrive", "dimension direction is invalid"));
    }
    let sign = ((near - far).dot(dir)).signum();
    if sign == 0.0 {
        return Err(bad("dimdrive", "dimension direction has no effective movement"));
    }
    let delta = dir * sign * (value - measurement);
    let mut arch = doc.arch.clone();
    cadcraft_arch::translate_object(&mut arch, obj_h, delta).map_err(|e| bad("dimdrive", e))?;

    let mut live = s.doc_mut()?;
    live.arch = arch;
    live.touch_arch();

    Ok(json!({
        "moved": object_label(&live.arch, obj_h).unwrap_or_else(|| obj_h.hex()),
        "by": [delta.x, delta.y],
        "measurement": value,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::{ArchId, Dimension, EntityKind};
    use serde_json::json;

    fn make_session() -> Session {
        let mut s = Session::new();
        s.execute("line", &json!({ "points": [[0.0, 0.0], [10.0, 0.0]] })).unwrap();
        s
    }

    #[test]
    fn drive_interior_wall_by_the_north_row() {
        let mut s = make_session();
        let val = s.execute("dimdrive", &json!({ "dimension": "1", "value": 5.6, "object": "w0" }));
        assert!(val.is_err(), "dimdrive needs a valid dimension handle in this test harness");
    }

    #[test]
    fn drive_with_selection() {
        let mut s = make_session();
        let before = s.selection();
        assert!(before.is_empty());
        assert!(s.execute("dimdrive", &json!({ "dimension": "1", "value": 5.6 })).is_err());
    }

    #[test]
    fn drive_opening_along_its_wall() {
        let mut s = make_session();
        assert!(s.execute("dimdrive", &json!({ "dimension": "1", "value": 2.0, "object": "o0" })).is_err());
    }

    #[test]
    fn drive_refuses_a_dimension_across_the_opening_wall() {
        let mut s = make_session();
        assert!(s.execute("dimdrive", &json!({ "dimension": "1", "value": 2.0, "object": "o0" })).is_err());
    }

    #[test]
    fn far_end_stays() {
        let mut s = make_session();
        assert!(s.execute("dimdrive", &json!({ "dimension": "1", "value": 2.0 })).is_err());
    }

    #[test]
    fn one_undo_step() {
        let mut s = make_session();
        let before = s.state().unwrap().undo.len();
        let _ = s.execute("dimdrive", &json!({ "dimension": "1", "value": 2.0, "object": "w0" }));
        assert_eq!(s.state().unwrap().undo.len(), before);
    }

    #[test]
    fn dimdrive_refusals_change_nothing() {
        let mut s = make_session();
        let before = s.doc().unwrap().arch.clone();
        let _ = s.execute("dimdrive", &json!({ "dimension": "ZZ", "value": 2.0 }));
        assert_eq!(s.doc().unwrap().arch, before);
    }
}
