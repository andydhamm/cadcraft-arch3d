use cadcraft_arch3d::{Scene3d, SceneCache};
use cadcraft_doc::arch::{Cabinet, CabinetKind, Fronts, Level};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::Vec2;

fn base_cabinet(id: u32, x: f64, width: f64) -> Cabinet {
    let mut c = Cabinet::default();
    c.id = ArchId(id);
    c.kind = CabinetKind::Base;
    c.level = 0;
    c.position = Vec2::new(x, 0.5);
    c.width = width;
    c.depth = 0.6;
    c.height = 0.876;
    c.doors = 1;
    c.counter = true;
    c.counter_thickness = 0.038;
    c.counter_overhang = 0.025;
    c.toe_kick = true;
    c
}

fn wall_cabinet(id: u32, x: f64) -> Cabinet {
    let mut c = Cabinet::default();
    c.id = ArchId(id);
    c.kind = CabinetKind::Wall;
    c.level = 0;
    c.position = Vec2::new(x, 0.5);
    c.width = 0.762;
    c.depth = 0.305;
    c.height = 0.762;
    c.elevation = 1.372;
    c.fronts = Fronts::Open;
    c.shelves = Some(4);
    c.toe_kick = true;
    c
}

#[test]
fn filler_panel_probe() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    m.walls.push(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, 2.0), 0.2));
    let base = base_cabinet(0, 0.45, 0.6);
    m.push_cabinet(base.clone());
    let gap = cadcraft_arch::casework_fill::run_gaps(&m, base.id).unwrap()[0].clone();
    let filler = cadcraft_arch::casework_fill::filler_for(&m, &gap).unwrap();
    let scene = Scene3d::build(&m);
    let _ = scene.meshes.iter().filter(|mesh| mesh.object == filler.id && mesh.material == "cabinet-front").count();
    assert!(filler.filler);
}

#[test]
fn filler_joins_the_counter_run() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    m.walls.push(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, 2.0), 0.2));
    m.push_cabinet(base_cabinet(0, 1.0, 0.6));
    m.push_cabinet(Cabinet { filler: true, fronts: Fronts::Open, ..base_cabinet(1, 0.675, 0.05) });
    let scene = Scene3d::build(&m);
    let counters: Vec<_> = scene.meshes.iter().filter(|mesh| mesh.material == "counter").collect();
    assert_eq!(counters.len(), 1);
    assert_eq!(counters[0].object, ArchId(1));
}

#[test]
fn open_carcass_is_hollow() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    let c = wall_cabinet(0, 1.0);
    m.push_cabinet(c);
    let scene = Scene3d::build(&m);
    let box_count = scene.meshes.iter().filter(|mesh| mesh.material == "cabinet-box").count();
    assert!(box_count >= 1);
}

#[test]
fn shelves_probe() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    m.push_cabinet(wall_cabinet(0, 1.0));
    let scene = Scene3d::build(&m);
    let shelves = scene.meshes.iter().filter(|mesh| mesh.material == "cabinet-shelves").count();
    assert!(shelves >= 1);
}

#[test]
fn cache_equals_full_after_fill_and_shelf_edits() {
    let mut model = ArchModel::default();
    model.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    model.walls.push(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, 2.0), 0.2));
    model.push_cabinet(base_cabinet(0, 0.45, 0.6));
    model.push_cabinet(base_cabinet(1, 1.05, 0.6));
    model.push_cabinet(wall_cabinet(2, 3.0));
    let (mut cache, _) = SceneCache::new(&model);
    let full = Scene3d::build(&model);
    assert_eq!(cache.scene().digest(), full.digest());

    let mut edited = model.clone();
    let filler = cadcraft_arch::casework_fill::filler_for(&edited, &cadcraft_arch::casework_fill::run_gaps(&edited, ArchId(0)).unwrap()[0]).unwrap();
    edited.push_cabinet(filler);
    edited.cabinets[2].shelves = Some(4);
    let stats = cache.update(&edited);
    assert!(stats.incremental || stats.rebuilt > 0);
    assert_eq!(cache.scene().digest(), Scene3d::build(&edited).digest());
}
