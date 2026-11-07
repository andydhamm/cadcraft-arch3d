//! Electrical devices and switch-to-light connections in the plan (phase 15): each symbol is
//! drawn on its kind's layer and each arc is dashed on `E-LITE`, all tagged with their proxy
//! handles so plan picking resolves to the right electrical object.

use cadcraft_color::Color;
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::{connection_handle, electrical_handle};
use cadcraft_geom::Vec2;

use crate::DisplayList;
use crate::arch::{arch_layer_skipped, on_shown_level, push_polyline};

fn ink_for(d: &Drawing, layer: &str, fallback: Color) -> crate::Ink {
    crate::ink(Color::ByLayer, d.layer(layer).map(|l| l.color).unwrap_or(fallback), fallback)
}

fn dashed_segments(d: &Drawing, layer: &str, pts: &[Vec2]) -> Vec<Vec<Vec2>> {
    if pts.len() < 2 {
        return Vec::new();
    }
    let lt = d
        .layer(layer)
        .and_then(|l| d.linetype(&l.linetype).filter(|lt| !lt.pattern.is_empty()).cloned())
        .unwrap_or_else(|| cadcraft_doc::Linetype::simple("DASHED", "", &[0.1, -0.05]));
    let segments = crate::linetype::apply(pts, &lt, 1.0, 0.0);
    if segments.is_empty() { vec![pts.to_vec()] } else { segments }
}

/// Append each electrical device symbol and switch-connection arc to `list`.
pub(crate) fn push_electrical(d: &Drawing, list: &mut DisplayList, plotting: bool) {
    for device in &d.arch.electrical {
        let layer = device.kind.layer();
        if !on_shown_level(d, device.level) || arch_layer_skipped(d, layer, plotting) {
            continue;
        }
        let ink = ink_for(d, layer, if device.kind.is_switch() { Color::Index(2) } else { Color::Index(3) });
        for poly in cadcraft_arch::electrical::device_symbol(&d.arch, device) {
            if poly.len() < 2 || !poly.iter().all(|p| p.is_finite()) {
                continue;
            }
            push_polyline(list, electrical_handle(device.id), &ink, &poly);
        }
    }

    let layer = "E-LITE";
    if arch_layer_skipped(d, layer, plotting) {
        return;
    }
    let ink = ink_for(d, layer, Color::Index(2));
    for conn in &d.arch.connections {
        let Some(switch) = d.arch.electrical.iter().find(|device| device.id == conn.from) else {
            continue;
        };
        if !on_shown_level(d, switch.level) {
            continue;
        }
        let arcs = cadcraft_arch::connections::connection_arcs(&d.arch, conn);
        if arcs.is_empty() {
            // Light is missing; draw a simple line from the switch so the connection is visible.
            let switch_pos = switch_plan_position(d, switch);
            let arc = vec![switch_pos, switch_pos + Vec2::new(1.0, 0.0)];
            for dash in dashed_segments(d, layer, &arc) {
                if dash.len() < 2 {
                    continue;
                }
                push_polyline(list, connection_handle(conn.id), &ink, &dash);
            }
        } else {
            for arc in arcs {
                if arc.len() < 2 || !arc.iter().all(|p| p.is_finite()) {
                    continue;
                }
                for dash in dashed_segments(d, layer, &arc) {
                    if dash.len() < 2 {
                        continue;
                    }
                    push_polyline(list, connection_handle(conn.id), &ink, &dash);
                }
            }
        }
    }
}

/// The plan position of an electrical device on its host wall.
fn switch_plan_position(d: &Drawing, device: &cadcraft_doc::arch::ElectricalDevice) -> Vec2 {
    let Some(wall) = d.arch.wall(device.wall) else {
        return Vec2::new(0.0, 0.0);
    };
    let dir = (wall.end - wall.start).normalized();
    let perp = dir.perp();
    let pos = wall.start + dir * device.offset;
    if device.side == cadcraft_doc::arch::WallSide::Left { pos + perp * wall.thickness / 2.0 } else { pos - perp * wall.thickness / 2.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DisplayList, Options, build_arch};
    use cadcraft_doc::arch::{Connection, ElectricalDevice, ElectricalKind, Level, LightFixture, Wall, WallSide};
    use cadcraft_doc::{ArchId, Drawing};
    use cadcraft_geom::Vec2;

    fn base_wall() -> Wall {
        Wall { id: ArchId(0), start: Vec2::new(0.0, 0.0), end: Vec2::new(4.0, 0.0), thickness: 0.2, height: 2.4, ..Default::default() }
    }

    #[test]
    fn device_symbols_draw_on_their_layers() {
        let mut d = Drawing::default();
        d.ensure_layer("E-POWR");
        d.ensure_layer("E-LITE");
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        let wall = base_wall();
        let wall_id = wall.id;
        d.arch.walls.push(wall);
        let outlet = ElectricalDevice {
            id: ArchId(1),
            kind: ElectricalKind::Duplex,
            wall: wall_id,
            side: WallSide::Left,
            offset: 1.0,
            height: 0.3,
            level: 0,
            ..Default::default()
        };
        let switch = ElectricalDevice {
            id: ArchId(2),
            kind: ElectricalKind::SinglePole,
            wall: wall_id,
            side: WallSide::Left,
            offset: 2.5,
            height: 1.2,
            level: 0,
            ..Default::default()
        };
        let outlet_id = outlet.id;
        let switch_id = switch.id;
        d.arch.electrical.push(outlet);
        d.arch.electrical.push(switch);

        let mut list = DisplayList::default();
        push_electrical(&d, &mut list, false);
        assert!(list.prims.iter().any(|p| p.handle == electrical_handle(outlet_id)));
        assert!(list.prims.iter().any(|p| p.handle == electrical_handle(switch_id)));

        let mut hidden = d.clone();
        hidden.ensure_layer("E-POWR");
        if let Some(layer) = hidden.layers.iter_mut().find(|layer| layer.name == "E-POWR") {
            layer.on = false;
        }
        let mut hidden_list = DisplayList::default();
        push_electrical(&hidden, &mut hidden_list, false);
        assert!(!hidden_list.prims.iter().any(|p| p.handle == electrical_handle(outlet_id)));
        assert!(hidden_list.prims.iter().any(|p| p.handle == electrical_handle(switch_id)));
    }

    #[test]
    fn connection_arcs_draw_on_e_lite() {
        let mut d = Drawing::default();
        d.ensure_layer("E-LITE");
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        let wall = base_wall();
        let wall_id = wall.id;
        d.arch.walls.push(wall);
        let switch = ElectricalDevice {
            id: ArchId(10),
            kind: ElectricalKind::SinglePole,
            wall: wall_id,
            side: WallSide::Left,
            offset: 1.6,
            height: 1.2,
            level: 0,
            ..Default::default()
        };
        let switch_id = switch.id;
        let light = LightFixture::new(ArchId(11), "recessed-can", Vec2::new(3.0, 1.0));
        let light_id = light.id;
        d.arch.electrical.push(switch);
        d.arch.lights.push(light);
        let conn = Connection { id: ArchId(20), from: switch_id, to: vec![light_id], bulge: 0.2, ..Default::default() };
        let conn_id = conn.id;
        d.arch.connections.push(conn);

        let list = build_arch(&d, &Options::default(), false);
        let handle_hits: Vec<_> = list.prims.iter().filter(|p| p.handle == connection_handle(conn_id)).collect();
        assert!(!handle_hits.is_empty());
        assert!(handle_hits.len() > 1, "connection arcs should be dashed across multiple segments: {}", handle_hits.len());

        let mut hidden = d.clone();
        hidden.ensure_layer("E-LITE");
        if let Some(layer) = hidden.layers.iter_mut().find(|layer| layer.name == "E-LITE") {
            layer.on = false;
        }
        let hidden_list = build_arch(&hidden, &Options::default(), false);
        assert!(!hidden_list.prims.iter().any(|p| p.handle == connection_handle(conn_id)));
    }

    #[test]
    fn electrical_prims_carry_their_handles() {
        let mut d = Drawing::default();
        d.ensure_layer("E-POWR");
        d.ensure_layer("E-LITE");
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        let wall = base_wall();
        let wall_id = wall.id;
        d.arch.walls.push(wall);
        let outlet = ElectricalDevice {
            id: ArchId(30),
            kind: ElectricalKind::Duplex,
            wall: wall_id,
            side: WallSide::Left,
            offset: 1.0,
            height: 0.3,
            level: 0,
            ..Default::default()
        };
        let switch = ElectricalDevice {
            id: ArchId(31),
            kind: ElectricalKind::SinglePole,
            wall: wall_id,
            side: WallSide::Left,
            offset: 2.2,
            height: 1.2,
            level: 0,
            ..Default::default()
        };
        let outlet_id = outlet.id;
        let switch_id = switch.id;
        let light = LightFixture::new(ArchId(32), "recessed-can", Vec2::new(3.0, 1.0));
        let light_id = light.id;
        d.arch.electrical.push(outlet);
        d.arch.electrical.push(switch);
        d.arch.lights.push(light);
        d.arch.connections.push(Connection { id: ArchId(33), from: switch_id, to: vec![light_id], bulge: 0.1, ..Default::default() });
        let conn_id = d.arch.connections[0].id;

        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.iter().any(|p| p.handle == electrical_handle(outlet_id)));
        assert!(list.prims.iter().any(|p| p.handle == electrical_handle(switch_id)));
        assert!(list.prims.iter().any(|p| p.handle == connection_handle(conn_id)));
    }

    #[test]
    fn empty_electrical_model_draws_nothing() {
        let d = Drawing::default();
        let list = build_arch(&d, &Options::default(), false);
        // No electrical devices or connections means no electrical prims.
        assert!(list.prims.iter().all(|p| p.handle != electrical_handle(ArchId(0))));
        assert!(list.prims.iter().all(|p| p.handle != connection_handle(ArchId(0))));
    }

    #[test]
    fn device_on_hidden_level_is_not_drawn() {
        let mut d = Drawing::default();
        d.ensure_layer("E-POWR");
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        d.arch.levels.push(Level { index: 1, elevation: 2.7, ..Default::default() });
        let wall = base_wall();
        let wall_id = wall.id;
        d.arch.walls.push(wall);
        let outlet = ElectricalDevice {
            id: ArchId(50),
            kind: ElectricalKind::Duplex,
            wall: wall_id,
            side: WallSide::Left,
            offset: 1.0,
            height: 0.3,
            level: 1,
            ..Default::default()
        };
        let outlet_id = outlet.id;
        d.arch.electrical.push(outlet);

        // Showing level 0: the level-1 outlet is hidden.
        d.arch.plan_level = Some(0);
        let list = build_arch(&d, &Options::default(), false);
        assert!(!list.prims.iter().any(|p| p.handle == electrical_handle(outlet_id)));

        // Showing level 1: the outlet draws.
        d.arch.plan_level = Some(1);
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.iter().any(|p| p.handle == electrical_handle(outlet_id)));
    }

    #[test]
    fn connection_with_missing_switch_is_skipped() {
        let mut d = Drawing::default();
        d.ensure_layer("E-LITE");
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        let light = LightFixture::new(ArchId(60), "recessed-can", Vec2::new(3.0, 1.0));
        let light_id = light.id;
        d.arch.lights.push(light);
        // conn.from = ArchId(999) which does not exist.
        let conn = Connection { id: ArchId(70), from: ArchId(999), to: vec![light_id], bulge: 0.1, ..Default::default() };
        let conn_id = conn.id;
        d.arch.connections.push(conn);

        let list = build_arch(&d, &Options::default(), false);
        assert!(!list.prims.iter().any(|p| p.handle == connection_handle(conn_id)));
    }

    #[test]
    fn connection_with_missing_light_still_draws() {
        let mut d = Drawing::default();
        d.ensure_layer("E-LITE");
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        let wall = base_wall();
        let wall_id = wall.id;
        d.arch.walls.push(wall);
        let switch = ElectricalDevice {
            id: ArchId(80),
            kind: ElectricalKind::SinglePole,
            wall: wall_id,
            side: WallSide::Left,
            offset: 1.6,
            height: 1.2,
            level: 0,
            ..Default::default()
        };
        let switch_id = switch.id;
        d.arch.electrical.push(switch);
        // conn.to contains a light that does not exist.
        let conn = Connection { id: ArchId(81), from: switch_id, to: vec![ArchId(999)], bulge: 0.1, ..Default::default() };
        let conn_id = conn.id;
        d.arch.connections.push(conn);

        let list = build_arch(&d, &Options::default(), false);
        // The arc should still be drawn from the switch toward the (missing) light position.
        assert!(list.prims.iter().any(|p| p.handle == connection_handle(conn_id)));
    }

    #[test]
    fn multiple_connections_from_one_switch() {
        let mut d = Drawing::default();
        d.ensure_layer("E-LITE");
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        let wall = base_wall();
        let wall_id = wall.id;
        d.arch.walls.push(wall);
        let switch = ElectricalDevice {
            id: ArchId(90),
            kind: ElectricalKind::SinglePole,
            wall: wall_id,
            side: WallSide::Left,
            offset: 1.6,
            height: 1.2,
            level: 0,
            ..Default::default()
        };
        let switch_id = switch.id;
        d.arch.electrical.push(switch);
        let light1 = LightFixture::new(ArchId(91), "recessed-can", Vec2::new(3.0, 1.0));
        let light2 = LightFixture::new(ArchId(92), "recessed-can", Vec2::new(3.0, 2.0));
        let light1_id = light1.id;
        let light2_id = light2.id;
        d.arch.lights.push(light1);
        d.arch.lights.push(light2);
        let conn1 = Connection { id: ArchId(93), from: switch_id, to: vec![light1_id], bulge: 0.1, ..Default::default() };
        let conn2 = Connection { id: ArchId(94), from: switch_id, to: vec![light2_id], bulge: 0.1, ..Default::default() };
        let conn1_id = conn1.id;
        let conn2_id = conn2.id;
        d.arch.connections.push(conn1);
        d.arch.connections.push(conn2);

        let list = build_arch(&d, &Options::default(), false);
        let h1: Vec<&crate::DPrim> = list.prims.iter().filter(|p| p.handle == connection_handle(conn1_id)).collect();
        let h2: Vec<&crate::DPrim> = list.prims.iter().filter(|p| p.handle == connection_handle(conn2_id)).collect();
        assert!(!h1.is_empty(), "connection 1 should draw");
        assert!(!h2.is_empty(), "connection 2 should draw");
    }

    #[test]
    fn device_with_nan_offset_draws_nothing() {
        let mut d = Drawing::default();
        d.ensure_layer("E-POWR");
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        let wall = base_wall();
        let wall_id = wall.id;
        d.arch.walls.push(wall);
        let outlet = ElectricalDevice {
            id: ArchId(110),
            kind: ElectricalKind::Duplex,
            wall: wall_id,
            side: WallSide::Left,
            offset: f64::NAN,
            height: 0.3,
            level: 0,
            ..Default::default()
        };
        let outlet_id = outlet.id;
        d.arch.electrical.push(outlet);

        let list = build_arch(&d, &Options::default(), false);
        assert!(!list.prims.iter().any(|p| p.handle == electrical_handle(outlet_id)));
    }

    #[test]
    fn device_with_huge_offset_still_draws() {
        let mut d = Drawing::default();
        d.ensure_layer("E-POWR");
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        let wall = base_wall();
        let wall_id = wall.id;
        d.arch.walls.push(wall);
        let outlet = ElectricalDevice {
            id: ArchId(120),
            kind: ElectricalKind::Duplex,
            wall: wall_id,
            side: WallSide::Left,
            offset: 1000.0,
            height: 0.3,
            level: 0,
            ..Default::default()
        };
        let outlet_id = outlet.id;
        d.arch.electrical.push(outlet);

        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.iter().any(|p| p.handle == electrical_handle(outlet_id)));
    }

    #[test]
    fn connection_with_huge_bulge_still_draws() {
        let mut d = Drawing::default();
        d.ensure_layer("E-LITE");
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        let wall = base_wall();
        let wall_id = wall.id;
        d.arch.walls.push(wall);
        let switch = ElectricalDevice {
            id: ArchId(130),
            kind: ElectricalKind::SinglePole,
            wall: wall_id,
            side: WallSide::Left,
            offset: 1.6,
            height: 1.2,
            level: 0,
            ..Default::default()
        };
        let switch_id = switch.id;
        let light = LightFixture::new(ArchId(131), "recessed-can", Vec2::new(3.0, 1.0));
        let light_id = light.id;
        d.arch.electrical.push(switch);
        d.arch.lights.push(light);
        let conn = Connection { id: ArchId(140), from: switch_id, to: vec![light_id], bulge: 1e6, ..Default::default() };
        let conn_id = conn.id;
        d.arch.connections.push(conn);

        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.iter().any(|p| p.handle == connection_handle(conn_id)));
    }

    #[test]
    fn electrical_layers_respect_no_plot() {
        let mut d = Drawing::default();
        d.ensure_layer("E-POWR");
        d.ensure_layer("E-LITE");
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        let wall = base_wall();
        let wall_id = wall.id;
        d.arch.walls.push(wall);
        let outlet = ElectricalDevice {
            id: ArchId(100),
            kind: ElectricalKind::Duplex,
            wall: wall_id,
            side: WallSide::Left,
            offset: 1.0,
            height: 0.3,
            level: 0,
            ..Default::default()
        };
        let outlet_id = outlet.id;
        d.arch.electrical.push(outlet);

        // Screen build: draws even with no-plot.
        if let Some(l) = d.layer_mut("E-POWR") {
            l.plot = false;
        }
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.iter().any(|p| p.handle == electrical_handle(outlet_id)));

        // Plot build: skipped when no-plot.
        let list = build_arch(&d, &Options::default(), true);
        assert!(!list.prims.iter().any(|p| p.handle == electrical_handle(outlet_id)));
    }
}
