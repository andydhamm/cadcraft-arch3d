// Path tracer: lights, sun and sky.
//
// Units (see render3d::pathtrace): a point or spot light's rgb is its intensity in cd times the
// colour (luminance 1), so li = rgb / d² is lux on a surface facing it; an area light's rgb is its
// radiance in nits; the sun is frame.sun_rgb in lux; the sky is radiance in nits.

// Light arriving at p from one light: along unit dir, from distance dist (PT_INF for the sun),
// radiance-like value li such that the contribution is bsdf_eval(...) * li / pdf; delta = 1 for
// a point, spot or sun (pdf 1, no MIS), 0 for an area light (pdf in solid angle).
struct LightSample { dir: vec3<f32>, dist: f32, li: vec3<f32>, pdf: f32, delta: u32 };

fn light_luminance(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// A spot's cone factor toward c = cos(angle from the aim): 0 beyond cos_outer, 1 inside cos_inner
// (or when the cone has no falloff), a smoothstep between (SceneLight::intensity_toward; a
// direction exactly on the edge counts as inside).
fn spot_cone_factor(l: Light, c: f32) -> f32 {
    if (c < l.cos_outer - 1.0e-6) {
        return 0.0;
    }
    if (c >= l.cos_inner - 1.0e-6 || l.cos_inner <= l.cos_outer) {
        return 1.0;
    }
    let t = clamp((c - l.cos_outer) / (l.cos_inner - l.cos_outer), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

// Sample light i from p with two uniform numbers. Point: li = rgb / d²; spot: times the cone
// (smoothstep between cos_outer and cos_inner, as SceneLight::intensity_toward); IES: the table's
// candela instead (ies_candela); area: a uniform point on the rectangle, li = its radiance, pdf =
// d² / (area · cos at the light) per solid angle (so li / pdf is the radiance times the rectangle's
// projected area over d², and the light-side cosine lives in the pdf); nothing behind the face.
// An area light that has an IES table is a point source at its centre with the table's candela
// (as SceneLight::intensity_toward). A sample that cannot light p has li 0.
fn light_sample(i: u32, p: vec3<f32>, u: vec2<f32>) -> LightSample {
    var s: LightSample;
    s.dir = vec3<f32>(0.0, 0.0, 1.0);
    s.dist = 0.0;
    s.li = vec3<f32>(0.0);
    s.pdf = 1.0;
    s.delta = 1u;
    if (i >= arrayLength(&lights)) {
        return s;
    }
    let l = lights[i];
    if (l.kind == LIGHT_STRIP) {
        // A strip segment: a point uniformly along it (u.x), on the emitting side 2 mm below the
        // housing face; li = rgb (cd per metre) x length x max(0, n_e.-dir) / d^2, pdf 1, delta (the
        // lens is camera-only emissive, so no MIS).
        let q = l.position + l.axis * (u.x * l.length) + l.direction * 0.002;
        let to_q = q - p;
        let d2 = dot(to_q, to_q);
        if (!(d2 > 1.0e-12)) {
            return s;
        }
        let dist = sqrt(d2);
        s.dir = to_q / dist;
        s.dist = dist;
        s.li = l.rgb * (l.length * max(0.0, dot(-s.dir, l.direction)) / d2);
        return s;
    }
    if (l.kind != LIGHT_AREA || l.ies_v > 0u) {
        let to_light = l.position - p;
        let d2 = dot(to_light, to_light);
        if (!(d2 > 1.0e-12)) {
            return s;
        }
        let dist = sqrt(d2);
        s.dir = to_light / dist;
        s.dist = dist;
        let to = -s.dir;
        if (l.ies_v > 0u) {
            // The table is in cd; rgb carries the colour (its luminance, a light's candela, cancels).
            let y = light_luminance(l.rgb);
            if (y > 0.0) {
                s.li = l.rgb * (ies_candela(l, to) / (y * d2));
            }
        } else if (l.kind == LIGHT_SPOT) {
            s.li = l.rgb * (spot_cone_factor(l, dot(to, l.direction)) / d2);
        } else {
            s.li = l.rgb / d2;
        }
        return s;
    }
    // Area light: a uniform point on the width x length rectangle around position.
    s.delta = 0u;
    let area = l.width * l.length;
    if (!(area > 0.0)) {
        return s;
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
    let q = l.position + axis * ((u.x - 0.5) * l.width) + side * ((u.y - 0.5) * l.length);
    let to_q = q - p;
    let d2 = dot(to_q, to_q);
    if (!(d2 > 1.0e-12)) {
        return s;
    }
    let dist = sqrt(d2);
    let dir = to_q / dist;
    let cos_l = dot(-dir, l.direction);
    if (!(cos_l > 0.0)) {
        return s;
    }
    s.dir = dir;
    s.dist = dist;
    s.li = l.rgb;
    s.pdf = d2 / (area * cos_l);
    return s;
}

// The sun toward frame.sun_dir: li = frame.sun_rgb (lux), delta, dist PT_INF; zero when FRAME_SUN
// is off.
fn sun_sample() -> LightSample {
    var s: LightSample;
    s.dir = frame.sun_dir.xyz;
    s.dist = PT_INF;
    s.li = vec3<f32>(0.0);
    if ((frame.flags & FRAME_SUN) != 0u) {
        s.li = frame.sun_rgb.xyz;
    }
    s.pdf = 1.0;
    s.delta = 1u;
    return s;
}

// Radiance from the sky (or the furnace environment) along unit d; zero below the horizon and when
// neither FRAME_SKY nor FRAME_FURNACE is set. The sky runs from the horizon's radiance at d.z = 0
// to the zenith's at d.z = 1 (sqrt of the elevation's sine, so most of the dome is near zenith).
fn sky_radiance(d: vec3<f32>) -> vec3<f32> {
    if ((frame.flags & FRAME_FURNACE) != 0u) {
        return vec3<f32>(frame.furnace.z);
    }
    if ((frame.flags & FRAME_SKY) != 0u && d.z >= 0.0) {
        let t = min(sqrt(max(d.z, 0.0)), 1.0);
        return mix(frame.sky_horizon.xyz, frame.sky_zenith.xyz, t);
    }
    return vec3<f32>(0.0);
}

// [p19-sun] what of the sun reaches o along d (cast shadows on/off).
fn sun_visibility(o: vec3<f32>, d: vec3<f32>) -> vec3<f32> { if ((frame.flags & FRAME_SUN_NO_SHADOW) != 0u) { return vec3<f32>(1.0); } return visibility(o, d, 1.0e6); }
// [p19-env-path]
fn env_direct(m: Material, p: vec3<f32>, n: vec3<f32>, wo: vec3<f32>, r: ptr<function, Rng>) -> vec3<f32> { return vec3<f32>(0.0); }
// [p19-env-path] the radiance an escaping ray sees (sky or environment map); depth, mode and prev_pdf are for MIS.
fn env_escape(d: vec3<f32>, depth: u32, mode: u32, prev_pdf: f32) -> vec3<f32> { return sky_radiance(d); }
// [p19-env-path]
fn env_irradiance(o: vec3<f32>, n: vec3<f32>, r: ptr<function, Rng>) -> vec3<f32> { return vec3<f32>(0.0); }
// [p19-sky]
fn moon_direct(m: Material, p: vec3<f32>, n: vec3<f32>, wo: vec3<f32>) -> vec3<f32> { return vec3<f32>(0.0); }
// [p19-sky]
fn moon_irradiance(o: vec3<f32>, n: vec3<f32>) -> vec3<f32> { return vec3<f32>(0.0); }
// [p19-normal-maps-render] the shading normal at barycentrics (u, v) of triangle tri; n is the geometric normal facing the ray.
fn pt_shading_normal(tri: u32, n: vec3<f32>, u: f32, v: f32) -> vec3<f32> { return n; }

// Two neighbouring entries of an angle list and how far between them (0 = lo, 1 = hi).
struct IesBracket { lo: u32, hi: u32, t: f32 };

// The fraction of x between a and b (0 when they coincide).
fn ies_fraction(a: f32, b: f32, x: f32) -> f32 {
    let span = b - a;
    if (span > 0.0) {
        return clamp((x - a) / span, 0.0, 1.0);
    }
    return 0.0;
}

// The neighbours of x in the n ascending angles at ies[base ..], an x outside them taking the
// nearest end (cadcraft_arch::ies bracket_clamped).
fn ies_bracket_clamped(base: u32, n: u32, x: f32) -> IesBracket {
    // upper: how many of the angles are <= x (binary search).
    var lo = 0u;
    var hi = n;
    while (lo < hi) {
        let mid = (lo + hi) >> 1u;
        if (ies[base + mid] <= x) {
            lo = mid + 1u;
        } else {
            hi = mid;
        }
    }
    var b: IesBracket;
    b.lo = 0u;
    b.hi = 0u;
    b.t = 0.0;
    if (lo == 0u || n == 0u) {
        return b;
    }
    if (lo >= n) {
        b.lo = n - 1u;
        b.hi = n - 1u;
        return b;
    }
    b.lo = lo - 1u;
    b.hi = lo;
    b.t = ies_fraction(ies[base + b.lo], ies[base + b.hi], x);
    return b;
}

fn ies_near(a: f32, b: f32) -> bool {
    return abs(a - b) < 1.0e-3;
}

// The horizontal columns a (degrees, any value) falls between once folded by the table's symmetry:
// one angle = the same everywhere; last 90 = mirrored into 0..90 in each quadrant; last 180 =
// mirrored across 0-180; 90..270 = mirrored across 90 and 270; otherwise the circle (the last angle
// wraps round to the first + 360). n >= 1, the angles at ies[base ..].
fn ies_horizontal_bracket(base: u32, n: u32, deg: f32) -> IesBracket {
    var h = deg - 360.0 * floor(deg / 360.0);
    if (n == 1u) {
        var b: IesBracket;
        b.lo = 0u;
        b.hi = 0u;
        b.t = 0.0;
        return b;
    }
    let first = ies[base];
    let last = ies[base + n - 1u];
    if (ies_near(last, 90.0)) {
        if (h > 180.0) {
            h = 360.0 - h;
        }
        if (h > 90.0) {
            h = 180.0 - h;
        }
    } else if (ies_near(last, 180.0)) {
        if (h > 180.0) {
            h = 360.0 - h;
        }
    } else if (ies_near(first, 90.0) && ies_near(last, 270.0)) {
        if (h < 90.0) {
            h = 180.0 - h;
        } else if (h > 270.0) {
            h = 540.0 - h;
        }
    } else {
        if (h < first) {
            h = h + 360.0;
        }
        if (h > last) {
            var wrap: IesBracket;
            wrap.lo = n - 1u;
            wrap.hi = 0u;
            wrap.t = ies_fraction(last, first + 360.0, h);
            return wrap;
        }
    }
    return ies_bracket_clamped(base, n, h);
}

// Candela of light l's IES table toward the unit direction to_point (from the light), bilinear,
// with the table's symmetry (arch::ies::candela_at); l.ies_v == 0: 0. The vertical angle is the
// angle from the aim (l.direction); the horizontal angle runs counter-clockwise (seen against the
// aim) from l.axis, atan2(q . (axis x aim), q . axis) for q the part of to_point perpendicular to
// the aim (arch3d::light::ies_toward). The table is read from ies[l.ies_offset ..]: ies_v vertical
// angles, ies_h horizontal angles, then candela horizontal-major. A vertical angle outside the
// table's range is 0 cd.
fn ies_candela(l: Light, to_point: vec3<f32>) -> f32 {
    let nv = l.ies_v;
    let nh = l.ies_h;
    if (nv == 0u || nh == 0u) {
        return 0.0;
    }
    let c = clamp(dot(to_point, l.direction), -1.0, 1.0);
    let q = to_point - l.direction * c;
    let sin_theta = length(q);
    let theta = degrees(atan2(sin_theta, c));
    var phi = 0.0;
    if (sin_theta >= 1.0e-6) {
        let side = cross(l.axis, l.direction);
        phi = degrees(atan2(dot(q, side), dot(q, l.axis)));
        if (phi < 0.0) {
            phi = phi + 360.0;
        }
    }
    let v_base = l.ies_offset;
    let h_base = v_base + nv;
    let c_base = h_base + nh;
    if (!(theta >= ies[v_base]) || theta > ies[v_base + nv - 1u]) {
        return 0.0;
    }
    let v = ies_bracket_clamped(v_base, nv, theta);
    let h = ies_horizontal_bracket(h_base, nh, phi);
    let row_lo = c_base + h.lo * nv;
    let row_hi = c_base + h.hi * nv;
    let along_lo = mix(ies[row_lo + v.lo], ies[row_lo + v.hi], v.t);
    let along_hi = mix(ies[row_hi + v.lo], ies[row_hi + v.hi], v.t);
    return mix(along_lo, along_hi, h.t);
}

// An area light's radiance seen along unit d from a surface, when a BSDF-sampled ray of length t
// hits it (for MIS with light_sample): zero behind its face (and for a light that is not an area,
// or is an area with an IES table, which is a point source and no geometry).
fn area_radiance(i: u32, d: vec3<f32>) -> vec3<f32> {
    if (i >= arrayLength(&lights)) {
        return vec3<f32>(0.0);
    }
    let l = lights[i];
    if (l.kind == LIGHT_AREA && l.ies_v == 0u && dot(-d, l.direction) > 0.0) {
        return l.rgb;
    }
    return vec3<f32>(0.0);
}

// ---- Test entry (render3d lights_tests; no pipeline uses it) --------------------------------------
//
// Probe i selects what to run by probes[i]._a:
//   a light index (0 .. 999_999): sample that light u32(probes[i]._b) times from probes[i].point and
//     write accum[i] = vec4(mean of li * max(dot(normal, dir), 0) / pdf, mean pdf), the irradiance
//     estimate on a surface with probes[i].normal. A perfect-square count uses a jittered grid over
//     the two random numbers (a steady estimate); any other count uses plain random numbers.
//   -1: accum[i] = vec4(sky_radiance(normalize(normal)), 0)
//   -2: the sun sample: vec4(li, delta)
//   -3: the sun sample: vec4(dir, 1 when dist is PT_INF else 0)
//   1_000_000 + k: vec4(ies_candela(lights[k], normalize(normal)), 0, 0, 0)
//   2_000_000 + k: vec4(area_radiance(k, normalize(normal)), 0)
@compute @workgroup_size(64)
fn lights_test_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.probe_count) {
        return;
    }
    let probe = probes[id.x];
    let mode = probe._a;
    if (mode < 0.0) {
        if (mode > -1.5) {
            accum[id.x] = vec4<f32>(sky_radiance(normalize(probe.normal)), 0.0);
        } else if (mode > -2.5) {
            let sun = sun_sample();
            accum[id.x] = vec4<f32>(sun.li, f32(sun.delta));
        } else {
            let sun = sun_sample();
            accum[id.x] = vec4<f32>(sun.dir, select(0.0, 1.0, sun.dist >= PT_INF));
        }
        return;
    }
    if (mode >= 2000000.0) {
        accum[id.x] = vec4<f32>(area_radiance(u32(mode - 2000000.0), normalize(probe.normal)), 0.0);
        return;
    }
    if (mode >= 1000000.0) {
        let k = u32(mode - 1000000.0);
        accum[id.x] = vec4<f32>(ies_candela(lights[k], normalize(probe.normal)), 0.0, 0.0, 0.0);
        return;
    }
    let light = u32(mode);
    let n = max(u32(probe._b), 1u);
    let grid = u32(round(sqrt(f32(n))));
    let stratified = grid * grid == n;
    var sum = vec3<f32>(0.0);
    var pdf_sum = 0.0;
    for (var k = 0u; k < n; k = k + 1u) {
        var r = rng_new(id.x, k, frame.seed);
        var u = vec2<f32>(rand(&r), rand(&r));
        if (stratified) {
            u = (vec2<f32>(f32(k % grid), f32(k / grid)) + u) / f32(grid);
        }
        let s = light_sample(light, probe.point, u);
        sum = sum + s.li * (max(dot(probe.normal, s.dir), 0.0) / s.pdf);
        pdf_sum = pdf_sum + s.pdf;
    }
    accum[id.x] = vec4<f32>(sum / f32(n), pdf_sum / f32(n));
}
