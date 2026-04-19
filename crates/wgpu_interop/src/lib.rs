//! Import external GPU textures into wgpu.
//!
//! This crate bridges the gap between wgpu and other GPU APIs by
//! using platform-specific memory sharing mechanisms. Pass any
//! supported source type to [`DeviceInterop::import_external_texture`]
//! (or the free function [`import_external_texture`]):
//!
//! - [`D3D11SharedHandle`] / [`D3D11Interop`] — Windows, via NTHANDLE → D3D12
//! - [`D3D12Resource`] — Windows, direct D3D12 resource wrap
//! - [`VulkanImage`] — all platforms, via opaque fd (Linux) or NTHANDLE (Windows)
//! - [`GlesTexture`] — all platforms, GL texture via GLES HAL or GPU blit
//!
//! # Example
//!
//! ```rust,ignore
//! use wgpu_interop::{DeviceInterop, D3D11SharedHandle};
//! use wgpu::{TextureDescriptor, TextureUsages, TextureDimension, Extent3d};
//!
//! let desc = TextureDescriptor {
//!     label: Some("imported"),
//!     size: Extent3d { width: 1920, height: 1080, depth_or_array_layers: 1 },
//!     format: wgpu::TextureFormat::Rgba8Unorm,
//!     usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
//!     mip_level_count: 1,
//!     sample_count: 1,
//!     dimension: TextureDimension::D2,
//!     view_formats: &[],
//! };
//!
//! // Extension trait style:
//! let texture = unsafe {
//!     wgpu_device.import_external_texture(D3D11SharedHandle { handle: nt_handle }, &desc)?
//! };
//!
//! // Free function style:
//! let texture = unsafe {
//!     wgpu_interop::import_external_texture(&wgpu_device, D3D11SharedHandle { handle: nt_handle }, &desc)?
//! };
//! ```

#[cfg(feature = "advanced")]
pub mod backends;
#[cfg(not(feature = "advanced"))]
pub(crate) mod backends;
mod sources;

mod gl_blit;
use backends::BackendImport;
pub use gl_blit::blit_framebuffer;

// Source types — all platforms
pub use sources::gles::GlesTexture;
pub use sources::vulkan::VulkanImage;

// Source types — Windows
#[cfg(target_os = "windows")]
pub use sources::d3d11::{D3D11Interop, D3D11SharedHandle};
#[cfg(target_os = "windows")]
pub use sources::d3d12::D3D12Resource;

// Source types — Windows/Linux
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub use sources::gl::{GlInterop, GlProcLoader};

/// Errors during external texture import.
#[derive(thiserror::Error, Debug)]
pub enum ImportError {
    /// wgpu is using a different GPU backend than expected.
    #[error("wgpu is not using the expected backend for this platform")]
    WrongBackend,

    /// An OpenGL operation failed.
    #[error("GL error: {0}")]
    OpenGL(String),

    /// A platform-specific interop call failed.
    #[error("{0}")]
    Platform(String),

    /// This import source is not supported on the current platform.
    #[error("not supported on this platform")]
    Unsupported,
}

/// All supported external texture source types.
pub enum TextureSource<'a> {
    #[cfg(target_os = "windows")]
    D3D12Resource(D3D12Resource),
    #[cfg(target_os = "windows")]
    D3D11SharedHandle(D3D11SharedHandle),
    VulkanImage(VulkanImage),
    GlesTexture(GlesTexture<'a>),
}

bitflags::bitflags! {
    /// Supported external texture source types, as a bitfield enum.
    pub struct TextureSourceTypes: u8 {
        #[cfg(target_os = "windows")]
        const D3D12Resource = 1 << 0;
        #[cfg(target_os = "windows")]
        const D3D11SharedHandle = 1 << 1;
        const VulkanImage = 1 << 2;
        const GlesTexture = 1 << 3;
    }
}

#[cfg(target_os = "windows")]
impl From<D3D12Resource> for TextureSource<'_> {
    fn from(r: D3D12Resource) -> Self {
        Self::D3D12Resource(r)
    }
}

#[cfg(target_os = "windows")]
impl From<D3D11SharedHandle> for TextureSource<'_> {
    fn from(h: D3D11SharedHandle) -> Self {
        Self::D3D11SharedHandle(h)
    }
}

impl From<VulkanImage> for TextureSource<'_> {
    fn from(v: VulkanImage) -> Self {
        Self::VulkanImage(v)
    }
}

impl<'a> From<GlesTexture<'a>> for TextureSource<'a> {
    fn from(t: GlesTexture<'a>) -> Self {
        Self::GlesTexture(t)
    }
}

/// Extension trait for importing external textures into wgpu.
pub trait DeviceInterop {
    /// Import an external texture source as a wgpu texture.
    ///
    /// Detects the active wgpu backend and delegates to the
    /// appropriate backend import implementation.
    ///
    /// # Safety
    ///
    /// `desc` must accurately describe the native resource held by
    /// `source` (format, dimensions, usage flags, mip levels, sample
    /// count). A mismatch causes undefined behavior.
    unsafe fn import_external_texture<'a>(
        &self,
        source: impl Into<TextureSource<'a>>,
        desc: &wgpu::TextureDescriptor<'_>,
    ) -> Result<wgpu::Texture, ImportError>;

    /// Query which [`TextureSource`] types the active backend supports.
    fn supported_sources(&self) -> TextureSourceTypes;
}

impl DeviceInterop for wgpu::Device {
    unsafe fn import_external_texture<'a>(
        &self,
        source: impl Into<TextureSource<'a>>,
        desc: &wgpu::TextureDescriptor<'_>,
    ) -> Result<wgpu::Texture, ImportError> {
        let source = source.into();
        unsafe {
            #[cfg(target_os = "windows")]
            if let Some(hal) = self.as_hal::<wgpu::wgc::api::Dx12>() {
                return <wgpu::wgc::api::Dx12 as BackendImport>::import(self, &*hal, source, desc);
            }
            if let Some(hal) = self.as_hal::<wgpu::wgc::api::Vulkan>() {
                return <wgpu::wgc::api::Vulkan as BackendImport>::import(
                    self, &*hal, source, desc,
                );
            }
            if let Some(hal) = self.as_hal::<wgpu::wgc::api::Gles>() {
                return <wgpu::wgc::api::Gles as BackendImport>::import(self, &*hal, source, desc);
            }
            Err(ImportError::WrongBackend)
        }
    }

    fn supported_sources(&self) -> TextureSourceTypes {
        #[cfg(target_os = "windows")]
        if unsafe { self.as_hal::<wgpu::wgc::api::Dx12>().is_some() } {
            return <wgpu::wgc::api::Dx12 as BackendImport>::supported_sources();
        }
        if unsafe { self.as_hal::<wgpu::wgc::api::Vulkan>().is_some() } {
            return <wgpu::wgc::api::Vulkan as BackendImport>::supported_sources();
        }
        if unsafe { self.as_hal::<wgpu::wgc::api::Gles>().is_some() } {
            return <wgpu::wgc::api::Gles as BackendImport>::supported_sources();
        }
        TextureSourceTypes::empty()
    }
}

/// Import an external texture into wgpu.
///
/// Convenience wrapper around [`DeviceInterop::import_external_texture`].
///
/// # Safety
///
/// `desc` must accurately describe the native resource held by
/// `source` (format, dimensions, usage flags, mip levels, sample
/// count). A mismatch causes undefined behavior.
pub unsafe fn import_external_texture<'a>(
    device: &wgpu::Device,
    source: impl Into<TextureSource<'a>>,
    desc: &wgpu::TextureDescriptor<'_>,
) -> Result<wgpu::Texture, ImportError> {
    unsafe { device.import_external_texture(source, desc) }
}
