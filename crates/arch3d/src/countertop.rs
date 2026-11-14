//! Custom countertop meshes.

use cadcraft_arch::countertop::CounterOutline;
use cadcraft_arch::countertop_edit::sampled_ring;
use cadcraft_doc::arch::{CounterEdge, Countertop, EdgeProfile};
use cadcraft_geom::Vec2;

use crate::mesh::Mesh;

const WATERFALL_PROUD: f64 = 0.001;

fn edge_normal(a: Vec2, b: Vec2, ccw: bool) -> Vec2 {
    let d = b - a;
    let len = d.len();
    if !(len.is_finite() && len > 1e-12) {
        return Vec2::ZERO;
    }
    let n = Vec2::new(d.y / len, -d.x / len);
    if ccw { n } else { -n }
}

/// The nose profile of an edge (x out, y up, counter-clockwise, from (0,0)); empty for square.
fn nose_profile(profile: EdgeProfile, t: f64) -> Vec<[f64; 2]> {
    match profile {
        EdgeProfile::Square => Vec::new(),
        EdgeProfile::Eased | EdgeProfile::Bevel => {
            let c = if profile == EdgeProfile::Eased { 0.003 } else { 0.01 };
            vec![[0.0, 0.0], [c, 0.0], [c, t - c], [0.0, t]]
        }
        EdgeProfile::Bullnose => {
            let r = t / 2.0;
            let steps = 12usize;
            let mut pts = vec![[0.0, 0.0]];
            for i in 1..steps {
                let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * (i as f64) / (steps as f64);
                pts.push([r * a.cos(), r * (1.0 + a.sin())]);
            }
            pts.push([0.0, t]);
            pts
        }
    }
}

/// How far the slab edge is pulled in so the nose reaches the original edge line.
fn nose_depth(profile: EdgeProfile, t: f64) -> f64 {
    match profile {
        EdgeProfile::Square => 0.0,
        EdgeProfile::Eased => 0.003,
        EdgeProfile::Bevel => 0.01,
        EdgeProfile::Bullnose => t / 2.0,
    }
}

fn waterfall_strip(a: Vec2, b: Vec2, thickness: f64, ccw: bool) -> Option<Vec<Vec2>> {
    let d = b - a;
    let len = d.len();
    if !(len.is_finite() && len > 1e-12) {
        return None;
    }
    let outward = edge_normal(a, b, ccw);
    let inward = -outward;
    let p0 = a + outward * WATERFALL_PROUD;
    let p1 = b + outward * WATERFALL_PROUD;
    let p2 = b + inward * thickness;
    let p3 = a + inward * thickness;
    Some(vec![p0, p1, p2, p3])
}

/// The meshes of one custom countertop (`countertop*` materials, object = the countertop), built
/// from its resolved `outline` (`None` when it has none), `level` being its level's elevation.
///
/// The slab is one prism over the outline (arc edges sampled), cut by the sink holes.
pub fn countertop_meshes(ct: &Countertop, outline: Option<&CounterOutline>, level: f64) -> Vec<Mesh> {
    let Some(o) = outline else { return Vec::new() };
    let ccw = o.ring.len() >= 3 && cadcraft_arch::join::polygon_area(&o.ring) >= 0.0;
    let mut meshes = Vec::new();

    // Ring edge k (counter-clockwise) is polygon edge `src(k)`; see `custom_outline`.
    let n = o.ring.len();
    let cw = cadcraft_arch::join::polygon_area(&ct.polygon) < 0.0;
    let src = |k: usize| if cw { (2 * n).saturating_sub(2 + k) % n.max(1) } else { k };
    let arc = |k: usize| o.sagittas.get(k).is_some_and(|s| *s != 0.0);
    let mut depths: Vec<f64> = (0..n)
        .map(|k| {
            let e = ct.edge(src(k));
            if e.profile == EdgeProfile::Square || arc(k) || e.waterfall || arc((k + n - 1) % n) || arc((k + 1) % n) {
                0.0
            } else {
                nose_depth(e.profile, o.thickness)
            }
        })
        .collect();
    let slab_ring = match cadcraft_arch::poly::offset_polygon(&o.ring, &depths, false) {
        Some(r) if r.len() == n => r,
        _ => {
            depths.iter_mut().for_each(|d| *d = 0.0);
            o.ring.clone()
        }
    };
    let z0 = level + o.top - o.thickness;
    for k in 0..n {
        if depths.get(k).copied().unwrap_or(0.0) <= 0.0 {
            continue;
        }
        let (Some(&a), Some(&b)) = (slab_ring.get(k), slab_ring.get((k + 1) % n)) else { continue };
        let prof = nose_profile(ct.edge(src(k)).profile, o.thickness);
        if let Some(mut m) = crate::sweep::sweep(&prof, &[a, b], &[z0, z0], false, -1.0, "countertop", ct.id) {
            m.level_elevation = level;
            if !m.indices.is_empty() {
                meshes.push(m);
            }
        }
    }

    let sagittas: Vec<f64> = (0..ct.polygon.len()).map(|i| ct.edge(i).sagitta).collect();
    let mut slab = Mesh::new("countertop", ct.id);
    slab.level_elevation = level;
    slab.push_prism_with_holes(&sampled_ring(&slab_ring, &sagittas), &o.holes, z0, level + o.top);
    if !slab.indices.is_empty() {
        meshes.push(slab);
    }

    let mut waterfall = Mesh::new("countertop-waterfall", ct.id);
    waterfall.level_elevation = level;
    for i in 0..ct.polygon.len() {
        if ct.edge(i).waterfall && ct.edge(i).sagitta == 0.0 {
            let a = ct.polygon[i];
            let b = ct.polygon[(i + 1) % ct.polygon.len()];
            if let Some(ring) = waterfall_strip(a, b, ct.thickness, ccw) {
                waterfall.push_prism(&ring, level, level + ct.top - ct.thickness);
            }
        }
    }
    if !waterfall.indices.is_empty() {
        meshes.push(waterfall);
    }
    meshes
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_arch::countertop::custom_outline;
    use cadcraft_doc::ArchModel;
    use cadcraft_geom::Vec2;

    #[test]
    fn debug_single_bullnose_meshes() {
        let mut m = ArchModel::default();
        m.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ..Default::default() });
        let mut ct = Countertop {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 1.0), Vec2::new(0.0, 1.0)],
            edges: vec![CounterEdge::default(); 4],
            top: 0.914,
            thickness: 0.038,
            ..Default::default()
        };
        ct.edges[0].profile = EdgeProfile::Bullnose;
        let id = m.push_countertop(ct.clone());
        let outline = custom_outline(&m, &ct).unwrap();
        let meshes = countertop_meshes(&ct, Some(&outline), 0.0);
        assert!(!meshes.is_empty());
    }

    fn bull_counter(cw: bool, profile: EdgeProfile) -> Vec<Mesh> {
        let mut m = ArchModel::default();
        let mut poly = vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 1.0), Vec2::new(0.0, 1.0)];
        let mut ct = Countertop { top: 0.914, thickness: 0.038, edges: vec![CounterEdge::default(); 4], ..Default::default() };
        if cw {
            poly.reverse();
            ct.edges[2].profile = profile;
        } else {
            ct.edges[0].profile = profile;
        }
        ct.polygon = poly;
        let id = m.push_countertop(ct);
        let ct = m.countertop(id).cloned().unwrap_or_default();
        let o = custom_outline(&m, &ct);
        countertop_meshes(&ct, o.as_ref(), 0.0)
    }

    fn hit(ms: &[Mesh], x: f64, y: f64, z: f64) -> bool {
        ms.iter().any(|m| crate::probe::probe_mesh(m, cadcraft_geom::Vec3::new(x, y, z)))
    }

    #[test]
    fn bullnose_rounds_the_top_front_corner() {
        let ms = bull_counter(false, EdgeProfile::Bullnose);
        assert!(!hit(&ms, 1.0, 0.002, 0.912));
        assert!(hit(&ms, 1.0, 0.5, 0.9));
        assert!(hit(&ms, 1.0, 0.03, 0.912));
    }

    #[test]
    fn bullnose_nose_reaches_the_edge_line_at_mid_thickness() {
        let ms = bull_counter(false, EdgeProfile::Bullnose);
        assert!(hit(&ms, 1.0, 0.002, 0.895));
        assert!(!hit(&ms, 1.0, -0.001, 0.895));
    }

    #[test]
    fn square_edge_keeps_the_full_corner() {
        let ms = bull_counter(false, EdgeProfile::Square);
        assert!(hit(&ms, 1.0, 0.002, 0.912));
    }

    #[test]
    fn clockwise_polygon_profiles_the_same_edge() {
        let ms = bull_counter(true, EdgeProfile::Bullnose);
        assert!(!hit(&ms, 1.0, 0.002, 0.912));
        assert!(hit(&ms, 1.0, 0.002, 0.895));
        assert!(hit(&ms, 1.0, 0.03, 0.912));
    }

    #[test]
    fn profiled_counter_meshes_are_manifold() {
        for p in [EdgeProfile::Bullnose, EdgeProfile::Eased, EdgeProfile::Bevel] {
            for m in bull_counter(false, p) {
                assert!(m.is_manifold());
            }
        }
    }

    #[test]
    fn the_stub_builds_nothing() {
        assert!(countertop_meshes(&Countertop::default(), None, 0.0).is_empty());
    }

    #[test]
    fn no_outline_builds_nothing_and_an_outline_builds_one_slab() {
        assert!(countertop_meshes(&Countertop::default(), None, 0.0).is_empty());
        let mut m = ArchModel::default();
        let id = m.push_countertop(Countertop {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)],
            ..Default::default()
        });
        let ct = m.countertop(id).cloned().unwrap_or_default();
        let meshes = countertop_meshes(&ct, custom_outline(&m, &ct).as_ref(), 0.0);
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].object, id);
    }

    #[test]
    fn waterfall_edge_has_no_nose() {
        let mut m = ArchModel::default();
        let mut ct = Countertop {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 1.0), Vec2::new(0.0, 1.0)],
            edges: vec![CounterEdge::default(); 4],
            top: 0.914,
            thickness: 0.038,
            ..Default::default()
        };
        ct.edges[0].profile = EdgeProfile::Bullnose;
        ct.edges[0].waterfall = true;
        let id = m.push_countertop(ct.clone());
        let outline = custom_outline(&m, &ct).unwrap();
        let meshes = countertop_meshes(&ct, Some(&outline), 0.0);
        // Waterfall edge keeps the full corner (no nose inset)
        assert!(hit(&meshes, 1.0, 0.002, 0.912));
    }

    #[test]
    fn arc_edge_has_no_nose() {
        let mut m = ArchModel::default();
        let mut ct = Countertop {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 1.0), Vec2::new(0.0, 1.0)],
            edges: vec![CounterEdge::default(); 4],
            top: 0.914,
            thickness: 0.038,
            ..Default::default()
        };
        ct.edges[0].profile = EdgeProfile::Bullnose;
        ct.edges[0].sagitta = 0.05;
        let id = m.push_countertop(ct.clone());
        let outline = custom_outline(&m, &ct).unwrap();
        let meshes = countertop_meshes(&ct, Some(&outline), 0.0);
        // Arc edge keeps the full corner (no nose inset)
        assert!(hit(&meshes, 1.0, 0.002, 0.912));
    }

    #[test]
    fn eased_profile_rounds_corner() {
        let ms = bull_counter(false, EdgeProfile::Eased);
        // Eased removes a small amount (c = 0.003) from the top-front corner
        assert!(!hit(&ms, 1.0, 0.001, 0.913));
        // But keeps the body
        assert!(hit(&ms, 1.0, 0.005, 0.912));
        assert!(hit(&ms, 1.0, 0.5, 0.9));
    }

    #[test]
    fn bevel_profile_rounds_corner() {
        let ms = bull_counter(false, EdgeProfile::Bevel);
        // Bevel removes a larger amount (c = 0.01) from the top-front corner
        assert!(!hit(&ms, 1.0, 0.005, 0.913));
        // But keeps the body
        assert!(hit(&ms, 1.0, 0.015, 0.912));
        assert!(hit(&ms, 1.0, 0.5, 0.9));
    }

    #[test]
    fn zero_thickness_does_not_panic() {
        let mut m = ArchModel::default();
        let id = m.push_countertop(Countertop {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 1.0), Vec2::new(0.0, 1.0)],
            edges: vec![CounterEdge::default(); 4],
            top: 0.914,
            thickness: 0.0,
            ..Default::default()
        });
        let ct = m.countertop(id).cloned().unwrap_or_default();
        let outline = custom_outline(&m, &ct);
        // Must not panic; may produce zero or more meshes
        let _meshes = countertop_meshes(&ct, outline.as_ref(), 0.0);
    }

    #[test]
    fn coincident_vertices_do_not_panic() {
        let mut m = ArchModel::default();
        let id = m.push_countertop(Countertop {
            polygon: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(0.0, 0.0), // duplicate
                Vec2::new(2.0, 0.0),
                Vec2::new(2.0, 1.0),
                Vec2::new(0.0, 1.0),
            ],
            edges: vec![CounterEdge::default(); 5],
            top: 0.914,
            thickness: 0.038,
            ..Default::default()
        });
        let ct = m.countertop(id).cloned().unwrap_or_default();
        let outline = custom_outline(&m, &ct);
        // Must not panic on degenerate polygon
        let _meshes = countertop_meshes(&ct, outline.as_ref(), 0.0);
    }
}
