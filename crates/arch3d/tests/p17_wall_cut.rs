//! Phase 17: a wall that runs under a stair flight stops at the flight's underside.

use cadcraft_arch::flight_cut::flight_cuts;
use cadcraft_arch3d::{ArchId, Mesh, Scene3d, SceneCache};
use cadcraft_doc::arch::{Level, Opening, OpeningKind, Roof, RoofPlane, Stair};
use cadcraft_doc::{ArchModel, Wall};
use cadcraft_geom::Vec2;

fn stair(start: Vec2, width: f64) -> Stair {
    Stair {
        start,
        direction: 0.0,
        width,
        from_level: 0,
        to_level: 1,
        rise: 2.7,
        risers: 16,
        riser_height: 0.16875,
        treads: 15,
        tread_depth: 0.254,
        run: 3.81,
        ..Default::default()
    }
}

fn base_model() -> ArchModel {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
    m.levels.push(Level { index: 1, elevation: 2.7, ceiling_height: 2.4, ..Default::default() });
    m
}

fn wall(a: (f64, f64), b: (f64, f64)) -> Wall {
    Wall { start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness: 0.1, height: 2.7, ..Default::default() }
}

/// Odd-even upward ray test against the wall's meshes: is `(x, y, z)` inside the wall?
fn solid(scene: &Scene3d, id: ArchId, x: f64, y: f64, z: f64) -> bool {
    let meshes: Vec<&Mesh> = scene.meshes.iter().filter(|m| m.object == id && m.material == "wall").collect();
    let mut count = 0;
    for mesh in meshes {
        for t in mesh.indices.chunks_exact(3) {
            let pt = |i: u32| mesh.positions.get(i as usize).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]);
            let (Some(a), Some(b), Some(c)) = (pt(t[0]), pt(t[1]), pt(t[2])) else { continue };
            let den = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            if den.abs() < 1e-12 {
                continue;
            }
            // A hair off the probe point keeps the ray off triangulation diagonals.
            let (rx, ry) = (x + 1.3e-5 - a[0], y + 2.9e-5 - a[1]);
            let s = (rx * (c[1] - a[1]) - ry * (c[0] - a[0])) / den;
            let u = (ry * (b[0] - a[0]) - rx * (b[1] - a[1])) / den;
            if s < -1e-6 || u < -1e-6 || s + u > 1.0 + 1e-6 {
                continue;
            }
            if a[2] + s * (b[2] - a[2]) + u * (c[2] - a[2]) > z + 1e-9 {
                count += 1;
            }
        }
    }
    count % 2 == 1
}

fn brief() -> (ArchModel, ArchId) {
    let mut m = base_model();
    let w = m.push_wall(wall((3.0, 0.0), (3.0, 3.0)));
    m.push_stair(stair(Vec2::new(1.0, 1.0), 0.9));
    (m, w)
}

#[test]
fn brief_wall_is_cut_at_1_2141() {
    let (m, w) = brief();
    let s = Scene3d::build(&m);
    assert!(solid(&s, w, 3.0, 1.0, 1.15));
    assert!(!solid(&s, w, 3.0, 1.0, 1.27));
    assert!(solid(&s, w, 3.0, 2.5, 2.0));
    assert!(solid(&s, w, 3.0, 0.4, 2.0));
}

#[test]
fn underside_0_5_lowers_it_to_1_0141() {
    let (mut m, w) = brief();
    m.stairs[0].underside = 0.5;
    let s = Scene3d::build(&m);
    assert!(!solid(&s, w, 3.0, 1.0, 1.05));
    assert!(solid(&s, w, 3.0, 1.0, 0.95));
}

#[test]
fn a_wall_along_the_flight_gets_a_sloped_cut() {
    let mut m = base_model();
    let w = m.push_wall(wall((0.0, 1.0), (6.0, 1.0)));
    m.push_stair(stair(Vec2::new(1.0, 1.0), 0.9));
    let cuts = flight_cuts(&m, &m.walls[0]);
    assert_eq!(cuts.len(), 1);
    let top = &cuts[0].top;
    // Floor-hugging start, a breakpoint, then the rising underside.
    assert!(top.len() >= 3, "{top:?}");
    assert!((top[0].x - 0.975).abs() < 1e-6 && (top[top.len() - 1].x - 4.81).abs() < 1e-6, "{top:?}");
    let s = Scene3d::build(&m);
    // x = 2.5 (u 1.5): underside 0.882; x = 4.5 (u 3.5): underside 2.21.
    assert!(solid(&s, w, 2.5, 1.0, 0.5));
    assert!(!solid(&s, w, 2.5, 1.0, 1.0));
    assert!(solid(&s, w, 4.5, 1.0, 2.0));
    assert!(!solid(&s, w, 4.5, 1.0, 2.4));
    // Beyond the flight the wall keeps its height.
    assert!(solid(&s, w, 5.5, 1.0, 2.5));
}

#[test]
fn walls_on_other_levels_and_dividers_are_not_cut() {
    let mut m = base_model();
    m.push_stair(stair(Vec2::new(1.0, 1.0), 0.9));
    let mut up = wall((3.0, 0.0), (3.0, 3.0));
    up.base_level = 1;
    let mut div = wall((3.0, 0.0), (3.0, 3.0));
    div.divider = true;
    assert!(flight_cuts(&m, &up).is_empty());
    assert!(flight_cuts(&m, &div).is_empty());
    assert_eq!(flight_cuts(&m, &wall((3.0, 0.0), (3.0, 3.0))).len(), 1);
}

#[test]
fn a_wall_beside_the_flight_is_not_cut() {
    let mut m = base_model();
    m.push_stair(stair(Vec2::new(1.0, 1.0), 0.9));
    assert!(flight_cuts(&m, &wall((3.0, 1.6), (3.0, 4.0))).is_empty());
    assert!(flight_cuts(&m, &wall((0.0, 0.0), (0.0, 3.0))).is_empty());
    assert!(flight_cuts(&m, &wall((6.0, 0.0), (6.0, 3.0))).is_empty());
}

#[test]
fn hostile_stairs_and_walls_give_no_cut_and_no_panic() {
    let (mut m, _) = brief();
    let good = m.walls[0].clone();
    for bad in [f64::NAN, f64::INFINITY, -1.0, 0.0, 1e300] {
        let mut s = m.stairs[0].clone();
        s.width = bad;
        s.riser_height = bad;
        s.start = Vec2::new(bad, bad);
        let mut h = m.clone();
        h.stairs[0] = s;
        let _ = flight_cuts(&h, &good);
        let _ = Scene3d::build(&h);
        let mut w = good.clone();
        w.start = Vec2::new(bad, 0.0);
        let _ = flight_cuts(&m, &w);
        let mut hw = m.clone();
        hw.walls[0] = w;
        let _ = Scene3d::build(&hw);
    }
    let mut zero = good.clone();
    zero.end = zero.start;
    assert!(flight_cuts(&m, &zero).is_empty());
    let mut nan = good.clone();
    nan.end = Vec2::new(f64::NAN, 1.0);
    assert!(flight_cuts(&m, &nan).is_empty());
    // Many stairs: capped per wall.
    for _ in 0..100 {
        m.push_stair(stair(Vec2::new(1.0, 1.0), 0.9));
    }
    assert_eq!(flight_cuts(&m, &good).len(), 64);
    let _ = Scene3d::build(&m);
}

#[test]
fn wall_profile_is_notched() {
    let (m, w) = brief();
    let s = Scene3d::build(&m);
    // Notched over the flight (y 0.55..1.45): the wall's top steps down, the sides are untouched.
    for y in [0.6, 1.0, 1.4] {
        assert!(solid(&s, w, 3.0, y, 1.0) && !solid(&s, w, 3.0, y, 1.3), "y {y}");
    }
    for y in [0.5, 1.5] {
        assert!(solid(&s, w, 3.0, y, 2.5), "y {y}");
    }
}

#[test]
fn the_notch_keeps_door_notches_and_drops_holes_it_cuts_into() {
    let (mut m, w) = brief();
    // A door at y 0.1..0.7 reaching into the flight zone, a window at y ~1.05..1.55 above the cut.
    let mut door = Opening::new(w, 0.4, 0.6, 2.0, OpeningKind::Door);
    door.sill = 0.0;
    m.push_opening(door);
    let mut win = Opening::new(w, 1.3, 0.5, 1.0, OpeningKind::Window);
    win.sill = 1.0;
    m.push_opening(win);
    let s = Scene3d::build(&m);
    // The door notch stays.
    assert!(!solid(&s, w, 3.0, 0.3, 1.0));
    assert!(solid(&s, w, 3.0, 0.3, 2.3));
    assert!(!solid(&s, w, 3.0, 0.65, 1.0));
    // The window hole would poke out of the cut wall, so it is dropped and the wall stands there.
    assert!(solid(&s, w, 3.0, 1.3, 1.1));
    assert!(!solid(&s, w, 3.0, 1.3, 1.3));
}

#[test]
fn gable_wall_under_a_flight_keeps_its_rake() {
    let mut m = base_model();
    m.levels[0].ceiling_height = 2.4;
    for (a, b) in [((0.0, 0.0), (10.0, 0.0)), ((10.0, 0.0), (10.0, 8.0)), ((10.0, 8.0), (0.0, 8.0)), ((0.0, 8.0), (0.0, 0.0))] {
        let mut w = wall(a, b);
        w.thickness = 0.2;
        w.height = 2.4;
        w.exterior = true;
        m.push_wall(w);
    }
    let mut roof = Roof::default();
    roof.id = ArchId(100);
    roof.level = 0;
    roof.planes.push(RoofPlane {
        base_edge: [-0.1, -0.1, 10.1, -0.1],
        ring: vec![Vec2::new(-0.1, -0.1), Vec2::new(10.1, -0.1), Vec2::new(10.1, 4.0), Vec2::new(-0.1, 4.0)],
        slope: 0.5,
        eave_z: 2.4,
        thickness: 0.04,
        ..Default::default()
    });
    roof.planes.push(RoofPlane {
        base_edge: [-0.1, 8.1, 10.1, 8.1],
        ring: vec![Vec2::new(-0.1, 8.1), Vec2::new(-0.1, 4.0), Vec2::new(10.1, 4.0), Vec2::new(10.1, 8.1)],
        slope: 0.5,
        eave_z: 2.4,
        thickness: 0.04,
        ..Default::default()
    });
    m.roofs.push(roof);
    // A flight along the west gable wall (x = 0), its middle at y = 3: the rake above stays.
    let mut st = stair(Vec2::new(0.0, 0.5), 0.8);
    st.direction = std::f64::consts::FRAC_PI_2;
    m.push_stair(st);
    let west = m.walls[3].id;
    assert_eq!(flight_cuts(&m, &m.walls[3]).len(), 1);
    let s = Scene3d::build(&m);
    // The west wall runs (0,8) -> (0,0); y = 3 is u = 2.5 on the flight: underside ~1.5.
    assert!(solid(&s, west, 0.0, 6.0, 3.0), "the gable rake away from the flight stays");
    assert!(solid(&s, west, 0.0, 3.0, 0.5), "below the underside the wall stands");
    assert!(!solid(&s, west, 0.0, 3.0, 2.2), "under the flight the wall stops");
}

#[test]
fn cache_rebuilds_the_wall_when_the_stairs_underside_changes() {
    let (mut m, w) = brief();
    let (mut cache, _) = SceneCache::new(&m);
    assert!(solid(cache.scene(), w, 3.0, 1.0, 1.15));
    assert!(!solid(cache.scene(), w, 3.0, 1.0, 1.05 + 0.2));
    m.stairs[0].underside = 0.5;
    let stats = cache.update(&m);
    assert!(stats.rebuilt > 0);
    assert!(!solid(cache.scene(), w, 3.0, 1.0, 1.05));
    assert!(solid(cache.scene(), w, 3.0, 1.0, 0.95));
    let full = Scene3d::build(&m);
    for z in [0.95, 1.05, 1.3] {
        assert_eq!(solid(cache.scene(), w, 3.0, 1.0, z), solid(&full, w, 3.0, 1.0, z));
    }
}
