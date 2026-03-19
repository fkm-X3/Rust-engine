use ash::vk;

/// A single vertex in our mesh format.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal:   [f32; 3],
    pub uv:       [f32; 2],
    pub tangent:  [f32; 4],
}

/// Renderable mesh. Holds GPU-side vertex and index buffers.
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices:  Vec<u32>,
    vertex_buffer: vk::Buffer,
    vertex_memory: vk::DeviceMemory,
    index_buffer:  vk::Buffer,
    index_memory:  vk::DeviceMemory,
}

impl Mesh {
    /// Create a mesh that owns its GPU buffers.
    /// In production use gpu-allocator; this is simplified.
    pub fn new_placeholder(
        vertices: Vec<Vertex>,
        indices:  Vec<u32>,
    ) -> Self {
        Self {
            vertices,
            indices,
            vertex_buffer: vk::Buffer::null(),
            vertex_memory: vk::DeviceMemory::null(),
            index_buffer:  vk::Buffer::null(),
            index_memory:  vk::DeviceMemory::null(),
        }
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

    pub fn vertex_buffer(&self) -> vk::Buffer { self.vertex_buffer }
    pub fn index_buffer(&self)  -> vk::Buffer { self.index_buffer }
    pub fn index_count(&self)   -> u32        { self.indices.len() as u32 }
}
