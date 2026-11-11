//! `arch.inspect {"countertops": true}` (phase 18): one row per counter outline. Read-only.

use cadcraft_arch::countertop::{self, CounterKey};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{COUNTER_LAYER, CounterEdge, cabinet_label, countertop_handle, countertop_label, handle_json};
use serde_json::{Map, Value, json};

/// Add the `countertops` rows to `out` when `p` asks for them.
pub(crate) fn inspect_countertops(model: &ArchModel, p: &Value, out: &mut Map<String, Value>) {
    if p.get("countertops").and_then(Value::as_bool) != Some(true) {
        return;
    }
    let rows: Vec<Value> = countertop::outlines(model).iter().map(|o| row(model, o)).collect();
    out.insert("countertops".to_string(), Value::Array(rows));
}

fn row(model: &ArchModel, o: &countertop::CounterOutline) -> Value {
    let custom = match o.key {
        Some(CounterKey::Custom(id)) => model.countertop(id),
        _ => None,
    };
    let (id, auto, handle) = match o.key {
        Some(CounterKey::Auto(c)) => (cabinet_label(c), true, Value::Null),
        Some(CounterKey::Custom(c)) => (countertop_label(c), false, Value::String(handle_json(countertop_handle(c)))),
        None => (String::new(), true, Value::Null),
    };
    let n = o.ring.len();
    let edges: Vec<Value> = (0..n)
        .filter_map(|i| {
            let a = o.ring.get(i)?;
            let b = o.ring.get((i + 1) % n)?;
            let length = (*b - *a).len();
            let s = o.sagittas.get(i).copied().unwrap_or(0.0);
            let radius = (s != 0.0 && s.is_finite()).then(|| (length * length / 4.0 + s * s) / (2.0 * s.abs()));
            let e = custom.map_or_else(CounterEdge::default, |c| c.edge(i));
            Some(json!({
                "index": i, "from": [a.x, a.y], "to": [b.x, b.y], "length": length, "sagitta": s,
                "radius": radius, "profile": e.profile.key(), "waterfall": e.waterfall,
            }))
        })
        .collect();
    let base = model.level(o.level).map_or(0.0, |l| l.elevation);
    json!({
        "id": id, "auto": auto, "handle": handle, "level": o.level,
        "cabinets": o.cabinets.iter().map(|c| cabinet_label(*c)).collect::<Vec<_>>(),
        "polygon": o.ring.iter().map(|p| json!([p.x, p.y])).collect::<Vec<_>>(),
        "edges": edges, "top": base + o.top, "thickness": o.thickness,
        "area": countertop::outline_area(o),
        "holes": o.holes.iter().map(|h| h.iter().map(|p| json!([p.x, p.y])).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "layer": custom.map_or(COUNTER_LAYER, |c| c.layer.as_str()),
    })
}
