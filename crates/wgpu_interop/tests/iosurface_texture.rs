#![cfg(target_vendor = "apple")]

mod common;

use objc2_io_surface::{IOSurfaceLockOptions, IOSurfaceRef};
use wgpu::TextureUsages;
use wgpu_interop::{DeviceInterop, IOSurfaceTexture, create_io_surface_bgra};

fn bgra_descriptor(w: u32, h: u32, usage: TextureUsages) -> wgpu::TextureDescriptor<'static> {
    wgpu::TextureDescriptor {
        label: Some("iosurface test"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        view_formats: &[],
    }
}

/// CPU-fill an IOSurface with a BGRA checkerboard. Returns the
/// equivalent BGRA pixel buffer for comparison.
fn fill_iosurface_checkerboard(surface: &IOSurfaceRef, w: u32, h: u32) -> Vec<u8> {
    let mut expected_bgra = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let block = ((x / 8) + (y / 8)) % 2;
            let offset = ((y * w + x) * 4) as usize;
            // BGRA layout: red = (0,0,255,255), blue = (255,0,0,255).
            if block == 0 {
                expected_bgra[offset..offset + 4].copy_from_slice(&[0, 0, 255, 255]);
            } else {
                expected_bgra[offset..offset + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
    }

    unsafe {
        let kr = surface.lock(IOSurfaceLockOptions::empty(), std::ptr::null_mut());
        assert_eq!(kr, 0, "IOSurfaceLock failed: {kr}");

        let base = surface.base_address();
        let row_bytes = surface.bytes_per_row();
        let dst = base.as_ptr() as *mut u8;
        for y in 0..h as usize {
            let src_row = expected_bgra.as_ptr().add(y * (w as usize) * 4);
            std::ptr::copy_nonoverlapping(src_row, dst.add(y * row_bytes), (w * 4) as usize);
        }

        let kr = surface.unlock(IOSurfaceLockOptions::empty(), std::ptr::null_mut());
        assert_eq!(kr, 0, "IOSurfaceUnlock failed: {kr}");
    }

    expected_bgra
}

#[test]
fn iosurface_into_metal() {
    let Some((device, queue)) = common::create_device(wgpu::Backends::METAL) else {
        eprintln!("SKIP: Metal not available");
        return;
    };

    let surface = create_io_surface_bgra(64, 64).expect("create IOSurface");
    let expected = fill_iosurface_checkerboard(&surface, 64, 64);

    let desc = bgra_descriptor(
        64,
        64,
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC,
    );

    let tex = unsafe {
        device
            .import_external_texture(IOSurfaceTexture::new(surface), &desc)
            .expect("import IOSurfaceTexture on Metal")
    };

    let actual = common::readback_texture(&device, &queue, &tex, 64, 64);
    assert_eq!(
        actual, expected,
        "pixel mismatch after IOSurface→Metal import"
    );
}

#[cfg(feature = "vulkan-portability")]
#[test]
fn iosurface_into_vulkan_via_moltenvk() {
    let Some((device, queue)) = common::create_device(wgpu::Backends::VULKAN) else {
        eprintln!("SKIP: MoltenVK not available");
        return;
    };

    let surface = create_io_surface_bgra(64, 64).expect("create IOSurface");
    let expected = fill_iosurface_checkerboard(&surface, 64, 64);

    let desc = bgra_descriptor(
        64,
        64,
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC,
    );

    let tex = unsafe {
        device
            .import_external_texture(IOSurfaceTexture::new(surface), &desc)
            .expect("import IOSurfaceTexture on Vulkan/MoltenVK")
    };

    let actual = common::readback_texture(&device, &queue, &tex, 64, 64);
    assert_eq!(
        actual, expected,
        "pixel mismatch after IOSurface→Vulkan import"
    );
}

/// Sanity: the same IOSurface imports correctly into Metal and (with
/// the feature enabled) into Vulkan/MoltenVK, proving the substrate
/// is shared zero-copy across both backends.
#[cfg(feature = "vulkan-portability")]
#[test]
fn iosurface_shared_metal_and_vulkan() {
    let Some((metal_device, metal_queue)) = common::create_device(wgpu::Backends::METAL) else {
        eprintln!("SKIP: Metal not available");
        return;
    };
    let Some((vk_device, vk_queue)) = common::create_device(wgpu::Backends::VULKAN) else {
        eprintln!("SKIP: MoltenVK not available");
        return;
    };

    let surface = create_io_surface_bgra(64, 64).expect("create IOSurface");
    let expected = fill_iosurface_checkerboard(&surface, 64, 64);

    let desc = bgra_descriptor(
        64,
        64,
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC,
    );

    let metal_tex = unsafe {
        metal_device
            .import_external_texture(
                IOSurfaceTexture {
                    surface: surface.clone(),
                    plane: 0,
                },
                &desc,
            )
            .expect("import IOSurfaceTexture on Metal")
    };
    let metal_pixels = common::readback_texture(&metal_device, &metal_queue, &metal_tex, 64, 64);

    let vk_tex = unsafe {
        vk_device
            .import_external_texture(IOSurfaceTexture::new(surface), &desc)
            .expect("import IOSurfaceTexture on Vulkan/MoltenVK")
    };
    let vk_pixels = common::readback_texture(&vk_device, &vk_queue, &vk_tex, 64, 64);

    assert_eq!(metal_pixels, expected, "Metal readback mismatch");
    assert_eq!(vk_pixels, expected, "Vulkan readback mismatch");
}
