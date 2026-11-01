use crate::pathtrace::scene_gpu::{GpuFrame, GpuProbe, build_gpu_scene};
use crate::pathtrace::tracer::{run_entry, upload_scene};
use crate::types::{ArchId, Mesh, Scene3d};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }

    fn between(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }
}

fn prism(material: &str, object: u32, min: [f32; 3], max: [f32; 3]) -> Mesh {
    let mut m = Mesh::new(material, ArchId(object));
    for k in 0..8 {
        m.positions.push([
            if k & 1 == 0 { min[0] } else { max[0] },
            if k & 2 == 0 { min[1] } else { max[1] },
            if k & 4 == 0 { min[2] } else { max[2] },
        ]);
    }
    let quads: [[u32; 4]; 6] = [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]];
    for q in quads {
        m.indices.extend([q[0], q[1], q[2], q[0], q[2], q[3]]);
    }
    m
}

fn random_prisms(rng: &mut Rng, count: usize) -> Scene3d {
    let mut scene = Scene3d::default();
    for i in 0..count {
        let c = [rng.between(-10.0, 10.0), rng.between(-10.0, 10.0), rng.between(-10.0, 10.0)];
        let h = [rng.between(0.25, 2.0), rng.between(0.25, 2.0), rng.between(0.25, 2.0)];
        let material = ["wall", "glass", "floor", "light-lens-lit"][i % 4];
        scene.meshes.push(prism(material, i as u32 + 1, [c[0] - h[0], c[1] - h[1], c[2] - h[2]], [c[0] + h[0], c[1] + h[1], c[2] + h[2]]));
    }
    scene
}

#[cfg(test)]
mod traverse_tests {
    use super::*;
    use crate::pathtrace::scene_gpu::material_for;

    #[test]
    fn gpu_traversal_matches_trace_cpu() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        let mut rng = Rng(42);
        let scene = random_prisms(&mut rng, 40);
        let gpu_scene = build_gpu_scene(&scene, false, None);
        let buffers = upload_scene(&gpu, &gpu_scene);

        let mut probes = Vec::with_capacity(256);
        for _ in 0..256 {
            let origin = [rng.between(-12.0, 12.0), rng.between(-12.0, 12.0), rng.between(-12.0, 12.0)];
            let target = [rng.between(-10.0, 10.0), rng.between(-10.0, 10.0), rng.between(-10.0, 10.0)];
            let mut dir = [target[0] - origin[0], target[1] - origin[1], target[2] - origin[2]];
            let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
            if len <= 1e-6 {
                dir = [1.0, 0.0, 0.0];
            } else {
                dir = [dir[0] / len, dir[1] / len, dir[2] / len];
            }
            probes.push(GpuProbe { point: origin, normal: dir, ..Default::default() });
        }
        let frame = GpuFrame { probe_count: probes.len() as u32, ..Default::default() };
        let out = run_entry(&gpu, &buffers, "traverse_test_main", &frame, &probes).unwrap();

        for (probe, got) in probes.iter().zip(out.iter()) {
            let want = gpu_scene.trace_cpu(probe.point, probe.normal, 1e30);
            let got_t = got[0];
            let got_tri = got[1].to_bits() as u32;
            match want {
                Some(w) => {
                    assert!((got_t - w.t).abs() < 1e-3, "ray {:?} => got {got_t}, want {}", probe.point, w.t);
                    if (got_t - w.t).abs() > 1e-5 {
                        assert_eq!(got_tri, w.tri, "ray {:?}: got tri {got_tri}, want {}", probe.point, w.tri);
                    }
                }
                None => assert!(got_t < 0.0, "miss expected: {got:?}"),
            }
        }
    }

    #[test]
    fn visibility_skips_fixture_housings_and_tints_through_glass() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        let material = material_for("glass");
        // Single glass triangle at normal incidence: tinted once, with Fresnel (1-F)^2.
        // F0 = 0.04 for IOR 1.5. cos_i = 1.0. F = 0.04. (1-F)^2 = 0.9216.
        let fresnel_sq = 0.9216;
        let expected_glass = [
            material.albedo[0] * material.transmission * fresnel_sq,
            material.albedo[1] * material.transmission * fresnel_sq,
            material.albedo[2] * material.transmission * fresnel_sq,
        ];
        let glass_pane = {
            let mut m = Mesh::new("glass", ArchId(2));
            m.positions = vec![[0.0, -1.0, -1.0], [0.0, 1.0, -1.0], [0.0, 1.0, 1.0]];
            m.indices = vec![0, 1, 2];
            m
        };
        let cases = [
            (Scene3d { meshes: vec![prism("light-housing", 1, [0.0, -0.5, -0.5], [1.0, 0.5, 0.5])], ..Default::default() }, [1.0, 1.0, 1.0]),
            (Scene3d { meshes: vec![glass_pane], ..Default::default() }, expected_glass),
            (Scene3d { meshes: vec![prism("wall", 3, [0.0, -0.5, -0.5], [1.0, 0.5, 0.5])], ..Default::default() }, [0.0, 0.0, 0.0]),
        ];

        for (scene, expected) in cases {
            let gpu_scene = build_gpu_scene(&scene, false, None);
            let buffers = upload_scene(&gpu, &gpu_scene);
            let probes = vec![GpuProbe { point: [-2.0, 0.0, 0.0], normal: [1.0, 0.0, 0.0], _a: 10.0, ..Default::default() }];
            let frame = GpuFrame { probe_count: 1, ..Default::default() };
            let out = run_entry(&gpu, &buffers, "visibility_test_main", &frame, &probes).unwrap();
            assert!(out[0][0..3].iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-5), "expected {expected:?}, got {:?}", out[0]);
            assert_eq!(out[0][3], 0.0);
        }
    }

    #[test]
    fn a_closed_glass_pane_tints_a_shadow_ray_once() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        let material = material_for("glass");
        // Prism crosses glass twice (enter, exit). Tinted once on entry with Fresnel.
        let fresnel_sq = 0.9216;
        let expected = [
            material.albedo[0] * material.transmission * fresnel_sq,
            material.albedo[1] * material.transmission * fresnel_sq,
            material.albedo[2] * material.transmission * fresnel_sq,
        ];
        let scene = Scene3d { meshes: vec![prism("glass", 2, [0.0, -1.0, -1.0], [0.02, 1.0, 1.0])], ..Default::default() };
        let gpu_scene = build_gpu_scene(&scene, false, None);
        let buffers = upload_scene(&gpu, &gpu_scene);
        let probes = vec![GpuProbe { point: [-2.0, 0.0, 0.0], normal: [1.0, 0.0, 0.0], _a: 10.0, ..Default::default() }];
        let frame = GpuFrame { probe_count: 1, ..Default::default() };
        let out = run_entry(&gpu, &buffers, "visibility_test_main", &frame, &probes).unwrap();
        assert!(out[0][0..3].iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-5), "expected {expected:?}, got {:?}", out[0]);
    }

    #[test]
    fn two_panes_tint_a_shadow_ray_twice() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        let material = material_for("glass");
        let fresnel_sq = 0.9216;
        let single = [
            material.albedo[0] * material.transmission * fresnel_sq,
            material.albedo[1] * material.transmission * fresnel_sq,
            material.albedo[2] * material.transmission * fresnel_sq,
        ];
        let expected = [single[0] * single[0], single[1] * single[1], single[2] * single[2]];
        let scene = Scene3d {
            meshes: vec![prism("glass", 2, [0.0, -1.0, -1.0], [0.02, 1.0, 1.0]), prism("glass", 3, [0.5, -1.0, -1.0], [0.52, 1.0, 1.0])],
            ..Default::default()
        };
        let gpu_scene = build_gpu_scene(&scene, false, None);
        let buffers = upload_scene(&gpu, &gpu_scene);
        let probes = vec![GpuProbe { point: [-2.0, 0.0, 0.0], normal: [1.0, 0.0, 0.0], _a: 10.0, ..Default::default() }];
        let frame = GpuFrame { probe_count: 1, ..Default::default() };
        let out = run_entry(&gpu, &buffers, "visibility_test_main", &frame, &probes).unwrap();
        assert!(out[0][0..3].iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-5), "expected {expected:?}, got {:?}", out[0]);
    }

    #[test]
    fn fresnel_dims_a_shadow_ray_at_a_steep_angle() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        let material = material_for("glass");
        // Ray direction (0.463, 0, 0.886). Normal is (1,0,0) for the prism face at x=0.
        // cos_i = dot(dir, normal) = 0.463.
        // F0 = 0.04.
        // F = 0.04 + 0.96 * (1 - 0.463)^5 = 0.04 + 0.96 * (0.537)^5.
        // (0.537)^5 ≈ 0.0445. F ≈ 0.04 + 0.0427 = 0.0827.
        // (1-F)^2 ≈ (0.9173)^2 ≈ 0.8414.
        let cos_i: f32 = 0.463;
        let f0: f32 = 0.04;
        let f = f0 + (1.0 - f0) * (1.0 - cos_i).powi(5);
        let fresnel_sq = (1.0 - f) * (1.0 - f);
        let expected = [
            material.albedo[0] * material.transmission * fresnel_sq,
            material.albedo[1] * material.transmission * fresnel_sq,
            material.albedo[2] * material.transmission * fresnel_sq,
        ];
        let scene = Scene3d { meshes: vec![prism("glass", 2, [0.0, -1.0, -1.0], [0.02, 1.0, 1.0])], ..Default::default() };
        let gpu_scene = build_gpu_scene(&scene, false, None);
        let buffers = upload_scene(&gpu, &gpu_scene);
        // Probe from (-1.0, 0.0, -1.914) along (0.463, 0.0, 0.886)
        // This ray crosses the pane near the origin.
        let probes = vec![GpuProbe { point: [-1.0, 0.0, -1.914], normal: [0.463, 0.0, 0.886], _a: 10.0, ..Default::default() }];
        let frame = GpuFrame { probe_count: 1, ..Default::default() };
        let out = run_entry(&gpu, &buffers, "visibility_test_main", &frame, &probes).unwrap();
        assert!(out[0][0..3].iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-4), "expected {expected:?}, got {:?}", out[0]);
    }

    #[test]
    fn an_empty_scene_misses() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        let scene = build_gpu_scene(&Scene3d::default(), false, None);
        let buffers = upload_scene(&gpu, &scene);
        let probes = vec![GpuProbe { point: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], _a: 1e30, ..Default::default() }];
        let frame = GpuFrame { probe_count: 1, ..Default::default() };
        let out = run_entry(&gpu, &buffers, "traverse_test_main", &frame, &probes).unwrap();
        assert!(out[0][0] < 0.0, "empty scene should miss: {:?}", out[0]);
    }
}
