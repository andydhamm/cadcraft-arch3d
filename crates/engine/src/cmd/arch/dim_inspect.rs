use cadcraft_arch::dimension::object_label;
use cadcraft_doc::{ArchFeature, AssocSnap, DimKind, Dimension, Drawing, EntityKind};
use serde_json::{Map as JsonMap, Value, json};

pub(crate) fn measurement(d: &Drawing, dm: &Dimension) -> f64 {
    let style = d.dim_style(&dm.style).cloned().unwrap_or_default();
    cadcraft_render::dimension_geometry(dm, &style, d.header.f64("DIMSCALE", 1.0)).measurement
}

fn kind_name(dm: &Dimension) -> &'static str {
    match dm.kind {
        DimKind::Linear { .. } => "linear",
        DimKind::Aligned => "aligned",
        DimKind::Angular => "angular",
        DimKind::Angular3P => "angular3p",
        DimKind::Diameter => "diameter",
        DimKind::Radius => "radius",
        DimKind::Ordinate { .. } => "ordinate",
        DimKind::ArcLength => "arcLength",
    }
}

fn rotation_deg(dm: &Dimension) -> Option<f64> {
    match dm.kind {
        DimKind::Linear { rotation } => Some(rotation.to_degrees()),
        _ => None,
    }
}

fn snap_feature_name(snap: &AssocSnap) -> &'static str {
    match snap {
        AssocSnap::Start => "start",
        AssocSnap::End => "end",
        AssocSnap::Mid => "mid",
        AssocSnap::Center => "center",
        AssocSnap::OnCircle { .. } => "onCircle",
        AssocSnap::Intersection { .. } => "intersection",
        AssocSnap::Vertex { .. } => "vertex",
        AssocSnap::Arch { feature, .. } => feature.key(),
    }
}

fn snap_edge(snap: &AssocSnap) -> Option<u32> {
    match snap {
        AssocSnap::Arch { feature: ArchFeature::Edge(i), .. } => Some(*i),
        _ => None,
    }
}

fn snap_along(snap: &AssocSnap) -> Option<f64> {
    match snap {
        AssocSnap::Arch { along, .. } => Some(*along),
        _ => None,
    }
}

pub(crate) fn inspect_dimensions(d: &Drawing, p: &Value, out: &mut JsonMap<String, Value>) {
    if p.get("dimensions").and_then(Value::as_bool) != Some(true) {
        return;
    }

    let mut rows = Vec::new();
    for e in d.model.iter() {
        let EntityKind::Dimension(dm) = &e.kind else { continue };
        if rows.len() >= 100_000 {
            break;
        }

        let mut attached = Vec::new();
        for a in &dm.assoc {
            if let AssocSnap::Arch { .. } = &a.snap {
                let Some(object) = object_label(&d.arch, a.handle) else {
                    continue;
                };
                let mut row = JsonMap::new();
                row.insert("point".to_string(), json!(a.point));
                row.insert("object".to_string(), json!(object));
                row.insert("feature".to_string(), json!(snap_feature_name(&a.snap)));
                row.insert("edge".to_string(), snap_edge(&a.snap).map_or(Value::Null, Value::from));
                row.insert("along".to_string(), snap_along(&a.snap).map_or(Value::Null, |n| json!(n)));
                attached.push(Value::Object(row));
                continue;
            }

            let mut row = JsonMap::new();
            row.insert("point".to_string(), json!(a.point));
            row.insert("object".to_string(), json!(a.handle.hex()));
            row.insert("feature".to_string(), json!(snap_feature_name(&a.snap)));
            row.insert("edge".to_string(), Value::Null);
            row.insert("along".to_string(), Value::Null);
            attached.push(Value::Object(row));
        }

        let p13 = dm.p13.xy();
        let p14 = dm.p14.xy();
        let defpt = dm.defpt.xy();
        rows.push(json!({
            "handle": e.handle.hex(),
            "kind": kind_name(dm),
            "rotation": rotation_deg(dm),
            "p1": [p13.x, p13.y],
            "p2": [p14.x, p14.y],
            "at": [defpt.x, defpt.y],
            "measurement": measurement(d, dm),
            "layer": e.common.layer,
            "style": dm.style,
            "text": dm.text,
            "attached": attached,
        }));
    }
    out.insert("dimensions".to_string(), Value::Array(rows));
}

pub(crate) fn inspect_auto_dims(d: &Drawing, p: &Value, out: &mut JsonMap<String, Value>) {
    let mut rows = Vec::new();
    for rec in &d.arch.auto_dims {
        for row in &rec.generated {
            let mut values = Vec::new();
            for handle in &row.handles {
                let Some(entity) = d.entity(*handle) else { continue };
                let EntityKind::Dimension(dm) = &entity.kind else { continue };
                values.push(measurement(d, dm));
            }
            rows.push(json!({
                "level": rec.level,
                "side": row.side,
                "row": row.kind.key(),
                "edited": row.edited,
                "handles": row.handles.iter().map(|h| h.hex()).collect::<Vec<_>>(),
                "values": values,
            }));
        }
    }
    rows.sort_by(|a, b| {
        let (Some(a_obj), Some(b_obj)) = (a.as_object(), b.as_object()) else {
            return std::cmp::Ordering::Equal;
        };
        let a_level = a_obj.get("level").and_then(Value::as_u64).unwrap_or(0) as u32;
        let b_level = b_obj.get("level").and_then(Value::as_u64).unwrap_or(0) as u32;
        let side_order = |s: &str| match s {
            "south" => 0,
            "east" => 1,
            "north" => 2,
            "west" => 3,
            _ => 4,
        };
        let a_side = a_obj.get("side").and_then(Value::as_str).unwrap_or("south");
        let b_side = b_obj.get("side").and_then(Value::as_str).unwrap_or("south");
        let row_rank = |kind: &str| match kind {
            "openings" => 0,
            "walls" => 1,
            "overall" => 2,
            _ => 3,
        };
        let a_row = a_obj.get("row").and_then(Value::as_str).unwrap_or("");
        let b_row = b_obj.get("row").and_then(Value::as_str).unwrap_or("");
        a_level.cmp(&b_level).then(side_order(a_side).cmp(&side_order(b_side))).then(row_rank(a_row).cmp(&row_rank(b_row)))
    });
    out.insert("autoDims".to_string(), Value::Array(rows));
}
