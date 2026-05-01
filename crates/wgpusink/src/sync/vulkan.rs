use super::GpuSync;
use crate::error::SinkError;

/// Marker held by `FrameGuard` for Vulkan-backed frames.
///
/// Sync currently happens at import time via `vkDeviceWaitIdle` (see
/// `backend/vulkan.rs`). This struct is the placeholder for a future
/// timeline-semaphore wait once `gst-vulkan` exposes the semaphore API.
pub(crate) struct VulkanSemaphoreSync;

impl GpuSync for VulkanSemaphoreSync {
    fn wait_before_sample(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<(), SinkError> {
        Ok(())
    }
}
