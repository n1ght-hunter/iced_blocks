#[cfg(all(feature = "d3d12", target_os = "windows"))]
pub(crate) mod d3d12;
#[cfg(feature = "vulkan")]
pub(crate) mod vulkan;

use crate::error::SinkError;

/// Wait for a GPU sync primitive before sampling an imported texture.
#[expect(
    dead_code,
    reason = "trait will be used when backends call wait_before_sample"
)]
pub(crate) trait GpuSync: Send + Sync {
    /// Insert a GPU-side wait on the wgpu device's command queue.
    /// This does NOT block the CPU.
    fn wait_before_sample(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), SinkError>;
}
