//! Furnishings in the plan (phase 14): each top-down outline as a closed polyline on its layer,
//! tagged with the furnishing's handle so the plan picks and selects it.

use cadcraft_arch::furnishing::{furnishing_layer, plan_outline};
use cadcraft_color::Color;
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::furnishing_handle;

use crate::DisplayList;
use crate::arch::{arch_layer_skipped, layer_transparency, on_shown_level, push_polyline_with_transparency};

/// Append every furnishing's plan outline to `list`.
pub(crate) fn push_furnishings(d: &Drawing, list: &mut DisplayList, plotting: bool) {
    for f in &d.arch.furnishings {
        if !on_shown_level(d, f.level) {
            continue;
        }
        let layer = furnishing_layer(f);
        if arch_layer_skipped(d, layer, plotting) {
            continue;
        }
        let mut ring = plan_outline(&d.arch, f);
        let Some(&first) = ring.first() else { continue };
        if ring.len() < 3 {
            continue;
        }
        ring.push(first);
        let color = d.layer(layer).map(|l| l.color).unwrap_or(Color::Index(7));
        let ink = crate::ink(Color::ByLayer, color, Color::Index(7));
        let transparency = layer_transparency(d, layer);
        push_polyline_with_transparency(list, furnishing_handle(f.id), &ink, &ring, transparency);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::Furnishing;

    const CASES: [(&str, &str, &str); 5] = [
        ("Furniture", "chair", "I-FURN"),
        ("Appliances and plumbing fixtures", "sink", "P-FLOR-FIXT"),
        ("Appliances and plumbing fixtures", "range", "A-FLOR-APPL"),
        ("Lighting and electrical", "light", "E-LITE"),
        ("Plants", "plant", "L-PLNT"),
    ];

    fn drawing() -> Drawing {
        let mut d = Drawing::default();
        for (i, (cat, ty, _)) in CASES.iter().enumerate() {
            d.arch.furnishings.push(Furnishing {
                id: ArchId(i as u32),
                category: (*cat).into(),
                symbol_type: (*ty).into(),
                size: [1.0, 1.0, 1.0],
                ..Default::default()
            });
        }
        d
    }

    #[test]
    fn furnishings_draw_on_their_category_layers() {
        let d = drawing();
        let mut list = DisplayList::default();
        push_furnishings(&d, &mut list, false);
        assert_eq!(list.prims.len(), 5, "one polyline per furnishing");
        for (f, (_, _, layer)) in d.arch.furnishings.iter().zip(CASES) {
            assert_eq!(furnishing_layer(f), layer);
        }
        for p in &list.prims {
            let s = p.start as usize;
            assert_eq!(list.verts[s], list.verts[s + p.len as usize - 1], "closed");
        }
        // A hidden layer drops only its own furnishing.
        let mut hidden = d.clone();
        hidden.ensure_layer("E-LITE");
        if let Some(l) = hidden.layers.iter_mut().find(|l| l.name == "E-LITE") {
            l.on = false;
        }
        let mut list = DisplayList::default();
        push_furnishings(&hidden, &mut list, false);
        assert_eq!(list.prims.len(), 4);
    }

    #[test]
    fn furnishing_outlines_carry_their_handles() {
        let d = drawing();
        let mut list = DisplayList::default();
        push_furnishings(&d, &mut list, false);
        for f in &d.arch.furnishings {
            assert_eq!(list.prims.iter().filter(|p| p.handle == furnishing_handle(f.id)).count(), 1);
        }
    }
}
