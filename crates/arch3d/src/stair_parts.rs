//! A stair's meshes: the part builders of the stair segments fill this in.

use cadcraft_doc::arch::Stair;

use crate::mesh::Mesh;
use crate::stairs;

/// Every mesh of `stair` standing on a level at `base`. `against` says which sides ([left, right])
/// stand against a wall. For now: today's flight solid and landing slab.
pub(crate) fn stair_meshes(stair: &Stair, base: f64, _against: [bool; 2]) -> Vec<Mesh> {
    stairs::stair_mesh(stair, base).into_iter().chain(stairs::landing_mesh(stair, base)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_geom::Vec2;

    /// The phase-5 stair: start (2, 1), +x, width 1.0, level 0 to 1 at 2.7.
    fn phase5_stair() -> Stair {
        Stair {
            start: Vec2::new(2.0, 1.0),
            width: 1.0,
            rise: 2.7,
            risers: 16,
            riser_height: 0.16875,
            treads: 15,
            tread_depth: 0.254,
            run: 3.81,
            ..Default::default()
        }
    }

    #[test]
    fn stub_matches_the_legacy_solid() {
        let s = phase5_stair();
        let got = stair_meshes(&s, 0.0, [false, false]);
        let want: Vec<Mesh> = stairs::stair_mesh(&s, 0.0).into_iter().chain(stairs::landing_mesh(&s, 0.0)).collect();
        assert!(!got.is_empty());
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(g.positions, w.positions);
            assert_eq!(g.indices, w.indices);
            assert_eq!(g.material, w.material);
        }
    }
}
