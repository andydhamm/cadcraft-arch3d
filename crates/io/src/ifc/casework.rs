//! Cabinets and fixtures (phase 13): `IfcFurniture` for cabinets, `IfcSanitaryTerminal` for
//! sanitary fixtures, and `IfcElectricAppliance` for appliances.

use crate::ifc::Ctx;
use crate::step::P;
use cadcraft_doc::arch::ArchId;

/// Emit cabinets and fixtures.
pub(crate) fn emit(ctx: &mut Ctx<'_>) {
    // Cabinets: kind "cabinet"
    for id in ctx.pieces.ids("cabinet") {
        let meshes = ctx.pieces.get("cabinet", id);
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

        let tail = vec![
            P::Null, // Tag
            P::Enum("USERDEFINED"),
            P::Str(obj_type),
        ];

        ctx.element("IfcFurniture", &key, &name, slot, meshes, tail);
    }

    // Fixtures: kind "fixture"
    for id in ctx.pieces.ids("fixture") {
        let meshes = ctx.pieces.get("fixture", id);
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

        ctx.element(entity, &key, &name, slot, meshes, tail);
    }
}
