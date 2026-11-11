//! Room molding runs (phase 18): the resolved paths a room's moldings follow.

use cadcraft_doc::arch::{ArchModel, MoldingKind, MoldingProfile, Room};
use cadcraft_geom::Vec2;

/// One continuous run of a room molding along the surface its back is fixed to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MoldingRun {
    /// Which molding it is.
    pub kind: MoldingKind,
    /// The index of the molding in the room's `moldings`.
    pub index: usize,
    /// The cross-section.
    pub profile: MoldingProfile,
    /// Plan points on the surface the profile's back is fixed to.
    pub path: Vec<Vec2>,
    /// Absolute elevation of the profile's BOTTOM at each path vertex.
    pub z: Vec<f64>,
    /// Whether the path closes on itself.
    pub closed: bool,
    /// `+1`: the profile's x is to the left of travel, `-1`: to the right.
    pub side: f64,
}

/// The molding runs of `room`.
///
/// Stub: owner p18-room-moldings.
pub fn room_runs(_model: &ArchModel, _room: &Room) -> Vec<MoldingRun> {
    Vec::new()
}

/// The length of a run: the sum of its segment lengths, plus the closing segment when closed;
/// 0 when any point is not finite.
pub fn run_length(r: &MoldingRun) -> f64 {
    if r.path.iter().any(|p| !p.is_finite()) {
        return 0.0;
    }
    let mut total: f64 = r.path.windows(2).map(|w| w[0].dist(w[1])).sum();
    if r.closed && r.path.len() > 2 {
        if let (Some(first), Some(last)) = (r.path.first(), r.path.last()) {
            total += last.dist(*first);
        }
    }
    if total.is_finite() { total } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_length_open_closed_and_hostile() {
        let sq = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)];
        let mut r = MoldingRun { path: sq, ..Default::default() };
        assert!((run_length(&r) - 3.0).abs() < 1e-12);
        r.closed = true;
        assert!((run_length(&r) - 4.0).abs() < 1e-12);
        r.path.push(Vec2::new(f64::NAN, 0.0));
        assert_eq!(run_length(&r), 0.0);
    }
}
