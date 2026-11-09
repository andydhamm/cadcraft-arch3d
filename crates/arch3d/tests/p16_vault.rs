//! Phase 16 vaulted ceilings: a room with `flat_ceiling` off builds its ceiling under the roof
//! underside, re-pitched with the roof; a flat ceiling a lower roof cuts follows the underside
//! there; rooms under a higher roof keep phase 3's slab exactly.

use cadcraft_arch::ceiling::{roof_underside_at, rough_ceiling_in};
use cadcraft_arch3d::{Mesh, ObjectKey, ObjectKind, Scene3d, SceneCache, changed_objects};
use cadcraft_doc::arch::LightFixture;
use cadcraft_doc::{ArchId, ArchModel, Roof, RoofDirective, Room, Wall};
use cadcraft_geom::{Vec2, Vec3, point_in_polygon};

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
    vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
}

/// The phase-1 house: four exterior walls 2.4 m high on a 10 x 8 m rectangle, interior walls on
/// y = 3 and x = 6, rooms S (r0, 0.1..9.9 x 0.1..2.95), NW (r1) and NE (r2) on level 0 (ceiling
/// 2.4), under a gable of `rise` over 12 with gables on x = 0 and x = 10 (no roof for `None`).
fn house(rise: Option<f64>) -> ArchModel {
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
    m.push_room(Room { name: "South".into(), polygon: rect(0.1, 0.1, 9.9, 2.95), ..Default::default() });
    m.push_room(Room { name: "North-west".into(), polygon: rect(0.1, 3.05, 5.95, 7.9), ..Default::default() });
    m.push_room(Room { name: "North-east".into(), polygon: rect(6.05, 3.05, 9.9, 7.9), ..Default::default() });
    m.walls[1].roof_directive = RoofDirective::Gable;
    m.walls[3].roof_directive = RoofDirective::Gable;
    if let Some(rise) = rise {
        let seed = Roof { pitch_rise: rise, pitch_run: 12.0, overhang: 0.5, thickness: 0.2, ..Roof::default() };
        let built = cadcraft_arch::roof::build_roof(&m, 0, &seed).expect("the gable roof builds");
        m.push_roof(built);
    }
    m
}

/// The gable rebuilt with another rise, keeping its id (as `roofauto` and the wall-edit refresh do).
fn repitch(m: &mut ArchModel, rise: f64) {
    let seed = Roof { pitch_rise: rise, ..m.roofs[0].clone() };
    m.roofs[0] = cadcraft_arch::roof::build_roof(m, 0, &seed).expect("the gable roof builds");
}

fn ceiling_of(s: &Scene3d, room: u32) -> &Mesh {
    s.meshes.iter().find(|m| m.material == "ceiling" && m.object == ArchId(room)).expect("the room's ceiling mesh")
}

/// The highest point of `mesh` over plan point `p`: the largest z of its non-vertical triangles
/// whose plan shadow holds `p`; `None` where no triangle covers `p`.
fn top_at(mesh: &Mesh, p: Vec2) -> Option<f64> {
    let mut best: Option<f64> = None;
    for t in mesh.indices.chunks(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.positions[i as usize].map(f64::from));
        let det = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        if det.abs() < 1e-12 {
            continue; // a vertical side
        }
        let u = ((p.x - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (p.y - a[1])) / det;
        let v = ((b[0] - a[0]) * (p.y - a[1]) - (p.x - a[0]) * (b[1] - a[1])) / det;
        if u < -1e-9 || v < -1e-9 || u + v > 1.0 + 1e-9 {
            continue;
        }
        let z = a[2] + u * (b[2] - a[2]) + v * (c[2] - a[2]);
        best = Some(best.map_or(z, |q: f64| q.max(z)));
    }
    best
}

fn part_at(s: &Scene3d, x: f64, y: f64, z: f64) -> Option<(String, String)> {
    s.probe(Vec3::new(x, y, z)).map(|h| (h.id, h.part))
}

fn key(kind: ObjectKind, n: u32) -> ObjectKey {
    let mut k = ObjectKey { kind, ..Default::default() };
    k.id = ArchId(n);
    k
}

/// What a cached scene must match: the digest, every mesh's material and owner in order, and the
/// counts.
fn signature(s: &Scene3d) -> (String, Vec<(String, u32)>, [usize; 4]) {
    (s.digest(), s.meshes.iter().map(|m| (m.material.clone(), m.object.0)).collect(), [s.floors, s.ceilings, s.roofs, s.lights.len()])
}

/// Plan points of room `r` on a grid, off the cell edges.
fn grid(r: &Room) -> Vec<Vec2> {
    let mut out = Vec::new();
    for i in 0..25 {
        for j in 0..25 {
            let p = Vec2::new(0.0613 + 0.4013 * i as f64, 0.0719 + 0.3271 * j as f64);
            if point_in_polygon(&r.polygon, p) {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn vault_mesh_matches_the_point_query() {
    let mut m = house(Some(6.0));
    m.rooms[0].flat_ceiling = false;
    m.rooms[1].flat_ceiling = false; // NW straddles the ridge at y = 4
    let s = Scene3d::build(&m);
    for room in [0u32, 1] {
        let r = &m.rooms[room as usize];
        let mesh = ceiling_of(&s, room);
        let pts = grid(r);
        assert!(pts.len() > 40, "room {room}: {} points", pts.len());
        for p in pts {
            let want = rough_ceiling_in(&m, r, p).expect("a ceiling");
            let got = top_at(mesh, p).unwrap_or_else(|| panic!("room {room}: no ceiling over {p:?}"));
            assert!((got - want).abs() < 1e-5, "room {room} at {p:?}: mesh top {got}, point query {want}");
            let label = format!("r{room}");
            assert_eq!(part_at(&s, p.x, p.y, want - 0.01), Some((label, "ceiling".to_string())), "room {room} at {p:?}");
            assert_eq!(part_at(&s, p.x, p.y, want - 0.03), None, "under the 20 mm ceiling at {p:?}");
        }
    }
    // The brief's numbers: 3.45 at (5, 2), 2.70 at (5, 0.5); NE stays flat at 2.4.
    let south = ceiling_of(&s, 0);
    assert!((top_at(south, Vec2::new(5.0, 2.0)).unwrap() - 3.45).abs() < 1e-5);
    assert!((top_at(south, Vec2::new(5.0, 0.5)).unwrap() - 2.70).abs() < 1e-5);
    assert_eq!(part_at(&s, 5.0, 2.0, 3.44).map(|h| h.1), Some("ceiling".to_string()));
    assert_eq!(part_at(&s, 5.0, 2.0, 3.40), None);
    assert!((top_at(ceiling_of(&s, 2), Vec2::new(8.0, 5.0)).unwrap() - 2.4).abs() < 1e-6);
    assert_eq!(s.ceilings, 3, "still one ceiling per room");
}

#[test]
fn flat_room_under_a_high_roof_is_unchanged() {
    // Under the 6/12 gable every flat ceiling is below the underside: phase 3's 20 mm slab, exactly.
    let m = house(Some(6.0));
    let s = Scene3d::build(&m);
    let level = m.levels.first().cloned().unwrap_or_default();
    for r in &m.rooms {
        let want = cadcraft_arch3d::floors::ceiling_surface(r, &level, "ceiling").expect("the slab");
        let got = ceiling_of(&s, r.id.0);
        assert_eq!(got.positions, want.positions, "{}", r.name);
        assert_eq!((&got.normals, &got.uvs, &got.indices), (&want.normals, &want.uvs, &want.indices), "{}", r.name);
        assert_eq!(got.level_elevation.to_bits(), want.level_elevation.to_bits());
    }
    // A vaulted room with no roof over it keeps the slab too.
    let mut bare = house(None);
    bare.rooms[0].flat_ceiling = false;
    let s = Scene3d::build(&bare);
    let want = cadcraft_arch3d::floors::ceiling_surface(&bare.rooms[0], &level, "ceiling").expect("the slab");
    assert_eq!(ceiling_of(&s, 0).positions, want.positions);
}

#[test]
fn clipped_flat_ceiling_follows_the_roof() {
    // S at 2.45 under a 2/12 gable (underside 2.4 + (y + 0.1) / 6): the ceiling follows the
    // underside south of y = 0.2 and stays flat at 2.45 north of it.
    let mut m = house(Some(2.0));
    m.rooms[0].ceiling_height = Some(2.45);
    let s = Scene3d::build(&m);
    let south = ceiling_of(&s, 0);
    assert!((top_at(south, Vec2::new(5.0, 0.15)).unwrap() - (2.4 + 0.25 / 6.0)).abs() < 1e-5);
    assert!((top_at(south, Vec2::new(5.0, 2.0)).unwrap() - 2.45).abs() < 1e-5);
    assert!((top_at(ceiling_of(&s, 1), Vec2::new(3.0, 5.0)).unwrap() - 2.4).abs() < 1e-6);
    assert_eq!(part_at(&s, 5.0, 0.15, 2.43).map(|h| h.1), Some("ceiling".to_string()), "the solid follows the underside");
    assert_ne!(part_at(&s, 5.0, 0.15, 2.445).map(|h| h.1), Some("ceiling".to_string()), "nothing of the ceiling above the roof");
    assert_eq!(part_at(&s, 5.0, 2.0, 2.44).map(|h| h.1), Some("ceiling".to_string()));
    // Nowhere does the ceiling rise above the underside, and it equals the point query.
    for p in grid(&m.rooms[0]) {
        let top = top_at(south, p).expect("a ceiling");
        let u = roof_underside_at(&m, 0, p).expect("under the roof");
        assert!(top <= u + 1e-5, "{p:?}: {top} above the underside {u}");
        assert!((top - rough_ceiling_in(&m, &m.rooms[0], p).unwrap()).abs() < 1e-5, "{p:?}");
    }
}

#[test]
fn repitch_rebuilds_the_vaulted_room_and_equals_full() {
    let mut m = house(Some(6.0));
    m.rooms[0].flat_ceiling = false;
    let (mut cache, _) = SceneCache::new(&m);
    for rise in [8.0, 6.0, 2.0, 6.0] {
        let prev = m.clone();
        repitch(&mut m, rise);
        // Only the roof changed as data: the room is rebuilt because its ceiling inputs hold the
        // roofs of its level, so its mesh changes with the pitch.
        let changed = changed_objects(&prev, &m).expect("a diff");
        assert!(changed.contains(&key(ObjectKind::Roof, 0)) && !changed.contains(&key(ObjectKind::Room, 0)), "rise {rise}: {changed:?}");
        let before = ceiling_of(cache.scene(), 0).positions.clone();
        let stats = cache.update(&m);
        assert_ne!(ceiling_of(cache.scene(), 0).positions, before, "rise {rise}: the vaulted room is rebuilt");
        let full = Scene3d::build(&m);
        assert_eq!(signature(cache.scene()), signature(&full), "rise {rise}: {stats:?}");
        assert!(stats.incremental && stats.rebuilt >= 1, "rise {rise}: {stats:?}");
        // 2.4 + rise / 12 * 2.1 at (5, 2): 3.8 for 8/12, 3.45 for 6/12.
        let want = 2.4 + rise / 12.0 * 2.1;
        assert!((top_at(ceiling_of(cache.scene(), 0), Vec2::new(5.0, 2.0)).unwrap() - want).abs() < 1e-5, "rise {rise}");
    }
    // Toggling the vault rebuilds the room and still equals the full build.
    m.rooms[0].flat_ceiling = true;
    let stats = cache.update(&m);
    assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)), "{stats:?}");
    assert!((top_at(ceiling_of(cache.scene(), 0), Vec2::new(5.0, 2.0)).unwrap() - 2.4).abs() < 1e-6);
}

#[test]
fn recessed_cuts_survive_the_vault_split() {
    // Two pot lights and a stair well in the vaulted south room: the ceiling stays open over each.
    let mut m = house(Some(6.0));
    m.rooms[0].flat_ceiling = false;
    m.rooms[0].holes = vec![rect(4.0, 1.0, 5.0, 2.0)];
    let pot = cadcraft_arch::light::find_type("Recessed pot light");
    for (n, (x, y)) in [(2.0, 1.5), (7.0, 2.2)].into_iter().enumerate() {
        let (id, p) = (ArchId(n as u32), Vec2::new(x, y));
        m.lights.push(
            pot.as_ref().map(|t| cadcraft_arch::light::fixture_from_type(id, t, p)).unwrap_or_else(|| LightFixture::new(id, "Recessed pot light", p)),
        );
    }
    m.next_light_id = 2;
    let level = m.levels.first().cloned().unwrap_or_default();
    assert_eq!(cadcraft_arch3d::fixture::ceiling_cutouts(&m, &m.rooms[0], &level).len(), 2, "a cut per pot");
    let s = Scene3d::build(&m);
    let south = ceiling_of(&s, 0);
    for (x, y) in [(2.0, 1.5), (7.0, 2.2), (4.5, 1.5)] {
        assert_eq!(top_at(south, Vec2::new(x, y)), None, "open over ({x}, {y})");
        let u = 2.4 + 0.5 * (y + 0.1);
        assert_ne!(part_at(&s, x, y, u - 0.01).map(|h| h.1), Some("ceiling".to_string()), "({x}, {y})");
    }
    for (x, y) in [(3.0, 1.5), (6.0, 2.2), (4.5, 2.5)] {
        let u = 2.4 + 0.5 * (y + 0.1);
        assert!((top_at(south, Vec2::new(x, y)).expect("ceiling") - u).abs() < 1e-5, "({x}, {y})");
        assert_eq!(part_at(&s, x, y, u - 0.01).map(|h| h.1), Some("ceiling".to_string()), "({x}, {y})");
    }
}
