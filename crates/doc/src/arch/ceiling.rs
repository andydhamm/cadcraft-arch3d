//! Ceiling trays and interior soffits (phase 16).
//!
//! Plain data on [`super::ArchModel`]; the ceiling queries live in `crates/arch`, the meshes in
//! `crates/arch3d`, the commands in `crates/engine`.

use super::ArchId;
use crate::Handle;
use cadcraft_geom::Vec2;
use serde::{Deserialize, Serialize};

/// Offset within the arch-handle range reserved for tray proxy handles (label `tr{n}`).
pub const TRAY_HANDLE_OFFSET: u64 = 0x78000;
/// Offset within the arch-handle range reserved for soffit proxy handles (label `sf{n}`).
pub const SOFFIT_HANDLE_OFFSET: u64 = 0x80000;
/// Most trays in a drawing.
pub const MAX_TRAYS: usize = 256;
/// Deepest tray nesting (a tray inside a tray inside …).
pub const MAX_TRAY_NESTING: usize = 4;
/// Most soffits in a drawing.
pub const MAX_SOFFITS: usize = 1024;
/// The layer trays and soffits are drawn on.
pub const CEILING_LAYER: &str = "A-CLNG";

/// How a tray's sides rise from the ceiling around it to the tray's top.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TraySides {
    /// Plumb sides.
    #[default]
    Vertical,
    /// Sides pitched at this angle, in degrees above horizontal.
    Pitched(f64),
}

/// A tray: the part of a room's ceiling recessed `depth` up into the attic.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tray {
    pub id: ArchId,
    /// The level of the room.
    pub level: u32,
    /// The room whose ceiling the tray recesses.
    pub room: ArchId,
    /// The tray this one nests in, if any.
    pub parent: Option<ArchId>,
    /// The tray's outline; empty = `inset` in from the room (or from the parent tray).
    pub polygon: Vec<Vec2>,
    /// How far the outline sits in from the room's walls or the parent tray's outline (m).
    pub inset: f64,
    /// How much higher the tray is than the ceiling around it (m).
    pub depth: f64,
    pub sides: TraySides,
    pub layer: String,
}

impl Default for Tray {
    fn default() -> Self {
        Tray {
            id: ArchId(0),
            level: 0,
            room: ArchId(0),
            parent: None,
            polygon: Vec::new(),
            inset: 0.0,
            depth: 0.3,
            sides: TraySides::default(),
            layer: CEILING_LAYER.to_string(),
        }
    }
}

/// An interior soffit: a box hung from the ceiling over a plan outline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Soffit {
    pub id: ArchId,
    pub level: u32,
    /// The plan outline.
    pub polygon: Vec<Vec2>,
    /// The bottom's height above the level's floor (m), when given.
    pub bottom: Option<f64>,
    /// The box's depth below the finished ceiling (m), when given.
    pub depth: Option<f64>,
    /// The wall cabinets the soffit fills over (from their tops to the ceiling).
    pub over: Vec<ArchId>,
    /// The material key of the box's faces, if not the default.
    pub material: Option<String>,
    pub layer: String,
}

impl Default for Soffit {
    fn default() -> Self {
        Soffit {
            id: ArchId(0),
            level: 0,
            polygon: Vec::new(),
            bottom: None,
            depth: None,
            over: Vec::new(),
            material: None,
            layer: CEILING_LAYER.to_string(),
        }
    }
}

/// The handle of tray `id`.
pub fn tray_handle(id: ArchId) -> Handle {
    Handle(super::ARCH_HANDLE_BASE.saturating_add(TRAY_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the tray handle range.
pub fn is_tray_handle(h: Handle) -> bool {
    let lo = super::ARCH_HANDLE_BASE.saturating_add(TRAY_HANDLE_OFFSET);
    h.0 >= lo && h.0 < lo.saturating_add(1u64 << 15)
}

/// `tr{n}`.
pub fn tray_label(id: ArchId) -> String {
    format!("tr{}", id.0)
}

/// The handle of soffit `id`.
pub fn soffit_handle(id: ArchId) -> Handle {
    Handle(super::ARCH_HANDLE_BASE.saturating_add(SOFFIT_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the soffit handle range.
pub fn is_soffit_handle(h: Handle) -> bool {
    let lo = super::ARCH_HANDLE_BASE.saturating_add(SOFFIT_HANDLE_OFFSET);
    h.0 >= lo && h.0 < lo.saturating_add(1u64 << 15)
}

/// `sf{n}`.
pub fn soffit_label(id: ArchId) -> String {
    format!("sf{}", id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_and_soffit_defaults() {
        let t = Tray::default();
        assert_eq!((t.depth, t.inset, t.layer.as_str(), t.sides), (0.3, 0.0, "A-CLNG", TraySides::Vertical));
        assert!(t.polygon.is_empty() && t.parent.is_none());
        let s = Soffit::default();
        assert_eq!(s.layer, "A-CLNG");
        assert!(s.bottom.is_none() && s.depth.is_none() && s.over.is_empty() && s.material.is_none());
    }

    #[test]
    fn tray_sides_serialize_in_camel_case() {
        assert_eq!(serde_json::to_string(&TraySides::Vertical).unwrap(), "\"vertical\"");
        assert_eq!(serde_json::to_string(&TraySides::Pitched(45.0)).unwrap(), "{\"pitched\":45.0}");
        let back: TraySides = serde_json::from_str("{\"pitched\":30.0}").unwrap();
        assert_eq!(back, TraySides::Pitched(30.0));
    }

    #[test]
    fn a_tray_from_an_empty_object_takes_the_defaults() {
        let t: Tray = serde_json::from_str("{}").unwrap();
        assert_eq!(t, Tray::default());
        let s: Soffit = serde_json::from_str("{}").unwrap();
        assert_eq!(s, Soffit::default());
    }

    #[test]
    fn tray_and_soffit_handles_and_labels() {
        let id = ArchId(9);
        assert!(is_tray_handle(tray_handle(id)) && !is_soffit_handle(tray_handle(id)));
        assert!(is_soffit_handle(soffit_handle(id)) && !is_tray_handle(soffit_handle(id)));
        assert_eq!(tray_label(id), "tr9");
        assert_eq!(soffit_label(id), "sf9");
        assert!(is_tray_handle(tray_handle(ArchId(32767))));
        assert!(!is_tray_handle(tray_handle(ArchId(32768))));
    }
}
