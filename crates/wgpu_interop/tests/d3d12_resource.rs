#![cfg(target_os = "windows")]

mod common;

use wgpu::TextureUsages;
use wgpu_interop::{D3D12Resource, DeviceInterop};
use windows::Win32::Graphics::{
    Direct3D12::{self, ID3D12Resource},
    Dxgi::Common::*,
};
use windows::core::Interface;

#[test]
fn import_on_dx12() {
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
        let tex = device
            .import_external_texture(D3D12Resource { resource }, &desc)
            .expect("import D3D12Resource");

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

    let expected = common::checkerboard_rgba(64, 64);

    unsafe {
        let hal = device.as_hal::<wgpu::wgc::api::Dx12>().unwrap();
        let d3d12_device = hal.raw_device();

        // Create the target texture
        let mut texture_resource: Option<ID3D12Resource> = None;
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
                Direct3D12::D3D12_RESOURCE_STATE_COPY_DEST,
                None,
                &mut texture_resource,
            )
            .expect("CreateCommittedResource (texture)");
        let texture_resource = texture_resource.unwrap();

        // Create an upload buffer — 64 pixels * 4 bytes = 256, already aligned
        let row_pitch: u64 = 256;
        let upload_size = row_pitch * 64;
        let mut upload_buffer: Option<ID3D12Resource> = None;
        d3d12_device
            .CreateCommittedResource(
                &Direct3D12::D3D12_HEAP_PROPERTIES {
                    Type: Direct3D12::D3D12_HEAP_TYPE_UPLOAD,
                    ..Default::default()
                },
                Direct3D12::D3D12_HEAP_FLAG_NONE,
                &Direct3D12::D3D12_RESOURCE_DESC {
                    Dimension: Direct3D12::D3D12_RESOURCE_DIMENSION_BUFFER,
                    Width: upload_size,
                    Height: 1,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    Format: DXGI_FORMAT_UNKNOWN,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Layout: Direct3D12::D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                    Flags: Direct3D12::D3D12_RESOURCE_FLAG_NONE,
                    Alignment: 0,
                },
                Direct3D12::D3D12_RESOURCE_STATE_GENERIC_READ,
                None,
                &mut upload_buffer,
            )
            .expect("CreateCommittedResource (upload)");
        let upload_buffer = upload_buffer.unwrap();

        // Map and copy checkerboard data
        let mut mapped_ptr: *mut u8 = std::ptr::null_mut();
        upload_buffer
            .Map(
                0,
                None,
                Some(&mut mapped_ptr as *mut *mut u8 as *mut *mut std::ffi::c_void),
            )
            .expect("Map upload buffer");
        std::ptr::copy_nonoverlapping(expected.as_ptr(), mapped_ptr, expected.len());
        upload_buffer.Unmap(0, None);

        // Create command allocator + command list
        let cmd_alloc: Direct3D12::ID3D12CommandAllocator = d3d12_device
            .CreateCommandAllocator(Direct3D12::D3D12_COMMAND_LIST_TYPE_DIRECT)
            .expect("CreateCommandAllocator");

        let cmd_list: Direct3D12::ID3D12GraphicsCommandList = d3d12_device
            .CreateCommandList(
                0,
                Direct3D12::D3D12_COMMAND_LIST_TYPE_DIRECT,
                &cmd_alloc,
                None,
            )
            .expect("CreateCommandList");

        // Copy from upload buffer to texture
        let dst = Direct3D12::D3D12_TEXTURE_COPY_LOCATION {
            pResource: std::mem::ManuallyDrop::new(Some(texture_resource.clone())),
            Type: Direct3D12::D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            Anonymous: Direct3D12::D3D12_TEXTURE_COPY_LOCATION_0 {
                SubresourceIndex: 0,
            },
        };
        let src = Direct3D12::D3D12_TEXTURE_COPY_LOCATION {
            pResource: std::mem::ManuallyDrop::new(Some(upload_buffer.clone())),
            Type: Direct3D12::D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            Anonymous: Direct3D12::D3D12_TEXTURE_COPY_LOCATION_0 {
                PlacedFootprint: Direct3D12::D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                    Offset: 0,
                    Footprint: Direct3D12::D3D12_SUBRESOURCE_FOOTPRINT {
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        Width: 64,
                        Height: 64,
                        Depth: 1,
                        RowPitch: row_pitch as u32,
                    },
                },
            },
        };
        cmd_list.CopyTextureRegion(&dst, 0, 0, 0, &src, None);

        // Transition texture from COPY_DEST to COMMON
        let barrier = Direct3D12::D3D12_RESOURCE_BARRIER {
            Type: Direct3D12::D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: Direct3D12::D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: Direct3D12::D3D12_RESOURCE_BARRIER_0 {
                Transition: std::mem::ManuallyDrop::new(
                    Direct3D12::D3D12_RESOURCE_TRANSITION_BARRIER {
                        pResource: std::mem::ManuallyDrop::new(Some(texture_resource.clone())),
                        StateBefore: Direct3D12::D3D12_RESOURCE_STATE_COPY_DEST,
                        StateAfter: Direct3D12::D3D12_RESOURCE_STATE_COMMON,
                        Subresource: Direct3D12::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    },
                ),
            },
        };
        cmd_list.ResourceBarrier(&[barrier]);

        cmd_list.Close().expect("Close command list");

        // Execute and wait
        let cmd_queue: Direct3D12::ID3D12CommandQueue = d3d12_device
            .CreateCommandQueue(&Direct3D12::D3D12_COMMAND_QUEUE_DESC {
                Type: Direct3D12::D3D12_COMMAND_LIST_TYPE_DIRECT,
                ..Default::default()
            })
            .expect("CreateCommandQueue");

        cmd_queue.ExecuteCommandLists(&[Some(cmd_list.cast().unwrap())]);

        let fence: Direct3D12::ID3D12Fence = d3d12_device
            .CreateFence(0, Direct3D12::D3D12_FENCE_FLAG_NONE)
            .expect("CreateFence");

        cmd_queue.Signal(&fence, 1).expect("Signal");

        if fence.GetCompletedValue() < 1 {
            let event = windows::Win32::System::Threading::CreateEventW(None, false, false, None)
                .expect("CreateEvent");
            fence
                .SetEventOnCompletion(1, event)
                .expect("SetEventOnCompletion");
            windows::Win32::System::Threading::WaitForSingleObject(event, u32::MAX);
            windows::Win32::Foundation::CloseHandle(event).ok();
        }

        drop(hal);

        let desc = common::test_desc(
            TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC,
        );
        let tex = device
            .import_external_texture(
                D3D12Resource {
                    resource: texture_resource,
                },
                &desc,
            )
            .expect("import D3D12Resource");

        let actual = common::readback_texture(&device, &queue, &tex, 64, 64);
        assert_eq!(
            actual, expected,
            "pixel data mismatch after D3D12 resource import"
        );
    }
}

#[test]
fn wrong_backend_vulkan() {
    let Some((device, _queue)) = common::create_device(wgpu::Backends::VULKAN) else {
        eprintln!("SKIP: Vulkan not available");
        return;
    };

    // Create a D3D12 resource from a standalone device to test wrong-backend rejection.
    unsafe {
        let hal = common::create_device(wgpu::Backends::DX12);
        let Some((dx12_device, _)) = hal else {
            eprintln!("SKIP: DX12 not available (needed for resource creation)");
            return;
        };

        let dx12_hal = dx12_device.as_hal::<wgpu::wgc::api::Dx12>().unwrap();
        let d3d12_device = dx12_hal.raw_device();

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

        drop(dx12_hal);

        let desc =
            common::test_desc(TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT);
        let result = device.import_external_texture(D3D12Resource { resource }, &desc);

        assert!(
            matches!(result, Err(wgpu_interop::ImportError::Unsupported)),
            "expected Unsupported, got {result:?}",
        );
    }
}
