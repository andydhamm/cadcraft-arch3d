//! Save-time proxies for electrical devices and switch→light connections (phase 15): each
//! device's plan symbol on its `E-POWR`/`E-LITE` layer and each connection as an arc on `E-LITE`,
//! so a drawing opened elsewhere still shows them. Written only on save, dropped on read while
//! the arch record is present (like light proxies).

use std::collections::BTreeSet;

use cadcraft_doc::arch::{ArchId, ArchModel, MAX_CONNECTIONS, MAX_ELECTRICAL};
use cadcraft_geom::Vec2;
use craft_dxf::Tag;

use cadcraft_arch::{connections, electrical};

/// Marker in the `CADCRAFT` xdata of every electrical proxy.
pub(crate) const ELECTRICAL_PROXY_MARKER: &str = "CADCRAFT-ELECTRICAL";
/// The layer connection arcs are written on.
const CONNECTION_LAYER: &str = "E-LITE";
/// Most proxies written for electrical content, so a pathological count can't flood a save.
const MAX_ELECTRICAL_PROXIES: usize = 100_000;

/// `(entity kind, layer, tags)` for every electrical proxy (all `LWPOLYLINE`s): the plan symbol
/// of each placeable device on its kind's layer, then one arc per switch→light connection on
/// `E-LITE`. Output is capped at `MAX_ELECTRICAL_PROXIES`.
pub(crate) fn proxy_electrical_entities(m: &ArchModel) -> Vec<(String, String, Vec<Tag>)> {
    let mut out: Vec<(String, String, Vec<Tag>)> = Vec::new();
    for d in m.electrical.iter().take(MAX_ELECTRICAL) {
        for ring in electrical::device_symbol(m, d) {
            if out.len() >= MAX_ELECTRICAL_PROXIES {
                return out;
            }
            let Some(tags) = ring_polyline(&ring) else {
                continue;
            };
            out.push(("LWPOLYLINE".into(), d.kind.layer().into(), tags));
        }
    }
    for c in m.connections.iter().take(MAX_CONNECTIONS) {
        if !c.bulge.is_finite() {
            continue;
        }
        let mut lights: Vec<ArchId> = c.to.clone();
        lights.sort();
        lights.dedup();
        for id in lights {
            if out.len() >= MAX_ELECTRICAL_PROXIES {
                return out;
            }
            let (Some(a), Some(b)) = (connections::device_plan(m, c.from), connections::light_plan(m, id)) else {
                continue;
            };
            if !a.is_finite() || !b.is_finite() || a == b {
                continue;
            }
            let tags = vec![Tag::i(90, 2), Tag::i(70, 0), Tag::f(10, a.x), Tag::f(20, a.y), Tag::f(42, c.bulge), Tag::f(10, b.x), Tag::f(20, b.y)];
            out.push(("LWPOLYLINE".into(), CONNECTION_LAYER.into(), marked(tags)));
        }
    }
    out
}

/// Tags for one symbol ring; `None` when it has fewer than two vertices or a non-finite one.
/// A ring repeating its first point is written closed with the repeated vertex dropped.
fn ring_polyline(pts: &[Vec2]) -> Option<Vec<Tag>> {
    if pts.len() < 2 || pts.iter().any(|p| !p.is_finite()) {
        return None;
    }
    let closed = pts.first() == pts.last();
    let keep = if closed { pts.len().saturating_sub(1) } else { pts.len() };
    let n = i64::try_from(keep).ok()?;
    let mut tags = vec![Tag::i(90, n), Tag::i(70, i64::from(closed))];
    for p in pts.iter().take(keep) {
        tags.push(Tag::f(10, p.x));
        tags.push(Tag::f(20, p.y));
    }
    Some(marked(tags))
}

/// Append the electrical-proxy marker xdata to `tags`.
fn marked(mut tags: Vec<Tag>) -> Vec<Tag> {
    tags.push(Tag::s(1001, crate::dxf_ext::APP));
    tags.push(Tag::s(1000, ELECTRICAL_PROXY_MARKER));
    tags
}

/// The layers the proxies need, sorted and without duplicates.
pub(crate) fn proxy_layers(m: &ArchModel) -> Vec<String> {
    let mut set: BTreeSet<&str> = m.electrical.iter().map(|d| d.kind.layer()).collect();
    if !m.connections.is_empty() {
        set.insert(CONNECTION_LAYER);
    }
    set.into_iter().map(String::from).collect()
}

/// True when an entity's `CADCRAFT` xdata marks it as an electrical proxy.
pub(crate) fn is_electrical_proxy(tags: &[Tag]) -> bool {
    crate::dxf_ext::xdata(tags, crate::dxf_ext::APP).iter().any(|t| t.code == 1000 && t.str().trim() == ELECTRICAL_PROXY_MARKER)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::Drawing;
    use cadcraft_doc::arch::{Connection, ElectricalDevice, ElectricalKind, Level, LightFixture, Wall, WallSide};

    fn model() -> ArchModel {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, ..Default::default() });
        m.push_wall(Wall::new(Vec2::new(0., 0.), Vec2::new(6., 0.), 0.2));
        m.push_electrical(ElectricalDevice { kind: ElectricalKind::Duplex, offset: 1.5, side: WallSide::Left, ..Default::default() });
        let sw = m.push_electrical(ElectricalDevice { kind: ElectricalKind::Dimmer, offset: 2.5, side: WallSide::Left, ..Default::default() });
        m.lights.push(LightFixture::new(ArchId(0), "Pot", Vec2::new(1.0, 2.0)));
        m.connections.push(Connection { id: ArchId(4), from: sw, to: vec![ArchId(0)], bulge: 0.3, ..Default::default() });
        m
    }

    #[test]
    fn dxf_proxies_are_on_the_electrical_layers() {
        let m = model();
        let p = proxy_electrical_entities(&m);
        assert!(!p.is_empty());
        for (kind, _, tags) in &p {
            assert_eq!(kind, "LWPOLYLINE");
            assert!(is_electrical_proxy(tags));
        }
        assert!(p.iter().any(|(_, l, _)| l == "E-POWR"), "outlet symbols");
        assert!(p.iter().any(|(_, l, _)| l == "E-LITE"), "switch symbols");
        let arcs: Vec<_> = p.iter().filter(|(_, _, t)| t.iter().any(|t| t.code == 42)).collect();
        assert_eq!(arcs.len(), 1);
        let (_, layer, tags) = arcs[0];
        assert_eq!(layer, "E-LITE");
        assert_eq!(tags.iter().filter(|t| t.code == 10).count(), 2, "two vertices");
        assert!(tags.iter().any(|t| t.code == 42 && (t.f64() - 0.3).abs() < 1e-12));
        assert_eq!(proxy_layers(&m), vec!["E-LITE".to_string(), "E-POWR".to_string()]);
        assert!(proxy_electrical_entities(&ArchModel::default()).is_empty());
    }

    fn drawing() -> Drawing {
        let mut d = Drawing::new_metric();
        d.arch = model();
        d
    }

    #[test]
    fn a_drawing_with_electrical_round_trips_through_dxf() {
        let d = drawing();
        let n = proxy_electrical_entities(&d.arch).len();
        let text = crate::write_dxf(&d);
        let markers = text.matches(ELECTRICAL_PROXY_MARKER).count();
        assert!(markers > 0);
        assert_eq!(markers, n);
        let back = crate::read_dxf(text.as_bytes()).unwrap();
        assert_eq!(back.arch.electrical, d.arch.electrical);
        assert_eq!(back.arch.connections, d.arch.connections);
        assert_eq!(back.model.len(), d.model.len(), "no proxy kept as linework");
        assert_eq!(crate::write_dxf(&back).matches(ELECTRICAL_PROXY_MARKER).count(), markers);
    }

    #[test]
    fn electrical_proxies_survive_without_the_record() {
        let d = drawing();
        let n = proxy_electrical_entities(&d.arch).len();
        let text = crate::write_dxf(&d).replace("CADCRAFT_ARCH", "CADCRAFT_ARCX");
        let back = crate::read_dxf(text.as_bytes()).unwrap();
        assert!(back.arch.electrical.is_empty());
        assert!(back.model.len() >= n, "proxies kept as ordinary linework");
    }

    #[test]
    fn connection_with_non_finite_bulge_is_skipped() {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, ..Default::default() });
        m.push_wall(Wall::new(Vec2::new(0., 0.), Vec2::new(6., 0.), 0.2));
        let sw = m.push_electrical(ElectricalDevice { kind: ElectricalKind::Dimmer, offset: 2.5, side: WallSide::Left, ..Default::default() });
        m.lights.push(LightFixture::new(ArchId(0), "Pot", Vec2::new(1.0, 2.0)));
        m.connections.push(Connection { id: ArchId(4), from: sw, to: vec![ArchId(0)], bulge: f64::NAN, ..Default::default() });
        let p = proxy_electrical_entities(&m);
        let arcs: Vec<_> = p.iter().filter(|(_, _, t)| t.iter().any(|t| t.code == 42)).collect();
        assert!(arcs.is_empty(), "connection with NaN bulge should be skipped");
    }

    #[test]
    fn closed_ring_drops_repeated_vertex() {
        let pts = vec![Vec2::new(0., 0.), Vec2::new(1., 0.), Vec2::new(1., 1.), Vec2::new(0., 1.), Vec2::new(0., 0.)];
        let tags = ring_polyline(&pts).unwrap();
        assert_eq!(tags.iter().find(|t| t.code == 90).unwrap().i64(), 4, "vertex count");
        assert_eq!(tags.iter().find(|t| t.code == 70).unwrap().i64(), 1, "closed flag");
        let x_count = tags.iter().filter(|t| t.code == 10).count();
        assert_eq!(x_count, 4, "repeated vertex should be dropped");
    }

    #[test]
    fn proxy_layers_without_connections_excludes_e_lite() {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, ..Default::default() });
        m.push_wall(Wall::new(Vec2::new(0., 0.), Vec2::new(6., 0.), 0.2));
        m.push_electrical(ElectricalDevice { kind: ElectricalKind::Duplex, offset: 1.5, side: WallSide::Left, ..Default::default() });
        let layers = proxy_layers(&m);
        assert_eq!(layers, vec!["E-POWR".to_string()], "no connections means no E-LITE");
    }

    #[test]
    fn is_electrical_proxy_rejects_other_markers() {
        let tags = vec![Tag::i(90, 2), Tag::i(70, 0)];
        assert!(!is_electrical_proxy(&tags), "no xdata");
        let tags = vec![Tag::i(90, 2), Tag::i(70, 0), Tag::s(1001, crate::dxf_ext::APP), Tag::s(1000, "CADCRAFT-LIGHT")];
        assert!(!is_electrical_proxy(&tags), "different marker");
        let tags = vec![Tag::i(90, 2), Tag::i(70, 0), Tag::s(1001, crate::dxf_ext::APP), Tag::s(1000, ELECTRICAL_PROXY_MARKER)];
        assert!(is_electrical_proxy(&tags), "correct marker");
    }

    #[test]
    fn connection_with_missing_light_is_skipped() {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, ..Default::default() });
        m.push_wall(Wall::new(Vec2::new(0., 0.), Vec2::new(6., 0.), 0.2));
        let sw = m.push_electrical(ElectricalDevice { kind: ElectricalKind::Dimmer, offset: 2.5, side: WallSide::Left, ..Default::default() });
        m.connections.push(Connection { id: ArchId(4), from: sw, to: vec![ArchId(0)], bulge: 0.2, ..Default::default() });
        let p = proxy_electrical_entities(&m);
        let arcs: Vec<_> = p.iter().filter(|(_, _, t)| t.iter().any(|t| t.code == 42)).collect();
        assert!(arcs.is_empty(), "connection to missing light should be skipped");
    }

    #[test]
    fn ring_with_fewer_than_two_vertices_is_skipped() {
        assert!(ring_polyline(&[]).is_none(), "empty ring");
        assert!(ring_polyline(&[Vec2::new(0., 0.)]).is_none(), "single vertex");
    }

    #[test]
    fn ring_with_non_finite_vertex_is_skipped() {
        let pts = vec![Vec2::new(0., 0.), Vec2::new(f64::NAN, 1.), Vec2::new(1., 1.)];
        assert!(ring_polyline(&pts).is_none(), "NaN vertex");
        let pts = vec![Vec2::new(0., 0.), Vec2::new(f64::INFINITY, 1.), Vec2::new(1., 1.)];
        assert!(ring_polyline(&pts).is_none(), "infinite vertex");
    }
}
