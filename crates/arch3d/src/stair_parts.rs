//! A stair's meshes: treads, risers, stringers or skirt boards, and the landing slab.
//!
//! Every part is its own closed mesh (never two overlapping solids in one mesh) built in the
//! flight frame of [`cadcraft_arch::stair_geom::Flight`]: `u` along travel from `start` (the
//! centre of the bottom nosing line), `t` across to the left, `v` up from the from-level.

use cadcraft_arch::stair_geom::{
    CENTRE_STRINGER_WIDTH, Flight, NOSING, RISER_THICKNESS, SKIRT_ABOVE, SKIRT_BELOW, SKIRT_THICKNESS, STRINGER_ABOVE, STRINGER_THICKNESS,
    TREAD_THICKNESS,
};
use cadcraft_doc::arch::{Stair, Stringers};
use cadcraft_geom::Vec2;

use crate::mesh::Mesh;
use crate::solid::{self, Frame};
use crate::stairs;

/// A side board: material, `t` range, and its band about the nosing line (`below` under it,
/// `above` over it; `above` may be negative).
struct Side {
    material: &'static str,
    t0: f64,
    t1: f64,
    below: f64,
    above: f64,
}

/// Every mesh of `stair` standing on a level at `base`. `against` says which sides ([left, right])
/// stand against a wall. Order: treads, risers, stringers / skirts, landing. Nothing for a stair
/// that cannot be built.
pub(crate) fn stair_meshes(stair: &Stair, base: f64, against: [bool; 2]) -> Vec<Mesh> {
    let mut out = Vec::new();
    let Some(f) = Flight::new(stair, base) else { return out };
    let Some(frame) = Frame::new(stair.start, f.line.dir, base) else { return out };
    let (rh, td) = (f.line.riser, f.line.tread);
    let h = f.width / 2.0;
    let (s_left, s_right) = match stair.stringers {
        Stringers::Two => (STRINGER_THICKNESS, STRINGER_THICKNESS),
        Stringers::Centre => (0.0, 0.0),
        Stringers::None => (if against[0] { SKIRT_THICKNESS } else { 0.0 }, if against[1] { SKIRT_THICKNESS } else { 0.0 }),
    };
    let (t0, t1) = (-h + s_right, h - s_left);

    // A plan box over u in [ua, ub], the treads' t range, v in [va, vb] (v from the from-level).
    let board = |mesh: &mut Mesh, ua: f64, ub: f64, va: f64, vb: f64| {
        if t1 - t0 <= 1e-9 || ub - ua <= 1e-9 {
            return;
        }
        let centre = f.at((ua + ub) / 2.0, (t0 + t1) / 2.0);
        solid::push_box(mesh, centre, f.line.dir, ub - ua, t1 - t0, base + va, base + vb);
    };
    let keep = |m: Mesh, out: &mut Vec<Mesh>| {
        if !m.indices.is_empty() {
            out.push(m);
        }
    };

    for k in 1..=f.line.treads {
        let mut m = Mesh::new("stair-tread", stair.id);
        m.level_elevation = base;
        let top = k as f64 * rh;
        board(&mut m, f.nosing_u(k), k as f64 * td, top - TREAD_THICKNESS, top);
        keep(m, &mut out);
    }
    if !stair.open_risers {
        for k in 1..=stair.risers.min(f.line.treads + 1) {
            let mut m = Mesh::new("stair-riser", stair.id);
            m.level_elevation = base;
            let u = (k - 1) as f64 * td;
            let top = if k <= f.line.treads { k as f64 * rh - TREAD_THICKNESS } else { stair.rise };
            board(&mut m, u, u + RISER_THICKNESS, (k - 1) as f64 * rh, top);
            keep(m, &mut out);
        }
    }

    let under = f.underside;
    let mut sides: Vec<Side> = Vec::new();
    match stair.stringers {
        Stringers::Two => {
            sides.push(Side { material: "stair-stringer", t0: h - STRINGER_THICKNESS, t1: h, below: under, above: STRINGER_ABOVE });
            sides.push(Side { material: "stair-stringer", t0: -h, t1: -h + STRINGER_THICKNESS, below: under, above: STRINGER_ABOVE });
        }
        Stringers::Centre => sides.push(Side {
            material: "stair-stringer",
            t0: -CENTRE_STRINGER_WIDTH / 2.0,
            t1: CENTRE_STRINGER_WIDTH / 2.0,
            below: under,
            above: -rh - TREAD_THICKNESS,
        }),
        Stringers::None => {
            if against[0] {
                sides.push(Side { material: "stair-skirt", t0: h - SKIRT_THICKNESS, t1: h, below: SKIRT_BELOW, above: SKIRT_ABOVE });
            }
            if against[1] {
                sides.push(Side { material: "stair-skirt", t0: -h, t1: -h + SKIRT_THICKNESS, below: SKIRT_BELOW, above: SKIRT_ABOVE });
            }
        }
    }
    let (ua, ub) = (-NOSING, f.line.run);
    for side in sides {
        // The band nosing_v - below <= v <= nosing_v + above, cut to u in [-NOSING, run], v in [0, rise].
        let boxed = [Vec2::new(ua, 0.0), Vec2::new(ub, 0.0), Vec2::new(ub, f.line.rise), Vec2::new(ua, f.line.rise)];
        let lo = |u: f64| Vec2::new(u, f.nosing_v(u) - side.below);
        let hi = |u: f64| Vec2::new(u, f.nosing_v(u) + side.above);
        let cut = solid::clip_left(&boxed, lo(ua), lo(ub));
        let cut = if cut.is_empty() { cut } else { solid::clip_left(&cut, hi(ub), hi(ua)) };
        let mut m = Mesh::new(side.material, stair.id);
        m.level_elevation = base;
        solid::push_profile(&mut m, &cut, side.t0, side.t1, &frame);
        keep(m, &mut out);
    }

    if let Some(mut landing) = stairs::landing_mesh(stair, base) {
        landing.material = "stair-landing".to_string();
        keep(landing, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The phase-5 stair: start (2, 1), +x, width 1.0, level 0 to 1 at 2.7.
    fn phase5_stair() -> Stair {
        Stair {
            start: Vec2::new(2.0, 1.0),
            width: 1.0,
            rise: 2.7,
            risers: 16,
            riser_height: 0.16875,
            treads: 15,
            tread_depth: 0.254,
            run: 3.81,
            landing_depth: 1.0,
            ..Default::default()
        }
    }

    fn of<'a>(ms: &'a [Mesh], material: &str) -> Vec<&'a Mesh> {
        ms.iter().filter(|m| m.material == material).collect()
    }

    /// Bounds as [minx, miny, minz, maxx, maxy, maxz].
    fn bb(m: &Mesh) -> [f64; 6] {
        let b = m.bounds();
        [b.min[0], b.min[1], b.min[2], b.max[0], b.max[1], b.max[2]]
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-4
    }

    /// Is `p` strictly inside some mesh of `ms` (axis-aligned bounds of boxes; fine for boxes).
    fn inside(ms: &[Mesh], material: &str, p: [f64; 3]) -> bool {
        of(ms, material).iter().any(|m| {
            let b = bb(m);
            (0..3).all(|i| p[i] > b[i] && p[i] < b[i + 3])
        })
    }

    /// The stub once equalled the legacy solid; now only the landing still does.
    #[test]
    fn stub_matches_the_legacy_solid() {
        let s = phase5_stair();
        let got = stair_meshes(&s, 0.0, [false, false]);
        assert!(!got.is_empty());
        let want = stairs::landing_mesh(&s, 0.0).expect("landing");
        let last = got.last().expect("last");
        assert_eq!(last.positions, want.positions);
        assert_eq!(last.indices, want.indices);
        assert_eq!(last.material, "stair-landing");
    }

    #[test]
    fn phase5_stair_parts() {
        let ms = stair_meshes(&phase5_stair(), 0.0, [false, false]);
        assert_eq!(of(&ms, "stair-tread").len(), 15);
        assert_eq!(of(&ms, "stair-riser").len(), 16);
        assert_eq!(of(&ms, "stair-stringer").len(), 2);
        assert_eq!(of(&ms, "stair-landing").len(), 1);
        // Order: treads, risers, stringers, landing.
        assert_eq!(ms.first().map(|m| m.material.as_str()), Some("stair-tread"));
        assert_eq!(ms.last().map(|m| m.material.as_str()), Some("stair-landing"));
        assert_eq!(ms.len(), 15 + 16 + 2 + 1);
    }

    #[test]
    fn tread_and_nosing_probes() {
        let ms = stair_meshes(&phase5_stair(), 0.0, [false, false]);
        let t1 = bb(of(&ms, "stair-tread")[0]);
        assert!(near(t1[0], 1.975) && near(t1[3], 2.254), "{t1:?}");
        assert!(near(t1[2], 0.14375) && near(t1[5], 0.16875), "{t1:?}");
        assert!(inside(&ms, "stair-tread", [2.1, 1.0, 0.16]));
        assert!(inside(&ms, "stair-tread", [1.99, 1.0, 0.16]));
    }

    #[test]
    fn riser_probe() {
        let ms = stair_meshes(&phase5_stair(), 0.0, [false, false]);
        let r1 = bb(of(&ms, "stair-riser")[0]);
        assert!(near(r1[0], 2.0) && near(r1[3], 2.019) && near(r1[2], 0.0) && near(r1[5], 0.14375), "{r1:?}");
        assert!(inside(&ms, "stair-riser", [2.01, 1.0, 0.07]));
    }

    #[test]
    fn open_risers_build_none() {
        let s = Stair { open_risers: true, ..phase5_stair() };
        let ms = stair_meshes(&s, 0.0, [false, false]);
        assert!(of(&ms, "stair-riser").is_empty());
        assert_eq!(of(&ms, "stair-tread").len(), 15);
    }

    #[test]
    fn stringer_band() {
        let ms = stair_meshes(&phase5_stair(), 0.0, [false, false]);
        let st = of(&ms, "stair-stringer");
        assert!(st.iter().all(|m| m.is_manifold()));
        let right = st.iter().find(|m| bb(m)[1] < 0.9).map(|m| bb(m)).unwrap_or([0.0; 6]);
        assert!(near(right[1], 0.5) && near(right[4], 0.538), "{right:?}");
        // Over the whole flight the band stays within [0, rise] (the probes live in p17_stairs.rs).
        assert!(near(right[2], 0.0) && near(right[5], 2.7), "{right:?}");
    }

    #[test]
    fn treads_stop_at_the_stringers() {
        let ms = stair_meshes(&phase5_stair(), 0.0, [false, false]);
        let t = bb(of(&ms, "stair-tread")[0]);
        assert!(near(t[1], 0.538) && near(t[4], 1.462), "{t:?}");
        let r = bb(of(&ms, "stair-riser")[0]);
        assert!(near(r[1], 0.538) && near(r[4], 1.462), "{r:?}");
    }

    #[test]
    fn centre_stringer() {
        let s = Stair { stringers: Stringers::Centre, ..phase5_stair() };
        let ms = stair_meshes(&s, 0.0, [false, false]);
        let st = of(&ms, "stair-stringer");
        assert_eq!(st.len(), 1);
        let b = bb(st[0]);
        assert!(near(b[1], 0.981) && near(b[4], 1.019), "{b:?}");
        let t = bb(of(&ms, "stair-tread")[0]);
        assert!(near(t[1], 0.5) && near(t[4], 1.5), "{t:?}");
        assert!(st[0].is_manifold());
    }

    #[test]
    fn no_stringers_with_a_wall_side_gets_a_skirt() {
        let s = Stair { stringers: Stringers::None, start: Vec2::new(2.0, 0.6), ..phase5_stair() };
        let ms = stair_meshes(&s, 0.0, [false, true]);
        let sk = of(&ms, "stair-skirt");
        assert_eq!(sk.len(), 1);
        let b = bb(sk[0]);
        assert!(near(b[1], 0.1) && near(b[4], 0.119), "{b:?}");
        assert!(inside(&ms, "stair-skirt", [2.6, 0.11, 0.5]));
        assert!(of(&ms, "stair-stringer").is_empty());
        let t = bb(of(&ms, "stair-tread")[0]);
        assert!(near(t[1], 0.119) && near(t[4], 1.1), "{t:?}");
    }

    #[test]
    fn no_stringers_in_the_open_gets_nothing() {
        let s = Stair { stringers: Stringers::None, ..phase5_stair() };
        let ms = stair_meshes(&s, 0.0, [false, false]);
        assert!(of(&ms, "stair-skirt").is_empty() && of(&ms, "stair-stringer").is_empty());
        let t = bb(of(&ms, "stair-tread")[0]);
        assert!(near(t[1], 0.5) && near(t[4], 1.5), "{t:?}");
    }

    #[test]
    fn under_a_flight_is_air() {
        let ms = stair_meshes(&phase5_stair(), 0.0, [false, false]);
        for m in &ms {
            assert!(m.is_manifold(), "{}", m.material);
        }
        // Under tread 3 at the centre line there is no solid.
        let any = ms.iter().any(|m| {
            let b = bb(m);
            let p = [2.6, 1.0, 0.3];
            (0..3).all(|i| p[i] > b[i] && p[i] < b[i + 3])
        });
        assert!(!any);
    }

    #[test]
    fn the_flight_is_centred() {
        let ms = stair_meshes(&phase5_stair(), 0.0, [false, false]);
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for m in ms.iter().filter(|m| m.material != "stair-landing") {
            let b = bb(m);
            lo = lo.min(b[1]);
            hi = hi.max(b[4]);
        }
        assert!(near(lo, 0.5) && near(hi, 1.5), "{lo} {hi}");
    }

    #[test]
    fn direction_90_rotates_everything() {
        let s = Stair { direction: std::f64::consts::FRAC_PI_2, start: Vec2::new(4.0, 0.0), ..phase5_stair() };
        let ms = stair_meshes(&s, 0.0, [false, false]);
        let t = bb(of(&ms, "stair-tread")[0]);
        // Left of +y travel is -x: the flight spans x 3.5..4.5 less the stringers, y -0.025..0.254.
        assert!(near(t[0], 3.538) && near(t[3], 4.462), "{t:?}");
        assert!(near(t[1], -0.025) && near(t[4], 0.254), "{t:?}");
    }

    #[test]
    fn top_riser_reaches_the_rise() {
        let ms = stair_meshes(&phase5_stair(), 0.0, [false, false]);
        let rs = of(&ms, "stair-riser");
        let top = bb(rs[15]);
        assert!(near(top[5], 2.7), "{top:?}");
        assert!(near(top[0], 15.0 * 0.254 + 2.0), "{top:?}");
    }

    #[test]
    fn huge_and_nan_stairs_build_nothing_or_capped() {
        let nan = Stair { width: f64::NAN, ..phase5_stair() };
        assert!(stair_meshes(&nan, 0.0, [false, false]).is_empty());
        assert!(stair_meshes(&phase5_stair(), f64::NAN, [false, false]).is_empty());
        let huge = Stair { treads: usize::MAX, risers: usize::MAX, run: 1.0e9, rise: 1.0e9, ..phase5_stair() };
        let ms = stair_meshes(&huge, 0.0, [true, true]);
        assert!(ms.len() <= 1024 * 2 + 8, "{}", ms.len());
        let zero = Stair { riser_height: 0.0, ..phase5_stair() };
        assert!(stair_meshes(&zero, 0.0, [false, false]).is_empty());
    }

    #[test]
    fn every_part_is_a_closed_mesh() {
        for stringers in [Stringers::Two, Stringers::Centre, Stringers::None] {
            for open_risers in [false, true] {
                let s = Stair { stringers, open_risers, ..phase5_stair() };
                for m in stair_meshes(&s, 1.5, [true, true]) {
                    assert!(m.is_manifold(), "{} {stringers:?}", m.material);
                    assert!(near(m.level_elevation, 1.5));
                }
            }
        }
    }
}
