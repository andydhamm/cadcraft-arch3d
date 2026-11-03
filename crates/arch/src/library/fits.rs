//! Context-aware query generation.

use super::search::{Chip, ChipKind, Query};

/// Generate a query for a specific target surface or object.
pub fn query_for(target: &str) -> Option<Query> {
    let mut parts = target.split('/');
    let kind = parts.next()?;
    let _id = parts.next()?;
    let sub = parts.next();

    match kind {
        "cabinet" => {
            let chip = match sub {
                Some("pull") => Chip { kind: ChipKind::SymbolType, value: "pull".into(), ..Default::default() },
                Some("front") => Chip { kind: ChipKind::Folder, value: "Front styles".into(), ..Default::default() },
                Some("carcass") => Chip { kind: ChipKind::Placement, value: "in-cabinet".into(), ..Default::default() },
                _ => return None,
            };
            Some(Query { text: String::new(), chips: vec![chip], ..Default::default() })
        }
        "wall" | "room" | "roof" => {
            // wall/<id>/<side>, room/<id>/floor|ceiling, roof/<id>
            // All map to Materials category
            Some(Query {
                text: String::new(),
                chips: vec![Chip { kind: ChipKind::Category, value: "Materials".into(), ..Default::default() }],
                ..Default::default()
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_for_targets() {
        let q = query_for("cabinet/cb0/pull").unwrap();
        assert_eq!(q.chips.len(), 1);
        assert_eq!(q.chips[0].kind, ChipKind::SymbolType);
        assert_eq!(q.chips[0].value, "pull");

        let q = query_for("cabinet/cb0/front").unwrap();
        assert_eq!(q.chips[0].kind, ChipKind::Folder);
        assert_eq!(q.chips[0].value, "Front styles");

        let q = query_for("cabinet/cb0/carcass").unwrap();
        assert_eq!(q.chips[0].kind, ChipKind::Placement);
        assert_eq!(q.chips[0].value, "in-cabinet");

        let q = query_for("wall/w3/left").unwrap();
        assert_eq!(q.chips[0].kind, ChipKind::Category);
        assert_eq!(q.chips[0].value, "Materials");

        let q = query_for("room/r0/floor").unwrap();
        assert_eq!(q.chips[0].kind, ChipKind::Category);
        assert_eq!(q.chips[0].value, "Materials");

        let q = query_for("roof/r0").unwrap();
        assert_eq!(q.chips[0].kind, ChipKind::Category);
        assert_eq!(q.chips[0].value, "Materials");

        assert!(query_for("unknown/x").is_none());
        assert!(query_for("cabinet/cb0/unknown").is_none());
    }
}
