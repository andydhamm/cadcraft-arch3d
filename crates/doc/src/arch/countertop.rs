//! Countertops and backsplashes (phase 18). Plain data; outlines live in `arch::countertop`.
//!
//! Lengths are metres. A countertop's polygon is counter-clockwise; edge `i` runs from
//! `polygon[i]` to `polygon[(i + 1) % n]`.

use super::ArchId;
use crate::Handle;
use cadcraft_geom::Vec2;
use serde::{Deserialize, Serialize};

/// Offset within the arch-handle range reserved for countertops (label `ct{n}`).
pub const COUNTERTOP_HANDLE_OFFSET: u64 = 0xC0000;
/// Offset within the arch-handle range reserved for backsplashes (label `bs{n}`).
pub const BACKSPLASH_HANDLE_OFFSET: u64 = 0xC8000;
/// Most countertops in a drawing.
pub const MAX_COUNTERTOPS: usize = 512;
/// Most backsplashes in a drawing.
pub const MAX_BACKSPLASHES: usize = 512;
/// Most vertices in a countertop polygon.
pub const MAX_COUNTER_VERTICES: usize = 256;
/// Default backsplash height.
pub const DEFAULT_BACKSPLASH_HEIGHT: f64 = 0.45;
/// Default backsplash thickness.
pub const BACKSPLASH_THICKNESS: f64 = 0.01;
/// Default layer of countertops and backsplashes.
pub const COUNTER_LAYER: &str = "A-FLOR-CASE";

/// The edge treatment of a countertop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeProfile {
    /// Square.
    #[default]
    Square,
    /// Eased.
    Eased,
    /// Beveled.
    Bevel,
    /// Bullnose.
    Bullnose,
}

impl EdgeProfile {
    /// The lower-case key.
    pub fn key(self) -> &'static str {
        match self {
            EdgeProfile::Square => "square",
            EdgeProfile::Eased => "eased",
            EdgeProfile::Bevel => "bevel",
            EdgeProfile::Bullnose => "bullnose",
        }
    }

    /// Parse an exact lower-case key.
    pub fn parse(s: &str) -> Option<EdgeProfile> {
        match s {
            "square" => Some(EdgeProfile::Square),
            "eased" => Some(EdgeProfile::Eased),
            "bevel" => Some(EdgeProfile::Bevel),
            "bullnose" => Some(EdgeProfile::Bullnose),
            _ => None,
        }
    }
}

/// One edge of a countertop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CounterEdge {
    /// Arc sagitta; > 0 bulges outward of the polygon.
    pub sagitta: f64,
    /// Edge treatment.
    pub profile: EdgeProfile,
    /// Whether a waterfall panel drops from this edge.
    pub waterfall: bool,
}

/// A free countertop.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Countertop {
    /// Id.
    pub id: ArchId,
    /// Level index.
    pub level: u32,
    /// Counter-clockwise chord vertices.
    pub polygon: Vec<Vec2>,
    /// Per-edge settings (missing = default).
    pub edges: Vec<CounterEdge>,
    /// The work surface above the level floor.
    pub top: f64,
    /// Slab thickness.
    pub thickness: f64,
    /// Cabinets it holds.
    pub cabinets: Vec<ArchId>,
    /// Layer.
    pub layer: String,
}

impl Default for Countertop {
    fn default() -> Self {
        Countertop {
            id: ArchId::default(),
            level: 0,
            polygon: Vec::new(),
            edges: Vec::new(),
            top: 0.914,
            thickness: 0.038,
            cabinets: Vec::new(),
            layer: COUNTER_LAYER.to_string(),
        }
    }
}

impl Countertop {
    /// Edge `i`, or the default when missing.
    pub fn edge(&self, i: usize) -> CounterEdge {
        self.edges.get(i).copied().unwrap_or_default()
    }
}

/// What a backsplash follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CounterRef {
    /// The automatic counter holding that cabinet.
    Cabinet(ArchId),
    /// A free countertop.
    Countertop(ArchId),
}

impl Default for CounterRef {
    fn default() -> Self {
        CounterRef::Cabinet(ArchId(0))
    }
}

/// A backsplash.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Backsplash {
    /// Id.
    pub id: ArchId,
    /// Level index.
    pub level: u32,
    /// The counter it follows.
    pub counter: Option<CounterRef>,
    /// The wall it is on.
    pub wall: Option<ArchId>,
    /// Height above the counter.
    pub height: f64,
    /// Run to the ceiling instead.
    pub to_ceiling: bool,
    /// Thickness.
    pub thickness: f64,
    /// Layer.
    pub layer: String,
}

impl Default for Backsplash {
    fn default() -> Self {
        Backsplash {
            id: ArchId::default(),
            level: 0,
            counter: None,
            wall: None,
            height: DEFAULT_BACKSPLASH_HEIGHT,
            to_ceiling: false,
            thickness: BACKSPLASH_THICKNESS,
            layer: COUNTER_LAYER.to_string(),
        }
    }
}

/// The handle of countertop `id`.
pub fn countertop_handle(id: ArchId) -> Handle {
    Handle(super::ARCH_HANDLE_BASE.saturating_add(COUNTERTOP_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the countertop handle range.
pub fn is_countertop_handle(h: Handle) -> bool {
    let lo = super::ARCH_HANDLE_BASE.saturating_add(COUNTERTOP_HANDLE_OFFSET);
    h.0 >= lo && h.0 < lo.saturating_add(1u64 << 15)
}

/// `ct{n}`.
pub fn countertop_label(id: ArchId) -> String {
    format!("ct{}", id.0)
}

/// The handle of backsplash `id`.
pub fn backsplash_handle(id: ArchId) -> Handle {
    Handle(super::ARCH_HANDLE_BASE.saturating_add(BACKSPLASH_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the backsplash handle range.
pub fn is_backsplash_handle(h: Handle) -> bool {
    let lo = super::ARCH_HANDLE_BASE.saturating_add(BACKSPLASH_HANDLE_OFFSET);
    h.0 >= lo && h.0 < lo.saturating_add(1u64 << 15)
}

/// `bs{n}`.
pub fn backsplash_label(id: ArchId) -> String {
    format!("bs{}", id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countertop_defaults() {
        let c = Countertop::default();
        assert_eq!(c.top, 0.914);
        assert_eq!(c.thickness, 0.038);
        assert_eq!(c.layer, COUNTER_LAYER);
        let c: Countertop = serde_json::from_str("{}").unwrap();
        assert_eq!(c, Countertop::default());
    }

    #[test]
    fn countertop_edge_defaults_when_missing() {
        let mut c = Countertop::default();
        c.edges.push(CounterEdge { sagitta: 0.1, profile: EdgeProfile::Bevel, waterfall: true });
        assert_eq!(c.edge(0).profile, EdgeProfile::Bevel);
        assert_eq!(c.edge(7), CounterEdge::default());
        for p in [EdgeProfile::Square, EdgeProfile::Eased, EdgeProfile::Bevel, EdgeProfile::Bullnose] {
            assert_eq!(EdgeProfile::parse(p.key()), Some(p));
        }
    }

    #[test]
    fn backsplash_defaults() {
        let b = Backsplash::default();
        assert_eq!(b.height, 0.45);
        assert_eq!(b.thickness, 0.01);
        assert_eq!(b.layer, COUNTER_LAYER);
        assert!(b.counter.is_none() && !b.to_ceiling);
    }

    #[test]
    fn counter_ref_serde() {
        let r = CounterRef::Countertop(ArchId(3));
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(serde_json::from_str::<CounterRef>(&j).unwrap(), r);
        assert!(j.contains("countertop"));
        let j = serde_json::to_string(&CounterRef::Cabinet(ArchId(2))).unwrap();
        assert!(j.contains("cabinet"));
        assert_eq!(CounterRef::default(), CounterRef::Cabinet(ArchId(0)));
    }

    #[test]
    fn handles_ct_bs_disjoint_from_every_other_kind() {
        use crate::arch::*;
        let ct = countertop_handle(ArchId(9));
        let bs = backsplash_handle(ArchId(9));
        let ml = molding_line_handle(ArchId(9));
        assert!(is_countertop_handle(ct) && !is_backsplash_handle(ct) && !is_molding_line_handle(ct));
        assert!(is_backsplash_handle(bs) && !is_countertop_handle(bs));
        assert!(!is_cabinet_handle(ct) && !is_cabinet_handle(bs) && !is_cabinet_handle(ml));
        assert!(!is_fixture_handle(ct) && !is_fixture_handle(bs));
        assert!(!is_countertop_handle(cabinet_handle(ArchId(9))));
        assert_eq!(countertop_label(ArchId(2)), "ct2");
        assert_eq!(backsplash_label(ArchId(2)), "bs2");
    }
}
