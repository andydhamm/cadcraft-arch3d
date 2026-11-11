//! Cabinets and casework fixtures in 3D.

use crate::Mesh;
use crate::furnishing::{pull_material, symbol_for};
use cadcraft_arch::casework_layout::{CounterRun, FrontInfo, FrontKind, counter_runs, fronts};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Cabinet, Fixture, FixtureKind, FrontStyle, FurnishingHost, MAX_SYMBOL_TRIANGLES};
use cadcraft_geom::Vec2;

fn rotate_local(p: Vec2, deg: f64) -> Vec2 {
    let a = deg.to_radians();
    Vec2::new(p.x * a.cos() - p.y * a.sin(), p.x * a.sin() + p.y * a.cos())
}

fn world_point(position: Vec2, rotation_deg: f64, local: Vec2) -> Vec2 {
    position + rotate_local(local, rotation_deg)
}

fn rect_ring(position: Vec2, rotation_deg: f64, x0: f64, x1: f64, y0: f64, y1: f64) -> [Vec2; 4] {
    [
        world_point(position, rotation_deg, Vec2::new(x0, y0)),
        world_point(position, rotation_deg, Vec2::new(x1, y0)),
        world_point(position, rotation_deg, Vec2::new(x1, y1)),
        world_point(position, rotation_deg, Vec2::new(x0, y1)),
    ]
}

fn push_box(mesh: &mut Mesh, position: Vec2, rotation_deg: f64, x0: f64, x1: f64, y0: f64, y1: f64, z0: f64, z1: f64) {
    if !(x0.is_finite() && x1.is_finite() && y0.is_finite() && y1.is_finite() && z0.is_finite() && z1.is_finite()) {
        return;
    }
    mesh.push_prism(&rect_ring(position, rotation_deg, x0, x1, y0, y1), z0, z1);
}

fn level_elevation(model: &ArchModel, level: u32) -> f64 {
    model
        .levels
        .iter()
        .rev()
        .find(|l| l.index == level)
        .map(|l| l.elevation)
        .unwrap_or_else(|| model.levels.last().map(|l| l.elevation).unwrap_or(0.0))
}

/// Meshes for a cabinet: carcass, front faces, pulls and optional counter.
pub fn cabinet_meshes(model: &ArchModel, c: &Cabinet) -> Vec<Mesh> {
    cabinet_meshes_in(model, c, &counter_runs(model))
}

/// [`cabinet_meshes`] with the model's counter runs already worked out (`runs` is
/// `counter_runs(model)`): a scene build works them out once, not once per cabinet.
pub(crate) fn cabinet_meshes_in(model: &ArchModel, c: &Cabinet, runs: &[CounterRun]) -> Vec<Mesh> {
    let level_z = level_elevation(model, c.level) + c.elevation;
    let toe = if c.toe_kick { 0.1 } else { 0.0 };
    let mut meshes = Vec::new();

    // The recessed plinth is part of the carcass mesh: exports name one node per mesh, so a second "cabinet-box"
    // mesh would be a second `cabinet-cb<n>` node (phase-13 check 13 counts one per cabinet).
    let mut carcass = Mesh::new("cabinet-box", c.id);
    push_box(&mut carcass, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, 0.0, c.depth - 0.019, level_z + toe, level_z + c.height);
    if c.toe_kick {
        push_box(&mut carcass, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, 0.0, c.depth - 0.075, level_z, level_z + 0.1);
    }
    if !carcass.indices.is_empty() {
        meshes.push(carcass);
    }

    let mut front = Mesh::new("cabinet-front", c.id);
    let layout = fronts(model, c);
    let frame = Frame { position: c.position, rotation: c.rotation };
    // The layout's v is above the level's floor (the cabinet's elevation included).
    let floor = level_elevation(model, c.level);
    for f in &layout {
        let [u0, v0, u1, v1] = f.rect;
        let style = match f.kind {
            FrontKind::Door => c.door_style,
            FrontKind::Drawer => c.drawer_style,
            FrontKind::False => FrontStyle::Slab,
            // Not drawn by the front loop: an opening has no front, an appliance is built elsewhere.
            FrontKind::Opening | FrontKind::Appliance => continue,
        };
        push_front(&mut front, &frame, style, c.depth, [u0, floor + v0, u1, floor + v1]);
    }
    if !front.indices.is_empty() {
        meshes.push(front);
    }

    meshes.extend(pull_meshes(model, c, &layout));

    let run = runs.iter().find(|r| r.cabinets.contains(&c.id));
    match run {
        Some(run) => {
            if run.owner == c.id
                && let Some(counter) = run_counter(model, run)
            {
                meshes.push(counter);
            }
        }
        None if c.counter => {
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

/// A cabinet's frame in plan: origin at the back edge's centre, rotated.
struct Frame {
    position: Vec2,
    rotation: f64,
}

impl Frame {
    /// A point `(u, y, z)` of the cabinet frame in the world.
    fn point(&self, u: f64, y: f64, z: f64) -> [f32; 3] {
        let p = world_point(self.position, self.rotation, Vec2::new(u, y));
        [p.x as f32, p.y as f32, z as f32]
    }

    /// A direction `(du, dy, dz)` of the cabinet frame in the world.
    fn dir(&self, d: [f64; 3]) -> [f32; 3] {
        let r = rotate_local(Vec2::new(d[0], d[1]), self.rotation);
        [r.x as f32, r.y as f32, d[2] as f32]
    }

    #[allow(clippy::too_many_arguments)]
    fn box_(&self, mesh: &mut Mesh, u0: f64, u1: f64, y0: f64, y1: f64, z0: f64, z1: f64) {
        push_box(mesh, self.position, self.rotation, u0, u1, y0, y1, z0, z1);
    }

    /// One flat quad of frame points (in order around it), wound so that its normal faces `outward`.
    fn quad(&self, mesh: &mut Mesh, pts: [[f64; 3]; 4], outward: [f64; 3]) {
        let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let (e1, e2) = (sub(pts[1], pts[0]), sub(pts[2], pts[0]));
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if !(len.is_finite() && len > 1e-12) {
            return;
        }
        let flip = n[0] * outward[0] + n[1] * outward[1] + n[2] * outward[2] < 0.0;
        let n = self.dir([n[0] / len, n[1] / len, n[2] / len]);
        let n = if flip { [-n[0], -n[1], -n[2]] } else { n };
        let base = mesh.positions.len() as u32;
        for p in pts {
            let w = self.point(p[0], p[1], p[2]);
            mesh.positions.push(w);
            mesh.normals.push(n);
            mesh.uvs.push([w[0], w[2]]);
        }
        if flip {
            mesh.indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        } else {
            mesh.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
}

/// Width of a shaker or raised-panel frame member.
const FRAME_W: f64 = 0.06;
/// How far the panel of a shaker front is recessed from the frame's face.
const RECESS: f64 = 0.006;
/// Inset of a raised panel's centre field from the frame.
const FIELD_INSET: f64 = 0.05;
/// How far a raised field stands back from the frame's face.
const FIELD_BACK: f64 = 0.002;

/// One front of `style`: the same outer box (y depth-0.019..depth) whatever the style.
fn push_front(mesh: &mut Mesh, fr: &Frame, style: FrontStyle, depth: f64, rect: [f64; 4]) {
    let [u0, z0, u1, z1] = rect;
    if !(u0.is_finite() && u1.is_finite() && z0.is_finite() && z1.is_finite()) || u1 <= u0 || z1 <= z0 {
        return;
    }
    let (y0, y1) = (depth - 0.019, depth);
    let framed = u1 - u0 > 2.0 * FRAME_W + 0.02 && z1 - z0 > 2.0 * FRAME_W + 0.02;
    if style == FrontStyle::Slab || !framed {
        fr.box_(mesh, u0, u1, y0, y1, z0, z1);
        return;
    }
    let (iu0, iu1, iz0, iz1) = (u0 + FRAME_W, u1 - FRAME_W, z0 + FRAME_W, z1 - FRAME_W);
    fr.box_(mesh, u0, iu0, y0, y1, z0, z1);
    fr.box_(mesh, iu1, u1, y0, y1, z0, z1);
    fr.box_(mesh, iu0, iu1, y0, y1, z0, iz0);
    fr.box_(mesh, iu0, iu1, y0, y1, iz1, z1);
    fr.box_(mesh, iu0, iu1, y0, depth - RECESS, iz0, iz1);
    if style == FrontStyle::RaisedPanel && iu1 - iu0 > 2.0 * FIELD_INSET + 0.01 && iz1 - iz0 > 2.0 * FIELD_INSET + 0.01 {
        let yb = depth - RECESS;
        let yt = depth - FIELD_BACK;
        let (fu0, fu1, fz0, fz1) = (iu0 + FIELD_INSET, iu1 - FIELD_INSET, iz0 + FIELD_INSET, iz1 - FIELD_INSET);
        // The field's top, then the four bevels from the recessed border up to it.
        fr.quad(mesh, [[fu0, yt, fz0], [fu1, yt, fz0], [fu1, yt, fz1], [fu0, yt, fz1]], [0.0, 1.0, 0.0]);
        fr.quad(mesh, [[iu0, yb, iz0], [iu1, yb, iz0], [fu1, yt, fz0], [fu0, yt, fz0]], [0.0, 0.5, -1.0]);
        fr.quad(mesh, [[iu0, yb, iz1], [iu1, yb, iz1], [fu1, yt, fz1], [fu0, yt, fz1]], [0.0, 0.5, 1.0]);
        fr.quad(mesh, [[iu0, yb, iz0], [iu0, yb, iz1], [fu0, yt, fz1], [fu0, yt, fz0]], [-1.0, 0.5, 0.0]);
        fr.quad(mesh, [[iu1, yb, iz0], [iu1, yb, iz1], [fu1, yt, fz1], [fu1, yt, fz0]], [1.0, 0.5, 0.0]);
    }
}

/// The pulls of `c`: one mesh `pull:<key>:<i>` per symbol part, a copy of the symbol on every front
/// with a pull.
fn pull_meshes(model: &ArchModel, c: &Cabinet, layout: &[FrontInfo]) -> Vec<Mesh> {
    let Some(key) = c.pull_symbol.as_deref() else { return Vec::new() };
    let Some(symbol) = symbol_for(model, key) else { return Vec::new() };
    let ext = symbol.extents();
    if !ext.iter().all(|e| e.is_finite() && *e >= 0.0) {
        return Vec::new();
    }
    let floor = level_elevation(model, c.level);
    let fr = Frame { position: c.position, rotation: c.rotation };
    let mut out: Vec<Mesh> = symbol.parts.iter().enumerate().map(|(i, _)| Mesh::new(&pull_material(key, i), c.id)).collect();
    // Keep a hostile symbol from multiplying past the symbol cap.
    let mut budget = MAX_SYMBOL_TRIANGLES;
    for f in layout {
        let Some([u, v]) = f.pull else { continue };
        let vertical = f.kind == FrontKind::Door && ext[0] > ext[2];
        // Turn 180 degrees about z (front out), then stand it up for a door.
        let turn = |p: [f64; 3]| {
            let q = [-p[0], -p[1], p[2]];
            if vertical { [q[2], q[1], -q[0]] } else { q }
        };
        for (part, mesh) in symbol.parts.iter().zip(out.iter_mut()) {
            let n = part.positions.len();
            let ok = n > 0
                && part.normals.len() == n
                && (part.uvs.is_empty() || part.uvs.len() == n)
                && part.indices.len().is_multiple_of(3)
                && part.indices.iter().all(|&i| usize::try_from(i).is_ok_and(|i| i < n));
            let tris = part.indices.len() / 3;
            if !ok || tris > budget || u32::try_from(mesh.positions.len() + n).is_err() {
                continue;
            }
            budget -= tris;
            let base = mesh.positions.len() as u32;
            for (p, nrm) in part.positions.iter().zip(&part.normals) {
                let q = turn([f64::from(p[0]), f64::from(p[1]), f64::from(p[2]) - ext[2] / 2.0]);
                mesh.positions.push(fr.point(u + q[0], c.depth + q[1] + ext[1] / 2.0, floor + v + q[2]));
                let r = turn([f64::from(nrm[0]), f64::from(nrm[1]), f64::from(nrm[2])]);
                mesh.normals.push(fr.dir(r));
            }
            if part.uvs.is_empty() {
                mesh.uvs.extend(std::iter::repeat_n([0.0, 0.0], n));
            } else {
                mesh.uvs.extend(part.uvs.iter().copied());
            }
            mesh.indices.extend(part.indices.iter().map(|i| i + base));
        }
    }
    out.retain(|m| !m.indices.is_empty());
    for m in &mut out {
        m.level_elevation = floor;
    }
    out
}

/// The most cut-outs one counter run takes (in model order). A real run has a few; the cell split
/// grows with the square of their count, so a crowd of them in a hostile file must not.
const MAX_COUNTER_HOLES: usize = 64;

/// The counter of a run, in the owner's frame, with a hole for every cut-out furnishing hosted on
/// a cabinet of the run or in one of its bridged gaps (the first [`MAX_COUNTER_HOLES`] of them).
fn run_counter(model: &ArchModel, run: &CounterRun) -> Option<Mesh> {
    let floor = level_elevation(model, run.level);
    // `run.top` is the cabinets' top (elevation + height): the counter sits on it, as a lone one always did.
    let (z0, z1) = (floor + run.top, floor + run.top + run.thickness);
    let (u0, u1, y0, y1) = (run.u0, run.u1, 0.0, run.depth + run.overhang);
    if !(u0.is_finite() && u1.is_finite() && y1.is_finite() && z0.is_finite() && z1.is_finite()) || u1 <= u0 || y1 <= y0 || z1 <= z0 {
        return None;
    }
    let mut holes: Vec<[f64; 4]> = Vec::new();
    for f in &model.furnishings {
        let hosted = match f.host {
            FurnishingHost::Cabinet { cabinet } => run.cabinets.contains(&cabinet),
            // The id alone is not enough: a furnishing elsewhere could share a bridged one's id.
            FurnishingHost::Gap { left, right } => run.bridged.contains(&f.id) && run.cabinets.contains(&left) && run.cabinets.contains(&right),
            FurnishingHost::None | FurnishingHost::Wall { .. } | FurnishingHost::Sink { .. } => false,
        };
        let Some(co) = f.cutout.filter(|_| hosted) else { continue };
        if !(f.at.iter().chain(co.iter()).chain([&f.rotation]).all(|v| v.is_finite()) && co[0] > 0.0 && co[1] > 0.0) {
            continue;
        }
        let rel = rotate_local(Vec2::new(f.at[0] - run.origin.x, f.at[1] - run.origin.y), -run.rotation);
        let turned = (f.rotation - run.rotation).to_radians().sin().abs() > 0.5;
        let (w, d) = if turned { (co[1], co[0]) } else { (co[0], co[1]) };
        let h = [(rel.x - w / 2.0).max(u0), (rel.y - d / 2.0).max(y0), (rel.x + w / 2.0).min(u1), (rel.y + d / 2.0).min(y1)];
        if h[2] - h[0] > 1e-6 && h[3] - h[1] > 1e-6 {
            holes.push(h);
            if holes.len() >= MAX_COUNTER_HOLES {
                break;
            }
        }
    }
    let mut mesh = Mesh::new("counter", run.owner);
    let fr = Frame { position: run.origin, rotation: run.rotation };
    let eps = 1e-6;
    let interior = |h: &[f64; 4]| h[0] > u0 + eps && h[1] > y0 + eps && h[2] < u1 - eps && h[3] < y1 - eps;
    let disjoint = holes
        .iter()
        .enumerate()
        .all(|(i, a)| holes.iter().skip(i + 1).all(|b| a[2] + eps < b[0] || b[2] + eps < a[0] || a[3] + eps < b[1] || b[3] + eps < a[1]));
    if holes.is_empty() {
        fr.box_(&mut mesh, u0, u1, y0, y1, z0, z1);
    } else if disjoint && holes.iter().all(interior) {
        let ring = |a: f64, b: f64, c: f64, d: f64| rect_ring(run.origin, run.rotation, a, c, b, d).to_vec();
        let outer = ring(u0, y0, u1, y1);
        let hs: Vec<Vec<Vec2>> = holes.iter().map(|h| ring(h[0], h[1], h[2], h[3])).collect();
        mesh.push_prism_with_holes(&outer, &hs, z0, z1);
    } else {
        // Split along every hole edge and keep the cells outside the holes.
        // The ends always stay: a hole edge within eps of one is not a cut (it would drop the end
        // or leave a sliver).
        let cuts = |lo: f64, hi: f64, pick: fn(&[f64; 4]) -> [f64; 2]| {
            let mut inner: Vec<f64> = holes.iter().flat_map(pick).filter(|&x| x > lo + eps && x < hi - eps).collect();
            inner.sort_by(f64::total_cmp);
            let mut out = vec![lo];
            for x in inner {
                if out.last().is_some_and(|l| x - l > eps) {
                    out.push(x);
                }
            }
            out.push(hi);
            out
        };
        let us = cuts(u0, u1, |h| [h[0], h[2]]);
        let ys = cuts(y0, y1, |h| [h[1], h[3]]);
        for uw in us.windows(2) {
            for yw in ys.windows(2) {
                let (cu, cy) = ((uw[0] + uw[1]) / 2.0, (yw[0] + yw[1]) / 2.0);
                if holes.iter().any(|h| cu > h[0] && cu < h[2] && cy > h[1] && cy < h[3]) {
                    continue;
                }
                fr.box_(&mut mesh, uw[0], uw[1], yw[0], yw[1], z0, z1);
            }
        }
    }
    mesh.level_elevation = floor;
    (!mesh.indices.is_empty()).then_some(mesh)
}

fn push_rect(mesh: &mut Mesh, position: Vec2, rotation_deg: f64, x0: f64, x1: f64, y0: f64, y1: f64, z0: f64, z1: f64) {
    push_box(mesh, position, rotation_deg, x0, x1, y0, y1, z0, z1);
}

/// Mesh for a casework fixture.
pub fn fixture_mesh(model: &ArchModel, f: &Fixture) -> Mesh {
    let kind_name = match f.kind {
        FixtureKind::Sink => "sink",
        FixtureKind::Lavatory => "lavatory",
        FixtureKind::Toilet => "toilet",
        FixtureKind::Bathtub => "bathtub",
        FixtureKind::Shower => "shower",
        FixtureKind::Range => "range",
        FixtureKind::Refrigerator => "refrigerator",
        FixtureKind::Dishwasher => "dishwasher",
    };
    let material = format!("fixture-{kind_name}");
    let mut mesh = Mesh::new(&material, f.id);
    let level_z = level_elevation(model, f.level) + f.elevation;
    match f.kind {
        FixtureKind::Toilet => {
            let tank_depth = 0.2;
            push_rect(&mut mesh, f.position, f.rotation, -f.width / 2.0, f.width / 2.0, 0.0, tank_depth, level_z, level_z + f.height);
            push_rect(&mut mesh, f.position, f.rotation, -0.8 * f.width / 2.0, 0.8 * f.width / 2.0, tank_depth, f.depth, level_z, level_z + 0.4);
        }
        _ => {
            push_rect(&mut mesh, f.position, f.rotation, -f.width / 2.0, f.width / 2.0, 0.0, f.depth, level_z, level_z + f.height);
        }
    }
    mesh.level_elevation = level_z;
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{Cabinet, CabinetKind, Fixture, FixtureKind, Fronts};
    use cadcraft_geom::Vec3;

    fn base_model() -> ArchModel {
        let mut model = ArchModel::default();
        model.cabinets.push(Cabinet {
            id: ArchId(0),
            kind: CabinetKind::Base,
            level: 0,
            position: Vec2::new(0.0, 0.0),
            rotation: 0.0,
            width: 0.6,
            depth: 0.6,
            height: 0.876,
            elevation: 0.0,
            fronts: Fronts::DrawerOverDoor,
            doors: 1,
            drawers: 1,
            counter: true,
            counter_thickness: 0.038,
            counter_overhang: 0.025,
            toe_kick: true,
            ..Cabinet::default()
        });
        model.fixtures.push(Fixture {
            id: ArchId(0),
            kind: FixtureKind::Sink,
            level: 0,
            position: Vec2::new(1.0, 1.0),
            rotation: 0.0,
            width: 0.8,
            depth: 0.55,
            height: 0.02,
            elevation: 0.914,
            ..Fixture::default()
        });
        model
    }

    #[test]
    fn base_cabinet_counter_top_is_at_0_914() {
        let model = base_model();
        let c = &model.cabinets[0];
        let meshes = cabinet_meshes(&model, c);
        let counter = meshes.iter().find(|m| m.material == "counter").expect("counter mesh");
        assert!((counter.bounds().max[2] - 0.914).abs() < 1e-6, "counter top is {}", counter.bounds().max[2]);
    }

    #[test]
    fn toe_kick_plinth_is_part_of_the_carcass_mesh() {
        let model = base_model();
        let meshes = cabinet_meshes(&model, &model.cabinets[0]);
        let boxes: Vec<&Mesh> = meshes.iter().filter(|m| m.material == "cabinet-box").collect();
        assert_eq!(boxes.len(), 1, "one carcass mesh, so one GLB/OBJ node cabinet-cb0, plinth included");
        let b = boxes[0].bounds();
        assert!(b.min[2].abs() < 1e-6, "the carcass reaches the floor through its plinth: min z {}", b.min[2]);
        assert!((b.max[2] - 0.876).abs() < 1e-6, "carcass top {}", b.max[2]);
    }

    #[test]
    fn wall_cabinet_bottom_is_at_its_elevation() {
        let mut model = ArchModel::default();
        model.cabinets.push(Cabinet {
            id: ArchId(0),
            kind: CabinetKind::Wall,
            level: 0,
            position: Vec2::new(0.0, 0.0),
            rotation: 90.0,
            width: 0.6,
            depth: 0.305,
            height: 0.762,
            elevation: 1.372,
            fronts: Fronts::Doors,
            doors: 2,
            ..Cabinet::default()
        });
        let min_z = cabinet_meshes(&model, &model.cabinets[0])
            .iter()
            .filter(|m| m.material == "cabinet-box")
            .map(|m| m.bounds().min[2])
            .fold(f64::INFINITY, f64::min);
        assert!((min_z - 1.372).abs() < 1e-6, "cabinet bottom = {min_z}");
    }

    #[test]
    fn fronts_split_by_doors_and_drawers() {
        let mut model = ArchModel::default();
        model.cabinets.push(Cabinet {
            id: ArchId(0),
            kind: CabinetKind::Base,
            level: 0,
            position: Vec2::new(0.0, 0.0),
            rotation: 0.0,
            width: 0.9,
            depth: 0.6,
            height: 0.9,
            elevation: 0.0,
            fronts: Fronts::DrawerOverDoor,
            doors: 2,
            drawers: 2,
            counter: false,
            toe_kick: false,
            ..Cabinet::default()
        });
        let meshes = cabinet_meshes(&model, &model.cabinets[0]);
        assert_eq!(meshes.iter().filter(|m| m.material == "cabinet-front").count(), 1);
        let front = meshes.iter().find(|m| m.material == "cabinet-front").unwrap();
        assert!(!front.indices.is_empty());
    }

    #[test]
    fn cabinet_follows_rotation() {
        let mut model = ArchModel::default();
        model.cabinets.push(Cabinet {
            id: ArchId(0),
            kind: CabinetKind::Base,
            level: 0,
            position: Vec2::new(2.0, 3.0),
            rotation: 90.0,
            width: 0.6,
            depth: 0.6,
            height: 0.876,
            elevation: 0.0,
            fronts: Fronts::Doors,
            doors: 1,
            ..Cabinet::default()
        });
        let bounds = cabinet_meshes(&model, &model.cabinets[0])[0].bounds();
        assert!((bounds.min[0] - 1.7).abs() < 1e-2 || (bounds.min[1] - 2.7).abs() < 1e-2);
    }

    #[test]
    fn fixture_box_has_its_size() {
        let mut model = ArchModel::default();
        model.fixtures.push(Fixture {
            id: ArchId(0),
            kind: FixtureKind::Sink,
            level: 0,
            position: Vec2::new(0.0, 0.0),
            rotation: 0.0,
            width: 0.8,
            depth: 0.55,
            height: 0.02,
            elevation: 0.914,
            ..Fixture::default()
        });
        let mesh = fixture_mesh(&model, &model.fixtures[0]);
        let b = mesh.bounds();
        assert!((b.max[0] - 0.4).abs() < 1e-6);
        assert!((b.max[1] - 0.55).abs() < 1e-6);
        assert!((b.max[2] - 0.934).abs() < 1e-6);
    }

    #[test]
    fn casework_meshes_are_closed() {
        let model = base_model();
        for mesh in cabinet_meshes(&model, &model.cabinets[0]) {
            assert!(!mesh.indices.is_empty(), "cabinet mesh should contain triangles");
            assert!(mesh.indices.len() % 3 == 0, "cabinet mesh index count must be a multiple of 3");
            assert!(mesh.positions.iter().all(|p| p.iter().all(|v| v.is_finite())), "cabinet mesh has non-finite vertices");
        }
        let mesh = fixture_mesh(&model, &model.fixtures[0]);
        assert!(!mesh.indices.is_empty());
        assert!(mesh.positions.iter().all(|p| p.iter().all(|v| v.is_finite())));
    }

    #[test]
    fn probe_hits_counter_and_cabinet_parts() {
        let model = base_model();
        let scene = crate::Scene3d::build(&model);
        let hit = scene.probe(Vec3::new(0.0, 0.1, 0.45)).expect("cabinet hit");
        assert_eq!(hit.part, "cabinet");
        let hit2 = scene.probe(Vec3::new(0.0, 0.35, 0.89)).expect("counter hit");
        assert_eq!(hit2.part, "counter");
    }

    use cadcraft_doc::arch::{Furnishing, SymbolMesh, SymbolPart};

    fn cab(id: u32, x: f64, fronts: Fronts) -> Cabinet {
        Cabinet {
            id: ArchId(id),
            kind: CabinetKind::Base,
            position: Vec2::new(x, 0.0),
            width: 0.6,
            depth: 0.6,
            height: 0.876,
            fronts,
            doors: 1,
            drawers: 2,
            counter: true,
            counter_thickness: 0.038,
            counter_overhang: 0.025,
            toe_kick: true,
            ..Cabinet::default()
        }
    }

    /// A pull bar 0.1 x 0.02 x 0.02 (long along x), bottom-centred.
    fn bar_symbol() -> SymbolMesh {
        let (hx, hy, hz) = (0.05_f32, 0.01_f32, 0.02_f32);
        let mut positions = Vec::new();
        for z in [0.0, hz] {
            for (x, y) in [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)] {
                positions.push([x, y, z]);
            }
        }
        let indices = vec![0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6, 3, 0, 4, 3, 4, 7];
        let normals = vec![[0.0, 0.0, 1.0]; 8];
        SymbolMesh {
            key: "bar".into(),
            parts: vec![SymbolPart {
                name: "bar".into(),
                positions,
                normals,
                indices,
                base_color: [1.0; 4],
                roughness: 0.5,
                ..SymbolPart::default()
            }],
            ..Default::default()
        }
    }

    fn with_pull(mut c: Cabinet) -> Cabinet {
        c.pull = Some("bar".into());
        c.pull_symbol = Some("bar".into());
        c
    }

    fn model_of(cabinets: Vec<Cabinet>) -> ArchModel {
        let mut model = ArchModel::default();
        model.cabinets = cabinets;
        model.symbols.insert("bar".into(), bar_symbol());
        model
    }

    fn mesh_named<'a>(meshes: &'a [Mesh], name: &str) -> Option<&'a Mesh> {
        meshes.iter().find(|m| m.material == name)
    }

    fn styled(door: FrontStyle, drawer: FrontStyle) -> Vec<Mesh> {
        let mut c = cab(0, 0.0, Fronts::DrawerOverDoor);
        c.door_style = door;
        c.drawer_style = drawer;
        c.width = 0.9;
        let model = model_of(vec![c]);
        cabinet_meshes(&model, &model.cabinets[0])
    }

    #[test]
    fn shaker_and_raised_panel_fronts_keep_the_slab_box() {
        let slab = styled(FrontStyle::Slab, FrontStyle::Slab);
        let b0 = mesh_named(&slab, "cabinet-front").map(|m| m.bounds()).expect("slab fronts");
        for style in [FrontStyle::Shaker, FrontStyle::RaisedPanel] {
            let meshes = styled(style, style);
            let b = mesh_named(&meshes, "cabinet-front").map(|m| m.bounds()).expect("fronts");
            for k in 0..3 {
                assert!((b.min[k] - b0.min[k]).abs() < 1e-6 && (b.max[k] - b0.max[k]).abs() < 1e-6, "{style:?} axis {k}");
            }
        }
    }

    #[test]
    fn front_triangles_grow_slab_shaker_raised() {
        let n = |s| mesh_named(&styled(s, s), "cabinet-front").map(|m| m.triangle_count()).unwrap_or(0);
        let (slab, shaker, raised) = (n(FrontStyle::Slab), n(FrontStyle::Shaker), n(FrontStyle::RaisedPanel));
        assert!(slab > 0 && slab < shaker && shaker < raised, "{slab} {shaker} {raised}");
    }

    fn halves(m: &Mesh, copies: usize) -> Vec<[f64; 6]> {
        let per = m.positions.len() / copies;
        m.positions
            .chunks(per)
            .map(|c| {
                let mut b = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
                for p in c {
                    for k in 0..3 {
                        b[k] = b[k].min(f64::from(p[k]));
                        b[k + 3] = b[k + 3].max(f64::from(p[k]));
                    }
                }
                b
            })
            .collect()
    }

    #[test]
    fn pulls_sit_on_each_front_at_the_set_position() {
        let model = model_of(vec![with_pull(cab(0, 0.0, Fronts::Drawers))]);
        let c = &model.cabinets[0];
        let layout = fronts(&model, c);
        let meshes = cabinet_meshes(&model, c);
        let pull = mesh_named(&meshes, "pull:bar:0").expect("pull mesh");
        assert_eq!(pull.object, c.id);
        let boxes = halves(pull, 2);
        assert_eq!(boxes.len(), 2);
        for (b, f) in boxes.iter().zip(&layout) {
            let [u, v] = f.pull.expect("drawer pull");
            assert!((((b[0] + b[3]) / 2.0) - u).abs() < 1e-5 && (((b[2] + b[5]) / 2.0) - v).abs() < 1e-5);
            assert!((b[1] - c.depth).abs() < 1e-5 && (b[4] - c.depth - 0.02).abs() < 1e-5, "stands proud of the front");
            assert!(b[3] - b[0] > b[5] - b[2], "drawer pulls run along u");
        }
    }

    #[test]
    fn door_pulls_turn_vertical() {
        let model = model_of(vec![with_pull(cab(0, 0.0, Fronts::Doors))]);
        let meshes = cabinet_meshes(&model, &model.cabinets[0]);
        let pull = mesh_named(&meshes, "pull:bar:0").expect("pull mesh");
        let b = halves(pull, 1)[0];
        assert!(b[5] - b[2] > b[3] - b[0], "door pulls run along v");
        assert!((b[4] - b[1] - 0.02).abs() < 1e-5);
    }

    #[test]
    fn no_pull_symbol_no_pull_meshes() {
        let mut c = cab(0, 0.0, Fronts::Doors);
        c.pull = Some("bar".into());
        let mut model = model_of(vec![c]);
        assert!(!cabinet_meshes(&model, &model.cabinets[0]).iter().any(|m| m.material.starts_with("pull:")));
        model.cabinets[0].pull_symbol = Some("missing".into());
        assert!(!cabinet_meshes(&model, &model.cabinets[0]).iter().any(|m| m.material.starts_with("pull:")));
    }

    fn counters(model: &ArchModel) -> Vec<(ArchId, Mesh)> {
        model.cabinets.iter().flat_map(|c| cabinet_meshes(model, c).into_iter().filter(|m| m.material == "counter").map(|m| (c.id, m))).collect()
    }

    #[test]
    fn touching_cabinets_build_one_counter() {
        let model = model_of(vec![cab(0, 0.0, Fronts::Doors), cab(1, 0.6, Fronts::Doors)]);
        let all = counters(&model);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].0, ArchId(0));
        let b = all[0].1.bounds();
        assert!((b.min[0] + 0.3).abs() < 1e-6 && (b.max[0] - 0.9).abs() < 1e-6, "{b:?}");
    }

    fn sink_on(host: FurnishingHost, at: [f64; 2]) -> Furnishing {
        Furnishing { id: ArchId(0), at, size: [0.5, 0.4, 0.2], host, cutout: Some([0.4, 0.3]), ..Furnishing::default() }
    }

    #[test]
    fn a_bridged_gap_carries_the_counter() {
        let mut model = model_of(vec![cab(0, 0.0, Fronts::Doors), cab(1, 1.0, Fronts::Doors)]);
        assert_eq!(counters(&model).len(), 2, "a bare gap breaks the run");
        model.furnishings.push(Furnishing {
            id: ArchId(0),
            at: [0.5, 0.3],
            size: [0.6, 0.6, 0.9],
            host: FurnishingHost::Gap { left: ArchId(0), right: ArchId(1) },
            ..Furnishing::default()
        });
        let all = counters(&model);
        assert_eq!(all.len(), 1);
        let b = all[0].1.bounds();
        assert!((b.min[0] + 0.3).abs() < 1e-6 && (b.max[0] - 1.3).abs() < 1e-6, "{b:?}");
    }

    /// Area of the upward-facing triangles of a counter.
    fn top_area(m: &Mesh) -> f64 {
        let p = |i: u32| m.positions.get(i as usize).map(|v| [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])]).unwrap_or([0.0; 3]);
        m.indices
            .chunks(3)
            .filter_map(|t| {
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                let z = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
                let up = m.normals.get(t[0] as usize).is_some_and(|n| n[2] > 0.9);
                up.then_some(z / 2.0)
            })
            .sum()
    }

    fn model_with_sink() -> ArchModel {
        let mut model = model_of(vec![cab(0, 0.0, Fronts::Doors)]);
        model.furnishings.push(sink_on(FurnishingHost::Cabinet { cabinet: ArchId(0) }, [0.0, 0.3]));
        model
    }

    #[test]
    fn a_cut_out_leaves_a_hole_of_its_size() {
        let model = model_with_sink();
        let all = counters(&model);
        assert_eq!(all.len(), 1);
        let area = top_area(&all[0].1);
        let expect = 0.6 * 0.625 - 0.4 * 0.3;
        assert!((area - expect).abs() < 1e-6, "{area} vs {expect}");
    }

    #[test]
    fn counter_with_a_hole_is_closed() {
        let model = model_with_sink();
        assert!(counters(&model)[0].1.is_manifold());
        // A hole through the counter's edge falls back to cells, which cover the same area.
        let mut edge = model_with_sink();
        edge.furnishings[0].at = [0.0, 0.1];
        let area = top_area(&counters(&edge)[0].1);
        assert!((area - (0.6 * 0.625 - 0.4 * 0.25)).abs() < 1e-6, "{area}");
    }

    #[test]
    fn a_cut_out_edge_just_inside_the_counter_end_keeps_its_length() {
        // The hole crosses the back edge (so the counter is built from cells) and its right edge
        // lies 5e-7 inside the counter's end: the end stays at u = 0.3, front at y = 0.625.
        let mut model = model_with_sink();
        model.furnishings[0].cutout = Some([0.3 - 5e-7, 0.3]);
        model.furnishings[0].at = [0.15 - 2.5e-7, 0.1];
        let b = counters(&model)[0].1.bounds();
        assert!((b.max[0] - 0.3).abs() < 1e-7 && (b.min[0] + 0.3).abs() < 1e-7, "{b:?}");
        model.furnishings[0].cutout = Some([0.2, 0.3]);
        model.furnishings[0].at = [0.0, 0.625 - 5e-7 - 0.15];
        let b = counters(&model)[0].1.bounds();
        assert!((b.max[1] - 0.625).abs() < 1e-7, "{b:?}");
    }

    #[test]
    fn only_hosted_cut_outs_cut_the_counter() {
        // A furnishing on no host that shares its id with the furnishing bridging the gap is not
        // the bridged one: it leaves the counter whole.
        let mut model = model_of(vec![cab(0, 0.0, Fronts::Doors), cab(1, 1.0, Fronts::Doors)]);
        let bridge = Furnishing {
            id: ArchId(0),
            at: [0.5, 0.3],
            size: [0.4, 0.6, 0.85],
            host: FurnishingHost::Gap { left: ArchId(0), right: ArchId(1) },
            ..Furnishing::default()
        };
        model.furnishings.push(bridge);
        model.furnishings.push(sink_on(FurnishingHost::None, [0.0, 0.3]));
        let all = counters(&model);
        assert_eq!(all.len(), 1);
        let area = top_area(&all[0].1);
        assert!((area - 1.6 * 0.625).abs() < 1e-6, "{area}");
    }

    #[test]
    fn several_cut_outs_on_a_run_are_closed_holes() {
        let mut model = model_of(vec![cab(0, 0.0, Fronts::Doors), cab(1, 0.6, Fronts::Doors), cab(2, 1.2, Fronts::Doors)]);
        for (k, x) in [0.0, 0.6, 1.2].into_iter().enumerate() {
            let mut s = sink_on(FurnishingHost::Cabinet { cabinet: ArchId(k as u32) }, [x, 0.3]);
            s.id = ArchId(k as u32);
            model.furnishings.push(s);
        }
        let all = counters(&model);
        assert_eq!(all.len(), 1);
        assert!(all[0].1.is_manifold());
        let area = top_area(&all[0].1);
        assert!((area - (1.8 * 0.625 - 3.0 * 0.4 * 0.3)).abs() < 1e-6, "{area}");
    }

    #[test]
    fn a_crowd_of_cut_outs_keeps_the_counter_bounded() {
        // 200 overlapping cut-outs, each offset 0.1 mm along u and y, force the cell split; the
        // counter takes at most MAX_COUNTER_HOLES of them, so it stays within that split's cells.
        let mut model = model_of(vec![cab(0, 0.0, Fronts::Doors)]);
        for k in 0..200u32 {
            let d = f64::from(k) * 1e-4;
            let mut s = sink_on(FurnishingHost::Cabinet { cabinet: ArchId(0) }, [d, 0.3 + d]);
            s.id = ArchId(k);
            model.furnishings.push(s);
        }
        let all = counters(&model);
        assert_eq!(all.len(), 1);
        let cells = (2 * MAX_COUNTER_HOLES + 1).pow(2);
        assert!(all[0].1.triangle_count() <= 12 * cells, "{} triangles", all[0].1.triangle_count());
    }

    #[test]
    fn a_full_level_of_cabinets_builds_in_seconds() {
        // MAX_CABINETS touching base cabinets (one run) and MAX_FURNISHINGS furnishings: the counter
        // runs are worked out once per build, not once per cabinet (that took over a minute).
        let n = cadcraft_doc::arch::MAX_CABINETS;
        let mut model = model_of((0..n).map(|i| cab(i as u32, i as f64 * 0.6, Fronts::Doors)).collect());
        for i in 0..cadcraft_doc::arch::MAX_FURNISHINGS {
            model.furnishings.push(Furnishing { id: ArchId(i as u32), at: [i as f64, 5.0], size: [0.5, 0.5, 0.5], ..Furnishing::default() });
        }
        let t = std::time::Instant::now();
        let scene = crate::Scene3d::build(&model);
        let took = t.elapsed();
        assert_eq!(scene.meshes.iter().filter(|m| m.material == "counter").count(), 1);
        assert!(took < std::time::Duration::from_secs(10), "{n} cabinets built in {took:?}");
    }

    #[test]
    fn casework_part_edits_keep_the_cache_exact() {
        let mut model = model_of(vec![cab(0, 0.0, Fronts::Doors), cab(1, 0.6, Fronts::Doors)]);
        let (mut cache, _) = crate::SceneCache::new(&model);
        let edits: Vec<Box<dyn Fn(&mut ArchModel)>> = vec![
            Box::new(|m| m.cabinets[0].door_style = FrontStyle::Shaker),
            Box::new(|m| m.cabinets[1].drawer_style = FrontStyle::RaisedPanel),
            Box::new(|m| {
                m.cabinets[1].pull = Some("bar".into());
                m.cabinets[1].pull_symbol = Some("bar".into());
            }),
            Box::new(|m| m.cabinets[1].position.x = 1.0),
            Box::new(|m| m.furnishings.push(sink_on(FurnishingHost::Cabinet { cabinet: ArchId(0) }, [0.0, 0.3]))),
            Box::new(|m| m.furnishings[0].cutout = Some([0.3, 0.2])),
            Box::new(|m| {
                let host = FurnishingHost::Gap { left: ArchId(0), right: ArchId(1) };
                m.furnishings.push(Furnishing { id: ArchId(1), at: [0.5, 0.3], size: [0.4, 0.6, 0.85], host, ..Furnishing::default() });
            }),
            Box::new(|m| {
                m.symbols.remove("bar");
            }),
            Box::new(|m| m.cabinets[0].counter = false),
        ];
        for edit in edits {
            edit(&mut model);
            cache.update(&model);
            assert_eq!(cache.scene().digest(), crate::Scene3d::build(&model).digest());
        }
    }

    #[test]
    fn cabinet_edit_rebuilds_only_itself() {
        let mut model = base_model();
        let (mut cache, _) = crate::SceneCache::new(&model);
        model.cabinets[0].position.x += 0.2;
        let stats = cache.update(&model);
        assert!(stats.incremental);
        assert_eq!(stats.rebuilt, 1);
    }

    /// A pull symbol with two parts (a bar and a cap).
    fn two_part_symbol() -> SymbolMesh {
        let positions = vec![
            [-0.05_f32, -0.01_f32, 0.0_f32],
            [0.05_f32, -0.01_f32, 0.0_f32],
            [0.05_f32, 0.01_f32, 0.0_f32],
            [-0.05_f32, 0.01_f32, 0.0_f32],
            [-0.05_f32, -0.01_f32, 0.02_f32],
            [0.05_f32, -0.01_f32, 0.02_f32],
            [0.05_f32, 0.01_f32, 0.02_f32],
            [-0.05_f32, 0.01_f32, 0.02_f32],
        ];
        let indices = vec![0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6, 3, 0, 4, 3, 4, 7];
        let normals = vec![[0.0, 0.0, 1.0]; 8];
        SymbolMesh {
            key: "two".into(),
            parts: vec![
                SymbolPart {
                    name: "bar".into(),
                    positions: positions.clone(),
                    normals: normals.clone(),
                    indices: indices.clone(),
                    base_color: [1.0; 4],
                    roughness: 0.5,
                    ..SymbolPart::default()
                },
                SymbolPart { name: "cap".into(), positions, normals, indices, base_color: [0.5; 4], roughness: 0.5, ..SymbolPart::default() },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn pull_symbol_with_multiple_parts() {
        let mut c = cab(0, 0.0, Fronts::Doors);
        c.pull = Some("two".into());
        c.pull_symbol = Some("two".into());
        let mut model = ArchModel::default();
        model.cabinets.push(c);
        model.symbols.insert("two".into(), two_part_symbol());
        let meshes = cabinet_meshes(&model, &model.cabinets[0]);
        let pulls: Vec<&Mesh> = meshes.iter().filter(|m| m.material.starts_with("pull:two:")).collect();
        assert_eq!(pulls.len(), 2, "two pull meshes for two parts");
        assert_eq!(pulls[0].material, "pull:two:0");
        assert_eq!(pulls[1].material, "pull:two:1");
    }

    #[test]
    fn cabinet_with_rotation_180_extends_in_negative_y() {
        let mut model = ArchModel::default();
        model.cabinets.push(Cabinet {
            id: ArchId(0),
            kind: CabinetKind::Base,
            level: 0,
            position: Vec2::new(2.0, 3.0),
            rotation: 180.0,
            width: 0.6,
            depth: 0.6,
            height: 0.876,
            elevation: 0.0,
            fronts: Fronts::Doors,
            doors: 1,
            ..Cabinet::default()
        });
        let meshes = cabinet_meshes(&model, &model.cabinets[0]);
        let mut bounds = meshes[0].bounds();
        for m in &meshes[1..] {
            bounds.extend(&m.bounds());
        }
        // With rotation 180, the cabinet extends from x=1.7 to x=2.3 and y=2.4 to y=3.0
        assert!((bounds.min[0] - 1.7).abs() < 1e-2, "min x {}", bounds.min[0]);
        assert!((bounds.max[0] - 2.3).abs() < 1e-2, "max x {}", bounds.max[0]);
        assert!((bounds.min[1] - 2.4).abs() < 1e-2, "min y {}", bounds.min[1]);
        assert!((bounds.max[1] - 3.0).abs() < 1e-2, "max y {}", bounds.max[1]);
    }

    #[test]
    fn counter_run_with_cutout_most_of_counter() {
        let mut model = model_of(vec![cab(0, 0.0, Fronts::Doors)]);
        // A cutout that covers most of the counter but not all of it
        model.furnishings.push(Furnishing {
            id: ArchId(0),
            at: [0.0, 0.3],
            size: [0.5, 0.4, 0.2],
            host: FurnishingHost::Cabinet { cabinet: ArchId(0) },
            cutout: Some([0.55, 0.55]),
            ..Furnishing::default()
        });
        let all = counters(&model);
        assert_eq!(all.len(), 1);
        // The hole is inside the counter, so the counter has a thin border
        let area = top_area(&all[0].1);
        let expect = 0.6 * 0.625 - 0.55 * 0.55;
        assert!((area - expect).abs() < 1e-6, "counter area {area} vs {expect}");
        assert!(all[0].1.is_manifold(), "counter with a large cutout should be closed");
    }

    #[test]
    fn cabinet_with_zero_width_does_not_crash() {
        let mut model = ArchModel::default();
        model.cabinets.push(Cabinet {
            id: ArchId(0),
            kind: CabinetKind::Base,
            level: 0,
            position: Vec2::new(0.0, 0.0),
            rotation: 0.0,
            width: 0.0,
            depth: 0.6,
            height: 0.876,
            elevation: 0.0,
            fronts: Fronts::Doors,
            doors: 1,
            ..Cabinet::default()
        });
        let meshes = cabinet_meshes(&model, &model.cabinets[0]);
        // A zero-width cabinet produces no non-finite vertices
        for m in &meshes {
            assert!(m.positions.iter().all(|p| p.iter().all(|v| v.is_finite())), "non-finite vertex");
        }
    }

    #[test]
    fn pull_on_a_small_front() {
        let mut model = ArchModel::default();
        model.cabinets.push(Cabinet {
            id: ArchId(0),
            kind: CabinetKind::Base,
            level: 0,
            position: Vec2::new(0.0, 0.0),
            rotation: 0.0,
            width: 0.1,
            depth: 0.6,
            height: 0.876,
            elevation: 0.0,
            fronts: Fronts::Doors,
            doors: 1,
            pull: Some("bar".into()),
            pull_symbol: Some("bar".into()),
            ..Cabinet::default()
        });
        model.symbols.insert("bar".into(), bar_symbol());
        let meshes = cabinet_meshes(&model, &model.cabinets[0]);
        let pull = mesh_named(&meshes, "pull:bar:0");
        assert!(pull.is_some(), "pull should be placed even on a small front");
    }
}
