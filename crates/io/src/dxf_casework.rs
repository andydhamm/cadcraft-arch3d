//! Save-time proxies for cabinets and fixtures (phase 13): each plan symbol polyline as an
//! `LWPOLYLINE` on its casework layer, so a drawing opened elsewhere still shows the casework.
//! Written only on save, dropped on read while the arch record is present (the record is the
//! truth) and kept as plain linework when it is not.

use cadcraft_arch::casework::{cabinet_layer, fixture_layer, fixture_symbol};
use cadcraft_arch::casework_symbol::cabinet_symbol_in;
use cadcraft_doc::arch::ArchModel;
use cadcraft_dxf::Tag;
use cadcraft_geom::Vec2;

/// Marker in the `CADCRAFT` xdata of every casework proxy.
pub(crate) const CASEWORK_PROXY_MARKER: &str = "CADCRAFT-CASEWORK";
/// Most proxy polylines written for casework, so a pathological count can't flood a save.
pub(crate) const MAX_CASEWORK_PROXIES: usize = 20_000;

/// `(entity kind, layer, tags)` for every casework proxy (`LWPOLYLINE`s on the item's layer).
///
/// One `LWPOLYLINE` per polyline of each cabinet's and fixture's plan symbol, skipping rings with
/// fewer than two finite vertices. A ring repeating its first point is written closed (70 = 1)
/// with the repeated vertex dropped. Output is capped at [`MAX_CASEWORK_PROXIES`].
pub(crate) fn proxy_casework_entities(m: &ArchModel) -> Vec<(String, String, Vec<Tag>)> {
    let mut out: Vec<(String, String, Vec<Tag>)> = Vec::new();
    let cabinets = m.cabinets.iter().map(|c| (cabinet_layer(c.kind), cabinet_symbol_in(m, c)));
    let fixtures = m.fixtures.iter().map(|f| (fixture_layer(f.kind), fixture_symbol(f)));
    // Custom countertops: the sampled ring, closed, on the countertop's layer.
    let counters = m.countertops.iter().filter_map(|ct| {
        let o = cadcraft_arch::countertop::custom_outline(m, ct)?;
        let mut ring = cadcraft_arch::countertop_edit::sampled_ring(&o.ring, &o.sagittas);
        ring.push(*ring.first()?);
        Some((ct.layer.as_str(), vec![ring]))
    });
    for (layer, symbol) in cabinets.chain(fixtures).chain(counters) {
        for poly in symbol {
            if out.len() >= MAX_CASEWORK_PROXIES {
                return out;
            }
            if let Some(tags) = casework_polyline(&poly) {
                out.push(("LWPOLYLINE".into(), layer.into(), tags));
            }
        }
    }
    out
}

/// Tags for one symbol ring; `None` with fewer than two vertices or a non-finite coordinate.
fn casework_polyline(pts: &[Vec2]) -> Option<Vec<Tag>> {
    if pts.len() < 2 || pts.iter().any(|p| !p.is_finite()) {
        return None;
    }
    let closed = pts.len() > 2 && pts.first() == pts.last();
    let keep = if closed { pts.len().saturating_sub(1) } else { pts.len() };
    let n = i64::try_from(keep).ok()?;
    let mut tags = vec![Tag::i(90, n), Tag::i(70, i64::from(closed))];
    for p in pts.iter().take(keep) {
        tags.push(Tag::f(10, p.x));
        tags.push(Tag::f(20, p.y));
    }
    tags.push(Tag::s(1001, crate::dxf_ext::APP));
    tags.push(Tag::s(1000, CASEWORK_PROXY_MARKER));
    Some(tags)
}

/// True when an entity's `CADCRAFT` xdata marks it as a casework proxy.
pub(crate) fn is_casework_proxy(tags: &[Tag]) -> bool {
    crate::dxf_ext::xdata(tags, crate::dxf_ext::APP).iter().any(|t| t.code == 1000 && t.str().trim() == CASEWORK_PROXY_MARKER)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_arch::casework::{cabinet_defaults, fixture_defaults};
    use cadcraft_doc::Drawing;
    use cadcraft_doc::arch::{CabinetKind, FixtureKind};

    fn drawing() -> Drawing {
        let mut d = Drawing::default();
        d.arch.push_cabinet(cabinet_defaults(CabinetKind::Base));
        let mut wall = cabinet_defaults(CabinetKind::Wall);
        wall.position = Vec2::new(3.0, 0.0);
        d.arch.push_cabinet(wall);
        d.arch.push_fixture(fixture_defaults(FixtureKind::Sink));
        d.arch.push_fixture(fixture_defaults(FixtureKind::Range));
        d
    }

    #[test]
    fn casework_proxies_sit_on_their_layers() {
        let d = drawing();
        let p = proxy_casework_entities(&d.arch);
        assert!(!p.is_empty());
        assert!(p.iter().all(|(k, _, t)| k == "LWPOLYLINE" && is_casework_proxy(t)));
        let on = |name: &str| p.iter().filter(|(_, l, _)| l == name).count();
        assert!(on("A-FLOR-CASE") > 0 && on("A-FLOR-CASE-UPPR") > 0 && on("P-FLOR-FIXT") > 0 && on("A-FLOR-APPL") > 0);
        assert_eq!(p.len(), on("A-FLOR-CASE") + on("A-FLOR-CASE-UPPR") + on("P-FLOR-FIXT") + on("A-FLOR-APPL"), "no proxy on another layer");
    }

    #[test]
    fn casework_proxies_are_capped_and_skip_non_finite() {
        let mut d = Drawing::default();
        for _ in 0..(MAX_CASEWORK_PROXIES + 10) {
            d.arch.cabinets.push(cabinet_defaults(CabinetKind::Base));
        }
        assert_eq!(proxy_casework_entities(&d.arch).len(), MAX_CASEWORK_PROXIES);
        let mut bad = Drawing::default();
        let mut c = cabinet_defaults(CabinetKind::Base);
        c.position = Vec2::new(f64::NAN, 0.0);
        bad.arch.cabinets.push(c);
        assert!(proxy_casework_entities(&bad.arch).is_empty());
    }

    #[test]
    fn casework_proxies_are_dropped_on_read_with_the_record() {
        let d = drawing();
        let text = crate::write_dxf(&d);
        let markers = text.matches(CASEWORK_PROXY_MARKER).count();
        assert_eq!(markers, proxy_casework_entities(&d.arch).len(), "save writes the proxies");
        let back = crate::read_dxf(text.as_bytes()).unwrap();
        assert_eq!(back.arch.cabinets, d.arch.cabinets);
        assert_eq!(back.arch.fixtures, d.arch.fixtures);
        assert_eq!(back.model.len(), d.model.len(), "no proxy kept as linework");
        assert_eq!(crate::write_dxf(&back).matches(CASEWORK_PROXY_MARKER).count(), markers, "no duplicates on re-save");
    }

    #[test]
    fn casework_proxies_stay_as_linework_without_the_record() {
        let d = drawing();
        let n = proxy_casework_entities(&d.arch).len();
        let chunks = crate::dxf_arch::arch_chunks(&d.arch).unwrap();
        let text = crate::write_dxf(&d).replace(&chunks[0], "not json at all");
        let back = crate::read_dxf(text.as_bytes()).unwrap();
        assert!(back.arch.is_empty());
        assert_eq!(back.model.len(), d.model.len() + n, "every proxy survives as plain linework");
    }

    #[test]
    fn a_custom_countertop_is_written_as_a_closed_proxy_and_dropped_on_read() {
        use cadcraft_doc::arch::Countertop;
        let mut d = Drawing::default();
        d.arch.push_countertop(Countertop {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 1.0), Vec2::new(0.0, 1.0)],
            ..Default::default()
        });
        let p = proxy_casework_entities(&d.arch);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].1, "A-FLOR-CASE");
        assert!(is_casework_proxy(&p[0].2));
        assert_eq!((p[0].2[0].i64(), p[0].2[1].i64()), (4, 1), "four vertices, closed");
        let back = crate::read_dxf(crate::write_dxf(&d).as_bytes()).unwrap();
        assert_eq!(back.arch.countertops, d.arch.countertops);
        assert_eq!(back.model.len(), d.model.len(), "the proxy is not kept as linework");
    }

    #[test]
    fn empty_model_produces_no_proxies() {
        let d = Drawing::default();
        assert!(proxy_casework_entities(&d.arch).is_empty());
    }

    #[test]
    fn two_point_polyline_is_written() {
        let pts = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)];
        let tags = casework_polyline(&pts).unwrap();
        assert_eq!(tags[0].code, 90);
        assert_eq!(tags[0].i64(), 2);
        assert_eq!(tags[1].code, 70);
        assert_eq!(tags[1].i64(), 0); // not closed
    }

    #[test]
    fn closed_ring_drops_repeated_vertex() {
        let pts = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 0.0)];
        let tags = casework_polyline(&pts).unwrap();
        assert_eq!(tags[0].i64(), 3); // 4 points, one dropped
        assert_eq!(tags[1].i64(), 1); // closed
    }

    #[test]
    fn casework_polyline_skips_one_point() {
        let pts = [Vec2::new(0.0, 0.0)];
        assert!(casework_polyline(&pts).is_none());
    }

    #[test]
    fn casework_polyline_skips_empty() {
        assert!(casework_polyline(&[]).is_none());
    }

    #[test]
    fn is_casework_proxy_requires_marker() {
        let tags: Vec<Tag> = vec![Tag::i(0, 0)];
        assert!(!is_casework_proxy(&tags));
        let tags2 = vec![Tag::s(1001, crate::dxf_ext::APP), Tag::s(1000, "CADCRAFT-LIGHT")];
        assert!(!is_casework_proxy(&tags2));
        let tags3 = vec![Tag::s(1001, crate::dxf_ext::APP), Tag::s(1000, CASEWORK_PROXY_MARKER)];
        assert!(is_casework_proxy(&tags3));
    }

    #[test]
    fn huge_coordinates_are_written() {
        let pts = [Vec2::new(1e12, 1e12), Vec2::new(-1e12, -1e12)];
        let tags = casework_polyline(&pts).unwrap();
        assert_eq!(tags[0].i64(), 2);
    }

    #[test]
    fn collinear_points_are_preserved() {
        let pts = [Vec2::new(0.0, 0.0), Vec2::new(0.5, 0.0), Vec2::new(1.0, 0.0)];
        let tags = casework_polyline(&pts).unwrap();
        assert_eq!(tags[0].i64(), 3); // all 3 points kept
    }

    #[test]
    fn cabinets_precede_fixtures_in_output() {
        let mut d = Drawing::default();
        d.arch.push_fixture(fixture_defaults(FixtureKind::Sink));
        d.arch.push_cabinet(cabinet_defaults(CabinetKind::Base));
        let p = proxy_casework_entities(&d.arch);
        let first_fixture = p.iter().position(|(_, l, _)| l == "P-FLOR-FIXT");
        let last_cabinet = p.iter().rposition(|(_, l, _)| l == "A-FLOR-CASE");
        assert!(first_fixture.is_some());
        assert!(last_cabinet.is_some());
        assert!(last_cabinet.unwrap() < first_fixture.unwrap());
    }
}
