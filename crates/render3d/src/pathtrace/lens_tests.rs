//! Tests of the path tracer's lens (`shaders/pt/lens.wgsl`, `lens.rs`): `camera_ray` is driven through
//! its test entry `lens_test_main` (one probe per ray: pixel x, y; normal.x picks direction or origin)
//! and compared with `cadcraft_arch3d::lens`, the reference. They skip with a note without an adapter.

use super::device::ComputeGpu;
use super::lens::fill_frame;
use super::scene_gpu::{GpuFrame, GpuProbe, GpuScene, PHOTO_DOF, PHOTO_PANINI, PHOTO_SHIFT};
use super::tracer::{run_entry, upload_scene};
use crate::camera::{Camera, FullCamera};
use crate::lens::LensOptions;
use crate::lit_frame::Lighting;
use cadcraft_arch3d::lens::{LensView, blur_diameter_px, camera_basis, dir_to_frame, ray_at_lens};
use cadcraft_geom::Vec3;

const SIZE: (u32, u32) = (200, 100);

fn gpu_or_skip() -> Option<ComputeGpu> {
    let gpu = crate::gputest::shared_compute();
    if gpu.is_none() {
        eprintln!("SKIP: no compute device");
    }
    gpu
}

/// A level camera at (1, -2, 1.5) looking along +y at (1, 3, 1.5): 60 degrees vertical.
fn camera() -> Camera {
    Camera::Full(FullCamera::new(Vec3::new(1.0, -2.0, 1.5), Vec3::new(1.0, 3.0, 1.5), 60.0))
}

fn frame_for(camera: &Camera, lens: LensOptions, probes: usize) -> GpuFrame {
    let eye = camera.eye();
    let mut frame = GpuFrame {
        eye: [eye.x as f32, eye.y as f32, eye.z as f32, 1.0],
        size: [SIZE.0, SIZE.1],
        probe_count: probes as u32,
        seed: 3,
        ..Default::default()
    };
    // The pinhole path reads inv_view_proj: ray = fwd + right x tan_h + up y tan_v (see kernel_tests).
    let (r, u, f) = camera_basis(camera);
    let tan_v = (camera.fov_deg().to_radians() * 0.5).tan();
    let tan_h = tan_v * f64::from(SIZE.0) / f64::from(SIZE.1);
    let e = [eye.x as f32, eye.y as f32, eye.z as f32];
    let a = -0.5f32;
    let col = |v: [f64; 3], s: f64| [(v[0] * s) as f32, (v[1] * s) as f32, (v[2] * s) as f32, 0.0];
    frame.inv_view_proj =
        [col(r, tan_h), col(u, tan_v), [e[0] * a, e[1] * a, e[2] * a, a], [e[0] + f[0] as f32, e[1] + f[1] as f32, e[2] + f[2] as f32, 1.0]];
    let lighting = Lighting { lens, ..Lighting::default() };
    fill_frame(&mut frame, &lighting, camera, SIZE);
    frame
}

/// The direction (`origin` false) or origin of the camera ray through each pixel.
fn rays(gpu: &ComputeGpu, frame: &GpuFrame, pixels: &[[f32; 2]], origin: bool) -> Vec<[f32; 3]> {
    let probes: Vec<GpuProbe> = pixels
        .iter()
        .map(|p| GpuProbe { point: [p[0], p[1], 0.0], normal: [if origin { 1.0 } else { 0.0 }, 0.0, 0.0], ..Default::default() })
        .collect();
    let frame = GpuFrame { probe_count: probes.len() as u32, ..*frame };
    let buffers = upload_scene(gpu, &GpuScene::default());
    match run_entry(gpu, &buffers, "lens_test_main", &frame, &probes) {
        Ok(out) => out.iter().map(|v| [v[0], v[1], v[2]]).collect(),
        Err(e) => {
            assert!(false, "lens_test_main: {e}");
            Vec::new()
        }
    }
}

fn grid() -> Vec<[f32; 2]> {
    let mut v = Vec::new();
    for j in 0..9 {
        for i in 0..9 {
            v.push([0.5 + i as f32 * 24.0, 0.5 + j as f32 * 12.0]);
        }
    }
    v
}

fn dot(a: [f32; 3], b: [f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[test]
fn pinhole_image_is_identical_with_the_lens_neutral() {
    // Whatever the compression and focus say, a lens with no panini, no shift and no aperture is the pinhole.
    let neutral = LensOptions { compression: 0.3, focus: Some(4.0), ..LensOptions::default() };
    let frame = frame_for(&camera(), neutral, 0);
    assert_eq!(frame.photo, [0; 4]);
    assert_eq!(frame.lens, [0.0; 4]);
    // An f-number that gives no aperture (not finite or not positive) is no depth of field either.
    for n in [Some(0.0), Some(-2.0), Some(f32::NAN)] {
        let f = frame_for(&camera(), LensOptions { aperture: n, ..LensOptions::default() }, 0);
        assert_eq!(f.photo[0] & PHOTO_DOF, 0, "{n:?}");
    }
    let Some(gpu) = gpu_or_skip() else { return };
    let px = grid();
    let out = rays(&gpu, &frame, &px, false);
    let origins = rays(&gpu, &frame, &px, true);
    let cam = camera();
    for ((p, d), o) in px.iter().zip(&out).zip(&origins) {
        let Some((eye, want)) = cadcraft_arch3d::camera::ray_at(&cam, SIZE, *p) else {
            assert!(false, "no ray at {p:?}");
            return;
        };
        let want = [want.x as f32, want.y as f32, want.z as f32];
        for k in 0..3 {
            assert!((d[k] - want[k]).abs() < 1e-4, "pixel {p:?}: {d:?} vs {want:?}");
        }
        assert!((o[0] - eye.x as f32).abs() < 1e-4 && (o[1] - eye.y as f32).abs() < 1e-4 && (o[2] - eye.z as f32).abs() < 1e-4);
    }
}

#[test]
fn panini_directions_match_the_cpu_reference() {
    let Some(gpu) = gpu_or_skip() else { return };
    let cam = camera();
    let px = grid();
    for (compression, shift) in [(0.0, 0.0), (0.3, 0.0), (0.7, 0.1), (1.0, -0.15), (3.0, 0.0)] {
        let opts = LensOptions { panini: true, compression, shift, ..LensOptions::default() };
        let frame = frame_for(&cam, opts, 0);
        assert_eq!(frame.photo[0] & PHOTO_PANINI, PHOTO_PANINI);
        assert_eq!(frame.photo[0] & PHOTO_SHIFT != 0, shift != 0.0);
        let view = LensView { panini: true, compression: f64::from(compression), shift: f64::from(shift) };
        let out = rays(&gpu, &frame, &px, false);
        for (p, d) in px.iter().zip(&out) {
            let Some((_, want)) = ray_at_lens(&cam, &view, SIZE, *p) else {
                assert!(false, "no reference ray at {p:?}");
                return;
            };
            let want = [want.x as f32, want.y as f32, want.z as f32];
            for k in 0..3 {
                assert!((d[k] - want[k]).abs() < 1e-4, "d {compression} shift {shift} pixel {p:?}: {d:?} vs {want:?}");
            }
        }
    }
}

#[test]
fn vertical_bars_keep_their_column_under_panini() {
    let Some(gpu) = gpu_or_skip() else { return };
    let cam = camera();
    let (tan_v, aspect) = ((30.0f64).to_radians().tan(), f64::from(SIZE.0) / f64::from(SIZE.1));
    let tan_h = tan_v * aspect;
    for compression in [0.3f32, 0.7, 1.0] {
        let opts = LensOptions { panini: true, compression, ..LensOptions::default() };
        let frame = frame_for(&cam, opts, 0);
        let rows: Vec<[f32; 2]> = (0..20).map(|j| [30.5, 2.5 + j as f32 * 4.9]).collect();
        let out = rays(&gpu, &frame, &rows, false);
        let view = LensView { panini: true, compression: f64::from(compression), shift: 0.0 };
        for d in &out {
            let c = [f64::from(dot(*d, frame.cam_right)), f64::from(dot(*d, frame.cam_up)), f64::from(dot(*d, frame.cam_fwd))];
            let Some(n) = dir_to_frame(&view, tan_h, tan_v, c) else {
                assert!(false, "no frame position for {d:?}");
                return;
            };
            let want = 2.0 * 30.5 / f64::from(SIZE.0) - 1.0;
            assert!((n[0] - want).abs() < 1e-4, "compression {compression}: column {} vs {want}", n[0]);
        }
    }
}

#[test]
fn shift_moves_the_horizon_a_quarter_frame() {
    let Some(gpu) = gpu_or_skip() else { return };
    let cam = camera();
    let (_, up, fwd) = {
        let (_, u, f) = camera_basis(&cam);
        ((), u, f)
    };
    let h = SIZE.1 as f32;
    let flat = frame_for(&cam, LensOptions::default(), 0);
    let shifted = frame_for(&cam, LensOptions { shift: 0.25, ..LensOptions::default() }, 0);
    assert_eq!(shifted.photo[0], PHOTO_SHIFT);
    let level = |frame: &GpuFrame, row: f32| {
        let d = rays(&gpu, frame, &[[SIZE.0 as f32 / 2.0, row]], false)[0];
        // The slope of the ray above the horizon (the camera is level, so forward is the horizon).
        (
            f64::from(d[0]) * up[0] + f64::from(d[1]) * up[1] + f64::from(d[2]) * up[2],
            f64::from(d[0]) * fwd[0] + f64::from(d[1]) * fwd[1] + f64::from(d[2]) * fwd[2],
        )
    };
    let (flat_up, _) = level(&flat, h / 2.0);
    assert!(flat_up.abs() < 1e-4, "no shift: the middle row is the horizon: {flat_up}");
    let (at_quarter, along) = level(&shifted, 0.75 * h);
    assert!(at_quarter.abs() < 1e-4 && along > 0.99, "shift 0.25: the horizon is a quarter of the height lower: {at_quarter}");
    let (middle, _) = level(&shifted, h / 2.0);
    assert!(middle > 0.1, "the middle row looks up: {middle}");
}

/// The lateral spread of the rays of the centre pixel where they cross the plane `distance` along the
/// view axis, in pixels of that plane: (largest offset of the mean from the axis, second-moment diameter).
fn spread_px(gpu: &ComputeGpu, frame: &GpuFrame, distance: f64, n: usize) -> (f64, f64) {
    let centre = [SIZE.0 as f32 / 2.0, SIZE.1 as f32 / 2.0];
    let px = vec![centre; n];
    let dirs = rays(gpu, frame, &px, false);
    let origins = rays(gpu, frame, &px, true);
    let cam = camera();
    let (right, up, fwd) = camera_basis(&cam);
    let eye = cam.eye();
    let eye = [eye.x, eye.y, eye.z];
    let tan_h = f64::from(frame.lens2[0]);
    let pixel = 2.0 * tan_h * distance / f64::from(SIZE.0);
    let (mut sx, mut sy, mut sxx) = (0.0, 0.0, 0.0);
    for (d, o) in dirs.iter().zip(&origins) {
        let d = [f64::from(d[0]), f64::from(d[1]), f64::from(d[2])];
        let o = [f64::from(o[0]), f64::from(o[1]), f64::from(o[2])];
        let along = |a: [f64; 3], v: [f64; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
        let rel = [o[0] - eye[0], o[1] - eye[1], o[2] - eye[2]];
        let t = (distance - along(fwd, rel)) / along(fwd, d);
        let hit = [rel[0] + d[0] * t, rel[1] + d[1] * t, rel[2] + d[2] * t];
        let (x, y) = (along(right, hit) / pixel, along(up, hit) / pixel);
        sx += x;
        sy += y;
        sxx += x * x + y * y;
    }
    let m = n as f64;
    // The second moment of a uniform disc of radius R about its centre is R^2 / 2.
    let var = sxx / m - (sx / m).powi(2) - (sy / m).powi(2);
    ((sx / m).abs().max((sy / m).abs()), 2.0 * (2.0 * var).sqrt())
}

/// A narrow field (10 degrees vertical), so that the blur is several pixels at 200 x 100.
fn dof_frame(f_number: f32, focus: f32) -> GpuFrame {
    let narrow = Camera::Full(FullCamera::new(Vec3::new(1.0, -2.0, 1.5), Vec3::new(1.0, 3.0, 1.5), 10.0));
    frame_for(&narrow, LensOptions { aperture: Some(f_number), focus: Some(focus), ..LensOptions::default() }, 0)
}

#[test]
fn in_focus_point_stays_sharp() {
    let Some(gpu) = gpu_or_skip() else { return };
    let frame = dof_frame(1.4, 3.0);
    assert_eq!(frame.photo[0], PHOTO_DOF);
    assert!((frame.lens[3] - 3.0).abs() < 1e-6 && frame.lens[2] > 0.0);
    let (mean, diameter) = spread_px(&gpu, &frame, 3.0, 4096);
    assert!(mean < 1.5 && diameter < 1.5, "the speck at the focus distance: offset {mean:.3} px, diameter {diameter:.3} px");
}

#[test]
fn thin_lens_blur_matches_the_formula() {
    let Some(gpu) = gpu_or_skip() else { return };
    let (n, focus) = (2.8f32, 3.0f32);
    let frame = dof_frame(n, focus);
    let tan_h = f64::from(frame.lens2[0]);
    for distance in [1.5, 9.0] {
        let (_, got) = spread_px(&gpu, &frame, distance, 8192);
        let want = blur_diameter_px(tan_h, f64::from(n), f64::from(focus), distance, SIZE.0);
        assert!(want > 2.0, "the case blurs: {want}");
        assert!((got - want).abs() <= 0.1 * want, "distance {distance}: measured {got:.3} px, formula {want:.3} px");
    }
}

#[test]
fn the_target_distance_is_the_default_focus() {
    // No focus: the distance from the eye to the target along the forward axis (5 m here).
    let frame = frame_for(&camera(), LensOptions { aperture: Some(2.8), ..LensOptions::default() }, 0);
    assert!((frame.lens[3] - 5.0).abs() < 1e-4, "{:?}", frame.lens);
    let f = frame_for(&camera(), LensOptions { aperture: Some(2.8), focus: Some(0.0), ..LensOptions::default() }, 0);
    assert!((f.lens[3] - 5.0).abs() < 1e-4, "a non-positive focus is the default");
}
