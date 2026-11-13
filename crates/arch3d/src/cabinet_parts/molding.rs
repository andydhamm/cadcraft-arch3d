//! Cabinet crown moldings and light rails.

use cadcraft_arch::casework_molding::{crown_run, light_rail_run};
use cadcraft_arch::molding_runs::run_length;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::Cabinet;

use crate::Mesh;

/// The crown and light rail meshes of a cabinet (a filler's copied ones too).
pub fn molding_meshes(model: &ArchModel, c: &Cabinet) -> Vec<Mesh> {
    let mut out = Vec::new();
    for (run, material) in [(crown_run(model, c).as_ref(), "cabinet-crown"), (light_rail_run(model, c).as_ref(), "cabinet-lightrail")] {
        let Some(run) = run else { continue };
        if run_length(run) <= 0.0 || run.path.len() < 2 || run.profile.points.is_empty() {
            continue;
        }
        if let Some(m) = crate::sweep::sweep(&run.profile.points, &run.path, &run.z, run.closed, run.side, material, c.id) {
            out.push(m);
        }
    }
    out
}
