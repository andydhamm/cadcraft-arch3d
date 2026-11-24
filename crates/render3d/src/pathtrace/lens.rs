//! The path tracer's camera frame (phase 22): the basis, field of view and lens the WGSL `camera_ray`
//! (`shaders/pt/lens.wgsl`) needs for a lens that is not a plain pinhole. [`fill_frame`] writes
//! `cam_right`, `cam_up`, `cam_fwd`, `lens2`, `lens` and the `PHOTO_PANINI` / `PHOTO_SHIFT` /
//! `PHOTO_DOF` flags from `lighting.lens`. A neutral lens leaves `lens` and the flags at zero, which
//! is the pinhole code path.

use super::scene_gpu::{GpuFrame, PHOTO_DOF, PHOTO_PANINI, PHOTO_SHIFT};
use crate::camera::Camera;
use crate::lens::LensOptions;
use crate::lit_frame::Lighting;
use cadcraft_arch3d::lens::{aperture_radius_m, camera_basis};

/// Set the camera basis (unit right, up and forward, w = 0), the tangents of the half field of view
/// (`lens2.x` horizontal, `lens2.y` vertical), the lens vector (`lens` = panini compression, shift,
/// aperture radius in metres, focus distance in metres) and the lens flags of `camera` at `size`
/// pixels. A camera with a non-finite number leaves the frame as it was.
pub(super) fn fill_frame(frame: &mut GpuFrame, lighting: &Lighting, camera: &Camera, size: (u32, u32)) {
    let eye = camera.eye();
    let target = camera.target();
    let fov = camera.fov_deg();
    if !eye.is_finite() || !target.is_finite() || !fov.is_finite() {
        return;
    }
    let (right, up, fwd) = camera_basis(camera);
    let tan_v = (fov.to_radians() * 0.5).tan();
    let aspect = f64::from(size.0.max(1)) / f64::from(size.1.max(1));
    let tan_h = tan_v * aspect;
    let all = [right[0], right[1], right[2], up[0], up[1], up[2], fwd[0], fwd[1], fwd[2], tan_v, tan_h];
    if !all.iter().all(|v| v.is_finite()) || tan_h <= 0.0 || tan_v <= 0.0 {
        return;
    }
    let as4 = |v: [f64; 3]| [v[0] as f32, v[1] as f32, v[2] as f32, 0.0];
    frame.cam_right = as4(right);
    frame.cam_up = as4(up);
    frame.cam_fwd = as4(fwd);
    frame.lens2 = [tan_h as f32, tan_v as f32, 0.0, 0.0];

    let opts = &lighting.lens;
    let mut flags = 0;
    let mut lens = [0.0f32; 4];
    if opts.panini && opts.compression.is_finite() {
        flags |= PHOTO_PANINI;
        lens[0] = opts.compression.clamp(0.0, 1.0e6);
    }
    if opts.shift.is_finite() && opts.shift != 0.0 {
        flags |= PHOTO_SHIFT;
        lens[1] = opts.shift;
    }
    if let Some(n) = opts.aperture {
        let radius = aperture_radius_m(tan_h, f64::from(n));
        if radius.is_finite() && radius > 0.0 {
            // The look's focus, or the distance from the eye to the target along the forward axis.
            let focus = match opts.focus {
                Some(f) if f.is_finite() && f > 0.0 => f64::from(f),
                _ => {
                    let (eye, target) = (camera.eye(), camera.target());
                    (target.x - eye.x) * fwd[0] + (target.y - eye.y) * fwd[1] + (target.z - eye.z) * fwd[2]
                }
            };
            if focus.is_finite() {
                flags |= PHOTO_DOF;
                lens[2] = radius as f32;
                lens[3] = focus.max(0.01) as f32;
            }
        }
    }
    frame.photo[0] |= flags;
    frame.lens = lens;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Orbit;
    use cadcraft_geom::Vec3;

    #[test]
    fn the_basis_is_orthonormal_and_the_fov_matches() {
        let camera = Camera::Orbit(Orbit::new(Vec3::new(0.0, 0.0, 0.0), 30.0, 20.0, 10.0, 60.0));
        let mut frame = GpuFrame::default();
        fill_frame(&mut frame, &Lighting::default(), &camera, (200, 100));
        let dot = |a: [f32; 4], b: [f32; 4]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        for v in [frame.cam_right, frame.cam_up, frame.cam_fwd] {
            assert!((dot(v, v) - 1.0).abs() < 1e-5);
        }
        assert!(dot(frame.cam_right, frame.cam_up).abs() < 1e-5);
        assert!(dot(frame.cam_right, frame.cam_fwd).abs() < 1e-5);
        assert!((frame.lens2[1] - 30f32.to_radians().tan()).abs() < 1e-5);
        assert!((frame.lens2[0] - 2.0 * frame.lens2[1]).abs() < 1e-5);
        assert_eq!(frame.lens, [0.0; 4]);
        assert_eq!(frame.photo, [0; 4]);
    }

    #[test]
    fn lens_flags_and_values() {
        let camera = Camera::Orbit(Orbit::new(Vec3::new(0.0, 0.0, 0.0), 30.0, 20.0, 10.0, 60.0));
        let mut frame = GpuFrame::default();
        let lighting = Lighting { lens: LensOptions { panini: true, compression: 0.5, ..LensOptions::default() }, ..Lighting::default() };
        fill_frame(&mut frame, &lighting, &camera, (200, 100));
        assert_eq!(frame.photo[0], PHOTO_PANINI);
        assert!((frame.lens[0] - 0.5).abs() < 1e-6);
        assert_eq!(frame.lens[1], 0.0);
        assert_eq!(frame.lens[2], 0.0);
        assert_eq!(frame.lens[3], 0.0);
        let mut frame = GpuFrame::default();
        let lighting = Lighting { lens: LensOptions { shift: 0.1, ..LensOptions::default() }, ..Lighting::default() };
        fill_frame(&mut frame, &lighting, &camera, (200, 100));
        assert_eq!(frame.photo[0], PHOTO_SHIFT);
        assert_eq!(frame.lens[0], 0.0);
        assert!((frame.lens[1] - 0.1).abs() < 1e-6);
        assert_eq!(frame.lens[2], 0.0);
        assert_eq!(frame.lens[3], 0.0);
        let mut frame = GpuFrame::default();
        let lighting = Lighting { lens: LensOptions { aperture: Some(2.8), focus: Some(3.0), ..LensOptions::default() }, ..Lighting::default() };
        fill_frame(&mut frame, &lighting, &camera, (200, 100));
        assert_eq!(frame.photo[0], PHOTO_DOF);
        assert_eq!(frame.lens[0], 0.0);
        assert_eq!(frame.lens[1], 0.0);
        assert!(frame.lens[2] > 0.0);
        assert!((frame.lens[3] - 3.0).abs() < 1e-6);
        let mut frame = GpuFrame::default();
        let lighting = Lighting {
            lens: LensOptions { panini: true, compression: 0.5, shift: 0.1, aperture: Some(2.8), focus: Some(3.0), ..LensOptions::default() },
            ..Lighting::default()
        };
        fill_frame(&mut frame, &lighting, &camera, (200, 100));
        assert_eq!(frame.photo[0], PHOTO_PANINI | PHOTO_SHIFT | PHOTO_DOF);
        assert!((frame.lens[0] - 0.5).abs() < 1e-6);
        assert!((frame.lens[1] - 0.1).abs() < 1e-6);
        assert!(frame.lens[2] > 0.0);
        assert!((frame.lens[3] - 3.0).abs() < 1e-6);
    }

    #[test]
    fn aperture_radius_matches_formula() {
        let camera = Camera::Orbit(Orbit::new(Vec3::new(0.0, 0.0, 0.0), 30.0, 20.0, 10.0, 60.0));
        let mut frame = GpuFrame::default();
        let lighting = Lighting { lens: LensOptions { aperture: Some(2.8), focus: Some(3.0), ..LensOptions::default() }, ..Lighting::default() };
        fill_frame(&mut frame, &lighting, &camera, (200, 100));
        let expected_radius = frame.lens2[0] / 2.8;
        assert!((frame.lens[2] - expected_radius).abs() < 1e-6, "aperture radius {}/{}", frame.lens[2], expected_radius);
    }

    #[test]
    fn nan_compression_is_ignored() {
        let camera = Camera::Orbit(Orbit::new(Vec3::new(0.0, 0.0, 0.0), 30.0, 20.0, 10.0, 60.0));
        let mut frame = GpuFrame::default();
        let lighting = Lighting { lens: LensOptions { panini: true, compression: f32::NAN, ..LensOptions::default() }, ..Lighting::default() };
        fill_frame(&mut frame, &lighting, &camera, (200, 100));
        assert_eq!(frame.photo[0] & PHOTO_PANINI, 0, "NaN compression should not set PHOTO_PANINI");
    }

    #[test]
    fn nan_shift_is_ignored() {
        let camera = Camera::Orbit(Orbit::new(Vec3::new(0.0, 0.0, 0.0), 30.0, 20.0, 10.0, 60.0));
        let mut frame = GpuFrame::default();
        let lighting = Lighting { lens: LensOptions { shift: f32::NAN, ..LensOptions::default() }, ..Lighting::default() };
        fill_frame(&mut frame, &lighting, &camera, (200, 100));
        assert_eq!(frame.photo[0] & PHOTO_SHIFT, 0, "NaN shift should not set PHOTO_SHIFT");
    }

    #[test]
    fn nan_aperture_is_ignored() {
        let camera = Camera::Orbit(Orbit::new(Vec3::new(0.0, 0.0, 0.0), 30.0, 20.0, 10.0, 60.0));
        let mut frame = GpuFrame::default();
        let lighting = Lighting { lens: LensOptions { aperture: Some(f32::NAN), focus: Some(3.0), ..LensOptions::default() }, ..Lighting::default() };
        fill_frame(&mut frame, &lighting, &camera, (200, 100));
        assert_eq!(frame.photo[0] & PHOTO_DOF, 0, "NaN aperture should not set PHOTO_DOF");
    }
}
