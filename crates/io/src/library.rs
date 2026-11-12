//! The sample library set, embedded in the binary so every app (native and
//! web) has it without an install path.

/// Every embedded sample file as `(file name, bytes)`: `catalog.json` first,
/// then the files it names, sorted by name.
pub fn sample_files() -> &'static [(&'static str, &'static [u8])] {
    const FILES: &[(&str, &[u8])] = &[
        ("catalog.json", include_bytes!("../../../assets/library/sample/catalog.json")),
        ("armchair.glb", include_bytes!("../../../assets/library/sample/armchair.glb")),
        ("backdrop-sky.png", include_bytes!("../../../assets/library/sample/backdrop-sky.png")),
        ("basin-built-in.glb", include_bytes!("../../../assets/library/sample/basin-built-in.glb")),
        ("bathtub.glb", include_bytes!("../../../assets/library/sample/bathtub.glb")),
        ("bed-double-wood.glb", include_bytes!("../../../assets/library/sample/bed-double-wood.glb")),
        ("bed-single.glb", include_bytes!("../../../assets/library/sample/bed-single.glb")),
        ("bookcase.glb", include_bytes!("../../../assets/library/sample/bookcase.glb")),
        ("chair-dining-2.glb", include_bytes!("../../../assets/library/sample/chair-dining-2.glb")),
        ("chair-dining-3.glb", include_bytes!("../../../assets/library/sample/chair-dining-3.glb")),
        ("chair-dining-wood.glb", include_bytes!("../../../assets/library/sample/chair-dining-wood.glb")),
        ("cooktop.glb", include_bytes!("../../../assets/library/sample/cooktop.glb")),
        ("desk.glb", include_bytes!("../../../assets/library/sample/desk.glb")),
        ("dishwasher-60.glb", include_bytes!("../../../assets/library/sample/dishwasher-60.glb")),
        ("dresser-6-drawers.glb", include_bytes!("../../../assets/library/sample/dresser-6-drawers.glb")),
        ("faucet-bathroom-chrome.glb", include_bytes!("../../../assets/library/sample/faucet-bathroom-chrome.glb")),
        ("faucet-kitchen-chrome.glb", include_bytes!("../../../assets/library/sample/faucet-kitchen-chrome.glb")),
        (
            "hdri/kloofendal_48d_partly_cloudy_puresky_1k.hdr",
            include_bytes!("../../../assets/library/sample/hdri/kloofendal_48d_partly_cloudy_puresky_1k.hdr"),
        ),
        ("lamp-desk.glb", include_bytes!("../../../assets/library/sample/lamp-desk.glb")),
        ("lamp-floor.glb", include_bytes!("../../../assets/library/sample/lamp-floor.glb")),
        ("lavatory-small.glb", include_bytes!("../../../assets/library/sample/lavatory-small.glb")),
        ("light-ceiling.glb", include_bytes!("../../../assets/library/sample/light-ceiling.glb")),
        ("light-pendant.glb", include_bytes!("../../../assets/library/sample/light-pendant.glb")),
        ("light-wall.glb", include_bytes!("../../../assets/library/sample/light-wall.glb")),
        ("material-fabric.jpg", include_bytes!("../../../assets/library/sample/material-fabric.jpg")),
        ("material-marble.jpg", include_bytes!("../../../assets/library/sample/material-marble.jpg")),
        ("material-oak.jpg", include_bytes!("../../../assets/library/sample/material-oak.jpg")),
        ("oven-built-in.glb", include_bytes!("../../../assets/library/sample/oven-built-in.glb")),
        ("plant-large.glb", include_bytes!("../../../assets/library/sample/plant-large.glb")),
        ("plant-small.glb", include_bytes!("../../../assets/library/sample/plant-small.glb")),
        ("pull-bar.glb", include_bytes!("../../../assets/library/sample/pull-bar.glb")),
        ("pull-knob.glb", include_bytes!("../../../assets/library/sample/pull-knob.glb")),
        ("range-30.glb", include_bytes!("../../../assets/library/sample/range-30.glb")),
        ("range-hood.glb", include_bytes!("../../../assets/library/sample/range-hood.glb")),
        ("refrigerator-fridge-freezer.glb", include_bytes!("../../../assets/library/sample/refrigerator-fridge-freezer.glb")),
        ("refrigerator-small.glb", include_bytes!("../../../assets/library/sample/refrigerator-small.glb")),
        ("rug-oriental.glb", include_bytes!("../../../assets/library/sample/rug-oriental.glb")),
        ("shower-rain-wall.glb", include_bytes!("../../../assets/library/sample/shower-rain-wall.glb")),
        ("sink-kitchen.glb", include_bytes!("../../../assets/library/sample/sink-kitchen.glb")),
        ("sofa-chaise.glb", include_bytes!("../../../assets/library/sample/sofa-chaise.glb")),
        ("sofa-futon.glb", include_bytes!("../../../assets/library/sample/sofa-futon.glb")),
        ("stool-bar.glb", include_bytes!("../../../assets/library/sample/stool-bar.glb")),
        ("table-bedside.glb", include_bytes!("../../../assets/library/sample/table-bedside.glb")),
        ("table-coffee-90.glb", include_bytes!("../../../assets/library/sample/table-coffee-90.glb")),
        ("table-dining-150.glb", include_bytes!("../../../assets/library/sample/table-dining-150.glb")),
        ("table-dining-oak.glb", include_bytes!("../../../assets/library/sample/table-dining-oak.glb")),
        ("toilet.glb", include_bytes!("../../../assets/library/sample/toilet.glb")),
        ("towel-rack.glb", include_bytes!("../../../assets/library/sample/towel-rack.glb")),
        ("tv.glb", include_bytes!("../../../assets/library/sample/tv.glb")),
        ("wardrobe-hanging.glb", include_bytes!("../../../assets/library/sample/wardrobe-hanging.glb")),
        ("washing-machine.glb", include_bytes!("../../../assets/library/sample/washing-machine.glb")),
    ];
    FILES
}

#[cfg(test)]
mod tests {
    use super::sample_files;

    #[test]
    fn sample_list_matches_the_catalog() {
        let files = sample_files();
        assert_eq!(files.first().map(|f| f.0), Some("catalog.json"));
        let catalog = files.first().map(|f| f.1).unwrap_or_default();
        let v: serde_json::Value = serde_json::from_slice(catalog).expect("catalog parses");
        let items = v["items"].as_array().expect("items array");
        let mut named: Vec<&str> = items.iter().map(|i| i["file"].as_str().expect("file")).collect();
        named.sort_unstable();
        let table: Vec<&str> = files.iter().skip(1).map(|f| f.0).collect();
        // Sorted and unique, and exactly the catalog's files.
        assert!(table.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(named, table);
    }

    #[test]
    fn test_keeps_the_hdri_item() {
        // The sample set carries the CC-0 sky HDRI and its file is embedded as a Radiance picture.
        let files = sample_files();
        let catalog = files.first().map(|f| f.1).unwrap_or_default();
        let v: serde_json::Value = serde_json::from_slice(catalog).expect("catalog parses");
        let items = v["items"].as_array().expect("items array");
        let item = items.iter().find(|i| i["id"] == "polyhaven:kloofendal-puresky");
        assert!(item.is_some(), "the HDRI item is missing from the catalog");
        let file = item.and_then(|i| i["file"].as_str()).unwrap_or_default();
        assert!(file.ends_with(".hdr"));
        let bytes = files.iter().find(|f| f.0 == file).map(|f| f.1).unwrap_or_default();
        assert!(bytes.starts_with(b"#?RADIANCE") || bytes.starts_with(b"#?RGBE"));
    }

    #[test]
    fn sample_files_are_not_empty() {
        for (name, bytes) in sample_files() {
            assert!(!bytes.is_empty(), "{name} is empty");
            if name.ends_with(".glb") {
                assert!(bytes.starts_with(b"glTF"), "{name} is not a GLB");
            }
        }
    }
}
