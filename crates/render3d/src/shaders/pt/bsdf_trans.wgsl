// Path tracer (phase 22): rough and thin transmission (MAT_TRANS). bsdf_sample, bsdf_eval and
// bsdf_pdf defer to these when trans_active(m) is true (checked before ext_active). The kernel
// treats such a material as non-glass (direct light) but passes the TRUE geometric side to the
// BSDF (`ns`), so wo below the normal means the ray is inside the solid. Same contracts as the
// bsdf.wgsl functions of the same names.
//
// Rough glass (solid): a GGX microfacet dielectric (Walter et al. 2007) with the material's ior and
// alpha = roughness^2. A visible normal is sampled (Heitz 2018); the exact dielectric Fresnel picks
// reflection or refraction through it (total internal reflection reflects). Weight = the albedo
// tint on a refraction, 1 on a reflection: Walter's G2 / G1 with G2 replaced by the viewer's G1
// (see trans_glass_lobe), so a sample that stays on its side of the surface loses nothing. Radiance
// is not scaled by 1/eta^2, as in bsdf_sample_glass, so the BTDF here keeps Walter's eta^2 and
// eval / pdf equals the weight. `transmission` < 1 mixes
// it with the opaque base: probability 1 - transmission samples the base (copies of the opaque
// bsdf.wgsl lobes, which cannot be called: bsdf_sample would dispatch back here).
// Thin sheet (MAT_THIN): two-sided; with probability `transmission` a cosine lobe on the far side
// (diffuse transmission, weight = albedo), else the base on the near side.
// Volume: a tinted solid absorbs along the inside segment, exp(-sigma d) with
// sigma = -ln(albedo) / max(thickness, 1e-3) (trans_sigma; the kernel applies it).

fn trans_active(m: Material) -> bool {
    return (frame.photo.x & PHOTO_MATERIALS) != 0u && (m.flags & MAT_TRANS) != 0u;
}

fn trans_is_thin(m: Material) -> bool {
    return (m.flags & MAT_THIN) != 0u;
}

// Absorption coefficient per metre inside a tinted solid (0 for white, thin or inactive).
fn trans_sigma(m: Material) -> vec3<f32> {
    if (!trans_active(m) || trans_is_thin(m)) {
        return vec3<f32>(0.0);
    }
    let a = clamp(m.albedo, vec3<f32>(1.0e-3), vec3<f32>(1.0));
    return -log(a) / max(m.thickness, 1.0e-3);
}

// The absorption the ray carries after leaving the vertex of material m whose true normal is
// tri_n, reached by a ray with viewer direction wo and continuing along dir: `cur` unless it
// refracted through a MAT_TRANS solid (entering sets sigma, leaving clears it).
fn trans_inside_next(m: Material, tri_n: vec3<f32>, wo: vec3<f32>, dir: vec3<f32>, cur: vec3<f32>) -> vec3<f32> {
    if (!trans_active(m) || trans_is_thin(m)) {
        return cur;
    }
    let a = dot(tri_n, wo);
    if (a * dot(tri_n, dir) >= 0.0) {
        return cur;
    }
    return select(vec3<f32>(0.0), trans_sigma(m), a > 0.0);
}

// Unpolarised dielectric reflectance at cosine c, rel = n_transmitted / n_incident.
fn trans_fresnel(c: f32, rel: f32) -> f32 {
    let e = 1.0 / rel;
    let sin2t = e * e * max(0.0, 1.0 - c * c);
    if (sin2t >= 1.0) {
        return 1.0;
    }
    let cost = sqrt(1.0 - sin2t);
    let rs = (e * c - cost) / (e * c + cost);
    let rp = (c - e * cost) / (c + e * cost);
    return 0.5 * (rs * rs + rp * rp);
}

fn trans_mix(m: Material) -> f32 {
    return clamp(m.transmission, 0.0, 1.0);
}

// ---- the opaque base (copies of the bsdf.wgsl lobes, no dispatch) ----

fn trans_base_eval(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> vec3<f32> {
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

fn trans_base_pdf(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> f32 {
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

// bsdf_sample's opaque path; n faces wo.
fn trans_base_sample(m: Material, n: vec3<f32>, wo: vec3<f32>, u: vec3<f32>) -> BsdfSample {
    let nov = dot(n, wo);
    if (nov <= 0.0) {
        return bsdf_invalid(n);
    }
    let lobes = bsdf_lobes(m, nov);
    var s: BsdfSample;
    s.specular = 0u;
    let basis = onb(n);
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
    } else {
        let r = sqrt(u.y);
        let phi = 2.0 * PT_PI * u.z;
        let local = vec3<f32>(r * cos(phi), r * sin(phi), sqrt(max(0.0, 1.0 - u.y)));
        s.dir = normalize(basis * local);
    }
    let nol = dot(n, s.dir);
    if (nol <= 0.0) {
        return bsdf_invalid(n);
    }
    s.pdf = trans_base_pdf(m, n, wo, s.dir);
    if (s.pdf <= 0.0) {
        return bsdf_invalid(n);
    }
    s.weight = trans_base_eval(m, n, wo, s.dir) / s.pdf;
    return s;
}

// ---- the rough dielectric lobe, in the frame whose z is the near-side normal ----

// (f cos, pdf) of the microfacet dielectric for v (z > 0) and l, rel = n_t / n_i, tint on a
// refraction. Reflection: F D G2 / (4 nov); refraction: Walter's BTDF times cos.
fn trans_glass_lobe(v: vec3<f32>, l: vec3<f32>, rel: f32, a: f32, tint: vec3<f32>) -> vec4<f32> {
    let nov = v.z;
    let nol = abs(l.z);
    if (nov <= 1.0e-5 || nol <= 1.0e-6) {
        return vec4<f32>(0.0);
    }
    let lv = ggx_lambda(nov, a);
    let g1 = 1.0 / (1.0 + lv);
    // Masking only (G1 of the viewer) stands in for the joint G2: the visible-normal sample then
    // weighs exactly 1 (times the tint), so the single-scatter lobe loses no energy to shadowing.
    // With G2 a closed rough-glass ball lost ~6 %, entering and leaving each taking 1-4 %.
    let g2 = g1;
    if (l.z > 0.0) {
        let hs = v + l;
        if (dot(hs, hs) <= 1.0e-12) {
            return vec4<f32>(0.0);
        }
        let h = normalize(hs);
        let voh = dot(v, h);
        if (voh <= 0.0 || h.z <= 0.0) {
            return vec4<f32>(0.0);
        }
        let f = trans_fresnel(voh, rel);
        let d = ggx_d(h.z, a);
        return vec4<f32>(vec3<f32>(f * d * g2 / (4.0 * nov)), g1 * d * f / (4.0 * nov));
    }
    var hs = v + rel * l;
    if (dot(hs, hs) <= 1.0e-12) {
        return vec4<f32>(0.0);
    }
    var h = normalize(hs);
    if (h.z < 0.0) {
        h = -h;
    }
    let voh = dot(v, h);
    let loh = dot(l, h);
    if (voh <= 0.0 || loh >= 0.0) {
        return vec4<f32>(0.0);
    }
    let den = voh + rel * loh;
    let den2 = den * den;
    if (den2 <= 1.0e-12) {
        return vec4<f32>(0.0);
    }
    let f = trans_fresnel(voh, rel);
    if (f >= 1.0) {
        return vec4<f32>(0.0);
    }
    let d = ggx_d(h.z, a);
    let k = rel * rel * (-loh) * (1.0 - f) * d / den2;
    return vec4<f32>(tint * (g2 * voh * k / nov), g1 * voh * k / nov);
}

fn trans_alpha(m: Material) -> f32 {
    let r = clamp(m.roughness, 0.0, 1.0);
    return max(r * r, BSDF_MIN_ALPHA);
}

// The near-side normal (facing wo) and n_t / n_i of the solid for the true normal n.
fn trans_near(m: Material, n: vec3<f32>, wo: vec3<f32>) -> vec3<f32> {
    return select(n, -n, dot(n, wo) < 0.0);
}

fn trans_rel(m: Material, n: vec3<f32>, wo: vec3<f32>) -> f32 {
    let ior = bsdf_glass_ior(m);
    return select(ior, 1.0 / ior, dot(n, wo) < 0.0);
}

fn trans_sample(m: Material, n: vec3<f32>, wo: vec3<f32>, u: vec3<f32>) -> BsdfSample {
    let nf = trans_near(m, n, wo);
    if (dot(nf, wo) <= 1.0e-5) {
        return bsdf_invalid(n);
    }
    let t = trans_mix(m);
    let basis = onb(nf);
    if (u.x < 1.0 - t) {
        // The opaque base on the near side.
        let ub = vec3<f32>(u.x / max(1.0 - t, 1.0e-6), u.y, u.z);
        var s = trans_base_sample(m, nf, wo, ub);
        if (s.pdf <= 0.0 || s.specular != 0u) {
            return s;
        }
        if (trans_is_thin(m)) {
            // Only the base has support on the near side: its weight stands.
            s.pdf = trans_pdf(m, n, wo, s.dir);
            return s;
        }
        s.pdf = trans_pdf(m, n, wo, s.dir);
        if (s.pdf <= 0.0) {
            return bsdf_invalid(n);
        }
        s.weight = trans_eval(m, n, wo, s.dir) / s.pdf;
        return s;
    }
    let ug = clamp((u.x - (1.0 - t)) / max(t, 1.0e-6), 0.0, 1.0);
    var s: BsdfSample;
    s.specular = 0u;
    if (trans_is_thin(m)) {
        // Diffuse transmission: a cosine lobe on the far side.
        let r = sqrt(u.y);
        let phi = 2.0 * PT_PI * u.z;
        let local = vec3<f32>(r * cos(phi), r * sin(phi), sqrt(max(0.0, 1.0 - u.y)));
        s.dir = -normalize(basis * local);
        s.weight = m.albedo;
        s.pdf = t * dot(-s.dir, nf) / PT_PI;
        if (s.pdf <= 0.0) {
            return bsdf_invalid(n);
        }
        return s;
    }
    let a = trans_alpha(m);
    let rel = trans_rel(m, n, wo);
    let v = transpose(basis) * wo;
    let h = ggx_sample_vndf(v, a, u.y, u.z);
    let voh = dot(v, h);
    if (voh <= 0.0) {
        return bsdf_invalid(n);
    }
    let f = trans_fresnel(voh, rel);
    var l: vec3<f32>;
    var tint = vec3<f32>(1.0);
    if (ug < f) {
        l = 2.0 * voh * h - v;
        // A microfacet reflection that ends below the surface (the heavy GGX tail) is folded back
        // up instead of dropped, so the lobe does not lose that energy.
        l.z = max(abs(l.z), 1.0e-3);
    } else {
        let e = 1.0 / rel;
        let cost = sqrt(max(0.0, 1.0 - e * e * (1.0 - voh * voh)));
        l = (e * voh - cost) * h - e * v;
        // Likewise a refraction that ends above the surface is folded down.
        l.z = -max(abs(l.z), 1.0e-3);
        tint = m.albedo;
    }
    l = normalize(l);
    s.dir = normalize(basis * l);
    // A folded sample is off the lobe's support: its pdf is only a floor for MIS.
    s.pdf = max(trans_pdf(m, n, wo, s.dir), 1.0e-3);
    if (t >= 1.0) {
        s.weight = tint;
    } else {
        s.weight = trans_eval(m, n, wo, s.dir) / s.pdf;
    }
    return s;
}

fn trans_eval(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> vec3<f32> {
    let nf = trans_near(m, n, wo);
    let nov = dot(nf, wo);
    if (nov <= 1.0e-5) {
        return vec3<f32>(0.0);
    }
    let t = trans_mix(m);
    let nol = dot(nf, wi);
    var f = vec3<f32>(0.0);
    if (nol > 0.0 && t < 1.0) {
        f = (1.0 - t) * trans_base_eval(m, nf, wo, wi);
    }
    if (trans_is_thin(m)) {
        if (nol < 0.0) {
            f = f + t * m.albedo * (-nol / PT_PI);
        }
        return f;
    }
    let basis = onb(nf);
    let g = trans_glass_lobe(transpose(basis) * wo, transpose(basis) * wi, trans_rel(m, n, wo), trans_alpha(m), m.albedo);
    return f + t * g.xyz;
}

fn trans_pdf(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> f32 {
    let nf = trans_near(m, n, wo);
    let nov = dot(nf, wo);
    if (nov <= 1.0e-5) {
        return 0.0;
    }
    let t = trans_mix(m);
    let nol = dot(nf, wi);
    var pdf = 0.0;
    if (nol > 0.0 && t < 1.0) {
        pdf = (1.0 - t) * trans_base_pdf(m, nf, wo, wi);
    }
    if (trans_is_thin(m)) {
        if (nol < 0.0) {
            pdf = pdf + t * (-nol) / PT_PI;
        }
        return pdf;
    }
    let basis = onb(nf);
    let g = trans_glass_lobe(transpose(basis) * wo, transpose(basis) * wi, trans_rel(m, n, wo), trans_alpha(m), m.albedo);
    return pdf + t * g.w;
}

// Test entry: for probe i, material = materials[u32(probes[i]._a)], n = probes[i].normal, the
// viewer direction wo = frame.sun_dir.xyz and wi = probes[i].point (both unit);
// accum[i] = (trans_eval rgb, trans_pdf).
@compute @workgroup_size(64)
fn trans_pdf_test_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.probe_count) {
        return;
    }
    let p = probes[id.x];
    let m = materials[u32(p._a)];
    accum[id.x] = vec4<f32>(trans_eval(m, p.normal, frame.sun_dir.xyz, p.point), trans_pdf(m, p.normal, frame.sun_dir.xyz, p.point));
}
