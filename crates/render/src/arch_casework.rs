//! Cabinets and fixtures in the plan (phase 13): each symbol on its layer, tagged with the item's
//! handle so the plan picks and selects it.

use cadcraft_arch::casework::{cabinet_layer, cabinet_symbol, fixture_layer, fixture_symbol};
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
        for sym in cabinet_symbol(c) {
            if sym.len() >= 2 {
                push_polyline_with_transparency(list, cabinet_handle(c.id), &ink, &sym, transparency);
            }
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
