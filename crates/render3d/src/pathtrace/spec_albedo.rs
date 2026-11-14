//! A conservative CPU table of the GGX specular lobe's directional albedo, used by the path
//! tracer to subtract the true lobe reflectance from the diffuse term instead of Schlick
//! at the normal (which over-removes the diffuse at grazing angles, e.g. the phase-19 check
//! 06(a2) horizon ground). Pure f64 math: no GPU, no shader, no panics.

use std::sync::OnceLock;

/// Nodes per axis of the roughness / view-cosine table.
pub const SPEC_E_N: usize = 16;

const ALPHA_MIN: f64 = 1.0e-4;
const NOV_MIN: f64 = 1.0e-3;
const PI: f64 = std::f64::consts::PI;

fn clamp_f64(v: f64, lo: f64, hi: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

fn ggx_lambda(c: f64, alpha: f64) -> f64 {
    let c = clamp_f64(c, 0.0, 1.0);
    let c2 = (c * c).max(1.0e-8);
    0.5 * ((1.0 + alpha * alpha * (1.0 - c2) / c2).sqrt() - 1.0)
}

fn ggx_sample_vndf(v: [f64; 3], alpha: f64, u1: f64, u2: f64) -> [f64; 3] {
    let vlen = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let v = if vlen <= 0.0 { [0.0, 0.0, 1.0] } else { [v[0] / vlen, v[1] / vlen, v[2] / vlen] };
    let vh = {
        let x = alpha * v[0];
        let y = alpha * v[1];
        let z = v[2];
        let len = (x * x + y * y + z * z).sqrt();
        if len <= 0.0 { [0.0, 0.0, 1.0] } else { [x / len, y / len, z / len] }
    };
    let lensq = vh[0] * vh[0] + vh[1] * vh[1];
    let t1 = if lensq > 1.0e-12 { [-vh[1], vh[0], 0.0] } else { [1.0, 0.0, 0.0] };
    let len_t1 = (t1[0] * t1[0] + t1[1] * t1[1] + t1[2] * t1[2]).sqrt();
    let t1 = if len_t1 > 0.0 { [t1[0] / len_t1, t1[1] / len_t1, t1[2] / len_t1] } else { [1.0, 0.0, 0.0] };
    let t2 = [vh[1] * t1[2] - vh[2] * t1[1], vh[2] * t1[0] - vh[0] * t1[2], vh[0] * t1[1] - vh[1] * t1[0]];
    let r = u1.sqrt();
    let phi = 2.0 * PI * u2;
    let p1 = r * phi.cos();
    let p2 = r * phi.sin();
    let s = 0.5 * (1.0 + vh[2]);
    let q2 = (1.0 - s) * (1.0 - p1 * p1).max(0.0).sqrt() + s * p2;
    let nh = [
        p1 * t1[0] + q2 * t2[0] + (1.0 - p1 * p1 - q2 * q2).max(0.0).sqrt() * vh[0],
        p1 * t1[1] + q2 * t2[1] + (1.0 - p1 * p1 - q2 * q2).max(0.0).sqrt() * vh[1],
        p1 * t1[2] + q2 * t2[2] + (1.0 - p1 * p1 - q2 * q2).max(0.0).sqrt() * vh[2],
    ];
    let len_nh = (nh[0] * nh[0] + nh[1] * nh[1] + nh[2] * nh[2]).sqrt();
    if len_nh <= 0.0 {
        [0.0, 0.0, 1.0]
    } else {
        let h = [alpha * nh[0] / len_nh, alpha * nh[1] / len_nh, (nh[2] / len_nh).max(1.0e-6)];
        let hlen = (h[0] * h[0] + h[1] * h[1] + h[2] * h[2]).sqrt();
        if hlen <= 0.0 { [0.0, 0.0, 1.0] } else { [h[0] / hlen, h[1] / hlen, h[2] / hlen] }
    }
}

/// Returns the GGX lobe's directional albedo coefficients `(a, b)` for a single-scatter lobe
/// matching the WGSL `bsdf.wgsl` implementation: `E = f0 * a + (1 - f0) * b`.
pub fn spec_albedo_terms(roughness: f64, nov: f64) -> (f64, f64) {
    let roughness = clamp_f64(roughness, 0.0, 1.0);
    let nov = clamp_f64(nov, NOV_MIN, 1.0);
    let alpha = (roughness * roughness).max(ALPHA_MIN);
    let v = [((1.0 - nov * nov).max(0.0)).sqrt(), 0.0, nov];
    let mut a = 0.0;
    let mut b = 0.0;
    let inv_n = 1.0 / 32.0;
    for i in 0..32 {
        let u1 = (i as f64 + 0.5) * inv_n;
        for j in 0..32 {
            let u2 = (j as f64 + 0.5) * inv_n;
            let h = ggx_sample_vndf(v, alpha, u1, u2);
            let voh = (v[0] * h[0] + v[1] * h[1] + v[2] * h[2]).max(0.0);
            if voh <= 0.0 {
                continue;
            }
            let wi = [2.0 * voh * h[0] - v[0], 2.0 * voh * h[1] - v[1], 2.0 * voh * h[2] - v[2]];
            if wi[2] <= 0.0 {
                continue;
            }
            let lambda_v = ggx_lambda(nov, alpha);
            let lambda_wi = ggx_lambda(wi[2], alpha);
            let w = (1.0 + lambda_v) / (1.0 + lambda_v + lambda_wi);
            a += w;
            b += w * (1.0 - voh).powi(5);
        }
    }
    (a / 1024.0, b / 1024.0)
}

/// A conservative table of the GGX lobe's directional-albedo coefficients.
pub struct SpecAlbedoTable {
    pub a: Vec<f32>,
    pub b: Vec<f32>,
}

fn table_cell_index(row: usize, col: usize) -> usize {
    row * SPEC_E_N + col
}

fn read_table(v: &[f32], idx: usize) -> f32 {
    v.get(idx).copied().unwrap_or(0.0)
}

/// CPU lookup for the tiled GGX directional albedo coefficients.
pub fn spec_albedo_table() -> &'static SpecAlbedoTable {
    static TABLE: OnceLock<SpecAlbedoTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = SpecAlbedoTable { a: vec![0.0; SPEC_E_N * SPEC_E_N], b: vec![0.0; SPEC_E_N * SPEC_E_N] };
        let h = 1.0 / (SPEC_E_N - 1) as f64;
        for ri in 0..SPEC_E_N {
            let roughness = ri as f64 / (SPEC_E_N - 1) as f64;
            for ci in 0..SPEC_E_N {
                let nov = ci as f64 / (SPEC_E_N - 1) as f64;
                let mut a_max: f64 = 0.0;
                let mut b_max: f64 = 0.0;
                for dr in [-1i32, 0, 1] {
                    let r = clamp_f64(roughness + dr as f64 * h * 0.5, 0.0, 1.0);
                    for dc in [-1i32, 0, 1] {
                        let n = clamp_f64(nov + dc as f64 * h * 0.5, 0.0, 1.0);
                        let (a, b) = spec_albedo_terms(r, n);
                        a_max = a_max.max(a);
                        b_max = b_max.max(b);
                    }
                }
                let idx = table_cell_index(ri, ci);
                table.a[idx] = a_max.min(1.0) as f32;
                table.b[idx] = b_max.min(1.0) as f32;
            }
        }
        table
    })
}

/// Bilinear interpolation of the conservative GGX directional-albedo tables.
pub fn spec_albedo_lookup(t: &SpecAlbedoTable, roughness: f32, nov: f32) -> (f32, f32) {
    let roughness = roughness.clamp(0.0, 1.0);
    let nov = nov.clamp(0.0, 1.0);
    let x = nov * (SPEC_E_N as f32 - 1.0);
    let y = roughness * (SPEC_E_N as f32 - 1.0);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x0 = x0.min(SPEC_E_N - 2);
    let y0 = y0.min(SPEC_E_N - 2);
    let x1 = (x0 + 1).min(SPEC_E_N - 1);
    let y1 = (y0 + 1).min(SPEC_E_N - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let a00 = read_table(&t.a, y0 * SPEC_E_N + x0);
    let a10 = read_table(&t.a, y0 * SPEC_E_N + x1);
    let a01 = read_table(&t.a, y1 * SPEC_E_N + x0);
    let a11 = read_table(&t.a, y1 * SPEC_E_N + x1);
    let b00 = read_table(&t.b, y0 * SPEC_E_N + x0);
    let b10 = read_table(&t.b, y0 * SPEC_E_N + x1);
    let b01 = read_table(&t.b, y1 * SPEC_E_N + x0);
    let b11 = read_table(&t.b, y1 * SPEC_E_N + x1);
    let a_top = a00 + (a10 - a00) * tx;
    let a_bot = a01 + (a11 - a01) * tx;
    let b_top = b00 + (b10 - b00) * tx;
    let b_bot = b01 + (b11 - b01) * tx;
    (a_top + (a_bot - a_top) * ty, b_top + (b_bot - b_top) * ty)
}

/// WGSL text for the directional-albedo tables used by the path tracer.
pub fn spec_albedo_wgsl() -> String {
    let table = spec_albedo_table();
    let mut a_vals = String::new();
    for (idx, v) in table.a.iter().enumerate() {
        if idx > 0 {
            a_vals.push_str(", ");
        }
        a_vals.push_str(&format!("{:.6}", v));
    }
    let mut b_vals = String::new();
    for (idx, v) in table.b.iter().enumerate() {
        if idx > 0 {
            b_vals.push_str(", ");
        }
        b_vals.push_str(&format!("{:.6}", v));
    }
    let n = SPEC_E_N;
    let k = SPEC_E_N * SPEC_E_N;
    format!(
        "const BSDF_E_N: u32 = {n}u;\nconst BSDF_E_K: u32 = {k}u;\nconst bsdf_e_a: array<f32, {k}> = array<f32, {k}>({a_vals});\nconst bsdf_e_b: array<f32, {k}> = array<f32, {k}>({b_vals});\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn direct_ggx_albedo(f0: f64, alpha: f64, theta_o: f64) -> f64 {
        let (so, co) = theta_o.sin_cos();
        let a2 = alpha * alpha;
        let lambda = |c: f64| 0.5 * ((1.0 + a2 * (1.0 - c * c) / (c * c)).sqrt() - 1.0);
        let (nt, np) = (600usize, 600usize);
        let dt = 0.5 * PI / nt as f64;
        let dp = 2.0 * PI / np as f64;
        let mut sum = 0.0;
        for it in 0..nt {
            let t = (it as f64 + 0.5) * dt;
            let (st, ct) = t.sin_cos();
            for ip in 0..np {
                let p = (ip as f64 + 0.5) * dp;
                let wi = [st * p.cos(), st * p.sin(), ct];
                let mut h = [so + wi[0], wi[1], co + wi[2]];
                let len = (h[0] * h[0] + h[1] * h[1] + h[2] * h[2]).sqrt();
                h = [h[0] / len, h[1] / len, h[2] / len];
                let noh = h[2];
                let voh = so * h[0] + co * h[2];
                let d = noh * noh * (a2 - 1.0) + 1.0;
                let d = a2 / (PI * d * d);
                let g2 = 1.0 / (1.0 + lambda(co) + lambda(ct));
                let k = (1.0 - voh).powi(5);
                let w = d * g2 / (4.0 * co) * st * dt * dp;
                sum += (f0 + (1.0 - f0) * k) * w;
            }
        }
        sum
    }

    fn count_float_literals(s: &str) -> usize {
        s.split(|c: char| !c.is_ascii_digit() && c != '.' && c != '-' && c != '+' && c != 'e' && c != 'E')
            .filter(|tok| tok.contains('.') && tok.parse::<f32>().is_ok())
            .count()
    }

    #[test]
    fn spec_albedo_smooth_limit_is_schlick() {
        let (a, b) = spec_albedo_terms(0.05, 0.5);
        assert!((a - 1.0).abs() <= 2.0e-3, "a={a}");
        assert!((b - 0.03125).abs() <= 2.0e-3, "b={b}");
    }

    #[test]
    fn spec_albedo_terms_match_a_direct_quadrature() {
        let cases: [(f64, f64, f64); 4] = [
            (0.5, 60.0_f64.to_radians(), 0.04),
            (0.5, 60.0_f64.to_radians(), 0.52),
            (1.0, 30.0_f64.to_radians(), 0.04),
            (1.0, 30.0_f64.to_radians(), 0.52),
        ];
        for (roughness, theta, f0) in cases {
            let alpha = (roughness * roughness).max(ALPHA_MIN);
            let (a, b) = spec_albedo_terms(roughness, theta.cos());
            let reference = direct_ggx_albedo(f0, alpha, theta);
            let value = f0 * a + (1.0 - f0) * b;
            assert!((value - reference).abs() <= 3.0e-3, "roughness={roughness} theta={theta} f0={f0}: got {value}, want {reference}");
        }
    }

    #[test]
    fn spec_albedo_table_bounds_the_terms() {
        let roughs = [0.1_f64, 0.3, 0.5, 0.7, 0.95, 1.0];
        let angles = [0.0_f64, 30.0, 60.0, 85.0, 88.6];
        let table = spec_albedo_table();
        for roughness in roughs {
            let roughness = roughness as f32;
            for deg in angles {
                let nov = deg.to_radians().cos() as f32;
                let (terms_a, terms_b) = spec_albedo_terms(roughness as f64, nov as f64);
                let (lookup_a, lookup_b) = spec_albedo_lookup(table, roughness, nov);
                assert!(lookup_a >= terms_a as f32 - 1.0e-4, "rough={roughness} nov={nov}: a={lookup_a} < {terms_a}");
                assert!(lookup_b >= terms_b as f32 - 1.0e-4, "rough={roughness} nov={nov}: b={lookup_b} < {terms_b}");
                assert!(lookup_a <= 1.0, "rough={roughness} nov={nov}: a={lookup_a}");
                assert!(lookup_b <= 1.0, "rough={roughness} nov={nov}: b={lookup_b}");
            }
        }
    }

    #[test]
    fn spec_albedo_rough_ground_is_flat_near_grazing() {
        let roughness = 0.95f32;
        let f0 = 0.04f32;
        let table = spec_albedo_table();
        let nov_1 = 0.024f32;
        let nov_2 = 0.096f32;
        let e1 = f0 * spec_albedo_lookup(table, roughness, nov_1).0 + (1.0 - f0) * spec_albedo_lookup(table, roughness, nov_1).1;
        let e2 = f0 * spec_albedo_lookup(table, roughness, nov_2).0 + (1.0 - f0) * spec_albedo_lookup(table, roughness, nov_2).1;
        let ratio = (1.0 - e1) / (1.0 - e2);
        assert!((0.85..=1.02).contains(&ratio), "ratio={ratio}");
        assert!(1.0 - e1 > 0.5, "e1={e1}");
    }

    #[test]
    fn spec_albedo_wgsl_declares_both_tables() {
        let s1 = spec_albedo_wgsl();
        let s2 = spec_albedo_wgsl();
        assert_eq!(s1, s2);
        assert!(s1.contains("BSDF_E_N"));
        assert!(s1.contains("bsdf_e_a"));
        assert!(s1.contains("bsdf_e_b"));
        let count = count_float_literals(&s1);
        assert_eq!(count, 2 * SPEC_E_N * SPEC_E_N, "unexpected literal count: {count}");
    }

    #[test]
    fn spec_albedo_clamps_hostile_input() {
        // Out of range values are clamped to [0, 1].
        let (a_hi, b_hi) = spec_albedo_terms(1.5, 0.5);
        let (a_one, b_one) = spec_albedo_terms(1.0, 0.5);
        assert_eq!(a_hi, a_one, "roughness > 1 clamped");
        assert_eq!(b_hi, b_one, "roughness > 1 clamped");

        let (a_lo, b_lo) = spec_albedo_terms(-0.2, 0.5);
        let (a_zero, b_zero) = spec_albedo_terms(0.0, 0.5);
        assert_eq!(a_lo, a_zero, "roughness < 0 clamped");
        assert_eq!(b_lo, b_zero, "roughness < 0 clamped");

        // Non-finite values are treated as 0.0 (clamp_f64), never panic.
        for (r, n) in
            [(f64::NAN, 0.5), (f64::INFINITY, 0.5), (f64::NEG_INFINITY, 0.5), (0.5, f64::NAN), (0.5, f64::INFINITY), (0.5, f64::NEG_INFINITY)]
        {
            let (a, b) = spec_albedo_terms(r, n);
            assert!(a.is_finite() && b.is_finite(), "terms({r}, {n}) not finite");
            assert!(a >= 0.0 && a <= 1.0, "a out of range for ({r}, {n})");
            assert!(b >= 0.0 && b <= 1.0, "b out of range for ({r}, {n})");
        }
    }

    #[test]
    fn spec_albedo_boundary_values() {
        // Roughness exactly 0 (perfectly smooth, alpha = ALPHA_MIN).
        let (a0, b0) = spec_albedo_terms(0.0, 0.5);
        assert!(a0.is_finite() && b0.is_finite());
        assert!(a0 >= 0.0 && a0 <= 1.0);
        assert!(b0 >= 0.0 && b0 <= 1.0);

        // Roughness exactly 1 (maximally rough).
        let (a1, b1) = spec_albedo_terms(1.0, 0.5);
        assert!(a1.is_finite() && b1.is_finite());
        assert!(a1 >= 0.0 && a1 <= 1.0);
        assert!(b1 >= 0.0 && b1 <= 1.0);

        // View angle 0 (straight on, nov = 1).
        let (a2, b2) = spec_albedo_terms(0.5, 1.0);
        assert!(a2.is_finite() && b2.is_finite());

        // View angle at the minimum (grazing, nov = NOV_MIN).
        let (a3, b3) = spec_albedo_terms(0.5, NOV_MIN);
        assert!(a3.is_finite() && b3.is_finite());

        // A: the lobe reflectance at normal incidence (nov = 1) should be close to 1 for smooth,
        // lower for rough.
        let (a_smooth, _) = spec_albedo_terms(0.05, 1.0);
        let (a_rough, _) = spec_albedo_terms(0.95, 1.0);
        assert!(a_smooth > a_rough, "smooth lobe reflects more at normal incidence");
    }

    #[test]
    fn spec_albedo_lookup_at_table_corners() {
        let table = spec_albedo_table();

        // All four corners of the bilinear grid.
        let (a00, b00) = spec_albedo_lookup(table, 0.0, 0.0);
        let (a10, b10) = spec_albedo_lookup(table, 1.0, 0.0);
        let (a01, b01) = spec_albedo_lookup(table, 0.0, 1.0);
        let (a11, b11) = spec_albedo_lookup(table, 1.0, 1.0);
        for (a, b, label) in [(a00, b00, "0,0"), (a10, b10, "1,0"), (a01, b01, "0,1"), (a11, b11, "1,1")] {
            assert!(a.is_finite() && b.is_finite(), "corner {label} not finite");
            assert!(a >= 0.0 && a <= 1.0, "a out of range at corner {label}");
            assert!(b >= 0.0 && b <= 1.0, "b out of range at corner {label}");
        }

        // Out of range clamps to the nearest edge.
        let (a_neg, b_neg) = spec_albedo_lookup(table, -0.5, -0.5);
        assert_eq!(a_neg, a00, "negative roughness clamps to 0");
        assert_eq!(b_neg, b00, "negative nov clamps to 0");

        let (a_big, b_big) = spec_albedo_lookup(table, 1.5, 1.5);
        assert_eq!(a_big, a11, "roughness > 1 clamps to 1");
        assert_eq!(b_big, b11, "nov > 1 clamps to 1");

        // Interior point: bilinear interpolation should be between the corners.
        let (a_mid, b_mid) = spec_albedo_lookup(table, 0.5, 0.5);
        assert!(a_mid.is_finite() && b_mid.is_finite());
        assert!(a_mid >= a00.min(a10.min(a01.min(a11))) - 1e-6);
        assert!(a_mid <= a00.max(a10.max(a01.max(a11))) + 1e-6);
    }
}
