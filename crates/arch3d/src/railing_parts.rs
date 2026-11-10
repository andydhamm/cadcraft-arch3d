//! A railing's meshes: the railing segment fills this in.

use cadcraft_arch::railing::RailSurface;
use cadcraft_doc::arch::Railing;

use crate::mesh::Mesh;
use crate::railing;

/// Every mesh of `r` standing on `surface`. For now: today's single railing mesh, at the flat
/// floor, the deck top or the base of the flight.
pub(crate) fn railing_meshes(r: &Railing, surface: &RailSurface) -> Vec<Mesh> {
    let z = match surface {
        RailSurface::Flat { z } => *z,
        RailSurface::Deck { top, .. } => *top,
        RailSurface::Flight { line } => line.base,
    };
    railing::railing_mesh(r, z).into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_geom::Vec2;

    #[test]
    fn stub_matches_the_legacy_mesh() {
        let r = Railing { path: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)], ..Default::default() };
        let got = railing_meshes(&r, &RailSurface::Flat { z: 0.3 });
        let want: Vec<Mesh> = railing::railing_mesh(&r, 0.3).into_iter().collect();
        assert_eq!(got.len(), 1);
        assert_eq!(got.len(), want.len());
        assert_eq!(got[0].positions, want[0].positions);
        assert_eq!(got[0].indices, want[0].indices);
        assert_eq!(got[0].material, "railing");
    }
}
