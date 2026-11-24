// Path tracer (phase 22): the clear coat, sheen and anisotropy lobes of the layered material
// (MAT_EXT). bsdf_sample, bsdf_eval and bsdf_pdf defer to ext_sample, ext_eval and ext_pdf when
// ext_active(m) is true; every other material runs bsdf.wgsl untouched.
//
// The model (formulas from the glTF KHR_materials_clearcoat / _sheen / _anisotropy
// specifications), from the top layer down:
//   coat   a GGX lobe, F0 0.04, alpha = coat_roughness^2, weight m.coat; everything below is
//          scaled by 1 - coat * F(NoV);
//   sheen  the Charlie distribution D = (2 + 1/a) sin^(1/a)(theta_h) / (2 pi) with the Neubelt
//          visibility 1 / (4 (NoL + NoV - NoL NoV)), colour m.sheen, a = sheen_roughness^2;
//          the base below is scaled by 1 - max(sheen) * E(NoV, a);
//   base   today's Lambert diffuse + GGX specular with the spec-albedo energy split of
//          bsdf_lobes (bsdf_spec_albedo is the isotropic table; the anisotropic lobe borrows it,
//          its albedo differs by well under a percent of F0). Anisotropy: alpha_b = alpha,
//          alpha_t = mix(alpha, 1, aniso^2), tangent pt_tangent(n, aniso_rot).
//
// E(NoV, a), the sheen lobe's directional albedo, is a bilinear lookup in the table below,
// computed by quadrature of D * V * NoL over the hemisphere (rows: a = 0.02 0.05 0.09 0.25
// 0.5 1; columns: NoV = 0.05 0.2 0.5 0.9 1) and rounded up. The table is convex in NoV, so the
// bilinear interpolation never underestimates it: the base loses at least what the sheen lobe
// returns and the layer pair does not gain energy. (Where the Charlie lobe itself returns more
// than 1, at a grazing view with a low alpha, the base scale is clamped at 0.)
//
// Sampling picks one of four lobes (diffuse, base specular, coat, sheen) with probability
// proportional to its estimated albedo; diffuse and sheen are sampled by cosine, the specular
// lobes by visible normals. The pdf is the mixture sum p_i pdf_i over the lobes that are not
// deltas, and the weight is ext_eval / ext_pdf, so one-sample MIS and next-event estimation
// agree. A perfect-mirror lobe (roughness below BSDF_MIRROR_ROUGHNESS) is a delta: specular = 1,
// weight F / p_i, no eval or pdf.

const EXT_SHEEN_MIN_ALPHA: f32 = 0.02;
const EXT_SHEEN_E: array<f32, 30> = array<f32, 30>(
    1.0, 0.39, 0.016, 0.0, 0.0,
    1.0, 0.50, 0.078, 0.002, 0.0,
    1.0, 0.52, 0.138, 0.009, 0.002,
    0.75, 0.49, 0.230, 0.058, 0.032,
    0.63, 0.46, 0.268, 0.114, 0.084,
    0.55, 0.43, 0.288, 0.168, 0.142,
);
const EXT_SHEEN_A: array<f32, 6> = array<f32, 6>(0.02, 0.05, 0.09, 0.25, 0.5, 1.0);
const EXT_SHEEN_MU: array<f32, 5> = array<f32, 5>(0.05, 0.2, 0.5, 0.9, 1.0);

// The tangent of a normal n rotated about n by rot radians: normalize(cross(z, n)), the X axis
// when n is near +-Z.
fn pt_tangent(n: vec3<f32>, rot: f32) -> vec3<f32> {
    var t0 = vec3<f32>(1.0, 0.0, 0.0);
    let c = cross(vec3<f32>(0.0, 0.0, 1.0), n);
    let l2 = dot(c, c);
    if (l2 > 1.0e-8) {
        t0 = c * inverseSqrt(l2);
    }
    return t0 * cos(rot) + cross(n, t0) * sin(rot);
}

// The sheen lobe's directional albedo E(nov, sheen_roughness) (see the table above), at most 1.
fn ext_sheen_e(nov: f32, sheen_roughness: f32) -> f32 {
    if (nov < EXT_SHEEN_MU[0]) {
        return 1.0;
    }
    let a = clamp(sheen_roughness * sheen_roughness, EXT_SHEEN_MIN_ALPHA, 1.0);
    var ta = EXT_SHEEN_A;
    var tm = EXT_SHEEN_MU;
    var tab = EXT_SHEEN_E;
    var ia = 0u;
    for (var i = 1u; i < 5u; i = i + 1u) {
        if (a >= ta[i]) {
            ia = i;
        }
    }
    var im = 0u;
    for (var i = 1u; i < 4u; i = i + 1u) {
        if (nov >= tm[i]) {
            im = i;
        }
    }
    let fa = clamp((a - ta[ia]) / (ta[ia + 1u] - ta[ia]), 0.0, 1.0);
    let fm = clamp((nov - tm[im]) / (tm[im + 1u] - tm[im]), 0.0, 1.0);
    let r0 = mix(tab[ia * 5u + im], tab[ia * 5u + im + 1u], fm);
    let r1 = mix(tab[(ia + 1u) * 5u + im], tab[(ia + 1u) * 5u + im + 1u], fm);
    return min(mix(r0, r1, fa), 1.0);
}

// The layers' shared numbers for a view cosine nov.
struct ExtMix {
    // Probabilities of the diffuse, base specular, coat and sheen lobes (0 for a lobe that is
    // absent; they sum to 1 unless the whole material is black).
    p: vec4<f32>,
    // The base's Lambert albedo (energy split applied, layers not).
    diffuse: vec3<f32>,
    f0: vec3<f32>,
    alpha_t: f32,
    alpha_b: f32,
    // What the layers above leave the base: (1 - coat F) (1 - sheen E).
    base_layer: f32,
    // coat * F(NoV): the coat lobe's albedo and the share it takes from the layers below.
    coat_albedo: f32,
    // 1 - coat F(NoV): what the coat leaves the sheen.
    coat_pass: f32,
    // Delta lobes: the base specular and the coat.
    base_mirror: u32,
    coat_mirror: u32,
};

// The directional albedo of an anisotropic GGX lobe with Fresnel F0 f0 seen from lo (frame T, B,
// n): the mean of F(VoH) G2 / G1 over 64 visible normals from a fixed low-discrepancy set (the
// R2 sequence), so it is the same number in eval, pdf and sample. It is the lobe's own albedo,
// which the layers below take out of their share: the coat's albedo is below F(NoV) because
// the visible normals tilt toward the viewer, and an anisotropic lobe loses a different
// amount below the horizon than the isotropic table.
fn ext_ggx_albedo(lo: vec3<f32>, at: f32, ab: f32, f0: vec3<f32>) -> vec3<f32> {
    let lam_o = ext_aniso_lambda(lo, at, ab);
    var acc = vec3<f32>(0.0);
    for (var k = 0u; k < 64u; k = k + 1u) {
        let u1 = fract(0.5 + f32(k) * 0.7548776662);
        let u2 = fract(0.5 + f32(k) * 0.5698402910);
        let h = ext_sample_vndf(lo, at, ab, u1, u2);
        let li = reflect(-lo, h);
        if (li.z > 0.0) {
            let g = (1.0 + lam_o) / (1.0 + lam_o + ext_aniso_lambda(li, at, ab));
            acc = acc + bsdf_schlick(f0, max(dot(lo, h), 0.0)) * g;
        }
    }
    return acc / 64.0;
}

// The view direction in the material's frame (tangent, bitangent, normal).
fn ext_local_view(m: Material, n: vec3<f32>, wo: vec3<f32>) -> vec3<f32> {
    let t = pt_tangent(n, m.aniso_rot);
    return vec3<f32>(dot(wo, t), dot(wo, cross(n, t)), dot(wo, n));
}

fn ext_mix(m: Material, lo: vec3<f32>) -> ExtMix {
    let nov = lo.z;
    var x: ExtMix;
    let l = bsdf_lobes(m, nov);
    let metal = clamp(m.metallic, 0.0, 1.0);
    let aniso = clamp(m.aniso, 0.0, 1.0);
    x.f0 = l.f0;
    x.alpha_b = l.alpha;
    x.alpha_t = mix(l.alpha, 1.0, aniso * aniso);
    x.base_mirror = select(0u, 1u, l.mirror != 0u && aniso <= 0.0);
    // Base lobes (the energy split of bsdf_lobes, recomputed because an anisotropic lobe is
    // never a mirror).
    var spec_albedo = vec3<f32>(0.0);
    var diffuse = m.albedo * (1.0 - metal);
    if (l.p_spec > 0.0) {
        if (x.base_mirror != 0u) {
            spec_albedo = l.fresnel;
        } else if (aniso > 0.0) {
            spec_albedo = ext_ggx_albedo(lo, x.alpha_t, x.alpha_b, l.f0);
        } else {
            spec_albedo = bsdf_spec_albedo(l.f0, clamp(m.roughness, 0.0, 1.0), nov);
        }
        diffuse = diffuse * (vec3<f32>(1.0) - spec_albedo);
    }
    x.diffuse = diffuse;
    // The layers.
    let coat = clamp(m.coat, 0.0, 1.0);
    var coat_f = 0.0;
    x.coat_mirror = select(0u, 1u, m.coat_roughness < BSDF_MIRROR_ROUGHNESS);
    if (coat > 0.0) {
        let f0c = vec3<f32>(BSDF_DIELECTRIC_F0);
        if (x.coat_mirror != 0u) {
            coat_f = coat * bsdf_schlick(f0c, nov).x;
        } else {
            let a = ext_coat_alpha(m);
            coat_f = coat * ext_ggx_albedo(lo, a, a, f0c).x;
        }
    }
    x.coat_albedo = coat_f;
    x.coat_pass = 1.0 - coat_f;
    let sc = clamp(max(m.sheen.x, max(m.sheen.y, m.sheen.z)), 0.0, 1.0);
    var sheen_loss = 0.0;
    var w_sheen = 0.0;
    if (sc > 0.0) {
        sheen_loss = clamp(sc * ext_sheen_e(nov, m.sheen_roughness), 0.0, 1.0);
        w_sheen = max(sheen_loss * x.coat_pass, 0.02 * sc);
    }
    x.base_layer = x.coat_pass * (1.0 - sheen_loss);
    // Lobe probabilities from the estimated albedos.
    let w_diff = bsdf_luma(diffuse) * x.base_layer;
    let w_spec = bsdf_luma(spec_albedo) * x.base_layer;
    let w = vec4<f32>(w_diff, w_spec, coat_f, w_sheen);
    let total = w.x + w.y + w.z + w.w;
    x.p = vec4<f32>(0.0);
    if (total > 0.0) {
        x.p = w / total;
    }
    return x;
}

// Anisotropic GGX D for a half vector h in the frame (T, B, n).
fn ext_aniso_d(h: vec3<f32>, at: f32, ab: f32) -> f32 {
    let q = h.x * h.x / (at * at) + h.y * h.y / (ab * ab) + h.z * h.z;
    return 1.0 / (PT_PI * at * ab * q * q);
}

// Smith's Lambda for a direction w in the frame (T, B, n), w.z > 0.
fn ext_aniso_lambda(w: vec3<f32>, at: f32, ab: f32) -> f32 {
    let c2 = max(w.z * w.z, 1.0e-8);
    return 0.5 * (sqrt(1.0 + (at * at * w.x * w.x + ab * ab * w.y * w.y) / c2) - 1.0);
}

// A visible normal (Heitz 2018) of the anisotropic GGX seen from v (z > 0), in the frame
// (T, B, n): the view is stretched by (alpha_t, alpha_b), sampled as a hemisphere, unstretched.
fn ext_sample_vndf(v: vec3<f32>, at: f32, ab: f32, u1: f32, u2: f32) -> vec3<f32> {
    let vh = normalize(vec3<f32>(at * v.x, ab * v.y, v.z));
    let lensq = vh.x * vh.x + vh.y * vh.y;
    var t1 = vec3<f32>(1.0, 0.0, 0.0);
    if (lensq > 1.0e-12) {
        t1 = vec3<f32>(-vh.y, vh.x, 0.0) * inverseSqrt(lensq);
    }
    let t2 = cross(vh, t1);
    let r = sqrt(u1);
    let phi = 2.0 * PT_PI * u2;
    let p1 = r * cos(phi);
    let p2 = r * sin(phi);
    let s = 0.5 * (1.0 + vh.z);
    let q2 = (1.0 - s) * sqrt(max(0.0, 1.0 - p1 * p1)) + s * p2;
    let nh = p1 * t1 + q2 * t2 + sqrt(max(0.0, 1.0 - p1 * p1 - q2 * q2)) * vh;
    return normalize(vec3<f32>(at * nh.x, ab * nh.y, max(nh.z, 1.0e-6)));
}

// Charlie sheen: D(noh) with a = alpha, times the Neubelt visibility, for (nov, nol).
fn ext_charlie_dv(noh: f32, nov: f32, nol: f32, a: f32) -> f32 {
    let sin_h = sqrt(max(1.0 - noh * noh, 0.0));
    let d = (2.0 + 1.0 / a) * exp(log(max(sin_h, 1.0e-12)) / a) / (2.0 * PT_PI);
    let v = 1.0 / (4.0 * (nol + nov - nol * nov));
    return d * v;
}

fn ext_sheen_alpha(m: Material) -> f32 {
    return clamp(m.sheen_roughness * m.sheen_roughness, EXT_SHEEN_MIN_ALPHA, 1.0);
}

// The coat's alpha.
fn ext_coat_alpha(m: Material) -> f32 {
    return max(m.coat_roughness * m.coat_roughness, BSDF_MIN_ALPHA);
}

fn ext_active(m: Material) -> bool {
    return (frame.photo.x & PHOTO_MATERIALS) != 0u && (m.flags & MAT_EXT) != 0u;
}

// f · cos of the whole stack for (wo, wi) at n (delta lobes are not in it).
fn ext_eval(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> vec3<f32> {
    let nov = dot(n, wo);
    let nol = dot(n, wi);
    if (nov <= 0.0 || nol <= 0.0) {
        return vec3<f32>(0.0);
    }
    let x = ext_mix(m, ext_local_view(m, n, wo));
    let t = pt_tangent(n, m.aniso_rot);
    let b = cross(n, t);
    let lo = vec3<f32>(dot(wo, t), dot(wo, b), nov);
    let li = vec3<f32>(dot(wi, t), dot(wi, b), nol);
    let h = normalize(lo + li);
    let voh = max(dot(lo, h), 0.0);
    // Base: diffuse + specular, under the layers.
    var f = x.diffuse * (nol / PT_PI);
    if (x.base_mirror == 0u && x.p.y > 0.0) {
        let g2 = 1.0 / (1.0 + ext_aniso_lambda(lo, x.alpha_t, x.alpha_b) + ext_aniso_lambda(li, x.alpha_t, x.alpha_b));
        f = f + bsdf_schlick(x.f0, voh) * (ext_aniso_d(h, x.alpha_t, x.alpha_b) * g2 / (4.0 * nov));
    }
    f = f * x.base_layer;
    // Sheen, under the coat.
    let sc = max(m.sheen.x, max(m.sheen.y, m.sheen.z));
    if (sc > 0.0) {
        f = f + m.sheen * (ext_charlie_dv(h.z, nov, nol, ext_sheen_alpha(m)) * nol * x.coat_pass);
    }
    // Coat.
    if (x.coat_albedo > 0.0 && x.coat_mirror == 0u) {
        let a = ext_coat_alpha(m);
        let g2 = 1.0 / (1.0 + ggx_lambda(nov, a) + ggx_lambda(nol, a));
        let fr = bsdf_schlick(vec3<f32>(BSDF_DIELECTRIC_F0), voh).x;
        f = f + vec3<f32>(clamp(m.coat, 0.0, 1.0) * fr * ggx_d(h.z, a) * g2 / (4.0 * nov));
    }
    return f;
}

// The mixture pdf ext_sample gives wi (the non-delta lobes).
fn ext_pdf(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> f32 {
    let nov = dot(n, wo);
    let nol = dot(n, wi);
    if (nov <= 0.0 || nol <= 0.0) {
        return 0.0;
    }
    let x = ext_mix(m, ext_local_view(m, n, wo));
    let t = pt_tangent(n, m.aniso_rot);
    let b = cross(n, t);
    let lo = vec3<f32>(dot(wo, t), dot(wo, b), nov);
    let li = vec3<f32>(dot(wi, t), dot(wi, b), nol);
    let h = normalize(lo + li);
    var pdf = (x.p.x + x.p.w) * nol / PT_PI;
    if (x.base_mirror == 0u && x.p.y > 0.0) {
        let g1 = 1.0 / (1.0 + ext_aniso_lambda(lo, x.alpha_t, x.alpha_b));
        pdf = pdf + x.p.y * g1 * ext_aniso_d(h, x.alpha_t, x.alpha_b) / (4.0 * nov);
    }
    if (x.coat_mirror == 0u && x.p.z > 0.0) {
        let a = ext_coat_alpha(m);
        pdf = pdf + x.p.z * (1.0 / (1.0 + ggx_lambda(nov, a))) * ggx_d(h.z, a) / (4.0 * nov);
    }
    return pdf;
}

// One-sample MIS over the lobes: u.x picks the lobe, (u.y, u.z) the direction in it.
fn ext_sample(m: Material, n: vec3<f32>, wo: vec3<f32>, u: vec3<f32>) -> BsdfSample {
    let nov = dot(n, wo);
    if (nov <= 0.0) {
        return bsdf_invalid(n);
    }
    let x = ext_mix(m, ext_local_view(m, n, wo));
    let total = x.p.x + x.p.y + x.p.z + x.p.w;
    if (total <= 0.0) {
        return bsdf_invalid(n);
    }
    let t = pt_tangent(n, m.aniso_rot);
    let b = cross(n, t);
    let lo = vec3<f32>(dot(wo, t), dot(wo, b), nov);
    let pick = u.x * total;
    var s: BsdfSample;
    s.specular = 0u;
    if (pick < x.p.y) {
        // Base specular.
        if (x.base_mirror != 0u) {
            s.dir = normalize(2.0 * nov * n - wo);
            s.weight = bsdf_schlick(x.f0, nov) * (x.base_layer / x.p.y);
            s.pdf = 1.0;
            s.specular = 1u;
            return s;
        }
        let h = ext_sample_vndf(lo, x.alpha_t, x.alpha_b, u.y, u.z);
        s.dir = normalize(reflect(-wo, t * h.x + b * h.y + n * h.z));
    } else if (pick < x.p.x + x.p.y) {
        // Diffuse.
        s.dir = ext_cosine_dir(t, b, n, u.y, u.z);
    } else if (pick < x.p.x + x.p.y + x.p.z) {
        // Coat.
        if (x.coat_mirror != 0u) {
            s.dir = normalize(2.0 * nov * n - wo);
            s.weight = vec3<f32>(clamp(m.coat, 0.0, 1.0) * bsdf_schlick(vec3<f32>(BSDF_DIELECTRIC_F0), nov).x / x.p.z);
            s.pdf = 1.0;
            s.specular = 1u;
            return s;
        }
        let a = ext_coat_alpha(m);
        let h = ext_sample_vndf(lo, a, a, u.y, u.z);
        s.dir = normalize(reflect(-wo, t * h.x + b * h.y + n * h.z));
    } else {
        // Sheen: cosine.
        s.dir = ext_cosine_dir(t, b, n, u.y, u.z);
    }
    if (dot(n, s.dir) <= 0.0) {
        return bsdf_invalid(n);
    }
    s.pdf = ext_pdf(m, n, wo, s.dir);
    if (s.pdf <= 0.0) {
        return bsdf_invalid(n);
    }
    s.weight = ext_eval(m, n, wo, s.dir) / s.pdf;
    return s;
}

// A cosine-weighted direction about n in the frame (t, b, n).
fn ext_cosine_dir(t: vec3<f32>, b: vec3<f32>, n: vec3<f32>, u1: f32, u2: f32) -> vec3<f32> {
    let r = sqrt(u1);
    let phi = 2.0 * PT_PI * u2;
    return normalize(t * (r * cos(phi)) + b * (r * sin(phi)) + n * sqrt(max(0.0, 1.0 - u1)));
}

// Test entry: for probe i (material _a, normal, view = point), integrates bsdf_pdf over the
// hemisphere on a grid uniform in cos(theta) and phi; accum[i] = (integral, 0, 0, 0).
@compute @workgroup_size(64)
fn bsdf_grid_test_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.probe_count) {
        return;
    }
    let p = probes[id.x];
    let m = materials[u32(p._a)];
    let basis = onb(p.normal);
    let nc = 512u;
    let np = 1024u;
    var sum = 0.0;
    for (var i = 0u; i < nc; i = i + 1u) {
        let c = (f32(i) + 0.5) / f32(nc);
        let s = sqrt(1.0 - c * c);
        for (var j = 0u; j < np; j = j + 1u) {
            let phi = 2.0 * PT_PI * (f32(j) + 0.5) / f32(np);
            let wi = basis * vec3<f32>(s * cos(phi), s * sin(phi), c);
            sum = sum + bsdf_pdf(m, p.normal, p.point, wi);
        }
    }
    accum[id.x] = vec4<f32>(sum * (2.0 * PT_PI) / f32(nc * np), 0.0, 0.0, 0.0);
}

// Test entry: the angle (radians) from the mirror direction at which bsdf_pdf falls to half its
// value there, scanning toward the tangent and toward the bitangent of the material's frame;
// accum[i] = (along tangent, along bitangent, pdf at the mirror direction, 0). Probe wo = point
// must be the normal for a symmetric scan.
@compute @workgroup_size(64)
fn bsdf_width_test_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.probe_count) {
        return;
    }
    let p = probes[id.x];
    let m = materials[u32(p._a)];
    let n = p.normal;
    let t = pt_tangent(n, m.aniso_rot);
    let b = cross(n, t);
    let mirror = normalize(2.0 * dot(n, p.point) * n - p.point);
    let peak = bsdf_pdf(m, n, p.point, mirror);
    var width = vec2<f32>(0.0);
    for (var k = 0u; k < 2u; k = k + 1u) {
        let axis = select(t, b, k == 1u);
        var found = false;
        for (var i = 1u; i < 3000u && !found; i = i + 1u) {
            let theta = 0.0005 * f32(i);
            let wi = normalize(mirror * cos(theta) + axis * sin(theta));
            if (bsdf_pdf(m, n, p.point, wi) < 0.5 * peak) {
                width[k] = theta;
                found = true;
            }
        }
    }
    accum[id.x] = vec4<f32>(width, peak, 0.0);
}
