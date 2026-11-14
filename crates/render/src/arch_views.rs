use cadcraft_color::Color;
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::{LAYER_ELEV_PATTERN, LAYER_ELEV_SWING, NoteKind, ViewNote, view_handle};
use cadcraft_fonts::{TextFont, TextParams, place};
use cadcraft_geom::Vec2;

use crate::{DisplayList, arch::arch_layer_skipped, arch::layer_transparency, arch::push_dim, arch::push_polyline_with_transparency};

const MAX_VIEW_ITEMS: usize = 100_000;

fn placed_point(view: &cadcraft_doc::arch::ArchView, p: [f64; 2]) -> Option<Vec2> {
    let q = if view.generated.is_none() { [view.insert[0] + p[0], view.insert[1] + p[1]] } else { view.placed(p) };
    if q.iter().all(|v| v.is_finite()) { Some(Vec2::new(q[0], q[1])) } else { None }
}

fn draw_text(list: &mut DisplayList, handle: cadcraft_doc::Handle, ink: &crate::Ink, transparency: u8, at: Vec2, text: &str, height: f64) {
    let params = TextParams::centered(at, height, 0.0);
    let (sh, _) = place(&TextFont::Stroke, text, &params);
    for stroke in &sh.strokes {
        if stroke.len() >= 2 && stroke.iter().all(|p| p.is_finite()) {
            push_polyline_with_transparency(list, handle, ink, stroke, transparency);
        }
    }
}

fn draw_marker(list: &mut DisplayList, handle: cadcraft_doc::Handle, ink: &crate::Ink, transparency: u8, at: Vec2, radius: f64, text: &str) {
    let mut pts = Vec::with_capacity(24);
    for i in 0..24 {
        let a = std::f64::consts::TAU * (i as f64 / 24.0);
        let p = Vec2::new(at.x + radius * a.cos(), at.y + radius * a.sin());
        pts.push(p);
    }
    for i in 0..24 {
        let j = (i + 1) % 24;
        let a = pts[i];
        let b = pts[j];
        push_polyline_with_transparency(list, handle, ink, &[a, b], transparency);
    }
    if !text.is_empty() {
        draw_text(list, handle, ink, transparency, at, text, radius);
    }
}

pub(crate) fn push_view_extras(d: &Drawing, view: &cadcraft_doc::arch::ArchView, list: &mut DisplayList, plotting: bool) {
    let handle = cadcraft_doc::arch::view_handle(view.id);
    let layer_ink = |name: &str| {
        let c = d.layer(name).map(|l| l.color).unwrap_or(Color::Index(7));
        crate::ink(Color::ByLayer, c, Color::Index(7))
    };

    if let Some(g) = &view.generated {
        if !arch_layer_skipped(d, cadcraft_doc::arch::LAYER_ELEV_PATTERN, plotting) {
            let ink = layer_ink(cadcraft_doc::arch::LAYER_ELEV_PATTERN);
            let transparency = layer_transparency(d, cadcraft_doc::arch::LAYER_ELEV_PATTERN);
            for line in g.patterns.iter().take(MAX_VIEW_ITEMS) {
                let (Some(a), Some(b)) = (placed_point(view, line.a), placed_point(view, line.b)) else { continue };
                push_polyline_with_transparency(list, handle, &ink, &[a, b], transparency);
            }
        }

        if !arch_layer_skipped(d, cadcraft_doc::arch::LAYER_ELEV_SWING, plotting) {
            let ink = layer_ink(cadcraft_doc::arch::LAYER_ELEV_SWING);
            let transparency = layer_transparency(d, cadcraft_doc::arch::LAYER_ELEV_SWING);
            for line in g.indicators.iter().take(MAX_VIEW_ITEMS) {
                let (Some(a), Some(b)) = (placed_point(view, line.a), placed_point(view, line.b)) else { continue };
                push_polyline_with_transparency(list, handle, &ink, &[a, b], transparency);
            }
        }

        for label in g.labels.iter().take(MAX_VIEW_ITEMS) {
            if arch_layer_skipped(d, &label.layer, plotting) {
                continue;
            }
            let Some(at) = placed_point(view, label.at) else { continue };
            let ink = layer_ink(&label.layer);
            let transparency = layer_transparency(d, &label.layer);
            draw_text(list, handle, &ink, transparency, at, &label.text, label.height);
        }
    }

    for note in view.notes.iter().take(MAX_VIEW_ITEMS) {
        let layer = note.layer.as_str();
        if arch_layer_skipped(d, layer, plotting) {
            continue;
        }
        let Some(at) = placed_point(view, note.at) else { continue };
        let ink = layer_ink(layer);
        let transparency = layer_transparency(d, layer);
        match note.kind {
            NoteKind::Text => draw_text(list, handle, &ink, transparency, at, &note.text, note.height),
            NoteKind::Marker => {
                if note.height <= 0.0 || !note.height.is_finite() {
                    continue;
                }
                draw_marker(list, handle, &ink, transparency, at, note.height, &note.text);
            }
            NoteKind::Dimension => {
                let Some(b) = note.b.and_then(|p| placed_point(view, p)) else { continue };
                let layer = "A-ANNO-DIMS";
                if arch_layer_skipped(d, layer, plotting) {
                    continue;
                }
                let ink = layer_ink(layer);
                let transparency = layer_transparency(d, layer);
                let k = if view.scale.is_finite() && view.scale > 0.0 { view.scale / 1000.0 } else { 0.05 };
                let offset = if note.offset[1].abs() <= 1e-12 { 0.5 } else { note.offset[1] };
                let len = at.dist(b);
                if len < 1e-12 || !len.is_finite() {
                    continue;
                }
                let dir = (b - at) * (1.0 / len);
                let on = at + dir.perp() * offset;
                let value = (at.x - b.x).hypot(at.y - b.y);
                let text = format!("{:.0}", value * 1000.0);
                push_dim(list, handle, &ink, transparency, k, at, b, on, &text);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DisplayList, Kind, Options, build_arch};
    use cadcraft_color::Color;
    use cadcraft_doc::Drawing;
    use cadcraft_doc::arch::{ArchId, ArchView, NoteKind, ViewGeometry, ViewLabel, ViewLine, ViewNote, ViewPatternLine, view_handle};
    use cadcraft_geom::Vec2;

    fn new_drawing() -> Drawing {
        let mut d = Drawing::new_metric();
        for (name, idx) in
            [(cadcraft_doc::arch::LAYER_ELEV_PATTERN, 1), (cadcraft_doc::arch::LAYER_ELEV_SWING, 2), ("A-ANNO-DIMS", 3), ("A-ELEV-IDEN", 4)]
        {
            d.ensure_layer(name);
            if let Some(layer) = d.layer_mut(name) {
                layer.color = Color::Index(idx);
            }
        }
        d
    }

    fn polyline_pts<'a>(list: &'a DisplayList, p: &crate::DPrim) -> &'a [Vec2] {
        &list.verts[p.start as usize..(p.start + p.len) as usize]
    }

    #[test]
    fn patterns_draw_on_a_elev_patt() {
        let mut d = new_drawing();
        let view = ArchView {
            name: "Pat".into(),
            insert: [10.0, 20.0],
            scale: 50.0,
            generated: Some(ViewGeometry {
                extent: [0.0, 0.0, 5.0, 5.0],
                patterns: vec![ViewPatternLine { a: [1.0, 1.0], b: [4.0, 1.0], ..Default::default() }],
                ..Default::default()
            }),
            ..ArchView::default()
        };
        let id = d.arch.push_view(view).expect("valid view");
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(id);
        let ink = cadcraft_color::aci_rgb(1);
        assert!(list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Polyline && p.color == ink));
    }

    #[test]
    fn indicators_and_tags_draw_on_their_layers() {
        let mut d = new_drawing();
        let view = ArchView {
            name: "Ind".into(),
            insert: [0.0, 0.0],
            scale: 50.0,
            generated: Some(ViewGeometry {
                extent: [0.0, 0.0, 1.0, 1.0],
                indicators: vec![ViewLine { a: [0.0, 0.0], b: [1.0, 0.0], ..Default::default() }],
                labels: vec![ViewLabel { text: "D1".into(), at: [0.5, 0.5], height: 0.1, layer: "A-ELEV-IDEN".into(), ..Default::default() }],
                ..Default::default()
            }),
            ..ArchView::default()
        };
        let id = d.arch.push_view(view).expect("valid view");
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(id);
        assert!(list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Polyline && p.color == cadcraft_color::aci_rgb(2)));
        assert!(list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Polyline && p.color == cadcraft_color::aci_rgb(4)));
    }

    #[test]
    fn notes_draw_at_their_placed_points() {
        let mut d = new_drawing();
        let note = ViewNote { id: 1, text: "N1".into(), at: [1.0, 1.0], height: 0.2, layer: "A-ELEV-IDEN".into(), ..Default::default() };
        let view = ArchView { name: "Note".into(), insert: [10.0, 20.0], scale: 50.0, notes: vec![note], ..ArchView::default() };
        let id = d.arch.push_view(view).expect("valid view");
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(id);
        let text = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Polyline).collect::<Vec<_>>();
        assert!(!text.is_empty());
        assert!(text.iter().any(|p| polyline_pts(&list, p).iter().any(|pt| (pt.x - 11.0).abs() < 1.0 && (pt.y - 21.0).abs() < 1.0)));
    }

    #[test]
    fn dimension_notes_use_push_dim() {
        let mut d = new_drawing();
        let view = ArchView {
            name: "Dim".into(),
            insert: [0.0, 0.0],
            scale: 50.0,
            notes: vec![ViewNote {
                id: 2,
                kind: NoteKind::Dimension,
                text: "d".into(),
                at: [0.0, 0.0],
                b: Some([3.0, 0.0]),
                height: 0.2,
                layer: "A-ANNO-DIMS".into(),
                ..Default::default()
            }],
            ..ArchView::default()
        };
        let id = d.arch.push_view(view).expect("valid view");
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(id);
        assert!(list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Polyline && p.color == cadcraft_color::aci_rgb(3)));
    }

    #[test]
    fn skipped_and_transparent_layers_are_honoured() {
        let mut d = new_drawing();
        let view = ArchView {
            name: "Skip".into(),
            insert: [0.0, 0.0],
            scale: 50.0,
            generated: Some(ViewGeometry {
                extent: [0.0, 0.0, 1.0, 1.0],
                patterns: vec![ViewPatternLine { a: [0.0, 0.0], b: [1.0, 0.0], ..Default::default() }],
                ..Default::default()
            }),
            ..ArchView::default()
        };
        let id = d.arch.push_view(view).expect("valid view");
        d.layer_mut(cadcraft_doc::arch::LAYER_ELEV_PATTERN).unwrap().on = false;
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(id);
        assert!(!list.prims.iter().any(|p| p.handle == h && p.color == cadcraft_color::aci_rgb(1)));

        let mut d = new_drawing();
        let view = ArchView {
            name: "Trans".into(),
            insert: [0.0, 0.0],
            scale: 50.0,
            generated: Some(ViewGeometry {
                extent: [0.0, 0.0, 1.0, 1.0],
                patterns: vec![ViewPatternLine { a: [0.0, 0.0], b: [1.0, 0.0], ..Default::default() }],
                ..Default::default()
            }),
            ..ArchView::default()
        };
        let id = d.arch.push_view(view).expect("valid view");
        d.layer_mut(cadcraft_doc::arch::LAYER_ELEV_PATTERN).unwrap().transparency = 42;
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.iter().any(|p| p.handle == view_handle(id) && p.transparency == 42));
    }

    #[test]
    fn nothing_without_generated_geometry() {
        let mut d = new_drawing();
        let view = ArchView { name: "Empty".into(), insert: [0.0, 0.0], scale: 50.0, ..ArchView::default() };
        let id = d.arch.push_view(view).expect("valid view");
        let list = build_arch(&d, &Options::default(), false);
        assert!(!list.prims.iter().any(|p| p.handle == view_handle(id)));
    }

    #[test]
    fn nan_pattern_endpoints_are_skipped() {
        let mut d = new_drawing();
        let view = ArchView {
            name: "NanPat".into(),
            insert: [0.0, 0.0],
            scale: 50.0,
            generated: Some(ViewGeometry {
                extent: [0.0, 0.0, 1.0, 1.0],
                patterns: vec![
                    ViewPatternLine { a: [f64::NAN, 0.0], b: [1.0, 0.0], ..Default::default() },
                    ViewPatternLine { a: [0.0, 0.0], b: [f64::NAN, 0.0], ..Default::default() },
                    ViewPatternLine { a: [0.0, 0.0], b: [1.0, 0.0], ..Default::default() },
                ],
                ..Default::default()
            }),
            ..ArchView::default()
        };
        let id = d.arch.push_view(view).expect("valid view");
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(id);
        // Only the finite pattern line draws (2 vertices)
        let pattern_prims: Vec<_> = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Polyline).collect();
        assert_eq!(pattern_prims.len(), 1, "only the finite pattern line should draw");
    }

    #[test]
    fn zero_length_dimension_is_skipped() {
        let mut d = new_drawing();
        let view = ArchView {
            name: "ZeroDim".into(),
            insert: [0.0, 0.0],
            scale: 50.0,
            notes: vec![ViewNote {
                id: 1,
                kind: NoteKind::Dimension,
                text: "d".into(),
                at: [1.0, 1.0],
                b: Some([1.0, 1.0]),
                height: 0.2,
                layer: "A-ANNO-DIMS".into(),
                ..Default::default()
            }],
            ..ArchView::default()
        };
        let id = d.arch.push_view(view).expect("valid view");
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(id);
        assert!(!list.prims.iter().any(|p| p.handle == h), "zero-length dimension should draw nothing");
    }

    #[test]
    fn dimension_without_b_is_skipped() {
        let mut d = new_drawing();
        let view = ArchView {
            name: "NoBDim".into(),
            insert: [0.0, 0.0],
            scale: 50.0,
            notes: vec![ViewNote {
                id: 1,
                kind: NoteKind::Dimension,
                text: "d".into(),
                at: [1.0, 1.0],
                b: None,
                height: 0.2,
                layer: "A-ANNO-DIMS".into(),
                ..Default::default()
            }],
            ..ArchView::default()
        };
        let id = d.arch.push_view(view).expect("valid view");
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(id);
        assert!(!list.prims.iter().any(|p| p.handle == h), "dimension without b should draw nothing");
    }

    #[test]
    fn marker_with_zero_height_is_skipped() {
        let mut d = new_drawing();
        let view = ArchView {
            name: "ZeroMarker".into(),
            insert: [0.0, 0.0],
            scale: 50.0,
            notes: vec![ViewNote {
                id: 1,
                kind: NoteKind::Marker,
                text: "M".into(),
                at: [1.0, 1.0],
                height: 0.0,
                layer: "A-ELEV-IDEN".into(),
                ..Default::default()
            }],
            ..ArchView::default()
        };
        let id = d.arch.views.len() as u32;
        d.arch.views.push(view);
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(ArchId(id));
        assert!(!list.prims.iter().any(|p| p.handle == h), "zero-height marker should draw nothing");
    }

    #[test]
    fn marker_with_nan_height_is_skipped() {
        let mut d = new_drawing();
        let view = ArchView {
            name: "NanMarker".into(),
            insert: [0.0, 0.0],
            scale: 50.0,
            notes: vec![ViewNote {
                id: 1,
                kind: NoteKind::Marker,
                text: "M".into(),
                at: [1.0, 1.0],
                height: f64::NAN,
                layer: "A-ELEV-IDEN".into(),
                ..Default::default()
            }],
            ..ArchView::default()
        };
        let id = d.arch.views.len() as u32;
        d.arch.views.push(view);
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(ArchId(id));
        assert!(!list.prims.iter().any(|p| p.handle == h), "NaN-height marker should draw nothing");
    }

    #[test]
    fn notes_draw_without_generated_geometry() {
        let mut d = new_drawing();
        let note = ViewNote { id: 1, text: "N".into(), at: [2.0, 3.0], height: 0.2, layer: "A-ELEV-IDEN".into(), ..Default::default() };
        let view = ArchView { name: "NoGen".into(), insert: [10.0, 20.0], scale: 50.0, notes: vec![note], ..ArchView::default() };
        let id = d.arch.push_view(view).expect("valid view");
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(id);
        let prims: Vec<_> = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Polyline).collect();
        assert!(!prims.is_empty(), "notes should draw even without generated geometry");
        // Note at [2,3] placed at insert [10,20] = [12,23]
        assert!(prims.iter().any(|p| polyline_pts(&list, p).iter().any(|pt| (pt.x - 12.0).abs() < 1.0 && (pt.y - 23.0).abs() < 1.0)));
    }

    #[test]
    fn dimension_with_zero_offset_defaults_to_half() {
        let mut d = new_drawing();
        let view = ArchView {
            name: "ZeroOff".into(),
            insert: [0.0, 0.0],
            scale: 50.0,
            notes: vec![ViewNote {
                id: 1,
                kind: NoteKind::Dimension,
                text: "d".into(),
                at: [0.0, 0.0],
                b: Some([2.0, 0.0]),
                height: 0.2,
                offset: [0.0, 0.0],
                layer: "A-ANNO-DIMS".into(),
                ..Default::default()
            }],
            ..ArchView::default()
        };
        let id = d.arch.push_view(view).expect("valid view");
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(id);
        // Should draw (offset defaults to 0.5)
        assert!(list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Polyline), "dimension with zero offset should draw with default 0.5");
    }

    #[test]
    fn dimension_with_nan_scale_uses_default_k() {
        let mut d = new_drawing();
        let view = ArchView {
            name: "NanScale".into(),
            insert: [0.0, 0.0],
            scale: f64::NAN,
            notes: vec![ViewNote {
                id: 1,
                kind: NoteKind::Dimension,
                text: "d".into(),
                at: [0.0, 0.0],
                b: Some([2.0, 0.0]),
                height: 0.2,
                layer: "A-ANNO-DIMS".into(),
                ..Default::default()
            }],
            ..ArchView::default()
        };
        let id = d.arch.views.len() as u32;
        d.arch.views.push(view);
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(ArchId(id));
        // Should draw with default k=0.05
        assert!(list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Polyline), "dimension with NaN scale should draw with default k");
    }

    #[test]
    fn huge_coordinates_do_not_overflow() {
        let mut d = new_drawing();
        let view = ArchView {
            name: "Huge".into(),
            insert: [1e12, 1e12],
            scale: 50.0,
            generated: Some(ViewGeometry {
                extent: [0.0, 0.0, 1.0, 1.0],
                patterns: vec![ViewPatternLine { a: [0.0, 0.0], b: [1.0, 0.0], ..Default::default() }],
                ..Default::default()
            }),
            ..ArchView::default()
        };
        let id = d.arch.views.len() as u32;
        d.arch.views.push(view);
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(ArchId(id));
        let prims: Vec<_> = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Polyline).collect();
        assert!(!prims.is_empty(), "huge coordinates should draw");
        for p in &prims {
            for pt in polyline_pts(&list, p) {
                assert!(pt.x.is_finite() && pt.y.is_finite(), "vertices must be finite");
            }
        }
    }

    #[test]
    fn empty_marker_text_draws_only_circle() {
        let mut d = new_drawing();
        let view = ArchView {
            name: "EmptyText".into(),
            insert: [0.0, 0.0],
            scale: 50.0,
            notes: vec![ViewNote {
                id: 1,
                kind: NoteKind::Marker,
                text: "".into(),
                at: [1.0, 1.0],
                height: 0.3,
                layer: "A-ELEV-IDEN".into(),
                ..Default::default()
            }],
            ..ArchView::default()
        };
        let id = d.arch.views.len() as u32;
        d.arch.views.push(view);
        let list = build_arch(&d, &Options::default(), false);
        let h = view_handle(ArchId(id));
        // 24 segments for the circle, no text
        let prims: Vec<_> = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Polyline).collect();
        assert_eq!(prims.len(), 24, "marker with empty text should draw 24 circle segments");
    }
}
