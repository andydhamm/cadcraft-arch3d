// p11-r3d-rt — ray-query traversal. Identical trace()/visibility() entry points to
// traverse.wgsl, but they resolve hits against an acceleration structure (group 1, binding 0)
// with rayQueryInitialize / rayQueryNextHit instead of walking the triangle array by hand.
// The kernel module (kernel.wgsl) is unchanged; only this module is swapped in.
enable wgpu_ray_query;

var acc: acceleration_structure @group(1) @binding(0);

struct Ray {
  origin: vec3<f32>,
  direction: vec3<f32>,
  near: f32,
  far: f32,
  payload: vec3<f32>,
}

// Returns the nearest committed triangle along the ray, or `vec3<f32>(1e30)` if none.
// The committed hit gives: distance t, the triangle index (primitive index) and barycentrics.
fn rq_nearest(ray: Ray) -> vec4<f32> {
  var hit = rayQueryInitialize(ray);
  var out = vec4<f32>(vec3<f32>(1e30), 0.0);
  while (hit) {
    let tri = rayQueryNextHit(ray);
    let dist = tri.payload[0];
    if (dist >= 0.001 && dist < out[0]) {
      out = vec4<f32>(dist, tri.payload[1], tri.payload[2], tri.payload[3]);
    }
    hit = rayQueryInitialize(ray);
  }
  return out;
}

fn trace(ray: Ray) -> vec4<f32> {
  return rq_nearest(ray);
}
// p11-r3d-rt — ray-query traversal. A drop-in replacement for traverse.wgsl: the same
// trace()/visibility() entry points and the same hit structure, but hits are resolved
// against an acceleration structure (group 1, binding 0) via rayQueryInitialize /
// rayQueryNextHit instead of walking the triangle array by hand. The kernel module
// (kernel.wgsl) and the BSDF/lights modules are unchanged; only this module is swapped in.
enable wgpu_ray_query;

var acc: acceleration_structure @group(1) @binding(0);

struct Hit {
    t: f32,
    tri: u32,
    u: f32,
    v: f32,
}

struct Ray {
    origin: vec3<f32>,
    direction: vec3<f32>,
    near: f32,
    far: f32,
    payload: vec3<f32>,
}

struct RqHit {
    t: f32,
    tri: u32,
    u: f32,
    v: f32,
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

// Nearest committed triangle along the ray o + t·d with PT_EPS < t < t_max.
// Returns t, the triangle index (primitive index) and barycentrics; t = -1 on a miss.
fn rq_nearest(o: vec3<f32>, d: vec3<f32>, t_max: f32) -> Hit {
    var h: Hit;
    h.t = -1.0;
    h.tri = 0u;
    h.u = 0.0;
    h.v = 0.0;
    if (scene_is_empty()) {
        return h;
    }
    if (all(d == vec3<f32>(0.0))) {
        return h;
    }
    let ray = Ray {
        origin: o,
        direction: d,
        near: PT_EPS,
        far: t_max,
        payload: vec3<f32>(0.0),
    };
    var hit = rayQueryInitialize(ray);
    var done = false;
    while (!done) {
        if (hit) {
            let t = hit.payload[0];
            if (t > PT_EPS && t < t_max) {
                h.t = t;
                h.tri = u32(hit.payload[1]);
                h.u = hit.payload[2];
                h.v = hit.payload[3];
                done = true;
            } else {
            // no further hits; rayQueryNextHit stops returning valid hits
        }
        done = true;
    }
    return h;
}

// The nearest triangle hit by o + t·d (unit d) with PT_EPS < t < t_max; t = -1 for a miss.
// Same semantics as the compute path in traverse.wgsl.
fn trace(o: vec3<f32>, d: vec3<f32>, t_max: f32) -> Hit {
    return rq_nearest(o, d, t_max);
}

// What reaches o + t_max·d from o: 0 behind an opaque triangle, the glass tint
// (albedo · transmission) through each glass triangle, 1 when clear. MAT_NO_SHADOW triangles
// (fixture housings) are skipped. Continues the ray query from the new origin after each hit.
fn visibility(o: vec3<f32>, d: vec3<f32>, t_max: f32) -> vec3<f32> {
    if (scene_is_empty()) {
        return vec3<f32>(1.0);
    }
    var transmittance = vec3<f32>(1.0);
    var total = 0.0;
    var origin = o;
    var hits = 0u;
    while (total < t_max && hits < 16u) {
        let hit = trace(origin, d, t_max - total);
        if (hit.t < 0.0) {
            break;
        }
        let tri = tris[hit.tri];
        if ((tri.flags & MAT_NO_SHADOW) == 0u) {
            if ((tri.flags & MAT_GLASS) != 0u) {
                let material = materials[tri.material];
                transmittance = transmittance * (material.albedo * material.transmission);
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
