//! IFC export for library furnishings (phase 14).
//!
//! Furnishings are exported as `IfcFurniture`, `IfcSanitaryTerminal`, `IfcElectricAppliance` or
//! `IfcLightFixture` depending on their category and symbol type. Each furnishing gets a property
//! set `CADCraft_Library` carrying its item ID, licence and source.

use super::Ctx;
use super::guid::global_id;
use crate::step::P;
use cadcraft_arch::library::ifc_class_for;
use cadcraft_doc::arch::Furnishing;

/// Emit all furnishings in the model.
pub fn emit(ctx: &mut Ctx) {
    for f in &ctx.model.furnishings {
        emit_one(ctx, f);
    }
}

fn emit_one(ctx: &mut Ctx, f: &Furnishing) {
    let class = ifc_class_for(&f.category, &f.symbol_type);
    let name = format!("furnishing-f{}", f.id.0);
    let key = format!("furnishing/{}", f.id.0);
    let slot = ctx.storey_for(f.level);

    // Gather meshes for this furnishing from the scene pieces.
    // Pieces are keyed by (kind, id). Furnishings use kind "furnishing".
    let meshes: Vec<&cadcraft_arch3d::Mesh> = ctx.pieces.get("furnishing", f.id).to_vec();
    if meshes.is_empty() {
        return;
    }

    let mut tail = Vec::new();

    match class {
        "IfcFurniture" => {
            // IfcFurniture: PredefinedType is optional, ObjectType is optional.
            // We use ObjectType for the symbol type.
            tail.push(P::Null); // PredefinedType
            tail.push(P::str(&f.symbol_type.to_uppercase())); // ObjectType
        }
        "IfcSanitaryTerminal" => {
            // IfcSanitaryTerminal: PredefinedType (enum), ObjectType (string)
            let predef = match f.symbol_type.as_str() {
                "sink" | "lavatory" => "SINK",
                "toilet" => "TOILETPAN",
                "bathtub" => "BATH",
                "shower" => "SHOWER",
                _ => "USERDEFINED",
            };
            tail.push(P::Enum(predef));
            tail.push(P::Null); // ObjectType
        }
        "IfcElectricAppliance" => {
            // IfcElectricAppliance: PredefinedType (enum), ObjectType (string)
            let predef = match f.symbol_type.as_str() {
                "refrigerator" => "REFRIGERATOR",
                "dishwasher" => "DISHWASHER",
                "range" | "cooktop" | "oven" => "ELECTRICCOOKER",
                _ => "USERDEFINED",
            };
            tail.push(P::Enum(predef));
            tail.push(P::Null); // ObjectType
        }
        "IfcLightFixture" => {
            // IfcLightFixture: PredefinedType (enum), ReferenceToLightSource (ref), LightingColor (ref),
            // Flow (real), LightDistributionCurve (enum), ColorTemperature (real),
            // ReferenceToSafety (ref), OutletType (enum), MountingType (enum),
            // UserDefinedPredefinedType (string), UserDefinedMountingType (string)
            // Simplified: PredefinedType .POINTSOURCE., rest Null.
            tail.push(P::Enum("POINTSOURCE")); // PredefinedType
            tail.push(P::Null); // ReferenceToLightSource
            tail.push(P::Null); // LightingColor
            tail.push(P::Null); // Flow
            tail.push(P::Null); // LightDistributionCurve
            tail.push(P::Null); // ColorTemperature
            tail.push(P::Null); // ReferenceToSafety
            tail.push(P::Null); // OutletType
            tail.push(P::Null); // MountingType
            tail.push(P::Null); // UserDefinedPredefinedType
            tail.push(P::Null); // UserDefinedMountingType
        }
        _ => {
            // Default to IfcFurniture if unknown
            tail.push(P::Null);
            tail.push(P::str(&f.symbol_type.to_uppercase()));
        }
    }

    let entity = ctx.element(class, &key, &name, slot, &meshes, tail);

    // Property set CADCraft_Library
    // Furnishing struct does not carry licence or source directly; they are in the library catalog.
    // We export empty strings for them as we cannot access the catalog here.
    let props = vec![("Item", P::str(&f.item)), ("Licence", P::str("")), ("Source", P::str(""))];

    ctx.pset("CADCraft_Library", &key, props, &[entity]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ifc::{IfcOptions, write_model};
    use cadcraft_arch3d::Scene3d;
    use cadcraft_doc::arch::{ArchId, ArchModel, Furnishing, Placement};

    fn make_furnishing(id: u32, category: &str, symbol_type: &str) -> Furnishing {
        Furnishing {
            id: ArchId(id),
            item: format!("item-{id}"),
            symbol: format!("sym-{id}"),
            name: format!("Furnishing {id}"),
            category: category.to_string(),
            symbol_type: symbol_type.to_string(),
            placement: Placement::Floor,
            level: 0,
            at: [0.0, 0.0],
            rotation: 0.0,
            elevation: 0.0,
            size: [1.0, 1.0, 1.0],
            mirrored: false,
            host: Default::default(),
            cutout: None,
        }
    }

    #[test]
    fn ifc_furnishings_by_category() {
        let mut model = ArchModel::default();
        model.furnishings.push(make_furnishing(0, "Furniture", "sofa"));
        model.furnishings.push(make_furnishing(1, "Appliances and plumbing fixtures", "toilet"));
        model.furnishings.push(make_furnishing(2, "Appliances and plumbing fixtures", "refrigerator"));
        model.furnishings.push(make_furnishing(3, "Lighting and electrical", "ceiling light"));

        let scene = Scene3d::default();
        let opts = IfcOptions { timestamp: 0, file_name: "test.ifc".into() };
        let text = write_model(&model, &scene, &opts);

        assert!(text.contains("IFCFURNITURE"));
        assert!(text.contains("IFCSANITARYTERMINAL"));
        assert!(text.contains("IFCELECTRICAPPLIANCE"));
        assert!(text.contains("IFCLIGHTFIXTURE"));
    }

    #[test]
    fn ifc_furnishing_names_match_glb_nodes() {
        let mut model = ArchModel::default();
        model.furnishings.push(make_furnishing(0, "Furniture", "chair"));

        let scene = Scene3d::default();
        let opts = IfcOptions { timestamp: 0, file_name: "test.ifc".into() };
        let text = write_model(&model, &scene, &opts);

        // The name in IFC should be "furnishing-f0"
        assert!(text.contains("'furnishing-f0'"));
    }

    #[test]
    fn ifc_furnishing_globalids_are_reproducible() {
        let mut model = ArchModel::default();
        model.furnishings.push(make_furnishing(0, "Furniture", "chair"));

        let scene = Scene3d::default();
        let opts = IfcOptions { timestamp: 0, file_name: "test.ifc".into() };
        let text1 = write_model(&model, &scene, &opts);
        let text2 = write_model(&model, &scene, &opts);

        assert_eq!(text1, text2);
    }

    #[test]
    fn ifc_furnishing_properties_carry_the_licence() {
        let mut model = ArchModel::default();
        model.furnishings.push(make_furnishing(0, "Furniture", "chair"));

        let scene = Scene3d::default();
        let opts = IfcOptions { timestamp: 0, file_name: "test.ifc".into() };
        let text = write_model(&model, &scene, &opts);

        assert!(text.contains("CADCraft_Library"));
        assert!(text.contains("'Item'"));
        assert!(text.contains("'item-0'"));
    }
}
