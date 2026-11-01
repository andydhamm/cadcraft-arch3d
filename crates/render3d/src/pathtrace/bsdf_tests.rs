//! GPU tests of the path tracer's materials (`shaders/pt/bsdf.wgsl`): Lambert, GGX glossy and
//! mirror, glass. Each runs a test entry of the kernel module over probes that carry a material
//! index (`_a`), the normal, the view direction (`point`) and a sample count (`_b`) through
//! [`run_entry`], on a scene that holds only materials. They skip (with a SKIP line) without a
//! compute device.

use super::scene_gpu::{GpuFrame, GpuMaterial, GpuProbe, GpuScene, MAT_GLASS};
use super::tracer::{run_entry, upload_scene};

const PI: f64 = std::f64::consts::PI;
const Z: [f32; 3] = [0.0, 0.0, 1.0];

fn opaque(albedo: [f32; 3], metallic: f32, roughness: f32, ior: f32) -> GpuMaterial {
    GpuMaterial { albedo, metallic, roughness, ior, ..Default::default() }
}

fn glass(albedo: [f32; 3], transmission: f32) -> GpuMaterial {
    GpuMaterial { albedo, ior: 1.5, transmission, flags: MAT_GLASS, ..Default::default() }
}

/// The unit direction `deg` degrees from +z toward +x.
fn toward(deg: f64) -> [f32; 3] {
    let r = deg.to_radians();
    [r.sin() as f32, 0.0, r.cos() as f32]
}

fn probe(material: usize, wo: [f32; 3], samples: u32) -> GpuProbe {
    GpuProbe { point: wo, _a: material as f32, normal: Z, _b: samples as f32 }
}

/// Run kernel entry `entry` over `probes` on a scene holding `materials`; `None` (after a SKIP
/// line) without a compute device.
fn run(entry: &str, materials: Vec<GpuMaterial>, probes: &[GpuProbe]) -> Option<Vec<[f32; 4]>> {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("SKIP: no compute device");
        return None;
    };
    let scene = GpuScene { materials, ..Default::default() };
    let buffers = upload_scene(&gpu, &scene);
    let frame = GpuFrame { probe_count: probes.len() as u32, seed: 11, ..Default::default() };
    match run_entry(&gpu, &buffers, entry, &frame, probes) {
        Ok(out) => {
            assert_eq!(out.len(), probes.len());
            Some(out)
        }
        Err(e) => {
            assert!(false, "{entry}: {e}");
            None
        }
    }
}

fn close(a: f32, b: f64, tol: f64, what: &str) {
    assert!((a as f64 - b).abs() <= tol, "{what}: got {a}, want {b} (tolerance {tol})");
}

/// Exact unpolarised Fresnel reflectance of a dielectric interface and the refraction's cosine,
/// for an incident cosine `cosi` and `eta` = n_incident / n_transmitted; (1, 0) beyond the
/// critical angle.
fn fresnel(cosi: f64, eta: f64) -> (f64, f64) {
    let sin2t = eta * eta * (1.0 - cosi * cosi).max(0.0);
    if sin2t >= 1.0 {
        return (1.0, 0.0);
    }
    let cost = (1.0 - sin2t).sqrt();
    let rs = (eta * cosi - cost) / (eta * cosi + cost);
    let rp = (cosi - eta * cost) / (cosi + eta * cost);
    (0.5 * (rs * rs + rp * rp), cost)
}

/// The GGX specular lobe's directional albedo, ∫ f_s cos dω by quadrature, for the F0 `f0`,
/// the roughness `alpha` and a view `theta_o` radians from the normal (the reference the
/// shader's sampling must average to).
fn ggx_albedo(f0: [f64; 3], alpha: f64, theta_o: f64) -> [f64; 3] {
    let (so, co) = theta_o.sin_cos();
    let a2 = alpha * alpha;
    let lambda = |c: f64| 0.5 * ((1.0 + a2 * (1.0 - c * c) / (c * c)).sqrt() - 1.0);
    let (nt, np) = (600usize, 600usize);
    let (dt, dp) = (0.5 * PI / nt as f64, 2.0 * PI / np as f64);
    let mut sum = [0.0f64; 3];
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
            for c in 0..3 {
                sum[c] += (f0[c] + (1.0 - f0[c]) * k) * w;
            }
        }
    }
    sum
}

#[test]
fn lambert_weight_is_albedo() {
    // IOR 1 and metallic 0: no specular lobe, a pure Lambert surface.
    let albedo = [0.2f32, 0.5, 0.8];
    let probes: Vec<GpuProbe> = [0.0, 45.0, 80.0].iter().map(|d| probe(0, toward(*d), 4096)).collect();
    let Some(out) = run("bsdf_test_main", vec![opaque(albedo, 0.0, 0.8, 1.0)], &probes) else {
        return;
    };
    for (k, o) in out.iter().enumerate() {
        for c in 0..3 {
            close(o[c], albedo[c] as f64, 1e-4, &format!("probe {k} channel {c}"));
        }
        assert!(o[3] > 0.9999, "probe {k}: eval/pdf must match every weight, got {}", o[3]);
    }
}

#[test]
fn ggx_sample_weight_equals_eval_over_pdf() {
    let albedo = [0.9f32, 0.8, 0.6];
    let angles = [0.0, 30.0, 60.0, 80.0];
    let probes: Vec<GpuProbe> = angles.iter().map(|d| probe(0, toward(*d), 16384)).collect();
    let Some(out) = run("bsdf_test_main", vec![opaque(albedo, 1.0, 0.5, 1.5)], &probes) else {
        return;
    };
    let f0 = [albedo[0] as f64, albedo[1] as f64, albedo[2] as f64];
    for (k, o) in out.iter().enumerate() {
        assert!(o[3] > 0.9999, "{} deg: every non-specular weight must equal eval/pdf, matched {}", angles[k], o[3]);
        // The weights average to the lobe's directional albedo.
        let reference = ggx_albedo(f0, 0.25, angles[k].to_radians());
        for c in 0..3 {
            close(o[c], reference[c], 0.012, &format!("{} deg channel {c} mean weight vs quadrature", angles[k]));
        }
    }
}

#[test]
fn a_smooth_metal_reflects_exactly() {
    let albedo = [0.9f32, 0.7, 0.5];
    let wo = toward(45.0);
    let mirror = opaque(albedo, 1.0, 0.0, 1.5);
    // The mirrored direction, flagged specular.
    let probes = vec![probe(0, wo, 256); 8];
    let Some(dirs) = run("bsdf_dir_test_main", vec![mirror], &probes) else {
        return;
    };
    for d in &dirs {
        close(d[0], -wo[0] as f64, 1e-5, "mirrored x");
        close(d[1], 0.0, 1e-5, "mirrored y");
        close(d[2], wo[2] as f64, 1e-5, "mirrored z");
        assert_eq!(d[3], 1.0, "a mirror is a specular sample");
    }
    // Weight F (≈ F0 at 45 deg: Schlick adds (1 - F0) * (1 - cos 45)^5 ≈ 0.002), and no density.
    let c45 = (45.0f64).to_radians().cos();
    let Some(w) = run("bsdf_test_main", vec![mirror], &[probe(0, wo, 64)]) else {
        return;
    };
    for c in 0..3 {
        let f0 = albedo[c] as f64;
        close(w[0][c], f0 + (1.0 - f0) * (1.0 - c45).powi(5), 1e-4, &format!("weight channel {c}"));
        close(w[0][c], f0, 0.003, &format!("weight channel {c} is F0"));
    }
    let Some(dens) = run("bsdf_density_test_main", vec![mirror], &probes) else {
        return;
    };
    assert!(dens.iter().all(|d| d.iter().all(|x| *x == 0.0)), "a delta lobe has no eval or pdf: {dens:?}");
    // Roughness 0.02 is the first glossy (sampled, not mirrored) lobe.
    let Some(glossy) = run("bsdf_dir_test_main", vec![opaque(albedo, 1.0, 0.02, 1.5)], &probes) else {
        return;
    };
    assert!(glossy.iter().all(|d| d[3] == 0.0), "roughness 0.02 is a GGX lobe: {glossy:?}");
}

#[test]
fn glass_splits_by_fresnel_and_refracts_by_snell() {
    let tint = glass([0.5, 0.4, 0.3], 0.8);
    const N: usize = 1000;
    // (view angle from +z, from outside (entering) or from inside)
    let cases: [(f64, bool); 4] = [(0.0, true), (45.0, true), (30.0, false), (60.0, false)];
    for (deg, entering) in cases {
        let mut wo = toward(deg);
        if !entering {
            wo[2] = -wo[2];
        }
        let Some(dirs) = run("bsdf_dir_test_main", vec![tint], &vec![probe(0, wo, 1); N]) else {
            return;
        };
        let cosi = deg.to_radians().cos();
        let eta = if entering { 1.0 / 1.5 } else { 1.5 };
        let (f, cost) = fresnel(cosi, eta);
        // Same side as the viewer = reflected.
        let side = if entering { 1.0 } else { -1.0 };
        let reflected = dirs.iter().filter(|d| d[2] as f64 * side > 0.0).count();
        let what = format!("{deg} deg {}", if entering { "entering" } else { "leaving" });
        assert!((reflected as f64 / N as f64 - f).abs() <= 0.002, "{what}: reflected {reflected}/{N}, Fresnel {f}");
        assert!(dirs.iter().all(|d| d[3] == 1.0), "{what}: glass samples are specular");
        let sini = deg.to_radians().sin();
        for d in &dirs {
            let is_reflection = d[2] as f64 * side > 0.0;
            if is_reflection {
                close(d[0], -sini, 1e-5, &format!("{what}: reflected x"));
                close(d[2], wo[2] as f64, 1e-5, &format!("{what}: reflected z"));
            } else {
                // Snell: sin θt = eta · sin θi, the same azimuth away from the view.
                close(d[0], -eta * sini, 1e-5, &format!("{what}: refracted x"));
                close(d[2], -side * cost, 1e-5, &format!("{what}: refracted z"));
            }
        }
        if !entering && deg >= 41.9 {
            assert_eq!(reflected, N, "{what}: beyond the critical angle everything reflects");
        }
    }
    // Weights: 1 for a reflection, albedo · transmission for a refraction; no eval or pdf.
    let probes = [probe(0, toward(0.0), 16384), probe(0, [-0.866_025_4, 0.0, -0.5], 16384)];
    let Some(w) = run("bsdf_test_main", vec![tint], &probes) else {
        return;
    };
    let t = [0.5f64 * 0.8, 0.4 * 0.8, 0.3 * 0.8];
    for c in 0..3 {
        close(w[0][c], 0.04 + 0.96 * t[c], 0.006, &format!("normal incidence mean weight channel {c}"));
        close(w[1][c], 1.0, 1e-5, &format!("total internal reflection weight channel {c}"));
    }
    assert!(w.iter().all(|x| x[3] == 1.0), "no non-specular sample in glass");
    let Some(dens) = run("bsdf_density_test_main", vec![tint], &vec![probe(0, toward(45.0), 1); 16]) else {
        return;
    };
    assert!(dens.iter().all(|d| d.iter().all(|x| *x == 0.0)), "glass has no eval or pdf: {dens:?}");
}

#[test]
fn white_bsdfs_do_not_create_energy() {
    // (material, name, strict): a strict material's weights are individually bounded by 1, so
    // its mean cannot exceed 1 + 1e-3; a dielectric blend's one-sample mixture estimator has a
    // statistical error of ~1e-3 around an expectation that is at most 1, so it gets 1 + 1e-2
    // (an uncompensated diffuse + specular sum would be 1.04 at normal incidence).
    let white = [1.0f32; 3];
    let materials: Vec<(GpuMaterial, &str, bool)> = vec![
        (opaque(white, 0.0, 0.5, 1.0), "lambert", true),
        (opaque(white, 1.0, 0.0, 1.5), "mirror metal", true),
        (opaque(white, 1.0, 0.03, 1.5), "metal roughness 0.03", true),
        (opaque(white, 1.0, 0.3, 1.5), "metal roughness 0.3", true),
        (opaque(white, 1.0, 0.5, 1.5), "metal roughness 0.5", true),
        (opaque(white, 1.0, 1.0, 1.5), "metal roughness 1", true),
        (glass(white, 1.0), "glass", true),
        (opaque(white, 0.0, 0.0, 1.5), "smooth dielectric", false),
        (opaque(white, 0.0, 0.1, 1.5), "dielectric roughness 0.1", false),
        (opaque(white, 0.0, 0.5, 1.5), "dielectric roughness 0.5", false),
        (opaque(white, 0.0, 1.0, 1.5), "dielectric roughness 1", false),
        (opaque(white, 0.5, 0.4, 1.5), "half metal roughness 0.4", false),
    ];
    let angles = [0.0, 30.0, 60.0, 85.0];
    let mut probes = Vec::new();
    for k in 0..materials.len() {
        for d in angles {
            probes.push(probe(k, toward(d), 32768));
        }
    }
    let Some(out) = run("bsdf_test_main", materials.iter().map(|m| m.0).collect(), &probes) else {
        return;
    };
    for (k, (_, name, strict)) in materials.iter().enumerate() {
        let limit = if *strict { 1.0 + 1e-3 } else { 1.0 + 1e-2 };
        for (a, d) in angles.iter().enumerate() {
            let o = out[k * angles.len() + a];
            for c in 0..3 {
                assert!((o[c] as f64) <= limit, "{name} at {d} deg channel {c}: mean weight {} exceeds {limit}", o[c]);
                assert!(o[c] > 0.3, "{name} at {d} deg channel {c}: mean weight {} lost most of the energy", o[c]);
                if *name == "lambert" || *name == "glass" || *name == "mirror metal" {
                    // Nothing is lost in these: a white one returns exactly what it receives.
                    close(o[c], 1.0, 1e-4, &format!("{name} at {d} deg channel {c}"));
                }
            }
        }
    }
}
