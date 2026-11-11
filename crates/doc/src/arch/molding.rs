//! Moldings (phase 18): room base, crown and chair moldings, free molding lines and the cabinet
//! crown and light-rail moldings. Plain data; the geometry lives in `arch3d`.
//!
//! Lengths are metres. A profile is a closed cross-section: x out from the surface it is fixed
//! to, y up, bounding box starting at (0, 0), counter-clockwise, no repeated closing point.

use super::ArchId;
use crate::Handle;
use cadcraft_geom::Vec2;
use serde::{Deserialize, Serialize};

/// Offset within the arch-handle range reserved for molding lines (label `ml{n}`).
pub const MOLDING_LINE_HANDLE_OFFSET: u64 = 0xD0000;
/// Most molding lines in a drawing.
pub const MAX_MOLDING_LINES: usize = 1_024;
/// Most moldings on one room.
pub const MAX_ROOM_MOLDINGS: usize = 8;
/// Most points in a molding profile.
pub const MAX_PROFILE_POINTS: usize = 256;
/// Default layer of molding lines.
pub const MOLDING_LAYER: &str = "A-FLOR-TRIM";

/// Where a room molding sits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MoldingKind {
    /// At the floor.
    #[default]
    Base,
    /// At the ceiling.
    Crown,
    /// A rail on the wall.
    Chair,
}

impl MoldingKind {
    /// The lower-case key.
    pub fn key(self) -> &'static str {
        match self {
            MoldingKind::Base => "base",
            MoldingKind::Crown => "crown",
            MoldingKind::Chair => "chair",
        }
    }

    /// Parse an exact lower-case key.
    pub fn parse(s: &str) -> Option<MoldingKind> {
        match s {
            "base" => Some(MoldingKind::Base),
            "crown" => Some(MoldingKind::Crown),
            "chair" => Some(MoldingKind::Chair),
            _ => None,
        }
    }
}

/// A molding cross-section.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MoldingProfile {
    /// The catalogue item it came from.
    pub item: String,
    /// The closed outline in metres.
    pub points: Vec<[f64; 2]>,
}

impl MoldingProfile {
    /// `[max x, max y]` over the finite points (`[0, 0]` when none).
    pub fn size(&self) -> [f64; 2] {
        let mut s = [0.0f64, 0.0f64];
        for p in &self.points {
            if !(p[0].is_finite() && p[1].is_finite()) {
                continue;
            }
            s[0] = s[0].max(p[0]);
            s[1] = s[1].max(p[1]);
        }
        s
    }
}

/// One molding on a room.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RoomMolding {
    /// Base, crown or chair.
    pub kind: MoldingKind,
    /// Its cross-section.
    pub profile: MoldingProfile,
    /// Base and chair: height of the bottom above the floor; crown: drop of the top below the
    /// finished ceiling.
    pub offset: f64,
}

/// A molding run along a free path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MoldingLine {
    /// Id.
    pub id: ArchId,
    /// Level index.
    pub level: u32,
    /// The path.
    pub points: Vec<Vec2>,
    /// Whether the path closes.
    pub closed: bool,
    /// The profile's bottom above the level floor.
    pub elevation: f64,
    /// Sideways shift of the path, positive to the left of travel.
    pub inset: f64,
    /// Profile x to the right of the path.
    pub flip: bool,
    /// Cross-section.
    pub profile: MoldingProfile,
    /// Layer.
    pub layer: String,
}

impl Default for MoldingLine {
    fn default() -> Self {
        MoldingLine {
            id: ArchId::default(),
            level: 0,
            points: Vec::new(),
            closed: false,
            elevation: 0.0,
            inset: 0.0,
            flip: false,
            profile: MoldingProfile::default(),
            layer: MOLDING_LAYER.to_string(),
        }
    }
}

/// A molding on a cabinet (crown or light rail).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CabinetMolding {
    /// Cross-section.
    pub profile: MoldingProfile,
    /// Overriding `[width, height]`, if any.
    pub size: Option<[f64; 2]>,
}

/// The handle of molding line `id`.
pub fn molding_line_handle(id: ArchId) -> Handle {
    Handle(super::ARCH_HANDLE_BASE.saturating_add(MOLDING_LINE_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the molding line handle range.
pub fn is_molding_line_handle(h: Handle) -> bool {
    let lo = super::ARCH_HANDLE_BASE.saturating_add(MOLDING_LINE_HANDLE_OFFSET);
    h.0 >= lo && h.0 < lo.saturating_add(1u64 << 15)
}

/// `ml{n}`.
pub fn molding_line_label(id: ArchId) -> String {
    format!("ml{}", id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn molding_kind_keys_round_trip() {
        for k in [MoldingKind::Base, MoldingKind::Crown, MoldingKind::Chair] {
            assert_eq!(MoldingKind::parse(k.key()), Some(k));
            let j = serde_json::to_string(&k).unwrap();
            assert_eq!(j, format!("\"{}\"", k.key()));
            assert_eq!(serde_json::from_str::<MoldingKind>(&j).unwrap(), k);
        }
        assert_eq!(MoldingKind::parse("Base"), None);
    }

    #[test]
    fn molding_profile_size_is_the_bounding_box() {
        assert_eq!(MoldingProfile::default().size(), [0.0, 0.0]);
        let p = MoldingProfile { item: String::new(), points: vec![[0.0, 0.0], [0.02, 0.0], [0.01, 0.09], [f64::NAN, 5.0]] };
        assert_eq!(p.size(), [0.02, 0.09]);
    }

    #[test]
    fn molding_line_handles_and_labels() {
        let h = molding_line_handle(ArchId(5));
        assert!(is_molding_line_handle(h));
        assert!(!is_molding_line_handle(super::super::light_handle(ArchId(5))));
        assert_eq!(molding_line_label(ArchId(5)), "ml5");
        assert_eq!(MoldingLine::default().layer, MOLDING_LAYER);
    }
}
