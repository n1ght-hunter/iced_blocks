#[cfg(not(target_os = "windows"))]
use std::cell::RefCell;
#[cfg(target_os = "windows")]
use std::cell::RefCell;

use glow::HasContext;
use wgpu::{Device as WgpuDevice, Texture};

use crate::ImportError;

/// Loads GL extension function pointers not covered by glow.
///
/// Needed for `GL_EXT_memory_object` / `GL_EXT_memory_object_fd`
/// on Linux, where GL imports Vulkan-exported memory.
pub trait GlProcLoader {
    fn get_proc_address(&self, name: &str) -> *const std::ffi::c_void;
}

/// Persistent context for importing GL framebuffer contents into wgpu.
///
/// On Windows (ANGLE/D3D11): blits GL framebuffer → D3D11 shared
/// texture → D3D12 → wgpu.
///
/// On Linux: creates a Vulkan image with exportable memory, imports
/// into GL via `GL_EXT_memory_object_fd`, blits, then wraps as wgpu.
pub struct GlInterop {
    #[cfg(target_os = "windows")]
    inner: WindowsGlInterop,
    #[cfg(not(target_os = "windows"))]
    inner: LinuxGlInterop,
}

#[cfg(target_os = "windows")]
struct WindowsGlInterop {
    d3d11_device: windows::Win32::Graphics::Direct3D11::ID3D11Device,
    d3d11: super::d3d11::D3D11Interop,
    wgl_fns: WglDxInteropFns,
    dx_device_handle: *mut std::ffi::c_void,
    state: RefCell<Option<WindowsGlState>>,
}

#[cfg(target_os = "windows")]
struct WindowsGlState {
    gl_texture: glow::NativeTexture,
    blit_texture: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
    dx_object_handle: *mut std::ffi::c_void,
    width: u32,
    height: u32,
}

/// WGL_NV_DX_interop2 function pointers.
///
/// No crate provides these bindings (`glutin_wgl_sys` doesn't include
/// WGL_NV_DX_interop), so we load them manually like the Linux
/// EXT_memory_object path.
#[cfg(target_os = "windows")]
#[allow(dead_code)] // close_device loaded for completeness but not yet called
struct WglDxInteropFns {
    open_device: unsafe extern "system" fn(*mut std::ffi::c_void) -> *mut std::ffi::c_void,
    close_device: unsafe extern "system" fn(*mut std::ffi::c_void) -> i32,
    register_object: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
        u32,
        u32,
        u32,
    ) -> *mut std::ffi::c_void,
    unregister_object:
        unsafe extern "system" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i32,
    lock_objects:
        unsafe extern "system" fn(*mut std::ffi::c_void, i32, *const *mut std::ffi::c_void) -> i32,
    unlock_objects:
        unsafe extern "system" fn(*mut std::ffi::c_void, i32, *const *mut std::ffi::c_void) -> i32,
}

#[cfg(target_os = "windows")]
const WGL_ACCESS_WRITE_DISCARD_NV: u32 = 0x0002;

#[cfg(not(target_os = "windows"))]
struct LinuxGlInterop {
    state: RefCell<Option<LinuxGlState>>,
    ext_fns: GlExtFunctions,
}

#[cfg(not(target_os = "windows"))]
struct LinuxGlState {
    wgpu_texture: Texture,
    gl_texture: glow::NativeTexture,
    gl_memory_object: u32,
    width: u32,
    height: u32,
}

#[cfg(not(target_os = "windows"))]
struct GlExtFunctions {
    create_memory_objects: unsafe extern "system" fn(i32, *mut u32),
    memory_object_parameter: unsafe extern "system" fn(u32, u32, *const i32),
    import_memory_fd: unsafe extern "system" fn(u32, u64, u32, i32),
    tex_storage_mem_2d: unsafe extern "system" fn(u32, i32, u32, i32, i32, u32, u64),
    delete_memory_objects: unsafe extern "system" fn(i32, *const u32),
}

#[cfg(not(target_os = "windows"))]
const GL_TEXTURE_2D: u32 = 0x0DE1;
#[cfg(not(target_os = "windows"))]
const GL_DEDICATED_MEMORY_OBJECT_EXT: u32 = 0x9581;
#[cfg(not(target_os = "windows"))]
const GL_HANDLE_TYPE_OPAQUE_FD_EXT: u32 = 0x9586;
#[cfg(not(target_os = "windows"))]
const GL_RGBA8: u32 = 0x8058;

impl GlInterop {

    /// Create a new GL interop context on Windows.
    ///
    /// Uses `WGL_NV_DX_interop2` to register D3D11 textures with GL.
    ///
    /// # Safety
    ///
    /// `d3d11_device_ptr` must be a valid `ID3D11Device` COM object.
    /// The GL context must be current and support `WGL_NV_DX_interop2`.
    #[cfg(target_os = "windows")]
    pub unsafe fn new(d3d11_device_ptr: *mut std::ffi::c_void) -> Result<Self, ImportError> {
        unsafe {
            if d3d11_device_ptr.is_null() {
                return Err(ImportError::Platform("D3D11 device pointer is null".into()));
            }

            let load = |name: &str| -> *const std::ffi::c_void {
                let cname = std::ffi::CString::new(name).unwrap();
                let addr = windows::Win32::Graphics::OpenGL::wglGetProcAddress(
                    windows::core::PCSTR(cname.as_ptr() as *const u8),
                );
                match addr {
                    Some(f) => f as *const std::ffi::c_void,
                    None => std::ptr::null(),
                }
            };

            #[allow(clippy::missing_transmute_annotations)]
            let wgl_fns = WglDxInteropFns {
                open_device: std::mem::transmute(load("wglDXOpenDeviceNV")),
                close_device: std::mem::transmute(load("wglDXCloseDeviceNV")),
                register_object: std::mem::transmute(load("wglDXRegisterObjectNV")),
                unregister_object: std::mem::transmute(load("wglDXUnregisterObjectNV")),
                lock_objects: std::mem::transmute(load("wglDXLockObjectsNV")),
                unlock_objects: std::mem::transmute(load("wglDXUnlockObjectsNV")),
            };

            let dx_device_handle = (wgl_fns.open_device)(d3d11_device_ptr);
            if dx_device_handle.is_null() {
                return Err(ImportError::Platform(
                    "wglDXOpenDeviceNV failed (WGL_NV_DX_interop2 not available?)".into(),
                ));
            }

            // from_raw takes ownership; clone adds a reference for us to keep
            let temp: windows::Win32::Graphics::Direct3D11::ID3D11Device =
                windows::core::Interface::from_raw(d3d11_device_ptr);
            let d3d11_device = temp.clone();
            // Give back the original reference for D3D11Interop::new
            let d3d11_device_ptr = windows::core::Interface::into_raw(temp);

            Ok(Self {
                inner: WindowsGlInterop {
                    d3d11_device,
                    d3d11: super::d3d11::D3D11Interop::new(d3d11_device_ptr)?,
                    wgl_fns,
                    dx_device_handle,
                    state: RefCell::new(None),
                },
            })
        }
    }

    /// Create a new GL interop context on Linux.
    ///
    /// # Safety
    ///
    /// The GL context must be current and support `GL_EXT_memory_object_fd`.
    #[cfg(not(target_os = "windows"))]
    pub unsafe fn new(proc_loader: &dyn GlProcLoader) -> Result<Self, ImportError> {
        unsafe {
            #[allow(clippy::missing_transmute_annotations)]
            let ext_fns = GlExtFunctions {
                create_memory_objects: std::mem::transmute(
                    proc_loader.get_proc_address("glCreateMemoryObjectsEXT"),
                ),
                memory_object_parameter: std::mem::transmute(
                    proc_loader.get_proc_address("glMemoryObjectParameterivEXT"),
                ),
                import_memory_fd: std::mem::transmute(
                    proc_loader.get_proc_address("glImportMemoryFdEXT"),
                ),
                tex_storage_mem_2d: std::mem::transmute(
                    proc_loader.get_proc_address("glTexStorageMem2DEXT"),
                ),
                delete_memory_objects: std::mem::transmute(
                    proc_loader.get_proc_address("glDeleteMemoryObjectsEXT"),
                ),
            };

            Ok(Self {
                inner: LinuxGlInterop {
                    state: RefCell::new(None),
                    ext_fns,
                },
            })
        }
    }

    /// Import the current GL framebuffer as a wgpu texture.
    ///
    /// Blits from `read_fbo` (or the default framebuffer if `None`)
    /// into a shared texture, then returns it as a wgpu texture.
    /// Recreates the shared texture if the size changed.
    ///
    /// # Safety
    ///
    /// The GL context must be current.
    pub unsafe fn import(
        &self,
        gl: &glow::Context,
        wgpu_device: &WgpuDevice,
        read_fbo: Option<glow::NativeFramebuffer>,
        width: u32,
        height: u32,
    ) -> Result<Texture, ImportError> {
        unsafe {
            #[cfg(target_os = "windows")]
            {
                self.import_windows(gl, wgpu_device, read_fbo, width, height)
            }
            #[cfg(not(target_os = "windows"))]
            {
                self.import_linux(gl, wgpu_device, read_fbo, width, height)
            }
        }
    }

    #[cfg(target_os = "windows")]
    unsafe fn import_windows(
        &self,
        gl: &glow::Context,
        wgpu_device: &WgpuDevice,
        read_fbo: Option<glow::NativeFramebuffer>,
        width: u32,
        height: u32,
    ) -> Result<Texture, ImportError> {
        use windows::Win32::Graphics::{Direct3D11, Dxgi::Common::*};
        use windows::core::Interface;

        self.inner.d3d11.import(wgpu_device, width, height)?;

        let needs_recreate = self
            .inner
            .state
            .borrow()
            .as_ref()
            .is_none_or(|s| s.width != width || s.height != height);

        if needs_recreate {
            unsafe {
                if let Some(old) = self.inner.state.borrow_mut().take() {
                    (self.inner.wgl_fns.unregister_object)(
                        self.inner.dx_device_handle,
                        old.dx_object_handle,
                    );
                    gl.delete_texture(old.gl_texture);
                }

                // Non-shared D3D11 texture for WGL registration
                let mut blit_texture: Option<Direct3D11::ID3D11Texture2D> = None;
                self.inner
                    .d3d11_device
                    .CreateTexture2D(
                        &Direct3D11::D3D11_TEXTURE2D_DESC {
                            Width: width,
                            Height: height,
                            MipLevels: 1,
                            ArraySize: 1,
                            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                            SampleDesc: DXGI_SAMPLE_DESC {
                                Count: 1,
                                Quality: 0,
                            },
                            Usage: Direct3D11::D3D11_USAGE_DEFAULT,
                            BindFlags: Direct3D11::D3D11_BIND_RENDER_TARGET.0 as u32,
                            MiscFlags: 0,
                            CPUAccessFlags: 0,
                        },
                        None,
                        Some(&mut blit_texture),
                    )
                    .map_err(|e| ImportError::Platform(format!("CreateTexture2D (blit): {e}")))?;
                let blit_texture = blit_texture.unwrap();

                let gl_texture = gl.create_texture().map_err(ImportError::OpenGL)?;

                let d3d11_raw = Interface::as_raw(&blit_texture);
                let dx_object_handle = (self.inner.wgl_fns.register_object)(
                    self.inner.dx_device_handle,
                    d3d11_raw,
                    gl_texture.0.get(),
                    glow::TEXTURE_2D,
                    WGL_ACCESS_WRITE_DISCARD_NV,
                );
                if dx_object_handle.is_null() {
                    gl.delete_texture(gl_texture);
                    return Err(ImportError::Platform("wglDXRegisterObjectNV failed".into()));
                }

                *self.inner.state.borrow_mut() = Some(WindowsGlState {
                    gl_texture,
                    blit_texture,
                    dx_object_handle,
                    width,
                    height,
                });
            }
        }

        let state = self.inner.state.borrow();
        let s = state.as_ref().unwrap();

        unsafe {
            // Lock the DX object for GL access
            (self.inner.wgl_fns.lock_objects)(self.inner.dx_device_handle, 1, &s.dx_object_handle);

            // Blit from source FBO into the registered GL texture
            let draw_fbo = gl.create_framebuffer().map_err(ImportError::OpenGL)?;
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(draw_fbo));
            gl.framebuffer_texture_2d(
                glow::DRAW_FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(s.gl_texture),
                0,
            );

            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, read_fbo);
            let (w, h) = (width as i32, height as i32);
            gl.blit_framebuffer(
                0,
                0,
                w,
                h,
                0,
                h,
                w,
                0,
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );
            gl.flush();

            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
            gl.delete_framebuffer(draw_fbo);

            // Unlock — changes visible to D3D11
            (self.inner.wgl_fns.unlock_objects)(
                self.inner.dx_device_handle,
                1,
                &s.dx_object_handle,
            );

            // Copy from blit texture to the shared texture
            let d3d11_shared = self
                .inner
                .d3d11
                .d3d11_texture()
                .ok_or_else(|| ImportError::Platform("no shared texture".into()))?;
            let ctx = self
                .inner
                .d3d11_device
                .GetImmediateContext()
                .map_err(|e| ImportError::Platform(format!("GetImmediateContext: {e}")))?;
            ctx.CopyResource(&d3d11_shared, &s.blit_texture);
            ctx.Flush();
        }

        self.inner.d3d11.import(wgpu_device, width, height)
    }

    #[cfg(not(target_os = "windows"))]
    unsafe fn import_linux(
        &self,
        gl: &glow::Context,
        wgpu_device: &WgpuDevice,
        read_fbo: Option<glow::NativeFramebuffer>,
        width: u32,
        height: u32,
    ) -> Result<Texture, ImportError> {
        let needs_recreate = self
            .inner
            .state
            .borrow()
            .as_ref()
            .is_none_or(|s| s.width != width || s.height != height);

        if needs_recreate {
            if let Some(old) = self.inner.state.borrow_mut().take() {
                unsafe {
                    gl.delete_texture(old.gl_texture);
                    (self.inner.ext_fns.delete_memory_objects)(1, &old.gl_memory_object);
                }
            }

            let interop_desc = wgpu::TextureDescriptor {
                label: Some("GlInterop shared texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                view_formats: &[],
            };

            // Safety: interop_desc is constructed here to match the image we create.
            let (fd, memory_size, wgpu_texture) = unsafe {
                crate::backends::vulkan::create_exportable_image_fd(wgpu_device, &interop_desc)?
            };

            unsafe {
                let mut gl_memory_object = 0u32;
                (self.inner.ext_fns.create_memory_objects)(1, &mut gl_memory_object);
                (self.inner.ext_fns.memory_object_parameter)(
                    gl_memory_object,
                    GL_DEDICATED_MEMORY_OBJECT_EXT,
                    &1i32,
                );
                (self.inner.ext_fns.import_memory_fd)(
                    gl_memory_object,
                    memory_size,
                    GL_HANDLE_TYPE_OPAQUE_FD_EXT,
                    fd,
                );

                let gl_texture = gl.create_texture().map_err(ImportError::OpenGL)?;
                gl.bind_texture(GL_TEXTURE_2D, Some(gl_texture));
                (self.inner.ext_fns.tex_storage_mem_2d)(
                    GL_TEXTURE_2D,
                    1,
                    GL_RGBA8,
                    width as i32,
                    height as i32,
                    gl_memory_object,
                    0,
                );
                gl.bind_texture(GL_TEXTURE_2D, None);

                *self.inner.state.borrow_mut() = Some(LinuxGlState {
                    wgpu_texture,
                    gl_texture,
                    gl_memory_object,
                    width,
                    height,
                });
            }
        }

        let state = self.inner.state.borrow();
        let s = state.as_ref().unwrap();

        unsafe {
            let draw_fbo = gl.create_framebuffer().map_err(ImportError::OpenGL)?;
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(draw_fbo));
            gl.framebuffer_texture_2d(
                glow::DRAW_FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                GL_TEXTURE_2D,
                Some(s.gl_texture),
                0,
            );

            crate::blit_framebuffer(
                gl,
                read_fbo,
                Some(draw_fbo),
                width as i32,
                height as i32,
            )?;

            gl.delete_framebuffer(draw_fbo);
        }

        Ok(s.wgpu_texture.clone())
    }
}
