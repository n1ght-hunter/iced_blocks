use ash::vk;
use wgpu::{Device as WgpuDevice, Texture, TextureDescriptor, TextureFormat, TextureUsages};

use super::BackendImport;
use crate::{ImportError, TextureSource, TextureSourceTypes};

impl BackendImport for wgpu::wgc::api::Vulkan {
    fn supported_sources() -> TextureSourceTypes {
        let mut types = TextureSourceTypes::VulkanImage | TextureSourceTypes::VulkanImageRaw;
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            types |= TextureSourceTypes::GlesTexture;
        }
        #[cfg(target_vendor = "apple")]
        {
            types |= TextureSourceTypes::IOSurfaceTexture;
        }
        types
    }

    unsafe fn import(
        device: &WgpuDevice,
        hal: &Self::Device,
        source: TextureSource<'_>,
        desc: &TextureDescriptor<'_>,
    ) -> Result<Texture, ImportError> {
        match source {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            TextureSource::VulkanImage(img) => unsafe {
                import_vulkan_image(device, hal, &img, desc)
            },
            TextureSource::VulkanImageRaw(img) => unsafe {
                wrap_image(device, hal, img.image, desc, None)
            },
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            TextureSource::GlesTexture(tex) => unsafe {
                super::gles::import_gles_via_blit(device, &tex, desc.size.width, desc.size.height)
            },
            #[cfg(target_vendor = "apple")]
            TextureSource::IOSurfaceTexture(s) => unsafe {
                import_iosurface_as_vkimage(device, hal, &s.surface, s.plane, desc)
            },
            _ => Err(ImportError::Unsupported),
        }
    }
}

/// Import an `IOSurface` as a wgpu Vulkan texture via MoltenVK's
/// `VK_EXT_metal_objects` extension.
///
/// `VkImportMetalIOSurfaceInfoEXT` has no plane field — the whole
/// IOSurface is bound as the image's backing memory. Multi-plane
/// IOSurfaces (e.g. NV12) aren't representable through this single
/// import; callers wanting per-plane VkImages must `vkCreateImage`
/// once per plane externally. The `plane` argument is therefore
/// ignored on Vulkan and only meaningful for the Metal backend.
///
/// # Safety
///
/// `desc` must accurately describe `surface`'s format and size. The
/// wgpu `device` must be using the Vulkan backend on a MoltenVK
/// driver. Caller is responsible for ensuring `VK_EXT_metal_objects`
/// is available on the device.
#[cfg(target_vendor = "apple")]
unsafe fn import_iosurface_as_vkimage(
    device: &WgpuDevice,
    hal: &<wgpu::wgc::api::Vulkan as wgpu::hal::Api>::Device,
    surface: &objc2_io_surface::IOSurfaceRef,
    _plane: u32,
    desc: &TextureDescriptor<'_>,
) -> Result<Texture, ImportError> {
    let vk_format = texture_format_to_vk(desc.format);
    let vk_usage = vk_image_usage(desc.usage);
    let iosurface_ptr = surface as *const objc2_io_surface::IOSurfaceRef as *mut std::ffi::c_void;

    unsafe {
        let vulkan_device = hal.raw_device().clone();

        let mut import_info = vk::ImportMetalIOSurfaceInfoEXT::default().io_surface(iosurface_ptr);

        let image = vulkan_device
            .create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk_format)
                    .extent(vk::Extent3D {
                        width: desc.size.width,
                        height: desc.size.height,
                        depth: 1,
                    })
                    .mip_levels(desc.mip_level_count)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk_usage)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .push_next(&mut import_info),
                None,
            )
            .map_err(|e| ImportError::Platform(format!("vkCreateImage (IOSurface): {e}")))?;

        // MoltenVK auto-binds IOSurface-backed memory at vkCreateImage
        // time when `VkImportMetalIOSurfaceInfoEXT` is in the pNext
        // chain. No vkBindImageMemory needed.

        let vk_device_clone = vulkan_device.clone();
        wrap_image(
            device,
            hal,
            image,
            desc,
            Some(Box::new(move || {
                vk_device_clone.destroy_image(image, None);
            })),
        )
    }
}

/// Import a Vulkan image from pre-allocated external memory.
///
/// On Linux, imports via `VK_KHR_external_memory_fd` (opaque fd).
/// On Windows, imports via `VK_KHR_external_memory_win32` (opaque NTHANDLE).
/// Not available on Apple — use `IOSurfaceTexture` instead.
///
/// # Safety
///
/// `desc` must accurately describe the image behind the handle.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub unsafe fn import_vulkan_image(
    device: &WgpuDevice,
    hal: &<wgpu::wgc::api::Vulkan as wgpu::hal::Api>::Device,
    img: &crate::VulkanImage,
    desc: &TextureDescriptor<'_>,
) -> Result<Texture, ImportError> {
    let vk_format = texture_format_to_vk(desc.format);
    let vk_usage = vk_image_usage(desc.usage);

    #[cfg(not(target_os = "windows"))]
    let handle_type = vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD;
    #[cfg(target_os = "windows")]
    let handle_type = vk::ExternalMemoryHandleTypeFlags::OPAQUE_WIN32;

    unsafe {
        let vulkan_device = hal.raw_device().clone();

        let mut external_memory_info =
            vk::ExternalMemoryImageCreateInfo::default().handle_types(handle_type);

        let image = vulkan_device
            .create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk_format)
                    .extent(vk::Extent3D {
                        width: desc.size.width,
                        height: desc.size.height,
                        depth: desc.size.depth_or_array_layers,
                    })
                    .mip_levels(desc.mip_level_count)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk_usage)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .push_next(&mut external_memory_info),
                None,
            )
            .map_err(|e| ImportError::Platform(format!("vkCreateImage: {e}")))?;

        let mut dedicated_info = vk::MemoryDedicatedAllocateInfo::default().image(image);

        #[cfg(not(target_os = "windows"))]
        let memory = {
            let mut import_info = vk::ImportMemoryFdInfoKHR::default()
                .handle_type(handle_type)
                .fd(img.fd);
            vulkan_device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(img.memory_size)
                        .push_next(&mut dedicated_info)
                        .push_next(&mut import_info),
                    None,
                )
                .map_err(|e| ImportError::Platform(format!("vkAllocateMemory: {e}")))?
        };

        #[cfg(target_os = "windows")]
        let memory = {
            let mut import_info = vk::ImportMemoryWin32HandleInfoKHR::default()
                .handle_type(handle_type)
                .handle(img.handle.0 as isize);
            vulkan_device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(img.memory_size)
                        .push_next(&mut dedicated_info)
                        .push_next(&mut import_info),
                    None,
                )
                .map_err(|e| ImportError::Platform(format!("vkAllocateMemory: {e}")))?
        };

        vulkan_device
            .bind_image_memory(image, memory, 0)
            .map_err(|e| ImportError::Platform(format!("vkBindImageMemory: {e}")))?;

        let vk_device_clone = vulkan_device.clone();
        wrap_image(
            device,
            hal,
            image,
            desc,
            Some(Box::new(move || {
                vk_device_clone.destroy_image(image, None);
                vk_device_clone.free_memory(memory, None);
            })),
        )
    }
}

/// Map `wgpu::TextureUsages` to Vulkan image usage flags.
pub fn vk_image_usage(usage: TextureUsages) -> vk::ImageUsageFlags {
    let mut flags = vk::ImageUsageFlags::empty();
    if usage.contains(TextureUsages::TEXTURE_BINDING) {
        flags |= vk::ImageUsageFlags::SAMPLED;
    }
    if usage.contains(TextureUsages::RENDER_ATTACHMENT) {
        flags |= vk::ImageUsageFlags::COLOR_ATTACHMENT;
    }
    if usage.contains(TextureUsages::COPY_SRC) {
        flags |= vk::ImageUsageFlags::TRANSFER_SRC;
    }
    if usage.contains(TextureUsages::COPY_DST) {
        flags |= vk::ImageUsageFlags::TRANSFER_DST;
    }
    if usage.contains(TextureUsages::STORAGE_BINDING) {
        flags |= vk::ImageUsageFlags::STORAGE;
    }
    flags
}

/// Wrap a Vulkan image (with pre-bound external memory) as a wgpu texture.
///
/// # Safety
///
/// `desc` must accurately describe `image`. The wgpu `device` must
/// be using the Vulkan backend.
pub unsafe fn wrap_image(
    device: &WgpuDevice,
    hal_device: &<wgpu::wgc::api::Vulkan as wgpu::hal::Api>::Device,
    image: vk::Image,
    desc: &TextureDescriptor<'_>,
    drop_callback: Option<Box<dyn FnOnce() + Send + Sync>>,
) -> Result<Texture, ImportError> {
    unsafe {
        Ok(device.create_texture_from_hal::<wgpu::wgc::api::Vulkan>(
            hal_device.texture_from_raw(
                image,
                &wgpu::hal::TextureDescriptor {
                    label: None,
                    size: desc.size,
                    format: desc.format,
                    dimension: desc.dimension,
                    mip_level_count: desc.mip_level_count,
                    sample_count: desc.sample_count,
                    usage: super::hal_usage(desc.usage),
                    view_formats: desc.view_formats.to_vec(),
                    memory_flags: wgpu::hal::MemoryFlags::empty(),
                },
                drop_callback,
            ),
            desc,
        ))
    }
}

/// Create a Vulkan image with exportable external memory, export an
/// opaque fd, and wrap as a wgpu texture.
///
/// Returns `(exported_fd, memory_size, wgpu_texture)`. The fd can be
/// imported into GL via `GL_EXT_memory_object_fd`. The Vulkan
/// image/memory are cleaned up when the wgpu texture is dropped.
///
/// # Safety
///
/// `desc` must be a valid texture descriptor for the created image.
#[cfg(target_os = "linux")]
pub unsafe fn create_exportable_image_fd(
    device: &WgpuDevice,
    desc: &TextureDescriptor<'_>,
) -> Result<(i32, u64, Texture), ImportError> {
    let vk_format = texture_format_to_vk(desc.format);
    let handle_type = vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD;

    unsafe {
        let hal_device = device
            .as_hal::<wgpu::wgc::api::Vulkan>()
            .ok_or(ImportError::WrongBackend)?;
        let vulkan_device = hal_device.raw_device().clone();
        let vulkan_instance = hal_device.shared_instance().raw_instance();

        let mut external_memory_info =
            vk::ExternalMemoryImageCreateInfo::default().handle_types(handle_type);

        let image = vulkan_device
            .create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk_format)
                    .extent(vk::Extent3D {
                        width: desc.size.width,
                        height: desc.size.height,
                        depth: desc.size.depth_or_array_layers,
                    })
                    .mip_levels(desc.mip_level_count)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk_image_usage(desc.usage))
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .push_next(&mut external_memory_info),
                None,
            )
            .map_err(|e| ImportError::Platform(format!("vkCreateImage: {e}")))?;

        let memory_requirements = vulkan_device.get_image_memory_requirements(image);

        let mut dedicated_info = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let mut export_info = vk::ExportMemoryAllocateInfo::default().handle_types(handle_type);

        let memory = vulkan_device
            .allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(memory_requirements.size)
                    .push_next(&mut dedicated_info)
                    .push_next(&mut export_info),
                None,
            )
            .map_err(|e| ImportError::Platform(format!("vkAllocateMemory: {e}")))?;

        vulkan_device
            .bind_image_memory(image, memory, 0)
            .map_err(|e| ImportError::Platform(format!("vkBindImageMemory: {e}")))?;

        let fd_api = ash::khr::external_memory_fd::Device::new(vulkan_instance, &vulkan_device);
        let fd = fd_api
            .get_memory_fd(
                &vk::MemoryGetFdInfoKHR::default()
                    .memory(memory)
                    .handle_type(handle_type),
            )
            .map_err(|e| ImportError::Platform(format!("vkGetMemoryFdKHR: {e}")))?;

        let vk_device_clone = vulkan_device.clone();
        let texture = wrap_image(
            device,
            &hal_device,
            image,
            desc,
            Some(Box::new(move || {
                vk_device_clone.destroy_image(image, None);
                vk_device_clone.free_memory(memory, None);
            })),
        )?;

        Ok((fd, memory_requirements.size, texture))
    }
}

/// Map a `wgpu::TextureFormat` to the equivalent `vk::Format`.
pub fn texture_format_to_vk(format: TextureFormat) -> vk::Format {
    match format {
        TextureFormat::Rgba8Unorm => vk::Format::R8G8B8A8_UNORM,
        TextureFormat::Bgra8Unorm => vk::Format::B8G8R8A8_UNORM,
        TextureFormat::Rgba8UnormSrgb => vk::Format::R8G8B8A8_SRGB,
        TextureFormat::Bgra8UnormSrgb => vk::Format::B8G8R8A8_SRGB,
        TextureFormat::Rgba16Float => vk::Format::R16G16B16A16_SFLOAT,
        _ => vk::Format::R8G8B8A8_UNORM,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vk_image_usage_sampled() {
        let flags = vk_image_usage(TextureUsages::TEXTURE_BINDING);
        assert!(flags.contains(vk::ImageUsageFlags::SAMPLED));
        assert!(!flags.contains(vk::ImageUsageFlags::COLOR_ATTACHMENT));
    }

    #[test]
    fn vk_image_usage_render_attachment() {
        let flags = vk_image_usage(TextureUsages::RENDER_ATTACHMENT);
        assert!(flags.contains(vk::ImageUsageFlags::COLOR_ATTACHMENT));
    }

    #[test]
    fn vk_image_usage_copy_src_dst() {
        let flags = vk_image_usage(TextureUsages::COPY_SRC | TextureUsages::COPY_DST);
        assert!(flags.contains(vk::ImageUsageFlags::TRANSFER_SRC));
        assert!(flags.contains(vk::ImageUsageFlags::TRANSFER_DST));
    }

    #[test]
    fn vk_image_usage_storage() {
        let flags = vk_image_usage(TextureUsages::STORAGE_BINDING);
        assert!(flags.contains(vk::ImageUsageFlags::STORAGE));
    }

    #[test]
    fn vk_image_usage_combined() {
        let flags = vk_image_usage(
            TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC,
        );
        assert!(flags.contains(vk::ImageUsageFlags::SAMPLED));
        assert!(flags.contains(vk::ImageUsageFlags::COLOR_ATTACHMENT));
        assert!(flags.contains(vk::ImageUsageFlags::TRANSFER_SRC));
        assert!(!flags.contains(vk::ImageUsageFlags::TRANSFER_DST));
    }

    #[test]
    fn vk_image_usage_empty() {
        let flags = vk_image_usage(TextureUsages::empty());
        assert!(flags.is_empty());
    }

    #[test]
    fn texture_format_rgba8() {
        assert_eq!(
            texture_format_to_vk(TextureFormat::Rgba8Unorm),
            vk::Format::R8G8B8A8_UNORM
        );
    }

    #[test]
    fn texture_format_bgra8() {
        assert_eq!(
            texture_format_to_vk(TextureFormat::Bgra8Unorm),
            vk::Format::B8G8R8A8_UNORM
        );
    }

    #[test]
    fn texture_format_srgb() {
        assert_eq!(
            texture_format_to_vk(TextureFormat::Rgba8UnormSrgb),
            vk::Format::R8G8B8A8_SRGB
        );
        assert_eq!(
            texture_format_to_vk(TextureFormat::Bgra8UnormSrgb),
            vk::Format::B8G8R8A8_SRGB
        );
    }

    #[test]
    fn texture_format_rgba16float() {
        assert_eq!(
            texture_format_to_vk(TextureFormat::Rgba16Float),
            vk::Format::R16G16B16A16_SFLOAT
        );
    }

    #[test]
    fn texture_format_unsupported_falls_back() {
        assert_eq!(
            texture_format_to_vk(TextureFormat::R8Unorm),
            vk::Format::R8G8B8A8_UNORM
        );
    }
}
