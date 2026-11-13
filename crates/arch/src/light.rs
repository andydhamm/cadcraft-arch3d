//! Light fixtures in plan (phase 10): the built-in catalogue, photometry, where a fixture's light
//! point is, an even grid in a room and the plan symbol. The data is
//! `cadcraft_doc::arch::LightFixture`; the 3D housing and the scene lights are in
//! `arch3d::light`.
//!
//! The catalogue and the photometry are complete; `validate_fixture`, `kelvin_to_rgb`,
//! `light_point`, `aim_direction` and `grid_positions` are stubs filled by `p10-arch-light-placement`
//! (their signatures are final); `plan_symbol` lives in `crate::light_symbol`.

use cadcraft_doc::arch::{ArchModel, FixtureShape, IesProfile, Level, LightFixture, LightKind, LightMount, Room};
use cadcraft_doc::arch::{
    DEFAULT_FLOOR_LAMP_HEIGHT, DEFAULT_PENDANT_DROP, DEFAULT_WALL_HEIGHT, MAX_AREA_SIDE, MAX_FIXTURE_GRID, MAX_IES_ANGLES, MAX_IES_VALUES,
    MAX_KELVIN, MAX_LUMENS, MAX_LUMENS_PER_METRE, MAX_SPOT_ANGLE, MAX_STRIP_LENGTH, MAX_STRIP_POINTS, MIN_AREA_SIDE, MIN_KELVIN,
    STRIP_SAMPLE_SPACING, StripFacing,
};
use cadcraft_geom::{Vec2, Vec3, point_in_polygon};

/// A catalogue entry: what `LIGHTFIXTURE {"type":…}` copies onto a new fixture.
#[derive(Clone, Debug, PartialEq)]
pub struct FixtureType {
    /// Display name, matched case-insensitively (`Recessed pot light`).
    pub name: &'static str,
    pub shape: FixtureShape,
    pub kind: LightKind,
    pub mount: LightMount,
    pub lumens: f64,
    pub kelvin: f64,
    /// Full apex angles in degrees (spots; a point light keeps 60/60, unused).
    pub cone: f64,
    pub falloff: f64,
    /// Housing diameter (m): the trim ring, the shade, the dome or the sconce's width.
    pub diameter: f64,
    /// An area light's size (m), `width` along the fixture's rotation; 0 for a point or spot.
    pub width: f64,
    pub length: f64,
}

/// A strip round tray `tray` (`tr<n>`): the tray's outline inset by `inset` metres, as one
/// counter-clockwise closed ring. Returns the tray's level, the ring and the default height above
/// that level: the finished ceiling just outside the outline minus the level's elevation, plus
/// 0.02 m (the strip sits on the recess's bottom edge).
pub fn tray_strip_path(model: &ArchModel, tray: &str, inset: f64) -> Result<(u32, Vec<Vec2>, f64), String> {
    let label = tray.trim();
    let unknown = || format!("unknown tray `{label}`");
    let n = label.strip_prefix("tr").or_else(|| label.strip_prefix("TR")).and_then(|d| d.parse::<u32>().ok()).ok_or_else(unknown)?;
    let t = model.trays.iter().find(|t| t.id.0 == n).ok_or_else(unknown)?;
    if !inset.is_finite() || inset < 0.0 {
        return Err(format!("`inset` must be 0 m or more; got {inset}"));
    }
    let no_path = || format!("the inset leaves no strip path inside tray `{label}`");
    let mut outline = crate::ceiling::tray::outline(model, t).map_err(|e| format!("tray `{label}`: {e}"))?;
    if outline.len() > 1 && outline.first() == outline.last() {
        outline.pop();
    }
    if outline.len() < 3 || outline.iter().any(|p| !p.is_finite()) {
        return Err(format!("tray `{label}` has no usable outline"));
    }
    let signed_area = |r: &[Vec2]| -> f64 { r.iter().zip(r.iter().cycle().skip(1)).map(|(a, b)| a.x * b.y - b.x * a.y).sum() };
    // The ceiling just outside the outline: its first edge's midpoint moved 0.01 m outward.
    let (a, b) = match (outline.first(), outline.get(1)) {
        (Some(a), Some(b)) => (*a, *b),
        _ => return Err(format!("tray `{label}` has no usable outline")),
    };
    let edge = b - a;
    let len = edge.len();
    if len <= 0.0 {
        return Err(format!("tray `{label}` has no usable outline"));
    }
    let right = Vec2::new(edge.y, -edge.x) * (1.0 / len);
    let outward = if signed_area(&outline) >= 0.0 { right } else { right * -1.0 };
    let probe = (a + b) * 0.5 + outward * 0.01;
    let ceiling = crate::ceiling::finished_ceiling_at(model, t.level, probe).ok_or_else(|| format!("no ceiling beside tray `{label}`"))?;
    let elevation = model.levels.iter().rev().find(|l| l.index == t.level).or_else(|| model.levels.last()).map_or(0.0, |l| l.elevation);
    let height = ceiling - elevation + 0.02;
    if !height.is_finite() {
        return Err(format!("no finite ceiling beside tray `{label}`"));
    }

    let mut ring = if inset > 0.0 {
        let regions = cadcraft_geom::skeleton::inset(&outline, &[], inset).map_err(|_| no_path())?;
        match regions.as_slice() {
            [r] if r.holes.is_empty() => r.outer.clone(),
            _ => return Err(no_path()),
        }
    } else {
        outline
    };
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    if ring.len() < 3 {
        return Err(no_path());
    }
    if signed_area(&ring) < 0.0 {
        ring.reverse();
    }
    Ok((t.level, ring, height))
}

/// Lumens per metre of the catalogue's LED strip.
pub const LED_STRIP_LUMENS_PER_METRE: f64 = 500.0;

/// The built-in catalogue, in this order: the recessed pot light (the default), the pendant, the
/// wall sconce, the surface ceiling fixture and the floor lamp.
pub fn catalogue() -> Vec<FixtureType> {
    vec![
        FixtureType {
            name: "Recessed pot light",
            shape: FixtureShape::RecessedCan,
            kind: LightKind::Spot,
            mount: LightMount::Ceiling,
            lumens: 800.0,
            kelvin: 3000.0,
            cone: 60.0,
            falloff: 60.0,
            diameter: 0.15,
            width: 0.0,
            length: 0.0,
        },
        FixtureType {
            name: "Pendant",
            shape: FixtureShape::Pendant,
            kind: LightKind::Point,
            mount: LightMount::Ceiling,
            lumens: 1100.0,
            kelvin: 2700.0,
            cone: 60.0,
            falloff: 60.0,
            diameter: 0.3,
            width: 0.0,
            length: 0.0,
        },
        FixtureType {
            name: "Wall sconce",
            shape: FixtureShape::Sconce,
            kind: LightKind::Point,
            mount: LightMount::Wall,
            lumens: 450.0,
            kelvin: 2700.0,
            cone: 60.0,
            falloff: 60.0,
            diameter: 0.2,
            width: 0.0,
            length: 0.0,
        },
        FixtureType {
            name: "Surface ceiling fixture",
            shape: FixtureShape::SurfaceDome,
            kind: LightKind::Point,
            mount: LightMount::Ceiling,
            lumens: 1500.0,
            kelvin: 3000.0,
            cone: 60.0,
            falloff: 60.0,
            diameter: 0.35,
            width: 0.0,
            length: 0.0,
        },
        FixtureType {
            name: "Floor lamp",
            shape: FixtureShape::FloorLamp,
            kind: LightKind::Point,
            mount: LightMount::Floor,
            lumens: 1200.0,
            kelvin: 2700.0,
            cone: 60.0,
            falloff: 60.0,
            diameter: 0.4,
            width: 0.0,
            length: 0.0,
        },
        FixtureType {
            name: "LED panel 600",
            shape: FixtureShape::Panel,
            kind: LightKind::Area,
            mount: LightMount::Ceiling,
            lumens: 3600.0,
            kelvin: 4000.0,
            cone: 60.0,
            falloff: 60.0,
            diameter: 0.6,
            width: 0.6,
            length: 0.6,
        },
        FixtureType {
            name: "LED strip",
            shape: FixtureShape::Strip,
            kind: LightKind::Strip,
            mount: LightMount::Ceiling,
            lumens: 0.0,
            kelvin: 3000.0,
            cone: 60.0,
            falloff: 60.0,
            diameter: 0.012,
            width: 0.0,
            length: 0.0,
        },
    ]
}

/// The catalogue entry called `name` (case-insensitive, surrounding spaces ignored).
pub fn find_type(name: &str) -> Option<FixtureType> {
    let n = name.trim();
    catalogue().into_iter().find(|t| t.name.eq_ignore_ascii_case(n))
}

/// A new fixture from a catalogue entry at `position` (every light value copied from `t`).
pub fn fixture_from_type(id: cadcraft_doc::arch::ArchId, t: &FixtureType, position: Vec2) -> LightFixture {
    LightFixture {
        shape: t.shape,
        kind: t.kind,
        mount: t.mount,
        lumens: t.lumens,
        kelvin: t.kelvin,
        cone: t.cone,
        falloff: t.falloff,
        width: t.width,
        length: t.length,
        lumens_per_metre: if t.kind == LightKind::Strip { LED_STRIP_LUMENS_PER_METRE } else { 0.0 },
        ..LightFixture::new(id, t.name, position)
    }
}

/// Housing diameter of a fixture (m): its catalogue entry's, or 0.15 when the name is unknown.
pub fn housing_diameter(f: &LightFixture) -> f64 {
    find_type(&f.fixture).map(|t| t.diameter).unwrap_or(0.15)
}

/// Solid angle (sr) of a cone with full apex angle `apex_deg`: 2π(1 − cos(apex/2)).
/// 60° gives 0.841787…; 360° gives 4π.
pub fn cone_solid_angle(apex_deg: f64) -> f64 {
    let half = (apex_deg.clamp(0.0, 360.0) * 0.5).to_radians();
    2.0 * std::f64::consts::PI * (1.0 - half.cos())
}

/// Luminous intensity (candela) of a fixture: a point light spreads its lumens over 4π sr, a spot
/// over its `cone`'s solid angle (the falloff band adds light outside it and is not counted), an
/// area light's peak along its aim is lumens / π (it falls off as cos θ), a strip's per-metre
/// intensity is lumens-per-metre / π. Zero for an off fixture or a non-finite input.
pub fn intensity_cd(f: &LightFixture) -> f64 {
    if !f.on {
        return 0.0;
    }
    if f.kind == LightKind::Strip {
        if !f.lumens_per_metre.is_finite() || f.lumens_per_metre <= 0.0 {
            return 0.0;
        }
        return f.lumens_per_metre / std::f64::consts::PI;
    }
    if !f.lumens.is_finite() || f.lumens <= 0.0 {
        return 0.0;
    }
    let omega = match f.kind {
        LightKind::Point => 4.0 * std::f64::consts::PI,
        LightKind::Spot => cone_solid_angle(f.cone),
        // A Lambertian face: Φ = π·I₀, so I₀ = Φ/π along the aim (I₀·cos θ off it).
        LightKind::Area => std::f64::consts::PI,
        // Handled above; unreachable.
        LightKind::Strip => 0.0,
    };
    if omega.is_finite() && omega > 0.0 { f.lumens / omega } else { 0.0 }
}

/// Check a fixture's values: finite position; lumens in `0..=MAX_LUMENS`; kelvin in
/// `MIN_KELVIN..=MAX_KELVIN`; a spot's cone in `(0, MAX_SPOT_ANGLE]` and falloff in
/// `cone..=MAX_SPOT_ANGLE`; tilt in `-90..=90`; rotation finite; a height, when set, finite and in
/// `0..=100`; an area light's `width` then `length` in `MIN_AREA_SIDE..=MAX_AREA_SIDE` (a point or
/// spot ignores them); an IES profile, when set, has non-empty angle lists of at most
/// `MAX_IES_ANGLES`, exactly `vertical × horizontal` candela values (at most `MAX_IES_VALUES`),
/// each finite and >= 0. The message names the field (`lumens must be 0..=200000, got -5`,
/// `width must be 0.01..=4, got 5`, `ies: candela has 3 values, expected 4`).
pub fn validate_fixture(f: &LightFixture) -> Result<(), String> {
    if !f.position.is_finite() {
        return Err(format!("position must be finite, got {:?}", f.position));
    }
    if f.kind == LightKind::Strip {
        validate_strip(f)?;
    }
    if !f.lumens.is_finite() || f.lumens < 0.0 || f.lumens > MAX_LUMENS {
        return Err(format!("lumens must be 0..={MAX_LUMENS}, got {}", f.lumens));
    }
    if !f.kelvin.is_finite() || f.kelvin < MIN_KELVIN || f.kelvin > MAX_KELVIN {
        return Err(format!("kelvin must be {MIN_KELVIN}..={MAX_KELVIN}, got {}", f.kelvin));
    }
    if !f.rotation.is_finite() {
        return Err(format!("rotation must be finite, got {}", f.rotation));
    }
    if !f.tilt.is_finite() || f.tilt < -90.0 || f.tilt > 90.0 {
        return Err(format!("tilt must be -90..=90, got {}", f.tilt));
    }
    if let Some(h) = f.height {
        if !h.is_finite() || h < 0.0 || h > 100.0 {
            return Err(format!("height must be 0..=100, got {h}"));
        }
    }

    if !f.cone.is_finite() || !f.falloff.is_finite() {
        return Err("cone and falloff must be finite".to_string());
    }

    if f.kind == LightKind::Spot {
        if f.cone <= 0.0 || f.cone > MAX_SPOT_ANGLE {
            return Err(format!("cone must be 0..={MAX_SPOT_ANGLE}, got {}", f.cone));
        }
        if f.falloff < f.cone || f.falloff > MAX_SPOT_ANGLE {
            return Err(format!("falloff must be >= cone ({}) and <= {MAX_SPOT_ANGLE}, got {}", f.cone, f.falloff));
        }
    }

    if f.kind == LightKind::Area {
        for (name, side) in [("width", f.width), ("length", f.length)] {
            if !side.is_finite() || side < MIN_AREA_SIDE || side > MAX_AREA_SIDE {
                return Err(format!("{name} must be {MIN_AREA_SIDE}..={MAX_AREA_SIDE}, got {side}"));
            }
        }
    }

    if let Some(ies) = &f.ies {
        validate_ies(ies)?;
    }

    Ok(())
}

/// A strip's path and output: at least two finite points (at most `MAX_STRIP_POINTS`, consecutive
/// ones at least 0.01 m apart), length `0.05..=MAX_STRIP_LENGTH`, lumens per metre
/// `0..=MAX_LUMENS_PER_METRE`.
fn validate_strip(f: &LightFixture) -> Result<(), String> {
    if f.path.len() < 2 || f.path.len() > MAX_STRIP_POINTS {
        return Err(format!("path must hold 2..={MAX_STRIP_POINTS} points, got {}", f.path.len()));
    }
    if let Some(bad) = f.path.iter().find(|p| !p.is_finite()) {
        return Err(format!("path points must be finite, got {bad:?}"));
    }
    for (a, b) in strip_segments(f) {
        if (b - a).len() < 0.01 {
            return Err(format!("path points must be at least 0.01 m apart, got {a:?} and {b:?}"));
        }
    }
    let len = strip_length(f);
    if !len.is_finite() || !(0.05..=MAX_STRIP_LENGTH).contains(&len) {
        return Err(format!("path length must be 0.05..={MAX_STRIP_LENGTH} m, got {len}"));
    }
    if !f.lumens_per_metre.is_finite() || f.lumens_per_metre < 0.0 || f.lumens_per_metre > MAX_LUMENS_PER_METRE {
        return Err(format!("lumensPerMetre must be 0..={MAX_LUMENS_PER_METRE}, got {}", f.lumens_per_metre));
    }
    Ok(())
}

/// The path's segments in order (a closed path adds the one back to the first point).
fn strip_segments(f: &LightFixture) -> Vec<(Vec2, Vec2)> {
    let mut v: Vec<(Vec2, Vec2)> = f.path.windows(2).map(|w| (w[0], w[1])).collect();
    if f.closed && f.path.len() > 2 {
        if let (Some(last), Some(first)) = (f.path.last(), f.path.first()) {
            v.push((*last, *first));
        }
    }
    v
}

/// Total length of a strip's path (m), counting the closing segment of a closed path; 0 when
/// there are fewer than two points or a coordinate is not finite.
pub fn strip_length(f: &LightFixture) -> f64 {
    if f.path.len() < 2 || f.path.iter().any(|p| !p.is_finite()) {
        return 0.0;
    }
    let l: f64 = strip_segments(f).iter().map(|(a, b)| (*b - *a).len()).sum();
    if l.is_finite() { l } else { 0.0 }
}

/// The emitters of a strip as `(point, emitting normal, lumens)`: every segment is cut into
/// `ceil(length / STRIP_SAMPLE_SPACING)` equal pieces, each a sample at its midpoint at z = level
/// elevation + height, carrying `lumens_per_metre x piece length`. The normal is `(0,0,-1)` for
/// [`StripFacing::Down`], or the horizontal unit vector to the right of the segment for `Out`
/// (outward on a counter-clockwise ring). Empty for a path that is too long or too big, or an
/// unknown height.
pub fn strip_samples(model: &ArchModel, f: &LightFixture) -> Vec<(Vec3, Vec3, f64)> {
    let total = strip_length(f);
    if f.path.len() > MAX_STRIP_POINTS || total <= 0.0 || total > MAX_STRIP_LENGTH {
        return Vec::new();
    }
    let Some(base) = strip_height(model, f) else { return Vec::new() };
    let mut out = Vec::new();
    for (a, b) in strip_segments(f) {
        let d = b - a;
        let len = d.len();
        if !len.is_finite() || len < 1e-9 {
            continue;
        }
        let pieces = ((len / STRIP_SAMPLE_SPACING).ceil() as usize).max(1);
        let normal = match f.facing {
            StripFacing::Down => Vec3::new(0.0, 0.0, -1.0),
            StripFacing::Out => Vec3::new(d.y / len, -d.x / len, 0.0),
        };
        let lumens = f.lumens_per_metre * len / pieces as f64;
        for k in 0..pieces {
            let t = (k as f64 + 0.5) / pieces as f64;
            let p = a + d * t;
            out.push((Vec3::new(p.x, p.y, base), normal, lumens));
        }
    }
    out
}

/// A strip's z: level elevation + height (the room ceiling under the midpoint when no height).
fn strip_height(model: &ArchModel, f: &LightFixture) -> Option<f64> {
    let level = model.levels.iter().rev().find(|l| l.index == f.level).or_else(|| model.levels.last());
    let elevation = level.map(|l| l.elevation).unwrap_or(0.0);
    let ceiling = level.map(|l| l.ceiling_height).unwrap_or(2.4);
    let z = match f.height {
        Some(h) => elevation + h,
        None => {
            let mut c = ceiling;
            for room in &model.rooms {
                if room.level == f.level && room.polygon.len() >= 3 && point_in_polygon(&room.polygon, f.position) {
                    if let Some(ch) = room.ceiling_height.filter(|v| v.is_finite()) {
                        c = ch;
                    }
                    break;
                }
            }
            elevation + c
        }
    };
    z.is_finite().then_some(z)
}

/// The IES table's shape and values (`ies: ...` messages): both angle lists non-empty, finite and
/// at most `MAX_IES_ANGLES` long; the candela table exactly `vertical × horizontal` long and at
/// most `MAX_IES_VALUES`; every candela value finite and >= 0.
fn validate_ies(ies: &IesProfile) -> Result<(), String> {
    for (name, angles) in [("vertical", &ies.vertical), ("horizontal", &ies.horizontal)] {
        if angles.is_empty() {
            return Err(format!("ies: {name} must list at least one angle"));
        }
        if angles.len() > MAX_IES_ANGLES {
            return Err(format!("ies: {name} must list 1..={MAX_IES_ANGLES} angles, got {}", angles.len()));
        }
        if let Some(bad) = angles.iter().find(|a| !a.is_finite()) {
            return Err(format!("ies: {name} angles must be finite, got {bad}"));
        }
    }
    let expected = ies.vertical.len() * ies.horizontal.len();
    if ies.candela.len() != expected {
        return Err(format!(
            "ies: candela must hold {} vertical x {} horizontal = {expected} values, got {}",
            ies.vertical.len(),
            ies.horizontal.len(),
            ies.candela.len()
        ));
    }
    if ies.candela.len() > MAX_IES_VALUES {
        return Err(format!("ies: candela must hold at most {MAX_IES_VALUES} values, got {}", ies.candela.len()));
    }
    if let Some(bad) = ies.candela.iter().find(|c| !c.is_finite() || **c < 0.0) {
        return Err(format!("ies: candela values must be finite and >= 0, got {bad}"));
    }
    Ok(())
}

/// Linear RGB (each 0..=1, the largest component 1) of a black body at `kelvin`, clamped to
/// `MIN_KELVIN..=MAX_KELVIN`; non-finite = 6500 K (white).
pub fn kelvin_to_rgb(kelvin: f64) -> [f64; 3] {
    let k = if kelvin.is_finite() { kelvin.clamp(MIN_KELVIN, MAX_KELVIN) } else { 6500.0 };
    let t = k / 100.0;

    let r = if t <= 66.0 { 255.0 } else { 329.698727446 * (t - 60.0).powf(-0.1332047592) };

    let g = if t <= 66.0 { 99.4708025861 * t.ln() - 161.1195681661 } else { 288.1221695283 * (t - 60.0).powf(-0.0755148492) };

    let b = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.5177312231 * (t - 10.0).ln() - 305.0447927307
    };

    let mut rgb = [r.clamp(0.0, 255.0) / 255.0, g.clamp(0.0, 255.0) / 255.0, b.clamp(0.0, 255.0) / 255.0];

    let max = rgb[0].max(rgb[1]).max(rgb[2]);
    if max > 0.0 {
        rgb[0] /= max;
        rgb[1] /= max;
        rgb[2] /= max;
    }

    rgb
}

/// The world position of the fixture's light point: its plan position at the level's elevation
/// plus its height (see `LightFixture::height` for the defaults by mount). `None` when the level
/// or a coordinate is not finite.
pub fn light_point(model: &ArchModel, f: &LightFixture) -> Option<Vec3> {
    if !f.position.is_finite() {
        return None;
    }

    let level = model.levels.iter().rev().find(|l| l.index == f.level).or_else(|| model.levels.last());

    let elevation = level.map(|l| l.elevation).unwrap_or(0.0);
    let level_ceiling: f64 = level.map(|l| l.ceiling_height).unwrap_or(2.4);

    if !elevation.is_finite() || !level_ceiling.is_finite() {
        return None;
    }

    let mut ceiling_height = level_ceiling;
    for room in &model.rooms {
        if room.level != f.level {
            continue;
        }
        if room.polygon.len() < 3 {
            continue;
        }
        if !point_in_polygon(&room.polygon, f.position) {
            continue;
        }
        let in_hole = room.holes.iter().any(|h| h.len() >= 3 && point_in_polygon(h, f.position));
        if !in_hole {
            if let Some(ch) = room.ceiling_height {
                if ch.is_finite() {
                    ceiling_height = ch;
                }
            }
            break;
        }
    }

    let height = match f.height {
        Some(h) => h,
        None => match f.mount {
            LightMount::Wall => DEFAULT_WALL_HEIGHT,
            LightMount::Floor => DEFAULT_FLOOR_LAMP_HEIGHT,
            LightMount::Ceiling => {
                let drop = match f.shape {
                    FixtureShape::Pendant => DEFAULT_PENDANT_DROP,
                    FixtureShape::SurfaceDome => 0.05,
                    _ => 0.0,
                };
                // Phase 16: the finished ceiling over the fixture (or a soffit's underside).
                let ceiling = crate::ceiling::hang_z(model, f.level, f.position).map_or(ceiling_height, |z| z - elevation);
                ceiling - drop
            }
        },
    };

    if !height.is_finite() {
        return None;
    }

    Some(Vec3::new(f.position.x, f.position.y, elevation + height))
}

/// Unit direction a spot points: `tilt` degrees below the horizontal along `rotation` (90 =
/// straight down `[0,0,-1]`). A point light also returns this (it is unused).
pub fn aim_direction(f: &LightFixture) -> Vec3 {
    if !f.rotation.is_finite() || !f.tilt.is_finite() {
        return Vec3::new(0.0, 0.0, -1.0);
    }
    let r = f.rotation.to_radians();
    let t = f.tilt.to_radians();

    let ct = t.cos();
    let st = t.sin();

    Vec3::new(ct * r.cos(), ct * r.sin(), -st)
}

/// Light positions of an even `rows` × `cols` grid over `ring`'s bounding box: cell centres
/// (x = min.x + (i + 0.5)·w/cols, y = min.y + (j + 0.5)·h/rows), kept only when inside `ring`, in
/// row-major order from the minimum corner. Empty for rows or cols outside `1..=MAX_FIXTURE_GRID`
/// or a ring of fewer than three finite points.
pub fn grid_positions(ring: &[Vec2], rows: u32, cols: u32) -> Vec<Vec2> {
    if rows < 1 || rows > MAX_FIXTURE_GRID || cols < 1 || cols > MAX_FIXTURE_GRID {
        return Vec::new();
    }
    if ring.len() < 3 {
        return Vec::new();
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for p in ring {
        if !p.is_finite() {
            return Vec::new();
        }
        if p.x < min_x {
            min_x = p.x;
        }
        if p.y < min_y {
            min_y = p.y;
        }
        if p.x > max_x {
            max_x = p.x;
        }
        if p.y > max_y {
            max_y = p.y;
        }
    }

    let w = max_x - min_x;
    let h = max_y - min_y;

    if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
        return Vec::new();
    }

    let mut out = Vec::new();
    for j in 0..rows {
        let y = min_y + (j as f64 + 0.5) * h / (rows as f64);
        for i in 0..cols {
            let x = min_x + (i as f64 + 0.5) * w / (cols as f64);
            let p = Vec2::new(x, y);
            if point_in_polygon(ring, p) {
                out.push(p);
            }
        }
    }

    out
}

/// Plan symbol polylines of a fixture (in `crate::light_symbol`, filled by `p10-arch-light-symbol`).
pub use crate::light_symbol::plan_symbol;

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::ArchId;

    fn strip(path: &[(f64, f64)], facing: StripFacing) -> LightFixture {
        let t = find_type("LED strip").unwrap_or_else(|| catalogue().remove(0));
        LightFixture {
            path: path.iter().map(|p| Vec2::new(p.0, p.1)).collect(),
            height: Some(2.4),
            facing,
            ..fixture_from_type(ArchId(0), &t, Vec2::new(0.0, 0.0))
        }
    }

    #[test]
    fn strip_samples_spacing_validate_and_catalogue() {
        let t = find_type("led STRIP").unwrap_or_else(|| catalogue().remove(0));
        assert_eq!((t.shape, t.kind, t.mount, t.kelvin, t.diameter), (FixtureShape::Strip, LightKind::Strip, LightMount::Ceiling, 3000.0, 0.012));
        let f = strip(&[(1.0, 2.0), (3.0, 2.0)], StripFacing::Down);
        assert_eq!(f.lumens_per_metre, 500.0);
        assert!((strip_length(&f) - 2.0).abs() < 1e-12);
        assert!((intensity_cd(&f) - 500.0 / std::f64::consts::PI).abs() < 1e-9);
        assert_eq!(validate_fixture(&f), Ok(()));
        let model = ArchModel::default();
        let samples = strip_samples(&model, &f);
        // 2 m at 0.15 m spacing = ceil(13.33) = 14 pieces at their midpoints.
        assert_eq!(samples.len(), 14);
        assert!((samples[0].0.x - (1.0 + 1.0 / 14.0)).abs() < 1e-9 && (samples[0].0.z - 2.4).abs() < 1e-9);
        assert_eq!(samples[0].1, Vec3::new(0.0, 0.0, -1.0));
        let total: f64 = samples.iter().map(|s| s.2).sum();
        assert!((total - 1000.0).abs() < 1e-6, "{total}");
        // Refusals name the field.
        let bad = |g: LightFixture| validate_fixture(&g).err().unwrap_or_default();
        assert!(bad(LightFixture { path: vec![Vec2::new(0.0, 0.0)], ..f.clone() }).contains("path"));
        assert!(bad(LightFixture { lumens_per_metre: -5.0, ..f.clone() }).contains("lumensPerMetre"));
        assert!(bad(LightFixture { lumens_per_metre: 10_001.0, ..f.clone() }).contains("lumensPerMetre"));
        assert!(bad(LightFixture { path: vec![Vec2::new(0.0, 0.0), Vec2::new(0.005, 0.0)], ..f.clone() }).contains("0.01"));
        assert!(bad(LightFixture { path: vec![Vec2::new(0.0, 0.0), Vec2::new(f64::NAN, 0.0)], ..f.clone() }).contains("finite"));
        assert!(bad(LightFixture { path: vec![Vec2::new(0.0, 0.0), Vec2::new(101.0, 0.0)], ..f.clone() }).contains("length"));
        assert!(bad(LightFixture { height: Some(101.0), ..f.clone() }).contains("height"));
        assert!(bad(LightFixture { path: vec![Vec2::new(0.0, 0.0); 257], ..f.clone() }).contains("path"));
    }

    #[test]
    fn out_facing_normal_right_of_path() {
        let model = ArchModel::default();
        // A counter-clockwise 2 x 2 ring: every normal points away from the centre.
        let mut f = strip(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)], StripFacing::Out);
        f.closed = true;
        assert!((strip_length(&f) - 8.0).abs() < 1e-12);
        let samples = strip_samples(&model, &f);
        assert!(!samples.is_empty());
        for (p, n, _) in &samples {
            assert!(n.z == 0.0 && (n.len() - 1.0).abs() < 1e-9);
            let outward = Vec2::new(p.x - 1.0, p.y - 1.0);
            assert!(outward.x * n.x + outward.y * n.y > 0.0, "{p:?} {n:?}");
        }
        // Heading +x, right is -y.
        let line = strip(&[(0.0, 0.0), (1.0, 0.0)], StripFacing::Out);
        assert_eq!(strip_samples(&model, &line)[0].1, Vec3::new(0.0, -1.0, 0.0));
    }

    #[test]
    fn the_pot_light_is_800_lumens_in_a_60_degree_cone() {
        let t = find_type(" recessed POT light ").unwrap_or_else(|| catalogue().remove(1));
        assert_eq!((t.kind, t.lumens, t.kelvin, t.cone, t.falloff), (LightKind::Spot, 800.0, 3000.0, 60.0, 60.0));
        assert_eq!(catalogue().len(), 7);
        assert!(find_type("Pendant").is_some() && find_type("Wall sconce").is_some() && find_type("Surface ceiling fixture").is_some());
    }

    #[test]
    fn a_60_degree_cone_of_800_lumens_is_950_candela() {
        assert!((cone_solid_angle(60.0) - 0.841_787).abs() < 1e-6);
        let t = find_type("Recessed pot light").unwrap_or_else(|| catalogue().remove(0));
        let f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));
        assert!((intensity_cd(&f) - 950.36).abs() < 0.01, "{}", intensity_cd(&f));
        let off = LightFixture { on: false, ..f.clone() };
        assert_eq!(intensity_cd(&off), 0.0);
        let point = LightFixture { kind: LightKind::Point, lumens: 4.0 * std::f64::consts::PI, ..f };
        assert!((intensity_cd(&point) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn validate_fixture_names_the_bad_field() {
        let t = find_type("Recessed pot light").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));

        f.lumens = -5.0;
        let err = validate_fixture(&f).unwrap_err();
        assert!(err.contains("lumens"), "{err}");
        assert!(err.contains("-5"), "{err}");

        f.lumens = 800.0;
        f.kelvin = 500.0;
        let err = validate_fixture(&f).unwrap_err();
        assert!(err.contains("kelvin"), "{err}");
        assert!(err.contains("500"), "{err}");

        f.kelvin = 3000.0;
        f.tilt = 100.0;
        let err = validate_fixture(&f).unwrap_err();
        assert!(err.contains("tilt"), "{err}");
        assert!(err.contains("100"), "{err}");
    }

    #[test]
    fn validate_fixture_refuses_falloff_below_cone() {
        let t = find_type("Recessed pot light").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));
        f.cone = 60.0;
        f.falloff = 40.0;
        let err = validate_fixture(&f).unwrap_err();
        assert!(err.contains("falloff"), "{err}");
        assert!(err.contains("40"), "{err}");
    }

    #[test]
    fn kelvin_3000_is_warm_and_6500_is_white() {
        let rgb = kelvin_to_rgb(3000.0);
        assert!((rgb[0] - 1.0).abs() < 0.01);
        assert!((rgb[1] - 0.695).abs() < 0.005, "g: {}", rgb[1]);
        assert!((rgb[2] - 0.431).abs() < 0.005, "b: {}", rgb[2]);

        let rgb = kelvin_to_rgb(6500.0);
        assert!(rgb[0] > 0.97);
        assert!(rgb[1] > 0.97);
        assert!(rgb[2] > 0.97);

        let rgb = kelvin_to_rgb(10000.0);
        assert!((rgb[2] - 1.0).abs() < 0.01);
        assert!(rgb[0] > 0.78 && rgb[0] < 0.80, "r: {}", rgb[0]);
    }

    #[test]
    fn light_point_takes_the_room_ceiling_height() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Level::default() });
        model.rooms.push(Room {
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0)],
            holes: vec![],
            ceiling_height: Some(3.0),
            ..Room::default()
        });

        let t = find_type("Recessed pot light").unwrap();
        let f = fixture_from_type(ArchId(0), &t, Vec2::new(5.0, 5.0));
        let p = light_point(&model, &f).unwrap();
        assert!((p.z - 3.0).abs() < 1e-6, "z: {}", p.z);
    }

    /// A ceiling light sits at the finished ceiling: the rough ceiling minus the room's stack.
    #[test]
    fn ceiling_light_uses_the_finished_ceiling() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Level::default() });
        model.rooms.push(Room {
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0)],
            ceiling_stack: vec![
                cadcraft_doc::arch::CeilingLayer { material: Some("drywall".into()), thickness: 0.0127, ..Default::default() },
                cadcraft_doc::arch::CeilingLayer { material: None, thickness: 0.2, ..Default::default() },
                cadcraft_doc::arch::CeilingLayer { material: Some("ceiling-tile".into()), thickness: 0.016, ..Default::default() },
            ],
            ..Room::default()
        });
        let t = find_type("Recessed pot light").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(5.0, 5.0));
        let p = light_point(&model, &f).unwrap();
        assert!((p.z - 2.1713).abs() < 1e-9, "z: {}", p.z);
        f.shape = FixtureShape::Pendant;
        let p = light_point(&model, &f).unwrap();
        assert!((p.z - (2.1713 - DEFAULT_PENDANT_DROP)).abs() < 1e-9, "z: {}", p.z);
        // Outside every room it keeps the level's ceiling; an explicit height wins.
        let out = fixture_from_type(ArchId(1), &t, Vec2::new(20.0, 20.0));
        assert!((light_point(&model, &out).unwrap().z - 2.4).abs() < 1e-9);
        f.height = Some(2.0);
        assert!((light_point(&model, &f).unwrap().z - 2.0).abs() < 1e-9);
    }

    #[test]
    fn light_point_defaults_by_mount_and_shape() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Level::default() });

        let t = find_type("Recessed pot light").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));
        f.mount = LightMount::Ceiling;
        f.shape = FixtureShape::RecessedCan;
        let p = light_point(&model, &f).unwrap();
        assert!((p.z - 2.4).abs() < 1e-6);

        f.shape = FixtureShape::Pendant;
        let p = light_point(&model, &f).unwrap();
        assert!((p.z - (2.4 - 0.6)).abs() < 1e-6);

        f.shape = FixtureShape::SurfaceDome;
        let p = light_point(&model, &f).unwrap();
        assert!((p.z - (2.4 - 0.05)).abs() < 1e-6);

        f.mount = LightMount::Wall;
        let p = light_point(&model, &f).unwrap();
        assert!((p.z - 1.8).abs() < 1e-6);

        f.mount = LightMount::Floor;
        let p = light_point(&model, &f).unwrap();
        assert!((p.z - 1.5).abs() < 1e-6);
    }

    #[test]
    fn light_point_honours_level_elevation_and_height() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 1, elevation: 2.7, ceiling_height: 2.4, ..Level::default() });

        let t = find_type("Recessed pot light").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));
        f.level = 1;
        f.height = Some(1.0);

        let p = light_point(&model, &f).unwrap();
        assert!((p.z - (2.7 + 1.0)).abs() < 1e-6);
    }

    #[test]
    fn aim_direction_straight_down_and_level() {
        let t = find_type("Recessed pot light").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));

        f.tilt = 90.0;
        f.rotation = 0.0;
        let d = aim_direction(&f);
        assert!((d.x).abs() < 1e-12);
        assert!((d.y).abs() < 1e-12);
        assert!((d.z + 1.0).abs() < 1e-12);

        f.tilt = 0.0;
        f.rotation = 90.0;
        let d = aim_direction(&f);
        assert!((d.x).abs() < 1e-12);
        assert!((d.y - 1.0).abs() < 1e-12);
        assert!((d.z).abs() < 1e-12);
    }

    #[test]
    fn grid_positions_two_by_two_in_a_square_room() {
        let ring = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        let pts = grid_positions(&ring, 2, 2);
        assert_eq!(pts.len(), 4);
        assert_eq!(pts[0], Vec2::new(1.0, 1.0));
        assert_eq!(pts[1], Vec2::new(3.0, 1.0));
        assert_eq!(pts[2], Vec2::new(1.0, 3.0));
        assert_eq!(pts[3], Vec2::new(3.0, 3.0));
    }

    #[test]
    fn grid_positions_skip_cells_outside_an_l_shaped_room() {
        // L shape: 0..4 x 0..4 minus 2..4 x 2..4
        let ring = vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 2.0), Vec2::new(2.0, 2.0), Vec2::new(2.0, 4.0), Vec2::new(0.0, 4.0)];
        // 2x2 grid over 0..4 x 0..4
        // Centers: (1,1), (3,1), (1,3), (3,3)
        // (1,1) inside
        // (3,1) inside
        // (1,3) inside
        // (3,3) outside (in the cutout)
        let pts = grid_positions(&ring, 2, 2);
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[0], Vec2::new(1.0, 1.0));
        assert_eq!(pts[1], Vec2::new(3.0, 1.0));
        assert_eq!(pts[2], Vec2::new(1.0, 3.0));
    }

    #[test]
    fn grid_positions_refuse_zero_and_too_many_rows() {
        let ring = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0)];
        assert!(grid_positions(&ring, 0, 1).is_empty());
        assert!(grid_positions(&ring, 1, 0).is_empty());
        assert!(grid_positions(&ring, MAX_FIXTURE_GRID + 1, 1).is_empty());
        assert!(grid_positions(&ring, 1, MAX_FIXTURE_GRID + 1).is_empty());
    }

    #[test]
    fn validate_area_sizes_name_the_field() {
        let t = find_type("LED panel 600").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));
        assert_eq!((f.width, f.length), (0.6, 0.6));
        assert!(validate_fixture(&f).is_ok(), "the catalogue panel is valid");

        f.width = 5.0;
        let err = validate_fixture(&f).unwrap_err();
        assert!(err.contains("width") && !err.contains("length"), "{err}");
        assert_eq!(err, "width must be 0.01..=4, got 5");

        // The width is checked first: both bad names the width.
        f.length = 0.001;
        let err = validate_fixture(&f).unwrap_err();
        assert!(err.starts_with("width"), "{err}");

        f.width = 0.6;
        let err = validate_fixture(&f).unwrap_err();
        assert!(err.contains("length") && !err.contains("width"), "{err}");
        assert!(err.contains("0.001"), "{err}");

        f.length = f64::NAN;
        assert!(validate_fixture(&f).unwrap_err().contains("length"));
        f.length = 0.0;
        assert!(validate_fixture(&f).unwrap_err().contains("length"), "an area of zero length is not an area");

        // The limits themselves are accepted.
        f.width = MIN_AREA_SIDE;
        f.length = MAX_AREA_SIDE;
        assert!(validate_fixture(&f).is_ok());

        // A point or spot light ignores its (unused) size.
        let mut spot = fixture_from_type(ArchId(1), &find_type("Recessed pot light").unwrap(), Vec2::new(1.0, 1.0));
        spot.width = 99.0;
        spot.length = -3.0;
        assert!(validate_fixture(&spot).is_ok());
    }

    #[test]
    fn validate_ies_tables() {
        let t = find_type("Recessed pot light").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));
        let good = IesProfile {
            name: "test".to_string(),
            vertical: vec![0.0, 90.0, 180.0],
            horizontal: vec![0.0, 180.0],
            candela: vec![100.0, 50.0, 0.0, 100.0, 50.0, 0.0],
            ..IesProfile::default()
        };
        f.ies = Some(good.clone());
        assert!(validate_fixture(&f).is_ok());

        let refused = |mutate: &dyn Fn(&mut IesProfile)| {
            let mut p = good.clone();
            mutate(&mut p);
            let mut g = f.clone();
            g.ies = Some(p);
            validate_fixture(&g).unwrap_err()
        };

        let err = refused(&|p| p.vertical.clear());
        assert!(err.starts_with("ies:") && err.contains("vertical"), "{err}");
        let err = refused(&|p| p.horizontal.clear());
        assert!(err.starts_with("ies:") && err.contains("horizontal"), "{err}");
        let err = refused(&|p| p.vertical = (0..=MAX_IES_ANGLES).map(|i| i as f64 * 0.1).collect());
        assert!(err.starts_with("ies:") && err.contains("vertical") && err.contains(&MAX_IES_ANGLES.to_string()), "{err}");
        let err = refused(&|p| p.horizontal = (0..=MAX_IES_ANGLES).map(|i| i as f64 * 0.3).collect());
        assert!(err.starts_with("ies:") && err.contains("horizontal"), "{err}");
        let err = refused(&|p| {
            p.candela.pop();
        });
        assert!(err.starts_with("ies:") && err.contains("candela"), "{err}");
        let err = refused(&|p| p.candela.push(1.0));
        assert!(err.starts_with("ies:") && err.contains("candela"), "{err}");
        let err = refused(&|p| p.candela[2] = -1.0);
        assert!(err.starts_with("ies:") && err.contains("candela") && err.contains("-1"), "{err}");
        let err = refused(&|p| p.candela[2] = f64::NAN);
        assert!(err.starts_with("ies:") && err.contains("candela"), "{err}");
        let err = refused(&|p| p.candela[0] = f64::INFINITY);
        assert!(err.starts_with("ies:") && err.contains("candela"), "{err}");
        let err = refused(&|p| p.vertical[1] = f64::NAN);
        assert!(err.starts_with("ies:") && err.contains("vertical"), "{err}");

        // Past MAX_IES_VALUES with matching lengths (1024 x 65 = 66560 > 65536).
        let err = refused(&|p| {
            p.vertical = (0..MAX_IES_ANGLES).map(|i| i as f64 * 0.17).collect();
            p.horizontal = (0..65).map(|i| i as f64 * 5.0).collect();
            p.candela = vec![1.0; MAX_IES_ANGLES * 65];
        });
        assert!(err.starts_with("ies:") && err.contains(&MAX_IES_VALUES.to_string()), "{err}");

        // The largest legal table is accepted (1024 x 64 = 65536).
        let mut p = good.clone();
        p.vertical = (0..MAX_IES_ANGLES).map(|i| i as f64 * 0.17).collect();
        p.horizontal = (0..64).map(|i| i as f64 * 5.0).collect();
        p.candela = vec![1.0; MAX_IES_ANGLES * 64];
        f.ies = Some(p);
        assert!(validate_fixture(&f).is_ok());
    }

    #[test]
    fn a_panel_takes_the_ceiling_height() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Level::default() });
        model.rooms.push(Room {
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
            holes: vec![],
            ceiling_height: None,
            ..Room::default()
        });

        let t = find_type("LED panel 600").unwrap();
        let f = fixture_from_type(ArchId(0), &t, Vec2::new(2.0, 2.0));
        assert_eq!((f.shape, f.mount), (FixtureShape::Panel, LightMount::Ceiling));
        let p = light_point(&model, &f).unwrap();
        assert!((p.z - 2.4).abs() < 1e-9, "z: {}", p.z);
        assert_eq!((p.x, p.y), (2.0, 2.0));

        // A room with its own ceiling height moves the panel with it.
        model.rooms[0].ceiling_height = Some(3.0);
        let p = light_point(&model, &f).unwrap();
        assert!((p.z - 3.0).abs() < 1e-9, "z: {}", p.z);
    }

    #[test]
    fn validate_area_nan_width_names_width() {
        let t = find_type("LED panel 600").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));
        f.width = f64::NAN;
        let err = validate_fixture(&f).unwrap_err();
        assert!(err.contains("width"), "{err}");
        assert!(err.contains("NaN"), "{err}");
    }

    #[test]
    fn validate_area_zero_width_names_width() {
        let t = find_type("LED panel 600").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));
        f.width = 0.0;
        let err = validate_fixture(&f).unwrap_err();
        assert!(err.contains("width"), "{err}");
        assert!(err.contains("0"), "{err}");
    }

    #[test]
    fn validate_area_infinity_length_names_length() {
        let t = find_type("LED panel 600").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));
        f.length = f64::INFINITY;
        let err = validate_fixture(&f).unwrap_err();
        assert!(err.contains("length"), "{err}");
        assert!(err.contains("inf"), "{err}");
    }

    #[test]
    fn validate_area_both_bad_names_width_first() {
        let t = find_type("LED panel 600").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));
        f.width = 10.0;
        f.length = 0.001;
        let err = validate_fixture(&f).unwrap_err();
        assert!(err.contains("width"), "should name width first: {err}");
        assert!(!err.contains("length"), "should not name length yet: {err}");
    }

    #[test]
    fn validate_area_negative_width() {
        let t = find_type("LED panel 600").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));
        f.width = -0.5;
        let err = validate_fixture(&f).unwrap_err();
        assert!(err.contains("width"), "{err}");
        assert!(err.contains("-0.5"), "{err}");
    }

    #[test]
    fn ies_with_zero_candela_values_is_valid() {
        let t = find_type("Recessed pot light").unwrap();
        let mut f = fixture_from_type(ArchId(0), &t, Vec2::new(1.0, 1.0));
        let p = IesProfile {
            name: "test".to_string(),
            vertical: vec![0.0, 90.0],
            horizontal: vec![0.0, 180.0],
            candela: vec![0.0, 0.0, 0.0, 0.0],
            ..IesProfile::default()
        };
        f.ies = Some(p);
        assert!(validate_fixture(&f).is_ok());
    }

    #[test]
    fn panel_light_point_in_room_with_hole() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Level::default() });
        model.rooms.push(Room {
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0)],
            holes: vec![vec![Vec2::new(4.0, 4.0), Vec2::new(6.0, 4.0), Vec2::new(6.0, 6.0), Vec2::new(4.0, 6.0)]],
            ceiling_height: Some(3.5),
            ..Room::default()
        });

        let t = find_type("LED panel 600").unwrap();
        // Panel in the room but not in the hole
        let f = fixture_from_type(ArchId(0), &t, Vec2::new(2.0, 2.0));
        let p = light_point(&model, &f).unwrap();
        assert!((p.z - 3.5).abs() < 1e-9, "z: {}", p.z);

        // Panel in the hole falls back to the level ceiling height
        let f2 = fixture_from_type(ArchId(1), &t, Vec2::new(5.0, 5.0));
        let p2 = light_point(&model, &f2).unwrap();
        assert!((p2.z - 2.4).abs() < 1e-9, "z: {}", p2.z);
    }

    fn tray_model() -> ArchModel {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Level::default() });
        model.push_room(Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(6.0, 0.0), Vec2::new(6.0, 5.0), Vec2::new(0.0, 5.0)],
            ..Room::default()
        });
        model.push_tray(cadcraft_doc::arch::Tray { room: ArchId(0), inset: 0.6, depth: 0.3, ..Default::default() });
        model
    }

    #[test]
    fn tray_strip_path_insets_the_outline_ccw() {
        let model = tray_model();
        let (level, ring, height) = tray_strip_path(&model, "tr0", 0.05).unwrap();
        assert_eq!(level, 0);
        let area: f64 = ring.iter().zip(ring.iter().cycle().skip(1)).map(|(a, b)| a.x * b.y - b.x * a.y).sum();
        assert!(area > 0.0, "counter-clockwise");
        let (x0, x1) = ring.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
        let (y0, y1) = ring.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        assert!((x0 - 0.65).abs() < 1e-6 && (x1 - 5.35).abs() < 1e-6 && (y0 - 0.65).abs() < 1e-6 && (y1 - 4.35).abs() < 1e-6, "{ring:?}");
        let outside = crate::ceiling::finished_ceiling_at(&model, 0, Vec2::new(0.55, 2.5)).unwrap();
        assert!((height - (outside + 0.02)).abs() < 1e-9, "{height} vs {outside}");
        assert!(tray_strip_path(&model, "tr9", 0.05).unwrap_err().contains("tr9"));
        assert!(tray_strip_path(&model, "tr0", 2.0).unwrap_err().contains("tr0"));
    }

    #[test]
    fn tray_strip_path_inset_zero_returns_outline() {
        let model = tray_model();
        let (_, ring, height) = tray_strip_path(&model, "tr0", 0.0).unwrap();
        // With inset 0, the ring is the tray outline itself (0.6 inset from the room)
        let (x0, x1) = ring.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
        let (y0, y1) = ring.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        assert!((x0 - 0.6).abs() < 1e-6 && (x1 - 5.4).abs() < 1e-6, "x range {x0}..{x1}");
        assert!((y0 - 0.6).abs() < 1e-6 && (y1 - 4.4).abs() < 1e-6, "y range {y0}..{y1}");
        assert!(height.is_finite());
    }

    #[test]
    fn tray_strip_path_refuses_non_finite_inset() {
        let model = tray_model();
        let err = tray_strip_path(&model, "tr0", f64::NAN).unwrap_err();
        assert!(err.contains("inset") && err.contains("tr0"), "{err}");
        let err = tray_strip_path(&model, "tr0", f64::INFINITY).unwrap_err();
        assert!(err.contains("inset"), "{err}");
        let err = tray_strip_path(&model, "tr0", -0.1).unwrap_err();
        assert!(err.contains("inset") && err.contains("-0.1"), "{err}");
    }

    #[test]
    fn tray_strip_path_polygon_tray() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        model.push_room(Room {
            id: ArchId(0),
            level: 0,
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0)],
            ..Room::default()
        });
        // A drawn polygon tray (not room-based)
        model.push_tray(cadcraft_doc::arch::Tray {
            room: ArchId(0),
            polygon: vec![Vec2::new(2.0, 2.0), Vec2::new(8.0, 2.0), Vec2::new(8.0, 6.0), Vec2::new(2.0, 6.0)],
            depth: 0.25,
            ..Default::default()
        });
        let (level, ring, height) = tray_strip_path(&model, "tr0", 0.05).unwrap();
        assert_eq!(level, 0);
        // The ring should be inset 0.05 from the polygon (2.05..7.95 x, 2.05..5.95 y)
        let (x0, x1) = ring.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
        let (y0, y1) = ring.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        assert!((x0 - 2.05).abs() < 1e-6 && (x1 - 7.95).abs() < 1e-6, "x range {x0}..{x1}");
        assert!((y0 - 2.05).abs() < 1e-6 && (y1 - 5.95).abs() < 1e-6, "y range {y0}..{y1}");
        assert!(height.is_finite());
    }
}
