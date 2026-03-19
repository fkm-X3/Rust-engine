use ash::vk;
use anyhow::Result;
use std::cell::UnsafeCell;
use std::sync::Arc;

use super::{context::VulkanContext, memory::BufferUploader};

/// A single vertex in our mesh format.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal:   [f32; 3],
    pub uv:       [f32; 2],
    pub tangent:  [f32; 4],
}

/// GPU buffer data for a mesh (separated for interior mutability).
struct GpuMeshData {
    vertex_buffer: vk::Buffer,
    vertex_memory: vk::DeviceMemory,
    index_buffer:  vk::Buffer,
    index_memory:  vk::DeviceMemory,
}

/// Renderable mesh. Holds GPU-side vertex and index buffers.
/// Uses interior mutability to allow lazy GPU upload even when stored by value in ECS.
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices:  Vec<u32>,
    gpu_data: UnsafeCell<Option<GpuMeshData>>,
}

// Safety: Mesh is Send+Sync because we control access to gpu_data through ensure_uploaded
unsafe impl Send for Mesh {}
unsafe impl Sync for Mesh {}

impl Mesh {
    /// Create a mesh with CPU-side data only (GPU upload happens later).
    pub fn new_placeholder(
        vertices: Vec<Vertex>,
        indices:  Vec<u32>,
    ) -> Self {
        Self {
            vertices,
            indices,
            gpu_data: UnsafeCell::new(None),
        }
    }

    /// Upload mesh data to GPU if not already uploaded.
    /// This uses interior mutability to allow upload even when mesh is borrowed immutably.
    /// Safe because upload is idempotent and only happens once.
    pub fn ensure_uploaded(&self, context: &Arc<VulkanContext>) -> Result<()> {
        // Safety: We check if data exists before uploading, making this effectively idempotent
        let gpu_data = unsafe { &mut *self.gpu_data.get() };
        
        if gpu_data.is_some() {
            return Ok(());
        }

        let uploader = BufferUploader::new(context.clone())?;

        // Upload vertex buffer
        let (vb, vm) = uploader.upload_buffer(&self.vertices, vk::BufferUsageFlags::VERTEX_BUFFER)?;

        // Upload index buffer
        let (ib, im) = uploader.upload_buffer(&self.indices, vk::BufferUsageFlags::INDEX_BUFFER)?;

        *gpu_data = Some(GpuMeshData {
            vertex_buffer: vb,
            vertex_memory: vm,
            index_buffer: ib,
            index_memory: im,
        });

        Ok(())
    }

    /// Check if this mesh has been uploaded to GPU.
    pub fn is_uploaded(&self) -> bool {
        unsafe { (*self.gpu_data.get()).is_some() }
    }

    /// Create a unit cube mesh.
    pub fn cube() -> Self {
        let v = |p: [f32; 3], n: [f32; 3], uv: [f32; 2]| Vertex {
            position: p, normal: n, uv, tangent: [1.0, 0.0, 0.0, 1.0],
        };

        let vertices = vec![
            // Front face
            v([-0.5, -0.5,  0.5], [0.0, 0.0, 1.0], [0.0, 1.0]),
            v([ 0.5, -0.5,  0.5], [0.0, 0.0, 1.0], [1.0, 1.0]),
            v([ 0.5,  0.5,  0.5], [0.0, 0.0, 1.0], [1.0, 0.0]),
            v([-0.5,  0.5,  0.5], [0.0, 0.0, 1.0], [0.0, 0.0]),
            // Back face
            v([ 0.5, -0.5, -0.5], [0.0, 0.0, -1.0], [0.0, 1.0]),
            v([-0.5, -0.5, -0.5], [0.0, 0.0, -1.0], [1.0, 1.0]),
            v([-0.5,  0.5, -0.5], [0.0, 0.0, -1.0], [1.0, 0.0]),
            v([ 0.5,  0.5, -0.5], [0.0, 0.0, -1.0], [0.0, 0.0]),
        ];

        let indices = vec![
            0, 1, 2,  2, 3, 0, // front
            4, 5, 6,  6, 7, 4, // back
            5, 0, 3,  3, 6, 5, // left
            1, 4, 7,  7, 2, 1, // right
            3, 2, 7,  7, 6, 3, // top
            5, 4, 1,  1, 0, 5, // bottom
        ];

        Self::new_placeholder(vertices, indices)
    }

    /// Create a UV sphere mesh with configurable subdivisions.
    pub fn sphere(segments: u32, rings: u32) -> Self {
        use std::f32::consts::PI;
        
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Generate vertices
        for ring in 0..=rings {
            let phi = PI * (ring as f32) / (rings as f32);
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();

            for segment in 0..=segments {
                let theta = 2.0 * PI * (segment as f32) / (segments as f32);
                let sin_theta = theta.sin();
                let cos_theta = theta.cos();

                let x = sin_phi * cos_theta;
                let y = cos_phi;
                let z = sin_phi * sin_theta;

                let u = (segment as f32) / (segments as f32);
                let v = (ring as f32) / (rings as f32);

                // Normal for sphere is just normalized position
                let normal = [x, y, z];
                
                // Calculate tangent (perpendicular to normal in horizontal plane)
                let tangent = [-sin_theta, 0.0, cos_theta, 1.0];

                vertices.push(Vertex {
                    position: [x * 0.5, y * 0.5, z * 0.5],
                    normal,
                    uv: [u, v],
                    tangent,
                });
            }
        }

        // Generate indices
        for ring in 0..rings {
            for segment in 0..segments {
                let current = ring * (segments + 1) + segment;
                let next = current + segments + 1;

                // Two triangles per quad
                indices.push(current);
                indices.push(next);
                indices.push(current + 1);

                indices.push(current + 1);
                indices.push(next);
                indices.push(next + 1);
            }
        }

        Self::new_placeholder(vertices, indices)
    }

    pub fn vertex_buffer(&self) -> vk::Buffer { 
        unsafe { 
            (*self.gpu_data.get())
                .as_ref()
                .map(|d| d.vertex_buffer)
                .unwrap_or(vk::Buffer::null())
        }
    }
    
    pub fn index_buffer(&self) -> vk::Buffer {
        unsafe { 
            (*self.gpu_data.get())
                .as_ref()
                .map(|d| d.index_buffer)
                .unwrap_or(vk::Buffer::null())
        }
    }
    
    pub fn index_count(&self) -> u32 {
        self.indices.len() as u32
    }
}

// Note: Drop implementation cannot be provided here because Mesh doesn't own VulkanContext.
// GPU resources should be cleaned up by a ResourceManager or during renderer shutdown.
