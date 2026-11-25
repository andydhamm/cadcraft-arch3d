// Path tracer (phase 22): the sun as a disc of finite angular size (PHOTO_SUN_DISC,
// frame.photo_f.y = cos of its half angle). sun_sample_at replaces sun_sample() in direct_light and
// irradiance_main and may draw random numbers from `r`; sun_disc_seen adds the disc's radiance to a
// ray that escapes (depth, mode and prev_pdf as env_escape's).
//
// The disc is the sun's own light: frame.sun_rgb (lux) spread over the solid angle, so a surface
// facing the sun still gets that lux. It is separate from the sky and the environment map exactly as
// the point sun was: the gradient sky has no sun in it and an environment is lit and seen as it
// always was, the disc only replaces the point (delta) sun, so nothing is counted twice. The camera
// does not see the disc itself (depth 0 returns zero; the backdrop shows the sun as before).

fn sun_disc_on() -> bool {
    return (frame.flags & FRAME_SUN) != 0u && (frame.photo.x & PHOTO_SUN_DISC) != 0u && frame.photo_f.y > 0.0;
}

// The solid angle of the disc (sr).
fn sun_disc_solid_angle() -> f32 {
    return 2.0 * PT_PI * max(1.0 - frame.photo_f.y, 1.0e-9);
}

// The radiance of the disc (nits): the sun's lux divided by the projected solid angle
// (the integral of cos over the disc), so the total illuminance equals the point sun's
// for any disc size.
fn sun_disc_radiance() -> vec3<f32> {
    let sin2 = max(1.0 - frame.photo_f.y * frame.photo_f.y, 1.0e-9);
    return frame.sun_rgb.xyz / (PT_PI * sin2);
}

// The sun toward a direction uniform in the cone around frame.sun_dir. li = the disc's radiance
// (the illuminance equals the point sun's for any disc size), delta 0 with that pdf so a
// BSDF-sampled ray that reaches the disc can be weighted against it. Without the disc:
// sun_sample(), no random number.
fn sun_sample_at(r: ptr<function, Rng>) -> LightSample {
    if (!sun_disc_on()) {
        return sun_sample();
    }
    let omega = sun_disc_solid_angle();
    let u1 = rand(r);
    let u2 = rand(r);
    let cos_t = 1.0 - u1 * (1.0 - frame.photo_f.y);
    let sin_t = sqrt(max(0.0, 1.0 - cos_t * cos_t));
    let phi = 2.0 * PT_PI * u2;
    var s: LightSample;
    s.dir = normalize(onb(normalize(frame.sun_dir.xyz)) * vec3<f32>(sin_t * cos(phi), sin_t * sin(phi), cos_t));
    s.dist = PT_INF;
    s.li = frame.sun_rgb.xyz;
    s.pdf = 1.0 / omega;
    s.delta = 0u;
    return s;
}

fn sun_disc_seen(d: vec3<f32>, depth: u32, mode: u32, prev_pdf: f32) -> vec3<f32> {
    if (!sun_disc_on() || depth == 0u || mode == AREA_SKIP) {
        return vec3<f32>(0.0);
    }
    if (dot(normalize(d), normalize(frame.sun_dir.xyz)) < frame.photo_f.y) {
        return vec3<f32>(0.0);
    }
    let omega = sun_disc_solid_angle();
    var w = 1.0;
    if (mode == AREA_MIS) {
        w = power_heuristic(prev_pdf, 1.0 / omega);
    }
    return sun_disc_radiance() * w;
}
