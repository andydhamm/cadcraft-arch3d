//! Interior soffits in the plan (phase 16; owner: p16-soffits): each soffit's outline, dashed on
//! its layer (`A-CLNG`), tagged with its handle.

use cadcraft_arch::ceiling::soffit::soffit_ring;
use cadcraft_color::Color;
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::soffit_handle;

use crate::DisplayList;
use crate::arch::{arch_layer_skipped, layer_transparency, on_shown_level, push_polyline_with_transparency};
use crate::arch_electrical::dashed_segments;

/// Append every interior soffit's dashed plan outline to `list`.
pub(crate) fn push_ceiling(d: &Drawing, list: &mut DisplayList, plotting: bool) {
    let layer = "A-CLNG";
    if arch_layer_skipped(d, layer, plotting) {
        return;
    }
    let ink = crate::ink(Color::ByLayer, d.layer(layer).map(|l| l.color).unwrap_or(Color::Index(7)), Color::Index(7));
    let transparency = layer_transparency(d, layer);
    for s in &d.arch.soffits {
        if !on_shown_level(d, s.level) {
            continue;
        }
        let Some(mut ring) = soffit_ring(&d.arch, s).filter(|r| r.len() >= 2 && r.iter().all(|p| p.is_finite())) else {
            continue;
        };
        if let Some(first) = ring.first().copied() {
            ring.push(first);
        }
        for dash in dashed_segments(d, layer, &ring) {
            if dash.len() < 2 {
                continue;
            }
            push_polyline_with_transparency(list, soffit_handle(s.id), &ink, &dash, transparency);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::{Options, build_arch};
    use cadcraft_doc::arch::{Level, Soffit};
    use cadcraft_doc::{ArchId, Handle};
    use cadcraft_geom::Vec2;

    fn drawing() -> Drawing {
        let mut d = Drawing::default();
        d.ensure_layer("A-CLNG");
        d.arch.levels.push(Level { index: 1, name: "Upper".into(), elevation: 3.0, ..Default::default() });
        d.arch.push_soffit(Soffit {
            level: 0,
            polygon: vec![Vec2::new(1.0, 1.0), Vec2::new(5.0, 1.0), Vec2::new(5.0, 3.0), Vec2::new(1.0, 3.0)],
            bottom: Some(2.1),
            ..Default::default()
        });
        d
    }

    fn prims_of(list: &DisplayList, h: Handle) -> Vec<Vec<Vec2>> {
        list.prims.iter().filter(|p| p.handle == h).map(|p| list.verts[p.start as usize..(p.start + p.len) as usize].to_vec()).collect()
    }

    #[test]
    fn soffit_outline_is_dashed_on_a_clng_on_its_level_only() {
        let mut d = drawing();
        let h = soffit_handle(ArchId(0));
        let prims = prims_of(&build_arch(&d, &Options::default(), false), h);
        assert!(prims.len() > 4, "dashed: {}", prims.len());
        let on_edge = |v: f64, edges: [f64; 2]| edges.iter().any(|e| (v - e).abs() < 1e-9);
        for dash in &prims {
            assert!(dash.iter().all(|p| on_edge(p.x, [1.0, 5.0]) || on_edge(p.y, [1.0, 3.0])), "{dash:?}");
        }
        d.arch.plan_level = Some(1);
        assert!(prims_of(&build_arch(&d, &Options::default(), false), h).is_empty());
        d.arch.plan_level = Some(0);
        assert!(!prims_of(&build_arch(&d, &Options::default(), false), h).is_empty());
    }

    #[test]
    fn hidden_when_a_clng_is_off() {
        let mut d = drawing();
        let h = soffit_handle(ArchId(0));
        if let Some(l) = d.layers.iter_mut().find(|l| l.name == "A-CLNG") {
            l.on = false;
        }
        assert!(prims_of(&build_arch(&d, &Options::default(), false), h).is_empty());
    }
}
