// Path tracer (phase 22): the sun as a disc of finite angular size (PHOTO_SUN_DISC,
// frame.photo_f.y = cos of its half angle). sun_sample_at replaces sun_sample() in direct_light and
// irradiance_main and may draw random numbers from `r`; sun_disc_seen adds the disc's radiance to a
// ray that escapes (depth, mode and prev_pdf as env_escape's). The stubs are today's point sun and
// nothing seen.

fn sun_sample_at(r: ptr<function, Rng>) -> LightSample {
    return sun_sample();
}

fn sun_disc_seen(d: vec3<f32>, depth: u32, mode: u32, prev_pdf: f32) -> vec3<f32> {
    return vec3<f32>(0.0);
}
