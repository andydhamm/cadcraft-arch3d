//! Phase 18 custom countertops in the 3D scene: probes, claimed cabinets, the carried-over sink
//! cut-out, and the incremental cache agreeing with a full build.

use cadcraft_arch::countertop_edit::{detach, move_edge};
use cadcraft_arch3d::probe::probe_mesh;
use cadcraft_arch3d::{Mesh, Scene3d, SceneCache};
use cadcraft_doc::arch::{Cabinet, CabinetKind, Countertop, Furnishing, FurnishingHost, Level};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::{Vec2, Vec3};

fn base(x: f64) -> Cabinet {
    Cabinet {
        kind: CabinetKind::Base,
        position: Vec2::new(x, 2.0),
        width: 0.9,
        depth: 0.6,
        height: 0.876,
        counter: true,
        counter_thickness: 0.038,
        counter_overhang: 0.025,
        ..Default::default()
    }
}

/// An island of two base cabinets (x 1.55..3.35, y 2.0..2.625, top 0.914) with a sink cut-out
/// (0.45 x 0.35) centred on (2.0, 2.3).
fn island() -> ArchModel {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
    m.push_cabinet(base(2.0));
    m.push_cabinet(base(2.9));
    m.push_furnishing(Furnishing {
        item: "sink".into(),
        at: [2.0, 2.3],
        size: [0.5, 0.4, 0.2],
        cutout: Some([0.45, 0.35]),
        host: FurnishingHost::Cabinet { cabinet: ArchId(0) },
        elevation: 0.9,
        ..Default::default()
    });
    m
}

fn detached(m: &mut ArchModel) -> ArchId {
    let ct = detach(m, ArchId(1)).unwrap_or_default();
    m.push_countertop(ct)
}

fn count(s: &Scene3d, material: &str) -> usize {
    s.meshes.iter().filter(|m| m.material == material).count()
}

fn slab(s: &Scene3d) -> Option<&Mesh> {
    s.meshes.iter().find(|m| m.material == "countertop")
}

#[test]
fn custom_countertop_probes() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.2, ..Default::default() });
    let id = m.push_countertop(Countertop {
        polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 1.0), Vec2::new(0.0, 1.0)],
        top: 0.914,
        thickness: 0.04,
        ..Default::default()
    });
    let s = Scene3d::build(&m);
    let hit = s.probe(Vec3::new(1.0, 0.5, 0.2 + 0.914 - 0.02)).map(|h| (h.id, h.part));
    assert_eq!(hit, Some((format!("ct{}", id.0), "counter".to_string())));
    assert!(s.probe(Vec3::new(3.0, 0.5, 0.2 + 0.914 - 0.02)).is_none(), "outside the outline");
    assert!(s.probe(Vec3::new(1.0, 0.5, 0.2 + 0.914 - 0.05)).is_none(), "below the slab");
    assert!(s.probe(Vec3::new(1.0, 0.5, 0.2 + 0.914 + 0.01)).is_none(), "above the slab");
    assert!(s.probe(Vec3::new(1.0, 0.5, 0.2 + 0.914 - 0.039)).is_some(), "just above the bottom");
}

#[test]
fn claimed_cabinets_build_no_automatic_counter() {
    let mut m = island();
    let before = Scene3d::build(&m);
    assert_eq!((count(&before, "counter"), count(&before, "countertop")), (1, 0));
    detached(&mut m);
    let after = Scene3d::build(&m);
    assert_eq!((count(&after, "counter"), count(&after, "countertop")), (0, 1));
    assert_eq!(count(&before, "cabinet-box"), count(&after, "cabinet-box"), "the cabinets stay");
}

#[test]
fn sink_hole_survives_detach() {
    let mut m = island();
    detached(&mut m);
    let scene = Scene3d::build(&m);
    let Some(mesh) = slab(&scene) else { panic!("the detached countertop has a mesh") };
    let z = 0.914 - 0.019;
    let (cx, cy, w, d) = (2.0, 2.3, 0.45, 0.35);
    let e = 0.003;
    let at = |x: f64, y: f64| probe_mesh(mesh, Vec3::new(x, y, z));
    for (x, y) in [(cx, cy), (cx - w / 2.0 + e, cy), (cx + w / 2.0 - e, cy), (cx, cy - d / 2.0 + e), (cx, cy + d / 2.0 - e)] {
        assert!(!at(x, y), "({x}, {y}) is inside the cut-out");
    }
    for (x, y) in [(cx - w / 2.0 - e, cy), (cx + w / 2.0 + e, cy), (cx, cy - d / 2.0 - e), (cx, cy + d / 2.0 + e)] {
        assert!(at(x, y), "({x}, {y}) is in the counter next to the cut-out");
    }
}

#[test]
fn cache_equals_full_after_detach_move_delete() {
    let mut m = island();
    let (mut cache, _) = SceneCache::new(&m);
    let check = |cache: &mut SceneCache, m: &ArchModel, what: &str| {
        let stats = cache.update(m);
        assert!(stats.incremental, "{what}");
        assert_eq!(cache.scene().digest(), Scene3d::build(m).digest(), "{what}");
    };
    let id = detached(&mut m);
    check(&mut cache, &m, "detach");
    let ct = m.countertop(id).cloned().unwrap_or_default();
    let edge = ct.polygon.iter().position(|p| (p.y - 2.625).abs() < 1e-9).unwrap_or(0);
    let moved = move_edge(&ct, edge, 0.3).unwrap_or(ct);
    if let Some(slot) = m.countertops.first_mut() {
        *slot = moved;
    }
    check(&mut cache, &m, "move");
    m.countertops.clear();
    check(&mut cache, &m, "delete");
}

#[test]
fn delete_brings_the_automatic_counter_back() {
    let mut m = island();
    let auto = Scene3d::build(&m).digest();
    detached(&mut m);
    m.countertops.clear();
    let back = Scene3d::build(&m);
    assert_eq!((count(&back, "counter"), count(&back, "countertop")), (1, 0));
    assert_eq!(back.digest(), auto);
}
