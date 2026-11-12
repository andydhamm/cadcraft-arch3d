//! View generation (phase 7): elevations and sections of the model as 2D linework, poché rings and overall
//! dimensions, all from the one hidden-line / section kernel ([`crate::hlr`], [`crate::section`]).
//!
//! Phase 21 plugs its features in through the sub-modules: [`interior`] (interior elevations), [`patterns`] (material
//! pattern lines), [`indicators`] (door swings and the like), [`tags`] (labels), [`interior_dims`] and [`notes`].
//!
//! Output is deterministic: lines and rings are sorted with `total_cmp`, so the same model and view always give
//! identical geometry.

pub(crate) mod indicators;
pub(crate) mod interior;
pub(crate) mod interior_dims;
pub mod notes;
pub(crate) mod patterns;
pub(crate) mod tags;

use crate::hlr::{Line2, OrthoView, merge_collinear, visible_lines};
use crate::section::{Plane, section_view};
use crate::{Mesh, Scene3d, build};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchView, DimKind, MAX_VIEW_LINES, ViewDim, ViewGeometry, ViewKind, ViewLabel, ViewLine, ViewPatternLine, ViewRing};
use std::cmp::Ordering;

/// How far (metres) a dimension line sits outside the extent.
const DIM_OFFSET: f64 = 0.75;

/// An empty geometry for `revision` carrying `error`.
fn failed(revision: u64, error: impl Into<String>) -> ViewGeometry {
    ViewGeometry { revision, error: Some(error.into()), ..Default::default() }
}

/// Plan bounds `[x0, y0, x1, y1]` of a mesh, `None` when it has no vertices.
fn plan_bounds(m: &Mesh) -> Option<[f64; 4]> {
    let mut it = m.positions.iter();
    let f = it.next()?;
    let mut b = [f64::from(f[0]), f64::from(f[1]), f64::from(f[0]), f64::from(f[1])];
    for p in it {
        let (x, y) = (f64::from(p[0]), f64::from(p[1]));
        b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
    }
    Some(b)
}

/// The scene without the meshes outside the view's scope, or (sections) farther than its depth beyond `plane`.
fn restrict(scene: &Scene3d, view: &ArchView, plane: Option<&Plane>) -> Scene3d {
    let scope = view.scope.map(|s| [s[0][0].min(s[1][0]), s[0][1].min(s[1][1]), s[0][0].max(s[1][0]), s[0][1].max(s[1][1])]);
    let depth = view.depth.filter(|d| d.is_finite());
    let keep = |m: &Mesh| {
        if let (Some(s), Some(b)) = (scope, plan_bounds(m)) {
            if b[2] < s[0] || b[0] > s[2] || b[3] < s[1] || b[1] > s[3] {
                return false;
            }
        }
        if let (Some(pl), Some(d)) = (plane, depth) {
            let nearest = m.positions.iter().map(|p| pl.distance([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])])).fold(f64::INFINITY, f64::min);
            if nearest > d {
                return false;
            }
        }
        true
    };
    let meshes = scene.meshes.iter().filter(|m| keep(m)).cloned().collect();
    Scene3d { meshes, ..Scene3d::default() }
}

fn cmp_pt(a: [f64; 2], b: [f64; 2]) -> Ordering {
    a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1]))
}

fn cmp_pts(a: &[[f64; 2]], b: &[[f64; 2]]) -> Ordering {
    for (p, q) in a.iter().zip(b) {
        let o = cmp_pt(*p, *q);
        if o != Ordering::Equal {
            return o;
        }
    }
    a.len().cmp(&b.len())
}

/// The overall width and height dimensions of `extent` when the view asks for dimensions.
pub(crate) fn overall_dims(view: &ArchView, extent: Option<[f64; 4]>) -> Vec<ViewDim> {
    let mut dims = Vec::new();
    if let (true, Some([u0, v0, u1, v1])) = (view.dimensions, extent) {
        let mm = |v: f64| format!("{:.0}", v * 1000.0);
        dims.push(ViewDim {
            kind: DimKind::Width,
            a: [u0, v0],
            b: [u1, v0],
            at: [(u0 + u1) / 2.0, v0 - DIM_OFFSET],
            value: u1 - u0,
            text: mm(u1 - u0),
            ..Default::default()
        });
        dims.push(ViewDim {
            kind: DimKind::Height,
            a: [u0, v0],
            b: [u0, v1],
            at: [u0 - DIM_OFFSET, (v0 + v1) / 2.0],
            value: v1 - v0,
            text: mm(v1 - v0),
            ..Default::default()
        });
    }
    dims
}

/// Bounds `[u0, v0, u1, v1]` of the lines and ring points only (patterns, indicators and labels never grow it).
fn extent_of(lines: &[ViewLine], rings: &[ViewRing]) -> Option<[f64; 4]> {
    let mut ext: Option<[f64; 4]> = None;
    let mut grow = |p: [f64; 2]| {
        ext = Some(match ext {
            Some(e) => [e[0].min(p[0]), e[1].min(p[1]), e[2].max(p[0]), e[3].max(p[1])],
            None => [p[0], p[1], p[0], p[1]],
        });
    };
    for l in lines {
        grow(l.a);
        grow(l.b);
    }
    for r in rings {
        r.points.iter().for_each(|p| grow(*p));
    }
    ext
}

/// The extras of a view: pattern lines, indicator lines and labels.
#[derive(Default)]
struct Extras {
    patterns: Vec<ViewPatternLine>,
    indicators: Vec<ViewLine>,
    labels: Vec<ViewLabel>,
}

/// Sort, cap and assemble the geometry. `dims` is given the extent of lines and rings.
fn assemble(
    revision: u64,
    lines: Vec<Line2>,
    rings: Vec<ViewRing>,
    extras: Extras,
    dims: impl FnOnce(Option<[f64; 4]>, &[ViewLine]) -> Vec<ViewDim>,
) -> ViewGeometry {
    let points: usize = rings.iter().map(|r| r.points.len()).sum();
    if lines.len().saturating_add(points) > MAX_VIEW_LINES {
        return failed(revision, format!("the view has more than {MAX_VIEW_LINES} lines"));
    }

    let mut lines: Vec<ViewLine> =
        lines.into_iter().map(|l| ViewLine { a: l.a, b: l.b, kind: l.kind, object: l.object, ..Default::default() }).collect();
    lines.sort_by(line_order);
    let mut rings = rings;
    rings.sort_by(|x, y| x.kind.cmp(&y.kind).then(x.object.cmp(&y.object)).then_with(|| cmp_pts(&x.points, &y.points)));

    let ext = extent_of(&lines, &rings);
    let dims = dims(ext, &lines);

    let Extras { mut patterns, mut indicators, mut labels } = extras;
    patterns.truncate(MAX_VIEW_LINES.saturating_sub(lines.len().saturating_add(points)));
    patterns.sort_by(|x, y| x.kind.cmp(&y.kind).then(x.object.cmp(&y.object)).then_with(|| cmp_pt(x.a, y.a)).then_with(|| cmp_pt(x.b, y.b)));
    indicators.sort_by(line_order);
    labels.sort_by(|x, y| x.kind.cmp(&y.kind).then(x.object.cmp(&y.object)).then_with(|| x.text.cmp(&y.text)));

    ViewGeometry { revision, extent: ext.unwrap_or_default(), lines, rings, dims, patterns, indicators, labels, ..Default::default() }
}

fn line_order(x: &ViewLine, y: &ViewLine) -> Ordering {
    x.kind.cmp(&y.kind).then(x.object.cmp(&y.object)).then_with(|| cmp_pt(x.a, y.a)).then_with(|| cmp_pt(x.b, y.b))
}

/// The geometry of `view` over `scene` (built from `model`), stamped with `revision`. Never panics; a view that
/// cannot be made (bad look direction, no section line, too many lines, an interior view not available) gives an
/// empty geometry with `error` set.
pub fn generate(model: &ArchModel, scene: &Scene3d, view: &ArchView, revision: u64) -> ViewGeometry {
    let (sc, ov, lines, rings): (Scene3d, OrthoView, Vec<Line2>, Vec<ViewRing>) = match view.kind {
        ViewKind::Elevation => {
            let Some(ov) = OrthoView::elevation(view.look) else { return failed(revision, "the look direction is not usable") };
            let sc = restrict(scene, view, None);
            let lines = merge_collinear(visible_lines(&sc, &ov));
            (sc, ov, lines, Vec::new())
        }
        ViewKind::Interior => {
            let Some(ov) = OrthoView::elevation(view.look) else { return failed(revision, "the look direction is not usable") };
            let sc = match interior::restrict(model, scene, view) {
                Ok(sc) => sc,
                Err(e) => return failed(revision, e),
            };
            let lines = merge_collinear(visible_lines(&sc, &ov));
            (sc, ov, lines, Vec::new())
        }
        ViewKind::Section => {
            let Some(cut) = view.cut else { return failed(revision, "a section needs a cut line") };
            let (dx, dy) = (cut[1][0] - cut[0][0], cut[1][1] - cut[0][1]);
            let len = dx.hypot(dy);
            if !len.is_finite() || len < 1e-9 {
                return failed(revision, "the cut line has no length");
            }
            // The cut line's normal on the side the viewer looks into.
            let n = [-dy / len, dx / len];
            let k = view.look[0] * n[0] + view.look[1] * n[1];
            if !k.is_finite() || k == 0.0 {
                return failed(revision, "the viewer looks along the cut line");
            }
            let s = k.signum();
            let Some(plane) = Plane::new([cut[0][0], cut[0][1], 0.0], [n[0] * s, n[1] * s, 0.0]) else {
                return failed(revision, "the cut plane is not usable");
            };
            let sc = restrict(scene, view, Some(&plane));
            let Some(sv) = section_view(&sc, &plane, [0.0, 0.0, 1.0]) else { return failed(revision, "the section view is not usable") };
            let mut lines = sv.cut.lines;
            lines.extend(sv.beyond);
            let rings = sv
                .cut
                .rings
                .into_iter()
                .map(|r| ViewRing { kind: r.kind, object: r.object, material: r.material, points: r.points, ..Default::default() })
                .collect();
            let Some(ov) = OrthoView::elevation(view.look) else { return failed(revision, "the look direction is not usable") };
            (sc, ov, lines, rings)
        }
    };

    let patterns = match view.kind {
        ViewKind::Elevation | ViewKind::Interior => patterns::pattern_lines(model, &sc, &ov),
        ViewKind::Section => Vec::new(),
    };
    let indicators = indicators::indicator_lines(model, &sc, view, &ov);
    let interior = view.kind == ViewKind::Interior;
    let mut g = assemble(revision, lines, rings, Extras { patterns, indicators, labels: Vec::new() }, |ext, _| {
        if interior { interior_dims::interior_dims(model, view, &sc, &ov, ext.unwrap_or_default()) } else { overall_dims(view, ext) }
    });
    if g.error.is_none() {
        let mut labels = tags::tag_labels(model, view, &ov, &g.lines);
        labels.sort_by(|x, y| x.kind.cmp(&y.kind).then(x.object.cmp(&y.object)).then_with(|| x.text.cmp(&y.text)));
        g.labels = labels;
    }
    g
}

/// Regenerate every view of `model` whose geometry is missing or not for `revision`; returns how many. The scene is
/// built once. `arch_revision` is not touched (it lives on the drawing, not the model).
pub fn refresh(model: &mut ArchModel, revision: u64) -> usize {
    if !model.views.iter().any(|v| v.is_stale(revision)) {
        return 0;
    }
    let scene = build(model);
    let model_ref: &ArchModel = model;
    let mut made: Vec<(usize, Option<cadcraft_doc::ArchId>, ViewGeometry)> = Vec::new();
    for (i, v) in model_ref.views.iter().enumerate() {
        if !v.is_stale(revision) {
            continue;
        }
        let room = if v.kind == ViewKind::Interior { interior::resolve_room(model_ref, v) } else { None };
        // Generate against the view as it will be stored, so the stored room is the one used.
        let g = match room {
            Some(r) if v.room != Some(r) => {
                let mut tmp = v.clone();
                tmp.room = Some(r);
                generate(model_ref, &scene, &tmp, revision)
            }
            _ => generate(model_ref, &scene, v, revision),
        };
        made.push((i, room, g));
    }
    let n = made.len();
    for (i, room, g) in made {
        if let Some(v) = model.views.get_mut(i) {
            if let Some(r) = room {
                v.room = Some(r);
            }
            v.notes = notes::resolve(&v.notes, &g);
            v.generated = Some(g);
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchKind, ViewNote};

    fn line(a: [f64; 2], b: [f64; 2]) -> Line2 {
        Line2::new(a, b, ArchKind::Wall, cadcraft_doc::ArchId(0))
    }

    #[test]
    fn interior_without_a_room_reports_an_error() {
        let m = ArchModel::default();
        let v = ArchView { kind: ViewKind::Interior, look: [0.0, 1.0], ..Default::default() };
        let g = generate(&m, &Scene3d::default(), &v, 3);
        assert_eq!(g.revision, 3);
        assert_eq!(g.error.as_deref(), Some("the room of  is gone"));
        assert!(g.lines.is_empty() && g.rings.is_empty() && g.dims.is_empty());
    }

    #[test]
    fn extras_never_grow_the_extent() {
        let extras = Extras {
            patterns: vec![ViewPatternLine { a: [-50.0, -50.0], b: [90.0, 90.0], ..Default::default() }],
            indicators: vec![ViewLine { a: [-70.0, 0.0], b: [70.0, 5.0], ..Default::default() }],
            labels: vec![ViewLabel { at: [100.0, 100.0], ..Default::default() }],
        };
        let v = ArchView { dimensions: true, ..Default::default() };
        let g = assemble(1, vec![line([0.0, 0.0], [4.0, 0.0]), line([4.0, 0.0], [4.0, 3.0])], Vec::new(), extras, |e, _| overall_dims(&v, e));
        assert_eq!(g.extent, [0.0, 0.0, 4.0, 3.0]);
        assert_eq!((g.patterns.len(), g.indicators.len(), g.labels.len()), (1, 1, 1));
        assert_eq!(g.dims.len(), 2);
        assert!((g.dims[0].value - 4.0).abs() < 1e-12);
    }

    #[test]
    fn patterns_are_capped_with_the_lines() {
        let lines: Vec<Line2> = (0..10).map(|i| line([f64::from(i), 0.0], [f64::from(i), 1.0])).collect();
        let patterns = vec![ViewPatternLine::default(); MAX_VIEW_LINES];
        let g = assemble(1, lines, Vec::new(), Extras { patterns, ..Default::default() }, |_, _| Vec::new());
        assert!(g.error.is_none());
        assert_eq!(g.lines.len(), 10);
        assert_eq!(g.patterns.len(), MAX_VIEW_LINES - 10);
    }

    #[test]
    fn refresh_resolves_notes_after_generating() {
        let mut m = ArchModel::default();
        let mut v = ArchView { look: [0.0, 1.0], ..Default::default() };
        v.notes.push(ViewNote { id: 1, text: "keep".into(), ..Default::default() });
        m.views.push(v);
        assert_eq!(refresh(&mut m, 4), 1);
        let v = &m.views[0];
        assert_eq!(v.generated.as_ref().map(|g| g.revision), Some(4));
        assert_eq!(v.notes.len(), 1);
        assert_eq!(v.notes[0].text, "keep");
    }
}
