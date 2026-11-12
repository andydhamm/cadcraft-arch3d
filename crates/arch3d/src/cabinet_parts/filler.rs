//! Filler strips: the panel and the plinth of a filler cabinet.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::Cabinet;

use crate::Mesh;
use crate::casework::{level_elevation, push_box};

/// The meshes of a filler (`c.filler`): its panel and its plinth only; its crown and its run's
/// counter come from the molding and counter parts.
pub fn filler_meshes(model: &ArchModel, c: &Cabinet) -> Vec<Mesh> {
    if !c.filler {
        return Vec::new();
    }
    let level_z = level_elevation(model, c.level) + c.elevation;
    let mut front = Mesh::new("cabinet-front", c.id);
    push_box(&mut front, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, c.depth - 0.019, c.depth, level_z + 0.0, level_z + c.height);
    let mut out = Vec::new();
    if !front.indices.is_empty() {
        out.push(front);
    }
    if c.toe_kick {
        let mut box_mesh = Mesh::new("cabinet-box", c.id);
        push_box(&mut box_mesh, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, 0.0, c.depth - 0.075, level_z, level_z + 0.1);
        if !box_mesh.indices.is_empty() {
            out.push(box_mesh);
        }
    }
    out
}
