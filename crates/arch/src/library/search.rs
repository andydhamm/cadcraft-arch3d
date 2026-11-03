//! Library search and filtering.

use super::record::ItemRecord;
use serde::{Deserialize, Serialize};

/// The kind of filter chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChipKind {
    Category,
    Folder,
    SymbolType,
    Placement,
    Source,
    Style,
    Tag,
}

impl ChipKind {
    /// Parse a chip kind from its camelCase name.
    pub fn parse(s: &str) -> Option<ChipKind> {
        match s {
            "category" => Some(ChipKind::Category),
            "folder" => Some(ChipKind::Folder),
            "symbolType" => Some(ChipKind::SymbolType),
            "placement" => Some(ChipKind::Placement),
            "source" => Some(ChipKind::Source),
            "style" => Some(ChipKind::Style),
            "tag" => Some(ChipKind::Tag),
            _ => None,
        }
    }
}

/// A filter chip.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Chip {
    pub kind: ChipKind,
    pub value: String,
}

/// A search query.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Query {
    pub text: String,
    pub chips: Vec<Chip>,
}

/// Check if an item matches a query.
pub fn matches(item: &ItemRecord, q: &Query) -> bool {
    let text_lower = q.text.to_lowercase();
    let words: Vec<&str> = text_lower.split_whitespace().collect();

    let name_lower = item.name.to_lowercase();
    let folder_lower = item.folder.to_lowercase();
    let keywords_lower: Vec<String> = item.keywords.iter().map(|k| k.to_lowercase()).collect();

    for word in &words {
        let found = name_lower.contains(word) || folder_lower.contains(word) || keywords_lower.iter().any(|k| k.contains(word));
        if !found {
            return false;
        }
    }

    for chip in &q.chips {
        let ok = match chip.kind {
            ChipKind::Category => item.category == chip.value,
            ChipKind::Folder => item.folder == chip.value,
            ChipKind::SymbolType => item.symbol_type == chip.value,
            ChipKind::Placement => item.placement.as_deref() == Some(&chip.value),
            ChipKind::Source => {
                if chip.value == "installed" {
                    item.catalog.starts_with("installed:")
                } else {
                    item.catalog == chip.value
                }
            }
            ChipKind::Style => item.style.as_deref() == Some(&chip.value),
            ChipKind::Tag => item.tags.contains(&chip.value),
        };
        if !ok {
            return false;
        }
    }

    true
}

/// Search items with a query, returning matches sorted by category, folder, name, id.
pub fn search<'a>(items: &'a [ItemRecord], q: &Query) -> Vec<&'a ItemRecord> {
    let mut matches: Vec<&ItemRecord> = items.iter().filter(|i| matches(i, q)).collect();
    matches.sort_by(|a, b| {
        let cat_a = Category::parse(&a.category).unwrap_or(Category::User);
        let cat_b = Category::parse(&b.category).unwrap_or(Category::User);
        cat_a.cmp(&cat_b).then_with(|| a.folder.cmp(&b.folder)).then_with(|| a.name.cmp(&b.name)).then_with(|| a.id.cmp(&b.id))
    });
    matches
}

/// Get folders and counts from a list of items.
pub fn folders(items: &[&ItemRecord]) -> Vec<(String, usize)> {
    let mut map = std::collections::BTreeMap::new();
    for item in items {
        *map.entry(item.folder.clone()).or_insert(0) += 1;
    }
    map.into_iter().collect()
}

use super::record::Category;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::record::ItemRecord;

    fn make_item(id: &str, name: &str, category: &str, folder: &str, symbol_type: &str, keywords: &[&str]) -> ItemRecord {
        ItemRecord {
            id: id.to_string(),
            name: name.to_string(),
            category: category.to_string(),
            folder: folder.to_string(),
            symbol_type: symbol_type.to_string(),
            keywords: keywords.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn text_words_must_all_match_name_keyword_or_folder() {
        let items = vec![
            make_item("1", "Red Chair", "Furniture", "Chairs and stools", "chair", &["red", "seat"]),
            make_item("2", "Blue Sofa", "Furniture", "Sofas", "sofa", &["blue"]),
        ];
        let q = Query { text: "red chair".into(), chips: vec![] };
        let res = search(&items, &q);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "1");

        let q2 = Query { text: "blue sofa".into(), chips: vec![] };
        let res2 = search(&items, &q2);
        assert_eq!(res2.len(), 1);
        assert_eq!(res2[0].id, "2");

        let q3 = Query { text: "red sofa".into(), chips: vec![] };
        let res3 = search(&items, &q3);
        assert!(res3.is_empty());
    }

    #[test]
    fn chips_combine_with_and() {
        let items =
            vec![make_item("1", "Chair", "Furniture", "Chairs and stools", "chair", &[]), make_item("2", "Sofa", "Furniture", "Sofas", "sofa", &[])];
        let q = Query { text: "".into(), chips: vec![Chip { kind: ChipKind::SymbolType, value: "chair".into() }] };
        let res = search(&items, &q);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "1");

        let q2 = Query {
            text: "".into(),
            chips: vec![Chip { kind: ChipKind::SymbolType, value: "chair".into() }, Chip { kind: ChipKind::Folder, value: "Sofas".into() }],
        };
        let res2 = search(&items, &q2);
        assert!(res2.is_empty());
    }

    #[test]
    fn source_chip_matches_installed_catalogs() {
        let mut item = make_item("1", "Item", "Furniture", "Other", "chair", &[]);
        item.catalog = "installed:modern".into();
        let items = vec![item];
        let q = Query { text: "".into(), chips: vec![Chip { kind: ChipKind::Source, value: "installed".into() }] };
        let res = search(&items, &q);
        assert_eq!(res.len(), 1);
    }

    #[test]
    fn an_unmatched_chip_returns_nothing() {
        let items = vec![make_item("1", "Chair", "Furniture", "Chairs and stools", "chair", &[])];
        let q = Query { text: "".into(), chips: vec![Chip { kind: ChipKind::SymbolType, value: "table".into() }] };
        let res = search(&items, &q);
        assert!(res.is_empty());
    }

    #[test]
    fn folders_count_items() {
        let items = vec![
            make_item("1", "A", "Furniture", "Chairs and stools", "chair", &[]),
            make_item("2", "B", "Furniture", "Chairs and stools", "chair", &[]),
            make_item("3", "C", "Furniture", "Sofas", "sofa", &[]),
        ];
        let refs: Vec<&ItemRecord> = items.iter().collect();
        let res = folders(&refs);
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].0, "Chairs and stools");
        assert_eq!(res[0].1, 2);
        assert_eq!(res[1].0, "Sofas");
        assert_eq!(res[1].1, 1);
    }
}
