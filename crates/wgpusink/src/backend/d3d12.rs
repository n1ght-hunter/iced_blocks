use std::any::Any;

use wgpu_interop::{D3D12Resource as D3D12ResourceSource, DeviceInterop};
use windows::Win32::Graphics::Direct3D12::{ID3D12Fence, ID3D12Resource};
use windows::core::Interface;

use super::{Backend, ImportResult, SinkError, gst_to_wgpu_format};
use crate::api::WgpuDeviceHandle;
use crate::sink::frame::PooledTexture;

pub(crate) struct D3D12Backend;

impl Backend for D3D12Backend {
    fn try_import(
        &self,
        buffer: &gst::Buffer,
        video_info: &gst_video::VideoInfo,
        device: &WgpuDeviceHandle,
    ) -> Option<ImportResult> {
        let mem = buffer.memory(0)?;
        if !mem.is_memory_type::<gst_d3d12::D3D12Memory>() {
            return None;
        }
        let d3d12_mem = mem.downcast_memory_ref::<gst_d3d12::D3D12Memory>()?;
        Some(import_d3d12(d3d12_mem, video_info, device))
    }
}

fn import_d3d12(
    mem: &gst_d3d12::D3D12MemoryRef,
    video_info: &gst_video::VideoInfo,
    device: &WgpuDeviceHandle,
) -> ImportResult {
    // Wait for any pending GPU work on this memory before we wrap it.
    mem.sync()
        .map_err(|e| SinkError::Import(format!("D3D12 memory sync failed: {e}")))?;

    // Extract raw resource pointer via FFI to avoid windows crate version conflicts
    // (gst-d3d12 uses windows 0.62, wgpu uses 0.58).
    let resource = extract_resource(mem)?;
    let format = gst_to_wgpu_format(video_info.format())
        .ok_or_else(|| SinkError::UnsupportedMemory(format!("{:?}", video_info.format())))?;

    let desc = wgpu::TextureDescriptor {
        label: Some("wgpusink_d3d12"),
        size: wgpu::Extent3d {
            width: video_info.width(),
            height: video_info.height(),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    };

    let texture = unsafe {
        device
            .device
            .import_external_texture(D3D12ResourceSource { resource }, &desc)
            .map_err(|e| SinkError::Import(format!("D3D12 texture import failed: {e}")))?
    };

    let sync: Option<Box<dyn Any + Send>> = extract_fence(mem).map(|(fence, value)| {
        Box::new(crate::sync::d3d12::D3D12FenceSync { fence, value }) as Box<dyn Any + Send>
    });

    Ok((PooledTexture::unmanaged(texture), sync))
}

/// Get the ID3D12Resource via FFI raw pointer, constructing our windows 0.58 type.
fn extract_resource(mem: &gst_d3d12::D3D12MemoryRef) -> Result<ID3D12Resource, SinkError> {
    unsafe {
        let raw = gst_d3d12::ffi::gst_d3d12_memory_get_resource_handle(
            mem as *const _ as *mut gst_d3d12::ffi::GstD3D12Memory,
        );
        if raw.is_null() {
            return Err(SinkError::Import("D3D12 resource handle is null".into()));
        }
        ID3D12Resource::from_raw_borrowed(&raw)
            .cloned()
            .ok_or_else(|| SinkError::Import("failed to wrap ID3D12Resource".into()))
    }
}

/// Get the D3D12 fence + value via FFI raw pointer.
fn extract_fence(mem: &gst_d3d12::D3D12MemoryRef) -> Option<(ID3D12Fence, u64)> {
    unsafe {
        let mut raw_fence: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut fence_value: u64 = 0;
        let ok = gst_d3d12::ffi::gst_d3d12_memory_get_fence(
            mem as *const _ as *mut gst_d3d12::ffi::GstD3D12Memory,
            &mut raw_fence,
            &mut fence_value,
        );
        if ok == 0 || raw_fence.is_null() {
            return None;
        }
        let fence = ID3D12Fence::from_raw_borrowed(&raw_fence)?.clone();
        Some((fence, fence_value))
    }
}
