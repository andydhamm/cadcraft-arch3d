//! Paint: per-surface material and tint overrides, addressed by surface key.

use super::{ArchId, Finish};
use serde::{Deserialize, Serialize};

/// A material a surface can be painted with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaterialRef {
    /// A built-in finish.
    Finish(Finish),
    /// A library material item id.
    Item(String),
    /// A flat sRGB colour.
    Colour([u8; 3]),
}

fn parse_hex6(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

impl MaterialRef {
    /// `finish:siding`, `item:material-oak` or `colour:#c04020`.
    pub fn key(&self) -> String {
        match self {
            MaterialRef::Finish(f) => format!("finish:{}", f.name()),
            MaterialRef::Item(i) => format!("item:{i}"),
            MaterialRef::Colour([r, g, b]) => format!("colour:#{r:02x}{g:02x}{b:02x}"),
        }
    }

    /// Parse a key form or a short form (`siding`, `#c04020`, any other non-empty id as an item).
    pub fn parse(s: &str) -> Option<MaterialRef> {
        if s.is_empty() {
            return None;
        }
        if let Some(rest) = s.strip_prefix("finish:") {
            return Finish::parse(rest).map(MaterialRef::Finish);
        }
        if let Some(rest) = s.strip_prefix("item:") {
            return (!rest.is_empty()).then(|| MaterialRef::Item(rest.to_string()));
        }
        if let Some(rest) = s.strip_prefix("colour:") {
            return parse_hex6(rest).map(MaterialRef::Colour);
        }
        if s.starts_with('#') {
            return parse_hex6(s).map(MaterialRef::Colour);
        }
        Some(match Finish::parse(s) {
            Some(f) => MaterialRef::Finish(f),
            None => MaterialRef::Item(s.to_string()),
        })
    }
}

/// Paint on one surface.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paint {
    /// `None` keeps the surface's own material.
    #[serde(default)]
    pub material: Option<MaterialRef>,
    /// sRGB tint.
    #[serde(default)]
    pub tint: Option<[u8; 3]>,
}

/// Side of a wall, looking from its start to its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    /// Left of start->end.
    Left,
    /// Right of start->end.
    Right,
}

/// A part of a cabinet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CabinetPart {
    /// The box.
    Carcass,
    /// Doors and drawer fronts.
    Front,
    /// The counter top.
    Counter,
    /// Pulls and handles.
    Pull,
}

impl CabinetPart {
    fn name(self) -> &'static str {
        match self {
            CabinetPart::Carcass => "carcass",
            CabinetPart::Front => "front",
            CabinetPart::Counter => "counter",
            CabinetPart::Pull => "pull",
        }
    }
}

/// A paintable surface.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Surface {
    /// One face of a wall.
    WallSide {
        /// The wall.
        wall: ArchId,
        /// Which face.
        side: Side,
    },
    /// A room's floor.
    RoomFloor(ArchId),
    /// A room's ceiling.
    RoomCeiling(ArchId),
    /// A roof.
    Roof(ArchId),
    /// A part of a cabinet.
    Cabinet {
        /// The cabinet.
        cabinet: ArchId,
        /// Which part.
        part: CabinetPart,
    },
    /// A casework fixture.
    Fixture(ArchId),
    /// A part (material slot) of a furnishing's symbol.
    Furnishing {
        /// The furnishing.
        furnishing: ArchId,
        /// Part index.
        part: u32,
    },
    /// A custom countertop.
    Countertop(ArchId),
    /// A backsplash.
    Backsplash(ArchId),
}

/// Parse a canonical decimal id with the given prefix letters (`w3`, no sign, no leading zeros).
fn labelled(s: &str, prefix: &str) -> Option<u32> {
    let digits = s.strip_prefix(prefix)?;
    number(digits)
}

fn number(digits: &str) -> Option<u32> {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || (digits.len() > 1 && digits.starts_with('0')) {
        return None;
    }
    digits.parse().ok()
}

impl Surface {
    /// The surface key, e.g. `wall/w3/left`.
    pub fn key(&self) -> String {
        match self {
            Surface::WallSide { wall, side } => {
                format!("wall/w{}/{}", wall.0, if *side == Side::Left { "left" } else { "right" })
            }
            Surface::RoomFloor(r) => format!("room/r{}/floor", r.0),
            Surface::RoomCeiling(r) => format!("room/r{}/ceiling", r.0),
            Surface::Roof(r) => format!("roof/r{}", r.0),
            Surface::Cabinet { cabinet, part } => format!("cabinet/cb{}/{}", cabinet.0, part.name()),
            Surface::Fixture(f) => format!("fixture/fx{}", f.0),
            Surface::Furnishing { furnishing, part } => format!("furnishing/f{}/{}", furnishing.0, part),
            Surface::Countertop(c) => format!("countertop/ct{}", c.0),
            Surface::Backsplash(b) => format!("backsplash/bs{}", b.0),
        }
    }

    /// The key of the whole object, e.g. `wall/w3`.
    pub fn object_key(&self) -> String {
        match self {
            Surface::WallSide { wall, .. } => format!("wall/w{}", wall.0),
            Surface::RoomFloor(r) | Surface::RoomCeiling(r) => format!("room/r{}", r.0),
            Surface::Roof(r) => format!("roof/r{}", r.0),
            Surface::Cabinet { cabinet, .. } => format!("cabinet/cb{}", cabinet.0),
            Surface::Fixture(f) => format!("fixture/fx{}", f.0),
            Surface::Furnishing { furnishing, .. } => format!("furnishing/f{}", furnishing.0),
            Surface::Countertop(_) | Surface::Backsplash(_) => self.key(),
        }
    }

    /// Parse a surface key; anything malformed is `None`.
    pub fn parse(s: &str) -> Option<Surface> {
        let parts: Vec<&str> = s.split('/').collect();
        match parts.as_slice() {
            ["wall", w, side] => {
                let side = match *side {
                    "left" => Side::Left,
                    "right" => Side::Right,
                    _ => return None,
                };
                Some(Surface::WallSide { wall: ArchId(labelled(w, "w")?), side })
            }
            ["room", r, which] => {
                let id = ArchId(labelled(r, "r")?);
                match *which {
                    "floor" => Some(Surface::RoomFloor(id)),
                    "ceiling" => Some(Surface::RoomCeiling(id)),
                    _ => None,
                }
            }
            ["roof", r] => Some(Surface::Roof(ArchId(labelled(r, "r")?))),
            ["cabinet", c, part] => {
                let part = match *part {
                    "carcass" => CabinetPart::Carcass,
                    "front" => CabinetPart::Front,
                    "counter" => CabinetPart::Counter,
                    "pull" => CabinetPart::Pull,
                    _ => return None,
                };
                Some(Surface::Cabinet { cabinet: ArchId(labelled(c, "cb")?), part })
            }
            ["fixture", f] => Some(Surface::Fixture(ArchId(labelled(f, "fx")?))),
            ["furnishing", f, part] => Some(Surface::Furnishing { furnishing: ArchId(labelled(f, "f")?), part: number(part)? }),
            ["countertop", c] => Some(Surface::Countertop(ArchId(labelled(c, "ct")?))),
            ["backsplash", b] => Some(Surface::Backsplash(ArchId(labelled(b, "bs")?))),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_keys_round_trip() {
        let all = [
            (Surface::WallSide { wall: ArchId(3), side: Side::Left }, "wall/w3/left", "wall/w3"),
            (Surface::WallSide { wall: ArchId(3), side: Side::Right }, "wall/w3/right", "wall/w3"),
            (Surface::RoomFloor(ArchId(0)), "room/r0/floor", "room/r0"),
            (Surface::RoomCeiling(ArchId(0)), "room/r0/ceiling", "room/r0"),
            (Surface::Roof(ArchId(0)), "roof/r0", "roof/r0"),
            (Surface::Cabinet { cabinet: ArchId(0), part: CabinetPart::Carcass }, "cabinet/cb0/carcass", "cabinet/cb0"),
            (Surface::Cabinet { cabinet: ArchId(1), part: CabinetPart::Front }, "cabinet/cb1/front", "cabinet/cb1"),
            (Surface::Cabinet { cabinet: ArchId(1), part: CabinetPart::Counter }, "cabinet/cb1/counter", "cabinet/cb1"),
            (Surface::Cabinet { cabinet: ArchId(1), part: CabinetPart::Pull }, "cabinet/cb1/pull", "cabinet/cb1"),
            (Surface::Fixture(ArchId(0)), "fixture/fx0", "fixture/fx0"),
            (Surface::Furnishing { furnishing: ArchId(2), part: 0 }, "furnishing/f2/0", "furnishing/f2"),
            (Surface::Countertop(ArchId(0)), "countertop/ct0", "countertop/ct0"),
            (Surface::Backsplash(ArchId(2)), "backsplash/bs2", "backsplash/bs2"),
        ];
        for (s, key, obj) in all {
            assert_eq!(s.key(), key);
            assert_eq!(s.object_key(), obj);
            assert_eq!(Surface::parse(key), Some(s));
        }
    }

    #[test]
    fn surface_parse_refuses_garbage() {
        for bad in [
            "",
            "wall",
            "wall/w3",
            "wall/w3/top",
            "wall/3/left",
            "wall/w/left",
            "wall/w+3/left",
            "wall/w03/left",
            "wall/w3/left/x",
            "wall/w3/left ",
            "room/w0/floor",
            "room/r0/wall",
            "roof/r0/x",
            "roof/0",
            "cabinet/cb0",
            "cabinet/cb0/door",
            "cabinet/r0/front",
            "fixture/f0",
            "fixture/fx0/x",
            "furnishing/f2",
            "furnishing/f2/x",
            "furnishing/f2/-1",
            "furnishing/fx2/0",
            "furnishing/f99999999999/0",
            "countertop/cb0",
            "backsplash/bs",
            "backsplash/bs0/x",
            "chair/c0",
        ] {
            assert_eq!(Surface::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn material_ref_parses_short_and_key_forms() {
        assert_eq!(MaterialRef::parse("siding"), Some(MaterialRef::Finish(Finish::Siding)));
        assert_eq!(MaterialRef::parse("finish:siding").map(|m| m.key()), Some("finish:siding".into()));
        assert_eq!(MaterialRef::parse("material-oak"), Some(MaterialRef::Item("material-oak".into())));
        assert_eq!(MaterialRef::parse("item:material-oak").map(|m| m.key()), Some("item:material-oak".into()));
        let c = MaterialRef::parse("#C04020");
        assert_eq!(c, Some(MaterialRef::Colour([0xc0, 0x40, 0x20])));
        assert_eq!(c.map(|m| m.key()), Some("colour:#c04020".into()));
        assert_eq!(MaterialRef::parse("colour:#c04020"), Some(MaterialRef::Colour([0xc0, 0x40, 0x20])));
        assert_eq!(MaterialRef::parse(""), None);
        assert_eq!(MaterialRef::parse("#12345"), None);
        assert_eq!(MaterialRef::parse("#1234567"), None);
        assert_eq!(MaterialRef::parse("#12345g"), None);
        assert_eq!(MaterialRef::parse("item:"), None);
    }
}
