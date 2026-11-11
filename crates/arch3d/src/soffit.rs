//! Interior soffits (phase 16; owner: p16-soffits): boxes hung from the ceiling, and the material
//! definitions of their faces.
//!
//! Their meshes are `soffit-box` (or `soffit-box:<material key>`), part `soffit`; the roof's own
//! soffit (material `soffit`) stays part `roof`.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Level, MaterialRef, Soffit};
use cadcraft_geom::Vec2;

use crate::Scene3d;
use crate::mesh::Mesh;
use crate::planar::{PlaneZ, centroid, push_between, under_planes};

/// Everything a soffit's meshes are made from.
#[derive(Clone, Debug, Default, PartialEq)]
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
    let (Some(ring), Some(bottom)) = (i.ring.as_ref(), i.bottom) else {
        return Vec::new();
    };
    let material = match i.soffit.material.as_deref() {
        Some(s) => MaterialRef::parse(s).map_or("soffit-box".to_string(), |m| format!("soffit-box:{}", m.key())),
        None => "soffit-box".to_string(),
    };
    let mut mesh = Mesh::new(&material, i.soffit.id);
    mesh.level_elevation = i.level.elevation;
    for (cell, plane) in &i.cells {
        if centroid(cell).is_some_and(|c| plane.z_at(c) > bottom) {
            push_between(&mut mesh, cell, &[], PlaneZ::flat(bottom), *plane);
        }
    }
    if mesh.positions.is_empty() { Vec::new() } else { vec![mesh] }
}

/// Add the material definitions of the soffits' faces to `scene`.
pub(crate) fn materials(model: &ArchModel, scene: &mut Scene3d) {
    for soffit in &model.soffits {
        let Some(material_str) = soffit.material.as_deref() else { continue };
        let Some(m) = MaterialRef::parse(material_str) else { continue };
        let key = format!("soffit-box:{}", m.key());
        scene.materials.entry(key).or_insert_with(|| crate::paint::ref_def(model, &m));
    }
}

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

    #[test]
    fn soffit_input_with_nan_in_polygon() {
        let m = model();
        let poly = vec![Vec2::new(1.0, 1.0), Vec2::new(2.0, 1.0), Vec2::new(f64::NAN, 2.0), Vec2::new(1.0, 2.0)];
        let s = Soffit { level: 0, polygon: poly, bottom: Some(2.0), ..Default::default() };
        let i = soffit_input(&m, &s);
        // A NaN point should either give no ring or empty cells; never a panic or a non-finite cell.
        assert!(i.ring.is_none() || i.cells.is_empty());
        assert!(i.cells.iter().all(|(poly, plane)| poly.iter().all(|p| p.x.is_finite() && p.y.is_finite()) && plane.is_finite()));
    }

    #[test]
    fn soffit_input_multiple_rooms_different_ceiling_heights() {
        let mut m = model();
        let mut room2 = Room { polygon: rect(4.0, 0.0, 8.0, 3.0), ..Default::default() };
        room2.ceiling_height = 3.0;
        m.push_room(room2);

        let s = Soffit { level: 0, polygon: rect(2.0, 1.0, 6.0, 2.0), bottom: Some(2.0), ..Default::default() };
        let i = soffit_input(&m, &s);
        assert!(i.ring.is_some());
        assert!(i.cells.len() >= 2);
        if i.cells.len() >= 2 {
            assert!((i.cells[0].1.c - i.cells[1].1.c).abs() > 1e-9);
        }
    }

    #[test]
    fn soffit_input_bottom_above_ceiling_still_gives_cells() {
        let m = model();
        let s = Soffit { level: 0, polygon: rect(1.0, 1.0, 2.0, 2.0), bottom: Some(3.0), ..Default::default() };
        let i = soffit_input(&m, &s);
        assert!(i.ring.is_some());
        assert_eq!(i.cells.len(), 1);
        assert!((i.cells[0].1.c - 2.4).abs() < 1e-9);
    }

    #[test]
    fn soffit_input_huge_coordinates() {
        let mut m = ArchModel::default();
        let huge = 1e300;
        m.push_room(Room { polygon: rect(0.0, 0.0, huge, huge), ..Default::default() });
        let s =
            Soffit { level: 0, polygon: rect(huge / 2.0, huge / 2.0, huge / 2.0 + 1.0, huge / 2.0 + 1.0), bottom: Some(2.0), ..Default::default() };
        let i = soffit_input(&m, &s);
        assert!(i.ring.as_ref().is_some_and(|r| r.iter().all(|p| p.x.is_finite() && p.y.is_finite())));
        assert!(i.cells.iter().all(|(_, plane)| plane.is_finite()));
    }
}
