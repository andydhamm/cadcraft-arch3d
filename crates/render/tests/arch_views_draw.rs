//! Linked views draw from the geometry stored on `ArchView.generated`: linework on the view's
//! layer, filled poché on `A-SECT-MCUT`, dimensions on `A-ANNO-DIMS`, all tagged with the view's
//! proxy handle and following layer state.

use cadcraft_color::{Color, Rgb};
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::{ArchId, ArchKind, ArchView, ViewDim, ViewGeometry, ViewLine, ViewRing, view_handle};
use cadcraft_geom::Vec2;
use cadcraft_render::{DPrim, DisplayList, Kind, Options, build_arch};

/// A drawing with one view inserted at (10, 20): two lines, one 1 m square ring, one width dim.
fn drawing(generated: bool) -> (Drawing, ArchId) {
    let mut d = Drawing::new_metric();
    for (name, idx) in [("A-ELEV", 1), ("A-SECT-MCUT", 3), ("A-ANNO-DIMS", 5)] {
        if let Some(l) = d.layer_mut(name) {
            l.color = Color::Index(idx);
        }
    }
    let line = |a: [f64; 2], b: [f64; 2]| ViewLine { a, b, object: ArchId(1), ..ViewLine::default() };
    let geometry = ViewGeometry {
        extent: [0.0, 0.0, 4.0, 3.0],
        lines: vec![line([0.0, 0.0], [4.0, 0.0]), line([4.0, 0.0], [4.0, 3.0])],
        rings: vec![ViewRing {
            kind: ArchKind::Wall,
            object: ArchId(1),
            points: vec![[1.0, 1.0], [2.0, 1.0], [2.0, 2.0], [1.0, 2.0]],
            ..ViewRing::default()
        }],
        dims: vec![ViewDim { a: [0.0, 0.0], b: [4.0, 0.0], at: [0.0, -1.0], value: 4.0, text: "4000".into(), ..ViewDim::default() }],
        ..ViewGeometry::default()
    };
    let view = ArchView { name: "South".into(), insert: [10.0, 20.0], scale: 50.0, generated: generated.then_some(geometry), ..ArchView::default() };
    let id = d.arch.push_view(view).expect("view is valid");
    (d, id)
}

fn color_of(d: &Drawing, layer: &str) -> cadcraft_color::Rgb {
    let l = d.layer(layer).expect("layer exists");
    // A prim's colour is the layer's ByLayer ink; read it back through a one-wall-free probe.
    match l.color {
        Color::Index(i) => cadcraft_color::aci_rgb(i),
        _ => cadcraft_color::aci_rgb(7),
    }
}

fn polyline_pts<'a>(list: &'a DisplayList, p: &DPrim) -> &'a [Vec2] {
    &list.verts[p.start as usize..(p.start + p.len) as usize]
}

fn build(d: &Drawing, plotting: bool) -> DisplayList {
    build_arch(d, &Options::default(), plotting)
}

#[test]
fn a_view_draws_its_lines_at_its_insertion_point() {
    let (d, id) = drawing(true);
    let list = build(&d, false);
    let h = view_handle(id);
    let ink = color_of(&d, "A-ELEV");
    let lines: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Polyline && p.color == ink && p.len == 2).collect();
    assert_eq!(lines.len(), 2, "two A-ELEV lines");
    let first = polyline_pts(&list, lines[0]);
    assert_eq!(first, [Vec2::new(10.0, 20.0), Vec2::new(14.0, 20.0)]);
    let second = polyline_pts(&list, lines[1]);
    assert_eq!(second, [Vec2::new(14.0, 20.0), Vec2::new(14.0, 23.0)]);
}

#[test]
fn section_poche_is_filled_on_a_sect_mcut() {
    let (d, id) = drawing(true);
    let list = build(&d, false);
    let h = view_handle(id);
    let ink = color_of(&d, "A-SECT-MCUT");
    let fills: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Tris).collect();
    assert_eq!(fills.len(), 1, "one filled ring");
    assert_eq!(fills[0].color, ink);
    assert_eq!(fills[0].len, 6, "a square is two triangles");
    let tris = &list.tris[fills[0].start as usize..(fills[0].start + fills[0].len) as usize];
    assert!(tris.iter().all(|p| p.x >= 11.0 - 1e-9 && p.x <= 12.0 + 1e-9 && p.y >= 21.0 - 1e-9 && p.y <= 22.0 + 1e-9));
    let outline = list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Polyline && p.color == ink && p.len == 5);
    assert!(outline, "the ring also gets a closed outline");
}

#[test]
fn view_dimensions_draw_ticks_lines_and_text() {
    let (d, id) = drawing(true);
    let list = build(&d, false);
    let h = view_handle(id);
    let ink = color_of(&d, "A-ANNO-DIMS");
    let dims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Polyline && p.color == ink).collect();
    let k = 50.0 / 1000.0;
    // Two extension lines (a -> dimension line + overshoot), the dimension line and two ticks.
    let two_pt: Vec<&&DPrim> = dims.iter().filter(|p| p.len == 2).collect();
    assert!(two_pt.len() >= 5, "extension lines, dimension line and ticks: {}", two_pt.len());
    // Dimension line at y = 20 - 1 between x = 10 and 14.
    assert!(two_pt.iter().any(|p| {
        let v = polyline_pts(&list, p);
        (v[0].y + 1.0 - 20.0).abs() < 1e-9 && (v[1].y + 1.0 - 20.0).abs() < 1e-9 && (v[0].dist(v[1]) - 4.0).abs() < 1e-9
    }));
    // Extension line from (10, 20) overshooting the dimension line (y = 19) by 1.5 mm at scale.
    assert!(two_pt.iter().any(|p| {
        let v = polyline_pts(&list, p);
        v[0].dist(Vec2::new(10.0, 20.0)) < 1e-9 && (v[1].y - (19.0 - 1.5 * k)).abs() < 1e-9
    }));
    // A 45-degree tick of length 3 mm at scale.
    assert!(two_pt.iter().any(|p| {
        let v = polyline_pts(&list, p);
        let d = v[1] - v[0];
        (d.len() - 3.0 * k).abs() < 1e-9 && (d.x.abs() - d.y.abs()).abs() < 1e-9 && d.x.abs() > 1e-9
    }));
    // Text sits above the dimension line midpoint (12, 19).
    let text: Vec<&&DPrim> = dims.iter().filter(|p| p.len > 2 || !two_pt.iter().any(|q| std::ptr::eq(**q, **p))).collect();
    assert!(!text.is_empty(), "the dimension text draws strokes");
    assert!(text.iter().all(|p| polyline_pts(&list, p).iter().all(|q| q.y > 19.0 && (q.x - 12.0).abs() < 1.0)));
}

#[test]
fn a_frozen_view_layer_hides_the_view() {
    let (mut d, id) = drawing(true);
    if let Some(l) = d.layer_mut("A-ELEV") {
        l.frozen = true;
    }
    let list = build(&d, false);
    let h = view_handle(id);
    let ink = color_of(&d, "A-ELEV");
    assert!(!list.prims.iter().any(|p| p.handle == h && p.color == ink), "no A-ELEV prims");
    // Other view layers still draw.
    assert!(list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Tris));
}

#[test]
fn an_ungenerated_view_draws_nothing() {
    let (d, _) = drawing(false);
    let list = build(&d, false);
    assert!(list.prims.is_empty());
}

#[test]
fn the_template_has_the_phase_7_and_railing_layers() {
    for d in [Drawing::new_imperial(), Drawing::new_metric()] {
        for name in ["A-RAIL", "A-ELEV", "A-SECT", "A-SECT-MCUT", "A-ANNO-DIMS"] {
            assert!(d.layer(name).is_some(), "{name} must be in the template");
        }
    }
}

// ---------- hardening tests: hostile / degenerate linked-view geometry ----------

/// Build a drawing carrying an arbitrary generated geometry for its sole view.
fn drawing_with(geometry: ViewGeometry) -> (Drawing, ArchId) {
    let mut d = Drawing::new_metric();
    for (name, idx) in [("A-ELEV", 1), ("A-SECT-MCUT", 3), ("A-ANNO-DIMS", 5)] {
        if let Some(l) = d.layer_mut(name) {
            l.color = Color::Index(idx);
        }
    }
    let view = ArchView { name: "South".into(), insert: [10.0, 20.0], scale: 50.0, generated: Some(geometry), ..ArchView::default() };
    let id = d.arch.push_view(view).expect("view is valid");
    (d, id)
}

/// Point the sole view at an (otherwise-default) layer by name.
fn set_view_layer(d: &mut Drawing, id: ArchId, name: &str) {
    for v in &mut d.arch.views {
        if v.id == id {
            v.layer = name.into();
            break;
        }
    }
}

fn dims_prims<'a>(list: &'a DisplayList, h: cadcraft_doc::Handle, ink: &Rgb) -> Vec<&'a DPrim> {
    list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Polyline && p.color == *ink && p.len == 2).collect()
}

/// While plotting, a no-plot view layer is skipped (as with walls/rooms); a screen build
/// still draws it. The spec calls for no-plot handling during plotting, which the frozen
/// test does not cover.
#[test]
fn a_no_plot_view_layer_hides_the_view() {
    let (mut d, id) = drawing(true);
    let h = view_handle(id);
    if let Some(l) = d.layer_mut("A-ELEV") {
        l.plot = false;
    }
    // Screen build (plotting = false) draws despite no-plot.
    let list = build(&d, false);
    assert!(list.prims.iter().any(|p| p.handle == h && p.color == color_of(&d, "A-ELEV")), "screen build must draw a no-plot view layer");
    // Plot build (plotting = true) skips the A-ELEV lines; the A-SECT-MCUT fill stays.
    let list = build(&d, true);
    assert!(
        !list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Polyline && p.color == color_of(&d, "A-ELEV")),
        "plot build must skip a no-plot view layer"
    );
    assert!(list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Tris), "A-SECT-MCUT is plotting, so the fill still draws");
}

/// A view whose `layer` does not exist in the drawing is never skipped: it draws with the
/// `Color::Index(7)` (aci7) fallback, exactly like a missing `A-WALL`/`A-ROOF` layer.
#[test]
fn a_view_layer_missing_falls_back_to_aci7() {
    let (mut d, id) = drawing(true);
    set_view_layer(&mut d, id, "A-MISSING");
    let h = view_handle(id);
    let list = build(&d, false);
    let a7: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Polyline && p.len == 2 && p.aci7).collect();
    assert_eq!(a7.len(), 2, "a missing view layer must still draw its two lines with aci7");
}

/// Prims carry their layer's transparency (percent), consistent with walls/rooms: lines
/// from A-ELEV, fills from A-SECT-MCUT, dims from A-ANNO-DIMS.
#[test]
fn view_prims_carry_layer_transparency() {
    let (mut d, id) = drawing(true);
    let h = view_handle(id);
    for (name, t) in [("A-ELEV", 40), ("A-SECT-MCUT", 25), ("A-ANNO-DIMS", 35)] {
        if let Some(l) = d.layer_mut(name) {
            l.transparency = t;
        }
    }
    let list = build(&d, false);
    let elev_ink = color_of(&d, "A-ELEV");
    let lines: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Polyline && p.color == elev_ink && p.len == 2).collect();
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|p| p.transparency == 40), "line prims must carry A-ELEV transparency");
    let fills = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Tris).collect::<Vec<&DPrim>>();
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].transparency, 25, "fill must carry A-SECT-MCUT transparency");
    let dims = dims_prims(&list, h, &color_of(&d, "A-ANNO-DIMS"));
    assert!(dims.iter().all(|p| p.transparency == 35), "dim prims must carry A-ANNO-DIMS transparency");
}

/// Non-finite geometry is skipped per-point. A NaN line, a ring with a NaN vertex, and a dim
/// with a NaN endpoint all produce no prims; the view must never emit a NaN vertex.
#[test]
fn a_view_with_nan_geometry_draws_nothing() {
    let (mut d, id) = drawing(true);
    let g = ViewGeometry {
        extent: [0.0, 0.0, 4.0, 3.0],
        lines: vec![
            ViewLine { a: [f64::NAN, 0.0], b: [4.0, 0.0], object: ArchId(1), ..ViewLine::default() },
            ViewLine { a: [0.0, 0.0], b: [f64::INFINITY, 0.0], object: ArchId(1), ..ViewLine::default() },
        ],
        rings: vec![ViewRing {
            kind: ArchKind::Wall,
            object: ArchId(1),
            points: vec![[1.0, f64::NAN], [2.0, 1.0], [2.0, 2.0], [1.0, 2.0]],
            ..ViewRing::default()
        }],
        dims: vec![ViewDim { a: [0.0, f64::NAN], b: [4.0, 0.0], at: [0.0, -1.0], value: 4.0, text: "4000".into(), ..ViewDim::default() }],
        ..ViewGeometry::default()
    };
    d.arch.views.iter_mut().find(|v| v.id == id).unwrap().generated = Some(g);
    let h = view_handle(id);
    let list = build(&d, false);
    assert!(list.prims.iter().all(|p| p.handle != h), "all-NaN geometry must draw no prims");
}

/// A view whose coordinates overflow to infinity on placement is skipped (per-point `at()`
/// returns `None`). Insert + point must not produce NaN or an out-of-range vertex.
#[test]
fn a_view_with_overflowing_geometry_draws_nothing() {
    let mut d = Drawing::new_metric();
    for (name, idx) in [("A-ELEV", 1), ("A-SECT-MCUT", 3), ("A-ANNO-DIMS", 5)] {
        if let Some(l) = d.layer_mut(name) {
            l.color = Color::Index(idx);
        }
    }
    let g = ViewGeometry {
        extent: [0.0, 0.0, 4.0, 3.0],
        lines: vec![ViewLine { a: [1e308, 0.0], b: [1e308, 4.0], object: ArchId(1), ..ViewLine::default() }],
        rings: vec![],
        dims: vec![ViewDim { a: [1e308, 0.0], b: [1e308, 0.0], at: [1e308, -1.0], value: 0.0, text: "x".into(), ..ViewDim::default() }],
        ..ViewGeometry::default()
    };
    // A fresh view placed at a huge insert so every placed point overflows to infinity.
    // `push_view` refuses to store such an insert (InsertTooLarge), so reach the same
    // placed-overflow state the renderer must survive by mutating the stored field — the
    // guard stays in force for programmatic creation.
    let view = ArchView { name: "Huge".into(), insert: [10.0, 20.0], scale: 50.0, generated: Some(g), ..ArchView::default() };
    let hid = d.arch.push_view(view).expect("view is valid");
    let h = view_handle(hid);
    d.arch.views.iter_mut().find(|v| v.id == hid).unwrap().insert = [1e308, 1e308];
    let list = build(&d, false);
    assert!(list.prims.iter().all(|p| p.handle != h), "overflowing geometry must emit no prims");
}

/// A dimension whose `a` and `b` coincide (zero length) is refused in `push_dim` and emits
/// no prims. The surrounding lines must still draw.
#[test]
fn a_zero_length_dimension_draws_nothing() {
    let (mut d, id) = drawing(true);
    let g = d.arch.views.iter_mut().find(|v| v.id == id).unwrap().generated.as_mut().unwrap();
    g.dims[0] = ViewDim { a: [2.0, 0.0], b: [2.0, 0.0], at: [2.0, -1.0], value: 0.0, text: "0".into(), ..g.dims[0] };
    let h = view_handle(id);
    let ink = color_of(&d, "A-ANNO-DIMS");
    let list = build(&d, false);
    assert!(dims_prims(&list, h, &ink).is_empty(), "a zero-length dimension must draw no prims");
    assert!(
        list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Polyline && p.color == color_of(&d, "A-ELEV") && p.len == 2),
        "the view's lines must still draw"
    );
}

/// A ring with fewer than 3 points is neither filled nor outlined.
#[test]
fn a_two_point_ring_draws_nothing() {
    let (mut d, id) = drawing(true);
    let g = d.arch.views.iter_mut().find(|v| v.id == id).unwrap().generated.as_mut().unwrap();
    g.rings[0].points = vec![[1.0, 1.0], [2.0, 1.0]];
    g.lines.clear();
    g.dims.clear();
    let h = view_handle(id);
    let ink = color_of(&d, "A-SECT-MCUT");
    let list = build(&d, false);
    assert!(!list.prims.iter().any(|p| p.handle == h && p.color == ink), "a 2-point ring must draw no fill and no outline");
}

/// A collinear ring has no interior to triangulate; only its closed outline draws, and every
/// emitted vertex is finite (no NaN from a zero-area polygon).
#[test]
fn a_collinear_ring_stays_finite() {
    let (mut d, id) = drawing(true);
    let g = d.arch.views.iter_mut().find(|v| v.id == id).unwrap().generated.as_mut().unwrap();
    g.rings[0].points = vec![[1.0, 1.0], [2.0, 1.0], [3.0, 1.0]];
    g.lines.clear();
    g.dims.clear();
    let h = view_handle(id);
    let list = build(&d, false);
    assert!(list.verts.iter().all(|p| p.is_finite()) && list.tris.iter().all(|p| p.is_finite()), "collinear ring must not emit NaN vertices");
    assert!(list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Polyline && p.len == 4), "collinear ring must still draw its outline");
}

/// An empty dim `text` still draws the 5 geometric prims (2 extension, 1 dim line, 2 ticks)
/// and no text strokes.
#[test]
fn a_view_with_empty_dimension_text_draws_no_text_prims() {
    let (mut d, id) = drawing(true);
    let g = d.arch.views.iter_mut().find(|v| v.id == id).unwrap().generated.as_mut().unwrap();
    g.dims[0].text = "".into();
    let h = view_handle(id);
    let ink = color_of(&d, "A-ANNO-DIMS");
    let list = build(&d, false);
    let dims = dims_prims(&list, h, &ink);
    assert_eq!(dims.len(), 5, "empty text must still draw 2 extensions + dim line + 2 ticks");
    assert_eq!(list.prims.iter().filter(|p| p.handle == h && p.color == ink).collect::<Vec<_>>().len(), 5, "no text strokes for empty text",);
}

/// A left-measuring dimension (a -> b pointing -x) still keeps its text upright and centred
/// above the line. The `dir.x < -1e-12` flip is what puts the text above; without it the text
/// would render flipped (below the line).
#[test]
fn a_left_pointing_dimension_draws_its_text_above_the_line() {
    let (mut d, id) = drawing(true);
    let g = d.arch.views.iter_mut().find(|v| v.id == id).unwrap().generated.as_mut().unwrap();
    g.lines.clear();
    g.rings.clear();
    g.dims[0] = ViewDim { a: [4.0, 0.0], b: [0.0, 0.0], at: [0.0, -1.0], value: 4.0, text: "4000".into(), ..g.dims[0] };
    let h = view_handle(id);
    let ink = color_of(&d, "A-ANNO-DIMS");
    let list = build(&d, false);
    let dims = dims_prims(&list, h, &ink);
    // The dimension line runs (14, 19) -> (10, 19): 4 units at y = 19.
    assert!(
        dims.iter().any(|p| {
            let v = polyline_pts(&list, p);
            (v[0].y - 19.0).abs() < 1e-9 && (v[1].y - 19.0).abs() < 1e-9 && (v[0].x - v[1].x).abs().abs() - 4.0 < 1e-6
        }),
        "left-pointing dimension line must be 4 units long at y = 19"
    );
    // The (upright) text is a centred stroke prim around x = 12, sitting above the line.
    assert!(
        list.prims.iter().any(|p| {
            let v = polyline_pts(&list, p);
            v.iter().all(|q| q.y > 19.0 + 1e-6 && (q.x - 12.0).abs() < 1.5)
        }),
        "left-pointing dimension text must sit centred above the line"
    );
}

/// The per-view `MAX_VIEW_ITEMS` (100_000) loop bound caps how many view lines a hostile
/// `generated` geometry can contribute, so a giant geometry can never blow up the prim list.
#[test]
fn a_view_with_many_lines_is_capped() {
    let mut lines = Vec::with_capacity(100_005);
    for i in 0..100_005usize {
        let x = i as f64 * 0.01;
        lines.push(ViewLine { a: [x, 0.0], b: [x + 1.0, 0.0], object: ArchId(1), ..ViewLine::default() });
    }
    let g = ViewGeometry { extent: [0.0, 0.0, 4.0, 3.0], lines, rings: vec![], dims: vec![], ..ViewGeometry::default() };
    let (d, id) = drawing_with(g);
    let h = view_handle(id);
    let ink = color_of(&d, "A-ELEV");
    let list = build(&d, false);
    let drawn = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Polyline && p.color == ink && p.len == 2).count();
    assert_eq!(drawn, 100_000, "view lines must be capped at MAX_VIEW_ITEMS (100_000), got {}", drawn);
}

/// Two views each draw their own linework tagged with their own proxy handle at their own
/// insertion point; the handles are distinct.
#[test]
fn multiple_views_get_distinct_handles() {
    let mut d = Drawing::new_metric();
    for (name, idx) in [("A-ELEV", 1), ("A-SECT-MCUT", 3), ("A-ANNO-DIMS", 5)] {
        if let Some(l) = d.layer_mut(name) {
            l.color = Color::Index(idx);
        }
    }
    let line = ViewLine { a: [0.0, 0.0], b: [1.0, 0.0], object: ArchId(1), ..ViewLine::default() };
    let g = ViewGeometry { extent: [0.0, 0.0, 1.0, 1.0], lines: vec![line.clone()], rings: vec![], dims: vec![], ..ViewGeometry::default() };
    let id0 = d
        .arch
        .push_view(ArchView { name: "V1".into(), insert: [10.0, 20.0], scale: 1.0, generated: Some(g.clone()), ..ArchView::default() })
        .expect("view1");
    let id1 = d
        .arch
        .push_view(ArchView { name: "V2".into(), insert: [100.0, 200.0], scale: 1.0, generated: Some(g), ..ArchView::default() })
        .expect("view2");
    let h0 = view_handle(id0);
    let h1 = view_handle(id1);
    assert_ne!(h0, h1, "view handles must be distinct");
    let ink = color_of(&d, "A-ELEV");
    let list = build(&d, false);
    let l0: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h0 && p.kind == Kind::Polyline && p.color == ink && p.len == 2).collect();
    let l1: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h1 && p.kind == Kind::Polyline && p.color == ink && p.len == 2).collect();
    assert_eq!(l0.len(), 1, "view 1 must draw one line");
    assert_eq!(l1.len(), 1, "view 2 must draw one line");
    assert_eq!(polyline_pts(&list, &l0[0]), [Vec2::new(10.0, 20.0), Vec2::new(11.0, 20.0)], "view 1 at its insert");
    assert_eq!(polyline_pts(&list, &l1[0]), [Vec2::new(100.0, 200.0), Vec2::new(101.0, 200.0)], "view 2 at its insert");
}

/// A NaN in the view's insert trips the renderer's whole-view finiteness guard and nothing
/// is drawn per-point; no NaN vertex escapes the list.
#[test]
fn a_view_with_nan_insert_draws_nothing() {
    let (mut d, id) = drawing(true);
    d.arch.views.iter_mut().find(|v| v.id == id).unwrap().insert = [f64::NAN, 20.0];
    let h = view_handle(id);
    let list = build(&d, false);
    assert!(list.prims.iter().all(|p| p.handle != h), "a NaN insert must draw no prims");
    assert!(list.verts.iter().all(|p| p.is_finite()), "a NaN insert must not leak NaN vertices");
}

/// A huge (but finite) scale grows the dimension ticks and text size; the placed geometry
/// itself stays finite, so no vertex or triangle may overflow to inf or NaN.
#[test]
fn a_view_with_huge_scale_draws_only_finite_vertices() {
    let (mut d, id) = drawing(true);
    d.arch.views.iter_mut().find(|v| v.id == id).unwrap().scale = 1e12;
    let h = view_handle(id);
    let list = build(&d, false);
    assert!(
        list.verts.iter().all(|p| p.is_finite()) && list.tris.iter().all(|p| p.is_finite()),
        "a huge scale must not overflow vertices or tris to inf or NaN",
    );
    assert!(list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Tris), "the fill must still draw");
}

/// The ring loop is capped at `MAX_VIEW_ITEMS` like the line loop: 100 005 valid triangle
/// rings draw exactly 100 000 fills and 100 000 outlines, no more.
#[test]
fn a_view_with_many_rings_is_capped() {
    let mut rings = Vec::with_capacity(100_005);
    for i in 0..100_005usize {
        let x = (i % 7) as f64;
        rings.push(ViewRing {
            kind: ArchKind::Wall,
            object: ArchId(1),
            points: vec![[x, 0.0], [x + 0.1, 0.0], [x + 0.1, 0.1]],
            ..ViewRing::default()
        });
    }
    let (d, id) = drawing_with(ViewGeometry { extent: [0.0, 0.0, 4.0, 3.0], lines: vec![], rings, dims: vec![], ..ViewGeometry::default() });
    let h = view_handle(id);
    let ink = color_of(&d, "A-SECT-MCUT");
    let list = build(&d, false);
    let tris = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Tris && p.color == ink).count();
    let outlines = list.prims.iter().filter(|p| p.handle == h && p.kind == Kind::Polyline && p.color == ink && p.len == 4).count();
    assert_eq!(tris, 100_000, "ring fills must be capped at MAX_VIEW_ITEMS, got {tris}");
    assert_eq!(outlines, 100_000, "ring outlines must be capped at MAX_VIEW_ITEMS, got {outlines}");
}

/// Freezing A-SECT-MCUT removes only the filled poché; the view's line and dimension prims
/// draw from their own (still-visible) layers.
#[test]
fn a_frozen_sect_mcut_layer_hides_only_the_fill() {
    let (mut d, id) = drawing(true);
    if let Some(l) = d.layer_mut("A-SECT-MCUT") {
        l.frozen = true;
    }
    let h = view_handle(id);
    let list = build(&d, false);
    assert!(list.prims.iter().all(|p| p.handle != h || p.kind != Kind::Tris), "freezing A-SECT-MCUT must remove the fill",);
    assert!(
        list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Polyline && p.color == color_of(&d, "A-ELEV")),
        "A-ELEV lines must still draw"
    );
    assert!(
        list.prims.iter().any(|p| p.handle == h && p.kind == Kind::Polyline && p.color == color_of(&d, "A-ANNO-DIMS") && p.len == 2),
        "dimension prims must still draw"
    );
}
