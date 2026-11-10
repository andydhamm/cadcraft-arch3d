//! A deck's meshes: the deck-frame segment fills this in.

use cadcraft_arch::deck::DeckFrame;
use cadcraft_doc::arch::{Level, Room};

use crate::floors;
use crate::mesh::Mesh;

/// Every mesh of the deck room `room` on `level`, with its substructure `frame` when it has one.
/// For now: today's board slab, material "deck" when the deck has a positive finite height, else
/// "floor".
pub(crate) fn deck_meshes(room: &Room, level: &Level, _frame: Option<&DeckFrame>) -> Vec<Mesh> {
    let material = if room.deck_height.is_some_and(|h| h > 0.0 && h.is_finite()) { "deck" } else { "floor" };
    floors::floor_slab(room, level, material).into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::RoomType;
    use cadcraft_geom::Vec2;

    #[test]
    fn stub_matches_the_legacy_slab() {
        let polygon = vec![Vec2::new(2.0, 8.1), Vec2::new(6.0, 8.1), Vec2::new(6.0, 11.1), Vec2::new(2.0, 11.1)];
        let level = Level::default();
        let deck = Room { polygon: polygon.clone(), room_type: RoomType::Deck, deck_height: Some(0.9), ..Default::default() };
        let got = deck_meshes(&deck, &level, None);
        let want: Vec<Mesh> = floors::floor_slab(&deck, &level, "deck").into_iter().collect();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].positions, want[0].positions);
        assert_eq!(got[0].indices, want[0].indices);
        assert_eq!(got[0].material, "deck");
        let flat = Room { deck_height: None, ..deck };
        assert_eq!(deck_meshes(&flat, &level, None)[0].material, "floor");
    }
}
