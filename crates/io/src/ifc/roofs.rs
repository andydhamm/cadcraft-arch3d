//! Roofs (phase 12).

use super::Ctx;
use super::guid::global_id;
use crate::step::{P, Ref};
use cadcraft_doc::arch::ArchId;

/// One `IfcRoof` (key `roof/<n>`, no body of its own) per roof, aggregating (`IfcRelAggregates`,
/// `rel/aggregates/roof/<n>`) one `IfcSlab` (`.ROOF.`, key `roofplane/<n>/<i>`) per roof-plane mesh
/// and one `IfcCovering` (`.CLADDING.`, keys `fascia/<n>` and `soffit/<n>`) for its fascia and its
/// soffit, all in the roof's level storey (the `IfcRoof` is contained; its parts are not).
pub(super) fn emit(ctx: &mut Ctx) {
    let mut ids: Vec<ArchId> = ctx.pieces.ids("roof");
    for kind in ["fascia", "soffit"] {
        for id in ctx.pieces.ids(kind) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    for roof in &ctx.model.roofs {
        if !roof.planes.is_empty() && !ids.contains(&roof.id) {
            ids.push(roof.id);
        }
    }
    for id in ids {
        let n = id.0;
        let slot = match ctx.model.roofs.iter().find(|r| r.id == id) {
            Some(r) => ctx.storey_for(r.level),
            None => ctx.storeys.len().saturating_sub(1),
        };
        let z0 = ctx.storeys.get(slot).map_or(0.0, |s| s.elevation);
        let roof = ctx.element("IfcRoof", &format!("roof/{n}"), &format!("roof-r{n}"), slot, &[], vec![P::Null, P::Enum("NOTDEFINED")]);
        let mut parts: Vec<Ref> = Vec::new();
        let planes = ctx.pieces.get("roof", id).to_vec();
        for (i, mesh) in planes.iter().enumerate() {
            parts.push(part(ctx, "IfcSlab", &format!("roofplane/{n}/{i}"), &format!("roof-r{n}p{i}"), slot, z0, &[*mesh], "ROOF"));
        }
        for kind in ["fascia", "soffit"] {
            let meshes = ctx.pieces.get(kind, id).to_vec();
            if !meshes.is_empty() {
                parts.push(part(ctx, "IfcCovering", &format!("{kind}/{n}"), &format!("roof-r{n}-{kind}"), slot, z0, &meshes, "CLADDING"));
            }
        }
        ctx.rel("IfcRelAggregates", &format!("rel/aggregates/roof/{n}"), vec![P::Ref(roof), P::refs(&parts)]);
    }
}

/// A product that is a part (not contained in the storey): placement, body, `Tag $`, predefined type.
#[allow(clippy::too_many_arguments)]
fn part(ctx: &mut Ctx, entity: &str, key: &str, name: &str, slot: usize, z0: f64, meshes: &[&cadcraft_arch3d::Mesh], kind: &'static str) -> Ref {
    let placement = ctx.placement_in_storey(slot);
    let body = ctx.body(meshes, z0).map_or(P::Null, P::Ref);
    let params = vec![P::str(global_id(key)), P::Null, P::str(name), P::Null, P::Null, P::Ref(placement), body, P::Null, P::Enum(kind)];
    ctx.w.add(entity, params)
}

#[cfg(test)]
mod tests {
    use super::super::{IfcOptions, write_model};
    use cadcraft_arch3d::Scene3d;
    use cadcraft_doc::arch::{ArchModel, Roof, RoofPlane};
    use cadcraft_geom::Vec2;

    fn plane(y0: f64, y1: f64) -> RoofPlane {
        RoofPlane {
            base_edge: [0.0, y0, 10.0, y0],
            ring: vec![Vec2::new(0.0, y0), Vec2::new(10.0, y0), Vec2::new(10.0, y1), Vec2::new(0.0, y1)],
            slope: 0.5,
            eave_z: 2.4,
            thickness: 0.04,
            fascia_depth: 0.15,
            soffit_depth: 0.1,
            overhang: 0.4,
            ..Default::default()
        }
    }

    fn export(fascia: bool) -> String {
        let mut m = ArchModel::default();
        let mut roof = Roof { level: 0, fascia, soffit: fascia, ..Roof::default() };
        roof.planes.push(plane(0.0, 4.0));
        roof.planes.push(plane(8.0, 4.0));
        m.push_roof(roof);
        write_model(&m, &Scene3d::build(&m), &IfcOptions::default())
    }

    #[test]
    fn a_roof_aggregates_one_slab_per_plane() {
        let text = export(false);
        assert_eq!(text.matches("=IFCROOF(").count(), 1, "{text}");
        assert_eq!(text.matches("=IFCSLAB(").count(), 2, "{text}");
        assert!(text.contains(",$,.ROOF.);"), "{text}");
        assert!(text.contains(",$,.NOTDEFINED.);"), "{text}");
        assert_eq!(text.matches("=IFCRELCONTAINEDINSPATIALSTRUCTURE(").count(), 1, "only the roof is contained");
    }

    #[test]
    fn roof_planes_are_named_like_the_glb_nodes() {
        let text = export(false);
        let n = text.split("'roof-r").nth(1).map(|s| s.chars().take_while(char::is_ascii_digit).collect::<String>()).unwrap_or_default();
        assert!(!n.is_empty(), "{text}");
        assert!(text.contains(&format!("'roof-r{n}p0'")) && text.contains(&format!("'roof-r{n}p1'")), "{text}");
    }

    #[test]
    fn fascia_and_soffit_are_cladding_parts() {
        let text = export(true);
        assert!(text.contains("-fascia'") && text.contains("-soffit'"), "{text}");
        assert_eq!(text.matches("=IFCCOVERING(").count(), 2, "{text}");
        assert_eq!(text.matches(",$,.CLADDING.);").count(), 2, "{text}");
        assert_eq!(text.matches("=IFCRELCONTAINEDINSPATIALSTRUCTURE(").count(), 1, "{text}");
    }

    #[test]
    fn no_roofs_produces_no_roof_entities() {
        let m = ArchModel::default();
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert_eq!(text.matches("=IFCROOF(").count(), 0, "{text}");
        assert_eq!(text.matches("=IFCSLAB(").count(), 0, "{text}");
        assert_eq!(text.matches("=IFCCOVERING(").count(), 0, "{text}");
    }

    #[test]
    fn multiple_roofs_get_separate_ifc_entities() {
        let mut m = ArchModel::default();
        let mut roof1 = Roof { level: 0, ..Roof::default() };
        roof1.planes.push(plane(0.0, 4.0));
        m.push_roof(roof1);
        let mut roof2 = Roof { level: 0, ..Roof::default() };
        roof2.planes.push(plane(0.0, 4.0));
        m.push_roof(roof2);
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert_eq!(text.matches("=IFCROOF(").count(), 2, "{text}");
        assert_eq!(text.matches("=IFCSLAB(").count(), 2, "{text}");
        assert_eq!(text.matches("=IFCRELAGGREGATES(").count(), 2, "{text}");
    }

    #[test]
    fn roof_with_no_planes_produces_no_mesh_entities() {
        let mut m = ArchModel::default();
        let roof = Roof { level: 0, ..Roof::default() };
        m.push_roof(roof);
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert_eq!(text.matches("=IFCROOF(").count(), 0, "{text}");
        assert_eq!(text.matches("=IFCSLAB(").count(), 0, "{text}");
    }

    #[test]
    fn single_plane_roof() {
        let mut m = ArchModel::default();
        let mut roof = Roof { level: 0, ..Roof::default() };
        roof.planes.push(plane(0.0, 4.0));
        m.push_roof(roof);
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert_eq!(text.matches("=IFCROOF(").count(), 1, "{text}");
        assert_eq!(text.matches("=IFCSLAB(").count(), 1, "{text}");
        assert_eq!(text.matches("=IFCRELAGGREGATES(").count(), 1, "{text}");
    }

    #[test]
    fn roof_with_many_planes() {
        let mut m = ArchModel::default();
        let mut roof = Roof { level: 0, ..Roof::default() };
        for i in 0..10 {
            let y0 = i as f64 * 2.0;
            let y1 = y0 + 1.0;
            roof.planes.push(plane(y0, y1));
        }
        m.push_roof(roof);
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert_eq!(text.matches("=IFCROOF(").count(), 1, "{text}");
        assert_eq!(text.matches("=IFCSLAB(").count(), 10, "{text}");
    }

    #[test]
    fn roof_with_fascia_but_no_soffit() {
        let mut m = ArchModel::default();
        let mut roof = Roof { level: 0, fascia: true, soffit: false, ..Roof::default() };
        roof.planes.push(plane(0.0, 4.0));
        roof.planes.push(plane(8.0, 4.0));
        m.push_roof(roof);
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert_eq!(text.matches("=IFCCOVERING(").count(), 1, "{text}");
        assert!(text.contains("-fascia'"), "{text}");
        assert!(!text.contains("-soffit'"), "{text}");
    }

    #[test]
    fn roof_with_soffit_but_no_fascia() {
        let mut m = ArchModel::default();
        let mut roof = Roof { level: 0, fascia: false, soffit: true, ..Roof::default() };
        roof.planes.push(plane(0.0, 4.0));
        roof.planes.push(plane(8.0, 4.0));
        m.push_roof(roof);
        let text = write_model(&m, &Scene3d::build(&m), &IfcOptions::default());
        assert_eq!(text.matches("=IFCCOVERING(").count(), 1, "{text}");
        assert!(text.contains("-soffit'"), "{text}");
        assert!(!text.contains("-fascia'"), "{text}");
    }
}
