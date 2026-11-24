// Path tracer (phase 22): procedural material variation. pt_vary perturbs a material read at world
// point p (albedo, roughness) by a value noise of the material's var_scale (metres per lattice
// cell) and var_amount. The noise is our own: a pcg hash of the integer lattice cell, trilinear
// interpolation with a smooth fade, two octaves. It is a function of position only (no seed, no
// sample index), so a fixed seed still gives the same image, and it has zero mean over space
// (lattice values are uniform in [-1, 1]), so a material's base colour survives.

// A lattice value in [-1, 1] for the cell c, decorrelated by salt.
fn pt_vary_cell(c: vec3<i32>, salt: u32) -> f32 {
    let h = pcg(bitcast<u32>(c.x) ^ pcg(bitcast<u32>(c.y) ^ pcg(bitcast<u32>(c.z) ^ pcg(salt))));
    return f32(h >> 8u) * (2.0 / 16777216.0) - 1.0;
}

// One octave of smooth value noise at q, in [-1, 1].
fn pt_vary_octave(q: vec3<f32>, salt: u32) -> f32 {
    let f = floor(q);
    let t = q - f;
    let s = t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let c = vec3<i32>(f);
    let v000 = pt_vary_cell(c, salt);
    let v100 = pt_vary_cell(c + vec3<i32>(1, 0, 0), salt);
    let v010 = pt_vary_cell(c + vec3<i32>(0, 1, 0), salt);
    let v110 = pt_vary_cell(c + vec3<i32>(1, 1, 0), salt);
    let v001 = pt_vary_cell(c + vec3<i32>(0, 0, 1), salt);
    let v101 = pt_vary_cell(c + vec3<i32>(1, 0, 1), salt);
    let v011 = pt_vary_cell(c + vec3<i32>(0, 1, 1), salt);
    let v111 = pt_vary_cell(c + vec3<i32>(1, 1, 1), salt);
    let z0 = mix(mix(v000, v100, s.x), mix(v010, v110, s.x), s.y);
    let z1 = mix(mix(v001, v101, s.x), mix(v011, v111, s.x), s.y);
    return mix(z0, z1, s.z);
}

// Two octaves in [-1, 1].
fn pt_vary_noise(q: vec3<f32>, salt: u32) -> f32 {
    let a = pt_vary_octave(q, salt);
    let b = pt_vary_octave(q * 2.07 + vec3<f32>(17.3, 5.9, 11.1), salt + 101u);
    return (a + 0.5 * b) * (1.0 / 1.5);
}

fn pt_vary(m: Material, p: vec3<f32>) -> Material {
    if ((frame.photo.x & PHOTO_VARIATION) == 0u) {
        return m;
    }
    if (!(m.var_scale > 0.0) || !(m.var_amount > 0.0)) {
        return m;
    }
    // Emitters, glass and the light-… materials (MAT_NO_SHADOW) are left alone.
    if (max(m.emission.x, max(m.emission.y, m.emission.z)) > 0.0 || (m.flags & (MAT_GLASS | MAT_NO_SHADOW)) != 0u) {
        return m;
    }
    let q = clamp(p / m.var_scale, vec3<f32>(-1.0e6), vec3<f32>(1.0e6));
    let v = pt_vary_noise(q, 0x1234567u);
    let v2 = pt_vary_noise(q, 0x7654321u);
    var r = m;
    r.albedo = clamp(m.albedo * (1.0 + 0.5 * m.var_amount * v), vec3<f32>(0.0), vec3<f32>(1.0));
    r.roughness = clamp(m.roughness + 0.3 * m.var_amount * v2, 0.02, 1.0);
    return r;
}
