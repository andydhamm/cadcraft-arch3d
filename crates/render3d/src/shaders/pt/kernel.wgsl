// Path tracer: the entry points and hook stubs.
//
// The hooks below are part of the shared WGSL kernel; later packages fill the stubs, while the
// traversal, BSDF, lights and kernel code here stays byte-identical across both kernels.
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

// Up to this many lights are all sampled at every vertex; more, and one chosen uniformly is.
const ALL_LIGHTS_MAX: u32 = 16u;

// How the previous ray treats the area lights it meets.
const AREA_SEEN: u32 = 0u; // visible as they are (camera ray, after a delta lobe)
const AREA_MIS: u32 = 1u; // weighted against the light sample
const AREA_SKIP: u32 = 2u; // already counted by a direct estimate

fn power_heuristic(a: f32, b: f32) -> f32 {
    let a2 = a * a;
    let b2 = b * b;
    if (!(a2 + b2 > 0.0)) {
        return 0.0;
    }
    return a2 / (a2 + b2);
}

fn finite_rgb(c: vec3<f32>) -> bool {
    return c.x == c.x && c.y == c.y && c.z == c.z && abs(c.x) < PT_INF && abs(c.y) < PT_INF && abs(c.z) < PT_INF;
}

// Where the ray o + t d crosses area light l's rectangle from the front: t, or -1. The cosine at
// the light comes back through cos_l.
fn area_hit(l: Light, o: vec3<f32>, d: vec3<f32>, cos_l: ptr<function, f32>) -> f32 {
    if (l.kind != LIGHT_AREA || l.ies_v > 0u) {
        return -1.0;
    }
    let denom = dot(d, l.direction);
    if (!(denom < -1.0e-6)) {
        return -1.0;
    }
    let t = dot(l.position - o, l.direction) / denom;
    if (!(t > PT_EPS)) {
        return -1.0;
    }
    var axis = l.axis;
    var side = cross(l.direction, axis);
    let side_len = length(side);
    if (side_len < 1.0e-6) {
        let b = onb(l.direction);
        axis = b[0];
        side = b[1];
    } else {
        side = side / side_len;
    }
    let q = o + d * t - l.position;
    if (abs(dot(q, axis)) > 0.5 * l.width || abs(dot(q, side)) > 0.5 * l.length) {
        return -1.0;
    }
    *cos_l = -denom;
    return t;
}

// The radiance of the area lights a ray meets before t_hit (< 0: nothing else), weighted by `mode`.
// prev_pdf is the BSDF pdf of the ray's direction; sel the probability a light sample picks a light.
fn area_lights_seen(o: vec3<f32>, d: vec3<f32>, t_hit: f32, mode: u32, prev_pdf: f32, sel: f32) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    if (mode == AREA_SKIP) {
        return sum;
    }
    let limit = select(t_hit, PT_INF, t_hit < 0.0);
    let count = min(arrayLength(&lights), frame.light_count);
    for (var i = 0u; i < count; i = i + 1u) {
        var cos_l = 0.0;
        let l = lights[i];
        let t = area_hit(l, o, d, &cos_l);
        if (t < 0.0 || t >= limit) {
            continue;
        }
        var w = 1.0;
        if (mode == AREA_MIS) {
            let pdf_l = sel * t * t / (l.width * l.length * cos_l);
            w = power_heuristic(prev_pdf, pdf_l);
        }
        sum = sum + l.rgb * w;
    }
    return sum;
}

// The lights one vertex samples: all (weight 1) when there are few, else one chosen uniformly
// (weight light_count).
struct Pick { first: u32, count: u32, weight: f32 };

fn pick_lights(r: ptr<function, Rng>) -> Pick {
    var p: Pick;
    let n = min(arrayLength(&lights), frame.light_count);
    p.first = 0u;
    p.count = n;
    p.weight = 1.0;
    if (n > ALL_LIGHTS_MAX) {
        p.first = min(u32(rand(r) * f32(n)), n - 1u);
        p.count = 1u;
        p.weight = f32(n);
    }
    return p;
}

// The radiance a vertex p (normal n, viewer direction wo) reflects toward wo of the light from the
// lights and the sun.
fn direct_light(m: Material, p: vec3<f32>, n: vec3<f32>, wo: vec3<f32>, r: ptr<function, Rng>) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    let o = p + n * PT_EPS;
    let pick = pick_lights(r);
    for (var k = 0u; k < pick.count; k = k + 1u) {
        let u = vec2<f32>(rand(r), rand(r));
        let s = light_sample(pick.first + k, o, u);
        if (!(light_luminance(s.li) > 0.0) || !(s.pdf > 0.0)) {
            continue;
        }
        let f = bsdf_eval(m, n, wo, s.dir);
        if (!(light_luminance(f) > 0.0)) {
            continue;
        }
        let v = visibility(o, s.dir, s.dist - PT_EPS);
        if (!(light_luminance(v) > 0.0)) {
            continue;
        }
        var w = 1.0;
        if (s.delta == 0u) {
            w = power_heuristic(s.pdf / pick.weight, bsdf_pdf(m, n, wo, s.dir));
        }
        sum = sum + f * s.li * v * (pick.weight * w / s.pdf);
    }
    let sun = sun_sample();
    if (light_luminance(sun.li) > 0.0) {
        let f = bsdf_eval(m, n, wo, sun.dir);
        if (light_luminance(f) > 0.0) {
            sum = sum + f * sun.li * sun_visibility(o, sun.dir);
        }
    }
    sum = sum + env_direct(m, p, n, wo, r) + moon_direct(m, p, n, wo);
    return sum;
}

// What a path adds up to and what its first hit looked like.
struct PathResult { radiance: vec3<f32>, albedo: vec3<f32>, normal: vec3<f32>, t: f32, hit: u32 };

// The radiance arriving along the ray (o, d) (unit d) of a path that has made start_depth
// vertices; `mode` and `prev_pdf` say how its first segment treats area lights.
fn path_radiance(o_in: vec3<f32>, d_in: vec3<f32>, start_depth: u32, mode_in: u32, prev_pdf_in: f32, r: ptr<function, Rng>) -> PathResult {
    var res: PathResult;
    res.radiance = vec3<f32>(0.0);
    res.albedo = vec3<f32>(0.0);
    res.normal = vec3<f32>(0.0);
    res.t = 0.0;
    res.hit = 0u;
    var o = o_in;
    var d = d_in;
    var mode = mode_in;
    var prev_pdf = prev_pdf_in;
    var throughput = vec3<f32>(1.0);
    let n_lights = min(arrayLength(&lights), frame.light_count);
    let sel = select(1.0 / f32(max(n_lights, 1u)), 1.0, n_lights <= ALL_LIGHTS_MAX);
    for (var depth = start_depth; depth < MAX_PATH_DEPTH; depth = depth + 1u) {
        let h = trace(o, d, PT_INF);
        res.radiance = res.radiance + throughput * area_lights_seen(o, d, h.t, mode, prev_pdf, sel);
        if (h.t < 0.0) {
            // The sky has no light sample, so every ray that escapes counts it.
            res.radiance = res.radiance + throughput * env_escape(d, depth, mode, prev_pdf);
            break;
        }
        let tri = tris[h.tri];
        var n = tri.n;
        if (dot(n, d) > 0.0) {
            n = -n;
        }
        n = pt_shading_normal(h.tri, n, h.u, h.v);
        let p = o + d * h.t;
        let m = materials[tri.material];
        if (depth == 0u) {
            res.albedo = m.albedo;
            res.normal = n;
            res.t = h.t;
            res.hit = 1u;
        }
        if (depth == 0u || (m.flags & MAT_EMIT_CAMERA_ONLY) == 0u) {
            res.radiance = res.radiance + throughput * m.emission;
        }
        let glass = (m.flags & MAT_GLASS) != 0u;
        if (!glass) {
            res.radiance = res.radiance + throughput * direct_light(m, p, n, -d, r);
        }
        if (frame.bounces == 0u) {
            break;
        }
        // Glass keeps the true side of the surface (entering or leaving).
        let ns = select(n, tri.n, glass);
        let s = bsdf_sample(m, ns, -d, vec3<f32>(rand(r), rand(r), rand(r)));
        if (!(s.pdf > 0.0) || !(light_luminance(s.weight) > 0.0)) {
            break;
        }
        throughput = throughput * s.weight;
        if (s.specular != 0u) {
            mode = AREA_SEEN;
        } else {
            mode = AREA_MIS;
            prev_pdf = s.pdf;
        }
        // Step off the surface on the side the new direction leaves to.
        o = p + select(1.0, -1.0, dot(s.dir, tri.n) < 0.0) * tri.n * PT_EPS;
        d = s.dir;
        if (depth >= frame.bounces) {
            let q = min(max(throughput.x, max(throughput.y, throughput.z)), 0.95);
            if (!(rand(r) < q)) {
                break;
            }
            throughput = throughput / q;
        }
    }
    return res;
}

@compute @workgroup_size(8, 8)
fn trace_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.size.x || id.y >= frame.size.y) {
        return;
    }
    let i = id.y * frame.size.x + id.x;
    var sum = vec3<f32>(0.0);
    var sq = 0.0;
    var hits = 0u;
    for (var k = 0u; k < frame.spp_this_pass; k = k + 1u) {
        let s = frame.sample_base + k;
        var r = rng_new(i, s, frame.seed);
        let px = f32(id.x) + rand(&r);
        let py = f32(id.y) + rand(&r);
        let ndc = vec2<f32>(2.0 * px / f32(frame.size.x) - 1.0, 1.0 - 2.0 * py / f32(frame.size.y));
        let far4 = frame.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
        let near4 = frame.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
        let dir = normalize(far4.xyz / far4.w - near4.xyz / near4.w);
        let t0 = camera_t_min(dir);
        let res = path_radiance(frame.eye.xyz + dir * t0, dir, 0u, AREA_SEEN, 0.0, &r);
        hits = hits + res.hit;
        if (finite_rgb(res.radiance)) {
            let l = dot(res.radiance, vec3<f32>(0.2126, 0.7152, 0.0722));
            sum = sum + res.radiance;
            sq = sq + l * l;
        }
        if (s == 0u) {
            aov[2u * i] = vec4<f32>(res.albedo, f32(res.hit));
            aov[2u * i + 1u] = vec4<f32>(res.normal, res.t);
        }
    }
    accum[i] = accum[i] + vec4<f32>(sum, f32(frame.spp_this_pass));
    accum[frame.size.x * frame.size.y + i].x = accum[frame.size.x * frame.size.y + i].x + sq;
    // The samples whose camera ray hit a surface: a pixel still at 0 missed on every sample, and only such a pixel
    // shows the backdrop (tracer.rs `backdrop_composite`), so an edge pixel some samples hit keeps its shading.
    accum[frame.size.x * frame.size.y + i].y = accum[frame.size.x * frame.size.y + i].y + f32(hits);
}

@compute @workgroup_size(64)
fn irradiance_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.probe_count) {
        return;
    }
    let probe = probes[id.x];
    let n = normalize(probe.normal);
    let o = probe.point + n * PT_EPS;
    var sum = vec3<f32>(0.0);
    for (var k = 0u; k < frame.spp_this_pass; k = k + 1u) {
        var r = rng_new(id.x, frame.sample_base + k, frame.seed);
        var e = vec3<f32>(0.0);
        let pick = pick_lights(&r);
        for (var j = 0u; j < pick.count; j = j + 1u) {
            let s = light_sample(pick.first + j, o, vec2<f32>(rand(&r), rand(&r)));
            let cos_n = max(dot(n, s.dir), 0.0);
            if (!(light_luminance(s.li) > 0.0) || !(s.pdf > 0.0) || !(cos_n > 0.0)) {
                continue;
            }
            let v = visibility(o, s.dir, s.dist - PT_EPS);
            e = e + s.li * v * (cos_n * pick.weight / s.pdf);
        }
        let sun = sun_sample();
        let cos_sun = max(dot(n, sun.dir), 0.0);
        if (light_luminance(sun.li) > 0.0 && cos_sun > 0.0) {
            e = e + sun.li * sun_visibility(o, sun.dir) * cos_sun;
        }
        e = e + env_irradiance(o, n, &r) + moon_irradiance(o, n);
        if (frame.bounces > 0u) {
            let u1 = rand(&r);
            let phi = 2.0 * PT_PI * rand(&r);
            let rad = sqrt(u1);
            let local = vec3<f32>(rad * cos(phi), rad * sin(phi), sqrt(max(0.0, 1.0 - u1)));
            let dir = normalize(onb(n) * local);
            let res = path_radiance(o, dir, 1u, AREA_SKIP, 0.0, &r);
            e = e + PT_PI * res.radiance;
        }
        if (finite_rgb(e)) {
            sum = sum + e;
        }
    }
    accum[id.x] = accum[id.x] + vec4<f32>(sum, f32(frame.spp_this_pass));
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
