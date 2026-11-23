//! The path tracer's camera frame (phase 22): the basis and field of view the WGSL `camera_ray`
//! (`shaders/pt/lens.wgsl`) needs for a lens that is not a plain pinhole. [`fill_frame`] writes
//! `cam_right`, `cam_up`, `cam_fwd` and `lens2` and nothing else; p22-r3d-lens fills the `lens`
//! vector and the `PHOTO_PANINI` / `PHOTO_DOF` / `PHOTO_SHIFT` flags.

use super::scene_gpu::GpuFrame;
use crate::camera::Camera;
use crate::lit_frame::Lighting;

/// Set the camera basis (unit right, up and forward, w = 0) and the tangents of the half field of
/// view (`lens2.x` horizontal, `lens2.y` vertical) of `camera` at `size` pixels. A camera with a
/// non-finite number leaves the frame as it was.
pub(super) fn fill_frame(frame: &mut GpuFrame, _lighting: &Lighting, camera: &Camera, size: (u32, u32)) {
    let view = camera.view();
    let row = |r: usize| [view.m[r][0], view.m[r][1], view.m[r][2]];
    let (right, up) = (row(0), row(1));
    let fwd = crate::camera::forward(camera);
    let tan_v = (camera.fov_deg().to_radians() * 0.5).tan();
    let aspect = f64::from(size.0.max(1)) / f64::from(size.1.max(1));
    let tan_h = tan_v * aspect;
    let all = [right[0], right[1], right[2], up[0], up[1], up[2], fwd.x, fwd.y, fwd.z, tan_v, tan_h];
    if !all.iter().all(|v| v.is_finite()) {
        return;
    }
    frame.cam_right = [right[0] as f32, right[1] as f32, right[2] as f32, 0.0];
    frame.cam_up = [up[0] as f32, up[1] as f32, up[2] as f32, 0.0];
    frame.cam_fwd = [fwd.x as f32, fwd.y as f32, fwd.z as f32, 0.0];
    frame.lens2 = [tan_h as f32, tan_v as f32, 0.0, 0.0];
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
}
