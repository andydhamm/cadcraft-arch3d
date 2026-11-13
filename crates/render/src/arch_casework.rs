//! Cabinets and fixtures in the plan (phase 13): each symbol on its layer, tagged with the item's
//! handle so the plan picks and selects it.

use cadcraft_arch::casework::{cabinet_layer, fixture_layer, fixture_symbol};
use cadcraft_color::Color;
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::{cabinet_handle, fixture_handle};

use crate::DisplayList;
use crate::arch::{arch_layer_skipped, layer_transparency, on_shown_level, push_polyline_with_transparency};

fn ink_for(d: &Drawing, layer: &str) -> crate::Ink {
    crate::ink(Color::ByLayer, d.layer(layer).map(|l| l.color).unwrap_or(Color::Index(7)), Color::Index(7))
}

/// Append every cabinet's and fixture's plan symbol to `list`.
pub(crate) fn push_casework(d: &Drawing, list: &mut DisplayList, plotting: bool) {
    for c in &d.arch.cabinets {
        if !on_shown_level(d, c.level) {
            continue;
        }
        let layer = cabinet_layer(c.kind);
        if arch_layer_skipped(d, layer, plotting) {
            continue;
        }
        let ink = ink_for(d, layer);
        let transparency = layer_transparency(d, layer);
        for sym in cadcraft_arch::casework_symbol::cabinet_symbol_in(&d.arch, c) {
            if sym.len() >= 2 {
                push_polyline_with_transparency(list, cabinet_handle(c.id), &ink, &sym, transparency);
            }
        }
    }
    for ct in &d.arch.countertops {
        if !on_shown_level(d, ct.level) || arch_layer_skipped(d, &ct.layer, plotting) {
            continue;
        }
        let Some(o) = cadcraft_arch::countertop::custom_outline(&d.arch, ct) else { continue };
        let mut ring = cadcraft_arch::countertop_edit::sampled_ring(&o.ring, &o.sagittas);
        if let Some(first) = ring.first().copied() {
            ring.push(first);
        }
        if ring.len() >= 3 {
            push_polyline_with_transparency(
                list,
                cadcraft_doc::arch::countertop_handle(ct.id),
                &ink_for(d, &ct.layer),
                &ring,
                layer_transparency(d, &ct.layer),
            );
        }
    }
    for f in &d.arch.fixtures {
        if !on_shown_level(d, f.level) {
            continue;
        }
        let layer = fixture_layer(f.kind);
        if arch_layer_skipped(d, layer, plotting) {
            continue;
        }
        let ink = ink_for(d, layer);
        let transparency = layer_transparency(d, layer);
        for sym in fixture_symbol(f) {
            if sym.len() >= 2 {
                push_polyline_with_transparency(list, fixture_handle(f.id), &ink, &sym, transparency);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_arch::casework::{cabinet_defaults, fixture_defaults};
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{CabinetKind, FixtureKind};

    #[test]
    fn custom_outline_drawn_and_claimed_cabinets_drop_their_counter_rect() {
        use cadcraft_doc::arch::{Cabinet, Countertop, countertop_handle};
        let mut d = Drawing::default();
        let base = Cabinet {
            kind: CabinetKind::Base,
            width: 0.9,
            depth: 0.6,
            counter: true,
            ..cadcraft_arch::casework::cabinet_defaults(CabinetKind::Base)
        };
        let cid = d.arch.push_cabinet(base);
        let prims = |d: &Drawing| {
            let mut list = DisplayList::default();
            push_casework(d, &mut list, false);
            list
        };
        let n_before = prims(&d).prims.iter().filter(|p| p.handle == cabinet_handle(cid)).count();
        let ct = Countertop {
            polygon: vec![
                cadcraft_geom::Vec2::new(0.0, 0.0),
                cadcraft_geom::Vec2::new(2.0, 0.0),
                cadcraft_geom::Vec2::new(2.0, 1.0),
                cadcraft_geom::Vec2::new(0.0, 1.0),
            ],
            cabinets: vec![cid],
            ..Default::default()
        };
        let tid = d.arch.push_countertop(ct);
        let list = prims(&d);
        assert_eq!(list.prims.iter().filter(|p| p.handle == countertop_handle(tid)).count(), 1, "one closed ring for the countertop");
        let n_after = list.prims.iter().filter(|p| p.handle == cabinet_handle(cid)).count();
        assert!(n_after < n_before, "the claimed cabinet drops its counter rectangle ({n_before} -> {n_after})");
    }

    #[test]
    fn casework_draws_on_its_layers_with_handles() {
        let mut d = Drawing::default();
        let mut c = cabinet_defaults(CabinetKind::Base);
        let (cid, fid) = (ArchId(1), ArchId(2));
        c.id = cid;
        let mut f = fixture_defaults(FixtureKind::Sink);
        f.id = fid;
        d.arch.cabinets.push(c);
        d.arch.fixtures.push(f);
        let mut list = DisplayList::default();
        push_casework(&d, &mut list, false);
        assert!(list.prims.iter().any(|p| p.handle == cabinet_handle(cid)));
        assert!(list.prims.iter().any(|p| p.handle == fixture_handle(fid)));

        let mut hidden = d.clone();
        hidden.ensure_layer("A-FLOR-CASE");
        if let Some(l) = hidden.layers.iter_mut().find(|l| l.name == "A-FLOR-CASE") {
            l.on = false;
        }
        let mut list = DisplayList::default();
        push_casework(&hidden, &mut list, false);
        assert!(!list.prims.iter().any(|p| p.handle == cabinet_handle(cid)));
        assert!(list.prims.iter().any(|p| p.handle == fixture_handle(fid)));
    }
}
