// Path tracer (phase 22): the camera ray. Today's pinhole through frame.inv_view_proj; the lens
// work (Panini, shift, thin-lens depth of field) replaces the body of camera_ray and nothing else.
// camera_ray draws no random number of its own: the jittered pixel position (px, py) comes from the
// caller, and `r` is there for the aperture sample a later package draws.

struct CameraRay { o: vec3<f32>, d: vec3<f32> };

// The ray through the (jittered) pixel position (px, py): its origin starts at the near clip.
fn camera_ray(px: f32, py: f32, r: ptr<function, Rng>) -> CameraRay {
    let ndc = vec2<f32>(2.0 * px / f32(frame.size.x) - 1.0, 1.0 - 2.0 * py / f32(frame.size.y));
    let far4 = frame.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    let near4 = frame.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    let dir = normalize(far4.xyz / far4.w - near4.xyz / near4.w);
    let t0 = camera_t_min(dir);
    var ray: CameraRay;
    ray.o = frame.eye.xyz + dir * t0;
    ray.d = dir;
    return ray;
}
