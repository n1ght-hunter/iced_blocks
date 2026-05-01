use windows::Win32::Graphics::Direct3D12::ID3D12Fence;

use super::GpuSync;
use crate::error::SinkError;

/// Holds a D3D12 fence + value extracted from GstD3D12Memory.
///
/// Kept alive via `FrameGuard._sync` to prevent the buffer from being
/// recycled before the GPU finishes reading the texture.
#[expect(dead_code, reason = "fields keep the fence alive until frame drop")]
pub(crate) struct D3D12FenceSync {
    pub fence: ID3D12Fence,
    pub value: u64,
}

impl GpuSync for D3D12FenceSync {
    fn wait_before_sample(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<(), SinkError> {
        // Sync is handled by gst_d3d12_memory_sync() in try_import before
        // wrapping the resource. This struct keeps the fence alive until the
        // frame is dropped.
        Ok(())
    }
}
