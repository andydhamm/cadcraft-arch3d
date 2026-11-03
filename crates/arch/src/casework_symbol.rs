//! Plan symbols of cabinets and fixtures (phase 13). Local frame: x along the width, centred on
//! the back edge; y out of the front. Every point is returned in world coordinates.

use cadcraft_doc::arch::{Cabinet, CabinetKind, Fixture, FixtureKind};
use cadcraft_geom::Vec2;

/// How far behind the front edge the fronts line sits (m).
const FRONT_INSET: f64 = 0.019;

/// Local-to-world mapper for an item at `pos` rotated `rot` degrees.
struct Frame {
    pos: Vec2,
    u: Vec2,
    v: Vec2,
}

impl Frame {
    fn new(pos: Vec2, rot: f64) -> Self {
        let u = Vec2::from_angle(rot.to_radians());
        Frame { pos, u, v: u.perp() }
    }
    fn p(&self, x: f64, y: f64) -> Vec2 {
        self.pos + self.u * x + self.v * y
    }
    fn rect(&self, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![self.p(x0, y0), self.p(x1, y0), self.p(x1, y1), self.p(x0, y1), self.p(x0, y0)]
    }
    fn line(&self, a: (f64, f64), b: (f64, f64)) -> Vec<Vec2> {
        vec![self.p(a.0, a.1), self.p(b.0, b.1)]
    }
    /// Closed ellipse centred (cx, cy) with radii rx, ry.
    fn ellipse(&self, cx: f64, cy: f64, rx: f64, ry: f64, n: usize) -> Vec<Vec2> {
        let mut r: Vec<Vec2> = (0..n)
            .map(|k| {
                let a = std::f64::consts::TAU * k as f64 / n as f64;
                self.p(cx + rx * a.cos(), cy + ry * a.sin())
            })
            .collect();
        if let Some(&f) = r.first() {
            r.push(f);
        }
        r
    }
    /// Closed rectangle with quarter-circle corners of radius `r`.
    fn rounded(&self, x0: f64, y0: f64, x1: f64, y1: f64, r: f64) -> Vec<Vec2> {
        let r = r.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0);
        let corners = [(x1 - r, y0 + r, -90.0), (x1 - r, y1 - r, 0.0), (x0 + r, y1 - r, 90.0), (x0 + r, y0 + r, 180.0)];
        let mut out = Vec::new();
        for (cx, cy, a0) in corners {
            for k in 0..=4 {
                let a = (a0 + 22.5 * k as f64).to_radians();
                out.push(self.p(cx + r * a.cos(), cy + r * a.sin()));
            }
        }
        if let Some(&f) = out.first() {
            out.push(f);
        }
        out
    }
}

fn finite(rings: Vec<Vec<Vec2>>) -> Vec<Vec<Vec2>> {
    if rings.iter().all(|r| r.iter().all(|p| p.is_finite())) { rings } else { Vec::new() }
}

/// The plan symbol of a cabinet: footprint, front line, counter outline (base) or the footprint
/// diagonals (wall cabinet).
pub fn cabinet_symbol(c: &Cabinet) -> Vec<Vec<Vec2>> {
    let f = Frame::new(c.position, c.rotation);
    let (hw, d) = (c.width / 2.0, c.depth);
    let mut out = vec![f.rect(-hw, 0.0, hw, d), f.line((-hw, d - FRONT_INSET), (hw, d - FRONT_INSET))];
    match c.kind {
        CabinetKind::Base if c.counter => {
            let o = c.counter_overhang;
            out.push(f.rect(-hw - o, -o, hw + o, d + o));
        }
        CabinetKind::Wall => {
            out.push(f.line((-hw, 0.0), (hw, d)));
            out.push(f.line((-hw, d), (hw, 0.0)));
        }
        _ => {}
    }
    finite(out)
}

/// The plan symbol of a fixture: footprint plus the detail of its kind.
pub fn fixture_symbol(x: &Fixture) -> Vec<Vec<Vec2>> {
    let f = Frame::new(x.position, x.rotation);
    let (w, d) = (x.width, x.depth);
    let hw = w / 2.0;
    let mut out = vec![f.rect(-hw, 0.0, hw, d)];
    match x.kind {
        FixtureKind::Sink => out.push(f.rect(-hw + 0.05, 0.05, hw - 0.05, d - 0.05)),
        FixtureKind::Lavatory => out.push(f.ellipse(0.0, d / 2.0, w * 0.4, d * 0.35, 24)),
        FixtureKind::Toilet => {
            let tank = 0.2_f64.min(d);
            out.push(f.rect(-hw, 0.0, hw, tank));
            out.push(f.ellipse(0.0, tank + (d - tank) / 2.0, hw, (d - tank) / 2.0, 24));
        }
        FixtureKind::Bathtub => out.push(f.rounded(-hw + 0.08, 0.08, hw - 0.08, d - 0.08, 0.1)),
        FixtureKind::Shower => {
            out.push(f.line((-hw, 0.0), (hw, d)));
            out.push(f.line((-hw, d), (hw, 0.0)));
        }
        FixtureKind::Range => {
            for (cx, cy) in [(-0.25, 0.25), (0.25, 0.25), (-0.25, 0.75), (0.25, 0.75)] {
                out.push(f.ellipse(w * cx, d * cy, 0.09, 0.09, 16));
            }
        }
        FixtureKind::Refrigerator => out.push(f.line((-hw, 0.0), (hw, d))),
        FixtureKind::Dishwasher => out.push(f.line((-hw, d - 0.05), (hw, d - 0.05))),
    }
    finite(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::casework::{cabinet_defaults, fixture_defaults};

    fn near(a: Vec2, x: f64, y: f64) -> bool {
        (a.x - x).abs() < 1e-9 && (a.y - y).abs() < 1e-9
    }

    #[test]
    fn cabinet_symbol_is_its_footprint_with_a_front_line() {
        let c = cabinet_defaults(CabinetKind::Base);
        let s = cabinet_symbol(&c);
        assert_eq!(s.len(), 3, "footprint, fronts, counter");
        assert_eq!(s[0].len(), 5);
        assert_eq!(s[0][0], s[0][4]);
        assert!(near(s[0][0], -0.3, 0.0) && near(s[0][2], 0.3, 0.6));
        assert!(near(s[1][0], -0.3, 0.6 - 0.019) && near(s[1][1], 0.3, 0.6 - 0.019));
        assert!(near(s[2][0], -0.3 - 0.025, -0.025));
    }

    #[test]
    fn wall_cabinet_symbol_has_diagonals() {
        let s = cabinet_symbol(&cabinet_defaults(CabinetKind::Wall));
        assert_eq!(s.len(), 4, "footprint, fronts, two diagonals");
        assert!(near(s[2][0], -0.3, 0.0) && near(s[2][1], 0.3, 0.305));
        assert!(near(s[3][0], -0.3, 0.305) && near(s[3][1], 0.3, 0.0));
    }

    #[test]
    fn sink_symbol_has_a_bowl() {
        let s = fixture_symbol(&fixture_defaults(FixtureKind::Sink));
        assert_eq!(s.len(), 2);
        let (o, i) = (&s[0], &s[1]);
        assert!(near(i[0], o[0].x + 0.05, o[0].y + 0.05));
        assert!(near(i[2], o[2].x - 0.05, o[2].y - 0.05));
    }

    #[test]
    fn range_symbol_has_four_burners() {
        let s = fixture_symbol(&fixture_defaults(FixtureKind::Range));
        assert_eq!(s.len(), 5);
        for ring in &s[1..] {
            assert_eq!(ring.len(), 17);
            assert_eq!(ring[0], ring[16]);
        }
    }

    #[test]
    fn symbols_follow_rotation() {
        let mut c = cabinet_defaults(CabinetKind::Base);
        c.position = Vec2::new(2.0, 3.0);
        c.rotation = 90.0;
        let s = cabinet_symbol(&c);
        // Local (-0.3, 0) -> (2, 2.7); local (0.3, 0.6) -> (1.4, 3.3).
        assert!(near(s[0][0], 2.0, 2.7) && near(s[0][2], 1.4, 3.3));
        let mut x = fixture_defaults(FixtureKind::Toilet);
        x.rotation = 45.0;
        assert!(fixture_symbol(&x).iter().flatten().all(|p| p.is_finite()));
    }

    #[test]
    fn cabinet_with_nan_position_has_no_symbol() {
        let mut c = cabinet_defaults(CabinetKind::Base);
        c.position = Vec2::new(f64::NAN, 0.0);
        let s = cabinet_symbol(&c);
        assert!(s.is_empty(), "NaN position should produce no symbol");
    }

    #[test]
    fn cabinet_with_nan_rotation_has_no_symbol() {
        let mut c = cabinet_defaults(CabinetKind::Base);
        c.rotation = f64::NAN;
        let s = cabinet_symbol(&c);
        assert!(s.is_empty(), "NaN rotation should produce no symbol");
    }

    #[test]
    fn cabinet_with_nan_width_has_no_symbol() {
        let mut c = cabinet_defaults(CabinetKind::Base);
        c.width = f64::NAN;
        let s = cabinet_symbol(&c);
        assert!(s.is_empty(), "NaN width should produce no symbol");
    }

    #[test]
    fn fixture_with_nan_depth_has_no_symbol() {
        let mut f = fixture_defaults(FixtureKind::Sink);
        f.depth = f64::NAN;
        let s = fixture_symbol(&f);
        assert!(s.is_empty(), "NaN depth should produce no symbol");
    }

    #[test]
    fn all_fixture_kinds_produce_symbols() {
        for kind in [
            FixtureKind::Sink,
            FixtureKind::Lavatory,
            FixtureKind::Toilet,
            FixtureKind::Bathtub,
            FixtureKind::Shower,
            FixtureKind::Range,
            FixtureKind::Refrigerator,
            FixtureKind::Dishwasher,
        ] {
            let f = fixture_defaults(kind);
            let s = fixture_symbol(&f);
            assert!(!s.is_empty(), "fixture {:?} should produce a symbol", kind);
            assert!(s.iter().all(|r| !r.is_empty()), "fixture {:?} should have non-empty rings", kind);
            assert!(s.iter().flatten().all(|p| p.is_finite()), "fixture {:?} should have finite points", kind);
        }
    }

    #[test]
    fn cabinet_rotated_180_flips_symbol() {
        let mut c = cabinet_defaults(CabinetKind::Base);
        c.rotation = 180.0;
        let s = cabinet_symbol(&c);
        // Local (-0.3, 0) -> (0.3, 0); local (0.3, 0.6) -> (-0.3, -0.6).
        assert!(near(s[0][0], 0.3, 0.0), "rotated 180 should flip x");
        assert!(near(s[0][2], -0.3, -0.6), "rotated 180 should flip both axes");
    }

    #[test]
    fn cabinet_with_huge_values_is_finite() {
        let mut c = cabinet_defaults(CabinetKind::Base);
        c.position = Vec2::new(1e12, 1e12);
        c.width = 1e6;
        c.depth = 1e6;
        let s = cabinet_symbol(&c);
        assert!(!s.is_empty(), "huge cabinet should produce a symbol");
        assert!(s.iter().flatten().all(|p| p.is_finite()), "all points should be finite");
    }

    #[test]
    fn cabinet_with_zero_depth_is_degenerate_but_finite() {
        let mut c = cabinet_defaults(CabinetKind::Base);
        c.depth = 0.0;
        let s = cabinet_symbol(&c);
        assert!(!s.is_empty(), "zero-depth cabinet should still produce a symbol");
        assert!(s.iter().flatten().all(|p| p.is_finite()), "all points should be finite");
    }
}
