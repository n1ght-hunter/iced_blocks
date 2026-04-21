//! Offscreen IOSurface import demo for both Metal and Vulkan/MoltenVK.
//!
//! - Default: runs on Metal.
//! - With `WGPU_BACKEND=vulkan` and `--features vulkan-portability`: runs on MoltenVK.
//!
//! Creates one IOSurface, CPU-fills it with a known solid color,
//! imports it through wgpu, copies the texture back via a GPU command,
//! and prints the readback color. This exercises the import path
//! end-to-end without needing a window surface (which is fragile on
//! wgpu+MoltenVK).
//!
//! ```bash
//! # Metal:
//! cargo run -p wgpu_interop --example iosurface_offscreen
//! # MoltenVK:
//! WGPU_BACKEND=vulkan cargo run -p wgpu_interop --example iosurface_offscreen \
//!     --features vulkan-portability
//! ```

#![cfg(target_vendor = "apple")]

use objc2_io_surface::{IOSurfaceLockOptions, IOSurfaceRef};
use wgpu_interop::{DeviceInterop, IOSurfaceTexture, create_io_surface_bgra};

const SIZE: u32 = 64;

fn main() {
    let backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY);
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends,
        ..Default::default()
    });

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: None,
        ..Default::default()
    }))
    .expect("request adapter");

    let info = adapter.get_info();
    eprintln!("Adapter: {} ({:?})", info.name, info.backend);

    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("request device");

    let supported = device.supported_sources();
    eprintln!("Supported sources: {:#010b}", supported.bits());

    // CPU-fill an IOSurface with a known BGRA color.
    let surface = create_io_surface_bgra(SIZE, SIZE).expect("IOSurface create");
    let expected = [0x12, 0x34, 0x56, 0xFF]; // BGRA
    fill_iosurface_bgra(&surface, expected);

    let desc = wgpu::TextureDescriptor {
        label: Some("imported"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        view_formats: &[],
    };

    let texture = unsafe {
        device
            .import_external_texture(IOSurfaceTexture::new(surface), &desc)
            .expect("import IOSurfaceTexture")
    };
    eprintln!(
        "Imported {}x{} BGRA texture.",
        texture.width(),
        texture.height()
    );

    // Read one pixel back via a GPU copy.
    let actual = readback_top_left(&device, &queue, &texture);
    eprintln!("Top-left pixel: {actual:02X?}  (expected {expected:02X?})");

    if actual == expected {
        eprintln!(
            "OK — IOSurface import roundtrip succeeded on {:?}.",
            info.backend
        );
    } else {
        eprintln!("MISMATCH — pixels differ.");
        std::process::exit(1);
    }
}

fn fill_iosurface_bgra(surface: &IOSurfaceRef, bgra: [u8; 4]) {
    unsafe {
        let kr = surface.lock(IOSurfaceLockOptions::empty(), std::ptr::null_mut());
        assert_eq!(kr, 0, "IOSurfaceLock failed: {kr}");

        let row_bytes = surface.bytes_per_row();
        let dst = surface.base_address().as_ptr() as *mut u8;
        for y in 0..SIZE as usize {
            for x in 0..SIZE as usize {
                let p = dst.add(y * row_bytes + x * 4);
                std::ptr::copy_nonoverlapping(bgra.as_ptr(), p, 4);
            }
        }

        let kr = surface.unlock(IOSurfaceLockOptions::empty(), std::ptr::null_mut());
        assert_eq!(kr, 0, "IOSurfaceUnlock failed: {kr}");
    }
}

fn readback_top_left(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> [u8; 4] {
    // 256-byte aligned row pitch: 64 px * 4 bytes = 256 bytes — already aligned.
    let row_pitch = SIZE * 4;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (row_pitch * SIZE) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_pitch),
                rows_per_image: Some(SIZE),
            },
        },
        wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(5)),
        })
        .unwrap();
    rx.recv().unwrap().unwrap();

    let mapped = slice.get_mapped_range();
    let mut out = [0u8; 4];
    out.copy_from_slice(&mapped[0..4]);
    drop(mapped);
    buf.unmap();
    out
}
