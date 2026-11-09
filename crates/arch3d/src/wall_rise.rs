//! Wall rises (phase 16; owner: p16-vault-walls): the part of a wall bounding a vaulted room that
//! rises from its top to the roof underside, along its thickness.

use cadcraft_arch::join;
use cadcraft_arch::join::WallShape;
use cadcraft_doc::arch::Room;
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::Vec2;

use crate::finish;
use crate::mesh::Mesh;
use crate::planar::{self, PlaneZ};

/// Everything a wall's rise is made from.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RiseIn {
    /// The solved join polygon (or raw wall points) defining the rise cross-section.
    pub polygon: Vec<Vec2>,
    /// The level elevation (base of the wall).
    pub base: f64,
    /// The top of the wall (bottom of the rise).
    pub bottom: f64,
    /// Cells under the roof planes, each paired with its underside plane.
    pub cells: Vec<(Vec<Vec2>, PlaneZ)>,
    /// The vaulted rooms this wall bounds (for incremental rebuild).
    pub vaulted: Vec<ArchId>,
}

/// The inputs of `wall`'s rise, or `None` when it has none. `shape` is its join shape, `span` its
/// base and top, `roof_top` whether a roof clips its top.
pub(crate) fn rise_input(model: &ArchModel, wall: &Wall, shape: Option<&WallShape>, span: Option<(f64, f64)>, roof_top: bool) -> Option<RiseIn> {
    if roof_top || wall.divider || wall.retaining {
        return None;
    }
    let (base, bottom) = span?;
    let polygon = match shape {
        Some(shape) => shape.polygon.clone(),
        None => wall_rectangle(wall),
    };
    if polygon.is_empty() || polygon.len() < 3 {
        return None;
    }

    let face_lines = match join::face_lines(wall) {
        Some(lines) => lines,
        None => return None,
    };

    let mut vaulted = Vec::new();
    for room in &model.rooms {
        if room.flat_ceiling || !room.room_type.has_ceiling() {
            continue;
        }
        let room_level = room.level;
        let relevant = model.levels.iter().any(|level| level.index == room_level && wall.spans_level(level, &model.levels));
        if !relevant {
            continue;
        }
        if room_bounds_wall(room, &face_lines) {
            vaulted.push(room.id);
        }
    }
    if vaulted.is_empty() {
        return None;
    }

    let planes: Vec<(Vec<Vec2>, PlaneZ)> = model
        .roofs
        .iter()
        .filter(|roof| roof.level == wall.base_level)
        .flat_map(|roof| {
            roof.planes.iter().filter_map(|plane| {
                let ring = plane.ring.clone();
                if ring.is_empty() || !ring.iter().all(|p| p.is_finite()) {
                    return None;
                }
                planar::underside_plane(plane).map(|plane| (ring, plane))
            })
        })
        .collect();
    if planes.is_empty() {
        return None;
    }

    let mut cells: Vec<(Vec<Vec2>, PlaneZ)> = Vec::new();
    for (cell, plane) in planar::under_planes(&polygon, &[], &planes) {
        let Some(plane) = plane else {
            continue;
        };
        let (_, above) = planar::split_by_level(&cell, plane, bottom);
        if !above.is_empty() {
            cells.push((above, plane));
        }
    }
    if cells.is_empty() {
        return None;
    }

    Some(RiseIn { polygon, base, bottom, cells, vaulted })
}

/// The wall-rise meshes for a solved rise. Each cell is extruded from the wall top to the roof
/// underside; the exterior face split is kept, just like the wall itself.
pub(crate) fn rise_meshes(wall: &Wall, rise: &RiseIn) -> Vec<Mesh> {
    if rise.cells.is_empty() {
        return Vec::new();
    }

    let mut mesh = Mesh::new("wall", wall.id);
    mesh.level_elevation = rise.base;
    for (cell, plane) in &rise.cells {
        planar::push_between(&mut mesh, cell, &[], PlaneZ::flat(rise.bottom), *plane);
    }
    if mesh.indices.is_empty() {
        return Vec::new();
    }

    let (rest, exterior) = finish::split_exterior_face(wall, mesh);
    let mut out = Vec::new();
    if !rest.indices.is_empty() {
        out.push(rest);
    }
    if let Some(exterior) = exterior.filter(|m| !m.indices.is_empty()) {
        out.push(exterior);
    }
    out
}

fn wall_rectangle(wall: &Wall) -> Vec<Vec2> {
    let dx = wall.end.x - wall.start.x;
    let dy = wall.end.y - wall.start.y;
    let len = (dx * dx + dy * dy).sqrt();
    if !len.is_finite() || len <= 1e-12 {
        return Vec::new();
    }
    let nx = -dy / len;
    let ny = dx / len;
    let half = wall.thickness / 2.0;
    let p0 = Vec2::new(wall.start.x + nx * half, wall.start.y + ny * half);
    let p1 = Vec2::new(wall.end.x + nx * half, wall.end.y + ny * half);
    let p2 = Vec2::new(wall.end.x - nx * half, wall.end.y - ny * half);
    let p3 = Vec2::new(wall.start.x - nx * half, wall.start.y - ny * half);
    vec![p0, p1, p2, p3]
}

fn room_bounds_wall(room: &Room, face_lines: &[(Vec2, Vec2); 2]) -> bool {
    let n = room.polygon.len();
    if n < 2 {
        return false;
    }
    for i in 0..n {
        let a = room.polygon[i];
        let b = room.polygon[(i + 1) % n];
        if room_edge_on_face(a, b, face_lines) {
            return true;
        }
    }
    false
}

fn room_edge_on_face(a: Vec2, b: Vec2, face_lines: &[(Vec2, Vec2); 2]) -> bool {
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    for &(face_a, face_b) in face_lines {
        if !face_a.is_finite() || !face_b.is_finite() {
            continue;
        }
        if point_to_segment_distance(a, face_a, face_b) > 1e-6 || point_to_segment_distance(b, face_a, face_b) > 1e-6 {
            continue;
        }
        let dir = face_b - face_a;
        let len_sq = dir.dot(dir);
        if !len_sq.is_finite() || len_sq <= 1e-24 {
            continue;
        }
        let t0 = ((a - face_a).dot(dir)) / len_sq;
        let t1 = ((b - face_a).dot(dir)) / len_sq;
        let lo = t0.min(t1).max(0.0);
        let hi = t0.max(t1).min(1.0);
        if hi - lo > 1e-6 {
            return true;
        }
    }
    false
}

fn point_to_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let len_sq = ab.dot(ab);
    if !len_sq.is_finite() || len_sq <= 1e-24 {
        return (p - a).len();
    }
    let t = ((p - a).dot(ab)) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let q = a + ab * t;
    (p - q).len()
}
