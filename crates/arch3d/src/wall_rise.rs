//! Wall rises (phase 16; owner: p16-vault-walls): the part of a wall bounding a vaulted room that
//! rises from its top to the roof underside, along its thickness.

use cadcraft_arch::join;
use cadcraft_arch::join::WallShape;
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_doc::WallType;
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
    // Gable walls already follow the roof through the roof-top polyline; no rise needed.
    if roof_top {
        return None;
    }
    // Divider and retaining walls never rise.
    if wall.type_ == WallType::Divider || wall.retaining {
        return None;
    }
    let (base, bottom) = span?;

    // The cross-section polygon: solved join shape, or the raw centerline.
    let polygon = match shape {
        Some(s) => s.polygon.clone(),
        None => wall.points.clone(),
    };
    if polygon.is_empty() {
        return None;
    }

    // Get the face lines of this wall (the edges that face the adjacent rooms).
    let face_lines = join::face_lines(wall, shape);

    // Find rooms on the same level that are vaulted (flat_ceiling false) and bound this wall.
    let mut vaulted = Vec::new();
    for room in &model.rooms {
        if room.level != wall.base_level {
            continue;
        }
        if room.flat_ceiling {
            continue;
        }
        if bounds_wall(&room.polygon, &face_lines) {
            vaulted.push(room.id);
        }
    }
    if vaulted.is_empty() {
        return None;
    }

    // Collect the underside planes of roofs on this level.
    let planes: Vec<(Vec<Vec2>, PlaneZ)> = model
        .roofs
        .iter()
        .filter(|r| r.level == wall.base_level)
        .flat_map(|roof| {
            roof.planes.iter().filter_map(|rp| {
                if !rp.ring.is_empty() && rp.ring.iter().all(|p| p.is_finite()) {
                    Some((rp.ring.clone(), planar::underside_plane(rp)))
                } else {
                    None
                }
            })
        })
        .collect();
    if planes.is_empty() {
        return None;
    }

    // Compute the cells under each plane, keeping only those above the wall top.
    let mut cells: Vec<(Vec<Vec2>, PlaneZ)> = Vec::new();
    for (ring, plane) in &planes {
        if !plane.is_finite() {
            continue;
        }
        for cell in planar::under_planes(&polygon, ring, &[*plane]) {
            if let Some((cell, plane)) = cell {
                let (_, above) = planar::split_by_level(&cell, plane, bottom);
                if !above.is_empty() {
                    cells.push((above, plane));
                }
            }
        }
    }

    if cells.is_empty() {
        return None;
    }

    Some(RiseIn {
        polygon,
        base,
        bottom,
        cells,
        vaulted,
    })
}

/// Returns true if any edge of `room_polygon` lies on one of `face_lines`.
fn bounds_wall(room_polygon: &[Vec2], face_lines: &[&Vec2>) -> bool {
    let n = room_polygon.len();
    if n < 3 {
        return false;
    }
    for i in 0..n {
        let a = room_polygon[i];
        let b = room_polygon[(i + 1) % n];
        for fl in face_lines {
            if overlaps_face(&a, &b, fl) {
                return true;
            }
        }
    }
    false
}

/// Checks whether the segment `a->b` lies on (or overlaps) the line through `fl` by more than 1e-6.
fn overlaps_face(a: &Vec2, b: &Vec2, fl: &[&Vec2]) -> bool {
    for (pa, pb) in [(fl[0], fl[1])].iter() {
        let fa = pa[0];
        let fb = pa[1];
        let fc = pb[0];
        let fd = pb[1];
        // Both a and b must be within 1e-6 of the infinite line through (fa,fb)-(fc,fd)
        let line_a = (fb - fa) * a[0] - (fc - fa) * b[1] + (fc * fa - fb * fc) / ((fb - fa) * ((fc - fa) + (fc - fa))) + 1e-6;
        let line_a = (fc - fa).abs() > 1e-12
            ? ((fa - fc) * (b[1] - fa) - (fb - fa) * (b[0] - fc)) / ((fc - fa).abs())
            : (fb - fa).abs() > 1e-12
                ? ((fb - fa) * (b[0] - fa) - (fa - fc) * (b[1] - fa)) / ((fb - fa).abs())
                : 0.0;
        let line_b = (fc - fa).abs() > 1e-12
            ? ((fa - fc) * (a[1] - fa) - (fb - fa) * (a[0] - fc)) / ((fc - fa).abs())
            : (fb - fa).abs() > 1e-12
                ? ((fb - fa) * (a[0] - fa) - (fa - fc) * (a[1] - fa)) / ((fb - fa).abs())
                : 0.0;
        if line_a.abs() > 1e-6 || line_b.abs() > 1e-6 {
            continue;
        }
        // The edge must overlap the face segment along the wall by more than 1e-6
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            continue;
        }
        // Project the edge onto the face direction and check overlap with the face segment
        let fdx = fc - fa;
        let fdy = fb - fa;
        let flen = (fdx * fdx + fdy * fdy).sqrt();
        if flen < 1e-6 {
            continue;
        }
        let t0 = ((a[0] - fa) * fdx + (a[1] - fa) * fdy) / flen;
        let t1 = ((b[0] - fa) * fdx + (b[1] - fa) * fdy) / flen;
        let lo = t0.min(t1);
        let hi = t0.max(t1);
        let overlap = hi.min(flen).max(lo.min(0.0));
        if overlap - overlap.min(0.0) > 1e-6 {
            return true;
        }
    }
    false
}
