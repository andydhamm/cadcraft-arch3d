//! Cabinets and casework fixtures in 3D.

use crate::Mesh;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Cabinet, Fixture, FixtureKind, Fronts};
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

fn door_count(c: &Cabinet) -> usize {
    c.doors.max(1) as usize
}

fn drawer_count(c: &Cabinet) -> usize {
    c.drawers.max(1) as usize
}

/// Meshes for a cabinet: carcass, front faces and optional counter.
pub fn cabinet_meshes(model: &ArchModel, c: &Cabinet) -> Vec<Mesh> {
    let level_z = level_elevation(model, c.level) + c.elevation;
    let toe = if c.toe_kick { 0.1 } else { 0.0 };
    let mut meshes = Vec::new();

    let mut carcass = Mesh::new("cabinet-box", c.id);
    push_box(&mut carcass, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, 0.0, c.depth - 0.019, level_z + toe, level_z + c.height);
    if !carcass.indices.is_empty() {
        meshes.push(carcass);
    }

    if c.toe_kick {
        let mut plinth = Mesh::new("cabinet-box", c.id);
        push_box(&mut plinth, c.position, c.rotation, -c.width / 2.0, c.width / 2.0, 0.0, c.depth - 0.075, level_z, level_z + 0.1);
        if !plinth.indices.is_empty() {
            meshes.push(plinth);
        }
    }

    let mut front = Mesh::new("cabinet-front", c.id);
    let front_y0 = c.depth - 0.019;
    let front_y1 = c.depth;
    let gap = 0.003;
    let usable_h = c.height - toe;
    let mut add_front = |x0: f64, x1: f64, z0: f64, z1: f64| {
        if x1 <= x0 || z1 <= z0 || !x0.is_finite() || !x1.is_finite() || !z0.is_finite() || !z1.is_finite() {
            return;
        }
        push_box(&mut front, c.position, c.rotation, x0, x1, front_y0, front_y1, level_z + z0, level_z + z1);
    };

    match c.fronts {
        Fronts::Doors => {
            let door_w = (c.width - gap * (door_count(c) as f64 + 1.0)) / door_count(c) as f64;
            for i in 0..door_count(c) {
                let x0 = -c.width / 2.0 + gap + (i as f64) * (door_w + gap);
                let x1 = x0 + door_w;
                add_front(x0, x1, toe, usable_h);
            }
        }
        Fronts::Drawers => {
            let drawer_h = usable_h / drawer_count(c) as f64;
            for i in 0..drawer_count(c) {
                let z0 = usable_h - (i as f64 + 1.0) * drawer_h;
                let z1 = usable_h - i as f64 * drawer_h;
                add_front(-c.width / 2.0 + gap, c.width / 2.0 - gap, z0, z1);
            }
        }
        Fronts::DrawerOverDoor => {
            let drawer_h = 0.15_f64.min(usable_h / drawer_count(c) as f64);
            let door_h = (usable_h - drawer_h * drawer_count(c) as f64) / door_count(c) as f64;
            for i in 0..drawer_count(c) {
                let z0 = usable_h - (i as f64 + 1.0) * drawer_h;
                let z1 = usable_h - i as f64 * drawer_h;
                add_front(-c.width / 2.0 + gap, c.width / 2.0 - gap, z0, z1);
            }
            let door_w = (c.width - gap * (door_count(c) as f64 + 1.0)) / door_count(c) as f64;
            for i in 0..door_count(c) {
                let x0 = -c.width / 2.0 + gap + (i as f64) * (door_w + gap);
                let x1 = x0 + door_w;
                add_front(x0, x1, toe, door_h);
            }
        }
    }
    if !front.indices.is_empty() {
        meshes.push(front);
    }

    if c.counter {
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

    meshes
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

    #[test]
    fn cabinet_edit_rebuilds_only_itself() {
        let mut model = base_model();
        let (mut cache, _) = crate::SceneCache::new(&model);
        model.cabinets[0].position.x += 0.2;
        let stats = cache.update(&model);
        assert!(stats.incremental);
        assert_eq!(stats.rebuilt, 1);
    }
}
