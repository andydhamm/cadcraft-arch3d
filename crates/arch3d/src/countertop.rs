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
    let n = Vec2::new(-d.y / len, d.x / len);
    if ccw { n } else { -n }
}

fn profile_section(profile: EdgeProfile, thickness: f64) -> Vec<[f64; 2]> {
    let t = thickness;
    match profile {
        EdgeProfile::Square => return vec![[0.0, 0.0], [0.0, t]],
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
            let mut pts = Vec::new();
            let steps = 12usize;
            for i in 0..=steps {
                let a = std::f64::consts::PI * (i as f64) / (steps as f64);
                let x = r * a.sin();
                let y = r * (1.0 - a.cos());
                pts.push([x, y]);
            }
            pts.push([0.0, t]);
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
    let n = edge_normal(a, b, ccw);
    let p0 = a + n * WATERFALL_PROUD;
    let p1 = b + n * WATERFALL_PROUD;
    let p2 = b + n * (-thickness);
    let p3 = a + n * (-thickness);
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

    for i in 0..ct.polygon.len() {
        let edge = ct.edge(i);
        if edge.profile == EdgeProfile::Square || edge.sagitta != 0.0 {
            continue;
        }
        let a = ct.polygon[i];
        let b = ct.polygon[(i + 1) % ct.polygon.len()];
        let path = vec![a, b];
        let z = [level + ct.top - ct.thickness, level + ct.top - ct.thickness];
        let prof = profile_section(edge.profile, ct.thickness);
        if let Some(m) = crate::sweep::sweep(&prof, &path, &z, false, 1.0, "countertop", ct.id) {
            if !m.indices.is_empty() {
                meshes.push(m);
            }
        }
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
