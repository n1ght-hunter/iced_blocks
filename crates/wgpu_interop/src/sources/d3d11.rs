use std::cell::RefCell;

use wgpu::{Device as WgpuDevice, Texture, TextureFormat};
use windows::{
    Win32::{
        Foundation,
        Graphics::{
            Direct3D11::{self, ID3D11Device, ID3D11Texture2D},
            Dxgi::{self, Common},
        },
    },
    core::{self, Interface},
};

use crate::{ImportError, backends};

/// One-shot import from a D3D11 shared NTHANDLE.
///
/// The handle must have been created with
/// `IDXGIResource1::CreateSharedHandle` and `DXGI_SHARED_RESOURCE_READ`
/// access. The caller is responsible for closing the handle after import.
pub struct D3D11SharedHandle {
    pub handle: Foundation::HANDLE,
}

/// Persistent interop context for a D3D11 device.
///
/// Manages a shared D3D11↔D3D12 texture that is recreated on resize.
pub struct D3D11Interop {
    d3d11_device: ID3D11Device,
    state: RefCell<Option<SharedTextureState>>,
}

struct SharedTextureState {
    d3d11_texture: ID3D11Texture2D,
    wgpu_texture: Texture,
    width: u32,
    height: u32,
}

impl D3D11Interop {
    /// Create from a raw D3D11 device pointer.
    ///
    /// # Safety
    ///
    /// The pointer must be a valid `ID3D11Device` COM object.
    pub unsafe fn new(d3d11_device_ptr: *mut std::ffi::c_void) -> Result<Self, ImportError> {
        if d3d11_device_ptr.is_null() {
            return Err(ImportError::Platform("D3D11 device pointer is null".into()));
        }
        let d3d11_device: ID3D11Device = unsafe { Interface::from_raw(d3d11_device_ptr) };
        Ok(Self {
            d3d11_device,
            state: RefCell::new(None),
        })
    }

    /// Import as a wgpu texture, recreating the shared texture if the
    /// size changed.
    pub fn import(
        &self,
        wgpu_device: &WgpuDevice,
        width: u32,
        height: u32,
    ) -> Result<Texture, ImportError> {
        let needs_recreate = self
            .state
            .borrow()
            .as_ref()
            .is_none_or(|s| s.width != width || s.height != height);

        if needs_recreate {
            let new_state = Self::create_shared(&self.d3d11_device, wgpu_device, width, height)?;
            *self.state.borrow_mut() = Some(new_state);
        }

        Ok(self.state.borrow().as_ref().unwrap().wgpu_texture.clone())
    }

    /// The underlying D3D11 shared texture. Callers can use this to
    /// blit GL content into it (via surfman's
    /// `create_surface_texture_from_texture`).
    pub fn d3d11_texture(&self) -> Option<ID3D11Texture2D> {
        self.state
            .borrow()
            .as_ref()
            .map(|s| s.d3d11_texture.clone())
    }

    fn create_shared(
        d3d11_device: &ID3D11Device,
        wgpu_device: &WgpuDevice,
        width: u32,
        height: u32,
    ) -> Result<SharedTextureState, ImportError> {
        unsafe {
            let mut texture_ptr: Option<ID3D11Texture2D> = None;

            d3d11_device
                .CreateTexture2D(
                    &Direct3D11::D3D11_TEXTURE2D_DESC {
                        Width: width,
                        Height: height,
                        MipLevels: 1,
                        ArraySize: 1,
                        CPUAccessFlags: 0,
                        Format: Common::DXGI_FORMAT_R8G8B8A8_UNORM,
                        SampleDesc: Common::DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Usage: Direct3D11::D3D11_USAGE_DEFAULT,
                        BindFlags: (Direct3D11::D3D11_BIND_RENDER_TARGET.0
                            | Direct3D11::D3D11_BIND_SHADER_RESOURCE.0)
                            as u32,
                        MiscFlags: (Direct3D11::D3D11_RESOURCE_MISC_SHARED.0
                            | Direct3D11::D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0)
                            as u32,
                    },
                    None,
                    Some(&mut texture_ptr),
                )
                .map_err(|e| ImportError::Platform(format!("CreateTexture2D: {e}")))?;

            let d3d11_texture = texture_ptr.unwrap();

            let nt_handle = d3d11_texture
                .cast::<Dxgi::IDXGIResource1>()
                .map_err(|e| ImportError::Platform(format!("cast IDXGIResource1: {e}")))?
                .CreateSharedHandle(
                    None,
                    Dxgi::DXGI_SHARED_RESOURCE_READ.0,
                    core::PCWSTR::null(),
                )
                .map_err(|e| ImportError::Platform(format!("CreateSharedHandle: {e}")))?;

            let desc = wgpu::TextureDescriptor {
                label: Some("D3D11Interop shared texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                format: TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                view_formats: &[],
            };

            let hal_device = wgpu_device
                .as_hal::<wgpu::wgc::api::Dx12>()
                .ok_or(ImportError::WrongBackend)?;
            let wgpu_texture =
                backends::dx12::wrap_shared_handle(wgpu_device, &hal_device, nt_handle, &desc)?;

            Foundation::CloseHandle(nt_handle)
                .map_err(|e| ImportError::Platform(format!("CloseHandle: {e}")))?;

            Ok(SharedTextureState {
                d3d11_texture,
                wgpu_texture,
                width,
                height,
            })
        }
    }
}
