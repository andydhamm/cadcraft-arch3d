// Path tracer (phase 22): rough and thin transmission (MAT_TRANS). bsdf_sample, bsdf_eval and
// bsdf_pdf defer to these when trans_active(m) is true (checked before ext_active); the stubs are
// never active. The kernel treats such a material as non-glass (direct light, the shading side), so
// these functions own the whole transmission. Same contracts as the bsdf.wgsl functions of the
// same names.

fn trans_active(m: Material) -> bool {
    return false;
}

fn trans_sample(m: Material, n: vec3<f32>, wo: vec3<f32>, u: vec3<f32>) -> BsdfSample {
    var s: BsdfSample;
    s.dir = n;
    s.weight = vec3<f32>(0.0);
    s.pdf = 0.0;
    s.specular = 0u;
    return s;
}

fn trans_eval(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(0.0);
}

fn trans_pdf(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> f32 {
    return 0.0;
}
