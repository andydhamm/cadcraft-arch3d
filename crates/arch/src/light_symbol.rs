//! Plan symbols of light fixtures (phase 10), drawn on `E-LITE`. Filled by `p10-arch-light-symbol`.

use cadcraft_doc::arch::{FixtureShape, LightFixture, StripFacing};
use cadcraft_geom::Vec2;

/// Plan symbol polylines (metres; a closed ring repeats its first point) of the fixture's shape at
/// its position, sized by `crate::light::housing_diameter`.
pub fn plan_symbol(f: &LightFixture) -> Vec<Vec<Vec2>> {
    if f.shape == FixtureShape::Strip {
        return strip_symbol(f);
    }
    let c = f.position;
    let d = crate::light::housing_diameter(f);
    let r = d / 2.0;
    if !c.is_finite() || !r.is_finite() {
        return Vec::new();
    }
    match f.shape {
        FixtureShape::RecessedCan => vec![circle(c, r), circle(c, r / 2.0)],
        FixtureShape::Pendant => {
            let u45 = unit(45.0);
            let u135 = unit(135.0);
            vec![circle(c, r), line(c, u45, r), line(c, u135, r)]
        }
        FixtureShape::SurfaceDome => {
            let mut rings = vec![circle(c, r)];
            for k in 0..8 {
                let u = unit(k as f64 * 45.0);
                rings.push(vec![c + u * r, c + u * (r + 0.05)]);
            }
            rings
        }
        FixtureShape::Sconce => {
            let mut ring = Vec::with_capacity(14);
            for k in 0..=12 {
                let a = f.rotation - 90.0 + k as f64 * 15.0;
                ring.push(c + unit(a) * r);
            }
            ring.push(ring[0].clone());
            vec![ring]
        }
        FixtureShape::FloorLamp => vec![circle(c, r), circle(c, 0.075)],
        FixtureShape::Panel => panel_symbol(f),
        FixtureShape::Strip => strip_symbol(f),
    }
}

/// The strip's plan symbol: its path as a polyline, with tick marks every 0.3 m on the emitting
/// side (the right of each segment) when facing out.
fn strip_symbol(f: &LightFixture) -> Vec<Vec<Vec2>> {
    if f.path.len() < 2 || f.path.iter().any(|p| !p.is_finite()) {
        return Vec::new();
    }
    let mut polys: Vec<Vec<Vec2>> = Vec::new();
    let mut path_poly = f.path.clone();
    if f.closed {
        path_poly.push(f.path[0].clone());
    }
    polys.push(path_poly);

    if f.facing == StripFacing::Out {
        const TICK_LEN: f64 = 0.02;
        const TICK_SPACING: f64 = 0.3;
        for segment in f.path.windows(2) {
            let (a, b) = (segment[0], segment[1]);
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let seg_len = (dx * dx + dy * dy).sqrt();
            if !seg_len.is_finite() || seg_len < 0.01 {
                continue;
            }
            let count = (seg_len / TICK_SPACING).ceil() as usize;
            for k in 1..=count {
                let t = (k as f64) * TICK_SPACING / seg_len;
                if t >= 1.0 {
                    break;
                }
                let px = a.x + t * dx;
                let py = a.y + t * dy;
                let rx = dy / seg_len;
                let ry = -dx / seg_len;
                polys.push(vec![Vec2::new(px, py), Vec2::new(px + rx * TICK_LEN, py + ry * TICK_LEN)]);
            }
        }
    }
    polys
}

/// The catalogue panel's side (m), used for a side the fixture stores as 0 (or not finite).
const DEFAULT_PANEL_SIDE: f64 = 0.6;

/// A panel's plan symbol: its `width` x `length` rectangle centred at the position, the width along
/// `rotation` (degrees CCW), as a closed 5-point ring (corners in the order (-w,-l), (+w,-l),
/// (+w,+l), (-w,+l) along and across the rotation) followed by its two diagonals (corner 0 to 2,
/// then 1 to 3). A side that is 0 or not finite is the catalogue's 0.6 m.
fn panel_symbol(f: &LightFixture) -> Vec<Vec<Vec2>> {
    let side = |v: f64| if v.is_finite() && v > 0.0 { v } else { DEFAULT_PANEL_SIDE };
    let half_w = side(f.width) / 2.0;
    let half_l = side(f.length) / 2.0;
    let rot = if f.rotation.is_finite() { f.rotation } else { 0.0 };
    let along = unit(rot);
    let across = unit(rot + 90.0);
    let c = f.position;
    let corner = |sw: f64, sl: f64| c + along * (sw * half_w) + across * (sl * half_l);
    let corners = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
    vec![vec![corners[0], corners[1], corners[2], corners[3], corners[0]], vec![corners[0], corners[2]], vec![corners[1], corners[3]]]
}

/// Unit vector at `deg` degrees (CCW from +X).
fn unit(deg: f64) -> Vec2 {
    let rad = deg * std::f64::consts::PI / 180.0;
    Vec2::new(rad.cos(), rad.sin())
}

/// 25-point circle centred at `c` with radius `r` (last point repeats the first).
fn circle(c: Vec2, r: f64) -> Vec<Vec2> {
    let mut pts = Vec::with_capacity(25);
    for k in 0..=24 {
        let rad = (k as f64 / 24.0) * 2.0 * std::f64::consts::PI;
        pts.push(c + Vec2::new(rad.cos() * r, rad.sin() * r));
    }
    pts
}

/// Two-point line centred at `c` along unit direction `u`, length `2r`.
fn line(c: Vec2, u: Vec2, r: f64) -> Vec<Vec2> {
    vec![c - u * r, c + u * r]
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::FixtureShape;

    fn make(id: u32, shape: FixtureShape) -> LightFixture {
        let mut f = LightFixture::new(cadcraft_doc::ArchId(id), "pot-light", Vec2::new(5.0, 3.0));
        f.shape = shape;
        f
    }

    fn all_finite(pts: &[Vec2]) -> bool {
        pts.iter().all(|p| p.is_finite())
    }

    fn is_closed(ring: &[Vec2]) -> bool {
        !ring.is_empty() && ring[0] == *ring.last().unwrap()
    }

    #[test]
    fn plan_symbol_counts_per_shape() {
        let f0 = make(0, FixtureShape::RecessedCan);
        let s0 = plan_symbol(&f0);
        assert_eq!(s0.len(), 2);
        for ring in &s0 {
            assert_eq!(ring.len(), 25);
        }

        let f1 = make(1, FixtureShape::Pendant);
        let s1 = plan_symbol(&f1);
        assert_eq!(s1.len(), 3);
        assert_eq!(s1[0].len(), 25);
        assert_eq!(s1[1].len(), 2);
        assert_eq!(s1[2].len(), 2);

        let f2 = make(2, FixtureShape::SurfaceDome);
        let s2 = plan_symbol(&f2);
        assert_eq!(s2.len(), 9);
        assert_eq!(s2[0].len(), 25);
        for tick in &s2[1..] {
            assert_eq!(tick.len(), 2);
        }

        let f3 = make(3, FixtureShape::Sconce);
        let s3 = plan_symbol(&f3);
        assert_eq!(s3.len(), 1);
        assert_eq!(s3[0].len(), 14);

        let f4 = make(4, FixtureShape::FloorLamp);
        let s4 = plan_symbol(&f4);
        assert_eq!(s4.len(), 2);
        for ring in &s4 {
            assert_eq!(ring.len(), 25);
        }

        for f in [f0, f1, f2, f3, f4] {
            assert!(!plan_symbol(&f).is_empty());
        }
    }

    #[test]
    fn plan_symbol_rings_are_closed_and_sized() {
        // RecessedCan: two closed 25-point circles, outer peak at +r from centre.
        let f = make(0, FixtureShape::RecessedCan);
        let r = crate::light::housing_diameter(&f) / 2.0;
        for ring in &plan_symbol(&f) {
            assert!(is_closed(ring));
            assert!(all_finite(ring));
        }
        assert!((plan_symbol(&f)[0][0].x - (f.position.x + r)).abs() < 1e-6);

        // Pendant: closed circle + two 2-point diagonal lines at 45/135 degrees.
        let fp = make(1, FixtureShape::Pendant);
        let pp = plan_symbol(&fp);
        assert!(is_closed(&pp[0]));
        assert!(all_finite(&pp[0]));
        assert!(all_finite(&pp[1]) && all_finite(&pp[2]));
        let c = fp.position;
        let u45 = unit(45.0);
        let u135 = unit(135.0);
        for (i, polyline) in pp[1..].iter().enumerate() {
            let u = if i == 0 { u45 } else { u135 };
            assert!((polyline[0].x - (c.x - u.x * r)).abs() < 1e-6);
            assert!((polyline[0].y - (c.y - u.y * r)).abs() < 1e-6);
            assert!((polyline[1].x - (c.x + u.x * r)).abs() < 1e-6);
            assert!((polyline[1].y - (c.y + u.y * r)).abs() < 1e-6);
        }

        // SurfaceDome: closed 25-point circle + 8 two-point radial ticks.
        let fs = make(2, FixtureShape::SurfaceDome);
        for ring in &plan_symbol(&fs) {
            assert!(all_finite(ring));
            if ring.len() != 2 {
                assert!(is_closed(ring));
            }
        }

        // FloorLamp: two closed 25-point circles.
        let ff = make(4, FixtureShape::FloorLamp);
        for ring in &plan_symbol(&ff) {
            assert!(is_closed(ring));
            assert!(all_finite(ring));
        }
    }

    #[test]
    fn sconce_symbol_faces_its_rotation() {
        let mut f = make(10, FixtureShape::Sconce);
        f.rotation = 0.0;
        let sc = plan_symbol(&f);
        assert_eq!(sc.len(), 1);
        let ring = &sc[0];
        assert_eq!(ring.len(), 14);
        assert!(all_finite(ring));
        // The arc should peak at +X (facing the fixture rotation of 0 degrees).
        assert!((ring[6].x - (f.position.x + crate::light::housing_diameter(&f) / 2.0)).abs() < 1e-6);
        // The chord endpoints are at -90 and +90 degrees (vertical, at x = centre).
        assert!((ring[0].x - f.position.x).abs() < 1e-6);
        assert!((ring[13].x - f.position.x).abs() < 1e-6);
    }

    #[test]
    fn plan_symbol_of_a_non_finite_fixture_is_empty() {
        let f = LightFixture::new(cadcraft_doc::ArchId(0), "pot-light", Vec2::new(f64::NAN, 3.0));
        assert!(plan_symbol(&f).is_empty());
        let f = LightFixture::new(cadcraft_doc::ArchId(1), "pot-light", Vec2::new(5.0, f64::INFINITY));
        assert!(plan_symbol(&f).is_empty());
    }

    #[test]
    fn sconce_symbol_at_various_rotations() {
        for rot in [0.0, 90.0, 180.0, 270.0] {
            let mut f = make(20, FixtureShape::Sconce);
            f.rotation = rot;
            let sc = plan_symbol(&f);
            assert_eq!(sc.len(), 1);
            let ring = &sc[0];
            assert_eq!(ring.len(), 14);
            assert!(is_closed(ring));
            assert!(all_finite(ring));
            // The arc should peak in the direction of rotation
            let r = crate::light::housing_diameter(&f) / 2.0;
            let u = unit(rot);
            let expected_x = f.position.x + u.x * r;
            let expected_y = f.position.y + u.y * r;
            assert!((ring[6].x - expected_x).abs() < 1e-6);
            assert!((ring[6].y - expected_y).abs() < 1e-6);
        }
    }

    #[test]
    fn plan_symbol_with_huge_position() {
        let mut f = make(30, FixtureShape::RecessedCan);
        f.position = Vec2::new(1e10, 1e10);
        let s = plan_symbol(&f);
        assert_eq!(s.len(), 2);
        for ring in &s {
            assert!(all_finite(ring));
        }
        // The first point of the outer circle is at (c.x + r, c.y)
        let r = crate::light::housing_diameter(&f) / 2.0;
        assert!((s[0][0].x - (1e10 + r)).abs() < 1e-6);
        assert!((s[0][0].y - 1e10).abs() < 1e-6);
    }

    #[test]
    fn all_shapes_produce_finite_non_empty_points() {
        let shapes = [FixtureShape::RecessedCan, FixtureShape::Pendant, FixtureShape::SurfaceDome, FixtureShape::Sconce, FixtureShape::FloorLamp];
        for (i, shape) in shapes.iter().enumerate() {
            let f = make(100 + i as u32, *shape);
            let s = plan_symbol(&f);
            assert!(!s.is_empty(), "shape {:?} produced no symbol", shape);
            for ring in &s {
                assert!(!ring.is_empty(), "shape {:?} produced empty ring", shape);
                assert!(all_finite(ring), "shape {:?} produced non-finite point", shape);
            }
        }
    }

    #[test]
    fn pendant_diagonal_lines_are_correct() {
        let f = make(40, FixtureShape::Pendant);
        let s = plan_symbol(&f);
        assert_eq!(s.len(), 3);
        let r = crate::light::housing_diameter(&f) / 2.0;
        let c = f.position;
        // Line 1: 45 degrees
        let u45 = unit(45.0);
        assert!((s[1][0].x - (c.x - u45.x * r)).abs() < 1e-6);
        assert!((s[1][0].y - (c.y - u45.y * r)).abs() < 1e-6);
        assert!((s[1][1].x - (c.x + u45.x * r)).abs() < 1e-6);
        assert!((s[1][1].y - (c.y + u45.y * r)).abs() < 1e-6);
        // Line 2: 135 degrees
        let u135 = unit(135.0);
        assert!((s[2][0].x - (c.x - u135.x * r)).abs() < 1e-6);
        assert!((s[2][0].y - (c.y - u135.y * r)).abs() < 1e-6);
        assert!((s[2][1].x - (c.x + u135.x * r)).abs() < 1e-6);
        assert!((s[2][1].y - (c.y + u135.y * r)).abs() < 1e-6);
    }

    #[test]
    fn surface_dome_ticks_at_correct_angles() {
        let f = make(50, FixtureShape::SurfaceDome);
        let s = plan_symbol(&f);
        assert_eq!(s.len(), 9);
        let r = crate::light::housing_diameter(&f) / 2.0;
        let c = f.position;
        // Check each tick is at the correct angle
        for k in 0..8 {
            let u = unit(k as f64 * 45.0);
            let tick = &s[k + 1];
            assert_eq!(tick.len(), 2);
            // Inner point at r
            assert!((tick[0].x - (c.x + u.x * r)).abs() < 1e-6);
            assert!((tick[0].y - (c.y + u.y * r)).abs() < 1e-6);
            // Outer point at r + 0.05
            assert!((tick[1].x - (c.x + u.x * (r + 0.05))).abs() < 1e-6);
            assert!((tick[1].y - (c.y + u.y * (r + 0.05))).abs() < 1e-6);
        }
    }

    #[test]
    fn panel_symbol_is_its_rectangle_and_diagonals() {
        // The catalogue panel (0.6 x 0.6) at rotation 0.
        let t = crate::light::find_type("LED panel 600").unwrap();
        let mut f = crate::light::fixture_from_type(cadcraft_doc::ArchId(0), &t, Vec2::new(5.0, 3.0));
        let s = plan_symbol(&f);
        assert_eq!(s.len(), 3);
        assert_eq!((s[0].len(), s[1].len(), s[2].len()), (5, 2, 2));
        assert!(is_closed(&s[0]));
        for line in &s {
            assert!(all_finite(line));
        }
        let near = |a: Vec2, x: f64, y: f64| (a.x - x).abs() < 1e-9 && (a.y - y).abs() < 1e-9;
        assert!(near(s[0][0], 4.7, 2.7) && near(s[0][1], 5.3, 2.7) && near(s[0][2], 5.3, 3.3) && near(s[0][3], 4.7, 3.3));
        // The diagonals join opposite corners.
        assert_eq!((s[1][0], s[1][1]), (s[0][0], s[0][2]));
        assert_eq!((s[2][0], s[2][1]), (s[0][1], s[0][3]));

        // 90 degrees CCW turns the corners about the centre: (+0.3, -0.3) -> (+0.3, +0.3), ...
        f.rotation = 90.0;
        let s = plan_symbol(&f);
        assert!(near(s[0][0], 5.3, 2.7) && near(s[0][1], 5.3, 3.3) && near(s[0][2], 4.7, 3.3) && near(s[0][3], 4.7, 2.7));

        // A 45 degree rotation puts the corners on the axes, 0.3 * sqrt(2) from the centre.
        f.rotation = 45.0;
        let s = plan_symbol(&f);
        let d = 0.3 * std::f64::consts::SQRT_2;
        assert!(near(s[0][0], 5.0, 3.0 - d) && near(s[0][1], 5.0 + d, 3.0) && near(s[0][2], 5.0, 3.0 + d) && near(s[0][3], 5.0 - d, 3.0));

        // The width runs along the rotation, the length across it (1.2 x 0.4 at 90 degrees).
        f.rotation = 90.0;
        f.width = 1.2;
        f.length = 0.4;
        let s = plan_symbol(&f);
        assert!(near(s[0][0], 5.2, 2.4) && near(s[0][1], 5.2, 3.6) && near(s[0][2], 4.8, 3.6) && near(s[0][3], 4.8, 2.4));

        // A panel stored without a size draws the catalogue's 0.6 x 0.6.
        f.rotation = 0.0;
        f.width = 0.0;
        f.length = 0.0;
        let s = plan_symbol(&f);
        assert!(near(s[0][0], 4.7, 2.7) && near(s[0][2], 5.3, 3.3));

        // A non-finite position gives no symbol.
        f.position = Vec2::new(f64::NAN, 3.0);
        assert!(plan_symbol(&f).is_empty());
    }

    #[test]
    fn panel_symbol_with_huge_dimensions() {
        let t = crate::light::find_type("LED panel 600").unwrap();
        let mut f = crate::light::fixture_from_type(cadcraft_doc::ArchId(0), &t, Vec2::new(5.0, 3.0));
        f.width = 4.0;
        f.length = 4.0;
        let s = plan_symbol(&f);
        assert_eq!(s.len(), 3);
        let near = |a: Vec2, x: f64, y: f64| (a.x - x).abs() < 1e-9 && (a.y - y).abs() < 1e-9;
        assert!(near(s[0][0], 3.0, 1.0));
        assert!(near(s[0][1], 7.0, 1.0));
        assert!(near(s[0][2], 7.0, 5.0));
        assert!(near(s[0][3], 3.0, 5.0));
    }

    #[test]
    fn panel_symbol_with_non_finite_dimensions_defaults_to_catalogue() {
        let t = crate::light::find_type("LED panel 600").unwrap();
        let mut f = crate::light::fixture_from_type(cadcraft_doc::ArchId(0), &t, Vec2::new(5.0, 3.0));
        f.width = f64::NAN;
        f.length = f64::INFINITY;
        let s = plan_symbol(&f);
        assert_eq!(s.len(), 3);
        let near = |a: Vec2, x: f64, y: f64| (a.x - x).abs() < 1e-9 && (a.y - y).abs() < 1e-9;
        assert!(near(s[0][0], 4.7, 2.7));
        assert!(near(s[0][2], 5.3, 3.3));
    }

    #[test]
    fn panel_symbol_at_180_degree_rotation() {
        let t = crate::light::find_type("LED panel 600").unwrap();
        let mut f = crate::light::fixture_from_type(cadcraft_doc::ArchId(0), &t, Vec2::new(5.0, 3.0));
        f.rotation = 180.0;
        let s = plan_symbol(&f);
        assert_eq!(s.len(), 3);
        let near = |a: Vec2, x: f64, y: f64| (a.x - x).abs() < 1e-9 && (a.y - y).abs() < 1e-9;
        assert!(near(s[0][0], 5.3, 3.3));
        assert!(near(s[0][1], 4.7, 3.3));
        assert!(near(s[0][2], 4.7, 2.7));
        assert!(near(s[0][3], 5.3, 2.7));
    }
}
