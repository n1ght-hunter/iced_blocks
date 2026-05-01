use std::num::NonZeroU32;
use std::sync::Arc;

use wgpu_interop::{DeviceInterop, GlesTexture};

use super::gl_context::GlContextBundle;
use super::{Backend, ImportResult, SinkError, gst_to_wgpu_format};
use crate::api::WgpuDeviceHandle;
use crate::sink::frame::PooledTexture;

pub(crate) struct GlBackend {
    bundle: Arc<GlContextBundle>,
}

impl GlBackend {
    pub(crate) fn new(bundle: Arc<GlContextBundle>) -> Self {
        Self { bundle }
    }
}

impl Backend for GlBackend {
    fn try_import(
        &self,
        buffer: &gst::Buffer,
        video_info: &gst_video::VideoInfo,
        device: &WgpuDeviceHandle,
    ) -> Option<ImportResult> {
        let mem = buffer.memory(0)?;
        if !mem.is_memory_type::<gst_gl::GLMemory>() {
            return None;
        }
        let gl_mem = mem.downcast_memory_ref::<gst_gl::GLMemory>()?;
        Some(import_gl(gl_mem, video_info, device, &self.bundle))
    }
}

fn import_gl(
    gl_mem: &gst_gl::GLMemoryRef,
    video_info: &gst_video::VideoInfo,
    device: &WgpuDeviceHandle,
    bundle: &GlContextBundle,
) -> ImportResult {
    let format = gst_to_wgpu_format(video_info.format())
        .ok_or_else(|| SinkError::UnsupportedMemory(format!("{:?}", video_info.format())))?;

    let tex_id = gl_mem.texture_id();
    let name = NonZeroU32::new(tex_id)
        .ok_or_else(|| SinkError::Import("GLMemory texture id is zero".into()))?;

    let desc = wgpu::TextureDescriptor {
        label: Some("wgpusink_gl"),
        size: wgpu::Extent3d {
            width: video_info.width(),
            height: video_info.height(),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    };

    let gles_tex = GlesTexture {
        gl: &bundle.glow,
        name,
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        interop: Some(&bundle.gl_interop),
    };

    let texture = unsafe {
        device
            .device
            .import_external_texture(gles_tex, &desc)
            .map_err(|e| SinkError::Import(format!("GL texture import failed: {e}")))?
    };

    Ok((PooledTexture::unmanaged(texture), None))
}
