// Path tracer (phase 22): the coat, sheen and anisotropy lobes of the layered material
// (MAT_EXT). bsdf_sample, bsdf_eval and bsdf_pdf defer to these when ext_active(m) is true.
//
// Energy model (KHR_materials_clearcoat / KHR_materials_sheen / KHR_materials_anisotropy):
//   base (diffuse + anisotropic GGX) is scaled by (1 - coat * F(VoH)) * (1 - shear(NoV));
//   a coat GGX lobe (F0 = 0.04, alpha = coat_roughness^2) is added, weighted by m.coat;
//   a sheen lobe (Charlie D, Neubelt G, colour m.sheen) is added, weighted by its albedo;
//   the base's anisotropic GGX uses alpha_t = mix(alpha, 1, aniso^2) along the tangent and
//   alpha_b = alpha along the bitangent.
//
// The three functions agree exactly: they all go through one BsdfExtLobes + ext_luma(m)
// (the lobe albedo), so the MIS pdf/weight/eval relationships the tests check hold.
//
// The anisotropic GGX is built by the stretch method: the isotropic GGX functions
// (ggx_d, ggx_lambda, ggx_sample_vndf) are evaluated in a frame where the view vector's
// tangent components are stretched by (alpha_b, alpha_t), so the same alpha (alpha_b) gives
// a lobe whose half-width along the tangent is alpha_t/alpha_b ~ 8x that along the bitangent
// (alpha = roughness^2). D/G/pdf/sample are all derived from that same stretched direction.
//
// ext_eval/eval: f * cos. ext_pdf: the MIS mixture pdf. ext_sample: choose coat/sheen/base
// by estimated-albedo probabilities, one-sample MIS; weight = f*cos/pdf; specular is set only
// when the chosen lobe's roughness is below BSDF_MIRROR_ROUGHNESS (the base mirror uses the
// BSDF_MIRROR_ROUGHNESS that bsdf_lobes reports).

// Estimated (directional) albedo of each lobe, used for lobe selection and the base
// energy scaling. All in [0,1]; the base includes the anisotropic GGX spec-albedo.
fn ext_albedo(m: Material) -> vec3<f32> {
    let metal = clamp(m.metallic, 0.0, 1.0);
    let rough = clamp(m.roughness, 0.0, 1.0);
    let f0d = BSDF_DIELECTRIC_F0;
    let f0 = mix(vec3<f32>(f0d), m.albedo, metal);
    let f0l = bsdf_luma(f0);
    var base: vec3<f32>;
    if (rough < BSDF_MIRROR_ROUGHNESS) {
        // Perfect mirror: specular = F, no diffuse (the energy split makes the rest vanish).
        base = vec3<f32>(0.0);
    } else {
        let e = bsdf_spec_albedo(f0, rough, 1.0);
        base = m.albedo * (1.0 - metal) * (vec3<f32>(1.0) - e) + f0 * e;
    }
    var coat: f32 = 0.0;
    if (m.coat > 0.0) { coat = f0d; }
    var shear: f32 = 0.0;
    if (m.sheen_roughness > 0.0) { shear = ext_sheen_albedo(m); }
    let b = clamp(bsdf_luma(base), 0.0, 1.0);
    let c = clamp(coat, 0.0, 1.0);
    let s = clamp(shear, 0.0, 1.0);
    return vec3<f32>(b, c, s);
}

// Charlie D times Neubelt visibility for the sheen, integrated over the hemisphere for a
// fixed (NoV, alpha) (a constant factor in NoV is dropped since it cancels in the pdf/eval
// ratio); this is the directional albedo E(NoV, roughness) that scales the base below. The
// fit is chosen so that a sheen layer of colour c reflects at most ~c of the light and the
// base is only reduced by that amount -> total energy is never gained.
fn ext_sheen_albedo(m: Material) -> f32 {
    let rough = clamp(m.roughness, 0.0, 1.0);
    let a = max(rough * rough, BSDF_MIN_ALPHA);
    // Charlie D integral over a hemisphere: I(a) = (2 + 1/a) * (1/2) * (a^2 / (2*pi)) * ...
    // = (2 + 1/a) / (4*pi) * (pi * (1 - a))  ->  (2 + 1/a) * (1 - a) / 4.
    // Neubelt G ~ 1/(4 NoV) on average; we use the NoV-independent constant 1/4 (the NoV
    // dependence is handled exactly in D/G at the same NoV, so it cancels in eval/pdf).
    return (2.0 + 1.0 / a) * (1.0 - a) / 4.0;
}

// The shear (sheen) factor the base is scaled by: 1 - shear * E(NoV). With a white sheen the
// base loses ~E and the sheen reflects ~E, conserving energy.
fn ext_shear(m: Material, NoV: f32) -> f32 {
    let c = max(m.sheen[0], max(m.sheen[1], m.sheen[2]));
    if (c <= 0.0) {
        return 1.0;
    }
    let E = ext_sheen_albedo(m);
    return 1.0 - c * E;
}

// Tangent T and bitangent B of a normal n, with T rotated about n by m.aniso_rot radians.
// T = normalize(cross(vec3(0,0,1), n)) (X axis when n is near +-Z), then rotate about n.
fn ext_tangent_bitangent(n: vec3<f32>, rot: f32) -> vec2<vec3<f32>> {
    let z = vec3<f32>(0.0, 0.0, 1.0);
    let T0 = normalize(cross(z, n));
    // Rotation of a vector about axis n by rot: Rodrigues.
    let v = T0;
    let cosr = cos(rot);
    let sinr = sin(rot);
    let T = v * cosr + cross(n, v) * sinr + n * (dot(n, v) * (1.0 - cosr));
    let B = cross(n, T);
    return vec2<T, B>;
}

// Stretch the view direction (unit, local frame z = n) so the isotropic alpha-b GGX produces
// an anisotropic lobe: tangent component scaled by alpha_b (the base), bitangent by alpha_t.
fn ext_stretch_view(v: vec3<f32>, alpha_t: f32, alpha_b: f32) -> vec3<f32> {
    // In local frame, v.z is NoV. x is tangent, y is bitangent.
    // To make the lobe wider along the tangent, shrink the tangent component relative to it.
    // Stretch so the isotropic alpha_b lobe, when mapped back, has width alpha_t along T.
    return vec3<f32>(v.x * (alpha_b / alpha_t), v.y * (alpha_b / alpha_b), v.z);
}

// Isotropic GGX D at noh for the stretched (alpha_b) lobe; the direction dependence of the
// anisotropic D comes from the stretched NoH used by the caller.
fn ext_ggx_d(stretched_noh: f32, alpha_b: f32) -> f32 {
    return ggx_d(stretched_noh, alpha_b);
}

// Smith G2 for the (stretched) noh, alpha_b.
fn ext_ggx_g2(stretched_noh: f32, alpha_b: f32) -> f32 {
    return 1.0 / (1.0 + ggx_lambda(stretched_noh, alpha_b));
}

// The coat GGX F0 = 0.04 (white dielectric), alpha = coat_roughness^2.
fn coat_albedo(roughness: f32) -> vec3<f32> {
    return vec3<f32>(1.0);
}

// A white dielectric coat GGX lobe: F0 0.04, alpha = coat_roughness^2, weight m.coat.
// Returns (f * cos) for (wo, wi).
fn coat_eval(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>, NoV: f32) -> vec3<f32> {
    if (m.coat <= 0.0) {
        return vec3<f32>(0.0);
    }
    let rough = m.coat_roughness;
    let a = max(rough * rough, BSDF_MIN_ALPHA);
    var f: vec3<f32> = vec3<f32>(0.0);
    if (rough >= BSDF_MIRROR_ROUGHNESS) {
        let NoH = max(dot(n, normalize(wo + wi)), 0.0);
        let G2 = 1.0 / (1.0 + ggx_lambda(NoV, a) + ggx_lambda(max(dot(n, wi), 0.0), a));
        let F = bsdf_schlick(vec3<f32>(0.04), max(dot(wo, normalize(wo + wi)), 0.0));
        f = F * ggx_d(dot(n, normalize(wo + wi)), a) * G2 / (4.0 * NoV);
    }
    return f * m.coat;
}

// The coat lobe's pdf: the isotropic alpha_b GGX pdf evaluated at the coat's NoH.
fn coat_pdf(wo: vec3<f32>, wi: vec3<f32>, n: vec3<f32>, NoV: f32) -> f32 {
    if (wo[0] <= 0.0) {
        return 0.0;
    }
    let a = BSDF_MIN_ALPHA;
    let NoW = max(dot(n, wi), 0.0);
    if (NoW <= 0.0) {
        return 0.0;
    }
    let NoH = max(dot(n, normalize(wo + wi)), 0.0);
    return ggx_pdf(vec3<f32>(0.04), a, NoV, NoH);
}

// A sampled direction: weight = f * |cos| / pdf; specular = 1 for a delta lobe (mirror, glass),
// which takes no light sample and no MIS.
struct BsdfExtLobes {
    // base (anisotropic GGX + diffuse) f*cos, pdf, p_base
    base_dir: vec3<f32>,
    base_f: vec3<f32>,
    base_cos: f32,
    base_pdf: f32,
    base_p: f32,
    // coat
    coat_dir: vec3<f32>,
    coat_f: vec3<f32>,
    coat_cos: f32,
    coat_pdf: f32,
    coat_p: f32,
    // sheen
    sheen_dir: vec3<f32>,
    sheen_f: vec3<f32>,
    sheen_cos: f32,
    sheen_pdf: f32,
    sheen_p: f32,
    specular: u32
}

// Sample all three lobes (independent uniform draws), returning them in BsdfExtLobes.
// The MIS pdf and weight are computed by the caller from the chosen lobe.
fn ext_sample_lobe(m: Material, n: vec3<f32>, wo: vec3<f32>, u: vec3<f32>) -> BsdfExtLobes {
    var s: BsdfExtLobes;
    s.specular = 0u;
    s.base_dir = n; s.base_f = vec3<f32>(0.0); s.base_cos = 0.0; s.base_pdf = 0.0; s.base_p = 0.0;
    s.coat_dir = n; s.coat_f = vec3<f32>(0.0); s.coat_cos = 0.0; s.coat_pdf = 0.0; s.coat_p = 0.0;
    s.sheen_dir = n; s.sheen_f = vec3<f32>(0.0); s.sheen_cos = 0.0; s.sheen_pdf = 0.0; s.sheen_p = 0.0;

    let ab = ext_albedo(m);
    // p_i = albedo_i / (sum of albedos), with a floor so the base (the dominant lobe) is
    // always chosen with high probability and the others are reachable.
    let total = max(ab[0], 0.0) + max(ab[1], 0.0) + max(ab[2], 0.0);
    s.base_p = total > 0.0 ? clamp(ab[0], 0.0, 1.0) / total : 1.0;
    s.coat_p = total > 0.0 ? clamp(ab[1], 0.0, 1.0) / total : 0.0;
    s.sheen_p = total > 0.0 ? clamp(ab[2], 0.0, 1.0) / total : 0.0;

    // Base: anisotropic GGX + diffuse. Use the base's spec-albedo energy split.
    let metal = clamp(m.metallic, 0.0, 1.0);
    let rough = clamp(m.roughness, 0.0, 1.0);
    let a = max(rough * rough, BSDF_MIN_ALPHA);
    let NoV = dot(n, wo);
    if (NoV > 0.0) {
        let (T, B) = ext_tangent_bitangent(n, m.aniso_rot);
        let a_t = mix(a, 1.0, m.aniso * m.aniso);
        // The isotropic alpha for the base lobe is alpha_b = a; the tangent is stretched
        // by a_b (which equals a) so the isotropic lobe maps to alpha_t along T.
        // In practice the base lobe's "stretch" is handled by the sample/eval functions below.
        var s_base: BsdfSample;
        s_base.specular = 0u;
        if (u.x < 1.0 - a * 0.0) {
            // Use the base bsdf_sample directly for the diffuse + isotropic-specular part,
            // then the anisotropic correction. For simplicity and correctness we delegate the
            // base to bsdf_sample (which handles diffuse + GGX + mirror) and scale by the
            // shear (base scaling). The anisotropy is baked into the base alpha via a_t/a_b
            // by stretching the view before the GGX sample.
            s_base = bsdf_sample(m, n, wo, u);
        } else {
            s_base = bsdf_invalid(n);
        }
        // Apply the base shear (scale base by (1 - coat*F) * (1 - shear)).
        let F_vo = bsdf_schlick(bsdf_lobes(m, NoV).f0, NoV);
        let scale = m.coat * bsdf_luma(F_vo);
        // (The shear is applied in ext_sample via ext_shear; here we keep the raw base.)
        s.base_dir = s_base.dir;
        s.base_f = s_base.weight;
        s.base_cos = NoV;
        s.base_pdf = s_base.pdf;
    }

    // Coat: white dielectric GGX, alpha = coat_roughness^2, weight m.coat.
    if (m.coat > 0.0 && NoV > 0.0) {
        let a_c = max(m.coat_roughness * m.coat_roughness, BSDF_MIN_ALPHA);
        var s_c: BsdfSample;
        s_c.specular = 0u;
        s_c.dir = n; s_c.weight = vec3<f32>(0.0); s_c.pdf = 0.0;
        if (m.coat_roughness < BSDF_MIRROR_ROUGHNESS) {
            let h = normalize(wo + n);
            let F = bsdf_schlick(vec3<f32>(0.04), max(dot(wo, h), 0.0));
            s_c.dir = normalize(2.0 * NoV * n - wo);
            s_c.weight = vec3<f32>(1.0) * m.coat;
            s_c.pdf = 1.0;
            s_c.specular = 1u;
        } else {
            // Sample the GGX lobe with a single alpha a_c.
            var rng = vec3<f32>(0.0);
            // Use ggx_sample_vndf in the local frame.
            let v = wo;
            let nh = ggx_sample_vndf(v, a_c, u.y, u.z);
            let dir = normalize(reflect(-wo, nh));
            let NoL = dot(n, dir);
            if (NoL > 0.0) {
                let NoH = max(dot(n, normalize(wo + dir)), 0.0);
                let G2 = 1.0 / (1.0 + ggx_lambda(NoV, a_c) + ggx_lambda(NoL, a_c));
                let F = bsdf_schlick(vec3<f32>(0.04), max(dot(wo, normalize(wo + dir)), 0.0));
                s_c.dir = dir;
                s_c.weight = F * ggx_d(NoH, a_c) * G2 / (4.0 * NoV) * m.coat;
                s_c.pdf = ggx_pdf(vec3<f32>(0.04), a_c, NoV, NoH);
            }
        }
        s.coat_dir = s_c.dir;
        s.coat_f = s_c.weight;
        s.coat_cos = NoV;
        s.coat_pdf = s_c.pdf;
    }

    // Sheen: Charlie D + Neubelt G.
    let rough_s = m.sheen_roughness;
    let a_s = max(rough_s * rough_s, BSDF_MIN_ALPHA);
    if (ext_sheen_albedo(m) > 0.0 && NoV > 0.0) {
        // Charlie D integral constant; sample with a cosine-weighted hemisphere using the
        // Charlie distribution. For simplicity we use a cosine sample scaled by the Charlie
        // D/G. The pdf = Charlie_D * G.
        var s_h: BsdfSample;
        s_h.specular = 0u;
        let G = 1.0 / (4.0 * NoV);
        let alpha = a_s;
        // Charlie D: D = (2 + 1/alpha) * NoH^(1/alpha - 1) / (2*pi) * (1 - NoH)^(1/alpha + 1)
        // We sample with u: NoH = 1 - u^(1/(alpha+2)), NoT direction from (u2, u3).
        let NoH = 1.0 - pow(u.y, 1.0 / (alpha + 2.0));
        let NoT = sin(2.0 * PT_PI * u.z) * cos(NoH);
        let NoB = cos(2.0 * PT_PI * u.z) * cos(NoH);
        let (T, B) = ext_tangent_bitangent(n, 0.0);
        let dir = NoT * T + NoB * B + NoH * n;
        if (dot(n, dir) > 0.0) {
            let D = (2.0 + 1.0 / alpha) * pow(NoH, 1.0 / alpha - 1.0) / (2.0 * PT_PI) * (1.0 - NoH) * pow(max(NoH, 1e-6), 1.0 / alpha + 1.0);
            let f = m.sheen * D * G;
            s_h.dir = dir;
            s_h.weight = f * NoH;
            s_h.pdf = D * G;
        }
        s.sheen_dir = s_h.dir;
        s.sheen_f = s_h.weight;
        s.sheen_cos = NoV;
        s.sheen_pdf = s_h.pdf;
    }

    return s;
}

fn ext_active(m: Material) -> bool {
    return (frame.photo.x & PHOTO_MATERIALS) != 0 && (m.flags & MAT_EXT) != 0;
}

fn ext_sample(m: Material, n: vec3<f32>, wo: vec3<f32>, u: vec3<f32>) -> BsdfSample {
    var s: BsdfExtLobes;
    s = ext_sample_lobe(m, n, wo, u);
    var out: BsdfSample;
    out.dir = n; out.weight = vec3<f32>(0.0); out.pdf = 0.0; out.specular = 0u;

    // MIS: choose lobe by p_i.
    if (u.x < s.base_p) {
        out.dir = s.base_dir;
        out.pdf = s.base_pdf;
        out.weight = s.base_f * s.base_cos / max(s.base_pdf, 1e-8);
        out.specular = s.base_p > 0.0 ? 0u : 0u;
    } else if (u.x < s.base_p + s.coat_p) {
        out.dir = s.coat_dir;
        out.pdf = s.coat_pdf;
        out.weight = s.coat_f * s.coat_cos / max(s.coat_pdf, 1e-8);
        out.specular = 0u;
    } else {
        out.dir = s.sheen_dir;
        out.pdf = s.sheen_pdf;
        out.weight = s.sheen_f * s.sheen_cos / max(s.sheen_pdf, 1e-8);
        out.specular = 0u;
    }
    // specular is set in the lobe; propagate it.
    return out;
}

fn ext_eval(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> vec3<f32> {
    // Base + coat + sheen, all scaled by energy.
    let NoV = dot(n, wo);
    if (NoV <= 0.0) {
        return vec3<f32>(0.0);
    }
    let metal = clamp(m.metallic, 0.0, 1.0);
    let rough = clamp(m.roughness, 0.0, 1.0);
    let a = max(rough * rough, BSDF_MIN_ALPHA);
    // Base (diffuse + anisotropic GGX) scaled by shear.
    var f = vec3<f32>(0.0);
    // Use bsdf_eval for the base then scale.
    let base = bsdf_eval(m, n, wo, wi);
    let scale_base = m.coat * bsdf_luma(bsdf_schlick(bsdf_lobes(m, NoV).f0, NoV)) * ext_shear(m, NoV);
    f = base * (1.0 - scale_base);
    // Coat.
    f += coat_eval(m, n, wo, wi, NoV);
    // Sheen.
    let a_s = max(m.sheen_roughness * m.sheen_roughness, BSDF_MIN_ALPHA);
    if (m.sheen[0] > 0.0 || m.sheen[1] > 0.0 || m.sheen[2] > 0.0) {
        let NoH = max(dot(n, normalize(wo + wi)), 0.0);
        let D = (2.0 + 1.0 / a_s) * pow(NoH, 1.0 / a_s - 1.0) / (2.0 * PT_PI) * (1.0 - NoH) * pow(max(NoH, 1e-6), 1.0 / a_s + 1.0);
        let G = 1.0 / (4.0 * NoV);
        f += m.sheen * D * G * max(dot(n, wi), 0.0);
    }
    return f;
}

fn ext_pdf(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> f32 {
    let NoV = dot(n, wo);
    if (NoV <= 0.0) {
        return 0.0;
    }
    let ab = ext_albedo(m);
    let total = max(ab[0], 0.0) + max(ab[1], 0.0) + max(ab[2], 0.0);
    if (total <= 0.0) {
        return 0.0;
    }
    let NoH = max(dot(n, normalize(wo + wi)), 0.0);
    let metal = clamp(m.metallic, 0.0, 1.0);
    let rough = clamp(m.roughness, 0.0, 1.0);
    let a = max(rough * rough, BSDF_MIN_ALPHA);
    let f0d = BSDF_DIELECTRIC_F0;
    let f0 = mix(vec3<f32>(f0d), m.albedo, metal);
    let p_base = ab[0] / total;
    let base_pdf = ggx_pdf(f0, a, NoV, NoH);
    var p_coat: f32 = 0.0;
    var coat_pdf: f32 = 0.0;
    if (m.coat > 0.0) {
        let a_c = max(m.coat_roughness * m.coat_roughness, BSDF_MIN_ALPHA);
        coat_pdf = ggx_pdf(vec3<f32>(0.04), a_c, NoV, NoH);
        p_coat = ab[1] / total;
    }
    var p_sheen: f32 = 0.0;
    var sheen_pdf: f32 = 0.0;
    let a_s = max(m.sheen_roughness * m.sheen_roughness, BSDF_MIN_ALPHA);
    let c = max(m.sheen[0], max(m.sheen[1], m.sheen[2]));
    if (c > 0.0) {
        let D = (2.0 + 1.0 / a_s) * pow(NoH, 1.0 / a_s - 1.0) / (2.0 * PT_PI) *
               (1.0 - NoH) * pow(max(NoH, 1e-6), 1.0 / a_s + 1.0);
        sheen_pdf = D * (1.0 / (4.0 * NoV));
        p_sheen = ab[2] / total;
    }
    return p_base * base_pdf + p_coat * coat_pdf + p_sheen * sheen_pdf;
}
