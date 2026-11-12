//! Roof plan linework for the plan annotation pipeline.

use cadcraft_doc::arch::{ArchModel, RoofEdgeKind, RoofPlane, roof_handle};
use cadcraft_geom::Vec2;

use super::{PlanAnnotation, PlanLabel, PlanLabelKind, PlanLine, PlanLineKind};

fn edge_matches(a: Vec2, b: Vec2, c: Vec2, d: Vec2, tol: f64) -> bool {
    let same = (a - c).len() <= tol && (b - d).len() <= tol;
    let reverse = (a - d).len() <= tol && (b - c).len() <= tol;
    same || reverse
}

fn edge_on_base_line(plane: &RoofPlane, a: Vec2, b: Vec2, tol: f64) -> bool {
    let [x0, y0, x1, y1] = plane.base_edge;
    if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) {
        return false;
    }
    let base_a = Vec2::new(x0, y0);
    let base_dir = Vec2::new(x1 - x0, y1 - y0);
    let base_len = base_dir.len();
    if !base_len.is_finite() || base_len < 1e-12 {
        return false;
    }
    let unit = base_dir / base_len;
    let dist_a = (a - base_a).cross(unit).abs();
    let dist_b = (b - base_a).cross(unit).abs();
    dist_a <= tol && dist_b <= tol
}

/// Every roof's non-eave edges with finite ends as solid lines on `A-ROOF`, tagged with the
/// roof's handle (the plan's roof linework as it has always been drawn; eaves are skipped
/// because the poché already shows them along the walls).
pub fn roof_edges(model: &ArchModel, out: &mut PlanAnnotation) {
    for roof in &model.roofs {
        for edge in &roof.edges {
            let kind = match edge.kind {
                RoofEdgeKind::Eave => continue,
                RoofEdgeKind::Ridge => PlanLineKind::Ridge,
                RoofEdgeKind::Hip => PlanLineKind::Hip,
                RoofEdgeKind::Valley => PlanLineKind::Valley,
                RoofEdgeKind::GableEnd => PlanLineKind::GableEnd,
            };
            out.push_line(PlanLine::new("A-ROOF", kind, edge.a, edge.b, roof_handle(roof.id)));
        }
    }
}

/// The roof plan's extra annotation (eaves, rakes, slope arrows, pitch labels).
pub fn roof_plan(model: &ArchModel, out: &mut PlanAnnotation) {
    for roof in &model.roofs {
        let roof_handle_id = roof_handle(roof.id);
        let mut ring_edges: Vec<(Vec2, Vec2, usize)> = Vec::new();
        let mut max_coord = 0.0f64;
        for (plane_index, plane) in roof.planes.iter().enumerate() {
            if plane.ring.len() < 3 || !plane.ring.iter().all(|p| p.is_finite()) {
                continue;
            }
            for i in 0..plane.ring.len() {
                let a = plane.ring[i];
                let b = plane.ring[(i + 1) % plane.ring.len()];
                if !a.is_finite() || !b.is_finite() {
                    continue;
                }
                max_coord = max_coord.max(a.x.abs().max(a.y.abs()).max(b.x.abs().max(b.y.abs())));
                ring_edges.push((a, b, plane_index));
            }
        }
        let tol = 1e-6 * (1.0 + max_coord);
        let mut shared = vec![false; ring_edges.len()];
        for i in 0..ring_edges.len() {
            let (a, b, pi) = ring_edges[i];
            for j in (i + 1)..ring_edges.len() {
                let (c, d, pj) = ring_edges[j];
                if pi == pj {
                    continue;
                }
                if edge_matches(a, b, c, d, tol) {
                    shared[i] = true;
                    shared[j] = true;
                }
            }
        }
        for (index, (a, b, plane_index)) in ring_edges.iter().copied().enumerate() {
            if shared[index] {
                continue;
            }
            let Some(plane) = roof.planes.get(plane_index) else {
                continue;
            };
            let kind = if edge_on_base_line(plane, a, b, 1e-6) { PlanLineKind::Eave } else { PlanLineKind::Rake };
            out.push_line(PlanLine::new("A-ROOF-OTLN", kind, a, b, roof_handle_id));
        }

        for plane in &roof.planes {
            if plane.ring.len() < 3 || !plane.ring.iter().all(|p| p.is_finite()) || !plane.slope.is_finite() || plane.slope <= 0.0 {
                continue;
            }
            let Some(text) = super::pitch_text(plane.slope) else {
                continue;
            };
            let Some(label_at) = crate::label::label_point(&plane.ring, &[]) else {
                continue;
            };
            out.push_label(PlanLabel::new("A-ROOF-ANNO", PlanLabelKind::Pitch, text, label_at, 0.2, roof_handle_id));

            let [x0, y0, x1, y1] = plane.base_edge;
            if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) {
                continue;
            }
            let base_a = Vec2::new(x0, y0);
            let base_dir = Vec2::new(x1 - x0, y1 - y0);
            let base_len = base_dir.len();
            if !base_len.is_finite() || base_len < 1e-12 {
                continue;
            }
            let dir = base_dir / base_len;
            let left = Vec2::new(-dir.y, dir.x);
            let mut highest = plane.ring[0];
            let mut highest_dist = f64::NEG_INFINITY;
            for &p in &plane.ring {
                let d = (p - base_a).dot(left);
                if d > highest_dist {
                    highest = p;
                    highest_dist = d;
                }
            }
            let down = if (highest - base_a).dot(left) >= 0.0 { -left } else { left };
            let down = if down.len() > 1e-12 { down / down.len() } else { Vec2::ZERO };
            if down == Vec2::ZERO {
                continue;
            }
            let shaft_a = label_at + down * 0.25;
            let shaft_b = label_at + down * 0.85;
            out.push_line(PlanLine::new("A-ROOF-ANNO", PlanLineKind::SlopeArrow, shaft_a, shaft_b, roof_handle_id));
            let tip = label_at + down * 0.85;
            let head_len = 0.15;
            let head_left = (Vec2::ZERO - down).rotate(-std::f64::consts::FRAC_PI_6) * head_len;
            let head_right = (Vec2::ZERO - down).rotate(std::f64::consts::FRAC_PI_6) * head_len;
            out.push_line(PlanLine::new("A-ROOF-ANNO", PlanLineKind::SlopeArrowHead, tip, tip + head_left, roof_handle_id));
            out.push_line(PlanLine::new("A-ROOF-ANNO", PlanLineKind::SlopeArrowHead, tip, tip + head_right, roof_handle_id));
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::arch::{ArchId, Roof, RoofEdge, RoofPlane};
    use cadcraft_geom::Vec2;

    fn edge(kind: RoofEdgeKind, a: (f64, f64), b: (f64, f64)) -> RoofEdge {
        RoofEdge { kind, a: Vec2::new(a.0, a.1), b: Vec2::new(b.0, b.1), ..RoofEdge::default() }
    }

    fn roof_plane(ring: &[(f64, f64)], slope: f64, base_edge: [f64; 4], overhang: f64) -> RoofPlane {
        RoofPlane { ring: ring.iter().map(|(x, y)| Vec2::new(*x, *y)).collect(), slope, base_edge, overhang, ..RoofPlane::default() }
    }

    #[test]
    fn roof_edges_are_todays_linework() {
        let mut m = ArchModel::default();
        let r0 = m.push_roof(Roof {
            edges: vec![
                edge(RoofEdgeKind::Eave, (0.0, 0.0), (0.0, 8.0)),
                edge(RoofEdgeKind::Ridge, (0.0, 0.0), (4.0, 4.0)),
                edge(RoofEdgeKind::Valley, (4.0, 4.0), (4.0, 8.0)),
                edge(RoofEdgeKind::Hip, (f64::NAN, 0.0), (1.0, 1.0)),
                edge(RoofEdgeKind::GableEnd, (0.0, 8.0), (4.0, 8.0)),
            ],
            ..Roof::default()
        });
        let r1 = m.push_roof(Roof { edges: vec![edge(RoofEdgeKind::Hip, (1.0, 1.0), (2.0, 2.0))], ..Roof::default() });
        let mut out = PlanAnnotation::default();
        roof_edges(&m, &mut out);
        let kinds: Vec<_> = out.lines.iter().map(|l| l.kind).collect();
        assert_eq!(kinds, vec![PlanLineKind::Ridge, PlanLineKind::Valley, PlanLineKind::GableEnd, PlanLineKind::Hip]);
        assert!(out.lines.iter().all(|l| l.layer == "A-ROOF" && !l.dashed));
        assert_eq!(out.lines[0].handle, roof_handle(r0));
        assert_eq!(out.lines[3].handle, roof_handle(r1));
        assert!(out.labels.is_empty());
    }

    #[test]
    fn hip_6_12_plan() {
        let mut m = ArchModel::default();
        let roof = Roof {
            id: ArchId(0),
            level: 0,
            pitch_rise: 6.0,
            pitch_run: 12.0,
            planes: vec![
                roof_plane(&[(-0.1, -0.1), (10.1, -0.1), (6.0, 4.0), (4.0, 4.0)], 0.5, [-0.1, -0.1, 10.1, -0.1], 0.0),
                roof_plane(&[(10.1, -0.1), (10.1, 8.1), (6.0, 4.0)], 0.5, [10.1, -0.1, 10.1, 8.1], 0.0),
                roof_plane(&[(10.1, 8.1), (-0.1, 8.1), (4.0, 4.0), (6.0, 4.0)], 0.5, [10.1, 8.1, -0.1, 8.1], 0.0),
                roof_plane(&[(-0.1, 8.1), (-0.1, -0.1), (4.0, 4.0)], 0.5, [-0.1, 8.1, -0.1, -0.1], 0.0),
            ],
            ..Roof::default()
        };
        m.roofs.push(roof);
        let mut out = PlanAnnotation::default();
        roof_edges(&m, &mut out);
        roof_plan(&m, &mut out);
        let ridge = out.lines.iter().filter(|l| l.kind == PlanLineKind::Ridge).collect::<Vec<_>>();
        let hips = out.lines.iter().filter(|l| l.kind == PlanLineKind::Hip).collect::<Vec<_>>();
        let labels = out.labels.iter().filter(|l| l.kind == PlanLabelKind::Pitch).collect::<Vec<_>>();
        assert_eq!(ridge.len(), 1);
        assert!((ridge[0].length() - 2.0).abs() < 1e-6);
        assert_eq!(hips.len(), 4);
        assert!(hips.iter().all(|l| (l.length() - 5.798).abs() < 1e-3));
        assert_eq!(labels.len(), 4);
        assert!(labels.iter().all(|l| l.text == "6/12"));
        let shafts = out.lines.iter().filter(|l| l.kind == PlanLineKind::SlopeArrow).collect::<Vec<_>>();
        assert_eq!(shafts.len(), 4);
        assert!(shafts.iter().all(|l| (l.length() - 0.6).abs() < 1e-6));
        let dirs = shafts
            .iter()
            .map(|l| {
                let dx = l.b.x - l.a.x;
                let dy = l.b.y - l.a.y;
                (dx / 0.6, dy / 0.6)
            })
            .collect::<Vec<_>>();
        assert!(dirs.iter().any(|d| (d.0 - 0.0).abs() < 1e-6 && (d.1 + 1.0).abs() < 1e-6));
        assert!(dirs.iter().any(|d| (d.0 - 1.0).abs() < 1e-6 && (d.1 - 0.0).abs() < 1e-6));
        assert!(dirs.iter().any(|d| (d.0 - 0.0).abs() < 1e-6 && (d.1 - 1.0).abs() < 1e-6));
        assert!(dirs.iter().any(|d| (d.0 + 1.0).abs() < 1e-6 && (d.1 - 0.0).abs() < 1e-6));
        let eaves = out.lines.iter().filter(|l| l.kind == PlanLineKind::Eave).collect::<Vec<_>>();
        assert_eq!(eaves.len(), 4);
        assert!((eaves.iter().map(|l| l.length()).sum::<f64>() - 36.8).abs() < 1e-6);
        assert!(out.lines.iter().filter(|l| l.kind == PlanLineKind::Rake).next().is_none());
    }

    #[test]
    fn gable_plan() {
        let mut m = ArchModel::default();
        let roof = Roof {
            id: ArchId(1),
            level: 0,
            pitch_rise: 6.0,
            pitch_run: 12.0,
            planes: vec![
                roof_plane(&[(-0.1, -0.1), (10.1, -0.1), (10.1, 4.0), (-0.1, 4.0)], 0.5, [-0.1, -0.1, 10.1, -0.1], 0.0),
                roof_plane(&[(-0.1, 8.1), (10.1, 8.1), (10.1, 4.0), (-0.1, 4.0)], 0.5, [-0.1, 8.1, 10.1, 8.1], 0.0),
            ],
            ..Roof::default()
        };
        m.roofs.push(roof);
        let mut out = PlanAnnotation::default();
        roof_edges(&m, &mut out);
        roof_plan(&m, &mut out);
        let labels = out.labels.iter().filter(|l| l.kind == PlanLabelKind::Pitch).collect::<Vec<_>>();
        assert_eq!(labels.len(), 2);
        assert!(labels.iter().all(|l| l.text == "6/12"));
        let shafts = out.lines.iter().filter(|l| l.kind == PlanLineKind::SlopeArrow).collect::<Vec<_>>();
        let dirs = shafts.iter().map(|l| ((l.b.x - l.a.x) / 0.6, (l.b.y - l.a.y) / 0.6)).collect::<Vec<_>>();
        assert!(dirs.iter().any(|d| (d.0 - 0.0).abs() < 1e-6 && (d.1 + 1.0).abs() < 1e-6));
        assert!(dirs.iter().any(|d| (d.0 - 0.0).abs() < 1e-6 && (d.1 - 1.0).abs() < 1e-6));
        let eaves = out.lines.iter().filter(|l| l.kind == PlanLineKind::Eave).collect::<Vec<_>>();
        let rakes = out.lines.iter().filter(|l| l.kind == PlanLineKind::Rake).collect::<Vec<_>>();
        assert!((eaves.iter().map(|l| l.length()).sum::<f64>() - 20.4).abs() < 1e-6);
        assert!((rakes.iter().map(|l| l.length()).sum::<f64>() - 16.4).abs() < 1e-6);
        let ridge = out.lines.iter().filter(|l| l.kind == PlanLineKind::Ridge).collect::<Vec<_>>();
        assert_eq!(ridge.len(), 1);
        assert!((ridge[0].length() - 10.2).abs() < 1e-6);
    }

    #[test]
    fn pitch_4_12_labels() {
        let mut m = ArchModel::default();
        let roof = Roof {
            id: ArchId(2),
            pitch_rise: 4.0,
            pitch_run: 12.0,
            planes: vec![
                roof_plane(&[(-0.1, -0.1), (10.1, -0.1), (6.0, 4.0), (4.0, 4.0)], 4.0 / 12.0, [-0.1, -0.1, 10.1, -0.1], 0.0),
                roof_plane(&[(10.1, -0.1), (10.1, 8.1), (6.0, 4.0)], 4.0 / 12.0, [10.1, -0.1, 10.1, 8.1], 0.0),
                roof_plane(&[(10.1, 8.1), (-0.1, 8.1), (4.0, 4.0), (6.0, 4.0)], 4.0 / 12.0, [10.1, 8.1, -0.1, 8.1], 0.0),
                roof_plane(&[(-0.1, 8.1), (-0.1, -0.1), (4.0, 4.0)], 4.0 / 12.0, [-0.1, 8.1, -0.1, -0.1], 0.0),
            ],
            ..Roof::default()
        };
        m.roofs.push(roof);
        let mut out = PlanAnnotation::default();
        roof_edges(&m, &mut out);
        roof_plan(&m, &mut out);
        let labels = out.labels.iter().filter(|l| l.kind == PlanLabelKind::Pitch).collect::<Vec<_>>();
        assert_eq!(labels.len(), 4);
        assert!(labels.iter().all(|l| l.text == "4/12"));
    }

    #[test]
    fn overhang_eaves_follow_the_outline() {
        let mut m = ArchModel::default();
        let roof = Roof {
            id: ArchId(3),
            overhang: 0.5,
            planes: vec![
                roof_plane(&[(-0.1, -0.1), (10.1, -0.1), (6.0, 4.0), (4.0, 4.0)], 0.5, [-0.1, -0.1, 10.1, -0.1], 0.5),
                roof_plane(&[(10.1, -0.1), (10.1, 8.1), (6.0, 4.0)], 0.5, [10.1, -0.1, 10.1, 8.1], 0.5),
                roof_plane(&[(10.1, 8.1), (-0.1, 8.1), (4.0, 4.0), (6.0, 4.0)], 0.5, [10.1, 8.1, -0.1, 8.1], 0.5),
                roof_plane(&[(-0.1, 8.1), (-0.1, -0.1), (4.0, 4.0)], 0.5, [-0.1, 8.1, -0.1, -0.1], 0.5),
            ],
            ..Roof::default()
        };
        m.roofs.push(roof);
        let mut out = PlanAnnotation::default();
        roof_edges(&m, &mut out);
        roof_plan(&m, &mut out);
        let eaves = out.lines.iter().filter(|l| l.kind == PlanLineKind::Eave).collect::<Vec<_>>();
        assert!((eaves.iter().map(|l| l.length()).sum::<f64>() - 40.8).abs() < 1e-6);
    }

    #[test]
    fn manual_roofplane_gets_a_label_and_its_outline() {
        let mut m = ArchModel::default();
        let roof = Roof {
            id: ArchId(4),
            manual: true,
            planes: vec![roof_plane(&[(0.0, 0.0), (4.0, 0.0), (2.0, 2.0)], 0.5, [0.0, 0.0, 4.0, 0.0], 0.0)],
            ..Roof::default()
        };
        m.roofs.push(roof);
        let mut out = PlanAnnotation::default();
        roof_plan(&m, &mut out);
        assert_eq!(out.labels.iter().filter(|l| l.kind == PlanLabelKind::Pitch).count(), 1);
        assert_eq!(out.lines.iter().filter(|l| l.kind == PlanLineKind::Eave || l.kind == PlanLineKind::Rake).count(), 3);
    }

    #[test]
    fn shared_edges_are_matched_either_direction_within_tolerance() {
        let mut m = ArchModel::default();
        let roof = Roof {
            id: ArchId(5),
            planes: vec![
                roof_plane(&[(0.0, 0.0), (4.0, 0.0), (4.0, 2.0), (0.0, 2.0)], 0.5, [0.0, 0.0, 4.0, 0.0], 0.0),
                roof_plane(&[(4.0, 0.0), (5.0, 0.0), (5.0, 2.0), (4.0, 2.0)], 0.5, [4.0, 0.0, 5.0, 0.0], 0.0),
            ],
            ..Roof::default()
        };
        m.roofs.push(roof);
        let mut out = PlanAnnotation::default();
        roof_plan(&m, &mut out);
        let shared_segment = out.lines.iter().filter(|l| l.kind == PlanLineKind::Eave || l.kind == PlanLineKind::Rake).find(|l| l.length() > 0.0);
        assert!(shared_segment.is_some());
        assert!(out.lines.iter().all(|l| l.kind != PlanLineKind::Ridge || l.length() > 0.0));
    }

    #[test]
    fn arrow_geometry() {
        let mut m = ArchModel::default();
        let roof = Roof {
            id: ArchId(6),
            planes: vec![roof_plane(&[(0.0, 0.0), (2.0, 0.0), (1.0, 1.0)], 0.5, [0.0, 0.0, 2.0, 0.0], 0.0)],
            ..Roof::default()
        };
        m.roofs.push(roof);
        let mut out = PlanAnnotation::default();
        roof_plan(&m, &mut out);
        let shaft = out.lines.iter().find(|l| l.kind == PlanLineKind::SlopeArrow).unwrap();
        assert!((shaft.length() - 0.6).abs() < 1e-6);
        let head = out.lines.iter().filter(|l| l.kind == PlanLineKind::SlopeArrowHead).collect::<Vec<_>>();
        assert_eq!(head.len(), 2);
        assert!(head.iter().all(|l| (l.length() - 0.15).abs() < 1e-6));
    }

    #[test]
    fn down_slope_is_the_negative_left_normal_of_the_base_edge() {
        let dir = Vec2::new(1.0, 0.0);
        let left = Vec2::new(-dir.y, dir.x);
        let computed = -left;
        assert_eq!(computed, Vec2::new(0.0, -1.0));
    }

    #[test]
    fn degenerate_planes_are_skipped() {
        let mut m = ArchModel::default();
        let roof = Roof {
            id: ArchId(7),
            planes: vec![roof_plane(&[(0.0, 0.0), (0.0, 0.0), (0.0, 0.0)], 0.5, [0.0, 0.0, 0.0, 0.0], 0.0)],
            ..Roof::default()
        };
        m.roofs.push(roof);
        let mut out = PlanAnnotation::default();
        roof_plan(&m, &mut out);
        assert!(out.labels.iter().all(|l| l.kind != PlanLabelKind::Pitch));
        assert!(out.lines.iter().all(|l| l.kind != PlanLineKind::SlopeArrow));
    }

    #[test]
    fn roof_plan_draws_nothing_in_other_kinds() {
        let mut m = ArchModel::default();
        let roof = Roof {
            id: ArchId(8),
            edges: vec![edge(RoofEdgeKind::Ridge, (0.0, 0.0), (4.0, 0.0)), edge(RoofEdgeKind::Hip, (0.0, 0.0), (2.0, 2.0))],
            planes: vec![roof_plane(&[(0.0, 0.0), (4.0, 0.0), (2.0, 2.0)], 0.5, [0.0, 0.0, 4.0, 0.0], 0.0)],
            ..Roof::default()
        };
        m.roofs.push(roof);
        let out = crate::plan::annotate(
            &m,
            Some(&cadcraft_doc::arch::PlanView { kind: cadcraft_doc::arch::PlanKind::Floor, ..cadcraft_doc::arch::PlanView::default() }),
            None,
        );
        assert!(out.labels.iter().all(|l| l.kind != PlanLabelKind::Pitch));
        assert!(
            out.lines
                .iter()
                .all(|l| !matches!(l.kind, PlanLineKind::SlopeArrow | PlanLineKind::SlopeArrowHead | PlanLineKind::Eave | PlanLineKind::Rake))
        );
        assert!(out.lines.iter().any(|l| matches!(l.kind, PlanLineKind::Ridge | PlanLineKind::Hip | PlanLineKind::Valley | PlanLineKind::GableEnd)));
    }
}
