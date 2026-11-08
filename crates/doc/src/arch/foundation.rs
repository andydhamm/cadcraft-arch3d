//! Foundations (phase 16): stem walls on footings, or slabs on grade with a thickened edge.
//!
//! Plain data on [`super::ArchModel`]; the 3D solids live in `crates/arch3d`, the plan linework in
//! `crates/render`, the command in `crates/engine`.

use super::ArchId;
use crate::Handle;
use serde::{Deserialize, Serialize};

/// Offset within the arch-handle range reserved for foundation proxy handles (label `fd{n}`).
pub const FOUNDATION_HANDLE_OFFSET: u64 = 0x88000;
/// Most foundations in a drawing.
pub const MAX_FOUNDATIONS: usize = 64;
/// The layer footings are drawn on.
pub const FOUNDATION_LAYER: &str = "S-FNDN";
/// How far a footing's bottom sits below the level's floor elevation, by default (m).
pub const DEFAULT_FOOTING_DEPTH: f64 = 1.2;
/// A stem wall's thickness (m).
pub const STEM_THICKNESS: f64 = 0.2;
/// A footing's width (m).
pub const FOOTING_WIDTH: f64 = 0.6;
/// A footing's thickness (m).
pub const FOOTING_THICKNESS: f64 = 0.25;
/// A slab on grade's thickness (m).
pub const SLAB_THICKNESS: f64 = 0.1;
/// The width of a slab's thickened edge, inward from the outer face (m).
pub const SLAB_EDGE_WIDTH: f64 = 0.3;
/// How far a slab's thickened edge reaches below the slab's top (m).
pub const SLAB_EDGE_DEPTH: f64 = 0.3;
/// The wall type a basement's exterior walls take.
pub const CONCRETE_WALL_TYPE: &str = "Concrete 0.2";

/// The kind of foundation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FoundationKind {
    /// A concrete stem wall on a footing under every exterior wall on grade.
    #[default]
    Stem,
    /// The level's floor structure becomes a slab on grade with a thickened edge.
    Slab,
}

/// A foundation under one level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Foundation {
    pub id: ArchId,
    /// The level it supports.
    pub level: u32,
    pub kind: FoundationKind,
    /// How far the footing's bottom sits below the level's floor elevation (m).
    pub footing_depth: f64,
    pub layer: String,
}

impl Default for Foundation {
    fn default() -> Self {
        Foundation {
            id: ArchId(0),
            level: 0,
            kind: FoundationKind::default(),
            footing_depth: DEFAULT_FOOTING_DEPTH,
            layer: FOUNDATION_LAYER.to_string(),
        }
    }
}

/// The handle of foundation `id`.
pub fn foundation_handle(id: ArchId) -> Handle {
    Handle(super::ARCH_HANDLE_BASE.saturating_add(FOUNDATION_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the foundation handle range.
pub fn is_foundation_handle(h: Handle) -> bool {
    let lo = super::ARCH_HANDLE_BASE.saturating_add(FOUNDATION_HANDLE_OFFSET);
    h.0 >= lo && h.0 < lo.saturating_add(1u64 << 15)
}

/// `fd{n}`.
pub fn foundation_label(id: ArchId) -> String {
    format!("fd{}", id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foundation_defaults_and_kind_keys() {
        let f = Foundation::default();
        assert_eq!((f.kind, f.footing_depth, f.layer.as_str()), (FoundationKind::Stem, 1.2, "S-FNDN"));
        assert_eq!(serde_json::to_string(&FoundationKind::Stem).unwrap(), "\"stem\"");
        assert_eq!(serde_json::to_string(&FoundationKind::Slab).unwrap(), "\"slab\"");
        let back: Foundation = serde_json::from_str("{\"kind\":\"slab\"}").unwrap();
        assert_eq!(back, Foundation { kind: FoundationKind::Slab, ..Foundation::default() });
    }

    #[test]
    fn foundation_handles_and_labels() {
        let id = ArchId(3);
        assert!(is_foundation_handle(foundation_handle(id)));
        assert!(!is_foundation_handle(super::super::soffit_handle(id)));
        assert_eq!(foundation_label(id), "fd3");
    }
}
