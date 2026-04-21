pub(crate) mod sysmem;

#[cfg(all(feature = "d3d12", target_os = "windows"))]
pub(crate) mod d3d12;
#[cfg(feature = "gl")]
pub(crate) mod gl;
#[cfg(feature = "gl")]
pub(crate) mod gl_context;
#[cfg(feature = "vulkan")]
pub(crate) mod vulkan;

use std::any::Any;

use crate::api::WgpuDeviceHandle;
use crate::error::SinkError;
use crate::sink::frame::PooledTexture;

pub(crate) type ImportResult = Result<(PooledTexture, Option<Box<dyn Any + Send>>), SinkError>;

/// Trait for each memory-type backend.
pub(crate) trait Backend: Send + Sync {
    /// Try to import a frame from this GstBuffer.
    ///
    /// Returns `None` if this backend doesn't handle the buffer's memory type.
    fn try_import(
        &self,
        buffer: &gst::Buffer,
        video_info: &gst_video::VideoInfo,
        device: &WgpuDeviceHandle,
    ) -> Option<ImportResult>;
}

/// Map GStreamer video format to wgpu texture format.
pub(crate) fn gst_to_wgpu_format(format: gst_video::VideoFormat) -> Option<wgpu::TextureFormat> {
    match format {
        gst_video::VideoFormat::Rgba | gst_video::VideoFormat::Rgbx => {
            Some(wgpu::TextureFormat::Rgba8Unorm)
        }
        gst_video::VideoFormat::Bgra | gst_video::VideoFormat::Bgrx => {
            Some(wgpu::TextureFormat::Bgra8Unorm)
        }
        gst_video::VideoFormat::Rgb10a2Le => Some(wgpu::TextureFormat::Rgb10a2Unorm),
        gst_video::VideoFormat::Gray8 => Some(wgpu::TextureFormat::R8Unorm),
        gst_video::VideoFormat::Gray16Le => Some(wgpu::TextureFormat::R16Unorm),
        gst_video::VideoFormat::Nv12 => Some(wgpu::TextureFormat::NV12),
        gst_video::VideoFormat::P01010le => Some(wgpu::TextureFormat::P010),
        _ => None,
    }
}

/// Select the appropriate backend based on negotiated caps features.
pub(crate) fn select_backend(
    caps: &gst::Caps,
    device: &WgpuDeviceHandle,
    imp: &crate::sink::imp::WgpuVideoSinkImp,
) -> Box<dyn Backend> {
    let features = caps.features(0);

    #[allow(unused_variables)]
    if let Some(f) = features {
        #[cfg(all(feature = "d3d12", target_os = "windows"))]
        if f.contains("memory:D3D12Memory") {
            tracing::info!("wgpusink: selected d3d12 backend");
            return Box::new(d3d12::D3D12Backend);
        }

        #[cfg(feature = "vulkan")]
        if f.contains("memory:VulkanImage") {
            tracing::info!("wgpusink: selected vulkan backend");
            return Box::new(vulkan::VulkanBackend);
        }

        #[cfg(feature = "gl")]
        if f.contains("memory:GLMemory") {
            if let Some(bundle) = imp.gl_bundle.lock().unwrap().clone() {
                tracing::info!("wgpusink: selected gl backend");
                return Box::new(gl::GlBackend::new(bundle));
            }
            tracing::warn!(
                "memory:GLMemory negotiated but no GL context bundle is available — \
                 falling back to sysmem"
            );
        }
    }

    tracing::info!("wgpusink: selected sysmem backend");
    let _ = (device, imp);
    Box::new(sysmem::SysmemBackend::new())
}
