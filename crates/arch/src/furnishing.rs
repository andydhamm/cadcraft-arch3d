//! Furnishings in the plan (phase 14): the top-down outline of a placed symbol.

use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::Furnishing;
use cadcraft_geom::Vec2;

use crate::library::layer_for;

/// Most points a plan outline keeps before near-duplicates are merged.
const MAX_OUTLINE_POINTS: usize = 64;

/// The layer a furnishing's plan outline is drawn on.
pub fn furnishing_layer(f: &Furnishing) -> &'static str {
    layer_for(&f.category, &f.symbol_type)
}

/// The convex hull (counter-clockwise, no repeated or collinear points) of the furnishing's
/// placed symbol seen from above; a missing symbol gives the rotated size rectangle.
pub fn plan_outline(model: &ArchModel, f: &Furnishing) -> Vec<Vec2> {
    let mut pts: Vec<Vec2> = Vec::new();
    match model.symbols.get(&f.symbol) {
        Some(sym) if sym.parts.iter().any(|p| !p.positions.is_empty()) => {
            let ext = sym.extents();
            for part in &sym.parts {
                for p in &part.positions {
                    let q = f.local_to_plan([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])], ext);
                    if q[0].is_finite() && q[1].is_finite() {
                        pts.push(Vec2::new(q[0], q[1]));
                    }
                }
            }
        }
        _ => {
            let (hx, hy) = (f.size[0] / 2.0, f.size[1] / 2.0);
            // Extents equal to the size make the scale 1: the corners map to themselves.
            let ext = f.size;
            for (x, y) in [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)] {
                let q = f.local_to_plan([x, y, 0.0], ext);
                if q[0].is_finite() && q[1].is_finite() {
                    pts.push(Vec2::new(q[0], q[1]));
                }
            }
        }
    }
    let mut hull = convex_hull(pts);
    if hull.len() > MAX_OUTLINE_POINTS {
        hull = merge_close(hull, 1e-4);
    }
    hull
}

/// Andrew's monotone chain; counter-clockwise, strictly convex.
fn convex_hull(mut pts: Vec<Vec2>) -> Vec<Vec2> {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup_by(|a, b| a.x == b.x && a.y == b.y);
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).cross(b - o);
    let mut hull: Vec<Vec2> = Vec::with_capacity(pts.len() * 2);
    for &p in &pts {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 1e-12 {
            hull.pop();
        }
        hull.push(p);
    }
    let lower = hull.len() + 1;
    for &p in pts.iter().rev().skip(1) {
        while hull.len() >= lower && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 1e-12 {
            hull.pop();
        }
        hull.push(p);
    }
    hull.pop();
    hull
}

/// Drop points closer than `eps` to the previously kept one (and to the first).
fn merge_close(pts: Vec<Vec2>, eps: f64) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().is_none_or(|l| l.dist(p) >= eps) {
            out.push(p);
        }
    }
    while out.len() > 1 && out.first().zip(out.last()).is_some_and(|(a, b)| a.dist(*b) < eps) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{SymbolMesh, SymbolPart};

    /// A unit cube symbol (1 x 1 x 1, bottom-centred) under key "cube".
    fn cube_model() -> ArchModel {
        let mut positions = Vec::new();
        for z in [0.0f32, 1.0] {
            for (x, y) in [(-0.5f32, -0.5f32), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
                positions.push([x, y, z]);
            }
        }
        let part = SymbolPart { name: "cube".into(), positions, ..Default::default() };
        let mesh = SymbolMesh { key: "cube".into(), parts: vec![part], ..Default::default() };
        let mut m = ArchModel::default();
        m.symbols.insert("cube".into(), mesh);
        m
    }

    fn cube(f: impl FnOnce(&mut Furnishing)) -> Furnishing {
        let mut x = Furnishing { symbol: "cube".into(), size: [1.0, 1.0, 1.0], ..Default::default() };
        f(&mut x);
        x
    }

    fn area(h: &[Vec2]) -> f64 {
        (0..h.len()).map(|i| h[i].cross(h[(i + 1) % h.len()])).sum::<f64>() / 2.0
    }

    fn span(h: &[Vec2], x: bool) -> f64 {
        let v = |p: &Vec2| if x { p.x } else { p.y };
        h.iter().map(v).fold(f64::MIN, f64::max) - h.iter().map(v).fold(f64::MAX, f64::min)
    }

    #[test]
    fn plan_outline_is_the_convex_hull_of_the_placed_mesh() {
        let m = cube_model();
        let f = cube(|f| {
            f.size = [2.0, 1.0, 1.0];
            f.at = [10.0, 5.0];
        });
        let h = plan_outline(&m, &f);
        assert_eq!(h.len(), 4);
        assert!((area(&h) - 2.0).abs() < 1e-9, "counter-clockwise, 2 x 1");
        assert!((span(&h, true) - 2.0).abs() < 1e-9 && (span(&h, false) - 1.0).abs() < 1e-9);
        let cx = h.iter().map(|p| p.x).sum::<f64>() / 4.0;
        let cy = h.iter().map(|p| p.y).sum::<f64>() / 4.0;
        assert!((cx - 10.0).abs() < 1e-9 && (cy - 5.0).abs() < 1e-9);
    }

    #[test]
    fn plan_outline_follows_rotation_and_mirror() {
        let m = cube_model();
        let f = cube(|f| {
            f.size = [2.0, 1.0, 1.0];
            f.rotation = 90.0;
        });
        let h = plan_outline(&m, &f);
        assert!((span(&h, true) - 1.0).abs() < 1e-9 && (span(&h, false) - 2.0).abs() < 1e-9);
        let h = plan_outline(&m, &cube(|f| f.rotation = 45.0));
        assert_eq!(h.len(), 4);
        let r = 0.5 * 2f64.sqrt();
        assert!(h.iter().any(|p| (p.x - r).abs() < 1e-9 && p.y.abs() < 1e-9));
        let h = plan_outline(&m, &cube(|f| f.mirrored = true));
        assert!(area(&h) > 0.99, "a mirrored outline stays counter-clockwise");
    }

    #[test]
    fn a_missing_symbol_outlines_its_size() {
        let m = ArchModel::default();
        let f = Furnishing { size: [2.0, 1.0, 1.0], at: [1.0, 1.0], rotation: 90.0, ..Default::default() };
        let h = plan_outline(&m, &f);
        assert_eq!(h.len(), 4);
        assert!((area(&h) - 2.0).abs() < 1e-9);
        assert!((span(&h, true) - 1.0).abs() < 1e-9 && (span(&h, false) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn layers_follow_the_category() {
        let f = Furnishing { category: "Furniture".into(), ..Default::default() };
        assert_eq!(furnishing_layer(&f), "I-FURN");
    }
}
