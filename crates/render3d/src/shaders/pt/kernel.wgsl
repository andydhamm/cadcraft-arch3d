// Path tracer: the entry points (stubs filled by p11-r3d-pt-kernel).
//
// trace_main: one invocation per pixel adds frame.spp_this_pass samples to accum (rgb sum, w
// count): a camera ray through a jittered point of the pixel (frame.inv_view_proj), then a path:
// emission seen (camera-only emitters at depth 0 only), next-event estimation at each diffuse or
// glossy vertex (every light with a visibility() shadow ray, the sun, area lights with MIS), the
// sky on a miss, frame.bounces full bounces then Russian roulette (survival = max throughput
// component, capped at 0.95) up to MAX_PATH_DEPTH; bounces 0 = direct light only. On a pixel's
// first sample it writes the AOVs. A sample is fully determined by (pixel, sample index, seed).
//
// irradiance_main: one invocation per probe adds frame.spp_this_pass samples of the irradiance at
// probes[i] (rgb lux in accum[i]): direct light from every light and the sun with shadow rays,
// plus, with bounces > 0, cosine-sampled paths times π.
//
// traverse_test_main: for the traversal test, probes[i] is a ray (point = origin, normal =
// direction) and accum[i] = (t, bitcast tri, u, v) of trace(origin, dir, PT_INF).

@compute @workgroup_size(8, 8)
fn trace_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.size.x || id.y >= frame.size.y) {
        return;
    }
    let i = id.y * frame.size.x + id.x;
    accum[i] = accum[i] + vec4<f32>(0.0, 0.0, 0.0, f32(frame.spp_this_pass));
}

@compute @workgroup_size(64)
fn irradiance_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.probe_count) {
        return;
    }
    accum[id.x] = accum[id.x] + vec4<f32>(0.0, 0.0, 0.0, f32(frame.spp_this_pass));
}

@compute @workgroup_size(64)
fn traverse_test_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.probe_count) {
        return;
    }
    let ray = probes[id.x];
    let h = trace(ray.point, ray.normal, PT_INF);
    accum[id.x] = vec4<f32>(h.t, bitcast<f32>(h.tri), h.u, h.v);
}
