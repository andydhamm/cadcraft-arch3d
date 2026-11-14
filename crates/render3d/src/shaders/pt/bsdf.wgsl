// Path tracer: materials (p11-r3d-pt-bsdf).
//
// Opaque materials are a dielectric/metal blend:
//   diffuse  = Lambert, albedo * (1 - metallic) * (1 - F(n.wo)), the (1 - F) being the energy the
//              specular lobe takes (a white dielectric then never returns more than it receives);
//   specular = GGX (Trowbridge-Reitz), alpha = max(roughness^2, 1e-4), F0 = mix(dielectric F0,
//              albedo, metallic), Schlick Fresnel, Smith height-correlated G. The dielectric F0
//              is 0.04 (what IOR 1.5 gives, and what an unset IOR of 0 means); a material whose
//              IOR is 1 or below has no dielectric reflection, so with metallic 0 it is a pure
//              Lambert surface (the furnace's "diffuse" material).
// bsdf_sample picks the specular lobe with probability p_spec = mix(clamp(lum(F), 0.1, 0.9), 1,
// metallic) (0 when there is no specular lobe), samples it by visible normals (Heitz 2018) and
// the diffuse lobe by cosine; the pdf is the mixture, so weight = bsdf_eval / bsdf_pdf.
// Roughness below 0.02 makes the specular lobe a perfect mirror: a delta (specular = 1), weight
// F / p_spec (F with p_spec = 1 for a metal), pdf 1, and bsdf_eval / bsdf_pdf carry only the
// diffuse lobe.
// MAT_GLASS is a smooth dielectric (specular = 1, both events deltas).
// A sample that falls below the surface (or an unusable wo) has weight 0 and pdf 0: the path ends.

// A sampled direction: weight = f · |cos| / pdf; specular = 1 for a delta lobe (mirror, glass),
// which takes no light sample and no MIS.
struct BsdfSample { dir: vec3<f32>, weight: vec3<f32>, pdf: f32, specular: u32 };

const BSDF_MIRROR_ROUGHNESS: f32 = 0.02;
const BSDF_MIN_ALPHA: f32 = 1.0e-4;
const BSDF_DIELECTRIC_F0: f32 = 0.04;

// The lobes of an opaque material seen from a direction with cosine nov to the normal.
struct BsdfLobes {
    // Lambert reflectance after subtracting the directional albedo the rough GGX lobe keeps for
    // the current view angle; it still includes the full (1 - E) from the specular lobe's actual
    // albedo rather than the Schlick normal reflectance at grazing angles.
    diffuse: vec3<f32>,
    f0: vec3<f32>,
    // Schlick Fresnel at the normal (the mirror's reflectance).
    fresnel: vec3<f32>,
    alpha: f32,
    // Probability of sampling the specular lobe; 0 = no specular lobe.
    p_spec: f32,
    // 1 when the specular lobe is a perfect mirror.
    mirror: u32,
};

fn bsdf_luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn bsdf_schlick(f0: vec3<f32>, c: f32) -> vec3<f32> {
    let k1 = 1.0 - clamp(c, 0.0, 1.0);
    let k2 = k1 * k1;
    return f0 + (vec3<f32>(1.0) - f0) * (k2 * k2 * k1);
}

fn bsdf_spec_albedo(f0: vec3<f32>, roughness: f32, nov: f32) -> vec3<f32> {
    let rough = clamp(roughness, 0.0, 1.0);
    let nov_clamped = clamp(nov, 0.0, 1.0);
    let x = nov_clamped * f32(BSDF_E_N - 1u);
    let y = rough * f32(BSDF_E_N - 1u);
    let x0 = min(u32(floor(x)), BSDF_E_N - 2u);
    let y0 = min(u32(floor(y)), BSDF_E_N - 2u);
    let x1 = min(x0 + 1u, BSDF_E_N - 1u);
    let y1 = min(y0 + 1u, BSDF_E_N - 1u);
    let tx = x - f32(x0);
    let ty = y - f32(y0);
    var a0: array<f32, 4> = array<f32, 4>(
        bsdf_e_a[y0 * BSDF_E_N + x0],
        bsdf_e_a[y0 * BSDF_E_N + x1],
        bsdf_e_a[y1 * BSDF_E_N + x0],
        bsdf_e_a[y1 * BSDF_E_N + x1],
    );
    var b0: array<f32, 4> = array<f32, 4>(
        bsdf_e_b[y0 * BSDF_E_N + x0],
        bsdf_e_b[y0 * BSDF_E_N + x1],
        bsdf_e_b[y1 * BSDF_E_N + x0],
        bsdf_e_b[y1 * BSDF_E_N + x1],
    );
    let a_top = a0[0] + (a0[1] - a0[0]) * tx;
    let a_bot = a0[2] + (a0[3] - a0[2]) * tx;
    let b_top = b0[0] + (b0[1] - b0[0]) * tx;
    let b_bot = b0[2] + (b0[3] - b0[2]) * tx;
    let a = a_top + (a_bot - a_top) * ty;
    let b = b_top + (b_bot - b_top) * ty;
    return f0 * a + (vec3<f32>(1.0) - f0) * b;
}

fn bsdf_lobes(m: Material, nov: f32) -> BsdfLobes {
    let metal = clamp(m.metallic, 0.0, 1.0);
    let rough = clamp(m.roughness, 0.0, 1.0);
    var f0_dielectric = BSDF_DIELECTRIC_F0;
    if (m.ior > 0.0) {
        let r = (m.ior - 1.0) / (m.ior + 1.0);
        f0_dielectric = select(0.0, r * r, m.ior > 1.0);
    }
    let has_spec = metal > 0.0 || f0_dielectric > 0.0;
    var l: BsdfLobes;
    l.f0 = mix(vec3<f32>(f0_dielectric), m.albedo, metal);
    // No specular lobe, no Fresnel (Schlick with F0 = 0 would still reflect at grazing angles).
    l.fresnel = select(vec3<f32>(0.0), bsdf_schlick(l.f0, nov), has_spec);
    l.alpha = max(rough * rough, BSDF_MIN_ALPHA);
    l.mirror = select(0u, 1u, rough < BSDF_MIRROR_ROUGHNESS);
    l.p_spec = 0.0;
    if (has_spec && l.mirror == 0u) {
        let e = bsdf_spec_albedo(l.f0, rough, nov);
        l.diffuse = m.albedo * (1.0 - metal) * (vec3<f32>(1.0) - e);
        l.p_spec = mix(clamp(bsdf_luma(e), 0.1, 0.9), 1.0, metal);
    } else {
        l.diffuse = m.albedo * (1.0 - metal) * (vec3<f32>(1.0) - l.fresnel);
        if (has_spec) {
            l.p_spec = mix(clamp(bsdf_luma(l.fresnel), 0.1, 0.9), 1.0, metal);
        }
    }
    return l;
}

// GGX normal distribution at cos(n, h) = noh.
fn ggx_d(noh: f32, a: f32) -> f32 {
    let a2 = a * a;
    let n2 = noh * noh;
    // 1 + n2 (a2 - 1), written without the cancellation at small alpha.
    let d = max(1.0 - n2, 0.0) + n2 * a2;
    return a2 / (PT_PI * d * d);
}

// Smith's Lambda at cosine c to the normal.
fn ggx_lambda(c: f32, a: f32) -> f32 {
    let c2 = max(c * c, 1.0e-8);
    return 0.5 * (sqrt(1.0 + a * a * (1.0 - c2) / c2) - 1.0);
}

// A visible GGX normal in the frame whose z is the surface normal, seen from v (z > 0).
fn ggx_sample_vndf(v: vec3<f32>, a: f32, u1: f32, u2: f32) -> vec3<f32> {
    let vh = normalize(vec3<f32>(a * v.x, a * v.y, v.z));
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
    return normalize(vec3<f32>(a * nh.x, a * nh.y, max(nh.z, 1.0e-6)));
}

// The specular lobe's f · cos for (wo, wi), both above the surface.
fn ggx_eval(l: BsdfLobes, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>, nov: f32, nol: f32) -> vec3<f32> {
    let h = normalize(wo + wi);
    let noh = dot(n, h);
    let voh = max(dot(wo, h), 0.0);
    let g2 = 1.0 / (1.0 + ggx_lambda(nov, l.alpha) + ggx_lambda(nol, l.alpha));
    let f = bsdf_schlick(l.f0, voh);
    return f * (ggx_d(noh, l.alpha) * g2 / (4.0 * nov));
}

// The pdf of the visible-normal sampling for wi.
fn ggx_pdf(l: BsdfLobes, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>, nov: f32) -> f32 {
    let h = normalize(wo + wi);
    let g1 = 1.0 / (1.0 + ggx_lambda(nov, l.alpha));
    return g1 * ggx_d(dot(n, h), l.alpha) / (4.0 * nov);
}

fn bsdf_invalid(n: vec3<f32>) -> BsdfSample {
    var s: BsdfSample;
    s.dir = n;
    s.weight = vec3<f32>(0.0);
    s.pdf = 0.0;
    s.specular = 0u;
    return s;
}

// The refractive index a glass material uses: its own when it is at least 1, else glass's 1.5.
fn bsdf_glass_ior(m: Material) -> f32 {
    return select(1.5, m.ior, m.ior >= 1.0);
}

// Glass: exact (unpolarised) Fresnel for the index; entering when dot(n, wo) > 0, else leaving
// (1/ior, n flipped). Reflect with probability F, else refract by Snell; total internal reflection
// is a reflection. Weight 1 for a reflection, albedo · transmission for a refraction (radiance is
// not scaled by 1/eta^2).
fn bsdf_sample_glass(m: Material, n: vec3<f32>, wo: vec3<f32>, u: f32) -> BsdfSample {
    let ior = bsdf_glass_ior(m);
    var nn = n;
    var eta = 1.0 / ior;
    var cosi = dot(n, wo);
    if (cosi < 0.0) {
        nn = -n;
        cosi = -cosi;
        eta = ior;
    }
    let sin2t = eta * eta * max(0.0, 1.0 - cosi * cosi);
    var fr = 1.0;
    var cost = 0.0;
    if (sin2t < 1.0) {
        cost = sqrt(1.0 - sin2t);
        let rs = (eta * cosi - cost) / (eta * cosi + cost);
        let rp = (cosi - eta * cost) / (cosi + eta * cost);
        fr = 0.5 * (rs * rs + rp * rp);
    }
    var s: BsdfSample;
    s.pdf = 1.0;
    s.specular = 1u;
    if (u < fr) {
        s.dir = normalize(2.0 * cosi * nn - wo);
        s.weight = vec3<f32>(1.0);
    } else {
        s.dir = normalize((eta * cosi - cost) * nn - eta * wo);
        s.weight = m.albedo * m.transmission;
    }
    return s;
}

// Sample an incoming direction at a surface with unit shading normal n (facing wo), the unit
// direction toward the viewer wo, and three uniform numbers u.
fn bsdf_sample(m: Material, n: vec3<f32>, wo: vec3<f32>, u: vec3<f32>) -> BsdfSample {
    if ((m.flags & MAT_GLASS) != 0u) {
        return bsdf_sample_glass(m, n, wo, u.x);
    }
    let nov = dot(n, wo);
    if (nov <= 0.0) {
        return bsdf_invalid(n);
    }
    let lobes = bsdf_lobes(m, nov);
    var s: BsdfSample;
    s.specular = 0u;
    let basis = onb(n);
    var spec_h = false;
    var spec_voh = 0.0;
    if (u.x < lobes.p_spec) {
        if (lobes.mirror != 0u) {
            s.dir = normalize(2.0 * nov * n - wo);
            s.weight = lobes.fresnel / lobes.p_spec;
            s.pdf = 1.0;
            s.specular = 1u;
            return s;
        }
        let h = basis * ggx_sample_vndf(transpose(basis) * wo, lobes.alpha, u.y, u.z);
        s.dir = normalize(reflect(-wo, h));
        spec_h = true;
        spec_voh = max(dot(wo, h), 0.0);
    } else {
        // Cosine-weighted hemisphere: pdf = cos/π.
        let r = sqrt(u.y);
        let phi = 2.0 * PT_PI * u.z;
        let local = vec3<f32>(r * cos(phi), r * sin(phi), sqrt(max(0.0, 1.0 - u.y)));
        s.dir = normalize(basis * local);
    }
    let nol = dot(n, s.dir);
    if (nol <= 0.0) {
        return bsdf_invalid(n);
    }
    if (lobes.p_spec <= 0.0) {
        // Pure Lambert: weight = albedo.
        s.weight = lobes.diffuse;
        s.pdf = nol / PT_PI;
        return s;
    }
    s.pdf = bsdf_pdf(m, n, wo, s.dir);
    if (s.pdf <= 0.0) {
        return bsdf_invalid(n);
    }
    if (lobes.p_spec >= 1.0 && spec_h) {
        // Pure specular (a metal): the visible-normal weight F · G2 / G1 in closed form.
        let lo = ggx_lambda(nov, lobes.alpha);
        let li = ggx_lambda(nol, lobes.alpha);
        s.weight = bsdf_schlick(lobes.f0, spec_voh) * ((1.0 + lo) / (1.0 + lo + li));
        return s;
    }
    s.weight = bsdf_eval(m, n, wo, s.dir) / s.pdf;
    return s;
}

// f · cos for the pair (wo, wi) at n (0 below the surface). A delta lobe (mirror, glass) is not
// in it.
fn bsdf_eval(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> vec3<f32> {
    if ((m.flags & MAT_GLASS) != 0u) {
        return vec3<f32>(0.0);
    }
    let nov = dot(n, wo);
    let nol = dot(n, wi);
    if (nov <= 0.0 || nol <= 0.0) {
        return vec3<f32>(0.0);
    }
    let lobes = bsdf_lobes(m, nov);
    var f = lobes.diffuse * (nol / PT_PI);
    if (lobes.p_spec > 0.0 && lobes.mirror == 0u) {
        f = f + ggx_eval(lobes, n, wo, wi, nov, nol);
    }
    return f;
}

// The pdf bsdf_sample gives wi (the mixture of the lobes; a delta lobe has none).
fn bsdf_pdf(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> f32 {
    if ((m.flags & MAT_GLASS) != 0u) {
        return 0.0;
    }
    let nov = dot(n, wo);
    let nol = dot(n, wi);
    if (nov <= 0.0 || nol <= 0.0) {
        return 0.0;
    }
    let lobes = bsdf_lobes(m, nov);
    var pdf = (1.0 - lobes.p_spec) * nol / PT_PI;
    if (lobes.p_spec > 0.0 && lobes.mirror == 0u) {
        pdf = pdf + lobes.p_spec * ggx_pdf(lobes, n, wo, wi, nov);
    }
    return pdf;
}

// Test entry: for probe i, material = materials[u32(probes[i]._a)], n = probes[i].normal,
// wo = probes[i].point (unit). Averages u32(probes[i]._b) samples (rng_new(i, sample, frame.seed))
// and writes accum[i] = (mean weight, fraction of the non-specular samples whose
// bsdf_eval / bsdf_pdf matches the weight within 1e-3; 1 when there are none).
@compute @workgroup_size(64)
fn bsdf_test_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.probe_count) {
        return;
    }
    let i = id.x;
    let p = probes[i];
    let m = materials[u32(p._a)];
    let count = max(u32(p._b), 1u);
    var sum = vec3<f32>(0.0);
    var matched = 0.0;
    var counted = 0.0;
    for (var k = 0u; k < count; k = k + 1u) {
        var rng = rng_new(i, k, frame.seed);
        let u0 = rand(&rng);
        let u1 = rand(&rng);
        let u2 = rand(&rng);
        let smp = bsdf_sample(m, p.normal, p.point, vec3<f32>(u0, u1, u2));
        sum = sum + smp.weight;
        if (smp.specular == 0u) {
            counted = counted + 1.0;
            var expect = vec3<f32>(0.0);
            if (smp.pdf > 0.0) {
                expect = bsdf_eval(m, p.normal, p.point, smp.dir) / bsdf_pdf(m, p.normal, p.point, smp.dir);
            }
            let tol = 1.0e-3 * max(1.0, max(smp.weight.x, max(smp.weight.y, smp.weight.z)));
            let d = abs(expect - smp.weight);
            if (max(d.x, max(d.y, d.z)) <= tol) {
                matched = matched + 1.0;
            }
        }
    }
    let frac = select(1.0, matched / max(counted, 1.0), counted > 0.0);
    accum[i] = vec4<f32>(sum / f32(count), frac);
}

// Test entry: one sample per probe with u = ((i + 0.5) / probe_count, 0.5, 0.5) (a stratified
// choice of lobe), same inputs as bsdf_test_main; accum[i] = (dir, specular).
@compute @workgroup_size(64)
fn bsdf_dir_test_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.probe_count) {
        return;
    }
    let i = id.x;
    let p = probes[i];
    let m = materials[u32(p._a)];
    let u = vec3<f32>((f32(i) + 0.5) / f32(frame.probe_count), 0.5, 0.5);
    let smp = bsdf_sample(m, p.normal, p.point, u);
    accum[i] = vec4<f32>(smp.dir, f32(smp.specular));
}

// Test entry: accum[i] = (bsdf_eval, bsdf_pdf) toward the direction bsdf_dir_test_main samples
// (a delta lobe's are 0).
@compute @workgroup_size(64)
fn bsdf_density_test_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.probe_count) {
        return;
    }
    let i = id.x;
    let p = probes[i];
    let m = materials[u32(p._a)];
    let u = vec3<f32>((f32(i) + 0.5) / f32(frame.probe_count), 0.5, 0.5);
    let smp = bsdf_sample(m, p.normal, p.point, u);
    let f = bsdf_eval(m, p.normal, p.point, smp.dir);
    accum[i] = vec4<f32>(f, bsdf_pdf(m, p.normal, p.point, smp.dir));
}
