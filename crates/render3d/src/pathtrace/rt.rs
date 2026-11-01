//! Ray-query acceleration structure build (BLAS + TLAS) for the [`PathOptions::rt`] traversal.

use super::device::ComputeGpu;
use super::scene_gpu::GpuScene;

/// A BLAS of every triangle plus a single-instance TLAS over it.
pub struct RqBuild {
    /// The TLAS (with one instance into the BLAS) that the rq pipelines bind.
    pub tlas: wgpu::AccelerationStructure,
    /// The BLAS (needed only to keep the TLAS's parent alive for the render's lifetime).
    pub blas: wgpu::AccelerationStructure,
    /// The vertex buffer holding the triangle vertices (owned by the TLAS build; kept alive).
    pub vertex_buffer: wgpu::Buffer,
    /// The index buffer holding the triangle indices (owned by the TLAS build; kept alive).
    pub index_buffer: wgpu::Buffer,
}

/// A raw triangle with its three vertices, used to fill the BLAS vertex buffer.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TriVerts {
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
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

        // Extract the three vertices from each triangle (first 36 bytes of each GpuTri).
        let mut positions: Vec<f32> = Vec::with_capacity(scene.tris.len() * 9);
        for tri in &scene.tris {
            if tri.len() < 36 {
                return None;
            }
            let v = bytemuck::cast_slice::<TriVerts>(&tri[0..36])[0];
            positions.extend_from_slice(&v.a);
            positions.extend_from_slice(&v.b);
            positions.extend_from_slice(&v.c);
        }
        if positions.len() != scene.tris.len() * 9 {
            return None;
        }

        // Triangle indices: 0,1,2 per triangle.
        let indices: Vec<u32> = (0..scene.tris.len())
            .flat_map(|i| [i * 3, i * 3 + 1, i * 3 + 2])
            .collect();

        // Compute the scene AABB from the vertices.
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        for i in 0..positions.len() step_by(3) {
            for axis in 0..3 {
                let v = positions[i + axis];
                if !v.is_finite() {
                    return None;
                }
                let idx = axis % 3;
                let (mn, mx) = match idx {
                    0 => &mut (min_x, max_x),
                    1 => &mut (min_y, max_y),
                    _ => &mut (min_z, max_z),
                };
                *mn = (*mn).min(v);
                *mx = (*mx).max(v);
            }
        }
        if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite()
            || !min_z.is_finite() || !max_z.is_finite()
        {
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

        let blas = match d.create_acceleration_structure(&wgpu::AccelerationStructureDescriptor {
            buffer: vertex_buffer.clone(),
            index: Some(index_buffer.clone()),
            index_type: wgpu::IndexType::U32,
            base_instance: 0,
            instance_count: 1,
            max_triangle_count: scene.tris.len() as u32,
            bounds: wgpu::AccelerationStructureBounds {
                min: [min_x, min_y, min_z],
                max: [max_x, max_y, max_z],
            },
            flags: 0,
        }) {
            Some(b) => b,
            None => return None,
        };

        let tlas = match d.create_tlase(&wgpu::AccelerationStructureDescriptor {
            blas: blas.clone(),
            buffer: wgpu::Buffer::none(),
            index: None,
            index_type: wgpu::IndexType::U32,
            base_instance: 0,
            instance_count: 1,
            max_triangle_count: scene.tris.len() as u32,
            bounds: wgpu::AccelerationStructureBounds {
                min: [min_x, min_y, min_z],
                max: [max_x, max_y, max_z],
            },
            flags: 0,
        }) {
            Some(t) => t,
            None => return None,
        };

        Some(RqBuild {
            tlas,
            blas,
            vertex_buffer,
            index_buffer,
        })
    }
}
