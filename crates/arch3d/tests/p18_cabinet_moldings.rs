use cadcraft_arch::molding_profile::builtin_profile;
use cadcraft_arch::molding_runs::run_length;
use cadcraft_arch3d::{Scene3d, SceneCache};
use cadcraft_doc::arch::{Cabinet, CabinetKind, CabinetMolding, Fronts, Level};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::Vec2;

fn wall_cabinet(id: u32, x: f64) -> Cabinet {
    Cabinet {
        id: ArchId(id),
        kind: CabinetKind::Wall,
        level: 0,
        position: Vec2::new(x, 0.0),
        width: 0.6,
        depth: 0.305,
        height: 0.762,
        elevation: 1.372,
        ..Default::default()
    }
}

fn crown_molding() -> CabinetMolding {
    CabinetMolding { profile: builtin_profile("molding:crown").unwrap(), size: None, ..Default::default() }
}

fn light_rail_molding() -> CabinetMolding {
    CabinetMolding { profile: builtin_profile("molding:casing").unwrap(), size: None, ..Default::default() }
}

#[test]
fn crown_probe_above_the_top() {
    let mut c = wall_cabinet(0, 0.0);
    c.crown = Some(crown_molding());
    let mut model = ArchModel::default();
    model.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    model.cabinets.push(c);
    let scene = Scene3d::build(&model);
    let hit = scene.probe(cadcraft_geom::Vec3::new(0.0, 0.308, 2.18));
    assert!(hit.as_ref().is_some_and(|h| h.id == "cb0" && h.part == "cabinet"), "hit={hit:?}");
    let above = scene.probe(cadcraft_geom::Vec3::new(0.0, 0.308, 2.25));
    assert!(above.is_none(), "above should miss crown: {above:?}");
}

#[test]
fn light_rail_probe_below() {
    let mut c = wall_cabinet(0, 0.0);
    c.light_rail = Some(light_rail_molding());
    let mut model = ArchModel::default();
    model.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    model.cabinets.push(c);
    let scene = Scene3d::build(&model);
    let hit = scene.probe(cadcraft_geom::Vec3::new(0.0, 0.308, 1.34));
    assert!(hit.as_ref().is_some_and(|h| h.id == "cb0" && h.part == "cabinet"), "hit={hit:?}");
}

#[test]
fn crown_mesh_is_part_cabinet_owned_by_the_cabinet() {
    let mut c = wall_cabinet(7, 0.0);
    c.crown = Some(crown_molding());
    let mut model = ArchModel::default();
    model.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    model.cabinets.push(c.clone());
    let meshes = cadcraft_arch3d::cabinet_parts::molding::molding_meshes(&model, &c);
    let mesh = meshes.iter().find(|m| m.material == "cabinet-crown").expect("crown mesh exists");
    assert_eq!(mesh.object, c.id);
    assert_eq!(mesh.material, "cabinet-crown");
    assert!(cadcraft_arch3d::probe::part_of(mesh).contains("cabinet") || mesh.material.starts_with("cabinet-"));
}

#[test]
fn toggling_a_neighbours_crown_rebuilds_both_and_equals_full() {
    let mut a = wall_cabinet(0, 0.0);
    let mut b = wall_cabinet(1, 0.9);
    a.crown = Some(crown_molding());
    b.crown = Some(crown_molding());
    let mut model = ArchModel::default();
    model.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    model.cabinets.push(a.clone());
    model.cabinets.push(b.clone());
    let full = Scene3d::build(&model);

    let mut one = model.clone();
    one.cabinets[1].crown = None;
    let (mut cache, _) = SceneCache::new(&one);
    cache.update(&model);
    assert_eq!(cache.scene().digest(), full.digest());
    let crown_count = full.meshes.iter().filter(|m| m.material == "cabinet-crown").count();
    assert!(crown_count >= 2);
}

#[test]
fn a_filler_under_crown_builds_its_crown_piece() {
    let mut cb0 = wall_cabinet(0, 0.0);
    cb0.crown = Some(crown_molding());
    let mut cb1 = wall_cabinet(1, 0.325);
    cb1.width = 0.05;
    cb1.filler = true;
    cb1.fronts = Fronts::Open;
    cb1.crown = Some(crown_molding());
    let mut model = ArchModel::default();
    model.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    model.cabinets.push(cb0);
    model.cabinets.push(cb1.clone());
    let scene = Scene3d::build(&model);
    let filler_mesh = scene.meshes.iter().find(|m| m.object == cb1.id && m.material == "cabinet-crown").expect("filler crown mesh exists");
    assert_eq!(filler_mesh.object, cb1.id);
    let hit = scene.probe(cadcraft_geom::Vec3::new(0.325, 0.308, 2.18));
    assert!(hit.as_ref().is_some_and(|h| h.id == "cb1" && h.part == "cabinet"), "hit={hit:?}");

    let mut without_filler = model.clone();
    without_filler.cabinets[1].crown = None;
    assert!((run_length(&cadcraft_arch::casework_molding::crown_run(&without_filler, &without_filler.cabinets[0]).unwrap()) - 1.21).abs() < 1e-9);
}
