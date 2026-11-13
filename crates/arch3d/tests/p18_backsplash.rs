//! Phase 18 backsplash: probes, the incremental cache and paint.

use cadcraft_arch3d::{Scene3d, SceneCache};
use cadcraft_doc::arch::{Backsplash, Cabinet, CabinetKind, CounterRef, Level, MaterialRef, Paint};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::{Vec2, Vec3};

fn cabinet(kind: CabinetKind, x0: f64, elevation: f64) -> Cabinet {
    let base = kind == CabinetKind::Base;
    Cabinet {
        kind,
        position: Vec2::new(x0 + 0.3, 0.0),
        width: 0.6,
        depth: if base { 0.6 } else { 0.33 },
        height: if base { 0.876 } else { 0.7 },
        elevation,
        counter: base,
        counter_thickness: 0.038,
        counter_overhang: 0.025,
        ..Default::default()
    }
}

/// A wall whose room face is y = 0, three base cabinets over x 0.6..2.4 (counter top 0.914) and a
/// backsplash following the run (`cb0`).
fn kitchen() -> ArchModel {
    let mut m = ArchModel::default();
    m.levels.push(Level::default());
    m.push_wall(Wall { start: Vec2::new(0.0, -0.1), end: Vec2::new(6.0, -0.1), thickness: 0.2, height: 2.4, ..Default::default() });
    for i in 0..3 {
        m.push_cabinet(cabinet(CabinetKind::Base, 0.6 + 0.6 * f64::from(i), 0.0));
    }
    m.push_backsplash(Backsplash { counter: Some(CounterRef::Cabinet(ArchId(0))), ..Default::default() });
    m
}

fn hit(scene: &Scene3d, p: [f64; 3]) -> Option<(String, String)> {
    scene.probe(Vec3::new(p[0], p[1], p[2])).map(|h| (h.id, h.part))
}

#[test]
fn backsplash_probes() {
    let mut model = kitchen();
    model.push_cabinet(cabinet(CabinetKind::Wall, 0.6, 1.372));
    let scene = Scene3d::build(&model);
    assert_eq!(scene.meshes.iter().filter(|m| m.material == "backsplash").count(), 1);
    let want = Some(("bs0".to_string(), "backsplash".to_string()));
    // Inside the 10 mm, at the middle of the run.
    assert_eq!(hit(&scene, [1.5, 0.005, 1.1]), want);
    // 15 mm out from the face: nothing.
    assert_eq!(hit(&scene, [1.5, 0.015, 1.1]), None);
    // Under the wall cabinet the top is 1.372; beside it 1.364.
    assert_eq!(hit(&scene, [0.9, 0.005, 1.368]), want);
    assert_eq!(hit(&scene, [1.5, 0.005, 1.368]), None);
    assert_eq!(hit(&scene, [1.5, 0.005, 1.360]), want);
    // Below the counter top, and past the run's ends.
    assert_ne!(hit(&scene, [1.5, 0.005, 0.9]), want);
    assert_eq!(hit(&scene, [2.5, 0.005, 1.1]), None);
    assert_eq!(hit(&scene, [0.5, 0.005, 1.1]), None);
    // Inside the wall: not the backsplash.
    assert_ne!(hit(&scene, [1.5, -0.05, 1.1]), want);
    // To the ceiling.
    model.backsplashes[0].to_ceiling = true;
    let scene = Scene3d::build(&model);
    assert_eq!(hit(&scene, [1.5, 0.005, 2.3]), want);
}

#[test]
fn adding_a_wall_cabinet_above_rebuilds_the_backsplash_and_equals_full() {
    let mut model = kitchen();
    let (mut cache, _) = SceneCache::new(&model);
    let before = Scene3d::build(&model);
    assert_eq!(hit(&before, [0.9, 0.005, 1.368]), None);
    model.push_cabinet(cabinet(CabinetKind::Wall, 0.6, 1.372));
    let stats = cache.update(&model);
    assert!(stats.incremental);
    let full = Scene3d::build(&model);
    assert_eq!(cache.scene().digest(), full.digest());
    let want = Some(("bs0".to_string(), "backsplash".to_string()));
    assert_eq!(hit(cache.scene(), [0.9, 0.005, 1.368]), want);
    assert_eq!(hit(cache.scene(), [1.5, 0.005, 1.368]), None);
    // Removing it again restores the old backsplash.
    model.cabinets.pop();
    cache.update(&model);
    assert_eq!(cache.scene().digest(), Scene3d::build(&model).digest());
    assert_eq!(hit(cache.scene(), [0.9, 0.005, 1.368]), None);
    // Turning a cabinet's backsplash off drops its stretch.
    model.cabinets[2].backsplash_off = true;
    cache.update(&model);
    assert_eq!(cache.scene().digest(), Scene3d::build(&model).digest());
    assert_eq!(hit(cache.scene(), [2.1, 0.005, 1.1]), None);
    assert_eq!(hit(cache.scene(), [1.5, 0.005, 1.1]), want);
}

#[test]
fn painted_backsplash_takes_its_material() {
    let mut model = kitchen();
    let plain = Scene3d::build(&model);
    assert!(plain.meshes.iter().any(|m| m.material == "backsplash" && m.object == ArchId(0)));
    model.paints.insert("backsplash/bs0".to_string(), Paint { material: Some(MaterialRef::Colour([0xc0, 0x40, 0x20])), ..Default::default() });
    let scene = Scene3d::build(&model);
    assert!(scene.meshes.iter().all(|m| m.material != "backsplash"));
    let painted: Vec<_> = scene.meshes.iter().filter(|m| m.material == "paint:backsplash/bs0").collect();
    assert_eq!(painted.len(), 1);
    assert_eq!(painted[0].object, ArchId(0));
    assert!(scene.materials.contains_key("paint:backsplash/bs0"));
    // The probe still reports the backsplash.
    assert_eq!(hit(&scene, [1.5, 0.005, 1.1]), Some(("bs0".to_string(), "backsplash".to_string())));
    // The cache agrees.
    let (mut cache, _) = SceneCache::new(&kitchen());
    cache.update(&model);
    assert_eq!(cache.scene().digest(), scene.digest());
}
