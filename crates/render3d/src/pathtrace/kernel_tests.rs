//! Tests of `shaders/pt/kernel.wgsl`: `irradiance_main` and `trace_main` driven through
//! `tracer::run_entry` (which dispatches one 64-wide workgroup per 64 probes, so an 8 x 8-pixel
//! frame with `probe_count` 64 runs `trace_main` over every pixel) with scenes from
//! `build_gpu_scene`. The AOV test dispatches by hand because `run_entry` does not read them back.

use super::device::ComputeGpu;
use super::scene_gpu::{FRAME_FURNACE, FRAME_SUN, GpuFrame, GpuMaterial, GpuProbe, GpuScene, build_gpu_scene, material_for};
use super::shaders::{PtPipelines, kernel_source};
use super::spec_albedo::{spec_albedo_lookup, spec_albedo_table};
use super::tracer::{run_entry, upload_scene};
use super::{Furnace, luminance};
use crate::types::{ArchId, Mesh, Scene3d};
use cadcraft_arch3d::light::{LightKind, SceneLight, light_axis};
use cadcraft_geom::Vec3;
use wgpu::util::DeviceExt;

fn gpu_or_skip() -> Option<ComputeGpu> {
    let gpu = crate::gputest::shared_compute();
    if gpu.is_none() {
        eprintln!("SKIP: no compute device");
    }
    gpu
}

fn opaque(albedo: [f32; 3], metallic: f32, roughness: f32, ior: f32) -> GpuMaterial {
    GpuMaterial { albedo, metallic, roughness, ior, ..Default::default() }
}

fn toward(deg: f64) -> [f32; 3] {
    let r = deg.to_radians();
    [r.sin() as f32, 0.0, r.cos() as f32]
}

fn probe(material: usize, wo: [f32; 3], samples: u32) -> GpuProbe {
    GpuProbe { point: wo, _a: material as f32, normal: [0.0, 0.0, 1.0], _b: samples as f32, ..Default::default() }
}

fn run_bsdf(entry: &str, materials: Vec<GpuMaterial>, probes: &[GpuProbe]) -> Option<Vec<[f32; 4]>> {
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

/// The phase-10 room: 4 x 4 x 2.4 inside, wall, floor and ceiling slabs, a 0.2 x 0.2 x 1 column at
/// (1.5, 1.5) and a pot light (950.36 cd, 60 degree hard cone) at each of four ceiling points.
fn room() -> Scene3d {
    let t = 0.2;
    let meshes = vec![
        prism("floor", 1, [-t, -t, -t], [4.0 + t, 4.0 + t, 0.0]),
        prism("ceiling", 2, [-t, -t, 2.4], [4.0 + t, 4.0 + t, 2.4 + t]),
        prism("wall", 3, [-t, -t, 0.0], [4.0 + t, 0.0, 2.4]),
        prism("wall", 4, [-t, 4.0, 0.0], [4.0 + t, 4.0 + t, 2.4]),
        prism("wall", 5, [-t, 0.0, 0.0], [0.0, 4.0, 2.4]),
        prism("wall", 6, [4.0, 0.0, 0.0], [4.0 + t, 4.0, 2.4]),
        prism("wall", 7, [1.4, 1.4, 0.0], [1.6, 1.6, 1.0]),
    ];
    let down = Vec3::new(0.0, 0.0, -1.0);
    let lights = [(1.0, 1.0), (1.0, 3.0), (3.0, 1.0), (3.0, 3.0)]
        .iter()
        .map(|&(x, y)| SceneLight {
            kind: LightKind::Spot,
            position: Vec3::new(x, y, 2.4),
            direction: down,
            intensity: 950.36,
            color: [1.0; 3],
            cos_inner: 30f64.to_radians().cos(),
            cos_outer: 30f64.to_radians().cos(),
            axis: light_axis(down, 0.0),
            ..Default::default()
        })
        .collect();
    Scene3d { meshes, lights, ..Default::default() }
}

fn up_probe(x: f32, y: f32) -> GpuProbe {
    GpuProbe { point: [x, y, 0.0], normal: [0.0, 0.0, 1.0], ..Default::default() }
}

fn check(result: Result<Vec<[f32; 4]>, String>) -> Vec<[f32; 4]> {
    match result {
        Ok(out) => out,
        Err(e) => {
            assert!(false, "{e}");
            Vec::new()
        }
    }
}

/// Mean illuminance (lux) at each probe.
fn irradiance(gpu: &ComputeGpu, scene: &GpuScene, bounces: u32, spp: u32, probes: &[GpuProbe]) -> Vec<f64> {
    let buffers = upload_scene(gpu, scene);
    let frame = GpuFrame {
        probe_count: probes.len() as u32,
        light_count: scene.lights.len() as u32,
        bounces,
        spp_this_pass: spp,
        seed: 5,
        ..Default::default()
    };
    check(run_entry(gpu, &buffers, "irradiance_main", &frame, probes))
        .iter()
        .map(|a| {
            assert_eq!(a[3], spp as f32, "the sample count");
            f64::from(luminance([a[0], a[1], a[2]])) / f64::from(spp)
        })
        .collect()
}

/// A frame for an 8 x 8 image from `eye` along unit `f` with right `r` and up `u` (a 90 degree
/// field): pixel ray = f + r x + u y, as an affine-plus-w inverse view-projection.
fn camera_frame(eye: [f32; 3], f: [f32; 3], r: [f32; 3], u: [f32; 3]) -> GpuFrame {
    let a = -0.5;
    let col2 = [eye[0] * a, eye[1] * a, eye[2] * a, a];
    let col3 = [eye[0] + f[0], eye[1] + f[1], eye[2] + f[2], 1.0];
    GpuFrame {
        inv_view_proj: [[r[0], r[1], r[2], 0.0], [u[0], u[1], u[2], 0.0], col2, col3],
        eye: [eye[0], eye[1], eye[2], 1.0],
        size: [8, 8],
        probe_count: 64,
        ..Default::default()
    }
}

fn trace(gpu: &ComputeGpu, scene: &GpuScene, frame: &GpuFrame) -> Vec<[f32; 4]> {
    let buffers = upload_scene(gpu, scene);
    let frame = GpuFrame { light_count: scene.lights.len() as u32, ..*frame };
    check(run_entry(gpu, &buffers, "trace_main", &frame, &vec![GpuProbe::default(); 64]))
}

fn room_frame(bounces: u32, spp: u32, seed: u32) -> GpuFrame {
    GpuFrame { bounces, spp_this_pass: spp, seed, ..camera_frame([2.0, 2.0, 1.2], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]) }
}

#[test]
fn irradiance_under_a_pot_light_is_165_lux() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = build_gpu_scene(&room(), false, None);
    let lux = irradiance(&gpu, &scene, 0, 16, &[up_probe(1.0, 1.0)])[0];
    assert!((lux - 165.0).abs() <= 165.0 * 0.03, "{lux}");
}

#[test]
fn irradiance_behind_a_column_is_dark_without_bounces_and_lit_with() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = build_gpu_scene(&room(), false, None);
    let probe = [up_probe(1.8, 1.8)];
    let direct = irradiance(&gpu, &scene, 0, 16, &probe)[0];
    assert!(direct <= 0.5, "{direct}");
    let indirect = irradiance(&gpu, &scene, 4, 2048, &probe)[0];
    assert!(indirect > 1.0, "{indirect}");
}

#[test]
fn furnace_pixels_converge_to_two() {
    let Some(gpu) = gpu_or_skip() else { return };
    let furnace = Furnace { albedo: 0.5, ..Default::default() };
    let furnace = Furnace { emission: 1.0, ..furnace };
    let mut scene = room();
    scene.lights.clear();
    let scene = build_gpu_scene(&scene, false, Some(furnace));
    let frame = GpuFrame { flags: FRAME_FURNACE, furnace: [0.5, 1.0, 2.0, 0.0], ..room_frame(4, 1024, 3) };
    let out = trace(&gpu, &scene, &frame);
    assert_eq!(out.len(), 64);
    for (i, a) in out.iter().enumerate() {
        let mean = a[0] / 1024.0;
        assert!((mean - 2.0).abs() <= 0.04, "pixel {i}: {mean}");
        assert!((a[1] / 1024.0 - 2.0).abs() <= 0.04 && (a[2] / 1024.0 - 2.0).abs() <= 0.04, "pixel {i}: {a:?}");
    }
}

#[test]
fn trace_accumulates_luminance_squares_after_the_colours() {
    let Some(gpu) = gpu_or_skip() else { return };
    let furnace = Furnace { albedo: 0.5, ..Default::default() };
    let furnace = Furnace { emission: 1.0, ..furnace };
    let mut scene = room();
    scene.lights.clear();
    let scene = build_gpu_scene(&scene, false, Some(furnace));
    let d = &gpu.device;
    let buffers = upload_scene(&gpu, &scene);
    let frame = GpuFrame {
        flags: FRAME_FURNACE,
        furnace: [0.5, 1.0, 2.0, 0.0],
        spp_this_pass: 4,
        seed: 11,
        ..camera_frame([2.0, 2.0, 1.2], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0])
    };
    let pipelines = PtPipelines::new(&gpu, None).unwrap();
    let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("pt test kernel"),
        source: wgpu::ShaderSource::Wgsl(kernel_source().into()),
    });
    let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pt test"),
        bind_group_layouts: &[Some(&pipelines.kernel_layout)],
        immediate_size: 0,
    });
    let pipeline = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("trace_main"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("trace_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let frame_buf = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("sq frame"),
        contents: bytemuck::bytes_of(&frame),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let accum = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sq accum"),
        size: (64 * 2 * 16) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let aov = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sq aov"),
        size: (64 * 2 * 16) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let probes = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("sq probes"),
        contents: bytemuck::cast_slice(&vec![GpuProbe::default(); 64]),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sq trace"),
        layout: &pipelines.kernel_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: frame_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: buffers.nodes.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: buffers.tris.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: buffers.materials.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 4, resource: buffers.lights.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 5, resource: buffers.ies.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 6, resource: accum.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 7, resource: aov.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 8, resource: probes.as_entire_binding() },
        ],
    });
    let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("sq trace") });
    {
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("trace_main"), timestamp_writes: None });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    let read = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sq read"),
        size: (64 * 2 * 16) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    enc.copy_buffer_to_buffer(&accum, 0, &read, 0, (64 * 2 * 16) as u64);
    gpu.queue.submit([enc.finish()]);
    let slice = read.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    d.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(300)) }).unwrap();
    rx.recv().unwrap().unwrap();
    let out: Vec<[f32; 4]> = bytemuck::cast_slice(&slice.get_mapped_range().unwrap()).to_vec();
    read.unmap();
    let first = &out[0];
    assert_eq!(first[3], 4.0, "sample count");
    let expected = 4.0 * luminance([first[0] / 4.0, first[1] / 4.0, first[2] / 4.0]).powi(2);
    assert!((out[64][0] - expected).abs() <= 1e-3 * expected.max(1.0), "variance storage mismatch: {:?}", (&out[0], &out[64]));
}

#[test]
fn a_seed_fixes_every_sample() {
    let Some(gpu) = gpu_or_skip() else { return };
    let scene = build_gpu_scene(&room(), false, None);
    let first = trace(&gpu, &scene, &room_frame(4, 8, 7));
    let again = trace(&gpu, &scene, &room_frame(4, 8, 7));
    assert_eq!(first, again, "the same seed");
    let other = trace(&gpu, &scene, &room_frame(4, 8, 8));
    assert_ne!(first, other, "another seed");
    // A sample depends on its index, not on how the passes were cut.
    let whole = trace(&gpu, &scene, &room_frame(4, 2, 7));
    let a = trace(&gpu, &scene, &GpuFrame { spp_this_pass: 1, ..room_frame(4, 1, 7) });
    let b = trace(&gpu, &scene, &GpuFrame { sample_base: 1, spp_this_pass: 1, ..room_frame(4, 1, 7) });
    for k in 0..whole.len() {
        for c in 0..3 {
            let sum = a[k][c] + b[k][c];
            assert!((whole[k][c] - sum).abs() <= 1e-3 * (1.0 + sum.abs()), "pixel {k}: {whole:?}");
        }
    }
}

#[test]
fn bsdf_spec_albedo_matches_the_cpu_table() {
    let Some(gpu) = gpu_or_skip() else { return };
    let material = opaque([1.0, 1.0, 1.0], 0.0, 0.95, 1.5);
    let table = spec_albedo_table();
    let probes: Vec<GpuProbe> = [60.0, 84.5, 88.6].iter().map(|deg| probe(0, toward(*deg), 4096)).collect();
    let Some(out) = run_bsdf("bsdf_test_main", vec![material], &probes) else {
        return;
    };
    for (idx, deg) in [60.0_f64, 84.5, 88.6].iter().enumerate() {
        let nov = (*deg).to_radians().cos() as f32;
        let (a, b) = spec_albedo_lookup(&table, 0.95, nov);
        let e = 0.04 * a as f64 + 0.96 * b as f64;
        let mean = ((out[idx][0] as f64 + out[idx][1] as f64 + out[idx][2] as f64) / 3.0);
        assert!(mean >= 1.0 - e - 0.03, "{} deg: mean {} below 1-E-0.03 ({})", deg, mean, 1.0 - e);
        assert!(mean <= 1.001, "{} deg: mean {} exceeds 1.001", deg, mean);
        if *deg >= 88.6 {
            assert!(mean >= 0.75, "{} deg: mean {} dropped below 0.75", deg, mean);
        }
    }
}

#[test]
fn path_horizon_plane_matches_flat_terrain_radiance() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut scene = Scene3d::default();
    scene.meshes.push(prism("terrain", 1, [0.0, 0.0, -0.1], [40.0, 30.0, 0.0]));
    let mut horizon = Mesh::new("terrain", ArchId(2));
    horizon.positions = vec![[-9500.0, -9500.0, 0.0], [9500.0, -9500.0, 0.0], [9500.0, 9500.0, 0.0], [-9500.0, 9500.0, 0.0]];
    horizon.indices = vec![0, 1, 2, 0, 2, 3];
    scene.horizon = Some(horizon);
    let scene = build_gpu_scene(&scene, false, None);
    let eye = [20.0f32, 5.0, 1.6];
    let normalize = |v: [f32; 3]| {
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if len > 0.0 { [v[0] / len, v[1] / len, v[2] / len] } else { [0.0, 0.0, 1.0] }
    };
    let trace_mean = |target: [f32; 3]| {
        let dir = normalize([target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]]);
        let right = normalize([dir[1], -dir[0], 0.0]);
        let up = normalize([dir[1] * right[2] - dir[2] * right[1], dir[2] * right[0] - dir[0] * right[2], dir[0] * right[1] - dir[1] * right[0]]);
        let frame = GpuFrame {
            flags: FRAME_SUN,
            sun_dir: [0.3, -0.4, 0.86, 0.0],
            sun_rgb: [1.0e6, 1.0e6, 1.0e6, 0.0],
            bounces: 0,
            spp_this_pass: 64,
            seed: 23,
            ..camera_frame(eye, dir, right, up)
        };
        let out = trace(&gpu, &scene, &frame);
        let mut sum = [0.0f64; 3];
        for px in &out {
            sum[0] += f64::from(px[0]) / 64.0;
            sum[1] += f64::from(px[1]) / 64.0;
            sum[2] += f64::from(px[2]) / 64.0;
        }
        [sum[0] / out.len() as f64, sum[1] / out.len() as f64, sum[2] / out.len() as f64]
    };
    let near = trace_mean([36.0, 5.0, 0.0]);
    let far = trace_mean([86.0, 5.0, 0.0]);
    for c in 0..3 {
        assert!(near[c] > 0.0 && far[c] > 0.0, "radiance channel {c} is non-positive: near={near:?} far={far:?}");
        assert!((near[c] - far[c]).abs() <= 0.2 * near[c].max(far[c]), "channel {c}: near={} far={}", near[c], far[c]);
    }
}

#[test]
fn path_horizon_plane_shadow_ray_starts_above_the_plane() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut scene = Scene3d::default();
    scene.meshes.push(prism("terrain", 1, [0.0, 0.0, -0.1], [40.0, 30.0, 0.0]));
    let mut horizon = Mesh::new("terrain", ArchId(2));
    horizon.positions = vec![[-9500.0, -9500.0, 1.0], [9500.0, -9500.0, 1.0], [9500.0, 9500.0, 1.0], [-9500.0, 9500.0, 1.0]];
    horizon.indices = vec![0, 1, 2, 0, 2, 3];
    scene.horizon = Some(horizon);
    let gpu_scene = build_gpu_scene(&scene, false, None);
    let eye = [20.0f32, 5.0, 1.6];
    let target = [86.0f32, 5.0, 1.0];
    let dir = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
    let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let dir = [dir[0] / len, dir[1] / len, dir[2] / len];
    let hit = gpu_scene.trace_cpu(eye, dir, 1e30).unwrap();
    let hit_p = [eye[0] + dir[0] * hit.t, eye[1] + dir[1] * hit.t, eye[2] + dir[2] * hit.t];
    let n = gpu_scene.tris[hit.tri as usize].n;
    let scale = 1e-4 * (1.0f32).max(hit_p.iter().map(|v| v.abs()).fold(0.0, f32::max));
    let offset = [hit_p[0] + n[0] * scale, hit_p[1] + n[1] * scale, hit_p[2] + n[2] * scale];
    let sun_dir = [0.3f32, -0.4, 0.86];
    let sun_len = (sun_dir[0] * sun_dir[0] + sun_dir[1] * sun_dir[1] + sun_dir[2] * sun_dir[2]).sqrt();
    let sun_dir = [sun_dir[0] / sun_len, sun_dir[1] / sun_len, sun_dir[2] / sun_len];
    let probes = vec![GpuProbe { point: offset, normal: sun_dir, _a: 1.0e6, ..Default::default() }];
    let frame = GpuFrame { probe_count: 1, ..Default::default() };
    let out = check(run_entry(&gpu, &upload_scene(&gpu, &gpu_scene), "visibility_test_main", &frame, &probes));
    assert!(out[0][0..3].iter().all(|v| (v - 1.0).abs() < 1e-5), "shadow ray starts below the plane: {:?}", out[0]);
}

#[test]
fn horizon_plane_is_lit_like_near_terrain() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut scene = Scene3d::default();
    scene.meshes.push(prism("terrain", 1, [-2.0, -2.0, 0.0], [2.0, 2.0, 0.0]));
    let mut horizon = Mesh::new("terrain", ArchId(2));
    horizon.positions = vec![[-500.0, -500.0, 0.0], [500.0, -500.0, 0.0], [500.0, 500.0, 0.0], [-500.0, 500.0, 0.0]];
    horizon.indices = vec![0, 1, 2, 0, 2, 3];
    scene.horizon = Some(horizon);
    let gpu_scene = build_gpu_scene(&scene, false, None);
    let probes = [
        GpuProbe { point: [0.0, 0.0, 0.001], normal: [0.0, 0.0, 1.0], ..Default::default() },
        GpuProbe { point: [0.0, 250.0, 0.001], normal: [0.0, 0.0, 1.0], ..Default::default() },
    ];
    let frame = GpuFrame {
        flags: FRAME_SUN,
        sun_dir: [0.0, 0.8944272, 0.4472136, 0.0],
        sun_rgb: [1.0e6, 1.0e6, 1.0e6, 0.0],
        bounces: 0,
        probe_count: probes.len() as u32,
        light_count: gpu_scene.lights.len() as u32,
        spp_this_pass: 256,
        seed: 23,
        ..Default::default()
    };
    let out = check(run_entry(&gpu, &upload_scene(&gpu, &gpu_scene), "irradiance_main", &frame, &probes));
    let lux = |a: &[f32; 4]| f64::from(luminance([a[0] / 256.0, a[1] / 256.0, a[2] / 256.0]));
    let (near, far) = (lux(&out[0]), lux(&out[1]));
    assert!((near - far).abs() <= 0.03 * near.max(far), "near {near} lux, far {far} lux");
}

#[test]
fn far_hit_shadow_rays_do_not_self_intersect() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut scene = Scene3d::default();
    let mut horizon = Mesh::new("terrain", ArchId(3));
    horizon.positions = vec![[-5000.0, -5000.0, -0.5], [5000.0, -5000.0, -0.5], [5000.0, 5000.0, -0.5], [-5000.0, 5000.0, -0.5]];
    horizon.indices = vec![0, 1, 2, 0, 2, 3];
    scene.horizon = Some(horizon);
    let gpu_scene = build_gpu_scene(&scene, false, None);
    let origin = [0.0f32, 500.0, 0.001];
    let dir = [0.0f32, 1.0, 0.01];
    let dir = [
        dir[0] / (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt(),
        dir[1] / (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt(),
        dir[2] / (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt(),
    ];
    let probes = vec![GpuProbe { point: origin, normal: dir, _a: 20.0, ..Default::default() }];
    let frame = GpuFrame { probe_count: 1, ..Default::default() };
    let out = check(run_entry(&gpu, &upload_scene(&gpu, &gpu_scene), "visibility_test_main", &frame, &probes));
    assert!(out[0][0..3].iter().all(|v| (v - 1.0).abs() < 1e-5), "self-shadowed horizon: {:?}", out[0]);
}

#[test]
fn horizon_material_index_with_and_without_ground() {
    let mut scene = Scene3d::default();
    scene.meshes.push(prism("terrain", 1, [-1.0, -1.0, 0.0], [1.0, 1.0, 0.0]));
    let mut horizon = Mesh::new("terrain", ArchId(2));
    horizon.positions = vec![[-500.0, -500.0, 0.0], [500.0, -500.0, 0.0], [500.0, 500.0, 0.0], [-500.0, 500.0, 0.0]];
    horizon.indices = vec![0, 1, 2, 0, 2, 3];
    scene.horizon = Some(horizon);
    let without = build_gpu_scene(&scene, false, None);
    let with_ground = build_gpu_scene(&scene, true, None);
    assert_eq!(with_ground.materials.len(), without.materials.len() + 1);
    let mut horizon_mat = material_for("terrain");
    horizon_mat.flags |= super::scene_gpu::TRI_SINGLE_SIDED;
    assert_eq!(with_ground.materials[with_ground.materials.len() - 1], horizon_mat);
    assert_eq!(without.materials[1], horizon_mat);
}

#[test]
fn camera_only_emitters_light_nothing() {
    let Some(gpu) = gpu_or_skip() else { return };
    // A lit lens above the probe, no lights: seen by a camera, it lights nothing.
    assert!(material_for("light-lens-lit").emission[0] > 0.0);
    let scene = Scene3d {
        meshes: vec![prism("floor", 1, [-5.0, -5.0, -0.2], [5.0, 5.0, 0.0]), prism("light-lens-lit", 2, [-0.5, -0.5, 1.0], [0.5, 0.5, 1.05])],
        ..Default::default()
    };
    let gpu_scene = build_gpu_scene(&scene, false, None);
    for bounces in [0, 4] {
        let lux = irradiance(&gpu, &gpu_scene, bounces, 512, &[up_probe(0.0, 0.0)])[0];
        assert!(lux.abs() < 1e-3, "bounces {bounces}: {lux}");
    }
    // The camera sees it: a camera under the lens looking up.
    let frame = GpuFrame { spp_this_pass: 4, ..camera_frame([0.0, 0.0, 0.5], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]) };
    let out = trace(&gpu, &gpu_scene, &frame);
    let centre = out[3 * 8 + 3];
    assert!(centre[0] / 4.0 > 1000.0, "{centre:?}");
}

#[test]
fn aovs_hold_the_first_hit() {
    let Some(gpu) = gpu_or_skip() else { return };
    // A 2 x 2 floor square seen from 2 m above, looking down.
    let scene = build_gpu_scene(&Scene3d { meshes: vec![prism("floor", 1, [-1.0, -1.0, -0.1], [1.0, 1.0, 0.0])], ..Default::default() }, false, None);
    let frame = GpuFrame { spp_this_pass: 2, ..camera_frame([0.0, 0.0, 2.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]) };
    let aov = run_with_aov(&gpu, &scene, &frame);
    let albedo = material_for("floor").albedo;
    // Pixel (3, 3) is on the floor.
    let (a, n) = (aov[2 * (3 * 8 + 3)], aov[2 * (3 * 8 + 3) + 1]);
    assert_eq!(a[3], 1.0);
    for c in 0..3 {
        assert!((a[c] - albedo[c]).abs() < 1e-6, "{a:?}");
    }
    assert!(n[0].abs() < 1e-6 && n[1].abs() < 1e-6 && (n[2] - 1.0).abs() < 1e-6, "{n:?}");
    assert!(n[3] >= 2.0 && n[3] <= 2.2, "{n:?}");
    // Pixel (0, 0) looks past the square.
    assert_eq!(aov[0], [0.0; 4]);
    assert_eq!(aov[1], [0.0; 4]);
}

#[test]
fn pt_clip_box_and_near_clip() {
    use super::scene_gpu::FRAME_CLIP;
    let Some(gpu) = gpu_or_skip() else { return };
    // A floor at z = 0 under a slab at z 1.0..1.1, seen from (0, 0, 2) looking down.
    let scene = build_gpu_scene(
        &Scene3d {
            meshes: vec![prism("floor", 1, [-1.0, -1.0, -0.1], [1.0, 1.0, 0.0]), prism("ceiling", 2, [-1.0, -1.0, 1.0], [1.0, 1.0, 1.1])],
            ..Default::default()
        },
        false,
        None,
    );
    let base = GpuFrame { spp_this_pass: 1, ..camera_frame([0.0, 0.0, 2.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]) };
    let t_at = |frame: &GpuFrame| run_with_aov(&gpu, &scene, frame)[2 * (3 * 8 + 3) + 1][3];
    // Nothing set: the slab is the first hit (about 0.9 away).
    let open = t_at(&base);
    assert!(open > 0.8 && open < 1.2, "{open}");
    // A clip box under the slab: the ray skips it and lands on the floor, about 2 away.
    let clipped = GpuFrame { flags: FRAME_CLIP, clip_min: [-9.0, -9.0, -9.0, 0.0], clip_max: [9.0, 9.0, 0.5, 0.0], ..base };
    let floor = t_at(&clipped);
    assert!(floor > 1.9 && floor < 2.3, "{floor}");
    // A near clip of 1.5 starts the ray below the slab: the floor is then about 0.5 away.
    let near = GpuFrame { view: [0.0, 0.0, -1.0, 1.5], ..base };
    let started = t_at(&near);
    assert!(started > 0.4 && started < 0.8, "{started}");
}

/// Run `trace_main` once over `frame`'s 8 x 8 pixels and read back the AOV buffer.
fn run_with_aov(gpu: &ComputeGpu, scene: &GpuScene, frame: &GpuFrame) -> Vec<[f32; 4]> {
    let d = &gpu.device;
    let buffers = upload_scene(gpu, scene);
    let pipelines = match PtPipelines::new(gpu, None) {
        Ok(p) => p,
        Err(e) => {
            assert!(false, "{e}");
            return Vec::new();
        }
    };
    let count = 64 * 2;
    let bytes = (count * 16) as u64;
    let frame = GpuFrame { light_count: scene.lights.len() as u32, ..*frame };
    let zero = vec![[0.0f32; 4]; count];
    let storage = |label: &str, data: &[u8]| {
        d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: data,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        })
    };
    let frame_buf = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("aov frame"),
        contents: bytemuck::bytes_of(&frame),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let accum = storage("aov accum", bytemuck::cast_slice(&zero));
    let aov = storage("aov aov", bytemuck::cast_slice(&zero));
    let probes = storage("aov probes", bytemuck::bytes_of(&GpuProbe::default()));
    let read = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("aov read"),
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let module =
        d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("aov kernel"), source: wgpu::ShaderSource::Wgsl(kernel_source().into()) });
    let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("aov"),
        bind_group_layouts: &[Some(&pipelines.kernel_layout)],
        immediate_size: 0,
    });
    let pipeline = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("trace_main"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("trace_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("aov"),
        layout: &pipelines.kernel_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: frame_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: buffers.nodes.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: buffers.tris.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: buffers.materials.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 4, resource: buffers.lights.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 5, resource: buffers.ies.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 6, resource: accum.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 7, resource: aov.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 8, resource: probes.as_entire_binding() },
        ],
    });
    let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("aov") });
    {
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("aov"), timestamp_writes: None });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    enc.copy_buffer_to_buffer(&aov, 0, &read, 0, bytes);
    gpu.queue.submit([enc.finish()]);
    let slice = read.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    if let Err(e) = d.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(300)) }) {
        assert!(false, "{e}");
    }
    assert!(matches!(rx.recv(), Ok(Ok(()))), "map failed");
    let out: Vec<[f32; 4]> = match slice.get_mapped_range() {
        Ok(view) => bytemuck::cast_slice(&view).to_vec(),
        Err(e) => {
            assert!(false, "{e}");
            Vec::new()
        }
    };
    read.unmap();
    out
}

// ---- Environment map (p19-env-path) ------------------------------------------------------------

/// A 100 m x 100 m slab under z = 0 (diffuse albedo 0.5 through the furnace materials, no lights).
fn lambert_plane() -> GpuScene {
    let scene = Scene3d { meshes: vec![prism("wall", 1, [-50.0, -50.0, -1.0], [50.0, 50.0, 0.0])], ..Default::default() };
    build_gpu_scene(&scene, false, Some(Furnace { albedo: 0.5, ..Default::default() }))
}

/// `base` with `e` baked into `scene`'s aux buffer (no sun).
fn with_environment(scene: &mut GpuScene, e: crate::environment::Environment, base: GpuFrame) -> GpuFrame {
    let lighting = crate::lit_frame::Lighting { sun: None, environment: Some(std::sync::Arc::new(e)), ..Default::default() };
    let mut frame = base;
    super::tracer::frame_environment(&mut frame, &mut scene.ies, &lighting);
    frame
}

/// Mean red radiance over the 8 x 8 pixels of a camera 5 m above the plane looking down.
fn plane_mean(gpu: &ComputeGpu, scene: &GpuScene, frame: &GpuFrame, spp: u32) -> f64 {
    let out = trace(gpu, scene, frame);
    assert_eq!(out.len(), 64);
    out.iter().map(|a| f64::from(a[0]) / f64::from(spp)).sum::<f64>() / 64.0
}

fn down_frame(bounces: u32, spp: u32) -> GpuFrame {
    GpuFrame { bounces, spp_this_pass: spp, seed: 11, ..camera_frame([0.0, 0.0, 5.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]) }
}

#[test]
fn uniform_environment_lights_a_lambert_plane() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut scene = lambert_plane();
    let frame = with_environment(&mut scene, crate::environment::Environment::uniform([1.0; 3], 16, 8), down_frame(2, 1024));
    assert_eq!(frame.flags & super::scene_gpu::FRAME_ENV, super::scene_gpu::FRAME_ENV);
    let mean = plane_mean(&gpu, &scene, &frame, 1024);
    // L = albedo * E / pi = 0.5 * pi / pi.
    assert!((mean - 0.5).abs() <= 0.005, "plane radiance {mean}");
}

#[test]
fn env_irradiance_up_pi_down_zero() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut scene = lambert_plane();
    let spp = 16_384;
    let frame = with_environment(
        &mut scene,
        crate::environment::Environment::uniform([1.0; 3], 16, 8),
        GpuFrame { probe_count: 2, bounces: 0, spp_this_pass: spp, seed: 5, ..Default::default() },
    );
    let buffers = upload_scene(&gpu, &scene);
    let probes = [up_probe(0.0, 0.0), GpuProbe { point: [0.0, 0.0, 0.0], normal: [0.0, 0.0, -1.0], ..Default::default() }];
    let out = check(run_entry(&gpu, &buffers, "irradiance_main", &frame, &probes));
    let lux = |a: &[f32; 4]| f64::from(luminance([a[0], a[1], a[2]])) / f64::from(spp);
    let (up, down) = (lux(&out[0]), lux(&out[1]));
    assert!((up - std::f64::consts::PI).abs() <= 0.03 * std::f64::consts::PI, "up {up}");
    assert!(down.abs() < 1e-3, "down {down}");
}

/// A map with a bright patch 34 degrees above the horizon on a dim sky.
fn patchy() -> crate::environment::Environment {
    let mut e = crate::environment::Environment::uniform([1.0; 3], 16, 8);
    e.rgb[2 * 16 + 3] = [20.0; 3];
    e
}

#[test]
fn env_mis_matches_bsdf_only() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut scene = lambert_plane();
    let frame = with_environment(&mut scene, patchy(), down_frame(2, 1024));
    let mis = plane_mean(&gpu, &scene, &frame, 1024);
    // No total importance: no light sample, and every escape is the whole BSDF-sampled share.
    let bsdf_only = GpuFrame { env_params: [0.0; 4], ..frame };
    let plain = plane_mean(&gpu, &scene, &bsdf_only, 1024);
    assert!(mis > 0.5 && plain > 0.5, "mis {mis}, bsdf only {plain}");
    assert!((mis - plain).abs() <= 0.02 * plain, "mis {mis}, bsdf only {plain}");
}

/// A 20 x 10 m wall on the plane y = 0 facing -y, UVs in metres (u along x, v up).
fn uv_wall(material: &str) -> Mesh {
    let mut m = Mesh::new(material, ArchId(1));
    for (p, uv) in
        [([0.0, 0.0, 0.0], [0.0, 0.0]), ([20.0, 0.0, 0.0], [20.0, 0.0]), ([20.0, 0.0, 10.0], [20.0, 10.0]), ([0.0, 0.0, 10.0], [0.0, 10.0])]
    {
        m.positions.push(p);
        m.normals.push([0.0, -1.0, 0.0]);
        m.uvs.push(uv);
    }
    m.indices.extend([0, 1, 2, 0, 2, 3]);
    m
}

/// Mean red radiance and every pixel of the wall seen face-on under a sun toward `dir`
/// (no bounces, 16 samples).
fn wall_pixels(gpu: &ComputeGpu, material: &str, dir: [f32; 3]) -> (f64, Vec<f64>) {
    let scene = Scene3d { meshes: vec![uv_wall(material)], ..Default::default() };
    let gpu_scene = build_gpu_scene(&scene, false, None);
    let n = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let mut frame = camera_frame([10.0, -2.0, 5.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    frame.flags |= FRAME_SUN;
    frame.sun_dir = [dir[0] / n, dir[1] / n, dir[2] / n, 0.0];
    frame.sun_rgb = [1.0; 4];
    frame.spp_this_pass = 16;
    frame.bounces = 0;
    frame.seed = 0;
    if gpu_scene.tex != [0; 4] {
        frame.tex = gpu_scene.tex;
        frame.flags |= super::scene_gpu::FRAME_NORMAL_MAPS;
    }
    let px: Vec<f64> = trace(gpu, &gpu_scene, &frame).iter().map(|a| f64::from(a[0]) / 16.0).collect();
    (px.iter().sum::<f64>() / px.len().max(1) as f64, px)
}

#[test]
fn pt_normal_map_differs_under_mirrored_suns() {
    let Some(gpu) = gpu_or_skip() else { return };
    let (east, west) = ([0.6, -0.5, 0.4], [-0.6, -0.5, 0.4]);
    let (ma, a) = wall_pixels(&gpu, "wall-brick", east);
    let (mb, b) = wall_pixels(&gpu, "wall-brick", west);
    let diff = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum::<f64>() / a.len().max(1) as f64;
    assert!(ma > 0.0 && mb > 0.0, "the wall is lit: {ma} {mb}");
    assert!(diff >= 0.01 * ma, "brick relief: mean |A - B| {diff} against mean {ma}");
    assert!((ma - mb).abs() <= 0.03 * ma, "means {ma} {mb}");
    // Without a normal map the mirrored suns read the same.
    let (pa, a) = wall_pixels(&gpu, "wall", east);
    let (pb, b) = wall_pixels(&gpu, "wall", west);
    let flat = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum::<f64>() / a.len().max(1) as f64;
    assert!(flat * 3.0 <= diff, "plain wall differs by {flat} (means {pa} {pb}), the brick by {diff}");
}
