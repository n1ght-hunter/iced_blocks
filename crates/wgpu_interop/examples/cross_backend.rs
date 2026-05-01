//! Cross-backend zero-copy: wgpu-on-Vulkan renders into an
//! `IOSurface`, then wgpu-on-Metal imports the *same* `IOSurface` and
//! reads back the pixels Vulkan wrote — proving the two backends
//! share GPU memory through the IOSurface substrate on Apple Silicon.
//!
//! ```bash
//! DYLD_LIBRARY_PATH=/opt/homebrew/opt/vulkan-loader/lib \
//! VK_ICD_FILENAMES=/opt/homebrew/etc/vulkan/icd.d/MoltenVK_icd.json \
//! cargo run -p wgpu_interop --example cross_backend --features vulkan-portability
//! ```
//!
//! Requires the `vulkan-portability` feature so wgpu links MoltenVK,
//! plus the env vars above so the loader can find MoltenVK.

#![cfg(all(target_vendor = "apple", feature = "vulkan-portability"))]

use objc2_core_foundation::CFRetained;
use objc2_io_surface::IOSurfaceRef;
use wgpu_interop::{DeviceInterop, IOSurfaceTexture, create_io_surface_bgra};

const SIZE: u32 = 64;

fn main() {
    // 1. Allocate one IOSurface — the shared backing store.
    let surface = create_io_surface_bgra(SIZE, SIZE).expect("IOSurface create");
    eprintln!("IOSurface allocated ({SIZE}×{SIZE} BGRA8).");

    // 2. wgpu-on-Vulkan: import the IOSurface, render a solid
    //    distinctive color into it, submit, wait. CFRetained::clone
    //    bumps the CoreFoundation refcount; both wgpu devices end up
    //    pointing at the same backing store.
    let vk_pixel = render_via_vulkan(surface.clone());
    eprintln!("Vulkan rendered clear color: {vk_pixel:02X?} (BGRA).");

    // 3. wgpu-on-Metal: import the *same* IOSurface and read the
    //    top-left pixel back through a GPU copy.
    let metal_pixel = read_via_metal(surface);
    eprintln!("Metal read top-left pixel:   {metal_pixel:02X?} (BGRA).");

    if metal_pixel == vk_pixel {
        eprintln!(
            "OK — wgpu Metal saw the exact pixel wgpu Vulkan wrote, with no \
             intermediate copy. Zero-copy cross-backend interop confirmed."
        );
    } else {
        eprintln!("MISMATCH — pixels differ.");
        std::process::exit(1);
    }
}

/// Boot a wgpu Vulkan device, import the IOSurface as a render
/// target, run a render pass that clears it to a known color, and
/// wait for GPU completion. Returns the BGRA color that was rendered.
fn render_via_vulkan(surface: CFRetained<IOSurfaceRef>) -> [u8; 4] {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("Vulkan/MoltenVK adapter");
    let info = adapter.get_info();
    eprintln!("  Vulkan adapter: {} ({:?})", info.name, info.backend);
    assert_eq!(info.backend, wgpu::Backend::Vulkan);

    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("Vulkan device");

    let desc = wgpu::TextureDescriptor {
        label: Some("vulkan-side IOSurface"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        view_formats: &[],
    };

    let texture = unsafe {
        device
            .import_external_texture(IOSurfaceTexture::new(surface), &desc)
            .expect("import IOSurface into Vulkan")
    };

    // Distinctive non-trivial color so we can be sure Metal is
    // reading what Vulkan wrote (not zeroes / not residual data).
    let clear = wgpu::Color {
        r: 0.30, // → 76
        g: 0.10, // → 25
        b: 0.85, // → 216
        a: 1.0,  // → 255
    };

    let view = texture.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("vulkan-clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
    }
    queue.submit(Some(encoder.finish()));
    // Wait for the Vulkan-side GPU work to actually complete before
    // handing the IOSurface to Metal.
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(5)),
        })
        .expect("Vulkan poll");

    // Convert the wgpu::Color to BGRA8Unorm bytes for comparison.
    [
        (clear.b * 255.0).round() as u8,
        (clear.g * 255.0).round() as u8,
        (clear.r * 255.0).round() as u8,
        (clear.a * 255.0).round() as u8,
    ]
}

/// Boot a wgpu Metal device, import the *same* IOSurface, copy the
/// texture into a readback buffer, return the top-left pixel.
fn read_via_metal(surface: CFRetained<IOSurfaceRef>) -> [u8; 4] {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..Default::default()
    });
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("Metal adapter");
    let info = adapter.get_info();
    eprintln!("  Metal adapter:  {} ({:?})", info.name, info.backend);
    assert_eq!(info.backend, wgpu::Backend::Metal);

    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("Metal device");

    let desc = wgpu::TextureDescriptor {
        label: Some("metal-side IOSurface"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        view_formats: &[],
    };

    let texture = unsafe {
        device
            .import_external_texture(IOSurfaceTexture::new(surface), &desc)
            .expect("import IOSurface into Metal")
    };

    // 64 * 4 = 256 bytes per row — already 256-byte aligned.
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
            texture: &texture,
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
        .expect("Metal poll");
    rx.recv().unwrap().unwrap();

    let mapped = slice.get_mapped_range();
    let mut out = [0u8; 4];
    out.copy_from_slice(&mapped[0..4]);
    drop(mapped);
    buf.unmap();
    out
}
