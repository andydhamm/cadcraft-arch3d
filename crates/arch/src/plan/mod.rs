//! The plan annotation pipeline: pure data describing what a plan view adds to the drawn plan
//! (roof linework, room labels, ceiling and reference overlays). `cadcraft-render` draws it and
//! `arch.inspect` reports it; nothing here draws.
//!
//! Each generator lives in its own file and adds only through [`PlanAnnotation::push_line`] and
//! [`PlanAnnotation::push_label`], which refuse hostile geometry and cap the output.

use cadcraft_doc::Handle;
use cadcraft_doc::arch::{ArchModel, PlanKind, PlanView, RoomLabelShow};
use cadcraft_geom::{Vec2, point_in_polygon};

pub mod ceiling;
pub mod labels;
pub mod reference;
pub mod roof;

/// The kind of a [`PlanLine`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanLineKind {
    Ridge,
    Hip,
    Valley,
    GableEnd,
    Eave,
    Rake,
    SlopeArrow,
    SlopeArrowHead,
    TrayOutline,
    TrayTop,
    VaultLine,
    CeilingItem,
    Soffit,
    Reference,
}

impl PlanLineKind {
    /// The kind's name in `arch.inspect`.
    pub fn key(self) -> &'static str {
        match self {
            PlanLineKind::Ridge => "ridge",
            PlanLineKind::Hip => "hip",
            PlanLineKind::Valley => "valley",
            PlanLineKind::GableEnd => "gableEnd",
            PlanLineKind::Eave => "eave",
            PlanLineKind::Rake => "rake",
            PlanLineKind::SlopeArrow => "slopeArrow",
            PlanLineKind::SlopeArrowHead => "slopeArrowHead",
            PlanLineKind::TrayOutline => "trayOutline",
            PlanLineKind::TrayTop => "trayTop",
            PlanLineKind::VaultLine => "vaultLine",
            PlanLineKind::CeilingItem => "ceilingItem",
            PlanLineKind::Soffit => "soffit",
            PlanLineKind::Reference => "reference",
        }
    }
}

/// The kind of a [`PlanLabel`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanLabelKind {
    RoomName,
    RoomArea,
    RoomDimensions,
    RoomCeiling,
    Pitch,
    TrayHeight,
}

impl PlanLabelKind {
    /// The kind's name in `arch.inspect`.
    pub fn key(self) -> &'static str {
        match self {
            PlanLabelKind::RoomName => "roomName",
            PlanLabelKind::RoomArea => "roomArea",
            PlanLabelKind::RoomDimensions => "roomDimensions",
            PlanLabelKind::RoomCeiling => "roomCeiling",
            PlanLabelKind::Pitch => "pitch",
            PlanLabelKind::TrayHeight => "trayHeight",
        }
    }
}

/// One line segment of plan annotation.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanLine {
    pub layer: String,
    pub kind: PlanLineKind,
    pub a: Vec2,
    pub b: Vec2,
    /// The proxy handle picking resolves to.
    pub handle: Handle,
    pub dashed: bool,
    /// An override ink (else the layer's), with its `transparency`.
    pub color: Option<[u8; 3]>,
    pub transparency: Option<u8>,
}

impl PlanLine {
    /// A solid line on `layer` using the layer's own ink.
    pub fn new(layer: &str, kind: PlanLineKind, a: Vec2, b: Vec2, handle: Handle) -> PlanLine {
        PlanLine { layer: layer.to_string(), kind, a, b, handle, dashed: false, color: None, transparency: None }
    }

    /// The same line, dashed.
    pub fn dashed(mut self) -> PlanLine {
        self.dashed = true;
        self
    }

    /// The same line with an override ink and transparency.
    pub fn colored(mut self, color: [u8; 3], transparency: u8) -> PlanLine {
        self.color = Some(color);
        self.transparency = Some(transparency);
        self
    }

    /// The segment's length.
    pub fn length(&self) -> f64 {
        (self.b - self.a).len()
    }
}

/// One text label of plan annotation, centred at `at`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanLabel {
    pub layer: String,
    pub kind: PlanLabelKind,
    pub text: String,
    pub at: Vec2,
    pub height: f64,
    pub rotation: f64,
    pub handle: Handle,
}

impl PlanLabel {
    /// An unrotated label.
    pub fn new(layer: &str, kind: PlanLabelKind, text: impl Into<String>, at: Vec2, height: f64, handle: Handle) -> PlanLabel {
        PlanLabel { layer: layer.to_string(), kind, text: text.into(), at, height, rotation: 0.0, handle }
    }
}

/// The most lines one plan annotation holds.
pub const MAX_PLAN_LINES: usize = 200_000;
/// The most labels one plan annotation holds.
pub const MAX_PLAN_LABELS: usize = 50_000;

/// Everything a plan view adds to the plan, in drawing order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanAnnotation {
    pub lines: Vec<PlanLine>,
    pub labels: Vec<PlanLabel>,
}

impl PlanAnnotation {
    /// Add a line; `false` (nothing added) for a non-finite point, a line shorter than 1e-12 or
    /// when the cap is reached.
    pub fn push_line(&mut self, l: PlanLine) -> bool {
        if self.lines.len() >= MAX_PLAN_LINES || !l.a.is_finite() || !l.b.is_finite() || !(l.length() >= 1e-12) {
            return false;
        }
        self.lines.push(l);
        true
    }

    /// Add a label; `false` (nothing added) for a non-finite point, height or rotation, empty
    /// text or when the cap is reached.
    pub fn push_label(&mut self, l: PlanLabel) -> bool {
        if self.labels.len() >= MAX_PLAN_LABELS || l.text.is_empty() || !l.at.is_finite() || !l.height.is_finite() || !l.rotation.is_finite() {
            return false;
        }
        self.labels.push(l);
        true
    }
}

/// The active plan view, when `model.active_plan_view` names one.
pub fn active_view(model: &ArchModel) -> Option<&PlanView> {
    model.active_plan_view.as_deref().and_then(|n| model.plan_view(n))
}

/// The level `view` opens at: its own level when set and in range, else `model.plan_level`.
pub fn opened_level(model: &ArchModel, view: Option<&PlanView>) -> Option<u32> {
    view.and_then(|v| v.level).filter(|l| usize::try_from(*l).is_ok_and(|i| i < model.levels.len())).or(model.plan_level)
}

/// Everything `view` (the plan with no view when `None`) adds at plan level `level`.
pub fn annotate(model: &ArchModel, view: Option<&PlanView>, level: Option<u32>) -> PlanAnnotation {
    let mut out = PlanAnnotation::default();
    let kind = view.map_or(PlanKind::Floor, |v| v.kind);
    if let (Some(r), Some(l)) = (view.and_then(|v| v.reference), level) {
        if r.level != l {
            reference::reference_lines(model, l, &r, &mut out);
        }
    }
    roof::roof_edges(model, &mut out);
    if kind == PlanKind::Roof {
        roof::roof_plan(model, &mut out);
    }
    if kind == PlanKind::ReflectedCeiling {
        ceiling::ceiling_plan(model, level, &mut out);
    }
    labels::room_labels(model, level, view.map_or_else(RoomLabelShow::default, |v| v.labels), &mut out);
    out
}

/// A pitch as `"6/12"` (whole rise per 12) or `"4.5/12"`; `None` for a flat, negative or
/// non-finite slope.
pub fn pitch_text(slope: f64) -> Option<String> {
    let r = slope * 12.0;
    if !r.is_finite() || r <= 1e-9 {
        return None;
    }
    if (r - r.round()).abs() <= 1e-6 {
        // `r` is finite and positive; a huge value fails the checked conversion.
        let n = i64::try_from(r.round() as i128).ok()?;
        return Some(format!("{n}/12"));
    }
    Some(format!("{r:.1}/12"))
}

const MIN_EDGE_DIST: f64 = 1e-9;
const GRID: usize = 64;
const MAX_GRID_EDGES: usize = 4096;

fn ring_edges<'a>(ring: &'a [Vec2]) -> impl Iterator<Item = (Vec2, Vec2)> + 'a {
    let n = ring.len();
    (0..n).filter_map(move |i| Some((*ring.get(i)?, *ring.get((i + 1) % n)?)))
}

fn edge_distance(p: Vec2, outer: &[Vec2], holes: &[Vec<Vec2>]) -> f64 {
    let mut d = f64::MAX;
    for (a, b) in ring_edges(outer).chain(holes.iter().flat_map(|h| ring_edges(h))) {
        d = d.min(crate::poly::point_segment_dist(p, a, b));
    }
    d
}

fn strictly_in_region(p: Vec2, outer: &[Vec2], holes: &[Vec<Vec2>]) -> bool {
    point_in_polygon(outer, p) && !holes.iter().any(|h| point_in_polygon(h, p)) && edge_distance(p, outer, holes) > MIN_EDGE_DIST
}

/// A label point inside `outer`, outside every hole and away from their edges: the vertex
/// average when it qualifies, else the roomiest cell centre of a 64 x 64 grid over `outer`'s
/// bounds (the first in row-major order on ties), else [`crate::label::label_point`] for rings
/// with more than 4096 edges. `None` for fewer than 3 points, a non-finite point or zero area.
///
/// Why: `label_point` cannot leave a hole centred in its ring (a tray with its nested tray as
/// the hole), where the grid finds a point in the band around it.
pub fn ring_label_point(outer: &[Vec2], holes: &[Vec<Vec2>]) -> Option<Vec2> {
    if outer.len() < 3 || !outer.iter().all(|p| p.is_finite()) || holes.iter().any(|h| !h.iter().all(|p| p.is_finite())) {
        return None;
    }
    if !(crate::poly::ring_area(outer).abs() >= 1e-9) {
        return None;
    }
    let holes: Vec<Vec<Vec2>> = holes.iter().filter(|h| h.len() >= 3).cloned().collect();
    let edges = outer.len() + holes.iter().map(Vec::len).sum::<usize>();
    if edges > MAX_GRID_EDGES {
        return crate::label::label_point(outer, &holes);
    }
    let n = outer.len() as f64;
    let avg = Vec2::new(outer.iter().map(|p| p.x).sum::<f64>() / n, outer.iter().map(|p| p.y).sum::<f64>() / n);
    if avg.is_finite() && strictly_in_region(avg, outer, &holes) {
        return Some(avg);
    }
    let (mut lo, mut hi) = (outer[0], outer[0]);
    for p in outer {
        lo = Vec2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Vec2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let (w, h) = ((hi.x - lo.x) / GRID as f64, (hi.y - lo.y) / GRID as f64);
    let mut best: Option<(f64, Vec2)> = None;
    if w.is_finite() && h.is_finite() {
        for row in 0..GRID {
            for col in 0..GRID {
                let c = Vec2::new(lo.x + (col as f64 + 0.5) * w, lo.y + (row as f64 + 0.5) * h);
                if !strictly_in_region(c, outer, &holes) {
                    continue;
                }
                let d = edge_distance(c, outer, &holes);
                if best.is_none_or(|(bd, _)| d > bd + 1e-12) {
                    best = Some((d, c));
                }
            }
        }
    }
    match best {
        Some((_, c)) => Some(c),
        None => crate::label::label_point(outer, &holes),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::arch::{Roof, RoofEdge, RoofEdgeKind, Room, RoomLabelShow, roof_handle, template_plan_views};

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    fn rect_dist(p: Vec2, r: &[Vec2]) -> f64 {
        edge_distance(p, r, &[])
    }

    #[test]
    fn pitch_text_cases() {
        assert_eq!(pitch_text(0.5).as_deref(), Some("6/12"));
        assert_eq!(pitch_text(1.0 / 12.0).as_deref(), Some("1/12"));
        assert_eq!(pitch_text(0.375).as_deref(), Some("4.5/12"));
        assert_eq!(pitch_text(0.0), None);
        assert_eq!(pitch_text(-0.5), None);
        assert_eq!(pitch_text(f64::NAN), None);
        assert_eq!(pitch_text(f64::INFINITY), None);
        assert_eq!(pitch_text(1e-12), None);
        assert_eq!(pitch_text(1e300), None);
    }

    #[test]
    fn plan_line_and_label_constructors_default_the_optional_fields() {
        let l = PlanLine::new("A-ROOF", PlanLineKind::Ridge, Vec2::ZERO, Vec2::new(3.0, 4.0), Handle(7));
        assert!(!l.dashed && l.color.is_none() && l.transparency.is_none());
        assert_eq!(l.length(), 5.0);
        let d = l.clone().dashed();
        assert!(d.dashed);
        let c = l.colored([1, 2, 3], 40);
        assert_eq!((c.color, c.transparency), (Some([1, 2, 3]), Some(40)));
        let t = PlanLabel::new("A-ROOM", PlanLabelKind::Pitch, "6/12", Vec2::ZERO, 0.2, Handle(1));
        assert_eq!((t.rotation, t.text.as_str()), (0.0, "6/12"));
        assert_eq!(PlanLineKind::SlopeArrowHead.key(), "slopeArrowHead");
        assert_eq!(PlanLabelKind::TrayHeight.key(), "trayHeight");
    }

    #[test]
    fn push_line_and_push_label_refuse_non_finite_empty_and_over_cap() {
        let mut out = PlanAnnotation::default();
        let line = |a: Vec2, b: Vec2| PlanLine::new("L", PlanLineKind::Hip, a, b, Handle(1));
        assert!(out.push_line(line(Vec2::ZERO, Vec2::new(1.0, 0.0))));
        assert!(!out.push_line(line(Vec2::ZERO, Vec2::new(f64::NAN, 0.0))));
        assert!(!out.push_line(line(Vec2::ZERO, Vec2::new(f64::INFINITY, 0.0))));
        assert!(!out.push_line(line(Vec2::ZERO, Vec2::new(1e-13, 0.0))));
        assert_eq!(out.lines.len(), 1);
        let label = |t: &str, at: Vec2, h: f64| PlanLabel::new("L", PlanLabelKind::RoomName, t, at, h, Handle(1));
        assert!(out.push_label(label("a", Vec2::ZERO, 0.2)));
        assert!(!out.push_label(label("", Vec2::ZERO, 0.2)));
        assert!(!out.push_label(label("a", Vec2::new(f64::NAN, 0.0), 0.2)));
        assert!(!out.push_label(label("a", Vec2::ZERO, f64::NAN)));
        let mut bad = label("a", Vec2::ZERO, 0.2);
        bad.rotation = f64::NAN;
        assert!(!out.push_label(bad));
        assert_eq!(out.labels.len(), 1);
        out.lines = vec![line(Vec2::ZERO, Vec2::new(1.0, 0.0)); MAX_PLAN_LINES];
        assert!(!out.push_line(line(Vec2::ZERO, Vec2::new(1.0, 0.0))));
        out.labels = vec![label("a", Vec2::ZERO, 0.2); MAX_PLAN_LABELS];
        assert!(!out.push_label(label("a", Vec2::ZERO, 0.2)));
    }

    #[test]
    fn annotate_calls_the_generators_by_kind_and_order() {
        let mut m = ArchModel::default();
        let rid = m.push_roof(Roof {
            edges: vec![RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::ZERO, b: Vec2::new(4.0, 0.0), ..RoofEdge::default() }],
            ..Roof::default()
        });
        m.push_room(Room { polygon: rect(0.0, 0.0, 4.0, 3.0), name: "A".into(), ..Room::default() });
        let none = annotate(&m, None, None);
        assert_eq!(none.lines.len(), 1);
        assert_eq!(none.lines[0].handle, roof_handle(rid));
        // Default view (Floor Plan) shows name, area, dimensions.
        assert_eq!(none.labels.len(), 3);
        for v in template_plan_views() {
            let a = annotate(&m, Some(&v), Some(0));
            assert_eq!(a.lines.len(), 1, "{}", v.name);
            let expected_labels =
                usize::from(v.labels.name) + usize::from(v.labels.area) + usize::from(v.labels.dimensions) + usize::from(v.labels.ceiling);
            assert_eq!(a.labels.len(), expected_labels, "{}", v.name);
        }
        // A level other than the room's drops its label.
        assert!(annotate(&m, None, Some(1)).labels.is_empty());
        let mut v = PlanView::default();
        v.labels = RoomLabelShow { name: false, area: false, dimensions: false, ceiling: false };
        assert!(annotate(&m, Some(&v), None).labels.is_empty());
        // active_view / opened_level.
        assert!(active_view(&m).is_none());
        m.active_plan_view = Some("Roof Plan".into());
        assert_eq!(active_view(&m).map(|v| v.kind), Some(PlanKind::Roof));
        m.plan_level = Some(0);
        assert_eq!(opened_level(&m, None), Some(0));
        let mut v = PlanView::default();
        v.level = Some(5);
        assert_eq!(opened_level(&m, Some(&v)), Some(0), "out of range falls back");
    }

    #[test]
    fn ring_label_point_cases() {
        // A convex ring -> its vertex average.
        let r = rect(0.0, 0.0, 4.0, 2.0);
        assert_eq!(ring_label_point(&r, &[]), Some(Vec2::new(2.0, 1.0)));
        // Phase 16's tray with its nested child as the hole.
        let tray = rect(0.7, 3.65, 5.35, 7.3);
        let nested = rect(1.3, 4.25, 4.75, 6.7);
        let p = ring_label_point(&tray, &[nested.clone()]).unwrap();
        assert!(point_in_polygon(&tray, p) && !point_in_polygon(&nested, p));
        assert!(rect_dist(p, &tray) >= 0.25 && rect_dist(p, &nested) >= 0.25, "{p:?}");
        // The NW room with that tray as the hole.
        let nw = rect(0.1, 3.05, 5.95, 7.9);
        let q = ring_label_point(&nw, &[tray.clone()]).unwrap();
        assert!(point_in_polygon(&nw, q) && !point_in_polygon(&tray, q));
        assert!(rect_dist(q, &nw) >= 0.25 && rect_dist(q, &tray) >= 0.25, "{q:?}");
        // An L ring whose vertex average is outside.
        let l =
            vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 1.0), Vec2::new(1.0, 1.0), Vec2::new(1.0, 10.0), Vec2::new(0.0, 10.0)];
        let avg = Vec2::new(22.0 / 6.0, 22.0 / 6.0);
        assert!(!point_in_polygon(&l, avg));
        let s = ring_label_point(&l, &[]).unwrap();
        assert!(point_in_polygon(&l, s) && rect_dist(s, &l) > 1e-9);
        assert_eq!(ring_label_point(&l, &[]), Some(s));
        // Hostile input.
        assert_eq!(ring_label_point(&[Vec2::new(f64::NAN, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)], &[]), None);
        assert_eq!(ring_label_point(&r[..2], &[]), None);
        assert_eq!(ring_label_point(&[Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0)], &[]), None);
        // 5000 hole edges -> label_point's answer.
        let hole: Vec<Vec2> = (0..5000)
            .map(|i| {
                let a = i as f64 / 5000.0 * std::f64::consts::TAU;
                Vec2::new(2.0 + 0.5 * a.cos(), 1.0 + 0.5 * a.sin())
            })
            .collect();
        assert_eq!(ring_label_point(&r, &[hole.clone()]), crate::label::label_point(&r, &[hole]));
    }
}
