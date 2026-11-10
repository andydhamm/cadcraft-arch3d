//! Where a stair's flight cuts a wall's top: the wall-under-flight segment fills this in.

use cadcraft_arch::flight_cut::FlightCut;

use crate::walls::WallProfile;

/// Lower `profile` (a wall standing on a level at `base`) under the `cuts`. For now: nothing.
pub(crate) fn apply(_profile: &mut WallProfile, _cuts: &[FlightCut], _base: f64) {}
