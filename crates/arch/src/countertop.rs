//! Counter outlines (phase 18): the plan outline of every countertop, automatic (a run of base
//! cabinets or a lone tall counter cabinet) or custom (a `Countertop` of the model). Pure.

use crate::casework_layout::{CounterRun, counter_runs};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchId, Cabinet, CabinetKind, Countertop, FurnishingHost, MAX_COUNTER_VERTICES};
use cadcraft_geom::Vec2;
use std::collections::BTreeSet;

/// Most cut-out holes of one counter.
const MAX_HOLES: usize = 64;

/// Which counter an outline is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CounterKey {
    /// The automatic counter whose owner (the lowest-u cabinet of the run) is this cabinet.
    Auto(ArchId),
    /// The custom countertop with this id.
    Custom(ArchId),
}

/// The plan outline of one counter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CounterOutline {
    /// Which counter it is.
    pub key: Option<CounterKey>,
    /// Level index.
    pub level: u32,
    /// The cabinets it covers.
    pub cabinets: Vec<ArchId>,
    /// Counter-clockwise chord vertices in world plan coordinates.
    pub ring: Vec<Vec2>,
    /// Sagitta of each edge (the ring's length); `> 0` bulges outward.
    pub sagittas: Vec<f64>,
    /// The work surface above the level floor.
    pub top: f64,
    /// Slab thickness.
    pub thickness: f64,
    /// Cut-out rectangles.
    pub holes: Vec<Vec<Vec2>>,
    /// Furnishings standing in bridged gaps.
    pub bridged: Vec<ArchId>,
}

fn rot(p: Vec2, deg: f64) -> Vec2 {
    let (s, c) = deg.to_radians().sin_cos();
    Vec2::new(p.x * c - p.y * s, p.x * s + p.y * c)
}

/// The world points of the rectangle `u0..u1 x y0..y1` in the frame of `origin` turned `rotation`
/// degrees, counter-clockwise.
fn frame_rect(origin: Vec2, rotation: f64, u0: f64, u1: f64, y0: f64, y1: f64) -> Vec<Vec2> {
    [(u0, y0), (u1, y0), (u1, y1), (u0, y1)].iter().map(|&(u, y)| origin + rot(Vec2::new(u, y), rotation)).collect()
}

fn shoelace(r: &[Vec2]) -> f64 {
    let n = r.len();
    let mut a = 0.0;
    for i in 0..n {
        if let (Some(p), Some(q)) = (r.get(i), r.get((i + 1) % n)) {
            a += p.x * q.y - q.x * p.y;
        }
    }
    a / 2.0
}

/// Every cabinet id listed by a custom countertop.
pub fn claimed_cabinets(model: &ArchModel) -> BTreeSet<ArchId> {
    model.countertops.iter().flat_map(|c| c.cabinets.iter().copied()).collect()
}

/// The run's counter rectangle `u0..u1 x 0..depth + overhang` in the owner's frame: counter-clockwise
/// world points.
pub fn run_ring(run: &CounterRun) -> Vec<Vec2> {
    frame_rect(run.origin, run.rotation, run.u0, run.u1, 0.0, run.depth + run.overhang)
}

/// The outlines of the automatic counters: one per counter run, one per unclaimed tall counter
/// cabinet that is in no run, then merged at corners.
pub fn auto_outlines(model: &ArchModel) -> Vec<CounterOutline> {
    let claimed = claimed_cabinets(model);
    let runs = counter_runs(model);
    let mut out: Vec<CounterOutline> = runs
        .iter()
        .map(|run| {
            let ring = run_ring(run);
            CounterOutline {
                key: Some(CounterKey::Auto(run.owner)),
                level: run.level,
                cabinets: run.cabinets.clone(),
                sagittas: vec![0.0; ring.len()],
                ring,
                top: run.top + run.thickness,
                thickness: run.thickness,
                holes: cutout_holes(model, &run.cabinets, &run.bridged),
                bridged: run.bridged.clone(),
            }
        })
        .collect();
    let mut talls: Vec<&Cabinet> = model
        .cabinets
        .iter()
        .filter(|c| {
            c.kind == CabinetKind::Tall
                && c.counter
                && !claimed.contains(&c.id)
                && !runs.iter().any(|r| r.cabinets.contains(&c.id))
                && [c.position.x, c.position.y, c.rotation, c.width, c.depth, c.height, c.elevation, c.counter_thickness, c.counter_overhang]
                    .iter()
                    .all(|v| v.is_finite())
        })
        .collect();
    talls.sort_by_key(|c| c.id);
    for c in talls {
        let ring = frame_rect(c.position, c.rotation, -c.width / 2.0, c.width / 2.0, 0.0, c.depth + c.counter_overhang);
        out.push(CounterOutline {
            key: Some(CounterKey::Auto(c.id)),
            level: c.level,
            cabinets: vec![c.id],
            sagittas: vec![0.0; ring.len()],
            ring,
            top: c.elevation + c.height + c.counter_thickness,
            thickness: c.counter_thickness,
            holes: cutout_holes(model, &[c.id], &[]),
            bridged: Vec::new(),
        });
    }
    crate::casework_corner::merge_corners(model, out)
}

/// The world rectangles (counter-clockwise) of the cut-outs of furnishings hosted by one of
/// `cabinets`, or by a gap between two of them whose furnishing is in `bridged`. At most 64.
pub fn cutout_holes(model: &ArchModel, cabinets: &[ArchId], bridged: &[ArchId]) -> Vec<Vec<Vec2>> {
    let mut holes = Vec::new();
    for f in &model.furnishings {
        let host = match f.host {
            FurnishingHost::Cabinet { cabinet } if cabinets.contains(&cabinet) => Some(cabinet),
            FurnishingHost::Gap { left, right } if bridged.contains(&f.id) && cabinets.contains(&left) && cabinets.contains(&right) => Some(left),
            _ => None,
        };
        let (Some(host), Some(co)) = (host, f.cutout) else { continue };
        let Some(hc) = model.cabinets.iter().find(|c| c.id == host) else { continue };
        if !(f.at.iter().chain(co.iter()).chain([&f.rotation, &hc.rotation]).all(|v| v.is_finite()) && co[0] > 0.0 && co[1] > 0.0) {
            continue;
        }
        let turned = (f.rotation - hc.rotation).to_radians().sin().abs() > 0.5;
        let (w, d) = if turned { (co[1], co[0]) } else { (co[0], co[1]) };
        let centre = Vec2::new(f.at[0], f.at[1]);
        holes.push(frame_rect(centre, hc.rotation, -w / 2.0, w / 2.0, -d / 2.0, d / 2.0));
        if holes.len() >= MAX_HOLES {
            break;
        }
    }
    holes
}

/// The outline of a custom countertop; `None` unless its polygon has 3..=256 finite vertices and
/// a non-zero area. A clockwise polygon is reversed (its sagittas follow their edges).
pub fn custom_outline(model: &ArchModel, ct: &Countertop) -> Option<CounterOutline> {
    let n = ct.polygon.len();
    if !(3..=MAX_COUNTER_VERTICES).contains(&n) || ct.polygon.iter().any(|p| !p.is_finite()) || shoelace(&ct.polygon) == 0.0 {
        return None;
    }
    let sag = |i: usize| ct.edge(i).sagitta;
    let (ring, sagittas): (Vec<Vec2>, Vec<f64>) = if shoelace(&ct.polygon) < 0.0 {
        // Reversed edge k joins r[n-1-k] to r[n-2-k]: the original edge (n-2-k) mod n, as 2n-2-k mod n.
        (ct.polygon.iter().rev().copied().collect(), (0..n).map(|k| sag((2 * n - 2 - k) % n)).collect())
    } else {
        (ct.polygon.clone(), (0..n).map(sag).collect())
    };
    let bridged: Vec<ArchId> = model
        .furnishings
        .iter()
        .filter(|f| matches!(f.host, FurnishingHost::Gap { left, right } if ct.cabinets.contains(&left) && ct.cabinets.contains(&right)))
        .map(|f| f.id)
        .collect();
    Some(CounterOutline {
        key: Some(CounterKey::Custom(ct.id)),
        level: ct.level,
        cabinets: ct.cabinets.clone(),
        ring,
        sagittas,
        top: ct.top,
        thickness: ct.thickness,
        holes: cutout_holes(model, &ct.cabinets, &bridged),
        bridged,
    })
}

/// Every counter outline: the automatic ones, then the custom ones in model order.
pub fn outlines(model: &ArchModel) -> Vec<CounterOutline> {
    let mut out = auto_outlines(model);
    out.extend(model.countertops.iter().filter_map(|ct| custom_outline(model, ct)));
    out
}

/// The area between a chord and its arc of the given sagitta; signed like the sagitta, 0 for a
/// non-finite or non-positive chord or a zero sagitta.
pub fn segment_area(chord: f64, sagitta: f64) -> f64 {
    if !chord.is_finite() || !sagitta.is_finite() || chord <= 0.0 || sagitta == 0.0 {
        return 0.0;
    }
    let s = sagitta.abs();
    let r = (chord * chord / 4.0 + s * s) / (2.0 * s);
    let a = r * r * ((r - s) / r).clamp(-1.0, 1.0).acos() - (r - s) * (2.0 * r * s - s * s).max(0.0).sqrt();
    if sagitta < 0.0 { -a } else { a }
}

/// The outline's area: the ring's area plus its arc segments (holes are not subtracted).
pub fn outline_area(o: &CounterOutline) -> f64 {
    let n = o.ring.len();
    let mut a = shoelace(&o.ring).abs();
    for i in 0..n {
        if let (Some(p), Some(q)) = (o.ring.get(i), o.ring.get((i + 1) % n)) {
            a += segment_area(p.dist(*q), o.sagittas.get(i).copied().unwrap_or(0.0));
        }
    }
    if a.is_finite() { a } else { 0.0 }
}

/// Base cabinets on `level` whose footprint centre lies inside `polygon`, in id order.
pub fn counter_cabinets_under(model: &ArchModel, level: u32, polygon: &[Vec2]) -> Vec<ArchId> {
    if polygon.len() < 3 || polygon.iter().any(|p| !p.is_finite()) {
        return Vec::new();
    }
    let mut out: Vec<ArchId> = model
        .cabinets
        .iter()
        .filter(|c| c.kind == CabinetKind::Base && c.level == level)
        .filter(|c| c.position.is_finite() && c.rotation.is_finite() && c.depth.is_finite())
        .filter(|c| inside(c.position + rot(Vec2::new(0.0, c.depth / 2.0), c.rotation), polygon))
        .map(|c| c.id)
        .collect();
    out.sort();
    out
}

/// Even-odd point in polygon.
fn inside(p: Vec2, poly: &[Vec2]) -> bool {
    let n = poly.len();
    let mut hit = false;
    for i in 0..n {
        let (Some(a), Some(b)) = (poly.get(i), poly.get((i + 1) % n)) else { continue };
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            hit = !hit;
        }
    }
    hit
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{CounterEdge, Furnishing};

    fn cab(id: u32, x: f64, kind: CabinetKind) -> Cabinet {
        Cabinet {
            id: ArchId(id),
            kind,
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

    fn model(cabs: Vec<Cabinet>) -> ArchModel {
        ArchModel { cabinets: cabs, ..Default::default() }
    }

    fn square(s: f64) -> Vec<Vec2> {
        vec![Vec2::new(0.0, 0.0), Vec2::new(s, 0.0), Vec2::new(s, s), Vec2::new(0.0, s)]
    }

    #[test]
    fn auto_outline_of_a_run_is_its_counter_rectangle() {
        let m = model(vec![cab(0, 0.0, CabinetKind::Base), cab(1, 0.6, CabinetKind::Base)]);
        let o = auto_outlines(&m);
        assert_eq!(o.len(), 1);
        let o = &o[0];
        assert_eq!(o.key, Some(CounterKey::Auto(ArchId(0))));
        assert_eq!(o.cabinets, vec![ArchId(0), ArchId(1)]);
        assert_eq!(o.ring.len(), 4);
        let (a, b) = (o.ring[0], o.ring[2]);
        assert!((a.x + 0.3).abs() < 1e-9 && a.y.abs() < 1e-9);
        assert!((b.x - 0.9).abs() < 1e-9 && (b.y - 0.625).abs() < 1e-9);
        assert!(shoelace(&o.ring) > 0.0);
        assert_eq!(o.sagittas, vec![0.0; 4]);
        assert!((o.top - 0.914).abs() < 1e-9 && (o.thickness - 0.038).abs() < 1e-12);
        assert!((outline_area(o) - 1.2 * 0.625).abs() < 1e-9);
        assert_eq!(outlines(&m), auto_outlines(&m));
    }

    #[test]
    fn lone_tall_counter_cabinet_has_an_outline() {
        let mut t = cab(3, 2.0, CabinetKind::Tall);
        t.height = 2.1;
        let o = auto_outlines(&model(vec![t]));
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].key, Some(CounterKey::Auto(ArchId(3))));
        assert!((o[0].top - 2.138).abs() < 1e-9);
        assert!((outline_area(&o[0]) - 0.6 * 0.625).abs() < 1e-9);
        // Without the counter flag it has none.
        let mut t2 = cab(3, 2.0, CabinetKind::Tall);
        t2.counter = false;
        assert!(auto_outlines(&model(vec![t2])).is_empty());
    }

    #[test]
    fn claimed_cabinets_leave_the_runs() {
        let mut m = model(vec![cab(0, 0.0, CabinetKind::Base), cab(1, 0.6, CabinetKind::Base), cab(2, 1.2, CabinetKind::Tall)]);
        assert_eq!(auto_outlines(&m).len(), 2);
        m.countertops.push(Countertop { id: ArchId(0), cabinets: vec![ArchId(1), ArchId(2)], polygon: square(1.0), ..Default::default() });
        assert_eq!(claimed_cabinets(&m), BTreeSet::from([ArchId(1), ArchId(2)]));
        let o = auto_outlines(&m);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].cabinets, vec![ArchId(0)]);
        let all = outlines(&m);
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].key, Some(CounterKey::Custom(ArchId(0))));
    }

    #[test]
    fn custom_outline_area_with_sagittas() {
        let m = ArchModel::default();
        let mut ct = Countertop { id: ArchId(5), polygon: square(2.0), ..Default::default() };
        ct.edges = vec![CounterEdge { sagitta: 0.5, ..Default::default() }];
        let o = custom_outline(&m, &ct).unwrap();
        assert_eq!(o.key, Some(CounterKey::Custom(ArchId(5))));
        assert!((o.top - 0.914).abs() < 1e-12);
        assert!((outline_area(&o) - (4.0 + segment_area(2.0, 0.5))).abs() < 1e-12);
        // A clockwise polygon is reversed and its sagittas follow their edges: edge 0 (0,0)->(2,0)
        // of the clockwise ring becomes the edge (2,0)->(0,0), the last of the reversed ring...
        let mut cw = ct.clone();
        cw.polygon = square(2.0).into_iter().rev().collect();
        // Clockwise ring: (0,2) (2,2) (2,0) (0,0); its edge 2 runs (2,0)->(0,0).
        cw.edges = vec![CounterEdge::default(), CounterEdge::default(), CounterEdge { sagitta: 0.5, ..Default::default() }];
        let r = custom_outline(&m, &cw).unwrap();
        assert!(shoelace(&r.ring) > 0.0);
        assert!((outline_area(&r) - outline_area(&o)).abs() < 1e-12);
        // The sagitta sits on the edge (0,0)->(2,0).
        let i = r.sagittas.iter().position(|&s| s != 0.0).unwrap();
        assert_eq!((r.ring[i], r.ring[(i + 1) % 4]), (Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0)));
    }

    #[test]
    fn segment_area_matches_the_closed_form() {
        // A half disc: chord 2r, sagitta r.
        assert!((segment_area(2.0, 1.0) - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        // A shallow arc is close to 2/3 chord x sagitta.
        assert!((segment_area(2.0, 0.01) - 2.0 / 3.0 * 2.0 * 0.01).abs() < 1e-5);
        // A major segment (sagitta above half the chord) is bigger than the half disc.
        assert!(segment_area(2.0, 1.5) > std::f64::consts::FRAC_PI_2);
    }

    #[test]
    fn segment_area_is_signed_and_zero_for_zero() {
        assert!((segment_area(2.0, -0.5) + segment_area(2.0, 0.5)).abs() < 1e-15);
        assert!(segment_area(2.0, -0.5) < 0.0);
        for (c, s) in [(2.0, 0.0), (0.0, 0.5), (-1.0, 0.5), (f64::NAN, 0.5), (2.0, f64::NAN), (f64::INFINITY, 1.0)] {
            assert_eq!(segment_area(c, s), 0.0);
        }
    }

    #[test]
    fn cutout_holes_follow_run_counter_rules() {
        let sink = |id: u32, host: FurnishingHost, rotation: f64| Furnishing {
            id: ArchId(id),
            at: [0.1, 0.3],
            rotation,
            cutout: Some([0.5, 0.4]),
            host,
            ..Default::default()
        };
        let mut m = model(vec![cab(0, 0.0, CabinetKind::Base), cab(1, 0.6, CabinetKind::Base)]);
        m.furnishings = vec![
            sink(10, FurnishingHost::Cabinet { cabinet: ArchId(0) }, 0.0),
            sink(11, FurnishingHost::Cabinet { cabinet: ArchId(9) }, 0.0),
            sink(12, FurnishingHost::Gap { left: ArchId(0), right: ArchId(1) }, 90.0),
        ];
        let ids = [ArchId(0), ArchId(1)];
        // The gap furnishing needs to be listed as bridged.
        assert_eq!(cutout_holes(&m, &ids, &[]).len(), 1);
        let h = cutout_holes(&m, &ids, &[ArchId(12)]);
        assert_eq!(h.len(), 2);
        let ext = |r: &[Vec2]| (r[2].x - r[0].x, r[2].y - r[0].y);
        let (w, d) = ext(&h[0]);
        assert!((w - 0.5).abs() < 1e-9 && (d - 0.4).abs() < 1e-9);
        // Turned 90 degrees against the cabinet: width and depth swap.
        let (w, d) = ext(&h[1]);
        assert!((w - 0.4).abs() < 1e-9 && (d - 0.5).abs() < 1e-9);
        assert!(shoelace(&h[1]) > 0.0);
        // Non-finite and non-positive cut-outs are skipped.
        m.furnishings = vec![
            Furnishing { cutout: Some([f64::NAN, 0.4]), ..sink(10, FurnishingHost::Cabinet { cabinet: ArchId(0) }, 0.0) },
            Furnishing { cutout: Some([0.0, 0.4]), ..sink(11, FurnishingHost::Cabinet { cabinet: ArchId(0) }, 0.0) },
        ];
        assert!(cutout_holes(&m, &ids, &[]).is_empty());
        // The run's outline carries them.
        m.furnishings = vec![sink(10, FurnishingHost::Cabinet { cabinet: ArchId(0) }, 0.0)];
        assert_eq!(auto_outlines(&m)[0].holes.len(), 1);
        // At most 64.
        m.furnishings = (0..100).map(|i| sink(10 + i, FurnishingHost::Cabinet { cabinet: ArchId(0) }, 0.0)).collect();
        assert_eq!(cutout_holes(&m, &ids, &[]).len(), 64);
    }

    #[test]
    fn counter_cabinets_under_picks_centres_inside() {
        let mut other = cab(2, 0.6, CabinetKind::Base);
        other.level = 1;
        let m = model(vec![
            cab(1, 0.6, CabinetKind::Base),
            cab(0, 0.0, CabinetKind::Base),
            cab(3, 5.0, CabinetKind::Base),
            cab(4, 0.0, CabinetKind::Wall),
            other,
        ]);
        // Footprint centres are at (x, 0.3).
        let poly = vec![Vec2::new(-0.5, -0.1), Vec2::new(1.0, -0.1), Vec2::new(1.0, 0.5), Vec2::new(-0.5, 0.5)];
        assert_eq!(counter_cabinets_under(&m, 0, &poly), vec![ArchId(0), ArchId(1)]);
        assert_eq!(counter_cabinets_under(&m, 1, &poly), vec![ArchId(2)]);
        assert!(counter_cabinets_under(&m, 0, &poly[..2]).is_empty());
    }

    #[test]
    fn outlines_survive_nan_and_empty_polygons() {
        let mut m = model(vec![cab(0, f64::NAN, CabinetKind::Base), cab(1, 0.0, CabinetKind::Tall)]);
        m.cabinets[1].depth = f64::INFINITY;
        let nan = Vec2::new(f64::NAN, 0.0);
        let line = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0)];
        let big: Vec<Vec2> = (0..=MAX_COUNTER_VERTICES).map(|i| Vec2::new((i as f64).cos(), (i as f64).sin())).collect();
        for polygon in [Vec::new(), vec![Vec2::new(0.0, 0.0)], line, vec![nan, Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0)], big] {
            m.countertops.push(Countertop { id: ArchId(0), polygon, ..Default::default() });
        }
        for ct in &m.countertops {
            assert!(custom_outline(&m, ct).is_none());
        }
        let _ = outlines(&m);
        assert!(counter_cabinets_under(&m, 0, &[nan, nan, nan]).is_empty());
        assert_eq!(outline_area(&CounterOutline::default()), 0.0);
        let o = CounterOutline { ring: vec![nan, Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)], ..Default::default() };
        assert_eq!(outline_area(&o), 0.0);
    }
}
