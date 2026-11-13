//! Phase 16 scene plumbing: the ceiling pipeline, slab floors, and the soffit and foundation
//! sections leave today's scenes as they were, follow every edit incrementally, and count one
//! ceiling per room.

use std::collections::BTreeSet;

use cadcraft_arch3d::probe::part_of;
use cadcraft_arch3d::{Mesh, ObjectKey, ObjectKind, Scene3d, SceneCache, changed_objects};
use cadcraft_doc::arch::{Finish, Foundation, FoundationKind, LightFixture, MaterialRef, Paint, RoomType, Soffit, Surface, Tray};
use cadcraft_doc::{ArchId, ArchModel, Roof, RoofDirective, Room, Wall};
use cadcraft_geom::{Vec2, Vec3};

/// The phase-1 house: four exterior walls 2.4 m high on a 10 x 8 m rectangle, interior walls on
/// y = 3 and x = 6, three rooms (the north-east one with a hole), a deck, two recessed pot lights in
/// the north-west room, a painted south ceiling and a 6/12 gable roof with gables on x = 0 and x = 10.
fn house() -> ArchModel {
    let mut m = ArchModel::default();
    let wall = |a: (f64, f64), b: (f64, f64), thickness: f64, exterior: bool| Wall {
        start: Vec2::new(a.0, a.1),
        end: Vec2::new(b.0, b.1),
        thickness,
        height: 2.4,
        exterior,
        ..Default::default()
    };
    for (a, b) in [((0.0, 0.0), (10.0, 0.0)), ((10.0, 0.0), (10.0, 8.0)), ((10.0, 8.0), (0.0, 8.0)), ((0.0, 8.0), (0.0, 0.0))] {
        m.push_wall(wall(a, b, 0.2, true));
    }
    m.push_wall(wall((0.0, 3.0), (10.0, 3.0), 0.1, false));
    m.push_wall(wall((6.0, 3.0), (6.0, 8.0), 0.1, false));
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64| vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)];
    m.push_room(Room { name: "South".into(), polygon: rect(0.1, 0.1, 9.9, 2.95), ..Default::default() });
    m.push_room(Room { name: "North-west".into(), polygon: rect(0.1, 3.05, 5.95, 7.9), ..Default::default() });
    m.push_room(Room { name: "North-east".into(), polygon: rect(6.05, 3.05, 9.9, 7.9), holes: vec![rect(8.0, 4.0, 8.5, 4.5)], ..Default::default() });
    m.push_room(Room {
        name: "Deck".into(),
        polygon: rect(0.0, -3.0, 4.0, -0.1),
        room_type: RoomType::Deck,
        deck_height: Some(0.5),
        ..Default::default()
    });
    let pot = cadcraft_arch::light::find_type("Recessed pot light");
    for (n, (x, y)) in [(2.0, 5.0), (4.0, 6.0)].into_iter().enumerate() {
        let (id, p) = (ArchId(n as u32), Vec2::new(x, y));
        m.lights.push(
            pot.as_ref().map(|t| cadcraft_arch::light::fixture_from_type(id, t, p)).unwrap_or_else(|| LightFixture::new(id, "Recessed pot light", p)),
        );
    }
    m.next_light_id = 2;
    m.paints.insert(Surface::RoomCeiling(ArchId(0)).key(), Paint { material: Some(MaterialRef::Colour([200, 180, 160])), ..Default::default() });
    m.walls[1].roof_directive = RoofDirective::Gable;
    m.walls[3].roof_directive = RoofDirective::Gable;
    let r = Roof { pitch_rise: 6.0, pitch_run: 12.0, overhang: 0.5, thickness: 0.2, ..Roof::default() };
    let built = cadcraft_arch::roof::build_roof(&m, 0, &r).expect("the gable roof builds");
    m.push_roof(built);
    m
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
    vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
}

fn key(kind: ObjectKind, n: u32) -> ObjectKey {
    let mut k = ObjectKey { kind, ..Default::default() };
    k.id = ArchId(n);
    k
}

/// What a cached scene must match: the digest, every mesh's material and owner in order, and the
/// counts.
fn signature(s: &Scene3d) -> (String, Vec<(String, u32)>, [usize; 7]) {
    (
        s.digest(),
        s.meshes.iter().map(|m| (m.material.clone(), m.object.0)).collect(),
        [s.openings_built, s.floors, s.ceilings, s.roofs, s.decks, s.fixtures, s.lights.len()],
    )
}

#[test]
fn flat_house_digest_unchanged_by_the_pipeline() {
    // Measured on op/next before phase 16, re-recorded when phase 17 framed the deck and again
    // when phase 18 gave the room floors their room-type finish (the only change).
    let s = Scene3d::build(&house());
    assert_eq!(s.digest(), "2f858ffc6757ece4");
    let meshes: Vec<(&str, u32)> = s.meshes.iter().map(|m| (m.material.as_str(), m.object.0)).collect();
    let want = [
        ("wall", 0),
        ("wall", 1),
        ("wall", 2),
        ("wall", 3),
        ("wall", 4),
        ("wall", 5),
        ("floor-wood", 0),
        ("paint:room/r0/ceiling", 0),
        ("floor-wood", 1),
        ("ceiling", 1),
        ("floor-wood", 2),
        ("ceiling", 2),
        ("roof", 0),
        ("fascia", 0),
        ("soffit", 0),
        ("roof", 0),
        ("fascia", 0),
        ("soffit", 0),
        ("deck", 3),
        // Phase 17 frames the deck: its edge y = -0.1 lies on w0's outer face (the ledger), then 9
        // joists, rims on the other 3 edges, one beam at y = -2.7 and 3 posts on 3 piers.
        ("deck-ledger", 3),
        ("deck-joist", 3),
        ("deck-joist", 3),
        ("deck-joist", 3),
        ("deck-joist", 3),
        ("deck-joist", 3),
        ("deck-joist", 3),
        ("deck-joist", 3),
        ("deck-joist", 3),
        ("deck-joist", 3),
        ("deck-rim", 3),
        ("deck-rim", 3),
        ("deck-rim", 3),
        ("deck-beam", 3),
        ("deck-post", 3),
        ("deck-post", 3),
        ("deck-post", 3),
        ("deck-footing", 3),
        ("deck-footing", 3),
        ("deck-footing", 3),
        ("light-trim", 0),
        ("light-lens-lit", 0),
        ("light-trim", 1),
        ("light-lens-lit", 1),
    ];
    assert_eq!(meshes, want);
    assert_eq!((s.floors, s.decks, s.roofs), (4, 1, 1));
    // The north-west ceiling keeps its two pot-light openings and its hole-free outline.
    let nw = s.meshes.iter().find(|m| m.material == "ceiling" && m.object == ArchId(1)).expect("north-west ceiling");
    assert!(nw.is_manifold());
    assert_eq!(s.probe(Vec3::new(2.0, 5.0, 2.385)), None, "open under the pot light");
    assert_eq!(s.probe(Vec3::new(3.0, 4.0, 2.39)).map(|h| h.part), Some("ceiling".to_string()));
    // And an empty model still builds nothing.
    assert!(Scene3d::build(&ArchModel::default()).meshes.is_empty());
}

#[test]
fn slab_level_rooms_build_a_0p1_slab() {
    let mut m = house();
    m.rooms[1].floor_finish = Some(Finish::Wood);
    m.push_foundation(Foundation { kind: FoundationKind::Slab, ..Default::default() });
    let s = Scene3d::build(&m);
    let floors: Vec<&Mesh> = s.meshes.iter().filter(|x| matches!(part_of(x), "slab" | "floor")).collect();
    let materials: Vec<(&str, u32)> = floors.iter().map(|x| (x.material.as_str(), x.object.0)).collect();
    assert_eq!(materials, vec![("slab", 0), ("floor-wood", 1), ("slab", 2)], "the deck keeps its own floor");
    for f in &floors {
        let b = f.bounds();
        assert!((b.min[2] + 0.1).abs() < 1e-6 && b.max[2].abs() < 1e-6, "{} spans {}..{}", f.material, b.min[2], b.max[2]);
        assert_eq!(f.level_elevation, 0.0);
    }
    assert_eq!(s.floors, 4, "three slab floors and the deck");
    let hit = s.probe(Vec3::new(5.0, 1.0, -0.05)).expect("the slab");
    assert_eq!((hit.id.as_str(), hit.part.as_str()), ("r0", "slab"));
    assert_eq!(s.probe(Vec3::new(5.0, 1.0, -0.2)), None, "a 0.1 m slab, not the 0.3 m floor structure");
    // A stem foundation, or a slab under another level, leaves the floors as they were.
    for f in [Foundation::default(), Foundation { kind: FoundationKind::Slab, level: 1, ..Default::default() }] {
        let mut m = house();
        m.push_foundation(f);
        let s = Scene3d::build(&m);
        let floors: Vec<&Mesh> = s.meshes.iter().filter(|x| x.material.starts_with("floor")).collect();
        assert_eq!(floors.len(), 3);
        assert!(floors.iter().all(|x| (x.bounds().min[2] + 0.3).abs() < 1e-6));
    }
}

#[test]
fn changed_objects_reports_trays_soffits_foundations() {
    let mut prev = house();
    let steps: Vec<(&str, Box<dyn Fn(&mut ArchModel)>, BTreeSet<ObjectKey>)> = vec![
        (
            "add a soffit",
            Box::new(|m: &mut ArchModel| {
                m.push_soffit(Soffit { polygon: rect(7.0, 7.3, 9.0, 7.9), bottom: Some(2.1), ..Default::default() });
            }),
            BTreeSet::from([key(ObjectKind::Soffit, 0)]),
        ),
        ("lower the soffit", Box::new(|m: &mut ArchModel| m.soffits[0].bottom = Some(2.0)), BTreeSet::from([key(ObjectKind::Soffit, 0)])),
        (
            "add a foundation",
            Box::new(|m: &mut ArchModel| {
                m.push_foundation(Foundation::default());
            }),
            BTreeSet::from([key(ObjectKind::Foundation, 0)]),
        ),
        ("deepen the footing", Box::new(|m: &mut ArchModel| m.foundations[0].footing_depth = 1.5), BTreeSet::from([key(ObjectKind::Foundation, 0)])),
        (
            "add a tray",
            Box::new(|m: &mut ArchModel| {
                m.push_tray(Tray { room: ArchId(1), inset: 0.6, ..Default::default() });
            }),
            BTreeSet::from([key(ObjectKind::Room, 1)]),
        ),
        ("deepen the tray", Box::new(|m: &mut ArchModel| m.trays[0].depth = 0.4), BTreeSet::from([key(ObjectKind::Room, 1)])),
        (
            "move the tray to another room",
            Box::new(|m: &mut ArchModel| m.trays[0].room = ArchId(2)),
            BTreeSet::from([key(ObjectKind::Room, 1), key(ObjectKind::Room, 2)]),
        ),
        ("remove the tray", Box::new(|m: &mut ArchModel| m.trays.clear()), BTreeSet::from([key(ObjectKind::Room, 2)])),
        ("remove the soffit", Box::new(|m: &mut ArchModel| m.soffits.clear()), BTreeSet::from([key(ObjectKind::Soffit, 0)])),
        ("remove the foundation", Box::new(|m: &mut ArchModel| m.foundations.clear()), BTreeSet::from([key(ObjectKind::Foundation, 0)])),
    ];
    for (what, edit, want) in steps {
        let mut next = prev.clone();
        edit(&mut next);
        assert_eq!(changed_objects(&prev, &next), Some(want), "{what}");
        assert_eq!(changed_objects(&next, &next), Some(BTreeSet::new()), "{what}: no edit");
        prev = next;
    }
}

#[test]
fn cache_equals_full_with_trays_soffits_foundations_present() {
    let mut m = house();
    m.push_tray(Tray { room: ArchId(1), inset: 0.6, ..Default::default() });
    m.push_tray(Tray { room: ArchId(1), parent: Some(ArchId(0)), inset: 0.6, depth: 0.15, ..Default::default() });
    m.push_soffit(Soffit { polygon: rect(7.0, 7.3, 9.0, 7.9), bottom: Some(2.1), ..Default::default() });
    m.push_foundation(Foundation::default());
    let (mut cache, full) = SceneCache::new(&m);
    assert_eq!(signature(cache.scene()), signature(&Scene3d::build(&m)));
    assert!(!full.incremental);
    let edits: Vec<(&str, Box<dyn Fn(&mut ArchModel)>)> = vec![
        ("deepen a tray", Box::new(|m: &mut ArchModel| m.trays[1].depth = 0.2)),
        ("move a tray", Box::new(|m: &mut ArchModel| m.trays[0].room = ArchId(2))),
        ("lower the soffit", Box::new(|m: &mut ArchModel| m.soffits[0].bottom = Some(1.9))),
        (
            "add a soffit",
            Box::new(|m: &mut ArchModel| {
                m.push_soffit(Soffit { polygon: rect(1.0, 1.0, 2.0, 2.0), depth: Some(0.3), ..Default::default() });
            }),
        ),
        ("make the foundation a slab", Box::new(|m: &mut ArchModel| m.foundations[0].kind = FoundationKind::Slab)),
        ("deepen the footing", Box::new(|m: &mut ArchModel| m.foundations[0].footing_depth = 1.6)),
        ("move a light", Box::new(|m: &mut ArchModel| m.lights[0].position.x += 0.2)),
        ("move a wall", Box::new(|m: &mut ArchModel| m.walls[4].start.y += 0.1)),
        (
            "remove a soffit",
            Box::new(|m: &mut ArchModel| {
                m.soffits.remove(0);
            }),
        ),
        ("back to a stem foundation", Box::new(|m: &mut ArchModel| m.foundations[0].kind = FoundationKind::Stem)),
        ("remove the foundation", Box::new(|m: &mut ArchModel| m.foundations.clear())),
        ("remove the trays", Box::new(|m: &mut ArchModel| m.trays.clear())),
    ];
    for (what, edit) in edits {
        edit(&mut m);
        let stats = cache.update(&m);
        let want = Scene3d::build(&m);
        assert_eq!(signature(cache.scene()), signature(&want), "{what}: the cache is not the full build; {stats:?}");
        assert!(stats.incremental && stats.rebuilt <= stats.objects, "{what}: {stats:?}");
    }
    // Without an edit nothing is rebuilt.
    assert_eq!(cache.update(&m).rebuilt, 0);
}

#[test]
fn ceilings_count_one_per_room() {
    // South (painted), north-west and north-east; the deck has none.
    let mut m = house();
    assert_eq!(Scene3d::build(&m).ceilings, 3);
    m.push_tray(Tray { room: ArchId(1), inset: 0.6, ..Default::default() });
    m.push_tray(Tray { room: ArchId(1), parent: Some(ArchId(0)), inset: 0.6, depth: 0.15, ..Default::default() });
    assert_eq!(Scene3d::build(&m).ceilings, 3, "trays do not add ceilings");
    m.rooms[0].room_type = RoomType::Porch;
    assert_eq!(Scene3d::build(&m).ceilings, 2, "a porch has no ceiling");
}
