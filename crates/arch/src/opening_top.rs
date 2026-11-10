//! Validation of an opening's shaped top.

use cadcraft_doc::arch::Opening;
use cadcraft_geom::Vec2;

/// Check a shaped top and, for a roof top, set `o.height`. `roof_top` is in the host wall's
/// profile coordinates measured from the wall's bottom (`u` along the wall from its start,
/// `v = z - base`); `o.sill` is measured from that bottom, so it is the outline's lower edge.
// STUB (owner p17-window-tops): accepts everything until the checks are built.
pub fn validate(_o: &mut Opening, _roof_top: Option<&[Vec2]>) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_accepts_a_square_opening() {
        let mut o = Opening::default();
        assert!(validate(&mut o, None).is_ok());
    }
}
