// Path tracer (phase 22): the firefly clamp (PHOTO_CLAMP, frame.photo_f.z = limit in nits). Every
// term path_radiance adds at depth >= 1 goes through pt_clamp; the luminance it removes is added to
// *loss (PathResult.loss, summed in accum[n + i].z).

fn pt_clamp(c: vec3<f32>, depth: u32, loss: ptr<function, f32>) -> vec3<f32> {
    if ((frame.photo.x & PHOTO_CLAMP) == 0u) || depth == 0u {
        return c;
    }
    let limit = max(frame.photo_f.z, 0.0);
    if (!(limit > 0.0)) {
        return c;
    }
    let lum = max(dot(c, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0);
    if (!(lum > limit)) {
        return c;
    }
    let scale = limit / lum;
    *loss = *loss + (lum - limit);
    return c * scale;
}
