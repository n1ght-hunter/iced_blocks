/// Standard 64×64 RGBA8 texture descriptor for tests.
#[allow(dead_code)]
pub fn test_desc(usage: wgpu::TextureUsages) -> wgpu::TextureDescriptor<'static> {
    wgpu::TextureDescriptor {
        label: Some("test texture"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        view_formats: &[],
    }
}

/// 8×8 block checkerboard: red `(255,0,0,255)` and blue `(0,0,255,255)`.
#[allow(dead_code)]
pub fn checkerboard_rgba(width: u32, height: u32) -> Vec<u8> {
    let mut data = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let block = ((x / 8) + (y / 8)) % 2;
            let offset = ((y * width + x) * 4) as usize;
            if block == 0 {
                data[offset..offset + 4].copy_from_slice(&[255, 0, 0, 255]);
            } else {
                data[offset..offset + 4].copy_from_slice(&[0, 0, 255, 255]);
            }
        }
    }
    data
}

/// Read back a wgpu texture to CPU bytes (RGBA8, tightly packed).
#[allow(dead_code)]
pub fn readback_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let bytes_per_pixel = 4u32;
    let unpadded_row = width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_row = unpadded_row.div_ceil(align) * align;

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (padded_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(5)),
        })
        .unwrap();
    rx.recv().unwrap().unwrap();

    let mapped = slice.get_mapped_range();
    let mut result = vec![0u8; (width * height * bytes_per_pixel) as usize];
    for row in 0..height as usize {
        let src_offset = row * padded_row as usize;
        let dst_offset = row * unpadded_row as usize;
        result[dst_offset..dst_offset + unpadded_row as usize]
            .copy_from_slice(&mapped[src_offset..src_offset + unpadded_row as usize]);
    }
    drop(mapped);
    buffer.unmap();
    result
}

pub fn create_device(backend: wgpu::Backends) -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: backend,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: None,
        ..Default::default()
    }))
    .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

/// Live WGL context for testing GL texture paths.
#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub struct WglContext {
    pub gl: std::sync::Arc<glow::Context>,
    hglrc: windows::Win32::Graphics::OpenGL::HGLRC,
    hdc: windows::Win32::Graphics::Gdi::HDC,
    hwnd: windows::Win32::Foundation::HWND,
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
impl WglContext {
    pub fn new() -> Self {
        use std::sync::Arc;
        use windows::Win32::Foundation;
        use windows::Win32::Graphics::{Gdi, OpenGL};
        use windows::Win32::UI::WindowsAndMessaging as Wm;

        unsafe extern "system" fn wnd_proc(
            hwnd: Foundation::HWND,
            msg: u32,
            wparam: Foundation::WPARAM,
            lparam: Foundation::LPARAM,
        ) -> Foundation::LRESULT {
            unsafe { Wm::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        unsafe {
            let class_name = windows::core::w!("wgpu_interop_test_wgl");
            let wc = Wm::WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                lpszClassName: class_name,
                ..Default::default()
            };
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
            .expect("CreateWindowEx");

            let hdc = Gdi::GetDC(hwnd);

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
            assert!(pf != 0, "ChoosePixelFormat failed");
            OpenGL::SetPixelFormat(hdc, pf, &pfd).expect("SetPixelFormat");

            let hglrc = OpenGL::wglCreateContext(hdc).expect("wglCreateContext");
            OpenGL::wglMakeCurrent(hdc, hglrc).expect("wglMakeCurrent");

            let opengl32 = windows::Win32::System::LibraryLoader::LoadLibraryA(windows::core::s!(
                "opengl32.dll"
            ))
            .expect("LoadLibrary opengl32.dll");

            let gl = Arc::new(glow::Context::from_loader_function(|name| {
                let cname = std::ffi::CString::new(name).unwrap();
                let addr =
                    OpenGL::wglGetProcAddress(windows::core::PCSTR(cname.as_ptr() as *const u8));
                match addr {
                    Some(f) => f as *const std::ffi::c_void,
                    None => {
                        let addr = windows::Win32::System::LibraryLoader::GetProcAddress(
                            opengl32,
                            windows::core::PCSTR(cname.as_ptr() as *const u8),
                        );
                        match addr {
                            Some(f) => f as *const std::ffi::c_void,
                            None => std::ptr::null(),
                        }
                    }
                }
            }));

            WglContext {
                gl,
                hglrc,
                hdc,
                hwnd,
            }
        }
    }

    /// Create a GL texture filled with the given RGBA data.
    pub fn create_texture_with_data(
        &self,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> std::num::NonZeroU32 {
        use glow::HasContext;
        unsafe {
            let tex = self.gl.create_texture().expect("glCreateTexture");
            self.gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(data)),
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            self.gl.bind_texture(glow::TEXTURE_2D, None);
            tex.0
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for WglContext {
    fn drop(&mut self) {
        use windows::Win32::Graphics::{Gdi, OpenGL};
        use windows::Win32::UI::WindowsAndMessaging as Wm;
        unsafe {
            OpenGL::wglMakeCurrent(self.hdc, OpenGL::HGLRC::default()).ok();
            OpenGL::wglDeleteContext(self.hglrc).ok();
            Gdi::ReleaseDC(self.hwnd, self.hdc);
            Wm::DestroyWindow(self.hwnd).ok();
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn create_d3d11_device() -> windows::Win32::Graphics::Direct3D11::ID3D11Device {
    use windows::Win32::Foundation;
    use windows::Win32::Graphics::{
        Direct3D::D3D_DRIVER_TYPE_HARDWARE,
        Direct3D11::{self, ID3D11Device},
    };

    unsafe {
        let mut device: Option<ID3D11Device> = None;
        Direct3D11::D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            Foundation::HMODULE::default(),
            Direct3D11::D3D11_CREATE_DEVICE_FLAG(0),
            None,
            Direct3D11::D3D11_SDK_VERSION,
            Some(&mut device as *mut _),
            None,
            None,
        )
        .expect("D3D11CreateDevice");
        device.unwrap()
    }
}
