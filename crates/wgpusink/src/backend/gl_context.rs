//! Internal GL context wrapper used by the GL backend.
//!
//! Owns a real GL context (raw WGL on Windows, surfman/EGL on Linux) plus a
//! cached `wgpu_interop::GlInterop` for cross-backend blits and a wrapped
//! `gst_gl::GLContext` posted to upstream so decoders share resources with us.
//!
//! macOS is currently unsupported — `wgpu_interop` does not yet implement the
//! IOSurface bridge for GL-backed Metal devices.

#![cfg(feature = "gl")]

use std::sync::Arc;

use wgpu_interop::GlInterop;

/// Wraps the GL context and associated interop state for one sink instance.
///
/// Created lazily on the first frame that arrives as `memory:GLMemory`. If
/// initialization fails on the current platform, the GL backend silently falls
/// back to sysmem.
pub(crate) struct GlContextBundle {
    pub(crate) glow: Arc<glow::Context>,
    pub(crate) gl_interop: GlInterop,
    /// Posted to upstream via `gst.gl.GLDisplay`.
    pub(crate) gst_gl_display: gst_gl::GLDisplay,
    /// Posted to upstream via `gst.gl.app_context`.
    pub(crate) gst_gl_context: gst_gl::GLContext,
    /// Platform-specific resources kept alive for the GL context's lifetime.
    _platform: PlatformContext,
}

// SAFETY: A `GlContextBundle` is created in `BaseSink::set_caps` and used
// only in `VideoSink::show_frame`, both of which GStreamer guarantees run on
// the same streaming thread for the lifetime of a sink instance. The
// `Mutex<Option<Box<dyn Backend>>>` on `WgpuVideoSinkImp` further serializes
// any access. The GL/D3D11 handles inside are never touched concurrently.
unsafe impl Send for GlContextBundle {}
unsafe impl Sync for GlContextBundle {}

impl GlContextBundle {
    /// Build a GL context bundle on the current thread.
    ///
    /// Returns `None` on platforms or configurations where the cross-backend
    /// blit cannot be set up (notably macOS, headless Linux without a display).
    pub(crate) fn new(wgpu_device: &wgpu::Device) -> Option<Self> {
        let _ = wgpu_device;
        platform::create()
    }
}

/// Platform-specific GL context that owns the OS-level GL resources.
struct PlatformContext {
    #[cfg(target_os = "windows")]
    #[allow(dead_code)]
    inner: platform::WglContext,
    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    inner: platform::SurfmanContext,
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    _phantom: std::marker::PhantomData<()>,
}

#[cfg(target_os = "windows")]
mod platform {
    use std::sync::Arc;

    use gst_gl::prelude::*;
    use wgpu_interop::GlInterop;
    use windows::Win32::Foundation::{self, HMODULE};
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{self, ID3D11Device};
    use windows::Win32::Graphics::Gdi;
    use windows::Win32::Graphics::OpenGL;
    use windows::Win32::System::LibraryLoader;
    use windows::Win32::UI::WindowsAndMessaging as Wm;
    use windows::core::Interface;

    /// Owns a hidden HWND + HDC + HGLRC for the lifetime of the bundle.
    pub(super) struct WglContext {
        hglrc: OpenGL::HGLRC,
        hdc: Gdi::HDC,
        hwnd: Foundation::HWND,
    }

    impl Drop for WglContext {
        fn drop(&mut self) {
            unsafe {
                let _ = OpenGL::wglMakeCurrent(self.hdc, OpenGL::HGLRC::default());
                let _ = OpenGL::wglDeleteContext(self.hglrc);
                Gdi::ReleaseDC(self.hwnd, self.hdc);
                let _ = Wm::DestroyWindow(self.hwnd);
            }
        }
    }

    pub(super) fn create() -> Option<super::GlContextBundle> {
        unsafe {
            let (hwnd, hdc, hglrc) = create_wgl_context()?;

            let opengl32 = LibraryLoader::LoadLibraryA(windows::core::s!("opengl32.dll")).ok()?;
            let load = move |name: &str| -> *const std::ffi::c_void {
                let cname = std::ffi::CString::new(name).unwrap();
                let pcstr = windows::core::PCSTR(cname.as_ptr() as *const u8);
                if let Some(f) = OpenGL::wglGetProcAddress(pcstr) {
                    f as *const _
                } else if let Some(f) = LibraryLoader::GetProcAddress(opengl32, pcstr) {
                    f as *const _
                } else {
                    std::ptr::null()
                }
            };
            let glow = Arc::new(glow::Context::from_loader_function(&load));

            // Separate D3D11 device for WGL_NV_DX_interop2.
            let mut d3d11_device: Option<ID3D11Device> = None;
            Direct3D11::D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                Direct3D11::D3D11_CREATE_DEVICE_FLAG(0),
                None,
                Direct3D11::D3D11_SDK_VERSION,
                Some(&mut d3d11_device as *mut _),
                None,
                None,
            )
            .ok()?;
            let d3d11_device = d3d11_device?;
            let d3d11_ptr = Interface::into_raw(d3d11_device);

            let gl_interop = GlInterop::new(d3d11_ptr).ok()?;

            let gst_gl_display = gst_gl::GLDisplay::new();
            let gst_gl_context = gst_gl::GLContext::new_wrapped(
                &gst_gl_display,
                hglrc.0 as usize,
                gst_gl::GLPlatform::WGL,
                gst_gl::GLAPI::OPENGL3,
            )?;
            gst_gl_context.activate(true).ok()?;
            gst_gl_context.fill_info().ok()?;

            Some(super::GlContextBundle {
                glow,
                gl_interop,
                gst_gl_display,
                gst_gl_context,
                _platform: super::PlatformContext {
                    inner: WglContext { hglrc, hdc, hwnd },
                },
            })
        }
    }

    unsafe fn create_wgl_context() -> Option<(Foundation::HWND, Gdi::HDC, OpenGL::HGLRC)> {
        unsafe {
            unsafe extern "system" fn wnd_proc(
                hwnd: Foundation::HWND,
                msg: u32,
                wparam: Foundation::WPARAM,
                lparam: Foundation::LPARAM,
            ) -> Foundation::LRESULT {
                unsafe { Wm::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }

            let class_name = windows::core::w!("wgpusink_gl_offscreen");
            let wc = Wm::WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                lpszClassName: class_name,
                ..Default::default()
            };
            // Ignore failure if the class is already registered.
            Wm::RegisterClassW(&wc);

            let hwnd = Wm::CreateWindowExW(
                Wm::WINDOW_EX_STYLE(0),
                class_name,
                windows::core::w!(""),
                Wm::WS_OVERLAPPEDWINDOW,
                0,
                0,
                64,
                64,
                None,
                None,
                None,
                None,
            )
            .ok()?;

            let hdc = Gdi::GetDC(hwnd);
            if hdc.is_invalid() {
                let _ = Wm::DestroyWindow(hwnd);
                return None;
            }

            let pfd = OpenGL::PIXELFORMATDESCRIPTOR {
                nSize: std::mem::size_of::<OpenGL::PIXELFORMATDESCRIPTOR>() as u16,
                nVersion: 1,
                dwFlags: OpenGL::PFD_DRAW_TO_WINDOW
                    | OpenGL::PFD_SUPPORT_OPENGL
                    | OpenGL::PFD_DOUBLEBUFFER,
                iPixelType: OpenGL::PFD_TYPE_RGBA,
                cColorBits: 32,
                cDepthBits: 24,
                cStencilBits: 8,
                ..Default::default()
            };

            let pf = OpenGL::ChoosePixelFormat(hdc, &pfd);
            if pf == 0 {
                Gdi::ReleaseDC(hwnd, hdc);
                let _ = Wm::DestroyWindow(hwnd);
                return None;
            }
            OpenGL::SetPixelFormat(hdc, pf, &pfd).ok()?;

            let hglrc = OpenGL::wglCreateContext(hdc).ok()?;
            OpenGL::wglMakeCurrent(hdc, hglrc).ok()?;

            Some((hwnd, hdc, hglrc))
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::sync::Arc;

    use gst_gl::prelude::*;
    use surfman::{Connection, ContextAttributeFlags, ContextAttributes, GLVersion};
    use wgpu_interop::{GlInterop, GlProcLoader};

    pub(super) struct SurfmanContext {
        device: surfman::Device,
        context: Option<surfman::Context>,
    }

    impl Drop for SurfmanContext {
        fn drop(&mut self) {
            if let Some(ctx) = self.context.take() {
                let _ = self.device.destroy_context(&mut { ctx });
            }
        }
    }

    struct SurfmanProcLoader<'a> {
        device: &'a surfman::Device,
        context: &'a surfman::Context,
    }

    impl<'a> GlProcLoader for SurfmanProcLoader<'a> {
        fn get_proc_address(&self, name: &str) -> *const std::ffi::c_void {
            self.device.get_proc_address(self.context, name) as *const _
        }
    }

    pub(super) fn create() -> Option<super::GlContextBundle> {
        let connection = Connection::new().ok()?;
        let adapter = connection.create_hardware_adapter().ok()?;
        let mut device = connection.create_device(&adapter).ok()?;

        let attrs = ContextAttributes {
            version: GLVersion { major: 3, minor: 3 },
            flags: ContextAttributeFlags::empty(),
        };
        let descriptor = device.create_context_descriptor(&attrs).ok()?;
        let mut context = device.create_context(&descriptor, None).ok()?;
        device.make_context_current(&context).ok()?;

        let glow = Arc::new(unsafe {
            glow::Context::from_loader_function(|name| {
                device.get_proc_address(&context, name) as *const _
            })
        });

        let proc_loader = SurfmanProcLoader {
            device: &device,
            context: &context,
        };
        let gl_interop = unsafe { GlInterop::new(&proc_loader).ok()? };

        // Best-effort native handle extraction for gst-gl wrapping. Surfman's
        // egl-context handle is the EGLContext pointer.
        let native_context_handle = device.native_context(&context);
        let handle_ptr = native_context_handle.0 as usize;

        let gst_gl_display = gst_gl::GLDisplay::new();
        let gst_gl_context = unsafe {
            gst_gl::GLContext::new_wrapped(
                &gst_gl_display,
                handle_ptr,
                gst_gl::GLPlatform::EGL,
                gst_gl::GLAPI::OPENGL3,
            )?
        };
        gst_gl_context.activate(true).ok()?;
        gst_gl_context.fill_info().ok()?;

        let inner = SurfmanContext {
            device,
            context: Some(context),
        };
        Some(super::GlContextBundle {
            glow,
            gl_interop,
            gst_gl_display,
            gst_gl_context,
            _platform: super::PlatformContext { inner },
        })
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
mod platform {
    pub(super) fn create() -> Option<super::GlContextBundle> {
        // macOS would need IOSurface bridging in wgpu_interop; not yet implemented.
        None
    }
}
