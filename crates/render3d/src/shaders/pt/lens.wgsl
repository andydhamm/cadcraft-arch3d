// Path tracer (phase 22): the camera ray. With none of the lens flags it is the pinhole through
// frame.inv_view_proj and draws no random number. Panini, the vertical shift and the thin lens run
// only under PHOTO_PANINI / PHOTO_SHIFT / PHOTO_DOF (pathtrace/lens.rs fills them). The camera-space
// direction is the port of cadcraft_arch3d::lens::frame_to_dir, which is the reference.
// camera_ray draws no random number except the two of the aperture sample under PHOTO_DOF: the
// jittered pixel position (px, py) comes from the caller, and `r` is for that sample.

struct CameraRay { o: vec3<f32>, d: vec3<f32> };

// The unit camera-space direction (x right, y up, z forward) for the normalised frame coordinates (nx, ny).
fn lens_dir(nx: f32, ny_in: f32) -> vec3<f32> {
    let flags = frame.photo.x;
    let tan_h = frame.lens2.x;
    let tan_v = frame.lens2.y;
    var ny = ny_in;
    if ((flags & PHOTO_SHIFT) != 0u) {
        ny = ny + 2.0 * frame.lens.y;
    }
    var v = vec3<f32>(nx * tan_h, ny * tan_v, 1.0);
    if ((flags & PHOTO_PANINI) != 0u) {
        let d = clamp(frame.lens.x, 0.0, 1.0e6);
        let pm = atan(tan_h);
        let k = (d + 1.0) / (d + cos(pm)) * sin(pm);
        let px = nx * k;
        let py = ny * k * tan_v / tan_h;
        // sin(phi) (d+1) / (d + cos(phi)) = px, solved through t = tan(phi/2).
        let disc = max((d + 1.0) * ((d + 1.0) - px * px * (d - 1.0)), 0.0);
        let t = px * (d + 1.0) / ((d + 1.0) + sqrt(disc));
        let phi = 2.0 * atan(t);
        let s = (d + 1.0) / (d + cos(phi));
        v = vec3<f32>(sin(phi), py / s, cos(phi));
    }
    return normalize(v);
}

// The ray through the (jittered) pixel position (px, py): its origin starts at the near clip.
fn camera_ray(px: f32, py: f32, r: ptr<function, Rng>) -> CameraRay {
    var ray: CameraRay;
    let nx = 2.0 * px / f32(frame.size.x) - 1.0;
    let ny = 1.0 - 2.0 * py / f32(frame.size.y);
    if ((frame.photo.x & (PHOTO_PANINI | PHOTO_SHIFT | PHOTO_DOF)) == 0u) {
        let ndc = vec2<f32>(nx, ny);
        let far4 = frame.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
        let near4 = frame.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
        let dir = normalize(far4.xyz / far4.w - near4.xyz / near4.w);
        let t0 = camera_t_min(dir);
        ray.o = frame.eye.xyz + dir * t0;
        ray.d = dir;
        return ray;
    }
    let c = lens_dir(nx, ny);
    let right = frame.cam_right.xyz;
    let up = frame.cam_up.xyz;
    let fwd = frame.cam_fwd.xyz;
    var dir = normalize(right * c.x + up * c.y + fwd * c.z);
    var origin = frame.eye.xyz;
    if ((frame.photo.x & PHOTO_DOF) != 0u) {
        // The thin lens: every ray through the lens disc meets the pinhole ray where it crosses the focus plane.
        let focal = origin + dir * (frame.lens.w / max(dot(dir, fwd), 0.05));
        let rad = sqrt(rand(r));
        let ang = 6.2831853 * rand(r);
        origin = origin + frame.lens.z * rad * (right * cos(ang) + up * sin(ang));
        dir = normalize(focal - origin);
    }
    let t0 = camera_t_min(dir);
    ray.o = origin + dir * t0;
    ray.d = dir;
    return ray;
}

// Test entry (pathtrace/lens_tests.rs): probe i = (pixel x, pixel y, _) with normal.x 0 for the ray's
// direction, 1 for its origin; the result is accum[i].xyz. The sample is the probe's, the jitter is none.
@compute @workgroup_size(64)
fn lens_test_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.probe_count) {
        return;
    }
    let p = probes[id.x];
    var r = rng_new(id.x, 0u, frame.seed);
    let cam = camera_ray(p.point.x, p.point.y, &r);
    accum[id.x] = vec4<f32>(select(cam.d, cam.o, p.normal.x > 0.5), 0.0);
}
