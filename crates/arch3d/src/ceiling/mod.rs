//! The ceiling pipeline (phase 16): a room's ceiling as [`Piece`]s, plan regions each under the
//! plane of its rough ceiling (the underside of the structure above).
//!
//! The level ceiling ([`base_pieces`]) follows the roof underside where the room vaults
//! ([`vault::apply`]) and rises into its trays ([`tray::apply`]); that is the rough ceiling. The
//! finish stack hangs below it ([`stack::drop`]), so the finished ceiling is the rough one moved
//! down by the stack. [`ceiling_meshes`] is what a room's part in the scene shows of it: the stack's
//! slabs and the trays' sides.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Level, Roof, Room, Tray};
use cadcraft_geom::Vec2;

use crate::floors;
use crate::mesh::Mesh;
use crate::planar::{Piece, PlaneZ};

pub(crate) mod stack;
pub(crate) mod tray;
pub(crate) mod vault;

/// Everything a room's ceiling is made from.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CeilingIn {
    pub room: Room,
    pub level: Level,
    /// The openings of the recessed fixtures in the ceiling.
    pub cuts: Vec<Vec<Vec2>>,
    /// The roofs on the room's level, in model order.
    pub roofs: Vec<Roof>,
    /// The room's trays, in model order.
    pub trays: Vec<Tray>,
}

/// The inputs of `room`'s ceiling on `level`, with the openings `cuts`.
pub(crate) fn ceiling_input(model: &ArchModel, room: &Room, level: &Level, cuts: Vec<Vec<Vec2>>) -> CeilingIn {
    CeilingIn {
        room: room.clone(),
        level: level.clone(),
        cuts,
        roofs: model.roofs.iter().filter(|r| r.level == room.level).cloned().collect(),
        trays: model.trays.iter().filter(|t| t.room == room.id && t.level == room.level).cloned().collect(),
    }
}

/// The level ceiling: the room's polygon, less its holes and the fixture openings, under the flat
/// ceiling height ([`floors::ceiling_top`]); empty for a room without a ceiling.
pub(crate) fn base_pieces(i: &CeilingIn) -> Vec<Piece> {
    let Some(top) = floors::ceiling_top(&i.room, &i.level) else {
        return Vec::new();
    };
    let holes = i.room.holes.iter().chain(&i.cuts).cloned().collect();
    vec![Piece::new(i.room.polygon.clone(), holes, PlaneZ::flat(top))]
}

/// The rough ceiling: the level ceiling, vaulted under the roof and raised into the trays.
pub(crate) fn rough_pieces(i: &CeilingIn) -> Vec<Piece> {
    tray::apply(i, vault::apply(i, base_pieces(i)))
}

/// The finished ceiling: the rough ceiling moved down by the finish stack.
#[allow(dead_code)] // the phase-16 feature packages hang soffits and items from it
pub(crate) fn finished_pieces(i: &CeilingIn) -> Vec<Piece> {
    let drop = stack::drop(i);
    rough_pieces(i).into_iter().map(|p| Piece { plane: p.plane.offset(-drop), ..p }).collect()
}

/// The meshes of the room's ceiling: the stack's slabs under the rough ceiling, then the trays'
/// sides.
pub(crate) fn ceiling_meshes(i: &CeilingIn) -> Vec<Mesh> {
    let mut out = stack::slabs(i, &rough_pieces(i));
    out.extend(tray::side_meshes(i, stack::drop(i)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{LightFixture, RoofPlane, RoomType};

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    /// A 6 x 4 m room with a hole, under a 2.4 m ceiling, with recessed pot lights at `pots`.
    fn room_with_pots(pots: &[(f64, f64)]) -> ArchModel {
        let mut m = ArchModel::default();
        m.rooms.push(Room { id: ArchId(3), polygon: rect(0.0, 0.0, 6.0, 4.0), holes: vec![rect(4.0, 1.0, 5.0, 2.0)], ..Default::default() });
        let t = cadcraft_arch::light::find_type("Recessed pot light");
        for (n, &(x, y)) in pots.iter().enumerate() {
            let (id, p) = (ArchId(n as u32), Vec2::new(x, y));
            m.lights.push(
                t.as_ref()
                    .map(|t| cadcraft_arch::light::fixture_from_type(id, t, p))
                    .unwrap_or_else(|| LightFixture::new(id, "Recessed pot light", p)),
            );
        }
        m
    }

    fn same(a: &Mesh, b: &Mesh) -> bool {
        a.positions == b.positions
            && a.normals == b.normals
            && a.uvs == b.uvs
            && a.indices == b.indices
            && a.material == b.material
            && a.object == b.object
            && a.level_elevation.to_bits() == b.level_elevation.to_bits()
    }

    #[test]
    fn flat_room_ceiling_equals_floors_ceiling_surface() {
        for pots in [&[][..], &[(1.0, 1.0), (2.5, 3.0)][..]] {
            let m = room_with_pots(pots);
            let (room, level) = (&m.rooms[0], &m.levels[0]);
            let cuts = crate::fixture::ceiling_cutouts(&m, room, level);
            assert_eq!(cuts.len(), pots.len(), "a cut per pot");
            let mut clone = room.clone();
            clone.holes.extend(cuts.iter().cloned());
            let want = floors::ceiling_surface(&clone, level, "ceiling").expect("today's ceiling");
            let got = ceiling_meshes(&ceiling_input(&m, room, level, cuts));
            assert_eq!(got.len(), 1);
            assert!(same(&got[0], &want), "{pots:?}");
            // Under an empty stack the finished ceiling is the rough one.
            let i = ceiling_input(&m, room, level, Vec::new());
            assert_eq!(finished_pieces(&i), rough_pieces(&i));
            assert_eq!(rough_pieces(&i), base_pieces(&i));
        }
        // A room with its own ceiling height.
        let mut m = room_with_pots(&[]);
        m.rooms[0].ceiling_height = Some(2.7);
        let (room, level) = (&m.rooms[0], &m.levels[0]);
        let want = floors::ceiling_surface(room, level, "ceiling").expect("ceiling");
        let got = ceiling_meshes(&ceiling_input(&m, room, level, Vec::new()));
        assert!(got.len() == 1 && same(&got[0], &want));
    }

    #[test]
    fn deck_and_porch_get_no_ceiling() {
        for kind in [RoomType::Deck, RoomType::Porch] {
            let mut m = room_with_pots(&[]);
            m.rooms[0].room_type = kind;
            let i = ceiling_input(&m, &m.rooms[0], &m.levels[0], Vec::new());
            assert!(base_pieces(&i).is_empty() && rough_pieces(&i).is_empty() && finished_pieces(&i).is_empty(), "{kind:?}");
            assert!(ceiling_meshes(&i).is_empty(), "{kind:?}");
        }
        // Nor does a room whose ceiling height is not a number.
        let mut m = room_with_pots(&[]);
        m.levels[0].ceiling_height = f64::NAN;
        assert!(ceiling_meshes(&ceiling_input(&m, &m.rooms[0], &m.levels[0], Vec::new())).is_empty());
    }

    #[test]
    fn ceiling_input_takes_the_roofs_of_its_level_and_the_trays_of_its_room() {
        let mut m = room_with_pots(&[]);
        m.rooms[0].level = 1;
        for level in [0, 1, 1] {
            let id = ArchId(m.next_roof_id);
            m.next_roof_id += 1;
            m.roofs.push(Roof { id, level, planes: vec![RoofPlane::default()], ..Default::default() });
        }
        m.push_tray(Tray { room: ArchId(3), level: 1, ..Default::default() });
        m.push_tray(Tray { room: ArchId(3), level: 0, ..Default::default() });
        m.push_tray(Tray { room: ArchId(4), level: 1, ..Default::default() });
        m.push_tray(Tray { room: ArchId(3), level: 1, depth: 0.15, ..Default::default() });
        let i = ceiling_input(&m, &m.rooms[0], &m.levels[0], Vec::new());
        assert_eq!(i.roofs.iter().map(|r| r.id).collect::<Vec<_>>(), vec![ArchId(1), ArchId(2)]);
        assert_eq!(i.trays.iter().map(|t| t.id).collect::<Vec<_>>(), vec![ArchId(0), ArchId(3)]);
    }
}
