//! `arch.inspect {"cabinets": true}` rows' `corner` (phase 18): null, or the style, end, leg, partner
//! and footprint of a corner cabinet; with `"symbols": true` each row also carries its plan symbol.
//! Read-only.

use cadcraft_arch::casework_corner::{CornerEnd, CornerStyle, corner_footprint, corner_info};
use cadcraft_arch::casework_symbol::cabinet_symbol_in;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::cabinet_label;
use serde_json::{Map, Value, json};

fn points(ring: &[cadcraft_geom::Vec2]) -> Value {
    Value::Array(ring.iter().map(|p| json!([p.x, p.y])).collect())
}

/// Add `corner` (and, with `symbols`, `symbol`) to the `cabinets` rows already in `out`.
pub(crate) fn inspect_cabinet_corners(model: &ArchModel, p: &Value, out: &mut Map<String, Value>) {
    let symbols = p.get("symbols").and_then(Value::as_bool) == Some(true);
    let Some(Value::Array(rows)) = out.get_mut("cabinets") else { return };
    for (row, c) in rows.iter_mut().zip(model.cabinets.iter()) {
        let Value::Object(obj) = row else { continue };
        let corner = corner_info(model, c).map_or(Value::Null, |info| {
            json!({
                "style": match info.style {
                    CornerStyle::Blind => "blind",
                    CornerStyle::Diagonal => "diagonal",
                    CornerStyle::LazySusan => "lazySusan",
                },
                "end": match info.end {
                    CornerEnd::Start => "start",
                    CornerEnd::End => "end",
                },
                "leg": info.leg,
                "partner": cabinet_label(info.partner),
                "footprint": points(&corner_footprint(c, &info)),
            })
        });
        obj.insert("corner".to_string(), corner);
        if symbols {
            let rings: Vec<Value> = cabinet_symbol_in(model, c).iter().map(|r| points(r)).collect();
            obj.insert("symbol".to_string(), Value::Array(rings));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_arch::casework::cabinet_defaults;
    use cadcraft_doc::arch::{Cabinet, CabinetKind};
    use cadcraft_geom::Vec2;

    fn base(x: f64, y: f64, rotation: f64, width: f64) -> Cabinet {
        Cabinet { position: Vec2::new(x, y), rotation, width, ..cabinet_defaults(CabinetKind::Base) }
    }

    fn inspect(s: &mut crate::Session, symbols: bool) -> Vec<Value> {
        let v = s.execute("arch.inspect", &json!({ "cabinets": true, "symbols": symbols })).unwrap_or(Value::Null);
        v["cabinets"].as_array().cloned().unwrap_or_default()
    }

    #[test]
    fn inspect_corner_rows_and_symbols() {
        let mut s = crate::Session::new();
        if let Ok(mut d) = s.doc_mut() {
            // cb0 along the south wall from the corner; cb1 down the west wall, butting against it.
            d.arch.push_cabinet(base(0.45, 0.0, 0.0, 0.9));
            d.arch.push_cabinet(base(0.0, 0.9, 270.0, 0.6));
            d.touch_arch();
        }
        let rows = inspect(&mut s, false);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.get("symbol").is_none()), "symbols only on request");
        let co = &rows[0]["corner"];
        assert_eq!(co["style"], "blind", "{co}");
        assert_eq!(co["end"], "start");
        assert_eq!(co["partner"], "cb1");
        assert!((co["leg"].as_f64().unwrap_or(0.0) - 0.6).abs() < 1e-9);
        assert_eq!(co["footprint"].as_array().map(Vec::len), Some(4));
        assert!(rows[1]["corner"].is_null(), "the partner is no corner cabinet");

        let rows = inspect(&mut s, true);
        let sym = rows[0]["symbol"].as_array().cloned().unwrap_or_default();
        assert!(sym.len() >= 3, "{sym:?}");
        assert_eq!(sym[0].as_array().map(Vec::len), Some(5), "the closed footprint comes first");
        assert!(rows[1]["symbol"].is_array());

        // A lazy susan answers with its L: the other run starts further out.
        if let Ok(mut d) = s.doc_mut() {
            d.arch.cabinets[1].position = Vec2::new(0.0, 1.2);
            d.touch_arch();
        }
        let rows = inspect(&mut s, true);
        let co = &rows[0]["corner"];
        assert_eq!((co["style"].as_str(), co["footprint"].as_array().map(Vec::len)), (Some("lazySusan"), Some(6)), "{co}");
        let sym = rows[0]["symbol"].as_array().cloned().unwrap_or_default();
        assert!(sym.iter().any(|r| r.as_array().map(Vec::len) == Some(25)), "the lazy susan's circle");

        // `corner: none` clears it.
        let r = s.execute("cabinet.edit", &json!({ "id": "cb0", "corner": "none" }));
        assert!(r.is_ok(), "{r:?}");
        assert!(inspect(&mut s, false)[0]["corner"].is_null());
    }
}
