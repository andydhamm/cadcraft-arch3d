//! Tests of `shaders/pt/lights.wgsl`, run through `lights_test_main` (the test entry at the end of
//! that file) with `tracer::run_entry` over scenes from `build_gpu_scene`. Probe `i` picks what the
//! entry does through `_a`: a light index samples that light `_b` times from `point` and writes the
//! irradiance estimate on a surface with `normal` (xyz) and the mean pdf (w); the query modes read
//! the sky, the sun, an IES table or an area light's radiance.
//!
//! `cadcraft_arch::ies::candela_at` is not a dependency of this crate, so the IES test checks the
//! shader against `reference_candela`, an f64 re-statement of that function's documented rules
//! (bilinear in the two angles, the horizontal angle folded by the table's symmetry).

use super::device::ComputeGpu;
use super::scene_gpu::{FRAME_FURNACE, FRAME_SKY, FRAME_SUN, GpuFrame, GpuProbe, GpuScene, build_gpu_scene};
use super::tracer::{run_entry, upload_scene};
use super::{SKY_HORIZON_NITS, SKY_ZENITH_NITS, SUN_LUX, luminance};
use crate::types::{Mesh, Scene3d};
use cadcraft_arch3d::light::{LightKind, SceneLight, light_axis};
use cadcraft_doc::ArchId;
use cadcraft_doc::arch::{ArchModel, FixtureShape, Level, LightFixture, LightMount, Room, StripFacing};
use cadcraft_geom::{Vec2, Vec3};
use std::sync::Arc;

const ENTRY: &str = "lights_test_main";
/// `probe._a` of the query modes (a light index samples instead).
const SKY: f32 = -1.0;
const SUN_LI: f32 = -2.0;
const SUN_DIR: f32 = -3.0;
const IES_QUERY: f32 = 1_000_000.0;
const AREA_QUERY: f32 = 2_000_000.0;

fn gpu_or_skip() -> Option<ComputeGpu> {
    let gpu = crate::gputest::shared_compute();
    if gpu.is_none() {
        eprintln!("SKIP: no compute device");
    }
    gpu
}

fn v(a: [f64; 3]) -> Vec3 {
    Vec3::new(a[0], a[1], a[2])
}

fn f32s(a: Vec3) -> [f32; 3] {
    [a.x as f32, a.y as f32, a.z as f32]
}

/// Sample `light` `count` times from `point` for a surface facing `normal`.
fn sample_probe(light: usize, point: Vec3, normal: Vec3, count: u32) -> GpuProbe {
    GpuProbe { point: f32s(point), _a: light as f32, normal: f32s(normal), _b: count as f32, ..Default::default() }
}

/// A query-mode probe (`mode` is one of the constants above, plus a light index for the tables).
fn query_probe(mode: f32, dir: Vec3) -> GpuProbe {
    GpuProbe { point: [0.0; 3], _a: mode, normal: f32s(dir), _b: 0.0, ..Default::default() }
}

/// Run a WGSL entry point over `probes` with `frame` (the probe and light counts are filled in).
fn run_entry_point(gpu: &ComputeGpu, scene: &GpuScene, entry: &str, frame: GpuFrame, probes: &[GpuProbe]) -> Vec<[f32; 4]> {
    let buffers = upload_scene(gpu, scene);
    let frame = GpuFrame { probe_count: probes.len() as u32, light_count: scene.lights.len() as u32, seed: 11, ..frame };
    match run_entry(gpu, &buffers, entry, &frame, probes) {
        Ok(out) => {
            assert_eq!(out.len(), probes.len());
            out
        }
        Err(e) => {
            assert!(false, "{e}");
            Vec::new()
        }
    }
}

/// Run `lights_test_main` over `probes` with `frame` (the probe and light counts are filled in).
fn run(gpu: &ComputeGpu, scene: &GpuScene, frame: GpuFrame, probes: &[GpuProbe]) -> Vec<[f32; 4]> {
    run_entry_point(gpu, scene, ENTRY, frame, probes)
}

/// Run `irradiance_main` over `probes` with `spp` samples per probe, returning the sum after the
/// frame's sample count is added by the shader.
fn run_irradiance(gpu: &ComputeGpu, scene: &GpuScene, spp: u32, probes: &[GpuProbe]) -> Vec<[f32; 4]> {
    run_entry_point(gpu, scene, "irradiance_main", GpuFrame { bounces: 0, spp_this_pass: spp, ..Default::default() }, probes)
}

fn lux(o: [f32; 4]) -> f64 {
    f64::from(luminance([o[0], o[1], o[2]]))
}

fn lux_irradiance(o: [f32; 4], spp: u32) -> f64 {
    let avg = [o[0] / spp as f32, o[1] / spp as f32, o[2] / spp as f32];
    f64::from(luminance(avg))
}

fn down() -> Vec3 {
    Vec3::new(0.0, 0.0, -1.0)
}

fn up() -> Vec3 {
    Vec3::new(0.0, 0.0, 1.0)
}

fn scene_of(lights: &[SceneLight]) -> GpuScene {
    build_gpu_scene(&Scene3d { lights: lights.to_vec(), ..Default::default() }, false, None)
}

fn prism(material: &str, object: u32, min: [f32; 3], max: [f32; 3]) -> Mesh {
    let mut m = Mesh::new(material, ArchId(object));
    for k in 0..8 {
        m.positions.push([
            if k & 1 == 0 { min[0] } else { max[0] },
            if k & 2 == 0 { min[1] } else { max[1] },
            if k & 4 == 0 { min[2] } else { max[2] },
        ]);
    }
    let quads: [[u32; 4]; 6] = [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]];
    for q in quads {
        m.indices.extend([q[0], q[1], q[2], q[0], q[2], q[3]]);
    }
    m
}

// ---------------------------------------------------------------------------------------------
// Points and spots
// ---------------------------------------------------------------------------------------------

/// A spot aiming down; `cone` and `falloff` are the half angles in degrees.
fn spot(position: [f64; 3], intensity: f64, cone: f64, falloff: f64) -> SceneLight {
    SceneLight {
        kind: LightKind::Spot,
        position: v(position),
        direction: down(),
        intensity,
        color: [1.0; 3],
        cos_inner: cone.to_radians().cos(),
        cos_outer: falloff.to_radians().cos(),
        axis: light_axis(down(), 0.0),
        ..Default::default()
    }
}

#[test]
fn point_and_spot_samples_match_intensity_toward() {
    let Some(gpu) = gpu_or_skip() else { return };
    // The phase-10 pot light (a 60 degree hard cone), a soft-edged coloured spot and a coloured point.
    let hard = spot([0.0, 0.0, 2.4], 950.36, 30.0, 30.0);
    let soft = SceneLight { color: [1.0, 0.8, 0.6], ..spot([10.0, 0.0, 2.4], 500.0, 20.0, 45.0) };
    let point = SceneLight {
        kind: LightKind::Point,
        cos_inner: -1.0,
        cos_outer: -1.0,
        color: [1.0, 0.9, 0.7],
        intensity: 200.0,
        ..spot([20.0, 0.0, 2.4], 200.0, 0.0, 0.0)
    };
    let lights = [hard, soft, point];
    let scene = scene_of(&lights);
    assert_eq!(scene.lights.len(), 3);

    let angles: [&[f64]; 3] = [&[0.0, 12.0, 28.0, 32.0, 60.0], &[0.0, 15.0, 25.0, 35.0, 44.0, 50.0], &[0.0, 70.0, 130.0, 180.0]];
    let dist = 2.4;
    let (mut probes, mut expected) = (Vec::new(), Vec::new());
    for (k, list) in angles.iter().enumerate() {
        for &deg in *list {
            let t = deg.to_radians();
            // From the light toward the lit point, and the point on a surface facing the light.
            let to = Vec3::new(t.sin(), 0.0, -t.cos());
            probes.push(sample_probe(k, lights[k].position + to * dist, -to, 1));
            expected.push((k, deg, lights[k].intensity_toward(to) / (dist * dist)));
        }
    }
    let out = run(&gpu, &scene, GpuFrame::default(), &probes);
    for ((k, deg, want), got) in expected.iter().zip(&out) {
        let lx = lux(*got);
        assert!((lx - want).abs() <= want.abs() * 1e-3 + 1e-4, "light {k} at {deg} deg: {lx} lx, want {want}");
        assert_eq!(got[3], 1.0, "a point or spot sample has pdf 1 (light {k} at {deg} deg)");
    }
    // The soft spot really is between its two cones at 25 and 35 degrees, so the smoothstep ran.
    let soft_at = |deg: f64| expected.iter().find(|(k, d, _)| *k == 1 && *d == deg).map_or(0.0, |e| e.2);
    let full = 500.0 / (dist * dist);
    assert!(soft_at(25.0) > 0.0 && soft_at(25.0) < full, "{}", soft_at(25.0));
    assert!(soft_at(35.0) > 0.0 && soft_at(35.0) < soft_at(25.0));
    assert_eq!(soft_at(50.0), 0.0);

    // 165 lx straight down at 2.4 m on the floor, and nothing outside the cone.
    let at_floor = |dx: f64| sample_probe(0, Vec3::new(dx, 0.0, 0.0), up(), 4);
    let floor = run(&gpu, &scene, GpuFrame::default(), &[at_floor(0.0), at_floor(1.0), at_floor(2.0)]);
    assert!((lux(floor[0]) - 165.0).abs() < 0.5, "{}", lux(floor[0]));
    // 1.0 m off axis is 22.6 degrees (inside the 30 degree half cone): I cos(theta)^3 / h^2 ... = 950.36 * cos^3 / 5.76.
    let c = 2.4 / (1.0f64 + 2.4 * 2.4).sqrt();
    assert!((lux(floor[1]) - 950.36 * c * c * c / 5.76).abs() < 0.5, "{}", lux(floor[1]));
    // 2.0 m off axis is 39.8 degrees: outside.
    assert_eq!(lux(floor[2]), 0.0);

    // The colour's ratios survive: the soft spot's blue over red is 0.6, its luminance the candela.
    let first = out[expected.iter().position(|(k, d, _)| *k == 1 && *d == 0.0).unwrap_or(0)];
    assert!((first[2] / first[0] - 0.6).abs() < 1e-4, "{first:?}");
}

// ---------------------------------------------------------------------------------------------
// IES
// ---------------------------------------------------------------------------------------------

/// An IES table as the doc crate stores it (candela horizontal-major).
struct Table {
    vertical: Vec<f64>,
    horizontal: Vec<f64>,
    candela: Vec<f64>,
}

impl Table {
    /// The phase-11 acceptance check's test downlight: 1000 cd down, 900 at 22.5 degrees, 600, 200, 0 at 90.
    fn downlight() -> Table {
        Table { vertical: vec![0.0, 22.5, 45.0, 67.5, 90.0], horizontal: vec![0.0], candela: vec![1000.0, 900.0, 600.0, 200.0, 0.0] }
    }

    /// Some asymmetric table over the given angles (`salt` varies the values).
    fn varied(vertical: &[f64], horizontal: &[f64], salt: usize) -> Table {
        let candela = (0..vertical.len() * horizontal.len()).map(|i| 40.0 + 37.0 * ((i * 7 + salt) % 11) as f64).collect();
        Table { vertical: vertical.to_vec(), horizontal: horizontal.to_vec(), candela }
    }

    fn peak(&self) -> f64 {
        self.candela.iter().copied().fold(0.0, f64::max)
    }
}

/// `cadcraft_arch::ies::candela_at`'s documented rules, in f64.
fn reference_candela(t: &Table, vertical_deg: f64, horizontal_deg: f64) -> f64 {
    let (nv, nh) = (t.vertical.len(), t.horizontal.len());
    let lerp = |a: f64, b: f64, x: f64| a + (b - a) * x;
    let fraction = |a: f64, b: f64, x: f64| if b - a > 0.0 { ((x - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 };
    // (lo, hi, t) of x among ascending angles, the nearest end outside them.
    let clamped = |angles: &[f64], x: f64| -> (usize, usize, f64) {
        let upper = angles.partition_point(|&a| a <= x);
        if upper == 0 {
            return (0, 0, 0.0);
        }
        if upper >= angles.len() {
            return (angles.len() - 1, angles.len() - 1, 0.0);
        }
        (upper - 1, upper, fraction(angles[upper - 1], angles[upper], x))
    };
    if vertical_deg < t.vertical[0] || vertical_deg > t.vertical[nv - 1] {
        return 0.0;
    }
    let (v_lo, v_hi, v_t) = clamped(&t.vertical, vertical_deg);
    let (first, last) = (t.horizontal[0], t.horizontal[nh - 1]);
    let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let mut h = horizontal_deg.rem_euclid(360.0);
    let (h_lo, h_hi, h_t) = if nh == 1 {
        (0, 0, 0.0)
    } else {
        if near(last, 90.0) {
            if h > 180.0 {
                h = 360.0 - h;
            }
            if h > 90.0 {
                h = 180.0 - h;
            }
        } else if near(last, 180.0) {
            if h > 180.0 {
                h = 360.0 - h;
            }
        } else if near(first, 90.0) && near(last, 270.0) {
            if h < 90.0 {
                h = 180.0 - h;
            } else if h > 270.0 {
                h = 540.0 - h;
            }
        } else {
            if h < first {
                h += 360.0;
            }
            if h > last {
                return {
                    let along = |hi: usize| lerp(t.candela[hi * nv + v_lo], t.candela[hi * nv + v_hi], v_t);
                    lerp(along(nh - 1), along(0), fraction(last, first + 360.0, h))
                };
            }
        }
        clamped(&t.horizontal, h)
    };
    let along = |hi: usize| lerp(t.candela[hi * nv + v_lo], t.candela[hi * nv + v_hi], v_t);
    lerp(along(h_lo), along(h_hi), h_t)
}

/// A light at `position` aiming along `aim` with `table` (scaled to nothing: the table is the candela).
fn ies_light(position: Vec3, table: &Table, aim: Vec3, rotation_deg: f64) -> SceneLight {
    let mut light = SceneLight {
        kind: LightKind::Point,
        position,
        direction: aim,
        intensity: table.peak(),
        color: [1.0; 3],
        cos_inner: -1.0,
        cos_outer: -1.0,
        axis: light_axis(aim, rotation_deg),
        ies: Some(Arc::new(Default::default())),
        ..Default::default()
    };
    if let Some(p) = light.ies.as_mut().and_then(Arc::get_mut) {
        p.vertical = table.vertical.clone();
        p.horizontal = table.horizontal.clone();
        p.candela = table.candela.clone();
        p.lumens = -1.0;
    }
    light
}

/// The unit direction at `theta` from the aim and `phi` counter-clockwise from the axis (the
/// convention of `arch3d::light::ies_toward`: phi = atan2(q . (axis x aim), q . axis)).
fn direction_at(light: &SceneLight, theta_deg: f64, phi_deg: f64) -> Vec3 {
    let (t, p) = (theta_deg.to_radians(), phi_deg.to_radians());
    let side = light.axis.cross(light.direction);
    (light.direction * t.cos() + light.axis * (t.sin() * p.cos()) + side * (t.sin() * p.sin())).normalized()
}

#[test]
fn ies_candela_matches_arch_candela_at() {
    let Some(gpu) = gpu_or_skip() else { return };
    // The reference reproduces the documented number: 898.4 cd at 22.62 degrees of the test downlight.
    let downlight = Table::downlight();
    assert!((reference_candela(&downlight, 22.62, 0.0) - 898.4).abs() < 0.05);

    // Every shape of horizontal list (one angle, the full circle, a closed circle, 0..90, 0..180 and
    // 90..270), the lights tilted off the vertical and rotated so theta and phi are not the world's.
    let tables = [
        downlight,
        Table::varied(&[0.0, 30.0, 60.0, 90.0, 120.0, 150.0, 180.0], &[0.0, 90.0, 180.0, 270.0, 360.0], 1),
        Table::varied(&[0.0, 45.0, 90.0, 135.0, 180.0], &[0.0, 120.0, 240.0], 2),
        Table::varied(&[0.0, 45.0, 90.0, 135.0, 180.0], &[0.0, 45.0, 90.0], 3),
        Table::varied(&[0.0, 90.0, 180.0], &[0.0, 60.0, 120.0, 180.0], 4),
        Table::varied(&[0.0, 60.0, 120.0, 180.0], &[90.0, 180.0, 270.0], 5),
    ];
    let aim = Vec3::new(0.3, -0.2, -0.9).normalized();
    let lights: Vec<SceneLight> = tables
        .iter()
        .enumerate()
        .map(|(k, t)| ies_light(Vec3::new(10.0 * k as f64, 0.0, 3.0), t, if k == 0 { down() } else { aim }, 25.0 * k as f64))
        .collect();
    let scene = scene_of(&lights);
    assert_eq!(scene.lights.len(), tables.len());
    assert!(scene.lights.iter().all(|l| l.ies_v > 0 && l.ies_h > 0), "every table packed");

    let (mut probes, mut expected) = (Vec::new(), Vec::new());
    for (k, (light, table)) in lights.iter().zip(&tables).enumerate() {
        // 19 spread directions and the 22.62 degree one: 20 per table.
        let mut dirs: Vec<(f64, f64)> = (0..19).map(|i| (0.7 + 9.2 * f64::from(i), (17.0 + 53.0 * f64::from(i)) % 360.0)).collect();
        dirs.push((22.62, 0.0));
        assert_eq!(dirs.len(), 20);
        for (theta, phi) in dirs {
            probes.push(query_probe(IES_QUERY + k as f32, direction_at(light, theta, phi)));
            expected.push((k, theta, phi, reference_candela(table, theta, phi)));
        }
    }
    let out = run(&gpu, &scene, GpuFrame::default(), &probes);
    for ((k, theta, phi, want), got) in expected.iter().zip(&out) {
        let cd = f64::from(got[0]);
        assert!((cd - want).abs() <= want.abs() * 1e-3 + 1e-3, "table {k} at theta {theta} phi {phi}: {cd} cd, want {want}");
    }
    // The tables are not flat: more than one value per table, so the comparison means something.
    for k in 0..tables.len() {
        let mine: Vec<f64> = expected.iter().filter(|e| e.0 == k).map(|e| e.3).collect();
        assert!(mine.iter().any(|c| (c - mine[0]).abs() > 1.0), "table {k} is flat");
    }

    // Sampling an IES light gives li with the table's candela: luminance(li) = cd / d^2 on a surface
    // facing the light, whatever the cone fields say.
    let dist = 2.0;
    let mut sampled = Vec::new();
    let mut want = Vec::new();
    for (theta, phi) in [(0.0, 0.0), (22.62, 0.0), (50.0, 70.0), (100.0, 200.0)] {
        let to = direction_at(&lights[0], theta, phi);
        sampled.push(sample_probe(0, lights[0].position + to * dist, -to, 1));
        want.push(reference_candela(&tables[0], theta, phi) / (dist * dist));
    }
    for (got, want) in run(&gpu, &scene, GpuFrame::default(), &sampled).iter().zip(&want) {
        assert!((lux(*got) - want).abs() <= want * 1e-3 + 1e-3, "{} vs {want}", lux(*got));
    }
}

// ---------------------------------------------------------------------------------------------
// Area lights
// ---------------------------------------------------------------------------------------------

/// A width x length rectangle emitting like a Lambertian surface toward `direction`.
fn area_light(position: Vec3, direction: Vec3, rotation_deg: f64, size: [f64; 2], lumens: f64, color: [f64; 3]) -> SceneLight {
    SceneLight {
        kind: LightKind::Area,
        position,
        direction,
        intensity: lumens / std::f64::consts::PI,
        color,
        cos_inner: 1.0,
        cos_outer: 0.0,
        size,
        axis: light_axis(direction, rotation_deg),
        ..Default::default()
    }
}

/// The exact irradiance (lux) at `p` on a surface with unit normal `n` from `light`, and the mean of
/// the solid-angle pdf the shader's uniform rectangle sampling has, from an `cells x cells` midpoint grid.
fn rectangle_reference(light: &SceneLight, p: Vec3, n: Vec3, cells: usize) -> (f64, f64) {
    let (w, l) = (light.size[0], light.size[1]);
    let radiance = light.intensity / (w * l);
    let side = light.direction.cross(light.axis).normalized();
    let cell = w * l / (cells * cells) as f64;
    let (mut lux, mut pdf) = (0.0, 0.0);
    for i in 0..cells {
        for j in 0..cells {
            let (a, b) = ((i as f64 + 0.5) / cells as f64 - 0.5, (j as f64 + 0.5) / cells as f64 - 0.5);
            let q = light.position + light.axis * (a * w) + side * (b * l);
            let d = q - p;
            let d2 = d.dot(d);
            let dir = d * (1.0 / d2.sqrt());
            let (cos_s, cos_l) = (n.dot(dir).max(0.0), (-dir).dot(light.direction));
            if cos_l > 0.0 {
                lux += radiance * cell * cos_s * cos_l / d2;
                pdf += d2 / (w * l * cos_l);
            }
        }
    }
    (lux, pdf / (cells * cells) as f64)
}

#[test]
fn area_light_samples_integrate_to_its_irradiance() {
    let Some(gpu) = gpu_or_skip() else { return };
    // The LED panel 600 at the ceiling: 3600 lm over 0.6 x 0.6 m (3183 nits, 1145.9 cd along the aim),
    // in a 4000 K white; and a 0.4 x 1.2 m panel tilted and rotated, off to the side.
    let panel = area_light(Vec3::new(2.0, 2.0, 2.4), down(), 0.0, [0.6, 0.6], 3600.0, [1.0, 0.93, 0.82]);
    let strip = area_light(Vec3::new(8.0, 1.0, 2.4), Vec3::new(0.25, 0.1, -1.0).normalized(), 30.0, [0.4, 1.2], 2000.0, [1.0; 3]);
    let lights = [panel.clone(), strip.clone()];
    let scene = scene_of(&lights);
    assert_eq!(scene.lights.len(), 2);
    let nits = 3600.0 / std::f64::consts::PI / 0.36;
    assert!((lux([scene.lights[0].rgb[0], scene.lights[0].rgb[1], scene.lights[0].rgb[2], 0.0]) - nits).abs() < nits * 1e-4);

    let samples = 4096;
    let tilt = Vec3::new(0.2, 0.1, 1.0).normalized();
    let beside = Vec3::new(-1.0, 0.0, 0.0);
    let cases = [
        // (light, point, surface normal)
        (0, Vec3::new(2.0, 2.0, 2.0), up()),
        (0, Vec3::new(2.0, 2.0, 0.0), up()),
        (1, Vec3::new(8.4, 0.3, 1.1), tilt),
        (1, Vec3::new(8.9, 1.9, 0.7), beside),
        (1, Vec3::new(7.2, 0.5, 1.5), Vec3::new(0.3, 0.1, 0.9).normalized()),
    ];
    let probes: Vec<GpuProbe> = cases.iter().map(|&(k, p, n)| sample_probe(k, p, n, samples)).collect();
    let out = run(&gpu, &scene, GpuFrame::default(), &probes);

    // The panel's exact values: 4128.4 lx 0.4 m below its centre, 194.9 lx on the floor.
    assert!((lux(out[0]) - 4128.4).abs() < 41.3, "0.4 m below: {}", lux(out[0]));
    assert!((lux(out[1]) - 194.9).abs() < 1.95, "on the floor: {}", lux(out[1]));
    // Every case against a fine grid over the rectangle, and the mean pdf against the same grid's.
    for ((&(k, p, n), got), case) in cases.iter().zip(&out).zip(0..) {
        let (want, mean_pdf) = rectangle_reference(&lights[k], p, n, 128);
        assert!(want > 1.0, "case {case} is lit ({want})");
        assert!((lux(*got) - want).abs() < want * 0.01, "case {case}: {} lx, want {want}", lux(*got));
        assert!((f64::from(got[3]) - mean_pdf).abs() < mean_pdf * 0.02, "case {case}: mean pdf {} vs {mean_pdf}", got[3]);
    }
    // The colour is the fixture's: luminance is the radiance times the geometry; red leads blue.
    assert!(out[0][0] > out[0][2], "{:?}", out[0]);

    // Plain random numbers (a count that is not a square) give the same value, within noise.
    let random = run(&gpu, &scene, GpuFrame::default(), &[sample_probe(0, Vec3::new(2.0, 2.0, 2.0), up(), 20_000)]);
    assert!((lux(random[0]) - 4128.4).abs() < 4128.4 * 0.03, "{}", lux(random[0]));

    // Nothing behind the face (above the panel, facing down) and nothing onto a surface facing away.
    let dark = run(
        &gpu,
        &scene,
        GpuFrame::default(),
        &[sample_probe(0, Vec3::new(2.0, 2.0, 2.8), down(), 64), sample_probe(0, Vec3::new(2.0, 2.0, 0.0), down(), 64)],
    );
    assert_eq!(&dark[0][..3], &[0.0; 3], "behind the panel's face");
    assert_eq!(&dark[1][..3], &[0.0; 3], "a surface facing away from it");
    assert_eq!(dark[0][3], 1.0, "a sample that cannot light has pdf 1");
}

#[test]
fn area_radiance_is_the_light_radiance_in_front_of_its_face() {
    let Some(gpu) = gpu_or_skip() else { return };
    let panel = area_light(Vec3::new(2.0, 2.0, 2.4), down(), 0.0, [0.6, 0.6], 3600.0, [1.0; 3]);
    let pot = spot([0.0, 0.0, 2.4], 950.36, 30.0, 30.0);
    let scene = scene_of(&[panel, pot]);
    let nits = 3600.0 / std::f64::consts::PI / 0.36;
    let probes = [
        // A ray travelling up hits the face from below; one travelling down is behind it.
        query_probe(AREA_QUERY, up()),
        query_probe(AREA_QUERY, down()),
        // Sideways along the face: not in front of it.
        query_probe(AREA_QUERY, Vec3::new(1.0, 0.0, 0.0)),
        // A spot is no geometry: no radiance whatever the direction.
        query_probe(AREA_QUERY + 1.0, up()),
    ];
    let out = run(&gpu, &scene, GpuFrame::default(), &probes);
    assert!((f64::from(out[0][0]) - nits).abs() < nits * 1e-5, "{:?}", out[0]);
    assert_eq!(&out[1][..3], &[0.0; 3]);
    assert_eq!(&out[2][..3], &[0.0; 3]);
    assert_eq!(&out[3][..3], &[0.0; 3]);
}

// ---------------------------------------------------------------------------------------------
// Sky and sun
// ---------------------------------------------------------------------------------------------

fn elevation(deg: f64) -> Vec3 {
    let e = deg.to_radians();
    Vec3::new(e.cos(), 0.0, e.sin())
}

#[test]
fn sky_is_brighter_at_the_horizon_and_black_below() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = GpuScene::default();
    let sky = GpuFrame {
        flags: FRAME_SKY,
        sky_zenith: [SKY_ZENITH_NITS, SKY_ZENITH_NITS, SKY_ZENITH_NITS, 0.0],
        sky_horizon: [SKY_HORIZON_NITS, SKY_HORIZON_NITS, SKY_HORIZON_NITS, 0.0],
        ..Default::default()
    };
    let above = [0.0, 5.0, 15.0, 30.0, 45.0, 60.0, 75.0, 90.0];
    let below = [-0.5, -10.0, -45.0, -90.0];
    let probes: Vec<GpuProbe> = above.iter().chain(&below).map(|&e| query_probe(SKY, elevation(e))).collect();
    let out = run(&gpu, &scene, sky, &probes);
    // From the horizon's radiance to the zenith's: mix by sqrt(sin(elevation)), strictly darker upward.
    for (&e, got) in above.iter().zip(&out) {
        let t = e.to_radians().sin().max(0.0).sqrt();
        let want = f64::from(SKY_HORIZON_NITS) + (f64::from(SKY_ZENITH_NITS) - f64::from(SKY_HORIZON_NITS)) * t;
        for c in 0..3 {
            assert!((f64::from(got[c]) - want).abs() < want * 1e-4, "elevation {e}: {got:?}, want {want}");
        }
    }
    assert!(out[0][0] > out[1][0] && out[0][0] > out[7][0], "the horizon is the brightest part");
    assert!(out.windows(2).take(7).all(|w| w[0][0] > w[1][0]), "darker with elevation: {out:?}");
    assert!((out[0][0] - SKY_HORIZON_NITS).abs() < 0.5 && (out[7][0] - SKY_ZENITH_NITS).abs() < 0.5);
    for got in &out[above.len()..] {
        assert_eq!(&got[..3], &[0.0; 3], "nothing below the horizon");
    }

    // A coloured sky mixes per channel.
    let tinted =
        GpuFrame { flags: FRAME_SKY, sky_zenith: [1000.0, 2000.0, 3000.0, 0.0], sky_horizon: [4000.0, 5000.0, 6000.0, 0.0], ..Default::default() };
    let out = run(&gpu, &scene, tinted, &[query_probe(SKY, elevation(0.0)), query_probe(SKY, elevation(90.0)), query_probe(SKY, elevation(30.0))]);
    assert_eq!(&out[0][..3], &[4000.0, 5000.0, 6000.0]);
    assert!((out[1][0] - 1000.0).abs() < 0.1 && (out[1][1] - 2000.0).abs() < 0.1 && (out[1][2] - 3000.0).abs() < 0.1, "{:?}", out[1]);
    let t = 0.5f32.sqrt();
    assert!((out[2][1] - (5000.0 + (2000.0 - 5000.0) * t)).abs() < 0.5, "{:?}", out[2]);

    // No sky flag: black everywhere. The furnace flag wins over the sky and fills every direction.
    let probes = [query_probe(SKY, elevation(0.0)), query_probe(SKY, elevation(60.0)), query_probe(SKY, elevation(-60.0))];
    let off = run(&gpu, &scene, GpuFrame { sky_zenith: [9.0; 4], sky_horizon: [9.0; 4], ..Default::default() }, &probes);
    assert!(off.iter().all(|o| o[..3] == [0.0; 3]), "{off:?}");
    let furnace = GpuFrame {
        flags: FRAME_FURNACE | FRAME_SKY,
        furnace: [0.5, 1.0, 2.0, 0.0],
        sky_zenith: [9.0; 4],
        sky_horizon: [9.0; 4],
        ..Default::default()
    };
    let lit = run(&gpu, &scene, furnace, &probes);
    assert!(lit.iter().all(|o| o[..3] == [2.0; 3]), "{lit:?}");
}

#[test]
fn the_sun_sample_carries_the_frame_sun() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = GpuScene::default();
    let dir = [0.3f32, -0.4, 0.75f32.sqrt()];
    let unit = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let dir = [dir[0] / unit, dir[1] / unit, dir[2] / unit];
    let rgb = [SUN_LUX, SUN_LUX * 0.95, SUN_LUX * 0.9, 0.0];
    let on = GpuFrame { flags: FRAME_SUN, sun_dir: [dir[0], dir[1], dir[2], 0.0], sun_rgb: rgb, ..Default::default() };
    let probes = [query_probe(SUN_LI, up()), query_probe(SUN_DIR, up())];
    let out = run(&gpu, &scene, on, &probes);
    // Its illuminance on a surface facing it, toward the frame's sun, from infinitely far, a delta light.
    assert_eq!(&out[0][..3], &rgb[..3], "li is frame.sun_rgb");
    assert_eq!(out[0][3], 1.0, "delta");
    for k in 0..3 {
        assert!((out[1][k] - dir[k]).abs() < 1e-6, "dir {:?} vs {dir:?}", out[1]);
    }
    assert_eq!(out[1][3], 1.0, "the sun is at PT_INF");

    // With the sun off (a sky only, or night) the sample is black.
    let sky_only = GpuFrame { flags: FRAME_SKY, sun_dir: [dir[0], dir[1], dir[2], 0.0], sun_rgb: rgb, ..Default::default() };
    let night = GpuFrame { flags: 0, sun_dir: [dir[0], dir[1], dir[2], 0.0], sun_rgb: rgb, ..Default::default() };
    for frame in [sky_only, night] {
        let out = run(&gpu, &scene, frame, &probes);
        assert_eq!(&out[0][..3], &[0.0; 3], "no sun flag, no sun");
    }

    // Sun with the sky on is the same sun.
    let both = GpuFrame { flags: FRAME_SUN | FRAME_SKY, ..on };
    assert_eq!(&run(&gpu, &scene, both, &probes)[0][..3], &rgb[..3]);
}

// ---------------------------------------------------------------------------------------------
// Strips
// ---------------------------------------------------------------------------------------------

/// A strip light of `lm_per_m` lumens per metre along `a`..`b` at height `z`, emitting down, in
/// the pieces `cadcraft_arch::light::strip_samples` makes (at most 0.15 m).
fn strip_light(a: [f64; 2], b: [f64; 2], z: f64, lm_per_m: f64, colour: [f64; 3]) -> SceneLight {
    use cadcraft_arch3d::light::StripSample;
    let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    let pieces = ((len / 0.15).ceil() as usize).max(1);
    let samples: Vec<StripSample> = (0..pieces)
        .map(|k| {
            let t = (k as f64 + 0.5) / pieces as f64;
            StripSample {
                position: Vec3::new(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, z),
                normal: down(),
                intensity: lm_per_m * len / pieces as f64 / std::f64::consts::PI,
                ..Default::default()
            }
        })
        .collect();
    SceneLight {
        kind: LightKind::Strip,
        position: Vec3::new((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, z),
        direction: down(),
        color: colour,
        strip: Some(Arc::new(samples)),
        ..Default::default()
    }
}

#[test]
fn pt_strip_irradiance_matches_line_integral() {
    let Some(gpu) = gpu_or_skip() else { return };
    // Check 05's strip: 2 m of 500 lm/m at 2.4 m, in 3000 K white; 49.72 lx under its centre.
    let colour = [1.0, 0.76, 0.43];
    let strip = strip_light([1.0, 2.0], [3.0, 2.0], 2.4, 500.0, colour);
    let scene = scene_of(&[strip.clone()]);
    assert_eq!(scene.lights.len(), 1, "one light per straight segment");
    let l = scene.lights[0];
    assert_eq!(l.kind, super::scene_gpu::LIGHT_STRIP);
    assert!((l.length - 2.0).abs() < 1e-5 && (l.axis[0] - 1.0).abs() < 1e-6 && l.direction[2] == -1.0, "{l:?}");
    let want = 500.0 / std::f64::consts::PI * (1.0 / (2.4f64 * 2.4 + 1.0) + (1.0f64 / 2.4).atan() / 2.4);
    assert!((want - 49.72).abs() < 0.01, "{want}");
    let cases = [(Vec3::new(2.0, 2.0, 0.0), up()), (Vec3::new(2.0, 3.8, 0.0), up())];
    let probes: Vec<GpuProbe> = cases.iter().map(|&(p, n)| sample_probe(0, p, n, 4096)).collect();
    let out = run(&gpu, &scene, GpuFrame::default(), &probes);
    assert!((lux(out[0]) - want).abs() < want * 0.02, "{} lx, want {want}", lux(out[0]));
    // The second point against the CPU mirror of the raster sum.
    let runs = crate::lights::strip_runs(&strip);
    let segs: Vec<_> = runs.iter().map(|r| crate::lights::strip_seg(r, 1.0)).collect();
    let cpu = crate::lights::strip_irradiance_cpu(&segs, [2.0, 3.8, 0.0], [0.0, 0.0, 1.0]);
    let want2 = cpu * 1.0;
    // The CPU sum carries the candela per metre of the strip (500 / pi), with the same geometry.
    assert!((lux(out[1]) - want2).abs() < want2 * 0.03, "{} lx, want {want2}", lux(out[1]));
    // Nothing on a surface above the strip (behind its emitting side) or facing away.
    let dark = run(
        &gpu,
        &scene,
        GpuFrame::default(),
        &[sample_probe(0, Vec3::new(2.0, 2.0, 2.8), down(), 64), sample_probe(0, Vec3::new(2.0, 2.0, 0.0), down(), 64)],
    );
    assert!(lux(dark[0]) < 1e-6 && lux(dark[1]) < 1e-6, "{dark:?}");
}

fn strip_model(facing: StripFacing, path: Vec<Vec2>) -> ArchModel {
    let level = Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() };
    let room = Room {
        level: 0,
        polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)],
        ceiling_height: Some(2.4),
        ..Default::default()
    };
    let mut fixture = LightFixture::new(ArchId(1), "LED strip", Vec2::new(2.0, 2.0));
    fixture.shape = FixtureShape::Strip;
    fixture.kind = LightKind::Strip;
    fixture.mount = LightMount::Ceiling;
    fixture.level = 0;
    fixture.path = path;
    fixture.facing = facing;
    fixture.lumens_per_metre = 500.0;
    fixture.on = true;
    fixture.kelvin = 3000.0;
    ArchModel { levels: vec![level], rooms: vec![room], lights: vec![fixture], ..Default::default() }
}

#[test]
fn strip_with_its_housing_lights_the_floor_under_it() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = cadcraft_arch3d::build(&strip_model(StripFacing::Down, vec![Vec2::new(1.0, 2.0), Vec2::new(3.0, 2.0)]));
    let gpu_scene = build_gpu_scene(&scene, false, None);
    let probe = sample_probe(0, Vec3::new(2.0, 2.0, 0.0), up(), 1024);
    let out = run_irradiance(&gpu, &gpu_scene, 1024, &[probe]);
    let want = 49.72;
    assert!((lux_irradiance(out[0], 1024) - want).abs() < want * 0.02, "{} lx, want {want}", lux_irradiance(out[0], 1024));
}

#[test]
fn strip_at_the_ceiling_lights_the_floor_49_72_lux() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = cadcraft_arch3d::build(&strip_model(StripFacing::Down, vec![Vec2::new(1.0, 2.0), Vec2::new(3.0, 2.0)]));
    let gpu_scene = build_gpu_scene(&scene, false, None);
    let probe = sample_probe(0, Vec3::new(2.0, 2.0, 0.0), up(), 1024);
    let out = run_irradiance(&gpu, &gpu_scene, 1024, &[probe]);
    let want = 49.72;
    assert!((lux_irradiance(out[0], 1024) - want).abs() < want * 0.02, "{} lx, want {want}", lux_irradiance(out[0], 1024));
}

#[test]
fn strip_shadow_ray_ignores_its_own_housing_but_not_a_column() {
    let Some(gpu) = gpu_or_skip() else { return };
    let strip = strip_light([1.0, 2.0], [3.0, 2.0], 2.4, 500.0, [1.0, 0.76, 0.43]);
    let floor = prism("floor", 100, [-0.2, -0.2, -0.2], [4.2, 4.2, 0.0]);
    let housing = prism("light-housing", 101, [0.9, 1.9, 2.35], [3.1, 2.1, 2.45]);
    let column = prism("wall", 102, [0.8, 1.8, 0.0], [3.2, 2.2, 1.2]);
    let base = Scene3d { meshes: vec![floor.clone(), housing.clone()], lights: vec![strip.clone()], ..Default::default() };
    let probe = sample_probe(0, Vec3::new(2.0, 2.0, 0.0), up(), 4096);
    let clean = build_gpu_scene(&base, false, None);
    let out = run_irradiance(&gpu, &clean, 4096, &[probe]);
    assert!(lux_irradiance(out[0], 4096) > 45.0, "housing should not occlude the strip itself: {} lx", lux_irradiance(out[0], 4096));

    let hit = cadcraft_arch3d::pick::Bvh::build(&base).ray(Vec3::new(2.0, 2.0, 0.0), Vec3::new(0.0, 0.0, 1.0), 2.7);
    eprintln!("strip shadow ray from floor to sample hits: {hit:?}");

    let blocked = build_gpu_scene(&Scene3d { meshes: vec![floor, housing, column], lights: vec![strip], ..Default::default() }, false, None);
    let out = run_irradiance(&gpu, &blocked, 4096, &[probe]);
    assert!(lux_irradiance(out[0], 4096) < 1.0, "a real column must still shadow the strip: {} lx", lux_irradiance(out[0], 4096));
}

#[test]
fn out_facing_strip_with_housing_lights_the_wall_it_faces() {
    let Some(gpu) = gpu_or_skip() else { return };
    let model = strip_model(StripFacing::Out, vec![Vec2::new(0.0, 2.0), Vec2::new(4.0, 2.0)]);
    let scene = cadcraft_arch3d::build(&model);
    let gpu_scene = build_gpu_scene(&scene, false, None);
    let want = {
        let segs: Vec<_> = crate::lights::strip_runs(&scene.lights[0]).iter().map(|r| crate::lights::strip_seg(r, 1.0)).collect();
        crate::lights::strip_irradiance_cpu(&segs, [2.0, 0.2, 1.0], [0.0, 1.0, 0.0])
    };
    let out = run(&gpu, &gpu_scene, GpuFrame::default(), &[sample_probe(0, Vec3::new(2.0, 0.2, 1.0), Vec3::new(0.0, 1.0, 0.0), 1024)]);
    assert!(want > 0.0, "expected wall-side illumination from the out-facing strip");
    assert!((lux(out[0]) - want).abs() <= want * 0.02, "{} lx, want {want}", lux(out[0]));
}

#[test]
fn down_strip_gives_nothing_above_itself() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = cadcraft_arch3d::build(&strip_model(StripFacing::Down, vec![Vec2::new(1.0, 2.0), Vec2::new(3.0, 2.0)]));
    let gpu_scene = build_gpu_scene(&scene, false, None);
    let out = run(&gpu, &gpu_scene, GpuFrame::default(), &[sample_probe(0, Vec3::new(2.0, 2.0, 2.8), down(), 128)]);
    assert!(lux(out[0]) < 1e-6, "{} lx above a down strip", lux(out[0]));
}
