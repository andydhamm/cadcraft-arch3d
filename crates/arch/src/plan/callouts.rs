//! Plan callout markers.
//!
//! Marks every placed elevation, section and interior elevation with a callout label
//! (`Callout`), and resolves a point near a placed elevation to its view.
//!
//! Owner: p21-callouts.

use cadcraft_doc::arch::{ArchId, ArchKind, ArchModel, ArchView, ViewKind};
use cadcraft_geom::Vec2;

use super::{PlanAnnotation, PlanLabel, PlanLabelKind, PlanLine, PlanLineKind};

const LAYER_CALLOUT: &str = "A-ANNO-SYMB";
const MARKER_RADIUS: f64 = 0.45;
const POINTER_OFFSET: f64 = 0.75;
const SECTION_OFFSET: f64 = 0.6;
const LABEL_HEIGHT: f64 = 0.25;
const CIRCLE_SEGMENTS: usize = 24;

/// Add callout plan lines/labels for the given plan level.
pub fn callouts(model: &ArchModel, level: Option<u32>, out: &mut PlanAnnotation) {
    // Track which views we've already processed to ensure "first sheet wins"
    let mut seen_views = std::collections::BTreeSet::new();

    for sheet in &model.sheets {
        // Sheet views are ordered; iterate in order
        for view_entry in &sheet.views {
            // Only handle views sourced from ArchViews
            if let cadcraft_doc::arch::SheetSource::View { id } = view_entry.source {
                // Skip if already processed (first sheet wins)
                if !seen_views.insert(id) {
                    continue;
                }

                if let Some(view) = model.views.iter().find(|v| v.id == id) {
                    // Filter by level if requested
                    if let Some(lvl) = level {
                        if view.level != Some(lvl) {
                            continue;
                        }
                    }

                    let text = format!("{}/{}", view_entry.number, sheet.number);
                    let handle = cadcraft_doc::arch::view_handle(id);
                    let look = Vec2::new(view.look[0], view.look[1]);

                    match view.kind {
                        ViewKind::Elevation => {
                            if let Some(center) = elevation_marker_center(model, view, level) {
                                push_marker(out, center, look, &text, handle);
                            }
                        }
                        ViewKind::Section => {
                            if let Some((c1, c2)) = section_marker_centers(model, view) {
                                push_marker(out, c1, look, &text, handle);
                                push_marker(out, c2, look, &text, handle);
                            }
                        }
                        ViewKind::Interior => {
                            if let Some(center) = interior_marker_center(model, view, level) {
                                push_marker(out, center, look, &text, handle);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// The view id whose callout marker lies near `p` (within `tol`), if any.
pub fn callout_at(model: &ArchModel, level: Option<u32>, p: Vec2, tol: f64) -> Option<ArchId> {
    let mut best_id: Option<ArchId> = None;
    let mut best_dist_sq = f64::INFINITY;
    let threshold = tol.max(MARKER_RADIUS);
    let threshold_sq = threshold * threshold;

    let mut seen_views = std::collections::BTreeSet::new();

    for sheet in &model.sheets {
        for view_entry in &sheet.views {
            if let cadcraft_doc::arch::SheetSource::View { id } = view_entry.source {
                if !seen_views.insert(id) {
                    continue;
                }

                if let Some(view) = model.views.iter().find(|v| v.id == id) {
                    if let Some(lvl) = level {
                        if view.level != Some(lvl) {
                            continue;
                        }
                    }

                    let centers = marker_centers(model, view, level);
                    for center in centers {
                        let dist = center.dist(p);
                        let dist_sq = dist * dist;
                        if dist_sq <= threshold_sq && dist_sq < best_dist_sq {
                            best_dist_sq = dist_sq;
                            best_id = Some(id);
                        }
                    }
                }
            }
        }
    }

    best_id
}

fn marker_centers(model: &ArchModel, view: &ArchView, level: Option<u32>) -> Vec<Vec2> {
    match view.kind {
        ViewKind::Elevation => {
            if let Some(c) = elevation_marker_center(model, view, level) {
                vec![c]
            } else {
                vec![]
            }
        }
        ViewKind::Section => {
            if let Some((c1, c2)) = section_marker_centers(model, view) {
                vec![c1, c2]
            } else {
                vec![]
            }
        }
        ViewKind::Interior => {
            if let Some(c) = interior_marker_center(model, view, level) {
                vec![c]
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

fn elevation_marker_center(model: &ArchModel, view: &ArchView, level: Option<u32>) -> Option<Vec2> {
    // Filter walls by level: walls with no level are on all levels
    let walls: Vec<_> = match level {
        None => model.walls.iter().collect(),
        Some(lvl) => model.walls.iter().filter(|w| w.base_level == lvl).collect(),
    };

    if walls.is_empty() {
        return None;
    }

    // Compute bounds of the filtered walls
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for wall in walls {
        min_x = min_x.min(wall.start.x);
        min_y = min_y.min(wall.start.y);
        max_x = max_x.max(wall.start.x);
        max_y = max_y.max(wall.start.y);
        min_x = min_x.min(wall.end.x);
        min_y = min_y.min(wall.end.y);
        max_x = max_x.max(wall.end.x);
        max_y = max_y.max(wall.end.y);
    }

    let center = Vec2::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
    let look = Vec2::new(view.look[0], view.look[1]);

    let corners = [Vec2::new(min_x, min_y), Vec2::new(max_x, min_y), Vec2::new(max_x, max_y), Vec2::new(min_x, max_y)];

    let mut max_proj = 0.0;
    for corner in corners {
        let proj = (corner - center).dot(look);
        if proj > max_proj {
            max_proj = proj;
        }
    }

    Some(center - look * (max_proj + 2.0))
}

fn section_marker_centers(_model: &ArchModel, view: &ArchView) -> Option<(Vec2, Vec2)> {
    let cut = view.cut?;
    let p1 = Vec2::new(cut[0][0], cut[0][1]);
    let p2 = Vec2::new(cut[1][0], cut[1][1]);

    let dir = p2 - p1;
    if dir.len() < 1e-9 {
        return None;
    }
    let dir_norm = dir.normalized();

    // Push outward along the cut direction
    // "both ends of the cut line, each pushed 0.6 m outward along the cut direction"
    // This implies extending the line segment.
    let c1 = p1 - dir_norm * SECTION_OFFSET;
    let c2 = p2 + dir_norm * SECTION_OFFSET;

    Some((c1, c2))
}

fn interior_marker_center(_model: &ArchModel, view: &ArchView, level: Option<u32>) -> Option<Vec2> {
    // "view.room_point, only when level is None or equals view.level"
    if let Some(lvl) = level {
        if view.level != Some(lvl) {
            return None;
        }
    }
    view.room_point.map(|p| Vec2::new(p[0], p[1]))
}

fn push_marker(out: &mut PlanAnnotation, center: Vec2, look: Vec2, text: &str, handle: cadcraft_doc::Handle) {
    // Circle
    let step = std::f64::consts::TAU / (CIRCLE_SEGMENTS as f64);
    let mut prev = center + Vec2::from_angle(0.0) * MARKER_RADIUS;

    for i in 1..=CIRCLE_SEGMENTS {
        let angle = i as f64 * step;
        let curr = center + Vec2::from_angle(angle) * MARKER_RADIUS;
        out.push_line(PlanLine::new(LAYER_CALLOUT, PlanLineKind::Callout, prev, curr, handle));
        prev = curr;
    }

    // Pointer lines
    // "two lines from the circle toward centre + look * 0.75"
    // The pointer tip is at center + look * POINTER_OFFSET
    let tip = center + look * POINTER_OFFSET;

    // Find the two points on the circle that connect to the tip.
    // The prompt says "two lines from the circle". It doesn't specify which points.
    // Usually callouts have a triangle pointer.
    // If we just draw lines from the tip to the circle, we need tangent points or just intersection?
    // "two lines from the circle toward centre + look * 0.75"
    // This phrasing is slightly ambiguous. It likely means lines connecting the circle to the tip.
    // To make a clean arrow, we might pick points on the circle perpendicular to look?
    // Or just the intersection of the line from center to tip with the circle? That's one point.
    // Let's assume standard arrow: lines from tip to two points on the circle.
    // Which points? The ones that make a "V" shape pointing at the center?
    // Let's pick points on the circle that are roughly 30 degrees off the look direction backwards?
    // Or simpler: The prompt says "two lines from the circle toward [tip]".
    // Let's pick the points on the circle where the tangent from the tip touches? No, tip is inside/outside?
    // Tip is at 0.75 radius. Circle is 0.45 radius. Tip is outside.
    // Lines from tip to circle.
    // Let's pick two points on the circle that are symmetric around the look axis.
    // Angle offset from look: 150 degrees?
    let angle_offset = std::f64::consts::FRAC_PI_6; // 30 degrees
    let look_angle = look.y.atan2(look.x);

    let p1_angle = look_angle + std::f64::consts::PI - angle_offset;
    let p2_angle = look_angle + std::f64::consts::PI + angle_offset;

    let p1 = center + Vec2::from_angle(p1_angle) * MARKER_RADIUS;
    let p2 = center + Vec2::from_angle(p2_angle) * MARKER_RADIUS;

    out.push_line(PlanLine::new(LAYER_CALLOUT, PlanLineKind::Callout, tip, p1, handle));
    out.push_line(PlanLine::new(LAYER_CALLOUT, PlanLineKind::Callout, tip, p2, handle));

    // Label
    out.push_label(PlanLabel::new(LAYER_CALLOUT, PlanLabelKind::Callout, text, center, LABEL_HEIGHT, handle));
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::Handle;
    use cadcraft_doc::arch::{ArchKind, ArchModel, ArchView, Level, Sheet, SheetSource, SheetView, ViewKind, Wall};
    use cadcraft_geom::Vec2;

    fn make_view(id: u32, kind: ViewKind, look: Vec2, level: Option<u32>, room_point: Option<Vec2>, cut: Option<[[f64; 2]; 2]>) -> ArchView {
        ArchView { id: ArchId(id), kind, look: [look.x, look.y], cut, room_point: room_point.map(|p| [p.x, p.y]), level, ..Default::default() }
    }

    fn make_sheet(name: &str, number: &str, views: Vec<SheetView>) -> Sheet {
        Sheet { name: name.to_string(), number: number.to_string(), views, ..Default::default() }
    }

    fn make_sheet_view(number: &str, id: u32) -> SheetView {
        SheetView { number: number.parse().unwrap_or(0), source: SheetSource::View { id: ArchId(id) }, ..Default::default() }
    }

    fn setup_model() -> ArchModel {
        let mut model = ArchModel::default();
        // Add a wall to give bounds
        model.walls.push(Wall {
            id: ArchId::wall(0),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            height: 2.4,
            ..Default::default()
        });
        model.walls.push(Wall {
            id: ArchId::wall(1),
            start: Vec2::new(10.0, 0.0),
            end: Vec2::new(10.0, 10.0),
            thickness: 0.2,
            height: 2.4,
            ..Default::default()
        });
        model.walls.push(Wall {
            id: ArchId::wall(2),
            start: Vec2::new(10.0, 10.0),
            end: Vec2::new(0.0, 10.0),
            thickness: 0.2,
            height: 2.4,
            ..Default::default()
        });
        model.walls.push(Wall {
            id: ArchId::wall(3),
            start: Vec2::new(0.0, 10.0),
            end: Vec2::new(0.0, 0.0),
            thickness: 0.2,
            height: 2.4,
            ..Default::default()
        });
        model
    }

    #[test]
    fn placed_views_get_callouts_unplaced_none() {
        let mut model = setup_model();
        let view = make_view(0, ViewKind::Elevation, Vec2::Y, None, None, None);
        model.views.push(view.clone());

        // No sheets -> no callouts
        let mut out = PlanAnnotation::default();
        callouts(&model, None, &mut out);
        assert!(out.labels.is_empty());
        assert!(out.lines.is_empty());

        // Add sheet
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        model.sheets.push(sheet);

        let mut out = PlanAnnotation::default();
        callouts(&model, None, &mut out);
        assert!(!out.labels.is_empty());
        assert!(!out.lines.is_empty());
    }

    #[test]
    fn callout_text_is_view_number_over_sheet_number() {
        let mut model = setup_model();
        let view = make_view(0, ViewKind::Elevation, Vec2::Y, None, None, None);
        model.views.push(view);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("5", 0)]);
        model.sheets.push(sheet);

        let mut out = PlanAnnotation::default();
        callouts(&model, None, &mut out);

        assert_eq!(out.labels.len(), 1);
        assert_eq!(out.labels[0].text, "5/A-101");
    }

    #[test]
    fn elevation_marker_outside_on_the_viewer_side() {
        let mut model = setup_model();
        // House is 0..10. Center 5,5.
        // Look North (0,1). Viewer is South.
        // Marker should be South of house (y < 0).
        let view = make_view(0, ViewKind::Elevation, Vec2::Y, None, None, None);
        model.views.push(view);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        model.sheets.push(sheet);

        let mut out = PlanAnnotation::default();
        callouts(&model, None, &mut out);

        assert_eq!(out.labels.len(), 1);
        let label = &out.labels[0];
        assert!(label.at.y < 0.0, "Marker should be south of the house");
    }

    #[test]
    fn section_markers_at_both_cut_ends() {
        let mut model = setup_model();
        // Section from (-1, 5) to (11, 5)
        let cut = [[-1.0, 5.0], [11.0, 5.0]];
        let view = make_view(0, ViewKind::Section, Vec2::Y, None, None, Some(cut));
        model.views.push(view);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        model.sheets.push(sheet);

        let mut out = PlanAnnotation::default();
        callouts(&model, None, &mut out);

        // Two labels
        assert_eq!(out.labels.len(), 2);

        // Check positions
        // Cut dir is X. Offset 0.6.
        // P1: -1 - 0.6 = -1.6. P2: 11 + 0.6 = 11.6.
        let l1 = &out.labels[0];
        let l2 = &out.labels[1];

        // Order might depend on implementation, but both should be present
        let has_left = (l1.at.x - -1.6).abs() < 1e-6 || (l2.at.x - -1.6).abs() < 1e-6;
        let has_right = (l1.at.x - 11.6).abs() < 1e-6 || (l2.at.x - 11.6).abs() < 1e-6;

        assert!(has_left && has_right);
        assert!((l1.at.y - 5.0).abs() < 1e-6);
        assert!((l2.at.y - 5.0).abs() < 1e-6);
    }

    #[test]
    fn interior_marker_in_its_room_on_its_level() {
        let mut model = setup_model();
        let room_point = Vec2::new(5.0, 5.0);
        let view = make_view(0, ViewKind::Interior, Vec2::Y, Some(0), Some(room_point), None);
        model.views.push(view);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        model.sheets.push(sheet);

        let mut out = PlanAnnotation::default();
        callouts(&model, Some(0), &mut out);
        assert_eq!(out.labels.len(), 1);
        assert_eq!(out.labels[0].at, room_point);

        // Wrong level
        let mut out2 = PlanAnnotation::default();
        callouts(&model, Some(1), &mut out2);
        assert!(out2.labels.is_empty());
    }

    #[test]
    fn marker_lines_carry_the_view_handle() {
        let mut model = setup_model();
        let view = make_view(0, ViewKind::Elevation, Vec2::Y, None, None, None);
        model.views.push(view);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        model.sheets.push(sheet);

        let mut out = PlanAnnotation::default();
        callouts(&model, None, &mut out);

        let expected_handle = cadcraft_doc::arch::view_handle(ArchId(0));

        for line in &out.lines {
            assert_eq!(line.handle, expected_handle);
        }
        for label in &out.labels {
            assert_eq!(label.handle, expected_handle);
        }
    }

    #[test]
    fn callout_at_finds_the_nearest_marker() {
        let mut model = setup_model();
        let view = make_view(0, ViewKind::Elevation, Vec2::Y, None, None, None);
        model.views.push(view);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        model.sheets.push(sheet);

        // Calculate expected marker position
        let bounds = model.bounds();
        let center = bounds.center();
        let look = Vec2::Y;
        let mut max_proj = 0.0;
        let corners = [
            Vec2::new(bounds.min.x, bounds.min.y),
            Vec2::new(bounds.max.x, bounds.min.y),
            Vec2::new(bounds.max.x, bounds.max.y),
            Vec2::new(bounds.min.x, bounds.max.y),
        ];
        for corner in corners {
            let proj = (corner - center).dot(look);
            if proj > max_proj {
                max_proj = proj;
            }
        }
        let marker_pos = center - look * (max_proj + 2.0);

        // Find it
        let found = callout_at(&model, None, marker_pos, 0.1);
        assert_eq!(found, Some(ArchId(0)));

        // Too far
        let far = marker_pos + Vec2::X * 10.0;
        let not_found = callout_at(&model, None, far, 0.1);
        assert_eq!(not_found, None);
    }

    #[test]
    fn first_sheet_wins_for_a_view_on_two_sheets() {
        let mut model = setup_model();
        let view = make_view(0, ViewKind::Elevation, Vec2::Y, None, None, None);
        model.views.push(view);

        let sheet1 = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        let sheet2 = make_sheet("S2", "A-102", vec![make_sheet_view("2", 0)]);

        model.sheets.push(sheet1);
        model.sheets.push(sheet2);

        let mut out = PlanAnnotation::default();
        callouts(&model, None, &mut out);

        assert_eq!(out.labels.len(), 1);
        assert_eq!(out.labels[0].text, "1/A-101");
    }

    #[test]
    fn elevation_no_walls_no_marker() {
        // Empty model (no walls) -> no elevation marker
        let model = ArchModel::default();
        let view = make_view(0, ViewKind::Elevation, Vec2::Y, None, None, None);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);

        let mut model_with_view = model.clone();
        model_with_view.views.push(view);
        model_with_view.sheets.push(sheet);

        let mut out = PlanAnnotation::default();
        callouts(&model_with_view, None, &mut out);
        assert!(out.labels.is_empty());
        assert!(out.lines.is_empty());
    }

    #[test]
    fn elevation_marker_uses_walls_on_level() {
        let mut model = setup_model();
        // All walls in setup_model have no level (None), so they're on all levels.
        // Create a view on level 0 and check that the marker is produced.
        let view = make_view(0, ViewKind::Elevation, Vec2::Y, Some(0), None, None);
        model.views.push(view);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        model.sheets.push(sheet);

        // Level 0: walls exist -> marker produced
        let mut out = PlanAnnotation::default();
        callouts(&model, Some(0), &mut out);
        assert_eq!(out.labels.len(), 1);

        // Now add a wall on level 1 only and query level 1
        let mut model2 = setup_model();
        // Clear the level-None walls and add a wall on level 1
        model2.walls.clear();
        model2.walls.push(Wall {
            id: ArchId::wall(0),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            height: 2.4,
            base_level: 1,
            ..Default::default()
        });
        let view2 = make_view(0, ViewKind::Elevation, Vec2::Y, Some(1), None, None);
        model2.views.push(view2);
        model2.sheets.push(make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]));

        // Level 0: no walls on level 0 -> no marker
        let mut out2 = PlanAnnotation::default();
        callouts(&model2, Some(0), &mut out2);
        assert!(out2.labels.is_empty());

        // Level 1: wall exists -> marker produced
        let mut out3 = PlanAnnotation::default();
        callouts(&model2, Some(1), &mut out3);
        assert_eq!(out3.labels.len(), 1);
    }

    #[test]
    fn section_zero_length_cut_no_marker() {
        let mut model = setup_model();
        // Zero-length cut: both endpoints the same
        let cut = [[5.0, 5.0], [5.0, 5.0]];
        let view = make_view(0, ViewKind::Section, Vec2::Y, None, None, Some(cut));
        model.views.push(view);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        model.sheets.push(sheet);

        let mut out = PlanAnnotation::default();
        callouts(&model, None, &mut out);
        assert!(out.labels.is_empty());
        assert!(out.lines.is_empty());
    }

    #[test]
    fn interior_no_room_point_no_marker() {
        let mut model = setup_model();
        // Interior view with no room_point
        let view = make_view(0, ViewKind::Interior, Vec2::Y, None, None, None);
        model.views.push(view);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        model.sheets.push(sheet);

        let mut out = PlanAnnotation::default();
        callouts(&model, None, &mut out);
        assert!(out.labels.is_empty());
        assert!(out.lines.is_empty());
    }

    #[test]
    fn callout_at_level_filtering() {
        let mut model = setup_model();
        let room_point = Vec2::new(5.0, 5.0);
        let view = make_view(0, ViewKind::Interior, Vec2::Y, Some(0), Some(room_point), None);
        model.views.push(view);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        model.sheets.push(sheet);

        // On level 0: found
        let found = callout_at(&model, Some(0), room_point, 0.1);
        assert_eq!(found, Some(ArchId(0)));

        // On level 1: not found (view is on level 0)
        let not_found = callout_at(&model, Some(1), room_point, 0.1);
        assert_eq!(not_found, None);
    }

    #[test]
    fn elevation_marker_ignores_walls_on_other_levels() {
        let mut model = ArchModel::default();
        // Wall on level 0
        model.walls.push(Wall {
            id: ArchId::wall(0),
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            height: 2.4,
            base_level: 0,
            ..Default::default()
        });
        // Wall on level 1, far away
        model.walls.push(Wall {
            id: ArchId::wall(1),
            start: Vec2::new(100.0, 100.0),
            end: Vec2::new(110.0, 100.0),
            thickness: 0.2,
            height: 2.4,
            base_level: 1,
            ..Default::default()
        });

        let view = make_view(0, ViewKind::Elevation, Vec2::Y, Some(0), None, None);
        model.views.push(view);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        model.sheets.push(sheet);

        // Level 0: only the level-0 wall is used, marker near y=0
        let mut out = PlanAnnotation::default();
        callouts(&model, Some(0), &mut out);
        assert_eq!(out.labels.len(), 1);
        let label = &out.labels[0];
        assert!(label.at.y < 0.0, "marker should be south of level-0 wall");
        assert!(label.at.y > -50.0, "marker should not be near level-1 wall");

        // Level 1: only the level-1 wall is used, marker near y=100
        let mut out2 = PlanAnnotation::default();
        callouts(&model, Some(1), &mut out2);
        assert_eq!(out2.labels.len(), 1);
        let label2 = &out2.labels[0];
        assert!(label2.at.y < 100.0);
        assert!(label2.at.y > 50.0);
    }

    #[test]
    fn section_marker_independent_of_walls() {
        // No walls at all: section markers still appear at cut ends
        let mut model = ArchModel::default();
        let cut = [[-1.0, 5.0], [11.0, 5.0]];
        let view = make_view(0, ViewKind::Section, Vec2::Y, None, None, Some(cut));
        model.views.push(view);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        model.sheets.push(sheet);

        let mut out = PlanAnnotation::default();
        callouts(&model, None, &mut out);
        assert_eq!(out.labels.len(), 2);
        let xs: Vec<f64> = out.labels.iter().map(|l| l.at.x).collect();
        assert!(xs.iter().any(|x| (x - (-1.6)).abs() < 1e-6), "left marker missing");
        assert!(xs.iter().any(|x| (x - 11.6).abs() < 1e-6), "right marker missing");
    }

    #[test]
    fn interior_view_no_level_shows_on_all_levels() {
        let mut model = setup_model();
        let room_point = Vec2::new(5.0, 5.0);
        // Interior view with no level restriction
        let view = make_view(0, ViewKind::Interior, Vec2::Y, None, Some(room_point), None);
        model.views.push(view);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0)]);
        model.sheets.push(sheet);

        // Shows on level 0
        let mut out = PlanAnnotation::default();
        callouts(&model, Some(0), &mut out);
        assert_eq!(out.labels.len(), 1);

        // Shows on level 1
        let mut out2 = PlanAnnotation::default();
        callouts(&model, Some(1), &mut out2);
        assert_eq!(out2.labels.len(), 1);

        // Shows with no level filter
        let mut out3 = PlanAnnotation::default();
        callouts(&model, None, &mut out3);
        assert_eq!(out3.labels.len(), 1);
    }

    #[test]
    fn callout_at_nearest_of_two() {
        let mut model = setup_model();
        // Two interior views at different points
        let p1 = Vec2::new(2.0, 2.0);
        let p2 = Vec2::new(8.0, 8.0);
        let view1 = make_view(0, ViewKind::Interior, Vec2::Y, None, Some(p1), None);
        let view2 = make_view(1, ViewKind::Interior, Vec2::Y, None, Some(p2), None);
        model.views.push(view1);
        model.views.push(view2);
        let sheet = make_sheet("S1", "A-101", vec![make_sheet_view("1", 0), make_sheet_view("2", 1)]);
        model.sheets.push(sheet);

        // Point closer to p1
        let near_p1 = Vec2::new(2.5, 2.5);
        let found = callout_at(&model, None, near_p1, 2.0);
        assert_eq!(found, Some(ArchId(0)));

        // Point closer to p2
        let near_p2 = Vec2::new(7.5, 7.5);
        let found2 = callout_at(&model, None, near_p2, 2.0);
        assert_eq!(found2, Some(ArchId(1)));
    }
}
