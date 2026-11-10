//! Save-time proxies for interior soffits and foundation footings (phase 16): each soffit's ring
//! on `A-CLNG` and each footing outline on `S-FNDN`, dashed closed polylines, so a drawing opened
//! elsewhere still shows them. Written only on save, dropped on read while the arch record is
//! present (like the electrical proxies). Trays have no plan drawing until the reflected ceiling
//! plan, so they have no proxy.

use std::collections::BTreeSet;

use cadcraft_arch::ceiling::soffit::soffit_ring;
use cadcraft_arch::foundation::{footing_outline, foundation_walls};
use cadcraft_doc::arch::{ArchModel, CEILING_LAYER, FOUNDATION_LAYER};
use cadcraft_dxf::Tag;
use cadcraft_geom::Vec2;

/// Marker in the `CADCRAFT` xdata of every ceiling/foundation proxy.
pub(crate) const CEILING_PROXY_MARKER: &str = "CADCRAFT-CEILING";
/// Most proxies written, so a pathological count can't flood a save.
const MAX_CEILING_PROXIES: usize = 100_000;

/// `(entity kind, layer, tags)` for every soffit ring (on `A-CLNG`) then every footing outline
/// (on `S-FNDN`), all closed dashed `LWPOLYLINE`s. Rings with a non-finite vertex are skipped.
/// Output is capped at `MAX_CEILING_PROXIES`.
pub(crate) fn proxy_ceiling_entities(m: &ArchModel) -> Vec<(String, String, Vec<Tag>)> {
    let mut out: Vec<(String, String, Vec<Tag>)> = Vec::new();
    for s in &m.soffits {
        if out.len() >= MAX_CEILING_PROXIES {
            return out;
        }
        let Some(ring) = soffit_ring(m, s) else { continue };
        if let Some(tags) = closed_polyline(&ring) {
            out.push(("LWPOLYLINE".into(), CEILING_LAYER.into(), tags));
        }
    }
    for f in &m.foundations {
        for w in foundation_walls(m, f.level) {
            if out.len() >= MAX_CEILING_PROXIES {
                return out;
            }
            if let Some(tags) = closed_polyline(&footing_outline(&w)) {
                out.push(("LWPOLYLINE".into(), FOUNDATION_LAYER.into(), tags));
            }
        }
    }
    out
}

/// Tags of a closed dashed polyline through `pts`; `None` with fewer than three vertices or a
/// non-finite one. A repeated first vertex at the end is dropped.
fn closed_polyline(pts: &[Vec2]) -> Option<Vec<Tag>> {
    if pts.iter().any(|p| !p.is_finite()) {
        return None;
    }
    let keep = if pts.len() > 1 && pts.first() == pts.last() { pts.len() - 1 } else { pts.len() };
    if keep < 3 {
        return None;
    }
    let mut tags = vec![Tag::s(6, "DASHED"), Tag::i(90, i64::try_from(keep).ok()?), Tag::i(70, 1)];
    for p in pts.iter().take(keep) {
        tags.push(Tag::f(10, p.x));
        tags.push(Tag::f(20, p.y));
    }
    tags.push(Tag::s(1001, crate::dxf_ext::APP));
    tags.push(Tag::s(1000, CEILING_PROXY_MARKER));
    Some(tags)
}

/// The layers the proxies need, sorted and without duplicates.
pub(crate) fn proxy_layers(m: &ArchModel) -> Vec<String> {
    let mut set: BTreeSet<&str> = BTreeSet::new();
    if !m.soffits.is_empty() {
        set.insert(CEILING_LAYER);
    }
    if !m.foundations.is_empty() {
        set.insert(FOUNDATION_LAYER);
    }
    set.into_iter().map(String::from).collect()
}

/// True when an entity's `CADCRAFT` xdata marks it as a ceiling/foundation proxy.
pub(crate) fn is_ceiling_proxy(tags: &[Tag]) -> bool {
    crate::dxf_ext::xdata(tags, crate::dxf_ext::APP).iter().any(|t| t.code == 1000 && t.str().trim() == CEILING_PROXY_MARKER)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::Drawing;
    use cadcraft_doc::arch::{Foundation, Level, Soffit, Wall};

    fn model() -> ArchModel {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, ..Default::default() });
        let c = [(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0), (0.0, 0.0)];
        for i in 0..4 {
            let mut w = Wall::new(Vec2::new(c[i].0, c[i].1), Vec2::new(c[i + 1].0, c[i + 1].1), 0.2);
            w.exterior = true;
            m.push_wall(w);
        }
        let p = |x, y| Vec2::new(x, y);
        m.push_soffit(Soffit { polygon: vec![p(7.0, 7.3), p(9.0, 7.3), p(9.0, 7.9), p(7.0, 7.9)], bottom: Some(2.1), ..Default::default() });
        m.push_foundation(Foundation::default());
        m
    }

    fn on_layer<'a>(p: &'a [(String, String, Vec<Tag>)], layer: &str) -> Vec<&'a (String, String, Vec<Tag>)> {
        p.iter().filter(|e| e.1 == layer).collect()
    }

    #[test]
    fn soffit_and_footing_proxies_on_their_layers_dashed() {
        let m = model();
        let p = proxy_ceiling_entities(&m);
        let clng = on_layer(&p, "A-CLNG");
        let fndn = on_layer(&p, "S-FNDN");
        assert_eq!(clng.len(), 1);
        assert_eq!(fndn.len(), 4);
        for (kind, _, tags) in &p {
            assert_eq!(kind, "LWPOLYLINE");
            assert!(is_ceiling_proxy(tags));
            assert!(tags.iter().any(|t| t.code == 6 && t.str() == "DASHED"));
            assert!(tags.iter().any(|t| t.code == 70 && t.i64() == 1), "closed");
        }
        assert_eq!(clng[0].2.iter().filter(|t| t.code == 10).count(), 4);
        assert_eq!(proxy_layers(&m), vec!["A-CLNG".to_string(), "S-FNDN".to_string()]);
        assert!(proxy_ceiling_entities(&ArchModel::default()).is_empty());
    }

    #[test]
    fn proxies_dropped_on_read_and_not_duplicated_on_resave() {
        let mut d = Drawing::new_metric();
        d.arch = model();
        let n = proxy_ceiling_entities(&d.arch).len();
        let text = crate::write_dxf(&d);
        assert_eq!(text.matches(CEILING_PROXY_MARKER).count(), n);
        let back = crate::read_dxf(text.as_bytes()).unwrap();
        assert_eq!(back.arch.soffits, d.arch.soffits);
        assert_eq!(back.arch.foundations, d.arch.foundations);
        assert_eq!(back.model.len(), d.model.len(), "no proxy kept as linework");
        assert_eq!(crate::write_dxf(&back).matches(CEILING_PROXY_MARKER).count(), n);
        // Without the record the proxies stay as ordinary linework.
        let lost = crate::write_dxf(&d).replace("CADCRAFT_ARCH", "CADCRAFT_ARCX");
        let back = crate::read_dxf(lost.as_bytes()).unwrap();
        assert!(back.arch.soffits.is_empty());
        assert!(back.model.len() >= n);
    }

    #[test]
    fn nan_soffit_ring_skipped() {
        let mut m = model();
        let p = |x, y| Vec2::new(x, y);
        m.soffits.push(Soffit {
            id: cadcraft_doc::arch::ArchId(99),
            polygon: vec![p(0.0, 0.0), p(f64::NAN, 0.0), p(1.0, 1.0)],
            ..Default::default()
        });
        let p = proxy_ceiling_entities(&m);
        assert_eq!(on_layer(&p, "A-CLNG").len(), 1);
        assert!(closed_polyline(&[Vec2::new(0.0, 0.0), Vec2::new(f64::INFINITY, 0.0), Vec2::new(1.0, 1.0)]).is_none());
        assert!(closed_polyline(&[Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)]).is_none());
    }
}
