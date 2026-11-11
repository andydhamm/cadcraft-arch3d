//! Light fixtures (phase 10): placed in plan on a ceiling, a wall or the floor, each with a plan
//! symbol, a 3D housing and a light (point, spot or, since phase 11, a rectangular area; lumens,
//! kelvin; optionally an IES photometric distribution). Plain data on
//! [`super::ArchModel`]; the catalogue, placement and photometry live in `arch::light`, the 3D
//! housing, the scene lights and illuminance in `arch3d::light`, the GPU lighting in `render3d`.

use super::ArchId;
use crate::Handle;
use cadcraft_geom::Vec2;
use serde::{Deserialize, Serialize};

/// Offset within the arch-handle range reserved for light fixtures (label `lf{n}`).
pub const LIGHT_HANDLE_OFFSET: u64 = 0x48000;
/// Most light fixtures in a drawing.
pub const MAX_LIGHT_FIXTURES: usize = 1_024;
/// Most rows or columns `LIGHTFIXTURE {"room":…}` lays out.
pub const MAX_FIXTURE_GRID: u32 = 32;
/// Light output limits (lumens) and colour temperature limits (kelvin) a fixture accepts.
pub const MAX_LUMENS: f64 = 200_000.0;
pub const MIN_KELVIN: f64 = 1_000.0;
pub const MAX_KELVIN: f64 = 12_000.0;
/// Widest spot cone or falloff (full apex angle, degrees); a wider light is a point light.
pub const MAX_SPOT_ANGLE: f64 = 170.0;
/// Light-point height above the level floor of a wall fixture and of a floor lamp when the
/// fixture stores none (m).
pub const DEFAULT_WALL_HEIGHT: f64 = 1.8;
pub const DEFAULT_FLOOR_LAMP_HEIGHT: f64 = 1.5;
/// How far a pendant's light point hangs below the ceiling when the fixture stores no height (m).
pub const DEFAULT_PENDANT_DROP: f64 = 0.6;
/// Longest side of an area light (m), and the shortest (a side below it is not an area).
pub const MAX_AREA_SIDE: f64 = 4.0;
pub const MIN_AREA_SIDE: f64 = 0.01;
/// Most vertical or horizontal angles an IES profile may carry, and most candela values.
pub const MAX_IES_ANGLES: usize = 1_024;
pub const MAX_IES_VALUES: usize = 65_536;
/// Largest IES file read (bytes).
pub const MAX_IES_BYTES: usize = 1_000_000;

/// Most points in a strip path.
pub const MAX_STRIP_POINTS: usize = 256;
/// Most total length of a strip path (m).
pub const MAX_STRIP_LENGTH: f64 = 100.0;
/// Most lumens per metre a strip can emit.
pub const MAX_LUMENS_PER_METRE: f64 = 10_000.0;
/// Spacing of sample points along a strip for illuminance (m).
pub const STRIP_SAMPLE_SPACING: f64 = 0.15;

/// How the fixture emits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LightKind {
    /// Equal intensity in every direction; one cube shadow map.
    #[default]
    Point,
    /// Uniform intensity inside `cone`, fading to zero at `falloff`; one shadow map.
    Spot,
    /// A `width` × `length` rectangle (phase 11) emitting like a Lambertian surface from its face
    /// toward the aim: peak intensity `lumens / π` cd along the aim, `cos θ` off it, nothing
    /// behind. The raster view draws it as a spot over the hemisphere; the path tracer samples
    /// the rectangle.
    Area,
    /// A line of emitters along a 2D path (phase 19), emitting `lumensPerMetre / π` cd per metre
    /// at each sample; the path tracer skips it until p19-strip-render.
    Strip,
}

/// What the fixture is fixed to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LightMount {
    /// Takes its room's ceiling height.
    #[default]
    Ceiling,
    Wall,
    Floor,
}

/// The housing drawn in plan and built in 3D.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FixtureShape {
    /// A recessed can: a trim ring flush with the ceiling.
    #[default]
    RecessedCan,
    /// A shade on a cord below the ceiling.
    Pendant,
    /// A half-cylinder shade against a wall.
    Sconce,
    /// A shallow dome on the ceiling.
    SurfaceDome,
    /// A base, a pole and a shade standing on the floor.
    FloorLamp,
    /// A rectangular recessed panel (troffer) in the ceiling: a frame and a diffuser, the area
    /// light's face flush with the ceiling (phase 11).
    Panel,
    /// A line of LED emitters along a 2D path (phase 19).
    Strip,
}

impl FixtureShape {
    /// The name used in JSON and DXF (`recessed-can`, `pendant`, …).
    pub fn name(self) -> &'static str {
        match self {
            FixtureShape::RecessedCan => "recessed-can",
            FixtureShape::Pendant => "pendant",
            FixtureShape::Sconce => "sconce",
            FixtureShape::SurfaceDome => "surface-dome",
            FixtureShape::FloorLamp => "floor-lamp",
            FixtureShape::Panel => "panel",
            FixtureShape::Strip => "strip",
        }
    }
}

/// A light fixture. The light values are the fixture's own (copied from the catalogue entry it
/// was placed from, then editable).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightFixture {
    pub id: ArchId,
    /// The catalogue entry it was placed from, e.g. `Recessed pot light` (display only).
    pub fixture: String,
    pub shape: FixtureShape,
    pub kind: LightKind,
    pub mount: LightMount,
    /// The level it stands on (its `index`).
    #[serde(default)]
    pub level: u32,
    /// Plan position of the light point (a wall fixture: on the wall face).
    pub position: Vec2,
    /// Height of the light point above the level floor; `None` = derived from the mount (a
    /// ceiling fixture takes its room's ceiling height, a pendant hangs [`DEFAULT_PENDANT_DROP`]
    /// below it, a wall fixture [`DEFAULT_WALL_HEIGHT`], a floor lamp
    /// [`DEFAULT_FLOOR_LAMP_HEIGHT`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// Plan direction the fixture faces, degrees counter-clockwise from +X (a sconce's outward
    /// wall normal, a tilted spot's aim).
    #[serde(default)]
    pub rotation: f64,
    /// A spot's aim below the horizontal, degrees: 90 = straight down, 0 = level.
    #[serde(default = "default_tilt")]
    pub tilt: f64,
    pub lumens: f64,
    pub kelvin: f64,
    /// Full apex angle of a spot's uniform cone, degrees.
    #[serde(default = "default_cone")]
    pub cone: f64,
    /// Full apex angle where a spot's light reaches zero, degrees (>= `cone`; equal = a hard edge).
    #[serde(default = "default_cone")]
    pub falloff: f64,
    /// An off fixture keeps its housing and emits nothing.
    #[serde(default = "default_on")]
    pub on: bool,
    /// Dimmer level (0..=1); 1 = full brightness.
    #[serde(default = "default_brightness")]
    pub brightness: f64,
    /// An area light's size (m): `width` along the fixture's `rotation`, `length` across it. Zero
    /// for a point or spot light.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub width: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub length: f64,
    /// The fixture's measured light distribution (an IES LM-63 file read into the drawing), which
    /// replaces the cone; `None` = the cone or the area's cosine law.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ies: Option<IesProfile>,
    /// The 2D path a strip light follows (metres, plan space; closed ring for tray rings).
    #[serde(default)]
    pub path: Vec<Vec2>,
    /// Whether the strip path is a closed ring (tray rings, phase 19).
    #[serde(default)]
    pub closed: bool,
    /// Lumens per metre of strip (phase 19).
    #[serde(default)]
    pub lumens_per_metre: f64,
    /// How the strip emits (down or out, phase 19).
    #[serde(default)]
    pub facing: StripFacing,
    /// The tray `ArchId` this strip runs along (set by p19-strip-tray).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tray: Option<ArchId>,
}

/// How a strip light emits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StripFacing {
    /// Emits downward (Lambertian, toward -Z).
    #[default]
    Down,
    /// Emits outward, horizontal, to the right of each segment (outward on a counter-clockwise ring).
    Out,
}

/// A photometric distribution read from an IES LM-63 file (type C photometry): candela by
/// vertical angle (0° = straight down along the aim, 180° = straight up) and horizontal angle
/// (0° = the fixture's `rotation` direction, counter-clockwise seen from above).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IesProfile {
    /// The file's `[LUMINAIRE]` (else `[TEST]`) keyword, else the file name, for display.
    pub name: String,
    /// Vertical angles (degrees, ascending, within 0..=180).
    pub vertical: Vec<f64>,
    /// Horizontal angles (degrees, ascending, within 0..=360; one angle = rotationally
    /// symmetric; last 90 = quadrant-symmetric; last 180 = bilaterally symmetric).
    pub horizontal: Vec<f64>,
    /// Candela, already multiplied by the file's multiplier and ballast factor, horizontal-major:
    /// the value at (`horizontal[h]`, `vertical[v]`) is `candela[h * vertical.len() + v]`.
    pub candela: Vec<f64>,
    /// The rated lumens of the lamps (lamps × lumens per lamp); `-1` = absolute photometry.
    pub lumens: f64,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

fn default_tilt() -> f64 {
    90.0
}

fn default_cone() -> f64 {
    60.0
}

fn default_on() -> bool {
    true
}

fn default_brightness() -> f64 {
    1.0
}

impl LightFixture {
    /// A fixture at `position` with the recessed pot light's values (spot, 60° cone, 800 lm,
    /// 3000 K, ceiling, aimed straight down); set the rest with struct update syntax.
    pub fn new(id: ArchId, fixture: &str, position: Vec2) -> Self {
        LightFixture {
            id,
            fixture: fixture.to_string(),
            shape: FixtureShape::RecessedCan,
            kind: LightKind::Spot,
            mount: LightMount::Ceiling,
            level: 0,
            position,
            height: None,
            rotation: 0.0,
            tilt: 90.0,
            lumens: 800.0,
            kelvin: 3000.0,
            cone: 60.0,
            falloff: 60.0,
            on: true,
            brightness: 1.0,
            width: 0.0,
            length: 0.0,
            ies: None,
            path: Vec::new(),
            closed: false,
            lumens_per_metre: 0.0,
            facing: StripFacing::default(),
            tray: None,
        }
    }
}

/// The handle of light fixture `id`.
pub fn light_handle(id: ArchId) -> Handle {
    Handle(super::ARCH_HANDLE_BASE.saturating_add(LIGHT_HANDLE_OFFSET).saturating_add(u64::from(id.0)))
}

/// Whether `h` falls in the light-fixture handle range.
pub fn is_light_handle(h: Handle) -> bool {
    let lo = super::ARCH_HANDLE_BASE.saturating_add(LIGHT_HANDLE_OFFSET);
    h.0 >= lo && h.0 < lo.saturating_add(1u64 << 15)
}

/// `lf{n}`.
pub fn light_label(id: ArchId) -> String {
    format!("lf{}", id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_handles_follow_the_plant_range() {
        let plants_end = super::super::ARCH_HANDLE_BASE + super::super::PLANT_HANDLE_OFFSET + (1u64 << 15);
        assert!(light_handle(ArchId(0)).0 >= plants_end);
        assert!(is_light_handle(light_handle(ArchId(7))));
        assert!(!is_light_handle(super::super::plant_handle(ArchId(7))));
        assert_eq!(light_label(ArchId(3)), "lf3");
    }

    #[test]
    fn a_fixture_without_optional_fields_reads_with_the_defaults() {
        let json = r#"{"id":0,"fixture":"Pendant","shape":"pendant","kind":"point","mount":"ceiling","position":{"x":1.0,"y":2.0},"lumens":1100.0,"kelvin":2700.0}"#;
        let f: LightFixture = serde_json::from_str(json).unwrap_or_else(|_| LightFixture::new(ArchId(9), "x", Vec2::new(0.0, 0.0)));
        assert_eq!(f.id, ArchId(0));
        assert!(f.on);
        assert_eq!(f.tilt, 90.0);
        assert_eq!((f.cone, f.falloff), (60.0, 60.0));
        assert_eq!(f.height, None);
        assert_eq!(serde_json::to_string(&f).map(|s| s.contains("height")).ok(), Some(false));
    }

    #[test]
    fn light_brightness_defaults_to_one() {
        let json = r#"{"id":0,"fixture":"Pendant","shape":"pendant","kind":"point","mount":"ceiling","position":{"x":1.0,"y":2.0},"lumens":1100.0,"kelvin":2700.0}"#;
        let f: LightFixture = serde_json::from_str(json).unwrap_or_else(|_| LightFixture::new(ArchId(9), "x", Vec2::new(0.0, 0.0)));
        assert_eq!(f.brightness, 1.0);
        assert_eq!(LightFixture::new(ArchId(1), "x", Vec2::new(0.0, 0.0)).brightness, 1.0);
    }
}
