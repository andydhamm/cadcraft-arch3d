//! Roof plan linework for the plan annotation pipeline.

use cadcraft_doc::arch::{ArchModel, RoofEdgeKind, roof_handle};

use super::{PlanAnnotation, PlanLine, PlanLineKind};

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
/// Stub: owner p20-roof-plan.
pub fn roof_plan(_model: &ArchModel, _out: &mut PlanAnnotation) {}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::arch::{Roof, RoofEdge};
    use cadcraft_geom::Vec2;

    fn edge(kind: RoofEdgeKind, a: (f64, f64), b: (f64, f64)) -> RoofEdge {
        RoofEdge { kind, a: Vec2::new(a.0, a.1), b: Vec2::new(b.0, b.1), ..RoofEdge::default() }
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
}
