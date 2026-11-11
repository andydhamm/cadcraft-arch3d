//! Pure layout of cabinet fronts, pulls, counter runs and the gaps between base cabinets (phase 14).
//!
//! A cabinet's frame: the origin is the centre of its back edge, `u` runs along its width and `y`
//! towards the front; plan point = position + R(rotation)·(u, y).

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchId, Cabinet, CabinetKind, Fronts, FurnishingHost};
use cadcraft_geom::Vec2;

const GAP: f64 = 0.003;
const EPS: f64 = 1e-6;

/// What a front is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontKind {
    /// Hinged door.
    Door,
    /// Drawer front.
    Drawer,
    /// Fixed panel (no pull) in place of a drawer or door strip.
    False,
}

/// Which side a door is hinged on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hinge {
    /// Hinged on the left.
    Left,
    /// Hinged on the right.
    Right,
}

/// One front of a cabinet.
#[derive(Clone, Debug, PartialEq)]
pub struct FrontInfo {
    /// What it is.
    pub kind: FrontKind,
    /// `u0, v0, u1, v1`: u along the width in the cabinet frame, v above the level's floor.
    pub rect: [f64; 4],
    /// The hinge side of a door.
    pub hinge: Option<Hinge>,
    /// `(u, v)` of the pull's centre.
    pub pull: Option<[f64; 2]>,
}

/// The furnishing hosted by `c` that has a cut-out: its id, `at` and the cut-out size.
pub fn hosted_cutout(model: &ArchModel, c: &Cabinet) -> Option<(ArchId, [f64; 2], [f64; 2])> {
    model
        .furnishings
        .iter()
        .find(|f| f.cutout.is_some() && f.host == (FurnishingHost::Cabinet { cabinet: c.id }))
        .and_then(|f| f.cutout.map(|co| (f.id, f.at, co)))
}

fn hinge_of(i: usize, n: usize) -> Hinge {
    if n == 1 || i % 2 == 0 { Hinge::Left } else { Hinge::Right }
}

/// The fronts of a cabinet, in the order arch3d builds them.
pub fn fronts(model: &ArchModel, c: &Cabinet) -> Vec<FrontInfo> {
    let toe = if c.toe_kick { 0.1 } else { 0.0 };
    let usable = c.height - toe;
    let e = c.elevation;
    let n_doors = c.doors.max(1) as usize;
    let n_drawers = c.drawers.max(1) as usize;
    let cutout = hosted_cutout(model, c).is_some();
    let mut out: Vec<FrontInfo> = Vec::new();

    let mut push = |kind: FrontKind, x0: f64, x1: f64, z0: f64, z1: f64, hinge: Option<Hinge>| {
        if !(x0.is_finite() && x1.is_finite() && z0.is_finite() && z1.is_finite()) || x1 <= x0 || z1 <= z0 {
            return;
        }
        let (v0, v1) = (e + z0, e + z1);
        let pull = if c.pull.is_none() {
            None
        } else {
            match kind {
                FrontKind::False => None,
                FrontKind::Drawer => Some([(x0 + x1) / 2.0, (v0 + v1) / 2.0]),
                FrontKind::Door => {
                    let u = if hinge == Some(Hinge::Right) { x0 + 0.05 } else { x1 - 0.05 };
                    let v = if c.kind == CabinetKind::Wall { v0 + 0.075 } else { v1 - 0.075 };
                    Some([u, v])
                }
            }
        };
        out.push(FrontInfo { kind, rect: [x0, v0, x1, v1], hinge, pull });
    };
    let (l, r) = (-c.width / 2.0 + GAP, c.width / 2.0 - GAP);
    let door_w = (c.width - GAP * (n_doors as f64 + 1.0)) / n_doors as f64;

    match c.fronts {
        Fronts::Open => {}
        Fronts::Doors => {
            let door_top = if cutout { usable - 0.15 - GAP } else { usable };
            for i in 0..n_doors {
                let x0 = -c.width / 2.0 + GAP + i as f64 * (door_w + GAP);
                push(FrontKind::Door, x0, x0 + door_w, toe, door_top, Some(hinge_of(i, n_doors)));
            }
            if cutout {
                push(FrontKind::False, l, r, usable - 0.15, usable, None);
            }
        }
        Fronts::Drawers => {
            let h = usable / n_drawers as f64;
            for i in 0..n_drawers {
                let kind = if i == 0 && cutout { FrontKind::False } else { FrontKind::Drawer };
                push(kind, l, r, usable - (i as f64 + 1.0) * h, usable - i as f64 * h, None);
            }
        }
        Fronts::DrawerOverDoor => {
            let h = 0.15_f64.min(usable / n_drawers as f64);
            // The doors stand side by side under the drawers, each up to the lowest drawer (phase 13
            // divided this by the door count, so two doors were half height under a bare band).
            let door_top = usable - h * n_drawers as f64;
            for i in 0..n_drawers {
                let kind = if i == 0 && cutout { FrontKind::False } else { FrontKind::Drawer };
                push(kind, l, r, usable - (i as f64 + 1.0) * h, usable - i as f64 * h, None);
            }
            for i in 0..n_doors {
                let x0 = -c.width / 2.0 + GAP + i as f64 * (door_w + GAP);
                push(FrontKind::Door, x0, x0 + door_w, toe, door_top, Some(hinge_of(i, n_doors)));
            }
        }
    }
    out
}

/// A counter top running over neighbouring base cabinets.
#[derive(Clone, Debug, PartialEq)]
pub struct CounterRun {
    /// The first cabinet along the run.
    pub owner: ArchId,
    /// Every cabinet in the run, left to right.
    pub cabinets: Vec<ArchId>,
    /// Furnishings standing in bridged gaps.
    pub bridged: Vec<ArchId>,
    /// Level index.
    pub level: u32,
    /// The owner's back-edge centre.
    pub origin: Vec2,
    /// The owner's rotation, degrees.
    pub rotation: f64,
    /// Start along the run in the owner's frame.
    pub u0: f64,
    /// End along the run in the owner's frame.
    pub u1: f64,
    /// Depth of the cabinets.
    pub depth: f64,
    /// Counter top above the level floor.
    pub top: f64,
    /// Counter thickness.
    pub thickness: f64,
    /// Overhang at the front.
    pub overhang: f64,
}

/// A gap between two in-line base cabinets.
#[derive(Clone, Debug, PartialEq)]
pub struct CabinetGap {
    /// Left cabinet.
    pub left: ArchId,
    /// Right cabinet.
    pub right: ArchId,
    /// Plan centre of the gap.
    pub centre: Vec2,
    /// Gap width.
    pub width: f64,
    /// Cabinet depth.
    pub depth: f64,
    /// Rotation, degrees.
    pub rotation: f64,
    /// Level index.
    pub level: u32,
    /// Middle of the gap on the back line.
    pub back_mid: Vec2,
}

fn same_angle(a: f64, b: f64) -> bool {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d) <= EPS
}

fn dir(rot: f64) -> (f64, f64) {
    let a = rot.to_radians();
    (a.cos(), a.sin())
}

fn finite(c: &Cabinet) -> bool {
    [c.position.x, c.position.y, c.rotation, c.width, c.depth, c.height, c.elevation, c.counter_thickness, c.counter_overhang]
        .iter()
        .all(|v| v.is_finite())
}

fn in_line(a: &Cabinet, b: &Cabinet) -> bool {
    if a.level != b.level || !same_angle(a.rotation, b.rotation) || (a.depth - b.depth).abs() > EPS {
        return false;
    }
    let (dx, dy) = dir(a.rotation);
    let (px, py) = (b.position.x - a.position.x, b.position.y - a.position.y);
    (dx * py - dy * px).abs() <= EPS
}

/// Groups base cabinets accepted by `pre` into lines (members compatible per `ok` with the line's
/// first member), each sorted by `u`: `(cabinet index, u of its back centre)`.
fn lines(model: &ArchModel, ok: impl Fn(&Cabinet, &Cabinet) -> bool, pre: impl Fn(&Cabinet) -> bool) -> Vec<Vec<(usize, f64)>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut order: Vec<usize> = (0..model.cabinets.len()).collect();
    order.sort_by_key(|&i| model.cabinets.get(i).map(|c| c.id));
    for i in order {
        let Some(c) = model.cabinets.get(i) else { continue };
        if c.kind != CabinetKind::Base || !finite(c) || !pre(c) {
            continue;
        }
        let slot = groups.iter_mut().find(|g| g.first().and_then(|&j| model.cabinets.get(j)).is_some_and(|f| in_line(f, c) && ok(f, c)));
        match slot {
            Some(g) => g.push(i),
            None => groups.push(vec![i]),
        }
    }
    groups
        .into_iter()
        .filter_map(|g| {
            let f = model.cabinets.get(*g.first()?)?;
            let (dx, dy) = dir(f.rotation);
            let mut v: Vec<(usize, f64)> = g
                .iter()
                .filter_map(|&i| {
                    let c = model.cabinets.get(i)?;
                    Some((i, dx * (c.position.x - f.position.x) + dy * (c.position.y - f.position.y)))
                })
                .collect();
            v.sort_by(|a, b| {
                a.1.total_cmp(&b.1).then_with(|| {
                    let ia = model.cabinets.get(a.0).map(|c| c.id);
                    let ib = model.cabinets.get(b.0).map(|c| c.id);
                    ia.cmp(&ib)
                })
            });
            Some(v)
        })
        .collect()
}

/// Counter runs: touching (or bridged) in-line base cabinets with a matching counter.
pub fn counter_runs(model: &ArchModel) -> Vec<CounterRun> {
    let top = |c: &Cabinet| c.elevation + c.height;
    let ok = |a: &Cabinet, b: &Cabinet| (top(a) - top(b)).abs() <= EPS && (a.counter_thickness - b.counter_thickness).abs() <= EPS;
    let mut runs = Vec::new();
    for line in lines(model, ok, |c| c.counter) {
        let mut cur: Vec<(usize, f64)> = Vec::new();
        let mut bridged: Vec<ArchId> = Vec::new();
        let flush = |cur: &mut Vec<(usize, f64)>, bridged: &mut Vec<ArchId>, runs: &mut Vec<CounterRun>| {
            if let (Some(&(fi, fu)), Some(&(li, lu))) = (cur.first(), cur.last())
                && let (Some(f), Some(l)) = (model.cabinets.get(fi), model.cabinets.get(li))
            {
                let mut b = std::mem::take(bridged);
                b.sort();
                b.dedup();
                runs.push(CounterRun {
                    owner: f.id,
                    cabinets: cur.iter().filter_map(|&(i, _)| model.cabinets.get(i).map(|c| c.id)).collect(),
                    bridged: b,
                    level: f.level,
                    origin: f.position,
                    rotation: f.rotation,
                    u0: -f.width / 2.0,
                    u1: lu + l.width / 2.0 - fu,
                    depth: f.depth,
                    top: top(f),
                    thickness: f.counter_thickness,
                    overhang: f.counter_overhang,
                });
            }
            cur.clear();
            bridged.clear();
        };
        for &(i, u) in &line {
            let Some(c) = model.cabinets.get(i) else { continue };
            if let Some(&(pi, pu)) = cur.last()
                && let Some(p) = model.cabinets.get(pi)
            {
                let start = u - c.width / 2.0;
                let end = pu + p.width / 2.0;
                let bridge: Vec<ArchId> =
                    model.furnishings.iter().filter(|f| f.host == (FurnishingHost::Gap { left: p.id, right: c.id })).map(|f| f.id).collect();
                if (start - end).abs() <= EPS || (start > end && !bridge.is_empty()) {
                    bridged.extend(bridge);
                } else {
                    flush(&mut cur, &mut bridged, &mut runs);
                }
            }
            cur.push((i, u));
        }
        flush(&mut cur, &mut bridged, &mut runs);
    }
    runs.sort_by_key(|r| r.owner);
    runs
}

/// Gaps of 0.3 to 1.2 m between neighbouring in-line base cabinets.
pub fn gaps(model: &ArchModel) -> Vec<CabinetGap> {
    let mut out = Vec::new();
    for line in lines(model, |_, _| true, |_| true) {
        for w in line.windows(2) {
            let (Some(&(ai, au)), Some(&(bi, bu))) = (w.first(), w.get(1)) else { continue };
            let (Some(a), Some(b)) = (model.cabinets.get(ai), model.cabinets.get(bi)) else { continue };
            let (s, e) = (au + a.width / 2.0, bu - b.width / 2.0);
            let width = e - s;
            if !(0.3 - 1e-9..=1.2 + 1e-9).contains(&width) {
                continue;
            }
            let (dx, dy) = dir(a.rotation);
            let mid = (s + e) / 2.0 - au;
            let back = Vec2::new(a.position.x + dx * mid, a.position.y + dy * mid);
            let centre = Vec2::new(back.x - dy * a.depth / 2.0, back.y + dx * a.depth / 2.0);
            out.push(CabinetGap { left: a.id, right: b.id, centre, width, depth: a.depth, rotation: a.rotation, level: a.level, back_mid: back });
        }
    }
    out.sort_by_key(|g| (g.left, g.right));
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::Furnishing;

    fn cab(id: u32, x: f64, fronts: Fronts) -> Cabinet {
        Cabinet {
            id: ArchId(id),
            kind: CabinetKind::Base,
            position: Vec2::new(x, 0.0),
            width: 0.6,
            depth: 0.6,
            height: 0.876,
            fronts,
            doors: 1,
            drawers: 1,
            counter: true,
            counter_thickness: 0.038,
            counter_overhang: 0.025,
            toe_kick: true,
            ..Default::default()
        }
    }

    fn model(cabs: Vec<Cabinet>) -> ArchModel {
        ArchModel { cabinets: cabs, ..Default::default() }
    }

    fn sink_on(cab: u32) -> Furnishing {
        Furnishing { id: ArchId(0), cutout: Some([0.5, 0.4]), host: FurnishingHost::Cabinet { cabinet: ArchId(cab) }, ..Default::default() }
    }

    fn rects(m: &ArchModel, c: &Cabinet) -> Vec<[f64; 4]> {
        fronts(m, c).iter().map(|f| f.rect).collect()
    }

    #[test]
    fn fronts_match_the_phase13_layout_for_every_kind() {
        let m = ArchModel::default();
        let mut c = cab(0, 0.0, Fronts::Doors);
        c.doors = 2;
        let f = fronts(&m, &c);
        assert_eq!(f.len(), 2);
        let w = (0.6 - 0.003 * 3.0) / 2.0;
        assert!((f[0].rect[0] + 0.3 - 0.003).abs() < 1e-12 && (f[0].rect[2] - f[0].rect[0] - w).abs() < 1e-12);
        assert!((f[0].rect[1] - 0.1).abs() < 1e-12 && (f[0].rect[3] - 0.776).abs() < 1e-12);
        assert_eq!((f[0].hinge, f[1].hinge), (Some(Hinge::Left), Some(Hinge::Right)));
        c.fronts = Fronts::Drawers;
        c.drawers = 3;
        let r = rects(&m, &c);
        assert_eq!(r.len(), 3);
        assert!((r[0][3] - 0.776).abs() < 1e-12 && (r[2][1] - 0.0).abs() < 1e-12);
        c.fronts = Fronts::DrawerOverDoor;
        c.drawers = 1;
        c.doors = 2;
        let f = fronts(&m, &c);
        assert_eq!(f.iter().map(|x| x.kind).collect::<Vec<_>>(), vec![FrontKind::Drawer, FrontKind::Door, FrontKind::Door]);
        // Phase 13 put two doors' tops at 0.313 (half the space); they reach the drawer.
        assert!((f[0].rect[1] - 0.626).abs() < 1e-12 && (f[1].rect[3] - 0.626).abs() < 1e-12);
        c.elevation = 1.4;
        assert!((fronts(&m, &c)[0].rect[3] - 2.176).abs() < 1e-12);
    }

    #[test]
    fn doors_under_a_drawer_reach_it_however_many() {
        // The doors stand side by side: two or three of them are as tall as one.
        let m = ArchModel::default();
        for doors in 1..=3 {
            let mut c = cab(0, 0.0, Fronts::DrawerOverDoor);
            c.width = 0.9;
            c.doors = doors;
            let f = fronts(&m, &c);
            let drawer_bottom = f[0].rect[1];
            for d in f.iter().filter(|x| x.kind == FrontKind::Door) {
                assert!((d.rect[1] - 0.1).abs() < 1e-12 && (d.rect[3] - drawer_bottom).abs() < 1e-12, "{doors} doors: {:?}", d.rect);
            }
        }
    }

    #[test]
    fn door_pulls_sit_by_the_free_edge() {
        let m = ArchModel::default();
        let mut c = cab(0, 0.0, Fronts::Doors);
        c.doors = 2;
        c.pull = Some("bar".into());
        let f = fronts(&m, &c);
        let (a, b) = (f[0].pull.unwrap(), f[1].pull.unwrap());
        assert!((a[0] - (f[0].rect[2] - 0.05)).abs() < 1e-12);
        assert!((b[0] - (f[1].rect[0] + 0.05)).abs() < 1e-12);
        assert!((a[1] - (0.776 - 0.075)).abs() < 1e-12);
    }

    #[test]
    fn wall_cabinet_door_pulls_sit_low() {
        let m = ArchModel::default();
        let mut c = cab(0, 0.0, Fronts::Doors);
        c.kind = CabinetKind::Wall;
        c.toe_kick = false;
        c.elevation = 1.4;
        c.pull = Some("bar".into());
        let f = fronts(&m, &c);
        assert!((f[0].pull.unwrap()[1] - (1.4 + 0.075)).abs() < 1e-12);
    }

    #[test]
    fn drawer_pulls_are_centred() {
        let m = ArchModel::default();
        let mut c = cab(0, 0.0, Fronts::Drawers);
        c.drawers = 2;
        c.pull = Some("bar".into());
        for f in fronts(&m, &c) {
            let p = f.pull.unwrap();
            assert!((p[0] - (f.rect[0] + f.rect[2]) / 2.0).abs() < 1e-12);
            assert!((p[1] - (f.rect[1] + f.rect[3]) / 2.0).abs() < 1e-12);
        }
    }

    #[test]
    fn no_pull_no_pull_positions() {
        let m = ArchModel::default();
        let c = cab(0, 0.0, Fronts::DrawerOverDoor);
        assert!(fronts(&m, &c).iter().all(|f| f.pull.is_none()));
    }

    #[test]
    fn a_hosted_sink_makes_a_false_top_front() {
        let mut m = model(vec![cab(0, 0.0, Fronts::DrawerOverDoor)]);
        m.furnishings.push(sink_on(0));
        let c = m.cabinets[0].clone();
        assert_eq!(hosted_cutout(&m, &c).map(|x| x.0), Some(ArchId(0)));
        let f = fronts(&m, &c);
        assert_eq!(f[0].kind, FrontKind::False);
        assert!(f[0].pull.is_none());
        assert_eq!(f[1].kind, FrontKind::Door);
    }

    #[test]
    fn a_doors_only_cabinet_gets_a_false_strip() {
        let mut m = model(vec![cab(0, 0.0, Fronts::Doors)]);
        m.furnishings.push(sink_on(0));
        let c = m.cabinets[0].clone();
        let f = fronts(&m, &c);
        let strip = f.iter().find(|x| x.kind == FrontKind::False).unwrap();
        assert!((strip.rect[3] - 0.776).abs() < 1e-12 && (strip.rect[1] - 0.626).abs() < 1e-12);
        let door = f.iter().find(|x| x.kind == FrontKind::Door).unwrap();
        assert!((door.rect[3] - (0.626 - 0.003)).abs() < 1e-12);
    }

    #[test]
    fn touching_base_cabinets_share_a_counter_run() {
        let m = model(vec![cab(0, 0.0, Fronts::Doors), cab(1, 0.6, Fronts::Doors), cab(2, 1.2, Fronts::Doors)]);
        let runs = counter_runs(&m);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].owner, ArchId(0));
        assert_eq!(runs[0].cabinets, vec![ArchId(0), ArchId(1), ArchId(2)]);
        assert!((runs[0].u0 + 0.3).abs() < 1e-9 && (runs[0].u1 - 1.5).abs() < 1e-9);
        assert!((runs[0].top - 0.876).abs() < 1e-9);
    }

    #[test]
    fn a_bridged_gap_joins_the_run() {
        let mut m = model(vec![cab(0, 0.0, Fronts::Doors), cab(1, 1.2, Fronts::Doors)]);
        m.furnishings.push(Furnishing { id: ArchId(5), host: FurnishingHost::Gap { left: ArchId(0), right: ArchId(1) }, ..Default::default() });
        let runs = counter_runs(&m);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].bridged, vec![ArchId(5)]);
        assert!((runs[0].u1 - 1.5).abs() < 1e-9);
    }

    #[test]
    fn an_unbridged_gap_splits_runs() {
        let m = model(vec![cab(0, 0.0, Fronts::Doors), cab(1, 1.2, Fronts::Doors)]);
        let runs = counter_runs(&m);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[1].owner, ArchId(1));
    }

    #[test]
    fn cabinets_out_of_line_never_join() {
        let mut off = cab(1, 0.6, Fronts::Doors);
        off.position.y = 0.01;
        let mut rot = cab(2, 1.2, Fronts::Doors);
        rot.rotation = 90.0;
        let mut deep = cab(3, 1.8, Fronts::Doors);
        deep.depth = 0.7;
        let m = model(vec![cab(0, 0.0, Fronts::Doors), off, rot, deep]);
        assert_eq!(counter_runs(&m).len(), 4);
    }

    #[test]
    fn gaps_lists_gaps_between_0_3_and_1_2_m() {
        let m = model(vec![
            cab(0, 0.0, Fronts::Doors),
            cab(1, 0.9, Fronts::Doors), // 0.3 gap
            cab(2, 2.7, Fronts::Doors), // 1.2 gap
            cab(3, 3.5, Fronts::Doors), // 0.2 gap
            cab(4, 5.0, Fronts::Doors), // 0.9 gap
            cab(5, 9.0, Fronts::Doors), // too wide
        ]);
        let g = gaps(&m);
        let pairs: Vec<_> = g.iter().map(|x| (x.left.0, x.right.0)).collect();
        assert_eq!(pairs, vec![(0, 1), (1, 2), (3, 4)]);
        assert!((g[0].width - 0.3).abs() < 1e-9);
        assert!((g[0].back_mid.x - 0.45).abs() < 1e-9 && g[0].back_mid.y.abs() < 1e-9);
        assert!((g[0].centre.y - 0.3).abs() < 1e-9);
    }
}
