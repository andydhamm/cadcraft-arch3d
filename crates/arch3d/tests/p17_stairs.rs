//! Phase 17 stair flight: probes into a built scene (treads, risers, stringers, skirts).

use cadcraft_arch3d::Scene3d;
use cadcraft_doc::arch::{Level, Stair, Stringers};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::{Vec2, Vec3};

fn model(stringers: Stringers) -> ArchModel {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
    m.levels.push(Level { index: 1, elevation: 2.7, ceiling_height: 2.4, ..Default::default() });
    m.stairs.push(Stair {
        id: ArchId(200),
        start: Vec2::new(2.0, 1.0),
        width: 1.0,
        from_level: 0,
        to_level: 1,
        rise: 2.7,
        risers: 16,
        riser_height: 0.16875,
        treads: 15,
        tread_depth: 0.254,
        run: 3.81,
        stringers,
        ..Default::default()
    });
    m
}

fn part(s: &Scene3d, x: f64, y: f64, z: f64) -> Option<String> {
    s.probe(Vec3::new(x, y, z)).map(|h| h.part)
}

#[test]
fn stringer_probes() {
    let s = Scene3d::build(&model(Stringers::Two));
    assert_eq!(part(&s, 2.6, 0.52, 0.3).as_deref(), Some("stringer"));
    assert_eq!(part(&s, 2.6, 0.52, 0.25), None);
    assert_eq!(part(&s, 2.6, 0.52, 0.66), None);
    assert_eq!(part(&s, 2.6, 1.0, 0.3), None);
}

#[test]
fn centre_stringer_probes() {
    let s = Scene3d::build(&model(Stringers::Centre));
    assert_eq!(part(&s, 2.6, 1.0, 0.3).as_deref(), Some("stringer"));
    assert_eq!(part(&s, 2.6, 1.0, 0.4), None);
    assert_eq!(part(&s, 2.6, 0.7, 0.3), None);
}
