//! Foundations and interior soffits (phase 16).

use super::Ctx;
use crate::step::P;

/// Per foundation with stem meshes one `IfcWall` (key `foundation/<n>`, name `foundation-fd<n>`,
/// `.STANDARD.`), per foundation with footing meshes one `IfcFooting` (key `footing/<n>`, name
/// `footing-fd<n>`, `.STRIP_FOOTING.`), each contained in its level's storey; then per soffit one
/// `IfcCovering` (`.CEILING.`, key `soffit-box/<n>`, name `soffit-sf<n>`) in its level's storey.
/// Bodies come from `ctx.pieces`. In foundation then soffit order (model order).
pub(super) fn emit(ctx: &mut Ctx) {
    for f in &ctx.model.foundations {
        let n = f.id.0;
        let slot = ctx.storey_for(f.level);
        let stems = ctx.pieces.get("foundation", f.id).to_vec();
        if !stems.is_empty() {
            ctx.element("IfcWall", &format!("foundation/{n}"), &format!("foundation-fd{n}"), slot, &stems, vec![P::Null, P::Enum("STANDARD")]);
        }
        let footings = ctx.pieces.get("footing", f.id).to_vec();
        if !footings.is_empty() {
            ctx.element("IfcFooting", &format!("footing/{n}"), &format!("footing-fd{n}"), slot, &footings, vec![P::Null, P::Enum("STRIP_FOOTING")]);
        }
    }
    for s in &ctx.model.soffits {
        let n = s.id.0;
        let meshes = ctx.pieces.get("isoffit", s.id).to_vec();
        if meshes.is_empty() {
            continue;
        }
        let slot = ctx.storey_for(s.level);
        ctx.element("IfcCovering", &format!("soffit-box/{n}"), &format!("soffit-sf{n}"), slot, &meshes, vec![P::Null, P::Enum("CEILING")]);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{IfcOptions, write_model};
    use cadcraft_arch3d::{Mesh, Scene3d};
    use cadcraft_doc::arch::{ArchId, ArchModel, Foundation, Level, Room, Soffit};
    use cadcraft_geom::Vec2;

    fn square(material: &str, object: u32) -> Mesh {
        Mesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: material.into(),
            object: ArchId(object),
            ..Mesh::default()
        }
    }

    fn model() -> ArchModel {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, ..Default::default() });
        m.push_foundation(Foundation::default());
        m.push_soffit(Soffit {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 1.0), Vec2::new(0.0, 1.0)],
            ..Default::default()
        });
        m.push_room(Room { polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(3.0, 3.0), Vec2::new(0.0, 3.0)], ..Room::default() });
        m
    }

    fn export(m: &ArchModel, meshes: Vec<Mesh>) -> String {
        write_model(m, &Scene3d { meshes, ..Scene3d::default() }, &IfcOptions::default())
    }

    fn line_with<'a>(text: &'a str, needle: &str) -> &'a str {
        text.lines().find(|l| l.contains(needle)).unwrap_or_default()
    }

    #[test]
    fn foundation_writes_ifcwall_and_ifcfooting() {
        let m = model();
        let text = export(&m, vec![square("foundation", 0), square("foundation", 0), square("footing", 0)]);
        let wall = line_with(&text, "'foundation-fd0'");
        assert!(wall.contains("=IFCWALL("), "{text}");
        assert!(wall.ends_with(",$,.STANDARD.);"), "{wall}");
        let footing = line_with(&text, "'footing-fd0'");
        assert!(footing.contains("=IFCFOOTING("), "{text}");
        assert!(footing.ends_with(",$,.STRIP_FOOTING.);"), "{footing}");
        assert_eq!(text.matches("'foundation-fd0'").count(), 1, "one wall per foundation");
    }

    #[test]
    fn soffit_is_an_ifccovering_ceiling() {
        let m = model();
        let text = export(&m, vec![square("soffit-box", 0), square("soffit-box:paint", 0)]);
        let line = line_with(&text, "'soffit-sf0'");
        assert!(line.contains("=IFCCOVERING("), "{text}");
        assert!(line.ends_with(",$,.CEILING.);"), "{line}");
        assert_eq!(text.matches("'soffit-sf0'").count(), 1);
    }

    #[test]
    fn slab_rooms_are_baseslabs() {
        let m = model();
        let text = export(&m, vec![square("slab", 0)]);
        let line = line_with(&text, "'slab-r0'");
        assert!(line.contains("=IFCSLAB("), "{text}");
        assert!(line.ends_with(",$,.BASESLAB.);"), "{line}");
    }

    #[test]
    fn stack_layers_join_the_ceiling_covering() {
        let m = model();
        let text = export(&m, vec![square("ceiling", 0), square("ceiling-layer", 0)]);
        assert_eq!(text.matches("'ceiling-r0'").count(), 1, "{text}");
        assert!(!text.contains("ceiling-r0-layer"), "{text}");
        // One face set holding both meshes' eight points.
        assert!(text.contains("IFCTRIANGULATEDFACESET(#"), "{text}");
        let list = line_with(&text, "IFCCARTESIANPOINTLIST3D");
        assert_eq!(list.matches("),(").count(), 7, "{list}");
    }

    #[test]
    fn every_new_product_has_a_body() {
        let m = model();
        let meshes = vec![square("foundation", 0), square("footing", 0), square("soffit-box", 0), square("slab", 0), square("ceiling-layer", 0)];
        let text = export(&m, meshes);
        for name in ["foundation-fd0", "footing-fd0", "soffit-sf0", "slab-r0", "ceiling-r0"] {
            let line = line_with(&text, &format!("'{name}'"));
            assert!(!line.is_empty(), "{name}: {text}");
            assert!(!line.contains(",$,$,$,$,#"), "{name} has a placement but check its body: {line}");
            // Name, $, $, placement, body: the body is a reference, not $.
            let after = line.split_once(&format!("'{name}'")).map_or("", |(_, r)| r);
            let parts: Vec<&str> = after.split(',').collect();
            assert!(parts.get(3).is_some_and(|b| b.starts_with('#')), "{name}: {line}");
        }
    }
}
