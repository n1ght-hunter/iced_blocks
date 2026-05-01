#![cfg(target_os = "windows")]

mod common;

use wgpu_interop::D3D11Interop;
use windows::core::Interface;

#[test]
fn import_and_resize() {
    let Some((wgpu_device, _)) = common::create_device(wgpu::Backends::DX12) else {
        eprintln!("SKIP: DX12 not available");
        return;
    };

    let d3d11_device = common::create_d3d11_device();
    let ptr = Interface::into_raw(d3d11_device);

    unsafe {
        let interop = D3D11Interop::new(ptr).expect("D3D11Interop::new");

        let tex1 = interop.import(&wgpu_device, 64, 64).expect("first import");
        assert_eq!(tex1.width(), 64);
        assert_eq!(tex1.height(), 64);
        assert!(interop.d3d11_texture().is_some());

        let tex2 = interop
            .import(&wgpu_device, 128, 128)
            .expect("resize import");
        assert_eq!(tex2.width(), 128);
        assert_eq!(tex2.height(), 128);
    }
}

#[test]
fn same_size_reuses_texture() {
    let Some((wgpu_device, _)) = common::create_device(wgpu::Backends::DX12) else {
        eprintln!("SKIP: DX12 not available");
        return;
    };

    let d3d11_device = common::create_d3d11_device();
    let ptr = Interface::into_raw(d3d11_device);

    unsafe {
        let interop = D3D11Interop::new(ptr).expect("D3D11Interop::new");

        let tex1 = interop.import(&wgpu_device, 64, 64).expect("first import");
        let tex2 = interop.import(&wgpu_device, 64, 64).expect("second import");

        assert_eq!(tex1.width(), tex2.width());
        assert_eq!(tex1.height(), tex2.height());
    }
}
