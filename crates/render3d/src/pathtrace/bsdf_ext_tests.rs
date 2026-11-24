//! GPU tests of the layered material's extra lobes (`shaders/pt/bsdf_ext.wgsl`: clear coat, sheen,
//! anisotropy), through the same test entries as `bsdf_tests.rs` (`bsdf_test_main` and friends
//! dispatch through `bsdf_sample`, so they reach the new code once the frame's `photo.x` has
//! `PHOTO_MATERIALS` and the material `MAT_EXT`) plus `bsdf_grid_test_main` and
//! `bsdf_width_test_main` of the extension file. They skip (with a SKIP line) without a compute
//! device.
//!
//! White furnace (albedo 1, metallic 0, IOR 1.5, view `NoV` from the probe angle; the mean of
//! 2^16 sample weights, which the furnace's uniform radiance 1 turns into the radiance a surface
//! returns; 1 is lossless) measured by these tests, `loss = 1 - mean`:
//!
//! | lobe (material roughness 0.3)        | NoV 0.2        | NoV 0.5        | NoV 0.9        |
//! |--------------------------------------|----------------|----------------|----------------|
//! | clear coat 1, coat roughness 0.2     | 0.0415         | 0.0110         | -0.0002        |
//! | sheen 1 (white), sheen roughness 0.3 | 0.0346         | 0.0117         | 0.0012         |
//! | anisotropy 0.8                       | -0.0036        | -0.0001        | 0.0008         |
//!
//! (The coat and sheen losses at grazing views are the bilinear table's and the single-scatter
//! lobes' known loss; no case gains energy beyond sampling noise.)

use super::scene_gpu::{GpuFrame, GpuMaterial, GpuProbe, GpuScene, MAT_EXT, PHOTO_MATERIALS};
use super::tracer::{run_entry, upload_scene};

const Z: [f32; 3] = [0.0, 0.0, 1.0];
/// The furnace's view cosines.
const NOV: [f64; 3] = [0.2, 0.5, 0.9];
/// Probes per view and samples per probe: 32 * 2048 = 2^16 samples per view.
const CHUNKS: usize = 32;
const PER_PROBE: u32 = 2048;
const BAND: (f64, f64) = (0.95, 1.02);

/// The layered material: white, dielectric (IOR 1.5), `MAT_EXT`.
fn layered(roughness: f32) -> GpuMaterial {
    GpuMaterial { albedo: [1.0; 3], ior: 1.5, roughness, flags: MAT_EXT, ..Default::default() }
}

/// The unit view direction whose cosine to +z is `nov`.
fn view(nov: f64) -> [f32; 3] {
    [(1.0 - nov * nov).max(0.0).sqrt() as f32, 0.0, nov as f32]
}

fn probe(material: usize, wo: [f32; 3], samples: u32) -> GpuProbe {
    GpuProbe { point: wo, _a: material as f32, normal: Z, _b: samples as f32, ..Default::default() }
}

/// Run kernel entry `entry` with `photo.x = photo` over `probes` on a scene of `materials`;
/// `None` (after a SKIP line) without a compute device.
fn run(entry: &str, photo: u32, materials: Vec<GpuMaterial>, probes: &[GpuProbe]) -> Option<Vec<[f32; 4]>> {
    let Some(gpu) = crate::gputest::shared_compute() else {
        eprintln!("SKIP: no compute device");
        return None;
    };
    let scene = GpuScene { materials, ..Default::default() };
    let buffers = upload_scene(&gpu, &scene);
    let frame = GpuFrame { probe_count: probes.len() as u32, seed: 11, photo: [photo, 0, 0, 0], ..Default::default() };
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

/// The mean weight (over all channels) at each of [`NOV`] for a white furnace of `material`.
fn furnace(material: GpuMaterial) -> Option<[f64; 3]> {
    let mut probes = Vec::new();
    for nov in NOV {
        for _ in 0..CHUNKS {
            probes.push(probe(0, view(nov), PER_PROBE));
        }
    }
    let out = run("bsdf_test_main", PHOTO_MATERIALS, vec![material], &probes)?;
    let mut mean = [0.0f64; 3];
    for (k, m) in mean.iter_mut().enumerate() {
        for o in &out[k * CHUNKS..(k + 1) * CHUNKS] {
            *m += (o[0] + o[1] + o[2]) as f64 / (3.0 * CHUNKS as f64);
        }
    }
    Some(mean)
}

fn check_furnace(name: &str, material: GpuMaterial) {
    let Some(mean) = furnace(material) else {
        return;
    };
    for (k, m) in mean.iter().enumerate() {
        eprintln!("furnace {name} NoV {}: mean {m:.4} loss {:.4}", NOV[k], 1.0 - m);
        assert!(*m >= BAND.0 && *m <= BAND.1, "{name} at NoV {}: mean weight {m} outside {BAND:?}", NOV[k]);
    }
}

#[test]
fn ext_inactive_without_the_materials_flag() {
    // A material with every layer set but a frame without PHOTO_MATERIALS runs today's code: it
    // gives exactly the samples and densities of the plain material.
    let plain = GpuMaterial { flags: 0, ..layered(0.4) };
    let rich = GpuMaterial { coat: 1.0, coat_roughness: 0.3, sheen: [1.0; 3], sheen_roughness: 0.5, aniso: 0.8, aniso_rot: 0.7, ..layered(0.4) };
    let probes: Vec<GpuProbe> = (0..64).map(|k| probe(k % 2, view(0.2 + 0.8 * (k / 2) as f64 / 32.0), 256)).collect();
    for entry in ["bsdf_test_main", "bsdf_dir_test_main", "bsdf_density_test_main"] {
        let (Some(off), Some(off_plain)) = (run(entry, 0, vec![rich, plain], &probes), run(entry, PHOTO_MATERIALS, vec![plain, plain], &probes))
        else {
            return;
        };
        // Probes alternate material 0 (rich) / 1 (plain); with the flag off both are the plain
        // material, whose values the plain-everywhere run gives.
        assert_eq!(off, off_plain, "{entry}: ext code ran without PHOTO_MATERIALS");
    }
    // With the flag on, the layers change the directions.
    let (Some(on), Some(off)) =
        (run("bsdf_dir_test_main", PHOTO_MATERIALS, vec![rich, plain], &probes), run("bsdf_dir_test_main", 0, vec![rich, plain], &probes))
    else {
        return;
    };
    assert_ne!(on, off, "the layered material must sample differently when active");
}

#[test]
fn clearcoat_furnace_conserves_energy() {
    check_furnace("clear coat", GpuMaterial { coat: 1.0, coat_roughness: 0.2, ..layered(0.3) });
}

#[test]
fn sheen_furnace_conserves_energy() {
    check_furnace("sheen", GpuMaterial { sheen: [1.0; 3], sheen_roughness: 0.3, ..layered(0.3) });
}

#[test]
fn anisotropy_furnace_conserves_energy() {
    check_furnace("anisotropy", GpuMaterial { aniso: 0.8, ..layered(0.3) });
}

#[test]
fn ext_pdf_integrates_to_one() {
    // Moderate roughness keeps the mass that reflects below the horizon small. Each material
    // exercises one lobe plus the base; the last has all three.
    let base = layered(0.4);
    let materials = [
        GpuMaterial { aniso: 0.4, aniso_rot: 0.6, ..base },
        GpuMaterial { coat: 1.0, coat_roughness: 0.5, ..base },
        GpuMaterial { sheen: [0.8, 0.6, 0.4], sheen_roughness: 0.5, ..base },
        GpuMaterial { aniso: 0.4, coat: 0.7, coat_roughness: 0.5, sheen: [1.0; 3], sheen_roughness: 0.5, ..base },
    ];
    let probes: Vec<GpuProbe> = (0..materials.len()).map(|k| probe(k, view(0.9), 1)).collect();
    let Some(out) = run("bsdf_grid_test_main", PHOTO_MATERIALS, materials.to_vec(), &probes) else {
        return;
    };
    for (k, o) in out.iter().enumerate() {
        let integral = o[0] as f64;
        assert!((0.95..=1.01).contains(&integral), "material {k}: pdf integrates to {integral}");
    }
    // eval / pdf match every sampled weight (the same material set, away from the normal).
    for nov in [0.3, 0.7, 0.95] {
        let probes: Vec<GpuProbe> = (0..materials.len()).map(|k| probe(k, view(nov), 4096)).collect();
        let Some(out) = run("bsdf_test_main", PHOTO_MATERIALS, materials.to_vec(), &probes) else {
            return;
        };
        for (k, o) in out.iter().enumerate() {
            assert!(o[3] > 0.9999, "material {k} at NoV {nov}: eval/pdf matched only {} of the samples", o[3]);
        }
    }
}

#[test]
fn anisotropic_lobe_is_three_times_longer_than_wide() {
    let metal = GpuMaterial { metallic: 1.0, ..layered(0.3) };
    let aniso = GpuMaterial { aniso: 0.8, ..metal };
    let probes = [probe(0, Z, 1), probe(1, Z, 1)];
    let Some(out) = run("bsdf_width_test_main", PHOTO_MATERIALS, vec![metal, aniso], &probes) else {
        return;
    };
    let (iso, an) = (out[0], out[1]);
    assert!(iso[0] > 0.0 && (iso[0] / iso[1] - 1.0).abs() < 0.05, "an isotropic lobe is round: {iso:?}");
    assert!(an[1] > 0.0 && an[0] >= 3.0 * an[1], "anisotropy 0.8: half-width along the tangent {} vs the bitangent {}", an[0], an[1]);
}
