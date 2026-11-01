// Path tracer: materials. Lambert diffuse is complete; the GGX microfacet (metallic, roughness:
// glossy and mirror) and glass (Fresnel, refraction) are filled by p11-r3d-pt-bsdf.

// A sampled direction: weight = f · |cos| / pdf; specular = 1 for a delta lobe (mirror, glass),
// which takes no light sample and no MIS.
struct BsdfSample { dir: vec3<f32>, weight: vec3<f32>, pdf: f32, specular: u32 };

// Sample an incoming direction at a surface with unit shading normal n (facing wo), the unit
// direction toward the viewer wo, and three uniform numbers u.
fn bsdf_sample(m: Material, n: vec3<f32>, wo: vec3<f32>, u: vec3<f32>) -> BsdfSample {
    // Cosine-weighted hemisphere: pdf = cos/π, so weight = albedo.
    let r = sqrt(u.x);
    let phi = 2.0 * PT_PI * u.y;
    let local = vec3<f32>(r * cos(phi), r * sin(phi), sqrt(max(0.0, 1.0 - u.x)));
    var s: BsdfSample;
    s.dir = normalize(onb(n) * local);
    s.weight = m.albedo;
    s.pdf = local.z / PT_PI;
    s.specular = 0u;
    return s;
}

// f · cos for the pair (wo, wi) at n (0 below the surface).
fn bsdf_eval(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> vec3<f32> {
    return m.albedo * (max(dot(n, wi), 0.0) / PT_PI);
}

// The pdf bsdf_sample gives wi.
fn bsdf_pdf(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> f32 {
    return max(dot(n, wi), 0.0) / PT_PI;
}
