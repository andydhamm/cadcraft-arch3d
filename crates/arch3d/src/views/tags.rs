//! Schedule-number tags (D01, W01, C01) on the openings and cabinets an elevation draws.

use super::indicators::{cabinet_front_centre, opening_box};
use crate::hlr::OrthoView;
use cadcraft_arch::schedule::numbers;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchId, ArchKind, ArchView, LAYER_ELEV_TAG, ViewLabel, ViewLine};
use std::collections::BTreeSet;

/// Text height of a tag, metres.
const TAG_HEIGHT: f64 = 0.15;
/// Most tags in one view.
const MAX_TAGS: usize = 20_000;

/// Tag labels for `view`, given its final `lines`: one per numbered opening or cabinet with at least one line.
pub(crate) fn tag_labels(model: &ArchModel, _view: &ArchView, ov: &OrthoView, lines: &[ViewLine]) -> Vec<ViewLabel> {
    let drawn: BTreeSet<(ArchKind, ArchId)> = lines.iter().map(|l| (l.kind, l.object)).collect();
    let nums = numbers(model);
    let mut out: Vec<ViewLabel> = Vec::new();
    let mut push = |text: &String, c: [f64; 3], kind: ArchKind, object: ArchId| {
        let p = ov.project(c);
        if p[0].is_finite() && p[1].is_finite() && out.len() < MAX_TAGS {
            out.push(ViewLabel {
                text: text.clone(),
                at: [p[0], p[1]],
                height: TAG_HEIGHT,
                layer: LAYER_ELEV_TAG.into(),
                kind,
                object,
                ..ViewLabel::default()
            });
        }
    };
    for o in &model.openings {
        let Some(text) = nums.get(&(ArchKind::Opening, o.id)) else { continue };
        if !drawn.contains(&(ArchKind::Opening, o.id)) {
            continue;
        }
        if let Some(b) = opening_box(model, o, None) {
            push(text, b.centre(), ArchKind::Opening, o.id);
        }
    }
    for c in &model.cabinets {
        let Some(text) = nums.get(&(ArchKind::Cabinet, c.id)) else { continue };
        if !drawn.contains(&(ArchKind::Cabinet, c.id)) {
            continue;
        }
        if let Some(p) = cabinet_front_centre(model, c, None) {
            push(text, p, ArchKind::Cabinet, c.id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{Cabinet, Opening, OpeningKind, Wall};
    use cadcraft_geom::Vec2;

    fn model() -> ArchModel {
        let mut m = ArchModel::default();
        let w = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        m.push_opening(Opening::new(w, 3.0, 0.9, 2.1, OpeningKind::Door));
        let mut win = Opening::new(w, 7.0, 0.9, 1.2, OpeningKind::Window);
        win.sill = 0.9;
        m.push_opening(win);
        m
    }

    fn line(kind: ArchKind, object: ArchId) -> ViewLine {
        ViewLine { a: [0.0, 0.0], b: [1.0, 1.0], kind, object, ..ViewLine::default() }
    }

    fn south() -> OrthoView {
        OrthoView::elevation([0.0, 1.0]).expect("view")
    }

    #[test]
    fn tags_sit_in_their_openings() {
        let m = model();
        let lines = [line(ArchKind::Opening, ArchId(0)), line(ArchKind::Opening, ArchId(1))];
        let t = tag_labels(&m, &ArchView::default(), &south(), &lines);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].text, "D01");
        assert_eq!(t[1].text, "W01");
        assert!((t[0].at[0] - 3.0).abs() < 1e-9 && (t[0].at[1] - 1.05).abs() < 1e-9);
        assert!((t[1].at[0] - 7.0).abs() < 1e-9 && (t[1].at[1] - 1.5).abs() < 1e-9);
        assert!(t.iter().all(|l| l.layer == LAYER_ELEV_TAG && l.height == 0.15 && l.kind == ArchKind::Opening));
    }

    #[test]
    fn cabinet_tags_on_the_front() {
        let mut m = ArchModel::default();
        let id = m.push_cabinet(Cabinet { position: Vec2::new(5.0, 2.0), width: 1.2, depth: 0.6, height: 0.9, ..Cabinet::default() });
        let ov = OrthoView::elevation([0.0, -1.0]).expect("view");
        let t = tag_labels(&m, &ArchView::default(), &ov, &[line(ArchKind::Cabinet, id)]);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].text, "C01");
        assert_eq!(t[0].object, id);
        // Looking south, u runs toward -x: the cabinet centre at x = 5 projects to -5.
        assert!((t[0].at[0] + 5.0).abs() < 1e-9 && (t[0].at[1] - 0.45).abs() < 1e-9, "{:?}", t[0].at);
    }

    #[test]
    fn only_drawn_objects_are_tagged() {
        let m = model();
        let t = tag_labels(&m, &ArchView::default(), &south(), &[line(ArchKind::Opening, ArchId(1))]);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].text, "W01");
        assert!(tag_labels(&m, &ArchView::default(), &south(), &[]).is_empty());
        // A wall line with the same id is not an opening line.
        assert!(tag_labels(&m, &ArchView::default(), &south(), &[line(ArchKind::Wall, ArchId(0))]).is_empty());
    }

    #[test]
    fn empty_model_has_no_tags() {
        let m = ArchModel::default();
        let t = tag_labels(&m, &ArchView::default(), &south(), &[]);
        assert!(t.is_empty());
    }

    #[test]
    fn tags_use_correct_layer_and_height() {
        let m = model();
        let lines = [line(ArchKind::Opening, ArchId(0))];
        let t = tag_labels(&m, &ArchView::default(), &south(), &lines);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].layer, LAYER_ELEV_TAG.to_string());
        assert_eq!(t[0].height, TAG_HEIGHT);
        assert_eq!(t[0].height, 0.15);
    }

    #[test]
    fn large_coordinates_still_tagged() {
        let mut m = ArchModel::default();
        let w = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(1e10, 0.0), 0.2));
        m.push_opening(Opening::new(w, 5e9, 0.9, 2.1, OpeningKind::Door));
        let lines = [line(ArchKind::Opening, ArchId(0))];
        let t = tag_labels(&m, &ArchView::default(), &south(), &lines);
        assert_eq!(t.len(), 1);
        assert!(t[0].at[0].is_finite() && t[0].at[1].is_finite());
    }

    #[test]
    fn opening_at_wall_start_is_tagged() {
        let mut m = ArchModel::default();
        let w = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        m.push_opening(Opening::new(w, 0.45, 0.9, 2.1, OpeningKind::Door));
        let lines = [line(ArchKind::Opening, ArchId(0))];
        let t = tag_labels(&m, &ArchView::default(), &south(), &lines);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].text, "D01");
    }
}
