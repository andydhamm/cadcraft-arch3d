//! A cabinet's counter: its run's counter when it owns the run, else a lone counter.

use cadcraft_arch::casework_layout::CounterRun;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::Cabinet;

use crate::Mesh;
use crate::casework::{level_elevation, push_box, run_counter};

/// The counter meshes of `c`: the counter of its run when it owns the run (the cabinet with the
/// lowest u), a lone counter for a counter cabinet in no run, and nothing for a cabinet a custom
/// countertop claims.
pub fn counter_meshes(model: &ArchModel, c: &Cabinet, runs: &[CounterRun]) -> Vec<Mesh> {
    let mut meshes = Vec::new();
    let level_z = level_elevation(model, c.level) + c.elevation;
    let run = runs.iter().find(|r| r.cabinets.contains(&c.id));
    match run {
        Some(run) => {
            if run.owner == c.id
                && let Some(counter) = run_counter(model, run)
            {
                meshes.push(counter);
            }
        }
        None if c.counter && !cadcraft_arch::countertop::claimed_cabinets(model).contains(&c.id) => {
            let mut counter = Mesh::new("counter", c.id);
            push_box(
                &mut counter,
                c.position,
                c.rotation,
                -c.width / 2.0,
                c.width / 2.0,
                0.0,
                c.depth + c.counter_overhang,
                level_z + c.height,
                level_z + c.height + c.counter_thickness,
            );
            if !counter.indices.is_empty() {
                meshes.push(counter);
            }
        }
        None => {}
    }
    meshes
}
