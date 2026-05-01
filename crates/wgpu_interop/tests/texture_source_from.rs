#[cfg(target_os = "windows")]
#[path = "common/mod.rs"]
mod common;

use wgpu_interop::{TextureSource, VulkanImage};

#[test]
fn from_vulkan_image() {
    let source: TextureSource = VulkanImage {
        #[cfg(not(target_os = "windows"))]
        fd: 42,
        #[cfg(target_os = "windows")]
        handle: windows::Win32::Foundation::HANDLE(42 as *mut std::ffi::c_void),
        memory_size: 4096,
    }
    .into();
    assert!(matches!(source, TextureSource::VulkanImage(v) if v.memory_size == 4096));
}

#[test]
#[cfg(target_os = "windows")]
fn from_d3d12_resource() {
    use wgpu_interop::D3D12Resource;
    use windows::Win32::Graphics::{
        Direct3D12::{self, ID3D12Resource},
        Dxgi::Common::*,
    };

    let Some((device, _)) = common::create_device(wgpu::Backends::DX12) else {
        eprintln!("SKIP: DX12 not available");
        return;
    };

    unsafe {
        let hal = device.as_hal::<wgpu::wgc::api::Dx12>().unwrap();
        let d3d12_device = hal.raw_device();

        let mut resource: Option<ID3D12Resource> = None;
        d3d12_device
            .CreateCommittedResource(
                &Direct3D12::D3D12_HEAP_PROPERTIES {
                    Type: Direct3D12::D3D12_HEAP_TYPE_DEFAULT,
                    ..Default::default()
                },
                Direct3D12::D3D12_HEAP_FLAG_NONE,
                &Direct3D12::D3D12_RESOURCE_DESC {
                    Dimension: Direct3D12::D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                    Width: 16,
                    Height: 16,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Layout: Direct3D12::D3D12_TEXTURE_LAYOUT_UNKNOWN,
                    Flags: Direct3D12::D3D12_RESOURCE_FLAG_NONE,
                    Alignment: 0,
                },
                Direct3D12::D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut resource,
            )
            .expect("CreateCommittedResource");

        let source: TextureSource = D3D12Resource {
            resource: resource.unwrap(),
        }
        .into();
        assert!(matches!(source, TextureSource::D3D12Resource(_)));
    }
}

#[test]
#[cfg(target_os = "windows")]
fn from_d3d11_shared_handle() {
    use wgpu_interop::D3D11SharedHandle;

    let handle = windows::Win32::Foundation::HANDLE(0x1234 as *mut std::ffi::c_void);
    let source: TextureSource = D3D11SharedHandle { handle }.into();
    assert!(matches!(source, TextureSource::D3D11SharedHandle(_)));
}

#[test]
#[cfg(target_os = "windows")]
fn from_gles_texture() {
    use std::num::NonZeroU32;
    use wgpu_interop::GlesTexture;

    let wgl = common::WglContext::new();
    let name = NonZeroU32::new(1).unwrap();
    let source: TextureSource = GlesTexture {
        gl: &wgl.gl,
        name,
        interop: None,
    }
    .into();
    assert!(matches!(source, TextureSource::GlesTexture(_)));
}
