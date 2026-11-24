// Path tracer (phase 22): the firefly clamp (PHOTO_CLAMP, frame.photo_f.z = limit in nits). Every
// term path_radiance adds at depth >= 1 goes through pt_clamp; the luminance it removes is added to
// *loss (PathResult.loss, summed in accum[n + i].z). The stub returns c and removes nothing.

fn pt_clamp(c: vec3<f32>, depth: u32, loss: ptr<function, f32>) -> vec3<f32> {
    return c;
}
