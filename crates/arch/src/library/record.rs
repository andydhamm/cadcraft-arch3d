//! Library item records and categories.

use serde::{Deserialize, Serialize};

/// The top-level categories of library items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Furniture,
    AppliancesAndPlumbing,
    CabinetParts,
    LightingAndElectrical,
    Materials,
    DoorsAndWindows,
    Plants,
    Backdrops,
    User,
}

impl Category {
    /// All categories in display order.
    pub const ALL: [Category; 9] = [
        Category::Furniture,
        Category::AppliancesAndPlumbing,
        Category::CabinetParts,
        Category::LightingAndElectrical,
        Category::Materials,
        Category::DoorsAndWindows,
        Category::Plants,
        Category::Backdrops,
        Category::User,
    ];

    /// The display name of the category.
    pub fn name(self) -> &'static str {
        match self {
            Category::Furniture => "Furniture",
            Category::AppliancesAndPlumbing => "Appliances and plumbing fixtures",
            Category::CabinetParts => "Cabinet parts",
            Category::LightingAndElectrical => "Lighting and electrical",
            Category::Materials => "Materials",
            Category::DoorsAndWindows => "Doors and windows",
            Category::Plants => "Plants",
            Category::Backdrops => "Backdrops",
            Category::User => "User catalog",
        }
    }

    /// Parse a category from its display name.
    pub fn parse(s: &str) -> Option<Category> {
        match s {
            "Furniture" => Some(Category::Furniture),
            "Appliances and plumbing fixtures" => Some(Category::AppliancesAndPlumbing),
            "Cabinet parts" => Some(Category::CabinetParts),
            "Lighting and electrical" => Some(Category::LightingAndElectrical),
            "Materials" => Some(Category::Materials),
            "Doors and windows" => Some(Category::DoorsAndWindows),
            "Plants" => Some(Category::Plants),
            "Backdrops" => Some(Category::Backdrops),
            "User catalog" => Some(Category::User),
            _ => None,
        }
    }
}

/// The kind of item in the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    #[default]
    Symbol,
    Material,
    Backdrop,
    Door,
    Window,
    Plant,
    LightFixture,
    Finish,
    Cabinet,
    Fixture,
    FrontStyle,
    Group,
    /// A molding profile (`profile` holds the closed outline).
    Molding,
}

/// A single item in the library.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemRecord {
    pub id: String,
    pub name: String,
    pub category: String,
    pub folder: String,
    pub kind: ItemKind,
    pub symbol_type: String,
    pub placement: Option<String>,
    pub size: Option<[f64; 3]>,
    pub keywords: Vec<String>,
    pub tags: Vec<String>,
    pub style: Option<String>,
    pub source: String,
    pub licence: String,
    pub author: String,
    pub attribution: String,
    pub catalog: String,
    pub read_only: bool,
    pub file: Option<String>,
    pub elevation: Option<f64>,
    pub cutout: Option<[f64; 2]>,
    pub notes: String,
    /// The closed 2D outline of a molding item (metres, x out, y up).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<Vec<[f64; 2]>>,
}

/// Determine the folder for a given category and symbol type.
pub fn folder_for(category: &str, symbol_type: &str) -> &'static str {
    match symbol_type {
        "chair" | "stool" => "Chairs and stools",
        "sofa" => "Sofas",
        "bed" => "Beds",
        "table" => "Tables",
        "desk" => "Desks",
        "storage" => "Storage",
        "rug" => "Rugs",
        "décor" => "Décor",
        "sink" | "lavatory" | "faucet" => "Sinks and faucets",
        "toilet" | "bathtub" | "shower" => "Bath",
        "range" | "cooktop" | "oven" | "hood" => "Cooking",
        "refrigerator" => "Refrigerators",
        "dishwasher" | "washer" => "Laundry and dishwashers",
        "light" => "Lights",
        "pull" => "Pulls",
        "plant" => "Plants",
        "material" => "Images",
        "backdrop" => "Backdrops",
        _ => "Other",
    }
}

/// Determine the layer for a given category and symbol type.
pub fn layer_for(category: &str, symbol_type: &str) -> &'static str {
    match category {
        "Furniture" => "I-FURN",
        "Appliances and plumbing fixtures" => match symbol_type {
            "sink" | "lavatory" | "faucet" | "toilet" | "bathtub" | "shower" => "P-FLOR-FIXT",
            _ => "A-FLOR-APPL",
        },
        "Lighting and electrical" => "E-LITE",
        "Plants" => "L-PLNT",
        _ => "I-FURN",
    }
}

/// Determine the IFC class for a given category and symbol type.
pub fn ifc_class_for(category: &str, symbol_type: &str) -> &'static str {
    match category {
        "Appliances and plumbing fixtures" => match symbol_type {
            "sink" | "lavatory" | "faucet" | "toilet" | "bathtub" | "shower" => "IfcSanitaryTerminal",
            _ => "IfcElectricAppliance",
        },
        "Lighting and electrical" => "IfcLightFixture",
        _ => "IfcFurniture",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_names_and_order() {
        assert_eq!(Category::ALL.len(), 9);
        assert_eq!(Category::Furniture.name(), "Furniture");
        assert_eq!(Category::AppliancesAndPlumbing.name(), "Appliances and plumbing fixtures");
        assert_eq!(Category::CabinetParts.name(), "Cabinet parts");
        assert_eq!(Category::LightingAndElectrical.name(), "Lighting and electrical");
        assert_eq!(Category::Materials.name(), "Materials");
        assert_eq!(Category::DoorsAndWindows.name(), "Doors and windows");
        assert_eq!(Category::Plants.name(), "Plants");
        assert_eq!(Category::Backdrops.name(), "Backdrops");
        assert_eq!(Category::User.name(), "User catalog");

        assert_eq!(Category::parse("Furniture"), Some(Category::Furniture));
        assert_eq!(Category::parse("User catalog"), Some(Category::User));
        assert_eq!(Category::parse("Unknown"), None);
    }

    #[test]
    fn folder_for_known_and_unknown_types() {
        assert_eq!(folder_for("Furniture", "chair"), "Chairs and stools");
        assert_eq!(folder_for("Furniture", "sofa"), "Sofas");
        assert_eq!(folder_for("Appliances and plumbing fixtures", "sink"), "Sinks and faucets");
        assert_eq!(folder_for("Appliances and plumbing fixtures", "toilet"), "Bath");
        assert_eq!(folder_for("Lighting and electrical", "light"), "Lights");
        assert_eq!(folder_for("Plants", "plant"), "Plants");
        assert_eq!(folder_for("Materials", "material"), "Images");
        assert_eq!(folder_for("Backdrops", "backdrop"), "Backdrops");
        assert_eq!(folder_for("Furniture", "unknown"), "Other");
    }

    #[test]
    fn layer_for_categories() {
        assert_eq!(layer_for("Furniture", "chair"), "I-FURN");
        assert_eq!(layer_for("Appliances and plumbing fixtures", "sink"), "P-FLOR-FIXT");
        assert_eq!(layer_for("Appliances and plumbing fixtures", "range"), "A-FLOR-APPL");
        assert_eq!(layer_for("Lighting and electrical", "light"), "E-LITE");
        assert_eq!(layer_for("Plants", "plant"), "L-PLNT");
        assert_eq!(layer_for("Materials", "material"), "I-FURN");
    }

    #[test]
    fn ifc_class_for_categories() {
        assert_eq!(ifc_class_for("Furniture", "chair"), "IfcFurniture");
        assert_eq!(ifc_class_for("Appliances and plumbing fixtures", "sink"), "IfcSanitaryTerminal");
        assert_eq!(ifc_class_for("Appliances and plumbing fixtures", "range"), "IfcElectricAppliance");
        assert_eq!(ifc_class_for("Lighting and electrical", "light"), "IfcLightFixture");
    }
}
