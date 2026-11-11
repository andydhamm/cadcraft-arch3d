//! Free molding line meshes.

use cadcraft_arch::molding_profile::builtin_profile;
use cadcraft_doc::arch::MoldingLine;

use crate::mesh::Mesh;
use crate::sweep::sweep;

/// The meshes of one molding line (`molding-line*` materials, object = the line), `level` being its
/// level's elevation.
pub fn line_meshes(line: &MoldingLine, level: f64) -> Vec<Mesh> {
    let Some(run) = cadcraft_arch::molding_line::line_run_at(line, level) else {
        return Vec::new();
    };
    let path = run.path.clone();
    let z = run.z.clone();
    let profile = run.profile.points.clone();
    let mesh = sweep(&profile, &path, &z, run.closed, run.side, "molding-line", line.id);
    mesh.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::MoldingLine;

    #[test]
    fn line_mesh_probes() {
        let profile = builtin_profile("molding:chair-rail").unwrap();
        let line = MoldingLine {
            id: ArchId(7),
            points: vec![cadcraft_geom::Vec2::new(0.0, 0.0), cadcraft_geom::Vec2::new(2.0, 0.0), cadcraft_geom::Vec2::new(2.0, 1.0)],
            elevation: 0.1,
            inset: 0.05,
            profile,
            ..Default::default()
        };
        let meshes = line_meshes(&line, 0.4);
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].material, "molding-line");
        assert_eq!(meshes[0].object, line.id);
        assert!(!meshes[0].positions.is_empty());
    }
}
