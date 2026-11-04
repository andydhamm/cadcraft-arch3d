//! Save-time proxies for furnishings (phase 14): each furnishing's plan outline as one closed
//! `LWPOLYLINE` on its layer, so a drawing opened elsewhere still shows the furniture. Written
//! only on save, dropped on read while the arch record is present (the record is the truth) and
//! kept as plain linework when it is not.

use std::collections::BTreeSet;

use cadcraft_arch::furnishing::{furnishing_layer, plan_outline};
use cadcraft_doc::arch::ArchModel;
use cadcraft_dxf::Tag;

/// Marker in the `CADCRAFT` xdata of every furnishing proxy.
pub(crate) const FURNISHING_PROXY_MARKER: &str = "CADCRAFT-FURNISHING";

/// `(entity kind, layer, tags)` for every furnishing proxy: one closed `LWPOLYLINE` per
/// furnishing whose outline has at least three finite vertices (the count is capped by the
/// model's own `MAX_FURNISHINGS`).
pub(crate) fn proxy_furnishing_entities(m: &ArchModel) -> Vec<(String, String, Vec<Tag>)> {
    let mut out = Vec::new();
    for f in m.furnishings.iter().take(cadcraft_doc::arch::MAX_FURNISHINGS) {
        let pts = plan_outline(m, f);
        if pts.len() < 3 || pts.iter().any(|p| !p.is_finite()) {
            continue;
        }
        let Ok(n) = i64::try_from(pts.len()) else { continue };
        let mut tags = vec![Tag::i(90, n), Tag::i(70, 1)];
        for p in &pts {
            tags.push(Tag::f(10, p.x));
            tags.push(Tag::f(20, p.y));
        }
        tags.push(Tag::s(1001, crate::dxf_ext::APP));
        tags.push(Tag::s(1000, FURNISHING_PROXY_MARKER));
        out.push(("LWPOLYLINE".to_string(), furnishing_layer(f).to_string(), tags));
    }
    out
}

/// The layers the proxies need, sorted and without duplicates.
pub(crate) fn proxy_layers(m: &ArchModel) -> Vec<String> {
    let set: BTreeSet<&str> = m.furnishings.iter().map(furnishing_layer).collect();
    set.into_iter().map(String::from).collect()
}

/// True when an entity's `CADCRAFT` xdata marks it as a furnishing proxy.
pub(crate) fn is_furnishing_proxy(tags: &[Tag]) -> bool {
    crate::dxf_ext::xdata(tags, crate::dxf_ext::APP).iter().any(|t| t.code == 1000 && t.str().trim() == FURNISHING_PROXY_MARKER)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::Drawing;
    use cadcraft_doc::arch::{Backdrop, Furnishing, ImageRgba, MaterialRef, Paint, Placement, Projection, Surface, SymbolMesh, SymbolPart};

    fn textured_symbol() -> SymbolMesh {
        let part = SymbolPart {
            name: "top".into(),
            positions: vec![[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5]],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            indices: vec![0, 1, 2, 0, 2, 3],
            base_color: [0.5, 0.25, 0.125, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            texture: Some(0),
        };
        let tex = ImageRgba { width: 2, height: 2, rgba: vec![200; 16] };
        SymbolMesh { key: "00112233aabbccdd".into(), parts: vec![part], textures: vec![tex] }
    }

    fn drawing() -> Drawing {
        let mut d = Drawing::new_metric();
        d.arch.symbols.insert("00112233aabbccdd".into(), textured_symbol());
        d.arch.push_furnishing(Furnishing {
            item: "armchair".into(),
            symbol: "00112233aabbccdd".into(),
            name: "Armchair".into(),
            category: "Furniture".into(),
            symbol_type: "armchair".into(),
            at: [2.0, 3.0],
            rotation: 30.0,
            size: [0.9, 0.9, 0.9],
            ..Default::default()
        });
        d.arch.push_furnishing(Furnishing {
            item: "plain".into(),
            name: "Plain".into(),
            category: "Furniture".into(),
            symbol_type: "table".into(),
            placement: Placement::Floor,
            at: [5.0, 1.0],
            size: [1.2, 0.6, 0.75],
            ..Default::default()
        });
        d.arch.images.insert("material-oak".into(), ImageRgba { width: 1, height: 1, rgba: vec![10, 20, 30, 255] });
        d.arch.paints.insert(
            Surface::Furnishing { furnishing: d.arch.furnishings[0].id, part: 0 }.key(),
            Paint { material: MaterialRef::parse("#c04020"), tint: Some([1, 2, 3]) },
        );
        d.arch.backdrop = Some(Backdrop::Image { item: "material-oak".into(), projection: Projection::Equirect });
        d.arch.defaults.insert("door".into(), serde_json::json!({"width": 0.9}));
        d
    }

    #[test]
    fn furnishing_proxies_on_their_layers() {
        let d = drawing();
        let p = proxy_furnishing_entities(&d.arch);
        assert_eq!(p.len(), 2);
        assert!(p.iter().all(|(k, _, t)| k == "LWPOLYLINE" && is_furnishing_proxy(t)));
        for ((_, layer, _), f) in p.iter().zip(&d.arch.furnishings) {
            assert_eq!(layer, furnishing_layer(f));
        }
        assert_eq!(proxy_layers(&d.arch).len(), p.iter().map(|(_, l, _)| l.clone()).collect::<BTreeSet<_>>().len());
        assert!(proxy_furnishing_entities(&Drawing::default().arch).is_empty());
    }

    #[test]
    fn proxies_are_dropped_on_read() {
        let d = drawing();
        let text = crate::write_dxf(&d);
        let markers = text.matches(FURNISHING_PROXY_MARKER).count();
        assert_eq!(markers, 2, "save writes the proxies");
        let back = crate::read_dxf(text.as_bytes()).unwrap();
        assert_eq!(back.model.len(), d.model.len(), "no proxy kept as linework");
        assert_eq!(crate::write_dxf(&back).matches(FURNISHING_PROXY_MARKER).count(), markers, "no duplicates on re-save");
    }

    #[test]
    fn furnishings_symbols_and_paints_round_trip() {
        let d = drawing();
        let back = crate::read_dxf(crate::write_dxf(&d).as_bytes()).unwrap();
        assert_eq!(back.arch.furnishings, d.arch.furnishings);
        assert_eq!(back.arch.next_furnishing_id, d.arch.next_furnishing_id);
        assert_eq!(back.arch.symbols, d.arch.symbols);
        assert_eq!(back.arch.images, d.arch.images);
        assert_eq!(back.arch.paints, d.arch.paints);
    }

    #[test]
    fn backdrop_and_defaults_round_trip() {
        let mut d = drawing();
        d.arch.defaults.insert("furnishing.armchair".into(), serde_json::json!({"size": [1.0, 1.0, 1.0]}));
        let back = crate::read_dxf(crate::write_dxf(&d).as_bytes()).unwrap();
        assert_eq!(back.arch.backdrop, d.arch.backdrop);
        assert_eq!(back.arch.defaults, d.arch.defaults);
    }

    #[test]
    fn a_phase13_dxf_still_reads() {
        use cadcraft_arch::casework::cabinet_defaults;
        use cadcraft_doc::arch::CabinetKind;
        let mut d = Drawing::new_metric();
        d.arch.push_cabinet(cabinet_defaults(CabinetKind::Base));
        let text = crate::write_dxf(&d);
        assert_eq!(text.matches(FURNISHING_PROXY_MARKER).count(), 0);
        let back = crate::read_dxf(text.as_bytes()).unwrap();
        assert_eq!(back.arch.cabinets, d.arch.cabinets);
        assert!(back.arch.furnishings.is_empty() && back.arch.symbols.is_empty() && back.arch.backdrop.is_none());
    }
}
