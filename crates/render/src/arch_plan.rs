//! Draws the plan annotation pipeline (`cadcraft_arch::plan::annotate`): lines (dashed, colour
//! overrides, reference lines beneath the walls) and stroke-text labels.

use cadcraft_arch::plan::{PlanAnnotation, PlanLineKind, active_view, annotate};
use cadcraft_color::{Color, Rgb};
use cadcraft_doc::Drawing;
use cadcraft_fonts::{TextFont, TextParams, place};
use cadcraft_geom::Vec2;

use crate::DisplayList;
use crate::arch::{arch_layer_skipped, layer_transparency, push_polyline_with_transparency};

/// Dash length of a dashed annotation line, metres.
const DASH: f64 = 0.15;
/// Gap between dashes, metres.
const GAP: f64 = 0.1;
/// A line needing more dashes than this is drawn solid.
const MAX_DASHES: f64 = 10_000.0;

/// What the active plan view adds, made at the level the walls are drawn at (`plan_level`), never
/// the view's own level: `level.show` and level exports change `plan_level` while a bound view
/// stays active.
pub(crate) fn plan_annotation(d: &Drawing) -> PlanAnnotation {
    annotate(&d.arch, active_view(&d.arch), d.arch.plan_level)
}

fn layer_ink(d: &Drawing, layer: &str, color: Option<[u8; 3]>) -> crate::Ink {
    let layer_color = d.layer(layer).map_or(Color::Index(7), |l| l.color);
    match color {
        Some([r, g, b]) => crate::ink(Color::True(Rgb(r, g, b)), layer_color, Color::Index(7)),
        None => crate::ink(Color::ByLayer, layer_color, Color::Index(7)),
    }
}

/// Draw the annotation lines whose kind is `Reference` exactly when `reference` is true.
pub(crate) fn push_plan_lines(d: &Drawing, list: &mut DisplayList, plotting: bool, ann: &PlanAnnotation, reference: bool) {
    for line in &ann.lines {
        if (line.kind == PlanLineKind::Reference) != reference || arch_layer_skipped(d, &line.layer, plotting) {
            continue;
        }
        if !line.a.is_finite() || !line.b.is_finite() {
            continue;
        }
        let ink = layer_ink(d, &line.layer, line.color);
        let t = line.transparency.unwrap_or_else(|| layer_transparency(d, &line.layer));
        let len = line.length();
        if !len.is_finite() {
            continue;
        }
        if !line.dashed || len / (DASH + GAP) > MAX_DASHES {
            push_polyline_with_transparency(list, line.handle, &ink, &[line.a, line.b], t);
            continue;
        }
        let dir = (line.b - line.a) * (1.0 / len);
        let mut s = 0.0;
        while s < len - 1e-12 {
            let e = (s + DASH).min(len);
            let pts = [line.a + dir * s, line.a + dir * e];
            if pts.iter().all(|p| p.is_finite()) {
                push_polyline_with_transparency(list, line.handle, &ink, &pts, t);
            }
            s += DASH + GAP;
        }
    }
}

/// Draw the annotation labels as stroke text.
pub(crate) fn push_plan_labels(d: &Drawing, list: &mut DisplayList, plotting: bool, ann: &PlanAnnotation) {
    for label in &ann.labels {
        if arch_layer_skipped(d, &label.layer, plotting) {
            continue;
        }
        let ink = layer_ink(d, &label.layer, None);
        let t = layer_transparency(d, &label.layer);
        let params = TextParams::centered(label.at, label.height, label.rotation);
        let (sh, _) = place(&TextFont::Stroke, &label.text, &params);
        for stroke in &sh.strokes {
            if stroke.len() >= 2 && stroke.iter().all(|p: &Vec2| p.is_finite()) {
                push_polyline_with_transparency(list, label.handle, &ink, stroke, t);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DPrim, Kind, Options, build_arch};
    use cadcraft_arch::plan::{PlanLabel, PlanLabelKind, PlanLine};
    use cadcraft_doc::arch::{Level, PlanView, ReferenceDisplay, Roof, RoofEdge, RoofEdgeKind, Room, roof_handle};
    use cadcraft_doc::{Handle, Wall, room_handle};

    fn line(layer: &str, a: Vec2, b: Vec2) -> PlanLine {
        PlanLine::new(layer, PlanLineKind::Ridge, a, b, Handle(0x77))
    }

    fn ann_of(lines: Vec<PlanLine>, labels: Vec<PlanLabel>) -> PlanAnnotation {
        PlanAnnotation { lines, labels }
    }

    fn lines_list(d: &Drawing, plotting: bool, ann: &PlanAnnotation, reference: bool) -> DisplayList {
        let mut list = DisplayList::default();
        push_plan_lines(d, &mut list, plotting, ann, reference);
        list
    }

    fn sq(x: f64, y: f64) -> Vec<Vec2> {
        vec![Vec2::new(x, y), Vec2::new(x + 4.0, y), Vec2::new(x + 4.0, y + 3.0), Vec2::new(x, y + 3.0)]
    }

    /// The old room-label and roof blocks, copied here as the reference.
    fn old_blocks(d: &Drawing, plotting: bool, list: &mut DisplayList) {
        use crate::arch::{on_shown_level, push_polyline_with_transparency as push};
        if !arch_layer_skipped(d, "A-ROOM", plotting) {
            for r in &d.arch.rooms {
                if r.polygon.len() < 3 || !on_shown_level(d, r.level) || !r.polygon.iter().all(|p| p.is_finite()) {
                    continue;
                }
                let text = if r.name.is_empty() { format!("r{}", r.id.0) } else { r.name.clone() };
                let n = r.polygon.len() as f64;
                let c = Vec2::new(r.polygon.iter().map(|p| p.x).sum::<f64>() / n, r.polygon.iter().map(|p| p.y).sum::<f64>() / n);
                let (sh, _) = place(&TextFont::Stroke, &text, &TextParams::centered(c, 0.24, 0.0));
                let ink = layer_ink(d, "A-ROOM", None);
                for s in &sh.strokes {
                    if s.len() >= 2 && s.iter().all(|p| p.is_finite()) {
                        push(list, room_handle(r.id), &ink, s, layer_transparency(d, "A-ROOM"));
                    }
                }
            }
        }
        if !arch_layer_skipped(d, "A-ROOF", plotting) {
            for roof in &d.arch.roofs {
                let ink = layer_ink(d, "A-ROOF", None);
                for e in &roof.edges {
                    if matches!(e.kind, RoofEdgeKind::Eave) || !e.a.is_finite() || !e.b.is_finite() {
                        continue;
                    }
                    push(list, roof_handle(roof.id), &ink, &[e.a, e.b], layer_transparency(d, "A-ROOF"));
                }
            }
        }
    }

    fn same_prims(a: &[DPrim], b: &[DPrim], av: &[Vec2], bv: &[Vec2]) {
        assert_eq!(a.len(), b.len());
        for (p, q) in a.iter().zip(b) {
            assert_eq!((p.handle, p.color, p.aci7, p.transparency, p.kind, p.len), (q.handle, q.color, q.aci7, q.transparency, q.kind, q.len));
            for i in 0..p.len as usize {
                assert_eq!(av.get(p.start as usize + i), bv.get(q.start as usize + i));
            }
        }
    }

    #[test]
    fn default_plan_is_unchanged() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-ROOM");
        d.ensure_layer("A-ROOF");
        if let Some(l) = d.layer_mut("A-ROOF") {
            l.transparency = 30;
        }
        for (i, x) in [0.0, 5.0, 10.0].into_iter().enumerate() {
            d.arch.push_room(Room { polygon: sq(x, 0.0), name: if i == 2 { String::new() } else { format!("Room {i}") }, ..Room::default() });
        }
        d.arch.push_roof(Roof {
            edges: vec![
                RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(0.0, 1.0), b: Vec2::new(10.0, 1.0), ..RoofEdge::default() },
                RoofEdge { kind: RoofEdgeKind::Hip, a: Vec2::new(10.0, 1.0), b: Vec2::new(12.0, 3.0), ..RoofEdge::default() },
                RoofEdge { kind: RoofEdgeKind::Eave, a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), ..RoofEdge::default() },
                RoofEdge { kind: RoofEdgeKind::Valley, a: Vec2::new(0.0, 0.0), b: Vec2::new(2.0, 2.0), ..RoofEdge::default() },
            ],
            ..Roof::default()
        });
        d.touch_arch();
        let new = build_arch(&d, &Options::default(), false);
        let mut old = DisplayList::default();
        old_blocks(&d, false, &mut old);
        assert!(!old.prims.is_empty());
        // No walls, stairs etc.: the whole arch list is exactly labels then roof lines.
        same_prims(&new.prims, &old.prims, &new.verts, &old.verts);
    }

    #[test]
    fn annotation_lines_follow_their_layer() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("L1");
        let ann = ann_of(vec![line("L1", Vec2::ZERO, Vec2::new(1.0, 0.0)), line("NOPE", Vec2::ZERO, Vec2::new(1.0, 0.0))], vec![]);
        assert_eq!(lines_list(&d, false, &ann, false).prims.len(), 2, "a missing layer draws");
        if let Some(l) = d.layer_mut("L1") {
            l.transparency = 40;
        }
        let l = lines_list(&d, false, &ann, false);
        assert_eq!(l.prims.first().map(|p| p.transparency), Some(40));
        assert_eq!(l.prims.get(1).map(|p| p.transparency), Some(0));
        if let Some(l) = d.layer_mut("L1") {
            l.on = false;
        }
        assert_eq!(lines_list(&d, false, &ann, false).prims.len(), 1);
        if let Some(l) = d.layer_mut("L1") {
            l.on = true;
            l.frozen = true;
        }
        assert_eq!(lines_list(&d, false, &ann, false).prims.len(), 1);
        if let Some(l) = d.layer_mut("L1") {
            l.frozen = false;
            l.plot = false;
        }
        assert_eq!(lines_list(&d, false, &ann, false).prims.len(), 2);
        assert_eq!(lines_list(&d, true, &ann, false).prims.len(), 1);
    }

    #[test]
    fn colour_override_draws_true_colour_and_its_transparency() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("L1");
        if let Some(l) = d.layer_mut("L1") {
            l.transparency = 10;
        }
        let ann = ann_of(vec![line("L1", Vec2::ZERO, Vec2::new(1.0, 0.0)).colored([255, 0, 128], 55)], vec![]);
        let l = lines_list(&d, false, &ann, false);
        let p = l.prims.first();
        assert_eq!(p.map(|p| p.color), Some(Rgb(255, 0, 128)));
        assert_eq!(p.map(|p| p.aci7), Some(false));
        assert_eq!(p.map(|p| p.transparency), Some(55));
    }

    #[test]
    fn dashed_line_splits_into_dashes() {
        let d = Drawing::new_metric();
        let ann = ann_of(vec![line("L", Vec2::ZERO, Vec2::new(1.0, 0.0)).dashed()], vec![]);
        let l = lines_list(&d, false, &ann, false);
        assert_eq!(l.prims.len(), 4);
        assert!(l.prims.iter().all(|p| p.len == 2 && p.handle == Handle(0x77)));
        // A 0.9 m line: the last dash (0.75..0.9) fits; 0.8 m: it is clipped to 0.8.
        let ann = ann_of(vec![line("L", Vec2::ZERO, Vec2::new(0.8, 0.0)).dashed()], vec![]);
        let l = lines_list(&d, false, &ann, false);
        assert_eq!(l.prims.len(), 4);
        assert_eq!(l.verts.last().map(|v| v.x), Some(0.8));
        // Too many dashes: solid.
        let ann = ann_of(vec![line("L", Vec2::ZERO, Vec2::new(5000.0, 0.0)).dashed()], vec![]);
        assert_eq!(lines_list(&d, false, &ann, false).prims.len(), 1);
    }

    #[test]
    fn reference_lines_come_before_the_wall_poche() {
        let mut d = Drawing::new_metric();
        d.arch.levels = vec![Level { index: 0, elevation: 0.0, ..Default::default() }, Level { index: 1, elevation: 3.0, ..Default::default() }];
        d.arch.add_wall(Wall { start: Vec2::ZERO, end: Vec2::new(4.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        let up = d.arch.add_wall(Wall { base_level: 1, start: Vec2::new(0.0, 5.0), end: Vec2::new(4.0, 5.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        let mut v = PlanView::default();
        v.name = "Ref".into();
        v.level = Some(1);
        v.reference = Some(ReferenceDisplay { level: 0, ..ReferenceDisplay::default() });
        d.arch.plan_views.push(v);
        d.arch.active_plan_view = Some("Ref".into());
        d.arch.plan_level = Some(1);
        d.touch_arch();
        // The reference generator is another package's; inject its kind of line directly.
        let mut ann = plan_annotation(&d);
        let ref_handle = Handle(0x55);
        ann.push_line(PlanLine::new("A-WALL", PlanLineKind::Reference, Vec2::ZERO, Vec2::new(4.0, 0.0), ref_handle));
        let refs: Vec<Handle> = ann.lines.iter().filter(|l| l.kind == PlanLineKind::Reference).map(|l| l.handle).collect();
        assert!(!refs.is_empty());
        let out = crate::arch::build_arch_with(&d, false, &ann);
        let last_ref = out.prims.iter().rposition(|p| refs.contains(&p.handle));
        let first_wall = out.prims.iter().position(|p| p.handle == crate::arch_handle(up));
        let (Some(r), Some(w)) = (last_ref, first_wall) else { panic!("missing prims") };
        assert!(r < w, "reference {r} before wall {w}");
        // The non-reference pass draws none of them.
        assert!(lines_list(&d, false, &ann, false).prims.iter().all(|p| !refs.contains(&p.handle)));
    }

    #[test]
    fn labels_draw_stroke_text_with_their_handles() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-ROOM");
        if let Some(l) = d.layer_mut("A-ROOM") {
            l.transparency = 20;
        }
        let ann = ann_of(
            vec![],
            vec![
                PlanLabel::new("A-ROOM", PlanLabelKind::RoomName, "Hall", Vec2::new(1.0, 1.0), 0.24, Handle(0x42)),
                PlanLabel::new("OFF", PlanLabelKind::RoomArea, "9 m2", Vec2::ZERO, 0.2, Handle(0x43)),
            ],
        );
        if d.layer("OFF").is_none() {
            d.ensure_layer("OFF");
        }
        if let Some(l) = d.layer_mut("OFF") {
            l.on = false;
        }
        let mut list = DisplayList::default();
        push_plan_labels(&d, &mut list, false, &ann);
        assert!(!list.prims.is_empty());
        assert!(list.prims.iter().all(|p| p.handle == Handle(0x42) && p.transparency == 20 && p.kind == Kind::Polyline && p.len >= 2));
    }

    #[test]
    fn active_view_reaches_annotate() {
        let mut d = Drawing::new_metric();
        d.arch.push_room(Room { polygon: sq(0.0, 0.0), name: "A".into(), ..Room::default() });
        d.arch.plan_views = cadcraft_doc::arch::template_plan_views();
        for name in ["Roof Plan", "Reflected Ceiling", "No Such View"] {
            d.arch.active_plan_view = Some(name.into());
            d.touch_arch();
            // Must not panic; a dangling name falls back to no view (the room label draws).
            let list = build_arch(&d, &Options::default(), false);
            if name == "No Such View" {
                assert!(!list.prims.is_empty());
            }
        }
        d.arch.active_plan_view = Some("Dangling".into());
        assert!(active_view(&d.arch).is_none());
    }

    #[test]
    fn huge_annotation_is_capped() {
        let d = Drawing::new_metric();
        let mut ann = PlanAnnotation::default();
        for _ in 0..cadcraft_arch::plan::MAX_PLAN_LINES + 10 {
            ann.push_line(line("L", Vec2::ZERO, Vec2::new(1.0, 0.0)));
        }
        assert_eq!(ann.lines.len(), cadcraft_arch::plan::MAX_PLAN_LINES);
        // A hostile (non-finite) line pushed straight into the public field is skipped.
        ann.lines.truncate(3);
        ann.lines.push(line("L", Vec2::new(f64::NAN, 0.0), Vec2::new(1.0, 0.0)).dashed());
        ann.lines.push(line("L", Vec2::ZERO, Vec2::new(f64::INFINITY, 0.0)).dashed());
        let l = lines_list(&d, false, &ann, false);
        assert_eq!(l.prims.len(), 3);
        assert!(l.verts.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn annotation_follows_plan_level_not_the_active_views_level() {
        let mut d = Drawing::new_metric();
        d.arch.levels = vec![Level { index: 0, elevation: 0.0, ..Default::default() }, Level { index: 1, elevation: 3.0, ..Default::default() }];
        let r0 = d.arch.push_room(Room { polygon: sq(0.0, 0.0), name: "Down".into(), level: 0, ..Room::default() });
        let r1 = d.arch.push_room(Room { polygon: sq(0.0, 10.0), name: "Up".into(), level: 1, ..Room::default() });
        d.arch.add_wall(Wall { start: Vec2::ZERO, end: Vec2::new(4.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        d.arch.add_wall(Wall { base_level: 1, start: Vec2::new(0.0, 10.0), end: Vec2::new(4.0, 10.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        let mut v = PlanView::default();
        v.name = "Upper".into();
        v.level = Some(1);
        v.reference = Some(ReferenceDisplay { level: 0, ..ReferenceDisplay::default() });
        d.arch.plan_views.push(v);
        d.arch.active_plan_view = Some("Upper".into());
        d.arch.plan_level = Some(0);
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        let ann = plan_annotation(&d);
        assert!(ann.lines.iter().all(|l| l.kind != PlanLineKind::Reference));
        let label_handles: Vec<Handle> = list.prims.iter().map(|p| p.handle).filter(|h| *h == room_handle(r0) || *h == room_handle(r1)).collect();
        assert!(!label_handles.is_empty() && label_handles.iter().all(|h| *h == room_handle(r0)));
        d.arch.plan_level = Some(1);
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        // (At level 1 the reference level 0 differs, so the reference generator runs; its lines
        // are that package's to assert.)
        let label_handles: Vec<Handle> = list.prims.iter().map(|p| p.handle).filter(|h| *h == room_handle(r0) || *h == room_handle(r1)).collect();
        assert!(!label_handles.is_empty() && label_handles.iter().all(|h| *h == room_handle(r1)));
    }
}
