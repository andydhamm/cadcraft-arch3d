//! Glass walls.

use crate::incremental::{WallIn, wall_base_mesh_span};
use crate::mesh::Mesh;
use cadcraft_arch::join::WallShape;
use cadcraft_arch::opening::ROUGH_ALLOWANCE;
use cadcraft_doc::arch::is_glass_type;
use cadcraft_doc::arch::{GLASS_THICKNESS, GLASS_WALL_HEIGHT, GLASS_WALL_TYPE, Opening, Wall};
use cadcraft_geom::Vec2;

fn straight_wall_shape(wall: &Wall, thickness: f64) -> Option<WallShape> {
    let dx = wall.end.x - wall.start.x;
    let dy = wall.end.y - wall.start.y;
    let len = (dx * dx + dy * dy).sqrt();
    if !len.is_finite() || len < 1e-9 || !thickness.is_finite() || thickness <= 0.0 {
        return None;
    }
    let nx = -dy / len;
    let ny = dx / len;
    let half = thickness / 2.0;
    let p0 = Vec2::new(wall.start.x + nx * half, wall.start.y + ny * half);
    let p1 = Vec2::new(wall.end.x + nx * half, wall.end.y + ny * half);
    let p2 = Vec2::new(wall.end.x - nx * half, wall.end.y - ny * half);
    let p3 = Vec2::new(wall.start.x - nx * half, wall.start.y - ny * half);
    Some(WallShape::new(wall.id, vec![p0, p1, p2, p3], (wall.start, wall.end)))
}

fn clipped_opening(op: &Opening, base: f64, top: f64) -> Option<Opening> {
    let inset = 2e-3;
    let start = (op.sill.max(base) + inset).min(top - inset);
    let max_end = top - ROUGH_ALLOWANCE - inset;
    let end = (op.sill + op.height).min(max_end);
    if !(end > start + 1e-6 && start.is_finite() && end.is_finite()) {
        return None;
    }
    let mut out = op.clone();
    out.sill = start;
    out.height = end - start;
    Some(out)
}

fn segment_openings<'a>(i: &'a WallIn, base: f64, top: f64) -> Vec<Opening> {
    i.openings
        .iter()
        .filter_map(|op| {
            let start = op.sill;
            let end = op.sill + op.height;
            if !(start < top && end > base) {
                return None;
            }
            clipped_opening(op, base, top)
        })
        .collect()
}

fn upper_mesh(i: &WallIn, base: f64, top: f64, upper: &str) -> Option<Mesh> {
    let split = i.wall.split.as_ref()?;
    let thickness = if split.upper_thickness.is_finite() && split.upper_thickness > 0.0 { split.upper_thickness } else { GLASS_THICKNESS };
    let shape = straight_wall_shape(&i.wall, thickness)?;
    let clipped = segment_openings(i, base, top);
    let refs: Vec<&Opening> = clipped.iter().collect();
    let profile = crate::walls::elevation_profile(&shape, &i.wall, base, top, i.clip.as_ref(), i.roof_top.as_deref(), &refs)?;
    crate::walls::extrude_wall_profile(&shape, profile, base, upper)
}

/// The meshes of a glass (or glass-split) wall, or `None` for an ordinary wall, which then builds
/// as before. When it returns `Some` for a split wall it must call `niche::add_back_blocks` and
/// `niche::back_skins` on its lower part itself.
pub(crate) fn wall_meshes(i: &WallIn) -> Option<Vec<Mesh>> {
    let wall = &i.wall;
    let split = wall.split.as_ref();

    if split.is_none() && !wall.is_glass() {
        return None;
    }

    if split.is_none() {
        let mut mesh = wall_base_mesh_span(i, i.span?)?;
        mesh.material = "glass-wall".to_string();
        return Some(vec![mesh]);
    }

    let (base, top) = i.span?;
    let split_height = split.unwrap().height;
    let lower_top = base + split_height;
    let mut out = Vec::new();
    let mut lower_in = i.clone();
    lower_in.openings = segment_openings(i, base, lower_top);
    if let Some(mut lower) = wall_base_mesh_span(&lower_in, (base, lower_top)) {
        if is_glass_type(&wall.wall_type) {
            lower.material = "glass-wall".to_string();
        }
        crate::niche::add_back_blocks(&mut lower, &i.wall, &lower_in.openings, (base, lower_top));
        let (rest, outer) = crate::finish::split_exterior_face(wall, lower);
        if !rest.indices.is_empty() {
            out.push(rest);
        }
        out.extend(outer);
        out.extend(crate::niche::back_skins(wall, &lower_in.openings, (base, lower_top)));
    }

    let upper = if is_glass_type(&split.unwrap().upper) { "glass-wall" } else { "wall" };
    if let Some(mesh) = upper_mesh(i, lower_top, top, upper) {
        out.push(mesh);
    }
    if out.is_empty() { None } else { Some(out) }
}
