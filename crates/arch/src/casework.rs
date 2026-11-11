//! Cabinet and fixture catalogue (phase 13): default sizes, validation, wall placement and
//! layers. The data lives in `cadcraft_doc::arch::casework`.

use crate::join::ArchError;
use cadcraft_doc::arch::{Cabinet, CabinetKind, Fixture, FixtureKind, Fronts};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_geom::Vec2;

pub use crate::casework_symbol::{cabinet_symbol, fixture_symbol};

/// Widest cabinet that still gets a single door (m).
const SINGLE_DOOR_MAX_WIDTH: f64 = 0.6;
/// Highest a cabinet may reach above the floor (m).
const MAX_TOP: f64 = 3.0;
/// Position coordinate limit (m).
const MAX_COORD: f64 = 1e6;

fn bad<T>(msg: impl Into<String>) -> Result<T, ArchError> {
    Err(ArchError::Invalid(msg.into()))
}

/// The default cabinet of `kind`: a base cabinet is 0.6 x 0.6 x 0.876 with a 38 mm counter (top
/// at 0.914), a wall cabinet 0.6 x 0.305 x 0.762 hung at 1.372, a tall one 0.6 x 0.6 x 2.134.
pub fn cabinet_defaults(kind: CabinetKind) -> Cabinet {
    let mut c = Cabinet {
        kind,
        width: 0.6,
        depth: 0.6,
        height: 0.876,
        fronts: Fronts::Doors,
        doors: 1,
        counter_thickness: 0.038,
        counter_overhang: 0.025,
        ..Cabinet::default()
    };
    match kind {
        CabinetKind::Base => {
            c.fronts = Fronts::DrawerOverDoor;
            c.drawers = 1;
            c.counter = true;
            c.toe_kick = true;
        }
        CabinetKind::Wall => {
            c.depth = 0.305;
            c.height = 0.762;
            c.elevation = 1.372;
        }
        CabinetKind::Tall => {
            c.height = 2.134;
            c.toe_kick = true;
        }
    }
    if c.width > SINGLE_DOOR_MAX_WIDTH {
        c.doors = 2;
    }
    c
}

/// The default fixture of `kind` (width x depth x height at an elevation).
pub fn fixture_defaults(kind: FixtureKind) -> Fixture {
    let (width, depth, height, elevation) = match kind {
        FixtureKind::Sink => (0.8, 0.55, 0.02, 0.914),
        FixtureKind::Lavatory => (0.5, 0.45, 0.85, 0.0),
        FixtureKind::Toilet => (0.45, 0.7, 0.76, 0.0),
        FixtureKind::Bathtub => (1.52, 0.76, 0.51, 0.0),
        FixtureKind::Shower => (0.9, 0.9, 0.05, 0.0),
        FixtureKind::Range => (0.76, 0.65, 0.914, 0.0),
        FixtureKind::Refrigerator => (0.76, 0.76, 1.78, 0.0),
        FixtureKind::Dishwasher => (0.6, 0.6, 0.876, 0.0),
    };
    Fixture { kind, width, depth, height, elevation, ..Fixture::default() }
}

fn in_range(v: f64, lo: f64, hi: f64) -> bool {
    v.is_finite() && v >= lo && v <= hi
}

fn check_position(p: Vec2) -> Result<(), ArchError> {
    if !in_range(p.x, -MAX_COORD, MAX_COORD) || !in_range(p.y, -MAX_COORD, MAX_COORD) {
        return bad("position must be finite and within 1e6 m");
    }
    Ok(())
}

/// Refuse (never clamp) a cabinet with out-of-range or inconsistent values.
pub fn validate_cabinet(c: &Cabinet) -> Result<(), ArchError> {
    check_position(c.position)?;
    if !c.rotation.is_finite() {
        return bad("rotation must be finite");
    }
    if !in_range(c.width, 0.2, 1.5) {
        return bad("cabinet width must be 0.2..=1.5 m");
    }
    if !in_range(c.depth, 0.2, 0.9) {
        return bad("cabinet depth must be 0.2..=0.9 m");
    }
    if !in_range(c.height, 0.2, 2.6) {
        return bad("cabinet height must be 0.2..=2.6 m");
    }
    if !in_range(c.elevation, 0.0, 2.6) {
        return bad("cabinet elevation must be 0..=2.6 m");
    }
    if c.doors > 4 {
        return bad("a cabinet has at most 4 doors");
    }
    if c.drawers > 6 {
        return bad("a cabinet has at most 6 drawers");
    }
    if c.kind == CabinetKind::Wall && c.counter {
        return bad("a wall cabinet has no counter");
    }
    if !in_range(c.counter_thickness, 0.01, 0.1) {
        return bad("counter thickness must be 0.01..=0.1 m");
    }
    if !in_range(c.counter_overhang, 0.0, 0.1) {
        return bad("counter overhang must be 0..=0.1 m");
    }
    if c.elevation + c.height > MAX_TOP {
        return bad("cabinet top must not exceed 3.0 m");
    }
    let ok = match c.fronts {
        Fronts::Doors => c.doors >= 1,
        Fronts::Drawers => c.drawers >= 1,
        Fronts::DrawerOverDoor => c.doors >= 1 && c.drawers >= 1,
        Fronts::Open => true,
    };
    if !ok {
        return bad("fronts do not match the door and drawer counts");
    }
    Ok(())
}

/// Refuse (never clamp) a fixture with out-of-range values.
pub fn validate_fixture(f: &Fixture) -> Result<(), ArchError> {
    check_position(f.position)?;
    if !f.rotation.is_finite() {
        return bad("rotation must be finite");
    }
    if !in_range(f.width, 0.2, 2.5) || !in_range(f.depth, 0.2, 2.5) {
        return bad("fixture width and depth must be 0.2..=2.5 m");
    }
    if !in_range(f.height, 0.01, 2.5) {
        return bad("fixture height must be 0.01..=2.5 m");
    }
    if !in_range(f.elevation, 0.0, 2.0) {
        return bad("fixture elevation must be 0..=2.0 m");
    }
    Ok(())
}

/// Place an item against a wall's LEFT face (left of start -> end; the inside of a
/// counter-clockwise ring). Its centre is `offset` along the centre line from the start.
/// Returns `(position, rotation in degrees, level)`; the position is the centre of the back
/// edge. Refuses an unknown wall, a non-finite offset or size, or an item past either wall end.
pub fn on_wall(model: &ArchModel, wall: ArchId, offset: f64, width: f64, depth: f64) -> Result<(Vec2, f64, u32), ArchError> {
    let w = model.wall(wall).ok_or(ArchError::NoSuchWall(wall))?;
    if !offset.is_finite() || !width.is_finite() || !depth.is_finite() {
        return Err(ArchError::NotFinite);
    }
    let dir = crate::wall::wall_dir(w).ok_or_else(|| ArchError::Invalid("wall has no length".into()))?;
    let length = w.start.dist(w.end);
    if offset - width / 2.0 < 0.0 || offset + width / 2.0 > length {
        return bad("item extends past the end of the wall");
    }
    let left = dir.perp();
    let position = w.start + dir * offset + left * (w.thickness / 2.0);
    Ok((position, dir.y.atan2(dir.x).to_degrees(), w.base_level))
}

/// The wall a cabinet stands against, and the distances of its two ends from that
/// wall's start: `(wall, s0, s1)`. `None` for a free-standing cabinet (`at` placement),
/// a non-finite placement, or when no wall's left face carries its back edge.
pub fn host_wall(model: &ArchModel, c: &Cabinet) -> Option<(ArchId, f64, f64)> {
    if !c.position.is_finite() || !c.rotation.is_finite() || !c.width.is_finite() {
        return None;
    }
    let mut best: Option<(f64, ArchId, f64, f64)> = None;
    for w in &model.walls {
        if !w.start.is_finite() || !w.end.is_finite() || !w.thickness.is_finite() {
            continue;
        }
        let length = w.start.dist(w.end);
        if length <= 0.0 {
            continue;
        }
        let Some(dir) = crate::wall::wall_dir(w) else { continue };
        let left = dir.perp();
        let d = (c.position - (w.start + left * (w.thickness / 2.0))).dot(left);
        if !(d.abs() <= 1e-6) {
            continue;
        }
        let turn = (c.rotation - dir.y.atan2(dir.x).to_degrees() + 180.0).rem_euclid(360.0) - 180.0;
        if !(turn.abs() <= 1e-6) {
            continue;
        }
        let s = (c.position - w.start).dot(dir);
        let (s0, s1) = (s - c.width / 2.0, s + c.width / 2.0);
        if !(s0 >= -1e-9 && s1 <= length + 1e-9) {
            continue;
        }
        let better = match best {
            None => true,
            Some((bd, bid, _, _)) => d.abs() < bd || (d.abs() == bd && w.id < bid),
        };
        if better {
            best = Some((d.abs(), w.id, s0, s1));
        }
    }
    best.map(|(_, id, s0, s1)| (id, s0, s1))
}

/// The layer a cabinet draws on.
pub fn cabinet_layer(kind: CabinetKind) -> &'static str {
    match kind {
        CabinetKind::Base | CabinetKind::Tall => "A-FLOR-CASE",
        CabinetKind::Wall => "A-FLOR-CASE-UPPR",
    }
}

/// The layer a fixture draws on.
pub fn fixture_layer(kind: FixtureKind) -> &'static str {
    match kind {
        FixtureKind::Sink | FixtureKind::Lavatory | FixtureKind::Toilet | FixtureKind::Bathtub | FixtureKind::Shower => "P-FLOR-FIXT",
        FixtureKind::Range | FixtureKind::Refrigerator | FixtureKind::Dishwasher => "A-FLOR-APPL",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::Wall;

    #[test]
    fn base_cabinet_counter_top_is_0_914() {
        let c = cabinet_defaults(CabinetKind::Base);
        assert!(c.counter && c.toe_kick);
        assert!((c.elevation + c.height + c.counter_thickness - 0.914).abs() < 1e-9);
        assert!(validate_cabinet(&c).is_ok());
        assert!(validate_cabinet(&cabinet_defaults(CabinetKind::Wall)).is_ok());
        assert!(validate_cabinet(&cabinet_defaults(CabinetKind::Tall)).is_ok());
        for k in [
            FixtureKind::Sink,
            FixtureKind::Lavatory,
            FixtureKind::Toilet,
            FixtureKind::Bathtub,
            FixtureKind::Shower,
            FixtureKind::Range,
            FixtureKind::Refrigerator,
            FixtureKind::Dishwasher,
        ] {
            assert!(validate_fixture(&fixture_defaults(k)).is_ok());
        }
        assert_eq!(cabinet_layer(CabinetKind::Wall), "A-FLOR-CASE-UPPR");
        assert_eq!(fixture_layer(FixtureKind::Range), "A-FLOR-APPL");
    }

    #[test]
    fn wall_cabinet_with_counter_is_refused() {
        let mut c = cabinet_defaults(CabinetKind::Wall);
        c.counter = true;
        assert!(validate_cabinet(&c).is_err());
        let mut c = cabinet_defaults(CabinetKind::Base);
        c.width = f64::NAN;
        assert!(validate_cabinet(&c).is_err());
        let mut c = cabinet_defaults(CabinetKind::Base);
        c.fronts = Fronts::Drawers;
        c.drawers = 0;
        assert!(validate_cabinet(&c).is_err());
    }

    fn model_with_wall() -> (ArchModel, ArchId) {
        let mut m = ArchModel::default();
        let id = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        (m, id)
    }

    #[test]
    fn on_wall_puts_the_back_on_the_left_face() {
        let (m, id) = model_with_wall();
        let (p, rot, level) = on_wall(&m, id, 1.0, 0.6, 0.6).unwrap();
        assert!((p.x - 1.0).abs() < 1e-9 && (p.y - 0.1).abs() < 1e-9);
        assert!(rot.abs() < 1e-9);
        assert_eq!(level, 0);
    }

    fn cabinet_at(position: Vec2, rotation: f64) -> Cabinet {
        let mut c = cabinet_defaults(CabinetKind::Base);
        c.position = position;
        c.rotation = rotation;
        c
    }

    #[test]
    fn host_wall_finds_the_host_and_span() {
        let (m, id) = model_with_wall();
        let (p, rot, _) = on_wall(&m, id, 1.0, 0.6, 0.6).unwrap();
        let mut c = cabinet_at(p, rot);
        c.width = 0.6;
        let (wid, s0, s1) = host_wall(&m, &c).unwrap();
        assert_eq!(wid, id);
        assert!((s0 - 0.7).abs() < 1e-9 && (s1 - 1.3).abs() < 1e-9);
    }

    #[test]
    fn host_wall_is_none_for_a_free_standing_cabinet() {
        let (m, _) = model_with_wall();
        assert!(host_wall(&m, &cabinet_at(Vec2::new(2.0, 1.5), 0.0)).is_none());
        assert!(host_wall(&m, &cabinet_at(Vec2::new(f64::NAN, 0.1), 0.0)).is_none());
        assert!(host_wall(&m, &cabinet_at(Vec2::new(1.0, 0.1), 90.0)).is_none());
    }

    #[test]
    fn host_wall_refuses_a_cabinet_past_the_wall_end() {
        let (m, _) = model_with_wall();
        let mut c = cabinet_at(Vec2::new(3.9, 0.1), 0.0);
        c.width = 0.6;
        assert!(host_wall(&m, &c).is_none());
    }

    #[test]
    fn on_wall_refuses_an_item_past_the_wall_end() {
        let (m, id) = model_with_wall();
        assert!(on_wall(&m, id, 3.8, 0.6, 0.6).is_err());
        assert!(on_wall(&m, id, 0.2, 0.6, 0.6).is_err());
        assert!(on_wall(&m, id, f64::NAN, 0.6, 0.6).is_err());
        assert!(on_wall(&m, ArchId(99), 1.0, 0.6, 0.6).is_err());
        assert!(on_wall(&m, id, 0.3, 0.6, 0.6).is_ok());
    }

    #[test]
    fn host_wall_picks_closest_of_two_walls() {
        let mut m = ArchModel::default();
        let w1 = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        let w2 = m.push_wall(Wall::new(Vec2::new(0.0, 2.0), Vec2::new(4.0, 2.0), 0.2));
        // Cabinet on w1's left face (y = 0.1), far from w2's left face (y = 2.1)
        let (p, rot, _) = on_wall(&m, w1, 1.0, 0.6, 0.6).unwrap();
        let c = cabinet_at(p, rot);
        let (wid, s0, s1) = host_wall(&m, &c).unwrap();
        assert_eq!(wid, w1);
        assert!((s0 - 0.7).abs() < 1e-9 && (s1 - 1.3).abs() < 1e-9);
    }

    #[test]
    fn host_wall_skips_zero_length_wall() {
        let mut m = ArchModel::default();
        let _zero = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, 0.0), 0.2));
        let w1 = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        let (p, rot, _) = on_wall(&m, w1, 1.0, 0.6, 0.6).unwrap();
        let c = cabinet_at(p, rot);
        let (wid, _, _) = host_wall(&m, &c).unwrap();
        assert_eq!(wid, w1);
    }

    #[test]
    fn host_wall_skips_non_finite_wall_endpoints() {
        let mut m = ArchModel::default();
        let nan_wall = Wall { start: Vec2::new(f64::NAN, 0.0), end: Vec2::new(4.0, 0.0), thickness: 0.2, ..Wall::default() };
        m.push_wall(nan_wall);
        let w1 = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        let (p, rot, _) = on_wall(&m, w1, 1.0, 0.6, 0.6).unwrap();
        let c = cabinet_at(p, rot);
        let (wid, _, _) = host_wall(&m, &c).unwrap();
        assert_eq!(wid, w1);
    }

    #[test]
    fn host_wall_cabinet_at_wall_start_and_end() {
        let (m, id) = model_with_wall();
        // Cabinet exactly at the wall's start: offset = width/2 = 0.3
        let (p1, rot1, _) = on_wall(&m, id, 0.3, 0.6, 0.6).unwrap();
        let c1 = cabinet_at(p1, rot1);
        let (wid1, s01, s11) = host_wall(&m, &c1).unwrap();
        assert_eq!(wid1, id);
        assert!((s01 - 0.0).abs() < 1e-9 && (s11 - 0.6).abs() < 1e-9);
        // Cabinet exactly at the wall's end: offset = 4.0 - 0.3 = 3.7
        let (p2, rot2, _) = on_wall(&m, id, 3.7, 0.6, 0.6).unwrap();
        let c2 = cabinet_at(p2, rot2);
        let (wid2, s02, s12) = host_wall(&m, &c2).unwrap();
        assert_eq!(wid2, id);
        assert!((s02 - 3.4).abs() < 1e-9 && (s12 - 4.0).abs() < 1e-9);
    }

    #[test]
    fn host_wall_on_vertical_wall() {
        let mut m = ArchModel::default();
        let w = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, 4.0), 0.2));
        let (p, rot, _) = on_wall(&m, w, 1.0, 0.6, 0.6).unwrap();
        // Left face of a wall going +Y is at x = -0.1
        assert!((p.x + 0.1).abs() < 1e-9 && (p.y - 1.0).abs() < 1e-9);
        let c = cabinet_at(p, rot);
        let (wid, s0, s1) = host_wall(&m, &c).unwrap();
        assert_eq!(wid, w);
        assert!((s0 - 0.7).abs() < 1e-9 && (s1 - 1.3).abs() < 1e-9);
    }

    #[test]
    fn host_wall_tie_breaks_by_smaller_id() {
        let mut m = ArchModel::default();
        // Two coincident walls, same direction
        let w1 = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        let w2 = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        let (p, rot, _) = on_wall(&m, w1, 1.0, 0.6, 0.6).unwrap();
        let c = cabinet_at(p, rot);
        let (wid, _, _) = host_wall(&m, &c).unwrap();
        // Both walls match exactly; the smaller ArchId wins
        assert_eq!(wid, w1);
        assert!(w1 < w2);
    }

    #[test]
    fn host_wall_refuses_cabinet_wider_than_wall() {
        let (m, id) = model_with_wall();
        // Wall is 4m; cabinet is 5m wide, placed at offset 2.0
        let mut c = cabinet_at(Vec2::new(2.0, 0.1), 0.0);
        c.width = 5.0;
        assert!(host_wall(&m, &c).is_none());
    }
}
