//! The plan symbol of a stair (phase 17): nosing lines solid up to the plan cut, a break line,
//! the lines beyond dashed and a walk arrow UP on the lower level; the part seen through the
//! well, solid, with an arrow DN on the upper level. Pure 2D, centred on the stair's `start`.

use cadcraft_doc::{ArchModel, Stair};
use cadcraft_geom::Vec2;

use crate::stair_geom::{self, Flight, MAX_FLIGHT_TREADS, NOSING};

/// Plan cut used when a stair's own is not a positive finite number.
const DEFAULT_CUT: f64 = 1.2;
/// Length of the arrowhead strokes.
const HEAD: f64 = 0.1;
/// Distance of the UP / DN label from the flight end.
const LABEL_GAP: f64 = 0.15;
/// Height of the UP / DN label.
const LABEL_HEIGHT: f64 = 0.15;

/// What a symbol line is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolRole {
    /// A nosing line across the width.
    Nosing,
    /// The diagonal break line at the plan cut.
    Break,
    /// A side line along the flight.
    Side,
    /// The walk arrow (shaft and head strokes).
    Arrow,
    /// The outline of the well.
    Well,
    /// The UP / DN text (never a line).
    Label,
}

impl SymbolRole {
    /// The lower-case role name.
    pub fn name(self) -> &'static str {
        match self {
            SymbolRole::Nosing => "nosing",
            SymbolRole::Break => "break",
            SymbolRole::Side => "side",
            SymbolRole::Arrow => "arrow",
            SymbolRole::Well => "well",
            SymbolRole::Label => "label",
        }
    }
}

/// One polyline of the symbol.
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolLine {
    /// What the line is for.
    pub role: SymbolRole,
    /// Drawn dashed (above the plan cut).
    pub dashed: bool,
    /// Plan points; a closed line repeats its first point.
    pub points: Vec<Vec2>,
}

/// The UP / DN text.
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolLabel {
    /// The text.
    pub text: String,
    /// Centre of the text.
    pub at: Vec2,
    /// Reading direction in radians (along the flight).
    pub angle: f64,
    /// Text height in metres.
    pub height: f64,
}

/// A stair's plan symbol.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StairSymbol {
    /// The lines.
    pub lines: Vec<SymbolLine>,
    /// The UP / DN label.
    pub label: Option<SymbolLabel>,
}

impl StairSymbol {
    fn line(&mut self, role: SymbolRole, dashed: bool, points: Vec<Vec2>) {
        if points.len() >= 2 && points.iter().all(|p| p.is_finite()) && self.lines.len() < 4 * MAX_FLIGHT_TREADS {
            self.lines.push(SymbolLine { role, dashed, points });
        }
    }

    /// An arrow along `t = 0` from `u0` to `u1`; the head points toward `u1`.
    fn arrow(&mut self, f: &Flight, u0: f64, u1: f64) {
        if !(u0.is_finite() && u1.is_finite()) || (u1 - u0).abs() < 1e-9 {
            return;
        }
        self.line(SymbolRole::Arrow, false, vec![f.at(u0, 0.0), f.at(u1, 0.0)]);
        let back = if u1 > u0 { -1.0 } else { 1.0 };
        let (c, s) = (std::f64::consts::FRAC_PI_6.cos() * HEAD, std::f64::consts::FRAC_PI_6.sin() * HEAD);
        for side in [-1.0, 1.0] {
            self.line(SymbolRole::Arrow, false, vec![f.at(u1, 0.0), f.at(u1 + back * c, side * s)]);
        }
    }
}

/// The plan symbol of `s` for the plan showing `level` (`None`: all levels, which draws the lower
/// symbol and the well). `None` for a stair without a flight, or a level the stair does not touch.
pub fn stair_symbol(model: &ArchModel, s: &Stair, level: Option<u32>) -> Option<StairSymbol> {
    let f = stair_geom::flight(model, s)?;
    let well = well_ring(s);
    match level {
        None => {
            let mut sym = lower(&f, s);
            if let Some(ring) = well {
                sym.line(SymbolRole::Well, false, ring);
            }
            Some(sym)
        }
        Some(l) if l == s.from_level => Some(lower(&f, s)),
        Some(l) if l == s.to_level => Some(upper(&f, s, well)),
        Some(_) => None,
    }
}

/// The closed well outline, when the well has three or more finite points.
fn well_ring(s: &Stair) -> Option<Vec<Vec2>> {
    if s.well.len() < 3 || !s.well.iter().all(|p| p.is_finite()) {
        return None;
    }
    let mut ring = s.well.clone();
    ring.push(s.well[0]);
    Some(ring)
}

fn lower(f: &Flight, s: &Stair) -> StairSymbol {
    let mut sym = StairSymbol::default();
    let cut = if s.plan_cut.is_finite() && s.plan_cut > 0.0 { s.plan_cut } else { DEFAULT_CUT };
    let (l, w) = (&f.line, f.width / 2.0);
    let treads = l.treads.min(MAX_FLIGHT_TREADS);
    let solid_n = (1..=treads).take_while(|k| *k as f64 * l.riser <= cut + 1e-9).count();
    for k in 1..=treads {
        let u = f.nosing_u(k);
        sym.line(SymbolRole::Nosing, k > solid_n, vec![f.at(u, -w), f.at(u, w)]);
    }
    let broken = solid_n >= 1 && solid_n < treads;
    let (mut shaft_end, run) = (l.run - HEAD, l.run);
    if broken {
        let us = f.nosing_u(solid_n);
        let (ua, ub) = (us + 0.2 * l.tread, us + 0.8 * l.tread);
        sym.line(SymbolRole::Break, false, vec![f.at(ua, -w), f.at(ub, w)]);
        sym.line(SymbolRole::Side, false, vec![f.at(-NOSING, -w), f.at(ua, -w)]);
        sym.line(SymbolRole::Side, true, vec![f.at(ua, -w), f.at(run, -w)]);
        sym.line(SymbolRole::Side, false, vec![f.at(-NOSING, w), f.at(ub, w)]);
        sym.line(SymbolRole::Side, true, vec![f.at(ub, w), f.at(run, w)]);
        shaft_end = us + 0.5 * l.tread;
    } else {
        sym.line(SymbolRole::Side, false, vec![f.at(-NOSING, -w), f.at(run, -w)]);
        sym.line(SymbolRole::Side, false, vec![f.at(-NOSING, w), f.at(run, w)]);
    }
    sym.arrow(f, -NOSING + 0.1, shaft_end);
    sym.label = label("UP", f, s, -NOSING - LABEL_GAP);
    sym
}

fn upper(f: &Flight, s: &Stair, well: Option<Vec<Vec2>>) -> StairSymbol {
    let mut sym = StairSymbol::default();
    let (l, w) = (&f.line, f.width / 2.0);
    let u_w = well.as_ref().map(|ring| ring.iter().map(|p| f.local(*p).0).fold(f64::INFINITY, f64::min)).filter(|u| u.is_finite()).unwrap_or(-NOSING);
    for k in 1..=l.treads.min(MAX_FLIGHT_TREADS) {
        let u = f.nosing_u(k);
        if u >= u_w - 1e-9 {
            sym.line(SymbolRole::Nosing, false, vec![f.at(u, -w), f.at(u, w)]);
        }
    }
    if u_w < l.run {
        sym.line(SymbolRole::Side, false, vec![f.at(u_w, -w), f.at(l.run, -w)]);
        sym.line(SymbolRole::Side, false, vec![f.at(u_w, w), f.at(l.run, w)]);
    }
    if let Some(ring) = well {
        sym.line(SymbolRole::Well, false, ring);
    }
    if u_w + 0.1 < l.run - 0.1 {
        sym.arrow(f, l.run - 0.1, u_w + 0.1);
    }
    sym.label = label("DN", f, s, l.run + LABEL_GAP);
    sym
}

fn label(text: &str, f: &Flight, s: &Stair, u: f64) -> Option<SymbolLabel> {
    let at = f.at(u, 0.0);
    (at.is_finite() && s.direction.is_finite()).then(|| SymbolLabel { text: text.into(), at, angle: s.direction, height: LABEL_HEIGHT })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::Level;

    fn model(stair: Stair) -> (ArchModel, Stair) {
        let mut m = ArchModel::default();
        m.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        m.levels.push(Level { index: 1, elevation: 2.7, ..Default::default() });
        let id = m.push_stair(stair);
        let s = m.stairs.iter().find(|s| s.id == id).cloned().unwrap();
        (m, s)
    }

    fn phase5() -> Stair {
        Stair {
            start: Vec2::new(2.0, 1.0),
            well: vec![Vec2::new(2.508, 0.5), Vec2::new(5.81, 0.5), Vec2::new(5.81, 1.5), Vec2::new(2.508, 1.5)],
            ..Stair::default()
        }
    }

    fn of(sym: &StairSymbol, role: SymbolRole, dashed: bool) -> Vec<&SymbolLine> {
        sym.lines.iter().filter(|l| l.role == role && l.dashed == dashed).collect()
    }

    #[test]
    fn lower_symbol_brief_numbers() {
        let (m, s) = model(phase5());
        let sym = stair_symbol(&m, &s, Some(0)).unwrap();
        let solid = of(&sym, SymbolRole::Nosing, false);
        let dashed = of(&sym, SymbolRole::Nosing, true);
        assert_eq!((solid.len(), dashed.len()), (7, 8));
        for (k, l) in solid.iter().enumerate() {
            assert!((l.points[0].x - (1.975 + 0.254 * k as f64)).abs() < 1e-9);
            assert!((l.points[0].y - 0.5).abs() < 1e-9 && (l.points[1].y - 1.5).abs() < 1e-9);
        }
        assert!((dashed[0].points[0].x - 3.753).abs() < 1e-9);
        assert!((dashed[7].points[0].x - 5.531).abs() < 1e-9);
        let brk = of(&sym, SymbolRole::Break, false);
        assert_eq!(brk.len(), 1);
        assert!((brk[0].points[0].x - 3.5498).abs() < 1e-9 && (brk[0].points[1].x - 3.7022).abs() < 1e-9);
        assert!((brk[0].points[0].y - 0.5).abs() < 1e-9 && (brk[0].points[1].y - 1.5).abs() < 1e-9);
        assert_eq!(of(&sym, SymbolRole::Side, false).len(), 2);
        assert_eq!(of(&sym, SymbolRole::Side, true).len(), 2);
        assert_eq!(of(&sym, SymbolRole::Arrow, false).len(), 3);
        assert!(of(&sym, SymbolRole::Well, false).is_empty());
        let label = sym.label.unwrap();
        assert_eq!(label.text, "UP");
        assert!((label.at.x - 1.825).abs() < 1e-9 && (label.at.y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn plan_cut_moves_the_break() {
        let (m, s) = model(Stair { plan_cut: 1.5, ..phase5() });
        let sym = stair_symbol(&m, &s, Some(0)).unwrap();
        assert_eq!((of(&sym, SymbolRole::Nosing, false).len(), of(&sym, SymbolRole::Nosing, true).len()), (8, 7));
        // Non-finite or non-positive cuts fall back to 1.2.
        for bad in [f64::NAN, 0.0, -1.0] {
            let (m, s) = model(Stair { plan_cut: bad, ..phase5() });
            let sym = stair_symbol(&m, &s, Some(0)).unwrap();
            assert_eq!(of(&sym, SymbolRole::Nosing, false).len(), 7);
        }
    }

    #[test]
    fn a_short_flight_has_no_break() {
        let (m, s) = model(Stair { plan_cut: 10.0, ..phase5() });
        let sym = stair_symbol(&m, &s, Some(0)).unwrap();
        assert!(of(&sym, SymbolRole::Break, false).is_empty());
        assert_eq!(of(&sym, SymbolRole::Nosing, false).len(), 15);
        assert!(sym.lines.iter().all(|l| !l.dashed));
        assert_eq!(of(&sym, SymbolRole::Side, false).len(), 2);
    }

    #[test]
    fn upper_symbol_shows_the_well_part() {
        let (m, s) = model(phase5());
        let sym = stair_symbol(&m, &s, Some(1)).unwrap();
        let solid = of(&sym, SymbolRole::Nosing, false);
        assert_eq!(solid.len(), 12);
        assert!((solid[0].points[0].x - 2.737).abs() < 1e-9);
        assert!(sym.lines.iter().all(|l| !l.dashed));
        assert_eq!(of(&sym, SymbolRole::Well, false).len(), 1);
        assert_eq!(sym.label.as_ref().unwrap().text, "DN");
        let arrow = of(&sym, SymbolRole::Arrow, false);
        assert_eq!(arrow.len(), 3);
        // The shaft points down the flight: from the run end toward the well start.
        assert!(arrow[0].points[0].x > arrow[0].points[1].x);
    }

    #[test]
    fn all_levels_draws_the_lower_symbol_and_the_well() {
        let (m, s) = model(phase5());
        let all = stair_symbol(&m, &s, None).unwrap();
        let low = stair_symbol(&m, &s, Some(0)).unwrap();
        assert_eq!(all.lines.len(), low.lines.len() + 1);
        assert_eq!(of(&all, SymbolRole::Well, false).len(), 1);
        assert_eq!(all.label.unwrap().text, "UP");
    }

    #[test]
    fn other_levels_draw_nothing() {
        let (m, s) = model(phase5());
        assert!(stair_symbol(&m, &s, Some(2)).is_none());
    }

    #[test]
    fn direction_90_rotates() {
        let (m, s) = model(Stair { direction: std::f64::consts::FRAC_PI_2, ..phase5() });
        let sym = stair_symbol(&m, &s, Some(0)).unwrap();
        let first = &of(&sym, SymbolRole::Nosing, false)[0].points;
        // Travel is +y; left of travel is -x: the line spans x 1.5 to 0.5 at y = 1 - 0.025.
        assert!((first[0].y - 0.975).abs() < 1e-9 && (first[0].x - 2.5).abs() < 1e-9, "{first:?}");
        assert!((first[1].x - 1.5).abs() < 1e-9);
        let label = sym.label.unwrap();
        assert!((label.angle - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn hostile_stairs_give_none() {
        for bad in [
            Stair { width: f64::NAN, ..phase5() },
            Stair { tread_depth: 0.0, ..phase5() },
            Stair { start: Vec2::new(f64::INFINITY, 0.0), ..phase5() },
            Stair { riser_height: -1.0, ..phase5() },
        ] {
            let (m, s) = model(bad);
            assert!(stair_symbol(&m, &s, Some(0)).is_none());
            assert!(stair_symbol(&m, &s, None).is_none());
        }
        // A NaN in the well draws no well and cannot move the upper start.
        let (m, s) = model(Stair { well: vec![Vec2::ZERO, Vec2::new(f64::NAN, 0.0), Vec2::new(2.0, 1.0)], ..phase5() });
        let sym = stair_symbol(&m, &s, None).unwrap();
        assert!(of(&sym, SymbolRole::Well, false).is_empty());
        let up = stair_symbol(&m, &s, Some(1)).unwrap();
        assert_eq!(of(&up, SymbolRole::Nosing, false).len(), 15);
        // A tiny tread depth is capped.
        let (m, s) = model(Stair { run: 10.0, tread_depth: 0.001, treads: usize::MAX, ..phase5() });
        let sym = stair_symbol(&m, &s, Some(0)).unwrap();
        assert!(sym.lines.len() <= 4 * MAX_FLIGHT_TREADS);
    }
}
