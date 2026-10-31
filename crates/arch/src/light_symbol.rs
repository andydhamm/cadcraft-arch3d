//! Plan symbols of light fixtures (phase 10), drawn on `E-LITE`. Filled by `p10-arch-light-symbol`.

use cadcraft_doc::arch::LightFixture;
use cadcraft_geom::Vec2;

/// Plan symbol polylines (metres; a closed ring repeats its first point) of the fixture's shape at
/// its position, sized by `crate::light::housing_diameter`.
pub fn plan_symbol(f: &LightFixture) -> Vec<Vec<Vec2>> {
    let c = f.position;
    let d = crate::light::housing_diameter(f);
    let r = d / 2.0;
    if !c.is_finite() || !r.is_finite() {
        return Vec::new();
    }
    match f.shape {
        crate::light::FixtureShape::RecessedCan => vec![circle(c, r), circle(c, r / 2.0)],
        crate::light::FixtureShape::Pendant => {
            let u45 = unit(45.0);
            let u135 = unit(135.0);
            vec![circle(c, r), line(c, u45, r), line(c, u135, r)]
        }
        crate::light::FixtureShape::SurfaceDome => {
            let mut rings = vec![circle(c, r)];
            for k in 0..8 {
                let u = unit(k as f64 * 45.0);
                rings.push(vec![c + u * r, c + u * (r + 0.05)]);
            }
            rings
        }
        crate::light::FixtureShape::Sconce => {
            let mut ring = Vec::with_capacity(14);
            for k in 0..=12 {
                let a = f.rotation - 90.0 + k as f64 * 15.0;
                ring.push(c + unit(a) * r);
            }
            ring.push(ring[0].clone());
            vec![ring]
        }
        crate::light::FixtureShape::FloorLamp => vec![circle(c, r), circle(c, 0.075)],
    }
}

/// Unit vector at `deg` degrees (CCW from +X).
fn unit(deg: f64) -> Vec2 {
    let rad = deg * f64::consts::PI / 180.0;
    Vec2::new(rad.cos(), rad.sin())
}

/// 25-point circle centred at `c` with radius `r` (last point repeats the first).
fn circle(c: Vec2, r: f64) -> Vec<Vec2> {
    let mut pts = Vec::with_capacity(25);
    for k in 0..=24 {
        let rad = (k as f64 / 24.0) * 2.0 * f64::consts::PI;
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
    use crate::light::FixtureShape;

    fn make(id: u32, shape: FixtureShape) -> LightFixture {
        let mut f = LightFixture::new(id, "pot-light", Vec2::new(5.0, 3.0));
        f.shape = shape;
        f
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
        fn all_finite(pts: &[Vec2]) -> bool {
            pts.iter().all(|p| p.is_finite())
        }
        fn is_closed(ring: &[Vec2]) -> bool {
            !ring.is_empty() && ring[0] == ring.last().unwrap()
        }

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
        for (i, u) in pp[1..].iter().enumerate() {
            let u = if i == 0 { u45 } else { u135 };
            assert!((u[0].x - (c.x - u.x * r)).abs() < 1e-6);
            assert!((u[0].y - (c.y - u.y * r)).abs() < 1e-6);
            assert!((u[1].x - (c.x + u.x * r)).abs() < 1e-6);
            assert!((u[1].y - (c.y + u.y * r)).abs() < 1e-6);
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
        let f = make(10, FixtureShape::Sconce);
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
        let f = LightFixture::new(0, "pot-light", Vec2::new(f64::NAN, 3.0));
        assert!(plan_symbol(&f).is_empty());
        let f = LightFixture::new(1, "pot-light", Vec2::new(5.0, f64::POS_INFINITY));
        assert!(plan_symbol(&f).is_empty());
    }
}
