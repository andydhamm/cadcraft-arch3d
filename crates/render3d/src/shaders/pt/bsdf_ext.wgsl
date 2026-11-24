// Path tracer (phase 22): the coat, sheen and anisotropy lobes of the layered material
// (MAT_EXT). bsdf_sample, bsdf_eval and bsdf_pdf defer to these when ext_active(m) is true; the
// stubs are never active. Same contracts as the bsdf.wgsl functions of the same names.

fn ext_active(m: Material) -> bool {
    return false;
}

fn ext_sample(m: Material, n: vec3<f32>, wo: vec3<f32>, u: vec3<f32>) -> BsdfSample {
    var s: BsdfSample;
    s.dir = n;
    s.weight = vec3<f32>(0.0);
    s.pdf = 0.0;
    s.specular = 0u;
    return s;
}

fn ext_eval(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(0.0);
}

fn ext_pdf(m: Material, n: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>) -> f32 {
    return 0.0;
}
