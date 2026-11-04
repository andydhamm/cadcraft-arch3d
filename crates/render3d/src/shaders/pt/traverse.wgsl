// Path tracer: BVH traversal. Must find what GpuScene::trace_cpu finds.

struct Hit { t: f32, tri: u32, u: f32, v: f32 };

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

fn box_entry(node: Node, origin: vec3<f32>, dir: vec3<f32>, inv: vec3<f32>, t_far: f32) -> f32 {
    var near = 0.0;
    var far = t_far;
    for (var k = 0u; k < 3u; k = k + 1u) {
        if (dir[k] == 0.0) {
            if (origin[k] < node.min[k] || origin[k] > node.max[k]) {
                return -1.0;
            }
            continue;
        }
        let a = (node.min[k] - origin[k]) * inv[k];
        let b = (node.max[k] - origin[k]) * inv[k];
        near = max(near, min(a, b));
        far = min(far, max(a, b) * 1.0000004);
    }
    if (near <= far) {
        return near;
    }
    return -1.0;
}

fn tri_hit(tri: Tri, origin: vec3<f32>, dir: vec3<f32>) -> vec4<f32> {
    let p = cross(dir, tri.e2);
    let det = dot(tri.e1, p);
    if (abs(det) <= 1.0e-12) {
        return vec4<f32>(-1.0, 0.0, 0.0, 0.0);
    }
    let inv = 1.0 / det;
    let s = origin - tri.v0;
    let u = dot(s, p) * inv;
    if (!(u >= -1.0e-6 && u <= 1.0 + 1.0e-6)) {
        return vec4<f32>(-1.0, 0.0, 0.0, 0.0);
    }
    let q = cross(s, tri.e1);
    let v = dot(dir, q) * inv;
    if (!(v >= -1.0e-6) || (u + v > 1.0 + 1.0e-6)) {
        return vec4<f32>(-1.0, 0.0, 0.0, 0.0);
    }
    let t = dot(tri.e2, q) * inv;
    return vec4<f32>(t, u, v, det);
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

// The nearest triangle hit by o + t·d (unit d) with PT_EPS < t < t_max; t = -1 for a miss.
// Ties by the lower triangle index. Barycentrics u, v of the hit.
fn trace(o: vec3<f32>, d: vec3<f32>, t_max: f32) -> Hit {
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
    let inv = vec3<f32>(1.0 / d.x, 1.0 / d.y, 1.0 / d.z);
    var best_t = t_max;
    var best_tri = 0u;
    var best_u = 0.0;
    var best_v = 0.0;
    var have_best = false;
    var stack: array<u32, STACK_SIZE>;
    var stack_t: array<f32, STACK_SIZE>;
    let root = box_entry(nodes[0], o, d, inv, t_max);
    if (root < 0.0) {
        return h;
    }
    stack[0] = 0u;
    stack_t[0] = root;
    var top = 1u;
    while (top > 0u) {
        top = top - 1u;
        let index = stack[top];
        let entry = stack_t[top];
        if (entry > best_t) {
            continue;
        }
        let node = nodes[index];
        if (node.count > 0u) {
            let first = node.left_or_first;
            let end = min(node.left_or_first + node.count, arrayLength(&tris));
            for (var i = first; i < end; i = i + 1u) {
                let hit = tri_hit(tris[i], o, d);
                let t = hit.x;
                // best_t starts at t_max, so a hit at or past t_max is never taken (as trace_cpu).
                if (t > PT_EPS && (t < best_t || (have_best && abs(t - best_t) <= 1.0e-5 && i < best_tri))) {
                    best_t = t;
                    best_tri = i;
                    best_u = hit.y;
                    best_v = hit.z;
                    have_best = true;
                }
            }
            continue;
        }
        let left_index = index + 1u;
        let left_entry = box_entry(nodes[left_index], o, d, inv, best_t);
        let right_index = node.left_or_first;
        let right_entry = box_entry(nodes[right_index], o, d, inv, best_t);
        if (left_entry >= 0.0 && right_entry >= 0.0) {
            if (left_entry <= right_entry) {
                stack[top] = right_index;
                stack_t[top] = right_entry;
                top = top + 1u;
                stack[top] = left_index;
                stack_t[top] = left_entry;
                top = top + 1u;
            } else {
                stack[top] = left_index;
                stack_t[top] = left_entry;
                top = top + 1u;
                stack[top] = right_index;
                stack_t[top] = right_entry;
                top = top + 1u;
            }
        } else if (left_entry >= 0.0) {
            stack[top] = left_index;
            stack_t[top] = left_entry;
            top = top + 1u;
        } else if (right_entry >= 0.0) {
            stack[top] = right_index;
            stack_t[top] = right_entry;
            top = top + 1u;
        }
    }
    if (!have_best) {
        return h;
    }
    h.t = best_t;
    h.tri = best_tri;
    h.u = best_u;
    h.v = best_v;
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

