//! Architectural data model: walls and their identifiers, stored on [`Drawing::arch`]
//! so that undo, autosave, AUDIT and DXF save all see them.
//!
//! This is the data layer (L1). 2D algorithms (joins, room detection, roof generation)
//! live in `crates/arch` (L2) and operate on this model.

use crate::Handle;
use cadcraft_geom::{Bounds2, Vec2};
use serde::{Deserialize, Serialize};

pub mod backdrop;
pub mod casework;
pub mod ceiling;
pub mod countertop;
pub mod detail;
pub mod electrical;
pub mod finish;
pub mod foundation;
pub mod furnishing;
pub mod light;
pub mod look;
pub mod molding;
pub mod paint;
pub mod planview;
pub mod sheet;
pub mod site;
pub use backdrop::*;
pub use casework::*;
pub use ceiling::*;
pub use countertop::*;
pub use detail::*;
pub use electrical::*;
pub use finish::*;
pub use foundation::*;
pub use furnishing::*;
pub use light::*;
pub use look::*;
pub use molding::*;
pub use paint::*;
pub use planview::*;
pub use sheet::*;
pub use site::*;

/// A stable identifier for an architectural object.
///
/// Assigned by [`ArchModel::add_wall`]; monotonically increasing within a drawing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ArchId(pub u32);

impl ArchId {
    /// Build an [`ArchId`] from a raw number.
    pub fn wall(n: u32) -> Self {
        ArchId(n)
    }
}

impl std::fmt::Display for ArchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "w{}", self.0)
    }
}

impl std::str::FromStr for ArchId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let digits = if s.starts_with('w') || s.starts_with('W') { &s[1..] } else { s };
        let n = u32::from_str_radix(digits, 10).map_err(|_| format!("invalid arch id `{s}`"))?;
        Ok(ArchId(n))
    }
}

/// Base of the synthetic handle range reserved for architectural objects.
///
/// Lives at the very top of the u64 space so that real DXF entity handles —
/// which are arbitrary 64-bit hex values — can never collide with it.
/// Arch objects are not entities in the store; render, grip and pick layers
/// route on [`is_arch_handle`] to distinguish them from real entity handles.
pub const ARCH_HANDLE_BASE: u64 = 0xFFFF_FFFF_0000_0000;

/// The synthetic proxy handle for an architectural object.
///
/// Arch objects have no entity handle, so selection and picking map back to the
/// [`ArchId`] through this reserved range.
pub fn arch_handle(id: ArchId) -> Handle {
    Handle(ARCH_HANDLE_BASE.saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the synthetic arch-handle range (walls, openings, rooms).
pub fn is_arch_handle(h: Handle) -> bool {
    h.0 >= ARCH_HANDLE_BASE && h.0 <= ARCH_HANDLE_BASE.saturating_add(1u64 << 32)
}

/// Size of the range above [`ARCH_HANDLE_BASE`] that arch proxies actually use (every object
/// kind sits well below it); a file handle above it is an ordinary handle.
pub const ARCH_HANDLE_SPAN: u64 = 0x10_0000;

/// Whether a handle read from a file collides with the arch proxies' working range.
pub fn is_reserved_file_handle(h: Handle) -> bool {
    h.0 >= ARCH_HANDLE_BASE && h.0 < ARCH_HANDLE_BASE.saturating_add(ARCH_HANDLE_SPAN)
}

/// The synthetic proxy handle for an architectural room.
pub fn room_handle(id: ArchId) -> Handle {
    Handle(ARCH_HANDLE_BASE.saturating_add(ROOM_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the synthetic room-handle range.
pub fn is_room_handle(h: Handle) -> bool {
    h.0 >= ARCH_HANDLE_BASE.saturating_add(ROOM_HANDLE_OFFSET) && h.0 < ARCH_HANDLE_BASE.saturating_add(ROOM_HANDLE_OFFSET).saturating_add(1u64 << 15)
}

/// Human-readable label for a room id, e.g. `r0`, `r1`.
pub fn room_label(id: ArchId) -> String {
    format!("r{}", id.0)
}

/// Offset within the arch-handle range reserved for opening proxy handles.
///
/// Wall handles occupy `[ARCH_HANDLE_BASE, ARCH_HANDLE_BASE + OPENING_HANDLE_OFFSET)`.
/// Opening handles occupy `[ARCH_HANDLE_BASE + OPENING_HANDLE_OFFSET, ...)` and remain
/// recognisable by [`is_arch_handle`] (the full `1 << 32` range).
pub const OPENING_HANDLE_OFFSET: u64 = 0x8000;

/// The synthetic proxy handle for an architectural opening.
///
/// Openings are not entities; their proxy handle lives in the upper half of the
/// arch range so picking/selection can disambiguate from wall handles.
pub fn opening_handle(id: ArchId) -> Handle {
    Handle(ARCH_HANDLE_BASE.saturating_add(OPENING_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the synthetic opening-handle range.
pub fn is_opening_handle(h: Handle) -> bool {
    h.0 >= ARCH_HANDLE_BASE.saturating_add(OPENING_HANDLE_OFFSET)
        && h.0 < ARCH_HANDLE_BASE.saturating_add(OPENING_HANDLE_OFFSET).saturating_add(1u64 << 15)
}

/// The one JSON spelling of a handle: for an arch proxy, its low bits over
/// `0xA000_0000`, lower-case hex (`w3` -> `a0000003`, `o0` -> `a0008000`);
/// any other handle is plain lower-case hex.
pub fn handle_json(h: Handle) -> String {
    match h.0.checked_sub(ARCH_HANDLE_BASE) {
        Some(low) => format!("{:x}", 0xA000_0000u64.saturating_add(low)),
        None => format!("{:x}", h.0),
    }
}

/// Proxy handle of the object a 3D probe `part` (see `arch3d::probe::part_of`)
/// belongs to; unknown parts are walls.
pub fn part_handle(id: ArchId, part: &str) -> Handle {
    match part {
        "leaf" | "frame" | "casing" | "glass" | "sill" | "header" | "grille" => opening_handle(id),
        "floor" | "ceiling" | "deck" | "slab" | "ledger" | "joist" | "rim" | "beam" => room_handle(id),
        "roof" => roof_handle(id),
        "stair" | "tread" | "riser" | "stringer" | "skirt" => stair_handle(id),
        "railing" | "post" => railing_handle(id),
        "soffit" => soffit_handle(id),
        "foundation" | "footing" => foundation_handle(id),
        "plant" => plant_handle(id),
        "light" => light_handle(id),
        "cabinet" | "counter" => cabinet_handle(id),
        "fixture" => fixture_handle(id),
        _ => arch_handle(id),
    }
}

/// Human-readable label for an opening id, e.g. `o0`, `o1`.
pub fn opening_label(id: ArchId) -> String {
    format!("o{}", id.0)
}

/// Human-readable label for a wall id, e.g. `w0`, `w1`.
pub fn wall_label(id: ArchId) -> String {
    format!("w{}", id.0)
}

/// Offset within the arch-handle range reserved for roof proxy handles.
/// Roof handles occupy `[ARCH_HANDLE_BASE + ROOF_HANDLE_OFFSET, ...)`.
pub const ROOF_HANDLE_OFFSET: u64 = 0x18000;

/// Maximum number of roofs in a drawing.
pub const MAX_ROOFS: usize = 64;

/// Maximum number of roof edges in a single roof.
pub const MAX_ROOF_EDGES: usize = 256;

/// Maximum number of roof planes in a single roof.
pub const MAX_ROOF_PLANES: usize = 256;

/// The synthetic proxy handle for an architectural roof.
pub fn roof_handle(id: ArchId) -> Handle {
    Handle(ARCH_HANDLE_BASE.saturating_add(ROOF_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the synthetic roof-handle range.
pub fn is_roof_handle(h: Handle) -> bool {
    h.0 >= ARCH_HANDLE_BASE.saturating_add(ROOF_HANDLE_OFFSET) && h.0 < ARCH_HANDLE_BASE.saturating_add(ROOF_HANDLE_OFFSET).saturating_add(1u64 << 15)
}

/// Human-readable label for a roof id, e.g. `r0`, `r1`.
pub fn roof_label(id: ArchId) -> String {
    format!("r{}", id.0)
}

/// Offset within the arch-handle range reserved for stair proxy handles.
/// Stair handles occupy `[ARCH_HANDLE_BASE + STAIR_HANDLE_OFFSET, ...)`.
pub const STAIR_HANDLE_OFFSET: u64 = 0x20000;

/// Maximum number of stairs in a drawing.
pub const MAX_STAIRS: usize = 512;

/// The synthetic proxy handle for an architectural stair.
pub fn stair_handle(id: ArchId) -> Handle {
    Handle(ARCH_HANDLE_BASE.saturating_add(STAIR_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the synthetic stair-handle range.
pub fn is_stair_handle(h: Handle) -> bool {
    h.0 >= ARCH_HANDLE_BASE.saturating_add(STAIR_HANDLE_OFFSET)
        && h.0 < ARCH_HANDLE_BASE.saturating_add(STAIR_HANDLE_OFFSET).saturating_add(1u64 << 15)
}

/// Human-readable label for a stair id, e.g. `s0`, `s1`.
pub fn stair_label(id: ArchId) -> String {
    format!("s{}", id.0)
}

/// Offset within the arch-handle range reserved for railing proxy handles.
/// Railing handles occupy `[ARCH_HANDLE_BASE + RAILING_HANDLE_OFFSET, ...)`.
pub const RAILING_HANDLE_OFFSET: u64 = 0x28000;

/// Maximum number of railings in a drawing.
pub const MAX_RAILINGS: usize = 2048;

/// The synthetic proxy handle for an architectural railing.
pub fn railing_handle(id: ArchId) -> Handle {
    Handle(ARCH_HANDLE_BASE.saturating_add(RAILING_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the synthetic railing-handle range.
pub fn is_railing_handle(h: Handle) -> bool {
    h.0 >= ARCH_HANDLE_BASE.saturating_add(RAILING_HANDLE_OFFSET)
        && h.0 < ARCH_HANDLE_BASE.saturating_add(RAILING_HANDLE_OFFSET).saturating_add(1u64 << 15)
}

/// Human-readable label for a railing id, e.g. `ra0`, `ra1`.
pub fn railing_label(id: ArchId) -> String {
    format!("ra{}", id.0)
}

/// Offset within the arch-handle range reserved for view proxy handles.
/// View handles occupy `[ARCH_HANDLE_BASE + VIEW_HANDLE_OFFSET, ...)`.
pub const VIEW_HANDLE_OFFSET: u64 = 0x30000;

/// Maximum number of views in a drawing.
pub const MAX_VIEWS: usize = 64;

/// Maximum number of lines + ring points per view geometry.
pub const MAX_VIEW_LINES: usize = 200_000;

/// The synthetic proxy handle for an architectural view.
pub fn view_handle(id: ArchId) -> Handle {
    Handle(ARCH_HANDLE_BASE.saturating_add(VIEW_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the synthetic view-handle range.
pub fn is_view_handle(h: Handle) -> bool {
    h.0 >= ARCH_HANDLE_BASE.saturating_add(VIEW_HANDLE_OFFSET) && h.0 < ARCH_HANDLE_BASE.saturating_add(VIEW_HANDLE_OFFSET).saturating_add(1u64 << 15)
}

/// Human-readable label for a view id, e.g. `v0`, `v1`.
pub fn view_label(id: ArchId) -> String {
    format!("v{}", id.0)
}

/// The kind of an architectural object, used to disambiguate IDs and map parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum ArchKind {
    /// A wall.
    #[default]
    Wall,
    /// An opening (door, window, etc.).
    Opening,
    /// A room.
    Room,
    /// A roof.
    Roof,
    /// A stair.
    Stair,
    /// A railing.
    Railing,
    /// A view (elevation or section).
    View,
    /// Phase 16: a ceiling tray (`tr{n}`).
    Tray,
    /// Phase 16: an interior soffit (`sf{n}`).
    Soffit,
    /// Phase 16: a foundation (`fd{n}`).
    Foundation,
    /// Phase 18: a countertop (`ct{n}`).
    Countertop,
    /// Phase 18: a backsplash (`bs{n}`).
    Backsplash,
    /// Phase 18: a molding line (`ml{n}`).
    MoldingLine,
    /// Phase 21: a cabinet (`cb{n}`).
    Cabinet,
    /// Phase 21: a fixture (`fx{n}`).
    Fixture,
}

impl ArchKind {
    /// Map a part string (from `arch3d::probe::part_of`) to an `ArchKind`.
    pub fn from_part(part: &str) -> ArchKind {
        match part {
            "leaf" | "frame" | "casing" | "glass" | "sill" | "header" | "grille" => ArchKind::Opening,
            "floor" | "ceiling" | "deck" | "slab" | "ledger" | "joist" | "rim" | "beam" => ArchKind::Room,
            "roof" => ArchKind::Roof,
            "stair" | "tread" | "riser" | "stringer" | "skirt" => ArchKind::Stair,
            "railing" | "post" => ArchKind::Railing,
            "soffit" => ArchKind::Soffit,
            "foundation" | "footing" => ArchKind::Foundation,
            "backsplash" => ArchKind::Backsplash,
            "cabinet" | "counter" | "pull" => ArchKind::Cabinet,
            "fixture" => ArchKind::Fixture,
            _ => ArchKind::Wall,
        }
    }

    /// The JSON spelling of this kind.
    pub fn key(self) -> &'static str {
        match self {
            ArchKind::Wall => "wall",
            ArchKind::Opening => "opening",
            ArchKind::Room => "room",
            ArchKind::Roof => "roof",
            ArchKind::Stair => "stair",
            ArchKind::Railing => "railing",
            ArchKind::View => "view",
            ArchKind::Tray => "tray",
            ArchKind::Soffit => "soffit",
            ArchKind::Foundation => "foundation",
            ArchKind::Countertop => "countertop",
            ArchKind::Backsplash => "backsplash",
            ArchKind::MoldingLine => "moldingLine",
            ArchKind::Cabinet => "cabinet",
            ArchKind::Fixture => "fixture",
        }
    }

    /// The synthetic proxy handle for an object of this kind.
    pub fn handle(self, id: ArchId) -> Handle {
        match self {
            ArchKind::Wall => arch_handle(id),
            ArchKind::Opening => opening_handle(id),
            ArchKind::Room => room_handle(id),
            ArchKind::Roof => roof_handle(id),
            ArchKind::Stair => stair_handle(id),
            ArchKind::Railing => railing_handle(id),
            ArchKind::View => view_handle(id),
            ArchKind::Tray => tray_handle(id),
            ArchKind::Soffit => soffit_handle(id),
            ArchKind::Foundation => foundation_handle(id),
            ArchKind::Countertop => countertop_handle(id),
            ArchKind::Backsplash => backsplash_handle(id),
            ArchKind::MoldingLine => molding_line_handle(id),
            ArchKind::Cabinet => cabinet_handle(id),
            ArchKind::Fixture => fixture_handle(id),
        }
    }

    /// The human-readable label for an object of this kind.
    pub fn label(self, id: ArchId) -> String {
        match self {
            ArchKind::Wall => wall_label(id),
            ArchKind::Opening => opening_label(id),
            ArchKind::Room => room_label(id),
            ArchKind::Roof => roof_label(id),
            ArchKind::Stair => stair_label(id),
            ArchKind::Railing => railing_label(id),
            ArchKind::View => view_label(id),
            ArchKind::Tray => tray_label(id),
            ArchKind::Soffit => soffit_label(id),
            ArchKind::Foundation => foundation_label(id),
            ArchKind::Countertop => countertop_label(id),
            ArchKind::Backsplash => backsplash_label(id),
            ArchKind::MoldingLine => molding_line_label(id),
            ArchKind::Cabinet => cabinet_label(id),
            ArchKind::Fixture => fixture_label(id),
        }
    }
}

/// Maximum number of walls in a drawing (caps join-stretch BFS and allocations).
pub const MAX_WALLS: usize = 4096;

/// Minimum wall thickness, in metres.
pub const MIN_WALL_THICKNESS: f64 = 1e-4;

/// Minimum wall length, in metres.
pub const MIN_WALL_LENGTH: f64 = 1e-6;

/// Maximum number of openings in a drawing.
pub const MAX_OPENINGS: usize = 8192;

/// Maximum number of rooms in a drawing.
pub const MAX_ROOMS: usize = 1024;

/// Maximum number of saved cameras in a drawing.
pub const MAX_CAMERAS: usize = 64;

/// Minimum room area (m²) to be considered valid.
pub const MIN_ROOM_AREA: f64 = 0.05;

/// Offset within the arch-handle range reserved for room proxy handles.
/// Room handles occupy `[ARCH_HANDLE_BASE + ROOM_HANDLE_OFFSET, ...)`.
pub const ROOM_HANDLE_OFFSET: u64 = 0x10000;

/// Minimum opening width, in metres.
pub const MIN_OPENING_WIDTH: f64 = 0.1;

/// A wall: centre-line segment, thickness, height, and layer bands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wall {
    /// Centre-line start point.
    pub start: Vec2,
    /// Centre-line end point.
    pub end: Vec2,
    /// Stable identifier assigned by [`ArchModel::push_wall`].
    pub id: ArchId,
    /// The building level this wall sits on.
    pub base_level: u32,
    /// Whether the wall extends to the ceiling of its level.
    pub to_ceiling: bool,
    /// Whether this is an exterior wall.
    pub exterior: bool,
    /// Name of the wall type (e.g. "Exterior", "Interior").
    #[serde(default = "default_wall_type")]
    pub wall_type: String,
    /// Join kind at the start end of the wall.
    pub join_start: JoinKind,
    /// Join kind at the end end of the wall.
    pub join_end: JoinKind,
    /// Wall thickness (perpendicular to the centre line).
    pub thickness: f64,
    /// Wall height (not used in 2D poché, but kept for 3D).
    pub height: f64,
    /// Layer bands: each `(name, thickness)` splits the wall thickness into nested
    /// sub-bands. The sum of all band thicknesses must equal `thickness`.
    pub layers: Vec<(String, f64)>,
    /// Whether this wall is a room-divider (encloses rooms but produces no poché/3D).
    #[serde(default)]
    pub divider: bool,
    /// How the roof meets this wall (auto-detected, hip, gable, or valley).
    #[serde(default)]
    pub roof_directive: RoofDirective,
    /// Per-wall roof pitch override as rise (e.g. 6 for 6/12); `None` means use the roof's default.
    #[serde(default)]
    pub roof_pitch: Option<f64>,
    /// Per-wall overhang override; `None` means use the roof's overhang.
    #[serde(default)]
    pub roof_overhang: Option<f64>,
    /// Eave height above the level floor; `None` means derive from level.
    #[serde(default)]
    pub eave_height: Option<f64>,
    /// Level index the wall spans to: a spanning wall runs from its base level's floor to
    /// `t`'s ceiling, and its own `height` is ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_level: Option<u32>,
    /// A retaining wall: a terrain breakline whose top and bottom follow the ground on each side
    /// (phase 9). Its top is the high side's grade plus `freeboard`, its bottom the low side's
    /// grade minus `footing`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retaining: bool,
    /// `None` = [`DEFAULT_RETAINING_FREEBOARD`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeboard: Option<f64>,
    /// `None` = [`DEFAULT_RETAINING_FOOTING`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footing: Option<f64>,
    /// Finish of the wall's EXTERIOR face: the face on the right of start->end, i.e. the side of
    /// its first layer band in `arch::wall::wall_layer_polygons` (outside for a counter-clockwise
    /// ring).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish: Option<Finish>,
}

fn default_wall_type() -> String {
    "Exterior".into()
}

/// A builtin wall-type name always accepted even without a `WallType` entry in
/// `ArchModel::types` (matches the names the `wall` command can store).
fn is_builtin_wall_type(name: &str) -> bool {
    matches!(name.to_lowercase().as_str(), "exterior" | "interior" | "knee" | "railing" | "retaining")
}

impl Wall {
    /// A wall with the given centre line and thickness, 2.4 m high and no layer bands.
    pub fn new(start: Vec2, end: Vec2, thickness: f64) -> Self {
        Wall {
            start,
            end,
            thickness,
            height: 2.4,
            layers: Vec::new(),
            id: ArchId(0),
            base_level: 0,
            to_ceiling: false,
            exterior: true,
            wall_type: "Exterior".into(),
            join_start: JoinKind::Mitre,
            join_end: JoinKind::Mitre,
            divider: false,
            roof_directive: RoofDirective::default(),
            roof_pitch: None,
            roof_overhang: None,
            eave_height: None,
            top_level: None,
            retaining: false,
            freeboard: None,
            footing: None,
            finish: None,
        }
    }

    /// Elevation of the floor at which this wall sits (its base level's elevation).
    /// Returns 0.0 when the base level is not found in `levels`.
    pub fn bottom_z(&self, levels: &[Level]) -> f64 {
        level_by_index(levels, self.base_level).map_or(0.0, |l| l.elevation)
    }

    /// Elevation of this wall's top. A spanning wall (`top_level` set) reaches that level's
    /// ceiling; otherwise it is base floor plus `height`.
    pub fn top_z(&self, levels: &[Level]) -> f64 {
        match self.top_level {
            Some(t) => match level_by_index(levels, t) {
                Some(l) => l.elevation + l.ceiling_height,
                None => self.bottom_z(levels) + self.height,
            },
            None => self.bottom_z(levels) + self.height,
        }
    }

    /// Whether `level` falls within the wall's vertical extent. Always true for the base level;
    /// for a spanning wall, also true for every level whose floor lies in [bottom_z, top_z - 0.01).
    pub fn spans_level(&self, level: &Level, levels: &[Level]) -> bool {
        if level.index == self.base_level {
            return true;
        }
        if self.top_level.is_none() {
            return false;
        }
        let bottom = self.bottom_z(levels);
        let top = self.top_z(levels);
        level.elevation >= bottom && level.elevation < top - 0.01
    }
}

impl Default for Wall {
    fn default() -> Self {
        Wall {
            start: Vec2::ZERO,
            end: Vec2::ZERO,
            thickness: 0.2,
            height: 2.4,
            layers: Vec::new(),
            id: ArchId(0),
            base_level: 0,
            to_ceiling: false,
            exterior: true,
            wall_type: "Exterior".into(),
            join_start: JoinKind::Mitre,
            join_end: JoinKind::Mitre,
            divider: false,
            roof_directive: RoofDirective::default(),
            roof_pitch: None,
            roof_overhang: None,
            eave_height: None,
            top_level: None,
            retaining: false,
            freeboard: None,
            footing: None,
            finish: None,
        }
    }
}

/// Which side of the wall centre line the reference is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefSide {
    Left,
    Right,
    #[default]
    Centre,
}

/// A single layer within a wall type's build-up.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WallLayer {
    pub name: String,
    pub thickness: f64,
    pub material: String,
}

/// A named wall type with its layer build-up and reference side.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WallType {
    pub name: String,
    pub layers: Vec<WallLayer>,
    pub reference: RefSide,
}

/// Per-wall join override. `Auto`/`Mitre` mean the solver decides.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JoinKind {
    /// No explicit preference; the join solver decides.
    Auto,
    /// Default preference for new walls; the join solver decides.
    #[default]
    Mitre,
    /// Force a butt joint.
    Butt,
    /// Force a square joint.
    Square,
}

/// The kind of architectural opening.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OpeningKind {
    /// A hinged or sliding door.
    #[default]
    Door,
    /// A window.
    Window,
    /// A garage door.
    GarageDoor,
    /// A cased opening (no swing).
    CasedOpening,
}

/// Which side the hinge is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Hinge {
    /// Hinge on the left.
    #[default]
    Left,
    /// Hinge on the right.
    Right,
}

/// Which direction the opening swings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Swing {
    /// Swings inward.
    #[default]
    In,
    /// Swings outward.
    Out,
}

/// The kind of a building level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LevelKind {
    /// A full floor level.
    #[default]
    Floor,
    /// A landing level (e.g. reached by stairs between floors).
    Landing,
}

impl LevelKind {
    /// True for [`LevelKind::Floor`], the default (used to skip it when serializing).
    pub fn is_floor(&self) -> bool {
        *self == LevelKind::Floor
    }
}

/// A building level (floor plate).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Level {
    pub index: u32,
    pub name: String,
    pub elevation: f64,
    pub ceiling_height: f64,
    pub structure: f64,
    /// Floor or landing. Not written for a floor, so a drawing without landings keeps its old payload.
    #[serde(default, skip_serializing_if = "LevelKind::is_floor")]
    pub kind: LevelKind,
}

impl Default for Level {
    fn default() -> Self {
        Level { index: 0, name: String::new(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, kind: LevelKind::default() }
    }
}

impl Level {
    /// Derived floor-to-floor height: ceiling + structure.
    /// Returns 0.0 when either component is non-finite or <= 0.
    pub fn floor_to_floor(&self) -> f64 {
        if self.ceiling_height.is_finite() && self.structure.is_finite() && self.ceiling_height > 0.0 && self.structure > 0.0 {
            self.ceiling_height + self.structure
        } else {
            0.0
        }
    }
}

/// Look up a level by its index in `levels`. Returns `None` when no level has that index.
pub fn level_by_index(levels: &[Level], index: u32) -> Option<&Level> {
    levels.iter().find(|l| l.index == index)
}

/// An architectural opening (door, window, garage door, or cased opening).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Opening {
    pub id: ArchId,
    pub host: ArchId,
    pub offset: f64,
    pub width: f64,
    pub height: f64,
    pub sill: f64,
    pub kind: OpeningKind,
    pub style: String,
    pub hinge: Hinge,
    pub swing: Swing,
    pub recess: f64,
    pub frame_width: f64,
    pub frame_depth: f64,
    pub casing_width: f64,
    pub casing_depth: f64,
    pub sill_depth: f64,
    pub rough_width: f64,
    pub rough_height: f64,
    pub swing_angle: f64,
    pub percent_open: f64,
    pub glass_mode: String,
    pub mull_group: Option<ArchId>,
    pub layer: String,
    /// Window grille (lites across and down); `None` is plain glass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grille: Option<Grille>,
    /// Shape of the opening's top.
    #[serde(default, skip_serializing_if = "OpeningTop::is_square")]
    pub top: OpeningTop,
    /// Rise of a segment top (0 = unset).
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub top_rise: f64,
    /// How far a `top: roof` window's rough top stays under the roof underside.
    #[serde(default = "default_roof_offset", skip_serializing_if = "is_default_roof_offset")]
    pub roof_offset: f64,
    /// Which sides of the host wall carry the sill.
    #[serde(default, skip_serializing_if = "OpeningSides::is_right_only")]
    pub sill_sides: OpeningSides,
    /// Which sides of the host wall carry the casing.
    #[serde(default, skip_serializing_if = "OpeningSides::is_right_only")]
    pub casing_sides: OpeningSides,
}

impl Default for Opening {
    fn default() -> Self {
        Self {
            id: ArchId(0),
            host: ArchId(0),
            offset: 0.0,
            width: 0.9,
            height: 2.1,
            sill: 0.0,
            kind: OpeningKind::Door,
            style: "hinged".into(),
            hinge: Hinge::Left,
            swing: Swing::In,
            recess: 0.0,
            frame_width: 0.06,
            frame_depth: 0.10,
            casing_width: 0.05,
            casing_depth: 0.015,
            sill_depth: 0.03,
            rough_width: 1.02,
            rough_height: 2.22,
            swing_angle: 0.0,
            percent_open: 0.0,
            glass_mode: "automatic".into(),
            mull_group: None,
            layer: "A-DOOR".into(),
            grille: None,
            top: OpeningTop::Square,
            top_rise: 0.0,
            roof_offset: DEFAULT_ROOF_OFFSET,
            sill_sides: OpeningSides::RIGHT,
            casing_sides: OpeningSides::RIGHT,
        }
    }
}

impl Opening {
    /// Create a new opening on the given host wall.
    ///
    /// Rough dimensions are set to finished + 0.12 m. Style and layer are
    /// derived from `kind`. Non-finite numeric inputs are clamped to defaults.
    pub fn new(host: ArchId, offset: f64, width: f64, height: f64, kind: OpeningKind) -> Self {
        let offset = if offset.is_finite() { offset } else { 0.0 };
        let width = if width.is_finite() { width } else { 0.9 };
        let height = if height.is_finite() { height } else { 2.1 };
        let style = match kind {
            OpeningKind::Window => "slider".to_string(),
            OpeningKind::GarageDoor => "garage".to_string(),
            OpeningKind::CasedOpening => "cased".to_string(),
            OpeningKind::Door => "hinged".to_string(),
        };
        let layer = if kind == OpeningKind::Window { "A-GLAZ" } else { "A-DOOR" };
        Self {
            id: ArchId(0),
            host,
            offset,
            width,
            height,
            sill: 0.0,
            kind,
            style,
            hinge: Hinge::Left,
            swing: Swing::In,
            recess: 0.0,
            frame_width: 0.06,
            frame_depth: 0.10,
            casing_width: 0.05,
            casing_depth: 0.015,
            sill_depth: 0.03,
            rough_width: width + 0.12,
            rough_height: height + 0.12,
            swing_angle: 0.0,
            percent_open: 0.0,
            glass_mode: "automatic".into(),
            mull_group: None,
            layer: layer.to_string(),
            grille: None,
            top: OpeningTop::Square,
            top_rise: 0.0,
            roof_offset: DEFAULT_ROOF_OFFSET,
            sill_sides: OpeningSides::RIGHT,
            casing_sides: OpeningSides::RIGHT,
        }
    }
}

/// The type of an architectural room.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoomType {
    Living,
    Bedroom,
    Kitchen,
    Bath,
    Closet,
    Utility,
    Dining,
    Family,
    Entry,
    Laundry,
    Garage,
    Porch,
    Deck,
    OpenBelow,
    #[default]
    Other,
}

/// The shape of a stair flight.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StairShape {
    /// A straight run.
    #[default]
    Straight,
    /// An L-shaped run with a landing.
    L,
    /// A U-shaped run with a landing.
    U,
}

/// The style of a railing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RailingStyle {
    /// A solid panel railing.
    #[default]
    Panel,
    /// A railing with vertical balusters.
    Balusters,
}

impl RoomType {
    /// Whether this room type counts as interior (excludes Garage/Porch/Deck/OpenBelow).
    pub fn is_interior(self) -> bool {
        !matches!(self, RoomType::Garage | RoomType::Porch | RoomType::Deck | RoomType::OpenBelow)
    }

    /// Whether this room type has a ceiling (Deck/Porch do not).
    pub fn has_ceiling(self) -> bool {
        !matches!(self, RoomType::Deck | RoomType::Porch)
    }

    /// Whether this room type has a floor (OpenBelow does not).
    pub fn has_floor(self) -> bool {
        !matches!(self, RoomType::OpenBelow)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LabelArea {
    #[default]
    Interior,
    Standard,
}

impl LabelArea {
    pub fn key(self) -> &'static str {
        match self {
            LabelArea::Interior => "interior",
            LabelArea::Standard => "standard",
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        match s {
            "interior" => Some(LabelArea::Interior),
            "standard" => Some(LabelArea::Standard),
            _ => None,
        }
    }

    pub fn is_interior(&self) -> bool {
        matches!(self, LabelArea::Interior)
    }
}

/// An architectural room: enclosed area defined by wall faces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Room {
    pub id: ArchId,
    pub name: String,
    pub room_type: RoomType,
    pub level: u32,
    pub polygon: Vec<Vec2>,
    pub holes: Vec<Vec<Vec2>>,
    pub ceiling_height: Option<f64>,
    pub floor_height: Option<f64>,
    pub interior_area: f64,
    pub standard_area: f64,
    #[serde(default, skip_serializing_if = "LabelArea::is_interior")]
    pub label_area: LabelArea,
    /// Height of the deck surface above the level elevation, if this is a deck room.
    pub deck_height: Option<f64>,
    /// The room's floor finish.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_finish: Option<Finish>,
    /// A flat ceiling at the ceiling height (the default). Off, under a roof, the ceiling
    /// follows the underside of the roof planes over the room (a cathedral or vaulted ceiling).
    /// `ceiling_height` is the room's ROUGH ceiling either way: the underside of the structure
    /// above, relative to the level's floor.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub flat_ceiling: bool,
    /// The ceiling's finish stack, top to bottom. The finished ceiling is the rough ceiling
    /// minus the stack's total thickness; empty (the default) means no finish layers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ceiling_stack: Vec<CeilingLayer>,
    /// Deck board direction in degrees counter-clockwise from +x, in [0, 180); `None` is
    /// automatic (parallel to the ledger).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deck_boards: Option<f64>,
    /// Deck railing setting per polygon edge i (vertex i to i+1); a missing entry is Auto.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deck_rails: Vec<RailSetting>,
    /// Moldings on the room's walls.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moldings: Vec<RoomMolding>,
}

impl Default for Room {
    fn default() -> Self {
        Room {
            id: ArchId(0),
            name: String::new(),
            room_type: RoomType::default(),
            level: 0,
            polygon: Vec::new(),
            holes: Vec::new(),
            ceiling_height: None,
            floor_height: None,
            interior_area: 0.0,
            standard_area: 0.0,
            label_area: LabelArea::Interior,
            deck_height: None,
            floor_finish: None,
            flat_ceiling: true,
            ceiling_stack: Vec::new(),
            deck_boards: None,
            deck_rails: Vec::new(),
            moldings: Vec::new(),
        }
    }
}

/// One layer of a ceiling finish stack (listed top to bottom): a material and a thickness.
/// `material: None` is an air gap (a suspended ceiling's plenum).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CeilingLayer {
    /// A display name ("drywall", "air", "ceiling tile").
    pub name: String,
    /// The layer's material key or name; `None` is an air gap.
    pub material: Option<String>,
    /// Thickness in metres.
    pub thickness: f64,
}

impl CeilingLayer {
    /// A layer named `name` of `thickness` metres of `material` (`None`: an air gap). Code
    /// outside this file builds layers with this, so a later field does not break it.
    pub fn new(name: impl Into<String>, material: Option<&str>, thickness: f64) -> Self {
        Self { name: name.into(), material: material.map(str::to_string), thickness }
    }
}

fn yes() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

/// How a roof meets a wall: auto-detected, hip, gable, or valley.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoofDirective {
    /// No explicit preference; the roof generator decides.
    #[default]
    Auto,
    /// Hip roof meets this wall.
    Hip,
    /// Gable roof meets this wall.
    Gable,
    /// Valley where two gable roofs meet.
    Valley,
}

/// Classification of a roof edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoofEdgeKind {
    /// The lower edge where the roof meets the walls.
    #[default]
    Eave,
    /// The upper ridge line where two planes meet.
    Ridge,
    /// A sloped corner edge (hip).
    Hip,
    /// A valley corner edge.
    Valley,
    /// A vertical gable end face edge.
    GableEnd,
}

/// A single roof plane: its footprint ring, slope, and dimensions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoofPlane {
    /// Planar ring (outer boundary) of this roof plane, CCW.
    pub ring: Vec<Vec2>,
    /// Slope (rise/run) of this plane.
    pub slope: f64,
    /// Projected area onto the plan, in square metres.
    pub projected_area: f64,
    /// Wall id this plane's eave rests on (if any).
    pub eave_wall: Option<ArchId>,
    /// 4-float encoding of the plane's base edge (x0, y0, x1, y1) in plan.
    pub base_edge: [f64; 4],
    /// Vertical thickness of the roof material at this plane.
    pub thickness: f64,
    /// Plan overhang: the distance from the ring's base edge (the overhang outline) to
    /// the architectural eave line, where `eave_z` applies.
    pub overhang: f64,
    /// Height of the roof underside at the base edge (eave height).
    pub eave_z: f64,
    /// Fascia depth at this plane, in metres (0.0 = none). Kept numeric so the 3D
    /// builder can emit a fascia band of the correct thickness.
    pub fascia_depth: f64,
    /// Soffit depth at this plane, in metres (0.0 = none). Kept numeric so the 3D
    /// builder can emit a soffit of the correct thickness.
    pub soffit_depth: f64,
}

/// A classified edge of a roof (eave, ridge, hip, valley, gable end).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoofEdge {
    pub kind: RoofEdgeKind,
    /// Plan point A.
    pub a: Vec2,
    /// Plan point B.
    pub b: Vec2,
    /// Height (elevation) at A.
    pub za: f64,
    /// Height (elevation) at B.
    pub zb: f64,
}

/// An automatic or manual roof model, stored per level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Roof {
    /// Stable identifier assigned by [`ArchModel::push_roof`].
    pub id: ArchId,
    pub level: u32,
    /// Roof pitch rise (e.g. 6 for 6/12).
    pub pitch_rise: f64,
    /// Roof pitch run (e.g. 12 for 6/12).
    pub pitch_run: f64,
    /// Uniform overhang, in metres.
    pub overhang: f64,
    /// Roof material thickness, in metres.
    pub thickness: f64,
    /// Emit fascia boards around the eave.
    pub fascia: bool,
    /// Emit soffit panels under the fascia.
    pub soffit: bool,
    /// Created by a manual `roofplane` command rather than `ROOFAUTO`.
    pub manual: bool,
    /// User-edited planes; rebuilds stop touching this roof.
    pub hand_edited: bool,
    pub planes: Vec<RoofPlane>,
    pub edges: Vec<RoofEdge>,
    /// The covering of the roof planes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub covering: Option<Finish>,
}

/// An architectural stair.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stair {
    /// Stable identifier assigned by [`ArchModel::push_stair`].
    pub id: ArchId,
    /// Start point of the stair in plan.
    pub start: Vec2,
    /// Direction of the stair in radians (+x is 0, CCW).
    pub direction: f64,
    /// Width of the stair.
    pub width: f64,
    /// Level index the stair starts from.
    pub from_level: u32,
    /// Level index the stair goes to.
    pub to_level: u32,
    /// Total rise (elevation difference).
    pub rise: f64,
    /// Shape of the stair.
    pub shape: StairShape,
    /// Number of risers.
    pub risers: usize,
    /// Height of each riser.
    pub riser_height: f64,
    /// Depth of each tread.
    pub tread_depth: f64,
    /// Number of treads.
    pub treads: usize,
    /// Total horizontal run.
    pub run: f64,
    /// Depth of the landing (if L or U).
    pub landing_depth: f64,
    /// Polygon defining the stair well (hole in upper floor).
    pub well: Vec<Vec2>,
    /// Room ID of the OpenBelow room created for the well.
    pub well_room: Option<ArchId>,
    /// Layer name.
    pub layer: String,
    /// Open risers (no riser boards).
    #[serde(default, skip_serializing_if = "is_false")]
    pub open_risers: bool,
    /// Stringer layout under the treads.
    #[serde(default, skip_serializing_if = "Stringers::is_two")]
    pub stringers: Stringers,
    /// Depth of the stair's underside below the treads (metres).
    #[serde(default = "default_underside", skip_serializing_if = "is_default_underside")]
    pub underside: f64,
    /// Railing on the left side of the flight.
    #[serde(default, skip_serializing_if = "RailSetting::is_auto")]
    pub rail_left: RailSetting,
    /// Railing on the right side of the flight.
    #[serde(default, skip_serializing_if = "RailSetting::is_auto")]
    pub rail_right: RailSetting,
    /// Plan cut height above the stair's from-level floor (metres).
    #[serde(default = "default_plan_cut", skip_serializing_if = "is_default_plan_cut")]
    pub plan_cut: f64,
}

impl Default for Stair {
    fn default() -> Self {
        Stair {
            id: ArchId(0),
            start: Vec2::ZERO,
            direction: 0.0,
            width: 1.0,
            from_level: 0,
            to_level: 1,
            rise: 2.7,
            shape: StairShape::Straight,
            risers: 16,
            riser_height: 0.16875,
            tread_depth: 0.254,
            treads: 15,
            run: 3.81,
            landing_depth: 0.0,
            well: Vec::new(),
            well_room: None,
            layer: "A-STAIR".into(),
            open_risers: false,
            stringers: Stringers::Two,
            underside: DEFAULT_UNDERSIDE,
            rail_left: RailSetting::Auto,
            rail_right: RailSetting::Auto,
            plan_cut: DEFAULT_PLAN_CUT,
        }
    }
}

/// An architectural railing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Railing {
    /// Stable identifier assigned by [`ArchModel::push_railing`].
    pub id: ArchId,
    /// Path of the railing in plan.
    pub path: Vec<Vec2>,
    /// Level index.
    pub level: u32,
    /// Height of the railing.
    pub height: f64,
    /// Style of the railing.
    pub style: RailingStyle,
    /// Thickness of the railing elements.
    pub thickness: f64,
    /// Gap between balusters.
    pub baluster_gap: f64,
    /// Spacing between posts.
    pub post_spacing: f64,
    /// ID of the stair this railing follows (if any).
    pub follows: Option<ArchId>,
    /// Horizontal offset of the built railing from its path (positive = left of travel).
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub inset: f64,
    /// What `follows` names: a stair's well, a side of a stair's flight, or a deck room.
    #[serde(default, skip_serializing_if = "RailingHost::is_well")]
    pub host: RailingHost,
    /// Made automatically for a stair or deck and remade with it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub auto: bool,
    /// Layer name.
    pub layer: String,
}

impl Default for Railing {
    fn default() -> Self {
        Railing {
            id: ArchId(0),
            path: Vec::new(),
            level: 0,
            height: 0.914,
            style: RailingStyle::Panel,
            thickness: 0.05,
            baluster_gap: 0.099,
            post_spacing: DEFAULT_POST_SPACING,
            follows: None,
            inset: 0.0,
            host: RailingHost::Well,
            auto: false,
            layer: "A-RAIL".into(),
        }
    }
}

impl Default for Roof {
    fn default() -> Self {
        Roof {
            id: ArchId(0),
            level: 0,
            pitch_rise: 6.0,
            pitch_run: 12.0,
            overhang: 0.4,
            thickness: 0.25,
            fascia: true,
            soffit: true,
            manual: false,
            hand_edited: false,
            planes: Vec::new(),
            edges: Vec::new(),
            covering: None,
        }
    }
}

impl Roof {
    /// Rise/run ratio when both are finite and run > 0, else 0.5.
    pub fn slope(&self) -> f64 {
        if self.pitch_rise.is_finite() && self.pitch_run.is_finite() && self.pitch_run > 0.0 { self.pitch_rise / self.pitch_run } else { 0.5 }
    }

    /// Sum of all finite plane projected areas.
    pub fn projected_area(&self) -> f64 {
        self.planes.iter().filter(|p| p.projected_area.is_finite()).map(|p| p.projected_area).sum()
    }
}

/// The kind of a saved 3D camera.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CameraKind {
    /// An orbit camera that revolves around its target.
    #[default]
    Orbit,
    /// A full (framed) camera.
    Full,
    /// A floor overview: the camera level and the ones below, no ceilings.
    /// Posed like an orbit camera.
    #[serde(alias = "overview")]
    Overview,
}

/// The kind of an architectural view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ViewKind {
    /// An elevation view (looking in a direction).
    #[default]
    Elevation,
    /// A section view (cutting through a line).
    Section,
    /// An interior elevation, looking from a room at one of its walls (phase 21).
    Interior,
}

impl ViewKind {
    /// The JSON spelling of this kind.
    pub fn key(self) -> &'static str {
        match self {
            ViewKind::Elevation => "elevation",
            ViewKind::Section => "section",
            ViewKind::Interior => "interior",
        }
    }
}

/// A line segment in a generated view geometry.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewLine {
    /// Start point (u, v) in view coordinates (metres).
    pub a: [f64; 2],
    /// End point (u, v) in view coordinates (metres).
    pub b: [f64; 2],
    /// The kind of object this line belongs to.
    pub kind: ArchKind,
    /// The ID of the object this line belongs to.
    pub object: ArchId,
}

/// A closed ring in a generated view geometry (e.g. poché fill).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewRing {
    /// The kind of object this ring belongs to.
    pub kind: ArchKind,
    /// The ID of the object this ring belongs to.
    pub object: ArchId,
    /// Material name for fill.
    pub material: String,
    /// Points (u, v) in view coordinates (metres).
    pub points: Vec<[f64; 2]>,
}

impl ViewRing {
    /// Absolute area of the ring (shoelace formula).
    pub fn area(&self) -> f64 {
        let mut area = 0.0;
        let n = self.points.len();
        if n < 3 {
            return 0.0;
        }
        for i in 0..n {
            let p1 = self.points[i];
            let p2 = self.points[(i + 1) % n];
            area += p1[0] * p2[1] - p2[0] * p1[1];
        }
        (area / 2.0).abs()
    }
}

/// The kind of dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DimKind {
    /// Horizontal dimension.
    #[default]
    Width,
    /// Vertical dimension.
    Height,
}

/// A dimension annotation in a generated view geometry.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewDim {
    /// The kind of dimension.
    pub kind: DimKind,
    /// Measured point A (u, v).
    pub a: [f64; 2],
    /// Measured point B (u, v).
    pub b: [f64; 2],
    /// A point on the dimension line (u, v).
    pub at: [f64; 2],
    /// The measured value.
    pub value: f64,
    /// Text label.
    pub text: String,
}

/// Generated geometry for a view, regenerated on revision change.
///
/// Coordinates are in view space (u right, v up, metres).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewGeometry {
    /// The `arch_revision` this geometry was generated for.
    pub revision: u64,
    /// Bounds of lines and ring points: [u0, v0, u1, v1].
    pub extent: [f64; 4],
    /// Line segments.
    pub lines: Vec<ViewLine>,
    /// Closed rings (fills).
    pub rings: Vec<ViewRing>,
    /// Dimensions.
    pub dims: Vec<ViewDim>,
    /// Material pattern lines (phase 21).
    pub patterns: Vec<ViewPatternLine>,
    /// Indicator lines such as door swings (phase 21).
    pub indicators: Vec<ViewLine>,
    /// Text labels (phase 21).
    pub labels: Vec<ViewLabel>,
    /// Error message if generation failed.
    pub error: Option<String>,
}

/// An architectural view object (Elevation or Section).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArchView {
    /// Stable identifier assigned by [`ArchModel::push_view`].
    pub id: ArchId,
    /// Name of the view.
    pub name: String,
    /// Kind of view (Elevation or Section).
    pub kind: ViewKind,
    /// Unit plan direction the viewer looks (x, y).
    pub look: [f64; 2],
    /// Section cut line in plan (start, end). `Some` for Section, `None` for Elevation.
    pub cut: Option<[[f64; 2]; 2]>,
    /// Depth beyond the cut to show (None = unlimited).
    pub depth: Option<f64>,
    /// Plan rectangle [[x0, y0], [x1, y1]] to scope objects. None = all.
    pub scope: Option<[[f64; 2]; 2]>,
    /// Drawing scale denominator (e.g. 50 for 1:50).
    pub scale: f64,
    /// Model-space point where the view's lower-left is placed.
    pub insert: [f64; 2],
    /// Whether to generate dimensions.
    pub dimensions: bool,
    /// Layer name ("A-ELEV" or "A-SECT").
    pub layer: String,
    /// Interior view: the room looked from (phase 21).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<ArchId>,
    /// A point inside the room when the view was made; finds the room after renumbering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_point: Option<[f64; 2]>,
    /// Interior view: the wall looked at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall: Option<ArchId>,
    /// The level the view shows, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    /// Notes drawn on the view.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<ViewNote>,
    /// The next note id.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_note_id: u32,
    /// Generated geometry, not serialized. Regenerated by engine.
    #[serde(skip)]
    pub generated: Option<ViewGeometry>,
}

impl Default for ArchView {
    fn default() -> Self {
        ArchView {
            id: ArchId(0),
            name: String::new(),
            kind: ViewKind::default(),
            look: [0.0, 1.0],
            cut: None,
            depth: None,
            scope: None,
            scale: 50.0,
            insert: [0.0, 0.0],
            dimensions: false,
            layer: "A-ELEV".into(),
            room: None,
            room_point: None,
            wall: None,
            level: None,
            notes: Vec::new(),
            next_note_id: 0,
            generated: None,
        }
    }
}

impl ArchView {
    /// Place a view-space point into model space using the generated extent.
    pub fn placed(&self, p: [f64; 2]) -> [f64; 2] {
        if let Some(g) = &self.generated {
            let u0 = g.extent[0];
            let v0 = g.extent[1];
            [self.insert[0] + (p[0] - u0), self.insert[1] + (p[1] - v0)]
        } else {
            self.insert
        }
    }

    /// The model-space bounds of the view geometry, if generated.
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let g = self.generated.as_ref()?;
        let u0 = g.extent[0];
        let v0 = g.extent[1];
        let u1 = g.extent[2];
        let v1 = g.extent[3];
        Some([self.insert[0] + (u0 - u0), self.insert[1] + (v0 - v0), self.insert[0] + (u1 - u0), self.insert[1] + (v1 - v0)])
    }

    /// Whether the generated geometry is missing or stale for the given revision.
    pub fn is_stale(&self, revision: u64) -> bool {
        match &self.generated {
            Some(g) => g.revision != revision,
            None => true,
        }
    }

    /// Validate the view definition.
    pub fn validate(&self) -> Result<(), ViewError> {
        if self.name.trim().is_empty() {
            return Err(ViewError::EmptyName);
        }
        if self.name.len() > 128 {
            return Err(ViewError::NameTooLong);
        }
        if !self.look[0].is_finite() || !self.look[1].is_finite() {
            return Err(ViewError::NonFiniteLook);
        }
        let len_sq = self.look[0] * self.look[0] + self.look[1] * self.look[1];
        if len_sq <= 0.25 || len_sq >= 4.0 {
            return Err(ViewError::BadLookLength);
        }

        if self.kind == ViewKind::Section {
            let cut = self.cut.ok_or(ViewError::MissingCut)?;
            if !cut[0][0].is_finite() || !cut[0][1].is_finite() || !cut[1][0].is_finite() || !cut[1][1].is_finite() {
                return Err(ViewError::NonFiniteCut);
            }
            if cut[0][0].abs() > 1e6 || cut[0][1].abs() > 1e6 || cut[1][0].abs() > 1e6 || cut[1][1].abs() > 1e6 {
                return Err(ViewError::CutTooLarge);
            }
            let dx = cut[1][0] - cut[0][0];
            let dy = cut[1][1] - cut[0][1];
            if dx * dx + dy * dy <= 1e-12 {
                return Err(ViewError::CutTooShort);
            }
            // Cross product of cut direction and look direction
            let cross = dx * self.look[1] - dy * self.look[0];
            if cross.abs() <= 1e-6 {
                return Err(ViewError::LookParallelToCut);
            }
        } else {
            if self.cut.is_some() {
                return Err(ViewError::UnexpectedCut);
            }
            if self.kind == ViewKind::Interior {
                if self.room.is_none() {
                    return Err(ViewError::MissingRoom);
                }
                if self.wall.is_none() {
                    return Err(ViewError::MissingWall);
                }
                match self.room_point {
                    Some(p) if p[0].is_finite() && p[1].is_finite() => {}
                    _ => return Err(ViewError::MissingRoom),
                }
            }
        }

        if self.notes.len() > MAX_VIEW_NOTES {
            return Err(ViewError::BadNote(format!("at most {MAX_VIEW_NOTES} notes")));
        }
        for n in &self.notes {
            validate_note(n).map_err(ViewError::BadNote)?;
        }

        if let Some(d) = self.depth {
            if !d.is_finite() || d <= 0.0 {
                return Err(ViewError::BadDepth);
            }
        }

        if let Some(s) = self.scope {
            if !s[0][0].is_finite() || !s[0][1].is_finite() || !s[1][0].is_finite() || !s[1][1].is_finite() {
                return Err(ViewError::NonFiniteScope);
            }
            if s[0][0].abs() > 1e6 || s[0][1].abs() > 1e6 || s[1][0].abs() > 1e6 || s[1][1].abs() > 1e6 {
                return Err(ViewError::ScopeTooLarge);
            }
            if s[0][0] >= s[1][0] || s[0][1] >= s[1][1] {
                return Err(ViewError::BadScope);
            }
        }

        if !self.scale.is_finite() || self.scale < 1.0 || self.scale > 10000.0 {
            return Err(ViewError::BadScale);
        }

        if !self.insert[0].is_finite() || !self.insert[1].is_finite() {
            return Err(ViewError::NonFiniteInsert);
        }
        if self.insert[0].abs() > 1e6 || self.insert[1].abs() > 1e6 {
            return Err(ViewError::InsertTooLarge);
        }

        if self.layer.is_empty() {
            return Err(ViewError::EmptyLayer);
        }

        Ok(())
    }
}

/// Errors returned by [`ArchView::validate`] and [`ArchModel::push_view`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewError {
    /// The view name was empty.
    EmptyName,
    /// The view name was too long.
    NameTooLong,
    /// The look direction was non-finite.
    NonFiniteLook,
    /// The look direction length was out of bounds.
    BadLookLength,
    /// A section view was missing a cut line.
    MissingCut,
    /// The cut line had non-finite coordinates.
    NonFiniteCut,
    /// The cut line coordinates were too large.
    CutTooLarge,
    /// The cut line endpoints were too close.
    CutTooShort,
    /// The look direction was parallel to the cut line.
    LookParallelToCut,
    /// An elevation view had a cut line.
    UnexpectedCut,
    /// The depth was invalid.
    BadDepth,
    /// The scope had non-finite coordinates.
    NonFiniteScope,
    /// The scope coordinates were too large.
    ScopeTooLarge,
    /// The scope rectangle was invalid.
    BadScope,
    /// The scale was invalid.
    BadScale,
    /// The insert point had non-finite coordinates.
    NonFiniteInsert,
    /// The insert point coordinates were too large.
    InsertTooLarge,
    /// The layer name was empty.
    EmptyLayer,
    /// Adding the view would exceed [`MAX_VIEWS`].
    TooMany,
    /// An interior view had no (usable) room.
    MissingRoom,
    /// An interior view had no wall.
    MissingWall,
    /// A view note was invalid; the text says why.
    BadNote(String),
}

impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewError::MissingRoom => write!(f, "an interior view needs a room and a finite room point"),
            ViewError::MissingWall => write!(f, "an interior view needs a wall"),
            ViewError::BadNote(s) => write!(f, "bad note: {s}"),
            other => write!(f, "{:?}", other),
        }
    }
}

/// A saved 3D camera, stored on [`ArchModel::cameras`] so that it participates in
/// undo, autosave, AUDIT and round-trip (decision 1).
///
/// `target`/`distance`/`azimuth_deg`/`elevation_deg` pose an orbit camera;
/// `eye`/`eye_height` are the full-camera eye. `kind` selects which fields
/// the consumer honours.
///
/// Numeric fields are `Option` to allow `null` in JSON (e.g. from non-finite values
/// serialized as null) without failing deserialization; invalid cameras are dropped
/// by `parse_arch`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Camera3d {
    /// Name of the camera, used for lookup and the active selection.
    pub name: String,
    /// How the camera is posed.
    pub kind: CameraKind,
    /// Point the orbit camera revolves around, in metres.
    pub target: Option<[f64; 3]>,
    /// Orbit azimuth, in degrees.
    pub azimuth_deg: Option<f64>,
    /// Orbit elevation, in degrees.
    pub elevation_deg: Option<f64>,
    /// Orbit distance from the target, in metres.
    pub distance: Option<f64>,
    /// Full-camera eye position (x, y), in metres.
    pub eye: Option<[f64; 2]>,
    /// Full-camera eye height, in metres.
    pub eye_height: Option<f64>,
    /// Field of view, in degrees.
    pub fov_deg: Option<f64>,
    /// The light set to apply to this camera's lights (a name into
    /// [`ArchModel::light_sets`]); `None` = no set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light_set: Option<String>,
    /// This camera's render look; `None` = follow the global look.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<Look>,
    /// The saved state `camera.reset` returns to (its own `saved` is always
    /// `None`); `None` = never saved with a state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved: Option<Box<Camera3d>>,
    /// Overview cameras: the level shown (index into the levels); `None` =
    /// the plan level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    /// Overview cameras: whether the levels below are shown; `None` = true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower_levels: Option<bool>,
}

impl Default for Camera3d {
    fn default() -> Self {
        Camera3d {
            name: String::new(),
            kind: CameraKind::default(),
            target: Some([0.0; 3]),
            azimuth_deg: Some(0.0),
            elevation_deg: Some(30.0),
            distance: Some(10.0),
            eye: Some([0.0, -10.0]),
            eye_height: Some(1.6),
            fov_deg: Some(45.0),
            light_set: None,
            look: None,
            saved: None,
            level: None,
            lower_levels: None,
        }
    }
}

/// Errors returned by [`ArchModel::set_camera`] / [`ArchModel::remove_camera`].
///
/// Errors are returned rather than clamping or rejecting silently, matching the
/// "refuse, never clamp" policy (decision 12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CameraError {
    /// The camera name was empty or all whitespace.
    EmptyName,
    /// A camera with the same name already exists.
    Duplicate,
    /// Adding the camera would exceed [`MAX_CAMERAS`].
    TooMany,
    /// The camera contained a non-finite number (NaN or infinity).
    NotFinite,
}

impl std::fmt::Display for CameraError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// Deserialize views with the same read-time filtering as [`ArchModel::validate`]:
/// invalid definitions and entries beyond [`MAX_VIEWS`] are dropped.
fn deserialize_views<'de, D>(deserializer: D) -> Result<Vec<ArchView>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mut views = Vec::<ArchView>::deserialize(deserializer)?;
    for view in &mut views {
        view.notes.truncate(MAX_VIEW_NOTES);
        view.notes.retain(|n| validate_note(n).is_ok());
    }
    views.retain(|view| view.validate().is_ok());
    views.truncate(MAX_VIEWS);
    Ok(views)
}

/// The architectural object model for a drawing.
///
/// Stored on [`Drawing::arch`]; the 3D pane and incremental rebuilds key off
/// [`Drawing::arch_revision`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArchModel {
    pub walls: Vec<Wall>,
    #[serde(default)]
    pub openings: Vec<Opening>,
    #[serde(default)]
    pub rooms: Vec<Room>,
    #[serde(default)]
    pub next_id: u32,
    #[serde(default)]
    pub next_opening_id: u32,
    #[serde(default)]
    pub next_room_id: u32,
    #[serde(default)]
    pub levels: Vec<Level>,
    #[serde(default)]
    pub types: Vec<WallType>,
    #[serde(default)]
    pub roofs: Vec<Roof>,
    #[serde(default)]
    pub next_roof_id: u32,
    #[serde(default)]
    pub stairs: Vec<Stair>,
    #[serde(default)]
    pub next_stair_id: u32,
    #[serde(default)]
    pub railings: Vec<Railing>,
    #[serde(default)]
    pub next_railing_id: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<Camera3d>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_camera: Option<String>,
    /// The one level the plan draws; `None` = all levels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_level: Option<u32>,
    #[serde(default = "template_plan_views", skip_serializing_if = "is_template_plan_views")]
    pub plan_views: Vec<PlanView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_plan_views: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_plan_view: Option<String>,
    // Skipped when empty, so a model without views writes the same payload it did before views existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "deserialize_views")]
    pub views: Vec<ArchView>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_view_id: u32,
    // Phase 9: the site. Each is skipped when empty, so a model without a site writes the
    // payload it wrote before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<SiteLocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terrain: Option<Terrain>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plants: Vec<Plant>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_plant_id: u32,
    /// Species the user added (the built-in catalogue is in `arch::plant`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plant_species: Vec<PlantSpecies>,
    // Phase 10: light fixtures, skipped when empty like the site.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lights: Vec<LightFixture>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_light_id: u32,
    // Phase 15: electrical devices, connections and light sets, skipped when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub electrical: Vec<ElectricalDevice>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_electrical_id: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<Connection>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_connection_id: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub light_sets: Vec<LightSet>,
    // Phase 13: cabinets and fixtures, skipped when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cabinets: Vec<Cabinet>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_cabinet_id: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixtures: Vec<Fixture>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_fixture_id: u32,
    // Phase 14: furnishings, symbols, paints and backdrop, skipped when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub furnishings: Vec<Furnishing>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_furnishing_id: u32,
    /// Every symbol a furnishing or a cabinet pull uses, by key.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub symbols: std::collections::BTreeMap<String, SymbolMesh>,
    /// Library material and backdrop images by item id.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub images: std::collections::BTreeMap<String, ImageRgba>,
    /// Paint by surface key.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub paints: std::collections::BTreeMap<String, Paint>,
    /// The 3D backdrop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backdrop: Option<Backdrop>,
    /// The global render look (its `backdrop` stays `None`: see `ArchModel::backdrop`).
    #[serde(default, skip_serializing_if = "Look::is_default")]
    pub look: Look,
    /// Named sun directions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sun_angles: Vec<SunAngle>,
    /// Set as Default: command parameters used when a command omits them.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub defaults: std::collections::BTreeMap<String, serde_json::Value>,
    // Phase 16: ceiling trays, interior soffits and foundations, skipped when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trays: Vec<Tray>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_tray_id: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub soffits: Vec<Soffit>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_soffit_id: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub foundations: Vec<Foundation>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_foundation_id: u32,
    // Phase 18: countertops, backsplashes and molding lines, skipped when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub countertops: Vec<Countertop>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_countertop_id: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backsplashes: Vec<Backsplash>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_backsplash_id: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub molding_lines: Vec<MoldingLine>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_molding_line_id: u32,
    /// Schedules (phase 21).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schedules: Vec<Schedule>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_schedule_id: u32,
    /// Sheets (phase 21).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sheets: Vec<Sheet>,
    /// Sheet-set project data.
    #[serde(default, skip_serializing_if = "SheetSet::is_default")]
    pub sheet_set: SheetSet,
}

impl Default for ArchModel {
    fn default() -> Self {
        ArchModel {
            walls: Vec::new(),
            openings: Vec::new(),
            rooms: Vec::new(),
            next_id: 0,
            next_opening_id: 0,
            next_room_id: 0,
            roofs: Vec::new(),
            next_roof_id: 0,
            stairs: Vec::new(),
            next_stair_id: 0,
            railings: Vec::new(),
            next_railing_id: 0,
            cameras: Vec::new(),
            active_camera: None,
            plan_level: None,
            plan_views: template_plan_views(),
            open_plan_views: Vec::new(),
            active_plan_view: None,
            views: Vec::new(),
            next_view_id: 0,
            site: None,
            terrain: None,
            plants: Vec::new(),
            next_plant_id: 0,
            lights: Vec::new(),
            next_light_id: 0,
            electrical: Vec::new(),
            next_electrical_id: 0,
            connections: Vec::new(),
            next_connection_id: 0,
            light_sets: Vec::new(),
            cabinets: Vec::new(),
            next_cabinet_id: 0,
            fixtures: Vec::new(),
            next_fixture_id: 0,
            furnishings: Vec::new(),
            next_furnishing_id: 0,
            symbols: Default::default(),
            images: Default::default(),
            paints: Default::default(),
            backdrop: None,
            look: Look::default(),
            sun_angles: Vec::new(),
            defaults: Default::default(),
            trays: Vec::new(),
            next_tray_id: 0,
            soffits: Vec::new(),
            next_soffit_id: 0,
            foundations: Vec::new(),
            next_foundation_id: 0,
            countertops: Vec::new(),
            next_countertop_id: 0,
            backsplashes: Vec::new(),
            next_backsplash_id: 0,
            molding_lines: Vec::new(),
            next_molding_line_id: 0,
            schedules: Vec::new(),
            next_schedule_id: 0,
            sheets: Vec::new(),
            sheet_set: SheetSet::default(),
            plant_species: Vec::new(),
            levels: vec![Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, kind: LevelKind::Floor }],
            types: vec![
                WallType {
                    name: "Exterior".into(),
                    layers: vec![
                        WallLayer { name: "siding".into(), thickness: 0.02, material: "siding".into() },
                        WallLayer { name: "sheathing".into(), thickness: 0.015, material: "sheathing".into() },
                        WallLayer { name: "stud".into(), thickness: 0.09, material: "stud".into() },
                        WallLayer { name: "drywall".into(), thickness: 0.013, material: "drywall".into() },
                    ],
                    reference: RefSide::Left,
                },
                WallType {
                    name: "Interior".into(),
                    layers: vec![
                        WallLayer { name: "drywall".into(), thickness: 0.013, material: "drywall".into() },
                        WallLayer { name: "stud".into(), thickness: 0.074, material: "stud".into() },
                        WallLayer { name: "drywall".into(), thickness: 0.013, material: "drywall".into() },
                    ],
                    reference: RefSide::Centre,
                },
            ],
        }
    }
}

impl ArchModel {
    /// Look up a plan view by trimmed, case-insensitive name.
    pub fn plan_view(&self, name: &str) -> Option<&PlanView> {
        let key = name.trim();
        if key.is_empty() {
            return None;
        }
        self.plan_views.iter().find(|v| v.name.trim().eq_ignore_ascii_case(key))
    }

    /// Look up a mutable plan view by trimmed, case-insensitive name.
    pub fn plan_view_mut(&mut self, name: &str) -> Option<&mut PlanView> {
        let key = name.trim();
        if key.is_empty() {
            return None;
        }
        self.plan_views.iter_mut().find(|v| v.name.trim().eq_ignore_ascii_case(key))
    }

    /// Add a wall and return its new id.
    pub fn push_wall(&mut self, mut w: Wall) -> ArchId {
        let id = ArchId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        w.id = id;
        self.walls.push(w);
        id
    }

    /// Add a wall and return its new id.
    pub fn add_wall(&mut self, w: Wall) -> ArchId {
        self.push_wall(w)
    }

    /// Look up a wall by id.
    pub fn wall(&self, id: ArchId) -> Option<&Wall> {
        self.walls.iter().find(|w| w.id == id)
    }

    /// The index of a wall by id, or `None` if not found.
    pub fn wall_index(&self, id: ArchId) -> Option<usize> {
        self.walls.iter().position(|w| w.id == id)
    }

    /// The id that will be assigned to the next wall.
    pub fn next_wall_id(&self) -> ArchId {
        ArchId(self.next_id)
    }

    /// The number of walls in the model.
    pub fn len(&self) -> usize {
        self.walls.len()
    }

    /// Iterate over all wall ids.
    pub fn wall_ids(&self) -> impl Iterator<Item = ArchId> + '_ {
        self.walls.iter().map(|w| w.id)
    }

    /// Whether the model has no walls, openings, rooms, roofs, stairs, railings, cameras or views.
    pub fn is_empty(&self) -> bool {
        self.walls.is_empty()
            && self.openings.is_empty()
            && self.rooms.is_empty()
            && self.roofs.is_empty()
            && self.stairs.is_empty()
            && self.railings.is_empty()
            && self.cameras.is_empty()
            && self.views.is_empty()
            && self.schedules.is_empty()
            && self.sheets.is_empty()
            && self.site.is_none()
            && self.terrain.is_none()
            && self.plants.is_empty()
            && self.plant_species.is_empty()
            && self.lights.is_empty()
            && self.electrical.is_empty()
            && self.connections.is_empty()
            && self.light_sets.is_empty()
            && self.cabinets.is_empty()
            && self.fixtures.is_empty()
            && self.furnishings.is_empty()
            && self.symbols.is_empty()
            && self.images.is_empty()
            && self.paints.is_empty()
            && self.backdrop.is_none()
            && self.look.is_default()
            && self.sun_angles.is_empty()
            && self.defaults.is_empty()
            && self.trays.is_empty()
            && self.soffits.is_empty()
            && self.foundations.is_empty()
            && self.countertops.is_empty()
            && self.backsplashes.is_empty()
            && self.molding_lines.is_empty()
    }

    /// Ensure there is always a ground level at index 0 for plan operations.
    pub fn ensure_ground_level(&mut self) {
        if self.levels.is_empty() {
            self.levels.push(Level { index: 0, name: "Level 1".to_string(), elevation: 0.0, ..Default::default() });
        }
    }

    /// Add a furnishing and return its new id.
    pub fn push_furnishing(&mut self, mut f: Furnishing) -> ArchId {
        let id = ArchId(self.next_furnishing_id);
        self.next_furnishing_id = self.next_furnishing_id.saturating_add(1);
        f.id = id;
        self.furnishings.push(f);
        id
    }

    /// The furnishing with id `id`.
    pub fn furnishing(&self, id: ArchId) -> Option<&Furnishing> {
        self.furnishings.iter().find(|f| f.id == id)
    }

    /// Mutable access to furnishing `id`.
    pub fn furnishing_mut(&mut self, id: ArchId) -> Option<&mut Furnishing> {
        self.furnishings.iter_mut().find(|f| f.id == id)
    }

    /// Add a cabinet and return its new id.
    pub fn push_cabinet(&mut self, mut c: Cabinet) -> ArchId {
        let id = ArchId(self.next_cabinet_id);
        self.next_cabinet_id = self.next_cabinet_id.saturating_add(1);
        c.id = id;
        self.cabinets.push(c);
        id
    }

    /// Add a plumbing or appliance fixture and return its new id.
    pub fn push_fixture(&mut self, mut f: Fixture) -> ArchId {
        let id = ArchId(self.next_fixture_id);
        self.next_fixture_id = self.next_fixture_id.saturating_add(1);
        f.id = id;
        self.fixtures.push(f);
        id
    }

    /// Add an electrical device and return its new id.
    pub fn push_electrical(&mut self, mut d: ElectricalDevice) -> ArchId {
        let id = ArchId(self.next_electrical_id);
        self.next_electrical_id = self.next_electrical_id.saturating_add(1);
        d.id = id;
        self.electrical.push(d);
        id
    }

    /// Add a connection and return its new id.
    pub fn push_connection(&mut self, mut c: Connection) -> ArchId {
        let id = ArchId(self.next_connection_id);
        self.next_connection_id = self.next_connection_id.saturating_add(1);
        c.id = id;
        self.connections.push(c);
        id
    }

    /// Add a ceiling tray and return its new id.
    pub fn push_tray(&mut self, mut t: Tray) -> ArchId {
        let id = ArchId(self.next_tray_id);
        self.next_tray_id = self.next_tray_id.saturating_add(1);
        t.id = id;
        self.trays.push(t);
        id
    }

    /// The tray with id `id`.
    pub fn tray(&self, id: ArchId) -> Option<&Tray> {
        self.trays.iter().find(|t| t.id == id)
    }

    /// Add an interior soffit and return its new id.
    pub fn push_soffit(&mut self, mut s: Soffit) -> ArchId {
        let id = ArchId(self.next_soffit_id);
        self.next_soffit_id = self.next_soffit_id.saturating_add(1);
        s.id = id;
        self.soffits.push(s);
        id
    }

    /// The soffit with id `id`.
    pub fn soffit(&self, id: ArchId) -> Option<&Soffit> {
        self.soffits.iter().find(|s| s.id == id)
    }

    /// Add a foundation and return its new id.
    pub fn push_foundation(&mut self, mut f: Foundation) -> ArchId {
        let id = ArchId(self.next_foundation_id);
        self.next_foundation_id = self.next_foundation_id.saturating_add(1);
        f.id = id;
        self.foundations.push(f);
        id
    }

    /// The foundation with id `id`.
    pub fn foundation(&self, id: ArchId) -> Option<&Foundation> {
        self.foundations.iter().find(|f| f.id == id)
    }

    /// Add a countertop and return its new id.
    pub fn push_countertop(&mut self, mut c: Countertop) -> ArchId {
        let id = ArchId(self.next_countertop_id);
        self.next_countertop_id = self.next_countertop_id.saturating_add(1);
        c.id = id;
        self.countertops.push(c);
        id
    }

    /// The countertop with id `id`.
    pub fn countertop(&self, id: ArchId) -> Option<&Countertop> {
        self.countertops.iter().find(|c| c.id == id)
    }

    /// The index of a countertop by id, or `None` if not found.
    pub fn countertop_index(&self, id: ArchId) -> Option<usize> {
        self.countertops.iter().position(|c| c.id == id)
    }

    /// Add a backsplash and return its new id.
    pub fn push_backsplash(&mut self, mut b: Backsplash) -> ArchId {
        let id = ArchId(self.next_backsplash_id);
        self.next_backsplash_id = self.next_backsplash_id.saturating_add(1);
        b.id = id;
        self.backsplashes.push(b);
        id
    }

    /// The backsplash with id `id`.
    pub fn backsplash(&self, id: ArchId) -> Option<&Backsplash> {
        self.backsplashes.iter().find(|b| b.id == id)
    }

    /// The index of a backsplash by id, or `None` if not found.
    pub fn backsplash_index(&self, id: ArchId) -> Option<usize> {
        self.backsplashes.iter().position(|b| b.id == id)
    }

    /// Add a molding line and return its new id.
    pub fn push_molding_line(&mut self, mut m: MoldingLine) -> ArchId {
        let id = ArchId(self.next_molding_line_id);
        self.next_molding_line_id = self.next_molding_line_id.saturating_add(1);
        m.id = id;
        self.molding_lines.push(m);
        id
    }

    /// The molding line with id `id`.
    pub fn molding_line(&self, id: ArchId) -> Option<&MoldingLine> {
        self.molding_lines.iter().find(|m| m.id == id)
    }

    /// The index of a molding line by id, or `None` if not found.
    pub fn molding_line_index(&self, id: ArchId) -> Option<usize> {
        self.molding_lines.iter().position(|m| m.id == id)
    }

    /// Look up a wall type by name (case-insensitive).
    pub fn wall_type(&self, name: &str) -> Option<&WallType> {
        self.types.iter().find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// Add an opening and return its new id.
    ///
    /// Openings use their own counter (`next_opening_id`) so the first opening
    /// placed is always `ArchId(0)` regardless of how many walls exist.
    pub fn push_opening(&mut self, mut o: Opening) -> ArchId {
        let id = ArchId(self.next_opening_id);
        self.next_opening_id = self.next_opening_id.saturating_add(1);
        o.id = id;
        self.openings.push(o);
        id
    }

    /// Look up an opening by id.
    pub fn opening(&self, id: ArchId) -> Option<&Opening> {
        self.openings.iter().find(|o| o.id == id)
    }

    /// The index of an opening by id, or `None` if not found.
    pub fn opening_index(&self, id: ArchId) -> Option<usize> {
        self.openings.iter().position(|o| o.id == id)
    }

    /// All openings on a given host wall, sorted by offset.
    pub fn openings_on(&self, host: ArchId) -> Vec<&Opening> {
        let mut v: Vec<&Opening> = self.openings.iter().filter(|o| o.host == host).collect();
        v.sort_by(|a, b| a.offset.partial_cmp(&b.offset).unwrap_or(std::cmp::Ordering::Equal));
        v
    }

    /// Remove an opening by id. Returns true if it was found.
    pub fn remove_opening(&mut self, id: ArchId) -> bool {
        if let Some(pos) = self.openings.iter().position(|o| o.id == id) {
            self.openings.remove(pos);
            true
        } else {
            false
        }
    }

    /// Look up a level by index.
    pub fn level(&self, index: u32) -> Option<&Level> {
        self.levels.iter().find(|l| l.index == index)
    }

    /// Add a room and return its new id.
    pub fn push_room(&mut self, mut r: Room) -> ArchId {
        let id = ArchId(self.next_room_id);
        self.next_room_id = self.next_room_id.saturating_add(1);
        r.id = id;
        self.rooms.push(r);
        id
    }

    /// Look up a room by id.
    pub fn room(&self, id: ArchId) -> Option<&Room> {
        self.rooms.iter().find(|r| r.id == id)
    }

    /// The index of a room by id, or `None` if not found.
    pub fn room_index(&self, id: ArchId) -> Option<usize> {
        self.rooms.iter().position(|r| r.id == id)
    }

    /// Remove a room by id. Returns true if it was found.
    pub fn remove_room(&mut self, id: ArchId) -> bool {
        if let Some(pos) = self.rooms.iter().position(|r| r.id == id) {
            self.rooms.remove(pos);
            true
        } else {
            false
        }
    }

    /// Iterate over all room ids.
    pub fn room_ids(&self) -> impl Iterator<Item = ArchId> + '_ {
        self.rooms.iter().map(|r| r.id)
    }

    /// Add a roof and return its new id.
    pub fn push_roof(&mut self, mut r: Roof) -> ArchId {
        let id = ArchId(self.next_roof_id);
        self.next_roof_id = self.next_roof_id.saturating_add(1);
        r.id = id;
        self.roofs.push(r);
        id
    }

    /// Look up a roof by id.
    pub fn roof(&self, id: ArchId) -> Option<&Roof> {
        self.roofs.iter().find(|r| r.id == id)
    }

    /// The index of a roof by id, or `None` if not found.
    pub fn roof_index(&self, id: ArchId) -> Option<usize> {
        self.roofs.iter().position(|r| r.id == id)
    }

    /// Iterate over all roof ids.
    pub fn roof_ids(&self) -> impl Iterator<Item = ArchId> + '_ {
        self.roofs.iter().map(|r| r.id)
    }

    /// Add a stair and return its new id.
    pub fn push_stair(&mut self, mut s: Stair) -> ArchId {
        let id = ArchId(self.next_stair_id);
        self.next_stair_id = self.next_stair_id.saturating_add(1);
        s.id = id;
        self.stairs.push(s);
        id
    }

    /// Look up a stair by id.
    pub fn stair(&self, id: ArchId) -> Option<&Stair> {
        self.stairs.iter().find(|s| s.id == id)
    }

    /// The index of a stair by id, or `None` if not found.
    pub fn stair_index(&self, id: ArchId) -> Option<usize> {
        self.stairs.iter().position(|s| s.id == id)
    }

    /// Iterate over all stair ids.
    pub fn stair_ids(&self) -> impl Iterator<Item = ArchId> + '_ {
        self.stairs.iter().map(|s| s.id)
    }

    /// Add a railing and return its new id.
    pub fn push_railing(&mut self, mut r: Railing) -> ArchId {
        let id = ArchId(self.next_railing_id);
        self.next_railing_id = self.next_railing_id.saturating_add(1);
        r.id = id;
        self.railings.push(r);
        id
    }

    /// Look up a railing by id.
    pub fn railing(&self, id: ArchId) -> Option<&Railing> {
        self.railings.iter().find(|r| r.id == id)
    }

    /// The index of a railing by id, or `None` if not found.
    pub fn railing_index(&self, id: ArchId) -> Option<usize> {
        self.railings.iter().position(|r| r.id == id)
    }

    /// Iterate over all railing ids.
    pub fn railing_ids(&self) -> impl Iterator<Item = ArchId> + '_ {
        self.railings.iter().map(|r| r.id)
    }

    /// Add a view and return its new id.
    ///
    /// Validates the view and refuses if it exceeds [`MAX_VIEWS`].
    pub fn push_view(&mut self, mut v: ArchView) -> Result<ArchId, ViewError> {
        if self.views.len() >= MAX_VIEWS {
            return Err(ViewError::TooMany);
        }
        v.validate()?;
        let id = ArchId(self.next_view_id);
        self.next_view_id = self.next_view_id.saturating_add(1);
        v.id = id;
        self.views.push(v);
        Ok(id)
    }

    /// Look up a view by id.
    pub fn view(&self, id: ArchId) -> Option<&ArchView> {
        self.views.iter().find(|v| v.id == id)
    }

    /// Look up a mutable view by id.
    pub fn view_mut(&mut self, id: ArchId) -> Option<&mut ArchView> {
        self.views.iter_mut().find(|v| v.id == id)
    }

    /// Remove a view by id.
    pub fn remove_view(&mut self, id: ArchId) -> Option<ArchView> {
        if let Some(pos) = self.views.iter().position(|v| v.id == id) { Some(self.views.remove(pos)) } else { None }
    }

    /// Look up a view by its label (e.g. "v0").
    pub fn view_by_label(&self, label: &str) -> Option<&ArchView> {
        self.views.iter().find(|v| view_label(v.id) == label)
    }

    /// A view by "v<n>", else by a unique case-insensitive name match.
    pub fn view_by_ref(&self, s: &str) -> Option<&ArchView> {
        let s = s.trim();
        if let Some(v) = self.view_by_label(s) {
            return Some(v);
        }
        let mut found = self.views.iter().filter(|v| v.name.trim().eq_ignore_ascii_case(s));
        let first = found.next()?;
        if found.next().is_some() { None } else { Some(first) }
    }

    /// The schedule with this id.
    pub fn schedule(&self, id: ArchId) -> Option<&Schedule> {
        self.schedules.iter().find(|s| s.id == id)
    }

    /// The schedule with this id, mutable.
    pub fn schedule_mut(&mut self, id: ArchId) -> Option<&mut Schedule> {
        self.schedules.iter_mut().find(|s| s.id == id)
    }

    /// A schedule by "sc<n>" or by name (trimmed, case-insensitive).
    pub fn schedule_by_ref(&self, s: &str) -> Option<&Schedule> {
        let s = s.trim();
        if let Some(id) = parse_schedule_label(s) {
            if let Some(found) = self.schedule(id) {
                return Some(found);
            }
        }
        self.schedules.iter().find(|x| x.name.trim().eq_ignore_ascii_case(s))
    }

    /// A sheet by name (trimmed, case-insensitive).
    pub fn sheet(&self, name: &str) -> Option<&Sheet> {
        let name = name.trim();
        self.sheets.iter().find(|s| s.name.trim().eq_ignore_ascii_case(name))
    }

    /// A sheet by name, mutable.
    pub fn sheet_mut(&mut self, name: &str) -> Option<&mut Sheet> {
        let name = name.trim();
        self.sheets.iter_mut().find(|s| s.name.trim().eq_ignore_ascii_case(name))
    }

    /// Union of every wall's poché ring bounds.
    ///
    /// Computed as `Bounds2::from_points` over each wall's four offset corners
    /// (start/end expanded by `thickness / 2.0` perpendicular to the centre line).
    /// Returns [`Bounds2::EMPTY`] when there are no walls.
    pub fn bounds(&self) -> Bounds2 {
        if self.walls.is_empty() {
            return Bounds2::EMPTY;
        }
        let mut pts = Vec::with_capacity(self.walls.len() * 4);
        for w in &self.walls {
            if !w.start.is_finite() || !w.end.is_finite() || w.thickness <= 0.0 || !w.thickness.is_finite() {
                continue;
            }
            let len = w.start.dist(w.end);
            if len < 1e-12 || !len.is_finite() {
                continue;
            }
            let half = w.thickness / 2.0;
            let dir = (w.end - w.start).normalized();
            let perp = dir.perp();
            pts.push(w.start - perp * half);
            pts.push(w.end - perp * half);
            pts.push(w.end + perp * half);
            pts.push(w.start + perp * half);
        }
        if pts.is_empty() { Bounds2::EMPTY } else { Bounds2::from_points(pts.iter().copied()) }
    }

    /// The index of a saved camera by name, or `None` if not found.
    pub fn camera_index(&self, name: &str) -> Option<usize> {
        self.cameras.iter().position(|c| c.name == name)
    }

    /// Look up a saved camera by name.
    pub fn camera_by_name(&self, name: &str) -> Option<&Camera3d> {
        self.cameras.iter().find(|c| c.name == name)
    }

    /// Whether every numeric field of a camera is finite.
    fn camera_is_finite(c: &Camera3d) -> bool {
        c.azimuth_deg.map_or(true, |v| v.is_finite())
            && c.elevation_deg.map_or(true, |v| v.is_finite())
            && c.distance.map_or(true, |v| v.is_finite())
            && c.eye_height.map_or(true, |v| v.is_finite())
            && c.fov_deg.map_or(true, |v| v.is_finite())
            && c.target.map_or(true, |t| t.iter().all(|v| v.is_finite()))
            && c.eye.map_or(true, |e| e.iter().all(|v| v.is_finite()))
            && c.look.as_ref().is_none_or(|l| l.validate().is_ok())
            && c.saved.as_ref().is_none_or(|s| s.saved.is_none() && Self::camera_is_finite(s))
    }

    /// Add or update a saved camera, storing it on the model so that it
    /// participates in undo, autosave, AUDIT and round-trip (decision 1).
    ///
    /// Refuses — never clamps — when the name is empty or whitespace, when a
    /// camera with that name already exists, when adding would exceed
    /// [`MAX_CAMERAS`], or when any numeric field is non-finite (decision 12).
    pub fn set_camera(&mut self, c: Camera3d) -> Result<(), CameraError> {
        if c.name.trim().is_empty() {
            return Err(CameraError::EmptyName);
        }
        if self.camera_by_name(&c.name).is_some() {
            return Err(CameraError::Duplicate);
        }
        if self.cameras.len() >= MAX_CAMERAS {
            return Err(CameraError::TooMany);
        }
        if !Self::camera_is_finite(&c) {
            return Err(CameraError::NotFinite);
        }
        self.cameras.push(c);
        Ok(())
    }

    /// Remove a saved camera by name. Returns `true` if it was found.
    ///
    /// Clears [`ArchModel::active_camera`] when it pointed at the removed
    /// camera.
    pub fn remove_camera(&mut self, name: &str) -> bool {
        if let Some(pos) = self.camera_index(name) {
            let was_active = self.active_camera.as_deref() == Some(name);
            self.cameras.remove(pos);
            if was_active {
                self.active_camera = None;
            }
            true
        } else {
            false
        }
    }

    /// A message when `counter` is at or below the largest id already in use, or
    /// `None` when nothing is in use or the in-use ids are at [`u32::MAX`].
    fn stale_next_id(&self, counter_name: &str, counter: u32, max_in_use: Option<u32>) -> Option<String> {
        let max = max_in_use?;
        if max == u32::MAX || counter > max {
            return None;
        }
        Some(format!("{}: next id {} is not above the largest id in use {}", counter_name, counter, max))
    }

    /// Validate the model, returning a list of human-readable error messages.
    ///
    /// Checks:
    /// - Wall count does not exceed [`MAX_WALLS`].
    /// - Per-wall: finite endpoints, thickness above minimum, non-zero length,
    ///   a finite height above zero, a known wall type, and a known level.
    /// - Per-kind id caps and no duplicate ids per kind.
    /// - Each `next_*_id` counter is above the largest id already in use for
    ///   its kind (skipped when the in-use ids reach [`u32::MAX`]).
    pub fn validate(&mut self) -> Vec<String> {
        let mut messages = Vec::new();

        if let Err(e) = self.look.validate() {
            messages.push(format!("render look reset: {e}"));
            self.look = Look::default();
        }
        self.look.backdrop = None;
        let before = self.sun_angles.len();
        let mut seen_angles = std::collections::HashSet::new();
        self.sun_angles.retain(|a| a.validate().is_ok() && seen_angles.insert(a.name.clone()));
        self.sun_angles.truncate(MAX_SUN_ANGLES);
        if self.sun_angles.len() != before {
            messages.push(format!("{} invalid or extra sun angle(s) dropped", before - self.sun_angles.len()));
        }

        if self.walls.len() > MAX_WALLS {
            messages.push(format!("model has {} walls, max {MAX_WALLS}", self.walls.len()));
        }

        for w in &self.walls {
            let id = format!("{}", w.id);

            if !w.start.is_finite() || !w.end.is_finite() {
                messages.push(format!("{}: non-finite endpoints", id));
                continue;
            }

            if !(w.thickness > MIN_WALL_THICKNESS) {
                messages.push(format!("{}: thickness {} below minimum {}", id, w.thickness, MIN_WALL_THICKNESS));
            }

            let length = w.start.dist(w.end);
            if !(length > MIN_WALL_LENGTH) {
                messages.push(format!("{}: zero or near-zero length", id));
            }

            if !w.height.is_finite() || w.height.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
                messages.push(format!("{}: invalid height {} (must be finite and > 0)", id, w.height));
            }

            if !w.wall_type.is_empty() && !is_builtin_wall_type(&w.wall_type) && self.wall_type(&w.wall_type).is_none() {
                messages.push(format!("{}: unknown wall type `{}`", id, w.wall_type));
            }

            if self.level(w.base_level).is_none() {
                messages.push(format!("{}: unknown level {}", id, w.base_level));
            }
        }

        let mut seen = std::collections::HashSet::new();
        let mut dupes = Vec::new();
        for w in &self.walls {
            if seen.contains(&w.id) {
                if !dupes.contains(&w.id) {
                    dupes.push(w.id);
                }
            } else {
                seen.insert(w.id);
            }
        }
        for id in &dupes {
            messages.push(format!("duplicate wall id {}", id));
        }

        if self.openings.len() > MAX_OPENINGS {
            messages.push(format!("model has {} openings, max {MAX_OPENINGS}", self.openings.len()));
        }

        for o in &self.openings {
            let id = format!("{}", o.id);

            if !o.offset.is_finite()
                || !o.width.is_finite()
                || !o.height.is_finite()
                || !o.sill.is_finite()
                || !o.recess.is_finite()
                || !o.frame_width.is_finite()
                || !o.frame_depth.is_finite()
                || !o.casing_width.is_finite()
                || !o.casing_depth.is_finite()
                || !o.sill_depth.is_finite()
                || !o.rough_width.is_finite()
                || !o.rough_height.is_finite()
                || !o.swing_angle.is_finite()
                || !o.percent_open.is_finite()
            {
                messages.push(format!("{}: non-finite numeric value", id));
                continue;
            }

            if !(o.width >= MIN_OPENING_WIDTH) {
                messages.push(format!("{}: width {} below minimum {}", id, o.width, MIN_OPENING_WIDTH));
            }

            if o.sill < 0.0 {
                messages.push(format!("{}: negative sill {}", id, o.sill));
            }

            if let Some(host) = self.wall(o.host) {
                if o.sill + o.height > host.height {
                    messages.push(format!("{}: sill+height {} exceeds wall height {}", id, o.sill + o.height, host.height));
                }
            } else {
                messages.push(format!("{}: unknown host wall {}", id, o.host));
                continue;
            }

            if o.rough_width < o.width {
                messages.push(format!("{}: rough width {} below finished width {}", id, o.rough_width, o.width));
            }

            if o.rough_height < o.height {
                messages.push(format!("{}: rough height {} below finished height {}", id, o.rough_height, o.height));
            }
        }

        let mut seen_op = std::collections::HashSet::new();
        let mut dupe_ops = Vec::new();
        for o in &self.openings {
            if seen_op.contains(&o.id) {
                if !dupe_ops.contains(&o.id) {
                    dupe_ops.push(o.id);
                }
            } else {
                seen_op.insert(o.id);
            }
        }
        for id in &dupe_ops {
            messages.push(format!("duplicate opening id {}", id));
        }

        if self.rooms.len() > MAX_ROOMS {
            messages.push(format!("model has {} rooms, max {MAX_ROOMS}", self.rooms.len()));
        }

        for r in &self.rooms {
            let id = format!("{}", r.id);
            if r.polygon.len() < 3 {
                messages.push(format!("{}: outer polygon has {} vertices (<3)", id, r.polygon.len()));
            }
            for p in &r.polygon {
                if !p.is_finite() {
                    messages.push(format!("{}: non-finite polygon point", id));
                    break;
                }
            }
            for hole in &r.holes {
                if hole.len() < 3 {
                    messages.push(format!("{}: hole has {} vertices (<3)", id, hole.len()));
                }
                for p in hole {
                    if !p.is_finite() {
                        messages.push(format!("{}: non-finite hole point", id));
                        break;
                    }
                }
            }
            if !r.interior_area.is_finite() || r.interior_area < 0.0 {
                messages.push(format!("{}: invalid interior area {}", id, r.interior_area));
            }
            if !r.standard_area.is_finite() || r.standard_area < 0.0 {
                messages.push(format!("{}: invalid standard area {}", id, r.standard_area));
            }
            if self.level(r.level).is_none() {
                messages.push(format!("{}: unknown level {}", id, r.level));
            }
        }

        let mut seen_room = std::collections::HashSet::new();
        let mut dupe_rooms = Vec::new();
        for r in &self.rooms {
            if seen_room.contains(&r.id) {
                if !dupe_rooms.contains(&r.id) {
                    dupe_rooms.push(r.id);
                }
            } else {
                seen_room.insert(r.id);
            }
        }
        for id in &dupe_rooms {
            messages.push(format!("duplicate room id {}", id));
        }

        if self.roofs.len() > MAX_ROOFS {
            messages.push(format!("model has {} roofs, max {MAX_ROOFS}", self.roofs.len()));
        }

        for r in &self.roofs {
            let id = format!("{}", r.id);

            if !r.pitch_rise.is_finite() || !r.pitch_run.is_finite() || !r.overhang.is_finite() || !r.thickness.is_finite() {
                messages.push(format!("{}: non-finite numeric value", id));
                continue;
            }

            if r.pitch_run <= 0.0 {
                messages.push(format!("{}: pitch run {} must be positive", id, r.pitch_run));
            }

            if r.planes.len() > MAX_ROOF_PLANES {
                messages.push(format!("{}: {} planes, max {MAX_ROOF_PLANES}", id, r.planes.len()));
            }

            if r.edges.len() > MAX_ROOF_EDGES {
                messages.push(format!("{}: {} edges, max {MAX_ROOF_EDGES}", id, r.edges.len()));
            }

            for (i, p) in r.planes.iter().enumerate() {
                if !p.slope.is_finite() || !p.projected_area.is_finite() || !p.thickness.is_finite() || !p.overhang.is_finite() {
                    messages.push(format!("{}: plane {} has non-finite value", id, i));
                }
            }

            for (i, e) in r.edges.iter().enumerate() {
                if !e.a.is_finite() || !e.b.is_finite() || !e.za.is_finite() || !e.zb.is_finite() {
                    messages.push(format!("{}: edge {} has non-finite value", id, i));
                }
            }
        }

        let mut seen_roof = std::collections::HashSet::new();
        let mut dupe_roofs = Vec::new();
        for r in &self.roofs {
            if seen_roof.contains(&r.id) {
                if !dupe_roofs.contains(&r.id) {
                    dupe_roofs.push(r.id);
                }
            } else {
                seen_roof.insert(r.id);
            }
        }
        for id in &dupe_roofs {
            messages.push(format!("duplicate roof id {}", id));
        }

        if self.stairs.len() > MAX_STAIRS {
            messages.push(format!("model has {} stairs, max {MAX_STAIRS}", self.stairs.len()));
        }

        for s in &self.stairs {
            let id = format!("{}", s.id);
            if !s.start.is_finite()
                || !s.direction.is_finite()
                || !s.width.is_finite()
                || !s.rise.is_finite()
                || !s.riser_height.is_finite()
                || !s.tread_depth.is_finite()
                || !s.run.is_finite()
                || !s.landing_depth.is_finite()
            {
                messages.push(format!("{}: non-finite numeric value", id));
            }
            if s.width <= 0.0 {
                messages.push(format!("{}: width {} must be positive", id, s.width));
            }
            if s.risers == 0 {
                messages.push(format!("{}: risers must be > 0", id));
            }
            if s.riser_height <= 0.0 {
                messages.push(format!("{}: riser height {} must be positive", id, s.riser_height));
            }
            if s.tread_depth <= 0.0 {
                messages.push(format!("{}: tread depth {} must be positive", id, s.tread_depth));
            }
            if self.level(s.from_level).is_none() {
                messages.push(format!("{}: unknown from_level {}", id, s.from_level));
            }
            if self.level(s.to_level).is_none() {
                messages.push(format!("{}: unknown to_level {}", id, s.to_level));
            }
            for p in &s.well {
                if !p.is_finite() {
                    messages.push(format!("{}: non-finite well point", id));
                    break;
                }
            }
            if !s.well.is_empty() && s.well.len() < 3 {
                messages.push(format!("{}: well has {} vertices (<3)", id, s.well.len()));
            }
            if let Some(room_id) = s.well_room {
                if self.room(room_id).is_none() {
                    messages.push(format!("{}: unknown well_room {}", id, room_id));
                }
            }
        }

        let mut seen_stair = std::collections::HashSet::new();
        let mut dupe_stairs = Vec::new();
        for s in &self.stairs {
            if seen_stair.contains(&s.id) {
                if !dupe_stairs.contains(&s.id) {
                    dupe_stairs.push(s.id);
                }
            } else {
                seen_stair.insert(s.id);
            }
        }
        for id in &dupe_stairs {
            messages.push(format!("duplicate stair id {}", id));
        }

        if self.railings.len() > MAX_RAILINGS {
            messages.push(format!("model has {} railings, max {MAX_RAILINGS}", self.railings.len()));
        }

        for r in &self.railings {
            let id = format!("{}", r.id);
            if !r.height.is_finite() || !r.thickness.is_finite() || !r.baluster_gap.is_finite() || !r.post_spacing.is_finite() {
                messages.push(format!("{}: non-finite numeric value", id));
            }
            if r.height <= 0.0 {
                messages.push(format!("{}: height {} must be positive", id, r.height));
            }
            if self.level(r.level).is_none() {
                messages.push(format!("{}: unknown level {}", id, r.level));
            }
            for p in &r.path {
                if !p.is_finite() {
                    messages.push(format!("{}: non-finite path point", id));
                    break;
                }
            }
            if !r.path.is_empty() && r.path.len() < 2 {
                messages.push(format!("{}: path has {} vertices (<2)", id, r.path.len()));
            }
            if r.thickness <= 0.0 {
                messages.push(format!("{}: thickness {} must be positive", id, r.thickness));
            }
            if r.post_spacing <= 0.0 {
                messages.push(format!("{}: post spacing {} must be positive", id, r.post_spacing));
            }
            if let Some(host_id) = r.follows {
                if r.host == RailingHost::Deck {
                    if !self.room(host_id).is_some_and(|rm| rm.room_type == RoomType::Deck) {
                        messages.push(format!("{}: unknown follows deck {}", id, host_id));
                    }
                } else if self.stair(host_id).is_none() {
                    messages.push(format!("{}: unknown follows stair {}", id, host_id));
                }
            }
        }

        let mut seen_railing = std::collections::HashSet::new();
        let mut dupe_railings = Vec::new();
        for r in &self.railings {
            if seen_railing.contains(&r.id) {
                if !dupe_railings.contains(&r.id) {
                    dupe_railings.push(r.id);
                }
            } else {
                seen_railing.insert(r.id);
            }
        }
        for id in &dupe_railings {
            messages.push(format!("duplicate railing id {}", id));
        }

        // Deserialization bypasses set_camera(), so apply the same checks here.
        if self.cameras.len() > MAX_CAMERAS {
            messages.push(format!("model has {} cameras, max {MAX_CAMERAS}", self.cameras.len()));
        }

        let mut seen_camera = std::collections::HashSet::new();
        let mut dupe_cameras = Vec::new();
        for c in &self.cameras {
            if c.name.trim().is_empty() {
                messages.push(format!("camera `{}`: empty name", c.name));
            }
            if !Self::camera_is_finite(c) {
                messages.push(format!("camera `{}`: non-finite numeric value", c.name));
            }
            if seen_camera.contains(&c.name) {
                if !dupe_cameras.contains(&c.name) {
                    dupe_cameras.push(c.name.clone());
                }
            } else {
                seen_camera.insert(c.name.clone());
            }
        }
        for name in &dupe_cameras {
            messages.push(format!("duplicate camera name `{name}`"));
        }

        // Stale id counters: the next id to assign must be above every id in use.
        if let Some(msg) = self.stale_next_id("next_id", self.next_id, self.walls.iter().map(|w| w.id.0).max()) {
            messages.push(msg);
        }
        if let Some(msg) = self.stale_next_id("next_opening_id", self.next_opening_id, self.openings.iter().map(|o| o.id.0).max()) {
            messages.push(msg);
        }
        if let Some(msg) = self.stale_next_id("next_room_id", self.next_room_id, self.rooms.iter().map(|r| r.id.0).max()) {
            messages.push(msg);
        }
        if let Some(msg) = self.stale_next_id("next_roof_id", self.next_roof_id, self.roofs.iter().map(|r| r.id.0).max()) {
            messages.push(msg);
        }
        if let Some(msg) = self.stale_next_id("next_stair_id", self.next_stair_id, self.stairs.iter().map(|s| s.id.0).max()) {
            messages.push(msg);
        }
        if let Some(msg) = self.stale_next_id("next_railing_id", self.next_railing_id, self.railings.iter().map(|r| r.id.0).max()) {
            messages.push(msg);
        }
        if let Some(msg) = self.stale_next_id("next_view_id", self.next_view_id, self.views.iter().map(|v| v.id.0).max()) {
            messages.push(msg);
        }
        if self.cabinets.len() > MAX_CABINETS {
            messages.push(format!("model has {} cabinets, max {MAX_CABINETS}", self.cabinets.len()));
        }
        if self.fixtures.len() > MAX_FIXTURES {
            messages.push(format!("model has {} fixtures, max {MAX_FIXTURES}", self.fixtures.len()));
        }
        if let Some(msg) = self.stale_next_id("next_cabinet_id", self.next_cabinet_id, self.cabinets.iter().map(|c| c.id.0).max()) {
            messages.push(msg);
        }
        if let Some(msg) = self.stale_next_id("next_fixture_id", self.next_fixture_id, self.fixtures.iter().map(|f| f.id.0).max()) {
            messages.push(msg);
        }

        // Validate views and drop invalid ones or those past the cap.
        if self.views.len() > MAX_VIEWS {
            messages.push(format!("model has {} views, max {MAX_VIEWS}", self.views.len()));
            self.views.truncate(MAX_VIEWS);
        }
        let mut valid_views = Vec::new();
        for mut v in self.views.drain(..) {
            let notes_before = v.notes.len();
            v.notes.truncate(MAX_VIEW_NOTES);
            v.notes.retain(|n| validate_note(n).is_ok());
            if v.notes.len() != notes_before {
                messages.push(format!("view `{}`: {} invalid or extra note(s) dropped", v.name, notes_before - v.notes.len()));
            }
            if v.validate().is_ok() {
                valid_views.push(v);
            } else {
                messages.push(format!("view `{}`: invalid definition", v.name));
            }
        }
        self.views = valid_views;

        // Schedules and sheets: drop invalid, repeated and extra ones.
        let level_count = self.levels.len();
        let before = self.schedules.len();
        let mut seen_ids = std::collections::BTreeSet::new();
        let mut seen_names = std::collections::BTreeSet::new();
        self.schedules
            .retain(|s| validate_schedule(s, level_count).is_ok() && seen_ids.insert(s.id) && seen_names.insert(s.name.trim().to_lowercase()));
        self.schedules.truncate(MAX_SCHEDULES);
        if self.schedules.len() != before {
            messages.push(format!("{} invalid or repeated schedule(s) dropped", before - self.schedules.len()));
        }
        if let Some(msg) = self.stale_next_id("next_schedule_id", self.next_schedule_id, self.schedules.iter().map(|s| s.id.0).max()) {
            messages.push(msg);
        }
        let before = self.sheets.len();
        let mut seen_names = std::collections::BTreeSet::new();
        let mut seen_numbers = std::collections::BTreeSet::new();
        self.sheets.retain(|s| {
            validate_sheet(s, level_count).is_ok()
                && seen_names.insert(s.name.trim().to_lowercase())
                && seen_numbers.insert(s.number.trim().to_lowercase())
        });
        self.sheets.truncate(MAX_SHEETS);
        if self.sheets.len() != before {
            messages.push(format!("{} invalid or repeated sheet(s) dropped", before - self.sheets.len()));
        }

        messages.extend(repair_plan_views(self));
        messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Drawing;
    use cadcraft_geom::Vec2;

    #[test]
    fn handle_json_is_the_low_bits_over_a000_0000_for_every_kind() {
        assert_eq!(handle_json(arch_handle(ArchId(3))), "a0000003");
        assert_eq!(handle_json(opening_handle(ArchId(0))), "a0008000");
        assert_eq!(handle_json(room_handle(ArchId(0))), "a0010000");
        assert_eq!(handle_json(roof_handle(ArchId(0))), "a0018000");
        assert_eq!(handle_json(stair_handle(ArchId(0))), "a0020000");
        assert_eq!(handle_json(railing_handle(ArchId(0))), "a0028000");
        assert_eq!(handle_json(Handle(0x2F)), "2f");
        assert_eq!(part_handle(ArchId(1), "glass"), opening_handle(ArchId(1)));
        assert_eq!(part_handle(ArchId(1), "deck"), room_handle(ArchId(1)));
        assert_eq!(part_handle(ArchId(1), "roof"), roof_handle(ArchId(1)));
        assert_eq!(part_handle(ArchId(1), "stair"), stair_handle(ArchId(1)));
        assert_eq!(part_handle(ArchId(1), "railing"), railing_handle(ArchId(1)));
        assert_eq!(part_handle(ArchId(1), "wall"), arch_handle(ArchId(1)));
    }

    fn test_wall() -> Wall {
        Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 2.0, height: 3.0, ..Wall::default() }
    }

    #[test]
    fn push_wall_assigns_increasing_ids_and_stores_them() {
        let mut m = ArchModel::default();
        let id1 = m.push_wall(test_wall());
        let id2 = m.push_wall(test_wall());
        assert_eq!(id1, ArchId(0));
        assert_eq!(id2, ArchId(1));
        assert!(m.wall(id1).is_some());
        assert!(m.wall(id2).is_some());
        assert_eq!(m.wall(id1).unwrap().start, Vec2::new(0.0, 0.0));
        assert_eq!(m.wall(id2).unwrap().end, Vec2::new(10.0, 0.0));
    }

    #[test]
    fn arch_model_add_returns_increasing_ids() {
        let mut m = ArchModel::default();
        let id1 = m.add_wall(Wall::default());
        let id2 = m.add_wall(Wall::default());
        assert_eq!(id1, ArchId(0));
        assert_eq!(id2, ArchId(1));
        assert!(m.wall(id1).is_some());
        assert!(m.wall(id2).is_some());
    }

    #[test]
    fn arch_model_lookup_by_id() {
        let mut m = ArchModel::default();
        let id = m.add_wall(test_wall());
        assert!(m.wall(id).is_some());
        assert_eq!(m.wall(id).unwrap().thickness, 2.0);
        assert!(m.wall(ArchId(999)).is_none());
    }

    #[test]
    fn empty_arch_model_is_empty() {
        let m = ArchModel::default();
        assert!(m.is_empty());
        assert_eq!(m.bounds(), Bounds2::EMPTY);
    }

    #[test]
    fn touch_arch_bumps_revision() {
        let mut d = Drawing::new_metric();
        assert_eq!(d.arch_revision, 0);
        d.touch_arch();
        d.touch_arch();
        assert_eq!(d.arch_revision, 2);
    }

    #[test]
    fn wall_new_has_default_height_and_no_layers() {
        let w = Wall::new(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), 0.15);
        assert_eq!(w.thickness, 0.15);
        assert_eq!(w.height, 2.4);
        assert!(w.layers.is_empty());
        assert_eq!(w.start, Vec2::new(0.0, 0.0));
        assert_eq!(w.end, Vec2::new(1.0, 0.0));
    }

    #[test]
    fn wall_default_is_a_usable_thin_wall() {
        let w = Wall::default();
        assert!(w.thickness > 0.0);
        assert!(w.height > 0.0);
    }

    #[test]
    fn wall_default_works_in_struct_update() {
        let w = Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), ..Wall::default() };
        assert_eq!(w.thickness, Wall::default().thickness);
    }

    #[test]
    fn wall_index_finds_position_and_none_for_unknown() {
        let mut m = ArchModel::default();
        m.push_wall(test_wall());
        m.push_wall(test_wall());
        m.push_wall(test_wall());
        assert_eq!(m.wall_index(ArchId(0)), Some(0));
        assert_eq!(m.wall_index(ArchId(1)), Some(1));
        assert_eq!(m.wall_index(ArchId(2)), Some(2));
        assert_eq!(m.wall_index(ArchId(99)), None);
    }

    #[test]
    fn next_wall_id_tracks_next_id() {
        let mut m = ArchModel::default();
        assert_eq!(m.next_wall_id(), ArchId(0));
        m.push_wall(test_wall());
        assert_eq!(m.next_wall_id(), ArchId(1));
        m.push_wall(test_wall());
        assert_eq!(m.next_wall_id(), ArchId(2));
    }

    #[test]
    fn wall_defaults_are_a_plain_exterior_wall() {
        let w = Wall::default();
        assert_eq!(w.join_start, JoinKind::Mitre);
        assert_eq!(w.join_end, JoinKind::Mitre);
        assert_eq!(w.base_level, 0);
        assert!(w.exterior);
        assert_eq!(w.wall_type, "Exterior");
    }

    #[test]
    fn arch_model_bounds_uses_poché_envelope() {
        let mut m = ArchModel::default();
        // Bottom wall: (0,0) to (10,0)
        m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, ..Wall::default() });
        // Right wall: (10,0) to (10,8)
        m.push_wall(Wall { start: Vec2::new(10.0, 0.0), end: Vec2::new(10.0, 8.0), thickness: 0.2, ..Wall::default() });
        // Top wall: (10,8) to (0,8)
        m.push_wall(Wall { start: Vec2::new(10.0, 8.0), end: Vec2::new(0.0, 8.0), thickness: 0.2, ..Wall::default() });
        // Left wall: (0,8) to (0,0)
        m.push_wall(Wall { start: Vec2::new(0.0, 8.0), end: Vec2::new(0.0, 0.0), thickness: 0.2, ..Wall::default() });
        // Degenerate zero-length wall (should be skipped)
        m.push_wall(Wall { start: Vec2::new(5.0, 4.0), end: Vec2::new(5.0, 4.0), thickness: 0.2, ..Wall::default() });

        let b = m.bounds();
        assert!((b.min.x - (-0.1)).abs() < 1e-9, "min.x: {} != -0.1", b.min.x);
        assert!((b.min.y - (-0.1)).abs() < 1e-9, "min.y: {} != -0.1", b.min.y);
        assert!((b.max.x - 10.1).abs() < 1e-9, "max.x: {} != 10.1", b.max.x);
        assert!((b.max.y - 8.1).abs() < 1e-9, "max.y: {} != 8.1", b.max.y);
    }

    #[test]
    fn arch_model_defaults_have_two_wall_types() {
        let m = ArchModel::default();
        assert_eq!(m.types.len(), 2);
        assert_eq!(m.types[0].name, "Exterior");
        assert_eq!(m.types[1].name, "Interior");
    }

    #[test]
    fn exterior_wall_type_layers_sum_to_thickness() {
        let m = ArchModel::default();
        let exterior = &m.types[0];
        assert_eq!(exterior.layers.len(), 4);
        let sum: f64 = exterior.layers.iter().map(|l| l.thickness).sum();
        assert!((sum - 0.138).abs() < 1e-9);
    }

    #[test]
    fn level_default_is_ground_floor() {
        let m = ArchModel::default();
        let level = &m.levels[0];
        assert_eq!(level.index, 0);
        assert_eq!(level.elevation, 0.0);
        assert_eq!(level.ceiling_height, 2.4);
        assert_eq!(level.structure, 0.3);
    }

    #[test]
    fn level_kind_defaults_to_floor() {
        assert_eq!(LevelKind::default(), LevelKind::Floor);
        assert_eq!(Level::default().kind, LevelKind::Floor);
    }

    #[test]
    fn a_floor_level_serializes_as_before_and_a_landing_keeps_its_kind() {
        // A drawing without landings writes the payload it wrote before level kinds existed.
        let floor = serde_json::to_value(Level::default()).unwrap();
        assert!(floor.get("kind").is_none(), "a floor level writes no kind: {floor}");
        let landing = Level { index: 1, elevation: 1.35, kind: LevelKind::Landing, ..Level::default() };
        let v = serde_json::to_value(&landing).unwrap();
        assert_eq!(v.get("kind").and_then(|k| k.as_str()), Some("landing"));
        let back: Level = serde_json::from_value(v).unwrap();
        assert_eq!(back, landing);
    }

    #[test]
    fn a_wall_spanning_two_levels_reaches_the_upper_ceiling() {
        let levels = vec![
            Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Level::default() },
            Level { index: 1, elevation: 3.0, ceiling_height: 2.8, ..Level::default() },
        ];
        let wall = Wall { base_level: 0, top_level: Some(1), height: 2.0, ..Wall::default() };
        assert!((wall.bottom_z(&levels) - 0.0).abs() < 1e-12);
        assert!((wall.top_z(&levels) - 5.8).abs() < 1e-12);
    }

    #[test]
    fn a_plain_wall_spans_only_its_level() {
        let levels = vec![
            Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Level::default() },
            Level { index: 1, elevation: 3.0, ceiling_height: 2.8, ..Level::default() },
        ];
        let wall = Wall { base_level: 0, height: 2.4, ..Wall::default() };
        assert!(wall.spans_level(&levels[0], &levels));
        assert!(!wall.spans_level(&levels[1], &levels));
    }

    #[test]
    fn new_room_types_round_trip() {
        for room_type in [RoomType::Dining, RoomType::Family, RoomType::Entry, RoomType::Laundry] {
            let json = serde_json::to_string(&room_type).unwrap();
            let back: RoomType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, room_type);
        }
    }

    #[test]
    fn ensure_ground_level_adds_level_one_once() {
        let mut m = ArchModel::default();
        m.levels.clear();
        m.ensure_ground_level();
        assert_eq!(m.levels.len(), 1);
        assert_eq!(m.levels[0].index, 0);
        assert_eq!(m.levels[0].name, "Level 1");
        assert_eq!(m.levels[0].elevation, 0.0);
        m.ensure_ground_level();
        assert_eq!(m.levels.len(), 1);
    }

    #[test]
    fn join_kind_defaults_to_mitre() {
        assert_eq!(JoinKind::default(), JoinKind::Mitre);
    }

    #[test]
    fn wall_type_lookup_is_case_insensitive() {
        let m = ArchModel::default();
        assert!(m.wall_type("exterior").is_some());
    }

    #[test]
    fn validate_accepts_a_clean_model() {
        let mut m = ArchModel::default();
        assert!(m.validate().is_empty());

        let mut m = ArchModel::default();
        m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        assert!(m.validate().is_empty());
    }

    #[test]
    fn validate_rejects_non_positive_thickness_and_zero_length() {
        let mut m = ArchModel::default();

        // thickness 0
        m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.0, ..Wall::default() });

        // thickness -1
        m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: -1.0, ..Wall::default() });

        // start == end (zero length)
        m.push_wall(Wall { start: Vec2::new(5.0, 5.0), end: Vec2::new(5.0, 5.0), thickness: 0.2, ..Wall::default() });

        // NaN thickness
        m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: f64::NAN, ..Wall::default() });

        let msgs = m.validate();
        assert_eq!(msgs.len(), 4);
        assert!(msgs[0].contains("below minimum"));
        assert!(msgs[1].contains("below minimum"));
        assert!(msgs[2].contains("zero or near-zero length"));
        assert!(msgs[3].contains("below minimum"));
    }

    #[test]
    fn validate_reports_unknown_wall_type_and_level() {
        let mut m = ArchModel::default();
        m.push_wall(Wall {
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            thickness: 0.2,
            height: 2.7,
            base_level: 7,
            exterior: true,
            wall_type: "Bogus".into(),
            ..Wall::default()
        });

        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("unknown wall type")));
        assert!(msgs.iter().any(|m| m.contains("unknown level")));
    }

    #[test]
    fn validate_reports_duplicate_ids() {
        let mut m = ArchModel::default();
        m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        m.push_wall(Wall::new(Vec2::new(20.0, 0.0), Vec2::new(30.0, 0.0), 0.2));
        m.walls[1].id = m.walls[0].id;

        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("duplicate wall id")));
    }

    #[test]
    fn validate_flags_too_many_walls() {
        let mut m = ArchModel::default();
        for _ in 0..MAX_WALLS + 1 {
            m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), 0.2));
        }
        let msgs = m.validate();
        let count = msgs.iter().filter(|m| m.contains("max")).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn arch_handle_round_trips_and_is_recognised() {
        assert!(is_arch_handle(arch_handle(ArchId(0))));
        assert!(is_arch_handle(arch_handle(ArchId(7))));
        assert_ne!(arch_handle(ArchId(0)), arch_handle(ArchId(1)));
    }

    #[test]
    fn is_arch_handle_false_for_entity_handles() {
        assert!(!is_arch_handle(Handle(0)));
        assert!(!is_arch_handle(Handle(0x9FFF_FFFF)));
    }

    #[test]
    fn drawing_default_has_an_empty_arch_model() {
        assert!(Drawing::new_metric().arch.is_empty());
        assert!(Drawing::default().arch.walls.is_empty());
    }

    #[test]
    fn next_wall_id_never_reuses_an_id() {
        let mut m = ArchModel::default();
        let id1 = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), 0.2));
        let id2 = m.push_wall(Wall::new(Vec2::new(2.0, 0.0), Vec2::new(3.0, 0.0), 0.2));
        assert_ne!(id1, id2);
        assert_ne!(id1, m.next_wall_id());
        assert_ne!(id2, m.next_wall_id());
        assert_eq!(m.next_wall_id(), ArchId(2));
    }

    #[test]
    fn arch_id_from_str_is_case_insensitive() {
        assert_eq!("w12".parse::<ArchId>().unwrap(), ArchId(12));
        assert_eq!("W12".parse::<ArchId>().unwrap(), ArchId(12));
    }

    #[test]
    fn opening_ids_have_their_own_counter() {
        let mut m = ArchModel::default();
        let wid = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        let oid = m.push_opening(Opening::new(wid, 1.0, 0.9, 2.1, OpeningKind::Door));
        assert_eq!(wid, ArchId(0));
        assert_eq!(oid, ArchId(0));
        assert_eq!(m.next_wall_id(), ArchId(1));
        assert_eq!(m.next_opening_id, 1);
        assert!(m.opening(oid).is_some());
        assert!(m.opening(ArchId(999)).is_none());
    }

    #[test]
    fn push_opening_shares_the_id_counter_with_walls() {
        let mut m = ArchModel::default();
        let w0 = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        let w1 = m.push_wall(Wall::new(Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), 0.2));
        let o0 = m.push_opening(Opening::new(w0, 1.0, 0.9, 2.1, OpeningKind::Door));
        let o1 = m.push_opening(Opening::new(w1, 1.0, 0.9, 2.1, OpeningKind::Window));
        assert_eq!(w0, ArchId(0));
        assert_eq!(w1, ArchId(1));
        assert_eq!(o0, ArchId(0));
        assert_eq!(o1, ArchId(1));
        assert_eq!(m.next_wall_id(), ArchId(2));
        assert_eq!(m.next_opening_id, 2);
        assert!(m.opening(o0).is_some());
        assert!(m.opening(o1).is_some());
        assert!(m.opening(ArchId(2)).is_none());
    }

    #[test]
    fn opening_defaults_have_rough_larger_than_finished() {
        let o = Opening::new(ArchId(1), 0.5, 0.9, 2.1, OpeningKind::Door);
        assert!((o.rough_width - (o.width + 0.12)).abs() < 1e-12);
        assert!((o.rough_height - (o.height + 0.12)).abs() < 1e-12);
        assert!(o.rough_width > o.width);
        assert!(o.rough_height > o.height);

        let d = Opening::default();
        assert!(d.rough_width > d.width);
        assert!(d.rough_height > d.height);
    }

    #[test]
    fn opening_defaults_pick_the_a_door_or_a_glaz_layer() {
        let door = Opening::new(ArchId(1), 0.0, 0.9, 2.1, OpeningKind::Door);
        let window = Opening::new(ArchId(1), 0.0, 0.9, 2.1, OpeningKind::Window);
        let garage = Opening::new(ArchId(1), 0.0, 2.0, 2.5, OpeningKind::GarageDoor);
        let cased = Opening::new(ArchId(1), 0.0, 0.5, 2.0, OpeningKind::CasedOpening);
        assert_eq!(door.layer, "A-DOOR");
        assert_eq!(window.layer, "A-GLAZ");
        assert_eq!(garage.layer, "A-DOOR");
        assert_eq!(cased.layer, "A-DOOR");
    }

    #[test]
    fn validate_rejects_an_opening_on_a_missing_host() {
        let mut m = ArchModel::default();
        m.push_opening(Opening::new(ArchId(999), 0.0, 0.9, 2.1, OpeningKind::Door));
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("unknown host wall")));
    }

    #[test]
    fn validate_rejects_a_sill_above_the_wall_height() {
        let mut m = ArchModel::default();
        let wid = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        let mut o = Opening::new(wid, 1.0, 0.9, 1.5, OpeningKind::Window);
        o.sill = 2.0;
        m.push_opening(o);
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("exceeds wall height")));
    }

    #[test]
    fn validate_rejects_too_many_openings() {
        let mut m = ArchModel::default();
        for i in 0..MAX_OPENINGS + 1 {
            m.push_opening(Opening::new(ArchId(i as u32), 0.0, 0.9, 2.1, OpeningKind::Door));
        }
        let msgs = m.validate();
        let count = msgs.iter().filter(|m| m.contains("max")).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn opening_json_roundtrips_with_rough_and_recess() {
        let mut o = Opening::new(ArchId(1), 0.5, 1.2, 1.8, OpeningKind::Window);
        o.recess = 0.08;
        o.rough_width = 1.32;
        o.rough_height = 1.92;
        o.id = ArchId(42);
        let json = serde_json::to_string(&o).unwrap();
        let back: Opening = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, ArchId(42));
        assert_eq!(back.host, ArchId(1));
        assert_eq!(back.recess, 0.08);
        assert_eq!(back.rough_width, 1.32);
        assert_eq!(back.rough_height, 1.92);
        assert_eq!(back.layer, "A-GLAZ");
    }

    #[test]
    fn opening_handles_are_distinct_from_wall_handles() {
        let wh = arch_handle(ArchId(0));
        let oh = opening_handle(ArchId(0));
        assert_ne!(wh, oh);
        assert!(is_arch_handle(wh));
        assert!(is_arch_handle(oh));
        assert!(is_opening_handle(oh));
        assert!(!is_opening_handle(wh));
        assert!(!is_arch_handle(Handle(0)));
    }

    #[test]
    fn opening_handle_hex_starts_with_a000() {
        // ARCH_HANDLE_BASE moved from 0xA000_0000 to 0xFFFF_FFFF_0000_0000; the hex
        // prefix is now "ffffffff0000" rather than "a000". Assertions updated accordingly.
        let oh = opening_handle(ArchId(0));
        let hex = oh.hex().to_lowercase();
        assert!(hex.starts_with("ffffffff0000"), "opening handle hex is `{hex}`, expected arch-base prefix");
        let oh7 = opening_handle(ArchId(7));
        let hex7 = oh7.hex().to_lowercase();
        assert!(hex7.starts_with("ffffffff0000"), "opening handle hex is `{hex7}`, expected arch-base prefix");
    }

    #[test]
    fn opening_handle_hex_starts_with_arch_base_prefix() {
        // With ARCH_HANDLE_BASE = 0xFFFF_FFFF_0000_0000 the prefix is "ffffffff0000".
        let oh = opening_handle(ArchId(0));
        let hex = oh.hex().to_lowercase();
        assert!(hex.starts_with("ffffffff0000"), "opening handle hex is `{hex}`, expected arch-base prefix");
        let oh7 = opening_handle(ArchId(7));
        let hex7 = oh7.hex().to_lowercase();
        assert!(hex7.starts_with("ffffffff0000"), "opening handle hex is `{hex7}`, expected arch-base prefix");
    }

    #[test]
    fn validate_flags_duplicate_opening_ids() {
        let mut m = ArchModel::default();
        let wid = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        m.push_opening(Opening::new(wid, 1.0, 0.9, 2.1, OpeningKind::Door));
        let oid = m.openings.last().unwrap().id.clone();
        m.push_opening(Opening::new(wid, 2.0, 0.9, 2.1, OpeningKind::Door));
        m.openings.last_mut().unwrap().id = oid;

        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("duplicate opening id")));
    }

    #[test]
    fn level_floor_to_floor_is_derived() {
        let l = Level { ceiling_height: 2.4, structure: 0.3, ..Level::default() };
        assert!((l.floor_to_floor() - 2.7).abs() < 1e-12);
        let l2 = Level { ceiling_height: 3.0, structure: 0.2, ..Level::default() };
        assert!((l2.floor_to_floor() - 3.2).abs() < 1e-12);
        let l3 = Level { ceiling_height: 0.0, structure: 0.3, ..Level::default() };
        assert_eq!(l3.floor_to_floor(), 0.0);
        let l4 = Level { ceiling_height: f64::NAN, structure: 0.3, ..Level::default() };
        assert_eq!(l4.floor_to_floor(), 0.0);
    }

    #[test]
    fn level_defaults_are_ceiling_2_4_structure_0_3() {
        let l = Level::default();
        assert!((l.ceiling_height - 2.4).abs() < 1e-12);
        assert!((l.structure - 0.3).abs() < 1e-12);
        let m = ArchModel::default();
        let level = &m.levels[0];
        assert!((level.ceiling_height - 2.4).abs() < 1e-12);
        assert!((level.structure - 0.3).abs() < 1e-12);
    }

    #[test]
    fn push_room_assigns_increasing_ids() {
        let mut m = ArchModel::default();
        let r1 = m.push_room(Room::default());
        let r2 = m.push_room(Room::default());
        assert_eq!(r1, ArchId(0));
        assert_eq!(r2, ArchId(1));
        assert!(m.room(r1).is_some());
        assert!(m.room(r2).is_some());
        assert_eq!(m.next_room_id, 2);
    }

    #[test]
    fn room_handle_is_in_the_room_range() {
        let rh = room_handle(ArchId(0));
        assert!(is_room_handle(rh));
        assert!(is_arch_handle(rh));
        assert!(!is_opening_handle(rh));
        let rh7 = room_handle(ArchId(7));
        assert!(is_room_handle(rh7));
        assert_ne!(rh, rh7);
        // Wall handle should not be a room handle.
        assert!(!is_room_handle(arch_handle(ArchId(0))));
    }

    #[test]
    fn room_type_interior_and_ceiling_rules() {
        assert!(RoomType::Living.is_interior());
        assert!(RoomType::Bedroom.is_interior());
        assert!(RoomType::Kitchen.is_interior());
        assert!(RoomType::Bath.is_interior());
        assert!(RoomType::Closet.is_interior());
        assert!(RoomType::Utility.is_interior());
        assert!(RoomType::Other.is_interior());
        assert!(!RoomType::Garage.is_interior());
        assert!(!RoomType::Porch.is_interior());
        assert!(!RoomType::Deck.is_interior());
        assert!(!RoomType::OpenBelow.is_interior());

        assert!(RoomType::Living.has_ceiling());
        assert!(RoomType::Bedroom.has_ceiling());
        assert!(!RoomType::Deck.has_ceiling());
        assert!(!RoomType::Porch.has_ceiling());

        assert!(RoomType::Living.has_floor());
        assert!(RoomType::Garage.has_floor());
        assert!(!RoomType::OpenBelow.has_floor());

        assert_eq!(RoomType::default(), RoomType::Other);
    }

    #[test]
    fn validate_rejects_degenerate_room() {
        let mut m = ArchModel::default();
        // Polygon with only 2 points
        m.push_room(Room { polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)], ..Room::default() });
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("<3")));
    }

    #[test]
    fn divider_wall_defaults_to_false() {
        let w = Wall::default();
        assert!(!w.divider);
        let w2 = Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2);
        assert!(!w2.divider);
    }

    #[test]
    fn roof_default_is_six_twelfths() {
        let r = Roof::default();
        assert_eq!(r.pitch_rise, 6.0);
        assert_eq!(r.pitch_run, 12.0);
        assert!((r.slope() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn push_roof_assigns_ids() {
        let mut m = ArchModel::default();
        let id1 = m.push_roof(Roof::default());
        let id2 = m.push_roof(Roof::default());
        assert_eq!(id1, ArchId(0));
        assert_eq!(id2, ArchId(1));
        assert!(m.roof(id1).is_some());
        assert!(m.roof(id2).is_some());
        assert_eq!(m.next_roof_id, 2);
    }

    #[test]
    fn roof_handle_is_disjoint_from_room_and_opening() {
        let rh = roof_handle(ArchId(0));
        let rhm = room_handle(ArchId(0));
        let oh = opening_handle(ArchId(0));
        assert!(is_roof_handle(rh));
        assert!(is_arch_handle(rh));
        assert_ne!(rh, rhm);
        assert_ne!(rh, oh);
        assert!(!is_room_handle(rh));
        assert!(!is_opening_handle(rh));
    }

    #[test]
    fn wall_roof_fields_default_to_auto() {
        let w = Wall::default();
        assert_eq!(w.roof_directive, RoofDirective::default());
        assert!(w.roof_pitch.is_none());
        assert!(w.roof_overhang.is_none());
        assert!(w.eave_height.is_none());

        let w2 = Wall::new(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), 0.15);
        assert_eq!(w2.roof_directive, RoofDirective::default());
        assert!(w2.roof_pitch.is_none());
        assert!(w2.roof_overhang.is_none());
        assert!(w2.eave_height.is_none());
    }

    #[test]
    fn roof_projected_area_sums_planes() {
        let mut r = Roof::default();
        r.planes.push(RoofPlane {
            ring: Vec::new(),
            slope: 0.5,
            projected_area: 10.5,
            eave_wall: None,
            base_edge: [0.0, 0.0, 0.0, 0.0],
            thickness: 0.25,
            eave_z: 0.0,
            ..RoofPlane::default()
        });
        r.planes.push(RoofPlane {
            ring: Vec::new(),
            slope: 0.5,
            projected_area: 7.3,
            eave_wall: None,
            base_edge: [0.0, 0.0, 0.0, 0.0],
            thickness: 0.25,
            eave_z: 0.0,
            ..RoofPlane::default()
        });
        assert!((r.projected_area() - 17.8).abs() < 1e-9);
    }

    #[test]
    fn roof_slope_handles_degenerate_inputs() {
        let r = Roof { pitch_rise: f64::NAN, pitch_run: 12.0, ..Roof::default() };
        assert!((r.slope() - 0.5).abs() < 1e-12);

        let r2 = Roof { pitch_rise: 6.0, pitch_run: 0.0, ..Roof::default() };
        assert!((r2.slope() - 0.5).abs() < 1e-12);

        let r3 = Roof { pitch_rise: 6.0, pitch_run: -12.0, ..Roof::default() };
        assert!((r3.slope() - 0.5).abs() < 1e-12);

        let r4 = Roof { pitch_rise: f64::INFINITY, pitch_run: 12.0, ..Roof::default() };
        assert!((r4.slope() - 0.5).abs() < 1e-12);

        let r5 = Roof { pitch_rise: 8.0, pitch_run: 12.0, ..Roof::default() };
        assert!((r5.slope() - 2.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn roof_projected_area_ignores_non_finite() {
        let mut r = Roof::default();
        r.planes.push(RoofPlane {
            ring: Vec::new(),
            slope: 0.5,
            projected_area: 10.0,
            eave_wall: None,
            base_edge: [0.0; 4],
            thickness: 0.25,
            eave_z: 0.0,
            ..RoofPlane::default()
        });
        r.planes.push(RoofPlane {
            ring: Vec::new(),
            slope: 0.5,
            projected_area: f64::NAN,
            eave_wall: None,
            base_edge: [0.0; 4],
            thickness: 0.25,
            eave_z: 0.0,
            ..RoofPlane::default()
        });
        r.planes.push(RoofPlane {
            ring: Vec::new(),
            slope: 0.5,
            projected_area: f64::INFINITY,
            eave_wall: None,
            base_edge: [0.0; 4],
            thickness: 0.25,
            eave_z: 0.0,
            ..RoofPlane::default()
        });
        assert!((r.projected_area() - 10.0).abs() < 1e-12);
    }

    #[test]
    fn roof_handle_range_boundaries() {
        let base = ARCH_HANDLE_BASE + ROOF_HANDLE_OFFSET;
        assert!(is_roof_handle(Handle(base)));
        assert!(is_roof_handle(Handle(base + 32767)));
        assert!(!is_roof_handle(Handle(base + 32768)));
        assert!(!is_roof_handle(Handle(base - 1)));
    }

    #[test]
    fn validate_rejects_too_many_roofs() {
        let mut m = ArchModel::default();
        for _ in 0..MAX_ROOFS + 1 {
            m.push_roof(Roof::default());
        }
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("max") && m.contains("roofs")));
    }

    #[test]
    fn validate_flags_duplicate_roof_ids() {
        let mut m = ArchModel::default();
        m.push_roof(Roof::default());
        m.push_roof(Roof::default());
        m.roofs[1].id = m.roofs[0].id;
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("duplicate roof id")));
    }

    #[test]
    fn validate_rejects_non_finite_roof_fields() {
        let mut m = ArchModel::default();
        m.push_roof(Roof { pitch_rise: f64::NAN, ..Roof::default() });
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("non-finite")));
    }

    #[test]
    fn roof_json_roundtrips() {
        let mut r = Roof::default();
        r.id = ArchId(5);
        r.planes.push(RoofPlane {
            ring: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0)],
            slope: 0.5,
            projected_area: 80.0,
            eave_wall: Some(ArchId(2)),
            base_edge: [0.0, 0.0, 10.0, 0.0],
            thickness: 0.25,
            eave_z: 0.0,
            ..RoofPlane::default()
        });
        r.edges.push(RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(4.0, 4.0), b: Vec2::new(6.0, 4.0), za: 4.45, zb: 4.45 });
        let json = serde_json::to_string(&r).unwrap();
        let back: Roof = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, ArchId(5));
        assert_eq!(back.planes.len(), 1);
        assert_eq!(back.planes[0].projected_area, 80.0);
        assert_eq!(back.planes[0].eave_wall, Some(ArchId(2)));
        assert_eq!(back.edges.len(), 1);
        assert_eq!(back.edges[0].kind, RoofEdgeKind::Ridge);
        assert!((back.edges[0].za - 4.45).abs() < 1e-12);
    }

    #[test]
    fn validate_reports_a_non_positive_or_nan_height() {
        for (i, height) in [0.0, -2.0, f64::NAN, f64::INFINITY].iter().enumerate() {
            let mut m = ArchModel::default();
            m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: *height, ..Wall::default() });
            let msgs = m.validate();
            assert!(!msgs.is_empty() && msgs.iter().any(|m| m.contains("height")), "bad height {i} ({}) not reported: {msgs:?}", height);
        }
    }

    #[test]
    fn validate_reports_a_stale_next_id_counter() {
        // Two walls use ids 0 and 1; pinning next_id to 1 is not above the in-use max.
        let mut m = ArchModel::default();
        m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        m.push_wall(Wall::new(Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), 0.2));
        m.next_id = 1;
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("next id") && m.contains("next_id")), "stale next_id not reported: {msgs:?}");

        // One opening uses id 0; pinning next_opening_id to 0 is not above the in-use max.
        let mut m = ArchModel::default();
        let wid = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        m.push_opening(Opening::new(wid, 0.0, 0.9, 2.1, OpeningKind::Door));
        m.next_opening_id = 0;
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("next id") && m.contains("next_opening_id")), "stale next_opening_id not reported: {msgs:?}");
    }

    #[test]
    fn validate_reports_stale_next_room_id_counter() {
        let mut m = ArchModel::default();
        m.push_room(Room::default());
        m.push_room(Room::default());
        m.next_room_id = 1;
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("next id") && m.contains("next_room_id")), "stale next_room_id not reported: {msgs:?}");
    }

    #[test]
    fn validate_reports_stale_next_roof_id_counter() {
        let mut m = ArchModel::default();
        m.push_roof(Roof::default());
        m.push_roof(Roof::default());
        m.next_roof_id = 1;
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("next id") && m.contains("next_roof_id")), "stale next_roof_id not reported: {msgs:?}");
    }

    #[test]
    fn validate_accepts_very_small_positive_height() {
        let mut m = ArchModel::default();
        m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 1e-9, ..Wall::default() });
        let msgs = m.validate();
        assert!(msgs.iter().all(|m| !m.contains("height")), "tiny positive height wrongly rejected: {msgs:?}");
    }

    #[test]
    fn validate_accepts_empty_wall_type() {
        let mut m = ArchModel::default();
        m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, wall_type: String::new(), ..Wall::default() });
        let msgs = m.validate();
        assert!(msgs.iter().all(|m| !m.contains("unknown wall type")), "empty wall_type wrongly rejected: {msgs:?}");
    }

    #[test]
    fn validate_rejects_opening_width_below_minimum() {
        let mut m = ArchModel::default();
        let wid = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        m.push_opening(Opening { host: wid, width: 0.05, ..Opening::default() });
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("below minimum")), "opening width below minimum not reported: {msgs:?}");
    }

    #[test]
    fn validate_reports_multiple_errors_on_same_wall() {
        let mut m = ArchModel::default();
        // Wall with zero thickness, zero length, unknown type, unknown level
        m.push_wall(Wall {
            start: Vec2::new(5.0, 5.0),
            end: Vec2::new(5.0, 5.0),
            thickness: 0.0,
            height: 2.4,
            base_level: 99,
            exterior: true,
            wall_type: "BogusType".into(),
            ..Wall::default()
        });
        let msgs = m.validate();
        // Should report: thickness below minimum, zero length, unknown wall type, unknown level
        assert!(msgs.iter().any(|m| m.contains("below minimum")), "thickness error missing: {msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("zero or near-zero length")), "length error missing: {msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("unknown wall type")), "wall type error missing: {msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("unknown level")), "level error missing: {msgs:?}");
    }

    #[test]
    fn validate_accepts_the_builtin_wall_type_names() {
        for name in ["exterior", "INTERIOR", "kNeE", "Railing", "RETAINING", "Exterior", "Interior"] {
            let mut m = ArchModel::default();
            m.push_wall(Wall {
                start: Vec2::new(0.0, 0.0),
                end: Vec2::new(10.0, 0.0),
                thickness: 0.2,
                wall_type: name.to_string(),
                ..Wall::default()
            });
            assert!(!m.validate().iter().any(|m| m.contains("unknown wall type")), "builtin wall type `{name}` was reported as unknown");
        }

        // An unknown name is still reported.
        let mut m = ArchModel::default();
        m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, wall_type: "bogus".into(), ..Wall::default() });
        assert!(m.validate().iter().any(|m| m.contains("unknown wall type")), "unknown wall type `bogus` was not reported");
    }

    #[test]
    fn handle_just_below_base_is_not_arch() {
        assert!(!is_arch_handle(Handle(ARCH_HANDLE_BASE - 1)));
        assert!(is_arch_handle(Handle(ARCH_HANDLE_BASE)));
    }

    #[test]
    fn arch_handle_at_max_id_is_within_range() {
        let h = arch_handle(ArchId(u32::MAX));
        assert!(is_arch_handle(h));
        assert!(!is_opening_handle(h));
        assert!(!is_room_handle(h));
        assert!(!is_roof_handle(h));
    }

    #[test]
    fn opening_handle_at_max_id_is_within_range() {
        let h = opening_handle(ArchId(32767));
        assert!(is_opening_handle(h));
        assert!(is_arch_handle(h));
        assert!(!is_room_handle(h));
        assert!(!is_roof_handle(h));
    }

    #[test]
    fn room_handle_at_max_id_is_within_range() {
        let h = room_handle(ArchId(32767));
        assert!(is_room_handle(h));
        assert!(is_arch_handle(h));
        assert!(!is_opening_handle(h));
        assert!(!is_roof_handle(h));
    }

    #[test]
    fn roof_handle_at_max_id_is_within_range() {
        let h = roof_handle(ArchId(32767));
        assert!(is_roof_handle(h));
        assert!(is_arch_handle(h));
        assert!(!is_opening_handle(h));
        assert!(!is_room_handle(h));
    }

    #[test]
    fn all_proxy_handle_ranges_are_disjoint() {
        let wall = arch_handle(ArchId(100));
        let opening = opening_handle(ArchId(100));
        let room = room_handle(ArchId(100));
        let roof = roof_handle(ArchId(100));
        assert_ne!(wall, opening);
        assert_ne!(wall, room);
        assert_ne!(wall, roof);
        assert_ne!(opening, room);
        assert_ne!(opening, roof);
        assert_ne!(room, roof);
    }

    #[test]
    fn room_json_roundtrips() {
        let mut r = Room::default();
        r.id = ArchId(7);
        r.name = "Kitchen".into();
        r.room_type = RoomType::Kitchen;
        r.level = 1;
        r.polygon = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)];
        r.holes = vec![vec![Vec2::new(1.0, 1.0), Vec2::new(2.0, 1.0), Vec2::new(2.0, 2.0), Vec2::new(1.0, 2.0)]];
        r.ceiling_height = Some(2.6);
        r.floor_height = Some(0.0);
        r.interior_area = 12.0;
        r.standard_area = 13.5;
        let json = serde_json::to_string(&r).unwrap();
        let back: Room = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, ArchId(7));
        assert_eq!(back.name, "Kitchen");
        assert_eq!(back.room_type, RoomType::Kitchen);
        assert_eq!(back.level, 1);
        assert_eq!(back.polygon.len(), 4);
        assert_eq!(back.holes.len(), 1);
        assert_eq!(back.holes[0].len(), 4);
        assert_eq!(back.ceiling_height, Some(2.6));
        assert_eq!(back.floor_height, Some(0.0));
        assert!((back.interior_area - 12.0).abs() < 1e-12);
        assert!((back.standard_area - 13.5).abs() < 1e-12);
    }

    #[test]
    fn validate_rejects_negative_and_nan_room_areas() {
        let mut m = ArchModel::default();
        // Negative interior area
        m.push_room(Room { polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0)], interior_area: -1.0, ..Room::default() });
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("invalid interior area")), "negative interior area not reported: {msgs:?}");

        // NaN standard area
        let mut m2 = ArchModel::default();
        m2.push_room(Room {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0)],
            standard_area: f64::NAN,
            ..Room::default()
        });
        let msgs2 = m2.validate();
        assert!(msgs2.iter().any(|m| m.contains("invalid standard area")), "NaN standard area not reported: {msgs2:?}");
    }

    #[test]
    fn validate_rejects_room_on_unknown_level() {
        let mut m = ArchModel::default();
        m.push_room(Room { polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0)], level: 99, ..Room::default() });
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("unknown level")), "room on unknown level not reported: {msgs:?}");
    }

    #[test]
    fn validate_flags_duplicate_room_ids() {
        let mut m = ArchModel::default();
        m.push_room(Room::default());
        m.push_room(Room::default());
        m.rooms[1].id = m.rooms[0].id;
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("duplicate room id")), "duplicate room id not reported: {msgs:?}");
    }

    #[test]
    fn validate_rejects_too_many_rooms() {
        let mut m = ArchModel::default();
        for _ in 0..MAX_ROOMS + 1 {
            m.push_room(Room::default());
        }
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("max") && m.contains("rooms")), "too many rooms not reported: {msgs:?}");
    }

    #[test]
    fn level_floor_to_floor_rejects_negative_components() {
        let l = Level { ceiling_height: 2.4, structure: -0.1, ..Level::default() };
        assert_eq!(l.floor_to_floor(), 0.0, "negative structure should give 0.0");
        let l2 = Level { ceiling_height: -2.4, structure: 0.3, ..Level::default() };
        assert_eq!(l2.floor_to_floor(), 0.0, "negative ceiling height should give 0.0");
        let l3 = Level { ceiling_height: 0.0, structure: 0.0, ..Level::default() };
        assert_eq!(l3.floor_to_floor(), 0.0, "zero components should give 0.0");
    }

    #[test]
    fn camera_defaults_are_sane() {
        let c = Camera3d::default();
        assert!(c.name.is_empty());
        assert_eq!(c.kind, CameraKind::Orbit);
        assert_eq!(c.target, Some([0.0; 3]));
        assert_eq!(c.azimuth_deg, Some(0.0));
        assert_eq!(c.elevation_deg, Some(30.0));
        assert_eq!(c.distance, Some(10.0));
        assert_eq!(c.eye, Some([0.0, -10.0]));
        assert_eq!(c.eye_height, Some(1.6));
        assert_eq!(c.fov_deg, Some(45.0));
    }

    #[test]
    fn set_camera_refuses_empty_duplicate_and_non_finite() {
        let mut m = ArchModel::default();

        // Empty or whitespace-only names are refused.
        assert_eq!(m.set_camera(Camera3d { name: String::new(), ..Camera3d::default() }), Err(CameraError::EmptyName));
        assert_eq!(m.set_camera(Camera3d { name: "   ".into(), ..Camera3d::default() }), Err(CameraError::EmptyName));
        assert_eq!(m.set_camera(Camera3d { name: "\t\n ".into(), ..Camera3d::default() }), Err(CameraError::EmptyName));
        assert_eq!(m.cameras.len(), 0);

        // A valid camera is accepted.
        assert_eq!(m.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }), Ok(()));
        assert_eq!(m.cameras.len(), 1);
        assert!(m.camera_by_name("front").is_some());

        // A duplicate name is refused.
        let dup = Camera3d { name: "front".into(), ..Camera3d::default() };
        assert_eq!(m.set_camera(dup.clone()), Err(CameraError::Duplicate));
        assert_eq!(m.cameras.len(), 1);

        // A non-finite field is refused (never clamped).
        assert_eq!(m.set_camera(Camera3d { name: "bad".into(), distance: Some(f64::NAN), ..Camera3d::default() }), Err(CameraError::NotFinite));
        assert_eq!(m.set_camera(Camera3d { name: "bad2".into(), target: Some([f64::NAN; 3]), ..Camera3d::default() }), Err(CameraError::NotFinite));
        assert_eq!(
            m.set_camera(Camera3d { name: "bad3".into(), eye: Some([0.0, f64::INFINITY]), ..Camera3d::default() }),
            Err(CameraError::NotFinite)
        );
        assert_eq!(m.cameras.len(), 1);
    }

    #[test]
    fn cameras_are_capped_at_max_cameras() {
        let mut m = ArchModel::default();
        for i in 0..MAX_CAMERAS {
            assert_eq!(m.set_camera(Camera3d { name: format!("c{}", i), ..Camera3d::default() }), Ok(()));
        }
        // At the cap: adding one more is refused, and the length is unchanged.
        assert_eq!(m.set_camera(Camera3d { name: "extra".into(), ..Camera3d::default() }), Err(CameraError::TooMany));
        assert_eq!(m.cameras.len(), MAX_CAMERAS);
    }

    #[test]
    fn remove_camera_clears_the_active_pointer() {
        let mut m = ArchModel::default();
        assert_eq!(m.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }), Ok(()));
        assert_eq!(m.set_camera(Camera3d { name: "back".into(), ..Camera3d::default() }), Ok(()));
        assert_eq!(m.cameras.len(), 2);

        // Pointing at the active camera that is not removed stays put.
        m.active_camera = Some("front".into());
        assert_eq!(m.remove_camera("back"), true);
        assert_eq!(m.cameras.len(), 1);
        assert_eq!(m.active_camera, Some("front".into()));
        assert_eq!(m.camera_by_name("front"), Some(&m.cameras[0]));

        // Removing the active camera clears the pointer.
        assert_eq!(m.remove_camera("front"), true);
        assert_eq!(m.cameras.len(), 0);
        assert_eq!(m.active_camera, None);

        // Removing a camera that is not there does nothing.
        assert_eq!(m.remove_camera("ghost"), false);
        assert_eq!(m.cameras.len(), 0);
    }

    #[test]
    fn arch_model_default_has_no_cameras() {
        let m = ArchModel::default();
        assert_eq!(m.cameras.len(), 0);
        assert_eq!(m.active_camera, None);
    }

    #[test]
    fn camera_json_roundtrips() {
        let c = Camera3d {
            name: "front".into(),
            kind: CameraKind::Orbit,
            target: Some([1.0, 2.0, 3.0]),
            azimuth_deg: Some(45.0),
            elevation_deg: Some(30.0),
            distance: Some(20.0),
            eye: Some([5.0, -15.0]),
            eye_height: Some(1.7),
            fov_deg: Some(50.0),
            ..Camera3d::default()
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Camera3d = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.name, "front");
        assert_eq!(back.kind, CameraKind::Orbit);
        assert_eq!(back.target, Some([1.0, 2.0, 3.0]));
        assert_eq!(back.azimuth_deg, Some(45.0));
        assert_eq!(back.elevation_deg, Some(30.0));
        assert_eq!(back.distance, Some(20.0));
        assert_eq!(back.eye, Some([5.0, -15.0]));
        assert_eq!(back.eye_height, Some(1.7));
        assert_eq!(back.fov_deg, Some(50.0));
    }

    #[test]
    fn camera_kind_serializes_as_camel_case() {
        let orbit = CameraKind::Orbit;
        let full = CameraKind::Full;
        assert_eq!(serde_json::to_string(&orbit).unwrap(), "\"Orbit\"");
        assert_eq!(serde_json::to_string(&full).unwrap(), "\"Full\"");
        let back_orbit: CameraKind = serde_json::from_str("\"Orbit\"").unwrap();
        let back_full: CameraKind = serde_json::from_str("\"Full\"").unwrap();
        assert_eq!(back_orbit, CameraKind::Orbit);
        assert_eq!(back_full, CameraKind::Full);
    }

    #[test]
    fn overview_camera_and_saved_state_serde() {
        let saved = Camera3d { name: "ov".into(), ..Camera3d::default() };
        let cam = Camera3d {
            name: "ov".into(),
            kind: CameraKind::Overview,
            level: Some(1),
            lower_levels: Some(false),
            saved: Some(Box::new(saved)),
            ..Camera3d::default()
        };
        let json = serde_json::to_string(&cam).unwrap();
        assert!(json.contains("\"Overview\""));
        let back: Camera3d = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cam);
        let lower: CameraKind = serde_json::from_str("\"overview\"").unwrap();
        assert_eq!(lower, CameraKind::Overview);
        // Older drawings (no new keys) open unchanged and omit the keys again.
        let plain = Camera3d { name: "p".into(), ..Camera3d::default() };
        let pj = serde_json::to_string(&plain).unwrap();
        assert!(!pj.contains("saved") && !pj.contains("level") && !pj.contains("lower_levels"));
        let old: Camera3d = serde_json::from_str("{\"name\":\"o\"}").unwrap();
        assert!(old.saved.is_none() && old.level.is_none() && old.lower_levels.is_none());
        // A non-finite saved state is refused.
        let mut m = ArchModel::default();
        let mut bad = cam.clone();
        if let Some(s) = bad.saved.as_mut() {
            s.distance = Some(f64::NAN);
        }
        assert_eq!(m.set_camera(bad), Err(CameraError::NotFinite));
        assert_eq!(m.set_camera(cam), Ok(()));
    }

    #[test]
    fn duplicate_camera_names_are_case_sensitive() {
        let mut m = ArchModel::default();
        assert_eq!(m.set_camera(Camera3d { name: "Front".into(), ..Camera3d::default() }), Ok(()));
        // "front" is different from "Front"
        assert_eq!(m.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }), Ok(()));
        assert_eq!(m.cameras.len(), 2);
        // But exact duplicate is refused
        assert_eq!(m.set_camera(Camera3d { name: "Front".into(), ..Camera3d::default() }), Err(CameraError::Duplicate));
        assert_eq!(m.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }), Err(CameraError::Duplicate));
    }

    #[test]
    fn camera_index_returns_none_for_missing_camera() {
        let mut m = ArchModel::default();
        assert_eq!(m.camera_index("nonexistent"), None);
        assert_eq!(m.camera_by_name("nonexistent"), None);

        assert_eq!(m.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }), Ok(()));
        assert_eq!(m.camera_index("front"), Some(0));
        assert_eq!(m.camera_index("other"), None);
    }

    #[test]
    fn remove_camera_with_empty_name_returns_false() {
        let mut m = ArchModel::default();
        assert_eq!(m.set_camera(Camera3d { name: "front".into(), ..Camera3d::default() }), Ok(()));
        // Empty name is not a valid camera name, so removal should fail
        assert_eq!(m.remove_camera(""), false);
        assert_eq!(m.cameras.len(), 1);
    }

    #[test]
    fn validate_reports_camera_issues() {
        // Empty name
        let mut m = ArchModel::default();
        m.cameras.push(Camera3d { name: String::new(), ..Camera3d::default() });
        let msgs = m.validate();
        assert!(msgs.iter().any(|m| m.contains("empty name")), "empty camera name not reported: {msgs:?}");

        // Non-finite field
        let mut m2 = ArchModel::default();
        m2.cameras.push(Camera3d { name: "bad".into(), distance: Some(f64::NAN), ..Camera3d::default() });
        let msgs2 = m2.validate();
        assert!(msgs2.iter().any(|m| m.contains("non-finite")), "non-finite camera field not reported: {msgs2:?}");

        // Duplicate names
        let mut m3 = ArchModel::default();
        m3.cameras.push(Camera3d { name: "dup".into(), ..Camera3d::default() });
        m3.cameras.push(Camera3d { name: "dup".into(), ..Camera3d::default() });
        let msgs3 = m3.validate();
        assert!(msgs3.iter().any(|m| m.contains("duplicate camera name")), "duplicate camera name not reported: {msgs3:?}");

        // Too many cameras
        let mut m4 = ArchModel::default();
        for i in 0..MAX_CAMERAS + 1 {
            m4.cameras.push(Camera3d { name: format!("c{}", i), ..Camera3d::default() });
        }
        let msgs4 = m4.validate();
        assert!(msgs4.iter().any(|m| m.contains("cameras") && m.contains("max")), "too many cameras not reported: {msgs4:?}");
    }

    #[test]
    fn arch_model_with_cameras_json_roundtrips() {
        let mut m = ArchModel::default();
        m.set_camera(Camera3d { name: "front".into(), distance: Some(15.0), ..Camera3d::default() }).unwrap();
        m.set_camera(Camera3d { name: "top".into(), kind: CameraKind::Full, eye: Some([0.0, 0.0]), eye_height: Some(20.0), ..Camera3d::default() })
            .unwrap();
        m.active_camera = Some("front".into());

        let json = serde_json::to_string(&m).unwrap();
        let back: ArchModel = serde_json::from_str(&json).unwrap();

        assert_eq!(back.cameras.len(), 2);
        assert_eq!(back.cameras[0].name, "front");
        assert_eq!(back.cameras[0].distance, Some(15.0));
        assert_eq!(back.cameras[1].name, "top");
        assert_eq!(back.cameras[1].kind, CameraKind::Full);
        assert_eq!(back.cameras[1].eye_height, Some(20.0));
        assert_eq!(back.active_camera, Some("front".into()));
    }

    #[test]
    fn empty_cameras_skipped_in_serialization() {
        let m = ArchModel::default();
        let json = serde_json::to_string(&m).unwrap();
        // Empty cameras vec should be skipped
        assert!(!json.contains("\"cameras\""), "empty cameras array should be skipped: {json}");
        // None active_camera should be skipped
        assert!(!json.contains("\"active_camera\""), "None active_camera should be skipped: {json}");
    }

    #[test]
    fn camera_with_infinity_is_rejected() {
        let mut m = ArchModel::default();
        assert_eq!(
            m.set_camera(Camera3d { name: "inf".into(), azimuth_deg: Some(f64::INFINITY), ..Camera3d::default() }),
            Err(CameraError::NotFinite)
        );
        assert_eq!(
            m.set_camera(Camera3d { name: "inf2".into(), elevation_deg: Some(f64::NEG_INFINITY), ..Camera3d::default() }),
            Err(CameraError::NotFinite)
        );
        assert_eq!(m.set_camera(Camera3d { name: "inf3".into(), fov_deg: Some(f64::INFINITY), ..Camera3d::default() }), Err(CameraError::NotFinite));
        assert_eq!(m.cameras.len(), 0);
    }

    #[test]
    fn a_drawing_without_electrical_serializes_as_before() {
        let d = Drawing::new_metric();
        let json = serde_json::to_string(&d.arch).unwrap();
        assert!(!json.contains("\"electrical\""));
        assert!(!json.contains("\"connections\""));
        assert!(!json.contains("\"light_sets\""));
    }

    #[test]
    fn wall_spanning_three_levels_spans_all() {
        let levels = vec![
            Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Level::default() },
            Level { index: 1, elevation: 3.0, ceiling_height: 2.8, ..Level::default() },
            Level { index: 2, elevation: 6.0, ceiling_height: 2.5, ..Level::default() },
        ];
        // Wall from level 0 to level 2
        let wall = Wall { base_level: 0, top_level: Some(2), height: 2.0, ..Wall::default() };
        assert!(wall.spans_level(&levels[0], &levels), "should span base level");
        assert!(wall.spans_level(&levels[1], &levels), "should span intermediate level");
        assert!(wall.spans_level(&levels[2], &levels), "should span top level");
        // top_z should be level 2's ceiling
        assert!((wall.top_z(&levels) - 8.5).abs() < 1e-12, "top_z should be 6.0 + 2.5 = 8.5");
    }

    #[test]
    fn wall_with_missing_top_level_falls_back_to_height() {
        let levels = vec![Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Level::default() }];
        // Wall says top_level is 5, but only level 0 exists
        let wall = Wall { base_level: 0, top_level: Some(5), height: 3.0, ..Wall::default() };
        // Should fall back to bottom_z + height = 0.0 + 3.0 = 3.0
        assert!((wall.top_z(&levels) - 3.0).abs() < 1e-12, "should fall back to height when top_level missing");
        // Should not span a non-existent level 5
        let phantom = Level { index: 5, elevation: 15.0, ceiling_height: 2.4, ..Level::default() };
        assert!(!wall.spans_level(&phantom, &levels), "should not span missing top level");
    }

    #[test]
    fn wall_with_missing_base_level_returns_zero() {
        let levels = vec![Level { index: 1, elevation: 3.0, ceiling_height: 2.4, ..Level::default() }];
        // Wall's base_level is 0, but only level 1 exists
        let wall = Wall { base_level: 0, height: 2.4, ..Wall::default() };
        assert!((wall.bottom_z(&levels) - 0.0).abs() < 1e-12, "bottom_z should be 0 when base_level missing");
        assert!((wall.top_z(&levels) - 2.4).abs() < 1e-12, "top_z should be height when base_level missing");
    }

    #[test]
    fn level_by_index_returns_none_for_missing() {
        let levels = vec![Level { index: 0, elevation: 0.0, ..Level::default() }, Level { index: 2, elevation: 6.0, ..Level::default() }];
        assert!(level_by_index(&levels, 0).is_some());
        assert!(level_by_index(&levels, 1).is_none(), "should return None for missing index");
        assert!(level_by_index(&levels, 2).is_some());
        assert!(level_by_index(&levels, 99).is_none());
    }

    #[test]
    fn level_by_index_returns_first_for_duplicates() {
        let levels = vec![
            Level { index: 0, elevation: 0.0, name: "first".into(), ..Level::default() },
            Level { index: 0, elevation: 1.0, name: "second".into(), ..Level::default() },
        ];
        let found = level_by_index(&levels, 0).expect("should find index 0");
        assert_eq!(found.name, "first", "should return the first level with the given index");
        assert!((found.elevation - 0.0).abs() < 1e-12);
    }

    #[test]
    fn ensure_ground_level_does_not_duplicate() {
        let mut m = ArchModel::default();
        // Default model already has a level at index 0
        assert_eq!(m.levels.len(), 1);
        assert_eq!(m.levels[0].index, 0);
        // Calling ensure_ground_level again should not add another
        m.ensure_ground_level();
        assert_eq!(m.levels.len(), 1, "should not add a duplicate level");
        // But if we clear levels, it should add one back
        m.levels.clear();
        m.ensure_ground_level();
        assert_eq!(m.levels.len(), 1);
        assert_eq!(m.levels[0].index, 0);
    }

    #[test]
    fn wall_spans_level_at_exact_boundary() {
        let levels = vec![
            Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Level::default() },
            Level { index: 1, elevation: 2.7, ceiling_height: 2.4, ..Level::default() },
        ];
        // Wall from level 0 to level 1: top_z = 2.7 + 2.4 = 5.1
        let wall = Wall { base_level: 0, top_level: Some(1), height: 2.0, ..Wall::default() };
        let top = wall.top_z(&levels);
        assert!((top - 5.1).abs() < 1e-12);
        // A level at exactly top - 0.01 should NOT span (the interval is half-open: [bottom, top - 0.01))
        let at_boundary = Level { index: 99, elevation: top - 0.01, ceiling_height: 2.4, ..Level::default() };
        assert!(!wall.spans_level(&at_boundary, &levels), "level at top-0.01 should not span");
        // A level at exactly top should not span
        let at_top = Level { index: 98, elevation: top, ceiling_height: 2.4, ..Level::default() };
        assert!(!wall.spans_level(&at_top, &levels), "level at top should not span");
        // A level just below top - 0.01 should span
        let just_below = Level { index: 97, elevation: top - 0.02, ceiling_height: 2.4, ..Level::default() };
        assert!(wall.spans_level(&just_below, &levels), "level just below top-0.01 should span");
    }

    #[test]
    fn level_kind_serializes_as_camel_case() {
        let floor = LevelKind::Floor;
        let landing = LevelKind::Landing;
        assert_eq!(serde_json::to_string(&floor).unwrap(), "\"floor\"");
        assert_eq!(serde_json::to_string(&landing).unwrap(), "\"landing\"");
        let back_floor: LevelKind = serde_json::from_str("\"floor\"").unwrap();
        let back_landing: LevelKind = serde_json::from_str("\"landing\"").unwrap();
        assert_eq!(back_floor, LevelKind::Floor);
        assert_eq!(back_landing, LevelKind::Landing);
    }

    /// A room saved before phase 16 (no `flat_ceiling`, no `ceiling_stack`) loads with a flat
    /// ceiling and an empty finish stack.
    #[test]
    fn room_without_ceiling_fields_loads_flat_and_unstacked() {
        let v = serde_json::json!({"id": 3, "name": "Hall", "polygon": [{"x": 0.0, "y": 0.0}, {"x": 1.0, "y": 0.0}, {"x": 1.0, "y": 1.0}], "ceiling_height": 2.7});
        let r: Room = serde_json::from_value(v).unwrap();
        assert!(r.flat_ceiling);
        assert!(r.ceiling_stack.is_empty());
        assert_eq!(r.ceiling_height, Some(2.7));
        assert!(Room::default().flat_ceiling, "Room::default is flat too");
        let stacked = Room {
            flat_ceiling: false,
            ceiling_stack: vec![
                CeilingLayer { name: "drywall".into(), material: Some("drywall".into()), thickness: 0.0127 },
                CeilingLayer { name: "air".into(), material: None, thickness: 0.2 },
            ],
            ..Room::default()
        };
        let v = serde_json::to_value(&stacked).unwrap();
        assert_eq!(v["flat_ceiling"], false);
        assert_eq!(v["ceiling_stack"][1]["material"], serde_json::Value::Null);
        assert_eq!(v["ceiling_stack"][0]["thickness"], 0.0127);
        let back: Room = serde_json::from_value(v).unwrap();
        assert_eq!(back, stacked);
        // A layer with missing keys takes the defaults.
        let l: CeilingLayer = serde_json::from_value(serde_json::json!({"thickness": 0.1})).unwrap();
        assert_eq!(l, CeilingLayer { thickness: 0.1, ..CeilingLayer::default() });
        // The constructor other crates use.
        assert_eq!(CeilingLayer::new("air", None, 0.2), CeilingLayer { name: "air".into(), material: None, thickness: 0.2 });
        assert_eq!(CeilingLayer::new("drywall", Some("drywall"), 0.0127).material.as_deref(), Some("drywall"));
    }

    /// A default room writes neither key, so an old drawing saves the same payload.
    #[test]
    fn default_room_writes_no_ceiling_keys() {
        let v = serde_json::to_value(Room::default()).unwrap();
        let o = v.as_object().unwrap();
        assert!(!o.contains_key("flat_ceiling"), "{v}");
        assert!(!o.contains_key("ceiling_stack"), "{v}");
        let old = serde_json::json!({"id": 1, "name": "A", "polygon": [{"x": 0.0, "y": 0.0}, {"x": 2.0, "y": 0.0}, {"x": 2.0, "y": 2.0}]});
        let r: Room = serde_json::from_value(old).unwrap();
        let again = serde_json::to_value(&r).unwrap();
        let back: Room = serde_json::from_value(again.clone()).unwrap();
        assert_eq!(back, r);
        assert!(!again.as_object().unwrap().contains_key("flat_ceiling"));
    }

    // ---------- phase 16: trays, soffits and foundations ----------

    fn p16_model() -> ArchModel {
        let mut m = ArchModel::default();
        let square = vec![Vec2::new(1.0, 1.0), Vec2::new(2.0, 1.0), Vec2::new(2.0, 2.0), Vec2::new(1.0, 2.0)];
        m.push_tray(Tray { room: ArchId(2), inset: 0.6, sides: TraySides::Pitched(30.0), ..Default::default() });
        m.push_tray(Tray { room: ArchId(2), parent: Some(ArchId(0)), polygon: square.clone(), depth: 0.15, ..Default::default() });
        m.push_soffit(Soffit {
            level: 1,
            polygon: square,
            bottom: Some(2.1),
            over: vec![ArchId(4)],
            material: Some("finish:oak".into()),
            ..Default::default()
        });
        m.push_foundation(Foundation { kind: FoundationKind::Slab, footing_depth: 0.9, ..Default::default() });
        m
    }

    #[test]
    fn tray_soffit_foundation_round_trip() {
        let m = p16_model();
        assert_eq!((m.trays.len(), m.next_tray_id, m.soffits.len(), m.next_soffit_id, m.foundations.len(), m.next_foundation_id), (2, 2, 1, 1, 1, 1));
        assert_eq!(m.tray(ArchId(1)).map(|t| t.parent), Some(Some(ArchId(0))));
        assert_eq!(m.soffit(ArchId(0)).and_then(|s| s.bottom), Some(2.1));
        assert_eq!(m.foundation(ArchId(0)).map(|f| f.kind), Some(FoundationKind::Slab));
        assert!(m.tray(ArchId(7)).is_none() && m.soffit(ArchId(1)).is_none() && m.foundation(ArchId(1)).is_none());
        assert!(!m.is_empty());
        let json = serde_json::to_string(&m).unwrap();
        let back: ArchModel = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
        // Each alone keeps the model non-empty.
        for strip in 0..3 {
            let mut one = ArchModel { walls: Vec::new(), levels: Vec::new(), types: Vec::new(), ..p16_model() };
            match strip {
                0 => (one.soffits.clear(), one.foundations.clear()),
                1 => (one.trays.clear(), one.foundations.clear()),
                _ => (one.trays.clear(), one.soffits.clear()),
            };
            assert!(!one.is_empty(), "strip {strip}");
        }
    }

    #[test]
    fn model_without_them_writes_no_new_keys() {
        let json = serde_json::to_value(ArchModel::default()).unwrap();
        for key in ["trays", "next_tray_id", "soffits", "next_soffit_id", "foundations", "next_foundation_id"] {
            assert!(json.get(key).is_none(), "{key} written for a model without it");
        }
        // A model saved before phase 16 loads with none of them.
        let old = r#"{"walls":[],"openings":[],"rooms":[]}"#;
        let m: ArchModel = serde_json::from_str(old).unwrap();
        assert!(m.trays.is_empty() && m.soffits.is_empty() && m.foundations.is_empty());
        assert_eq!((m.next_tray_id, m.next_soffit_id, m.next_foundation_id), (0, 0, 0));
        let mut empty = ArchModel::default();
        empty.walls.clear();
        assert!(empty.is_empty());
    }

    #[test]
    fn new_handle_ranges_are_disjoint_from_every_other_kind() {
        type Pred = fn(Handle) -> bool;
        let preds: [(&str, Pred); 16] = [
            ("opening", is_opening_handle),
            ("room", is_room_handle),
            ("roof", is_roof_handle),
            ("stair", is_stair_handle),
            ("railing", is_railing_handle),
            ("view", is_view_handle),
            ("terrain", is_terrain_handle),
            ("plant", is_plant_handle),
            ("light", is_light_handle),
            ("cabinet", is_cabinet_handle),
            ("fixture", is_fixture_handle),
            ("furnishing", is_furnishing_handle),
            ("electrical", is_electrical_handle),
            ("connection", is_connection_handle),
            ("tray", is_tray_handle),
            ("soffit", is_soffit_handle),
        ];
        let preds: Vec<(&str, Pred)> = preds.into_iter().chain([("foundation", is_foundation_handle as Pred)]).collect();
        let makers: [(&str, fn(ArchId) -> Handle); 3] = [("tray", tray_handle), ("soffit", soffit_handle), ("foundation", foundation_handle)];
        for (name, make) in makers {
            for id in [0u32, 1, 4095, 32767] {
                let h = make(ArchId(id));
                let claims: Vec<&str> = preds.iter().filter(|(_, p)| p(h)).map(|(n, _)| *n).collect();
                assert_eq!(claims, vec![name], "{name} {id}");
                assert!(is_arch_handle(h) && is_reserved_file_handle(h), "{name} {id} outside the arch span");
                // Not a wall either: walls sit below the opening offset.
                assert!(h.0 - ARCH_HANDLE_BASE >= OPENING_HANDLE_OFFSET);
            }
        }
        // Every other kind's handle is claimed by none of the three new ranges.
        let others = [
            arch_handle(ArchId(5)),
            opening_handle(ArchId(5)),
            room_handle(ArchId(5)),
            roof_handle(ArchId(5)),
            stair_handle(ArchId(5)),
            railing_handle(ArchId(5)),
            view_handle(ArchId(5)),
            terrain_handle(Some(5)),
            plant_handle(ArchId(5)),
            light_handle(ArchId(5)),
            cabinet_handle(ArchId(5)),
            fixture_handle(ArchId(5)),
            furnishing_handle(ArchId(5)),
            electrical_handle(ArchId(32767)),
            connection_handle(ArchId(32767)),
        ];
        for h in others {
            assert!(!is_tray_handle(h) && !is_soffit_handle(h) && !is_foundation_handle(h), "{h:?}");
        }
    }

    #[test]
    fn arch_kind_tray_soffit_foundation_keys_labels_handles() {
        let id = ArchId(3);
        assert_eq!((ArchKind::Tray.key(), ArchKind::Soffit.key(), ArchKind::Foundation.key()), ("tray", "soffit", "foundation"));
        assert_eq!(
            (ArchKind::Tray.label(id), ArchKind::Soffit.label(id), ArchKind::Foundation.label(id)),
            ("tr3".into(), "sf3".into(), "fd3".into())
        );
        assert_eq!(ArchKind::Tray.handle(id), tray_handle(id));
        assert_eq!(ArchKind::Soffit.handle(id), soffit_handle(id));
        assert_eq!(ArchKind::Foundation.handle(id), foundation_handle(id));
        // The new kinds sort after every older one.
        assert!(ArchKind::View < ArchKind::Tray && ArchKind::Tray < ArchKind::Soffit && ArchKind::Soffit < ArchKind::Foundation);
    }

    #[test]
    fn from_part_maps_new_parts() {
        assert_eq!(ArchKind::from_part("soffit"), ArchKind::Soffit);
        assert_eq!(ArchKind::from_part("foundation"), ArchKind::Foundation);
        assert_eq!(ArchKind::from_part("footing"), ArchKind::Foundation);
        assert_eq!(ArchKind::from_part("slab"), ArchKind::Room);
        // The roof's own soffit is part "roof".
        assert_eq!(ArchKind::from_part("roof"), ArchKind::Roof);
        assert_eq!(part_handle(ArchId(2), "soffit"), soffit_handle(ArchId(2)));
        assert_eq!(part_handle(ArchId(2), "footing"), foundation_handle(ArchId(2)));
        assert_eq!(part_handle(ArchId(2), "foundation"), foundation_handle(ArchId(2)));
        assert_eq!(part_handle(ArchId(2), "slab"), room_handle(ArchId(2)));
    }

    #[test]
    fn rail_setting_names_round_trip() {
        for s in [RailSetting::Auto, RailSetting::On, RailSetting::Off] {
            assert_eq!(RailSetting::parse(s.name()), Some(s));
            assert_eq!(RailSetting::parse(&format!(" {} ", s.name().to_uppercase())), Some(s));
        }
        assert!(RailSetting::Auto.is_auto() && !RailSetting::On.is_auto());
        assert_eq!(RailSetting::parse("maybe"), None);
    }

    #[test]
    fn stringers_parse_accepts_center_and_centre() {
        assert_eq!(Stringers::parse("center"), Some(Stringers::Centre));
        assert_eq!(Stringers::parse("Centre"), Some(Stringers::Centre));
        assert_eq!(Stringers::parse("none"), Some(Stringers::None));
        assert_eq!(Stringers::parse("two"), Some(Stringers::Two));
        assert!(Stringers::Two.is_two() && !Stringers::None.is_two());
        assert_eq!(Stringers::Centre.name(), "centre");
    }

    #[test]
    fn railing_host_names() {
        assert_eq!(RailingHost::Well.name(), "well");
        assert_eq!(RailingHost::StairLeft.name(), "stairLeft");
        assert_eq!(RailingHost::StairRight.name(), "stairRight");
        assert_eq!(RailingHost::Deck.name(), "deck");
        assert!(RailingHost::default().is_well());
        assert_eq!(serde_json::to_string(&RailingHost::StairLeft).unwrap(), "\"stairLeft\"");
    }

    #[test]
    fn opening_top_names() {
        for t in [OpeningTop::Square, OpeningTop::Round, OpeningTop::Segment, OpeningTop::Roof] {
            assert_eq!(OpeningTop::parse(t.name()), Some(t));
        }
        assert!(OpeningTop::Square.is_square() && !OpeningTop::Roof.is_square());
        assert_eq!(OpeningTop::parse("gothic"), None);
    }

    #[test]
    fn opening_sides_names_both_left_right_none() {
        assert_eq!(OpeningSides::BOTH.name(), "both");
        assert_eq!(OpeningSides::RIGHT.name(), "right");
        assert_eq!(OpeningSides::NONE.name(), "none");
        let left = OpeningSides { left: true, ..OpeningSides::NONE };
        assert_eq!(left.name(), "left");
        for n in ["left", "right", "both", "none"] {
            assert_eq!(OpeningSides::parse(n).map(|s| s.name()), Some(n));
        }
        assert_eq!(OpeningSides::default(), OpeningSides::RIGHT);
        assert!(OpeningSides::RIGHT.is_right_only() && !OpeningSides::BOTH.is_right_only());
        assert_eq!(OpeningSides::parse("up"), None);
    }

    #[test]
    fn grille_default_is_one_by_one() {
        let g = Grille::default();
        assert_eq!((g.across, g.down, g.top_only), (1, 1, false));
        let back: Grille = serde_json::from_str(r#"{"across":3}"#).unwrap();
        assert_eq!((back.across, back.down), (3, 1));
        assert_eq!(MAX_GRILLE_LITES, 12);
    }

    #[test]
    fn stair_without_phase17_keys_loads_with_the_defaults() {
        let s: Stair = serde_json::from_str(r#"{"width":1.2}"#).unwrap();
        assert_eq!(s.width, 1.2);
        assert!(!s.open_risers);
        assert_eq!(s.stringers, Stringers::Two);
        assert_eq!(s.underside, 0.3);
        assert_eq!((s.rail_left, s.rail_right), (RailSetting::Auto, RailSetting::Auto));
        assert_eq!(s.plan_cut, 1.2);
        let v = serde_json::to_value(Stair::default()).unwrap();
        for k in ["open_risers", "stringers", "underside", "rail_left", "rail_right", "plan_cut"] {
            assert!(v.get(k).is_none(), "{k} written");
        }
        let t = Stair { stringers: Stringers::Centre, plan_cut: 1.5, ..Stair::default() };
        let back: Stair = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn railing_saved_before_keeps_post_spacing_1_2() {
        let r: Railing = serde_json::from_str(r#"{"post_spacing":1.2}"#).unwrap();
        assert_eq!(r.post_spacing, 1.2);
        assert_eq!(Railing::default().post_spacing, DEFAULT_POST_SPACING);
        assert_eq!(DEFAULT_POST_SPACING, 1.8);
    }

    #[test]
    fn railing_host_defaults_to_well_and_auto_false() {
        let r: Railing = serde_json::from_str(r#"{"follows":3}"#).unwrap();
        assert_eq!(r.host, RailingHost::Well);
        assert!(!r.auto);
        assert_eq!(r.inset, 0.0);
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("host").is_none() && v.get("auto").is_none() && v.get("inset").is_none());
        let d = Railing { host: RailingHost::Deck, auto: true, inset: -0.1, ..Railing::default() };
        let back: Railing = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn room_deck_fields_default_and_skip() {
        let r = Room::default();
        assert_eq!(r.deck_boards, None);
        assert!(r.deck_rails.is_empty());
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("deck_boards").is_none() && v.get("deck_rails").is_none());
        let d = Room { deck_boards: Some(30.0), deck_rails: vec![RailSetting::Off, RailSetting::On], ..Room::default() };
        let back: Room = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn opening_defaults_square_top_right_sides_no_grille_roof_offset_0_15() {
        for o in [Opening::default(), Opening::new(ArchId(1), 1.0, 0.9, 1.2, OpeningKind::Window)] {
            assert_eq!(o.grille, None);
            assert_eq!(o.top, OpeningTop::Square);
            assert_eq!(o.top_rise, 0.0);
            assert_eq!(o.roof_offset, 0.15);
            assert_eq!(o.sill_sides, OpeningSides::RIGHT);
            assert_eq!(o.casing_sides, OpeningSides::RIGHT);
        }
        let o: Opening = serde_json::from_str(r#"{"width":2.0}"#).unwrap();
        assert_eq!(o.sill_sides, OpeningSides::RIGHT);
        assert_eq!(o.roof_offset, 0.15);
    }

    #[test]
    fn model_without_phase17_values_writes_the_same_json() {
        let mut m = ArchModel::default();
        let w = m.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 0.0), 0.2));
        m.push_opening(Opening::new(w, 1.0, 0.9, 2.1, OpeningKind::Door));
        m.push_stair(Stair::default());
        m.push_railing(Railing::default());
        m.push_room(Room::default());
        let text = serde_json::to_string(&m).unwrap();
        for k in [
            "grille",
            "top",
            "top_rise",
            "roof_offset",
            "sill_sides",
            "casing_sides",
            "open_risers",
            "stringers",
            "underside",
            "rail_left",
            "rail_right",
            "plan_cut",
            "inset",
            "host_kind",
            "auto",
            "deck_boards",
            "deck_rails",
        ] {
            assert!(!text.contains(&format!("\"{k}\":")), "{k} written");
        }
        let back: ArchModel = serde_json::from_str(&text).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), text);
    }

    #[test]
    fn room_without_moldings_writes_no_key() {
        let r = Room::default();
        assert!(r.moldings.is_empty());
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("moldings").is_none(), "moldings written for a default room");
    }

    #[test]
    fn model_without_phase18_objects_writes_no_new_keys() {
        let json = serde_json::to_value(ArchModel::default()).unwrap();
        for key in ["countertops", "next_countertop_id", "backsplashes", "next_backsplash_id", "molding_lines", "next_molding_line_id"] {
            assert!(json.get(key).is_none(), "{key} written for a model without it");
        }
    }

    #[test]
    fn push_countertop_backsplash_molding_line_assign_ids() {
        let mut m = ArchModel::default();
        assert_eq!(m.push_countertop(Countertop::default()), ArchId(0));
        assert_eq!(m.push_countertop(Countertop::default()), ArchId(1));
        assert_eq!(m.next_countertop_id, 2);
        assert!(m.countertop(ArchId(0)).is_some());
        assert_eq!(m.countertop_index(ArchId(1)), Some(1));
        assert_eq!(m.push_backsplash(Backsplash::default()), ArchId(0));
        assert_eq!(m.push_backsplash(Backsplash::default()), ArchId(1));
        assert_eq!(m.next_backsplash_id, 2);
        assert!(m.backsplash(ArchId(0)).is_some());
        assert_eq!(m.backsplash_index(ArchId(1)), Some(1));
        assert_eq!(m.push_molding_line(MoldingLine::default()), ArchId(0));
        assert_eq!(m.push_molding_line(MoldingLine::default()), ArchId(1));
        assert_eq!(m.next_molding_line_id, 2);
        assert!(m.molding_line(ArchId(0)).is_some());
        assert_eq!(m.molding_line_index(ArchId(1)), Some(1));
    }

    #[test]
    fn arch_kind_countertop_backsplash_molding_line_keys_labels_handles() {
        let id = ArchId(3);
        assert_eq!(
            (ArchKind::Countertop.key(), ArchKind::Backsplash.key(), ArchKind::MoldingLine.key()),
            ("countertop", "backsplash", "moldingLine")
        );
        assert_eq!(
            (ArchKind::Countertop.label(id), ArchKind::Backsplash.label(id), ArchKind::MoldingLine.label(id)),
            ("ct3".into(), "bs3".into(), "ml3".into())
        );
        assert_eq!(ArchKind::Countertop.handle(id), countertop_handle(id));
        assert_eq!(ArchKind::Backsplash.handle(id), backsplash_handle(id));
        assert_eq!(ArchKind::MoldingLine.handle(id), molding_line_handle(id));
        assert!(ArchKind::Foundation < ArchKind::Countertop);
        assert!(ArchKind::Countertop < ArchKind::Backsplash);
        assert!(ArchKind::Backsplash < ArchKind::MoldingLine);
    }

    #[test]
    fn a_model_holding_only_a_molding_line_or_a_countertop_or_a_backsplash_is_not_empty() {
        let mut m = ArchModel::default();
        m.walls.clear();
        m.levels.clear();
        m.types.clear();
        m.push_molding_line(MoldingLine::default());
        assert!(!m.is_empty());

        let mut m = ArchModel::default();
        m.walls.clear();
        m.levels.clear();
        m.types.clear();
        m.push_countertop(Countertop::default());
        assert!(!m.is_empty());

        let mut m = ArchModel::default();
        m.walls.clear();
        m.levels.clear();
        m.types.clear();
        m.push_backsplash(Backsplash::default());
        assert!(!m.is_empty());
    }
}
