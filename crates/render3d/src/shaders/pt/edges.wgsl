// Path tracer (phase 22): edge rounding. pt_round_edge bends the shading normal near an edge of
// triangle `tri` at world point p toward the neighbouring face, so a sharp edge catches a highlight.
// Called in path_radiance right after pt_shading_normal.

fn oct_decode_dir(word: u32) -> vec3<f32> {
    let x = f32(i32(word & 0x7fffu)) / 32767.0;
    let y = f32(i32((word >> 15u) & 0x7fffu)) / 32767.0;
    var nx = x;
    var ny = y;
    var nz = 1.0 - abs(nx) - abs(ny);
    if (nz < 0.0) {
        let ox = nx;
        let oy = ny;
        nx = (1.0 - abs(oy)) * select(1.0, -1.0, ox < 0.0);
        ny = (1.0 - abs(ox)) * select(1.0, -1.0, oy < 0.0);
        nz = 1.0 - abs(nx) - abs(ny);
    }
    let v = vec3<f32>(nx, ny, nz);
    if (dot(v, v) <= 1.0e-12) {
        return vec3<f32>(0.0, 0.0, 1.0);
    }
    return normalize(v);
}

fn pt_round_edge(tri: u32, n: vec3<f32>, p: vec3<f32>) -> vec3<f32> {
    if ((frame.photo.x & PHOTO_EDGES) == 0u) {
        return n;
    }
    let t = tris[tri];
    var radius = frame.photo_f.x;
    if (radius < 0.0) {
        radius = materials[t.material].edge_radius;
    }
    if (radius <= 0.0) {
        return n;
    }
    var result = n;
    var wsum = 0.0;
    for (var e = 0u; e < 3u; e = e + 1u) {
        let entry = bitcast<u32>(ies[frame.photo.y + tri * 3u + e]);
        if ((entry & 0x80000000u) == 0u) {
            continue;
        }
        var a = t.v0;
        var b = t.v0;
        if (e == 0u) {
            a = t.v0;
            b = t.v0 + t.e1;
        } else if (e == 1u) {
            a = t.v0 + t.e1;
            b = t.v0 + t.e2;
        } else {
            a = t.v0 + t.e2;
            b = t.v0;
        }
        let ab = b - a;
        let h = clamp(dot(p - a, ab) / max(dot(ab, ab), 1.0e-6), 0.0, 1.0);
        let q = a + ab * h;
        let d = length(p - q);
        if (d >= radius) {
            continue;
        }
        let t0 = clamp(d / radius, 0.0, 1.0);
        let edge_t = smoothstep(0.0, 1.0, t0);
        let n_face = n;
        let n_nei = oct_decode_dir(entry & 0x7fffffffu);
        let bis = normalize(n_face + n_nei);
        let bent = normalize(mix(bis, n_face, edge_t));
        let w = 1.0 - edge_t;
        result = normalize(result * (1.0 - w) + bent * w);
        wsum = wsum + w;
    }
    if (wsum > 0.0) {
        result = normalize(result);
    }
    let dn = dot(result, n);
    if (dn < 0.05) {
        result = normalize(result + (0.05 - dn) * n);
    }
    if (dot(result, n) < 0.0) {
        result = -result;
    }
    return normalize(result);
}
