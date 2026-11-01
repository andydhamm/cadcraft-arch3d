//! Ray-query acceleration structure build (BLAS + TLAS) for the [`PathOptions::rt`] traversal.

use super::device::ComputeGpu;
use super::scene_gpu::GpuScene;

use wgpu::api::blas::{AccelerationStructure, AccelerationStructureBounds, AccelerationStructureDescriptor, IndexType};

/// A BLAS of every triangle plus a single-instance TLAS over it.
pub struct RqBuild {
    /// The TLAS (with one instance into the BLAS) that the rq pipelines bind.
    pub tlas: AccelerationStructure,
    /// The BLAS (needed only to keep the TLAS's parent alive for the render's lifetime).
    pub blas: AccelerationStructure,
    /// The vertex buffer holding the triangle vertices (owned by the TLAS build; kept alive).
    pub vertex_buffer: wgpu::Buffer,
    /// The index buffer holding the triangle indices (owned by the TLAS build; kept alive).
    pub index_buffer: wgpu::Buffer,
}

impl RqBuild {
    /// Build a BLAS of every triangle in `scene` and a TLAS with one instance. Returns `None` when
    /// the scene has no triangles (nothing to accelerate) or the device cannot create the
    /// acceleration structure.
    pub fn build(gpu: &ComputeGpu, scene: &GpuScene) -> Option<RqBuild> {
        if scene.tris.is_empty() {
            return None;
        }
        let d = &gpu.device;

        // Extract the three vertices from each triangle (first 36 bytes of each 64-byte GpuTri).
        let n = scene.tris.len();
        let mut positions: Vec<f32> = Vec::with_capacity(n * 9);
        for tri in &scene.tris {
            let tri_bytes: [u8; 64] = bytemuck::cast(tri);
            let verts: [f32; 9] = bytemuck::cast(&tri_bytes[0..36]);
            for v in &verts {
                if !v.is_finite() {
                    return None;
                }
                positions.push(*v);
            }
        }

        // Triangle indices: 0,1,2 per triangle.
        let indices: Vec<u32> = (0..n as u32).flat_map(|i| [i * 3, i * 3 + 1, i * 3 + 2]).collect();

        // Compute the scene AABB from the vertices.
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        for i in (0..positions.len()).step_by(3) {
            for axis in 0..3 {
                let v = positions[i + axis];
                match axis {
                    0 => {
                        min_x = min_x.min(v);
                        max_x = max_x.max(v);
                    }
                    1 => {
                        min_y = min_y.min(v);
                        max_y = max_y.max(v);
                    }
                    _ => {
                        min_z = min_z.min(v);
                        max_z = max_z.max(v);
                    }
                }
            }
        }
        if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() || !min_z.is_finite() || !max_z.is_finite() {
            return None;
        }

        let vertex_buffer = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pt rq verts"),
            contents: bytemuck::cast_slice(&positions),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let index_buffer = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pt rq indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let blas = match d.create_acceleration_structure(&AccelerationStructureDescriptor {
            buffer: vertex_buffer.clone(),
            index: Some(index_buffer.clone()),
            index_type: IndexType::U32,
            base_instance: 0,
            instance_count: 1,
            max_triangle_count: n as u32,
            bounds: AccelerationStructureBounds { min: [min_x, min_y, min_z], max: [max_x, max_y, max_z] },
            flags: 0,
        }) {
            Some(b) => b,
            None => return None,
        };

        let tlas = match d.create_tlase(&AccelerationStructureDescriptor {
            blas: blas.clone(),
            buffer: wgpu::Buffer::none(),
            index: None,
            index_type: IndexType::U32,
            base_instance: 0,
            instance_count: 1,
            max_triangle_count: n as u32,
            bounds: AccelerationStructureBounds { min: [min_x, min_y, min_z], max: [max_x, max_y, max_z] },
            flags: 0,
        }) {
            Some(t) => t,
            None => return None,
        };

        Some(RqBuild { tlas, blas, vertex_buffer, index_buffer })
    }
}
