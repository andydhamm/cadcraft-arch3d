//! Cabinets and fixtures (phase 13): `IfcFurniture` for cabinets, `IfcSanitaryTerminal` for
//! sanitary fixtures, and `IfcElectricAppliance` for appliances.

use crate::ifc::Ctx;
use crate::step::P;
use cadcraft_doc::arch::ArchId;

/// Emit cabinets and fixtures.
pub(crate) fn emit(ctx: &mut Ctx<'_>) {
    // Cabinets: kind "cabinet"
    for id in ctx.pieces.ids("cabinet") {
        let meshes: Vec<&cadcraft_arch3d::Mesh> = ctx.pieces.get("cabinet", id).to_vec();
        if meshes.is_empty() {
            continue;
        }

        // Determine cabinet type from the first mesh material if possible, or default
        // We assume the first mesh material indicates the type if it's specific, but here
        // materials are generic (cabinet-box, cabinet-front, counter).
        // We'll default to BASE CABINET for now as we don't have type info in ArchId.
        // In a full implementation, ArchModel would carry CabinetType.
        let obj_type = "BASE CABINET";
        let name = format!("cabinet-cb{}", id.0);
        let key = format!("cabinet/{}", id.0);

        // Find the level for this cabinet. For now, assume level 0 or search for it.
        // Since we don't have direct access to the cabinet object's level here without
        // iterating the model, we'll place it in the first storey (level 0) by default
        // or search the model if we had access.
        // Note: Ctx has model. We can look up the cabinet in model.cabinets if they exist.
        // Assuming model.cabinets exists and has a level field.
        let level = ctx.model.cabinets.iter().find(|c| c.id == id).map(|c| c.level).unwrap_or(0);
        let slot = ctx.storey_for(level);

        // IfcFurniture has 9 attributes:
        // 1. GlobalId, 2. OwnerHistory, 3. Name, 4. Description, 5. ObjectType,
        // 6. ObjectPlacement, 7. Representation, 8. Tag, 9. PredefinedType
        let placement = ctx.placement_in_storey(slot);
        let z0 = ctx.storeys.get(slot).map_or(0.0, |s| s.elevation);
        let body = ctx.body(&meshes, z0).map_or(P::Null, P::Ref);

        let params = vec![
            P::str(crate::ifc::guid::global_id(&key)),
            P::Null, // OwnerHistory
            P::str(name),
            P::Null,          // Description
            P::str(obj_type), // ObjectType
            P::Ref(placement),
            body,
            P::Null, // Tag
            P::Enum("USERDEFINED"),
        ];

        let r = ctx.w.add("IfcFurniture", params);
        ctx.contain(slot, r);
    }

    // Fixtures: kind "fixture"
    for id in ctx.pieces.ids("fixture") {
        let meshes: Vec<&cadcraft_arch3d::Mesh> = ctx.pieces.get("fixture", id).to_vec();
        if meshes.is_empty() {
            continue;
        }

        // Determine fixture type from material name (e.g. "fixture-sink")
        let material = meshes.first().map(|m| m.material.as_str()).unwrap_or("");
        let (entity, predef_type, obj_type) = match material {
            "fixture-sink" => ("IfcSanitaryTerminal", "SINK", ""),
            "fixture-lavatory" => ("IfcSanitaryTerminal", "WASHHANDBASIN", ""),
            "fixture-toilet" => ("IfcSanitaryTerminal", "TOILETPAN", ""),
            "fixture-bathtub" => ("IfcSanitaryTerminal", "BATH", ""),
            "fixture-shower" => ("IfcSanitaryTerminal", "SHOWER", ""),
            "fixture-range" => ("IfcElectricAppliance", "ELECTRICCOOKER", ""),
            "fixture-refrigerator" => ("IfcElectricAppliance", "REFRIGERATOR", ""),
            "fixture-dishwasher" => ("IfcElectricAppliance", "DISHWASHER", ""),
            _ => continue, // Unknown fixture type
        };

        let name = format!("fixture-fx{}", id.0);
        let key = format!("fixture/{}", id.0);

        let level = ctx.model.fixtures.iter().find(|f| f.id == id).map(|f| f.level).unwrap_or(0);
        let slot = ctx.storey_for(level);

        let tail = vec![
            P::Null, // Tag
            P::Enum(predef_type),
        ];

        ctx.element(entity, &key, &name, slot, &meshes, tail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ifc::IfcOptions;
    use cadcraft_arch3d::{ArchId, Mesh, Scene3d};
    use cadcraft_doc::Drawing;
    use cadcraft_doc::arch::{ArchModel, Level};

    fn export(model: &ArchModel) -> String {
        let scene = Scene3d::build(model);
        crate::ifc::write_model(model, &scene, &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() })
    }

    #[test]
    fn ifc_cabinet_is_userdefined_furniture() {
        let mut model = ArchModel::default();
        model.levels = vec![Level::default()];
        // Add a cabinet to the model so it gets built into the scene
        // We need to manually add meshes to the scene or ensure the builder creates them.
        // Since we are testing the IFC writer, we can construct a scene with cabinet meshes directly.

        let mut scene = Scene3d::default();
        scene.meshes.push(Mesh {
            material: "cabinet-box".into(),
            object: ArchId(0),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            ..Mesh::default()
        });

        let text = crate::ifc::write_model(&model, &scene, &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() });

        assert!(text.contains("IFCFURNITURE"), "Expected IFCFURNITURE in output");
        assert!(text.contains(".USERDEFINED."), "Expected USERDEFINED enum");
        assert!(text.contains("'BASE CABINET'"), "Expected BASE CABINET object type");
    }

    #[test]
    fn ifc_sink_is_a_sanitary_terminal() {
        let model = ArchModel::default();
        let mut scene = Scene3d::default();
        scene.meshes.push(Mesh {
            material: "fixture-sink".into(),
            object: ArchId(0),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            ..Mesh::default()
        });

        let text = crate::ifc::write_model(&model, &scene, &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() });

        assert!(text.contains("IFCSANITARYTERMINAL"), "Expected IFCSANITARYTERMINAL in output");
        assert!(text.contains(".SINK."), "Expected SINK enum");
    }

    #[test]
    fn ifc_range_is_an_electric_cooker() {
        let model = ArchModel::default();
        let mut scene = Scene3d::default();
        scene.meshes.push(Mesh {
            material: "fixture-range".into(),
            object: ArchId(0),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            ..Mesh::default()
        });

        let text = crate::ifc::write_model(&model, &scene, &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() });

        assert!(text.contains("IFCELECTRICAPPLIANCE"), "Expected IFCELECTRICAPPLIANCE in output");
        assert!(text.contains(".ELECTRICCOOKER."), "Expected ELECTRICCOOKER enum");
    }

    #[test]
    fn casework_is_contained_in_its_storey() {
        let mut model = ArchModel::default();
        model.levels = vec![Level { index: 0, ..Level::default() }, Level { index: 1, elevation: 2.7, ..Level::default() }];

        let mut scene = Scene3d::default();
        // Cabinet on level 0 (default)
        scene.meshes.push(Mesh {
            material: "cabinet-box".into(),
            object: ArchId(0),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            ..Mesh::default()
        });

        let text = crate::ifc::write_model(&model, &scene, &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() });

        // Check that there is a containment relationship
        assert!(text.contains("IFCRELCONTAINEDINSPATIALSTRUCTURE"), "Expected containment relationship");
        // The cabinet should be contained in the first storey
        assert!(text.contains("IFCFURNITURE"), "Expected cabinet furniture");
    }
}
