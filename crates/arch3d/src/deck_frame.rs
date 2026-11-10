//! A deck's meshes: the boards, and the frame under them when the deck has one (phase 17).

use std::sync::OnceLock;

use cadcraft_arch::deck::{DECK_BOARD, DECK_PIER_DEPTH, DECK_PIER_DIAMETER, DeckFrame, Member};
use cadcraft_doc::arch::{Finish, Level, Room};
use cadcraft_geom::Vec2;

use crate::floors;
use crate::mesh::Mesh;
use crate::solid::{push_box, push_cylinder, push_member};

/// Metres one repeat of the Wood board texture covers along u and v.
fn wood_tile() -> [f64; 2] {
    static TILE: OnceLock<[f64; 2]> = OnceLock::new();
    *TILE.get_or_init(|| crate::texture::texture(Finish::Wood).tile)
}

/// `mesh` with texture coordinates for boards running along the unit vector `d`: face-aligned UVs
/// (`uv::face_uvs` at the Wood tile), then every vertex of a horizontal face takes
/// `u = p.d / tile_u`, `v = p.perp(d) / tile_v`. Side faces keep the face-aligned ones.
fn board_uvs(mesh: &Mesh, d: Vec2) -> Mesh {
    let tile = wood_tile();
    let mut out = crate::uv::face_uvs(mesh, tile);
    let d = if d.is_finite() && d.len() > 1e-12 { d.normalized() } else { Vec2::new(1.0, 0.0) };
    let n = d.perp();
    for ((p, normal), uv) in out.positions.iter().zip(&out.normals).zip(out.uvs.iter_mut()) {
        if f64::from(normal[2]).abs() > 0.999 {
            let q = Vec2::new(f64::from(p[0]), f64::from(p[1]));
            *uv = [(q.dot(d) / tile[0]) as f32, (q.dot(n) / tile[1]) as f32];
        }
    }
    out
}

/// A mesh of `material` for room `room` holding one `member`.
fn member_mesh(room: &Room, level: &Level, material: &str, m: &Member) -> Mesh {
    let mut mesh = Mesh::new(material, room.id);
    mesh.level_elevation = level.elevation;
    push_member(&mut mesh, m.a, m.b, m.width, m.bottom, m.top);
    mesh
}

/// Every mesh of the deck room `room` on `level`, with its substructure `frame` when it has one.
///
/// Without a frame: today's board slab (material "deck" when the deck has a positive finite height,
/// else "floor"), drawn as boards along x. With one, in this order: the boards ("deck", 25 mm), the
/// ledger ("deck-ledger"), a mesh per joist ("deck-joist"), rim ("deck-rim"), beam ("deck-beam"),
/// post ("deck-post") and the pier under each post ("deck-footing").
pub(crate) fn deck_meshes(room: &Room, level: &Level, frame: Option<&DeckFrame>) -> Vec<Mesh> {
    let Some(frame) = frame else {
        let material = if room.deck_height.is_some_and(|h| h > 0.0 && h.is_finite()) { "deck" } else { "floor" };
        return floors::floor_slab(room, level, material)
            .into_iter()
            .map(|m| if m.material == "deck" { board_uvs(&m, Vec2::new(1.0, 0.0)) } else { m })
            .collect();
    };
    let mut out = Vec::new();
    let mut boards = Mesh::new("deck", room.id);
    boards.level_elevation = level.elevation;
    boards.push_prism_with_holes(&room.polygon, &room.holes, frame.top - DECK_BOARD, frame.top);
    if !boards.positions.is_empty() {
        out.push(board_uvs(&boards, frame.board_dir));
    }
    if let Some(l) = &frame.ledger {
        out.push(member_mesh(room, level, "deck-ledger", l));
    }
    out.extend(frame.joists.iter().map(|m| member_mesh(room, level, "deck-joist", m)));
    out.extend(frame.rims.iter().map(|m| member_mesh(room, level, "deck-rim", m)));
    out.extend(frame.beams.iter().map(|m| member_mesh(room, level, "deck-beam", m)));
    for p in &frame.posts {
        let mut mesh = Mesh::new("deck-post", room.id);
        mesh.level_elevation = level.elevation;
        push_box(&mut mesh, p.at, p.axis, p.size, p.size, p.grade, p.top);
        out.push(mesh);
    }
    for p in &frame.posts {
        let mut mesh = Mesh::new("deck-footing", room.id);
        mesh.level_elevation = level.elevation;
        push_cylinder(&mut mesh, p.at, DECK_PIER_DIAMETER / 2.0, 16, p.grade - DECK_PIER_DEPTH, p.grade);
        out.push(mesh);
    }
    out.retain(|m| !m.positions.is_empty());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_arch::deck::frame;
    use cadcraft_doc::arch::{ArchModel, RoomType};
    use cadcraft_doc::{ArchId, Wall};

    fn house() -> ArchModel {
        let mut m = ArchModel::default();
        m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        m.push_wall(Wall::new(Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), 0.2));
        m.push_wall(Wall::new(Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0), 0.2));
        m.push_wall(Wall::new(Vec2::new(0.0, 8.0), Vec2::new(0.0, 0.0), 0.2));
        m
    }

    fn north_deck() -> Room {
        Room {
            id: ArchId(5),
            polygon: vec![Vec2::new(2.0, 8.1), Vec2::new(6.0, 8.1), Vec2::new(6.0, 11.1), Vec2::new(2.0, 11.1)],
            room_type: RoomType::Deck,
            deck_height: Some(0.9),
            ..Default::default()
        }
    }

    fn built(room: &Room) -> Vec<Mesh> {
        let m = house();
        let level = Level::default();
        let f = frame(&m, room, &|_| 0.0);
        deck_meshes(room, &level, f.as_ref())
    }

    fn inside(m: &Mesh, p: [f32; 3]) -> bool {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &m.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
        (0..3).all(|k| p[k] >= lo[k] && p[k] <= hi[k])
    }

    fn hit(meshes: &[Mesh], p: [f32; 3]) -> Option<&str> {
        meshes.iter().find(|m| m.material != "deck" && inside(m, p)).or_else(|| meshes.iter().find(|m| inside(m, p))).map(|m| m.material.as_str())
    }

    #[test]
    fn deck_meshes_probe_the_brief_points() {
        let ms = built(&north_deck());
        assert_eq!(hit(&ms, [4.0, 10.8, 0.2]), Some("deck-post"));
        assert_eq!(hit(&ms, [3.15, 10.8, 0.2]), None);
        assert_eq!(hit(&ms, [3.15, 10.8, 0.6]), Some("deck-beam"));
        assert_eq!(hit(&ms, [2.8, 9.5, 0.8]), Some("deck-joist"));
        assert_eq!(hit(&ms, [3.0, 9.5, 0.8]), None);
        assert_eq!(hit(&ms, [3.0, 9.5, 0.89]), Some("deck"));
    }

    #[test]
    fn one_mesh_per_member() {
        let ms = built(&north_deck());
        let count = |name: &str| ms.iter().filter(|m| m.material == name).count();
        assert_eq!(
            (
                count("deck"),
                count("deck-ledger"),
                count("deck-joist"),
                count("deck-rim"),
                count("deck-beam"),
                count("deck-post"),
                count("deck-footing")
            ),
            (1, 1, 9, 3, 1, 3, 3)
        );
        let order: Vec<&str> = ms.iter().map(|m| m.material.as_str()).collect();
        let mut sorted = order.clone();
        sorted.dedup();
        assert_eq!(sorted, ["deck", "deck-ledger", "deck-joist", "deck-rim", "deck-beam", "deck-post", "deck-footing"]);
        assert!(ms.iter().all(|m| m.object == ArchId(5) && m.is_manifold()));
    }

    #[test]
    fn boards_uv_follow_the_board_direction() {
        let top_face = |room: &Room| {
            let ms = built(room);
            let b = &ms[0];
            let mut seen = Vec::new();
            for (p, (n, uv)) in b.positions.iter().zip(b.normals.iter().zip(&b.uvs)) {
                if n[2] > 0.999 {
                    seen.push((p[0], p[1], uv[0], uv[1]));
                }
            }
            assert!(!seen.is_empty());
            seen
        };
        // Along x: u = x / 1.2, v = y / 0.84.
        let seen = top_face(&north_deck());
        for s in &seen {
            assert!((f64::from(s.2) - f64::from(s.0) / 1.2).abs() < 1e-4 && (f64::from(s.3) - f64::from(s.1) / 0.84).abs() < 1e-4);
        }
        // Boards at 90 degrees: u = y / 1.2, v = -x / 0.84.
        let seen = top_face(&Room { deck_boards: Some(90.0), ..north_deck() });
        for s in &seen {
            assert!((f64::from(s.2) - f64::from(s.1) / 1.2).abs() < 1e-4, "u {} y {}", s.2, s.1);
            assert!((f64::from(s.3) + f64::from(s.0) / 0.84).abs() < 1e-4, "v {} x {}", s.3, s.0);
        }
    }

    #[test]
    fn boards_keep_material_deck_and_part_deck() {
        let ms = built(&north_deck());
        assert_eq!(ms[0].material, "deck");
        assert_eq!(crate::probe::part_of(&ms[0]), "deck");
        assert!(inside(&ms[0], [3.0, 9.5, 0.89]) && !inside(&ms[0], [3.0, 9.5, 0.86]));
        assert!((f64::from(ms[0].positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max)) - 0.9).abs() < 1e-6);
    }

    #[test]
    fn deck_without_frame_keeps_the_legacy_slab() {
        let polygon = vec![Vec2::new(2.0, 8.1), Vec2::new(6.0, 8.1), Vec2::new(6.0, 11.1), Vec2::new(2.0, 11.1)];
        let level = Level::default();
        let deck = Room { polygon, room_type: RoomType::Deck, deck_height: Some(0.9), ..Default::default() };
        let got = deck_meshes(&deck, &level, None);
        let want: Vec<Mesh> = floors::floor_slab(&deck, &level, "deck").into_iter().collect();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].material, "deck");
        let mut a = got[0].positions.clone();
        let mut b = want[0].positions.clone();
        a.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        b.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        a.dedup();
        b.dedup();
        assert_eq!(a, b);
        // Top face UVs run along x: u = x / 1.2.
        for (p, (n, uv)) in got[0].positions.iter().zip(got[0].normals.iter().zip(&got[0].uvs)) {
            if n[2] > 0.999 {
                assert!((uv[0] - p[0] / 1.2).abs() < 1e-4);
            }
        }
        let flat = Room { deck_height: None, ..deck };
        assert_eq!(deck_meshes(&flat, &level, None)[0].material, "floor");
    }
}
