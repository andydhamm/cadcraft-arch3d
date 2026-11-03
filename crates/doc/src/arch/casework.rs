//! Cabinets and fixtures (phase 13): kitchen and bath casework and the plumbing and appliance
//! fixtures that go with it. Plain data on [`super::ArchModel`]; the catalogue, validation and
//! wall placement live in `arch::casework`.
//!
//! All lengths are metres, angles degrees. `position` is the centre of the item's BACK edge and
//! `rotation` the direction of its width axis: local +x = (cos r, sin r) runs along the width,
//! local +y = (-sin r, cos r) points out of the front. `elevation` is the bottom of the item
//! above its level's floor.

use super::ArchId;
use crate::Handle;
use cadcraft_geom::Vec2;
use serde::{Deserialize, Serialize};

/// Offset within the arch-handle range reserved for cabinets (label `cb{n}`).
pub const CABINET_HANDLE_OFFSET: u64 = 0x50000;
/// Offset within the arch-handle range reserved for fixtures (label `fx{n}`).
pub const FIXTURE_HANDLE_OFFSET: u64 = 0x58000;
/// Most cabinets in a drawing.
pub const MAX_CABINETS: usize = 1_024;
/// Most fixtures in a drawing.
pub const MAX_FIXTURES: usize = 1_024;

/// Where a cabinet sits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CabinetKind {
    /// On the floor, under a counter.
    #[default]
    Base,
    /// Hung on the wall above the counter.
    Wall,
    /// Floor to near ceiling (pantry, oven or linen cabinet).
    Tall,
}

/// The style used for a cabinet front.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrontStyle {
    /// Flat slab front.
    #[default]
    Slab,
    /// Shaker-style front with a recessed centre panel.
    Shaker,
    /// Raised panel front.
    RaisedPanel,
}

impl FrontStyle {
    /// Parses a front-style name as it appears in JSON or library data.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "slab" => Some(Self::Slab),
            "shaker" => Some(Self::Shaker),
            "raised-panel" => Some(Self::RaisedPanel),
            _ => None,
        }
    }

    /// Returns the canonical encoded name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Slab => "slab",
            Self::Shaker => "shaker",
            Self::RaisedPanel => "raised-panel",
        }
    }
}

impl FrontStyle {
    /// All supported front styles, in canonical order.
    pub const ALL: [FrontStyle; 3] = [Self::Slab, Self::Shaker, Self::RaisedPanel];
}

fn is_default_front_style(style: &FrontStyle) -> bool {
    *style == FrontStyle::Slab
}

/// What the front of a cabinet is made of.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Fronts {
    /// Hinged doors only.
    #[default]
    Doors,
    /// Stacked drawers only.
    Drawers,
    /// A drawer above doors.
    DrawerOverDoor,
}

/// A cabinet.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cabinet {
    pub id: ArchId,
    pub kind: CabinetKind,
    /// The level it stands on (its `index`).
    #[serde(default)]
    pub level: u32,
    /// Centre of the back edge.
    pub position: Vec2,
    /// Direction of the width axis, degrees counter-clockwise from +X.
    #[serde(default)]
    pub rotation: f64,
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    /// Bottom of the carcass above the level's floor.
    #[serde(default)]
    pub elevation: f64,
    pub fronts: Fronts,
    /// Door front style.
    #[serde(default, skip_serializing_if = "is_default_front_style")]
    pub door_style: FrontStyle,
    /// Drawer front style.
    #[serde(default, skip_serializing_if = "is_default_front_style")]
    pub drawer_style: FrontStyle,
    /// Library item id for the installed pull.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull: Option<String>,
    /// Symbol key for the pull in `ArchModel.symbols`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_symbol: Option<String>,
    /// Door count (0..=4).
    #[serde(default)]
    pub doors: u32,
    /// Drawer count (0..=6).
    #[serde(default)]
    pub drawers: u32,
    /// Whether a counter top covers it (base and tall only).
    #[serde(default)]
    pub counter: bool,
    pub counter_thickness: f64,
    /// How far the counter overhangs the front.
    pub counter_overhang: f64,
    /// Whether the base is recessed for a toe kick.
    #[serde(default)]
    pub toe_kick: bool,
}

/// The kind of plumbing or appliance fixture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FixtureKind {
    #[default]
    Sink,
    Lavatory,
    Toilet,
    Bathtub,
    Shower,
    Range,
    Refrigerator,
    Dishwasher,
}

/// A plumbing or appliance fixture.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fixture {
    pub id: ArchId,
    pub kind: FixtureKind,
    /// The level it stands on (its `index`).
    #[serde(default)]
    pub level: u32,
    /// Centre of the back edge.
    pub position: Vec2,
    /// Direction of the width axis, degrees counter-clockwise from +X.
    #[serde(default)]
    pub rotation: f64,
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    /// Bottom above the level's floor.
    #[serde(default)]
    pub elevation: f64,
}

/// The handle of cabinet `id`.
pub fn cabinet_handle(id: ArchId) -> Handle {
    Handle(super::ARCH_HANDLE_BASE.saturating_add(CABINET_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the cabinet handle range.
pub fn is_cabinet_handle(h: Handle) -> bool {
    let lo = super::ARCH_HANDLE_BASE.saturating_add(CABINET_HANDLE_OFFSET);
    h.0 >= lo && h.0 < lo.saturating_add(1u64 << 15)
}

/// `cb{n}`.
pub fn cabinet_label(id: ArchId) -> String {
    format!("cb{}", id.0)
}

/// The handle of fixture `id`.
pub fn fixture_handle(id: ArchId) -> Handle {
    Handle(super::ARCH_HANDLE_BASE.saturating_add(FIXTURE_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the fixture handle range.
pub fn is_fixture_handle(h: Handle) -> bool {
    let lo = super::ARCH_HANDLE_BASE.saturating_add(FIXTURE_HANDLE_OFFSET);
    h.0 >= lo && h.0 < lo.saturating_add(1u64 << 15)
}

/// `fx{n}`.
pub fn fixture_label(id: ArchId) -> String {
    format!("fx{}", id.0)
}

#[cfg(test)]
mod tests {
    use super::super::ArchModel;
    use super::*;

    fn cab() -> Cabinet {
        Cabinet {
            id: ArchId(0),
            kind: CabinetKind::Base,
            level: 0,
            position: Vec2::new(0.0, 0.0),
            rotation: 0.0,
            width: 0.6,
            depth: 0.6,
            height: 0.876,
            elevation: 0.0,
            fronts: Fronts::Doors,
            door_style: FrontStyle::Slab,
            drawer_style: FrontStyle::Slab,
            pull: None,
            pull_symbol: None,
            doors: 1,
            drawers: 0,
            counter: false,
            counter_thickness: 0.038,
            counter_overhang: 0.025,
            toe_kick: true,
        }
    }

    fn fix() -> Fixture {
        Fixture {
            id: ArchId(0),
            kind: FixtureKind::Sink,
            level: 0,
            position: Vec2::new(0.0, 0.0),
            rotation: 0.0,
            width: 0.8,
            depth: 0.55,
            height: 0.02,
            elevation: 0.914,
        }
    }

    #[test]
    fn front_style_names_round_trip() {
        let want = [FrontStyle::Slab, FrontStyle::Shaker, FrontStyle::RaisedPanel];
        for style in want {
            assert_eq!(FrontStyle::parse(style.name()), Some(style));
            assert_eq!(style.name(), format!("{}", style.name()));
        }
        assert_eq!(FrontStyle::ALL, want);
    }

    #[test]
    fn front_style_parse_refuses_others() {
        assert_eq!(FrontStyle::parse("slab"), Some(FrontStyle::Slab));
        assert_eq!(FrontStyle::parse("SLA B"), None);
        assert_eq!(FrontStyle::parse("shaker"), Some(FrontStyle::Shaker));
        assert_eq!(FrontStyle::parse("raised_panel"), None);
        assert_eq!(FrontStyle::parse("raised-panel"), Some(FrontStyle::RaisedPanel));
        assert_eq!(FrontStyle::parse(""), None);
    }

    #[test]
    fn a_phase13_cabinet_reads_with_slab_fronts_and_no_pull() {
        let json = serde_json::json!({
            "id": 7,
            "kind": "base",
            "level": 0,
            "position": [1.0, 2.0],
            "rotation": 90.0,
            "width": 0.6,
            "depth": 0.6,
            "height": 0.876,
            "elevation": 0.0,
            "fronts": "doors",
            "doors": 2,
            "drawers": 0,
            "counter": true,
            "counter_thickness": 0.038,
            "counter_overhang": 0.025,
            "toe_kick": true
        });
        let cabinet: Cabinet = serde_json::from_value(json).unwrap();
        assert_eq!(cabinet.door_style, FrontStyle::Slab);
        assert_eq!(cabinet.drawer_style, FrontStyle::Slab);
        assert_eq!(cabinet.pull, None);
        assert_eq!(cabinet.pull_symbol, None);
    }

    #[test]
    fn cabinet_with_parts_round_trips_through_serde() {
        let original = Cabinet {
            id: ArchId(5),
            kind: CabinetKind::Base,
            level: 2,
            position: Vec2::new(1.5, 2.5),
            rotation: 45.0,
            width: 0.9,
            depth: 0.6,
            height: 0.9,
            elevation: 0.1,
            fronts: Fronts::DrawerOverDoor,
            door_style: FrontStyle::RaisedPanel,
            drawer_style: FrontStyle::Shaker,
            pull: Some("pull-01".to_owned()),
            pull_symbol: Some("pull:brushed.nickel".to_owned()),
            doors: 2,
            drawers: 2,
            counter: true,
            counter_thickness: 0.04,
            counter_overhang: 0.03,
            toe_kick: true,
        };
        let json = serde_json::to_value(&original).unwrap();
        assert_eq!(json.get("door_style"), Some(&serde_json::Value::String("raised-panel".into())));
        assert_eq!(json.get("drawer_style"), Some(&serde_json::Value::String("shaker".into())));
        assert_eq!(json.get("pull"), Some(&serde_json::Value::String("pull-01".into())));
        assert_eq!(json.get("pull_symbol"), Some(&serde_json::Value::String("pull:brushed.nickel".into())));
        let round_trip: Cabinet = serde_json::from_value(json).unwrap();
        assert_eq!(round_trip, original);
    }

    #[test]
    fn cabinet_handles_and_labels_round_trip() {
        let c = cabinet_handle(ArchId(7));
        assert!(is_cabinet_handle(c));
        assert!(!is_fixture_handle(c));
        assert!(!is_cabinet_handle(super::super::light_handle(ArchId(7))));
        assert_eq!(cabinet_label(ArchId(3)), "cb3");
        let f = fixture_handle(ArchId(7));
        assert!(is_fixture_handle(f));
        assert!(!is_cabinet_handle(f));
        assert_eq!(fixture_label(ArchId(3)), "fx3");
        assert_eq!(super::super::part_handle(ArchId(7), "cabinet"), c);
        assert_eq!(super::super::part_handle(ArchId(7), "counter"), c);
        assert_eq!(super::super::part_handle(ArchId(7), "fixture"), f);
    }

    #[test]
    fn push_cabinet_and_push_fixture_assign_increasing_ids() {
        let mut m = ArchModel::default();
        assert!(m.is_empty());
        assert_eq!(m.push_cabinet(cab()), ArchId(0));
        assert_eq!(m.push_cabinet(cab()), ArchId(1));
        assert_eq!(m.push_fixture(fix()), ArchId(0));
        assert_eq!(m.push_fixture(fix()), ArchId(1));
        assert_eq!(m.cabinets.iter().map(|c| c.id.0).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(m.fixtures.iter().map(|f| f.id.0).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!((m.next_cabinet_id, m.next_fixture_id), (2, 2));
        assert!(!m.is_empty());
    }

    #[test]
    fn validate_refuses_more_cabinets_than_the_cap() {
        let mut m = ArchModel::default();
        for _ in 0..=MAX_CABINETS {
            m.push_cabinet(cab());
        }
        assert!(m.validate().iter().any(|s| s.contains("cabinets")));
        let mut m = ArchModel::default();
        for _ in 0..=MAX_FIXTURES {
            m.push_fixture(fix());
        }
        assert!(m.validate().iter().any(|s| s.contains("fixtures")));
        let mut m = ArchModel::default();
        m.push_cabinet(cab());
        m.next_cabinet_id = 0;
        assert!(m.validate().iter().any(|s| s.contains("next_cabinet_id")));
    }
}
