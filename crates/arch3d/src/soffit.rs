//! Interior soffits (phase 16; owner: p16-soffits): boxes hung from the ceiling, and the material
//! definitions of their faces.
//!
//! Their meshes are `soffit-box` (or `soffit-box:<material key>`), part `soffit`; the roof's own
//! soffit (material `soffit`) stays part `roof`.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Level, Soffit};
use cadcraft_geom::Vec2;

use crate::Scene3d;
use crate::mesh::Mesh;
use crate::planar::{PlaneZ, under_planes};

/// Everything a soffit's meshes are made from.
#[derive(Clone, Debug, Default, PartialEq)]
#[allow(dead_code)] // the meshes land in p16-soffit-meshes
pub(crate) struct SoffitIn {
    pub soffit: Soffit,
    pub level: Level,
    /// The soffit's plan ring, `None` when it has none.
    pub ring: Option<Vec<Vec2>>,
    /// The absolute z of the soffit's underside, `None` when it has none.
    pub bottom: Option<f64>,
    /// The parts of the ring, each under the plane of the finished ceiling above it.
    pub cells: Vec<(Vec<Vec2>, PlaneZ)>,
}

/// The inputs of soffit `s` in `model`.
pub(crate) fn soffit_input(model: &ArchModel, s: &Soffit) -> SoffitIn {
    let level = crate::level_for(model, s.level);
    let ring = cadcraft_arch::ceiling::soffit::soffit_ring(model, s);
    let bottom = cadcraft_arch::ceiling::soffit::soffit_bottom(model, s);
    let mut cells = Vec::new();
    if let (Some(ring), Some(b)) = (&ring, bottom)
        && b.is_finite()
    {
        let mut planes: Vec<(Vec<Vec2>, PlaneZ)> = Vec::new();
        for room in model.rooms.iter().filter(|r| r.level == s.level) {
            let input = crate::ceiling::ceiling_input(model, room, &level, Vec::new());
            for piece in crate::ceiling::finished_pieces(&input) {
                if piece.plane.is_finite() {
                    planes.push((piece.outer, piece.plane));
                }
            }
        }
        let under = under_planes(ring, &[], &planes);
        // `under_planes` triangulates the ring; when every triangle hangs under one plane the ring
        // is a single cell.
        let one_plane = match under.first() {
            Some((_, Some(first))) => under.iter().all(|(_, p)| p.as_ref() == Some(first)).then_some(*first),
            _ => None,
        };
        cells = match one_plane {
            Some(plane) => vec![(ring.clone(), plane)],
            None => under.into_iter().filter_map(|(poly, plane)| plane.map(|p| (poly, p))).collect(),
        };
    }
    SoffitIn { soffit: s.clone(), level, ring, bottom, cells }
}

/// The meshes of one soffit.
pub(crate) fn soffit_meshes(i: &SoffitIn) -> Vec<Mesh> {
    let _inputs = (&i.soffit, &i.level); // read once soffits are built
    Vec::new()
}

/// Add the material definitions of the soffits' faces to `scene`.
pub(crate) fn materials(_model: &ArchModel, _scene: &mut Scene3d) {}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::Room;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    fn model() -> ArchModel {
        let mut m = ArchModel::default();
        m.push_room(Room { polygon: rect(0.0, 0.0, 4.0, 3.0), ..Default::default() });
        m
    }

    #[test]
    fn soffit_input_cells_follow_the_finished_ceiling() {
        let mut m = model();
        let s = Soffit { level: 0, polygon: rect(1.0, 1.0, 2.0, 2.0), bottom: Some(2.0), ..Default::default() };
        let i = soffit_input(&m, &s);
        assert!(i.ring.as_ref().is_some_and(|r| r.len() == 4 && r.iter().all(|p| p.x.is_finite() && p.y.is_finite())));
        assert_eq!(i.bottom, Some(2.0));
        assert_eq!(i.cells.len(), 1);
        assert!(i.cells[0].1.is_flat() && (i.cells[0].1.c - 2.4).abs() < 1e-9);

        m.rooms[0].ceiling_stack = vec![cadcraft_doc::arch::CeilingLayer::new("drywall", Some("gypsum"), 0.0127)];
        let i = soffit_input(&m, &s);
        assert_eq!(i.cells.len(), 1);
        assert!((i.cells[0].1.c - 2.3873).abs() < 1e-9);
    }

    #[test]
    fn soffit_input_without_a_ring_or_bottom_is_empty() {
        let m = model();
        let i = soffit_input(&m, &Soffit { level: 0, ..Default::default() });
        assert!(i.ring.is_none() && i.bottom.is_none() && i.cells.is_empty());

        let s = Soffit { level: 0, polygon: rect(1.0, 1.0, 2.0, 2.0), bottom: None, depth: None, over: vec![], ..Default::default() };
        let i = soffit_input(&m, &s);
        assert!(i.bottom.is_none() && i.cells.is_empty());

        let s = Soffit { level: 0, polygon: rect(10.0, 10.0, 11.0, 11.0), bottom: Some(2.0), ..Default::default() };
        assert!(soffit_input(&m, &s).cells.is_empty());
    }
}
