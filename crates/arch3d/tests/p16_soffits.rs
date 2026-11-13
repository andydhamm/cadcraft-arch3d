//! Phase 16 soffits (interior boxes hung from the ceiling; owner: p16-soffit-meshes). The
//! soffit is a closed `soffit-box` (or `soffit-box:<material key>`) under each soffit's ring,
//! from the finished ceiling down to the soffit's bottom; the roof's own soffit (material
//! `soffit`) stays part `roof`. The numbers are the house's: NE ceiling 2.4, soffit ring
//! (7, 7.3)…(9, 7.9), bottom 2.1.

use cadcraft_arch::ceiling::stack;
use cadcraft_arch3d::hlr::OrthoView;
use cadcraft_arch3d::probe::part_of;
use cadcraft_arch3d::section::{Plane, section_cut};
use cadcraft_arch3d::{Mesh, Scene3d, SceneCache};
use cadcraft_doc::arch::{ArchKind, Level, Roof, RoofDirective, Room, Soffit};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::{Vec2, Vec3};

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
    vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
}

fn wall(a: (f64, f64), b: (f64, f64), thickness: f64, exterior: bool, level: u32) -> Wall {
    Wall { start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness, height: 2.4, exterior, base_level: level, ..Default::default() }
}

fn ring(m: &mut ArchModel, x0: f64, y0: f64, x1: f64, y1: f64, level: u32) {
    for (a, b) in [((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))] {
        m.push_wall(wall(a, b, 0.2, true, level));
    }
}

/// The phase-1 house: ring w0 (south) w1 (east) w2 (north) w3 (west), interior walls w4 and w5,
/// rooms S, NW and NE on level 0 (at 0, ceiling 2.4, structure 0.3); NE (rooms[2]) has a
/// 0.5 m square hole. No roof.
fn house() -> ArchModel {
    let mut m = ArchModel::default();
    ring(&mut m, 0.0, 0.0, 10.0, 8.0, 0);
    m.push_wall(wall((0.0, 3.0), (10.0, 3.0), 0.1, false, 0));
    m.push_wall(wall((6.0, 3.0), (6.0, 8.0), 0.1, false, 0));
    for poly in [rect(0.1, 0.1, 9.9, 2.95), rect(0.1, 3.05, 5.95, 7.9), rect(6.05, 3.05, 9.9, 7.9)] {
        m.push_room(Room { level: 0, polygon: poly, ..Default::default() });
    }
    m.rooms[2].holes = vec![rect(8.0, 4.0, 8.5, 4.5)];
    m
}

/// The test soffit: id `sf0`, under the NE room, ring (7, 7.3)…(9, 7.9), bottom 2.1.
fn test_soffit(bottom: Option<f64>) -> Soffit {
    Soffit { level: 0, polygon: rect(7.0, 7.3, 9.0, 7.9), bottom, ..Default::default() }
}

fn hit(s: &Scene3d, x: f64, y: f64, z: f64) -> Option<(String, String)> {
    s.probe(Vec3::new(x, y, z)).map(|h| (h.id, h.part))
}

fn part_meshes<'a>(s: &'a Scene3d, part: &str) -> Vec<&'a Mesh> {
    s.meshes.iter().filter(|m| part_of(m) == part).collect()
}

/// What a cached scene must match: the digest, every mesh's material and owner in order, and the
/// counts.
fn signature(s: &Scene3d) -> (String, Vec<(String, u32)>, [usize; 4]) {
    (s.digest(), s.meshes.iter().map(|m| (m.material.clone(), m.object.0)).collect(), [s.floors, s.ceilings, s.roofs, s.decks])
}

#[test]
fn soffit_box_probes() {
    let mut m = house();
    m.push_soffit(test_soffit(Some(2.1)));
    let s = Scene3d::build(&m);
    let box_part = Some(("sf0".to_string(), "soffit".to_string()));
    assert_eq!(hit(&s, 8.0, 7.6, 2.2), box_part, "inside the box");
    assert_eq!(hit(&s, 8.0, 7.2, 2.2), None, "below the ring's bottom edge");
    assert_eq!(hit(&s, 8.0, 7.6, 2.05), None, "below the box");
    assert_eq!(hit(&s, 8.0, 7.6, 2.45), None, "above the box");
    let meshes = part_meshes(&s, "soffit");
    assert_eq!(meshes.len(), 1, "one mesh per soffit");
    let mesh = &meshes[0];
    assert!(mesh.is_manifold(), "un-manifold box: {}", mesh.material);
    assert_eq!((mesh.object, mesh.level_elevation), (ArchId(0), 0.0));
    // The flat NE ceiling is the 20 mm slab at 2.38..2.4 (rooms build before soffits); the top
    // is read from the box, not probed, so that the slab does not shadow it.
    assert!((mesh.bounds().max[2] - 2.4).abs() < 1e-6, "box top at the finished ceiling");
}

#[test]
fn soffit_top_follows_a_stack_change() {
    let make = |stack: Vec<cadcraft_doc::arch::CeilingLayer>| {
        let mut m = house();
        m.rooms[2].ceiling_stack = stack;
        m.push_soffit(test_soffit(Some(2.1)));
        Scene3d::build(&m)
    };
    let plain = make(Vec::new());
    let top0 = part_meshes(&plain, "soffit")[0].bounds().max[2];
    assert!((top0 - 2.4).abs() < 1e-6);
    let stack = vec![cadcraft_doc::arch::CeilingLayer::new("drywall", Some("gypsum"), 0.0127)];
    let stacked = make(stack.clone());
    let top1 = part_meshes(&stacked, "soffit")[0].bounds().max[2];
    let want = 2.4 - stack::thickness(&stack);
    assert!((top1 - want).abs() < 1e-6, "got {top1}, wanted {want}");
    // In both states 2.35 is in the box; with the stack 2.39 is above it (in the slab band 2.38..2.4).
    assert_eq!(hit(&plain, 8.0, 7.6, 2.35), Some(("sf0".to_string(), "soffit".to_string())));
    assert_eq!(hit(&stacked, 8.0, 7.6, 2.35), Some(("sf0".to_string(), "soffit".to_string())));
    assert_ne!(hit(&stacked, 8.0, 7.6, 2.39).map(|h| h.1).as_deref(), Some("soffit"));
}

#[test]
#[allow(clippy::unwrap_used)]
fn soffit_under_a_vault_has_a_sloped_top() {
    let mut m = house();
    m.walls[1].roof_directive = RoofDirective::Gable;
    m.walls[3].roof_directive = RoofDirective::Gable;
    let roof =
        cadcraft_arch::roof::build_roof(&m, 0, &Roof { pitch_rise: 6.0, pitch_run: 12.0, overhang: 0.5, thickness: 0.2, ..Default::default() })
            .unwrap();
    m.push_roof(roof);
    m.rooms[2].flat_ceiling = false;
    m.push_soffit(test_soffit(Some(2.1)));
    let s = Scene3d::build(&m);
    assert_eq!(hit(&s, 8.0, 7.6, 2.6), Some(("sf0".to_string(), "soffit".to_string())), "top 2.65");
    assert_eq!(hit(&s, 8.0, 7.35, 2.7), Some(("sf0".to_string(), "soffit".to_string())), "top 2.775");
    // Above the box at y = 7.6, z = 2.7 is inside the roof solid (2.65..2.85), not the box.
    assert_ne!(hit(&s, 8.0, 7.6, 2.7).map(|h| h.1).as_deref(), Some("soffit"));
    assert_eq!(part_meshes(&s, "soffit").len(), 1);
}

#[test]
fn soffit_material_key_and_materialdef() {
    let mut m = house();
    let mut s = test_soffit(Some(2.1));
    s.material = Some("brick".to_string());
    m.push_soffit(s);
    let scene = Scene3d::build(&m);
    let meshes = part_meshes(&scene, "soffit");
    assert_eq!(meshes.len(), 1);
    assert_eq!(meshes[0].material, "soffit-box:finish:brick");
    assert_eq!(part_of(&meshes[0]), "soffit");
    assert!(scene.materials.iter().any(|(k, d)| k == "soffit-box:finish:brick" && d.texture.as_deref() == Some("finish:brick")));
}

#[test]
#[allow(clippy::unwrap_used)]
fn roof_soffit_and_interior_soffit_parts_differ() {
    let gable = |with_interior: bool| {
        let mut m = house();
        m.walls[1].roof_directive = RoofDirective::Gable;
        m.walls[3].roof_directive = RoofDirective::Gable;
        let roof =
            cadcraft_arch::roof::build_roof(&m, 0, &Roof { pitch_rise: 6.0, pitch_run: 12.0, overhang: 0.5, thickness: 0.2, ..Default::default() })
                .unwrap();
        m.push_roof(roof);
        m.rooms[2].flat_ceiling = false;
        if with_interior {
            m.push_soffit(test_soffit(Some(2.1)));
        }
        Scene3d::build(&m)
    };
    let only_roof = gable(false);
    // The roof's own soffit stays part `roof` (material `soffit`): nothing has part `soffit` yet.
    assert!(part_meshes(&only_roof, "soffit").is_empty());
    assert!(only_roof.meshes.iter().any(|m| m.material == "soffit" && part_of(m) == "roof"));
    let with_both = gable(true);
    assert_eq!(part_meshes(&with_both, "soffit").len(), 1);
    assert!(with_both.meshes.iter().any(|m| m.material == "soffit" && part_of(m) == "roof"), "the roof soffit is still part roof");
}

#[test]
fn cache_equals_full_after_soffit_edits() {
    let mut m = house();
    m.push_soffit(test_soffit(Some(2.1)));
    let (mut cache, _) = SceneCache::new(&m);
    assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)));
    let edits: Vec<(&str, Box<dyn Fn(&mut ArchModel)>)> = vec![
        (
            "move the soffit ring",
            Box::new(|m: &mut ArchModel| {
                m.soffits[0].polygon = rect(7.0, 7.0, 9.0, 7.6);
            }),
        ),
        (
            "lower the bottom",
            Box::new(|m: &mut ArchModel| {
                m.soffits[0].bottom = Some(1.9);
                m.soffits[0].depth = None;
            }),
        ),
        (
            "hang by depth",
            Box::new(|m: &mut ArchModel| {
                m.soffits[0].bottom = None;
                m.soffits[0].depth = Some(0.25);
            }),
        ),
        (
            "change the ceiling stack",
            Box::new(|m: &mut ArchModel| {
                m.rooms[2].ceiling_stack = vec![cadcraft_doc::arch::CeilingLayer::new("drywall", Some("gypsum"), 0.0127)];
            }),
        ),
        (
            "paint the box",
            Box::new(|m: &mut ArchModel| {
                m.soffits[0].material = Some("brick".to_string());
            }),
        ),
        (
            "clear the soffits",
            Box::new(|m: &mut ArchModel| {
                m.soffits.clear();
            }),
        ),
    ];
    for (what, edit) in edits {
        edit(&mut m);
        let _stats = cache.update(&m);
        assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)), "{what}");
    }
    assert_eq!(cache.update(&m).rebuilt, 0, "without an edit nothing is rebuilt");
}

#[test]
fn multiple_soffits_build_separate_meshes() {
    let mut m = house();
    m.push_soffit(test_soffit(Some(2.1)));
    let s2 = Soffit { level: 0, polygon: rect(1.0, 1.0, 3.0, 2.0), bottom: Some(2.0), ..Default::default() };
    m.push_soffit(s2);
    let s = Scene3d::build(&m);
    let meshes = part_meshes(&s, "soffit");
    assert_eq!(meshes.len(), 2, "one mesh per soffit");
    // First soffit at (8, 7.6)
    assert_eq!(hit(&s, 8.0, 7.6, 2.2), Some(("sf0".to_string(), "soffit".to_string())));
    // Second soffit at (2, 1.5)
    assert_eq!(hit(&s, 2.0, 1.5, 2.05), Some(("sf1".to_string(), "soffit".to_string())));
    // Outside both
    assert_eq!(hit(&s, 5.0, 1.5, 2.05), None);
}

#[test]
fn soffit_with_unparseable_material_falls_back() {
    let mut m = house();
    let mut s = test_soffit(Some(2.1));
    s.material = Some("not-a-real-material".to_string());
    m.push_soffit(s);
    let scene = Scene3d::build(&m);
    let meshes = part_meshes(&scene, "soffit");
    assert_eq!(meshes.len(), 1);
    // MaterialRef::parse treats a bare string as an item reference.
    assert_eq!(meshes[0].material, "soffit-box:item:not-a-real-material");
    assert!(hit(&scene, 8.0, 7.6, 2.2) == Some(("sf0".to_string(), "soffit".to_string())));
}

#[test]
fn soffit_hanging_by_depth() {
    let mut m = house();
    let s = Soffit { level: 0, polygon: rect(7.0, 7.3, 9.0, 7.9), bottom: None, depth: Some(0.2), ..Default::default() };
    m.push_soffit(s);
    let scene = Scene3d::build(&m);
    let meshes = part_meshes(&scene, "soffit");
    assert_eq!(meshes.len(), 1);
    // Ceiling is 2.4, depth 0.2 -> bottom at 2.2
    assert!((meshes[0].bounds().min[2] - 2.2).abs() < 1e-6, "bottom at 2.2");
    assert!((meshes[0].bounds().max[2] - 2.4).abs() < 1e-6, "top at 2.4");
    assert_eq!(hit(&scene, 8.0, 7.6, 2.3), Some(("sf0".to_string(), "soffit".to_string())));
    assert_eq!(hit(&scene, 8.0, 7.6, 2.15), None, "below the depth-hung soffit");
}

#[test]
fn two_overlapping_soffits_both_build() {
    let mut m = house();
    m.push_soffit(test_soffit(Some(2.1)));
    // Overlaps the first soffit in plan
    let s2 = Soffit { level: 0, polygon: rect(8.0, 7.0, 10.0, 8.0), bottom: Some(2.0), ..Default::default() };
    m.push_soffit(s2);
    let s = Scene3d::build(&m);
    let meshes = part_meshes(&s, "soffit");
    assert_eq!(meshes.len(), 2, "each soffit builds its own mesh");
    // In the overlap region, both exist at different heights
    assert_eq!(hit(&s, 8.5, 7.5, 2.15), Some(("sf0".to_string(), "soffit".to_string())));
    assert_eq!(hit(&s, 8.5, 7.5, 2.05), Some(("sf1".to_string(), "soffit".to_string())));
}
