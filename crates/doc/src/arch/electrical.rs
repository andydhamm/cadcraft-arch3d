//! Electrical devices, connections and light sets (phase 15).
//!
//! Plain data on [`super::ArchModel`]; the 2D plan symbols and 3D housings live in
//! `crates/arch` / `crates/arch3d`, the commands in `crates/engine`.

use super::ArchId;
use crate::Handle;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Offset within the arch-handle range reserved for electrical device proxy handles
/// (label `el{n}`).
pub const ELECTRICAL_HANDLE_OFFSET: u64 = 0x68000;
/// Offset within the arch-handle range reserved for connection proxy handles
/// (label `cn{n}`).
pub const CONNECTION_HANDLE_OFFSET: u64 = 0x70000;
/// Most electrical devices in a drawing.
pub const MAX_ELECTRICAL: usize = 4096;
/// Most connections in a drawing.
pub const MAX_CONNECTIONS: usize = 4096;

/// The kind of electrical device.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElectricalKind {
    /// A standard duplex outlet.
    #[default]
    Duplex,
    /// A GFCI (ground-fault circuit interrupter) outlet.
    Gfci,
    /// A 240 V outlet.
    Outlet240,
    /// A single-pole switch.
    SinglePole,
    /// A three-way switch.
    ThreeWay,
    /// A dimmer switch.
    Dimmer,
}

impl ElectricalKind {
    /// The JSON key for this kind.
    pub fn key(self) -> &'static str {
        match self {
            ElectricalKind::Duplex => "duplex",
            ElectricalKind::Gfci => "gfci",
            ElectricalKind::Outlet240 => "240v",
            ElectricalKind::SinglePole => "single-pole",
            ElectricalKind::ThreeWay => "three-way",
            ElectricalKind::Dimmer => "dimmer",
        }
    }

    /// Parse a key into an `ElectricalKind`, or `None` if unknown.
    pub fn parse(s: &str) -> Option<Self> {
        for k in [
            ElectricalKind::Duplex,
            ElectricalKind::Gfci,
            ElectricalKind::Outlet240,
            ElectricalKind::SinglePole,
            ElectricalKind::ThreeWay,
            ElectricalKind::Dimmer,
        ] {
            if k.key() == s {
                return Some(k);
            }
        }
        None
    }

    /// Whether this kind is a switch (drawn on `E-LITE`).
    pub fn is_switch(self) -> bool {
        matches!(self, ElectricalKind::SinglePole | ElectricalKind::ThreeWay | ElectricalKind::Dimmer)
    }

    /// The DXF layer for this kind: outlets on `E-POWR`, switches on `E-LITE`.
    pub fn layer(self) -> &'static str {
        if self.is_switch() { "E-LITE" } else { "E-POWR" }
    }

    /// The default height of the plate centre above the level floor (m).
    pub fn default_height(self) -> f64 {
        if self.is_switch() { 1.2 } else { 0.3 }
    }
}

/// Which side of the wall the device is on. Left/right of the wall's start→end,
/// the same sides as surface keys `wall/w3/left|right`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WallSide {
    #[default]
    Left,
    Right,
}

/// An electrical device mounted on a wall.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ElectricalDevice {
    pub id: ArchId,
    pub kind: ElectricalKind,
    /// The wall the device is mounted on.
    pub wall: ArchId,
    pub side: WallSide,
    /// Distance from the wall's start to the plate centre (m).
    pub offset: f64,
    /// Height of the plate centre above the level floor (m).
    pub height: f64,
    /// When `true`, `height` is measured from the counter top in front, not the floor.
    pub counter: bool,
    /// The level the device is on.
    pub level: u32,
}

impl Default for ElectricalDevice {
    fn default() -> Self {
        ElectricalDevice {
            id: ArchId(0),
            kind: ElectricalKind::default(),
            wall: ArchId(0),
            side: WallSide::default(),
            offset: 0.0,
            height: 0.3,
            counter: false,
            level: 0,
        }
    }
}

/// A connection from a switch to one or more light fixtures.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Connection {
    pub id: ArchId,
    /// The switch this connection starts from.
    pub from: ArchId,
    /// The light fixtures (`lf{n}`) this connection drives.
    pub to: Vec<ArchId>,
    /// The arc's sagitta as a fraction of half the chord (-1..1); 0 = straight.
    pub bulge: f64,
}

impl Default for Connection {
    fn default() -> Self {
        Connection { id: ArchId(0), from: ArchId(0), to: Vec::new(), bulge: 0.0 }
    }
}

/// The state of one light in a light set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightState {
    pub on: bool,
    pub brightness: f64,
}

impl Default for LightState {
    fn default() -> Self {
        LightState { on: true, brightness: 1.0 }
    }
}

/// A named set of light states, keyed by light label (`lf{n}`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightSet {
    pub name: String,
    pub lights: BTreeMap<String, LightState>,
}

/// The handle of electrical device `id`.
pub fn electrical_handle(id: ArchId) -> Handle {
    Handle(super::ARCH_HANDLE_BASE.saturating_add(ELECTRICAL_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the electrical-device handle range.
pub fn is_electrical_handle(h: Handle) -> bool {
    let lo = super::ARCH_HANDLE_BASE.saturating_add(ELECTRICAL_HANDLE_OFFSET);
    h.0 >= lo && h.0 < lo.saturating_add(1u64 << 15)
}

/// The handle of connection `id`.
pub fn connection_handle(id: ArchId) -> Handle {
    Handle(super::ARCH_HANDLE_BASE.saturating_add(CONNECTION_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the connection handle range.
pub fn is_connection_handle(h: Handle) -> bool {
    let lo = super::ARCH_HANDLE_BASE.saturating_add(CONNECTION_HANDLE_OFFSET);
    h.0 >= lo && h.0 < lo.saturating_add(1u64 << 15)
}

/// `el{n}`.
pub fn electrical_label(id: ArchId) -> String {
    format!("el{}", id.0)
}

/// `cn{n}`.
pub fn connection_label(id: ArchId) -> String {
    format!("cn{}", id.0)
}

/// Parse an electrical label `el{n}` into an `ArchId`.
pub fn parse_electrical_label(s: &str) -> Option<ArchId> {
    let s = s.trim();
    let digits = if s.starts_with("el") || s.starts_with("EL") { &s[2..] } else { s };
    u32::from_str_radix(digits, 10).ok().map(ArchId)
}

/// Parse a connection label `cn{n}` into an `ArchId`.
pub fn parse_connection_label(s: &str) -> Option<ArchId> {
    let s = s.trim();
    let digits = if s.starts_with("cn") || s.starts_with("CN") { &s[2..] } else { s };
    u32::from_str_radix(digits, 10).ok().map(ArchId)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn electrical_kinds_round_trip_their_keys() {
        let kinds = [
            ElectricalKind::Duplex,
            ElectricalKind::Gfci,
            ElectricalKind::Outlet240,
            ElectricalKind::SinglePole,
            ElectricalKind::ThreeWay,
            ElectricalKind::Dimmer,
        ];
        for k in kinds {
            assert_eq!(ElectricalKind::parse(k.key()), Some(k));
        }
        assert_eq!(ElectricalKind::parse("bogus"), None);
        assert_eq!(ElectricalKind::parse(""), None);
    }

    #[test]
    fn electrical_handles_and_labels_round_trip() {
        let id = ArchId(7);
        let h = electrical_handle(id);
        assert!(is_electrical_handle(h));
        assert_eq!(electrical_label(id), "el7");
        assert_eq!(parse_electrical_label("el7"), Some(id));
        assert_eq!(parse_electrical_label("EL7"), Some(id));
        assert_eq!(parse_electrical_label("bogus"), None);

        let cn_id = ArchId(3);
        let ch = connection_handle(cn_id);
        assert!(is_connection_handle(ch));
        assert_eq!(connection_label(cn_id), "cn3");
        assert_eq!(parse_connection_label("cn3"), Some(cn_id));
        assert_eq!(parse_connection_label("CN3"), Some(cn_id));
        assert_eq!(parse_connection_label("bogus"), None);

        // Ranges are disjoint.
        assert!(!is_electrical_handle(connection_handle(cn_id)));
        assert!(!is_connection_handle(electrical_handle(id)));
    }
}
