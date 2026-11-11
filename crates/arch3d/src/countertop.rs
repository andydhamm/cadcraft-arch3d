//! Custom countertop meshes.

use cadcraft_arch::countertop::CounterOutline;
use cadcraft_arch::countertop_edit::sampled_ring;
use cadcraft_doc::arch::Countertop;

use crate::mesh::Mesh;

/// The meshes of one custom countertop (`countertop*` materials, object = the countertop), built
/// from its resolved `outline` (`None` when it has none), `level` being its level's elevation.
///
/// The slab is one prism over the outline (arc edges sampled), cut by the sink holes.
pub fn countertop_meshes(ct: &Countertop, outline: Option<&CounterOutline>, level: f64) -> Vec<Mesh> {
    let Some(o) = outline else { return Vec::new() };
    let mut slab = Mesh::new("countertop", ct.id);
    slab.level_elevation = level;
    slab.push_prism_with_holes(&sampled_ring(&o.ring, &o.sagittas), &o.holes, level + o.top - o.thickness, level + o.top);
    if slab.indices.is_empty() { Vec::new() } else { vec![slab] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_arch::countertop::custom_outline;
    use cadcraft_doc::ArchModel;
    use cadcraft_geom::Vec2;

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
