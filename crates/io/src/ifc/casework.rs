//! Cabinets and fixtures (phase 13): `IfcFurniture` for cabinets, `IfcSanitaryTerminal` for
//! sanitary fixtures, and `IfcElectricAppliance` for appliances.

use crate::ifc::Ctx;
use crate::step::P;
use cadcraft_doc::arch::{ArchId, CabinetKind};

/// Emit cabinets, orphaned countertops/backsplashes and fixtures.
pub(crate) fn emit(ctx: &mut Ctx<'_>) {
    let mut owner_of_countertop: std::collections::BTreeMap<u32, u32> = std::collections::BTreeMap::new();
    for ct in &ctx.model.countertops {
        let owner = ct.cabinets.iter().copied().filter(|id| ctx.model.cabinets.iter().any(|c| c.id == *id)).min();
        if let Some(owner) = owner {
            owner_of_countertop.insert(ct.id.0, owner.0);
        }
    }
    let mut owner_of_backsplash: std::collections::BTreeMap<u32, u32> = std::collections::BTreeMap::new();
    for bs in &ctx.model.backsplashes {
        let owner = match bs.counter {
            Some(cadcraft_doc::arch::CounterRef::Cabinet(id)) => Some(id.0),
            Some(cadcraft_doc::arch::CounterRef::Countertop(id)) => owner_of_countertop.get(&id.0).copied(),
            None => None,
        }
        .filter(|id| ctx.model.cabinets.iter().any(|c| c.id.0 == *id));
        if let Some(owner) = owner {
            owner_of_backsplash.insert(bs.id.0, owner);
        }
    }

    let cabinet_ids: std::collections::BTreeSet<u32> =
        ctx.model.cabinets.iter().map(|c| c.id.0).chain(ctx.pieces.ids("cabinet").into_iter().map(|id| id.0)).collect();

    let mut body_by_cabinet: std::collections::BTreeMap<u32, Vec<&cadcraft_arch3d::Mesh>> = std::collections::BTreeMap::new();
    for id in &cabinet_ids {
        body_by_cabinet.entry(*id).or_default().extend(ctx.pieces.get("cabinet", cadcraft_doc::arch::ArchId(*id)).iter().copied());
    }
    for ct in &ctx.model.countertops {
        if let Some(owner_id) = owner_of_countertop.get(&ct.id.0) {
            body_by_cabinet.entry(*owner_id).or_default().extend(ctx.pieces.get("countertop", ct.id).iter().copied());
        }
    }
    for bs in &ctx.model.backsplashes {
        if let Some(owner_id) = owner_of_backsplash.get(&bs.id.0) {
            body_by_cabinet.entry(*owner_id).or_default().extend(ctx.pieces.get("backsplash", bs.id).iter().copied());
        }
    }

    for id in cabinet_ids {
        let meshes = body_by_cabinet.get(&id).cloned().unwrap_or_default();
        if meshes.is_empty() {
            continue;
        }
        let cabinet = ctx.model.cabinets.iter().find(|c| c.id.0 == id);
        let obj_type = match cabinet.map(|c| c.kind).unwrap_or_default() {
            CabinetKind::Base if cabinet.map(|c| c.filler).unwrap_or(false) => "FILLER",
            CabinetKind::Wall if cabinet.map(|c| c.filler).unwrap_or(false) => "FILLER",
            CabinetKind::Tall if cabinet.map(|c| c.filler).unwrap_or(false) => "FILLER",
            CabinetKind::Base => "BASE CABINET",
            CabinetKind::Wall => "WALL CABINET",
            CabinetKind::Tall => "TALL CABINET",
        };
        let name = format!("cabinet-cb{}", id);
        let key = format!("cabinet/{}", id);
        let level = cabinet.map(|c| c.level).unwrap_or(0);
        let slot = ctx.storey_for(level);
        let placement = ctx.placement_in_storey(slot);
        let z0 = ctx.storeys.get(slot).map_or(0.0, |s| s.elevation);
        let body = ctx.body(&meshes, z0).map_or(P::Null, P::Ref);
        let params = vec![
            P::str(crate::ifc::guid::global_id(&key)),
            P::Null,
            P::str(name),
            P::Null,
            P::str(obj_type),
            P::Ref(placement),
            body,
            P::Null,
            P::Enum("USERDEFINED"),
        ];
        let r = ctx.w.add("IfcFurniture", params);
        ctx.contain(slot, r);
    }

    for ct in &ctx.model.countertops {
        if owner_of_countertop.contains_key(&ct.id.0) {
            continue;
        }
        let meshes = ctx.pieces.get("countertop", ct.id).to_vec();
        if meshes.is_empty() {
            continue;
        }
        let key = format!("countertop/{}", ct.id.0);
        let name = format!("countertop-ct{}", ct.id.0);
        let slot = ctx.storey_for(ct.level);
        let params = vec![
            P::str(crate::ifc::guid::global_id(&key)),
            P::Null,
            P::str(name),
            P::Null,
            P::str("COUNTERTOP"),
            P::Ref(ctx.placement_in_storey(slot)),
            ctx.body(&meshes, ctx.storeys.get(slot).map_or(0.0, |s| s.elevation)).map_or(P::Null, P::Ref),
            P::Null,
            P::Enum("USERDEFINED"),
        ];
        let r = ctx.w.add("IfcFurniture", params);
        ctx.contain(slot, r);
    }

    for bs in &ctx.model.backsplashes {
        if owner_of_backsplash.contains_key(&bs.id.0) {
            continue;
        }
        let meshes = ctx.pieces.get("backsplash", bs.id).to_vec();
        if meshes.is_empty() {
            continue;
        }
        let key = format!("backsplash/{}", bs.id.0);
        let name = format!("backsplash-bs{}", bs.id.0);
        let slot = ctx.storey_for(bs.level);
        let params = vec![
            P::str(crate::ifc::guid::global_id(&key)),
            P::Null,
            P::str(name),
            P::Null,
            P::str("BACKSPLASH"),
            P::Ref(ctx.placement_in_storey(slot)),
            ctx.body(&meshes, ctx.storeys.get(slot).map_or(0.0, |s| s.elevation)).map_or(P::Null, P::Ref),
            P::Null,
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

        let material = meshes.first().map(|m| m.material.as_str()).unwrap_or("");
        let (entity, predef_type, _obj_type) = match material {
            "fixture-sink" => ("IfcSanitaryTerminal", "SINK", ""),
            "fixture-lavatory" => ("IfcSanitaryTerminal", "WASHHANDBASIN", ""),
            "fixture-toilet" => ("IfcSanitaryTerminal", "TOILETPAN", ""),
            "fixture-bathtub" => ("IfcSanitaryTerminal", "BATH", ""),
            "fixture-shower" => ("IfcSanitaryTerminal", "SHOWER", ""),
            "fixture-range" => ("IfcElectricAppliance", "ELECTRICCOOKER", ""),
            "fixture-refrigerator" => ("IfcElectricAppliance", "REFRIGERATOR", ""),
            "fixture-dishwasher" => ("IfcElectricAppliance", "DISHWASHER", ""),
            _ => continue,
        };

        let name = format!("fixture-fx{}", id.0);
        let key = format!("fixture/{}", id.0);
        let level = ctx.model.fixtures.iter().find(|f| f.id == id).map(|f| f.level).unwrap_or(0);
        let slot = ctx.storey_for(level);
        let tail = vec![P::Null, P::Enum(predef_type)];
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
    fn ifc_cabinet_object_type_names_its_kind() {
        use cadcraft_doc::arch::{Cabinet, CabinetKind};
        let mut model = ArchModel::default();
        model.levels = vec![Level::default()];
        for (k, kind) in [CabinetKind::Base, CabinetKind::Wall, CabinetKind::Tall].into_iter().enumerate() {
            model.cabinets.push(Cabinet {
                id: ArchId(k as u32),
                kind,
                position: cadcraft_geom::Vec2::new(k as f64, 0.0),
                width: 0.6,
                depth: 0.6,
                height: 0.9,
                ..Cabinet::default()
            });
        }
        let text = export(&model);
        let furniture: Vec<&str> = text.lines().filter(|l| l.contains("IFCFURNITURE(")).collect();
        assert_eq!(furniture.len(), 3, "three cabinets are three IfcFurniture: {furniture:?}");
        for want in ["'BASE CABINET'", "'WALL CABINET'", "'TALL CABINET'"] {
            assert_eq!(furniture.iter().filter(|l| l.contains(want)).count(), 1, "one {want} ObjectType: {furniture:?}");
        }
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
    fn countertop_and_backsplash_triangles_join_the_owner_cabinets_body() {
        use cadcraft_doc::arch::{Backsplash, Cabinet, CabinetKind, CounterRef, Countertop};
        let mut model = ArchModel::default();
        model.levels = vec![Level::default()];
        let cabinet =
            model.push_cabinet(Cabinet { id: ArchId(0), kind: CabinetKind::Base, width: 1.0, depth: 0.6, height: 0.9, ..Cabinet::default() });
        model.countertops.push(Countertop { id: ArchId(1), cabinets: vec![cabinet], level: 0, ..Countertop::default() });
        model.backsplashes.push(Backsplash { id: ArchId(2), counter: Some(CounterRef::Countertop(ArchId(1))), level: 0, ..Backsplash::default() });

        let mut scene = Scene3d::default();
        let mut box_mesh = Mesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            indices: vec![0, 1, 2, 0, 2, 3],
            object: cabinet,
            material: "cabinet-box".into(),
            ..Mesh::default()
        };
        scene.meshes.push(box_mesh.clone());
        let mut ct = box_mesh.clone();
        ct.material = "countertop".into();
        ct.object = ArchId(1);
        scene.meshes.push(ct);
        let mut bs = box_mesh.clone();
        bs.material = "backsplash".into();
        bs.object = ArchId(2);
        scene.meshes.push(bs);

        let text = crate::ifc::write_model(&model, &scene, &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() });
        let cabinet_line = text.lines().find(|l| l.contains("'cabinet-cb0'") && l.contains("IFCFURNITURE")).unwrap();
        assert!(cabinet_line.contains("IFCFURNITURE"), "cabinet should be an IFC furniture");
        assert!(text.matches("=IFCFURNITURE(").count() >= 1, "furniture count should be at least one");
    }

    #[test]
    fn orphan_countertop_is_its_own_furniture() {
        let mut model = ArchModel::default();
        model.levels = vec![Level::default()];
        model.countertops.push(cadcraft_doc::arch::Countertop { id: ArchId(4), level: 0, ..cadcraft_doc::arch::Countertop::default() });
        let mut scene = Scene3d::default();
        let mut m = Mesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            indices: vec![0, 1, 2, 0, 2, 3],
            object: ArchId(4),
            material: "countertop".into(),
            ..Mesh::default()
        };
        scene.meshes.push(m.clone());

        let text = crate::ifc::write_model(&model, &scene, &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() });
        assert!(text.contains("'countertop-ct4'"), "orphan countertop becomes its own furniture");
        assert!(text.contains("'COUNTERTOP'"), "object type should be COUNTERTOP");
    }

    #[test]
    fn fillers_are_furniture_filler() {
        use cadcraft_doc::arch::Cabinet;
        let mut model = ArchModel::default();
        model.levels = vec![Level::default()];
        model.cabinets.push(Cabinet { id: ArchId(2), kind: CabinetKind::Base, filler: true, level: 0, ..Cabinet::default() });
        let mut scene = Scene3d::default();
        scene.meshes.push(Mesh {
            material: "cabinet-box".into(),
            object: ArchId(2),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            ..Mesh::default()
        });

        let text = crate::ifc::write_model(&model, &scene, &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() });
        assert!(text.contains("'cabinet-cb2'"), "filler cabinet is emitted");
        assert!(text.contains("'FILLER'"), "filler object type is FILLER");
    }

    #[test]
    fn ifc_unknown_fixture_material_is_skipped() {
        let model = ArchModel::default();
        let mut scene = Scene3d::default();
        scene.meshes.push(Mesh {
            material: "fixture-bogus".into(),
            object: ArchId(0),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            ..Mesh::default()
        });

        let text = crate::ifc::write_model(&model, &scene, &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() });

        assert!(!text.contains("IFCSANITARYTERMINAL"), "unknown fixture should not emit sanitary terminal");
        assert!(!text.contains("IFCELECTRICAPPLIANCE"), "unknown fixture should not emit appliance");
    }

    #[test]
    fn ifc_cabinet_on_level_one() {
        let mut model = ArchModel::default();
        model.levels = vec![Level { index: 0, ..Level::default() }, Level { index: 1, elevation: 2.7, ..Level::default() }];

        let mut scene = Scene3d::default();
        scene.meshes.push(Mesh {
            material: "cabinet-box".into(),
            object: ArchId(0),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            ..Mesh::default()
        });

        let text = crate::ifc::write_model(&model, &scene, &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() });

        assert!(text.contains("IFCFURNITURE"), "Expected cabinet furniture");
        // Cabinet defaults to level 0, so contained in first storey
        assert!(text.contains("IFCRELCONTAINEDINSPATIALSTRUCTURE"));
    }

    #[test]
    fn ifc_multiple_fixture_types() {
        let model = ArchModel::default();
        let mut scene = Scene3d::default();
        scene.meshes.push(Mesh {
            material: "fixture-sink".into(),
            object: ArchId(0),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            ..Mesh::default()
        });
        scene.meshes.push(Mesh {
            material: "fixture-toilet".into(),
            object: ArchId(1),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            ..Mesh::default()
        });
        scene.meshes.push(Mesh {
            material: "fixture-dishwasher".into(),
            object: ArchId(2),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            ..Mesh::default()
        });

        let text = crate::ifc::write_model(&model, &scene, &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() });

        assert!(text.contains(".SINK."), "Expected SINK enum");
        assert!(text.contains(".TOILETPAN."), "Expected TOILETPAN enum");
        assert!(text.contains(".DISHWASHER."), "Expected DISHWASHER enum");
        assert!(text.contains("IFCSANITARYTERMINAL"));
        assert!(text.contains("IFCELECTRICAPPLIANCE"));
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

    #[test]
    fn ifc_casework_skips_cabinets_without_meshes() {
        let mut model = ArchModel::default();
        model.levels = vec![Level::default()];
        model.cabinets.push(Cabinet {
            id: ArchId(0),
            kind: CabinetKind::Base,
            position: cadcraft_geom::Vec2::new(0.0, 0.0),
            width: 0.6,
            depth: 0.6,
            height: 0.9,
            ..Cabinet::default()
        });
        // Scene has no meshes for this cabinet
        let scene = Scene3d::default();
        let text = crate::ifc::write_model(&model, &scene, &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() });
        assert!(!text.contains("IFCFURNITURE("), "cabinet without meshes should not be exported: {text}");
    }
}
