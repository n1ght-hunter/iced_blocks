#![cfg(target_os = "windows")]

mod common;

use wgpu::TextureUsages;
use wgpu_interop::D3D12Resource;
use windows::Win32::Graphics::{
    Direct3D12::{self, ID3D12Resource},
    Dxgi::Common::*,
};

#[test]
fn import_via_free_function() {
    let Some((device, _queue)) = common::create_device(wgpu::Backends::DX12) else {
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
                    Width: 64,
                    Height: 64,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Layout: Direct3D12::D3D12_TEXTURE_LAYOUT_UNKNOWN,
                    Flags: Direct3D12::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
                    Alignment: 0,
                },
                Direct3D12::D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut resource,
            )
            .expect("CreateCommittedResource");
        let resource = resource.unwrap();

        drop(hal);

        let desc =
            common::test_desc(TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT);
        let tex = wgpu_interop::import_external_texture(&device, D3D12Resource { resource }, &desc)
            .expect("import via free function");

        assert_eq!(tex.width(), 64);
        assert_eq!(tex.height(), 64);
    }
}
