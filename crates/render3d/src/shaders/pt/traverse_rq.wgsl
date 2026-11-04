// p11-r3d-rt — ray-query traversal. A drop-in replacement for traverse.wgsl: the same Hit, trace()
// and visibility(), with the nearest hit found by wgpu's ray queries in the acceleration structure
// (group 1, binding 0; one BLAS of every triangle, primitive index = triangle index, one TLAS
// instance) instead of walking the BVH by hand. The kernel, BSDF and lights modules are unchanged.
// rq_kernel_source() moves the `enable` directive below to the top of the module: WGSL directives
// must come before every declaration (until 2026-11-03 it sat after common.wgsl and the module
// never compiled).
enable wgpu_ray_query;

@group(1) @binding(0) var acc: acceleration_structure;

struct Hit { t: f32, tri: u32, u: f32, v: f32 };

// The compute traversal's own cross and dot: the BSDF, lights and kernel modules call them, so both
// kernels evaluate those modules with the same arithmetic.
fn cross(a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    );
}

fn dot(a: vec3<f32>, b: vec3<f32>) -> f32 {
    return a.x * b.x + a.y * b.y + a.z * b.z;
}

fn scene_is_empty() -> bool {
    if (arrayLength(&nodes) == 0u) {
        return true;
    }
    if (arrayLength(&nodes) == 1u) {
        let n = nodes[0];
        return n.count == 0u && n.left_or_first == 0u && all(n.min == vec3<f32>(0.0)) && all(n.max == vec3<f32>(0.0));
    }
    return false;
}

// The nearest triangle hit by o + t·d (unit d) with PT_EPS < t < t_max; t = -1 for a miss. Barycentrics
// u, v of the hit (the weights of v0 + e1 and v0 + e2, as tri_hit's). The BLAS is opaque, so one
// rayQueryProceed commits the closest hit; equal distances may resolve to either triangle.
fn trace(o: vec3<f32>, d: vec3<f32>, t_max: f32) -> Hit {
    var h: Hit;
    h.t = -1.0;
    h.tri = 0u;
    h.u = 0.0;
    h.v = 0.0;
    if (scene_is_empty() || all(d == vec3<f32>(0.0)) || !(t_max > PT_EPS)) {
        return h;
    }
    var rq: ray_query;
    rayQueryInitialize(&rq, acc, RayDesc(RAY_FLAG_NONE, 0xFFu, PT_EPS, t_max, o, d));
    while (rayQueryProceed(&rq)) {
    }
    let hit = rayQueryGetCommittedIntersection(&rq);
    if (hit.kind != RAY_QUERY_INTERSECTION_TRIANGLE) {
        return h;
    }
    h.t = hit.t;
    h.tri = hit.primitive_index;
    h.u = hit.barycentrics.x;
    h.v = hit.barycentrics.y;
    return h;
}

// What reaches o + t_max·d from o: 0 behind an opaque triangle, the glass tint
// (albedo · transmission) and Fresnel transmittance at both faces, once per pane (on entering it),
// 1 when clear. MAT_NO_SHADOW triangles (fixture housings) are skipped.
fn visibility(o: vec3<f32>, d: vec3<f32>, t_max: f32) -> vec3<f32> {
    if (scene_is_empty()) {
        return vec3<f32>(1.0);
    }
    var transmittance = vec3<f32>(1.0);
    var total = 0.0;
    var origin = o;
    var hits = 0u;
    var glass_hits = 0u;
    while (total < t_max && hits < 16u) {
        let hit = trace(origin, d, t_max - total);
        if (hit.t < 0.0) {
            break;
        }
        let tri = tris[hit.tri];
        if ((tri.flags & MAT_NO_SHADOW) == 0u) {
            if ((tri.flags & MAT_GLASS) != 0u) {
                glass_hits = glass_hits + 1u;
                if (glass_hits % 2u == 1u) {
                    let material = materials[tri.material];
                    let tint = material.albedo * material.transmission;
                    let ior = material.ior;
                    let f0 = ((ior - 1.0) / (ior + 1.0)) * ((ior - 1.0) / (ior + 1.0));
                    let cos_i = abs(dot(d, tri.n));
                    let f = f0 + (1.0 - f0) * pow(1.0 - cos_i, 5.0);
                    let fresnel = (1.0 - f) * (1.0 - f);
                    transmittance = transmittance * tint * fresnel;
                }
            } else {
                return vec3<f32>(0.0);
            }
        }
        total = total + hit.t + PT_EPS;
        origin = o + d * total;
        hits = hits + 1u;
    }
    return transmittance;
}

@compute @workgroup_size(64)
fn visibility_test_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.probe_count) {
        return;
    }
    let probe = probes[id.x];
    accum[id.x] = vec4<f32>(visibility(probe.point, probe.normal, probe._a), 0.0);
}

