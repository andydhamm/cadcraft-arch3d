//! Fixture lights on the GPU (phase 10): which lights are drawn and shadowed, their uniform, and
//! the projection of their shadow maps.
//!
//! **Group 2 of the lit pipelines** (`crate::lit`), all read in the fragment stage:
//! - binding 0: `FrameU` ([`FRAME_UNIFORM_SIZE`] bytes): `sun_vp: mat4x4<f32>` (the sun map's
//!   light matrix), `sun: vec4<f32>` (x = depth bias, y = 1 when a sun map is bound, z = 1 when
//!   the sun is on, w = 0), `params: vec4<f32>` (x = exposure in display units per lux, y = 1 to
//!   tone-map (Finalize), z = PCF radius in texels (0 = one tap), w = shadow-atlas layer size),
//!   `ext: vec4<f32>` (x = ambient illuminance in lux, y = 1 when `sh` is valid),
//!   `sh: array<vec4<f32>, 9>` (spherical-harmonic ambient, rgb in xyz); 256 bytes in all.
//! - binding 1: the sun map, `texture_depth_2d` (a 1×1 map when there is none).
//! - binding 2: `LightsU` ([`LIGHTS_UNIFORM_SIZE`] bytes): `count: vec4<u32>` (x = lights
//!   drawn), `lights: array<GpuLight, 64>`, `spot_vp: array<mat4x4<f32>, 64>` (light i's spot
//!   matrix in slot i), `strip_segs: array<StripSeg, 128>` (the segments of the strip lights,
//!   [`StripSeg`], 48 bytes each).
//! - binding 3: the shadow atlas, `texture_depth_2d_array` (one layer per spot, six per point
//!   light; a 1×1×1 array when nothing is shadowed).
//!
//! A [`GpuLight`] is four `vec4<f32>`: `pos_range` (xyz light point, w = far plane of its map),
//! `dir_cos_outer` (xyz unit spot aim, w = cos of half the falloff, −2 for a point light),
//! `color_cos_inner` (xyz = colour × intensity in candela, w = cos of half the cone), `shadow`
//! (x = first atlas layer or −1 when unshadowed, y = 1 for a cube (point light), z = near
//! plane, w = depth bias in metres).
//!
//! A **strip light** is one [`GpuLight`] (a marker: `dir_cos_outer.w` = −3, `pos_range.xyz` the
//! mean of its samples, `color_cos_inner.xyz` the kelvin colour, `shadow.x` = −1 so it is never
//! shadowed in the raster, `shadow.z`/`shadow.w` = its first segment and segment count) plus its
//! [`StripSeg`]s; it counts once against [`MAX_GPU_LIGHTS`]. The shader sums its pieces (at most
//! 0.15 m, at their midpoints); shadows from a strip are the path tracer's.
//!
//! Shadow maps store the perspective depth `far/(far−near)·(1 − near/z)` of the distance `z`
//! along the face's forward axis; `shaders/light_math.wgsl` turns it back into metres with
//! [`linear_depth`] and compares distances. The six cube faces follow [`cube_face`].
//!
//! Filled here: the constants, [`cube_face`], [`cube_face_view_proj`], [`spot_view_proj`],
//! [`linear_depth`]. Stubs filled by `p10-r3d-light-pack`: [`shadow_priority`],
//! [`select_shadowed`], [`select_drawn`], [`pack_lights`].

use std::cmp::Ordering;

use cadcraft_arch3d::light::{LightKind, SceneLight, StripSample};
use cadcraft_geom::Vec3;

/// Most lights one frame draws (the uniform's array length).
pub const MAX_GPU_LIGHTS: usize = 64;
/// Lights shadowed in the interactive view unless the caller asks for another budget.
pub const DEFAULT_SHADOW_BUDGET: usize = 8;
/// Shadow-map edge (texels) per atlas layer in the interactive view and in Finalize Render.
pub const INTERACTIVE_SHADOW_SIZE: u32 = 512;
pub const FINAL_SHADOW_SIZE: u32 = 2048;
/// Most atlas layers (the WebGL2 downlevel `max_texture_array_layers`).
pub const MAX_SHADOW_LAYERS: u32 = 256;
/// Near plane of every fixture shadow map (m).
pub const SHADOW_NEAR: f32 = 0.05;
/// Display units per lux: a white diffuse face under 100 lx shows 100 · 0.01 / π ≈ 0.32.
pub const DEFAULT_EXPOSURE: f32 = 0.01;
/// Ambient level with the sun off (night).
pub const NIGHT_AMBIENT: f32 = 0.02;
/// Bytes of `FrameU` (binding 0) and `LightsU` (binding 2).
pub const FRAME_UNIFORM_SIZE: u64 = 256;
/// Most strip segments one frame draws (the length of `LightsU::strip_segs`).
pub const MAX_STRIP_SEGS: usize = 128;
/// Bytes of one [`StripSeg`].
pub const STRIP_SEG_SIZE: usize = 48;
/// Byte offset of `strip_segs` in `LightsU`.
pub const STRIP_SEGS_OFFSET: usize = 16 + 64 * MAX_GPU_LIGHTS + 64 * MAX_GPU_LIGHTS;
/// 16 + 64 × 64 + 64 × 64 + 128 × 48 = 14 352 bytes, within the WebGL2 uniform limit of 16 384.
pub const LIGHTS_UNIFORM_SIZE: u64 = (STRIP_SEGS_OFFSET + STRIP_SEG_SIZE * MAX_STRIP_SEGS) as u64;
/// Longest piece (m) a strip segment is summed in: the spacing of the strip's samples.
pub const STRIP_PIECE: f64 = cadcraft_doc::arch::STRIP_SAMPLE_SPACING;
/// `dir_cos_outer.w` of a strip light's marker.
pub const STRIP_MARKER: f32 = -3.0;
/// Most pieces one segment is summed in (a guard for the shader's loop).
pub const MAX_STRIP_PIECES: u32 = 1024;

/// One straight run of a strip light in the uniform (`StripSeg` in `lit.wgsl`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StripSeg {
    /// Start point; w = candela per metre of strip along the emitting normal.
    pub a: [f32; 4],
    /// End point; w = number of equal pieces the run is summed in.
    pub b: [f32; 4],
    /// Unit emitting normal; w unused.
    pub n: [f32; 4],
}

/// A straight run of a strip light: `pieces` equal pieces from `start` to `end`, each a Lambertian
/// emitter of `cd_per_metre` × its length along `normal`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StripRun {
    pub start: Vec3,
    pub end: Vec3,
    pub normal: Vec3,
    pub cd_per_metre: f64,
    pub pieces: u32,
}

/// The runs of a strip light, rebuilt from its samples: consecutive samples with the same normal
/// and intensity at an equal step are one run (spanning half a step beyond its end samples). A
/// lone sample becomes a run of [`STRIP_PIECE`] across its normal, keeping its lumens. Empty for a
/// light without samples.
pub fn strip_runs(light: &SceneLight) -> Vec<StripRun> {
    let samples: &[StripSample] = light.strip.as_ref().map_or(&[], |v| v.as_slice());
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(first) = samples.get(i) {
        let mut count = 1usize;
        let mut step = Vec3::new(0.0, 0.0, 0.0);
        while let (Some(prev), Some(next)) = (samples.get(i + count - 1), samples.get(i + count)) {
            let d = next.position - prev.position;
            let same =
                (next.normal - first.normal).len() < 1e-6 && (next.intensity - first.intensity).abs() <= 1e-9 * first.intensity.abs().max(1e-30);
            let joins = if count == 1 { d.len().is_finite() } else { (d - step).len() < 1e-7 };
            if !(same && joins) {
                break;
            }
            step = d;
            count += 1;
        }
        let normal = first.normal;
        let last = samples.get(i + count - 1).unwrap_or(first);
        if count == 1 {
            let c = normal.cross(Vec3::new(0.0, 0.0, 1.0));
            let along = if c.len() > 1e-6 { c * (1.0 / c.len()) } else { Vec3::new(1.0, 0.0, 0.0) };
            let half = along * (STRIP_PIECE * 0.5);
            out.push(StripRun {
                start: first.position - half,
                end: first.position + half,
                normal,
                cd_per_metre: first.intensity / STRIP_PIECE,
                pieces: 1,
            });
        } else {
            out.push(StripRun {
                start: first.position - step * 0.5,
                end: last.position + step * 0.5,
                normal,
                cd_per_metre: first.intensity / step.len(),
                pieces: count as u32,
            });
        }
        i += count;
    }
    out
}

/// How many `strip_segs` entries a light needs (0 for anything but a strip).
pub fn strip_seg_count(light: &SceneLight) -> usize {
    if light.kind == LightKind::Strip { strip_runs(light).len() } else { 0 }
}

/// The mean of a strip's sample positions (its light point when it has none).
fn strip_centre(light: &SceneLight) -> Vec3 {
    match light.strip.as_ref().filter(|v| !v.is_empty()) {
        Some(v) => {
            let mut sum = Vec3::new(0.0, 0.0, 0.0);
            for s in v.iter() {
                sum = sum + s.position;
            }
            sum * (1.0 / v.len() as f64)
        }
        None => light.position,
    }
}

/// The uniform's segment for a run, its candela per metre scaled by `scale`.
pub fn strip_seg(run: &StripRun, scale: f64) -> StripSeg {
    let f = |v: Vec3, w: f64| [v.x as f32, v.y as f32, v.z as f32, w as f32];
    StripSeg { a: f(run.start, run.cd_per_metre * scale), b: f(run.end, f64::from(run.pieces)), n: f(run.normal, 0.0) }
}

/// CPU mirror of the strip sum in `fixture_light` (`lit.wgsl`): the illuminance (lux, white) the
/// `segs` give a surface at `point` with unit `normal`. Every piece (at most [`STRIP_PIECE`]), at
/// its midpoint, adds `I × piece length × max(0, n_e·−l) × max(0, n·l) / d²`, `l` the unit vector
/// from the point to the piece, `I` the segment's candela per metre.
pub fn strip_irradiance_cpu(segs: &[StripSeg], point: [f64; 3], normal: [f64; 3]) -> f64 {
    let mut total = 0.0;
    for s in segs {
        let pieces = f64::from(s.b[3]).round().clamp(1.0, f64::from(MAX_STRIP_PIECES)) as u32;
        let a = [f64::from(s.a[0]), f64::from(s.a[1]), f64::from(s.a[2])];
        let d = [f64::from(s.b[0]) - a[0], f64::from(s.b[1]) - a[1], f64::from(s.b[2]) - a[2]];
        let piece_len = dot(d, d).sqrt() / f64::from(pieces);
        let ne = [f64::from(s.n[0]), f64::from(s.n[1]), f64::from(s.n[2])];
        for k in 0..pieces {
            let t = (f64::from(k) + 0.5) / f64::from(pieces);
            let w = [a[0] + d[0] * t - point[0], a[1] + d[1] * t - point[1], a[2] + d[2] * t - point[2]];
            let d2 = dot(w, w).max(1e-8);
            let inv = 1.0 / d2.sqrt();
            let l = [w[0] * inv, w[1] * inv, w[2] * inv];
            total += f64::from(s.a[3]) * piece_len * (-dot(ne, l)).max(0.0) * dot(normal, l).max(0.0) / d2;
        }
    }
    total
}

/// A light's shadow-map slot in the atlas.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShadowSlot {
    /// First layer (a cube uses six from here, in [`cube_face`] order).
    pub first_layer: u32,
    /// A point light's cube (six layers) rather than a spot's single map.
    pub cube: bool,
    /// The spot's matrix ([`spot_view_proj`]); unused for a cube.
    pub view_proj: [[f32; 4]; 4],
    pub near: f32,
    pub far: f32,
    /// Depth bias in metres.
    pub bias: f32,
}

/// The intensity (cd) a light ranks by: a strip's is the sum of its pieces' peak candela.
fn priority_intensity(l: &SceneLight) -> f64 {
    match (&l.strip, l.kind) {
        (Some(v), LightKind::Strip) => v.iter().map(|s| s.intensity).sum(),
        _ => l.intensity,
    }
}

/// Indices of `lights` ordered by how much they matter from `eye`: intensity (cd) divided by
/// max(squared distance from `eye` to the light, 1 m²), largest first; ties by lower index.
/// Non-finite scores sort last.
pub fn shadow_priority(lights: &[SceneLight], eye: [f64; 3]) -> Vec<usize> {
    let mut scored: Vec<(usize, f64)> = (0..lights.len())
        .map(|i| {
            let l = &lights[i];
            let dx = l.position.x - eye[0];
            let dy = l.position.y - eye[1];
            let dz = l.position.z - eye[2];
            let dsq = dx * dx + dy * dy + dz * dz;
            let finite = dx.is_finite() && dy.is_finite() && dz.is_finite() && dsq.is_finite() && l.intensity.is_finite();
            let score = if finite { priority_intensity(l) / dsq.max(1.0) } else { f64::NAN };
            (i, score)
        })
        .collect();
    scored.sort_by(|a, b| {
        let af = a.1.is_finite();
        let bf = b.1.is_finite();
        if af != bf {
            af.then(|| Ordering::Less).unwrap_or(Ordering::Greater)
        } else if af {
            b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal).then_with(|| a.0.cmp(&b.0))
        } else {
            a.0.cmp(&b.0)
        }
    });
    scored.into_iter().map(|(i, _)| i).collect()
}

/// The first `budget` of [`shadow_priority`], returned in ascending index order.
///
/// A strip light is never shadowed in the raster view, so it is never picked.
pub fn select_shadowed(lights: &[SceneLight], eye: [f64; 3], budget: usize) -> Vec<usize> {
    let mut sel: Vec<usize> =
        shadow_priority(lights, eye).iter().filter(|&&i| lights.get(i).is_some_and(|l| l.kind != LightKind::Strip)).take(budget).copied().collect();
    sel.sort();
    sel
}

/// The lights a frame draws: all of them when there are at most [`MAX_GPU_LIGHTS`], else the
/// first [`MAX_GPU_LIGHTS`] of [`shadow_priority`]; ascending index order. A strip counts as one
/// light; when the strips' segments exceed [`MAX_STRIP_SEGS`] the farthest strips from `eye` are
/// dropped first (ties: the higher index), and a strip with more segments than fit is never drawn.
pub fn select_drawn(lights: &[SceneLight], eye: [f64; 3]) -> Vec<usize> {
    let mut sel: Vec<usize> = if lights.len() <= MAX_GPU_LIGHTS {
        (0..lights.len()).collect()
    } else {
        let mut sel: Vec<usize> = shadow_priority(lights, eye).iter().take(MAX_GPU_LIGHTS).copied().collect();
        sel.sort();
        sel
    };
    let segs = |i: usize| lights.get(i).map_or(0, strip_seg_count);
    sel.retain(|&i| segs(i) <= MAX_STRIP_SEGS);
    let mut total: usize = sel.iter().map(|&i| segs(i)).sum();
    let dist = |i: usize| {
        lights.get(i).map_or(f64::INFINITY, |l| {
            let c = strip_centre(l);
            let d = (c.x - eye[0]).powi(2) + (c.y - eye[1]).powi(2) + (c.z - eye[2]).powi(2);
            if d.is_finite() { d } else { f64::INFINITY }
        })
    };
    while total > MAX_STRIP_SEGS {
        let farthest =
            sel.iter().copied().filter(|&i| segs(i) > 0).max_by(|&a, &b| dist(a).partial_cmp(&dist(b)).unwrap_or(Ordering::Equal).then(a.cmp(&b)));
        let Some(f) = farthest else { break };
        total = total.saturating_sub(segs(f));
        sel.retain(|&i| i != f);
    }
    sel
}

/// The `LightsU` bytes ([`LIGHTS_UNIFORM_SIZE`] long, std140, little-endian) for the `drawn`
/// lights (at most [`MAX_GPU_LIGHTS`], in that order) with their shadow slots (`slots` pairs a
/// light index with its slot; a drawn light without one is unshadowed). Colour = `color` ×
/// `intensity`.
pub fn pack_lights(lights: &[SceneLight], drawn: &[usize], slots: &[(usize, ShadowSlot)]) -> Vec<u8> {
    let mut buf = vec![0u8; LIGHTS_UNIFORM_SIZE as usize];
    write_u32(&mut buf, 0, drawn.len().min(MAX_GPU_LIGHTS) as u32);
    // The next free entry of `strip_segs`.
    let mut next_seg = 0usize;
    for (k, &i) in drawn.iter().take(MAX_GPU_LIGHTS).enumerate() {
        let Some(l) = lights.get(i) else { continue };
        let base = 16 + 64 * k;
        if l.kind == LightKind::Strip {
            // A marker light plus its segments; a strip that no longer fits gets no segments.
            let runs = strip_runs(l);
            let first = next_seg;
            let count = if first + runs.len() <= MAX_STRIP_SEGS { runs.len() } else { 0 };
            let c = strip_centre(l);
            let head = [c.x, c.y, c.z, 0.0, l.direction.x, l.direction.y, l.direction.z];
            for (j, v) in head.iter().enumerate() {
                write_f32(&mut buf, base + 4 * j, *v as f32);
            }
            write_f32(&mut buf, base + 28, STRIP_MARKER);
            for (j, c) in l.color.iter().enumerate() {
                write_f32(&mut buf, base + 32 + 4 * j, *c as f32);
            }
            // cos_inner 0; shadow = (-1: unshadowed, 0, first segment, segment count).
            for (off, v) in [(44, 0.0), (48, -1.0), (52, 0.0), (56, first as f32), (60, count as f32)] {
                write_f32(&mut buf, base + off, v);
            }
            for run in runs.iter().take(count) {
                let seg = strip_seg(run, 1.0);
                let sb = STRIP_SEGS_OFFSET + STRIP_SEG_SIZE * next_seg;
                for (j, v) in seg.a.iter().chain(&seg.b).chain(&seg.n).enumerate() {
                    write_f32(&mut buf, sb + 4 * j, *v);
                }
                next_seg += 1;
            }
            continue;
        }
        // `slots` pairs a light index with its slot, in the atlas's order, not the light's.
        let slot = slots.iter().find(|(j, _)| *j == i).map(|(_, s)| s);
        let far = slot.map_or(0.0, |s| s.far);
        write_f32(&mut buf, base + 0, l.position.x as f32);
        write_f32(&mut buf, base + 4, l.position.y as f32);
        write_f32(&mut buf, base + 8, l.position.z as f32);
        write_f32(&mut buf, base + 12, far as f32);
        // An area light is drawn as a spot over its hemisphere (cos 0..1 smoothstepped, close to its
        // cosine law); the path tracer samples its rectangle.
        let cos_outer = match l.kind {
            LightKind::Spot | LightKind::Area => l.cos_outer,
            LightKind::Point | LightKind::Strip => -2.0,
        };
        write_f32(&mut buf, base + 16, l.direction.x as f32);
        write_f32(&mut buf, base + 20, l.direction.y as f32);
        write_f32(&mut buf, base + 24, l.direction.z as f32);
        write_f32(&mut buf, base + 28, cos_outer as f32);
        write_f32(&mut buf, base + 32, (l.color[0] * l.intensity) as f32);
        write_f32(&mut buf, base + 36, (l.color[1] * l.intensity) as f32);
        write_f32(&mut buf, base + 40, (l.color[2] * l.intensity) as f32);
        write_f32(&mut buf, base + 44, l.cos_inner as f32);
        match slot {
            Some(s) => {
                write_f32(&mut buf, base + 48, s.first_layer as f32);
                write_f32(&mut buf, base + 52, if s.cube { 1.0 } else { 0.0 });
                write_f32(&mut buf, base + 56, s.near);
                write_f32(&mut buf, base + 60, s.bias);
            }
            None => {
                write_f32(&mut buf, base + 48, -1.0);
                write_f32(&mut buf, base + 52, 0.0);
                write_f32(&mut buf, base + 56, 0.0);
                write_f32(&mut buf, base + 60, 0.0);
            }
        }
        if let Some(s) = slot {
            if !s.cube {
                // mat4x4<f32>, column-major: 16 bytes a column.
                let vp_base = 16 + 64 * 64 + 64 * k;
                for (col, col_arr) in s.view_proj.iter().enumerate() {
                    for (row, v) in col_arr.iter().enumerate() {
                        write_f32(&mut buf, vp_base + col * 16 + row * 4, *v);
                    }
                }
            }
        }
    }
    buf
}

/// Forward and up axes of cube face `face`: 0 +X, 1 −X, 2 +Y, 3 −Y (up +Z), 4 +Z, 5 −Z (up +Y).
/// The face's right axis is `forward × up`. Faces past 5 read as 5.
pub fn cube_face(face: usize) -> ([f64; 3], [f64; 3]) {
    match face {
        0 => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        1 => ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        2 => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        3 => ([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
        4 => ([0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        _ => ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn unit(a: [f64; 3]) -> Option<[f64; 3]> {
    let l = dot(a, a).sqrt();
    (l.is_finite() && l > 1e-12).then(|| [a[0] / l, a[1] / l, a[2] / l])
}

/// Column-major `clip = M · [p, 1]` of a perspective camera at `eye` looking along unit `f` with
/// up hint `u`, `scale` = cot(fov/2), depth 0 at `near` and 1 at `far` (wgpu range).
fn perspective_look(eye: [f64; 3], f: [f64; 3], u: [f64; 3], scale: f64, near: f64, far: f64) -> Option<[[f32; 4]; 4]> {
    if !(near.is_finite() && far.is_finite() && near > 0.0 && far > near && scale.is_finite() && scale > 0.0) {
        return None;
    }
    let f = unit(f)?;
    let r = unit(cross(f, u))?;
    let u = cross(r, f);
    let a = far / (near - far);
    let b = far * near / (near - far);
    // Rows of the combined matrix: x = s·r·(p−e), y = s·u·(p−e), z = A·(−f·(p−e)) + B, w = f·(p−e).
    let rows = [
        [scale * r[0], scale * r[1], scale * r[2], -scale * dot(r, eye)],
        [scale * u[0], scale * u[1], scale * u[2], -scale * dot(u, eye)],
        [-a * f[0], -a * f[1], -a * f[2], a * dot(f, eye) + b],
        [f[0], f[1], f[2], -dot(f, eye)],
    ];
    let mut m = [[0.0f32; 4]; 4];
    for (row, vals) in rows.iter().enumerate() {
        for (col, v) in vals.iter().enumerate() {
            if !v.is_finite() {
                return None;
            }
            m[col][row] = *v as f32;
        }
    }
    Some(m)
}

/// The matrix of cube face `face` of a point light at `pos` (90° field of view, square).
pub fn cube_face_view_proj(pos: [f64; 3], face: usize, near: f32, far: f32) -> Option<[[f32; 4]; 4]> {
    let (f, u) = cube_face(face);
    perspective_look(pos, f, u, 1.0, f64::from(near), f64::from(far))
}

/// Widest spot map (full field of view, degrees); a spot whose falloff is wider is shadowed with
/// a cube like a point light.
pub const MAX_SPOT_MAP_FOV: f64 = 150.0;

/// A spot's shadow-map matrix: from its light point along its aim, with a field of view of its
/// falloff plus 4° (so the cone's edge is inside the map), up = +Z unless the aim is within 8° of
/// vertical (then +Y). `None` when the field of view would exceed [`MAX_SPOT_MAP_FOV`] or a value
/// is not finite.
pub fn spot_view_proj(light: &SceneLight, near: f32, far: f32) -> Option<[[f32; 4]; 4]> {
    let half_falloff = light.cos_outer.clamp(-1.0, 1.0).acos();
    let fov = half_falloff.to_degrees() * 2.0 + 4.0;
    if !fov.is_finite() || fov > MAX_SPOT_MAP_FOV {
        return None;
    }
    let f = [light.direction.x, light.direction.y, light.direction.z];
    let up = if f[2].abs() > 8f64.to_radians().cos() { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
    let scale = 1.0 / (fov.to_radians() * 0.5).tan();
    let p = light.position;
    perspective_look([p.x, p.y, p.z], f, up, scale, f64::from(near), f64::from(far))
}

/// Distance along the map's forward axis (m) of a stored depth `d` (0..1) — the inverse of
/// `far/(far−near)·(1 − near/z)`. Mirrors `linear_depth` in `shaders/light_math.wgsl`.
pub fn linear_depth(d: f32, near: f32, far: f32) -> f32 {
    let k = 1.0 - d * (far - near) / far;
    if k <= 1e-7 { f32::MAX } else { near / k }
}

/// WGSL helpers every lit shader includes: `linear_depth`, `cube_face_of`, `cube_uv_depth`.
pub const LIGHT_MATH_WGSL: &str = include_str!("shaders/light_math.wgsl");

fn write_u32(buf: &mut Vec<u8>, off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// Store `v` (non-finite becomes 0.0) little-endian at `off`.
fn write_f32(buf: &mut Vec<u8>, off: usize, v: f32) {
    buf[off..off + 4].copy_from_slice(&if v.is_finite() { v } else { 0.0 }.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f32_at(buf: &[u8], off: usize) -> f32 {
        f32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
    }

    fn spot_at(id: u32, x: f64) -> SceneLight {
        SceneLight {
            id: crate::types::ArchId(id),
            kind: LightKind::Spot,
            position: Vec3::new(x, 0.0, 2.4),
            direction: Vec3::new(0.0, 0.0, -1.0),
            intensity: 950.0,
            color: [1.0; 3],
            cos_inner: 0.866,
            cos_outer: 0.866,
            ..Default::default()
        }
    }

    #[test]
    fn packed_spot_matrix_is_column_major_sixteen_bytes_a_column() {
        let mut vp = [[0.0f32; 4]; 4];
        for (c, col) in vp.iter_mut().enumerate() {
            for (r, v) in col.iter_mut().enumerate() {
                *v = (c * 4 + r + 1) as f32;
            }
        }
        let slot = ShadowSlot { first_layer: 0, cube: false, view_proj: vp, near: 0.05, far: 9.0, bias: 0.02 };
        let buf = pack_lights(&[spot_at(0, 0.0)], &[0], &[(0, slot)]);
        let base = 16 + 64 * MAX_GPU_LIGHTS;
        for c in 0..4 {
            for r in 0..4 {
                assert_eq!(f32_at(&buf, base + c * 16 + r * 4), (c * 4 + r + 1) as f32, "column {c} row {r}");
            }
        }
    }

    #[test]
    fn each_drawn_light_packs_its_own_slot() {
        let lights = [spot_at(0, 0.0), spot_at(1, 2.0)];
        let slot =
            |layer: u32, far: f32| ShadowSlot { first_layer: layer, cube: false, view_proj: [[layer as f32; 4]; 4], near: 0.05, far, bias: 0.02 };
        // The atlas lists light 1's slot first.
        let buf = pack_lights(&lights, &[0, 1], &[(1, slot(5, 7.0)), (0, slot(3, 4.0))]);
        assert_eq!(f32_at(&buf, 16 + 48), 3.0, "light 0's layer");
        assert_eq!(f32_at(&buf, 16 + 12), 4.0, "light 0's far");
        assert_eq!(f32_at(&buf, 16 + 64 + 48), 5.0, "light 1's layer");
        assert_eq!(f32_at(&buf, 16 + 64 + 12), 7.0, "light 1's far");
        assert_eq!(f32_at(&buf, 16 + 64 * MAX_GPU_LIGHTS + 64), 5.0, "light 1's matrix");
        // A drawn light with no slot is unshadowed.
        let buf = pack_lights(&lights, &[0, 1], &[(1, slot(5, 7.0))]);
        assert_eq!(f32_at(&buf, 16 + 48), -1.0);
    }

    fn apply(m: &[[f32; 4]; 4], p: [f64; 3]) -> [f64; 4] {
        let mut out = [0.0; 4];
        for (row, o) in out.iter_mut().enumerate() {
            *o = (0..4).map(|col| f64::from(m[col][row]) * if col < 3 { p[col] } else { 1.0 }).sum();
        }
        out
    }

    /// Rust mirror of `cube_uv_depth` in light_math.wgsl.
    fn cube_uv_depth(v: [f64; 3], near: f64, far: f64) -> (usize, [f64; 2], f64) {
        let (ax, ay, az) = (v[0].abs(), v[1].abs(), v[2].abs());
        let face = if ax >= ay && ax >= az {
            if v[0] >= 0.0 { 0 } else { 1 }
        } else if ay >= az {
            if v[1] >= 0.0 { 2 } else { 3 }
        } else if v[2] >= 0.0 {
            4
        } else {
            5
        };
        let (f, u) = cube_face(face);
        let r = cross(f, u);
        let ma = dot(v, f);
        let ndc = [dot(v, r) / ma, dot(v, u) / ma];
        (face, [ndc[0] * 0.5 + 0.5, 0.5 - ndc[1] * 0.5], far / (far - near) * (1.0 - near / ma))
    }

    #[test]
    fn cube_lookup_matches_the_face_matrices() {
        let pos = [1.0, 2.0, 2.4];
        for p in [[3.0, 2.5, 1.0], [-2.0, 2.2, 2.0], [1.3, 5.0, 0.0], [0.0, -3.0, 2.0], [1.1, 2.1, 7.0], [1.2, 1.9, -3.0]] {
            let v = [p[0] - pos[0], p[1] - pos[1], p[2] - pos[2]];
            let (face, uv, depth) = cube_uv_depth(v, 0.05, 20.0);
            let m = cube_face_view_proj(pos, face, 0.05, 20.0).unwrap_or([[0.0; 4]; 4]);
            let c = apply(&m, p);
            let ndc = [c[0] / c[3], c[1] / c[3], c[2] / c[3]];
            assert!((ndc[0] * 0.5 + 0.5 - uv[0]).abs() < 1e-5 && (0.5 - ndc[1] * 0.5 - uv[1]).abs() < 1e-5, "{p:?} face {face}");
            assert!((ndc[2] - depth).abs() < 1e-5, "{p:?} depth {} vs {depth}", ndc[2]);
            let back = linear_depth(depth as f32, 0.05, 20.0);
            assert!((f64::from(back) - dot(v, cube_face(face).0)).abs() < 1e-3);
            assert!((0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1]));
        }
    }

    #[test]
    fn a_spot_straight_down_maps_its_axis_to_the_map_centre() {
        let l = SceneLight {
            kind: cadcraft_arch3d::light::LightKind::Spot,
            position: cadcraft_geom::Vec3::new(1.0, 1.0, 2.4),
            direction: cadcraft_geom::Vec3::new(0.0, 0.0, -1.0),
            intensity: 950.4,
            color: [1.0; 3],
            cos_inner: 30f64.to_radians().cos(),
            cos_outer: 30f64.to_radians().cos(),
            ..Default::default()
        };
        let m = spot_view_proj(&l, 0.05, 10.0).unwrap_or([[0.0; 4]; 4]);
        let c = apply(&m, [1.0, 1.0, 0.0]);
        assert!((c[0] / c[3]).abs() < 1e-6 && (c[1] / c[3]).abs() < 1e-6);
        let z = f64::from(linear_depth((c[2] / c[3]) as f32, 0.05, 10.0));
        assert!((z - 2.4).abs() < 1e-3, "{z}");
        // The cone's edge (30° off the axis) lies inside the map.
        let edge = apply(&m, [1.0 + 2.4 * 30f64.to_radians().tan(), 1.0, 0.0]);
        assert!((edge[0] / edge[3]).abs() < 1.0 && (edge[1] / edge[3]).abs() < 1.0);
        let wide = SceneLight { cos_outer: -0.5, ..l };
        assert!(spot_view_proj(&wide, 0.05, 10.0).is_none());
    }

    use cadcraft_arch3d::light::{LightKind, SceneLight};
    use cadcraft_geom::Vec3;

    /// A scene light at `pos`, `intensity` cd, of the given `kind`, `cos` inner/outer.
    fn light(pos: [f64; 3], intensity: f64, kind: LightKind, cos: f64) -> SceneLight {
        SceneLight {
            kind,
            position: Vec3::new(pos[0], pos[1], pos[2]),
            direction: Vec3::new(0.0, 0.0, 1.0),
            intensity,
            color: [1.0; 3],
            cos_inner: cos,
            cos_outer: cos,
            ..Default::default()
        }
    }

    fn row(n: usize) -> Vec<SceneLight> {
        (0..n).map(|i| light([i as f64, 0.0, 0.0], 1.0, LightKind::Point, 1.0)).collect()
    }

    #[test]
    fn shadow_priority_prefers_near_and_bright() {
        // Uniform 12-light row along +x from the eye: nearest first.
        let p = shadow_priority(&row(12), [0.0, 0.0, 0.0]);
        assert_eq!(p, (0..12).collect::<Vec<usize>>());
        // A light 10x brighter at twice the distance outranks its dimmer nearer neighbour
        // (score 10/4 = 2.5 > 1/1 = 1.0).
        let bright_far = light([2.0, 0.0, 0.0], 10.0, LightKind::Point, 1.0);
        let dim_near = light([1.0, 0.0, 0.0], 1.0, LightKind::Point, 1.0);
        assert_eq!(shadow_priority(&vec![bright_far, dim_near], [0.0, 0.0, 0.0]), vec![0, 1]);
    }

    #[test]
    fn select_shadowed_keeps_the_budget_in_index_order() {
        // 12-light row along +x: the 8 nearest are shadowed, reported in ascending order.
        let s = select_shadowed(&row(12), [0.0, 0.0, 0.0], 8);
        assert_eq!(s, (0..8).collect::<Vec<usize>>());
    }

    #[test]
    fn select_drawn_caps_at_64() {
        // 70 lights draw 64.
        let d = select_drawn(&row(70), [0.0, 0.0, 0.0]);
        assert_eq!(d.len(), MAX_GPU_LIGHTS);
        assert_eq!(d, (0..64).collect::<Vec<usize>>());
        // A short list draws everything.
        assert_eq!(select_drawn(&row(3), [0.0, 0.0, 0.0]), vec![0, 1, 2]);
    }

    fn read_u8s(buf: &[u8], off: usize) -> [u8; 4] {
        [
            buf.get(off).copied().unwrap_or(0),
            buf.get(off + 1).copied().unwrap_or(0),
            buf.get(off + 2).copied().unwrap_or(0),
            buf.get(off + 3).copied().unwrap_or(0),
        ]
    }

    fn read_f32(buf: &[u8], off: usize) -> f32 {
        f32::from_bits(u32::from_le_bytes(read_u8s(buf, off)))
    }

    fn read_u32(buf: &[u8], off: usize) -> u32 {
        u32::from_le_bytes(read_u8s(buf, off))
    }

    #[test]
    fn pack_lights_layout_matches_the_wgsl_struct() {
        let l = vec![light([10.0, 20.0, 30.0], 5.0, LightKind::Spot, 0.5), light([40.0, 50.0, 60.0], 3.0, LightKind::Spot, 0.6)];
        let slots = vec![
            (0, ShadowSlot { first_layer: 7, cube: false, view_proj: [[0.0; 4]; 4], near: 0.05, far: 10.0, bias: 0.001 }),
            (1, ShadowSlot { first_layer: 8, cube: false, view_proj: [[0.0; 4]; 4], near: 0.05, far: 10.0, bias: 0.001 }),
        ];
        let buf = pack_lights(&l, &[0, 1], &slots);
        assert_eq!(read_u32(&buf, 0), 2);
        // Light 0 at offset 16: pos_range = (x, y, z, far) at offsets 0, 4, 8, 12.
        assert_eq!(read_f32(&buf, 16 + 0), 10.0, "light 0 position.x at 16");
        assert_eq!(read_f32(&buf, 16 + 8), 30.0, "light 0 position.z at 16 + 8");
        assert_eq!(read_f32(&buf, 16 + 12), 10.0, "light 0 far at 16 + 12");
        // Light 1: position at offset 16 + 64, first_layer at offset 16 + 64 + 48.
        let base1 = 16 + 64;
        assert_eq!(read_f32(&buf, base1 + 0), 40.0, "light 1 position.x at 16 + 64");
        assert_eq!(read_f32(&buf, base1 + 8), 60.0, "light 1 position.z at 16 + 64 + 8");
        assert_eq!(read_f32(&buf, base1 + 12), 10.0, "light 1 far at 16 + 64 + 12");
        assert_eq!(read_f32(&buf, base1 + 48), 8.0, "light 1 first_layer at 16 + 64 + 48");
    }

    #[test]
    fn pack_lights_marks_unshadowed_lights() {
        let l = vec![light([1.0, 2.0, 3.0], 5.0, LightKind::Spot, 0.5), light([4.0, 5.0, 6.0], 3.0, LightKind::Point, -2.0)];
        let slots = vec![(0, ShadowSlot { first_layer: 3, cube: false, view_proj: [[0.0; 4]; 4], near: 0.05, far: 10.0, bias: 0.001 })];
        let buf = pack_lights(&l, &[0, 1], &slots);
        // Light 0 shadowed: (first_layer, cube, near, bias) at offset 16 + 48.
        assert_eq!(read_f32(&buf, 16 + 48), 3.0);
        assert_eq!(read_f32(&buf, 16 + 52), 0.0);
        assert_eq!(read_f32(&buf, 16 + 56), 0.05);
        assert_eq!(read_f32(&buf, 16 + 60), 0.001);
        // Light 1 unshadowed (point, no slot): shadow = (-1, 0, 0, 0); cos_outer w = -2.
        let base1 = 16 + 64;
        assert_eq!(read_f32(&buf, base1 + 28), -2.0, "point cos_outer w");
        assert_eq!(read_f32(&buf, base1 + 48), -1.0, "unshadowed first_layer -1");
        assert_eq!(read_f32(&buf, base1 + 52), 0.0);
        assert_eq!(read_f32(&buf, base1 + 56), 0.0);
        assert_eq!(read_f32(&buf, base1 + 60), 0.0);
    }

    /// A strip of `lm_per_m` lumens per metre from `a` to `b` at `z`, emitting down, in pieces of
    /// at most 0.15 m (as `cadcraft_arch::light::strip_samples`).
    fn strip(a: [f64; 2], b: [f64; 2], z: f64, lm_per_m: f64) -> SceneLight {
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        let pieces = ((len / 0.15).ceil() as usize).max(1);
        let samples: Vec<StripSample> = (0..pieces)
            .map(|k| {
                let t = (k as f64 + 0.5) / pieces as f64;
                StripSample {
                    position: Vec3::new(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, z),
                    normal: Vec3::new(0.0, 0.0, -1.0),
                    intensity: lm_per_m * len / pieces as f64 / std::f64::consts::PI,
                    ..Default::default()
                }
            })
            .collect();
        SceneLight {
            kind: LightKind::Strip,
            position: Vec3::new((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, z),
            direction: Vec3::new(0.0, 0.0, -1.0),
            color: [1.0, 0.8, 0.6],
            strip: Some(std::sync::Arc::new(samples)),
            ..Default::default()
        }
    }

    #[test]
    fn lights_uniform_fits_webgl2() {
        assert_eq!(LIGHTS_UNIFORM_SIZE, 8208 + 6144);
        assert_eq!(LIGHTS_UNIFORM_SIZE, 14_352);
        assert!(LIGHTS_UNIFORM_SIZE <= 16_384, "the WebGL2 uniform block limit");
        assert_eq!(STRIP_SEGS_OFFSET % 16, 0);
        assert_eq!(STRIP_SEG_SIZE, 48);
        assert_eq!(pack_lights(&[], &[], &[]).len() as u64, LIGHTS_UNIFORM_SIZE);
    }

    #[test]
    fn strip_one_drawn_light_never_shadowed() {
        // An L-shaped strip: two runs, one light.
        let mut l = strip([0.5, 0.5], [3.5, 0.5], 2.4, 500.0);
        let second = strip([3.5, 0.5], [3.5, 1.5], 2.4, 500.0);
        if let (Some(a), Some(b)) = (l.strip.take(), second.strip) {
            let mut v = (*a).clone();
            v.extend(b.iter().copied());
            l.strip = Some(std::sync::Arc::new(v));
        }
        assert_eq!(strip_runs(&l).len(), 2);
        let mut lights = vec![l, spot_at(1, 1.0), spot_at(2, 2.0)];
        let eye = [2.0, 2.0, 1.5];
        assert_eq!(select_drawn(&lights, eye), vec![0, 1, 2], "a strip counts once");
        // Never shadowed, even with a budget to spare.
        assert_eq!(select_shadowed(&lights, eye, 8), vec![1, 2]);
        assert_eq!(select_shadowed(&lights[..1], eye, 8), Vec::<usize>::new());
        let buf = pack_lights(&lights, &[0, 1, 2], &[]);
        assert_eq!(read_u32(&buf, 0), 3);
        assert_eq!(read_f32(&buf, 16 + 28), STRIP_MARKER, "the marker");
        assert_eq!(read_f32(&buf, 16 + 48), -1.0, "raster-unshadowed");
        assert_eq!((read_f32(&buf, 16 + 56), read_f32(&buf, 16 + 60)), (0.0, 2.0), "first segment and count");
        let seg0 = STRIP_SEGS_OFFSET;
        assert!((read_f32(&buf, seg0 + 12) - (500.0 / std::f64::consts::PI) as f32).abs() < 1e-2, "cd per metre");
        assert_eq!(read_f32(&buf, seg0 + 28), 20.0, "3 m in 20 pieces");
        assert_eq!(read_f32(&buf, seg0 + 40), -1.0, "emitting down");
        // Past 128 segments the farthest strips go first.
        let many: Vec<SceneLight> = (0..3)
            .map(|k| {
                let mut s = strip([0.0, k as f64 * 10.0], [1.0, k as f64 * 10.0], 2.4, 500.0);
                // 60 one-piece runs: alternate normals so none merge.
                let samples: Vec<StripSample> = (0..60)
                    .map(|j| StripSample {
                        position: Vec3::new(j as f64, k as f64 * 10.0, 2.4),
                        normal: Vec3::new(0.0, 0.0, if j % 2 == 0 { -1.0 } else { 1.0 }),
                        intensity: 1.0,
                        ..Default::default()
                    })
                    .collect();
                s.strip = Some(std::sync::Arc::new(samples));
                s
            })
            .collect();
        assert_eq!(strip_seg_count(&many[0]), 60);
        assert_eq!(select_drawn(&many, [0.0, 0.0, 1.0]), vec![0, 1], "the farthest strip is dropped");
        assert_eq!(select_drawn(&many, [0.0, 20.0, 1.0]), vec![1, 2]);
        lights.extend(many);
        let all: Vec<usize> = (0..lights.len()).collect();
        let sel = select_drawn(&lights, [0.0, 0.0, 1.0]);
        assert!(sel.iter().map(|&i| strip_seg_count(&lights[i])).sum::<usize>() <= MAX_STRIP_SEGS);
        assert_eq!(pack_lights(&lights, &all, &[]).len() as u64, LIGHTS_UNIFORM_SIZE);
    }

    #[test]
    fn strip_irradiance_cpu_matches_line_integral() {
        // Check 05's strip: 2 m of 500 lm/m at 2.4 m: 49.72 lx under its centre.
        let l = strip([1.0, 2.0], [3.0, 2.0], 2.4, 500.0);
        let segs: Vec<StripSeg> = strip_runs(&l).iter().map(|r| strip_seg(r, 1.0)).collect();
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].b[3], 14.0, "2 m in 14 pieces of at most 0.15 m");
        let e = strip_irradiance_cpu(&segs, [2.0, 2.0, 0.0], [0.0, 0.0, 1.0]);
        assert!((e - 49.72).abs() < 49.72 * 0.01, "{e}");
        // Behind the emitting side, and facing away: nothing.
        assert_eq!(strip_irradiance_cpu(&segs, [2.0, 2.0, 2.8], [0.0, 0.0, 1.0]), 0.0);
        assert_eq!(strip_irradiance_cpu(&segs, [2.0, 2.0, 0.0], [0.0, 0.0, -1.0]), 0.0);
        // The lone-piece run keeps its lumens: a 0.1 m strip is 50 lm, nearly a point source.
        let short = strip([2.0, 2.0], [2.1, 2.0], 2.4, 500.0);
        let s: Vec<StripSeg> = strip_runs(&short).iter().map(|r| strip_seg(r, 1.0)).collect();
        let e = strip_irradiance_cpu(&s, [2.05, 2.0, 0.0], [0.0, 0.0, 1.0]);
        let point = 50.0 / std::f64::consts::PI / (2.4 * 2.4);
        assert!((e - point).abs() < point * 0.05, "{e} vs {point}");
    }
}
