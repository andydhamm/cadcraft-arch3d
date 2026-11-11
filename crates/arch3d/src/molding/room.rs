//! Room molding meshes.

use cadcraft_arch::molding_runs::MoldingRun;
use cadcraft_doc::ArchId;

use crate::mesh::Mesh;

/// The meshes of one room's moldings, one per molding index (`molding-room-*` materials, object = `room`),
/// `level` being the room level's elevation.
pub fn room_molding_meshes(room: ArchId, level: f64, runs: &[MoldingRun]) -> Vec<Mesh> {
    let mut meshes = Vec::new();

    // Group runs by index
    let mut by_index: std::collections::BTreeMap<usize, Vec<&MoldingRun>> = std::collections::BTreeMap::new();
    for r in runs {
        by_index.entry(r.index).or_default().push(r);
    }

    for (idx, group) in by_index {
        if group.is_empty() {
            continue;
        }
        let kind = group[0].kind;
        let mut mesh = Mesh::new();
        for run in group {
            if run.path.len() < 2 || run.z.len() != run.path.len() {
                continue;
            }
            if let Some(swept) = crate::sweep::sweep(&run.profile.points, &run.path, &run.z, run.closed, run.side) {
                mesh.append(&swept);
            }
        }
        if !mesh.positions.is_empty() {
            mesh.material = format!("molding-room-{}-{}", kind.key(), idx);
            mesh.object = room;
            mesh.level_elevation = level;
            meshes.push(mesh);
        }
    }

    meshes
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_arch::molding_runs::MoldingRun;
    use cadcraft_doc::arch::MoldingKind;
    use cadcraft_geom::Vec2;

    #[test]
    fn base_mesh_probes() {
        // Test that base molding mesh is created and probeable
        let runs = vec![MoldingRun {
            kind: MoldingKind::Base,
            index: 0,
            profile: Default::default(),
            path: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)],
            z: vec![0.0, 0.0],
            closed: false,
            side: 1.0,
        }];
        let meshes = room_molding_meshes(ArchId(0), 0.0, &runs);
        assert_eq!(meshes.len(), 1);
        assert!(meshes[0].material.starts_with("molding-room-base"));
    }

    #[test]
    fn crown_is_a_closed_ring_mesh() {
        // Test that crown molding creates a closed ring mesh
        let runs = vec![MoldingRun {
            kind: MoldingKind::Crown,
            index: 0,
            profile: Default::default(),
            path: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)],
            z: vec![2.4, 2.4, 2.4, 2.4],
            closed: true,
            side: 1.0,
        }];
        let meshes = room_molding_meshes(ArchId(0), 0.0, &runs);
        assert_eq!(meshes.len(), 1);
        assert!(meshes[0].material.starts_with("molding-room-crown"));
    }
}
