#![cfg(any(target_os = "windows", target_os = "linux"))]

mod common;

use ash::vk;
use wgpu::TextureUsages;
use wgpu_interop::{DeviceInterop, VulkanImage};

#[cfg(not(target_os = "windows"))]
const HANDLE_TYPE: vk::ExternalMemoryHandleTypeFlags = vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD;
#[cfg(target_os = "windows")]
const HANDLE_TYPE: vk::ExternalMemoryHandleTypeFlags =
    vk::ExternalMemoryHandleTypeFlags::OPAQUE_WIN32;

#[test]
fn import_on_vulkan() {
    let Some((device, _queue)) = common::create_device(wgpu::Backends::VULKAN) else {
        eprintln!("SKIP: Vulkan not available");
        return;
    };

    unsafe {
        let hal = device
            .as_hal::<wgpu::wgc::api::Vulkan>()
            .expect("Vulkan HAL");
        let vk_device = hal.raw_device().clone();
        let vk_instance = hal.shared_instance().raw_instance().clone();

        let mut ext_mem_info =
            vk::ExternalMemoryImageCreateInfo::default().handle_types(HANDLE_TYPE);

        let image = vk_device
            .create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D {
                        width: 64,
                        height: 64,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::COLOR_ATTACHMENT)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .push_next(&mut ext_mem_info),
                None,
            )
            .expect("vkCreateImage");

        let mem_reqs = vk_device.get_image_memory_requirements(image);

        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let mut export_info = vk::ExportMemoryAllocateInfo::default().handle_types(HANDLE_TYPE);

        let memory = vk_device
            .allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(mem_reqs.size)
                    .push_next(&mut dedicated)
                    .push_next(&mut export_info),
                None,
            )
            .expect("vkAllocateMemory");

        vk_device
            .bind_image_memory(image, memory, 0)
            .expect("vkBindImageMemory");

        #[cfg(not(target_os = "windows"))]
        let vulkan_image = {
            let fd_api = ash::khr::external_memory_fd::Device::new(&vk_instance, &vk_device);
            let fd = fd_api
                .get_memory_fd(
                    &vk::MemoryGetFdInfoKHR::default()
                        .memory(memory)
                        .handle_type(HANDLE_TYPE),
                )
                .expect("vkGetMemoryFdKHR");

            // The fd holds its own reference to the underlying GPU memory.
            vk_device.destroy_image(image, None);
            vk_device.free_memory(memory, None);

            VulkanImage {
                fd,
                memory_size: mem_reqs.size,
            }
        };

        #[cfg(target_os = "windows")]
        let vulkan_image = {
            let handle_api = ash::khr::external_memory_win32::Device::new(&vk_instance, &vk_device);
            let raw_handle = handle_api
                .get_memory_win32_handle(
                    &vk::MemoryGetWin32HandleInfoKHR::default()
                        .memory(memory)
                        .handle_type(HANDLE_TYPE),
                )
                .expect("vkGetMemoryWin32HandleKHR");

            vk_device.destroy_image(image, None);
            vk_device.free_memory(memory, None);

            VulkanImage {
                handle: windows::Win32::Foundation::HANDLE(raw_handle as *mut std::ffi::c_void),
                memory_size: mem_reqs.size,
            }
        };

        drop(hal);

        let desc =
            common::test_desc(TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT);
        let tex = device
            .import_external_texture(vulkan_image, &desc)
            .expect("import VulkanImage");

        assert_eq!(tex.width(), 64);
        assert_eq!(tex.height(), 64);
    }
}

#[test]
fn verify_pixels() {
    let Some((device, queue)) = common::create_device(wgpu::Backends::VULKAN) else {
        eprintln!("SKIP: Vulkan not available");
        return;
    };

    let expected = common::checkerboard_rgba(64, 64);

    unsafe {
        let hal = device
            .as_hal::<wgpu::wgc::api::Vulkan>()
            .expect("Vulkan HAL");
        let vk_device = hal.raw_device().clone();
        let vk_instance = hal.shared_instance().raw_instance().clone();
        let vk_phys_device = hal.raw_physical_device();

        let mut ext_mem_info =
            vk::ExternalMemoryImageCreateInfo::default().handle_types(HANDLE_TYPE);

        let image = vk_device
            .create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D {
                        width: 64,
                        height: 64,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(
                        vk::ImageUsageFlags::SAMPLED
                            | vk::ImageUsageFlags::COLOR_ATTACHMENT
                            | vk::ImageUsageFlags::TRANSFER_DST
                            | vk::ImageUsageFlags::TRANSFER_SRC,
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

        // Find a device-local memory type for the image
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

        // Create a host-visible staging buffer
        let staging_size = (64 * 64 * 4) as u64;
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

        // Map and copy checkerboard data
        let mapped = vk_device
            .map_memory(staging_memory, 0, staging_size, vk::MemoryMapFlags::empty())
            .expect("vkMapMemory") as *mut u8;
        std::ptr::copy_nonoverlapping(expected.as_ptr(), mapped, expected.len());
        vk_device.unmap_memory(staging_memory);

        // Upload via command buffer
        let queue_family = hal.queue_family_index();
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

        // Transition image: UNDEFINED → TRANSFER_DST_OPTIMAL
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

        // Copy buffer → image
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
                    width: 64,
                    height: 64,
                    depth: 1,
                },
            }],
        );

        // Transition image: TRANSFER_DST_OPTIMAL → GENERAL
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

        // Clean up Vulkan upload resources
        vk_device.destroy_fence(fence, None);
        vk_device.destroy_command_pool(cmd_pool, None);
        vk_device.destroy_buffer(staging_buffer, None);
        vk_device.free_memory(staging_memory, None);

        // Export handle
        #[cfg(not(target_os = "windows"))]
        let vulkan_image = {
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

            VulkanImage {
                fd,
                memory_size: mem_reqs.size,
            }
        };

        #[cfg(target_os = "windows")]
        let vulkan_image = {
            let handle_api = ash::khr::external_memory_win32::Device::new(&vk_instance, &vk_device);
            let raw_handle = handle_api
                .get_memory_win32_handle(
                    &vk::MemoryGetWin32HandleInfoKHR::default()
                        .memory(memory)
                        .handle_type(HANDLE_TYPE),
                )
                .expect("vkGetMemoryWin32HandleKHR");

            vk_device.destroy_image(image, None);
            vk_device.free_memory(memory, None);

            VulkanImage {
                handle: windows::Win32::Foundation::HANDLE(raw_handle as *mut std::ffi::c_void),
                memory_size: mem_reqs.size,
            }
        };

        drop(hal);

        let desc = common::test_desc(
            TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC,
        );
        let tex = device
            .import_external_texture(vulkan_image, &desc)
            .expect("import VulkanImage");

        let actual = common::readback_texture(&device, &queue, &tex, 64, 64);
        assert_eq!(
            actual, expected,
            "pixel data mismatch after Vulkan image import"
        );
    }
}
