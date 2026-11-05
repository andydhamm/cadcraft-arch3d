//! Surfaces, the rooms they face and their effective materials; painter scopes (phase 14).
//!
//! Pure: everything here reads an [`ArchModel`] and returns plain data. Output order is by
//! construction (model order) or sorted by surface key, never by hash order.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchId, CabinetPart, Finish, Side, Surface};
use cadcraft_geom::{Vec2, point_in_polygon};

/// How far outside a wall face the room probe point is moved.
const PROBE: f64 = 0.05;

/// One paintable surface with its room, level and effective material.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceInfo {
    /// The surface.
    pub surface: Surface,
    /// The room it faces (`None`: outside, or a roof).
    pub room: Option<ArchId>,
    /// The level it belongs to.
    pub level: u32,
    /// The effective material key (`finish:siding`, `default:wall`, `sym:<key>:<i>`, ...).
    pub material: String,
    /// The paint's tint.
    pub tint: Option<[u8; 3]>,
}

/// How far a painter's choice reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Just the clicked surface.
    Component,
    /// Every surface of the clicked object.
    Object,
    /// Like surfaces in the same room.
    Room,
    /// Like surfaces on the same level.
    Floor,
    /// Like surfaces anywhere.
    Plan,
}

impl Scope {
    /// Parse `component`, `object`, `room`, `floor` or `plan`.
    pub fn parse(s: &str) -> Option<Scope> {
        match s {
            "component" => Some(Scope::Component),
            "object" => Some(Scope::Object),
            "room" => Some(Scope::Room),
            "floor" => Some(Scope::Floor),
            "plan" => Some(Scope::Plan),
            _ => None,
        }
    }
}

/// The room on `level` containing `p` (inside the outer polygon, outside every hole), if any.
fn room_at(model: &ArchModel, level: u32, p: Vec2) -> Option<ArchId> {
    if !p.is_finite() {
        return None;
    }
    model
        .rooms
        .iter()
        .find(|r| {
            r.level == level
                && r.polygon.len() >= 3
                && point_in_polygon(&r.polygon, p)
                && !r.holes.iter().any(|h| h.len() >= 3 && point_in_polygon(h, p))
        })
        .map(|r| r.id)
}

/// The plan point `origin + R(rotation degrees)·(0, dy)`.
fn frame_point(origin: Vec2, rotation: f64, dy: f64) -> Vec2 {
    origin + Vec2::new(0.0, dy).rotate(rotation.to_radians())
}

/// The room a wall side faces.
fn wall_side_room(model: &ArchModel, wall: &cadcraft_doc::Wall, side: Side) -> Option<ArchId> {
    let dir = (wall.end - wall.start).normalized();
    let n = match side {
        Side::Left => dir.perp(),
        Side::Right => -dir.perp(),
    };
    let mid = wall.start.mid(wall.end);
    let p = mid + n * (wall.thickness / 2.0 + PROBE);
    if let Some(r) = room_at(model, wall.base_level, p) {
        return Some(r);
    }
    // A wall spanning several levels also bounds the rooms on the levels it passes.
    model.levels.iter().filter(|l| l.index != wall.base_level && wall.spans_level(l, &model.levels)).find_map(|l| room_at(model, l.index, p))
}

/// The material a surface has before any paint.
fn unpainted(model: &ArchModel, s: &Surface) -> String {
    let finish = |f: Finish| format!("finish:{}", f.name());
    let default = |n: &str| format!("default:{n}");
    match s {
        Surface::WallSide { wall, side } => match (side, model.wall(*wall).and_then(|w| w.finish)) {
            (Side::Right, Some(f)) => finish(f),
            _ => default("wall"),
        },
        Surface::RoomFloor(r) => match model.room(*r).and_then(|r| r.floor_finish) {
            Some(f) => finish(f),
            None => default("floor"),
        },
        Surface::RoomCeiling(_) => default("ceiling"),
        Surface::Roof(r) => match model.roof(*r).and_then(|r| r.covering) {
            Some(f) => finish(f),
            None => default("roof"),
        },
        Surface::Cabinet { cabinet, part } => {
            let c = model.cabinets.iter().find(|c| c.id == *cabinet);
            match part {
                CabinetPart::Carcass => default("cabinet"),
                CabinetPart::Front => format!("frontstyle:{}", c.map(|c| c.door_style.name()).unwrap_or("slab")),
                CabinetPart::Counter => default("counter"),
                CabinetPart::Pull => format!("item:{}", c.and_then(|c| c.pull.as_deref()).unwrap_or("")),
            }
        }
        Surface::Fixture(_) => default("fixture"),
        Surface::Furnishing { furnishing, part } => {
            let key = model.furnishing(*furnishing).map(|f| f.symbol.as_str()).unwrap_or("");
            format!("sym:{key}:{part}")
        }
    }
}

/// The effective material key of `s`: the paint's material when it has one, else the unpainted one.
pub fn effective_material(model: &ArchModel, s: &Surface) -> String {
    match model.paints.get(&s.key()).and_then(|p| p.material.as_ref()) {
        Some(m) => m.key(),
        None => unpainted(model, s),
    }
}

fn info(model: &ArchModel, surface: Surface, room: Option<ArchId>, level: u32) -> SurfaceInfo {
    let material = effective_material(model, &surface);
    let tint = model.paints.get(&surface.key()).and_then(|p| p.tint);
    SurfaceInfo { surface, room, level, material, tint }
}

/// Every paintable surface of the model, in the documented order.
pub fn surfaces(model: &ArchModel) -> Vec<SurfaceInfo> {
    let mut out = Vec::new();
    for w in &model.walls {
        for side in [Side::Left, Side::Right] {
            let room = wall_side_room(model, w, side);
            out.push(info(model, Surface::WallSide { wall: w.id, side }, room, w.base_level));
        }
    }
    for r in &model.rooms {
        out.push(info(model, Surface::RoomFloor(r.id), Some(r.id), r.level));
        out.push(info(model, Surface::RoomCeiling(r.id), Some(r.id), r.level));
    }
    for r in &model.roofs {
        out.push(info(model, Surface::Roof(r.id), None, r.level));
    }
    let owners: Vec<ArchId> = crate::casework_layout::counter_runs(model).iter().map(|r| r.owner).collect();
    for c in &model.cabinets {
        let room = room_at(model, c.level, frame_point(c.position, c.rotation, c.depth / 2.0));
        let mut parts = vec![CabinetPart::Carcass, CabinetPart::Front];
        if owners.contains(&c.id) {
            parts.push(CabinetPart::Counter);
        }
        if c.pull.is_some() {
            parts.push(CabinetPart::Pull);
        }
        for part in parts {
            out.push(info(model, Surface::Cabinet { cabinet: c.id, part }, room, c.level));
        }
    }
    for f in &model.fixtures {
        let room = room_at(model, f.level, frame_point(f.position, f.rotation, f.depth / 2.0));
        out.push(info(model, Surface::Fixture(f.id), room, f.level));
    }
    for f in &model.furnishings {
        let room = room_at(model, f.level, Vec2::new(f.at[0], f.at[1]));
        let n = model.symbols.get(&f.symbol).map_or(1, |s| s.parts.len().max(1));
        for i in 0..n {
            let part = u32::try_from(i).unwrap_or(u32::MAX);
            out.push(info(model, Surface::Furnishing { furnishing: f.id, part }, room, f.level));
        }
    }
    out
}

fn find<'a>(all: &'a [SurfaceInfo], target: &Surface) -> Result<&'a SurfaceInfo, String> {
    all.iter().find(|i| &i.surface == target).ok_or_else(|| format!("no such surface: {}", target.key()))
}

fn sorted(mut v: Vec<Surface>) -> Vec<Surface> {
    v.sort_by_key(|s| s.key());
    v.dedup();
    v
}

/// The surfaces a painter with `scope` takes when `target` is clicked (like materials).
pub fn select(model: &ArchModel, target: &Surface, scope: Scope) -> Result<Vec<Surface>, String> {
    let all = surfaces(model);
    let t = find(&all, target)?;
    let picked: Vec<Surface> = match scope {
        Scope::Component => vec![t.surface.clone()],
        Scope::Object => {
            let key = t.surface.object_key();
            all.iter().filter(|i| i.surface.object_key() == key).map(|i| i.surface.clone()).collect()
        }
        Scope::Room => match t.room {
            None => vec![t.surface.clone()],
            Some(room) => all.iter().filter(|i| i.room == Some(room) && i.material == t.material).map(|i| i.surface.clone()).collect(),
        },
        Scope::Floor => all.iter().filter(|i| i.level == t.level && i.material == t.material).map(|i| i.surface.clone()).collect(),
        Scope::Plan => all.iter().filter(|i| i.material == t.material).map(|i| i.surface.clone()).collect(),
    };
    Ok(sorted(picked))
}

/// Whether `s` is a cabinet surface of `part` for a cabinet satisfying `ok`.
fn cabinet_like(model: &ArchModel, s: &Surface, want: CabinetPart, ok: impl Fn(&cadcraft_doc::arch::Cabinet) -> bool) -> bool {
    match s {
        Surface::Cabinet { cabinet, part } if *part == want => model.cabinets.iter().find(|c| c.id == *cabinet).is_some_and(ok),
        _ => false,
    }
}

/// The surfaces the object painter takes: like items instead of like materials.
pub fn select_objects(model: &ArchModel, target: &Surface, scope: Scope) -> Result<Vec<Surface>, String> {
    let all = surfaces(model);
    let t = find(&all, target)?;
    if matches!(scope, Scope::Component | Scope::Object) {
        return Ok(vec![t.surface.clone()]);
    }
    let cabinet_of = |id: &ArchId| model.cabinets.iter().find(|c| c.id == *id);
    let like: Box<dyn Fn(&Surface) -> bool + '_> = match &t.surface {
        Surface::Cabinet { cabinet, part: CabinetPart::Pull } => {
            let pull = cabinet_of(cabinet).and_then(|c| c.pull.clone());
            Box::new(move |s| cabinet_like(model, s, CabinetPart::Pull, |c| c.pull.is_some() && c.pull == pull))
        }
        Surface::Cabinet { cabinet, part: CabinetPart::Front } => {
            let style = cabinet_of(cabinet).map(|c| c.door_style);
            Box::new(move |s| cabinet_like(model, s, CabinetPart::Front, |c| Some(c.door_style) == style))
        }
        Surface::Furnishing { furnishing, .. } => {
            let item = model.furnishing(*furnishing).map(|f| f.item.clone());
            Box::new(move |s| match s {
                Surface::Furnishing { furnishing, part: 0 } => model.furnishing(*furnishing).map(|f| f.item.clone()) == item,
                _ => false,
            })
        }
        _ => return select(model, target, scope),
    };
    let mut picked: Vec<Surface> = all
        .iter()
        .filter(|i| like(&i.surface))
        .filter(|i| match scope {
            Scope::Room => t.room.is_some() && i.room == t.room,
            Scope::Floor => i.level == t.level,
            _ => true,
        })
        .map(|i| i.surface.clone())
        .collect();
    // The clicked surface is always part of its own selection (no room, or a furnishing's part > 0).
    if !picked.contains(&t.surface) {
        picked.push(t.surface.clone());
    }
    Ok(sorted(picked))
}

/// Whether finish `f` can be painted on `surface` (the phase-13 rules).
pub fn finish_fits(surface: &Surface, f: Finish) -> bool {
    match surface {
        Surface::WallSide { .. } => f.for_walls(),
        Surface::Roof(_) => f.for_roofs(),
        Surface::RoomFloor(_) => f.for_floors(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::arch::{Cabinet, Fixture, Furnishing, MaterialRef, Paint, Roof, SymbolMesh, SymbolPart};
    use cadcraft_doc::{Room, Wall};

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    fn wall(model: &mut ArchModel, a: (f64, f64), b: (f64, f64)) -> ArchId {
        model.push_wall(Wall::new(Vec2::new(a.0, a.1), Vec2::new(b.0, b.1), 0.2))
    }

    fn room(model: &mut ArchModel, p: Vec<Vec2>, level: u32) -> ArchId {
        model.push_room(Room { polygon: p, level, ..Default::default() })
    }

    fn wall_side(w: ArchId, side: Side) -> Surface {
        Surface::WallSide { wall: w, side }
    }

    /// A 4x4 room whose south wall runs west to east, so its left side faces the room.
    fn boxed() -> (ArchModel, ArchId, ArchId) {
        let mut m = ArchModel::default();
        let w = wall(&mut m, (0.0, 0.0), (4.0, 0.0));
        let r = room(&mut m, rect(0.0, 0.0, 4.0, 4.0), 0);
        (m, w, r)
    }

    fn info_of(m: &ArchModel, s: &Surface) -> SurfaceInfo {
        surfaces(m).into_iter().find(|i| &i.surface == s).unwrap()
    }

    #[test]
    fn wall_sides_find_their_rooms() {
        let (m, w, r) = boxed();
        assert_eq!(info_of(&m, &wall_side(w, Side::Left)).room, Some(r));
    }

    #[test]
    fn exterior_sides_have_no_room() {
        let (m, w, _) = boxed();
        assert_eq!(info_of(&m, &wall_side(w, Side::Right)).room, None);
    }

    #[test]
    fn a_partition_faces_two_rooms() {
        let mut m = ArchModel::default();
        let w = wall(&mut m, (4.0, 0.0), (4.0, 4.0));
        let a = room(&mut m, rect(0.0, 0.0, 3.9, 4.0), 0);
        let b = room(&mut m, rect(4.1, 0.0, 8.0, 4.0), 0);
        assert_eq!(info_of(&m, &wall_side(w, Side::Left)).room, Some(a));
        assert_eq!(info_of(&m, &wall_side(w, Side::Right)).room, Some(b));
    }

    #[test]
    fn surfaces_list_every_part_once() {
        let (mut m, w, r) = boxed();
        let roof = m.push_roof(Roof::default());
        let c = m.push_cabinet(Cabinet {
            width: 0.6,
            depth: 0.6,
            counter: true,
            pull: Some("pull-bar".into()),
            position: Vec2::new(2.0, 1.0),
            ..Default::default()
        });
        let c2 = m.push_cabinet(Cabinet { width: 0.6, depth: 0.6, position: Vec2::new(3.0, 1.0), ..Default::default() });
        let fx = m.push_fixture(Fixture::default());
        let f = m.push_furnishing(Furnishing { symbol: "none".into(), at: [2.0, 2.0], ..Default::default() });
        let keys: Vec<String> = surfaces(&m).iter().map(|i| i.surface.key()).collect();
        let cab = |cabinet, part| Surface::Cabinet { cabinet, part }.key();
        let want = vec![
            wall_side(w, Side::Left).key(),
            wall_side(w, Side::Right).key(),
            Surface::RoomFloor(r).key(),
            Surface::RoomCeiling(r).key(),
            Surface::Roof(roof).key(),
            cab(c, CabinetPart::Carcass),
            cab(c, CabinetPart::Front),
            cab(c, CabinetPart::Pull),
            cab(c2, CabinetPart::Carcass),
            cab(c2, CabinetPart::Front),
            Surface::Fixture(fx).key(),
            Surface::Furnishing { furnishing: f, part: 0 }.key(),
        ];
        // Counter parts exist only for a counter run's owner; compare ignoring them.
        let keys: Vec<String> = keys.into_iter().filter(|k| !k.ends_with("/counter")).collect();
        assert_eq!(keys, want);
        let mut dedup = keys.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(dedup.len(), keys.len());
        // The counter run owner does get its counter.
        let all = surfaces(&m);
        assert!(all.iter().any(|i| i.surface == Surface::Cabinet { cabinet: c, part: CabinetPart::Counter }));
    }

    #[test]
    fn effective_materials_follow_paints_and_finishes() {
        let (mut m, w, r) = boxed();
        assert_eq!(effective_material(&m, &wall_side(w, Side::Left)), "default:wall");
        assert_eq!(effective_material(&m, &Surface::RoomFloor(r)), "default:floor");
        assert_eq!(effective_material(&m, &Surface::RoomCeiling(r)), "default:ceiling");
        m.walls[0].finish = Some(Finish::Brick);
        m.rooms[0].floor_finish = Some(Finish::Wood);
        assert_eq!(effective_material(&m, &wall_side(w, Side::Right)), "finish:brick");
        assert_eq!(effective_material(&m, &wall_side(w, Side::Left)), "default:wall");
        assert_eq!(effective_material(&m, &Surface::RoomFloor(r)), "finish:wood");
        let s = wall_side(w, Side::Right);
        m.paints.insert(s.key(), Paint { material: Some(MaterialRef::Colour([0xc0, 0x40, 0x20])), tint: Some([1, 2, 3]), ..Default::default() });
        assert_eq!(effective_material(&m, &s), "colour:#c04020");
        assert_eq!(info_of(&m, &s).tint, Some([1, 2, 3]));
        // A tint alone keeps the material.
        m.paints.insert(s.key(), Paint { material: None, tint: Some([9, 9, 9]), ..Default::default() });
        assert_eq!(effective_material(&m, &s), "finish:brick");
    }

    /// Two rooms on level 0, each with a wall facing it, and one room and wall on level 1.
    fn two_rooms() -> (ArchModel, Vec<ArchId>) {
        let mut m = ArchModel::default();
        let w0 = wall(&mut m, (0.0, 0.0), (4.0, 0.0));
        let w1 = wall(&mut m, (10.0, 0.0), (14.0, 0.0));
        let w2 = m.push_wall(Wall { base_level: 1, ..Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2) });
        room(&mut m, rect(0.0, 0.0, 4.0, 4.0), 0);
        room(&mut m, rect(10.0, 0.0, 14.0, 4.0), 0);
        room(&mut m, rect(0.0, 0.0, 4.0, 4.0), 1);
        (m, vec![w0, w1, w2])
    }

    #[test]
    fn room_scope_takes_like_materials_in_the_room() {
        let (mut m, w) = two_rooms();
        let t = wall_side(w[0], Side::Left);
        // Room 0 holds its left wall side (default:wall) only; floor and ceiling differ.
        assert_eq!(select(&m, &t, Scope::Room).unwrap(), vec![t.clone()]);
        // Painting the ceiling and the wall alike makes them join.
        let ceiling = Surface::RoomCeiling(ArchId(0));
        let red = Paint { material: MaterialRef::parse("#c04020"), tint: None, ..Default::default() };
        m.paints.insert(ceiling.key(), red.clone());
        m.paints.insert(t.key(), red);
        let got = select(&m, &t, Scope::Room).unwrap();
        assert_eq!(got, vec![ceiling, t.clone()]);
        // The outside side has no room: just itself.
        let out = wall_side(w[0], Side::Right);
        assert_eq!(select(&m, &out, Scope::Room).unwrap(), vec![out]);
    }

    #[test]
    fn plan_scope_takes_every_like_material() {
        let (m, w) = two_rooms();
        let t = wall_side(w[0], Side::Left);
        let got = select(&m, &t, Scope::Plan).unwrap();
        // Six wall sides share default:wall.
        assert_eq!(got.len(), 6);
        assert!(got.windows(2).all(|p| p[0].key() < p[1].key()));
    }

    #[test]
    fn floor_scope_stays_on_the_level() {
        let (m, w) = two_rooms();
        let t = wall_side(w[0], Side::Left);
        let got = select(&m, &t, Scope::Floor).unwrap();
        assert_eq!(got.len(), 4);
        assert!(!got.contains(&wall_side(w[2], Side::Left)));
    }

    #[test]
    fn object_scope_takes_the_whole_object() {
        let (mut m, _, _) = boxed();
        let c = m.push_cabinet(Cabinet { counter: true, pull: Some("p".into()), ..Default::default() });
        let t = Surface::Cabinet { cabinet: c, part: CabinetPart::Front };
        assert_eq!(select(&m, &t, Scope::Object).unwrap().len(), 4);
        assert_eq!(select(&m, &t, Scope::Component).unwrap(), vec![t]);
    }

    #[test]
    fn select_objects_matches_like_pulls() {
        let mut m = ArchModel::default();
        let a = m.push_cabinet(Cabinet { pull: Some("bar".into()), ..Default::default() });
        let b = m.push_cabinet(Cabinet { pull: Some("bar".into()), ..Default::default() });
        let c = m.push_cabinet(Cabinet { pull: Some("knob".into()), ..Default::default() });
        let pull = |id| Surface::Cabinet { cabinet: id, part: CabinetPart::Pull };
        assert_eq!(select_objects(&m, &pull(a), Scope::Plan).unwrap(), vec![pull(a), pull(b)]);
        assert_eq!(select_objects(&m, &pull(c), Scope::Plan).unwrap(), vec![pull(c)]);
        assert_eq!(select_objects(&m, &pull(a), Scope::Component).unwrap(), vec![pull(a)]);
        let f0 = m.push_furnishing(Furnishing { item: "armchair".into(), ..Default::default() });
        let f1 = m.push_furnishing(Furnishing { item: "armchair".into(), ..Default::default() });
        let f2 = m.push_furnishing(Furnishing { item: "sofa".into(), ..Default::default() });
        let part0 = |id| Surface::Furnishing { furnishing: id, part: 0 };
        assert_eq!(select_objects(&m, &part0(f0), Scope::Plan).unwrap(), vec![part0(f0), part0(f1)]);
        assert!(!select_objects(&m, &part0(f0), Scope::Plan).unwrap().contains(&part0(f2)));
    }

    #[test]
    fn an_unknown_target_is_refused() {
        let m = ArchModel::default();
        let t = Surface::Roof(ArchId(7));
        let e = select(&m, &t, Scope::Plan).unwrap_err();
        assert!(e.contains("roof/r7"));
        assert!(select_objects(&m, &t, Scope::Plan).is_err());
        assert_eq!(Scope::parse("floor"), Some(Scope::Floor));
        assert_eq!(Scope::parse("x"), None);
    }

    #[test]
    fn finish_fits_follows_the_phase13_rules() {
        let wallside = wall_side(ArchId(0), Side::Left);
        assert!(finish_fits(&wallside, Finish::Siding));
        assert!(!finish_fits(&wallside, Finish::Shingles));
        assert!(finish_fits(&Surface::Roof(ArchId(0)), Finish::Shingles));
        assert!(!finish_fits(&Surface::Roof(ArchId(0)), Finish::Brick));
        assert!(finish_fits(&Surface::RoomFloor(ArchId(0)), Finish::Wood));
        assert!(!finish_fits(&Surface::RoomFloor(ArchId(0)), Finish::Siding));
        assert!(finish_fits(&Surface::Fixture(ArchId(0)), Finish::Brick));
    }

    #[test]
    fn surfaces_of_empty_model_is_empty() {
        let m = ArchModel::default();
        assert!(surfaces(&m).is_empty());
    }

    #[test]
    fn nan_wall_coordinates_do_not_crash() {
        let mut m = ArchModel::default();
        m.push_wall(Wall::new(Vec2::new(f64::NAN, 0.0), Vec2::new(1.0, f64::NAN), 0.2));
        let all = surfaces(&m);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].room, None);
        assert_eq!(all[1].room, None);
    }

    #[test]
    fn zero_length_wall_still_has_two_sides() {
        let mut m = ArchModel::default();
        m.push_wall(Wall::new(Vec2::new(1.0, 1.0), Vec2::new(1.0, 1.0), 0.2));
        let all = surfaces(&m);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn select_room_scope_on_roof_returns_only_roof() {
        let (mut m, _, _) = boxed();
        let roof = m.push_roof(Roof::default());
        let t = Surface::Roof(roof);
        assert_eq!(select(&m, &t, Scope::Room).unwrap(), vec![t]);
    }

    #[test]
    fn furnishing_without_symbol_has_one_part() {
        let mut m = ArchModel::default();
        let f = m.push_furnishing(Furnishing { symbol: "missing".into(), at: [0.0, 0.0], ..Default::default() });
        let all = surfaces(&m);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].surface, Surface::Furnishing { furnishing: f, part: 0 });
    }

    #[test]
    fn surfaces_are_deterministic() {
        let (mut m, _, _) = boxed();
        m.push_cabinet(Cabinet { pull: Some("p".into()), ..Default::default() });
        m.push_furnishing(Furnishing { symbol: "s".into(), ..Default::default() });
        let a = surfaces(&m);
        let b = surfaces(&m);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.surface.key(), y.surface.key());
        }
    }

    #[test]
    fn object_scope_wall_returns_both_sides() {
        let (m, w, _) = boxed();
        let t = wall_side(w, Side::Left);
        let got = select(&m, &t, Scope::Object).unwrap();
        assert_eq!(got.len(), 2);
        assert!(got.contains(&wall_side(w, Side::Left)));
        assert!(got.contains(&wall_side(w, Side::Right)));
    }

    #[test]
    fn object_scope_room_returns_floor_and_ceiling() {
        let (m, _, r) = boxed();
        let t = Surface::RoomFloor(r);
        let got = select(&m, &t, Scope::Object).unwrap();
        assert_eq!(got.len(), 2);
        assert!(got.contains(&Surface::RoomFloor(r)));
        assert!(got.contains(&Surface::RoomCeiling(r)));
    }

    #[test]
    fn cabinet_in_room_finds_room() {
        let (mut m, _, r) = boxed();
        let c = m.push_cabinet(Cabinet { position: Vec2::new(2.0, 2.0), depth: 0.6, ..Default::default() });
        let cab_surface = Surface::Cabinet { cabinet: c, part: CabinetPart::Carcass };
        assert_eq!(info_of(&m, &cab_surface).room, Some(r));
    }

    #[test]
    fn fixture_in_room_finds_room() {
        let (mut m, _, r) = boxed();
        let f = m.push_fixture(Fixture { position: Vec2::new(2.0, 2.0), depth: 0.5, ..Default::default() });
        assert_eq!(info_of(&m, &Surface::Fixture(f)).room, Some(r));
    }

    #[test]
    fn furnishing_in_room_finds_room() {
        let (mut m, _, r) = boxed();
        let f = m.push_furnishing(Furnishing { at: [2.0, 2.0], ..Default::default() });
        let surf = Surface::Furnishing { furnishing: f, part: 0 };
        assert_eq!(info_of(&m, &surf).room, Some(r));
    }

    #[test]
    fn room_with_hole_excludes_hole() {
        let mut m = ArchModel::default();
        let w = wall(&mut m, (0.0, 0.0), (4.0, 0.0));
        let r = m.push_room(Room { polygon: rect(0.0, 0.0, 4.0, 4.0), holes: vec![rect(1.5, 1.5, 2.5, 2.5)], level: 0, ..Default::default() });
        // Probe point is inside the room but outside the hole.
        let dir = Vec2::new(1.0, 0.0);
        let n = dir.perp();
        let mid = Vec2::new(2.0, 0.0);
        let probe = mid + n * (0.2 / 2.0 + PROBE);
        assert!(point_in_polygon(&m.rooms[0].polygon, probe));
        assert!(!point_in_polygon(&m.rooms[0].holes[0], probe));
        assert_eq!(info_of(&m, &wall_side(w, Side::Left)).room, Some(r));
        // A point inside the hole:
        let hole_center = Vec2::new(2.0, 2.0);
        assert_eq!(room_at(&m, 0, hole_center), None);
    }

    #[test]
    fn roof_covering_effective_material() {
        let (mut m, _, _) = boxed();
        let roof = m.push_roof(Roof::default());
        assert_eq!(effective_material(&m, &Surface::Roof(roof)), "default:roof");
        m.roofs[0].covering = Some(Finish::Shingles);
        assert_eq!(effective_material(&m, &Surface::Roof(roof)), "finish:shingles");
    }

    #[test]
    fn cabinet_front_style_material() {
        let (mut m, _, _) = boxed();
        let c = m.push_cabinet(Cabinet { door_style: cadcraft_doc::arch::FrontStyle::Shaker, ..Default::default() });
        let surf = Surface::Cabinet { cabinet: c, part: CabinetPart::Front };
        assert_eq!(effective_material(&m, &surf), "frontstyle:shaker");
    }

    #[test]
    fn cabinet_pull_material() {
        let (mut m, _, _) = boxed();
        let c = m.push_cabinet(Cabinet { pull: Some("pull-bar".into()), ..Default::default() });
        let surf = Surface::Cabinet { cabinet: c, part: CabinetPart::Pull };
        assert_eq!(effective_material(&m, &surf), "item:pull-bar");
    }

    #[test]
    fn select_objects_falls_through_for_walls() {
        let (m, w, _) = boxed();
        let t = wall_side(w, Side::Left);
        let mat = select(&m, &t, Scope::Plan).unwrap();
        let obj = select_objects(&m, &t, Scope::Plan).unwrap();
        assert_eq!(mat, obj);
    }

    #[test]
    fn nan_cabinet_position_no_room() {
        let (mut m, _, _) = boxed();
        let c = m.push_cabinet(Cabinet { position: Vec2::new(f64::NAN, f64::NAN), ..Default::default() });
        let surf = Surface::Cabinet { cabinet: c, part: CabinetPart::Carcass };
        assert_eq!(info_of(&m, &surf).room, None);
    }

    #[test]
    fn huge_coordinates() {
        let mut m = ArchModel::default();
        let big = 1e12;
        let w = wall(&mut m, (0.0, 0.0), (big, 0.0));
        let r = room(&mut m, rect(0.0, 0.0, big, big), 0);
        assert_eq!(info_of(&m, &wall_side(w, Side::Left)).room, Some(r));
        assert_eq!(info_of(&m, &wall_side(w, Side::Right)).room, None);
    }

    #[test]
    fn select_objects_furnishing_part_one_includes_target() {
        let mut m = ArchModel::default();
        let sym = SymbolMesh {
            key: "sym1".into(),
            parts: vec![SymbolPart { name: "a".into(), ..Default::default() }, SymbolPart { name: "b".into(), ..Default::default() }],
            ..Default::default()
        };
        m.symbols.insert("sym1".into(), sym);
        let f0 = m.push_furnishing(Furnishing { item: "chair".into(), symbol: "sym1".into(), ..Default::default() });
        let f1 = m.push_furnishing(Furnishing { item: "chair".into(), symbol: "sym1".into(), ..Default::default() });
        let part1_f0 = Surface::Furnishing { furnishing: f0, part: 1 };
        let got = select_objects(&m, &part1_f0, Scope::Plan).unwrap();
        assert!(got.contains(&Surface::Furnishing { furnishing: f0, part: 0 }));
        assert!(got.contains(&Surface::Furnishing { furnishing: f1, part: 0 }));
        assert!(got.contains(&part1_f0));
        assert_eq!(got.len(), 3);
    }
}
