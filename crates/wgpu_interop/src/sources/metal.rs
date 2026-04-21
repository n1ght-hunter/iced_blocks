//! Metal texture source + GL↔Metal IOSurface interop.
//!
//! Two public types:
//! - [`MetalTexture`] — direct wrap of a caller-owned `metal::Texture`.
//! - [`MetalInterop`] — persistent context that blits a GL framebuffer
//!   into an IOSurface-backed Metal texture, used by non-GL wgpu
//!   Metal backends to accept `GlesTexture` sources.

use std::cell::RefCell;
use std::ffi::c_void;

use glow::HasContext;
use metal::foreign_types::ForeignType;
use objc2_core_foundation::CFRetained;
use objc2_io_surface::IOSurfaceRef;
use wgpu::{Device as WgpuDevice, Texture, TextureDescriptor};

use super::iosurface::create_io_surface_bgra;
use crate::ImportError;

/// A `metal::Texture` to import directly into wgpu.
///
/// `wgpu_interop` re-wraps the texture via the Metal HAL. The caller
/// retains the Metal texture handle through `metal::Texture` (which is
/// an `Arc`-like Objective-C reference) — wgpu takes its own reference
/// during the wrap.
///
/// The `metal::Texture` passed here must come from the same
/// `metal::Device` that backs the wgpu `Device`. Import from a
/// cross-device texture is undefined.
pub struct MetalTexture {
    pub texture: metal::Texture,
}

/// Persistent context for importing a GL framebuffer into a wgpu
/// Metal texture via a shared IOSurface.
///
/// Flow: caller binds the source GL texture to a read framebuffer, then
/// [`import`](Self::import) blits it into an IOSurface-backed GL
/// rectangle texture. The same IOSurface is also the backing of a
/// Metal texture, which is wrapped as a `wgpu::Texture`. Both sides
/// therefore read/write the same GPU memory — zero-copy after the
/// blit.
///
/// The IOSurface and the two textures are allocated lazily on the
/// first [`import`] call and reused across frames of the same size.
/// A size change triggers a full rebuild.
pub struct MetalInterop {
    cgl_context: *mut c_void,
    metal_device: metal::Device,
    state: RefCell<Option<MetalState>>,
}

struct MetalState {
    _io_surface: CFRetained<IOSurfaceRef>,
    gl_texture: glow::NativeTexture,
    wgpu_texture: Texture,
    width: u32,
    height: u32,
}

const GL_BGRA: u32 = 0x80E1;
const GL_UNSIGNED_INT_8_8_8_8_REV: u32 = 0x8367;
const GL_RGBA8: u32 = 0x8058;

impl MetalInterop {
    /// Create a new Metal interop context.
    ///
    /// # Safety
    ///
    /// - `cgl_context` must be a valid `CGLContextObj` and current on
    ///   the calling thread when [`import`](Self::import) is called.
    /// - `metal_device` must be the same `metal::Device` that backs
    ///   the wgpu `Device` used at import time. The wgpu texture
    ///   returned by `import` is only valid against that same device.
    pub unsafe fn new(
        cgl_context: *mut c_void,
        metal_device: metal::Device,
    ) -> Result<Self, ImportError> {
        if cgl_context.is_null() {
            return Err(ImportError::Platform("CGL context is null".into()));
        }
        Ok(Self {
            cgl_context,
            metal_device,
            state: RefCell::new(None),
        })
    }

    /// Blit the bound read framebuffer into an IOSurface-backed
    /// texture and return the Metal-side `wgpu::Texture`.
    ///
    /// `read_fbo` is the framebuffer containing the source GL
    /// texture as `COLOR_ATTACHMENT0`. Pass `None` to read from the
    /// default framebuffer.
    ///
    /// # Safety
    ///
    /// The GL context tied to `cgl_context` must be current on this
    /// thread, and `wgpu_device` must be using the Metal backend
    /// driven by the `metal::Device` passed to [`new`](Self::new).
    pub unsafe fn import(
        &self,
        gl: &glow::Context,
        wgpu_device: &WgpuDevice,
        read_fbo: Option<glow::NativeFramebuffer>,
        width: u32,
        height: u32,
    ) -> Result<Texture, ImportError> {
        let mut state_cell = self.state.borrow_mut();

        let needs_rebuild = match state_cell.as_ref() {
            None => true,
            Some(s) => s.width != width || s.height != height,
        };

        if needs_rebuild {
            if let Some(old) = state_cell.take() {
                unsafe { gl.delete_texture(old.gl_texture) };
            }
            let new_state = unsafe {
                Self::build_state(
                    self.cgl_context,
                    &self.metal_device,
                    gl,
                    wgpu_device,
                    width,
                    height,
                )?
            };
            *state_cell = Some(new_state);
        }

        let state = state_cell.as_ref().expect("state just built");

        unsafe {
            let draw_fbo = gl.create_framebuffer().map_err(ImportError::OpenGL)?;
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(draw_fbo));
            gl.framebuffer_texture_2d(
                glow::DRAW_FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_RECTANGLE,
                Some(state.gl_texture),
                0,
            );

            let blit_result =
                crate::blit_framebuffer(gl, read_fbo, Some(draw_fbo), width as i32, height as i32);

            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
            gl.delete_framebuffer(draw_fbo);
            gl.flush();

            blit_result?;
        }

        Ok(state.wgpu_texture.clone())
    }

    unsafe fn build_state(
        cgl_context: *mut c_void,
        metal_device: &metal::Device,
        gl: &glow::Context,
        wgpu_device: &WgpuDevice,
        width: u32,
        height: u32,
    ) -> Result<MetalState, ImportError> {
        let io_surface = create_io_surface_bgra(width, height)?;

        let gl_texture = unsafe {
            let tex = gl.create_texture().map_err(ImportError::OpenGL)?;
            gl.bind_texture(glow::TEXTURE_RECTANGLE, Some(tex));

            let cgl_err = CGLTexImageIOSurface2D(
                cgl_context,
                glow::TEXTURE_RECTANGLE,
                GL_RGBA8,
                width as i32,
                height as i32,
                GL_BGRA,
                GL_UNSIGNED_INT_8_8_8_8_REV,
                (&*io_surface) as *const IOSurfaceRef as *mut c_void,
                0,
            );
            gl.bind_texture(glow::TEXTURE_RECTANGLE, None);

            if cgl_err != 0 {
                gl.delete_texture(tex);
                return Err(ImportError::Platform(format!(
                    "CGLTexImageIOSurface2D failed: code {cgl_err}"
                )));
            }
            tex
        };

        let metal_texture = unsafe {
            create_metal_texture_from_iosurface(metal_device, &io_surface, 0, width, height)?
        };

        let desc = TextureDescriptor {
            label: Some("wgpu_interop MetalInterop blit target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let wgpu_texture = unsafe {
            crate::backends::metal::wrap_metal_texture(wgpu_device, metal_texture, &desc)?
        };

        Ok(MetalState {
            _io_surface: io_surface,
            gl_texture,
            wgpu_texture,
            width,
            height,
        })
    }
}

// SAFETY: `metal::Device` is a retained Objective-C handle that is
// safe to use from any thread provided it is used from one thread at
// a time. The RefCell ensures `state` is not accessed concurrently.
// `cgl_context` is a raw pointer — the caller is responsible for
// ensuring the CGL context is only used from the thread that
// `import` is called on.
unsafe impl Send for MetalInterop {}

/// Create a `metal::Texture` backed by the given IOSurface via
/// `newTextureWithDescriptor:iosurface:plane:`.
#[allow(unexpected_cfgs)] // objc 0.2's sel_impl! uses legacy `cargo-clippy` cfg
pub(crate) unsafe fn create_metal_texture_from_iosurface(
    metal_device: &metal::Device,
    io_surface: &IOSurfaceRef,
    plane: u32,
    width: u32,
    height: u32,
) -> Result<metal::Texture, ImportError> {
    use metal::objc::runtime::Object;
    use metal::objc::{msg_send, sel, sel_impl};

    let descriptor = metal::TextureDescriptor::new();
    descriptor.set_texture_type(metal::MTLTextureType::D2);
    descriptor.set_pixel_format(metal::MTLPixelFormat::BGRA8Unorm);
    descriptor.set_width(u64::from(width));
    descriptor.set_height(u64::from(height));
    descriptor.set_mipmap_level_count(1);
    descriptor.set_sample_count(1);
    descriptor.set_storage_mode(metal::MTLStorageMode::Private);
    descriptor.set_usage(
        metal::MTLTextureUsage::ShaderRead
            | metal::MTLTextureUsage::RenderTarget
            | metal::MTLTextureUsage::PixelFormatView,
    );

    let iosurface_ptr: *mut c_void = io_surface as *const IOSurfaceRef as *mut c_void;
    let device_ref: &metal::DeviceRef = metal_device;
    let desc_ref: &metal::TextureDescriptorRef = &descriptor;

    unsafe {
        let tex_ptr: *mut Object = msg_send![
            device_ref,
            newTextureWithDescriptor: desc_ref
            iosurface: iosurface_ptr
            plane: u64::from(plane)
        ];

        if tex_ptr.is_null() {
            return Err(ImportError::Platform(
                "newTextureWithDescriptor:iosurface:plane: returned nil".into(),
            ));
        }

        Ok(metal::Texture::from_ptr(tex_ptr as *mut metal::MTLTexture))
    }
}

#[link(name = "OpenGL", kind = "framework")]
unsafe extern "C" {
    /// `CGLTexImageIOSurface2D` — attach an IOSurface to a GL texture.
    ///
    /// The `cgl` crate exposes this too but also pulls in a heavier
    /// set of legacy GL bindings we don't need.
    fn CGLTexImageIOSurface2D(
        ctx: *mut c_void,
        target: u32,
        internal_format: u32,
        width: i32,
        height: i32,
        format: u32,
        ty: u32,
        io_surface: *mut c_void,
        plane: u32,
    ) -> i32;
}
