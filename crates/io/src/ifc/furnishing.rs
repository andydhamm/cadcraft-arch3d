//! Library furnishings (phase 14): one element per furnishing of the model, after the casework.
//!
//! The class follows the item's category and symbol type ([`ifc_class_for`]): `IfcFurniture`
//! (`.USERDEFINED.`, its ObjectType the symbol type upper-cased), `IfcSanitaryTerminal`,
//! `IfcElectricAppliance` (each with the PredefinedType its symbol type names, else `.USERDEFINED.`
//! with the symbol type as ObjectType) or `IfcLightFixture` (`.POINTSOURCE.`). Name is the GLB node
//! name `furnishing-f<n>`, Description the item's name, the GlobalId comes from `furnishing/<n>`, the
//! element is contained in its level's storey, and its body is one face set of every mesh of the
//! furnishing (its `sym:` parts and its painted parts, [`super::geometry::kind_of`]). A property set
//! `CADCraft_Library` carries `Item` (the library item's id), `Licence` and `Source`.
//!
//! The export sees the drawing, not the library: an item's licence and source come from the sample
//! set embedded in the binary ([`crate::library::sample_files`]); an item that is not one of its
//! items (an installed, local or user item) keeps both properties without a value rather than an
//! invented one.

use std::collections::{BTreeMap, BTreeSet};

use super::Ctx;
use crate::step::P;
use cadcraft_arch::library::{ItemRecord, ifc_class_for};
use cadcraft_doc::arch::{Furnishing, furnishing_label};

/// The ObjectType of a user-defined furnishing whose symbol type is empty (IFC wants one).
const UNNAMED_TYPE: &str = "FURNISHING";

/// One element (and its property set) per furnishing, in the model's order; a repeated id is
/// written once, so no GlobalId repeats.
pub(super) fn emit(ctx: &mut Ctx) {
    let model = ctx.model;
    if model.furnishings.is_empty() {
        return;
    }
    let records = sample_records();
    let mut written = BTreeSet::new();
    for f in &model.furnishings {
        if written.insert(f.id.0) {
            emit_one(ctx, f, records.get(f.item.as_str()));
        }
    }
}

fn emit_one(ctx: &mut Ctx, f: &Furnishing, record: Option<&ItemRecord>) {
    let class = ifc_class_for(&f.category, &f.symbol_type);
    let predefined = predefined_type(class, &f.symbol_type);
    let key = format!("furnishing/{}", f.id.0);
    let name = format!("furnishing-{}", furnishing_label(f.id));
    // IFC's CorrectPredefinedType rule: a user-defined type is named by ObjectType.
    let object_type = if predefined == "USERDEFINED" {
        let t = f.symbol_type.trim().to_uppercase();
        P::str(if t.is_empty() { UNNAMED_TYPE.to_string() } else { t })
    } else {
        P::Null
    };
    let description = if f.name.trim().is_empty() { P::Null } else { P::str(f.name.clone()) };
    let slot = ctx.storey_for(f.level);
    let meshes = ctx.pieces.get("furnishing", f.id).to_vec();
    let element = ctx.element_described(class, &key, [P::str(name), description, object_type], slot, &meshes, vec![P::Null, P::Enum(predefined)]);
    let text = |kind: &'static str, value: Option<&str>| value.filter(|v| !v.trim().is_empty()).map_or(P::Null, |v| P::typed(kind, P::str(v)));
    let props = vec![
        ("Item", P::typed("IFCIDENTIFIER", P::str(f.item.clone()))),
        ("Licence", text("IFCLABEL", record.map(|r| r.licence.as_str()))),
        ("Source", text("IFCTEXT", record.map(|r| r.source.as_str()))),
    ];
    ctx.pset("CADCraft_Library", &key, props, &[element]);
}

/// The PredefinedType of a furnishing of IFC class `class` (from [`ifc_class_for`]) and symbol
/// type `symbol_type`.
fn predefined_type(class: &str, symbol_type: &str) -> &'static str {
    match (class, symbol_type) {
        ("IfcSanitaryTerminal", "sink" | "lavatory") => "SINK",
        ("IfcSanitaryTerminal", "toilet") => "TOILETPAN",
        ("IfcSanitaryTerminal", "bathtub") => "BATH",
        ("IfcSanitaryTerminal", "shower") => "SHOWER",
        ("IfcElectricAppliance", "refrigerator") => "REFRIGERATOR",
        ("IfcElectricAppliance", "dishwasher") => "DISHWASHER",
        ("IfcElectricAppliance", "range" | "cooktop" | "oven") => "ELECTRICCOOKER",
        ("IfcLightFixture", _) => "POINTSOURCE",
        _ => "USERDEFINED",
    }
}

/// The embedded sample catalog's items by id; an entry that does not read as a record is left out.
fn sample_records() -> BTreeMap<String, ItemRecord> {
    let catalog: Option<serde_json::Value> =
        crate::library::sample_files().iter().find(|(name, _)| *name == "catalog.json").and_then(|(_, bytes)| serde_json::from_slice(bytes).ok());
    let items = catalog.as_ref().and_then(|c| c.get("items")).and_then(serde_json::Value::as_array).map_or(&[][..], Vec::as_slice);
    items.iter().filter_map(|v| serde_json::from_value::<ItemRecord>(v.clone()).ok()).map(|r| (r.id.clone(), r)).collect()
}

#[cfg(test)]
mod tests {
    use crate::ifc::guid::global_id;
    use crate::ifc::{IfcOptions, write_model};
    use crate::step::encode_string;
    use cadcraft_arch3d::Scene3d;
    use cadcraft_doc::arch::{ArchId, ArchModel, Furnishing, Level, SymbolMesh, SymbolPart};
    use std::collections::{BTreeMap, BTreeSet};

    const KEY: &str = "00112233aabbccdd";
    const CLASSES: [&str; 4] = ["IFCFURNITURE", "IFCSANITARYTERMINAL", "IFCELECTRICAPPLIANCE", "IFCLIGHTFIXTURE"];
    const SOFA: (&str, &str, &str) = ("sofa-futon", "Furniture", "sofa");
    const TOILET: (&str, &str, &str) = ("toilet", "Appliances and plumbing fixtures", "toilet");
    const FRIDGE: (&str, &str, &str) = ("refrigerator-small", "Appliances and plumbing fixtures", "refrigerator");
    const LIGHT: (&str, &str, &str) = ("light-ceiling", "Lighting and electrical", "light");

    /// A triangle standing at height `z`: one symbol part.
    fn part(z: f32) -> SymbolPart {
        SymbolPart {
            positions: vec![[-0.5, -0.5, z], [0.5, -0.5, z], [0.0, 0.5, z + 0.5]],
            normals: vec![[0.0, -1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
            ..Default::default()
        }
    }

    /// Two levels (0 at 0 m, 1 at 3 m), a two-part symbol, and one furnishing of it per (item,
    /// category, symbol type) in `items`: ids 0.., named `Item <n>`, on level 0, 2 m apart.
    fn model(items: &[(&str, &str, &str)]) -> ArchModel {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, name: "Ground".into(), ..Level::default() }, Level { index: 1, elevation: 3.0, ..Level::default() }];
        m.symbols.insert(KEY.into(), SymbolMesh { key: KEY.into(), parts: vec![part(0.0), part(0.5)], ..Default::default() });
        for (k, (item, category, symbol_type)) in items.iter().enumerate() {
            m.furnishings.push(Furnishing {
                id: ArchId(k as u32),
                item: (*item).into(),
                symbol: KEY.into(),
                name: format!("Item {k}"),
                category: (*category).into(),
                symbol_type: (*symbol_type).into(),
                at: [k as f64 * 2.0, 1.0],
                size: [1.0, 1.0, 1.0],
                ..Default::default()
            });
        }
        m
    }

    fn export(m: &ArchModel) -> String {
        write_model(m, &Scene3d::build(m), &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() })
    }

    /// A parameter list split at its top-level commas (quotes and parentheses respected).
    fn split_params(raw: &str) -> Vec<String> {
        let (mut out, mut cur, mut depth, mut quoted) = (Vec::new(), String::new(), 0i32, false);
        for c in raw.chars() {
            match c {
                '\'' => quoted = !quoted,
                '(' if !quoted => depth += 1,
                ')' if !quoted => depth -= 1,
                ',' if !quoted && depth == 0 => {
                    out.push(std::mem::take(&mut cur));
                    continue;
                }
                _ => {}
            }
            cur.push(c);
        }
        out.push(cur);
        out
    }

    /// (entity, parameters) by instance number.
    fn instances(text: &str) -> BTreeMap<u32, (String, Vec<String>)> {
        let mut out = BTreeMap::new();
        for line in text.lines().filter(|l| l.starts_with('#')) {
            let (n, rest) = line[1..].split_once('=').unwrap_or_default();
            let (name, params) = rest.split_once('(').unwrap_or_default();
            let params = params.strip_suffix(");").unwrap_or(params);
            out.insert(n.parse().unwrap_or(0), (name.to_string(), split_params(params)));
        }
        out
    }

    fn reference(p: &str) -> u32 {
        p.strip_prefix('#').and_then(|n| n.parse().ok()).unwrap_or_else(|| panic!("{p} is not a reference"))
    }

    fn references(p: &str) -> Vec<u32> {
        p.trim_start_matches('(').trim_end_matches(')').split(',').filter(|r| !r.is_empty()).map(reference).collect()
    }

    /// The furnishing elements: (instance, entity, parameters) of every element of the four classes.
    fn elements(inst: &BTreeMap<u32, (String, Vec<String>)>) -> Vec<(u32, &str, &Vec<String>)> {
        inst.iter().filter(|(_, (e, _))| CLASSES.contains(&e.as_str())).map(|(n, (e, p))| (*n, e.as_str(), p)).collect()
    }

    /// The element named `name`.
    fn element<'i>(inst: &'i BTreeMap<u32, (String, Vec<String>)>, name: &str) -> (u32, &'i str, &'i Vec<String>) {
        let found: Vec<_> = elements(inst).into_iter().filter(|(_, _, p)| p.get(2).map(String::as_str) == Some(&format!("'{name}'"))).collect();
        assert_eq!(found.len(), 1, "one element named {name}: {found:?}");
        found[0]
    }

    /// The number of points and triangles of a product's body (its one face set).
    fn body_size(inst: &BTreeMap<u32, (String, Vec<String>)>, product: &[String]) -> (usize, usize) {
        let shape = &inst[&reference(&product[6])];
        assert_eq!(shape.0, "IFCPRODUCTDEFINITIONSHAPE");
        let reps = references(&shape.1[2]);
        assert_eq!(reps.len(), 1);
        let rep = &inst[&reps[0]];
        assert_eq!((rep.0.as_str(), rep.1[1].as_str(), rep.1[2].as_str()), ("IFCSHAPEREPRESENTATION", "'Body'", "'Tessellation'"));
        let sets = references(&rep.1[3]);
        assert_eq!(sets.len(), 1, "one face set");
        let set = &inst[&sets[0]];
        assert_eq!(set.0, "IFCTRIANGULATEDFACESET");
        let points = &inst[&reference(&set.1[0])];
        assert_eq!(points.0, "IFCCARTESIANPOINTLIST3D");
        (points.1[0].matches("),(").count() + 1, set.1[3].matches("),(").count() + 1)
    }

    #[test]
    fn ifc_furnishings_by_category() {
        let m = model(&[SOFA, TOILET, FRIDGE, LIGHT]);
        let text = export(&m);
        let inst = instances(&text);
        assert_eq!(elements(&inst).len(), 4, "one element per furnishing");
        let want = [
            ("IFCFURNITURE", "'SOFA'", ".USERDEFINED."),
            ("IFCSANITARYTERMINAL", "$", ".TOILETPAN."),
            ("IFCELECTRICAPPLIANCE", "$", ".REFRIGERATOR."),
            ("IFCLIGHTFIXTURE", "$", ".POINTSOURCE."),
        ];
        let contains: Vec<&Vec<String>> = inst.values().filter(|(e, _)| e == "IFCRELCONTAINEDINSPATIALSTRUCTURE").map(|(_, p)| p).collect();
        assert_eq!(contains.len(), 1, "everything is on level 0");
        let contained = references(&contains[0][4]);
        for (k, (class, object_type, predefined)) in want.into_iter().enumerate() {
            let (n, entity, p) = element(&inst, &format!("furnishing-f{k}"));
            assert_eq!(entity, class, "f{k}");
            assert_eq!(p.len(), 9, "{class} has IFC4's nine attributes: {p:?}");
            assert_eq!(p[0], format!("'{}'", global_id(&format!("furnishing/{k}"))));
            assert_eq!(p[3], format!("'Item {k}'"), "Description is the item's name");
            assert_eq!(p[4], object_type, "{class} ObjectType");
            assert_eq!(p[7], "$", "Tag");
            assert_eq!(p[8], predefined, "{class} PredefinedType");
            assert_eq!(body_size(&inst, p), (6, 2), "f{k}'s body is both parts of its symbol");
            assert!(contained.contains(&n), "f{k} is contained in its storey");
        }
    }

    #[test]
    fn ifc_furnishing_predefined_types_follow_the_symbol_type() {
        let plumbing = "Appliances and plumbing fixtures";
        let cases = [
            ("sink", "IFCSANITARYTERMINAL", ".SINK."),
            ("lavatory", "IFCSANITARYTERMINAL", ".SINK."),
            ("bathtub", "IFCSANITARYTERMINAL", ".BATH."),
            ("shower", "IFCSANITARYTERMINAL", ".SHOWER."),
            ("faucet", "IFCSANITARYTERMINAL", ".USERDEFINED."),
            ("dishwasher", "IFCELECTRICAPPLIANCE", ".DISHWASHER."),
            ("range", "IFCELECTRICAPPLIANCE", ".ELECTRICCOOKER."),
            ("cooktop", "IFCELECTRICAPPLIANCE", ".ELECTRICCOOKER."),
            ("oven", "IFCELECTRICAPPLIANCE", ".ELECTRICCOOKER."),
            ("washer", "IFCELECTRICAPPLIANCE", ".USERDEFINED."),
        ];
        let items: Vec<(&str, &str, &str)> = cases.iter().map(|(t, _, _)| ("x", plumbing, *t)).collect();
        let text = export(&model(&items));
        let inst = instances(&text);
        for (k, (symbol_type, class, predefined)) in cases.into_iter().enumerate() {
            let (_, entity, p) = element(&inst, &format!("furnishing-f{k}"));
            assert_eq!((entity, p[8].as_str()), (class, predefined), "{symbol_type}");
            // A user-defined type is named by ObjectType (IFC's CorrectPredefinedType rule).
            let named = if predefined == ".USERDEFINED." { format!("'{}'", symbol_type.to_uppercase()) } else { "$".into() };
            assert_eq!(p[4], named, "{symbol_type} ObjectType");
        }
    }

    #[test]
    fn ifc_furnishing_is_in_its_levels_storey() {
        let mut m = model(&[SOFA, LIGHT]);
        m.furnishings[1].level = 1;
        m.furnishings[1].elevation = 2.5;
        let text = export(&m);
        let inst = instances(&text);
        let storey = |index: u32| {
            inst.iter()
                .find(|(_, (e, p))| e == "IFCBUILDINGSTOREY" && p[0] == format!("'{}'", global_id(&format!("storey/{index}"))))
                .map(|(n, _)| *n)
        };
        let (light, _, p) = element(&inst, "furnishing-f1");
        let rels: Vec<&Vec<String>> = inst.values().filter(|(e, _)| e == "IFCRELCONTAINEDINSPATIALSTRUCTURE").map(|(_, p)| p).collect();
        let holder: Vec<u32> = rels.iter().filter(|r| references(&r[4]).contains(&light)).map(|r| reference(&r[5])).collect();
        assert_eq!(holder, vec![storey(1).unwrap_or(0)], "the light on level 1 is in storey 1");
        // Its body is in the storey's coordinates: 2.5 m above the storey's floor, not 5.5 m.
        let shape = &inst[&reference(&p[6])];
        let rep = &inst[&references(&shape.1[2])[0]];
        let set = &inst[&references(&rep.1[3])[0]];
        let points = &inst[&reference(&set.1[0])].1[0];
        assert!(points.contains(",2.5)"), "{points}");
        assert!(!points.contains(",5.5)"), "{points}");
    }

    #[test]
    fn ifc_furnishing_names_match_glb_nodes() {
        let m = model(&[SOFA, TOILET, FRIDGE, LIGHT]);
        let scene = Scene3d::build(&m);
        let text = write_model(&m, &scene, &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() });
        let inst = instances(&text);
        let glb: BTreeSet<String> = crate::glb::node_names(&scene).into_iter().filter(|n| n.starts_with("furnishing-")).collect();
        let ifc: BTreeSet<String> = elements(&inst).iter().map(|(_, _, p)| p[2].trim_matches('\'').to_string()).collect();
        assert_eq!(glb.len(), 4, "{glb:?}");
        assert_eq!(ifc, glb);
    }

    #[test]
    fn ifc_furnishing_globalids_are_reproducible() {
        let a = model(&[SOFA, TOILET, FRIDGE]);
        assert_eq!(export(&a), export(&a), "two exports of one model are byte-identical");
        // An id follows its furnishing, not its place in the list: f1 keeps its GlobalId when f0
        // goes or another furnishing comes first.
        let mut b = a.clone();
        b.furnishings.remove(0);
        let mut c = a.clone();
        let mut extra = c.furnishings[0].clone();
        extra.id = ArchId(9);
        c.furnishings.insert(0, extra);
        for m in [&a, &b, &c] {
            let text = export(m);
            let inst = instances(&text);
            let (_, _, p) = element(&inst, "furnishing-f1");
            assert_eq!(p[0], format!("'{}'", global_id("furnishing/1")));
            let pset = global_id("pset/CADCraft_Library/furnishing/1");
            assert!(inst.values().any(|(e, p)| e == "IFCPROPERTYSET" && p[0] == format!("'{pset}'")), "f1's property set keeps its id");
        }
        // A repeated id (a hostile file) is written once, so no GlobalId repeats.
        let mut d = a.clone();
        let twin = d.furnishings[1].clone();
        d.furnishings.push(twin);
        let text = export(&d);
        let inst = instances(&text);
        assert_eq!(elements(&inst).len(), 3);
        let ids: Vec<&String> = inst.values().filter_map(|(_, p)| p.first()).filter(|p| p.len() == 24 && p.starts_with('\'')).collect();
        assert_eq!(ids.len(), ids.iter().collect::<BTreeSet<_>>().len(), "GlobalIds repeat");
    }

    #[test]
    fn ifc_furnishing_properties_carry_the_licence() {
        let m = model(&[SOFA, ("kit-chair", "Furniture", "chair/stool")]);
        let text = export(&m);
        let inst = instances(&text);
        // The sofa is a sample item: its licence and source are the sample catalog's.
        let catalog: serde_json::Value = serde_json::from_slice(crate::library::sample_files()[0].1).unwrap_or_default();
        let futon = catalog["items"].as_array().and_then(|items| items.iter().find(|i| i["id"] == "sofa-futon")).cloned().unwrap_or_default();
        let (licence, source) = (futon["licence"].as_str().unwrap_or_default(), futon["source"].as_str().unwrap_or_default());
        assert!(!licence.is_empty() && !source.is_empty(), "the sample catalog names the futon's licence and source");
        let props = |k: u32| -> Vec<(String, String)> {
            let (owner, _, _) = element(&inst, &format!("furnishing-f{k}"));
            let id = format!("'{}'", global_id(&format!("pset/CADCraft_Library/furnishing/{k}")));
            let (set, (_, p)) = inst.iter().find(|(_, (e, p))| e == "IFCPROPERTYSET" && p[0] == id).unwrap_or_else(|| panic!("no pset for f{k}"));
            assert_eq!(p[2], "'CADCraft_Library'");
            let rel = inst
                .values()
                .find(|(e, r)| e == "IFCRELDEFINESBYPROPERTIES" && reference(&r[5]) == *set)
                .unwrap_or_else(|| panic!("f{k}'s pset is related"));
            assert_eq!(references(&rel.1[4]), vec![owner], "the pset is f{k}'s");
            references(&p[4])
                .into_iter()
                .map(|v| {
                    let (e, p) = &inst[&v];
                    assert_eq!(e, "IFCPROPERTYSINGLEVALUE");
                    (p[0].clone(), p[2].clone())
                })
                .collect()
        };
        assert_eq!(
            props(0),
            vec![
                ("'Item'".to_string(), "IFCIDENTIFIER('sofa-futon')".to_string()),
                ("'Licence'".to_string(), format!("IFCLABEL('{}')", encode_string(licence))),
                ("'Source'".to_string(), format!("IFCTEXT('{}')", encode_string(source))),
            ]
        );
        // An item outside the sample set (an installed catalog's): the export has no record of it, so
        // its licence and source are written without a value rather than invented.
        assert_eq!(
            props(1),
            vec![
                ("'Item'".to_string(), "IFCIDENTIFIER('kit-chair')".to_string()),
                ("'Licence'".to_string(), "$".to_string()),
                ("'Source'".to_string(), "$".to_string()),
            ]
        );
    }

    #[test]
    fn ifc_furnishing_empty_symbol_type_uses_unnamed_type() {
        let m = model(&[("item", "Furniture", "")]);
        let text = export(&m);
        let inst = instances(&text);
        let (_, entity, p) = element(&inst, "furnishing-f0");
        assert_eq!(entity, "IFCFURNITURE");
        assert_eq!(p[4], "'FURNISHING'", "empty symbol_type uses UNNAMED_TYPE");
        assert_eq!(p[8], ".USERDEFINED.");
    }

    #[test]
    fn ifc_furnishing_empty_name_has_null_description() {
        let mut m = model(&[SOFA]);
        m.furnishings[0].name = "".into();
        let text = export(&m);
        let inst = instances(&text);
        let (_, _, p) = element(&inst, "furnishing-f0");
        assert_eq!(p[3], "$", "empty name gives null Description");
    }

    #[test]
    fn ifc_furnishing_without_a_mesh_is_still_one_element() {
        let mut m = model(&[SOFA, TOILET]);
        m.furnishings[1].at = [f64::NAN, 0.0];
        let text = export(&m);
        let inst = instances(&text);
        let (_, _, p) = element(&inst, "furnishing-f1");
        assert_eq!(p[6], "$", "an unbuildable furnishing has no body");
        assert!(!text.contains("NaN"), "nothing invalid is written");
        assert_eq!(elements(&inst).len(), 2);
    }
}
