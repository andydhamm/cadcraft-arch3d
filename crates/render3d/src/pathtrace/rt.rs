//! Ray-query acceleration structure build (BLAS + TLAS) for the [`PathOptions::rt`] traversal.

use super::device::ComputeGpu;
use super::scene_gpu::GpuScene;

use wgpu::util::DeviceExt;

/// A BLAS of every triangle plus a single-instance TLAS over it.
pub struct RqBuild {
    /// The TLAS (with one instance into the BLAS) that the rq pipelines bind.
    pub tlas: wgpu::Tlas,
    /// The BLAS the TLAS instance points at (kept alive for the render's lifetime).
    pub blas: wgpu::Blas,
    /// The vertex buffer holding the triangle vertices (kept alive).
    pub vertex_buffer: wgpu::Buffer,
}

impl RqBuild {
    /// Build a BLAS of every triangle in `scene` (primitive index = triangle index) and a TLAS with
    /// one instance. Returns `None` when the scene has no triangles or a vertex is not finite.
    pub fn build(gpu: &ComputeGpu, scene: &GpuScene) -> Option<RqBuild> {
        if scene.tris.is_empty() {
            return None;
        }
        let d = &gpu.device;

        // Three vertices per triangle (v0, v0 + e1, v0 + e2), unindexed.
        let mut positions: Vec<f32> = Vec::with_capacity(scene.tris.len().checked_mul(9)?);
        for tri in &scene.tris {
            for k in 0..3 {
                for a in 0..3 {
                    let v = match k {
                        0 => tri.v0[a],
                        1 => tri.v0[a] + tri.e1[a],
                        _ => tri.v0[a] + tri.e2[a],
                    };
                    if !v.is_finite() {
                        return None;
                    }
                    positions.push(v);
                }
            }
        }
        let vertex_count = u32::try_from(scene.tris.len().checked_mul(3)?).ok()?;

        let vertex_bytes = bytemuck::cast_slice(&positions);
        let vertex_size = ((vertex_bytes.len() + 255) / 256) * 256;
        let padded = if vertex_bytes.len() % 256 == 0 {
            vertex_bytes.to_vec()
        } else {
            let mut v = vec![0u8; vertex_size];
            v[..vertex_bytes.len()].copy_from_slice(vertex_bytes);
            v
        };
        let vertex_buffer = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pt rq verts"),
            size: vertex_size.max(256) as u64,
            usage: wgpu::BufferUsages::BLAS_INPUT | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&vertex_buffer, 0, &padded);

        let size = wgpu::BlasTriangleGeometrySizeDescriptor {
            vertex_format: wgpu::VertexFormat::Float32x3,
            vertex_count,
            index_format: None,
            index_count: None,
            flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
        };
        let blas = d.create_blas(
            &wgpu::CreateBlasDescriptor {
                label: Some("pt rq blas"),
                flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
                update_mode: wgpu::AccelerationStructureUpdateMode::Build,
            },
            wgpu::BlasGeometrySizeDescriptors::Triangles { descriptors: vec![size.clone()] },
        );
        let mut tlas = d.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some("pt rq tlas"),
            max_instances: 1,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        });
        *tlas.get_mut_single(0)? = Some(wgpu::TlasInstance::new(&blas, [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0], 0, 0xff));

        let entry = wgpu::BlasBuildEntry {
            blas: &blas,
            geometry: wgpu::BlasGeometries::TriangleGeometries(vec![wgpu::BlasTriangleGeometry {
                size: &size,
                vertex_buffer: &vertex_buffer,
                first_vertex: 0,
                vertex_stride: 12,
                index_buffer: None,
                first_index: None,
                transform_buffer: None,
                transform_buffer_offset: None,
            }]),
        };
        let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pt rq build") });
        enc.build_acceleration_structures([&entry], [&tlas]);
        gpu.queue.submit([enc.finish()]);

        Some(RqBuild { tlas, blas, vertex_buffer })
    }
}

#[cfg(test)]
mod tests {
    use super::super::scene_gpu::GpuTri;
    use super::*;
    use crate::gputest::shared_compute;

    #[test]
    fn rq_build_returns_none_for_empty_scene() {
        let Some(gpu) = shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        let scene = GpuScene::default();
        assert!(RqBuild::build(&gpu, &scene).is_none(), "empty scene should not build a rq structure");
    }

    #[test]
    fn rq_build_returns_none_for_non_finite_vertices() {
        let Some(gpu) = shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        let mut scene = GpuScene::default();
        scene.tris.push(GpuTri { v0: [0.0, 0.0, 0.0], e1: [1.0, 0.0, 0.0], e2: [0.0, f32::NAN, 0.0], ..Default::default() });
        assert!(RqBuild::build(&gpu, &scene).is_none(), "non-finite vertex should not build a rq structure");
    }

    #[test]
    fn rq_build_succeeds_for_single_triangle() {
        let Some(gpu) = shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        if !gpu.ray_query {
            eprintln!("SKIP: no ray queries on this device");
            return;
        }
        let mut scene = GpuScene::default();
        scene.tris.push(GpuTri { v0: [0.0, 0.0, 0.0], e1: [1.0, 0.0, 0.0], e2: [0.0, 1.0, 0.0], ..Default::default() });
        assert!(RqBuild::build(&gpu, &scene).is_some(), "single triangle should build a rq structure");
    }
}
