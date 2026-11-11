//! Deck structure (phase 17): ledgers, joists, rims and beams as `IfcMember`, posts as `IfcColumn`
//! and piers as `IfcFooting`, one per mesh, each in its deck room's level storey.

use super::Ctx;
use crate::step::P;
use cadcraft_arch3d::Mesh;
use cadcraft_doc::arch::ArchId;

/// For each deck room with structure pieces: one `IfcMember` (`.MEMBER.`) per `deck-member` mesh
/// (`deck-r<n>-ledger`, `-joist-<k>`, `-rim-<k>`, `-beam-<k>`, `k` counting per material from 0;
/// key `deck/<n>/member/<i>`, `i` the running index), one `IfcColumn` (`.COLUMN.`) per `deck-post`
/// mesh (`deck-r<n>-post-<k>`, key `deck/<n>/post/<k>`) and one `IfcFooting` (`.PAD_FOOTING.`) per
/// `deck-pier` mesh (`deck-r<n>-pier-<k>`, key `deck/<n>/pier/<k>`). Members first, then posts, then piers.
pub(super) fn emit(ctx: &mut Ctx) {
    for id in ctx.pieces.ids("deck-member") {
        let meshes = ctx.pieces.get("deck-member", id).to_vec();
        let n = id.0;
        let slot = room_slot(ctx, id);
        let mut per_material: Vec<(&str, usize)> = Vec::new();
        for (i, mesh) in meshes.iter().enumerate() {
            let material = mesh.material.as_str();
            let k = match per_material.iter_mut().find(|(m, _)| *m == material) {
                Some((_, c)) => {
                    let k = *c;
                    *c += 1;
                    k
                }
                None => {
                    per_material.push((material, 1));
                    0
                }
            };
            let name = match material {
                "deck-ledger" => format!("deck-r{n}-ledger"),
                "deck-joist" => format!("deck-r{n}-joist-{k}"),
                "deck-rim" => format!("deck-r{n}-rim-{k}"),
                "deck-beam" => format!("deck-r{n}-beam-{k}"),
                other => format!("deck-r{n}-{}-{k}", other.strip_prefix("deck-").unwrap_or(other)),
            };
            let one: [&Mesh; 1] = [mesh];
            ctx.element("IfcMember", &format!("deck/{n}/member/{i}"), &name, slot, &one, vec![P::Null, P::Enum("MEMBER")]);
        }
    }
    for (kind, entity, word, predefined) in [("deck-post", "IfcColumn", "post", "COLUMN"), ("deck-pier", "IfcFooting", "pier", "PAD_FOOTING")] {
        for id in ctx.pieces.ids(kind) {
            let meshes = ctx.pieces.get(kind, id).to_vec();
            let n = id.0;
            let slot = room_slot(ctx, id);
            for (k, mesh) in meshes.iter().enumerate() {
                let one: [&Mesh; 1] = [mesh];
                let tail = vec![P::Null, P::Enum(predefined)];
                ctx.element(entity, &format!("deck/{n}/{word}/{k}"), &format!("deck-r{n}-{word}-{k}"), slot, &one, tail);
            }
        }
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
    use cadcraft_doc::arch::{ArchModel, Room, RoomType};
    use cadcraft_geom::Vec2;

    fn deck_model() -> ArchModel {
        let mut m = ArchModel::default();
        let poly = vec![Vec2::new(2.0, 8.1), Vec2::new(6.0, 8.1), Vec2::new(6.0, 11.1), Vec2::new(2.0, 11.1)];
        m.push_room(Room { polygon: poly, room_type: RoomType::Deck, deck_height: Some(0.9), ..Room::default() });
        m
    }

    fn export(m: &ArchModel) -> String {
        write_model(m, &Scene3d::build(m), &IfcOptions::default())
    }

    #[test]
    fn deck_members_columns_and_piers() {
        let m = deck_model();
        let scene = Scene3d::build(&m);
        let count = |mat: &str| scene.meshes.iter().filter(|x| x.material == mat).count();
        let members = ["deck-ledger", "deck-joist", "deck-rim", "deck-beam"].iter().map(|k| count(k)).sum::<usize>();
        let (posts, piers) = (count("deck-post"), count("deck-footing"));
        let text = export(&m);
        assert!(members > 0 && posts > 0 && piers > 0, "the deck builds structure: {members} {posts} {piers}");
        assert_eq!(text.matches("=IFCMEMBER(").count(), members, "{text}");
        assert_eq!(text.matches("=IFCCOLUMN(").count(), posts, "{text}");
        assert_eq!(text.matches("=IFCFOOTING(").count(), piers, "{text}");
        assert_eq!(text.matches(",$,.MEMBER.);").count(), members);
        assert_eq!(text.matches(",$,.COLUMN.);").count(), posts);
        assert_eq!(text.matches(",$,.PAD_FOOTING.);").count(), piers);
        assert!(text.contains(",'deck-r0-joist-0',"), "{text}");
        assert!(text.contains(",'deck-r0-post-0',"), "{text}");
        assert!(text.contains(",'deck-r0-pier-0',"), "{text}");
    }

    #[test]
    fn deck_guids_are_stable() {
        let m = deck_model();
        let (a, b) = (export(&m), export(&m));
        assert_eq!(a, b);
        let lines: Vec<&str> = a.lines().filter(|l| l.contains("=IFCMEMBER(") || l.contains("=IFCCOLUMN(") || l.contains("=IFCFOOTING(")).collect();
        assert!(!lines.is_empty());
        let mut guids: Vec<&str> = lines.iter().filter_map(|l| l.split('\'').nth(1)).collect();
        let n = guids.len();
        guids.sort_unstable();
        guids.dedup();
        assert_eq!(guids.len(), n, "every deck piece has its own GlobalId");
    }
}
