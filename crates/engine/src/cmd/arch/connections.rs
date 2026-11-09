//! `connection`, `connection.edit`, `connection.delete` and `circuit.brightness`.

use cadcraft_arch::connections::{check_connection, circuit_of};
use cadcraft_doc::arch::{
    ArchId, Connection, connection_handle, connection_label, electrical_label, light_label, parse_connection_label, parse_electrical_label,
};
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, bad, str_param};
use crate::{Result, Session};

fn parse_connection_id(raw: &str, cmd: &str) -> Result<ArchId> {
    parse_connection_label(raw).ok_or_else(|| bad(cmd, format!("unknown connection `{raw}`")))
}

fn parse_light_ids(doc: &cadcraft_doc::Drawing, raw: &Value, cmd: &str) -> Result<Vec<ArchId>> {
    let list = raw.as_array().filter(|v| !v.is_empty()).ok_or_else(|| bad(cmd, "`to` must be a non-empty array of light labels"))?;
    let mut ids = Vec::with_capacity(list.len());
    let mut seen = std::collections::BTreeSet::new();
    for item in list {
        let text = item.as_str().filter(|s| !s.trim().is_empty()).ok_or_else(|| bad(cmd, "every light id in `to` must be a string"))?;
        let digits = text
            .strip_prefix("lf")
            .or_else(|| text.strip_prefix("LF"))
            .or_else(|| text.strip_prefix("Lf"))
            .or_else(|| text.strip_prefix("lF"))
            .or_else(|| text.strip_prefix('l'))
            .or_else(|| text.strip_prefix('L'))
            .unwrap_or(text);
        let id = digits.parse::<u32>().map(ArchId).map_err(|_| bad(cmd, format!("unknown light `{text}`")))?;
        if !doc.arch.lights.iter().any(|f| f.id == id) {
            return Err(bad(cmd, format!("light `{text}` does not exist")));
        }
        if !seen.insert(id) {
            return Err(bad(cmd, format!("light `{text}` is listed more than once")));
        }
        ids.push(id);
    }
    Ok(ids)
}

fn run_connection(session: &mut Session, params: &Value) -> Result<Value> {
    let cmd = "connection";
    let raw_from = str_param(params, "from").ok_or_else(|| bad(cmd, "`from` is required"))?;
    let from = parse_electrical_label(raw_from).ok_or_else(|| bad(cmd, format!("unknown electrical id `{raw_from}`")))?;
    let doc = session.doc()?.clone();
    let to = parse_light_ids(&doc, params.get("to").ok_or_else(|| bad(cmd, "`to` is required"))?, cmd)?;
    let bulge = if let Some(v) = params.get("bulge").filter(|value| !value.is_null()) {
        let b = v.as_f64().filter(|value| value.is_finite()).ok_or_else(|| bad(cmd, "`bulge` must be a finite number"))?;
        if !(-1.0..=1.0).contains(&b) {
            return Err(bad(cmd, "`bulge` must be in the range [-1, 1]"));
        }
        b
    } else {
        0.0
    };
    check_connection(&doc.arch, from, &to, bulge).map_err(|e| bad(cmd, e))?;
    let mut doc_mut = session.doc_mut()?;
    let id = doc_mut.arch.push_connection(Connection { from, to: to.clone(), bulge, ..Default::default() });
    let handle = cadcraft_doc::arch::handle_json(connection_handle(id));
    Ok(
        json!({ "connection": connection_label(id), "handle": handle, "from": electrical_label(from), "to": to.iter().map(|light| light_label(*light)).collect::<Vec<_>>(), "bulge": bulge }),
    )
}

fn run_connection_edit(session: &mut Session, params: &Value) -> Result<Value> {
    let cmd = "connection.edit";
    let raw = str_param(params, "id").ok_or_else(|| bad(cmd, "`id` is required"))?;
    let id = parse_connection_id(raw, cmd)?;
    let mut doc = session.doc_mut()?;
    let idx = doc.arch.connections.iter().position(|c| c.id == id).ok_or_else(|| bad(cmd, format!("unknown connection `{raw}`")))?;
    let mut connection = doc.arch.connections[idx].clone();
    if let Some(raw_to) = params.get("to") {
        if !raw_to.is_null() {
            let to = parse_light_ids(&doc.clone(), raw_to, cmd)?;
            connection.to = to;
        }
    }
    if let Some(raw_bulge) = params.get("bulge") {
        if !raw_bulge.is_null() {
            let bulge = raw_bulge.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`bulge` must be a finite number"))?;
            if !(-1.0..=1.0).contains(&bulge) {
                return Err(bad(cmd, "`bulge` must be in the range [-1, 1]"));
            }
            connection.bulge = bulge;
        }
    }
    check_connection(&doc.arch, connection.from, &connection.to, connection.bulge).map_err(|e| bad(cmd, e))?;
    doc.arch.connections[idx] = connection.clone();
    let handle = cadcraft_doc::arch::handle_json(connection_handle(id));
    Ok(
        json!({ "connection": connection_label(id), "handle": handle, "from": electrical_label(connection.from), "to": connection.to.iter().map(|light| light_label(*light)).collect::<Vec<_>>(), "bulge": connection.bulge }),
    )
}

fn run_connection_delete(session: &mut Session, params: &Value) -> Result<Value> {
    let cmd = "connection.delete";
    let raw = str_param(params, "id").ok_or_else(|| bad(cmd, "`id` is required"))?;
    let id = parse_connection_id(raw, cmd)?;
    let mut doc = session.doc_mut()?;
    let idx = doc.arch.connections.iter().position(|c| c.id == id).ok_or_else(|| bad(cmd, format!("unknown connection `{raw}`")))?;
    doc.arch.connections.remove(idx);
    Ok(json!({ "deleted": connection_label(id) }))
}

fn run_circuit_brightness(session: &mut Session, params: &Value) -> Result<Value> {
    let cmd = "circuit.brightness";
    let raw_switch = str_param(params, "switch").ok_or_else(|| bad(cmd, "`switch` is required"))?;
    let switch_id = parse_electrical_label(raw_switch).ok_or_else(|| bad(cmd, format!("unknown electrical id `{raw_switch}`")))?;
    let brightness = params.get("brightness").ok_or_else(|| bad(cmd, "`brightness` is required"))?;
    let brightness = brightness.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, "`brightness` must be a finite number"))?;
    if !(0.0..=1.0).contains(&brightness) {
        return Err(bad(cmd, "`brightness` must be in the range [0, 1]"));
    }
    let mut doc = session.doc_mut()?;
    let switch = doc.arch.electrical.iter().find(|d| d.id == switch_id).ok_or_else(|| bad(cmd, format!("switch `{raw_switch}` does not exist")))?;
    if !switch.kind.is_switch() {
        return Err(bad(cmd, format!("device `{raw_switch}` is not a switch")));
    }
    let circuit = circuit_of(&doc.arch, switch_id).ok_or_else(|| bad(cmd, format!("switch `{raw_switch}` is not part of a circuit")))?;
    for light_id in &circuit.lights {
        if let Some(light) = doc.arch.lights.iter_mut().find(|fixture| fixture.id == *light_id) {
            light.brightness = brightness;
        }
    }
    Ok(json!({ "lights": circuit.lights.iter().map(|id| light_label(*id)).collect::<Vec<_>>(), "brightness": brightness }))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("connection", "Connection", run_connection)
            .menu(&["Architecture", "Electrical", "Connection"])
            .params("{from: \"el0\", to: [\"lf0\", ...], bulge?: -1..1}"),
        CommandSpec::new("connection.edit", "Edit Connection", run_connection_edit)
            .menu(&["Architecture", "Electrical", "Edit Connection"])
            .params("{id: \"cn0\", to?: [\"lf0\", ...], bulge?: -1..1}"),
        CommandSpec::new("connection.delete", "Delete Connection", run_connection_delete)
            .menu(&["Architecture", "Electrical", "Delete Connection"])
            .params("{id: \"cn0\"}"),
        CommandSpec::new("circuit.brightness", "Circuit Brightness", run_circuit_brightness)
            .menu(&["Architecture", "Electrical", "Circuit Brightness"])
            .params("{switch: \"el0\", brightness: 0..=1}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ElectricalDevice, ElectricalKind, LightFixture, WallSide};
    use cadcraft_doc::{ArchId, Wall};
    use cadcraft_geom::Vec2;
    use serde_json::json;

    fn make_switch_and_lights() -> (crate::Session, ArchId, Vec<ArchId>) {
        let mut s = crate::Session::empty();
        s.new_drawing(false);
        let mut doc = s.doc_mut().unwrap();
        let wall = doc.arch.push_wall(Wall::new(Vec2::ZERO, Vec2::new(6.0, 0.0), 0.2));
        let switch_id = doc.arch.push_electrical(ElectricalDevice {
            wall,
            kind: ElectricalKind::SinglePole,
            side: WallSide::Left,
            offset: 1.0,
            height: 1.2,
            ..Default::default()
        });
        let mut light_ids = Vec::new();
        for i in 0..2 {
            let id = ArchId(i as u32);
            doc.arch.lights.push(LightFixture::new(id, "Recessed pot light", Vec2::new(2.0 + i as f64, 0.1)));
            light_ids.push(id);
        }
        drop(doc);
        (s, switch_id, light_ids)
    }

    #[test]
    fn a_connection_needs_a_switch_and_lights() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        let mut doc = s.doc_mut().unwrap();
        let outlet = doc.arch.push_electrical(ElectricalDevice {
            wall: doc.arch.walls[0].id,
            kind: ElectricalKind::Duplex,
            side: WallSide::Right,
            offset: 2.0,
            height: 0.3,
            ..Default::default()
        });
        drop(doc);
        let err = s.execute("connection", &json!({ "from": electrical_label(outlet), "to": [light_label(light_ids[0])] })).unwrap_err();
        assert!(err.to_string().contains("switch") || err.to_string().contains("connection"));
        assert!(s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [] })).is_err());
        assert!(s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": ["lf99"] })).is_err());
    }

    #[test]
    fn circuit_brightness_dims_every_light_on_the_circuit() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [light_label(light_ids[0]), light_label(light_ids[1])] }))
            .unwrap();
        let result = s.execute("circuit.brightness", &json!({ "switch": electrical_label(switch_id), "brightness": 0.25 })).unwrap();
        assert_eq!(result["brightness"], 0.25);
        for id in light_ids {
            assert_eq!(s.doc().unwrap().arch.lights.iter().find(|l| l.id == id).unwrap().brightness, 0.25);
        }
    }

    #[test]
    fn bulge_out_of_range_is_refused() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        assert!(s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [light_label(light_ids[0])], "bulge": 2.0 })).is_err());
        assert!(s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [light_label(light_ids[0])], "bulge": -2.0 })).is_err());
    }

    #[test]
    fn deleting_a_light_prunes_its_connections() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [light_label(light_ids[0]), light_label(light_ids[1])] }))
            .unwrap();
        s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [light_label(light_ids[1])] })).unwrap();
        assert!(s.doc().unwrap().arch.connections.len() >= 2);
        s.execute("lightfixture.delete", &json!({ "id": light_label(light_ids[1]) })).unwrap();
        let conns = &s.doc().unwrap().arch.connections;
        assert!(conns.iter().all(|c| !c.to.contains(&light_ids[1])));
        assert!(conns.iter().all(|c| !c.to.is_empty()));
    }

    #[test]
    fn connection_edit_updates_to_and_bulge() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [light_label(light_ids[0])] })).unwrap();
        let result = s
            .execute("connection.edit", &json!({ "id": "cn0", "to": [light_label(light_ids[0]), light_label(light_ids[1])], "bulge": 0.5 }))
            .unwrap();
        assert_eq!(result["bulge"], 0.5);
        let conn = s.doc().unwrap().arch.connections.iter().find(|c| c.id == ArchId(0)).unwrap();
        assert_eq!(conn.to.len(), 2);
        assert_eq!(conn.bulge, 0.5);
    }

    #[test]
    fn connection_edit_refuses_invalid_to() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [light_label(light_ids[0])] })).unwrap();
        assert!(s.execute("connection.edit", &json!({ "id": "cn0", "to": ["lf99"] })).is_err());
    }

    #[test]
    fn connection_edit_refuses_invalid_bulge() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [light_label(light_ids[0])] })).unwrap();
        assert!(s.execute("connection.edit", &json!({ "id": "cn0", "bulge": 2.0 })).is_err());
    }

    #[test]
    fn connection_delete_removes_connection() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [light_label(light_ids[0])] })).unwrap();
        assert_eq!(s.doc().unwrap().arch.connections.len(), 1);
        s.execute("connection.delete", &json!({ "id": "cn0" })).unwrap();
        assert_eq!(s.doc().unwrap().arch.connections.len(), 0);
    }

    #[test]
    fn connection_delete_refuses_invalid_id() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [light_label(light_ids[0])] })).unwrap();
        assert!(s.execute("connection.delete", &json!({ "id": "cn99" })).is_err());
    }

    #[test]
    fn circuit_brightness_refuses_non_switch() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        let mut doc = s.doc_mut().unwrap();
        let outlet = doc.arch.push_electrical(ElectricalDevice {
            wall: doc.arch.walls[0].id,
            kind: ElectricalKind::Duplex,
            side: WallSide::Right,
            offset: 2.0,
            height: 0.3,
            ..Default::default()
        });
        drop(doc);
        assert!(s.execute("circuit.brightness", &json!({ "switch": electrical_label(outlet), "brightness": 0.5 })).is_err());
    }

    #[test]
    fn circuit_brightness_refuses_switch_without_connection() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        assert!(s.execute("circuit.brightness", &json!({ "switch": electrical_label(switch_id), "brightness": 0.5 })).is_err());
    }

    #[test]
    fn connection_refuses_duplicate_lights() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        assert!(
            s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [light_label(light_ids[0]), light_label(light_ids[0])] }))
                .is_err()
        );
    }

    #[test]
    fn connection_refuses_non_numeric_bulge() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        assert!(
            s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [light_label(light_ids[0])], "bulge": "not a number" }))
                .is_err()
        );
    }

    #[test]
    fn circuit_brightness_refuses_non_numeric() {
        let (mut s, switch_id, light_ids) = make_switch_and_lights();
        s.execute("connection", &json!({ "from": electrical_label(switch_id), "to": [light_label(light_ids[0])] })).unwrap();
        assert!(s.execute("circuit.brightness", &json!({ "switch": electrical_label(switch_id), "brightness": "not a number" })).is_err());
    }
}
