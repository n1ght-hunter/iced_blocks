use std::num::NonZeroU32;

use wgpu::{Device as WgpuDevice, Texture, TextureDescriptor};

use super::BackendImport;
use crate::{ImportError, TextureSource, TextureSourceTypes};

impl BackendImport for wgpu::wgc::api::Gles {
    fn supported_sources() -> TextureSourceTypes {
        TextureSourceTypes::GlesTexture
    }

    unsafe fn import(
        device: &WgpuDevice,
        hal: &Self::Device,
        source: TextureSource<'_>,
        desc: &TextureDescriptor<'_>,
    ) -> Result<Texture, ImportError> {
        match source {
            TextureSource::GlesTexture(tex) => unsafe {
                wrap_gl_texture(device, hal, tex.name, desc)
            },
            _ => Err(ImportError::Unsupported),
        }
    }
}

/// Import a `GlesTexture` via cross-backend GPU blit through `GlInterop`.
///
/// # Safety
///
/// The GL context must be current.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub unsafe fn import_gles_via_blit(
    device: &WgpuDevice,
    tex: &crate::GlesTexture<'_>,
    width: u32,
    height: u32,
) -> Result<Texture, ImportError> {
    let interop = tex.interop.ok_or_else(|| {
        ImportError::Platform("GlInterop required for cross-backend GL texture import".into())
    })?;

    unsafe {
        use glow::HasContext;
        let read_fbo = tex.gl.create_framebuffer().map_err(ImportError::OpenGL)?;
        tex.gl
            .bind_framebuffer(glow::READ_FRAMEBUFFER, Some(read_fbo));
        tex.gl.framebuffer_texture_2d(
            glow::READ_FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(glow::NativeTexture(tex.name)),
            0,
        );

        let result = interop.import(tex.gl, device, Some(read_fbo), width, height);

        tex.gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
        tex.gl.delete_framebuffer(read_fbo);

        result
    }
}

/// Wrap a raw GL texture name as a wgpu texture via the GLES HAL.
///
/// # Safety
///
/// `desc` must accurately describe the GL texture `name`. The wgpu
/// `device` must be using the GLES backend.
pub unsafe fn wrap_gl_texture(
    device: &WgpuDevice,
    hal_device: &<wgpu::wgc::api::Gles as wgpu::hal::Api>::Device,
    name: NonZeroU32,
    desc: &TextureDescriptor<'_>,
) -> Result<Texture, ImportError> {
    unsafe {
        Ok(device.create_texture_from_hal::<wgpu::wgc::api::Gles>(
            hal_device.texture_from_raw(
                name,
                &wgpu::hal::TextureDescriptor {
                    label: None,
                    size: desc.size,
                    format: desc.format,
                    dimension: desc.dimension,
                    mip_level_count: desc.mip_level_count,
                    sample_count: desc.sample_count,
                    usage: super::hal_usage(desc.usage),
                    view_formats: desc.view_formats.to_vec(),
                    memory_flags: wgpu::hal::MemoryFlags::empty(),
                },
                None,
            ),
            desc,
        ))
    }
}
