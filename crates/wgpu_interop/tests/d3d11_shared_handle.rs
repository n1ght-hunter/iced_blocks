#![cfg(target_os = "windows")]

mod common;

use wgpu::TextureUsages;
use wgpu_interop::{D3D11SharedHandle, DeviceInterop};
use windows::Win32::Foundation;
use windows::Win32::Graphics::{
    Direct3D11::{self, ID3D11Texture2D},
    Dxgi::{self, Common::*},
};
use windows::core::Interface;

#[test]
fn import_on_dx12() {
    let Some((device, _queue)) = common::create_device(wgpu::Backends::DX12) else {
        eprintln!("SKIP: DX12 not available");
        return;
    };

    let d3d11_device = common::create_d3d11_device();

    unsafe {
        let mut texture: Option<ID3D11Texture2D> = None;
        d3d11_device
            .CreateTexture2D(
                &Direct3D11::D3D11_TEXTURE2D_DESC {
                    Width: 64,
                    Height: 64,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
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
                    CPUAccessFlags: 0,
                },
                None,
                Some(&mut texture as *mut _),
            )
            .expect("CreateTexture2D");

        let d3d11_tex = texture.unwrap();
        let dxgi_res: Dxgi::IDXGIResource1 = d3d11_tex.cast().unwrap();
        let handle = dxgi_res
            .CreateSharedHandle(
                None,
                Dxgi::DXGI_SHARED_RESOURCE_READ.0,
                windows::core::PCWSTR::null(),
            )
            .expect("CreateSharedHandle");

        let desc =
            common::test_desc(TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT);
        let result = device.import_external_texture(D3D11SharedHandle { handle }, &desc);

        let _ = Foundation::CloseHandle(handle);

        let tex = result.expect("import D3D11SharedHandle");
        assert_eq!(tex.width(), 64);
        assert_eq!(tex.height(), 64);
    }
}

#[test]
fn verify_pixels() {
    let Some((device, queue)) = common::create_device(wgpu::Backends::DX12) else {
        eprintln!("SKIP: DX12 not available");
        return;
    };

    let d3d11_device = common::create_d3d11_device();
    let expected = common::checkerboard_rgba(64, 64);

    unsafe {
        let init_data = Direct3D11::D3D11_SUBRESOURCE_DATA {
            pSysMem: expected.as_ptr() as *const _,
            SysMemPitch: 64 * 4,
            SysMemSlicePitch: 0,
        };

        let mut texture: Option<ID3D11Texture2D> = None;
        d3d11_device
            .CreateTexture2D(
                &Direct3D11::D3D11_TEXTURE2D_DESC {
                    Width: 64,
                    Height: 64,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
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
                    CPUAccessFlags: 0,
                },
                Some(&init_data),
                Some(&mut texture as *mut _),
            )
            .expect("CreateTexture2D with initial data");

        // Flush the D3D11 context so the initial data is committed
        // before D3D12 opens the shared handle.
        let ctx = d3d11_device
            .GetImmediateContext()
            .expect("GetImmediateContext");
        ctx.Flush();

        let d3d11_tex = texture.unwrap();
        let dxgi_res: Dxgi::IDXGIResource1 = d3d11_tex.cast().unwrap();
        let handle = dxgi_res
            .CreateSharedHandle(
                None,
                Dxgi::DXGI_SHARED_RESOURCE_READ.0,
                windows::core::PCWSTR::null(),
            )
            .expect("CreateSharedHandle");

        let desc = common::test_desc(
            TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC,
        );
        let tex = device
            .import_external_texture(D3D11SharedHandle { handle }, &desc)
            .expect("import D3D11SharedHandle");

        let _ = Foundation::CloseHandle(handle);

        let actual = common::readback_texture(&device, &queue, &tex, 64, 64);
        assert_eq!(
            actual, expected,
            "pixel data mismatch after D3D11 shared handle import"
        );
    }
}
