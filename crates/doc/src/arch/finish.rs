//! Surface finishes (phase 13-18): a wall's exterior face, a roof's covering, a room's floor, and
//! the phase-18 interior finishes (concrete, tile, carpet, drywall, plaster) that room-type
//! templates pick. Plain data on [`super::ArchModel`]; the `material` command, the inspect keys
//! and the 3D materials live elsewhere.

use serde::{Deserialize, Serialize};

/// A surface finish: a wall's exterior face, a roof's covering, a room's floor, or an interior
/// (wall, floor, ceiling) finish.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Finish {
    /// Siding (walls).
    Siding,
    /// Shingles (roofs).
    Shingles,
    /// Brick (walls).
    Brick,
    /// Wood (walls, roofs, floors).
    Wood,
    /// Concrete (walls and floors).
    Concrete,
    /// Tile (walls and floors).
    Tile,
    /// Carpet (floors).
    Carpet,
    /// Drywall (walls and ceilings).
    Drywall,
    /// Plaster (ceilings).
    Plaster,
}

impl Finish {
    /// Every finish, in declaration order.
    pub const ALL: [Finish; 9] = [
        Finish::Siding,
        Finish::Shingles,
        Finish::Brick,
        Finish::Wood,
        Finish::Concrete,
        Finish::Tile,
        Finish::Carpet,
        Finish::Drywall,
        Finish::Plaster,
    ];

    /// The JSON/DXF name: "siding" | "shingles" | "brick" | "wood" | "concrete" | "tile"
    /// | "carpet" | "drywall" | "plaster".
    pub fn name(self) -> &'static str {
        match self {
            Finish::Siding => "siding",
            Finish::Shingles => "shingles",
            Finish::Brick => "brick",
            Finish::Wood => "wood",
            Finish::Concrete => "concrete",
            Finish::Tile => "tile",
            Finish::Carpet => "carpet",
            Finish::Drywall => "drywall",
            Finish::Plaster => "plaster",
        }
    }

    /// Exact lower-case names only — no trimming, no case folding, no aliases; anything else is `None`.
    pub fn parse(s: &str) -> Option<Finish> {
        match s {
            "siding" => Some(Finish::Siding),
            "shingles" => Some(Finish::Shingles),
            "brick" => Some(Finish::Brick),
            "wood" => Some(Finish::Wood),
            "concrete" => Some(Finish::Concrete),
            "tile" => Some(Finish::Tile),
            "carpet" => Some(Finish::Carpet),
            "drywall" => Some(Finish::Drywall),
            "plaster" => Some(Finish::Plaster),
            _ => None,
        }
    }

    /// siding, brick, wood, tile, drywall, concrete.
    pub fn for_walls(self) -> bool {
        matches!(self, Finish::Siding | Finish::Brick | Finish::Wood | Finish::Tile | Finish::Drywall | Finish::Concrete)
    }

    /// shingles, wood.
    pub fn for_roofs(self) -> bool {
        matches!(self, Finish::Shingles | Finish::Wood)
    }

    /// wood, tile, carpet, concrete.
    pub fn for_floors(self) -> bool {
        matches!(self, Finish::Wood | Finish::Tile | Finish::Carpet | Finish::Concrete)
    }

    /// plaster, drywall, wood.
    pub fn for_ceilings(self) -> bool {
        matches!(self, Finish::Plaster | Finish::Drywall | Finish::Wood)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::{Roof, Room, Wall};

    #[test]
    fn finish_names_round_trip() {
        let names: Vec<&str> = Finish::ALL.iter().map(|f| f.name()).collect();
        assert_eq!(names, ["siding", "shingles", "brick", "wood", "concrete", "tile", "carpet", "drywall", "plaster"]);
        for f in Finish::ALL {
            assert_eq!(Finish::parse(f.name()), Some(f));
            let json = serde_json::to_string(&f).unwrap();
            assert_eq!(json, format!("\"{}\"", f.name()));
            assert_eq!(serde_json::from_str::<Finish>(&json).unwrap(), f);
        }
        for bad in ["Siding", " siding", "", "marble"] {
            assert_eq!(Finish::parse(bad), None);
        }
    }

    #[test]
    fn finish_target_kinds() {
        let rows = [
            (Finish::Siding, true, false, false),
            (Finish::Shingles, false, true, false),
            (Finish::Brick, true, false, false),
            (Finish::Wood, true, true, true),
            (Finish::Concrete, true, false, true),
            (Finish::Tile, true, false, true),
            (Finish::Carpet, false, false, true),
            (Finish::Drywall, true, false, false),
            (Finish::Plaster, false, false, false),
        ];
        for (f, w, r, fl) in rows {
            assert_eq!(f.for_walls(), w, "{f:?} walls");
            assert_eq!(f.for_roofs(), r, "{f:?} roofs");
            assert_eq!(f.for_floors(), fl, "{f:?} floors");
        }
    }

    #[test]
    fn for_ceilings() {
        let ceilings: Vec<Finish> = Finish::ALL.into_iter().filter(|f| f.for_ceilings()).collect();
        assert_eq!(ceilings, [Finish::Wood, Finish::Drywall, Finish::Plaster]);
    }

    #[test]
    fn finish_fields_are_optional_in_json() {
        let mut wall = serde_json::to_value(Wall::default()).unwrap();
        assert!(wall.get("finish").is_none());
        let room = serde_json::to_value(Room::default()).unwrap();
        assert!(room.get("floorFinish").is_none() && room.get("floor_finish").is_none());
        let roof = serde_json::to_value(Roof::default()).unwrap();
        assert!(roof.get("covering").is_none());

        let w: Wall = serde_json::from_value(wall.clone()).unwrap();
        assert_eq!(w.finish, None);
        let r: Room = serde_json::from_value(room).unwrap();
        assert_eq!(r.floor_finish, None);
        let rf: Roof = serde_json::from_value(roof).unwrap();
        assert_eq!(rf.covering, None);

        if let Some(o) = wall.as_object_mut() {
            o.insert("finish".into(), "brick".into());
        }
        let w: Wall = serde_json::from_value(wall).unwrap();
        assert_eq!(w.finish, Some(Finish::Brick));
        let again = serde_json::to_value(&w).unwrap();
        assert_eq!(again.get("finish"), Some(&serde_json::json!("brick")));

        let w2 = Wall { finish: Some(Finish::Wood), ..Default::default() };
        assert!(serde_json::to_value(&w2).unwrap().get("finish").is_some());
    }

    #[test]
    fn finish_all_has_nine_distinct_entries() {
        assert_eq!(Finish::ALL.len(), 9);
        let names: Vec<&str> = Finish::ALL.iter().map(|f| f.name()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 9, "all finish names must be distinct");
        // Every entry in ALL must parse back to itself.
        for f in Finish::ALL {
            assert_eq!(Finish::parse(f.name()), Some(f));
        }
    }

    #[test]
    fn parse_rejects_case_and_whitespace_variants() {
        // Case variations of each finish name must be rejected.
        for f in Finish::ALL {
            let name = f.name();
            let upper: String = name.chars().map(|c| c.to_ascii_uppercase()).collect();
            if upper != name {
                assert_eq!(Finish::parse(&upper), None, "parse({}) should be None", upper);
            }
            let lower: String = name.chars().map(|c| c.to_ascii_lowercase()).collect();
            if lower != name {
                assert_eq!(Finish::parse(&lower), None, "parse({}) should be None", lower);
            }
        }
        // Whitespace-only and whitespace-padded strings.
        for ws in ["", " ", "\t", "\n", "  ", "\t\n", "siding ", " siding", "s iding", " siding "] {
            assert_eq!(Finish::parse(ws), None, "parse({:?}) should be None", ws);
        }
        // Unicode lookalikes and zero-width characters.
        assert_eq!(Finish::parse("sïding"), None);
        assert_eq!(Finish::parse("siding\u{200b}"), None);
        assert_eq!(Finish::parse("siding\u{00a0}"), None);
        // A long concatenated string.
        assert_eq!(Finish::parse("sidingwoodbricktileconcretecarpetdrywallplaster"), None);
        // A name that is a prefix of another.
        assert_eq!(Finish::parse("wooden"), None);
        assert_eq!(Finish::parse("tiles"), None);
    }

    #[test]
    fn every_finish_is_valid_for_at_least_one_surface() {
        // Invariant: no finish is dead code; each is usable on at least one surface type.
        for f in Finish::ALL {
            assert!(f.for_walls() || f.for_roofs() || f.for_floors() || f.for_ceilings(), "{:?} is not valid for any surface type", f);
        }
    }

    #[test]
    fn finish_names_are_lowercase_alphabetic_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for f in Finish::ALL {
            let name = f.name();
            assert!(!name.is_empty(), "{:?} has an empty name", f);
            assert!(name.chars().all(|c| c.is_ascii_lowercase()), "{:?} name {:?} contains non-lowercase characters", f, name);
            assert!(name.chars().all(|c| c.is_alphabetic()), "{:?} name {:?} contains non-alphabetic characters", f, name);
            assert!(seen.insert(name.to_string()), "{:?} name {:?} is not unique", f, name);
        }
    }

    #[test]
    fn serde_rejects_unknown_finish_name_in_wall() {
        let json = serde_json::json!({
            "start": {"x": 0.0, "y": 0.0},
            "end": {"x": 1.0, "y": 0.0},
            "finish": "marble"
        });
        let result: Result<Wall, _> = serde_json::from_value(json);
        assert!(result.is_err(), "deserializing a wall with unknown finish 'marble' should fail");
    }

    #[test]
    fn finish_ord_matches_declaration_order() {
        // The derived Ord must agree with declaration order (Finish::ALL).
        for window in Finish::ALL.windows(2) {
            let (a, b) = (window[0], window[1]);
            assert!(a < b, "{:?} should be less than {:?} in declaration order", a, b);
        }
    }
}
