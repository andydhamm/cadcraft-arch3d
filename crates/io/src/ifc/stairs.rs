//! Stairs and railings (phase 12).

use super::Ctx;
use crate::step::P;
use cadcraft_doc::arch::StairShape;

/// One `IfcStair` (key `stair/<n>`, `PredefinedType` from the stair's shape) per stair in its
/// `from_level` storey, and one `IfcRailing` (`.HANDRAIL.` for a stair's own rail, else
/// `.GUARDRAIL.`; key `railing/<n>`) per railing in its level storey, bodies from `ctx.pieces`.
pub(super) fn emit(ctx: &mut Ctx) {
    for stair in &ctx.model.stairs {
        let meshes = ctx.pieces.get("stair", stair.id).to_vec();
        if meshes.is_empty() {
            continue;
        }
        let n = stair.id.0;
        let slot = ctx.storey_for(stair.from_level);
        let _ = ctx.element(
            "IfcStair",
            &format!("stair/{n}"),
            &format!("stair-s{n}"),
            slot,
            &meshes,
            vec![
                P::Null,
                match stair.shape {
                    StairShape::Straight => P::Enum("STRAIGHT_RUN_STAIR"),
                    StairShape::L => P::Enum("QUARTER_TURN_STAIR"),
                    StairShape::U => P::Enum("HALF_TURN_STAIR"),
                },
            ],
        );
    }
    for railing in &ctx.model.railings {
        let meshes = ctx.pieces.get("railing", railing.id).to_vec();
        if meshes.is_empty() {
            continue;
        }
        let n = railing.id.0;
        let slot = ctx.storey_for(railing.level);
        let _ = ctx.element(
            "IfcRailing",
            &format!("railing/{n}"),
            &format!("railing-ra{n}"),
            slot,
            &meshes,
            vec![P::Null, P::Enum(if railing.follows.is_some() { "HANDRAIL" } else { "GUARDRAIL" })],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::{IfcOptions, write_model};
    use cadcraft_arch3d::Scene3d;
    use cadcraft_doc::arch::{ArchModel, Railing, Stair, StairShape};
    use cadcraft_geom::Vec2;

    /// A model with one stair of each `StairShape` (straight, L, U).
    fn stairs_model() -> ArchModel {
        let mut m = ArchModel::default();
        m.push_stair(Stair { shape: StairShape::Straight, ..Stair::default() });
        m.push_stair(Stair { shape: StairShape::L, ..Stair::default() });
        m.push_stair(Stair { shape: StairShape::U, ..Stair::default() });
        m
    }

    #[test]
    fn a_stair_is_an_ifcstair_by_shape() {
        let m = stairs_model();
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert_eq!(text.matches("=IFCSTAIR(").count(), 3, "{text}");
        assert!(text.contains(".STRAIGHT_RUN_STAIR."), "straight stair");
        assert!(text.contains(".QUARTER_TURN_STAIR."), "L stair");
        assert!(text.contains(".HALF_TURN_STAIR."), "U stair");
    }

    #[test]
    fn railings_are_handrails_or_guardrails() {
        let mut m = stairs_model();
        let stair_id = m.stairs[0].id;
        // A rail that follows a stair (handrail) and one that doesn't (guardrail).
        m.push_railing(Railing { path: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0)], follows: Some(stair_id), ..Railing::default() });
        m.push_railing(Railing { path: vec![Vec2::new(0.0, 1.0), Vec2::new(2.0, 1.0)], follows: None, ..Railing::default() });
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert_eq!(text.matches("=IFCRAILING(").count(), 2, "{text}");
        assert!(text.contains(".HANDRAIL."), "stair rail is a handrail");
        assert!(text.contains(".GUARDRAIL."), "free-standing rail is a guardrail");
    }
}
