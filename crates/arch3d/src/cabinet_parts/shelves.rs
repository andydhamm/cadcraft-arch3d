//! Open shelving: the open carcass and the shelves.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Cabinet, Fronts};

use crate::Mesh;
use crate::casework::{level_elevation, push_box};

/// The carcass of a cabinet with open shelving, or `None` for a solid carcass.
pub fn open_carcass(model: &ArchModel, c: &Cabinet) -> Option<Mesh> {
    if c.fronts != Fronts::Open {
        return None;
    }
    let level_z = level_elevation(model, c.level) + c.elevation;
    let mut carcass = Mesh::new("cabinet-box", c.id);
    let y0 = 0.019;
    let y1 = c.depth - 0.019;
    let toe = if c.toe_kick { 0.1 } else { 0.0 };
    push_box(&mut carcass, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, 0.0, 0.019, level_z + toe, level_z + c.height);
    push_box(&mut carcass, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, y1, c.depth, level_z + toe, level_z + c.height);
    push_box(&mut carcass, c.position, c.rotation, -c.width / 2.0, -c.width / 2.0 + 0.019, y0, y1, level_z + toe, level_z + c.height);
    push_box(&mut carcass, c.position, c.rotation, c.width / 2.0 - 0.019, c.width / 2.0, y0, y1, level_z + toe, level_z + c.height);
    push_box(&mut carcass, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, y0, y1, level_z + toe, level_z + toe + 0.019);
    push_box(&mut carcass, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, y0, y1, level_z + c.height - 0.019, level_z + c.height);
    if carcass.indices.is_empty() { None } else { Some(carcass) }
}

/// The shelves of a cabinet.
pub fn shelf_meshes(model: &ArchModel, c: &Cabinet) -> Vec<Mesh> {
    if c.fronts != Fronts::Open {
        return Vec::new();
    }
    let mut out = Vec::new();
    // `shelf_levels` is measured from the level floor already (elevation and toe kick included).
    let floor = level_elevation(model, c.level);
    let levels = cadcraft_arch::casework_shelves::shelf_levels(c);
    if levels.is_empty() {
        return out;
    }
    let mut shelves = Mesh::new("cabinet-shelves", c.id);
    for (bottom, top) in levels {
        if top <= bottom {
            continue;
        }
        let (z0, z1) = (floor + bottom, floor + top);
        push_box(&mut shelves, c.position, c.rotation, -c.width / 2.0 + 0.019, c.width / 2.0 - 0.019, 0.019, c.depth - 0.019, z0, z1);
    }
    if !shelves.indices.is_empty() {
        out.push(shelves);
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::probe::probe_mesh;
    use cadcraft_doc::arch::Level;
    use cadcraft_geom::{Vec2, Vec3};

    fn model(level_z: f64) -> (ArchModel, Cabinet) {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, elevation: level_z, ..Default::default() });
        let c = Cabinet {
            fronts: Fronts::Open,
            width: 0.762,
            depth: 0.305,
            height: 0.762,
            elevation: 1.372,
            toe_kick: false,
            position: Vec2::new(0.0, 0.0),
            rotation: 0.0,
            ..Default::default()
        };
        (m, c)
    }

    fn check(level_z: f64) {
        let (m, c) = model(level_z);
        let levels = cadcraft_arch::casework_shelves::shelf_levels(&c);
        assert_eq!(levels.len(), 2);
        let shelves = shelf_meshes(&m, &c);
        let carcass = open_carcass(&m, &c);
        let at = |z: f64| Vec3::new(0.0, c.depth / 2.0, level_z + z);
        let mid = (levels[0].0 + levels[0].1) / 2.0;
        assert!(shelves.iter().any(|s| s.material == "cabinet-shelves" && probe_mesh(s, at(mid))));
        let between = (levels[0].1 + levels[1].0) / 2.0;
        assert!(!shelves.iter().any(|s| probe_mesh(s, at(between))));
        assert!(!carcass.iter().any(|s| probe_mesh(s, at(between))));
        for (b, t) in &levels {
            assert!(*b >= c.elevation + 0.019 - 1e-9 && *t <= c.elevation + c.height - 0.019 + 1e-9);
        }
    }

    #[test]
    fn open_shelves_are_built_at_their_reported_levels() {
        check(0.0);
    }

    #[test]
    fn open_shelves_follow_the_level_elevation() {
        check(2.7);
    }

    #[test]
    fn closed_cabinet_has_no_shelves_or_carcass() {
        let (m, c) = model(0.0);
        let closed = Cabinet { fronts: Fronts::Doors, ..c };
        assert!(shelf_meshes(&m, &closed).is_empty(), "closed cabinet should have no shelves");
        assert!(open_carcass(&m, &closed).is_none(), "closed cabinet should have no open carcass");
    }

    #[test]
    fn open_shelves_short_cabinet_has_no_shelves() {
        let (m, _) = model(0.0);
        let short = Cabinet {
            fronts: Fronts::Open,
            width: 0.762,
            depth: 0.305,
            height: 0.05,
            elevation: 0.0,
            toe_kick: false,
            position: Vec2::new(0.0, 0.0),
            rotation: 0.0,
            ..Default::default()
        };
        let shelves = shelf_meshes(&m, &short);
        assert!(shelves.is_empty(), "cabinet too short for shelves should have none");
    }

    #[test]
    fn open_shelves_with_toe_kick_stays_within_bounds() {
        let (m, _) = model(0.0);
        let with_toe = Cabinet {
            fronts: Fronts::Open,
            width: 0.762,
            depth: 0.305,
            height: 0.876,
            elevation: 0.0,
            toe_kick: true,
            position: Vec2::new(0.0, 0.0),
            rotation: 0.0,
            ..Default::default()
        };
        let shelves = shelf_meshes(&m, &with_toe);
        let levels = cadcraft_arch::casework_shelves::shelf_levels(&with_toe);
        for (b, t) in &levels {
            assert!(*b >= with_toe.elevation + 0.019 - 1e-9, "shelf bottom below carcass");
            assert!(*t <= with_toe.elevation + with_toe.height - 0.019 + 1e-9, "shelf top above carcass");
        }
        assert!(!shelves.is_empty(), "base cabinet with toe kick should have shelves");
    }

    #[test]
    fn open_shelves_zero_height_cabinet_has_no_shelves() {
        let (m, _) = model(0.0);
        let zero = Cabinet {
            fronts: Fronts::Open,
            width: 0.762,
            depth: 0.305,
            height: 0.0,
            elevation: 0.0,
            toe_kick: false,
            position: Vec2::new(0.0, 0.0),
            rotation: 0.0,
            ..Default::default()
        };
        let shelves = shelf_meshes(&m, &zero);
        assert!(shelves.is_empty(), "zero-height cabinet should have no shelves");
    }

    #[test]
    fn open_shelves_very_wide_cabinet_still_has_shelves() {
        let (m, _) = model(0.0);
        let wide = Cabinet {
            fronts: Fronts::Open,
            width: 3.0,
            depth: 0.305,
            height: 0.762,
            elevation: 0.0,
            toe_kick: false,
            position: Vec2::new(0.0, 0.0),
            rotation: 0.0,
            ..Default::default()
        };
        let shelves = shelf_meshes(&m, &wide);
        assert!(!shelves.is_empty(), "wide cabinet should have shelves");
        let levels = cadcraft_arch::casework_shelves::shelf_levels(&wide);
        assert_eq!(levels.len(), 2);
    }

    #[test]
    fn open_shelves_rotated_cabinet_meshes_correctly() {
        let (m, _) = model(0.0);
        let rotated = Cabinet {
            fronts: Fronts::Open,
            width: 0.762,
            depth: 0.305,
            height: 0.762,
            elevation: 0.0,
            toe_kick: false,
            position: Vec2::new(1.0, 1.0),
            rotation: std::f64::consts::PI / 2.0,
            ..Default::default()
        };
        let shelves = shelf_meshes(&m, &rotated);
        assert!(!shelves.is_empty(), "rotated cabinet should have shelves");
        assert_eq!(shelves.len(), 1);
    }
}
