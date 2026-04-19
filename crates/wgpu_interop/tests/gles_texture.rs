#![cfg(target_os = "windows")]

mod common;

use wgpu::TextureUsages;
use wgpu_interop::{DeviceInterop, GlesTexture};
use windows::core::Interface;

#[test]
fn no_interop_on_dx12_returns_error() {
    let Some((device, _queue)) = common::create_device(wgpu::Backends::DX12) else {
        eprintln!("SKIP: DX12 not available");
        return;
    };

    let wgl = common::WglContext::new();
    let name = wgl.create_texture_with_data(64, 64, &common::checkerboard_rgba(64, 64));

    let desc = common::test_desc(TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT);
    let result = unsafe {
        device.import_external_texture(
            GlesTexture {
                gl: &wgl.gl,
                name,
                interop: None,
            },
            &desc,
        )
    };

    assert!(
        matches!(result, Err(wgpu_interop::ImportError::Platform(_))),
        "expected Platform error without interop, got {result:?}",
    );
}

#[test]
fn no_interop_on_vulkan_returns_error() {
    let Some((device, _queue)) = common::create_device(wgpu::Backends::VULKAN) else {
        eprintln!("SKIP: Vulkan not available");
        return;
    };

    let wgl = common::WglContext::new();
    let name = wgl.create_texture_with_data(64, 64, &common::checkerboard_rgba(64, 64));

    let desc = common::test_desc(TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT);
    let result = unsafe {
        device.import_external_texture(
            GlesTexture {
                gl: &wgl.gl,
                name,
                interop: None,
            },
            &desc,
        )
    };

    assert!(
        matches!(result, Err(wgpu_interop::ImportError::Platform(_))),
        "expected Platform error without interop, got {result:?}",
    );
}

#[test]
fn cross_backend_blit_on_dx12() {
    let Some((device, _queue)) = common::create_device(wgpu::Backends::DX12) else {
        eprintln!("SKIP: DX12 not available");
        return;
    };

    let wgl = common::WglContext::new();
    let name = wgl.create_texture_with_data(64, 64, &common::checkerboard_rgba(64, 64));

    let d3d11_device = common::create_d3d11_device();
    let d3d11_ptr = Interface::into_raw(d3d11_device);

    let interop = unsafe {
        wgpu_interop::GlInterop::new(d3d11_ptr).expect("GlInterop::new")
    };

    let desc = common::test_desc(
        TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
    );
    let result = unsafe {
        device.import_external_texture(
            GlesTexture {
                gl: &wgl.gl,
                name,
                interop: Some(&interop),
            },
            &desc,
        )
    };

    let tex = result.expect("import GlesTexture via cross-backend blit");
    assert_eq!(tex.width(), 64);
    assert_eq!(tex.height(), 64);
}
