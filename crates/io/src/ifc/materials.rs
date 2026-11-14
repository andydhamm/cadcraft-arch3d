//! Materials for the IFC export: a glass wall association and the material definitions it needs.

use super::Ctx;
use crate::step::P;

/// One glass material and one `IfcRelAssociatesMaterial` for every wall that is a glass wall or
/// has a glass upper split. The rest of the export stays byte-for-byte unchanged when there is no
/// glass in the model.
pub(super) fn emit(ctx: &mut Ctx) {
    let refs: Vec<_> =
        ctx.model.walls.iter().filter(|w| w.is_glass() || w.upper_is_glass()).filter_map(|w| ctx.walls.get(&w.id.0).copied()).collect();
    if refs.is_empty() {
        return;
    }
    let material = ctx.w.add("IfcMaterial", vec![P::str("Glass"), P::Null, P::str("Glass")]);
    ctx.w.add("IfcRelAssociatesMaterial", vec![P::str("material/glass"), P::Null, P::Null, P::Null, P::refs(&refs), P::Ref(material)]);
}

#[cfg(test)]
mod tests {
    use super::super::{IfcOptions, write_model};
    use cadcraft_arch3d::Scene3d;
    use cadcraft_doc::arch::{ArchModel, Wall};
    use cadcraft_geom::Vec2;

    #[test]
    fn glass_wall_has_an_ifcmaterial_glass_association() {
        let mut model = ArchModel::default();
        model.push_wall(Wall {
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(3.0, 0.0),
            thickness: 0.2,
            height: 2.4,
            wall_type: "Glass".into(),
            ..Wall::default()
        });
        let text = write_model(&model, &Scene3d::build(&model), &IfcOptions::default());
        assert!(text.contains("IFCMATERIAL('Glass'"), "{text}");
        assert!(text.contains("IFCRELASSOCIATESMATERIAL"), "{text}");
        assert!(text.contains("'material/glass'"), "{text}");
    }
}
