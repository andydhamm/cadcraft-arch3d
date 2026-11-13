//! The render look: how a 3D view is lit, exposed and shown (phase 19).
//!
//! Data only. The engine reads and writes it (`look` command), the renderers honour it.

use super::Backdrop;
use serde::{Deserialize, Serialize};

/// Lowest `ev100` accepted.
pub const MIN_EV100: f64 = -10.0;
/// Highest `ev100` accepted.
pub const MAX_EV100: f64 = 24.0;
/// Highest sun intensity, in lux.
pub const MAX_SUN_LUX: f64 = 200_000.0;
/// Lowest sky turbidity.
pub const MIN_TURBIDITY: f64 = 1.7;
/// Highest sky turbidity.
pub const MAX_TURBIDITY: f64 = 10.0;
/// Highest moon illuminance, in lux.
pub const MAX_MOON_LUX: f64 = 10.0;
/// Largest absolute clip-box coordinate, in metres.
pub const MAX_CLIP_COORD: f64 = 1e6;
/// Largest near-clip distance, in metres.
pub const MAX_NEAR_CLIP: f64 = 1000.0;
/// Most named sun angles a drawing keeps.
pub const MAX_SUN_ANGLES: usize = 64;
/// Longest sun-angle name, in characters.
pub const MAX_SUN_ANGLE_NAME: usize = 64;

/// How a view is rendered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Technique {
    /// The standard raster view.
    #[default]
    Standard,
    /// The path tracer.
    Path,
    /// White clay.
    Clay,
    /// Technical line view.
    Technical,
}

/// Exposure control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum ExposureSetting {
    /// Chosen from the scene.
    #[default]
    Auto,
    /// A fixed exposure value.
    Manual {
        /// Exposure value at ISO 100.
        ev100: f64,
    },
}

/// Tone-mapping curve.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToneMap {
    /// ACES-like filmic curve.
    #[default]
    Aces,
    /// Hable filmic curve.
    Hable,
    /// No tone mapping.
    None,
}

/// The sun in a look.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SunLook {
    /// Whether the sun lights the scene.
    pub on: bool,
    /// Lux on a surface facing the sun.
    pub intensity: f64,
    /// Whether the sun casts shadows.
    pub shadows: bool,
    /// Name of a [`SunAngle`], if the look uses one.
    pub angle: Option<String>,
}

impl Default for SunLook {
    fn default() -> Self {
        SunLook { on: true, intensity: 100_000.0, shadows: true, angle: None }
    }
}

/// The moon in a look.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MoonLook {
    /// Illuminance in lux.
    pub lux: f64,
    /// Compass azimuth, degrees.
    pub azimuth: f64,
    /// Elevation, degrees.
    pub elevation: f64,
}

impl Default for MoonLook {
    fn default() -> Self {
        MoonLook { lux: 0.25, azimuth: 150.0, elevation: 35.0 }
    }
}

/// The sky in a look.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SkyLook {
    /// Whether the sky lights the scene.
    pub on: bool,
    /// Atmospheric turbidity.
    pub turbidity: f64,
    /// Whether stars show at night.
    pub stars: bool,
    /// The moon.
    pub moon: MoonLook,
}

impl Default for SkyLook {
    fn default() -> Self {
        SkyLook { on: true, turbidity: 3.0, stars: false, moon: MoonLook::default() }
    }
}

/// A section box, in metres.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClipBox {
    /// Minimum corner.
    pub min: [f64; 3],
    /// Maximum corner.
    pub max: [f64; 3],
}

impl Default for ClipBox {
    fn default() -> Self {
        ClipBox { min: [-1.0; 3], max: [1.0; 3] }
    }
}

/// A complete render look.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Look {
    /// Render technique.
    pub technique: Technique,
    /// Exposure.
    pub exposure: ExposureSetting,
    /// Tone map.
    pub tone_map: ToneMap,
    /// Sun.
    pub sun: SunLook,
    /// Sky.
    pub sky: SkyLook,
    /// Backdrop (per camera; the global one stays `ArchModel::backdrop`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop: Option<Backdrop>,
    /// Light set by name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light_set: Option<String>,
    /// Cut the roof and upper floors away.
    pub cutaway: bool,
    /// Section box.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<ClipBox>,
    /// Near clip distance, metres.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near_clip: Option<f64>,
}

impl Default for Look {
    fn default() -> Self {
        Look {
            technique: Technique::default(),
            exposure: ExposureSetting::default(),
            tone_map: ToneMap::default(),
            sun: SunLook::default(),
            sky: SkyLook::default(),
            backdrop: None,
            light_set: None,
            cutaway: false,
            clip: None,
            near_clip: None,
        }
    }
}

fn in_range(name: &str, v: f64, lo: f64, hi: f64) -> Result<(), String> {
    if v.is_finite() && v >= lo && v <= hi { Ok(()) } else { Err(format!("`{name}` must be a number from {lo} to {hi}, not {v}")) }
}

impl Look {
    /// Whether this is the default look.
    pub fn is_default(&self) -> bool {
        *self == Look::default()
    }

    /// Check every field; the error names the field and value.
    pub fn validate(&self) -> Result<(), String> {
        if let ExposureSetting::Manual { ev100 } = self.exposure {
            in_range("exposure.ev100", ev100, MIN_EV100, MAX_EV100)?;
        }
        in_range("sun.intensity", self.sun.intensity, 0.0, MAX_SUN_LUX)?;
        in_range("sky.turbidity", self.sky.turbidity, MIN_TURBIDITY, MAX_TURBIDITY)?;
        in_range("sky.moon.lux", self.sky.moon.lux, 0.0, MAX_MOON_LUX)?;
        in_range("sky.moon.elevation", self.sky.moon.elevation, -90.0, 90.0)?;
        if !self.sky.moon.azimuth.is_finite() {
            return Err(format!("`sky.moon.azimuth` must be finite, not {}", self.sky.moon.azimuth));
        }
        if let Some(c) = &self.clip {
            for axis in 0..3 {
                let (lo, hi) = (c.min[axis], c.max[axis]);
                in_range("clip.min", lo, -MAX_CLIP_COORD, MAX_CLIP_COORD)?;
                in_range("clip.max", hi, -MAX_CLIP_COORD, MAX_CLIP_COORD)?;
                if lo >= hi {
                    return Err(format!("`clip` min {lo} must be below max {hi} on axis {axis}"));
                }
            }
        }
        if let Some(n) = self.near_clip
            && !(n.is_finite() && n > 0.0 && n <= MAX_NEAR_CLIP)
        {
            return Err(format!("`nearClip` must be above 0 and at most {MAX_NEAR_CLIP}, not {n}"));
        }
        Ok(())
    }
}

/// A named sun direction: a date and time, or a compass azimuth and elevation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SunAngle {
    /// Name, unique within the drawing.
    pub name: String,
    /// `YYYY-MM-DD`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// `HH:MM`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    /// Compass azimuth, degrees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azimuth: Option<f64>,
    /// Elevation, degrees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation: Option<f64>,
}

fn valid_date(date: &str) -> bool {
    let parts: Vec<&str> = date.split('-').collect();
    let [y, m, d] = parts.as_slice() else { return false };
    let (Ok(y), Ok(m), Ok(d)) = (y.parse::<i64>(), m.parse::<u32>(), d.parse::<u32>()) else {
        return false;
    };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&d)
}

fn valid_time(time: &str) -> bool {
    if time == "solar-noon" {
        return true;
    }
    let mut it = time.split(':');
    let (Some(h), Some(m)) = (it.next(), it.next()) else { return false };
    let s = it.next();
    if it.next().is_some() {
        return false;
    }
    let ok = |t: &str, hi: u32| t.parse::<u32>().is_ok_and(|v| v <= hi);
    ok(h, 23) && ok(m, 59) && s.is_none_or(|s| ok(s, 59))
}

impl SunAngle {
    /// Check the angle; the error names the field and value.
    pub fn validate(&self) -> Result<(), String> {
        let n = self.name.chars().count();
        if self.name.trim().is_empty() || n > MAX_SUN_ANGLE_NAME {
            return Err(format!("sun angle name must be 1 to {MAX_SUN_ANGLE_NAME} characters, not `{}`", self.name));
        }
        let clock = self.date.is_some() || self.time.is_some();
        let compass = self.azimuth.is_some() || self.elevation.is_some();
        match (clock, compass) {
            (true, true) => return Err(format!("sun angle `{}`: give date and time, or azimuth and elevation, not both", self.name)),
            (false, false) => return Err(format!("sun angle `{}`: needs date and time, or azimuth and elevation", self.name)),
            _ => {}
        }
        if clock {
            let (Some(d), Some(t)) = (&self.date, &self.time) else {
                return Err(format!("sun angle `{}`: date and time go together", self.name));
            };
            if !valid_date(d) {
                return Err(format!("sun angle `{}`: `date` {d} is not a YYYY-MM-DD date", self.name));
            }
            if !valid_time(t) {
                return Err(format!("sun angle `{}`: `time` {t} is not an HH:MM time", self.name));
            }
        } else {
            let (Some(a), Some(e)) = (self.azimuth, self.elevation) else {
                return Err(format!("sun angle `{}`: azimuth and elevation go together", self.name));
            };
            if !a.is_finite() {
                return Err(format!("sun angle `{}`: `azimuth` must be finite, not {a}", self.name));
            }
            in_range("elevation", e, -90.0, 90.0)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::Projection;

    #[test]
    fn defaults_match_the_brief() {
        let l = Look::default();
        assert!(l.is_default());
        assert_eq!(l.technique, Technique::Standard);
        assert_eq!(l.exposure, ExposureSetting::Auto);
        assert_eq!(l.tone_map, ToneMap::Aces);
        assert!(l.sun.on && l.sun.shadows && l.sun.angle.is_none());
        assert_eq!(l.sun.intensity, 100_000.0);
        assert!(l.sky.on && !l.sky.stars);
        assert_eq!(l.sky.turbidity, 3.0);
        assert_eq!((l.sky.moon.lux, l.sky.moon.azimuth, l.sky.moon.elevation), (0.25, 150.0, 35.0));
        assert!(l.backdrop.is_none() && l.light_set.is_none() && !l.cutaway && l.clip.is_none() && l.near_clip.is_none());
        assert!(l.validate().is_ok());
        let m = crate::arch::ArchModel::default();
        assert!(m.look.is_default() && m.sun_angles.is_empty());
        assert!(crate::arch::Camera3d::default().look.is_none());
    }

    #[test]
    fn serde_round_trip_and_old_payloads_load() {
        let mut l = Look::default();
        l.technique = Technique::Path;
        l.exposure = ExposureSetting::Manual { ev100: 3.0 };
        l.tone_map = ToneMap::None;
        l.clip = Some(ClipBox { min: [0.0; 3], max: [1.0; 3] });
        l.near_clip = Some(0.5);
        let v = serde_json::to_value(&l).unwrap();
        assert_eq!(v["exposure"], serde_json::json!({"mode": "manual", "ev100": 3.0}));
        assert_eq!(v["toneMap"], "none");
        assert_eq!(serde_json::from_value::<Look>(v).unwrap(), l);
        assert_eq!(serde_json::from_str::<Look>("{}").unwrap(), Look::default());

        let mut m = crate::arch::ArchModel::default();
        let plain = serde_json::to_value(&m).unwrap();
        assert!(plain.get("look").is_none() && plain.get("sun_angles").is_none() && plain.get("sunAngles").is_none());
        m.look = l;
        m.sun_angles.push(SunAngle { name: "noon".into(), azimuth: Some(180.0), elevation: Some(60.0), ..Default::default() });
        m.cameras.push(crate::arch::Camera3d { name: "c".into(), look: Some(Look::default()), ..Default::default() });
        let back: crate::arch::ArchModel = serde_json::from_value(serde_json::to_value(&m).unwrap()).unwrap();
        assert_eq!(back.look, m.look);
        assert_eq!(back.sun_angles, m.sun_angles);
        assert_eq!(back.cameras[0].look, Some(Look::default()));
        let old: crate::arch::Camera3d = serde_json::from_str(r#"{"name":"old"}"#).unwrap();
        assert!(old.look.is_none());
    }

    #[test]
    fn validate_refuses_out_of_range() {
        let bad = |f: &dyn Fn(&mut Look)| {
            let mut l = Look::default();
            f(&mut l);
            l.validate().unwrap_err()
        };
        assert!(bad(&|l| l.exposure = ExposureSetting::Manual { ev100: 25.0 }).contains("ev100"));
        assert!(bad(&|l| l.exposure = ExposureSetting::Manual { ev100: f64::NAN }).contains("ev100"));
        assert!(bad(&|l| l.sun.intensity = 200_001.0).contains("sun.intensity"));
        assert!(bad(&|l| l.sun.intensity = -1.0).contains("sun.intensity"));
        assert!(bad(&|l| l.sky.turbidity = 1.0).contains("turbidity"));
        assert!(bad(&|l| l.sky.turbidity = 11.0).contains("turbidity"));
        assert!(bad(&|l| l.sky.moon.lux = 11.0).contains("moon.lux"));
        assert!(bad(&|l| l.sky.moon.elevation = 91.0).contains("moon.elevation"));
        assert!(bad(&|l| l.sky.moon.azimuth = f64::INFINITY).contains("azimuth"));
        assert!(bad(&|l| l.clip = Some(ClipBox { min: [0.0; 3], max: [0.0, 1.0, 1.0] })).contains("clip"));
        assert!(bad(&|l| l.clip = Some(ClipBox { min: [0.0; 3], max: [2e6, 1.0, 1.0] })).contains("clip.max"));
        assert!(bad(&|l| l.near_clip = Some(0.0)).contains("nearClip"));
        assert!(bad(&|l| l.near_clip = Some(1000.5)).contains("nearClip"));
        let mut ok = Look::default();
        ok.exposure = ExposureSetting::Manual { ev100: -10.0 };
        ok.near_clip = Some(1000.0);
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn sun_angle_needs_one_form() {
        let s = |f: &dyn Fn(&mut SunAngle)| {
            let mut a = SunAngle { name: "a".into(), ..Default::default() };
            f(&mut a);
            a.validate()
        };
        assert!(s(&|_| {}).is_err());
        assert!(
            s(&|a| {
                a.date = Some("2026-06-21".into());
                a.time = Some("12:00".into());
            })
            .is_ok()
        );
        assert!(
            s(&|a| {
                a.azimuth = Some(90.0);
                a.elevation = Some(-5.0);
            })
            .is_ok()
        );
        assert!(s(&|a| a.date = Some("2026-06-21".into())).is_err());
        assert!(s(&|a| a.azimuth = Some(90.0)).is_err());
        assert!(
            s(&|a| {
                a.date = Some("2026-02-30".into());
                a.time = Some("12:00".into());
            })
            .is_err()
        );
        assert!(
            s(&|a| {
                a.date = Some("2026-06-21".into());
                a.time = Some("25:00".into());
            })
            .is_err()
        );
        assert!(
            s(&|a| {
                a.azimuth = Some(90.0);
                a.elevation = Some(91.0);
            })
            .is_err()
        );
        assert!(
            s(&|a| {
                a.date = Some("2026-06-21".into());
                a.time = Some("12:00".into());
                a.azimuth = Some(1.0);
                a.elevation = Some(1.0);
            })
            .is_err()
        );
        assert!(
            s(&|a| {
                a.name = String::new();
                a.azimuth = Some(1.0);
                a.elevation = Some(1.0);
            })
            .is_err()
        );
        assert!(
            s(&|a| {
                a.name = "x".repeat(65);
                a.azimuth = Some(1.0);
                a.elevation = Some(1.0);
            })
            .is_err()
        );
    }

    #[test]
    fn sky_hdri_and_image_horizon_serde() {
        assert_eq!(serde_json::to_value(Backdrop::Sky).unwrap(), serde_json::json!({"kind": "sky"}));
        assert_eq!(serde_json::from_str::<Backdrop>(r#"{"kind":"sky"}"#).unwrap(), Backdrop::Sky);
        let h: Backdrop = serde_json::from_str(r#"{"kind":"hdri","image":"studio"}"#).unwrap();
        assert_eq!(h, Backdrop::Hdri { image: "studio".into(), intensity: 1.0, rotation: 0.0, horizon: 0.0 });
        let v = serde_json::to_value(&h).unwrap();
        assert_eq!(serde_json::from_value::<Backdrop>(v).unwrap(), h);
        let i = Backdrop::Image { item: "p".into(), projection: Projection::Flat, horizon: 0.0 };
        assert!(serde_json::to_value(&i).unwrap().get("horizon").is_none());
        let old: Backdrop = serde_json::from_str(r#"{"kind":"image","item":"p","projection":"flat"}"#).unwrap();
        assert_eq!(old, i);
        let j = Backdrop::Image { item: "p".into(), projection: Projection::Equirect, horizon: 12.5 };
        assert_eq!(serde_json::to_value(&j).unwrap()["horizon"], 12.5);
        assert_eq!(serde_json::from_value::<Backdrop>(serde_json::to_value(&j).unwrap()).unwrap(), j);
    }
}
