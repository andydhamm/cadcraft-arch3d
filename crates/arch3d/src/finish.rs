//! Finished faces in the 3D build (phase 13): a wall's exterior face, a roof's covering, a wood
//! floor. An object without a finish is built exactly as before.

use crate::mesh::Mesh;
use crate::texture::texture;
use crate::uv::face_uvs;
use cadcraft_doc::Wall;
use cadcraft_doc::arch::Finish;
use cadcraft_doc::arch::paint::Side;

/// Maximum angle (degrees) between a triangle's plan normal and the wall's side normal.
pub(crate) const FACE_ANGLE_DEG: f64 = 5.0;
/// How far (metres) a triangle's centroid may lie off the side plane.
pub(crate) const PLANE_TOL: f64 = 0.002;

/// The mesh with only the triangles whose `keep` flag equals `want` (all vertices kept, so vertex
/// order is unchanged).
pub(crate) fn pick(mesh: &Mesh, keep: &[bool], want: bool) -> Mesh {
    let mut out = Mesh { indices: Vec::new(), ..mesh.clone() };
    for (tri, k) in mesh.indices.chunks_exact(3).zip(keep) {
        if *k == want {
            out.indices.extend_from_slice(tri);
        }
    }
    out
}

/// Split a wall mesh into the triangles facing `side` (left or right of start -> end, at
/// ±thickness/2) and the rest. Every triangle lands in exactly one of the two meshes. If no triangle
/// faces `side` the original mesh is returned unchanged and the empty side mesh is the original.
pub(crate) fn split_side(wall: &Wall, mesh: &Mesh, side: Side) -> (Mesh, Mesh) {
    let (dx, dy) = (wall.end.x - wall.start.x, wall.end.y - wall.start.y);
    let len = (dx * dx + dy * dy).sqrt();
    if !(len.is_finite() && len > 1e-9 && wall.thickness.is_finite()) {
        return (mesh.clone(), Mesh::new("wall", wall.id));
    }
    let (rx, ry) = match side {
        Side::Right => (dy / len, -dx / len),
        Side::Left => (-dy / len, dx / len),
    };
    let target = match side {
        Side::Right => wall.thickness / 2.0,
        Side::Left => -wall.thickness / 2.0,
    };
    let cos_max = FACE_ANGLE_DEG.to_radians().cos();
    let pos = |i: u32| mesh.positions.get(i as usize).map(|p| [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]);
    let mut keep = Vec::with_capacity(mesh.indices.len() / 3);
    for tri in mesh.indices.chunks_exact(3) {
        let (Some(&i0), Some(&i1), Some(&i2)) = (tri.first(), tri.get(1), tri.get(2)) else {
            keep.push(false);
            continue;
        };
        let (Some(a), Some(b), Some(c)) = (pos(i0), pos(i1), pos(i2)) else {
            keep.push(false);
            continue;
        };
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let nx = e1[1] * e2[2] - e1[2] * e2[1];
        let ny = e1[2] * e2[0] - e1[0] * e2[2];
        let plan = (nx * nx + ny * ny).sqrt();
        let facing = plan.is_finite() && plan > 1e-18 && (nx * rx + ny * ry) / plan >= cos_max;
        let (cx, cy) = ((a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0);
        let dist = (cx - wall.start.x) * rx + (cy - wall.start.y) * ry;
        keep.push(facing && (dist - target).abs() <= PLANE_TOL);
    }
    if !keep.iter().any(|&k| k) {
        return (mesh.clone(), Mesh::new("wall", wall.id));
    }
    let side = pick(mesh, &keep, true);
    let rest = pick(mesh, &keep, false);
    (side, rest)
}

/// Split a wall's mesh in two when the wall has a finish: the triangles of the exterior face (right
/// of start -> end, at +thickness/2, facing the right normal) go to a second mesh with material
/// `wall-<finish>` and face-aligned UVs; everything else keeps the mesh's material. Without a finish
/// (or a finish that is not a wall finish) the mesh comes back unchanged. Every triangle lands in
/// exactly one of the two meshes.
pub fn split_exterior_face(wall: &Wall, mesh: Mesh) -> (Mesh, Option<Mesh>) {
    let Some(finish) = wall.finish.filter(|f| f.for_walls()) else {
        return (mesh, None);
    };
    let (side, rest) = split_side(wall, &mesh, Side::Right);
    if side.indices.is_empty() || side.indices.len() != mesh.indices.len() {
        // No triangles on the right: fall back to the pre-generalisation behaviour.
        return (mesh, None);
    }
    let mut outer = face_uvs(&side, texture(finish).tile);
    outer.material = format!("wall-{}", finish.name());
    (rest, Some(outer))
}

/// A roof plane's mesh under `covering`: material `roof-<finish>` and face-aligned UVs.
pub fn cover_roof(mesh: Mesh, covering: Option<Finish>) -> Mesh {
    restyle(mesh, covering.filter(|f| f.for_roofs()), "roof")
}

/// A room's floor mesh under `floor_finish`: material `floor-<finish>` and face-aligned UVs.
pub fn finish_floor(mesh: Mesh, finish: Option<Finish>) -> Mesh {
    restyle(mesh, finish.filter(|f| f.for_floors()), "floor")
}

fn restyle(mesh: Mesh, finish: Option<Finish>, prefix: &str) -> Mesh {
    let Some(f) = finish else {
        return mesh;
    };
    let mut out = face_uvs(&mesh, texture(f).tile);
    out.material = format!("{prefix}-{}", f.name());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Scene3d, SceneCache};
    use cadcraft_doc::arch::{Roof, RoofPlane, Room};
    use cadcraft_doc::{ArchId, ArchModel};
    use cadcraft_geom::Vec2;

    fn ring(finish: Option<Finish>) -> ArchModel {
        let pts = [(0.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0)];
        let mut model = ArchModel::default();
        for i in 0..4usize {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            model.walls.push(Wall {
                id: ArchId(i as u32),
                start: Vec2::new(a.0, a.1),
                end: Vec2::new(b.0, b.1),
                thickness: 0.2,
                height: 2.4,
                finish,
                ..Default::default()
            });
        }
        model
    }

    fn tris(scene: &Scene3d, id: ArchId, material: &str) -> usize {
        scene.meshes.iter().filter(|m| m.object == id && m.material == material).map(|m| m.indices.len() / 3).sum()
    }

    /// Each triangle as its sorted vertex positions (as bits), for comparing across meshes.
    fn keys(m: &Mesh) -> Vec<Vec<[u32; 3]>> {
        m.indices
            .chunks_exact(3)
            .map(|t| {
                let mut v: Vec<[u32; 3]> = t.iter().filter_map(|&i| m.positions.get(i as usize)).map(|p| p.map(f32::to_bits)).collect();
                v.sort();
                v
            })
            .collect()
    }

    #[test]
    fn exterior_face_split_keeps_every_triangle_once() {
        let model = ring(None);
        let whole = Scene3d::build(&model);
        let wall = &model.walls[0];
        let mesh = whole.meshes.iter().find(|m| m.object == wall.id && m.material == "wall").cloned();
        let mesh = mesh.expect("wall mesh");
        let total = mesh.indices.len() / 3;
        assert!(total > 0);
        let mut finished = wall.clone();
        finished.finish = Some(Finish::Brick);
        let (rest, outer) = split_exterior_face(&finished, mesh.clone());
        let outer = outer.expect("exterior face");
        assert_eq!(rest.indices.len() / 3 + outer.indices.len() / 3, total);
        assert!(!outer.indices.is_empty());
        let mut all = keys(&mesh);
        let mut got = keys(&rest);
        got.extend(keys(&outer));
        all.sort();
        got.sort();
        assert_eq!(all, got);
        // No finish, or a roof-only finish: the mesh comes back untouched.
        let (same, none) = split_exterior_face(wall, mesh.clone());
        assert!(none.is_none() && same.indices == mesh.indices && same.material == mesh.material);
        finished.finish = Some(Finish::Shingles);
        assert!(split_exterior_face(&finished, mesh).1.is_none());
    }

    #[test]
    fn a_ccw_ring_with_brick_faces_outwards() {
        let model = ring(Some(Finish::Brick));
        let scene = Scene3d::build(&model);
        let bricks: Vec<&Mesh> = scene.meshes.iter().filter(|m| m.material == "wall-brick").collect();
        assert_eq!(bricks.len(), 4);
        for m in bricks {
            assert_eq!(m.uvs.len(), m.positions.len());
            for tri in m.indices.chunks_exact(3) {
                let p = |k: usize| tri.get(k).and_then(|&i| m.positions.get(i as usize)).copied().unwrap_or([0.0; 3]);
                let c = [(p(0)[0] + p(1)[0] + p(2)[0]) / 3.0, (p(0)[1] + p(1)[1] + p(2)[1]) / 3.0];
                let n = tri.first().and_then(|&i| m.normals.get(i as usize)).copied().unwrap_or([0.0; 3]);
                // Outward: the normal points away from the ring centre (3, 2).
                assert!(n[0] * (c[0] - 3.0) + n[1] * (c[1] - 2.0) > 0.0, "normal {n:?} at {c:?}");
            }
        }
        // The inside faces stay "wall".
        for w in &model.walls {
            assert!(tris(&scene, w.id, "wall") > 0);
        }
        assert!(scene.meshes.iter().all(|m| crate::probe::part_of(m) == "wall"));
    }

    #[test]
    fn unfinished_walls_are_unchanged() {
        let model = ring(None);
        let scene = Scene3d::build(&model);
        assert_eq!(scene.meshes.len(), 4);
        assert!(scene.meshes.iter().all(|m| m.material == "wall"));
        // Toggling a finish on and off returns the very same digest.
        let mut on = model.clone();
        on.walls[1].finish = Some(Finish::Siding);
        assert_ne!(Scene3d::build(&on).digest(), scene.digest());
        on.walls[1].finish = None;
        assert_eq!(Scene3d::build(&on).digest(), scene.digest());
    }

    #[test]
    fn roof_covering_takes_the_roof_planes() {
        let plane = RoofPlane {
            ring: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 3.0), Vec2::new(0.0, 3.0)],
            slope: 0.5,
            thickness: 0.2,
            eave_z: 2.4,
            fascia_depth: 0.1,
            soffit_depth: 0.1,
            base_edge: [0.0, 0.0, 6.0, 0.0],
            ..Default::default()
        };
        let mut roof = Roof { fascia: true, soffit: true, planes: vec![plane], ..Default::default() };
        let plain = crate::roof::roof_meshes(&roof, 2.4);
        assert!(plain.iter().any(|m| m.material == "roof"));
        roof.covering = Some(Finish::Shingles);
        let covered = crate::roof::roof_meshes(&roof, 2.4);
        assert_eq!(plain.len(), covered.len());
        let planes: Vec<&Mesh> = covered.iter().filter(|m| m.material == "roof-shingles").collect();
        assert_eq!(planes.len(), plain.iter().filter(|m| m.material == "roof").count());
        assert!(planes.iter().all(|m| m.uvs.len() == m.positions.len() && crate::probe::part_of(m) == "roof"));
        // Fascia and soffit are unchanged.
        let rest = |v: &[Mesh]| {
            v.iter().filter(|m| m.material != "roof" && !m.material.starts_with("roof-")).map(|m| (m.material.clone(), keys(m))).collect::<Vec<_>>()
        };
        assert!(!rest(&plain).is_empty());
        assert_eq!(rest(&plain), rest(&covered));
        // A wall-only finish does not cover a roof.
        roof.covering = Some(Finish::Brick);
        assert!(crate::roof::roof_meshes(&roof, 2.4).iter().all(|m| m.material != "roof-brick"));
    }

    #[test]
    fn wood_floor_takes_the_room_floor() {
        let mut model = ArchModel::default();
        model.rooms.push(Room {
            id: ArchId(1),
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
            ..Default::default()
        });
        let plain = Scene3d::build(&model);
        assert!(plain.meshes.iter().any(|m| m.material == "floor"));
        model.rooms[0].floor_finish = Some(Finish::Wood);
        let wood = Scene3d::build(&model);
        assert!(wood.meshes.iter().all(|m| m.material != "floor"));
        let floor = wood.meshes.iter().find(|m| m.material == "floor-wood").expect("wood floor");
        assert_eq!(floor.uvs.len(), floor.positions.len());
        assert_eq!(crate::probe::part_of(floor), "floor");
        assert_eq!(crate::probe::mesh_label(floor), cadcraft_doc::arch::room_label(ArchId(1)));
        assert_eq!(wood.floors, plain.floors);
        assert_eq!(wood.ceilings, plain.ceilings);
        assert_eq!(crate::object_key(floor).kind, crate::ObjectKind::Room);
        // A brick floor is not a floor finish: it stays "floor".
        model.rooms[0].floor_finish = Some(Finish::Brick);
        assert!(Scene3d::build(&model).meshes.iter().any(|m| m.material == "floor"));
    }

    #[test]
    fn finish_change_rebuilds_only_that_wall() {
        let model = ring(None);
        let (mut cache, _) = SceneCache::new(&model);
        let mut next = model.clone();
        next.walls[2].finish = Some(Finish::Wood);
        let stats = cache.update(&next);
        assert!(stats.incremental);
        assert_eq!(stats.rebuilt, 1);
        assert_eq!(cache.scene().digest(), Scene3d::build(&next).digest());
        assert!(cache.scene().meshes.iter().any(|m| m.material == "wall-wood" && m.object == ArchId(2)));
        next.walls[2].finish = None;
        let back = cache.update(&next);
        assert_eq!(back.rebuilt, 1);
        assert_eq!(cache.scene().digest(), Scene3d::build(&model).digest());
    }
}
