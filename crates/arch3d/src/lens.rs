//! Lens math shared by every renderer and by picking: Panini projection, vertical shift and thin-lens blur.
//!
//! Camera space has x right, y up, z forward. Normalised frame coordinates `(nx, ny)` lie in `[-1, 1]`
//! with +ny up. Pure `f64` math, no document types.

use crate::camera::{self, Camera};
use cadcraft_geom::Vec3;

/// Panini and shift settings of a view. The neutral lens is plain rectilinear with no shift.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LensView {
    /// Use the general Panini projection instead of rectilinear.
    pub panini: bool,
    /// Panini compression `d` (0 = rectilinear, 1 = classic Panini).
    pub compression: f64,
    /// Vertical lens shift as a fraction of the frame height (+ moves the horizon down).
    pub shift: f64,
}

impl Default for LensView {
    fn default() -> Self {
        LensView { panini: false, compression: 0.7, shift: 0.0 }
    }
}

impl LensView {
    /// True when the lens changes nothing: no Panini and no shift.
    pub fn is_neutral(&self) -> bool {
        !self.panini && self.shift == 0.0
    }

    fn d(&self) -> f64 {
        if self.compression.is_finite() { self.compression.clamp(0.0, 1.0e6) } else { 0.0 }
    }
}

/// Sensor width of the thin-lens model, in metres.
pub const SENSOR_WIDTH_M: f64 = 0.036;

/// Focal length in metres for a horizontal half-field tangent.
pub fn focal_length_m(tan_h: f64) -> f64 {
    if tan_h.is_finite() && tan_h > 0.0 { SENSOR_WIDTH_M / 2.0 / tan_h } else { 0.0 }
}

/// Aperture radius in metres: focal length / (2 N).
pub fn aperture_radius_m(tan_h: f64, f_number: f64) -> f64 {
    if f_number.is_finite() && f_number > 0.0 { focal_length_m(tan_h) / (2.0 * f_number) } else { 0.0 }
}

/// Circle-of-confusion diameter in pixels for a point at `distance` when focused at `focus` (metres).
/// Returns 0 for non-finite or non-positive input.
pub fn blur_diameter_px(tan_h: f64, f_number: f64, focus: f64, distance: f64, width_px: u32) -> f64 {
    if !(focus.is_finite() && distance.is_finite() && focus > 0.0 && distance > 0.0 && tan_h.is_finite() && tan_h > 0.0) {
        return 0.0;
    }
    let r = aperture_radius_m(tan_h, f_number);
    let v = 2.0 * r * (distance - focus).abs() / (distance * focus) * (width_px as f64 / 2.0) / tan_h;
    if v.is_finite() { v } else { 0.0 }
}

/// Panini scale `S` at azimuth `phi`.
fn panini_s(d: f64, phi: f64) -> f64 {
    (d + 1.0) / (d + phi.cos())
}

/// Scale `k` that keeps the horizontal field of view.
fn panini_k(d: f64, tan_h: f64) -> f64 {
    let pm = tan_h.atan();
    panini_s(d, pm) * pm.sin()
}

/// Map a camera-space direction to normalised frame coordinates. `None` behind the eye or on bad input.
pub fn dir_to_frame(l: &LensView, tan_h: f64, tan_v: f64, cam_dir: [f64; 3]) -> Option<[f64; 2]> {
    if !cam_dir.iter().all(|v| v.is_finite()) || !(tan_h.is_finite() && tan_v.is_finite() && tan_h > 0.0 && tan_v > 0.0) {
        return None;
    }
    let [x, y, z] = cam_dir;
    let (nx, ny) = if l.panini {
        let phi = x.atan2(z);
        if phi.abs() >= 179.0_f64.to_radians() {
            return None;
        }
        let d = l.d();
        let s = panini_s(d, phi);
        let hyp = x.hypot(z);
        let k = panini_k(d, tan_h);
        if !(k > 1e-12) || !(hyp > 0.0) {
            return None;
        }
        (s * phi.sin() / k, s * y / hyp * tan_h / tan_v / k)
    } else {
        if z <= 1e-9 {
            return None;
        }
        ((x / z) / tan_h, (y / z) / tan_v)
    };
    let ny = ny - 2.0 * l.shift;
    if nx.is_finite() && ny.is_finite() { Some([nx, ny]) } else { None }
}

/// Unit camera-space direction for normalised frame coordinates; the exact inverse of [`dir_to_frame`].
/// Non-finite input yields the forward direction.
pub fn frame_to_dir(l: &LensView, tan_h: f64, tan_v: f64, n: [f64; 2]) -> [f64; 3] {
    const FORWARD: [f64; 3] = [0.0, 0.0, 1.0];
    if !n.iter().all(|v| v.is_finite()) || !(tan_h.is_finite() && tan_v.is_finite() && tan_h > 0.0 && tan_v > 0.0) {
        return FORWARD;
    }
    let nx = n[0];
    let ny = n[1] + 2.0 * l.shift;
    let v = if l.panini {
        let d = l.d();
        let k = panini_k(d, tan_h);
        let px = nx * k;
        let py = ny * k * tan_v / tan_h;
        // sin(phi) (d+1) / (d + cos(phi)) = px, solved through t = tan(phi/2).
        let disc = ((d + 1.0) * ((d + 1.0) - px * px * (d - 1.0))).max(0.0);
        let t = px * (d + 1.0) / ((d + 1.0) + disc.sqrt());
        let phi = 2.0 * t.atan();
        let s = panini_s(d, phi);
        [phi.sin(), py / s, phi.cos()]
    } else {
        [nx * tan_h, ny * tan_v, 1.0]
    };
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len.is_finite() && len > 1e-300 { [v[0] / len, v[1] / len, v[2] / len] } else { FORWARD }
}

/// The camera's `(right, up, forward)` world-space unit axes, built as [`camera::ray_at`] builds them.
pub fn camera_basis(camera: &Camera) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let m = camera.view().m;
    let row = |i: usize, s: f64| [s * m[i][0], s * m[i][1], s * m[i][2]];
    (row(0, 1.0), row(1, 1.0), row(2, -1.0))
}

fn tans(camera: &Camera, size: (u32, u32)) -> Option<(f64, f64)> {
    let (w, h) = size;
    if w == 0 || h == 0 {
        return None;
    }
    let tan_v = (camera.fov_deg() * std::f64::consts::PI / 360.0).tan();
    let tan_h = tan_v * w as f64 / h as f64;
    if tan_h.is_finite() && tan_v.is_finite() && tan_h > 0.0 && tan_v > 0.0 { Some((tan_h, tan_v)) } else { None }
}

/// Project a world point to viewport pixels (y-down) through the lens. A neutral lens equals
/// [`camera::project_point`].
pub fn project_point_lens(camera: &Camera, l: &LensView, size: (u32, u32), p: [f64; 3]) -> Option<[f32; 2]> {
    if l.is_neutral() {
        return camera::project_point(camera, size, p);
    }
    if !p.iter().all(|v| v.is_finite()) {
        return None;
    }
    let (tan_h, tan_v) = tans(camera, size)?;
    let eye = camera.eye();
    let rel = [p[0] - eye.x, p[1] - eye.y, p[2] - eye.z];
    let (r, u, f) = camera_basis(camera);
    let dot = |a: [f64; 3]| a[0] * rel[0] + a[1] * rel[1] + a[2] * rel[2];
    let n = dir_to_frame(l, tan_h, tan_v, [dot(r), dot(u), dot(f)])?;
    let px = ((n[0] * 0.5 + 0.5) * size.0 as f64) as f32;
    let py = ((1.0 - (n[1] * 0.5 + 0.5)) * size.1 as f64) as f32;
    if px.is_finite() && py.is_finite() { Some([px, py]) } else { None }
}

/// The world-space ray `(eye, unit direction)` through a pixel (y-down) through the lens. A neutral lens equals
/// [`camera::ray_at`].
pub fn ray_at_lens(camera: &Camera, l: &LensView, size: (u32, u32), pixel: [f32; 2]) -> Option<(Vec3, Vec3)> {
    if l.is_neutral() {
        return camera::ray_at(camera, size, pixel);
    }
    if !pixel.iter().all(|v| v.is_finite()) {
        return None;
    }
    let (tan_h, tan_v) = tans(camera, size)?;
    let (w, h) = (size.0 as f64, size.1 as f64);
    let (px, py) = (pixel[0] as f64, pixel[1] as f64);
    if px < -1.0 || px > w + 1.0 || py < -1.0 || py > h + 1.0 {
        return None;
    }
    let c = frame_to_dir(l, tan_h, tan_v, [px / w * 2.0 - 1.0, 1.0 - py / h * 2.0]);
    let (r, u, f) = camera_basis(camera);
    let dir = Vec3::new(r[0] * c[0] + u[0] * c[1] + f[0] * c[2], r[1] * c[0] + u[1] * c[1] + f[1] * c[2], r[2] * c[0] + u[2] * c[1] + f[2] * c[2]);
    if !dir.is_finite() {
        return None;
    }
    let dir = dir.normalized();
    if dir.is_finite() { Some((camera.eye(), dir)) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tan_hv(hfov_deg: f64, aspect: f64) -> (f64, f64) {
        let th = (hfov_deg.to_radians() / 2.0).tan();
        (th, th / aspect)
    }

    fn pan(d: f64) -> LensView {
        LensView { panini: true, compression: d, shift: 0.0 }
    }

    #[test]
    fn neutral_lens_matches_project_point_and_ray_at() {
        let cam = Camera::Full(camera::FullCamera::new(Vec3::new(4.0, -6.0, 3.0), Vec3::new(0.5, 0.0, 1.0), 50.0));
        let l = LensView::default();
        assert!(l.is_neutral());
        let size = (640, 480);
        for p in [[0.0, 0.0, 0.0], [1.0, 2.0, 0.5], [-3.0, 1.0, 2.0]] {
            let a = project_point_lens(&cam, &l, size, p);
            let b = camera::project_point(&cam, size, p);
            match (a, b) {
                (Some(a), Some(b)) => assert!((a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9),
                (a, b) => assert_eq!(a, b),
            }
        }
        for px in [[320.0, 240.0], [10.0, 400.0], [600.0, 5.0]] {
            let a = ray_at_lens(&cam, &l, size, px);
            let b = camera::ray_at(&cam, size, px);
            match (a, b) {
                (Some((o1, d1)), Some((o2, d2))) => {
                    assert!((o1.x - o2.x).abs() < 1e-9 && (d1.x - d2.x).abs() < 1e-9);
                    assert!((d1.y - d2.y).abs() < 1e-9 && (d1.z - d2.z).abs() < 1e-9);
                }
                (a, b) => assert_eq!(a.is_some(), b.is_some()),
            }
        }
    }

    #[test]
    fn panini_keeps_the_horizontal_field_and_verticals() {
        let (th, tv) = tan_hv(100.0, 16.0 / 9.0);
        let a = 47.5_f64.to_radians();
        let dir = [-a.sin(), 0.0, a.cos()];
        let r = dir_to_frame(&LensView::default(), th, tv, dir).unwrap();
        assert!((r[0] + 0.9157).abs() < 1e-3, "{r:?}");
        let p = dir_to_frame(&pan(1.0), th, tv, dir).unwrap();
        assert!((p[0] + 0.9436).abs() < 1e-3, "{p:?}");
        let e = 50.0_f64.to_radians();
        let edge = dir_to_frame(&pan(1.0), th, tv, [e.sin(), 0.0, e.cos()]).unwrap();
        assert!((edge[0] - 1.0).abs() < 1e-9);
        for d in [0.0, 0.5, 1.0, 2.0] {
            let nx0 = dir_to_frame(&pan(d), th, tv, [2.0, -1.0, 3.0]).unwrap()[0];
            for y in [-2.0, 0.3, 4.0] {
                let nx = dir_to_frame(&pan(d), th, tv, [2.0, y, 3.0]).unwrap()[0];
                assert!((nx - nx0).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn panini_compression_zero_is_rectilinear() {
        let (th, tv) = tan_hv(90.0, 1.5);
        for dir in [[0.3, 0.2, 1.0], [-1.2, -0.4, 0.9], [0.0, 0.5, 2.0]] {
            let a = dir_to_frame(&pan(0.0), th, tv, dir).unwrap();
            let b = dir_to_frame(&LensView::default(), th, tv, dir).unwrap();
            assert!((a[0] - b[0]).abs() < 1e-12 && (a[1] - b[1]).abs() < 1e-12);
        }
    }

    #[test]
    fn frame_to_dir_inverts_dir_to_frame() {
        let (th, tv) = tan_hv(100.0, 1.6);
        for l in [LensView::default(), pan(0.0), pan(0.7), pan(1.0), pan(3.0), LensView { panini: true, compression: 1.0, shift: 0.2 }] {
            for n in [[0.0, 0.0], [0.9, 0.5], [-1.0, -0.7], [0.3, 0.9]] {
                let d = frame_to_dir(&l, th, tv, n);
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                assert!((len - 1.0).abs() < 1e-12);
                let back = dir_to_frame(&l, th, tv, d).unwrap();
                assert!((back[0] - n[0]).abs() < 1e-9 && (back[1] - n[1]).abs() < 1e-9, "{l:?} {n:?} {back:?}");
            }
        }
    }

    #[test]
    fn shift_moves_the_horizon_by_its_fraction() {
        let (th, tv) = tan_hv(60.0, 1.5);
        for l in [LensView { shift: 0.25, ..LensView::default() }, LensView { panini: true, compression: 1.0, shift: 0.25 }] {
            let n = dir_to_frame(&l, th, tv, [0.0, 0.0, 1.0]).unwrap();
            let row = (1.0 - n[1]) / 2.0;
            assert!((row - 0.75).abs() < 1e-12, "{row}");
        }
    }

    #[test]
    fn thin_lens_blur_matches_the_brief_numbers() {
        let th = 30.0_f64.to_radians().tan();
        let near = blur_diameter_px(th, 2.8, 3.0, 1.5, 1921);
        let far = blur_diameter_px(th, 2.8, 3.0, 10.0, 1921);
        assert!((near - 6.17).abs() < 0.01, "{near}");
        assert!((far - 4.32).abs() < 0.01, "{far}");
        assert_eq!(blur_diameter_px(f64::NAN, 2.8, 3.0, 1.5, 100), 0.0);
        assert!((focal_length_m(th) - 0.018 / th).abs() < 1e-15);
    }

    #[test]
    fn dir_to_frame_rejects_non_finite_and_behind_eye() {
        let (th, tv) = tan_hv(60.0, 1.5);
        let l = LensView::default();
        // NaN in direction
        assert!(dir_to_frame(&l, th, tv, [f64::NAN, 0.0, 1.0]).is_none());
        assert!(dir_to_frame(&l, th, tv, [0.0, f64::NAN, 1.0]).is_none());
        assert!(dir_to_frame(&l, th, tv, [0.0, 0.0, f64::NAN]).is_none());
        // Inf in direction
        assert!(dir_to_frame(&l, th, tv, [f64::INFINITY, 0.0, 1.0]).is_none());
        // NaN/inf in tans
        assert!(dir_to_frame(&l, f64::NAN, tv, [0.0, 0.0, 1.0]).is_none());
        assert!(dir_to_frame(&l, th, f64::NAN, [0.0, 0.0, 1.0]).is_none());
        assert!(dir_to_frame(&l, 0.0, tv, [0.0, 0.0, 1.0]).is_none());
        // Behind the eye (rectilinear)
        assert!(dir_to_frame(&l, th, tv, [0.0, 0.0, 0.0]).is_none());
        assert!(dir_to_frame(&l, th, tv, [0.0, 0.0, -1.0]).is_none());
        assert!(dir_to_frame(&l, th, tv, [0.0, 0.0, 1e-10]).is_none());
        // Exactly at the threshold is accepted
        assert!(dir_to_frame(&l, th, tv, [0.0, 0.0, 1e-9]).is_some());
        // Panini: direction exactly at 179 degrees
        let l_pan = LensView { panini: true, compression: 0.7, shift: 0.0 };
        let phi = 179.0_f64.to_radians();
        assert!(dir_to_frame(&l_pan, th, tv, [phi.sin(), 0.0, phi.cos()]).is_none());
        // Just inside 179 degrees is accepted
        let phi_ok = 178.0_f64.to_radians();
        assert!(dir_to_frame(&l_pan, th, tv, [phi_ok.sin(), 0.0, phi_ok.cos()]).is_some());
    }

    #[test]
    fn frame_to_dir_handles_non_finite_and_returns_forward() {
        let (th, tv) = tan_hv(60.0, 1.5);
        let l = LensView::default();
        // Non-finite frame coords return forward
        let fwd = frame_to_dir(&l, th, tv, [f64::NAN, 0.0]);
        assert!((fwd[0] - 0.0).abs() < 1e-15 && (fwd[1] - 0.0).abs() < 1e-15 && (fwd[2] - 1.0).abs() < 1e-15);
        let fwd = frame_to_dir(&l, th, tv, [0.0, f64::INFINITY]);
        assert!((fwd[2] - 1.0).abs() < 1e-15);
        // Non-finite tans return forward
        let fwd = frame_to_dir(&l, f64::NAN, tv, [0.0, 0.0]);
        assert!((fwd[2] - 1.0).abs() < 1e-15);
        // Zero tans return forward
        let fwd = frame_to_dir(&l, 0.0, tv, [0.0, 0.0]);
        assert!((fwd[2] - 1.0).abs() < 1e-15);
        // Result is always unit length
        for n in [[0.0, 0.0], [1.0, 1.0], [-1.0, -1.0], [0.5, -0.3]] {
            let d = frame_to_dir(&l, th, tv, n);
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-12, "not unit: {d:?}");
        }
    }

    #[test]
    fn project_and_ray_round_trip_through_lens() {
        let cam = Camera::Full(camera::FullCamera::new(Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 1.0), 50.0));
        let l = LensView { panini: true, compression: 0.7, shift: 0.1 };
        let size = (800, 600);
        // Project a point, then ray at that pixel should pass near the point
        let p = [2.0, 1.0, 3.0];
        let px = project_point_lens(&cam, &l, size, p).expect("project failed");
        let (eye, dir) = ray_at_lens(&cam, &l, size, px).expect("ray failed");
        // The ray should pass within 1 pixel of the original point
        let t = (p[0] - eye.x) / dir.x;
        let py = eye.y + t * dir.y;
        let pz = eye.z + t * dir.z;
        assert!((py - p[1]).abs() < 0.05, "y error: {}", (py - p[1]).abs());
        assert!((pz - p[2]).abs() < 0.05, "z error: {}", (pz - p[2]).abs());
    }

    #[test]
    fn zero_size_camera_rejected() {
        let cam = Camera::Full(camera::FullCamera::new(Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 1.0), 50.0));
        let l = LensView::default();
        assert!(project_point_lens(&cam, &l, (0, 600), [0.0, 0.0, 0.0]).is_none());
        assert!(project_point_lens(&cam, &l, (800, 0), [0.0, 0.0, 0.0]).is_none());
        assert!(ray_at_lens(&cam, &l, (0, 600), [0.0, 0.0]).is_none());
        assert!(ray_at_lens(&cam, &l, (800, 0), [0.0, 0.0]).is_none());
    }

    #[test]
    fn thin_lens_edge_cases() {
        let th = 30.0_f64.to_radians().tan();
        // f_number <= 0
        assert_eq!(blur_diameter_px(th, 0.0, 3.0, 1.5, 100), 0.0);
        assert_eq!(blur_diameter_px(th, -1.0, 3.0, 1.5, 100), 0.0);
        // focus <= 0
        assert_eq!(blur_diameter_px(th, 2.8, 0.0, 1.5, 100), 0.0);
        assert_eq!(blur_diameter_px(th, 2.8, -1.0, 1.5, 100), 0.0);
        // distance <= 0
        assert_eq!(blur_diameter_px(th, 2.8, 3.0, 0.0, 100), 0.0);
        // At focus distance, blur is zero
        let at_focus = blur_diameter_px(th, 2.8, 3.0, 3.0, 100);
        assert!((at_focus - 0.0).abs() < 1e-12, "at focus blur should be 0, got {at_focus}");
        // Focal length edge cases
        assert_eq!(focal_length_m(0.0), 0.0);
        assert_eq!(focal_length_m(f64::NAN), 0.0);
        assert_eq!(focal_length_m(f64::INFINITY), 0.0);
        // Aperture radius edge cases
        assert_eq!(aperture_radius_m(th, 0.0), 0.0);
        assert_eq!(aperture_radius_m(th, f64::NAN), 0.0);
        // Blur with non-finite distance
        assert_eq!(blur_diameter_px(th, 2.8, 3.0, f64::INFINITY, 100), 0.0);
    }

    #[test]
    fn compression_clamped_and_huge_values() {
        let (th, tv) = tan_hv(60.0, 1.5);
        // Non-finite compression clamps to 0 (rectilinear)
        let l = LensView { panini: true, compression: f64::NAN, shift: 0.0 };
        let d = l.d();
        assert_eq!(d, 0.0);
        let l = LensView { panini: true, compression: f64::INFINITY, shift: 0.0 };
        assert_eq!(l.d(), 0.0);
        // Huge compression clamps to 1e6
        let l = LensView { panini: true, compression: 1e9, shift: 0.0 };
        assert_eq!(l.d(), 1e6);
        // Negative compression clamps to 0
        let l = LensView { panini: true, compression: -0.5, shift: 0.0 };
        assert_eq!(l.d(), 0.0);
        // Huge shift still works (horizon moves off screen)
        let l = LensView { shift: 10.0, ..LensView::default() };
        let n = dir_to_frame(&l, th, tv, [0.0, 0.0, 1.0]).unwrap();
        assert!((n[1] - (-20.0)).abs() < 1e-12, "horizon at ny={}", n[1]);
        // Round trip with huge shift
        let back = frame_to_dir(&l, th, tv, n);
        assert!((back[0] - 0.0).abs() < 1e-12 && (back[1] - 0.0).abs() < 1e-12 && (back[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn panini_and_rectilinear_agree_at_center() {
        let (th, tv) = tan_hv(100.0, 1.6);
        // At the center (nx=ny=0), both projections give the forward direction
        let l_rect = LensView::default();
        let l_pan = LensView { panini: true, compression: 0.7, shift: 0.0 };
        let d_rect = frame_to_dir(&l_rect, th, tv, [0.0, 0.0]);
        let d_pan = frame_to_dir(&l_pan, th, tv, [0.0, 0.0]);
        assert!((d_rect[0] - d_pan[0]).abs() < 1e-12);
        assert!((d_rect[1] - d_pan[1]).abs() < 1e-12);
        assert!((d_rect[2] - d_pan[2]).abs() < 1e-12);
        // And dir_to_frame agrees at the forward direction
        let n_rect = dir_to_frame(&l_rect, th, tv, [0.0, 0.0, 1.0]).unwrap();
        let n_pan = dir_to_frame(&l_pan, th, tv, [0.0, 0.0, 1.0]).unwrap();
        assert!((n_rect[0] - n_pan[0]).abs() < 1e-12);
        assert!((n_rect[1] - n_pan[1]).abs() < 1e-12);
    }
}
