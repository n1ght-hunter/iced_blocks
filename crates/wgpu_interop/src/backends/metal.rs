use wgpu::{Device as WgpuDevice, Texture, TextureDescriptor};

use super::BackendImport;
use crate::sources::metal::create_metal_texture_from_iosurface;
use crate::{ImportError, TextureSource, TextureSourceTypes};

impl BackendImport for wgpu::wgc::api::Metal {
    fn supported_sources() -> TextureSourceTypes {
        TextureSourceTypes::MetalTexture
            | TextureSourceTypes::IOSurfaceTexture
            | TextureSourceTypes::GlesTexture
    }

    unsafe fn import(
        device: &WgpuDevice,
        hal: &Self::Device,
        source: TextureSource<'_>,
        desc: &TextureDescriptor<'_>,
    ) -> Result<Texture, ImportError> {
        match source {
            TextureSource::MetalTexture(m) => unsafe {
                wrap_metal_texture(device, m.texture, desc)
            },
            TextureSource::IOSurfaceTexture(s) => unsafe {
                wrap_iosurface(device, hal, &s.surface, s.plane, desc)
            },
            TextureSource::GlesTexture(tex) => unsafe {
                import_gles_via_metal_interop(device, &tex, desc.size.width, desc.size.height)
            },
            _ => Err(ImportError::Unsupported),
        }
    }
}

/// Wrap an `IOSurface` as a wgpu texture on the Metal backend.
///
/// # Safety
///
/// `desc` must accurately describe the IOSurface's format and size.
/// The wgpu `device` must be using the Metal backend.
unsafe fn wrap_iosurface(
    device: &WgpuDevice,
    hal: &<wgpu::wgc::api::Metal as wgpu::hal::Api>::Device,
    surface: &objc2_io_surface::IOSurfaceRef,
    plane: u32,
    desc: &TextureDescriptor<'_>,
) -> Result<Texture, ImportError> {
    let metal_device = hal.raw_device().lock().clone();
    let texture = unsafe {
        create_metal_texture_from_iosurface(
            &metal_device,
            surface,
            plane,
            desc.size.width,
            desc.size.height,
        )?
    };
    unsafe { wrap_metal_texture(device, texture, desc) }
}

/// Import a `GlesTexture` into a Metal-backed wgpu texture via
/// `MetalInterop`'s IOSurface blit path.
///
/// # Safety
///
/// The GL context bound to the `GlesTexture`'s glow reference and
/// to the provided `MetalInterop`'s CGL context must be current.
unsafe fn import_gles_via_metal_interop(
    device: &WgpuDevice,
    tex: &crate::GlesTexture<'_>,
    width: u32,
    height: u32,
) -> Result<Texture, ImportError> {
    let interop = tex.interop.ok_or_else(|| {
        ImportError::Platform(
            "MetalInterop required for GlesTexture import on Metal backend".into(),
        )
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

/// Wrap a `metal::Texture` as a wgpu texture via the Metal HAL.
///
/// # Safety
///
/// `desc` must accurately describe `texture` (format, dimensions,
/// mip levels, array layers). The wgpu `device` must be using the
/// Metal backend, and `texture` must come from the same
/// `metal::Device` that backs the wgpu `Device`.
pub unsafe fn wrap_metal_texture(
    device: &WgpuDevice,
    texture: metal::Texture,
    desc: &TextureDescriptor<'_>,
) -> Result<Texture, ImportError> {
    let raw_type = match desc.dimension {
        wgpu::TextureDimension::D1 => metal::MTLTextureType::D1,
        wgpu::TextureDimension::D2 => {
            if desc.sample_count > 1 {
                metal::MTLTextureType::D2Multisample
            } else if desc.size.depth_or_array_layers > 1 {
                metal::MTLTextureType::D2Array
            } else {
                metal::MTLTextureType::D2
            }
        }
        wgpu::TextureDimension::D3 => metal::MTLTextureType::D3,
    };

    let array_layers = match desc.dimension {
        wgpu::TextureDimension::D3 => 1,
        _ => desc.size.depth_or_array_layers,
    };

    let depth = match desc.dimension {
        wgpu::TextureDimension::D3 => desc.size.depth_or_array_layers,
        _ => 1,
    };

    unsafe {
        let hal_texture = wgpu::hal::metal::Device::texture_from_raw(
            texture,
            desc.format,
            raw_type,
            array_layers,
            desc.mip_level_count,
            wgpu::hal::CopyExtent {
                width: desc.size.width,
                height: desc.size.height,
                depth,
            },
        );
        Ok(device.create_texture_from_hal::<wgpu::wgc::api::Metal>(hal_texture, desc))
    }
}
