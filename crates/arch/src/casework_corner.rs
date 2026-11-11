//! Corner cabinets (phase 18). Stubs: the corners segment fills them; until then no cabinet is in a
//! corner, counters are not merged and there is no corner symbol.

use crate::countertop::CounterOutline;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchId, Cabinet};
use cadcraft_geom::Vec2;

/// Which width end of the cabinet is in the corner: `u = -w/2` or `+w/2`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CornerEnd {
    /// `u = -w/2`.
    #[default]
    Start,
    /// `u = +w/2`.
    End,
}

/// How a corner cabinet turns the corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CornerStyle {
    /// Blind corner.
    #[default]
    Blind,
    /// Diagonal front.
    Diagonal,
    /// Lazy susan.
    LazySusan,
}

/// A cabinet standing in a corner.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CornerInfo {
    /// The end in the corner.
    pub end: CornerEnd,
    /// The style.
    pub style: CornerStyle,
    /// Distance from this cabinet's back line, along the other wall, to the other run's nearest cabinet.
    pub leg: f64,
    /// That cabinet's depth.
    pub other_depth: f64,
    /// That cabinet.
    pub partner: ArchId,
}

/// The corner `c` stands in, if any.
#[allow(unused_variables)]
pub fn corner_info(model: &ArchModel, c: &Cabinet) -> Option<CornerInfo> {
    None
}

/// Merges the outlines of runs that meet at a corner.
#[allow(unused_variables)]
pub fn merge_corners(model: &ArchModel, outlines: Vec<CounterOutline>) -> Vec<CounterOutline> {
    outlines
}

/// The plan symbol of a corner cabinet.
#[allow(unused_variables)]
pub fn corner_symbol(model: &ArchModel, c: &Cabinet) -> Option<Vec<Vec<Vec2>>> {
    None
}
