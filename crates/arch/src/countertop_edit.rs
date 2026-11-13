//! Editing custom countertops (phase 18): validation, detaching an automatic counter, moving one
//! edge by a typed distance and sampling arc edges into points. Pure.

use crate::countertop::{self, CounterKey, segment_area};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchId, COUNTER_LAYER, CounterEdge, Countertop, MAX_COUNTER_VERTICES, cabinet_label};
use cadcraft_geom::Vec2;

/// Most degrees one sampled arc segment spans.
const MAX_SEGMENT_DEG: f64 = 5.0;
/// Fewest and most segments one arc edge is sampled into.
const MIN_ARC_SEGMENTS: usize = 2;
const MAX_ARC_SEGMENTS: usize = 64;
/// Largest distance one edge moves.
const MAX_MOVE: f64 = 2.0;
/// Coordinates beyond this are refused.
const MAX_COORD: f64 = 1e6;

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

/// The polygon counter-clockwise with the sagitta of each of its edges.
fn oriented(ct: &Countertop) -> (Vec<Vec2>, Vec<f64>) {
    let n = ct.polygon.len();
    let sag = |i: usize| ct.edge(i).sagitta;
    if shoelace(&ct.polygon) < 0.0 {
        (ct.polygon.iter().rev().copied().collect(), (0..n).map(|k| sag((2 * n).saturating_sub(2 + k) % n.max(1))).collect())
    } else {
        (ct.polygon.clone(), (0..n).map(sag).collect())
    }
}

/// Check a countertop: 3..=256 finite vertices within 1e6 forming a simple outline (arcs
/// included) of area at least 0.01 m², `top` in [0, 3], `thickness` in [0.01, 0.1], every
/// sagitta finite with at most a half circle, and no more edge records than vertices.
pub fn validate(ct: &Countertop) -> Result<(), String> {
    let n = ct.polygon.len();
    if !(3..=MAX_COUNTER_VERTICES).contains(&n) {
        return Err(format!("a countertop needs 3 to {MAX_COUNTER_VERTICES} points, not {n}"));
    }
    if ct.polygon.iter().any(|p| !p.is_finite() || p.x.abs() > MAX_COORD || p.y.abs() > MAX_COORD) {
        return Err("countertop points must be finite and within 1e6 m".to_string());
    }
    if ct.edges.len() > n {
        return Err("a countertop has more edge settings than edges".to_string());
    }
    if !ct.top.is_finite() || !(0.0..=3.0).contains(&ct.top) {
        return Err("the countertop top must be within 0 to 3 m".to_string());
    }
    if !ct.thickness.is_finite() || !(0.01..=0.1).contains(&ct.thickness) {
        return Err("the countertop thickness must be within 0.01 to 0.1 m".to_string());
    }
    let (ring, sags) = oriented(ct);
    for i in 0..n {
        let (Some(p), Some(q)) = (ring.get(i), ring.get((i + 1) % n)) else { continue };
        let chord = p.dist(*q);
        let s = sags.get(i).copied().unwrap_or(0.0);
        if !s.is_finite() || s.abs() > chord / 2.0 {
            return Err(format!("edge {i}: the arc bulge must be finite and at most half the edge length"));
        }
    }
    let sampled = sampled_ring(&ring, &sags);
    if !crate::poly::is_simple(&sampled) {
        return Err("the countertop outline crosses itself".to_string());
    }
    let mut area = shoelace(&ring).abs();
    for i in 0..n {
        if let (Some(p), Some(q)) = (ring.get(i), ring.get((i + 1) % n)) {
            area += segment_area(p.dist(*q), sags.get(i).copied().unwrap_or(0.0));
        }
    }
    if !(area >= 0.01) {
        return Err("the countertop outline is smaller than 0.01 m²".to_string());
    }
    Ok(())
}

/// The custom countertop that replaces the automatic counter holding `cabinet`: the same ring,
/// top, thickness and cabinets, with plain edges.
pub fn detach(model: &ArchModel, cabinet: ArchId) -> Result<Countertop, String> {
    let outline = countertop::auto_outlines(model)
        .into_iter()
        .find(|o| matches!(o.key, Some(CounterKey::Auto(_))) && o.cabinets.contains(&cabinet))
        .ok_or_else(|| format!("{} has no automatic counter", cabinet_label(cabinet)))?;
    let n = outline.ring.len();
    Ok(Countertop {
        polygon: outline.ring,
        edges: vec![CounterEdge::default(); n],
        top: outline.top,
        thickness: outline.thickness,
        cabinets: outline.cabinets,
        level: outline.level,
        layer: COUNTER_LAYER.to_string(),
        ..Default::default()
    })
}

/// Where the line `a + t·da` meets the line `b + u·db`; `None` when they are parallel.
fn meet(a: Vec2, da: Vec2, b: Vec2, db: Vec2) -> Option<Vec2> {
    let c = da.cross(db);
    if c.abs() < 1e-9 {
        return None;
    }
    let t = (b - a).cross(db) / c;
    Some(a + da * t)
}

/// The unit direction from `a` to `b`; `None` for a zero-length edge.
fn direction(a: Vec2, b: Vec2) -> Option<Vec2> {
    let d = b - a;
    let l = d.len();
    (l.is_finite() && l > 1e-12).then(|| d * (1.0 / l))
}

/// Move edge `edge` outward by `d` metres (negative moves it inward). Its two vertices slide
/// along the neighbouring edges; a neighbour parallel to the edge just follows it. Sagittas are
/// kept. The result is validated.
pub fn move_edge(ct: &Countertop, edge: usize, d: f64) -> Result<Countertop, String> {
    let n = ct.polygon.len();
    if !d.is_finite() || d.abs() > MAX_MOVE {
        return Err(format!("an edge moves by at most {MAX_MOVE} m"));
    }
    if edge >= n || n < 3 {
        return Err(format!("edge {edge} does not exist (the countertop has {n})"));
    }
    let at = |i: usize| ct.polygon.get(i % n).copied().ok_or_else(|| "countertop has no such vertex".to_string());
    let (prev, a, b, next) = (at(edge + n - 1)?, at(edge)?, at(edge + 1)?, at(edge + 2)?);
    let e = direction(a, b).ok_or_else(|| format!("edge {edge} has no length"))?;
    // Right of travel is outward for a counter-clockwise polygon, left for a clockwise one.
    let right = Vec2::new(e.y, -e.x);
    let normal = if shoelace(&ct.polygon) < 0.0 { right * -1.0 } else { right };
    let shifted = a + normal * d;
    let new_a = match direction(prev, a).and_then(|dp| meet(shifted, e, prev, dp)) {
        Some(p) => p,
        None => a + normal * d,
    };
    let new_b = match direction(b, next).and_then(|dn| meet(shifted, e, b, dn)) {
        Some(p) => p,
        None => b + normal * d,
    };
    let mut out = ct.clone();
    if let Some(v) = out.polygon.get_mut(edge) {
        *v = new_a;
    }
    if let Some(v) = out.polygon.get_mut((edge + 1) % n) {
        *v = new_b;
    }
    validate(&out)?;
    if shoelace(&out.polygon) * shoelace(&ct.polygon) <= 0.0 {
        return Err("the edge would move through the opposite side".to_string());
    }
    Ok(out)
}

/// The ring with every arc edge (sagitta ≠ 0) replaced by points on its circular arc: the minor
/// arc through the chord's ends, bulging by `|s|` outward for `s > 0` and inward for `s < 0`
/// (outward is the right of travel of a counter-clockwise ring). At most 5° per segment and 2 to
/// 64 segments per edge; straight edges are unchanged.
pub fn sampled_ring(ring: &[Vec2], sagittas: &[f64]) -> Vec<Vec2> {
    let n = ring.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (Some(&p), Some(&q)) = (ring.get(i), ring.get((i + 1) % n)) else { continue };
        out.push(p);
        let s = sagittas.get(i).copied().unwrap_or(0.0);
        let chord = p.dist(q);
        if s == 0.0 || !s.is_finite() || !chord.is_finite() || chord <= 1e-12 {
            continue;
        }
        let m = s.abs();
        let r = (chord * chord / 4.0 + m * m) / (2.0 * m);
        let dir = (q - p) * (1.0 / chord);
        let normal = Vec2::new(dir.y, -dir.x);
        let mid = p.mid(q);
        let centre = mid + normal * (s - s.signum() * r);
        let half = (chord / 2.0).atan2(r - m);
        let total = 2.0 * half;
        let segs = ((total.to_degrees() / MAX_SEGMENT_DEG).ceil() as usize).clamp(MIN_ARC_SEGMENTS, MAX_ARC_SEGMENTS);
        let a0 = p - centre;
        let apex = mid + normal * s - centre;
        let turn = if a0.cross(apex) >= 0.0 { 1.0 } else { -1.0 };
        for k in 1..segs {
            let phi = turn * total * (k as f64) / (segs as f64);
            out.push(centre + a0.rotate(phi));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{Cabinet, CabinetKind};

    fn rect(w: f64, h: f64) -> Countertop {
        Countertop { polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(w, 0.0), Vec2::new(w, h), Vec2::new(0.0, h)], ..Default::default() }
    }

    fn area(ct: &Countertop) -> f64 {
        shoelace(&ct.polygon).abs()
    }

    fn island() -> ArchModel {
        let mut m = ArchModel::default();
        for x in [1.55 + 0.45, 1.55 + 1.35] {
            m.push_cabinet(Cabinet {
                kind: CabinetKind::Base,
                position: Vec2::new(x, 2.0),
                width: 0.9,
                depth: 0.6,
                height: 0.876,
                counter: true,
                counter_thickness: 0.038,
                counter_overhang: 0.025,
                ..Default::default()
            });
        }
        m
    }

    #[test]
    fn detach_keeps_ring_area_top_and_cabinets() {
        let m = island();
        let auto = countertop::auto_outlines(&m);
        assert_eq!(auto.len(), 1);
        let ct = detach(&m, ArchId(1)).unwrap_or_default();
        assert_eq!(ct.polygon, auto[0].ring);
        assert_eq!(ct.cabinets, auto[0].cabinets);
        assert_eq!((ct.top, ct.thickness, ct.level), (auto[0].top, auto[0].thickness, auto[0].level));
        assert_eq!(ct.edges.len(), ct.polygon.len());
        assert!((area(&ct) - countertop::outline_area(&auto[0])).abs() <= 1e-9);
        assert!(detach(&m, ArchId(9)).is_err());
        let mut claimed = m.clone();
        claimed.push_countertop(ct);
        assert_eq!(detach(&claimed, ArchId(0)), Err("cb0 has no automatic counter".to_string()));
    }

    #[test]
    fn move_edge_of_a_rectangle_adds_d_times_l() {
        let ct = rect(1.8, 0.625);
        let out = move_edge(&ct, 2, 0.3).unwrap_or_default();
        assert!((area(&out) - (area(&ct) + 0.3 * 1.8)).abs() < 1e-9);
        let inward = move_edge(&ct, 1, -0.2).unwrap_or_default();
        assert!((area(&inward) - (area(&ct) - 0.2 * 0.625)).abs() < 1e-9);
    }

    #[test]
    fn move_edge_keeps_adjacent_directions_on_a_trapezoid() {
        let ct =
            Countertop { polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(1.5, 1.0), Vec2::new(0.5, 1.0)], ..Default::default() };
        let out = move_edge(&ct, 2, 0.2).unwrap_or_default();
        let dir = |c: &Countertop, i: usize| direction(c.polygon[i], c.polygon[(i + 1) % 4]).unwrap_or_default();
        for i in [1, 3] {
            assert!(dir(&ct, i).cross(dir(&out, i)).abs() < 1e-9, "edge {i} keeps its direction");
        }
        assert!((out.polygon[2].y - 1.2).abs() < 1e-9 && (out.polygon[3].y - 1.2).abs() < 1e-9);
        assert!(out.polygon[2].x < 1.5 && out.polygon[3].x > 0.5, "the sides converge going up");
    }

    #[test]
    fn move_edge_with_parallel_neighbour_translates() {
        // A vertex whose neighbour is collinear with the moved edge moves by d·n.
        let ct = Countertop {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 1.0), Vec2::new(0.0, 1.0)],
            ..Default::default()
        };
        let out = move_edge(&ct, 0, 0.25).unwrap_or_default();
        assert!(out.polygon[1].dist(Vec2::new(1.0, -0.25)) < 1e-9);
        assert!(out.polygon[0].dist(Vec2::new(0.0, -0.25)) < 1e-9);
    }

    #[test]
    fn move_that_folds_the_polygon_is_refused() {
        let ct = rect(1.0, 0.5);
        assert!(move_edge(&ct, 2, -0.5).is_err(), "zero height");
        assert!(move_edge(&ct, 2, -0.9).is_err(), "folded through the opposite edge");
        assert!(move_edge(&ct, 2, f64::NAN).is_err());
        assert!(move_edge(&ct, 2, 2.5).is_err());
        assert!(move_edge(&ct, 4, 0.1).is_err());
    }

    #[test]
    fn validate_countertop() {
        assert!(validate(&rect(1.0, 0.5)).is_ok());
        assert!(validate(&Countertop::default()).is_err(), "no polygon");
        assert!(validate(&rect(0.05, 0.05)).is_err(), "too small");
        let mut c = rect(1.0, 0.5);
        c.top = 3.5;
        assert!(validate(&c).is_err());
        c = rect(1.0, 0.5);
        c.thickness = 0.5;
        assert!(validate(&c).is_err());
        c = rect(1.0, 0.5);
        c.polygon[0] = Vec2::new(f64::NAN, 0.0);
        assert!(validate(&c).is_err());
        c = rect(1.0, 0.5);
        c.polygon = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)];
        assert!(validate(&c).is_err(), "bow tie");
        c = rect(1.0, 0.5);
        c.edges = vec![CounterEdge { sagitta: 0.6, ..Default::default() }];
        assert!(validate(&c).is_err(), "more than a half circle");
        c.edges = vec![CounterEdge { sagitta: 0.5, ..Default::default() }];
        assert!(validate(&c).is_ok(), "a half circle");
        c.edges = vec![CounterEdge::default(); 5];
        assert!(validate(&c).is_err(), "more edges than vertices");
        c = rect(1.0, 0.5);
        c.polygon = (0..300).map(|i| Vec2::new(f64::from(i), 0.0)).collect();
        assert!(validate(&c).is_err(), "too many vertices");
    }

    #[test]
    fn sampled_ring_of_straight_edges_is_the_ring() {
        let ct = rect(1.0, 0.5);
        assert_eq!(sampled_ring(&ct.polygon, &[0.0; 4]), ct.polygon);
        assert_eq!(sampled_ring(&ct.polygon, &[]), ct.polygon);
    }

    #[test]
    fn sampled_ring_arc_vertices_lie_on_the_circle() {
        let ring = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)];
        let s = 0.2;
        let out = sampled_ring(&ring, &[s, 0.0, 0.0, 0.0]);
        let r = (0.25 + s * s) / (2.0 * s);
        let centre = Vec2::new(0.5, -s + r);
        assert!(out.len() > 4 + 2);
        assert!(out.len() - 4 <= 63);
        for p in &out[1..out.len() - 3] {
            assert!((p.dist(centre) - r).abs() < 1e-9);
            assert!(p.y <= 1e-9, "an outward bulge lies right of travel");
        }
        let inward = sampled_ring(&ring, &[-s, 0.0, 0.0, 0.0]);
        assert!(inward[1..inward.len() - 3].iter().all(|p| p.y >= -1e-9));
        let half = sampled_ring(&ring, &[0.5, 0.0, 0.0, 0.0]);
        assert!(half.iter().any(|p| (p.y + 0.5).abs() < 1e-6), "the apex of a half circle");
    }
}
