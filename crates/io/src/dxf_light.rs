//! Save-time proxies for light fixtures (phase 10): each fixture's plan symbol on `E-LITE`, so a
//! drawing opened elsewhere still shows the lights. Written only on save, dropped on read while
//! the arch record is present (like site proxies).

use cadcraft_doc::arch::ArchModel;
use cadcraft_dxf::Tag;
use cadcraft_geom::Vec2;

use cadcraft_arch::light;

/// Marker in the `CADCRAFT` xdata of every light proxy.
pub(crate) const LIGHT_PROXY_MARKER: &str = "CADCRAFT-LIGHT";
/// The AIA/NCS electrical lighting layer.
pub(crate) const LIGHT_LAYER: &str = "E-LITE";
/// Most proxies written for the lights, so a pathological fixture count can't flood a save.
const MAX_LIGHT_PROXIES: usize = 100_000;

/// `(entity kind, layer, tags)` for every light proxy (`LWPOLYLINE`s on [`LIGHT_LAYER`]).
///
/// One `LWPOLYLINE` is emitted per polyline of [`cadcraft_arch::light::plan_symbol`] for each
/// fixture, skipping rings with fewer than two finite vertices. A ring that repeats its first
/// point is written closed (flag 70 = 1) with the repeated vertex dropped, matching the site
/// proxy convention. Output is capped at `MAX_LIGHT_PROXIES`.
pub(crate) fn proxy_light_entities(m: &ArchModel) -> Vec<(String, String, Vec<Tag>)> {
    let mut out: Vec<(String, String, Vec<Tag>)> = Vec::new();
    'outer: for f in &m.lights {
        if out.len() >= MAX_LIGHT_PROXIES {
            break outer;
        }
        for poly in light::plan_symbol(f) {
            if out.len() >= MAX_LIGHT_PROXIES {
                break outer;
            }
            let Some(tags) = light_polyline(&poly) else {
                continue;
            };
            out.push(("LWPOLYLINE".into(), LIGHT_LAYER.into(), tags));
        }
    }
    out
}

/// Tag vector for a single light plan-symbol ring on [`LIGHT_LAYER`], mirroring the site
/// `polyline` helper. Returns `None` when the ring has fewer than two vertices or any
/// non-finite coordinate. A ring that repeats its first point is emitted closed (70 = 1)
/// with the repeated vertex dropped.
fn light_polyline(pts: &[Vec2]) -> Option<Vec<Tag>> {
    if pts.len() < 2 || pts.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return None;
    }
    let closed = matches!(pts.first(), Some(a)) && matches!(pts.last(), Some(b)) && a == b;
    let verts = if closed { pts.iter().take(pts.len().saturating_sub(1)) } else { pts.iter() };
    let n = i64::try_from(verts.len()).ok()?;
    let mut tags = vec![Tag::i(90, n), Tag::i(70, i64::from(closed))];
    for p in verts {
        tags.push(Tag::f(10, p.x));
        tags.push(Tag::f(20, p.y));
    }
    marked_light(tags)
}

/// Append the light-proxy marker xdata to `tags`, like the site `marked` helper.
fn marked_light(mut tags: Vec<Tag>) -> Vec<Tag> {
    tags.push(Tag::s(1001, crate::dxf_ext::APP));
    tags.push(Tag::s(1000, LIGHT_PROXY_MARKER));
    tags
}

/// True when an entity's `CADCRAFT` xdata marks it as a light proxy.
pub(crate) fn is_light_proxy(tags: &[Tag]) -> bool {
    crate::dxf_ext::xdata(tags, crate::dxf_ext::APP).iter().any(|t| t.code == 1000 && t.str().trim() == LIGHT_PROXY_MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchId, ArchModel, FixtureShape, LightFixture};

    /// A model holding a recessed pot light (two circles) and a wall sconce (one ring).
    fn model_with_lights() -> ArchModel {
        let mut m = ArchModel::default();
        m.lights.push(LightFixture::new(ArchId(0), "Recessed pot light", Vec2::new(1.0, 1.0)));
        let mut sconce = LightFixture::new(ArchId(1), "Wall sconce", Vec2::new(2.0, 1.0));
        sconce.shape = FixtureShape::Sconce;
        m.lights.push(sconce);
        m
    }

    #[test]
    fn light_proxies_are_symbols_on_e_lite() {
        let m = model_with_lights();
        let p = proxy_light_entities(&m);
        assert_eq!(p.len(), 3, "two circles for the pot light and one ring for the sconce");
        assert_eq!(p.iter().filter(|(_, l, _)| l == LIGHT_LAYER).count(), 3, "every light proxy sits on E-LITE");
        assert_eq!(p.iter().filter(|(k, _, _)| k == "LWPOLYLINE").count(), 3);
        assert!(p.iter().all(|(k, l, _)| k == "LWPOLYLINE" && l == LIGHT_LAYER), "each proxy is an LWPOLYLINE on E-LITE");
    }

    #[test]
    fn light_proxies_are_marked() {
        let m = model_with_lights();
        assert!(
            !proxy_light_entities(&m).is_empty() && proxy_light_entities(&m).iter().all(|(_, _, t)| is_light_proxy(t)),
            "all light proxies carry the CADCRAFT-LIGHT xdata marker"
        );
    }

    #[test]
    fn light_fixtures_round_trip_and_proxies_drop() {
        let mut d = cadcraft_doc::Drawing::default();
        d.arch = model_with_lights();
        let text = crate::write_dxf(&d);
        let markers = text.matches(LIGHT_PROXY_MARKER).count();
        assert!(markers > 0, "save writes the light proxies");
        let back = crate::read_dxf(text.as_bytes()).expect("reopen");
        // The fixtures survive in the arch record.
        assert_eq!(back.arch.lights.len(), 2, "light fixtures round-trip in the record");
        let shapes: Vec<_> = back.arch.lights.iter().map(|f| f.shape).collect();
        assert!(shapes.iter().contains(&&FixtureShape::RecessedCan), "pot light survives");
        assert!(shapes.iter().contains(&&FixtureShape::Sconce), "sconce survives");
        // Same proxies regenerate from the round-tripped model; none were kept as model-space
        // entities (written only on save, dropped on read).
        assert_eq!(proxy_light_entities(&back.arch).len(), 3, "same proxies, none duplicated");
        let again = crate::write_dxf(&back);
        assert_eq!(again.matches(LIGHT_PROXY_MARKER).count(), markers, "probes dropped on read, so no duplicate proxies on save");
    }
}
