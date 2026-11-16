//! A wall that carries a window (a through hole) and a niche (a recess) keeps its whole face.
//!
//! The wall's profile gets one hole per opening, each bridged into the outline before the caps are
//! triangulated. Two holes whose nearest outline vertex is the same corner used to bridge from the
//! wrong copy of that corner (the first bridge doubles it), which folded the ring over itself and
//! left a diagonal band of the face without triangles.

use cadcraft_arch3d::Scene3d;
use cadcraft_doc::arch::{Level, Opening, OpeningKind, Side};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::Vec2;

/// The outer face of the north wall (centre line y = 16, 0.2 thick).
const OUTER_Y: f64 = 16.1;
/// Its inner face.
const INNER_Y: f64 = 15.9;

/// A 26 x 16 house on one level at elevation 0, walls 2.7 high; returns the model and the id of
/// its north wall, drawn from (26, 16) to (0, 16) as the showcase's is.
fn house() -> (ArchModel, ArchId) {
    let mut model = ArchModel::default();
    model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
    let pts = [(0.0, 0.0), (26.0, 0.0), (26.0, 16.0), (0.0, 16.0), (0.0, 0.0)];
    let mut ids = Vec::new();
    for pair in pts.windows(2) {
        ids.push(model.push_wall(Wall {
            start: Vec2::new(pair[0].0, pair[0].1),
            end: Vec2::new(pair[1].0, pair[1].1),
            thickness: 0.2,
            height: 2.7,
            exterior: true,
            ..Default::default()
        }));
    }
    let north = ids.get(2).copied().unwrap_or(ArchId(3));
    (model, north)
}

/// The showcase's window: offset 5 along the wall (x = 21), 1.2 x 1.2, sill 1.0.
fn window(wall: ArchId) -> Opening {
    let mut op = Opening::new(wall, 5.0, 1.2, 1.2, OpeningKind::Window);
    op.sill = 1.0;
    op
}

/// The showcase's niche: offset 0.8 (x = 25.2), 0.6 x 0.9, sill 1.0, 0.1 deep from the inner face.
fn niche(wall: ArchId) -> Opening {
    let mut op = Opening::new(wall, 0.8, 0.6, 0.9, OpeningKind::Niche);
    op.sill = 1.0;
    op.depth = 0.1;
    op.face = Some(Side::Left);
    op
}

/// Where a ray along the y axis through (x, z) first meets a triangle of a mesh owned by `wall`:
/// travelling south from y = 30 (`from_north`) or north from y = 8. `None` when it meets none.
fn first_hit_y(scene: &Scene3d, wall: ArchId, x: f64, z: f64, from_north: bool) -> Option<f64> {
    let mut best: Option<f64> = None;
    for mesh in scene.meshes.iter().filter(|m| m.object == wall) {
        for tri in mesh.indices.chunks(3) {
            let [Some(a), Some(b), Some(c)] = [0, 1, 2].map(|k| tri.get(k).and_then(|&i| mesh.positions.get(i as usize))) else {
                continue;
            };
            let p = |v: &[f32; 3]| (f64::from(v[0]), f64::from(v[1]), f64::from(v[2]));
            let (a, b, c) = (p(a), p(b), p(c));
            // Barycentric coordinates of (x, z) in the triangle projected on the x-z plane.
            let det = (b.0 - a.0) * (c.2 - a.2) - (c.0 - a.0) * (b.2 - a.2);
            if det.abs() < 1e-12 {
                continue;
            }
            let u = ((x - a.0) * (c.2 - a.2) - (c.0 - a.0) * (z - a.2)) / det;
            let v = ((b.0 - a.0) * (z - a.2) - (x - a.0) * (b.2 - a.2)) / det;
            if u < -1e-9 || v < -1e-9 || u + v > 1.0 + 1e-9 {
                continue;
            }
            let y = a.1 + u * (b.1 - a.1) + v * (c.1 - a.1);
            if from_north {
                if y <= 30.0 && best.is_none_or(|h| y > h) {
                    best = Some(y);
                }
            } else if y >= 8.0 && best.is_none_or(|h| y < h) {
                best = Some(y);
            }
        }
    }
    best
}

/// The (x, z) sample points of the wall's face, 0.5 m apart in x and 0.25 m in z, clear of the
/// wall's ends, top and bottom.
fn face_grid() -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for i in 0..51 {
        for j in 0..11 {
            out.push((0.5 + 0.5 * f64::from(i), 0.1 + 0.25 * f64::from(j)));
        }
    }
    out
}

/// Whether (x, z) is in or within 0.15 m of the window's rough opening (x 20.4..21.6, z 1.0..2.2).
fn near_window(x: f64, z: f64) -> bool {
    (20.25..=21.75).contains(&x) && (0.85..=2.35).contains(&z)
}

/// The sample points where a ray from outside does not land on the wall's outer face.
fn outer_face_gaps(scene: &Scene3d, wall: ArchId, has_window: bool) -> Vec<(f64, f64)> {
    face_grid()
        .into_iter()
        .filter(|&(x, z)| !(has_window && near_window(x, z)))
        .filter(|&(x, z)| !first_hit_y(scene, wall, x, z, true).is_some_and(|y| (y - OUTER_Y).abs() < 1e-3))
        .collect()
}

/// Triangles of the meshes owned by `wall`.
fn triangles(scene: &Scene3d, wall: ArchId) -> usize {
    scene.meshes.iter().filter(|m| m.object == wall).map(|m| m.indices.len() / 3).sum()
}

#[test]
fn wall_with_window_and_niche_has_no_gaps_in_its_face() {
    let (mut model, north) = house();
    model.push_opening(window(north));
    model.push_opening(niche(north));
    let scene = Scene3d::build(&model);
    let gaps = outer_face_gaps(&scene, north, true);
    assert!(gaps.is_empty(), "{} of the outer face's sample points are not covered, e.g. {:?}", gaps.len(), &gaps[..gaps.len().min(8)]);
    // The inner face too, away from both openings.
    for (x, z) in face_grid().into_iter().filter(|&(x, z)| !near_window(x, z) && !((24.7..=25.7).contains(&x) && (0.8..=2.1).contains(&z))) {
        let y = first_hit_y(&scene, north, x, z, false);
        assert!(y.is_some_and(|y| (y - INNER_Y).abs() < 1e-3), "inner face at ({x}, {z}): {y:?}");
    }
    // The same two openings in the other order give the same face.
    let (mut model, north) = house();
    model.push_opening(niche(north));
    model.push_opening(window(north));
    let gaps = outer_face_gaps(&Scene3d::build(&model), north, true);
    assert!(gaps.is_empty(), "niche first: {} sample points are not covered, e.g. {:?}", gaps.len(), &gaps[..gaps.len().min(8)]);
}

#[test]
fn niche_recess_and_window_hole_are_still_there() {
    let (mut model, north) = house();
    model.push_opening(window(north));
    model.push_opening(niche(north));
    let scene = Scene3d::build(&model);
    // Through the window's glass: no wall triangle either way.
    for z in [1.2, 1.6, 2.0] {
        assert_eq!(first_hit_y(&scene, north, 21.0, z, true), None, "the window is open at z = {z}");
        assert_eq!(first_hit_y(&scene, north, 21.0, z, false), None, "the window is open at z = {z} (from inside)");
    }
    // From inside, the niche's back is 0.1 m behind the inner face (less its 1 mm skin); beside it
    // the wall is at the inner face; from outside the niche does not show.
    for z in [1.15, 1.45, 1.75] {
        let back = first_hit_y(&scene, north, 25.2, z, false);
        assert!(back.is_some_and(|y| (y - (INNER_Y + 0.1)).abs() < 2e-3), "niche back at z = {z}: {back:?}");
        let beside = first_hit_y(&scene, north, 24.5, z, false);
        assert!(beside.is_some_and(|y| (y - INNER_Y).abs() < 1e-3), "beside the niche at z = {z}: {beside:?}");
        let outside = first_hit_y(&scene, north, 25.2, z, true);
        assert!(outside.is_some_and(|y| (y - OUTER_Y).abs() < 1e-3), "outside the niche at z = {z}: {outside:?}");
    }
}

#[test]
fn wall_with_only_a_niche_is_unchanged() {
    let (mut model, north) = house();
    model.push_opening(niche(north));
    let scene = Scene3d::build(&model);
    assert_eq!(outer_face_gaps(&scene, north, false), Vec::new(), "the outer face is whole");
    let back = first_hit_y(&scene, north, 25.2, 1.45, false);
    assert!(back.is_some_and(|y| (y - (INNER_Y + 0.1)).abs() < 2e-3), "niche back: {back:?}");
    assert_eq!(triangles(&scene, north), NICHE_ONLY_TRIANGLES, "the niche-only wall's triangle count");
}

#[test]
fn wall_with_only_a_window_is_unchanged() {
    let (mut model, north) = house();
    model.push_opening(window(north));
    let scene = Scene3d::build(&model);
    assert_eq!(outer_face_gaps(&scene, north, true), Vec::new(), "the outer face is whole round the window");
    assert_eq!(first_hit_y(&scene, north, 21.0, 1.6, true), None, "the window is open");
    assert_eq!(triangles(&scene, north), WINDOW_ONLY_TRIANGLES, "the window-only wall's triangle count");
}

/// Triangle counts of the single-opening walls, as built before the two-hole fix.
const NICHE_ONLY_TRIANGLES: usize = 56;
const WINDOW_ONLY_TRIANGLES: usize = 32;

/// A door: offset 2 along the wall (x = 24), 0.9 x 2.1, sill 0.
fn door(wall: ArchId) -> Opening {
    Opening::new(wall, 2.0, 0.9, 2.1, OpeningKind::Door)
}

/// Whether (x, z) is in or within 0.15 m of the door's rough opening (x 23.55..24.45, z 0..2.1).
fn near_door(x: f64, z: f64) -> bool {
    (23.4..=24.6).contains(&x) && (0.0..=2.25).contains(&z)
}

/// Whether (x, z) is in or within 0.15 m of the niche's rough opening (x 24.9..25.5, z 1.0..1.9).
fn near_niche(x: f64, z: f64) -> bool {
    (24.75..=25.65).contains(&x) && (0.85..=2.05).contains(&z)
}

#[test]
fn wall_with_three_openings_near_one_corner_has_no_gaps() {
    let (mut model, north) = house();
    model.push_opening(window(north));
    model.push_opening(niche(north));
    model.push_opening(door(north));
    let scene = Scene3d::build(&model);
    // Check the outer face for gaps, excluding all three openings.
    let gaps = face_grid()
        .into_iter()
        .filter(|&(x, z)| !near_window(x, z) && !near_niche(x, z) && !near_door(x, z))
        .filter(|&(x, z)| !first_hit_y(&scene, north, x, z, true).is_some_and(|y| (y - OUTER_Y).abs() < 1e-3))
        .collect::<Vec<_>>();
    assert!(gaps.is_empty(), "{} of the outer face's sample points are not covered, e.g. {:?}", gaps.len(), &gaps[..gaps.len().min(8)]);
    // Check the inner face too, excluding the niche (the window and door are through-holes).
    for (x, z) in face_grid().into_iter().filter(|&(x, z)| !near_window(x, z) && !near_niche(x, z)) {
        let y = first_hit_y(&scene, north, x, z, false);
        assert!(y.is_some_and(|y| (y - INNER_Y).abs() < 1e-3), "inner face at ({x}, {z}): {y:?}");
    }
}

#[test]
fn wall_with_window_and_niche_at_different_heights_has_no_gaps() {
    let (mut model, north) = house();
    let mut win = window(north);
    win.sill = 1.5;
    let mut ni = niche(north);
    ni.sill = 0.5;
    model.push_opening(win);
    model.push_opening(ni);
    let scene = Scene3d::build(&model);
    // The window is now at z 1.5..2.7, the niche at z 0.5..1.4.
    let near_high_window = |x: f64, z: f64| (20.25..=21.75).contains(&x) && (1.35..=2.85).contains(&z);
    let near_low_niche = |x: f64, z: f64| (24.75..=25.65).contains(&x) && (0.35..=1.55).contains(&z);
    let gaps = face_grid()
        .into_iter()
        .filter(|&(x, z)| !near_high_window(x, z) && !near_low_niche(x, z))
        .filter(|&(x, z)| !first_hit_y(&scene, north, x, z, true).is_some_and(|y| (y - OUTER_Y).abs() < 1e-3))
        .collect::<Vec<_>>();
    assert!(gaps.is_empty(), "{} of the outer face's sample points are not covered, e.g. {:?}", gaps.len(), &gaps[..gaps.len().min(8)]);
}

#[test]
fn wall_with_openings_near_both_corners_has_no_gaps() {
    let (mut model, north) = house();
    // Window and niche near the right corner (offsets 5 and 0.8).
    model.push_opening(window(north));
    model.push_opening(niche(north));
    // Window and niche near the left corner (offsets 21 and 25.2).
    let mut win2 = window(north);
    win2.offset = 21.0;
    let mut ni2 = niche(north);
    ni2.offset = 25.2;
    model.push_opening(win2);
    model.push_opening(ni2);
    let scene = Scene3d::build(&model);
    let near_left_window = |x: f64, z: f64| (4.25..=5.75).contains(&x) && (0.85..=2.35).contains(&z);
    let near_left_niche = |x: f64, z: f64| (0.35..=1.25).contains(&x) && (0.85..=2.05).contains(&z);
    let gaps = face_grid()
        .into_iter()
        .filter(|&(x, z)| !near_window(x, z) && !near_niche(x, z) && !near_left_window(x, z) && !near_left_niche(x, z))
        .filter(|&(x, z)| !first_hit_y(&scene, north, x, z, true).is_some_and(|y| (y - OUTER_Y).abs() < 1e-3))
        .collect::<Vec<_>>();
    assert!(gaps.is_empty(), "{} of the outer face's sample points are not covered, e.g. {:?}", gaps.len(), &gaps[..gaps.len().min(8)]);
}

#[test]
fn narrow_wall_with_window_and_niche_has_no_gaps() {
    // A 6 x 4 house: the north wall is only 6 m long.
    let mut model = ArchModel::default();
    model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
    let pts = [(0.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0), (0.0, 0.0)];
    let mut ids = Vec::new();
    for pair in pts.windows(2) {
        ids.push(model.push_wall(Wall {
            start: Vec2::new(pair[0].0, pair[0].1),
            end: Vec2::new(pair[1].0, pair[1].1),
            thickness: 0.2,
            height: 2.7,
            exterior: true,
            ..Default::default()
        }));
    }
    let north = ids.get(2).copied().unwrap_or(ArchId(3));
    // Window at offset 1 (x = 5), niche at offset 0.5 (x = 5.5).
    let mut win = Opening::new(north, 1.0, 1.2, 1.2, OpeningKind::Window);
    win.sill = 1.0;
    let mut ni = Opening::new(north, 0.5, 0.6, 0.9, OpeningKind::Niche);
    ni.sill = 1.0;
    ni.depth = 0.1;
    ni.face = Some(Side::Left);
    model.push_opening(win);
    model.push_opening(ni);
    let scene = Scene3d::build(&model);
    const OUTER_Y: f64 = 4.1;
    const INNER_Y: f64 = 3.9;
    let near_win = |x: f64, z: f64| (4.25..=5.75).contains(&x) && (0.85..=2.35).contains(&z);
    let near_ni = |x: f64, z: f64| (5.05..=5.95).contains(&x) && (0.85..=2.05).contains(&z);
    let mut grid = Vec::new();
    for i in 0..11 {
        for j in 0..11 {
            grid.push((0.1 + 0.5 * f64::from(i), 0.1 + 0.25 * f64::from(j)));
        }
    }
    let gaps = grid
        .into_iter()
        .filter(|&(x, z)| !near_win(x, z) && !near_ni(x, z))
        .filter(|&(x, z)| !first_hit_y(&scene, north, x, z, true).is_some_and(|y| (y - OUTER_Y).abs() < 1e-3))
        .collect::<Vec<_>>();
    assert!(gaps.is_empty(), "{} of the outer face's sample points are not covered, e.g. {:?}", gaps.len(), &gaps[..gaps.len().min(8)]);
}
