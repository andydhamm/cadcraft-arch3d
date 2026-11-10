//! Phase 17 decks: the framed deck follows every edit incrementally exactly as a full build.

use cadcraft_arch3d::{Scene3d, SceneCache};
use cadcraft_doc::arch::RoomType;
use cadcraft_doc::{ArchId, ArchModel, Room, Wall};
use cadcraft_geom::Vec2;

fn house() -> ArchModel {
    let mut m = ArchModel::default();
    let walls = [((0.0, 0.0), (10.0, 0.0)), ((10.0, 0.0), (10.0, 8.0)), ((10.0, 8.0), (0.0, 8.0)), ((0.0, 8.0), (0.0, 0.0))];
    for (a, b) in walls {
        m.push_wall(Wall { start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness: 0.2, height: 2.4, exterior: true, ..Default::default() });
    }
    m.push_room(Room {
        polygon: vec![Vec2::new(2.0, 8.1), Vec2::new(6.0, 8.1), Vec2::new(6.0, 11.1), Vec2::new(2.0, 11.1)],
        room_type: RoomType::Deck,
        deck_height: Some(0.9),
        ..Default::default()
    });
    m
}

fn count(s: &Scene3d, material: &str) -> usize {
    s.meshes.iter().filter(|m| m.material == material).count()
}

#[test]
fn cache_equals_full_after_deck_edits() {
    let mut model = house();
    let (mut cache, _) = SceneCache::new(&model);
    assert_eq!(cache.scene().digest(), Scene3d::build(&model).digest());
    assert_eq!((count(cache.scene(), "deck-joist"), count(cache.scene(), "deck-post")), (9, 3));

    let edits: Vec<Box<dyn Fn(&mut ArchModel)>> = vec![
        // Turn the boards: the frame stays, the texture coordinates change.
        Box::new(|m| m.rooms[0].deck_boards = Some(90.0)),
        // Raise the deck.
        Box::new(|m| m.rooms[0].deck_height = Some(1.2)),
        // Widen it: more joists.
        Box::new(|m| m.rooms[0].polygon = vec![Vec2::new(2.0, 8.1), Vec2::new(8.0, 8.1), Vec2::new(8.0, 11.1), Vec2::new(2.0, 11.1)]),
        // Move the house wall the ledger sits on away: the deck is free-standing.
        Box::new(|m| {
            m.walls[2].start.y = 5.0;
            m.walls[2].end.y = 5.0;
        }),
        // Remove the height: a plain floor.
        Box::new(|m| m.rooms[0].deck_height = None),
        // Back to a deck with a hostile polygon.
        Box::new(|m| {
            m.rooms[0].deck_height = Some(0.9);
            m.rooms[0].polygon = vec![Vec2::new(f64::NAN, 0.0); 4];
        }),
    ];
    for (i, edit) in edits.iter().enumerate() {
        edit(&mut model);
        cache.update(&model);
        let full = Scene3d::build(&model);
        assert_eq!(cache.scene().digest(), full.digest(), "digest after edit {i}");
        let names = |s: &Scene3d| s.meshes.iter().map(|m| (m.material.clone(), m.object)).collect::<Vec<_>>();
        assert_eq!(names(cache.scene()), names(&full), "meshes after edit {i}");
    }
    // The last model has no deck mesh but is still a valid scene.
    assert_eq!(count(cache.scene(), "deck-joist"), 0);
    let _ = ArchId(0);
}

#[test]
fn a_framed_deck_stands_on_the_terrain() {
    let s = Scene3d::build(&house());
    assert_eq!(s.decks, 1);
    let post = s.probe(cadcraft_geom::Vec3::new(4.0, 10.8, 0.2)).map(|h| h.part);
    assert_eq!(post.as_deref(), Some("post"));
}
