//! Phase 17 detail settings: stair rails and stringers, railing hosts, opening tops, grilles and
//! sides. Every type has a default that reproduces the drawing as it was before phase 17, and
//! every field using one skips it when serializing.

use serde::{Deserialize, Serialize};

/// Default depth of a stair's underside below the treads (metres).
pub const DEFAULT_UNDERSIDE: f64 = 0.3;
/// Smallest accepted stair underside.
pub const MIN_UNDERSIDE: f64 = 0.25;
/// Largest accepted stair underside.
pub const MAX_UNDERSIDE: f64 = 1.0;
/// Default plan cut height above the stair's from-level floor (metres).
pub const DEFAULT_PLAN_CUT: f64 = 1.2;
/// Default railing post spacing (metres).
pub const DEFAULT_POST_SPACING: f64 = 1.8;
/// Default distance a roof-top window's rough top stays under the roof underside (metres).
pub const DEFAULT_ROOF_OFFSET: f64 = 0.15;
/// Most lites across or down in a grille.
pub const MAX_GRILLE_LITES: u32 = 12;

pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}
pub(crate) fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}
pub(crate) fn is_default_underside(v: &f64) -> bool {
    *v == DEFAULT_UNDERSIDE
}
pub(crate) fn is_default_plan_cut(v: &f64) -> bool {
    *v == DEFAULT_PLAN_CUT
}
pub(crate) fn is_default_roof_offset(v: &f64) -> bool {
    *v == DEFAULT_ROOF_OFFSET
}
pub(crate) fn default_underside() -> f64 {
    DEFAULT_UNDERSIDE
}
pub(crate) fn default_plan_cut() -> f64 {
    DEFAULT_PLAN_CUT
}
pub(crate) fn default_roof_offset() -> f64 {
    DEFAULT_ROOF_OFFSET
}

/// Whether a railing is made on a side: automatically, always, or never.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RailSetting {
    /// Decided by the model (stairs: open sides; decks: edges above the code height).
    #[default]
    Auto,
    /// Always.
    On,
    /// Never.
    Off,
}

impl RailSetting {
    /// True for [`RailSetting::Auto`].
    pub fn is_auto(&self) -> bool {
        *self == RailSetting::Auto
    }
    /// Lower-case name.
    pub fn name(self) -> &'static str {
        match self {
            RailSetting::Auto => "auto",
            RailSetting::On => "on",
            RailSetting::Off => "off",
        }
    }
    /// Parse a name (trimmed, case-insensitive).
    pub fn parse(s: &str) -> Option<RailSetting> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(RailSetting::Auto),
            "on" => Some(RailSetting::On),
            "off" => Some(RailSetting::Off),
            _ => None,
        }
    }
}

/// Stringer layout under a stair's treads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stringers {
    /// One at each side.
    #[default]
    Two,
    /// One under the middle.
    Centre,
    /// None.
    None,
}

impl Stringers {
    /// True for [`Stringers::Two`].
    pub fn is_two(&self) -> bool {
        *self == Stringers::Two
    }
    /// Lower-case name.
    pub fn name(self) -> &'static str {
        match self {
            Stringers::Two => "two",
            Stringers::Centre => "centre",
            Stringers::None => "none",
        }
    }
    /// Parse a name (trimmed, case-insensitive; "center" is accepted).
    pub fn parse(s: &str) -> Option<Stringers> {
        match s.trim().to_ascii_lowercase().as_str() {
            "two" => Some(Stringers::Two),
            "centre" | "center" => Some(Stringers::Centre),
            "none" => Some(Stringers::None),
            _ => None,
        }
    }
}

/// What a railing's `follows` id names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RailingHost {
    /// The guard ringing a stair's well (the phase-5 meaning).
    #[default]
    Well,
    /// The railing on the left (+t) side of a stair's flight.
    StairLeft,
    /// The railing on the right side of a stair's flight.
    StairRight,
    /// The railing on a deck room's edges (`follows` is the room id).
    Deck,
}

impl RailingHost {
    /// True for [`RailingHost::Well`].
    pub fn is_well(&self) -> bool {
        *self == RailingHost::Well
    }
    /// camelCase name.
    pub fn name(self) -> &'static str {
        match self {
            RailingHost::Well => "well",
            RailingHost::StairLeft => "stairLeft",
            RailingHost::StairRight => "stairRight",
            RailingHost::Deck => "deck",
        }
    }
}

/// Shape of an opening's top.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpeningTop {
    /// Flat.
    #[default]
    Square,
    /// Semicircular.
    Round,
    /// A circular segment of a set rise.
    Segment,
    /// Follows the roof underside.
    Roof,
}

impl OpeningTop {
    /// True for [`OpeningTop::Square`].
    pub fn is_square(&self) -> bool {
        *self == OpeningTop::Square
    }
    /// Lower-case name.
    pub fn name(self) -> &'static str {
        match self {
            OpeningTop::Square => "square",
            OpeningTop::Round => "round",
            OpeningTop::Segment => "segment",
            OpeningTop::Roof => "roof",
        }
    }
    /// Parse a name (trimmed, case-insensitive).
    pub fn parse(s: &str) -> Option<OpeningTop> {
        match s.trim().to_ascii_lowercase().as_str() {
            "square" => Some(OpeningTop::Square),
            "round" => Some(OpeningTop::Round),
            "segment" => Some(OpeningTop::Segment),
            "roof" => Some(OpeningTop::Roof),
            _ => None,
        }
    }
}

/// A window grille: lites across and down. 1 x 1 has no bars.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Grille {
    /// Lites across.
    pub across: u32,
    /// Lites down.
    pub down: u32,
    /// The upper half of the glass only.
    pub top_only: bool,
}

impl Default for Grille {
    fn default() -> Self {
        Grille { across: 1, down: 1, top_only: false }
    }
}

/// Which sides of the host wall carry a sill or casing (left or right of start -> end).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct OpeningSides {
    /// Left of start -> end.
    pub left: bool,
    /// Right of start -> end.
    pub right: bool,
}

impl Default for OpeningSides {
    fn default() -> Self {
        OpeningSides::RIGHT
    }
}

impl OpeningSides {
    /// Right side only (today's sill and casing).
    pub const RIGHT: OpeningSides = OpeningSides { right: true, ..OpeningSides::NONE };
    /// Both sides.
    pub const BOTH: OpeningSides = OpeningSides { left: true, ..OpeningSides::RIGHT };
    /// Neither side.
    pub const NONE: OpeningSides = OpeningSides { left: false, right: false };
    /// True for the default, right only.
    pub fn is_right_only(&self) -> bool {
        *self == OpeningSides::RIGHT
    }
    /// "left" | "right" | "both" | "none".
    pub fn name(self) -> &'static str {
        match (self.left, self.right) {
            (true, true) => "both",
            (true, false) => "left",
            (false, true) => "right",
            (false, false) => "none",
        }
    }
    /// Parse a name (trimmed, case-insensitive).
    pub fn parse(s: &str) -> Option<OpeningSides> {
        match s.trim().to_ascii_lowercase().as_str() {
            "left" => Some(OpeningSides { left: true, right: false }),
            "right" => Some(OpeningSides::RIGHT),
            "both" => Some(OpeningSides::BOTH),
            "none" => Some(OpeningSides::NONE),
            _ => None,
        }
    }
}
