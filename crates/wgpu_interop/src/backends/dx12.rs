use wgpu::{Device as WgpuDevice, Texture, TextureDescriptor};
use windows::Win32::Graphics::Direct3D12;

use super::BackendImport;
use crate::{ImportError, TextureSource, TextureSourceTypes};

impl BackendImport for wgpu::wgc::api::Dx12 {
    fn supported_sources() -> TextureSourceTypes {
        let mut types = TextureSourceTypes::D3D12Resource
            | TextureSourceTypes::D3D11SharedHandle
            | TextureSourceTypes::VulkanImage;
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            types |= TextureSourceTypes::GlesTexture;
        }
        types
    }

    unsafe fn import(
        device: &WgpuDevice,
        hal: &Self::Device,
        source: TextureSource<'_>,
        desc: &TextureDescriptor<'_>,
    ) -> Result<Texture, ImportError> {
        match source {
            TextureSource::D3D12Resource(res) => unsafe {
                wrap_resource(device, res.resource, desc)
            },
            TextureSource::D3D11SharedHandle(h) => unsafe {
                wrap_shared_handle(device, hal, h.handle, desc)
            },
            TextureSource::VulkanImage(v) => unsafe {
                wrap_shared_handle(device, hal, v.handle, desc)
            },
            TextureSource::GlesTexture(tex) => {
                #[cfg(any(target_os = "windows", target_os = "linux"))]
                {
                    unsafe {
                        super::gles::import_gles_via_blit(
                            device,
                            &tex,
                            desc.size.width,
                            desc.size.height,
                        )
                    }
                }
                #[cfg(not(any(target_os = "windows", target_os = "linux")))]
                {
                    let _ = tex;
                    Err(ImportError::Unsupported)
                }
            }
        }
    }
}

/// Wrap a raw `ID3D12Resource` as a wgpu texture via the D3D12 HAL.
///
/// # Safety
///
/// `desc` must accurately describe `resource`. The wgpu `device`
/// must be using the D3D12 backend.
pub unsafe fn wrap_resource(
    device: &WgpuDevice,
    resource: Direct3D12::ID3D12Resource,
    desc: &TextureDescriptor<'_>,
) -> Result<Texture, ImportError> {
    unsafe {
        Ok(device.create_texture_from_hal::<wgpu::wgc::api::Dx12>(
            <wgpu::wgc::api::Dx12 as wgpu::hal::Api>::Device::texture_from_raw(
                resource,
                desc.format,
                desc.dimension,
                desc.size,
                desc.mip_level_count,
                desc.sample_count,
            ),
            desc,
        ))
    }
}

/// Open a shared NTHANDLE as a D3D12 resource, then wrap it.
///
/// # Safety
///
/// `desc` must accurately describe the resource behind `handle`.
pub unsafe fn wrap_shared_handle(
    device: &WgpuDevice,
    hal: &<wgpu::wgc::api::Dx12 as wgpu::hal::Api>::Device,
    handle: windows::Win32::Foundation::HANDLE,
    desc: &TextureDescriptor<'_>,
) -> Result<Texture, ImportError> {
    unsafe {
        let dx12_device = hal.raw_device();

        let mut resource_ptr: Option<Direct3D12::ID3D12Resource> = None;
        dx12_device
            .OpenSharedHandle(handle, &mut resource_ptr)
            .map_err(|e| ImportError::Platform(format!("OpenSharedHandle: {e}")))?;
        let resource = resource_ptr
            .ok_or_else(|| ImportError::Platform("OpenSharedHandle returned null".into()))?;

        wrap_resource(device, resource, desc)
    }
}
