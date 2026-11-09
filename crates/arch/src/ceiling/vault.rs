//! Ceilings that follow the roof (phase 16): a room with `flat_ceiling` off takes the underside
//! of the roof planes over it (a cathedral or vaulted ceiling, re-pitched with the roof), and a
//! flat ceiling never rises above that underside.
//!
//! "Under a roof" is [`super::roof_underside_at`]: a plane of a roof whose `level` is the room's
//! level holds the point. Room and level edits that would put a flat ceiling above the underside
//! are refused ([`check_flat_ceiling`]); roof edits never are, so where one leaves a flat ceiling
//! above the underside, that ceiling follows the underside there ([`adjust`]) and the room carries
//! a warning ([`ceiling_warning`]).

use cadcraft_doc::arch::{ArchModel, Room, room_label};
use cadcraft_geom::Vec2;

use super::{flat_ceiling_z, lowest_underside_over, roof_underside_at};

/// How far a flat ceiling may sit above the roof underside before it counts as above it (m).
const ABOVE: f64 = 1e-9;

/// The rough ceiling of `room` at plan point `p`, given its flat rough ceiling `flat`
/// (absolute). A vaulted room (`flat_ceiling` off) takes the roof underside wherever a roof of
/// its level covers `p`, and `flat` elsewhere. A flat room keeps `flat`, except where the
/// underside is lower: a flat ceiling never rises above the roof.
pub fn adjust(model: &ArchModel, room: &Room, flat: f64, p: Vec2) -> f64 {
    let under = roof_underside_at(model, room.level, p);
    if !room.flat_ceiling {
        return under.unwrap_or(flat);
    }
    match under {
        Some(u) if u < flat => u,
        _ => flat,
    }
}

/// Refuse a flat ceiling at `flat_z` (absolute) that would rise above the roof underside over
/// `room` (by more than 1e-9 at the lowest point of the underside over its polygon); the error
/// names the room. Vaulted rooms, and rooms without a ceiling, are never refused.
pub fn check_flat_ceiling(model: &ArchModel, room: &Room, flat_z: f64) -> Result<(), String> {
    if !room.flat_ceiling || !room.room_type.has_ceiling() {
        return Ok(());
    }
    match lowest_underside_over(model, room.level, &room.polygon).0 {
        Some(m) if m.z < flat_z - ABOVE => Err(format!(
            "room {}: a flat ceiling at {:.3} m would rise above the roof underside ({:.3} m at {:.3}, {:.3})",
            room_label(room.id),
            flat_z,
            m.z,
            m.at.x,
            m.at.y
        )),
        _ => Ok(()),
    }
}

/// A warning for a flat ceiling a roof edit left above the roof underside (by more than 1e-9 at
/// the lowest point of the underside over the room), else `None`.
pub fn ceiling_warning(model: &ArchModel, room: &Room) -> Option<String> {
    if !room.flat_ceiling {
        return None;
    }
    let flat = flat_ceiling_z(model, room)?;
    let m = lowest_underside_over(model, room.level, &room.polygon).0?;
    (flat - m.z > ABOVE).then(|| {
        format!(
            "the roof underside drops below the flat ceiling ({:.3} m < {:.3} m at {:.3}, {:.3}); the ceiling follows the roof there",
            m.z, flat, m.at.x, m.at.y
        )
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::super::{flat_ceiling_z, rough_ceiling_in};
    use super::*;
    use cadcraft_doc::arch::{Level, Roof, RoofDirective, RoofPlane, RoomType, Wall};

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// The phase-1 house (as in `ceiling::tests::house`): rooms S = rooms[0] (0.1..9.9 x
    /// 0.1..2.95), NW = rooms[1] and NE = rooms[2] on level 0 (ceiling 2.4), under a gable over a
    /// run of 12 with rise `rise` (gables on x = 0 and x = 10, overhang 0.5); no roof for `None`.
    /// Its underside is 2.4 + rise / 12 (y + 0.1) for y <= 4.
    fn house(rise: Option<f64>) -> ArchModel {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, name: "Level 1".into(), ..Level::default() });
        let pts = [(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)];
        for k in 0..4 {
            let (a, b) = (pts[k], pts[(k + 1) % 4]);
            let mut w = Wall::new(Vec2::new(a.0, a.1), Vec2::new(b.0, b.1), 0.2);
            w.height = 2.4;
            w.exterior = true;
            if k == 1 || k == 3 {
                w.roof_directive = RoofDirective::Gable;
            }
            m.push_wall(w);
        }
        for poly in [rect(0.1, 0.1, 9.9, 2.95), rect(0.1, 3.05, 5.95, 7.9), rect(6.05, 3.05, 9.9, 7.9)] {
            m.push_room(Room { polygon: poly, level: 0, ..Room::default() });
        }
        if let Some(rise) = rise {
            let seed = Roof { pitch_rise: rise, pitch_run: 12.0, overhang: 0.5, thickness: 0.2, ..Roof::default() };
            let roof = crate::roof::build_roof(&m, 0, &seed).unwrap();
            m.push_roof(roof);
        }
        m
    }

    fn rough(m: &ArchModel, room: usize, x: f64, y: f64) -> f64 {
        rough_ceiling_in(m, &m.rooms[room], Vec2::new(x, y)).unwrap()
    }

    #[test]
    fn vault_follows_the_gable() {
        let mut m = house(Some(6.0));
        m.rooms[0].flat_ceiling = false;
        assert!(near(rough(&m, 0, 5.0, 2.0), 3.45), "{}", rough(&m, 0, 5.0, 2.0));
        assert!(near(rough(&m, 0, 5.0, 0.5), 2.70), "{}", rough(&m, 0, 5.0, 0.5));
        assert!(near(rough(&m, 0, 1.0, 2.95), 2.4 + 0.5 * 3.05));
        // `adjust` itself: the underside wherever the roof covers the point, whatever the flat one.
        let s = m.rooms[0].clone();
        assert!(near(adjust(&m, &s, 2.4, Vec2::new(5.0, 2.0)), 3.45));
        assert!(near(adjust(&m, &s, 9.0, Vec2::new(5.0, 2.0)), 3.45), "a vault may sit below the flat height too");
        // The flat rooms stay flat under the higher roof.
        assert!(near(rough(&m, 1, 3.0, 5.0), 2.4));
        assert!(near(rough(&m, 2, 8.0, 5.0), 2.4));
        // Re-pitching the roof re-pitches the vault: 8/12 gives 2.4 + (8/12) 2.1 = 3.8 at (5, 2).
        let mut steep = house(Some(8.0));
        steep.rooms[0].flat_ceiling = false;
        assert!(near(rough(&steep, 0, 5.0, 2.0), 3.8), "{}", rough(&steep, 0, 5.0, 2.0));
        // A vaulted room on another level than the roof stays flat.
        let mut up = m.clone();
        up.roofs[0].level = 1;
        assert!(near(rough(&up, 0, 5.0, 2.0), 2.4));
    }

    #[test]
    fn vault_without_a_roof_stays_flat() {
        let mut m = house(None);
        m.rooms[0].flat_ceiling = false;
        for (x, y) in [(5.0, 2.0), (5.0, 0.5), (0.2, 2.9)] {
            assert!(near(rough(&m, 0, x, y), 2.4), "({x}, {y})");
        }
        // Parts of a vaulted room the roof does not cover stay flat: a roof plane over x <= 5 only.
        let mut half = house(None);
        half.rooms[0].flat_ceiling = false;
        half.push_roof(Roof {
            planes: vec![RoofPlane {
                ring: rect(-1.0, -1.0, 5.0, 4.0),
                slope: 0.5,
                base_edge: [-1.0, -0.1, 5.0, -0.1],
                eave_z: 2.4,
                ..RoofPlane::default()
            }],
            ..Roof::default()
        });
        assert!(near(rough(&half, 0, 2.0, 2.0), 3.45));
        assert!(near(rough(&half, 0, 7.0, 2.0), 2.4), "outside the roof");
        // A room without a ceiling has none to vault.
        let mut deck = house(Some(6.0));
        deck.rooms[0].flat_ceiling = false;
        deck.rooms[0].room_type = RoomType::Deck;
        assert_eq!(rough_ceiling_in(&deck, &deck.rooms[0], Vec2::new(5.0, 2.0)), None);
    }

    #[test]
    fn flat_ceiling_follows_a_lower_roof_only_where_it_is_lower() {
        // A 2/12 gable: underside 2.4 + (y + 0.1) / 6, 2.45 at y = 0.2. S's own ceiling is 2.45.
        let mut m = house(Some(2.0));
        m.rooms[0].ceiling_height = Some(2.45);
        assert!(near(rough(&m, 0, 5.0, 0.15), 2.4 + 0.25 / 6.0), "{}", rough(&m, 0, 5.0, 0.15));
        assert!(near(rough(&m, 0, 5.0, 0.1), 2.4 + 0.2 / 6.0));
        assert!(near(rough(&m, 0, 5.0, 0.2), 2.45), "the underside meets the ceiling");
        assert!(near(rough(&m, 0, 5.0, 2.0), 2.45), "flat where the roof is higher");
        // The flat rooms at 2.4 stay at 2.4: the underside over them is never lower.
        assert!(near(rough(&m, 1, 3.0, 7.85), 2.4));
        assert!(near(rough(&m, 2, 8.0, 5.0), 2.4));
        // `adjust`: the flat height where no roof is lower, the underside where it is.
        let s = m.rooms[0].clone();
        assert!(near(adjust(&m, &s, 2.45, Vec2::new(5.0, 0.15)), 2.4 + 0.25 / 6.0));
        assert!(near(adjust(&m, &s, 2.45, Vec2::new(5.0, 3.0)), 2.45));
        assert!(near(adjust(&m, &s, 2.45, Vec2::new(50.0, 3.0)), 2.45), "outside every roof");
    }

    #[test]
    fn check_flat_ceiling_refuses_and_names_the_room() {
        let m = house(Some(6.0));
        let (s, ne) = (&m.rooms[0], &m.rooms[2]);
        let e = check_flat_ceiling(&m, ne, 3.2).unwrap_err();
        assert!(e.starts_with(&format!("room r{}:", ne.id.0)), "{e}");
        assert!(e.contains("3.200") && e.contains("2.500"), "{e}");
        // S: the underside is 2.5 along y = 0.1.
        let e = check_flat_ceiling(&m, s, 2.6).unwrap_err();
        assert!(e.contains(&format!("r{}", s.id.0)) && e.contains("2.500") && e.contains("0.100"), "{e}");
        assert_eq!(check_flat_ceiling(&m, s, 2.45), Ok(()));
        assert_eq!(check_flat_ceiling(&m, s, 2.5), Ok(()), "touching the underside is allowed");
        assert_eq!(check_flat_ceiling(&m, s, 2.5 + 5e-10), Ok(()), "within 1e-9");
        assert!(check_flat_ceiling(&m, s, 2.5 + 1e-6).is_err());
        // No roof over the room (none at all, or one on another level): nothing to refuse.
        assert_eq!(check_flat_ceiling(&house(None), s, 5.0), Ok(()));
        let mut up = m.clone();
        up.roofs[0].level = 1;
        assert_eq!(check_flat_ceiling(&up, s, 5.0), Ok(()));
    }

    #[test]
    fn check_flat_ceiling_ignores_vaulted_rooms_and_rooms_without_ceilings() {
        let mut m = house(Some(6.0));
        m.rooms[0].flat_ceiling = false;
        assert_eq!(check_flat_ceiling(&m, &m.rooms[0], 5.0), Ok(()));
        for t in [RoomType::Deck, RoomType::Porch] {
            let r = Room { room_type: t, ..m.rooms[2].clone() };
            assert_eq!(check_flat_ceiling(&m, &r, 5.0), Ok(()), "{t:?}");
        }
        // A flat room of another type is checked.
        let bed = Room { room_type: RoomType::Bedroom, ..m.rooms[2].clone() };
        assert!(check_flat_ceiling(&m, &bed, 5.0).is_err());
    }

    #[test]
    fn warning_after_a_lower_roof() {
        let mut m = house(Some(2.0));
        m.rooms[0].ceiling_height = Some(2.45);
        let w = ceiling_warning(&m, &m.rooms[0]).unwrap();
        // The lowest underside over S is 2.4 + 0.2 / 6 = 2.433 along y = 0.1.
        assert!(w.contains("2.433 m < 2.450 m") && w.contains("0.100"), "{w}");
        assert!(w.contains("follows the roof"), "{w}");
        assert_eq!(ceiling_warning(&m, &m.rooms[1]), None);
        assert_eq!(ceiling_warning(&m, &m.rooms[2]), None);
        // No warning for a vaulted room, a ceiling at the underside, a room without a ceiling or
        // without a roof.
        let mut vault = m.clone();
        vault.rooms[0].flat_ceiling = false;
        assert_eq!(ceiling_warning(&vault, &vault.rooms[0]), None);
        let mut low = m.clone();
        low.rooms[0].ceiling_height = Some(2.4 + 0.2 / 6.0);
        assert_eq!(ceiling_warning(&low, &low.rooms[0]), None);
        let mut deck = m.clone();
        deck.rooms[0].room_type = RoomType::Deck;
        assert_eq!(ceiling_warning(&deck, &deck.rooms[0]), None);
        let mut bare = house(None);
        bare.rooms[0].ceiling_height = Some(2.45);
        assert_eq!(ceiling_warning(&bare, &bare.rooms[0]), None);
        // The 6/12 roof leaves the flat house without warnings.
        let six = house(Some(6.0));
        assert!(six.rooms.iter().all(|r| ceiling_warning(&six, r).is_none()));
    }

    #[test]
    fn vault_queries_survive_nan_roof_planes() {
        let mut m = house(Some(6.0));
        m.rooms[0].flat_ceiling = false;
        m.roofs[0].planes[0].eave_z = f64::NAN;
        m.roofs[0].planes[1].ring = vec![Vec2::new(f64::NAN, 0.0), Vec2::new(1e300, 0.0), Vec2::new(0.0, f64::INFINITY)];
        m.push_roof(Roof {
            planes: vec![
                RoofPlane {
                    ring: rect(0.0, 0.0, 10.0, 8.0),
                    slope: f64::INFINITY,
                    base_edge: [0.0, 0.0, 10.0, 0.0],
                    eave_z: 2.4,
                    ..RoofPlane::default()
                },
                RoofPlane { ring: rect(0.0, 0.0, 10.0, 8.0), slope: 0.5, base_edge: [0.0, 0.0, 0.0, 0.0], eave_z: 2.4, ..RoofPlane::default() },
                RoofPlane {
                    ring: rect(-1e300, -1e300, 1e300, 1e300),
                    slope: 1e300,
                    base_edge: [0.0, 0.0, 1.0, 0.0],
                    eave_z: 1e300,
                    ..RoofPlane::default()
                },
                RoofPlane { ring: vec![Vec2::ZERO; 2], slope: 0.5, base_edge: [0.0, 0.0, 1.0, 0.0], ..RoofPlane::default() },
            ],
            ..Roof::default()
        });
        for room in m.rooms.clone() {
            for p in [Vec2::new(5.0, 2.0), Vec2::new(f64::NAN, 1.0), Vec2::new(1e300, -1e300), Vec2::new(f64::INFINITY, 0.0)] {
                for flat in [2.4, f64::NAN, 1e308] {
                    let z = adjust(&m, &room, flat, p);
                    assert!(z.is_finite() || !flat.is_finite(), "a finite flat ceiling stays finite: {z} at {p:?}");
                }
                let _ = rough_ceiling_in(&m, &room, p);
            }
            for z in [2.4, f64::NAN, f64::INFINITY, -1e308] {
                let _ = check_flat_ceiling(&m, &room, z);
            }
            let _ = ceiling_warning(&m, &room);
        }
        // Where the hostile planes give no underside, a vaulted point stays flat.
        let mut nan_only = house(None);
        nan_only.rooms[0].flat_ceiling = false;
        nan_only.push_roof(Roof {
            planes: vec![RoofPlane {
                ring: rect(0.0, 0.0, 10.0, 8.0),
                slope: f64::NAN,
                base_edge: [0.0, 0.0, 10.0, 0.0],
                eave_z: 2.4,
                ..RoofPlane::default()
            }],
            ..Roof::default()
        });
        assert!(near(rough(&nan_only, 0, 5.0, 2.0), 2.4));
        assert_eq!(check_flat_ceiling(&nan_only, &Room { flat_ceiling: true, ..nan_only.rooms[0].clone() }, 9.0), Ok(()));
        // Hostile rooms: a NaN polygon, a huge ceiling height.
        let mut hostile = house(Some(6.0));
        hostile.rooms[0].polygon = vec![Vec2::new(f64::NAN, 0.0); 4];
        hostile.rooms[1].ceiling_height = Some(1e308);
        for r in hostile.rooms.clone() {
            let _ = check_flat_ceiling(&hostile, &r, 2.4);
            let _ = ceiling_warning(&hostile, &r);
            let _ = flat_ceiling_z(&hostile, &r).map(|z| adjust(&hostile, &r, z, Vec2::new(3.0, 5.0)));
        }
    }
}
