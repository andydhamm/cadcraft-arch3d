// Path tracer (phase 22): edge rounding. pt_round_edge bends the shading normal near an edge of
// triangle `tri` at world point p toward the neighbouring face, so a sharp edge catches a highlight.
// The stub returns n. Called in path_radiance right after pt_shading_normal.

fn pt_round_edge(tri: u32, n: vec3<f32>, p: vec3<f32>) -> vec3<f32> {
    return n;
}
