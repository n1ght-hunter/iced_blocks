use std::num::NonZeroU32;

#[cfg(any(target_os = "windows", target_os = "linux"))]
use super::gl::GlInterop;
#[cfg(target_vendor = "apple")]
use super::metal::MetalInterop;

/// Import a GL texture into wgpu.
///
/// On the GLES backend, this wraps the texture directly via the HAL.
/// On other backends (DX12, Vulkan, Metal), it uses the
/// platform-specific interop context to GPU-blit the texture into a
/// shared resource. The `interop` field must be `Some` when the wgpu
/// backend is not GLES.
///
/// # Safety (caller responsibility)
///
/// The GL context must be current when calling `import`.
pub struct GlesTexture<'a> {
    pub gl: &'a glow::Context,
    pub name: NonZeroU32,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub interop: Option<&'a GlInterop>,
    #[cfg(target_vendor = "apple")]
    pub interop: Option<&'a MetalInterop>,
}
