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

use cadcraft_doc::arch::{Connection, ElectricalKind, LightFixture};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::Vec2;

/// A connection drives at most 64 lights.
pub const MAX_CONNECTION_LIGHTS: usize = 64;

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

        let c = Connection { id: ArchId(0), from: sw, to: vec![ArchId(0)], bulge: 0.5 };
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

        let c0 = Connection { id: ArchId(0), from: sw, to: vec![ArchId(0)], bulge: 0.0 };
        let arcs0 = connection_arcs(&m, &c0);
        assert_eq!(arcs0.len(), 1);
        assert_eq!(arcs0[0].len(), 2); // no bulge -> straight line segment

        let cn = Connection { id: ArchId(0), from: sw, to: vec![ArchId(0)], bulge: -0.5 };
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
        let c = Connection { id: ArchId(0), from: sw, to: vec![ArchId(1), ArchId(0)], bulge: 0.25 };

        let a = connection_arcs(&m, &c);
        let b = connection_arcs(&m, &c);
        assert_eq!(a, b);
        assert_eq!(a.len(), 2);
        assert!((a[0][16].x - 3.0).abs() <= 1e-9);
        assert!((a[1][16].x - 3.5).abs() <= 1e-9); // sorted by light id, not stored order

        // No switch -> empty, no panic.
        assert!(connection_arcs(&ArchModel::default(), &c).is_empty());
    }
}
