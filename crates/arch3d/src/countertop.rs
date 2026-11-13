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

fn profile_section(profile: EdgeProfile, thickness: f64) -> Vec<[f64; 2]> {
    let t = thickness;
    match profile {
        EdgeProfile::Square => vec![[0.0, 0.0], [0.0, t]],
        EdgeProfile::Eased => {
            let r = 0.003;
            vec![[0.0, 0.0], [r, 0.0], [r, t - r], [0.0, t]]
        }
        EdgeProfile::Bevel => {
            let r = 0.01;
            vec![[0.0, 0.0], [r, 0.0], [r, t - r], [0.0, t]]
        }
        EdgeProfile::Bullnose => {
            let r = t / 2.0;
            let mut pts = vec![[0.0, 0.0]];
            let steps = 12usize;
            for i in 1..=steps {
                let a = -std::f64::consts::PI / 2.0 + std::f64::consts::PI * (i as f64) / (steps as f64);
                let x = r * a.cos();
                let y = r * (1.0 + a.sin());
                let p = [x, y];
                if pts.last().is_none_or(|last| (last[0] - p[0]).abs() > 1e-12 || (last[1] - p[1]).abs() > 1e-12) {
                    pts.push(p);
                }
            }
            let mut twice = 0.0;
            for i in 0..pts.len() {
                let a = pts[i];
                let b = pts[(i + 1) % pts.len()];
                twice += a[0] * b[1] - b[0] * a[1];
            }
            if twice < 0.0 {
                pts.reverse();
            }
            pts
        }
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

    let mut profile_meshes = Vec::new();
    for i in 0..ct.polygon.len() {
        let edge = ct.edge(i);
        if edge.profile == EdgeProfile::Square || edge.sagitta != 0.0 || edge.waterfall {
            continue;
        }
        let a = ct.polygon[i];
        let b = ct.polygon[(i + 1) % ct.polygon.len()];
        let dir = b - a;
        let len = dir.len();
        if !(len.is_finite() && len > 1e-12) {
            continue;
        }
        let outward = edge_normal(a, b, ccw);
        let inward = -outward;
        let profile_r = match edge.profile {
            EdgeProfile::Eased => 0.003,
            EdgeProfile::Bevel => 0.01,
            EdgeProfile::Bullnose => ct.thickness / 2.0,
            EdgeProfile::Square => 0.0,
        };
        let path = vec![a + inward * profile_r, b + inward * profile_r];
        let z = [level + ct.top - ct.thickness, level + ct.top - ct.thickness];
        let prof = profile_section(edge.profile, ct.thickness);
        let side = if ccw { 1.0 } else { -1.0 };
        if let Some(m) = crate::sweep::sweep(&prof, &path, &z, false, side, "countertop", ct.id) {
            if !m.indices.is_empty() {
                profile_meshes.push(m);
            }
        }
    }
    meshes.extend(profile_meshes);

    let mut slab = Mesh::new("countertop", ct.id);
    slab.level_elevation = level;
    slab.push_prism_with_holes(&sampled_ring(&o.ring, &o.sagittas), &o.holes, level + o.top - o.thickness, level + o.top);
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
            top: 0.914,
            thickness: 0.038,
            ..Default::default()
        };
        ct.edges[0].profile = EdgeProfile::Bullnose;
        let id = m.push_countertop(ct.clone());
        let outline = custom_outline(&m, &ct).unwrap();
        let meshes = countertop_meshes(&ct, Some(&outline), 0.0);
        println!("id={} object={} meshes={}", id.0, ct.id.0, meshes.len());
        for (i, mesh) in meshes.iter().enumerate() {
            println!("mesh {} material={} object={} tris={} bounds={:?}", i, mesh.material, mesh.object.0, mesh.indices.len() / 3, mesh.bounds());
        }
        assert!(!meshes.is_empty());
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
}
