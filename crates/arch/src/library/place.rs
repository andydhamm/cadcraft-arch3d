//! Where a library item goes: floor, wall, ceiling, counter, gap, or a swap on a cabinet.
//!
//! Pure: the engine calls [`place`] with the model and a request and applies the outcome.

use crate::casework_layout::gaps;
use crate::library::record::ItemKind;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchId, Cabinet, CabinetKind, CabinetPart, Furnishing, FurnishingHost, Placement};
use cadcraft_geom::Vec2;

/// Largest accepted item dimension in metres.
const MAX_SIZE: f64 = 100.0;
/// Standard counter height used when a counter item has no cabinet under it.
const COUNTER_HEIGHT: f64 = 0.914;
/// How far from a wall face a wall item may be clicked.
const WALL_REACH: f64 = 0.3;
const EPS: f64 = 1e-9;

/// What the user asked to place, and where they clicked.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceRequest {
    /// The item's placement kind.
    pub placement: Placement,
    /// The item's symbol type ("sink", "faucet", "pull", ...).
    pub symbol_type: String,
    /// The item's kind.
    pub kind: ItemKind,
    /// Width x, depth y, height z.
    pub size: [f64; 3],
    /// Catalog elevation for wall items.
    pub elevation: Option<f64>,
    /// Counter cut-out width x depth.
    pub cutout: Option<[f64; 2]>,
    /// The clicked plan point.
    pub at: [f64; 2],
    /// The level index.
    pub level: u32,
    /// An explicit rotation in degrees (overrides the computed one for rules 5-7).
    pub rotation: Option<f64>,
}

impl Default for PlaceRequest {
    fn default() -> Self {
        PlaceRequest {
            placement: Placement::Floor,
            symbol_type: String::new(),
            kind: ItemKind::Symbol,
            size: [1.0, 1.0, 1.0],
            elevation: None,
            cutout: None,
            at: [0.0, 0.0],
            level: 0,
            rotation: None,
        }
    }
}

/// The result of resolving a placement.
#[derive(Clone, Debug, PartialEq)]
pub enum PlaceOutcome {
    /// Create a furnishing here.
    Place { at: [f64; 2], rotation: f64, elevation: f64, host: FurnishingHost },
    /// Replace a part of an existing cabinet.
    Swap { cabinet: ArchId, part: CabinetPart },
}

fn norm_deg(a: f64) -> f64 {
    let r = a.rem_euclid(360.0);
    if r >= 360.0 { 0.0 } else { r }
}

fn dir(deg: f64) -> (f64, f64) {
    let r = deg.to_radians();
    (r.cos(), r.sin())
}

/// Rotate (u, v) by `deg` counter-clockwise.
fn rot(deg: f64, u: f64, v: f64) -> (f64, f64) {
    let (c, s) = dir(deg);
    (c * u - s * v, s * u + c * v)
}

/// Coordinates of plan point `p` in the frame at `origin` rotated by `deg`.
fn to_frame(origin: [f64; 2], deg: f64, p: [f64; 2]) -> (f64, f64) {
    let (c, s) = dir(deg);
    let (dx, dy) = (p[0] - origin[0], p[1] - origin[1]);
    (c * dx + s * dy, -s * dx + c * dy)
}

fn cabinet_frame(c: &Cabinet, p: [f64; 2]) -> (f64, f64) {
    to_frame([c.position.x, c.position.y], c.rotation, p)
}

fn cabinet_point(c: &Cabinet, u: f64, v: f64) -> [f64; 2] {
    let (x, y) = rot(c.rotation, u, v);
    [c.position.x + x, c.position.y + y]
}

fn cabinet_contains(c: &Cabinet, p: [f64; 2], extra_depth: f64) -> bool {
    let (u, v) = cabinet_frame(c, p);
    u.abs() <= c.width / 2.0 + EPS && v >= -EPS && v <= c.depth + extra_depth + EPS
}

fn furnishing_contains(f: &Furnishing, p: [f64; 2]) -> bool {
    let (u, v) = to_frame(f.at, f.rotation, p);
    u.abs() <= f.size[0] / 2.0 + EPS && v.abs() <= f.size[1] / 2.0 + EPS
}

fn valid(r: &PlaceRequest) -> Result<(), String> {
    let sized = r.size.iter().all(|s| s.is_finite() && *s > 0.0 && *s <= MAX_SIZE);
    if !sized {
        return Err("item size must be finite, positive and at most 100 m".into());
    }
    if !r.at.iter().all(|v| v.is_finite()) {
        return Err("the placement point must be finite".into());
    }
    if r.elevation.is_some_and(|e| !e.is_finite()) || r.rotation.is_some_and(|e| !e.is_finite()) {
        return Err("elevation and rotation must be finite".into());
    }
    if r.cutout.is_some_and(|c| !c.iter().all(|v| v.is_finite() && *v > 0.0 && *v <= MAX_SIZE)) {
        return Err("the cut-out must be finite, positive and at most 100 m".into());
    }
    Ok(())
}

/// Resolve where the requested item goes (rules 1-7 in the module docs of the design).
pub fn place(model: &ArchModel, r: &PlaceRequest) -> Result<PlaceOutcome, String> {
    valid(r)?;
    let at = r.at;

    // 1. cabinet parts swap onto a cabinet.
    if r.kind == ItemKind::FrontStyle || r.symbol_type == "pull" {
        let mut cabs: Vec<&Cabinet> = model.cabinets.iter().filter(|c| c.level == r.level).collect();
        cabs.sort_by_key(|c| c.id);
        let hit = cabs.into_iter().find(|c| c.width.is_finite() && c.depth.is_finite() && cabinet_contains(c, at, 0.05));
        return match hit {
            Some(c) => {
                let part = if r.kind == ItemKind::FrontStyle { CabinetPart::Front } else { CabinetPart::Pull };
                Ok(PlaceOutcome::Swap { cabinet: c.id, part })
            }
            None => Err(format!("a cabinet part goes on a cabinet: no cabinet under ({}, {})", at[0], at[1])),
        };
    }

    // 2. a faucet snaps to the back of a sink.
    if r.symbol_type == "faucet" {
        let mut sinks: Vec<&Furnishing> = model
            .furnishings
            .iter()
            .filter(|f| f.level == r.level && (f.symbol_type == "sink" || f.symbol_type == "lavatory"))
            .filter(|f| f.size.iter().chain(f.at.iter()).chain([&f.rotation, &f.elevation]).all(|v| v.is_finite()))
            .collect();
        sinks.sort_by_key(|f| f.id);
        if let Some(s) = sinks.into_iter().find(|f| furnishing_contains(f, at)) {
            let (x, y) = rot(s.rotation, 0.0, s.size[1] / 2.0 - r.size[1] / 2.0);
            return Ok(PlaceOutcome::Place {
                at: [s.at[0] + x, s.at[1] + y],
                rotation: norm_deg(s.rotation),
                elevation: s.elevation + s.size[2],
                host: FurnishingHost::Sink { furnishing: s.id },
            });
        }
    }

    // 3. counter items drop into / stand on a base cabinet's counter.
    if r.cutout.is_some() || r.placement == Placement::Counter {
        let mut cabs: Vec<&Cabinet> = model
            .cabinets
            .iter()
            .filter(|c| c.level == r.level && c.kind == CabinetKind::Base && c.counter && c.width.is_finite() && c.depth.is_finite())
            .collect();
        cabs.sort_by_key(|c| c.id);
        if let Some(c) = cabs.into_iter().find(|c| cabinet_contains(c, at, 0.0)) {
            // Cabinet.height is the carcass; the counter sits on top of it (inspect's counterTop).
            let top = c.elevation + c.height + c.counter_thickness;
            let elevation = if r.cutout.is_some() { top - r.size[2] + 0.01 } else { top };
            return Ok(PlaceOutcome::Place {
                at: cabinet_point(c, 0.0, c.depth / 2.0),
                rotation: norm_deg(c.rotation + 180.0),
                elevation,
                host: FurnishingHost::Cabinet { cabinet: c.id },
            });
        }
        return Ok(PlaceOutcome::Place { at, rotation: norm_deg(r.rotation.unwrap_or(0.0)), elevation: COUNTER_HEIGHT, host: FurnishingHost::None });
    }

    // 4. a floor item that fits under the counter takes a gap between cabinets.
    if r.placement == Placement::Floor {
        for g in gaps(model) {
            if g.level != r.level || g.width < r.size[0] - 1e-6 {
                continue;
            }
            let Some(left) = model.cabinets.iter().find(|c| c.id == g.left) else { continue };
            // The counter's underside is the top of the carcass (Cabinet.height does not include the counter).
            let underside = left.elevation + left.height;
            if r.size[2] > underside + EPS || (r.size[1] - g.depth).abs() > 0.1 + EPS {
                continue;
            }
            let (u, v) = to_frame([g.centre.x, g.centre.y], g.rotation, at);
            if u.abs() <= g.width / 2.0 + EPS && v.abs() <= g.depth / 2.0 + EPS {
                let (x, y) = rot(g.rotation, 0.0, r.size[1] / 2.0);
                return Ok(PlaceOutcome::Place {
                    at: [g.back_mid.x + x, g.back_mid.y + y],
                    rotation: norm_deg(g.rotation + 180.0),
                    elevation: 0.0,
                    host: FurnishingHost::Gap { left: g.left, right: g.right },
                });
            }
        }
    }

    match r.placement {
        // 5. wall items back onto the nearest wall face.
        Placement::Wall => wall_item(model, r),
        // 6. ceiling items hang from the room's finished ceiling (its own height, less its finish
        // stack), or from a soffit's underside over the point (phase 16: `ceiling::hang_z`).
        Placement::Ceiling => {
            let h = crate::ceiling::hang_z(model, r.level, Vec2::new(at[0], at[1])).ok_or("a ceiling item goes in a room")?;
            let level = model.level(r.level).ok_or_else(|| format!("no level {}", r.level))?;
            let elevation = h - level.elevation - r.size[2];
            if !elevation.is_finite() {
                return Err("a ceiling item goes in a room".into());
            }
            Ok(PlaceOutcome::Place { at, rotation: norm_deg(r.rotation.unwrap_or(0.0)), elevation, host: FurnishingHost::None })
        }
        // 7. otherwise at the click.
        _ => Ok(PlaceOutcome::Place { at, rotation: norm_deg(r.rotation.unwrap_or(0.0)), elevation: 0.0, host: FurnishingHost::None }),
    }
}

fn wall_item(model: &ArchModel, r: &PlaceRequest) -> Result<PlaceOutcome, String> {
    let p = Vec2::new(r.at[0], r.at[1]);
    // (distance, wall id, side index, foot, outward normal)
    let mut best: Option<(f64, ArchId, usize, Vec2, Vec2)> = None;
    for w in model.walls.iter().filter(|w| crate::room::wall_on_level(model, w, r.level) && !w.divider) {
        let Some(faces) = crate::join::face_lines(w) else { continue };
        let Some(dir) = crate::wall::wall_dir(w) else { continue };
        for (i, (a, b)) in faces.iter().enumerate() {
            let ab = Vec2::new(b.x - a.x, b.y - a.y);
            let len2 = ab.x * ab.x + ab.y * ab.y;
            if !(len2 > EPS) {
                continue;
            }
            let t = (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0);
            let foot = Vec2::new(a.x + ab.x * t, a.y + ab.y * t);
            let d = ((p.x - foot.x).powi(2) + (p.y - foot.y).powi(2)).sqrt();
            let perp = Vec2::new(-dir.y, dir.x);
            let out = if i == 0 { perp } else { Vec2::new(-perp.x, -perp.y) };
            let better = match &best {
                None => true,
                Some((bd, bid, bi, ..)) => d < *bd - 1e-12 || ((d - *bd).abs() <= 1e-12 && (w.id, i) < (*bid, *bi)),
            };
            if better {
                best = Some((d, w.id, i, foot, out));
            }
        }
    }
    let Some((d, wall, _, foot, out)) = best.filter(|b| b.0 <= WALL_REACH + EPS) else {
        return Err(format!("no wall face within 0.3 m of ({}, {})", r.at[0], r.at[1]));
    };
    let toward = Vec2::new(p.x - foot.x, p.y - foot.y);
    // Face normal, signed towards the click (the outward normal when it is on the face).
    let n = if d > 1e-9 && toward.x * out.x + toward.y * out.y < 0.0 { Vec2::new(-out.x, -out.y) } else { out };
    let half = r.size[1] / 2.0;
    Ok(PlaceOutcome::Place {
        at: [foot.x + n.x * half, foot.y + n.y * half],
        rotation: norm_deg(r.rotation.unwrap_or_else(|| n.y.atan2(n.x).to_degrees() + 90.0)),
        elevation: r.elevation.unwrap_or(0.0),
        host: FurnishingHost::Wall { wall },
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{Level, Room, Wall};

    fn req(placement: Placement, symbol_type: &str, size: [f64; 3], at: [f64; 2]) -> PlaceRequest {
        PlaceRequest { placement, symbol_type: symbol_type.into(), size, at, ..Default::default() }
    }

    fn cab(id: u32, x: f64) -> Cabinet {
        Cabinet {
            id: ArchId(id),
            kind: CabinetKind::Base,
            position: Vec2::new(x, 0.0),
            width: 0.6,
            depth: 0.6,
            height: 0.876,
            counter: true,
            counter_thickness: 0.038,
            counter_overhang: 0.025,
            ..Default::default()
        }
    }

    fn model_with(cabs: Vec<Cabinet>) -> ArchModel {
        ArchModel { cabinets: cabs, levels: vec![Level::default()], ..Default::default() }
    }

    fn wall_model() -> ArchModel {
        let w = Wall { id: ArchId(1), start: Vec2::new(0.0, 0.0), end: Vec2::new(5.0, 0.0), thickness: 0.2, height: 2.4, ..Default::default() };
        ArchModel { walls: vec![w], levels: vec![Level::default()], ..Default::default() }
    }

    fn placed(o: Result<PlaceOutcome, String>) -> ([f64; 2], f64, f64, FurnishingHost) {
        match o.unwrap() {
            PlaceOutcome::Place { at, rotation, elevation, host } => (at, rotation, elevation, host),
            other => panic!("expected Place, got {other:?}"),
        }
    }

    #[test]
    fn a_floor_item_stays_at_the_click() {
        let m = ArchModel::default();
        let (at, rot, el, host) = placed(place(&m, &req(Placement::Floor, "chair", [0.5, 0.5, 0.9], [1.0, 2.0])));
        assert_eq!((at, rot, el, host), ([1.0, 2.0], 0.0, 0.0, FurnishingHost::None));
    }

    #[test]
    fn a_wall_item_backs_onto_the_nearest_face() {
        let m = wall_model();
        let mut r = req(Placement::Wall, "light", [0.4, 0.1, 0.3], [2.0, 0.25]);
        r.elevation = Some(1.5);
        let (at, rot, el, host) = placed(place(&m, &r));
        // Left face at y = 0.1, item back there; centre 0.05 further out.
        assert!((at[0] - 2.0).abs() < 1e-9 && (at[1] - 0.15).abs() < 1e-9, "{at:?}");
        assert!((rot - 180.0).abs() < 1e-9, "{rot}");
        assert_eq!(el, 1.5);
        assert_eq!(host, FurnishingHost::Wall { wall: ArchId(1) });
        // The other side faces -y.
        let (at, rot, ..) = placed(place(&m, &req(Placement::Wall, "light", [0.4, 0.1, 0.3], [2.0, -0.25])));
        assert!((at[1] + 0.15).abs() < 1e-9 && rot.abs() < 1e-9, "{at:?} {rot}");
    }

    #[test]
    fn a_wall_item_too_far_is_refused() {
        let m = wall_model();
        let e = place(&m, &req(Placement::Wall, "light", [0.4, 0.1, 0.3], [2.0, 1.0])).unwrap_err();
        assert!(e.contains("no wall face within 0.3 m"), "{e}");
    }

    fn room_model() -> ArchModel {
        let poly = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        ArchModel { rooms: vec![Room { id: ArchId(0), polygon: poly, ..Default::default() }], levels: vec![Level::default()], ..Default::default() }
    }

    #[test]
    fn a_ceiling_item_hangs_from_the_ceiling() {
        let m = room_model();
        let (at, _, el, host) = placed(place(&m, &req(Placement::Ceiling, "light", [0.5, 0.5, 0.3], [2.0, 2.0])));
        assert_eq!(at, [2.0, 2.0]);
        assert!((el - 2.1).abs() < 1e-9, "{el}");
        assert_eq!(host, FurnishingHost::None);
    }

    /// Phase 14 said a ceiling item hangs from "the room's ceiling height": a room's own height
    /// counts (it used to take the level's), on an upper level too (elevation stays relative).
    #[test]
    fn ceiling_item_hangs_from_the_room_ceiling_height() {
        let mut m = room_model();
        m.rooms[0].ceiling_height = Some(2.7);
        let (_, _, el, _) = placed(place(&m, &req(Placement::Ceiling, "light", [0.5, 0.5, 0.3], [2.0, 2.0])));
        assert!((el - 2.4).abs() < 1e-9, "{el}");
        // On a level at 2.7 the elevation is still measured from that level's floor.
        m.levels.push(Level { index: 1, elevation: 2.7, ..Level::default() });
        let poly = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        m.rooms.push(Room { id: ArchId(1), level: 1, polygon: poly, ..Default::default() });
        let mut r = req(Placement::Ceiling, "light", [0.5, 0.5, 0.3], [2.0, 2.0]);
        r.level = 1;
        let (_, _, el, _) = placed(place(&m, &r));
        assert!((el - 2.1).abs() < 1e-9, "{el}");
        // A finish stack lowers it.
        m.rooms[0].ceiling_stack =
            vec![cadcraft_doc::arch::CeilingLayer { material: Some("drywall".into()), thickness: 0.0127, ..Default::default() }];
        let (_, _, el, _) = placed(place(&m, &req(Placement::Ceiling, "light", [0.5, 0.5, 0.3], [2.0, 2.0])));
        assert!((el - (2.7 - 0.0127 - 0.3)).abs() < 1e-9, "{el}");
    }

    #[test]
    fn a_ceiling_item_outside_rooms_is_refused() {
        let m = room_model();
        let e = place(&m, &req(Placement::Ceiling, "light", [0.5, 0.5, 0.3], [9.0, 9.0])).unwrap_err();
        assert_eq!(e, "a ceiling item goes in a room");
    }

    fn sink_req(at: [f64; 2]) -> PlaceRequest {
        let mut r = req(Placement::Counter, "sink", [0.5, 0.4, 0.2], at);
        r.cutout = Some([0.45, 0.35]);
        r
    }

    #[test]
    fn a_sink_drops_into_a_base_cabinet() {
        let m = model_with(vec![cab(0, 0.0)]);
        let (at, rot, el, host) = placed(place(&m, &sink_req([0.1, 0.2])));
        assert!((at[0]).abs() < 1e-9 && (at[1] - 0.3).abs() < 1e-9, "{at:?}");
        assert!((rot - 180.0).abs() < 1e-9);
        // The counter top is the carcass (0.876) plus the counter (0.038): 0.914, as inspect's counterTop.
        assert!((el - (0.914 - 0.2 + 0.01)).abs() < 1e-9, "{el}");
        assert_eq!(host, FurnishingHost::Cabinet { cabinet: ArchId(0) });
    }

    #[test]
    fn a_cooktop_drops_in_and_a_faucet_snaps_to_the_sink_back() {
        let mut m = model_with(vec![cab(0, 0.0)]);
        let mut cooktop = req(Placement::Counter, "cooktop", [0.5, 0.5, 0.05], [0.0, 0.3]);
        cooktop.cutout = Some([0.45, 0.45]);
        let (.., host) = placed(place(&m, &cooktop));
        assert_eq!(host, FurnishingHost::Cabinet { cabinet: ArchId(0) });
        let (at, rot, el, _) = placed(place(&m, &sink_req([0.0, 0.3])));
        m.furnishings.push(Furnishing {
            id: ArchId(0),
            symbol_type: "sink".into(),
            at,
            rotation: rot,
            elevation: el,
            size: [0.5, 0.4, 0.2],
            ..Default::default()
        });
        let faucet = req(Placement::Counter, "faucet", [0.1, 0.1, 0.25], [0.0, 0.3]);
        let (fat, frot, fel, fhost) = placed(place(&m, &faucet));
        // Sink rotated 180: its back (local +y) points to -y in the plan, towards the wall at y = 0.
        assert!((fat[0]).abs() < 1e-9 && (fat[1] - (0.3 - 0.15)).abs() < 1e-9, "{fat:?}");
        assert!((frot - 180.0).abs() < 1e-9);
        assert!((fel - (el + 0.2)).abs() < 1e-9);
        assert_eq!(fhost, FurnishingHost::Sink { furnishing: ArchId(0) });
    }

    #[test]
    fn a_counter_item_stands_on_the_counter_top() {
        // Cabinet.height is the carcass only; the counter sits on it (inspect: counterTop = elevation + height + thickness).
        let m = model_with(vec![cab(0, 0.0)]);
        let (.., el, host) = placed(place(&m, &req(Placement::Counter, "lamp", [0.2, 0.2, 0.4], [0.1, 0.3])));
        assert!((el - 0.914).abs() < 1e-9, "{el}");
        assert_eq!(host, FurnishingHost::Cabinet { cabinet: ArchId(0) });
    }

    #[test]
    fn a_dishwasher_as_tall_as_the_carcass_fits_under_the_counter() {
        // The sample dishwasher is 0.876 m, the carcass height: the counter's underside is 0.876, not 0.876 - 0.038.
        let m = model_with(vec![cab(0, 0.0), cab(1, 1.2)]);
        let r = req(Placement::Floor, "dishwasher", [0.6, 0.6, 0.876], [0.6, 0.3]);
        let (.., host) = placed(place(&m, &r));
        assert_eq!(host, FurnishingHost::Gap { left: ArchId(0), right: ArchId(1) });
    }

    #[test]
    fn a_counter_item_without_a_cabinet_stands_at_counter_height() {
        let m = ArchModel::default();
        let (at, _, el, host) = placed(place(&m, &req(Placement::Counter, "lamp", [0.2, 0.2, 0.4], [3.0, 3.0])));
        assert_eq!((at, el, host), ([3.0, 3.0], 0.914, FurnishingHost::None));
    }

    #[test]
    fn a_dishwasher_takes_a_gap_of_its_width() {
        let m = model_with(vec![cab(0, 0.0), cab(1, 1.2)]);
        // Gap spans x 0.3..0.9 (width 0.6), depth 0.6, back at y = 0.
        let r = req(Placement::Floor, "dishwasher", [0.6, 0.6, 0.82], [0.6, 0.3]);
        let (at, rot, el, host) = placed(place(&m, &r));
        assert!((at[0] - 0.6).abs() < 1e-9 && (at[1] - 0.3).abs() < 1e-9, "{at:?}");
        assert!((rot - 180.0).abs() < 1e-9);
        assert_eq!(el, 0.0);
        assert_eq!(host, FurnishingHost::Gap { left: ArchId(0), right: ArchId(1) });
    }

    #[test]
    fn a_range_too_tall_for_a_gap_is_not_hosted() {
        let m = model_with(vec![cab(0, 0.0), cab(1, 1.2)]);
        let r = req(Placement::Floor, "range", [0.6, 0.6, 0.95], [0.6, 0.3]);
        let (at, _, _, host) = placed(place(&m, &r));
        assert_eq!((at, host), ([0.6, 0.3], FurnishingHost::None));
    }

    #[test]
    fn a_pull_over_a_cabinet_swaps() {
        let m = model_with(vec![cab(4, 0.0)]);
        let mut r = req(Placement::Floor, "pull", [0.1, 0.02, 0.02], [0.1, 0.3]);
        assert_eq!(place(&m, &r).unwrap(), PlaceOutcome::Swap { cabinet: ArchId(4), part: CabinetPart::Pull });
        r.kind = ItemKind::FrontStyle;
        r.symbol_type = String::new();
        assert_eq!(place(&m, &r).unwrap(), PlaceOutcome::Swap { cabinet: ArchId(4), part: CabinetPart::Front });
    }

    #[test]
    fn a_pull_over_nothing_is_refused() {
        let m = model_with(vec![cab(4, 0.0)]);
        let r = req(Placement::Floor, "pull", [0.1, 0.02, 0.02], [5.0, 5.0]);
        let e = place(&m, &r).unwrap_err();
        assert!(e.starts_with("a cabinet part goes on a cabinet"), "{e}");
    }
}
