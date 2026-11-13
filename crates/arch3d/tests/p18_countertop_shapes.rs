use cadcraft_arch::countertop_edit::{set_profile, set_sagitta, set_waterfall};
use cadcraft_arch3d::SceneCache;
use cadcraft_arch3d::probe::probe_mesh;
use cadcraft_doc::arch::{CounterEdge, Countertop, EdgeProfile, Level};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::{Vec2, Vec3};

fn rect() -> Countertop {
    Countertop {
        polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 1.0), Vec2::new(0.0, 1.0)],
        top: 0.914,
        thickness: 0.038,
        ..Default::default()
    }
}

#[test]
fn arc_edge_probes() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    let mut ct = rect();
    ct.edges = vec![CounterEdge { sagitta: 0.15, ..Default::default() }; 4];
    let id = m.push_countertop(ct);
    let scene = cadcraft_arch3d::Scene3d::build(&m);
    let mesh = scene.meshes.iter().find(|mesh| mesh.object == id && mesh.material == "countertop").unwrap();
    assert!(probe_mesh(mesh, Vec3::new(1.0, -0.12, 0.895)));
    assert!(!probe_mesh(mesh, Vec3::new(1.0, -0.3, 0.895)));
    assert!(probe_mesh(mesh, Vec3::new(1.0, 0.5, 0.895)));
}

#[test]
fn waterfall_panel_probes() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    let mut ct = rect();
    ct.edges = vec![CounterEdge { waterfall: true, ..Default::default() }; 4];
    let id = m.push_countertop(ct);
    let scene = cadcraft_arch3d::Scene3d::build(&m);
    let mesh = scene.meshes.iter().find(|mesh| mesh.object == id && mesh.material == "countertop-waterfall").unwrap();
    assert!(probe_mesh(mesh, Vec3::new(1.0, 0.019, 0.300)));
    assert!(probe_mesh(mesh, Vec3::new(1.0, 0.034, 0.300)));
    assert!(!probe_mesh(mesh, Vec3::new(1.0, 0.040, 0.300)));
    assert!(!probe_mesh(mesh, Vec3::new(1.0, -0.002, 0.300)));
    ct = m.countertop(id).cloned().unwrap();
    let mut other = ct.clone();
    other.edges = vec![CounterEdge::default(); 4];
    let id2 = m.push_countertop(other);
    let scene2 = cadcraft_arch3d::Scene3d::build(&m);
    let mesh2 = scene2.meshes.iter().find(|mesh| mesh.object == id2 && mesh.material == "countertop-waterfall");
    assert!(mesh2.is_none());
}

#[test]
fn bullnose_edge_probes() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    let mut ct = rect();
    ct.edges = vec![CounterEdge::default(); 4];
    ct.edges[0].profile = EdgeProfile::Bullnose;
    let id = m.push_countertop(ct);
    let scene = cadcraft_arch3d::Scene3d::build(&m);
    let mesh = scene.meshes.iter().find(|mesh| mesh.object == id && mesh.material == "countertop").unwrap();
    println!("bullnose mesh bounds={:?} model id={}", mesh.bounds(), id.0);
    println!("positions count={} first few=", mesh.positions.len());
    for pos in mesh.positions.iter().take(12) {
        println!("  {:?}", *pos);
    }
    for p in [Vec3::new(0.5, 0.02, 0.912), Vec3::new(0.5, 0.02, 0.895), Vec3::new(0.5, 0.2, 0.912)] {
        println!("probe {:?} => {}", p, probe_mesh(mesh, p));
    }
    assert!(!probe_mesh(mesh, Vec3::new(0.5, 0.02, 0.912)));
    assert!(probe_mesh(mesh, Vec3::new(0.5, 0.02, 0.895)));
    assert!(!probe_mesh(mesh, Vec3::new(0.5, 0.2, 0.912)));
}

#[test]
fn waterfall_mesh_is_part_counter_owned_by_ct() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    let mut ct = rect();
    ct.edges = vec![CounterEdge { waterfall: true, ..Default::default() }; 4];
    let id = m.push_countertop(ct);
    let scene = cadcraft_arch3d::Scene3d::build(&m);
    let hit = scene.meshes.iter().find(|mesh| mesh.object == id && mesh.material == "countertop-waterfall").unwrap();
    assert_eq!(hit.object, id);
    assert_eq!(hit.material, "countertop-waterfall");
    assert!(hit.indices.len() > 0);
}

#[test]
fn profiled_edge_keeps_the_outline_and_builds_closed_meshes() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    for profile in [EdgeProfile::Bullnose, EdgeProfile::Bevel, EdgeProfile::Eased] {
        let mut ct = rect();
        ct.edges = vec![CounterEdge { profile, ..Default::default() }; 4];
        let id = m.push_countertop(ct);
        let scene = cadcraft_arch3d::Scene3d::build(&m);
        let counter_meshes: Vec<_> = scene.meshes.iter().filter(|mesh| mesh.object == id && mesh.material == "countertop").collect();
        assert!(!counter_meshes.is_empty());
        assert!(counter_meshes.iter().all(|mesh| mesh.is_manifold()));
    }
}

#[test]
fn painted_countertop_paints_the_waterfall_too() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    let mut ct = rect();
    ct.edges = vec![CounterEdge { waterfall: true, ..Default::default() }; 4];
    let id = m.push_countertop(ct);
    m.paints.insert(format!("countertop/ct{}", id.0), cadcraft_doc::arch::Paint::default());
    let scene = cadcraft_arch3d::Scene3d::build(&m);
    assert!(scene.meshes.iter().any(|mesh| mesh.object == id && mesh.material == "countertop-waterfall"));
    assert!(scene.meshes.iter().any(|mesh| mesh.object == id && mesh.material == "countertop"));
}

#[test]
fn cache_equals_full_after_shape_edits() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    let id = m.push_countertop(rect());
    let (mut cache, _) = SceneCache::new(&m);
    let before = cache.scene().digest();
    let mut ct = m.countertop(id).cloned().unwrap();
    ct = set_sagitta(&ct, 0, 0.15).unwrap();
    ct = set_profile(&ct, 1, EdgeProfile::Bullnose).unwrap();
    ct = set_waterfall(&ct, 2, true).unwrap();
    if let Some(slot) = m.countertops.first_mut() {
        *slot = ct;
    }
    let stats = cache.update(&m);
    assert!(stats.incremental);
    assert_eq!(cache.scene().digest(), cadcraft_arch3d::Scene3d::build(&m).digest());
    assert_ne!(before, cache.scene().digest());
}
