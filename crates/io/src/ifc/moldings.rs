//! Room moldings and free molding lines as `IfcCovering`s (phase 18).

use super::Ctx;
use crate::step::P;
use std::collections::BTreeMap;

/// One `IfcCovering` per room molding and molding line.
pub(super) fn emit(ctx: &mut Ctx) {
    for room in &ctx.model.rooms {
        let mut by_material: BTreeMap<String, Vec<&cadcraft_arch3d::Mesh>> = BTreeMap::new();
        for mesh in ctx.pieces.get("molding", room.id).iter().copied() {
            let material = mesh.material.as_str();
            let Some(rest) = material.strip_prefix("molding-room-") else { continue };
            let Some((kind, idx)) = rest.rsplit_once('-') else { continue };
            if !matches!(kind, "base" | "crown" | "chair") {
                continue;
            }
            let ok = idx.parse::<u32>().unwrap_or(u32::MAX);
            by_material.entry(format!("molding-room-{}-{}", kind, ok)).or_default().push(mesh);
        }
        for (material, meshes) in by_material {
            if meshes.is_empty() {
                continue;
            }
            let Some((kind, idx)) = material.strip_prefix("molding-room-").and_then(|r| r.rsplit_once('-')) else {
                continue;
            };
            let slot = ctx.storey_for(room.level);
            let predefined = if kind == "base" { "SKIRTINGBOARD" } else { "MOLDING" };
            let key = format!("molding/{}/{}/{}", room.id.0, kind, idx);
            let name = format!("molding-r{}-{}-{}", room.id.0, kind, idx);
            ctx.element("IfcCovering", &key, &name, slot, &meshes, vec![P::Null, P::Enum(predefined)]);
        }
    }

    for line in &ctx.model.molding_lines {
        let meshes = ctx.pieces.get("moldingline", line.id).to_vec();
        if meshes.is_empty() {
            continue;
        }
        let slot = ctx.storey_for(line.level);
        ctx.element(
            "IfcCovering",
            &format!("moldingline/{}", line.id.0),
            &format!("molding-ml{}", line.id.0),
            slot,
            &meshes,
            vec![P::Null, P::Enum("MOLDING")],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::{IfcOptions, write_model};
    use cadcraft_arch3d::{Mesh, Scene3d};
    use cadcraft_doc::arch::{ArchId, ArchModel, Level, MoldingKind, MoldingLine, Room};
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

    #[test]
    fn room_moldings_are_ifccoverings() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, ..Level::default() });
        let room = model.push_room(Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 2.0)],
            ..Room::default()
        });
        let scene = Scene3d {
            meshes: vec![square("molding-room-base-0", room.0), square("molding-room-crown-0", room.0), square("molding-room-chair-1", room.0)],
            ..Scene3d::default()
        };
        let text = write_model(&model, &scene, &IfcOptions::default());
        assert!(text.contains("=IFCCOVERING("), "{text}");
        assert!(text.contains("'molding-r0-base-0'"), "{text}");
        assert!(text.contains("'molding-r0-crown-0'"), "{text}");
        assert!(text.contains(".SKIRTINGBOARD."), "{text}");
        assert!(text.contains(".MOLDING."), "{text}");
    }

    #[test]
    fn molding_lines_are_ifccoverings_molding() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, ..Level::default() });
        let id = model.push_molding_line(MoldingLine {
            id: ArchId(0),
            level: 0,
            points: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0)],
            ..MoldingLine::default()
        });
        let scene = Scene3d { meshes: vec![square("molding-line", id.0)], ..Scene3d::default() };
        let text = write_model(&model, &scene, &IfcOptions::default());
        assert!(text.contains("=IFCCOVERING("), "{text}");
        assert!(text.contains("'molding-ml0'"), "{text}");
        assert!(text.contains(".MOLDING."), "{text}");
    }
}
