//! Save-time proxies for light fixtures (phase 10): each fixture's plan symbol on `E-LITE`, so a
//! drawing opened elsewhere still shows the lights. Written only on save, dropped on read while
//! the arch record is present (like site proxies).

use cadcraft_doc::arch::ArchModel;
use cadcraft_geom::Vec2;
use craft_dxf::Tag;

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
    for f in &m.lights {
        for poly in light::plan_symbol(f) {
            if out.len() >= MAX_LIGHT_PROXIES {
                return out;
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
    if pts.len() < 2 || pts.iter().any(|p| !p.is_finite()) {
        return None;
    }
    let closed = pts.first() == pts.last();
    let verts: Vec<&Vec2> = if closed { pts.iter().take(pts.len().saturating_sub(1)).collect() } else { pts.iter().collect() };
    let n = i64::try_from(verts.len()).ok()?;
    let mut tags = vec![Tag::i(90, n), Tag::i(70, i64::from(closed))];
    for p in &verts {
        tags.push(Tag::f(10, p.x));
        tags.push(Tag::f(20, p.y));
    }
    Some(marked_light(tags))
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
    use cadcraft_doc::arch::{ArchId, ArchModel, FixtureShape, LightFixture, LightKind};

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
    fn empty_model_writes_no_proxies() {
        let m = ArchModel::default();
        let p = proxy_light_entities(&m);
        assert!(p.is_empty(), "empty model produces no proxies");
    }

    #[test]
    fn light_proxies_respect_cap() {
        let mut m = ArchModel::default();
        // Each pot light produces 2 polylines. 50,001 pot lights = 100,002 polylines > cap.
        for i in 0..50_001u32 {
            m.lights.push(LightFixture::new(ArchId(i), "Recessed pot light", Vec2::new(i as f64, 0.0)));
        }
        let p = proxy_light_entities(&m);
        assert_eq!(p.len(), 100_000, "proxies capped at 100,000");
    }

    #[test]
    fn light_proxies_skip_non_finite() {
        let mut m = ArchModel::default();
        m.lights.push(LightFixture::new(ArchId(0), "Recessed pot light", Vec2::new(f64::NAN, 1.0)));
        let p = proxy_light_entities(&m);
        // Should not crash; may produce 0 or 2 polylines depending on plan_symbol's handling
        for (_, _, tags) in &p {
            assert!(is_light_proxy(tags), "all proxies are marked");
        }
    }

    #[test]
    fn light_proxy_tag_structure() {
        let m = model_with_lights();
        let p = proxy_light_entities(&m);
        for (kind, layer, tags) in &p {
            assert_eq!(kind, "LWPOLYLINE", "proxy is a LWPOLYLINE");
            assert_eq!(layer, LIGHT_LAYER, "proxy is on E-LITE");
            // First tag should be 90 (vertex count)
            assert_eq!(tags[0].code, 90, "first tag is vertex count (90)");
            // Second tag should be 70 (closed flag)
            assert_eq!(tags[1].code, 70, "second tag is closed flag (70)");
            // Tag structure: [90, 70, 10, 20, ..., 1001, 1000]
            // Minimum: 2 vertices = 4 coord tags + 2 header + 2 xdata = 8
            assert!(tags.len() >= 8, "enough tags for at least 2 vertices and xdata");
            // Vertex coordinate tags alternate 10/20
            let coord_tags = &tags[2..tags.len() - 2];
            assert_eq!(coord_tags.len() % 2, 0, "even number of coordinate tags");
            for (j, t) in coord_tags.iter().enumerate() {
                let expected_code = if j % 2 == 0 { 10 } else { 20 };
                assert_eq!(t.code, expected_code, "coordinate tag code");
            }
            // Last two tags are the xdata marker
            assert_eq!(tags[tags.len() - 2].code, 1001, "xdata app tag (1001)");
            assert_eq!(tags[tags.len() - 2].str(), crate::dxf_ext::APP, "xdata app is CADCRAFT");
            assert_eq!(tags[tags.len() - 1].code, 1000, "xdata value tag (1000)");
            assert_eq!(tags[tags.len() - 1].str(), LIGHT_PROXY_MARKER, "xdata value is CADCRAFT-LIGHT");
        }
    }

    #[test]
    fn light_proxy_round_trip_no_lights() {
        let mut d = cadcraft_doc::Drawing::default();
        // No lights at all
        let text = crate::write_dxf(&d);
        assert_eq!(text.matches(LIGHT_PROXY_MARKER).count(), 0, "no markers when no lights");
        let back = crate::read_dxf(text.as_bytes()).expect("reopen");
        assert!(back.arch.lights.is_empty(), "no lights after round trip");
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
        assert!(shapes.contains(&FixtureShape::RecessedCan), "pot light survives");
        assert!(shapes.contains(&FixtureShape::Sconce), "sconce survives");
        // Same proxies regenerate from the round-tripped model; none were kept as model-space
        // entities (written only on save, dropped on read).
        assert_eq!(proxy_light_entities(&back.arch).len(), 3, "same proxies, none duplicated");
        let again = crate::write_dxf(&back);
        assert_eq!(again.matches(LIGHT_PROXY_MARKER).count(), markers, "probes dropped on read, so no duplicate proxies on save");
    }

    /// The coordinates (codes 10 and 20) of a proxy's tags, in order.
    fn proxy_points(tags: &[Tag]) -> Vec<(f64, f64)> {
        let xs = tags.iter().filter(|t| t.code == 10).map(|t| t.f64());
        let ys = tags.iter().filter(|t| t.code == 20).map(|t| t.f64());
        xs.zip(ys).collect()
    }

    #[test]
    fn dxf_panel_proxy_is_its_rectangle() {
        let mut m = ArchModel::default();
        let mut panel = LightFixture::new(ArchId(0), "LED panel 600", Vec2::new(2.0, 1.0));
        panel.shape = FixtureShape::Panel;
        panel.kind = LightKind::Area;
        panel.width = 0.6;
        panel.length = 0.4;
        m.lights.push(panel);

        let p = proxy_light_entities(&m);
        assert_eq!(p.len(), 3, "the rectangle and its two diagonals");
        assert!(p.iter().all(|(k, l, t)| k == "LWPOLYLINE" && l == LIGHT_LAYER && is_light_proxy(t)));

        // The rectangle: a closed ring of four corners (the repeated fifth point dropped).
        let ring = &p[0].2;
        assert_eq!((ring[0].code, ring[0].i64()), (90, 4), "four vertices");
        assert_eq!((ring[1].code, ring[1].i64()), (70, 1), "closed");
        let corners = proxy_points(ring);
        let expected = [(1.7, 0.8), (2.3, 0.8), (2.3, 1.2), (1.7, 1.2)];
        assert_eq!(corners.len(), 4);
        for (got, want) in corners.iter().zip(expected) {
            assert!((got.0 - want.0).abs() < 1e-9 && (got.1 - want.1).abs() < 1e-9, "{corners:?}");
        }

        // The diagonals: corner 0 to 2, then 1 to 3, open two-point lines.
        for (proxy, (from, to)) in p[1..].iter().zip([(0, 2), (1, 3)]) {
            assert_eq!((proxy.2[0].code, proxy.2[0].i64()), (90, 2));
            assert_eq!((proxy.2[1].code, proxy.2[1].i64()), (70, 0), "a diagonal is open");
            let pts = proxy_points(&proxy.2);
            assert_eq!(pts.len(), 2);
            assert!((pts[0].0 - expected[from].0).abs() < 1e-9 && (pts[0].1 - expected[from].1).abs() < 1e-9, "{pts:?}");
            assert!((pts[1].0 - expected[to].0).abs() < 1e-9 && (pts[1].1 - expected[to].1).abs() < 1e-9, "{pts:?}");
        }

        // Turned 90 degrees, the width runs along +Y.
        m.lights[0].rotation = 90.0;
        let turned = proxy_points(&proxy_light_entities(&m)[0].2);
        let (xs, ys): (Vec<f64>, Vec<f64>) = turned.iter().copied().unzip();
        let span = |v: &[f64]| v.iter().copied().fold(f64::MIN, f64::max) - v.iter().copied().fold(f64::MAX, f64::min);
        assert!((span(&xs) - 0.4).abs() < 1e-9 && (span(&ys) - 0.6).abs() < 1e-9, "{turned:?}");
    }

    #[test]
    fn dxf_panel_at_45_degrees() {
        let mut m = ArchModel::default();
        let mut panel = LightFixture::new(ArchId(0), "LED panel 600", Vec2::new(2.0, 2.0));
        panel.shape = FixtureShape::Panel;
        panel.kind = LightKind::Area;
        panel.width = 0.6;
        panel.length = 0.6;
        panel.rotation = 45.0;
        m.lights.push(panel);

        let p = proxy_light_entities(&m);
        assert_eq!(p.len(), 3, "rectangle and two diagonals");
        let ring = &p[0].2;
        let corners = proxy_points(ring);
        assert_eq!(corners.len(), 4);
        // At 45 degrees, the corners should be rotated around the center (2.0, 2.0)
        let cos = 45.0_f64.to_radians().cos();
        let sin = 45.0_f64.to_radians().sin();
        let rotate = |x: f64, y: f64| -> (f64, f64) {
            let dx = x - 2.0;
            let dy = y - 2.0;
            (2.0 + dx * cos - dy * sin, 2.0 + dx * sin + dy * cos)
        };
        let expected = [rotate(1.7, 1.7), rotate(2.3, 1.7), rotate(2.3, 2.3), rotate(1.7, 2.3)];
        for (got, want) in corners.iter().zip(expected) {
            assert!((got.0 - want.0).abs() < 1e-9 && (got.1 - want.1).abs() < 1e-9, "{corners:?}");
        }
    }

    #[test]
    fn dxf_panel_with_nan_position_skipped() {
        let mut m = ArchModel::default();
        let mut panel = LightFixture::new(ArchId(0), "LED panel 600", Vec2::new(f64::NAN, 1.0));
        panel.shape = FixtureShape::Panel;
        panel.kind = LightKind::Area;
        panel.width = 0.6;
        panel.length = 0.4;
        m.lights.push(panel);

        let p = proxy_light_entities(&m);
        // plan_symbol should produce polylines with NaN vertices, which light_polyline skips
        assert!(p.is_empty(), "NaN position should produce no proxies");
    }
}
