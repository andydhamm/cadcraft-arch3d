//! Reference-level display for the plan pipeline.

use std::collections::BTreeMap;

use cadcraft_doc::arch::{ArchModel, ReferenceDisplay, arch_handle};

use crate::{join::solve_joins, wall::wall_polygon};

/// Faint lines of the reference level's walls under the shown level `shown`.
pub fn reference_lines(model: &ArchModel, shown: u32, r: &ReferenceDisplay, out: &mut super::PlanAnnotation) {
    if r.level == shown {
        return;
    }
    let Some(ref_level) = model.level(r.level) else {
        return;
    };

    let solved: BTreeMap<_, _> = solve_joins(model).into_iter().map(|shape| (shape.id, shape)).collect();
    let shown_level = model.level(shown);
    for wall in &model.walls {
        if wall.divider || !wall.thickness.is_finite() || wall.thickness <= 0.0 || !wall.start.is_finite() || !wall.end.is_finite() {
            continue;
        }
        if wall.start.dist(wall.end) < 1e-6 || !wall.start.dist(wall.end).is_finite() {
            continue;
        }
        if !wall.spans_level(ref_level, &model.levels) {
            continue;
        }
        let spans_shown = match shown_level {
            Some(level) => wall.spans_level(level, &model.levels),
            None => wall.base_level == shown,
        };
        if spans_shown {
            continue;
        }

        let ring = match solved.get(&wall.id) {
            Some(shape) if !shape.polygon.is_empty() => shape.polygon.clone(),
            _ => match wall_polygon(wall) {
                Some(poly) => poly,
                None => continue,
            },
        };
        if ring.len() < 2 || !ring.iter().all(|p| p.is_finite()) {
            continue;
        }

        for i in 0..ring.len() {
            let a = ring[i];
            let b = ring[(i + 1) % ring.len()];
            if !a.is_finite() || !b.is_finite() {
                continue;
            }
            let seg = b - a;
            if !seg.is_finite() || seg.len() < 1e-6 {
                continue;
            }
            let line = super::PlanLine::new("A-WALL", super::PlanLineKind::Reference, a, b, arch_handle(wall.id)).colored(r.color, r.transparency);
            out.push_line(line);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use cadcraft_doc::Wall;
    use cadcraft_doc::arch::{Level, ReferenceDisplay};
    use cadcraft_geom::Vec2;

    use crate::plan::{PlanAnnotation, PlanLineKind};

    use super::*;

    #[test]
    fn reference_draws_the_other_levels_solved_poche_outlines() {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, elevation: 0.0, ..Level::default() }, Level { index: 1, elevation: 3.0, ..Level::default() }];
        let low0 = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        let low1 = m.push_wall(Wall::new(Vec2::new(0.0, 3.0), Vec2::new(4.0, 3.0), 0.2));
        let upper =
            m.push_wall(Wall { start: Vec2::new(0.0, 5.0), end: Vec2::new(4.0, 5.0), thickness: 0.2, height: 2.4, base_level: 1, ..Wall::default() });
        m.levels[1].ceiling_height = 2.4;
        let mut out = PlanAnnotation::default();
        reference_lines(&m, 1, &ReferenceDisplay::default(), &mut out);
        let handles: BTreeSet<_> = out.lines.iter().map(|line| line.handle).collect();
        assert!(handles.contains(&arch_handle(low0)));
        assert!(handles.contains(&arch_handle(low1)));
        assert!(!handles.contains(&arch_handle(upper)));
        assert!(out.lines.iter().all(|l| l.kind == PlanLineKind::Reference));
    }

    #[test]
    fn walls_spanning_both_levels_are_not_reference() {
        let mut m = ArchModel::default();
        m.levels = vec![
            Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Level::default() },
            Level { index: 1, elevation: 2.4, ceiling_height: 2.4, ..Level::default() },
        ];
        let shared = m.push_wall(Wall {
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(4.0, 0.0),
            thickness: 0.2,
            height: 2.4,
            top_level: Some(1),
            ..Wall::default()
        });
        let mut out = PlanAnnotation::default();
        reference_lines(&m, 1, &ReferenceDisplay { level: 0, ..ReferenceDisplay::default() }, &mut out);
        assert!(!out.lines.iter().any(|line| line.handle == arch_handle(shared)));
    }

    #[test]
    fn reference_lines_carry_colour_transparency_and_wall_handles() {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, elevation: 0.0, ..Level::default() }, Level { index: 1, elevation: 3.0, ..Level::default() }];
        let wall_id = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        let mut out = PlanAnnotation::default();
        let r = ReferenceDisplay { level: 0, color: [255, 136, 0], transparency: 42, ..ReferenceDisplay::default() };
        reference_lines(&m, 1, &r, &mut out);
        assert_eq!(out.lines.len(), 4);
        for line in &out.lines {
            assert_eq!(line.layer, "A-WALL");
            assert_eq!(line.kind, PlanLineKind::Reference);
            assert_eq!(line.handle, arch_handle(wall_id));
            assert_eq!(line.color, Some([255, 136, 0]));
            assert_eq!(line.transparency, Some(42));
        }
    }

    #[test]
    fn no_lines_when_the_reference_level_is_shown_or_missing() {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, elevation: 0.0, ..Level::default() }, Level { index: 1, elevation: 3.0, ..Level::default() }];
        m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        let mut out = PlanAnnotation::default();
        reference_lines(&m, 0, &ReferenceDisplay { level: 0, ..ReferenceDisplay::default() }, &mut out);
        assert!(out.lines.is_empty());
        let mut out = PlanAnnotation::default();
        reference_lines(&m, 1, &ReferenceDisplay { level: 9, ..ReferenceDisplay::default() }, &mut out);
        assert!(out.lines.is_empty());
    }

    #[test]
    fn dividers_draw_nothing() {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, elevation: 0.0, ..Level::default() }, Level { index: 1, elevation: 3.0, ..Level::default() }];
        m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.0, 0.0), thickness: 0.2, height: 2.4, divider: true, ..Wall::default() });
        let mut out = PlanAnnotation::default();
        reference_lines(&m, 1, &ReferenceDisplay { level: 0, ..ReferenceDisplay::default() }, &mut out);
        assert!(out.lines.is_empty());
    }

    #[test]
    fn hostile_walls_are_skipped() {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, elevation: 0.0, ..Level::default() }, Level { index: 1, elevation: 3.0, ..Level::default() }];
        let bad = Wall { start: Vec2::new(f64::NAN, 0.0), end: Vec2::new(4.0, 0.0), thickness: 0.2, height: 2.4, ..Wall::default() };
        m.push_wall(bad);
        let zero = Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(0.0, 0.0), thickness: 0.2, height: 2.4, ..Wall::default() };
        m.push_wall(zero);
        let mut out = PlanAnnotation::default();
        reference_lines(&m, 1, &ReferenceDisplay { level: 0, ..ReferenceDisplay::default() }, &mut out);
        assert!(out.lines.is_empty());
    }

    #[test]
    fn zero_and_negative_thickness_walls_are_skipped() {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, elevation: 0.0, ..Level::default() }, Level { index: 1, elevation: 3.0, ..Level::default() }];
        let zero_thick = Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.0, 0.0), thickness: 0.0, height: 2.4, ..Wall::default() };
        m.push_wall(zero_thick);
        let neg_thick = Wall { start: Vec2::new(0.0, 1.0), end: Vec2::new(4.0, 1.0), thickness: -0.1, height: 2.4, ..Wall::default() };
        m.push_wall(neg_thick);
        let good = Wall { start: Vec2::new(0.0, 2.0), end: Vec2::new(4.0, 2.0), thickness: 0.2, height: 2.4, ..Wall::default() };
        let good_id = m.push_wall(good);
        let mut out = PlanAnnotation::default();
        reference_lines(&m, 1, &ReferenceDisplay { level: 0, ..ReferenceDisplay::default() }, &mut out);
        let handles: BTreeSet<_> = out.lines.iter().map(|l| l.handle).collect();
        assert!(handles.contains(&arch_handle(good_id)));
        assert_eq!(handles.len(), 1);
    }

    #[test]
    fn non_finite_thickness_walls_are_skipped() {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, elevation: 0.0, ..Level::default() }, Level { index: 1, elevation: 3.0, ..Level::default() }];
        let nan_thick = Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.0, 0.0), thickness: f64::NAN, height: 2.4, ..Wall::default() };
        m.push_wall(nan_thick);
        let inf_thick = Wall { start: Vec2::new(0.0, 1.0), end: Vec2::new(4.0, 1.0), thickness: f64::INFINITY, height: 2.4, ..Wall::default() };
        m.push_wall(inf_thick);
        let mut out = PlanAnnotation::default();
        reference_lines(&m, 1, &ReferenceDisplay { level: 0, ..ReferenceDisplay::default() }, &mut out);
        assert!(out.lines.is_empty());
    }

    #[test]
    fn empty_model_produces_no_lines() {
        let m = ArchModel::default();
        let mut out = PlanAnnotation::default();
        reference_lines(&m, 0, &ReferenceDisplay::default(), &mut out);
        assert!(out.lines.is_empty());
    }

    #[test]
    fn shown_level_missing_falls_back_to_base_level() {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, elevation: 0.0, ..Level::default() }];
        let on_ref = Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(4.0, 0.0), thickness: 0.2, height: 2.4, base_level: 0, ..Wall::default() };
        let on_ref_id = m.push_wall(on_ref);
        let on_shown = Wall { start: Vec2::new(0.0, 1.0), end: Vec2::new(4.0, 1.0), thickness: 0.2, height: 2.4, base_level: 5, ..Wall::default() };
        let on_shown_id = m.push_wall(on_shown);
        let mut out = PlanAnnotation::default();
        reference_lines(&m, 5, &ReferenceDisplay { level: 0, ..ReferenceDisplay::default() }, &mut out);
        let handles: BTreeSet<_> = out.lines.iter().map(|l| l.handle).collect();
        assert!(handles.contains(&arch_handle(on_ref_id)));
        assert!(!handles.contains(&arch_handle(on_shown_id)));
    }
}
