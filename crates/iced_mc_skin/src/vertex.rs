//! Vertex layout for the skin mesh.

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub normal: [f32; 3],
}

impl Vertex {
    pub const ATTRIBS: [iced::wgpu::VertexAttribute; 3] = iced::wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x2,
        2 => Float32x3,
    ];

    pub fn desc() -> iced::wgpu::VertexBufferLayout<'static> {
        iced::wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as iced::wgpu::BufferAddress,
            step_mode: iced::wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}
