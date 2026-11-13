//! Open shelving: the open carcass and the shelves.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Cabinet, Fronts};

use crate::Mesh;
use crate::casework::{level_elevation, push_box};

/// The carcass of a cabinet with open shelving, or `None` for a solid carcass.
pub fn open_carcass(model: &ArchModel, c: &Cabinet) -> Option<Mesh> {
    if c.fronts != Fronts::Open {
        return None;
    }
    let level_z = level_elevation(model, c.level) + c.elevation;
    let mut carcass = Mesh::new("cabinet-box", c.id);
    let y0 = 0.019;
    let y1 = c.depth - 0.019;
    let toe = if c.toe_kick { 0.1 } else { 0.0 };
    push_box(&mut carcass, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, 0.0, 0.019, level_z + toe, level_z + c.height);
    push_box(&mut carcass, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, y1, c.depth, level_z + toe, level_z + c.height);
    push_box(&mut carcass, c.position, c.rotation, -c.width / 2.0, -c.width / 2.0 + 0.019, y0, y1, level_z + toe, level_z + c.height);
    push_box(&mut carcass, c.position, c.rotation, c.width / 2.0 - 0.019, c.width / 2.0, y0, y1, level_z + toe, level_z + c.height);
    push_box(&mut carcass, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, y0, y1, level_z + toe, level_z + toe + 0.019);
    push_box(&mut carcass, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, y0, y1, level_z + c.height - 0.019, level_z + c.height);
    if carcass.indices.is_empty() { None } else { Some(carcass) }
}

/// The shelves of a cabinet.
pub fn shelf_meshes(model: &ArchModel, c: &Cabinet) -> Vec<Mesh> {
    if c.fronts != Fronts::Open {
        return Vec::new();
    }
    let mut out = Vec::new();
    let level_z = level_elevation(model, c.level) + c.elevation;
    let toe = if c.toe_kick { 0.1 } else { 0.0 };
    let count = cadcraft_arch::casework_shelves::shelf_count(c) as usize;
    if count == 0 {
        return out;
    }
    let mut shelves = Mesh::new("cabinet-shelves", c.id);
    for (bottom, _) in cadcraft_arch::casework_shelves::shelf_levels(c) {
        let z0 = level_z + toe + bottom;
        let z1 = z0 + 0.019;
        push_box(&mut shelves, c.position, c.rotation, -c.width / 2.0 + 0.019, c.width / 2.0 - 0.019, 0.019, c.depth - 0.019, z0, z1);
    }
    if !shelves.indices.is_empty() {
        out.push(shelves);
    }
    out
}
