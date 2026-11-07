//! The electrical part of `arch.inspect` (phase 15): `{"electrical":true}` adds the devices,
//! connections, circuits and outlet gaps; `{"lightSets":true}` adds the named light sets.
//! Read-only and deterministic: rows follow the model's own order.

use cadcraft_arch::{connections, electrical, outlets};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{
    WallSide, connection_handle, connection_label, electrical_handle, electrical_label, handle_json, light_label, room_label, wall_label,
};
use serde_json::{Map, Value, json};

fn flag(p: &Value, key: &str) -> bool {
    p.get(key).and_then(Value::as_bool) == Some(true)
}

/// Add `electrical`, `connections`, `circuits` and `outletGaps` (when `{"electrical":true}`)
/// and `lightSets` (when `{"lightSets":true}`) to `out`. Read-only.
pub(crate) fn inspect_electrical(model: &ArchModel, p: &Value, out: &mut Map<String, Value>) {
    if flag(p, "electrical") {
        let devices: Vec<Value> = model
            .electrical
            .iter()
            .map(|d| {
                let place = electrical::place_device(model, d).ok();
                json!({
                    "id": electrical_label(d.id),
                    "handle": handle_json(electrical_handle(d.id)),
                    "kind": d.kind.key(),
                    "layer": d.kind.layer(),
                    "wall": wall_label(d.wall),
                    "side": match d.side {
                        WallSide::Left => "left",
                        WallSide::Right => "right",
                    },
                    "offset": d.offset,
                    "at": place.as_ref().map_or(Value::Null, |pl| json!([pl.at.x, pl.at.y])),
                    "z": place.as_ref().map_or(Value::Null, |pl| json!(pl.z)),
                    "level": d.level,
                    "room": electrical::device_room(model, d).map_or(Value::Null, |r| Value::String(room_label(r))),
                })
            })
            .collect();
        out.insert("electrical".into(), Value::Array(devices));

        let conns: Vec<Value> = model
            .connections
            .iter()
            .map(|c| {
                json!({
                    "id": connection_label(c.id),
                    "handle": handle_json(connection_handle(c.id)),
                    "from": electrical_label(c.from),
                    "to": c.to.iter().map(|t| light_label(*t)).collect::<Vec<_>>(),
                    "bulge": c.bulge,
                })
            })
            .collect();
        out.insert("connections".into(), Value::Array(conns));

        let circuits: Vec<Value> = connections::circuits(model)
            .iter()
            .map(|c| {
                json!({
                    "switches": c.switches.iter().map(|s| electrical_label(*s)).collect::<Vec<_>>(),
                    "lights": c.lights.iter().map(|l| light_label(*l)).collect::<Vec<_>>(),
                    "brightness": c.brightness,
                })
            })
            .collect();
        out.insert("circuits".into(), Value::Array(circuits));

        let mut gaps_rows = Vec::new();
        for r in &model.rooms {
            if outlets::usable_runs(model, r.id).is_empty() {
                continue;
            }
            let g = outlets::outlet_gaps(model, r.id);
            let max_gap = if g.max_gap.is_finite() { json!(g.max_gap) } else { Value::Null };
            let counter = g
                .counter
                .as_ref()
                .map_or(Value::Null, |c| json!({ "maxSpacing": c.max_spacing, "startDistance": c.start_distance, "endDistance": c.end_distance }));
            gaps_rows.push(json!({ "room": room_label(r.id), "maxGap": max_gap, "counter": counter }));
        }
        out.insert("outletGaps".into(), Value::Array(gaps_rows));
    }
    if flag(p, "lightSets") {
        out.insert("lightSets".into(), Value::Array(model.light_sets.iter().map(super::lightset::light_set_json).collect()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use cadcraft_arch::room::detect_rooms;
    use cadcraft_doc::arch::{Connection, Wall};
    use cadcraft_geom::Vec2;

    fn room64(s: &mut Session) {
        let doc = s.doc_mut().unwrap();
        let m = &mut doc.arch;
        for (a, b) in [((-0.1, -0.1), (6.1, -0.1)), ((6.1, -0.1), (6.1, 4.1)), ((6.1, 4.1), (-0.1, 4.1)), ((-0.1, 4.1), (-0.1, -0.1))] {
            m.push_wall(Wall { start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness: 0.2, ..Default::default() });
        }
        m.rooms = detect_rooms(m);
    }

    fn inspect(s: &mut Session, p: Value) -> Value {
        s.execute("arch.inspect", &p).unwrap()
    }

    #[test]
    fn inspect_lists_devices_connections_and_circuits() {
        let mut s = Session::new();
        room64(&mut s);
        s.execute("lightfixture", &json!({ "room": "r0", "rows": 1, "cols": 2 })).unwrap();
        s.execute("switch", &json!({ "kind": "three-way", "wall": "w0", "offset": 1.0, "side": "left" })).unwrap();
        s.execute("switch", &json!({ "kind": "three-way", "wall": "w0", "offset": 2.0, "side": "left" })).unwrap();
        s.execute("outlet", &json!({ "kind": "duplex", "wall": "w0", "offset": 4.0, "side": "left" })).unwrap();
        {
            let doc = s.doc_mut().unwrap();
            for from in [0, 1] {
                doc.arch.push_connection(Connection {
                    from: cadcraft_doc::ArchId(from),
                    to: vec![cadcraft_doc::ArchId(0), cadcraft_doc::ArchId(1)],
                    bulge: 0.25,
                    ..Connection::default()
                });
            }
        }
        let r = inspect(&mut s, json!({ "electrical": true }));
        let devs = r["electrical"].as_array().unwrap();
        let ids: Vec<_> = devs.iter().map(|d| d["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["el0", "el1", "el2"]);
        assert_eq!(devs[0]["layer"], "E-LITE");
        assert_eq!(devs[1]["layer"], "E-LITE");
        assert_eq!(devs[2]["layer"], "E-POWR");
        assert!(devs.iter().all(|d| !d["room"].is_null()));
        let cons = r["connections"].as_array().unwrap();
        assert_eq!(cons.len(), 2);
        assert_eq!(cons[0]["id"], "cn0");
        assert_eq!(cons[1]["id"], "cn1");
        assert_eq!(cons[1]["from"], "el1");
        assert_eq!(cons[0]["to"], json!(["lf0", "lf1"]));
        assert_eq!(cons[0]["bulge"], 0.25);
        let circuits = r["circuits"].as_array().unwrap();
        assert_eq!(circuits.len(), 1);
        assert_eq!(circuits[0]["switches"], json!(["el0", "el1"]));
        assert_eq!(circuits[0]["lights"], json!(["lf0", "lf1"]));
        let none = inspect(&mut s, json!({}));
        for k in ["electrical", "connections", "circuits", "outletGaps"] {
            assert!(none.get(k).is_none(), "{k}");
        }
    }

    #[test]
    fn inspect_reports_outlet_gaps() {
        let mut s = Session::new();
        room64(&mut s);
        s.execute("door", &json!({ "wall": "w0", "offset": 3.0, "width": 0.9 })).unwrap();
        s.execute("outlets.auto", &json!({ "room": "r0" })).unwrap();
        let r = inspect(&mut s, json!({ "electrical": true }));
        let rows = r["outletGaps"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["room"], "r0");
        assert!(rows[0]["maxGap"].as_f64().unwrap() <= 1.8 + 1e-6);
        assert!(rows[0]["counter"].is_null());
        s.execute("room", &json!({ "id": "r0", "type": "kitchen" })).unwrap();
        s.execute("outlets.auto", &json!({ "room": "r0" })).unwrap();
        let r = inspect(&mut s, json!({ "electrical": true }));
        let counter = &r["outletGaps"][0]["counter"];
        assert!(!counter.is_null());
        assert!(counter["maxSpacing"].as_f64().unwrap() <= 1.2 + 1e-6);
        // A room whose walls are gone has no usable wall and no row.
        s.doc_mut().unwrap().arch.walls.clear();
        let r = inspect(&mut s, json!({ "electrical": true }));
        assert!(r["outletGaps"].as_array().unwrap().is_empty());
    }

    #[test]
    fn inspect_lists_light_sets() {
        let mut s = Session::new();
        room64(&mut s);
        s.execute("lightfixture", &json!({ "room": "r0", "rows": 1, "cols": 2 })).unwrap();
        s.execute("lightset", &json!({ "name": "night", "lights": { "lf0": { "on": false, "brightness": 0.25 } } })).unwrap();
        s.execute("lightset", &json!({ "name": "day", "lights": { "lf0": { "on": true }, "lf1": { "on": true, "brightness": 0.5 } } })).unwrap();
        let r = inspect(&mut s, json!({ "lightSets": true }));
        let sets = r["lightSets"].as_array().unwrap();
        let names: Vec<_> = sets.iter().map(|v| v["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"night") && names.contains(&"day"), "{names:?}");
        let night = sets.iter().find(|v| v["name"] == "night").unwrap();
        assert_eq!(night["lights"]["lf0"]["on"], false);
        assert_eq!(night["lights"]["lf0"]["brightness"], 0.25);
        let day = sets.iter().find(|v| v["name"] == "day").unwrap();
        assert_eq!(day["lights"]["lf1"]["brightness"], 0.5);
        assert!(inspect(&mut s, json!({})).get("lightSets").is_none());
    }
}
