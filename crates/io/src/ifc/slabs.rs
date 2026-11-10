//! Floors, decks and ceilings (phase 12).

use super::Ctx;
use super::guid::global_id;
use crate::step::P;
use cadcraft_doc::arch::ArchId;

/// One `IfcSlab` per room floor (`.FLOOR.`, key `floor/<room n>`) and per deck (`ObjectType`
/// `'DECK'`, `.USERDEFINED.`, key `deck/<room n>`), and one `IfcCovering` (`.CEILING.`, key
/// `ceiling/<room n>`) per ceiling, each with its body from `ctx.pieces` and contained in its
/// room's level storey. Floors first, then decks, then ceilings, each in id order.
pub(super) fn emit(ctx: &mut Ctx) {
    for id in ctx.pieces.ids("floor") {
        let meshes = ctx.pieces.get("floor", id).to_vec();
        let n = id.0;
        let slot = room_slot(ctx, id);
        ctx.element("IfcSlab", &format!("floor/{n}"), &format!("floor-r{n}"), slot, &meshes, vec![P::Null, P::Enum("FLOOR")]);
    }
    for id in ctx.pieces.ids("deck") {
        let meshes = ctx.pieces.get("deck", id).to_vec();
        let n = id.0;
        let slot = room_slot(ctx, id);
        let placement = ctx.placement_in_storey(slot);
        let z0 = ctx.storeys.get(slot).map_or(0.0, |s| s.elevation);
        let body = ctx.body(&meshes, z0).map_or(P::Null, P::Ref);
        let params = vec![
            P::str(global_id(&format!("deck/{n}"))),
            P::Null,
            P::str(&format!("deck-r{n}")),
            P::Null,
            P::str("DECK"),
            P::Ref(placement),
            body,
            P::Null,
            P::Enum("USERDEFINED"),
        ];
        let r = ctx.w.add("IfcSlab", params);
        ctx.contain(slot, r);
    }
    for id in ctx.pieces.ids("slab") {
        let meshes = ctx.pieces.get("slab", id).to_vec();
        let n = id.0;
        let slot = room_slot(ctx, id);
        ctx.element("IfcSlab", &format!("slab/{n}"), &format!("slab-r{n}"), slot, &meshes, vec![P::Null, P::Enum("BASESLAB")]);
    }
    // Stack layers (`ceiling-layer` meshes) are part of the room's `ceiling` kind, so they join its body.
    for id in ctx.pieces.ids("ceiling") {
        let meshes = ctx.pieces.get("ceiling", id).to_vec();
        let n = id.0;
        let slot = room_slot(ctx, id);
        ctx.element("IfcCovering", &format!("ceiling/{n}"), &format!("ceiling-r{n}"), slot, &meshes, vec![P::Null, P::Enum("CEILING")]);
    }
}

/// The storey slot of room `id`'s level (the last storey when the room is missing).
fn room_slot(ctx: &Ctx, id: ArchId) -> usize {
    match ctx.model.rooms.iter().find(|r| r.id == id) {
        Some(room) => ctx.storey_for(room.level),
        None => ctx.storeys.len().saturating_sub(1),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{IfcOptions, write_model};
    use cadcraft_arch3d::Scene3d;
    use cadcraft_doc::arch::{ArchModel, Wall};
    use cadcraft_geom::Vec2;

    fn model() -> ArchModel {
        let mut m = ArchModel::default();
        let c = [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)];
        for i in 0..4 {
            let (a, b) = (c[i], c[(i + 1) % 4]);
            m.push_wall(Wall { start: Vec2::new(a.0, a.1), end: Vec2::new(b.0, b.1), thickness: 0.2, height: 2.4, ..Wall::default() });
        }
        m
    }

    fn export(m: &ArchModel) -> String {
        write_model(m, &Scene3d::build(m), &IfcOptions::default())
    }

    #[test]
    fn floors_are_floor_slabs_in_their_rooms_storey() {
        let mut m = model();
        arch_rooms(&mut m);
        let text = export(&m);
        assert!(text.matches("=IFCSLAB(").count() >= 1, "{text}");
        assert!(text.contains(",'floor-r"), "{text}");
        assert!(text.contains(",$,.FLOOR.);"), "{text}");
    }

    #[test]
    fn ceilings_are_ceiling_coverings() {
        let mut m = model();
        arch_rooms(&mut m);
        let text = export(&m);
        assert!(text.matches("=IFCCOVERING(").count() >= 1, "{text}");
        assert!(text.contains(",'ceiling-r"), "{text}");
        assert!(text.contains(",$,.CEILING.);"), "{text}");
    }

    #[test]
    fn a_deck_is_a_userdefined_slab() {
        let mut m = model();
        arch_rooms(&mut m);
        for r in &mut m.rooms {
            r.room_type = cadcraft_doc::arch::RoomType::Deck;
            r.deck_height = Some(0.5);
        }
        let text = export(&m);
        assert!(text.contains(",'deck-r"), "{text}");
        assert!(text.contains(",'DECK',#"), "{text}");
        assert!(text.contains(",$,.USERDEFINED.);"), "{text}");
    }

    fn arch_rooms(m: &mut ArchModel) {
        let poly = vec![Vec2::new(0.1, 0.1), Vec2::new(3.9, 0.1), Vec2::new(3.9, 2.9), Vec2::new(0.1, 2.9)];
        m.push_room(cadcraft_doc::arch::Room { polygon: poly, ..cadcraft_doc::arch::Room::default() });
    }
}
