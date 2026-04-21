//! IOSurface texture source — common substrate for Metal ↔ Vulkan
//! interop on Apple Silicon.
//!
//! Both Metal (`newTextureWithDescriptor:iosurface:plane:`) and
//! MoltenVK (`VK_EXT_metal_objects` →
//! `VkImportMetalIOSurfaceInfoEXT`) can wrap an `IOSurface` into their
//! native texture type. Pass an [`IOSurfaceTexture`] to either wgpu
//! backend on macOS for zero-copy import.

use std::ffi::c_void;

use objc2_core_foundation::{CFMutableDictionary, CFNumber, CFNumberType, CFRetained, CFString};
use objc2_io_surface::{
    IOSurfaceRef, kIOSurfaceBytesPerElement, kIOSurfaceHeight, kIOSurfacePixelFormat,
    kIOSurfaceWidth,
};

use crate::ImportError;

/// IOSurface pixel format `'BGRA'` (`0x42475241`). Matches
/// `wgpu::TextureFormat::Bgra8Unorm` and `MTLPixelFormat::BGRA8Unorm`.
pub const IOSURFACE_PIXEL_FORMAT_BGRA: i32 = 0x4247_5241;

/// An `IOSurface` to import as a wgpu texture on Metal or Vulkan/MoltenVK.
///
/// The caller retains ownership of the IOSurface through `CFRetained`.
/// wgpu takes its own reference during the wrap, so dropping the
/// `IOSurfaceTexture` after import is safe.
pub struct IOSurfaceTexture {
    pub surface: CFRetained<IOSurfaceRef>,
    /// Plane index — `0` for single-plane formats like BGRA8.
    pub plane: u32,
}

impl IOSurfaceTexture {
    /// Wrap an existing IOSurface, defaulting `plane` to `0`.
    pub fn new(surface: CFRetained<IOSurfaceRef>) -> Self {
        Self { surface, plane: 0 }
    }
}

/// Allocate a fresh BGRA8 IOSurface of the given size.
pub fn create_io_surface_bgra(
    width: u32,
    height: u32,
) -> Result<CFRetained<IOSurfaceRef>, ImportError> {
    const BYTES_PER_ELEMENT: i32 = 4;

    let dict = CFMutableDictionary::<CFString, CFNumber>::empty();

    let make_num = |v: i64, label: &str| -> Result<CFRetained<CFNumber>, ImportError> {
        // SAFETY: `&v as *const i64` is a valid pointer to an `i64`,
        // matching `CFNumberType::SInt64Type`.
        unsafe {
            CFNumber::new(
                None,
                CFNumberType::SInt64Type,
                &v as *const i64 as *const c_void,
            )
        }
        .ok_or_else(|| ImportError::Platform(format!("CFNumberCreate {label}")))
    };

    let width_num = make_num(i64::from(width), "width")?;
    let height_num = make_num(i64::from(height), "height")?;
    let bpe_num = make_num(i64::from(BYTES_PER_ELEMENT), "bytes-per-element")?;
    let fmt_num = make_num(i64::from(IOSURFACE_PIXEL_FORMAT_BGRA), "pixel-format")?;

    // SAFETY: the `kIOSurface*` keys are extern statics published by the
    // IOSurface framework; they live for the lifetime of the process.
    unsafe {
        dict.set(kIOSurfaceWidth, &width_num);
        dict.set(kIOSurfaceHeight, &height_num);
        dict.set(kIOSurfaceBytesPerElement, &bpe_num);
        dict.set(kIOSurfacePixelFormat, &fmt_num);
    }

    // SAFETY: typed dictionary upcast to its parent CF type.
    let immutable =
        unsafe { CFRetained::cast_unchecked::<objc2_core_foundation::CFDictionary>(dict) };

    // SAFETY: properties dictionary contains only valid CF types
    // matching the IOSurface key requirements.
    unsafe { IOSurfaceRef::new(&immutable) }
        .ok_or_else(|| ImportError::Platform("IOSurfaceCreate returned null".into()))
}
