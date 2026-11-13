//! Appliance fronts and bodies.

use cadcraft_arch::casework_layout::FrontInfo;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::Cabinet;
use cadcraft_geom::Vec2;

use crate::Mesh;

fn world_point(position: Vec2, rotation_deg: f64, local: Vec2) -> Vec2 {
    let a = rotation_deg.to_radians();
    let dx = local.x * a.cos() - local.y * a.sin();
    let dy = local.x * a.sin() + local.y * a.cos();
    position + Vec2::new(dx, dy)
}

/// The meshes of the appliance fronts in `layout`.
pub fn appliance_meshes(model: &ArchModel, c: &Cabinet, layout: &[FrontInfo]) -> Vec<Mesh> {
    let mut out = Vec::new();
    let mut mesh = Mesh::new("cabinet-appliance", c.id);
    let floor = model
        .levels
        .iter()
        .rev()
        .find(|l| l.index == c.level)
        .map(|l| l.elevation)
        .unwrap_or_else(|| model.levels.last().map(|l| l.elevation).unwrap_or(0.0));
    let y0 = c.depth - 0.019;
    let y1 = c.depth + 0.005;
    let mut added = false;
    for f in layout {
        if f.kind != cadcraft_arch::casework_layout::FrontKind::Appliance {
            continue;
        }
        let [u0, v0, u1, v1] = f.rect;
        if !(u0.is_finite() && u1.is_finite() && v0.is_finite() && v1.is_finite()) || u1 <= u0 || v1 <= v0 {
            continue;
        }
        let ring = [
            world_point(c.position, c.rotation, Vec2::new(u0, y0)),
            world_point(c.position, c.rotation, Vec2::new(u1, y0)),
            world_point(c.position, c.rotation, Vec2::new(u1, y1)),
            world_point(c.position, c.rotation, Vec2::new(u0, y1)),
        ];
        mesh.push_prism(&ring, floor + v0, floor + v1);
        added = true;
    }
    if added {
        out.push(mesh);
    }
    out
}
