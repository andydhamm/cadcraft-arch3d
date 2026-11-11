//! Phase 17 scene plumbing: the shared kit changes no scene yet. The phase-1 house and the phase-4
//! window have no stair, railing or deck, which no phase-17 segment changes, so their digest stays.
//! The digest was recorded from arch3d's head before the plumbing: the stubs build today's meshes
//! and the new part names only apply to the new materials.

use cadcraft_arch3d::Scene3d;
use cadcraft_doc::arch::{Opening, OpeningKind};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::Vec2;

/// The phase-1 house: four exterior walls 0.2 m thick on a 10 x 8 m rectangle, two interior walls.
fn house() -> ArchModel {
    let mut m = ArchModel::default();
    let wall = |a: (f64, f64), b: (f64, f64), thickness: f64, exterior: bool| Wall {
        start: Vec2::new(a.0, a.1),
        end: Vec2::new(b.0, b.1),
        thickness,
        height: 2.4,
        exterior,
        ..Default::default()
    };
    for (a, b) in [((0.0, 0.0), (10.0, 0.0)), ((10.0, 0.0), (10.0, 8.0)), ((10.0, 8.0), (0.0, 8.0)), ((0.0, 8.0), (0.0, 0.0))] {
        m.push_wall(wall(a, b, 0.2, true));
    }
    m.push_wall(wall((0.0, 3.0), (10.0, 3.0), 0.1, false));
    m.push_wall(wall((6.0, 3.0), (6.0, 8.0), 0.1, false));
    m
}

/// The house with the phase-4 window in its first wall.
fn house_and_window() -> ArchModel {
    let mut m = house();
    m.push_opening(Opening::new(ArchId(0), 4.0, 1.2, 1.2, OpeningKind::Window));
    m
}

#[test]
fn golden_house_and_window_unchanged() {
    let s = Scene3d::build(&house_and_window());
    assert_eq!(s.digest(), "1f150171e6be39af");
}
