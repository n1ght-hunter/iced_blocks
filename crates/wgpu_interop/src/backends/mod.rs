//! Backend-specific import implementations.
//!
//! Each sub-module implements [`BackendImport`] for its wgpu backend
//! marker type. The low-level wrapping functions are `pub` and
//! available when the `advanced` feature is enabled.

#[cfg(target_os = "windows")]
pub mod dx12;

#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    all(target_vendor = "apple", feature = "vulkan-portability"),
))]
pub mod vulkan;

#[cfg(target_vendor = "apple")]
pub mod metal;

pub mod gles;

use wgpu::{Texture, TextureDescriptor, TextureUsages};

use crate::{ImportError, TextureSource, TextureSourceTypes};

/// Backend import trait, modeled after `hal::Api`.
///
/// Each wgpu backend marker type implements this to handle
/// `TextureSource` variants it supports.
pub(crate) trait BackendImport: wgpu::hal::Api {
    /// # Safety
    ///
    /// `desc` must accurately describe the native resource in `source`.
    unsafe fn import(
        device: &wgpu::Device,
        hal: &Self::Device,
        source: TextureSource<'_>,
        desc: &TextureDescriptor<'_>,
    ) -> Result<Texture, ImportError>;

    fn supported_sources() -> TextureSourceTypes;
}

/// Map `wgpu::TextureUsages` to `wgpu::hal::TextureUses`.
#[cfg_attr(target_vendor = "apple", allow(dead_code))]
pub fn hal_usage(usage: TextureUsages) -> wgpu::TextureUses {
    let mut hal = wgpu::TextureUses::empty();
    if usage.contains(TextureUsages::TEXTURE_BINDING) {
        hal |= wgpu::TextureUses::RESOURCE;
    }
    if usage.contains(TextureUsages::RENDER_ATTACHMENT) {
        hal |= wgpu::TextureUses::COLOR_TARGET;
    }
    if usage.contains(TextureUsages::COPY_SRC) {
        hal |= wgpu::TextureUses::COPY_SRC;
    }
    if usage.contains(TextureUsages::COPY_DST) {
        hal |= wgpu::TextureUses::COPY_DST;
    }
    if usage.contains(TextureUsages::STORAGE_BINDING) {
        hal |= wgpu::TextureUses::STORAGE_READ_ONLY | wgpu::TextureUses::STORAGE_READ_WRITE;
    }
    hal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hal_usage_texture_binding() {
        let hal = hal_usage(TextureUsages::TEXTURE_BINDING);
        assert!(hal.contains(wgpu::TextureUses::RESOURCE));
        assert!(!hal.contains(wgpu::TextureUses::COLOR_TARGET));
    }

    #[test]
    fn hal_usage_render_attachment() {
        let hal = hal_usage(TextureUsages::RENDER_ATTACHMENT);
        assert!(hal.contains(wgpu::TextureUses::COLOR_TARGET));
        assert!(!hal.contains(wgpu::TextureUses::RESOURCE));
    }

    #[test]
    fn hal_usage_copy_src_dst() {
        let hal = hal_usage(TextureUsages::COPY_SRC | TextureUsages::COPY_DST);
        assert!(hal.contains(wgpu::TextureUses::COPY_SRC));
        assert!(hal.contains(wgpu::TextureUses::COPY_DST));
    }

    #[test]
    fn hal_usage_storage_binding() {
        let hal = hal_usage(TextureUsages::STORAGE_BINDING);
        assert!(hal.contains(wgpu::TextureUses::STORAGE_READ_ONLY));
        assert!(hal.contains(wgpu::TextureUses::STORAGE_READ_WRITE));
    }

    #[test]
    fn hal_usage_combined() {
        let hal = hal_usage(
            TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC,
        );
        assert!(hal.contains(wgpu::TextureUses::RESOURCE));
        assert!(hal.contains(wgpu::TextureUses::COLOR_TARGET));
        assert!(hal.contains(wgpu::TextureUses::COPY_SRC));
        assert!(!hal.contains(wgpu::TextureUses::COPY_DST));
    }

    #[test]
    fn hal_usage_empty() {
        let hal = hal_usage(TextureUsages::empty());
        assert!(hal.is_empty());
    }
}
