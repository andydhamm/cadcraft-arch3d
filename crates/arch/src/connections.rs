//! Plan connection validation and bulged connection arcs (Phase 15 electrical).
//!
//! A [`Connection`] runs from a switch to a set of [`LightFixture`]s. This module owns the
//! pure plan geometry and the validation rules:
//!
//! - [`device_plan`] / [`light_plan`] — the on-plan point of a device or a light.
//! - [`check_connection`] — validation rules for a connection (never clamped).
//! - [`bulged_polyline`] — the 17-point quadratic Bézier a connection bows.
//! - [`connection_arcs`] — one arc per light, lights sorted by id for stable output.
//!
//! Plan axes: x right, y up, metres. `bulge > 0` bows the arc to the LEFT of
//! switch→light; the sagitta at the arc midpoint is `bulge * half_chord` in the
//! perpendicular direction.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use cadcraft_doc::arch::{Connection, ElectricalKind, LightFixture};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::Vec2;

/// A connection drives at most 64 lights.
pub const MAX_CONNECTION_LIGHTS: usize = 64;

/// A group of switches transitively linked by the lights they drive.
///
/// Pure, derived data — never stored on a [`Drawing`]. A circuit is a transitive
/// group of switches connected through shared lights. Outside this module it must be
/// built with [`Circuit::default`] or [`Circuit::new`], never an exhaustive literal.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Circuit {
    /// The switch ids in the circuit, sorted ascending.
    pub switches: Vec<ArchId>,
    /// The light ids the circuit drives, sorted ascending and deduplicated.
    pub lights: Vec<ArchId>,
    /// Circuit brightness: the common brightness when all the lights agree, or their
    /// mean when they differ (1.0 when the circuit drives no readable light).
    pub brightness: f64,
}

impl Circuit {
    /// A circuit with no switches, no lights and brightness 1.0.
    pub fn new() -> Self {
        Circuit { switches: Vec::new(), lights: Vec::new(), brightness: 1.0 }
    }
}

/// A small union-find over the switch list used to merge switches sharing a light.
struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    /// `n` elements, each its own root.
    fn new(n: usize) -> Self {
        UnionFind { parent: (0..n).collect() }
    }

    /// Root of `i` with path halving (iterative, no recursion).
    fn find(&mut self, i: usize) -> usize {
        let mut x = i;
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Merge the sets containing `a` and `b`.
    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent[ra] = rb;
        }
    }
}

/// The device's on-plan point (the centre of its wall plate), if the device can be
/// resolved and placed on the wall. Returns `None` when the device is missing or
/// cannot be placed (e.g. it lies in a doorway or past the wall end).
pub fn device_plan(model: &ArchModel, id: ArchId) -> Option<Vec2> {
    model.electrical.iter().find(|d| d.id == id).and_then(|d| crate::electrical::place_device(model, d).ok().map(|p| p.at))
}

/// The light's on-plan point, if it exists and its position is finite.
pub fn light_plan(model: &ArchModel, id: ArchId) -> Option<Vec2> {
    model.lights.iter().find(|l| l.id == id).map(|l| l.position.clone()).filter(|p| p.is_finite())
}

/// Validate a connection, in this order, first failure wins. Never clamps.
pub fn check_connection(model: &ArchModel, from: ArchId, to: &[ArchId], bulge: f64) -> Result<(), String> {
    let device = model.electrical.iter().find(|d| d.id == from);
    match device {
        None => {
            return Err(format!("switch {} does not exist", cadcraft_doc::arch::electrical_label(from)));
        }
        Some(d) if !d.kind.is_switch() => {
            return Err("a connection must start from a switch".to_string());
        }
        _ => {}
    }

    if to.is_empty() {
        return Err("a connection needs at least one light".to_string());
    }
    if to.len() > MAX_CONNECTION_LIGHTS {
        return Err("a connection drives at most 64 lights".to_string());
    }

    // Duplicate light ids (compare a sorted copy).
    let mut sorted: Vec<ArchId> = to.iter().copied().collect();
    sorted.sort();
    if sorted.windows(2).any(|w| w[0] == w[1]) {
        return Err("a connection lists a light twice".to_string());
    }

    for id in to {
        let exists = model.lights.iter().any(|l| l.id == *id);
        if !exists {
            return Err(format!("light {} does not exist", cadcraft_doc::arch::light_label(*id)));
        }
    }

    if bulge.is_finite() && (-1.0..=1.0).contains(&bulge) { Ok(()) } else { Err("bulge must be a finite number in -1..=1".to_string()) }
}

/// A quadratic Bézier arc from `a` to `b` with sagitta `bulge * half_chord` to the
/// LEFT of a→b. 17 points (16 segments), `t = i / 16`. Returns `[a, b]` when the arc
/// is degenerate: non-finite `bulge`, `|bulge| < 1e-12`, or `a` ≈ `b`.
pub fn bulged_polyline(a: Vec2, b: Vec2, bulge: f64) -> Vec<Vec2> {
    if !bulge.is_finite() || bulge.abs() < 1e-12 || a.dist(b) < 1e-12 {
        return vec![a, b];
    }
    let mid = a.mid(b);
    let half = a.dist(b) / 2.0;
    let control = mid + (b - a).normalized().perp() * (2.0 * bulge * half);
    (0..=16)
        .map(|i| {
            let t = i as f64 / 16.0;
            let u = 1.0 - t;
            a * (u * u) + control * (2.0 * u * t) + b * (t * t)
        })
        .collect()
}

/// One arc per light, lights taken in `c.to` sorted by `ArchId` ascending (output is
/// independent of stored order). If the switch cannot be placed, returns an empty list;
/// lights that are missing are skipped.
pub fn connection_arcs(model: &ArchModel, c: &Connection) -> Vec<Vec<Vec2>> {
    let from = match device_plan(model, c.from) {
        Some(p) => p,
        None => return vec![],
    };
    let mut to: Vec<ArchId> = c.to.clone();
    to.sort();
    to.iter().filter_map(|&id| light_plan(model, id)).map(|light| bulged_polyline(from, light, c.bulge)).collect()
}

/// Group switches that share any light into circuits.
///
/// Two switches are in the same circuit when they are transitively linked by a light
/// they both drive (switch A — light L — switch B — light M — switch C is one circuit).
/// Only switches that drive at least one light take part: an outlet, a GFCI or a 240 V
/// device never forms a circuit. Returns one [`Circuit`] per transitive group, sorted
/// by the smallest switch id in each.
pub fn circuits(model: &ArchModel) -> Vec<Circuit> {
    // switch id -> lights it drives (only switches that drive a light).
    let mut per_switch: BTreeMap<ArchId, BTreeSet<ArchId>> = BTreeMap::new();
    for c in &model.connections {
        let is_switch = model.electrical.iter().find(|d| d.id == c.from).map_or(false, |d| d.kind.is_switch());
        if !is_switch {
            continue;
        }
        for &light in &c.to {
            per_switch.entry(c.from).or_default().insert(light);
        }
    }

    // Ordered switch list and its index into that list.
    let switches: Vec<ArchId> = per_switch.keys().copied().collect();
    let index: BTreeMap<ArchId, usize> = switches.iter().enumerate().map(|(i, &sw)| (sw, i)).collect();

    // light -> switches driving it (the link used by the union-find).
    let mut light_to_switches: BTreeMap<ArchId, BTreeSet<ArchId>> = BTreeMap::new();
    for (sw, lights) in &per_switch {
        for &light in lights {
            light_to_switches.entry(light).or_default().insert(*sw);
        }
    }

    let mut uf = UnionFind::new(switches.len());
    for sws in light_to_switches.values() {
        let mut first: Option<usize> = None;
        for &sw in sws {
            if let Some(i) = index.get(&sw) {
                match first {
                    None => first = Some(*i),
                    Some(fi) => uf.union(fi, *i),
                }
            }
        }
    }

    // Group by union-find root.
    let mut groups: Vec<Vec<ArchId>> = Vec::new();
    let mut root_to_group: BTreeMap<usize, usize> = BTreeMap::new();
    for (i, &sw) in switches.iter().enumerate() {
        let root = uf.find(i);
        let gid = match root_to_group.get(&root) {
            Some(&g) => g,
            None => {
                let gid = groups.len();
                groups.push(Vec::new());
                root_to_group.insert(root, gid);
                gid
            }
        };
        groups[gid].push(sw);
    }
    for g in groups.iter_mut() {
        g.sort();
    }
    groups.sort_by_key(|g| g[0]);

    // One circuit per group.
    groups
        .into_iter()
        .map(|switches| {
            let mut lights: BTreeSet<ArchId> = BTreeSet::new();
            for &sw in &switches {
                if let Some(ls) = per_switch.get(&sw) {
                    lights.extend(ls);
                }
            }
            let (mut sum, mut n) = (0.0f64, 0usize);
            for &light in &lights {
                if let Some(f) = model.lights.iter().find(|f2| f2.id == light) {
                    if f.brightness.is_finite() {
                        sum += f.brightness;
                        n += 1;
                    }
                }
            }
            Circuit { switches, lights: lights.into_iter().collect(), brightness: if n == 0 { 1.0 } else { sum / n as f64 } }
        })
        .collect()
}

/// The circuit a switch belongs to, or `None` when the switch drives no lights
/// (no connection, an outlet, or a model with no circuits at all).
pub fn circuit_of(model: &ArchModel, switch: ArchId) -> Option<Circuit> {
    circuits(model).into_iter().find(|c| c.switches.contains(&switch))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::arch::{Connection, ElectricalDevice, ElectricalKind, LightFixture, WallSide};
    use cadcraft_doc::{ArchId, ArchModel, Wall};
    use cadcraft_geom::Vec2;

    fn fixture() -> (ArchModel, ArchId) {
        let mut m = ArchModel::default();
        m.ensure_ground_level();
        let w = m.push_wall(Wall::new(Vec2::ZERO, Vec2::new(4.0, 0.0), 0.2));
        let sw = m.push_electrical(ElectricalDevice {
            wall: w,
            kind: ElectricalKind::SinglePole,
            side: WallSide::Left,
            offset: 1.0,
            height: 1.2,
            ..Default::default()
        });
        for (i, x) in [3.0, 3.5].iter().enumerate() {
            let mut f = LightFixture::new(ArchId(i as u32), "Recessed pot light", Vec2::new(*x, 0.1));
            f.id = ArchId(i as u32);
            m.lights.push(f);
        }
        (m, sw)
    }

    #[test]
    fn a_bulged_arc_bows_to_the_left() {
        let (m, sw) = fixture();

        let c = Connection { from: sw, to: vec![ArchId(0)], bulge: 0.5, ..Default::default() };
        let arcs = connection_arcs(&m, &c);
        assert_eq!(arcs.len(), 1);
        let a = &arcs[0];
        assert_eq!(a.len(), 17);
        assert!((a[0].x - 1.0).abs() <= 1e-9);
        assert!((a[0].y - 0.1).abs() <= 1e-9);
        assert!((a[16].x - 3.0).abs() <= 1e-9);
        assert!((a[16].y - 0.1).abs() <= 1e-9);
        assert!((a[8].x - 2.0).abs() <= 1e-9);
        assert!((a[8].y - 0.6).abs() <= 1e-9); // sagitta 0.5 to the left of +x

        let c0 = Connection { from: sw, to: vec![ArchId(0)], bulge: 0.0, ..Default::default() };
        let arcs0 = connection_arcs(&m, &c0);
        assert_eq!(arcs0.len(), 1);
        assert_eq!(arcs0[0].len(), 2); // no bulge -> straight line segment

        let cn = Connection { from: sw, to: vec![ArchId(0)], bulge: -0.5, ..Default::default() };
        let arcsn = connection_arcs(&m, &cn);
        assert_eq!(arcsn.len(), 1);
        assert!((arcsn[0][8].x - 2.0).abs() <= 1e-9);
        assert!((arcsn[0][8].y - (-0.4)).abs() <= 1e-9); // bows to the right
    }

    #[test]
    fn a_connection_from_an_outlet_is_refused() {
        let (mut m, sw) = fixture();
        let outlet = m.push_electrical(ElectricalDevice {
            wall: m.walls[0].id,
            kind: ElectricalKind::Duplex,
            side: WallSide::Left,
            offset: 2.0,
            height: 0.3,
            ..Default::default()
        });

        // From a non-switch device.
        assert!(check_connection(&m, outlet, &[ArchId(0)], 0.0).is_err());
        // From the switch: ok.
        assert!(check_connection(&m, sw, &[ArchId(0)], 0.0).is_ok());

        // No lights.
        assert!(check_connection(&m, sw, &[], 0.0).is_err());
        // Duplicate light.
        assert!(check_connection(&m, sw, &[ArchId(0), ArchId(0)], 0.0).is_err());
        // Light not present.
        assert!(check_connection(&m, sw, &[ArchId(9)], 0.0).is_err());
        // Bulge out of range.
        assert!(check_connection(&m, sw, &[ArchId(0)], 3.0).is_err());
        // Bulge non-finite.
        assert!(check_connection(&m, sw, &[ArchId(0)], f64::NAN).is_err());
        // More than 64 lights.
        let many: Vec<ArchId> = (0..65).map(ArchId).collect();
        assert!(check_connection(&m, sw, &many, 0.0).is_err());
    }

    #[test]
    fn connection_arcs_are_deterministic() {
        let (m, sw) = fixture();
        let c = Connection { from: sw, to: vec![ArchId(1), ArchId(0)], bulge: 0.25, ..Default::default() };

        let a = connection_arcs(&m, &c);
        let b = connection_arcs(&m, &c);
        assert_eq!(a, b);
        assert_eq!(a.len(), 2);
        assert!((a[0][16].x - 3.0).abs() <= 1e-9);
        assert!((a[1][16].x - 3.5).abs() <= 1e-9); // sorted by light id, not stored order

        // No switch -> empty, no panic.
        assert!(connection_arcs(&ArchModel::default(), &c).is_empty());
    }

    #[test]
    fn two_switches_sharing_lights_are_one_circuit() {
        let (mut m, s0) = fixture();

        let s1 = m.push_electrical(ElectricalDevice {
            wall: m.walls[0].id,
            kind: ElectricalKind::ThreeWay,
            side: WallSide::Left,
            offset: 2.0,
            height: 1.2,
            ..Default::default()
        });
        let outlet = m.push_electrical(ElectricalDevice {
            wall: m.walls[0].id,
            kind: ElectricalKind::Duplex,
            side: WallSide::Left,
            offset: 3.0,
            height: 0.3,
            ..Default::default()
        });

        m.push_connection(Connection { from: s0, to: vec![ArchId(0), ArchId(1)], ..Default::default() });
        m.push_connection(Connection { from: s1, to: vec![ArchId(1), ArchId(0)], bulge: 0.25, ..Default::default() });

        let cs = circuits(&m);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].switches, vec![s0, s1]);
        assert_eq!(cs[0].lights, vec![ArchId(0), ArchId(1)]);
        assert!((cs[0].brightness - 1.0).abs() <= 1e-9);
        assert_eq!(circuit_of(&m, s1), Some(cs[0].clone()));
        assert!(circuit_of(&m, outlet).is_none());
        assert!(circuits(&ArchModel::default()).is_empty());

        // A switch with no connection drives no lights.
        let none = m.push_electrical(ElectricalDevice {
            wall: m.walls[0].id,
            kind: ElectricalKind::SinglePole,
            side: WallSide::Left,
            offset: 0.5,
            height: 1.2,
            ..Default::default()
        });
        assert!(circuit_of(&m, none).is_none());
    }

    #[test]
    fn a_circuit_of_differing_lights_averages() {
        let (mut m, s0) = fixture();

        let s1 = m.push_electrical(ElectricalDevice {
            wall: m.walls[0].id,
            kind: ElectricalKind::ThreeWay,
            side: WallSide::Left,
            offset: 2.0,
            height: 1.2,
            ..Default::default()
        });

        m.lights[0].brightness = 0.4;
        m.lights[1].brightness = 0.2;

        m.push_connection(Connection { from: s0, to: vec![ArchId(0), ArchId(1)], ..Default::default() });
        m.push_connection(Connection { from: s1, to: vec![ArchId(1), ArchId(0)], bulge: 0.25, ..Default::default() });

        let cs = circuits(&m);
        assert_eq!(cs.len(), 1);
        assert!((cs[0].brightness - 0.3).abs() <= 1e-9);
        assert_eq!(circuit_of(&m, s0), Some(cs[0].clone()));
    }

    #[test]
    fn three_switches_in_a_chain_are_one_circuit() {
        let (mut m, s0) = fixture();

        let s1 = m.push_electrical(ElectricalDevice {
            wall: m.walls[0].id,
            kind: ElectricalKind::ThreeWay,
            side: WallSide::Left,
            offset: 2.0,
            height: 1.2,
            ..Default::default()
        });
        let s2 = m.push_electrical(ElectricalDevice {
            wall: m.walls[0].id,
            kind: ElectricalKind::ThreeWay,
            side: WallSide::Left,
            offset: 2.5,
            height: 1.2,
            ..Default::default()
        });

        // s0 -> l0, s1 -> l0 and l1, s2 -> l1: one transitive circuit.
        m.push_connection(Connection { from: s0, to: vec![ArchId(0)], ..Default::default() });
        m.push_connection(Connection { from: s1, to: vec![ArchId(0), ArchId(1)], ..Default::default() });
        m.push_connection(Connection { from: s2, to: vec![ArchId(1)], ..Default::default() });

        let cs = circuits(&m);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].switches, vec![s0, s1, s2]);
        assert_eq!(cs[0].lights, vec![ArchId(0), ArchId(1)]);
    }

    #[test]
    fn two_disjoint_groups_are_two_circuits() {
        let (mut m, s0) = fixture();

        let s1 = m.push_electrical(ElectricalDevice {
            wall: m.walls[0].id,
            kind: ElectricalKind::ThreeWay,
            side: WallSide::Left,
            offset: 2.0,
            height: 1.2,
            ..Default::default()
        });

        // s0 -> l0, s1 -> l1: no shared lights, two circuits.
        m.push_connection(Connection { from: s0, to: vec![ArchId(0)], ..Default::default() });
        m.push_connection(Connection { from: s1, to: vec![ArchId(1)], ..Default::default() });

        let cs = circuits(&m);
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0].switches, vec![s0]);
        assert_eq!(cs[0].lights, vec![ArchId(0)]);
        assert_eq!(cs[1].switches, vec![s1]);
        assert_eq!(cs[1].lights, vec![ArchId(1)]);
    }

    #[test]
    fn a_switch_with_an_empty_connection_is_in_no_circuit() {
        let (mut m, s0) = fixture();

        // A connection with no lights: the switch drives nothing.
        m.push_connection(Connection { from: s0, to: vec![], ..Default::default() });

        let cs = circuits(&m);
        assert!(cs.is_empty());
        assert!(circuit_of(&m, s0).is_none());
    }

    #[test]
    fn circuit_brightness_ignores_non_finite() {
        let (mut m, s0) = fixture();

        m.lights[0].brightness = f64::NAN;
        m.lights[1].brightness = 0.5;

        m.push_connection(Connection { from: s0, to: vec![ArchId(0), ArchId(1)], ..Default::default() });

        let cs = circuits(&m);
        assert_eq!(cs.len(), 1);
        // Only l1 has finite brightness, so the mean is 0.5.
        assert!((cs[0].brightness - 0.5).abs() <= 1e-9);
    }

    #[test]
    fn a_light_not_in_the_model_still_appears_in_the_circuit() {
        let (mut m, s0) = fixture();

        // Connect to l0 (exists) and l99 (does not exist).
        m.push_connection(Connection { from: s0, to: vec![ArchId(0), ArchId(99)], ..Default::default() });

        let cs = circuits(&m);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].lights, vec![ArchId(0), ArchId(99)]);
        // Brightness is based only on l0 (brightness 1.0 by default).
        assert!((cs[0].brightness - 1.0).abs() <= 1e-9);
    }

    #[test]
    fn circuit_of_finds_the_right_circuit() {
        let (mut m, s0) = fixture();

        let s1 = m.push_electrical(ElectricalDevice {
            wall: m.walls[0].id,
            kind: ElectricalKind::ThreeWay,
            side: WallSide::Left,
            offset: 2.0,
            height: 1.2,
            ..Default::default()
        });

        m.push_connection(Connection { from: s0, to: vec![ArchId(0)], ..Default::default() });
        m.push_connection(Connection { from: s1, to: vec![ArchId(1)], ..Default::default() });

        let c0 = circuit_of(&m, s0).unwrap();
        let c1 = circuit_of(&m, s1).unwrap();
        assert_eq!(c0.switches, vec![s0]);
        assert_eq!(c1.switches, vec![s1]);
        assert_ne!(c0, c1);
    }

    #[test]
    fn duplicate_connections_are_deduplicated() {
        let (mut m, s0) = fixture();

        // Push the same connection twice.
        m.push_connection(Connection { from: s0, to: vec![ArchId(0), ArchId(1)], ..Default::default() });
        m.push_connection(Connection { from: s0, to: vec![ArchId(0), ArchId(1)], ..Default::default() });

        let cs = circuits(&m);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].switches, vec![s0]);
        assert_eq!(cs[0].lights, vec![ArchId(0), ArchId(1)]);
    }

    #[test]
    fn a_connection_from_a_missing_switch_is_ignored() {
        let (mut m, _s0) = fixture();

        // A connection from a switch that doesn't exist.
        m.push_connection(Connection { from: ArchId(999), to: vec![ArchId(0)], ..Default::default() });

        let cs = circuits(&m);
        assert!(cs.is_empty());
    }
}
