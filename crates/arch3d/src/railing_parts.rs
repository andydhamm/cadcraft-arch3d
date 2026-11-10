//! Railing mesh generation for phase-17: posts at ends, corners and turns (in equal bays at
//! `post_spacing`), a top rail, a bottom rail 0.05 m above the walking surface, a panel or
//! balusters between, and an optional inset. Supports flat, deck and sloped stair surfaces.
use cadcraft_arch::railing::{RailSurface, baluster_spacing, built_path, post_points};
use cadcraft_doc::arch::{Railing, RailingStyle};
use cadcraft_geom::Vec2;

use crate::mesh::Mesh;
use crate::solid::{self, Frame};

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

fn walk_z(surface: &RailSurface, p: &Vec2) -> f64 {
    match surface {
        RailSurface::Flat { z } => *z,
        RailSurface::Deck { top, .. } => *top,
        RailSurface::Flight { line } => {
            let u = line.local(*p).0.clamp(0.0, line.run.max(0.0));
            line.base + line.nosing_v(u)
        }
    }
}

fn post_bottom(surface: &RailSurface, p: &Vec2) -> f64 {
    match surface {
        RailSurface::Flat { z } => *z,
        RailSurface::Deck { post_bottom, .. } => *post_bottom,
        RailSurface::Flight { line } => {
            let u = line.local(*p).0.clamp(0.0, line.run.max(0.0));
            line.base + line.nosing_v(u)
        }
    }
}

fn push_post(mesh: &mut Mesh, p: Vec2, z0: f64, z1: f64, axis: Vec2) {
    if z1 - z0 <= 1e-12 || !p.is_finite() || !axis.is_finite() {
        return;
    }
    let len = axis.len();
    if len < 1e-12 || !len.is_finite() {
        return;
    }
    solid::push_box(mesh, p, axis / len, RAIL_POST, RAIL_POST, z0, z1);
}

fn segment_axis_for_point(path: &[Vec2], p: Vec2) -> Vec2 {
    let mut best: Option<Vec2> = None;
    let mut best_dist = f64::INFINITY;
    for w in path.windows(2) {
        let a = w[0];
        let b = w[1];
        let d = b - a;
        let len = d.len();
        if !len.is_finite() || len <= 1e-9 {
            continue;
        }
        let proj = (p - a).dot(d) / len;
        if !(proj >= 0.0 && proj <= len) {
            continue;
        }
        let q = a + (d / len) * proj;
        let dist = (p - q).len();
        if dist < best_dist {
            best_dist = dist;
            best = Some(d / len);
        }
    }
    best.unwrap_or(Vec2::new(1.0, 0.0))
}

fn segment_bays(path: &[Vec2], posts: &[Vec2]) -> Vec<(Vec2, Vec2)> {
    if path.len() < 2 || posts.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for w in path.windows(2) {
        let a = w[0];
        let b = w[1];
        let d = b - a;
        let len = d.len();
        if !len.is_finite() || len <= 1e-9 {
            continue;
        }
        let mut local: Vec<(f64, Vec2)> = Vec::new();
        for &p in posts {
            let delta = p - a;
            let proj = delta.dot(d) / len;
            if proj < -1e-6 || proj > len + 1e-6 {
                continue;
            }
            local.push((proj, p));
        }
        if local.len() < 2 {
            continue;
        }
        local.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
        for win in local.windows(2) {
            let u0 = win[0].0;
            let u1 = win[1].0;
            if u1 - u0 < 1e-9 {
                continue;
            }
            let p0 = a + d * (u0 / len);
            let p1 = a + d * (u1 / len);
            out.push((p0, p1));
        }
    }
    out
}

/// Build all railing meshes for `r` on `surface`.
pub fn railing_meshes(r: &Railing, surface: &RailSurface) -> Vec<Mesh> {
    let h = r.height;
    let thk = r.thickness;
    if !h.is_finite() || h <= 0.0 || !thk.is_finite() || thk <= 0.0 {
        return Vec::new();
    }

    let built = built_path(&r.path, r.inset);
    if built.len() < 2 {
        return Vec::new();
    }
    let posts = post_points(&built, r.post_spacing);
    if posts.is_empty() {
        return Vec::new();
    }

    let level_elevation = walk_z(surface, &built[0]);
    let is_flight = matches!(surface, RailSurface::Flight { .. });
    let mut meshes: Vec<Mesh> = Vec::new();

    for (idx, p) in posts.iter().enumerate() {
        let is_newel = is_flight && (idx == 0 || idx == posts.len() - 1);
        let material = if is_newel { "railing-newel" } else { "railing-post" };
        let z0 = post_bottom(surface, p);
        let walk = walk_z(surface, p);
        let z1 = walk + h + if is_newel { NEWEL_CAP } else { 0.0 };
        let mut m = Mesh::new(material, r.id);
        m.level_elevation = level_elevation;
        push_post(&mut m, *p, z0, z1, segment_axis_for_point(&built, *p));
        if !m.indices.is_empty() {
            meshes.push(m);
        }
    }

    for w in built.windows(2) {
        let a = w[0];
        let b = w[1];
        let len = (b - a).len();
        if !len.is_finite() || len <= 1e-9 {
            continue;
        }
        let Some(frame) = Frame::new(a, b - a, 0.0) else { continue };

        let z_ta = walk_z(surface, &a) + h - RAIL_DEPTH;
        let z_tb = walk_z(surface, &b) + h - RAIL_DEPTH;
        let z_ha = walk_z(surface, &a) + h;
        let z_hb = walk_z(surface, &b) + h;
        let mut top = Mesh::new("railing-rail", r.id);
        top.level_elevation = level_elevation;
        solid::push_profile(
            &mut top,
            &[Vec2::new(0.0, z_ta), Vec2::new(len, z_tb), Vec2::new(len, z_hb), Vec2::new(0.0, z_ha)],
            -thk / 2.0,
            thk / 2.0,
            &frame,
        );
        if !top.indices.is_empty() {
            meshes.push(top);
        }

        let z_ba = walk_z(surface, &a) + BOTTOM_GAP;
        let z_bb = walk_z(surface, &b) + BOTTOM_GAP;
        let z_bha = walk_z(surface, &a) + BOTTOM_GAP + RAIL_DEPTH;
        let z_bhb = walk_z(surface, &b) + BOTTOM_GAP + RAIL_DEPTH;
        let mut bottom = Mesh::new("railing-rail", r.id);
        bottom.level_elevation = level_elevation;
        solid::push_profile(
            &mut bottom,
            &[Vec2::new(0.0, z_ba), Vec2::new(len, z_bb), Vec2::new(len, z_bhb), Vec2::new(0.0, z_bha)],
            -thk / 2.0,
            thk / 2.0,
            &frame,
        );
        if !bottom.indices.is_empty() {
            meshes.push(bottom);
        }
    }

    if r.style == RailingStyle::Panel {
        let mut panel = Mesh::new("railing-panel", r.id);
        panel.level_elevation = level_elevation;
        for (p0, p1) in segment_bays(&built, &posts) {
            let d = p1 - p0;
            let len = d.len();
            if !len.is_finite() || len <= 1e-9 {
                continue;
            }
            let Some(frame) = Frame::new(p0, d, 0.0) else { continue };
            let z0 = walk_z(surface, &p0) + BOTTOM_GAP + RAIL_DEPTH;
            let z1 = walk_z(surface, &p1) + BOTTOM_GAP + RAIL_DEPTH;
            let z2 = walk_z(surface, &p1) + h - RAIL_DEPTH;
            let z3 = walk_z(surface, &p0) + h - RAIL_DEPTH;
            solid::push_profile(
                &mut panel,
                &[Vec2::new(0.0, z0), Vec2::new(len, z1), Vec2::new(len, z2), Vec2::new(0.0, z3)],
                -thk / 2.0,
                thk / 2.0,
                &frame,
            );
        }
        if !panel.indices.is_empty() {
            meshes.push(panel);
        }
    } else {
        let mut balusters = Mesh::new("railing-baluster", r.id);
        balusters.level_elevation = level_elevation;
        for (p0, p1) in segment_bays(&built, &posts) {
            let d = p1 - p0;
            let len = d.len();
            if !len.is_finite() || len <= 1e-9 {
                continue;
            }
            let dir = d / len;
            let Some((gap, count)) = baluster_spacing(len, thk) else {
                continue;
            };
            if count == 0 || gap <= 0.0 {
                continue;
            }
            let step = thk + gap;
            for i in 0..count {
                let u = gap / 2.0 + i as f64 * step;
                if u > len - gap / 2.0 {
                    break;
                }
                let center = p0 + dir * u;
                let z0 = walk_z(surface, &center) + BOTTOM_GAP + RAIL_DEPTH;
                let z1 = walk_z(surface, &center) + h - RAIL_DEPTH;
                solid::push_box(&mut balusters, center, dir, thk, thk, z0, z1);
            }
        }
        if !balusters.indices.is_empty() {
            meshes.push(balusters);
        }
    }

    meshes
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_geom::Vec3;

    fn centre(m: &Mesh) -> [f64; 2] {
        let b = m.bounds();
        [(b.min[0] + b.max[0]) / 2.0, (b.min[1] + b.max[1]) / 2.0]
    }

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
        let posts: Vec<_> = meshes.iter().filter(|m| m.material == "railing-post").map(centre).collect();
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
        let mut unmatched = expected.to_vec();
        for p in &posts {
            let mut idx = None;
            for (i, e) in unmatched.iter().enumerate() {
                if (p[0] - e.x).abs() < 1e-3 && (p[1] - e.y).abs() < 1e-3 {
                    idx = Some(i);
                    break;
                }
            }
            if let Some(i) = idx {
                unmatched.remove(i);
            }
        }
        assert!(unmatched.is_empty(), "unmatched post centres: {:?}", unmatched);
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

        assert_eq!(part(&scene, 1.3333, 3.0, 0.5), Some("post".to_string()));
        assert_eq!(part(&scene, 2.0, 3.0, 0.89), Some("railing".to_string()));
        assert_eq!(part(&scene, 2.0, 3.0, 0.07), Some("railing".to_string()));
        assert_eq!(part(&scene, 2.0, 3.0, 0.5), Some("railing".to_string()));
        assert_eq!(part(&scene, 2.0, 3.0, 0.95), None);
        assert_eq!(part(&scene, 2.0, 3.0, 0.03), None);
    }

    #[test]
    fn balusters_leave_clear_gaps_under_101_6_mm() {
        let mut r = Railing::default();
        r.path = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)];
        r.height = 1.0;
        r.thickness = 0.05;
        r.style = RailingStyle::Balusters;
        let flat = RailSurface::Flat { z: 0.0 };
        let meshes = railing_meshes(&r, &flat);
        assert!(meshes.iter().any(|m| m.material == "railing-baluster"), "baluster mesh should be present");
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
        let posts: Vec<_> = meshes.iter().filter(|m| m.material == "railing-post").map(centre).collect();
        assert!(!posts.is_empty());
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
        let posts: Vec<_> = meshes.iter().filter(|m| m.material == "railing-post").map(centre).collect();
        assert_eq!(posts.len(), 5, "4 m at 1.0 spacing = 5 posts; got {:?}", posts);
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
        let posts: Vec<_> = meshes.iter().filter(|m| m.material == "railing-post").map(centre).collect();
        let mut seen: Vec<[f64; 2]> = Vec::new();
        for p in &posts {
            if !seen.iter().any(|q| (q[0] - p[0]).abs() < 1e-4 && (q[1] - p[1]).abs() < 1e-4) {
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
        let deck = RailSurface::Deck { top: 1.0, post_bottom: 1.0 - 0.209 };
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
        let mut r = Railing::default();
        r.path = vec![Vec2::new(2.0, 1.5), Vec2::new(5.81, 1.5)];
        r.height = 1.0;
        r.thickness = 0.05;
        r.inset = -0.045;
        r.post_spacing = 1.0;
        let surface = RailSurface::Flight {
            line: cadcraft_arch::stair_geom::FlightLine { origin: Vec2::new(2.0, 1.5), dir: Vec2::new(1.0, 0.0), base: 0.0, ..Default::default() },
        };
        let meshes = railing_meshes(&r, &surface);
        let rails: Vec<_> = meshes.iter().filter(|m| m.material == "railing-rail").collect();
        assert!(!rails.is_empty());
        let mut zmin = f64::INFINITY;
        let mut zmax = f64::NEG_INFINITY;
        for m in &rails {
            for p in &m.positions {
                let z = p[2] as f64;
                zmin = zmin.min(z);
                zmax = zmax.max(z);
            }
        }
        assert!((zmax - zmin).abs() > 1e-4, "flight rail z range = {}", zmax - zmin);
    }

    #[test]
    fn newels_cap_the_flight_ends() {
        let mut r = Railing::default();
        r.path = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)];
        r.height = 1.0;
        r.thickness = 0.05;
        r.post_spacing = 4.0;
        let surface = RailSurface::Flight {
            line: cadcraft_arch::stair_geom::FlightLine { origin: Vec2::ZERO, dir: Vec2::new(1.0, 0.0), base: 0.0, ..Default::default() },
        };
        let meshes = railing_meshes(&r, &surface);
        let newels: Vec<_> = meshes.iter().filter(|m| m.material == "railing-newel").collect();
        assert_eq!(newels.len(), 2, "expected 2 newels, got {}", newels.len());
        for m in &newels {
            let zmax = m.positions.iter().map(|p| p[2] as f64).fold(f64::NEG_INFINITY, f64::max);
            let b = m.bounds();
            let x = (b.min[0] + b.max[0]) / 2.0;
            let y = (b.min[1] + b.max[1]) / 2.0;
            let expected = surface.walk_z(Vec2::new(x, y)) + r.height + NEWEL_CAP;
            assert!((zmax - expected).abs() < 1e-3, "newel top z = {}, expected {}", zmax, expected);
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
