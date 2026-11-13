//! Phase 18 corner cabinets in 3D: one counter mesh round a corner, an L-shaped lazy-susan
//! carcass with a hollow notch, a blind corner's false panel, byte-identical unmerged runs and a
//! cache that follows corner edits. The kitchen is the check's: inner faces x 0..6, y 0..4, a south
//! run from the south-west corner and a west run butting against or starting away from it.

use cadcraft_arch::casework::cabinet_defaults;
use cadcraft_arch3d::probe::part_of;
use cadcraft_arch3d::{Mesh, Scene3d, SceneCache};
use cadcraft_doc::arch::{Cabinet, CabinetKind, CornerPref};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::{Vec2, Vec3};

fn base(id: u32, x: f64, y: f64, rotation: f64, width: f64) -> Cabinet {
    Cabinet { id: ArchId(id), position: Vec2::new(x, y), rotation, width, ..cabinet_defaults(CabinetKind::Base) }
}

/// A south run 0.9 + 0.6 + 0.6 (cb0..cb2) and a west run of two 0.6 (cb3 at y 1.2..1.8, cb4 at 0.6..1.2)
/// butting against it: cb0 is a blind corner.
fn blind() -> ArchModel {
    ArchModel {
        cabinets: vec![
            base(0, 0.45, 0.0, 0.0, 0.9),
            base(1, 1.2, 0.0, 0.0, 0.6),
            base(2, 1.8, 0.0, 0.0, 0.6),
            base(3, 0.0, 1.5, 270.0, 0.6),
            base(4, 0.0, 0.9, 270.0, 0.6),
        ],
        ..Default::default()
    }
}

/// A south run from x 0.9 (cb0, cb1), a west run from y 0.9 (cb3 at 1.5..2.1, cb4 at 0.9..1.5) and a 0.9
/// cabinet pushed into the corner (cb2): a lazy susan.
fn lazy() -> ArchModel {
    ArchModel {
        cabinets: vec![
            base(0, 1.2, 0.0, 0.0, 0.6),
            base(1, 1.8, 0.0, 0.0, 0.6),
            base(2, 0.45, 0.0, 0.0, 0.9),
            base(3, 0.0, 1.8, 270.0, 0.6),
            base(4, 0.0, 1.2, 270.0, 0.6),
        ],
        ..Default::default()
    }
}

fn hit(s: &Scene3d, x: f64, y: f64, z: f64) -> Option<(String, String)> {
    s.probe(Vec3::new(x, y, z)).map(|h| (h.id, h.part))
}

fn parts<'a>(s: &'a Scene3d, part: &str, id: u32) -> Vec<&'a Mesh> {
    s.meshes.iter().filter(|m| part_of(m) == part && m.object == ArchId(id)).collect()
}

fn signature(s: &Scene3d) -> (String, Vec<(String, u32)>) {
    (s.digest(), s.meshes.iter().map(|m| (m.material.clone(), m.object.0)).collect())
}

fn same(a: &Mesh, b: &Mesh) -> bool {
    a.positions == b.positions && a.normals == b.normals && a.indices == b.indices
}

#[test]
fn one_counter_mesh_for_a_merged_corner() {
    for (m, owner) in [(blind(), 0), (lazy(), 2)] {
        let s = Scene3d::build(&m);
        let counters: Vec<&Mesh> = s.meshes.iter().filter(|m| part_of(m) == "counter").collect();
        assert_eq!(counters.len(), 1, "one counter mesh for both runs");
        assert_eq!(counters[0].object, ArchId(owner), "built by the lowest run owner");
        let want = Some((format!("cb{owner}"), "counter".to_string()));
        // The counter's top is 0.876 + 0.038 above the floor: probe inside the slab on both legs.
        assert_eq!(hit(&s, 1.0, 0.3, 0.9), want, "south leg");
        assert_eq!(hit(&s, 0.3, 1.6, 0.9), want, "west leg");
        assert_eq!(hit(&s, 0.3, 0.3, 0.9), want, "the corner");
    }
    // With the corner switched off each run has its own counter.
    let mut m = blind();
    m.cabinets[0].corner = CornerPref::None;
    let s = Scene3d::build(&m);
    let owners: Vec<ArchId> = s.meshes.iter().filter(|m| part_of(m) == "counter").map(|m| m.object).collect();
    assert_eq!(owners, vec![ArchId(0), ArchId(3)]);
}

#[test]
fn lazy_susan_carcass_fills_the_l_and_leaves_the_notch() {
    let s = Scene3d::build(&lazy());
    let me = Some(("cb2".to_string(), "cabinet".to_string()));
    assert_eq!(hit(&s, 0.3, 0.75, 0.5), me, "inside the L, beside the notch");
    assert_eq!(hit(&s, 0.3, 0.3, 0.5), me);
    assert_eq!(hit(&s, 0.75, 0.3, 0.5), me, "the strip along the back wall");
    assert_eq!(hit(&s, 0.75, 0.75, 0.5), None, "the notch is hollow");
    // The counter follows the L: over the corner square, but not over the notch.
    assert_eq!(hit(&s, 0.3, 0.75, 0.9).map(|h| h.1).as_deref(), Some("counter"));
    assert_eq!(hit(&s, 0.75, 0.75, 0.9), None);
    // One carcass mesh and one fronts mesh, doors on the two notch faces (no pulls).
    assert_eq!(parts(&s, "cabinet", 2).iter().filter(|m| m.material == "cabinet-box").count(), 1);
    let fronts: Vec<&Mesh> = parts(&s, "cabinet", 2).into_iter().filter(|m| m.material == "cabinet-front").collect();
    assert_eq!(fronts.len(), 1);
    // The doors stand on y = 0.6 (0.581..0.6, over x 0.9..0.6) and x = 0.6 (0.581..0.6, over y 0.6..0.9).
    assert_eq!(hit(&s, 0.75, 0.59, 0.5), me);
    assert_eq!(hit(&s, 0.59, 0.75, 0.5), me);
    assert!(fronts[0].positions.iter().all(|p| p[0].is_finite() && p[1].is_finite() && p[2].is_finite()));
}

#[test]
fn blind_fronts_leave_a_false_panel() {
    let s = Scene3d::build(&blind());
    let front = |s: &Scene3d| parts(s, "cabinet", 0).into_iter().find(|m| m.material == "cabinet-front").cloned();
    let f = front(&s);
    let f = f.as_ref();
    let has_x = |m: &Mesh, x: f32| m.positions.iter().any(|p| (p[0] - x).abs() < 1e-5);
    // The false panel covers the blind part x 0.003..0.597, the doors the rest from 0.603.
    assert!(f.is_some_and(|m| has_x(m, 0.597) && has_x(m, 0.603) && has_x(m, 0.003) && has_x(m, 0.897)));
    // The panel is one slab from the toe to the top.
    for z in [0.15, 0.5, 0.85] {
        assert_eq!(hit(&s, 0.3, 0.59, z), Some(("cb0".to_string(), "cabinet".to_string())), "z {z}");
    }
    // The same cabinet without a corner has fronts over its whole width and no seam there.
    let mut plain = blind();
    plain.cabinets[0].corner = CornerPref::None;
    let p = Scene3d::build(&plain);
    let g = front(&p);
    assert!(g.is_some_and(|m| !has_x(&m, 0.597) && !has_x(&m, 0.603)));
}

#[test]
fn unmerged_runs_byte_identical() {
    // Two runs that do not meet: nothing is a corner, so the scene is the one corners-off gives.
    let apart = |pref: CornerPref| {
        let mut m = ArchModel {
            cabinets: vec![base(0, 0.45, 0.0, 0.0, 0.9), base(1, 1.2, 0.0, 0.0, 0.6), base(2, 0.0, 2.5, 270.0, 0.6), base(3, 0.0, 1.9, 270.0, 0.6)],
            ..Default::default()
        };
        for c in &mut m.cabinets {
            c.corner = pref;
        }
        m
    };
    let (a, b) = (Scene3d::build(&apart(CornerPref::Auto)), Scene3d::build(&apart(CornerPref::None)));
    assert_eq!(a.meshes.len(), b.meshes.len());
    assert!(a.meshes.iter().zip(&b.meshes).all(|(x, y)| x.material == y.material && x.object == y.object && same(x, y)));
    // A switched-off corner leaves each run's counter exactly as the run alone builds it.
    let mut off = blind();
    off.cabinets[0].corner = CornerPref::None;
    let both = Scene3d::build(&off);
    let south = Scene3d::build(&ArchModel { cabinets: off.cabinets[..3].to_vec(), ..Default::default() });
    let west = Scene3d::build(&ArchModel { cabinets: off.cabinets[3..].to_vec(), ..Default::default() });
    for (alone, id) in [(&south, 0), (&west, 3)] {
        let (x, y) = (parts(alone, "counter", id), parts(&both, "counter", id));
        assert!(x.len() == 1 && y.len() == 1 && same(x[0], y[0]), "counter of cb{id}");
    }
    // And every cabinet mesh of cb1 and cb2 (no corner anywhere near) is untouched by the other run.
    for id in [1, 2] {
        let (x, y) = (parts(&south, "cabinet", id), parts(&both, "cabinet", id));
        assert!(x.len() == y.len() && x.iter().zip(&y).all(|(p, q)| same(p, q)), "cb{id}");
    }
}

#[test]
fn cache_equals_full_after_corner_edits() {
    let mut m = lazy();
    let (mut cache, _) = SceneCache::new(&m);
    assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)));
    let edits: Vec<(&str, Box<dyn Fn(&mut ArchModel)>)> = vec![
        ("diagonal", Box::new(|m: &mut ArchModel| m.cabinets[2].corner = CornerPref::Diagonal)),
        ("blind", Box::new(|m: &mut ArchModel| m.cabinets[2].corner = CornerPref::Blind)),
        ("none", Box::new(|m: &mut ArchModel| m.cabinets[2].corner = CornerPref::None)),
        ("auto", Box::new(|m: &mut ArchModel| m.cabinets[2].corner = CornerPref::Auto)),
        ("move the west run away", Box::new(|m: &mut ArchModel| m.cabinets[4].position.y = 3.0)),
        ("bring it back", Box::new(|m: &mut ArchModel| m.cabinets[4].position.y = 1.2)),
        ("deepen the partner", Box::new(|m: &mut ArchModel| m.cabinets[4].depth = 0.7)),
        ("thicken the counter", Box::new(|m: &mut ArchModel| m.cabinets.iter_mut().for_each(|c| c.counter_thickness = 0.05))),
        ("remove the corner cabinet", Box::new(|m: &mut ArchModel| drop(m.cabinets.remove(2)))),
    ];
    for (what, edit) in edits {
        edit(&mut m);
        let stats = cache.update(&m);
        assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)), "{what}: {stats:?}");
        let cached: Vec<_> = cache.scene().meshes.iter().map(|x| (x.positions.clone(), x.indices.clone())).collect();
        let full: Vec<_> = Scene3d::build(&m).meshes.iter().map(|x| (x.positions.clone(), x.indices.clone())).collect();
        assert_eq!(cached, full, "{what}");
    }
}
