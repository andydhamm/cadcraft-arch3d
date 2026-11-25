//! Tests of the sun as a disc (`shaders/pt/sun_disc.wgsl`, `tracer::frame_sun`): the lux on open
//! ground, the width of a shadow edge, the point sun when the disc is 0, and the MIS of a glossy floor.

use super::device::ComputeGpu;
use super::scene_gpu::{FRAME_SUN, GpuFrame, GpuProbe, PHOTO_SUN_DISC, build_gpu_scene};
use super::tracer::{irradiance, run_entry, upload_scene};
use super::{PathOptions, SUN_LUX, luminance};
use crate::light::Light;
use crate::lit_frame::Lighting;
use crate::types::{ArchId, Mesh, Scene3d};

fn gpu_or_skip() -> Option<ComputeGpu> {
    let gpu = crate::gputest::shared_compute();
    if gpu.is_none() {
        eprintln!("SKIP: no compute device");
    }
    gpu
}

fn prism(object: u32, min: [f32; 3], max: [f32; 3]) -> Mesh {
    let mut m = Mesh::new("floor", ArchId(object));
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

fn ground() -> Mesh {
    prism(1, [-12.0, -12.0, -0.2], [12.0, 12.0, 0.0])
}

/// A sun `elevation` degrees up toward +x, a disc of `disc` degrees (0 = a point), no sky.
fn sun_lighting(elevation: f64, disc: f32) -> Lighting {
    let e = elevation.to_radians();
    let mut lighting = Lighting { sun: Some(Light::new([e.cos() as f32, 0.0, e.sin() as f32], 0.0)), sun_disc_deg: disc, ..Lighting::default() };
    lighting.look.sky.on = false;
    lighting
}

fn lux_at(gpu: &ComputeGpu, scene: &Scene3d, lighting: &Lighting, points: &[[f64; 3]], spp: u32) -> Vec<f64> {
    let probes: Vec<[[f64; 3]; 2]> = points.iter().map(|p| [*p, [0.0, 0.0, 1.0]]).collect();
    let opts = PathOptions { spp, bounces: 0, seed: 3, ..Default::default() };
    match irradiance(gpu, scene, lighting, false, &probes, &opts) {
        Ok(out) => out.iter().map(|v| f64::from(luminance(*v))).collect(),
        Err(e) => {
            assert!(false, "irradiance: {e}");
            Vec::new()
        }
    }
}

fn open_scene() -> Scene3d {
    let m = ground();
    Scene3d { bounds: m.bounds(), meshes: vec![m], lights: Vec::new(), ..Default::default() }
}

#[test]
fn disc_zero_is_the_point_sun() {
    let Some(gpu) = gpu_or_skip() else { return };
    // A box's shadow edge is sharp with disc 0: every probe is fully lit or fully dark.
    let mut scene = open_scene();
    scene.meshes.push(prism(2, [-0.15, -0.15, 0.0], [0.15, 0.15, 3.0]));
    let points: Vec<[f64; 3]> = (0..40).map(|k| [-5.2, 0.15 - 0.04 + 0.002 * k as f64, 0.001]).collect();
    let point = sun_lighting(30.0, 0.0);
    let a = lux_at(&gpu, &scene, &point, &points, 64);
    let full = f64::from(SUN_LUX) * 0.5;
    for (i, v) in a.iter().enumerate() {
        assert!((v - full).abs() < full * 1e-3 || v.abs() < 1.0, "probe {i}: {v} is neither lit ({full}) nor dark");
    }
    assert!(a.iter().any(|v| *v < 1.0) && a.iter().any(|v| *v > full * 0.99), "both sides of the edge are sampled");
    // The same seed gives the same numbers, and a negative disc is no disc.
    assert_eq!(a, lux_at(&gpu, &scene, &point, &points, 64));
    assert_eq!(a, lux_at(&gpu, &scene, &sun_lighting(30.0, -1.0), &points, 64));
}

#[test]
fn open_ground_lux_is_unchanged_by_the_disc() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = open_scene();
    let points = [[0.0, 0.0, 0.001], [3.0, -2.0, 0.001]];
    let point = lux_at(&gpu, &scene, &sun_lighting(30.0, 0.0), &points, 256);
    let disc = lux_at(&gpu, &scene, &sun_lighting(30.0, 0.53), &points, 256);
    let want = f64::from(SUN_LUX) * 0.5;
    for (p, d) in point.iter().zip(&disc) {
        assert!((p - want).abs() < want * 0.01, "point sun {p} lx, want {want}");
        assert!((d - p).abs() < p * 0.01, "disc {d} lx vs point {p} lx");
    }
}

#[test]
fn penumbra_width_matches_the_disc() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut scene = open_scene();
    scene.meshes.push(prism(2, [-0.15, -0.15, 0.0], [0.15, 0.15, 3.0]));
    // Near the tip of the shadow (the sun's ray grazes the box's top about 6 m from the ground) the
    // lateral edge at y = 0.15 is blurred by the disc: the lit fraction goes from 10 % to 90 % over
    // 0.687 x L x tan(0.53 deg), L the distance to the edge.
    let x = -5.2;
    let n = 81;
    let y0 = 0.15 - 0.08;
    let step = 0.16 / (n - 1) as f64;
    let points: Vec<[f64; 3]> = (0..n).map(|k| [x, y0 + step * k as f64, 0.001]).collect();
    let lit = lux_at(&gpu, &scene, &sun_lighting(30.0, 0.53), &points, 4096);
    let full = f64::from(SUN_LUX) * 0.5;
    let frac: Vec<f64> = lit.iter().map(|v| v / full).collect();
    assert!(frac[0] < 0.02 && frac[n - 1] > 0.98, "the edge is inside the strip: {} .. {}", frac[0], frac[n - 1]);
    let cross = |level: f64| -> f64 {
        for k in 1..n {
            if frac[k - 1] < level && frac[k] >= level {
                let t = (level - frac[k - 1]) / (frac[k] - frac[k - 1]);
                return y0 + step * (k - 1) as f64 + step * t;
            }
        }
        f64::NAN
    };
    let width = cross(0.9) - cross(0.1);
    let dist = (x.abs() - 0.15) / 30f64.to_radians().cos();
    let want = 0.687 * dist * 0.53f64.to_radians().tan();
    assert!((width - want).abs() < want * 0.15, "penumbra {width} m, want {want} m");
}

#[test]
fn glossy_floor_sees_the_sun_disc_with_mis() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut scene = build_gpu_scene(&Scene3d { bounds: ground().bounds(), meshes: vec![ground()], ..Default::default() }, false, None);
    for m in &mut scene.materials {
        m.albedo = [0.3; 3];
        m.metallic = 0.0;
        m.roughness = 0.25;
    }
    // The camera sits in the mirror direction of the sun and looks at the origin.
    let s = [0.6f32, 0.0, 0.8];
    let wo = [-s[0], 0.0, s[2]];
    let eye = [wo[0] * 3.0, 0.0, wo[2] * 3.0];
    let f = [-wo[0], 0.0, -wo[2]];
    let (r, u) = ([0.0, -1.0, 0.0], [0.8, 0.0, 0.6]);
    let a = -0.5;
    let base = GpuFrame {
        inv_view_proj: [
            [r[0], r[1], r[2], 0.0],
            [u[0], u[1], u[2], 0.0],
            [eye[0] * a, eye[1] * a, eye[2] * a, a],
            [eye[0] + f[0], eye[1] + f[1], eye[2] + f[2], 1.0],
        ],
        eye: [eye[0], eye[1], eye[2], 1.0],
        size: [8, 8],
        probe_count: 64,
        spp_this_pass: 128,
        seed: 9,
        flags: FRAME_SUN,
        sun_dir: [s[0], s[1], s[2], 0.0],
        sun_rgb: [SUN_LUX, SUN_LUX, SUN_LUX, 0.0],
        ..Default::default()
    };
    let half = (0.53f64 / 2.0).to_radians().cos() as f32;
    let mean = |bounces: u32, disc: bool| -> f64 {
        let mut frame = GpuFrame { bounces, ..base };
        if disc {
            frame.photo[0] |= PHOTO_SUN_DISC;
            frame.photo_f[1] = half;
        }
        let buffers = upload_scene(&gpu, &scene);
        let out = match run_entry(&gpu, &buffers, "trace_main", &frame, &vec![GpuProbe::default(); 64]) {
            Ok(o) => o,
            Err(e) => {
                assert!(false, "trace_main: {e}");
                return 0.0;
            }
        };
        out.iter().map(|a| f64::from(luminance([a[0], a[1], a[2]])) / f64::from(a[3])).sum::<f64>() / 64.0
    };
    let point = mean(1, false);
    assert!(point > 1.0, "the floor is lit: {point}");
    // The disc, direct samples weighted against the BSDF rays that reach it, is the point sun's
    // glossy highlight within noise; a double count would be well above it.
    let with_mis = mean(1, true);
    assert!((with_mis - point).abs() < point * 0.08, "disc with MIS {with_mis} vs point sun {point}");
}

#[test]
fn nan_disc_is_treated_as_no_disc() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = open_scene();
    let points = [[0.0, 0.0, 0.001]];
    let nan = lux_at(&gpu, &scene, &sun_lighting(30.0, f32::NAN), &points, 256);
    let zero = lux_at(&gpu, &scene, &sun_lighting(30.0, 0.0), &points, 256);
    let want = f64::from(SUN_LUX) * 0.5;
    for (n, z) in nan.iter().zip(&zero) {
        assert!((n - want).abs() < want * 0.01, "NaN disc {n} lx, want {want}");
        assert!((z - want).abs() < want * 0.01, "zero disc {z} lx, want {want}");
        assert!((n - z).abs() < z * 0.01, "NaN disc {n} vs zero disc {z}");
    }
}

#[test]
fn disc_larger_than_ninety_is_clamped() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = open_scene();
    let points = [[0.0, 0.0, 0.001]];
    let big = lux_at(&gpu, &scene, &sun_lighting(30.0, 180.0), &points, 256);
    let ninety = lux_at(&gpu, &scene, &sun_lighting(30.0, 90.0), &points, 256);
    // A disc larger than 90° is clamped to 90°, so it must match the 90° disc.
    for (b, n) in big.iter().zip(&ninety) {
        assert!((b - n).abs() < n * 0.01, "180 disc {b} vs 90 disc {n} (clamping failed)");
    }
}

#[test]
fn disc_with_sun_below_horizon_gives_no_light() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = open_scene();
    let points = [[0.0, 0.0, 0.001]];
    let below = lux_at(&gpu, &scene, &sun_lighting(-10.0, 0.53), &points, 64);
    for v in below.iter() {
        assert!(*v < 1.0, "sun below horizon should give no light: {v} lx");
    }
}

#[test]
fn disc_render_is_deterministic_with_same_seed() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = open_scene();
    let points = [[0.0, 0.0, 0.001], [1.0, 0.0, 0.001], [2.0, 0.0, 0.001]];
    let lighting = sun_lighting(30.0, 0.53);
    let a = lux_at(&gpu, &scene, &lighting, &points, 64);
    let b = lux_at(&gpu, &scene, &lighting, &points, 64);
    assert_eq!(a, b, "same seed must give same disc irradiance");
}

#[test]
fn very_small_disc_approaches_point_sun() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = open_scene();
    let points = [[0.0, 0.0, 0.001]];
    let tiny = lux_at(&gpu, &scene, &sun_lighting(30.0, 0.001), &points, 256);
    let point = lux_at(&gpu, &scene, &sun_lighting(30.0, 0.0), &points, 256);
    let want = f64::from(SUN_LUX) * 0.5;
    for (t, p) in tiny.iter().zip(&point) {
        assert!((t - want).abs() < want * 0.01, "tiny disc {t} lx, want {want}");
        assert!((p - want).abs() < want * 0.01, "point sun {p} lx, want {want}");
        assert!((t - p).abs() < p * 0.01, "tiny disc {t} vs point {p}");
    }
}
