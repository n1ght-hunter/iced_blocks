/// Import a wgpu texture from Vulkan-exported external memory.
///
/// On Linux the handle is an opaque fd from `vkGetMemoryFdKHR`
/// (`VK_EXTERNAL_MEMORY_HANDLE_TYPE_OPAQUE_FD_BIT`).
///
/// On Windows the handle is an NTHANDLE from `vkGetMemoryWin32HandleKHR`
/// (`VK_EXTERNAL_MEMORY_HANDLE_TYPE_OPAQUE_WIN32_BIT`).
///
/// Ownership of the handle transfers on successful import.
pub struct VulkanImage {
    #[cfg(not(target_os = "windows"))]
    pub fd: i32,
    #[cfg(target_os = "windows")]
    pub handle: windows::Win32::Foundation::HANDLE,
    pub memory_size: u64,
}
