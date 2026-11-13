//! Corner cabinets: L-shaped and diagonal carcasses, and the fronts of every corner style.

use cadcraft_arch::casework_corner::{CornerEnd, CornerInfo, CornerStyle, corner_local_footprint, front_edges, offset_ring};
use cadcraft_arch::casework_layout::{FrontKind, fronts as layout_fronts};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Cabinet, FrontStyle};
use cadcraft_geom::Vec2;

use crate::Mesh;
use crate::casework::{Frame, level_elevation, push_front};

/// Thickness of a front (m); the carcass's front edges are inset by it.
const FRONT_THICK: f64 = 0.019;
/// How far the toe plinth stands back from a front edge (m).
const PLINTH_INSET: f64 = 0.075;
/// Height of the toe plinth (m).
const TOE: f64 = 0.1;
/// Reveal between fronts (m).
const GAP: f64 = 0.003;
/// A blind corner's reduced width must exceed this to have doors of its own (m).
const MIN_REDUCED: f64 = 0.05;

fn rot(p: Vec2, deg: f64) -> Vec2 {
    let (s, c) = deg.to_radians().sin_cos();
    Vec2::new(p.x * c - p.y * s, p.x * s + p.y * c)
}

/// `c` as its doors see it when it is a blind corner: narrower by the blind part (the other run's
/// depth), shifted away from the corner. `None` for any other style or a cabinet too narrow for it.
pub fn blind_view(c: &Cabinet, info: &CornerInfo) -> Option<Cabinet> {
    if info.style != CornerStyle::Blind || !(c.width - info.other_depth > MIN_REDUCED) {
        return None;
    }
    let shift = match info.end {
        CornerEnd::End => -info.other_depth / 2.0,
        CornerEnd::Start => info.other_depth / 2.0,
    };
    let mut r = c.clone();
    r.width = c.width - info.other_depth;
    r.position = c.position + rot(Vec2::new(shift, 0.0), c.rotation);
    Some(r)
}

/// The footprint with its front edges moved in by `inset`, in world coordinates.
fn inset_world(c: &Cabinet, info: &CornerInfo, inset: f64) -> Vec<Vec2> {
    let ring = corner_local_footprint(c, info);
    let dist: Vec<f64> = front_edges(c, info, &ring).iter().map(|&f| if f { -inset } else { 0.0 }).collect();
    offset_ring(&ring, &dist).into_iter().map(|q| c.position + rot(q, c.rotation)).collect()
}

/// The carcass of a corner cabinet, or `None` when `c` builds the ordinary one: an L or diagonal
/// footprint prism with its front edges inset by the fronts' thickness, plus the toe plinth.
pub fn carcass(model: &ArchModel, c: &Cabinet, info: Option<&CornerInfo>) -> Option<Mesh> {
    let info = info.filter(|i| i.style != CornerStyle::Blind)?;
    let level_z = level_elevation(model, c.level) + c.elevation;
    let toe = if c.toe_kick { TOE } else { 0.0 };
    let mut mesh = Mesh::new("cabinet-box", c.id);
    mesh.push_prism(&inset_world(c, info, FRONT_THICK), level_z + toe, level_z + c.height);
    if c.toe_kick {
        mesh.push_prism(&inset_world(c, info, PLINTH_INSET), level_z, level_z + TOE);
    }
    (!mesh.indices.is_empty()).then_some(mesh)
}

/// The fronts of a corner cabinet, or `None` when `c` builds the ordinary ones. Blind: the doors of
/// the reduced width and a false panel over the blind part. Lazy susan: a door on each face of the
/// notch. Diagonal: a door on the diagonal face. Corner doors have no pulls.
pub fn fronts(model: &ArchModel, c: &Cabinet, info: Option<&CornerInfo>) -> Option<Mesh> {
    let info = info?;
    let floor = level_elevation(model, c.level);
    let level_z = floor + c.elevation;
    let toe = if c.toe_kick { TOE } else { 0.0 };
    let mut mesh = Mesh::new("cabinet-front", c.id);
    if info.style == CornerStyle::Blind {
        let reduced = blind_view(c, info)?;
        let frame = Frame { position: reduced.position, rotation: reduced.rotation };
        let plane = crate::cabinet_parts::frame::front_plane(&reduced);
        for f in layout_fronts(model, &reduced) {
            let [u0, v0, u1, v1] = f.rect;
            let style = match f.kind {
                FrontKind::Door => reduced.door_style,
                FrontKind::Drawer => reduced.drawer_style,
                FrontKind::False => FrontStyle::Slab,
                FrontKind::Opening | FrontKind::Appliance => continue,
            };
            push_front(&mut mesh, &frame, style, reduced.depth, plane, [u0, floor + v0, u1, floor + v1]);
        }
        if c.fronts != cadcraft_doc::arch::Fronts::Open {
            let w2 = c.width / 2.0;
            let (u0, u1) = match info.end {
                CornerEnd::End => (w2 - info.other_depth + GAP, w2 - GAP),
                CornerEnd::Start => (-w2 + GAP, -w2 + info.other_depth - GAP),
            };
            let whole = Frame { position: c.position, rotation: c.rotation };
            whole.box_(&mut mesh, u0, u1, c.depth - FRONT_THICK, c.depth, level_z + toe, level_z + c.height);
        }
    } else {
        let ring = corner_local_footprint(c, info);
        let edges = front_edges(c, info, &ring);
        let n = ring.len();
        for (i, &is_front) in edges.iter().enumerate() {
            let (Some(&a), Some(&b)) = (ring.get(i), ring.get((i + 1) % n)) else { continue };
            let dir = (b - a).normalized();
            let len = a.dist(b);
            if !is_front || len <= 2.0 * GAP {
                continue;
            }
            // The face shrunk by the reveal at both ends, thickened inward (to the left of the edge, which runs
            // counter-clockwise).
            let (p, q) = (a + dir * GAP, b - dir * GAP);
            let inner = dir.perp() * FRONT_THICK;
            let door: Vec<Vec2> = [p, q, q + inner, p + inner].into_iter().map(|v| c.position + rot(v, c.rotation)).collect();
            mesh.push_prism(&door, level_z + toe, level_z + c.height);
        }
    }
    (!mesh.indices.is_empty()).then_some(mesh)
}
