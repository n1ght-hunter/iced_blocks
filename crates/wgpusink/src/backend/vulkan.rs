use std::any::Any;

use ash::vk;
use wgpu_interop::{DeviceInterop, VulkanImageRaw};

use super::{Backend, ImportResult, SinkError, gst_to_wgpu_format};
use crate::api::WgpuDeviceHandle;
use crate::sink::frame::PooledTexture;

/// Allocator name reported by `gst-vulkan` for image memories. Matches
/// `GST_VULKAN_IMAGE_ALLOCATOR_NAME` in the C headers — used in lieu of a
/// `MemoryType` impl, which `gstreamer-vulkan` v0.25 does not provide.
const VULKAN_IMAGE_ALLOCATOR_NAME: &str = "VulkanImage";

unsafe extern "C" {
    /// Returns the underlying `VkImage` for a `GstVulkanImageMemory`.
    ///
    /// Declared in `gstvkimagememory.h` but not exposed by
    /// `gstreamer-vulkan-sys`, so we declare it directly.
    fn gst_vulkan_image_memory_get_image(
        mem: *mut gst_vulkan::ffi::GstVulkanImageMemory,
    ) -> vk::Image;

    /// Returns the underlying `VkDevice` for a `GstVulkanDevice`.
    ///
    /// Declared in `gstvkdevice.h` but not exposed by `gstreamer-vulkan-sys`.
    fn gst_vulkan_device_get_handle(device: *mut gst_vulkan::ffi::GstVulkanDevice) -> vk::Device;
}

pub(crate) struct VulkanBackend;

impl Backend for VulkanBackend {
    fn try_import(
        &self,
        buffer: &gst::Buffer,
        video_info: &gst_video::VideoInfo,
        device: &WgpuDeviceHandle,
    ) -> Option<ImportResult> {
        let mem = buffer.memory(0)?;
        if !is_vulkan_image_memory(&mem) {
            return None;
        }
        Some(import_vulkan(&mem, video_info, device))
    }
}

fn is_vulkan_image_memory(mem: &gst::Memory) -> bool {
    mem.allocator()
        .is_some_and(|a| a.memory_type().as_str() == VULKAN_IMAGE_ALLOCATOR_NAME)
}

fn import_vulkan(
    mem: &gst::Memory,
    video_info: &gst_video::VideoInfo,
    device: &WgpuDeviceHandle,
) -> ImportResult {
    let format = gst_to_wgpu_format(video_info.format())
        .ok_or_else(|| SinkError::UnsupportedMemory(format!("{:?}", video_info.format())))?;

    let img_mem = mem.as_ptr() as *mut gst_vulkan::ffi::GstVulkanImageMemory;

    let (gst_vk_device, gst_vk_image) = unsafe {
        let dev_ptr = (*img_mem).device;
        if dev_ptr.is_null() {
            return Err(SinkError::Import(
                "GstVulkanImageMemory has null device".into(),
            ));
        }
        let vk_dev = gst_vulkan_device_get_handle(dev_ptr);
        let vk_img = gst_vulkan_image_memory_get_image(img_mem);
        (vk_dev, vk_img)
    };

    // wgpu's VkImage handles are scoped to a specific VkDevice; without sharing
    // the same device, the GStreamer image is meaningless to us. When a future
    // change posts a `gst.vulkan.device` context wrapping wgpu's VkDevice, this
    // check will pass and zero-copy import becomes possible.
    let wgpu_vk_device = unsafe {
        device
            .device
            .as_hal::<wgpu::wgc::api::Vulkan>()
            .map(|hal| hal.raw_device().handle())
            .ok_or(SinkError::WrongBackend)?
    };

    if wgpu_vk_device != gst_vk_device {
        return Err(SinkError::WrongBackend);
    }

    // Heavy but correct sync until gst-vulkan exposes per-buffer semaphores.
    unsafe {
        let hal = device
            .device
            .as_hal::<wgpu::wgc::api::Vulkan>()
            .ok_or(SinkError::WrongBackend)?;
        hal.raw_device()
            .device_wait_idle()
            .map_err(|e| SinkError::Import(format!("vkDeviceWaitIdle failed: {e}")))?;
    }

    let desc = wgpu::TextureDescriptor {
        label: Some("wgpusink_vulkan"),
        size: wgpu::Extent3d {
            width: video_info.width(),
            height: video_info.height(),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    };

    let texture = unsafe {
        device
            .device
            .import_external_texture(
                VulkanImageRaw {
                    image: gst_vk_image,
                },
                &desc,
            )
            .map_err(|e| SinkError::Import(format!("Vulkan image import failed: {e}")))?
    };

    let sync: Box<dyn Any + Send> = Box::new(crate::sync::vulkan::VulkanSemaphoreSync);
    Ok((PooledTexture::unmanaged(texture), Some(sync)))
}
