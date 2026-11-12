//! Room-type finishes (phase 18): the floor, wall and ceiling finishes a room takes from its type.
//!
//! The template lives in code; rooms store no copy. The effective finish is derived at every read,
//! so a room takes its type's finishes when detected or retyped, and a painter's edit
//! ([`ArchModel::paints`]) always overrides it.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Finish, RoomType, Side, Surface};

/// The finishes of one room type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomFinishes {
    /// The floor finish.
    pub floor: Finish,
    /// The finish of the wall sides facing the room.
    pub wall: Finish,
    /// The ceiling finish.
    pub ceiling: Finish,
}

/// The template of room type `t`: drywall walls and plaster ceiling everywhere; the floor by type.
pub fn template(t: RoomType) -> RoomFinishes {
    let floor = match t {
        RoomType::Living | RoomType::Dining | RoomType::Other | RoomType::Deck => Finish::Wood,
        RoomType::Bedroom | RoomType::Closet | RoomType::Family | RoomType::OpenBelow => Finish::Carpet,
        RoomType::Kitchen | RoomType::Bath | RoomType::Laundry | RoomType::Utility | RoomType::Entry => Finish::Tile,
        RoomType::Garage | RoomType::Porch => Finish::Concrete,
    };
    RoomFinishes { floor, wall: Finish::Drywall, ceiling: Finish::Plaster }
}

/// The finish `s` shows before any paint, if it has one: the wall's own finish on its right
/// (outer) side, else the template of the room a wall side faces; a room floor's own finish if it
/// fits floors, else the template; the ceiling template; a roof's covering. Anything else, a
/// missing object and a wall side facing no room: `None`.
pub fn unpainted_finish(model: &ArchModel, s: &Surface) -> Option<Finish> {
    let room_type = |id| model.room(id).map(|r| r.room_type);
    match s {
        Surface::WallSide { wall, side } => {
            let w = model.wall(*wall)?;
            if *side == Side::Right
                && let Some(f) = w.finish.filter(|f| f.for_walls())
            {
                return Some(f);
            }
            let room = crate::paint::wall_side_room(model, w, *side)?;
            room_type(room).map(|t| template(t).wall)
        }
        Surface::RoomFloor(r) => {
            let room = model.room(*r)?;
            Some(room.floor_finish.filter(|f| f.for_floors()).unwrap_or(template(room.room_type).floor))
        }
        Surface::RoomCeiling(r) => room_type(*r).map(|t| template(t).ceiling),
        Surface::Roof(r) => model.roof(*r).and_then(|r| r.covering),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::arch::ArchId;
    use cadcraft_doc::{Room, Wall};
    use cadcraft_geom::Vec2;

    /// A wall along y = 0 (x 0..4) with a room above it: the room is on the wall's left side.
    fn boxed(t: RoomType) -> (ArchModel, ArchId, ArchId) {
        let mut m = ArchModel::default();
        let w = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        let ring = vec![Vec2::new(0.0, 0.1), Vec2::new(4.0, 0.1), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)];
        let r = m.push_room(Room { polygon: ring, room_type: t, ..Default::default() });
        (m, w, r)
    }

    #[test]
    fn template_matches_the_brief() {
        use RoomType::*;
        for t in [Living, Bedroom, Kitchen, Bath, Closet, Utility, Dining, Family, Entry, Laundry, Garage, Porch, Deck, OpenBelow, Other] {
            let f = template(t);
            assert_eq!((f.wall, f.ceiling), (Finish::Drywall, Finish::Plaster), "{t:?}");
        }
        for (t, floor) in [
            (Living, Finish::Wood),
            (Dining, Finish::Wood),
            (Other, Finish::Wood),
            (Deck, Finish::Wood),
            (Bedroom, Finish::Carpet),
            (Closet, Finish::Carpet),
            (Family, Finish::Carpet),
            (OpenBelow, Finish::Carpet),
            (Kitchen, Finish::Tile),
            (Bath, Finish::Tile),
            (Laundry, Finish::Tile),
            (Utility, Finish::Tile),
            (Entry, Finish::Tile),
            (Garage, Finish::Concrete),
            (Porch, Finish::Concrete),
        ] {
            assert_eq!(template(t).floor, floor, "{t:?}");
        }
    }

    #[test]
    fn unpainted_finish_precedence() {
        let (mut m, w, r) = boxed(RoomType::Bath);
        let left = Surface::WallSide { wall: w, side: Side::Left };
        let right = Surface::WallSide { wall: w, side: Side::Right };
        assert_eq!(unpainted_finish(&m, &left), Some(Finish::Drywall));
        assert_eq!(unpainted_finish(&m, &right), None, "faces no room");
        assert_eq!(unpainted_finish(&m, &Surface::RoomFloor(r)), Some(Finish::Tile));
        assert_eq!(unpainted_finish(&m, &Surface::RoomCeiling(r)), Some(Finish::Plaster));
        // The wall's own finish wins on its right side only.
        m.walls[0].finish = Some(Finish::Brick);
        assert_eq!(unpainted_finish(&m, &right), Some(Finish::Brick));
        assert_eq!(unpainted_finish(&m, &left), Some(Finish::Drywall));
        // An explicit floor finish wins; one that does not fit floors falls back to the template.
        m.rooms[0].floor_finish = Some(Finish::Carpet);
        assert_eq!(unpainted_finish(&m, &Surface::RoomFloor(r)), Some(Finish::Carpet));
        m.rooms[0].floor_finish = Some(Finish::Brick);
        assert_eq!(unpainted_finish(&m, &Surface::RoomFloor(r)), Some(Finish::Tile));
        assert_eq!(unpainted_finish(&m, &Surface::Fixture(ArchId(7))), None);
    }

    #[test]
    fn wall_side_outside_every_room_has_no_template() {
        let mut m = ArchModel::default();
        let w = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        for side in [Side::Left, Side::Right] {
            assert_eq!(unpainted_finish(&m, &Surface::WallSide { wall: w, side }), None);
        }
    }

    #[test]
    fn hostile_rooms_fall_back_to_none() {
        let (mut m, w, r) = boxed(RoomType::Kitchen);
        m.rooms[0].polygon = vec![Vec2::new(f64::NAN, 0.0), Vec2::new(1.0, f64::INFINITY)];
        assert_eq!(unpainted_finish(&m, &Surface::WallSide { wall: w, side: Side::Left }), None);
        assert_eq!(unpainted_finish(&m, &Surface::RoomFloor(ArchId(999))), None);
        assert_eq!(unpainted_finish(&m, &Surface::RoomCeiling(ArchId(999))), None);
        assert_eq!(unpainted_finish(&m, &Surface::WallSide { wall: ArchId(999), side: Side::Left }), None);
        assert_eq!(unpainted_finish(&m, &Surface::RoomFloor(r)), Some(Finish::Tile));
    }
}
