//! Shared texture creation helpers for wgpu_interop examples.

#[cfg(target_os = "windows")]
use wgpu_interop::D3D12Resource;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::{
    Direct3D::D3D_DRIVER_TYPE_HARDWARE,
    Direct3D11::{self, ID3D11Device, ID3D11Texture2D},
    Direct3D12::{self, ID3D12Resource},
    Dxgi::{self, Common::*},
};
#[cfg(target_os = "windows")]
use windows::core::Interface;

pub const SIZE: u32 = 64;

pub const FULLSCREEN_TRIANGLE_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    let x = f32(i32(idx & 1u)) * 4.0 - 1.0;
    let y = f32(i32(idx >> 1u)) * 4.0 - 1.0;
    var out: VertexOutput;
    out.position = vec4f(x, y, 0.0, 1.0);
    out.uv = vec2f((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return textureSample(tex, samp, in.uv);
}
"#;

#[cfg(target_os = "windows")]
struct SendableHandle(Foundation::HANDLE);
#[cfg(target_os = "windows")]
unsafe impl Send for SendableHandle {}

/// Generates a solid-color RGBA pixel buffer derived from a seed.
pub fn solid_pixels(seed: u64) -> Vec<u8> {
    let hash = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    let r = hash as u8;
    let g = (hash >> 8) as u8;
    let b = (hash >> 16) as u8;

    let mut data = vec![0u8; (SIZE * SIZE * 4) as usize];
    for pixel in data.chunks_exact_mut(4) {
        pixel[0] = r;
        pixel[1] = g;
        pixel[2] = b;
        pixel[3] = 255;
    }
    data
}

pub fn texture_desc() -> wgpu::TextureDescriptor<'static> {
    wgpu::TextureDescriptor {
        label: Some("imported"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        view_formats: &[],
    }
}

#[cfg(target_os = "windows")]
/// Creates a D3D12 texture filled with random pixel data, uploaded via staging buffer.
///
/// # Safety
///
/// The wgpu device must be using the DX12 backend.
pub unsafe fn create_d3d12_resource(device: &wgpu::Device, seed: u64) -> D3D12Resource {
    unsafe {
        let hal = device
            .as_hal::<wgpu::wgc::api::Dx12>()
            .expect("DX12 backend required");
        let d3d12_device = hal.raw_device();

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
                    Width: SIZE as u64,
                    Height: SIZE,
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

        let row_pitch: u64 = (SIZE * 4) as u64;
        let upload_size = row_pitch * SIZE as u64;

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

        let data = solid_pixels(seed);
        let mut mapped_ptr: *mut u8 = std::ptr::null_mut();
        upload_buffer
            .Map(
                0,
                None,
                Some(&mut mapped_ptr as *mut *mut u8 as *mut *mut std::ffi::c_void),
            )
            .expect("Map upload buffer");
        std::ptr::copy_nonoverlapping(data.as_ptr(), mapped_ptr, data.len());
        upload_buffer.Unmap(0, None);

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
                        Width: SIZE,
                        Height: SIZE,
                        Depth: 1,
                        RowPitch: row_pitch as u32,
                    },
                },
            },
        };
        cmd_list.CopyTextureRegion(&dst, 0, 0, 0, &src, None);

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

        D3D12Resource {
            resource: texture_resource,
        }
    }
}

#[cfg(target_os = "windows")]
/// Creates a D3D11 texture with random pixel data on a background thread
/// and returns an NTHANDLE for cross-API import.
pub fn create_d3d11_shared_handle(seed: u64) -> Foundation::HANDLE {
    let sendable = std::thread::spawn(move || unsafe {
        let mut device: Option<ID3D11Device> = None;
        Direct3D11::D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            Foundation::HMODULE::default(),
            Direct3D11::D3D11_CREATE_DEVICE_FLAG(0),
            None,
            Direct3D11::D3D11_SDK_VERSION,
            Some(&mut device as *mut _),
            None,
            None,
        )
        .expect("D3D11CreateDevice");
        let device = device.unwrap();

        let data = solid_pixels(seed);
        let init_data = Direct3D11::D3D11_SUBRESOURCE_DATA {
            pSysMem: data.as_ptr() as *const _,
            SysMemPitch: SIZE * 4,
            SysMemSlicePitch: 0,
        };

        let mut texture: Option<ID3D11Texture2D> = None;
        device
            .CreateTexture2D(
                &Direct3D11::D3D11_TEXTURE2D_DESC {
                    Width: SIZE,
                    Height: SIZE,
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
            .expect("CreateTexture2D");

        let ctx = device.GetImmediateContext().expect("GetImmediateContext");
        ctx.Flush();

        let dxgi_res: Dxgi::IDXGIResource1 = texture.unwrap().cast().unwrap();
        let handle = dxgi_res
            .CreateSharedHandle(
                None,
                Dxgi::DXGI_SHARED_RESOURCE_READ.0,
                windows::core::PCWSTR::null(),
            )
            .expect("CreateSharedHandle");
        SendableHandle(handle)
    })
    .join()
    .expect("texture thread panicked");
    sendable.0
}

/// Creates a D3D12 committed resource with shared flag, uploads pixel data,
/// and exports a shared NTHANDLE suitable for cross-API import.
///
/// On Windows, interop works by creating the resource in D3D12 and exporting
/// a shared handle that D3D12 can open via `OpenSharedHandle`.
#[cfg(target_os = "windows")]
pub fn create_vulkan_image(seed: u64) -> wgpu_interop::VulkanImage {
    unsafe {
        let mut d3d12_device: Option<Direct3D12::ID3D12Device> = None;
        Direct3D12::D3D12CreateDevice(
            None,
            windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0,
            &mut d3d12_device,
        )
        .expect("D3D12CreateDevice");
        let d3d12_device = d3d12_device.unwrap();

        let mut texture: Option<Direct3D12::ID3D12Resource> = None;
        d3d12_device
            .CreateCommittedResource(
                &Direct3D12::D3D12_HEAP_PROPERTIES {
                    Type: Direct3D12::D3D12_HEAP_TYPE_DEFAULT,
                    ..Default::default()
                },
                Direct3D12::D3D12_HEAP_FLAG_SHARED,
                &Direct3D12::D3D12_RESOURCE_DESC {
                    Dimension: Direct3D12::D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                    Width: SIZE as u64,
                    Height: SIZE,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Layout: Direct3D12::D3D12_TEXTURE_LAYOUT_UNKNOWN,
                    Flags: Direct3D12::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET
                        | Direct3D12::D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS,
                    Alignment: 0,
                },
                Direct3D12::D3D12_RESOURCE_STATE_COPY_DEST,
                None,
                &mut texture,
            )
            .expect("CreateCommittedResource (shared texture)");
        let texture = texture.unwrap();

        // Upload pixel data
        let row_pitch = (SIZE * 4) as u64;
        let upload_size = row_pitch * SIZE as u64;
        let mut upload_buf: Option<Direct3D12::ID3D12Resource> = None;
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
                &mut upload_buf,
            )
            .expect("CreateCommittedResource (upload)");
        let upload_buf = upload_buf.unwrap();

        let data = solid_pixels(seed);
        let mut mapped: *mut u8 = std::ptr::null_mut();
        upload_buf
            .Map(
                0,
                None,
                Some(&mut mapped as *mut *mut u8 as *mut *mut std::ffi::c_void),
            )
            .expect("Map");
        std::ptr::copy_nonoverlapping(data.as_ptr(), mapped, data.len());
        upload_buf.Unmap(0, None);

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

        let dst = Direct3D12::D3D12_TEXTURE_COPY_LOCATION {
            pResource: std::mem::ManuallyDrop::new(Some(texture.clone())),
            Type: Direct3D12::D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            Anonymous: Direct3D12::D3D12_TEXTURE_COPY_LOCATION_0 {
                SubresourceIndex: 0,
            },
        };
        let src = Direct3D12::D3D12_TEXTURE_COPY_LOCATION {
            pResource: std::mem::ManuallyDrop::new(Some(upload_buf.clone())),
            Type: Direct3D12::D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            Anonymous: Direct3D12::D3D12_TEXTURE_COPY_LOCATION_0 {
                PlacedFootprint: Direct3D12::D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                    Offset: 0,
                    Footprint: Direct3D12::D3D12_SUBRESOURCE_FOOTPRINT {
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        Width: SIZE,
                        Height: SIZE,
                        Depth: 1,
                        RowPitch: row_pitch as u32,
                    },
                },
            },
        };
        cmd_list.CopyTextureRegion(&dst, 0, 0, 0, &src, None);

        let barrier = Direct3D12::D3D12_RESOURCE_BARRIER {
            Type: Direct3D12::D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: Direct3D12::D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: Direct3D12::D3D12_RESOURCE_BARRIER_0 {
                Transition: std::mem::ManuallyDrop::new(
                    Direct3D12::D3D12_RESOURCE_TRANSITION_BARRIER {
                        pResource: std::mem::ManuallyDrop::new(Some(texture.clone())),
                        StateBefore: Direct3D12::D3D12_RESOURCE_STATE_COPY_DEST,
                        StateAfter: Direct3D12::D3D12_RESOURCE_STATE_COMMON,
                        Subresource: Direct3D12::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    },
                ),
            },
        };
        cmd_list.ResourceBarrier(&[barrier]);
        cmd_list.Close().expect("Close");

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
            Foundation::CloseHandle(event).ok();
        }

        // Export shared handle
        let handle: Foundation::HANDLE = d3d12_device
            .CreateSharedHandle(&texture, None, Foundation::GENERIC_ALL.0, None)
            .expect("CreateSharedHandle");

        let desc = texture.GetDesc();
        let alloc_info = d3d12_device.GetResourceAllocationInfo(0, &[desc]);

        wgpu_interop::VulkanImage {
            handle,
            memory_size: alloc_info.SizeInBytes,
        }
    }
}

/// Creates a Vulkan image with pixel data and exports an fd for cross-API import.
///
/// Uses a standalone Vulkan instance/device.
#[cfg(not(target_os = "windows"))]
pub fn create_vulkan_image(seed: u64) -> wgpu_interop::VulkanImage {
    use ash::vk;

    const HANDLE_TYPE: vk::ExternalMemoryHandleTypeFlags =
        vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD;

    unsafe {
        let entry = ash::Entry::load().expect("failed to load Vulkan entry");
        let vk_instance = entry
            .create_instance(
                &vk::InstanceCreateInfo::default().application_info(
                    &vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_1),
                ),
                None,
            )
            .expect("vkCreateInstance");

        let vk_phys_device = vk_instance
            .enumerate_physical_devices()
            .expect("enumerate physical devices")
            .into_iter()
            .next()
            .expect("no Vulkan physical device");

        let queue_family = vk_instance
            .get_physical_device_queue_family_properties(vk_phys_device)
            .iter()
            .position(|props| props.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .expect("no graphics queue family") as u32;

        let queue_priorities = [1.0f32];
        let queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family)
            .queue_priorities(&queue_priorities);

        let device_extensions = [
            ash::khr::external_memory::NAME.as_ptr(),
            ash::khr::external_memory_fd::NAME.as_ptr(),
        ];

        let vk_device = vk_instance
            .create_device(
                vk_phys_device,
                &vk::DeviceCreateInfo::default()
                    .queue_create_infos(&[queue_create_info])
                    .enabled_extension_names(&device_extensions),
                None,
            )
            .expect("vkCreateDevice");

        let mut ext_mem_info =
            vk::ExternalMemoryImageCreateInfo::default().handle_types(HANDLE_TYPE);

        let image = vk_device
            .create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D {
                        width: SIZE,
                        height: SIZE,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(
                        vk::ImageUsageFlags::SAMPLED
                            | vk::ImageUsageFlags::COLOR_ATTACHMENT
                            | vk::ImageUsageFlags::TRANSFER_DST,
                    )
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .push_next(&mut ext_mem_info),
                None,
            )
            .expect("vkCreateImage");

        let mem_reqs = vk_device.get_image_memory_requirements(image);

        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let mut export_info = vk::ExportMemoryAllocateInfo::default().handle_types(HANDLE_TYPE);

        let mem_props = vk_instance.get_physical_device_memory_properties(vk_phys_device);
        let image_mem_type = (0..mem_props.memory_type_count)
            .find(|&i| {
                (mem_reqs.memory_type_bits & (1 << i)) != 0
                    && mem_props.memory_types[i as usize]
                        .property_flags
                        .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
            })
            .expect("no suitable memory type for image");

        let memory = vk_device
            .allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(mem_reqs.size)
                    .memory_type_index(image_mem_type)
                    .push_next(&mut dedicated)
                    .push_next(&mut export_info),
                None,
            )
            .expect("vkAllocateMemory");

        vk_device
            .bind_image_memory(image, memory, 0)
            .expect("vkBindImageMemory");

        // Upload pixel data via staging buffer
        let staging_size = (SIZE * SIZE * 4) as u64;
        let staging_buffer = vk_device
            .create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(staging_size)
                    .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
            .expect("vkCreateBuffer (staging)");

        let buf_mem_reqs = vk_device.get_buffer_memory_requirements(staging_buffer);
        let staging_mem_type = (0..mem_props.memory_type_count)
            .find(|&i| {
                (buf_mem_reqs.memory_type_bits & (1 << i)) != 0
                    && mem_props.memory_types[i as usize].property_flags.contains(
                        vk::MemoryPropertyFlags::HOST_VISIBLE
                            | vk::MemoryPropertyFlags::HOST_COHERENT,
                    )
            })
            .expect("no host-visible memory type");

        let staging_memory = vk_device
            .allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(buf_mem_reqs.size)
                    .memory_type_index(staging_mem_type),
                None,
            )
            .expect("vkAllocateMemory (staging)");

        vk_device
            .bind_buffer_memory(staging_buffer, staging_memory, 0)
            .expect("vkBindBufferMemory");

        let data = solid_pixels(seed);
        let mapped = vk_device
            .map_memory(staging_memory, 0, staging_size, vk::MemoryMapFlags::empty())
            .expect("vkMapMemory") as *mut u8;
        std::ptr::copy_nonoverlapping(data.as_ptr(), mapped, data.len());
        vk_device.unmap_memory(staging_memory);

        let cmd_pool = vk_device
            .create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(queue_family)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
            .expect("vkCreateCommandPool");

        let cmd_bufs = vk_device
            .allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(cmd_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
            .expect("vkAllocateCommandBuffers");
        let cmd = cmd_bufs[0];

        vk_device
            .begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())
            .expect("vkBeginCommandBuffer");

        vk_device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[vk::ImageMemoryBarrier::default()
                .image(image)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })],
        );

        vk_device.cmd_copy_buffer_to_image(
            cmd,
            staging_buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[vk::BufferImageCopy {
                buffer_offset: 0,
                buffer_row_length: 0,
                buffer_image_height: 0,
                image_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
                image_extent: vk::Extent3D {
                    width: SIZE,
                    height: SIZE,
                    depth: 1,
                },
            }],
        );

        vk_device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[vk::ImageMemoryBarrier::default()
                .image(image)
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::GENERAL)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::empty())
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })],
        );

        vk_device
            .end_command_buffer(cmd)
            .expect("vkEndCommandBuffer");

        let vk_queue = vk_device.get_device_queue(queue_family, 0);
        let fence = vk_device
            .create_fence(&vk::FenceCreateInfo::default(), None)
            .expect("vkCreateFence");

        vk_device
            .queue_submit(
                vk_queue,
                &[vk::SubmitInfo::default().command_buffers(&[cmd])],
                fence,
            )
            .expect("vkQueueSubmit");

        vk_device
            .wait_for_fences(&[fence], true, u64::MAX)
            .expect("vkWaitForFences");

        vk_device.destroy_fence(fence, None);
        vk_device.destroy_command_pool(cmd_pool, None);
        vk_device.destroy_buffer(staging_buffer, None);
        vk_device.free_memory(staging_memory, None);

        let fd_api = ash::khr::external_memory_fd::Device::new(&vk_instance, &vk_device);
        let fd = fd_api
            .get_memory_fd(
                &vk::MemoryGetFdInfoKHR::default()
                    .memory(memory)
                    .handle_type(HANDLE_TYPE),
            )
            .expect("vkGetMemoryFdKHR");

        vk_device.destroy_image(image, None);
        vk_device.free_memory(memory, None);
        vk_device.destroy_device(None);
        vk_instance.destroy_instance(None);

        wgpu_interop::VulkanImage {
            fd,
            memory_size: mem_reqs.size,
        }
    }
}

/// Live WGL context for creating GL textures in examples.
#[cfg(target_os = "windows")]
pub struct WglContext {
    pub gl: std::sync::Arc<glow::Context>,
    hglrc: windows::Win32::Graphics::OpenGL::HGLRC,
    hdc: windows::Win32::Graphics::Gdi::HDC,
    hwnd: windows::Win32::Foundation::HWND,
}

#[cfg(target_os = "windows")]
impl WglContext {
    pub fn new() -> Self {
        use std::sync::Arc;
        use windows::Win32::Foundation as WinFoundation;
        use windows::Win32::Graphics::{Gdi, OpenGL};
        use windows::Win32::UI::WindowsAndMessaging as Wm;

        unsafe extern "system" fn wnd_proc(
            hwnd: WinFoundation::HWND,
            msg: u32,
            wparam: WinFoundation::WPARAM,
            lparam: WinFoundation::LPARAM,
        ) -> WinFoundation::LRESULT {
            unsafe { Wm::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        unsafe {
            let class_name = windows::core::w!("wgpu_interop_example_wgl");
            let wc = Wm::WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                lpszClassName: class_name,
                ..Default::default()
            };
            Wm::RegisterClassW(&wc);

            let hwnd = Wm::CreateWindowExW(
                Wm::WINDOW_EX_STYLE(0),
                class_name,
                windows::core::w!(""),
                Wm::WS_OVERLAPPEDWINDOW,
                0,
                0,
                64,
                64,
                None,
                None,
                None,
                None,
            )
            .expect("CreateWindowEx");

            let hdc = Gdi::GetDC(hwnd);

            let pfd = OpenGL::PIXELFORMATDESCRIPTOR {
                nSize: std::mem::size_of::<OpenGL::PIXELFORMATDESCRIPTOR>() as u16,
                nVersion: 1,
                dwFlags: OpenGL::PFD_DRAW_TO_WINDOW
                    | OpenGL::PFD_SUPPORT_OPENGL
                    | OpenGL::PFD_DOUBLEBUFFER,
                iPixelType: OpenGL::PFD_TYPE_RGBA,
                cColorBits: 32,
                cDepthBits: 24,
                cStencilBits: 8,
                ..Default::default()
            };

            let pf = OpenGL::ChoosePixelFormat(hdc, &pfd);
            assert!(pf != 0, "ChoosePixelFormat failed");
            OpenGL::SetPixelFormat(hdc, pf, &pfd).expect("SetPixelFormat");

            let hglrc = OpenGL::wglCreateContext(hdc).expect("wglCreateContext");
            OpenGL::wglMakeCurrent(hdc, hglrc).expect("wglMakeCurrent");

            let opengl32 = windows::Win32::System::LibraryLoader::LoadLibraryA(windows::core::s!(
                "opengl32.dll"
            ))
            .expect("LoadLibrary opengl32.dll");

            let gl = Arc::new(glow::Context::from_loader_function(|name| {
                let cname = std::ffi::CString::new(name).unwrap();
                let addr =
                    OpenGL::wglGetProcAddress(windows::core::PCSTR(cname.as_ptr() as *const u8));
                match addr {
                    Some(f) => f as *const std::ffi::c_void,
                    None => {
                        let addr = windows::Win32::System::LibraryLoader::GetProcAddress(
                            opengl32,
                            windows::core::PCSTR(cname.as_ptr() as *const u8),
                        );
                        match addr {
                            Some(f) => f as *const std::ffi::c_void,
                            None => std::ptr::null(),
                        }
                    }
                }
            }));

            WglContext {
                gl,
                hglrc,
                hdc,
                hwnd,
            }
        }
    }

    /// Create a GL texture filled with the given RGBA data.
    pub fn create_texture_with_data(
        &self,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> std::num::NonZeroU32 {
        use glow::HasContext;
        unsafe {
            let tex = self.gl.create_texture().expect("glCreateTexture");
            self.gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(data)),
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            self.gl.bind_texture(glow::TEXTURE_2D, None);
            tex.0
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for WglContext {
    fn drop(&mut self) {
        use windows::Win32::Graphics::{Gdi, OpenGL};
        use windows::Win32::UI::WindowsAndMessaging as Wm;
        unsafe {
            OpenGL::wglMakeCurrent(self.hdc, OpenGL::HGLRC::default()).ok();
            OpenGL::wglDeleteContext(self.hglrc).ok();
            Gdi::ReleaseDC(self.hwnd, self.hdc);
            Wm::DestroyWindow(self.hwnd).ok();
        }
    }
}

/// Creates a GL texture with random pixel data via WGL, with D3D11
/// interop for cross-backend import.
///
/// Returns `(texture_name, wgl_context, gl_interop)`. The caller
/// assembles a `GlesTexture` at the import site.
#[cfg(target_os = "windows")]
pub fn create_gles_texture(
    seed: u64,
) -> (std::num::NonZeroU32, WglContext, wgpu_interop::GlInterop) {
    let wgl = WglContext::new();
    let data = solid_pixels(seed);
    let name = wgl.create_texture_with_data(SIZE, SIZE, &data);

    let mut d3d11_device: Option<ID3D11Device> = None;
    unsafe {
        Direct3D11::D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            windows::Win32::Foundation::HMODULE::default(),
            Direct3D11::D3D11_CREATE_DEVICE_FLAG(0),
            None,
            Direct3D11::D3D11_SDK_VERSION,
            Some(&mut d3d11_device as *mut _),
            None,
            None,
        )
        .expect("D3D11CreateDevice");
    }
    let d3d11_device = d3d11_device.unwrap();
    let d3d11_ptr = Interface::into_raw(d3d11_device);

    let interop = unsafe {
        wgpu_interop::GlInterop::new(d3d11_ptr).expect("GlInterop::new")
    };

    (name, wgl, interop)
}
