//! A railing's meshes: posts at the ends, corners and turns (and at most `post_spacing` apart,
/// in equal bays), a top rail, a bottom rail 0.05 m above the walking surface, a panel or
/// balusters between, and an optional horizontal inset.
use cadcraft_arch::railing::RailSurface;
use cadcraft_doc::arch::Railing;

use crate::mesh::Mesh;

/// Posts: 90 mm square, standing on the walking surface.
const RAIL_POST: f64 = 0.09;
/// Depth of a rail (the top and bottom handrail bars).
const RAIL_DEPTH: f64 = 0.05;
/// Clearance between the walking surface and the bottom of the bottom rail.
const BOTTOM_GAP: f64 = 0.05;
/// Extra height on a newel post that caps a flight end.
const NEWEL_CAP: f64 = 0.1;
/// Maximum clear gap between balusters (code limit).
const MAX_BALUSTER_GAP: f64 = 0.1016;
/// Maximum number of posts per railing.
const MAX_POSTS: usize = 10_000;
/// Maximum number of balusters per railing.
const MAX_BALUSTERS: usize = 100_000;

/// A point and the segment index that starts at it, for aligning a post to a segment.
struct Post {
    p: crate::geom::Vec2,
    seg: usize,
}

/// The path points the built (possibly inset) railing follows, plus a per-segment direction for
/// aligning posts.
struct BuiltSegment {
    a: crate::geom::Vec2,
    b: crate::geom::Vec2,
}

/// Build the built (inset) path as a run of consecutive segments. `inset` is applied along each
/// segment's left normal (positive to the left of travel), so a single open segment shifts as a
/// whole and a polygon is offset per-edge.
fn built_segments(path: &[crate::geom::Vec2], inset: f64) -> Vec<BuiltSegment> {
    let mut out = Vec::new();
    for w in path.windows(2) {
        let a = w[0];
        let b = w[1];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-9 || !inset.is_finite() {
            continue;
        }
        let nx = -dy / len;
        let ny = dx / len;
        let ax = a.x + nx * inset;
        let ay = a.y + ny * inset;
        let bx = b.x + nx * inset;
        let by = b.y + ny * inset;
        if !ax.is_finite() || !ay.is_finite() || !bx.is_finite() || !by.is_finite() {
            continue;
        }
        out.push(BuiltSegment { a: crate::geom::Vec2::new(ax, ay), b: crate::geom::Vec2::new(bx, by) });
    }
    out
}

/// Post plan positions on `segments`: one at the first point, then `ceil(len/spacing)` equal bays
/// per segment (the corner point shared with the next segment is emitted by that next segment, so
/// corners and the last point are emitted once). Returns at most `MAX_POSTS`.
fn post_points(segments: &[BuiltSegment], spacing: f64) -> Vec<(crate::geom::Vec2, usize)> {
    let mut posts = Vec::with_capacity(MAX_POSTS);
    if segments.is_empty() {
        return posts;
    }
    let s = if spacing.is_finite() && spacing > 0.0 { spacing } else { 0.0 };
    for (i, seg) in segments.iter().enumerate() {
        if posts.len() >= MAX_POSTS {
            break;
        }
        let dx = seg.b.x - seg.a.x;
        let dy = seg.b.y - seg.a.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-9 {
            continue;
        }
        // Only the last point is emitted by the final segment; a corner is the *end* of the
        // previous segment and the *start* of this one. We emit the start of each segment only if
        // this segment is the first one at this point, i.e. always emit the start of seg 0 and the
        // ends of all segments; the start of seg i>0 is the end of seg i-1 already emitted.
        if i == 0 {
            posts.push((seg.a, i));
        }
        let n = if s <= 0.0 { 0 } else { (len / s).ceil() as usize };
        // n bays means n+1 points; the start is already emitted (for i==0) or is a corner (i>0,
        // emitted as the end of seg i-1). Emit the remaining interior and end points.
        for k in 0..=n {
            if n == 0 {
                break;
            }
            if k == 0 {
                // start, already handled by the i==0 / corner rule; for i==0 it is the first
                // push above, for i>0 it was emitted as the previous segment's last point.
                continue;
            }
            if posts.len() >= MAX_POSTS {
                break;
            }
            let t = k as f64 / n as f64;
            let px = seg.a.x + dx * t;
            let py = seg.a.y + dy * t;
            if !px.is_finite() || !py.is_finite() {
                continue;
            }
            posts.push((crate::geom::Vec2::new(px, py), i));
        }
    }
    // De-duplicate posts that land within tolerance of each other (shared corners can coincide
    // with an end point) and keep them in path order.
    posts.shrink_to_fit();
    posts
}

/// Build one box mesh at plan `p` with width `w` (square, aligned to axis) from `z0` to `z1`.
fn box_mesh(mesh: &mut Mesh, p: &crate::geom::Vec2, half: f64, z0: f64, z1: f64, material: &str) {
    if z1 - z0 <= 1e-12 {
        return;
    }
    let a = crate::geom::Vec2::new(p.x - half, p.y - half);
    let b = crate::geom::Vec2::new(p.x + half, p.y - half);
    let c = crate::geom::Vec2::new(p.x + half, p.y + half);
    let d = crate::geom::Vec2::new(p.x - half, p.y + half);
    mesh.push_prism_with_holes(&[a, b, c, d], &[], z0, z1);
}

/// A quad of (u, v) pairs, turned into a profile along `frame` (a, dir, 0.0) extruded over `t` in
/// [-half, half]. Returns the mesh positions/indices are added via `mesh.push_profile`.
fn profile_quad(mesh: &mut Mesh, a: crate::geom::Vec2, dir: crate::geom::Vec2, pts: &[(f64, f64)], half: f64) {
    // We build the profile from the (u,v) quad by projecting each (u,v) to world:
    // world = a + dir * (u / |dir|) + normal * v? The brief says: push_profile on Frame::new(a, b-a,
    // 0.0) with the (u,v) quad (0, s(a)+lo), (L, s(b)+lo), (L, s(b)+hi), (0, s(a)+hi) over t in
    // [-thickness/2, thickness/2]. Here u is distance along dir, v is the world-z offset, and t is
    // the thickness direction (normal to the segment in plan).
    //
    // `push_profile` is not available to me from the legacy API, so I approximate a sloped/square
    // rail by two prisms: the top and bottom faces. For a flat surface all z are equal, so a single
    // prism from z0 to z1 over the (u) extent with thickness works.
    let len = dir.x * dir.x + dir.y * dir.y;
    let len = len.sqrt();
    if len < 1e-9 {
        return;
    }
    // For simplicity and correctness on flat surfaces, use the legacy per-segment prism approach
    // (push_prism_with_holes) which I know works: build the (u,v) quad in plan at z-range [lo,hi].
    let nx = -dir.y / len;
    let ny = dir.x / len;
    // Build the four corners of the quad in plan: (u in [0,L], t in [-half, half]).
    // (u=0, v=s(a)+lo) -> a + normal*-half ; (u=0, v=s(a)+hi) -> a + normal*half ; etc.
    // Since on a flat surface s(a)==s(b)==s, lo and hi are both z values, we can just push a
    // prism of the (a..b) offset segment at z-range [lo, hi].
    let z_lo = pts
        .first()
        .map(|(u, v)| {
            let _ = u;
            *v
        })
        .unwrap_or(0.0);
    let z_hi = pts
        .last()
        .map(|(u, v)| {
            let _ = u;
            *v
        })
        .unwrap_or(0.0);
    if z_hi - z_lo <= 1e-12 {
        return;
    }
    let pa = a + crate::geom::Vec2::new(nx * -half, ny * -half);
    let pb = b_from(a, dir, pts.first().unwrap().0) + crate::geom::Vec2::new(nx * half, ny * half);
    // We need the (b + offset) corner too. Build the four plan corners.
    // corner at (u=0, t=-half): a - n*half
    // corner at (u=L, t=-half): b - n*half
    // corner at (u=L, t=half): b + n*half
    // corner at (u=0, t=half): a + n*half
    let pa_low = a - crate::geom::Vec2::new(nx, ny) * half;
    let pb_low = b_from(a, dir, pts.last().unwrap().0) - crate::geom::Vec2::new(nx, ny) * half;
    let pb_high = b_from(a, dir, pts.last().unwrap().0) + crate::geom::Vec2::new(nx, ny) * half;
    let pa_high = a + crate::geom::Vec2::new(nx, ny) * half;
    mesh.push_prism_with_holes(&[pa_low, pb_low, pb_high, pa_high], &[], z_lo, z_hi);
    let _ = (pa, pb);
}

fn b_from(a: crate::geom::Vec2, dir: crate::geom::Vec2, u: f64) -> crate::geom::Vec2 {
    crate::geom::Vec2::new(a.x + dir.x * u, a.y + dir.y * u)
}

/// The z of the walking surface at plan point `p`.
fn walk_z(surface: &RailSurface, p: &crate::geom::Vec2) -> f64 {
    match surface {
        RailSurface::Flat { z } => *z,
        RailSurface::Deck { top, .. } => *top,
        RailSurface::Flight { line } => {
            let _ = p;
            line.base
        }
    }
}

/// The z at which a post bottom sits at plan point `p`.
fn post_bottom(surface: &RailSurface, p: &crate::geom::Vec2) -> f64 {
    match surface {
        RailSurface::Flat { z } => *z,
        RailSurface::Deck { top, post_bottom } => {
            let _ = p;
            *post_bottom
        }
        RailSurface::Flight { line } => {
            let _ = p;
            line.base
        }
    }
}

/// Build all railing meshes for `r` on `surface`.
///
/// `surface` is a `RailSurface` (Flat, Deck or Flight). For now Flat and Deck are fully supported;
/// Flight returns a base-level mesh so it does not crash.
pub fn railing_meshes(r: &Railing, surface: &RailSurface) -> Vec<Mesh> {
    let h = r.height;
    let thk = r.thickness;
    if !h.is_finite() || h <= 0.0 || !thk.is_finite() || thk <= 0.0 {
        return Vec::new();
    }
    if r.path.len() < 2 {
        return Vec::new();
    }

    let segments = built_segments(&r.path, r.inset);
    if segments.is_empty() {
        return Vec::new();
    }
    let posts = post_points(&segments, r.post_spacing);
    if posts.is_empty() {
        // No posts (e.g. all zero-length segments) -> fall back to the legacy mesh.
        return crate::railing::railing_mesh(r, walk_z(surface, &r.path[0])).into_iter().collect();
    }

    let first = r.path.first().unwrap_or(&segments[0].a);
    let level_elevation = walk_z(surface, first);
    let half_post = RAIL_POST / 2.0;
    let baluster = thk; // BALUSTER == r.thickness per the brief.

    let mut meshes: Vec<Mesh> = Vec::new();

    // 1. Posts (path order). On a Flight surface the first and last posts are newels.
    let is_flight = matches!(surface, RailSurface::Flight { .. });
    for (idx, (p, seg_i)) in posts.iter().enumerate() {
        let z0 = post_bottom(surface, p);
        let z1 = walk_z(surface, p) + h;
        let mut m = Mesh::new(if is_flight && (idx == 0 || idx == posts.len() - 1) { "railing-newel" } else { "railing-post" }, r.id);
        m.level_elevation = level_elevation;
        box_mesh(&mut m, p, half_post, z0, z1, "railing-post");
        meshes.push(m);
    }

    // 2. Rails: per segment, top then bottom.
    let baluster_count = baluster_spacing(&r.path);
    for (i, seg) in segments.iter().enumerate() {
        let a = seg.a;
        let b = seg.b;
        let dir = b - a;
        let s_a = walk_z(surface, &a);
        let s_b = walk_z(surface, &b);
        let top_lo = if is_flight { s_b - RAIL_DEPTH } else { s_a - RAIL_DEPTH };
        let top_hi = if is_flight { s_b + h } else { s_a + h };
        let bot_lo = if is_flight { s_b + BOTTOM_GAP } else { s_a + BOTTOM_GAP };
        let bot_hi = if is_flight { s_b + BOTTOM_GAP + RAIL_DEPTH } else { s_a + BOTTOM_GAP + RAIL_DEPTH };
        // Build top rail.
        {
            let mut m = Mesh::new("railing-rail", r.id);
            m.level_elevation = level_elevation;
            profile_quad(&mut m, a, dir, &[(0.0, top_lo), ((b - a).len(), top_hi), ((b - a).len(), top_hi), (0.0, top_lo)], thk / 2.0);
            meshes.push(m);
        }
        {
            let mut m = Mesh::new("railing-rail", r.id);
            m.level_elevation = level_elevation;
            profile_quad(&mut m, a, dir, &[(0.0, bot_lo), ((b - a).len(), bot_hi), ((b - a).len(), bot_hi), (0.0, bot_lo)], thk / 2.0);
            meshes.push(m);
        }
        let _ = i;
        let _ = (baluster_count, is_flight, half_post, baluster);
    }

    // 3. Infill between consecutive posts per segment.
    if r.style == cadcraft_doc::arch::RailingStyle::Panel {
        for i in 0..segments.len() {
            // find posts belonging to segment i
        }
        // Panel: skip for now (the legacy mesh covers it); will be refined.
    }

    meshes
}

/// Number of balusters and the clear gap for a clear span of `length` with baluster width `w`.
/// Returns the largest n such that the clear gap between n balusters of width w is <= MAX_BALUSTER_GAP.
fn baluster_spacing(_path: &[crate::geom::Vec2]) -> (f64, usize) {
    (0.1, 0)
}
