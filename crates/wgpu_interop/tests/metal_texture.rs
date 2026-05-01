#![cfg(target_vendor = "apple")]

mod common;

use wgpu::TextureUsages;
use wgpu_interop::{DeviceInterop, MetalTexture};

fn create_metal_texture(
    metal_device: &metal::Device,
    width: u64,
    height: u64,
    usage: metal::MTLTextureUsage,
) -> metal::Texture {
    let desc = metal::TextureDescriptor::new();
    desc.set_texture_type(metal::MTLTextureType::D2);
    desc.set_pixel_format(metal::MTLPixelFormat::RGBA8Unorm);
    desc.set_width(width);
    desc.set_height(height);
    desc.set_mipmap_level_count(1);
    desc.set_sample_count(1);
    desc.set_storage_mode(metal::MTLStorageMode::Private);
    desc.set_usage(usage);
    metal_device.new_texture(&desc)
}

#[test]
fn import_on_metal() {
    let Some((device, _queue)) = common::create_device(wgpu::Backends::METAL) else {
        eprintln!("SKIP: Metal not available");
        return;
    };

    unsafe {
        let hal = device.as_hal::<wgpu::wgc::api::Metal>().expect("Metal HAL");
        let metal_device = hal.raw_device().lock().clone();
        drop(hal);

        let mtl_tex = create_metal_texture(
            &metal_device,
            64,
            64,
            metal::MTLTextureUsage::ShaderRead | metal::MTLTextureUsage::RenderTarget,
        );

        let desc =
            common::test_desc(TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT);
        let tex = device
            .import_external_texture(MetalTexture { texture: mtl_tex }, &desc)
            .expect("import MetalTexture");

        assert_eq!(tex.width(), 64);
        assert_eq!(tex.height(), 64);
    }
}

#[test]
fn verify_pixels() {
    let Some((device, queue)) = common::create_device(wgpu::Backends::METAL) else {
        eprintln!("SKIP: Metal not available");
        return;
    };

    let expected = common::checkerboard_rgba(64, 64);

    unsafe {
        let hal = device.as_hal::<wgpu::wgc::api::Metal>().expect("Metal HAL");
        let metal_device = hal.raw_device().lock().clone();
        drop(hal);

        // Shared storage so we can CPU-upload with `replace_region`.
        let mtl_desc = metal::TextureDescriptor::new();
        mtl_desc.set_texture_type(metal::MTLTextureType::D2);
        mtl_desc.set_pixel_format(metal::MTLPixelFormat::RGBA8Unorm);
        mtl_desc.set_width(64);
        mtl_desc.set_height(64);
        mtl_desc.set_mipmap_level_count(1);
        mtl_desc.set_sample_count(1);
        mtl_desc.set_storage_mode(metal::MTLStorageMode::Managed);
        mtl_desc
            .set_usage(metal::MTLTextureUsage::ShaderRead | metal::MTLTextureUsage::RenderTarget);
        let mtl_tex = metal_device.new_texture(&mtl_desc);

        mtl_tex.replace_region(
            metal::MTLRegion::new_2d(0, 0, 64, 64),
            0,
            expected.as_ptr() as *const std::ffi::c_void,
            64 * 4,
        );

        let desc = common::test_desc(
            TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC,
        );
        let tex = device
            .import_external_texture(MetalTexture { texture: mtl_tex }, &desc)
            .expect("import MetalTexture");

        let actual = common::readback_texture(&device, &queue, &tex, 64, 64);
        assert_eq!(
            actual, expected,
            "pixel data mismatch after Metal texture import"
        );
    }
}

#[test]
fn wrong_backend_metal_texture_rejected_elsewhere() {
    // Only run when a non-Metal backend is available (e.g. Vulkan via MoltenVK).
    // On pure macOS with only Metal, this is a no-op.
    let Some((device, _queue)) = common::create_device(wgpu::Backends::VULKAN) else {
        eprintln!("SKIP: Vulkan not available on this platform");
        return;
    };

    unsafe {
        // Spin up a Metal device independently to produce a texture.
        let Some((metal_wgpu_device, _)) = common::create_device(wgpu::Backends::METAL) else {
            eprintln!("SKIP: Metal not available");
            return;
        };
        let metal_hal = metal_wgpu_device
            .as_hal::<wgpu::wgc::api::Metal>()
            .expect("Metal HAL");
        let metal_device = metal_hal.raw_device().lock().clone();
        drop(metal_hal);

        let mtl_tex = create_metal_texture(
            &metal_device,
            64,
            64,
            metal::MTLTextureUsage::ShaderRead | metal::MTLTextureUsage::RenderTarget,
        );

        let desc =
            common::test_desc(TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT);
        let result = device.import_external_texture(MetalTexture { texture: mtl_tex }, &desc);

        // Either error is acceptable: `WrongBackend` if no Vulkan
        // backend is built into wgpu_interop on this target,
        // `Unsupported` if the Vulkan backend dispatched but has no
        // arm for `MetalTexture`.
        assert!(
            matches!(
                result,
                Err(wgpu_interop::ImportError::WrongBackend
                    | wgpu_interop::ImportError::Unsupported)
            ),
            "expected WrongBackend or Unsupported on non-Metal device, got {result:?}",
        );
    }
}
