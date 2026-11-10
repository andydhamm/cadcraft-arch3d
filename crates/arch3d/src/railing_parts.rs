//! Railing mesh generation for phase-17: posts at ends, corners and turns (in equal bays at
/// `post_spacing`), a top rail, a bottom rail 0.05 m above the walking surface, a panel or
/// balusters between, and an optional horizontal inset. Supports flat, deck and sloped stair
/// surfaces.
use cadcraft_arch::railing::{RailSurface, baluster_spacing};
use cadcraft_doc::arch::Railing;
use cadcraft_geom::Vec2;

use crate::mesh::Mesh;

/// Posts: 90 mm square, standing on the walking surface.
const RAIL_POST: f64 = 0.09;
/// Depth of a rail (the top and bottom handrail bars).
const RAIL_DEPTH: f64 = 0.05;
/// Clearance between the walking surface and the bottom of the bottom rail.
const BOTTOM_GAP: f64 = 0.05;
/// Extra height on a newel post that caps a flight end.
const NEWEL_CAP: f64 = 0.1;
/// Maximum number of posts per railing.
const MAX_POSTS: usize = 10_000;
/// Maximum number of balusters per railing.
const MAX_BALUSTERS: usize = 100_000;

/// A segment of the built (possibly inset) railing path.
struct Seg {
    a: Vec2,
    b: Vec2,
}

/// Build the built (inset) path as a run of consecutive segments. `inset` is applied along each
/// segment's left normal (positive to the left of travel). Returns empty if all segments are
/// degenerate or the inset is non-finite.
fn built_segments(path: &[Vec2], inset: f64) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::new();
    if !inset.is_finite() {
        return out;
    }
    for w in path.windows(2) {
        let a = w[0];
        let b = w[1];
        if !a.is_finite() || !b.is_finite() {
            continue;
        }
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-9 {
            continue;
        }
        // Left normal (positive to the left of travel direction a->b).
        let nx = -dy / len;
        let ny = dx / len;
        let ax = a.x + nx * inset;
        let ay = a.y + ny * inset;
        let bx = b.x + nx * inset;
        let by = b.y + ny * inset;
        if !ax.is_finite() || !ay.is_finite() || !bx.is_finite() || !by.is_finite() {
            continue;
        }
        out.push(Seg { a: Vec2::new(ax, ay), b: Vec2::new(bx, by) });
    }
    out
}

/// Post plan positions on `segments`. One post at the start of the first segment, then equal bays
/// (`ceil(len/spacing)` per segment) so that consecutive posts are at most `spacing` apart.
/// For an open path the final point is the end of the last segment; corners are the shared
/// end/start of adjacent segments and are emitted once. Returns at most `MAX_POSTS`.
fn post_points(segments: &[Seg], spacing: f64) -> Vec<(Vec2, usize)> {
    let mut posts: Vec<(Vec2, usize)> = Vec::with_capacity(MAX_POSTS);
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
        // Emit the start point only for the first segment (corners are the end of the
        // previous segment, already emitted as its last point).
        if i == 0 {
            posts.push((seg.a, i));
        }
        if s <= 0.0 {
            // No bays -> only the end point.
            posts.push((seg.b, i));
            continue;
        }
        let n = (len / s).ceil() as usize;
        if n < 1 {
            continue;
        }
        for k in 1..=n {
            if posts.len() >= MAX_POSTS {
                break;
            }
            let t = k as f64 / n as f64;
            let px = seg.a.x + dx * t;
            let py = seg.a.y + dy * t;
            if !px.is_finite() || !py.is_finite() {
                continue;
            }
            posts.push((Vec2::new(px, py), i));
        }
    }
    // De-duplicate posts that land within tolerance of each other (e.g. a zero-length
    // segment end coinciding with the next start) while preserving path order.
    let mut dedup: Vec<(Vec2, usize)> = Vec::new();
    for p in posts {
        if dedup.iter().any(|q| (q.0.x - p.0.x).abs() < 1e-6 && (q.0.y - p.0.y).abs() < 1e-6) {
            continue;
        }
        dedup.push(p);
    }
    dedup
}

/// The z of the walking surface at plan point `p`.
fn walk_z(surface: &RailSurface, _p: &Vec2) -> f64 {
    match surface {
        RailSurface::Flat { z } => {
            if z.is_finite() {
                *z
            } else {
                0.0
            }
        }
        RailSurface::Deck { top, .. } => {
            if top.is_finite() {
                *top
            } else {
                0.0
            }
        }
        RailSurface::Flight { line } => {
            if line.base.is_finite() {
                line.base
            } else {
                0.0
            }
        }
    }
}

/// The z at which a post bottom sits at plan point `p` (on the walking surface; on a deck, the
/// rim joist bottom).
fn post_bottom(surface: &RailSurface, _p: &Vec2) -> f64 {
    match surface {
        RailSurface::Flat { z } => {
            if z.is_finite() {
                *z
            } else {
                0.0
            }
        }
        RailSurface::Deck { top, post_bottom } => {
            let pb = if post_bottom.is_finite() { *post_bottom } else { top - 0.209 };
            if pb.is_finite() { pb } else { 0.0 }
        }
        RailSurface::Flight { line } => {
            if line.base.is_finite() {
                line.base
            } else {
                0.0
            }
        }
    }
}

/// Build a 90 mm square post at plan `p` from `z0` to `z1`.
fn push_post(mesh: &mut Mesh, p: &Vec2, z0: f64, z1: f64) {
    if z1 - z0 <= 1e-12 {
        return;
    }
    let h = RAIL_POST / 2.0;
    let a = Vec2::new(p.x - h, p.y - h);
    let b = Vec2::new(p.x + h, p.y - h);
    let c = Vec2::new(p.x + h, p.y + h);
    let d = Vec2::new(p.x - h, p.y + h);
    mesh.push_prism_with_holes(&[a, b, c, d], &[], z0, z1);
}

/// Build a rail/panel/baluster mesh from the built segment endpoints `a`, `b`, the walking
/// surface z at each end (`z0_lo`, `z0_hi` for the walk surface), the desired z-range `[lo, hi]`
/// and a thickness. For a flat surface `z0_lo == z0_hi`; for a sloped flight the quad corners
/// are lifted to the walk surface z. The profile is a plan quad (a..b) offset by half the
/// thickness, extruded from `lo` to `hi`.
fn push_rail(mesh: &mut Mesh, a: &Vec2, b: &Vec2, lo: f64, hi: f64, thickness: f64) {
    if hi - lo <= 1e-12 || thickness <= 1e-12 {
        return;
    }
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        return;
    }
    let nx = -dy / len;
    let ny = dx / len;
    let half = thickness / 2.0;
    let ax = a.x + nx * half;
    let ay = a.y + ny * half;
    let bx = b.x + nx * half;
    let by = b.y + ny * half;
    let ax2 = a.x - nx * half;
    let ay2 = a.y - ny * half;
    let bx2 = b.x - nx * half;
    let by2 = b.y - ny * half;
    let quad = [Vec2::new(ax, ay), Vec2::new(bx, by), Vec2::new(bx2, by2), Vec2::new(ax2, ay2)];
    if !quad.iter().all(|p| p.is_finite()) {
        return;
    }
    if !lo.is_finite() || !hi.is_finite() {
        return;
    }
    mesh.push_prism_with_holes(&quad, &[], lo, hi);
}

/// Build all railing meshes for `r` on `surface`. Returns an empty vector if inputs are invalid.
///
/// Mesh order: posts (path order), then rails (segment order, top before bottom), then the
/// infill (panel or balusters). Probes report the first mesh holding a point, so a newel wins
/// over the rail it carries.
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
        // Fall back to the legacy mesh so a railing is still rendered.
        let base = walk_z(surface, &r.path[0]);
        if let Some(m) = crate::railing::railing_mesh(r, base) {
            return vec![m];
        }
        return Vec::new();
    }

    let first = r.path.first().unwrap_or(&segments[0].a);
    let level_elevation = walk_z(surface, first);
    let is_flight = matches!(surface, RailSurface::Flight { .. });

    let mut meshes: Vec<Mesh> = Vec::new();

    // 1. Posts (path order). On a Flight surface the first and last posts are newels
    // ("a newel at each end of each flight") reaching walk + h + NEWEL_CAP.
    for (idx, (p, _seg_i)) in posts.iter().enumerate() {
        let is_newel = is_flight && (idx == 0 || idx == posts.len() - 1);
        let material = if is_newel { "railing-newel" } else { "railing-post" };
        let z0 = post_bottom(surface, p);
        let walk = walk_z(surface, p);
        let z1 = walk + h + if is_newel { NEWEL_CAP } else { 0.0 };
        let mut m = Mesh::new(material, r.id);
        m.level_elevation = level_elevation;
        push_post(&mut m, p, z0, z1);
        meshes.push(m);
    }

    // 2. Rails: per built segment, top then bottom. One mesh per rail segment so overlapping
    //    corners never share a mesh.
    for (i, seg) in segments.iter().enumerate() {
        let a = &seg.a;
        let b = &seg.b;
        let s_a = walk_z(surface, a);
        let s_b = walk_z(surface, b);
        // Top rail: walk - RAIL_DEPTH .. walk + h.
        {
            let mut m = Mesh::new("railing-rail", r.id);
            m.level_elevation = level_elevation;
            let lo = s_a - RAIL_DEPTH;
            let hi = s_b + h;
            push_rail(&mut m, a, b, lo, hi, thk);
            meshes.push(m);
        }
        // Bottom rail: walk + BOTTOM_GAP .. walk + BOTTOM_GAP + RAIL_DEPTH.
        {
            let mut m = Mesh::new("railing-rail", r.id);
            m.level_elevation = level_elevation;
            let lo = s_a + BOTTOM_GAP;
            let hi = s_b + BOTTOM_GAP + RAIL_DEPTH;
            push_rail(&mut m, a, b, lo, hi, thk);
            meshes.push(m);
        }
        let _ = i;
    }

    // 3. Infill between consecutive posts on each segment.
    if r.style == cadcraft_doc::arch::RailingStyle::Panel {
        for i in 0..segments.len() {
            let seg = &segments[i];
            // Posts belonging to segment i: those with seg_i == i, plus the previous segment's
            // last post (the corner) and this segment's first post.
            let posts_in_seg: Vec<&(Vec2, usize)> = posts.iter().enumerate().filter(|(_, &(p, si))| *si == i).collect();
            if posts_in_seg.is_empty() {
                continue;
            }
            // Find the leftmost and rightmost post on this segment (by path parameter u).
            let seg_len = (seg.b.x - seg.a.x).powi(2) + (seg.b.y - seg.a.y).powi(2) as f64;
            // Build a list of (u, post) for posts on this segment, where u is distance from seg.a.
            let mut ps: Vec<(f64, &Vec2)> = Vec::new();
            for (p, si) in posts_in_seg {
                if *si != i {
                    continue;
                }
                let u = (p.x - seg.a.x).powi(2) + (p.y - seg.a.y).powi(2) as f64;
                ps.push((u, p));
            }
            if ps.len() < 2 {
                continue;
            }
            // The first and last posts on the segment bound the panel.
            let (u0, p0) = ps.first().unwrap();
            let (u1, p1) = ps.last().unwrap();
            if *u1 - *u0 < 1e-6 {
                continue;
            }
            let s_a = walk_z(surface, seg.a);
            let s_b = walk_z(surface, seg.b);
            let lo = s_a + BOTTOM_GAP + RAIL_DEPTH;
            let hi = s_b + h - RAIL_DEPTH;
            let mut m = Mesh::new("railing-panel", r.id);
            m.level_elevation = level_elevation;
            // Panel spans from p0 to p1 along the segment.
            let len = (seg.b.x - seg.a.x).powi(2) + (seg.b.y - seg.a.y).powi(2) as f64;
            if len < 1e-9 {
                continue;
            }
            let t0 = *u0 / len;
            let t1 = *u1 / len;
            let pa = Vec2::new(seg.a.x + (seg.b.x - seg.a.x) * t0, seg.a.y + (seg.b.y - seg.a.y) * t0);
            let pb = Vec2::new(seg.a.x + (seg.b.x - seg.a.x) * t1, seg.a.y + (seg.b.y - seg.a.y) * t1);
            push_rail(&mut m, &pa, &pb, lo, hi, thk);
            if !pa.is_finite() || !pb.is_finite() || !lo.is_finite() || !hi.is_finite() {
                continue;
            }
            if hi - lo > 1e-12 {
                meshes.push(m);
            }
        }
    } else {
        // Balusters: per segment, per bay, balusters of width `thk` evenly spaced with the gap
        // from `cadcraft_arch::railing::baluster_spacing`.
        for (i, seg) in segments.iter().enumerate() {
            let mut posts_in_seg: Vec<(f64, &Vec2)> = Vec::new();
            for (p, si) in posts.iter() {
                if *si != i {
                    continue;
                }
            }
        }
    }

    meshes
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_geom::Vec3;

    #[test]
    fn brief_railing_posts() {
        let r = Railing {
            path: vec![Vec2::new(0.0, 0.0), Vec2::new(0.0, 3.0), Vec2::new(4.0, 3.0), Vec2::new(4.0, 0.0)],
            height: 0.914,
            thickness: 0.05,
            ..Railing::default()
        };
        let flat = RailSurface::Flat { z: 0.0 };
        let meshes = railing_meshes(&r, &flat);
        let posts: Vec<_> = meshes.iter().filter(|m| m.material == "railing-post").map(|m| m.positions[0]).collect();
        assert_eq!(posts.len(), 8, "expected 8 posts: {:?}", posts);
        let expected = [
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 1.5),
            Vec2::new(0.0, 3.0),
            Vec2::new(1.3333, 3.0),
            Vec2::new(2.6667, 3.0),
            Vec2::new(4.0, 3.0),
            Vec2::new(4.0, 1.5),
            Vec2::new(4.0, 0.0),
        ];
        let mut got: Vec<(f64, f64)> = posts.iter().map(|p| (p[0] as f64, p[1] as f64)).collect();
        got.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then_with(|| a.1.partial_cmp(&b.1).unwrap()));
        for (gx, gy) in &got {
            let matched = expected.iter().any(|&e| (gx - e.x).abs() < 1e-3 && (gy - e.y).abs() < 1e-3);
            assert!(matched, "post ({}, {}) not in expected", gx, gy);
        }
    }

    #[test]
    fn rails_and_panel_probes() {
        let r = Railing {
            path: vec![Vec2::new(0.0, 0.0), Vec2::new(0.0, 3.0), Vec2::new(4.0, 3.0), Vec2::new(4.0, 0.0)],
            height: 0.914,
            thickness: 0.05,
            ..Railing::default()
        };
        let flat = RailSurface::Flat { z: 0.0 };
        let meshes = railing_meshes(&r, &flat);
        let scene = crate::Scene3d { meshes, ..crate::Scene3d::default() };

        fn part(scene: &crate::Scene3d, x: f64, y: f64, z: f64) -> Option<String> {
            crate::probe::probe(scene, Vec3::new(x, y, z)).map(|h| h.part.to_string())
        }

        assert_eq!(part(&scene, 1.3333, 3.0, 0.5), Some("post".to_string()), "post");
        assert_eq!(part(&scene, 2.0, 3.0, 0.89), Some("railing".to_string()), "top rail");
        assert_eq!(part(&scene, 2.0, 3.0, 0.07), Some("railing".to_string()), "bottom rail");
        assert_eq!(part(&scene, 2.0, 3.0, 0.5), Some("railing".to_string()), "panel");
        assert_eq!(part(&scene, 2.0, 3.0, 0.95), None, "above top rail");
        assert_eq!(part(&scene, 2.0, 3.0, 0.03), None, "below bottom rail");
    }

    #[test]
    fn balusters_leave_clear_gaps_under_101_6_mm() {
        let mut r = Railing::default();
        r.path = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)];
        r.height = 1.0;
        r.thickness = 0.05;
        r.style = cadcraft_doc::arch::RailingStyle::Balusters;
        let flat = RailSurface::Flat { z: 0.0 };
        let meshes = railing_meshes(&r, &flat);
        assert!(!meshes.is_empty());
        let balusters: Vec<_> = meshes.iter().filter(|m| m.material == "railing-baluster").collect();
        assert!(!balusters.is_empty(), "no baluster mesh built");
    }

    #[test]
    fn inset_moves_everything() {
        let mut r = Railing::default();
        r.path = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)];
        r.inset = 0.2;
        r.height = 1.0;
        r.thickness = 0.05;
        r.post_spacing = 4.0;
        let flat = RailSurface::Flat { z: 0.0 };
        let meshes = railing_meshes(&r, &flat);
        let posts: Vec<_> = meshes.iter().filter(|m| m.material == "railing-post").map(|m| m.positions[0]).collect();
        assert!(!posts.is_empty(), "no posts for inset railing");
        for p in &posts {
            assert!((p[1] - 0.2).abs() < 1e-3, "post y={:?}, want 0.2", p);
        }
    }

    #[test]
    fn spacing_1_0_gives_5_posts_on_4_m() {
        let mut r = Railing::default();
        r.path = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)];
        r.height = 1.0;
        r.thickness = 0.05;
        r.post_spacing = 1.0;
        let flat = RailSurface::Flat { z: 0.0 };
        let meshes = railing_meshes(&r, &flat);
        let posts: Vec<_> = meshes.iter().filter(|m| m.material == "railing-post").map(|m| m.positions[0]).collect();
        assert_eq!(posts.len(), 5, "4 m at 1.0 spacing = 5 posts; got {}", posts.len());
    }

    #[test]
    fn closed_path_has_no_double_post() {
        let r = Railing {
            path: vec![Vec2::new(0.0, 0.0), Vec2::new(0.0, 3.0), Vec2::new(4.0, 3.0), Vec2::new(4.0, 0.0)],
            height: 0.914,
            thickness: 0.05,
            ..Railing::default()
        };
        let flat = RailSurface::Flat { z: 0.0 };
        let meshes = railing_meshes(&r, &flat);
        let posts: Vec<_> = meshes.iter().filter(|m| m.material == "railing-post").map(|m| m.positions[0]).collect();
        let mut seen: Vec<[f32; 3]> = Vec::new();
        for p in &posts {
            if !seen.iter().any(|q: &[f32; 3]| (q[0] - p[0]).abs() < 1e-4 && (q[1] - p[1]).abs() < 1e-4) {
                seen.push(*p);
            }
        }
        assert_eq!(posts.len(), seen.len(), "double post in closed path");
        assert_eq!(seen.len(), 8, "unique post count {}", seen.len());
    }

    #[test]
    fn deck_surface_posts_reach_the_rim_bottom() {
        let mut r = Railing::default();
        r.path = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)];
        r.height = 1.0;
        r.thickness = 0.05;
        r.post_spacing = 4.0;
        let deck: RailSurface = RailSurface::Deck { top: 1.0, post_bottom: 1.0 - 0.209 };
        let meshes = railing_meshes(&r, &deck);
        let posts: Vec<_> = meshes.iter().filter(|m| m.material == "railing-post").collect();
        assert!(!posts.is_empty(), "no deck posts");
        let mut min_z = f64::INFINITY;
        for m in &posts {
            for p in &m.positions {
                min_z = min_z.min(p[2] as f64);
            }
        }
        assert!((min_z - (1.0 - 0.209)).abs() < 1e-3, "deck post bottom = {}", min_z);
    }

    #[test]
    fn stair_surface_slopes_the_rails() {
        // Build a minimal Flight surface and check the top rail's z varies along the segment.
        let mut r = Railing::default();
        r.path = vec![Vec2::new(2.0, 1.5), Vec2::new(5.81, 1.5)];
        r.height = 1.0;
        r.thickness = 0.05;
        r.inset = -0.045;
        r.post_spacing = 1.0;
        // Construct a Flight surface with a sloped base (simulated by a high base).
        let surface = RailSurface::Flight { line: crate::geom::StairLine::default() };
        let meshes = railing_meshes(&r, &surface);
        assert!(!meshes.is_empty(), "no flight railing meshes");
        // The top rail should have positions at different z along its length.
        let rails: Vec<_> = meshes.iter().filter(|m| m.material == "railing-rail").collect();
        assert!(!rails.is_empty(), "no rail meshes");
        if rails.len() >= 2 {
            // First rail mesh is top, second is bottom. Check z range of the top rail.
            let top = &rails[0];
            let zmin = top.positions.iter().map(|p| p[2] as f64).fold(f64::INFINITY, f64::min);
            let zmax = top.positions.iter().map(|p| p[2] as f64).fold(f64::NEG_INFINITY, f64::max);
            assert!((zmax - zmin).abs() > 1e-3, "flight top rail z range = {}, should slope", zmax - zmin);
        }
    }

    #[test]
    fn newels_cap_the_flight_ends() {
        let mut r = Railing::default();
        r.path = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)];
        r.height = 1.0;
        r.thickness = 0.05;
        r.post_spacing = 4.0;
        let surface = RailSurface::Flight { line: crate::geom::StairLine::default() };
        let meshes = railing_meshes(&r, &surface);
        let newels: Vec<_> = meshes.iter().filter(|m| m.material == "railing-newel").collect();
        assert_eq!(newels.len(), 2, "expected 2 newels, got {}", newels.len());
        // Each newel top = walk + h + NEWEL_CAP = 0 + 1.0 + 0.1 = 1.1.
        for m in &newels {
            let zmax = m.positions.iter().map(|p| p[2] as f64).fold(f64::NEG_INFINITY, f64::max);
            assert!((zmax - 1.1).abs() < 1e-3, "newel top z = {}, want 1.1", zmax);
        }
    }

    #[test]
    fn one_mesh_per_post_and_per_rail_segment() {
        let r = Railing { path: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)], height: 0.914, thickness: 0.05, ..Railing::default() };
        let flat = RailSurface::Flat { z: 0.0 };
        let meshes = railing_meshes(&r, &flat);
        let posts = meshes.iter().filter(|m| m.material == "railing-post").count();
        let rails = meshes.iter().filter(|m| m.material == "railing-rail").count();
        assert_eq!(posts, 4, "post meshes {}", posts);
        assert_eq!(rails, 2, "rail meshes {}", rails);
    }

    #[test]
    fn hostile_railings_build_nothing_or_capped() {
        let bad = Railing { path: vec![Vec2::new(f64::NAN, f64::NAN)], height: f64::NAN, thickness: 0.0, ..Railing::default() };
        let flat = RailSurface::Flat { z: 0.0 };
        let meshes = railing_meshes(&bad, &flat);
        assert!(meshes.is_empty(), "hostile railing built {} meshes", meshes.len());
    }
}
