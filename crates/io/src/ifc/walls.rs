//! `IfcWall`s (phase 12). Written in full with the groundwork as the pattern for the other element
//! modules: a key and a name per object, `Ctx::element` for the product, then its property set.

use super::Ctx;
use crate::step::P;

/// One `IfcWall` (`PredefinedType` `.STANDARD.`) per wall that built meshes, in wall order: key
/// `wall/<n>`, name `wall-w<n>` (the GLB node's name), body from `ctx.pieces.get("wall", id)`,
/// contained in the storey of the wall's `base_level`; `Pset_WallCommon` with `IsExternal`
/// (`IFCBOOLEAN`) from `Wall::exterior`. Records each wall in `ctx.walls` for the openings.
pub(super) fn emit(ctx: &mut Ctx) {
    for wall in &ctx.model.walls {
        let meshes = ctx.pieces.get("wall", wall.id).to_vec();
        if meshes.is_empty() {
            continue;
        }
        let n = wall.id.0;
        let slot = ctx.storey_for(wall.base_level);
        let r = ctx.element("IfcWall", &format!("wall/{n}"), &format!("wall-w{n}"), slot, &meshes, vec![P::Null, P::Enum("STANDARD")]);
        ctx.pset("Pset_WallCommon", &format!("wall/{n}"), vec![("IsExternal", P::typed("IFCBOOLEAN", P::Bool(wall.exterior)))], &[r]);
        ctx.walls.insert(n, r);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{IfcOptions, write_model};
    use cadcraft_arch3d::Scene3d;
    use cadcraft_doc::arch::{ArchModel, Wall};
    use cadcraft_geom::Vec2;

    fn model() -> ArchModel {
        let mut m = ArchModel::default();
        for (a, b) in [((0.0, 0.0), (4.0, 0.0)), ((4.0, 0.0), (4.0, 3.0))] {
            m.push_wall(Wall {
                start: Vec2::new(a.0, a.1),
                end: Vec2::new(b.0, b.1),
                thickness: 0.2,
                height: 2.4,
                exterior: true,
                ..Wall::default()
            });
        }
        m
    }

    #[test]
    fn each_wall_is_an_ifcwall_with_its_body_and_pset() {
        let m = model();
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert_eq!(text.matches("=IFCWALL(").count(), 2, "{text}");
        assert!(text.contains(",'wall-w0',$,$,#") && text.contains(",'wall-w1',$,$,#"), "{text}");
        assert!(text.contains(",$,.STANDARD.);"), "{text}");
        assert_eq!(text.matches("=IFCPROPERTYSET(").count(), 2);
        assert!(text.contains("IFCPROPERTYSINGLEVALUE('IsExternal',$,IFCBOOLEAN(.T.),$)"), "{text}");
        assert_eq!(text.matches("=IFCTRIANGULATEDFACESET(").count(), 2, "one body per wall");
        assert_eq!(text.matches("=IFCRELCONTAINEDINSPATIALSTRUCTURE(").count(), 1, "both in the one storey");
    }
}
