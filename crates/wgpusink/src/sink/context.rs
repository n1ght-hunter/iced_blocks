/// Identifies a physical GPU so we can match the wgpu device with upstream decoders.
#[cfg_attr(
    not(any(feature = "d3d12", feature = "vulkan")),
    expect(dead_code, reason = "fields used by d3d12/vulkan context posting")
)]
pub(crate) struct DeviceIdentity {
    pub backend: wgpu::Backend,
    pub device_id: Vec<u8>,
}

/// Extract the GPU identity (LUID on D3D12, UUID on Vulkan) from the wgpu device.
pub(crate) fn extract_device_identity(device: &wgpu::Device) -> Option<DeviceIdentity> {
    #[cfg(all(feature = "d3d12", target_os = "windows"))]
    {
        if let Some(identity) = extract_d3d12_identity(device) {
            return Some(identity);
        }
    }

    // TODO: Vulkan — as_hal::<Vulkan>() → vkGetPhysicalDeviceProperties2 → deviceUUID

    let _ = device;
    None
}

#[cfg(all(feature = "d3d12", target_os = "windows"))]
fn extract_d3d12_identity(device: &wgpu::Device) -> Option<DeviceIdentity> {
    use windows::Win32::Graphics::Dxgi;
    use windows::core::Interface;

    unsafe {
        device.as_hal::<wgpu::wgc::api::Dx12>().and_then(|hal| {
            let dx12_device = hal.raw_device();
            let dxgi_device: Dxgi::IDXGIDevice = dx12_device.cast().ok()?;
            let adapter: Dxgi::IDXGIAdapter = dxgi_device.GetAdapter().ok()?;
            let desc = adapter.GetDesc().ok()?;
            let luid = desc.AdapterLuid;
            let mut id = Vec::with_capacity(8);
            id.extend_from_slice(&luid.LowPart.to_le_bytes());
            id.extend_from_slice(&luid.HighPart.to_le_bytes());
            Some(DeviceIdentity {
                backend: wgpu::Backend::Dx12,
                device_id: id,
            })
        })
    }
}

/// Post a GstContext containing the device identity so upstream decoders
/// (e.g. `d3d12h264dec`) allocate on the same GPU.
pub(crate) fn post_device_context(
    element: &impl gst::prelude::IsA<gst::Element>,
    identity: &DeviceIdentity,
) {
    #[cfg(all(feature = "d3d12", target_os = "windows"))]
    if identity.backend == wgpu::Backend::Dx12 {
        post_d3d12_context(element, identity);
    }

    // TODO: Vulkan — post gst.vulkan.device context with UUID

    let _ = (element, identity);
}

#[cfg(all(feature = "d3d12", target_os = "windows"))]
fn post_d3d12_context(element: &impl gst::prelude::IsA<gst::Element>, identity: &DeviceIdentity) {
    use gst::glib::translate::{ToGlibPtr, from_glib_full};
    use gst::prelude::*;

    if identity.device_id.len() < 8 {
        return;
    }

    let low = i32::from_le_bytes(identity.device_id[0..4].try_into().unwrap());
    let high = i32::from_le_bytes(identity.device_id[4..8].try_into().unwrap());
    let luid = ((high as i64) << 32) | (low as u32 as i64);

    if let Some(gst_device) = gst_d3d12::D3D12Device::for_adapter_luid(luid) {
        let ctx: gst::Context = unsafe {
            from_glib_full(gst_d3d12::ffi::gst_d3d12_context_new(
                ToGlibPtr::to_glib_none(&gst_device).0,
            ))
        };
        let msg = gst::message::HaveContext::new(ctx);
        let _ = element.as_ref().post_message(msg);
    }
}

/// Post our internal `GstGLDisplay` and `GstGLContext` so upstream decoders
/// allocate `GstGLMemory` in our context (the prerequisite for zero-copy).
#[cfg(feature = "gl")]
pub(crate) fn post_gl_context(
    element: &impl gst::prelude::IsA<gst::Element>,
    bundle: &crate::backend::gl_context::GlContextBundle,
) {
    use gst::prelude::*;
    use gst_gl::prelude::*;

    // gst.gl.GLDisplay — the display upstream should use.
    {
        let mut ctx = gst::Context::new("gst.gl.GLDisplay", true);
        ctx.get_mut()
            .unwrap()
            .set_gl_display(&bundle.gst_gl_display);
        let _ = element
            .as_ref()
            .post_message(gst::message::HaveContext::new(ctx));
    }

    // gst.gl.app_context — the app-provided GstGLContext upstream should
    // share with so its allocated GstGLMemory is valid for us.
    {
        let mut ctx = gst::Context::new("gst.gl.app_context", true);
        {
            let writable = ctx.get_mut().unwrap();
            let s = writable.structure_mut();
            s.set("context", &bundle.gst_gl_context);
        }
        let _ = element
            .as_ref()
            .post_message(gst::message::HaveContext::new(ctx));
    }
}
