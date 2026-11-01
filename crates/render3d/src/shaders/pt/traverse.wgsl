// Path tracer: BVH traversal (stubs filled by p11-r3d-pt-traverse). Must find what
// GpuScene::trace_cpu finds.

struct Hit { t: f32, tri: u32, u: f32, v: f32 };

// The nearest triangle hit by o + t·d (unit d) with PT_EPS < t < t_max; t = -1 for a miss.
// Ties by the lower triangle index. Barycentrics u, v of the hit.
fn trace(o: vec3<f32>, d: vec3<f32>, t_max: f32) -> Hit {
    var h: Hit;
    h.t = -1.0;
    h.tri = 0u;
    h.u = 0.0;
    h.v = 0.0;
    return h;
}

// What reaches o + t_max·d from o: 0 behind an opaque triangle, the glass tint
// (albedo · transmission) through each glass triangle, 1 when clear. MAT_NO_SHADOW triangles
// (fixture housings) are skipped.
fn visibility(o: vec3<f32>, d: vec3<f32>, t_max: f32) -> vec3<f32> {
    return vec3<f32>(1.0);
}
