use std::sync::{Mutex, mpsc};

use gst_video::prelude::*;

use super::{Backend, ImportResult, SinkError, gst_to_wgpu_format};
use crate::api::WgpuDeviceHandle;
use crate::sink::frame::PooledTexture;

#[derive(Clone, Copy, PartialEq, Eq)]
struct TextureKey {
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
}

struct TexturePool {
    key: TextureKey,
    free: Vec<wgpu::Texture>,
    rx: mpsc::Receiver<wgpu::Texture>,
    tx: mpsc::SyncSender<wgpu::Texture>,
}

impl TexturePool {
    fn new(key: TextureKey, size: usize) -> Self {
        let (tx, rx) = mpsc::sync_channel(size);
        Self {
            key,
            free: Vec::new(),
            rx,
            tx,
        }
    }

    fn collect_returned(&mut self) {
        while let Ok(tex) = self.rx.try_recv() {
            self.free.push(tex);
        }
    }

    fn take(&mut self) -> Option<wgpu::Texture> {
        self.collect_returned();
        self.free.pop()
    }

    fn sender(&self) -> mpsc::SyncSender<wgpu::Texture> {
        self.tx.clone()
    }
}

pub(crate) struct SysmemBackend {
    pool: Mutex<Option<TexturePool>>,
}

impl SysmemBackend {
    pub(crate) fn new() -> Self {
        Self {
            pool: Mutex::new(None),
        }
    }
}

impl Backend for SysmemBackend {
    fn try_import(
        &self,
        buffer: &gst::Buffer,
        video_info: &gst_video::VideoInfo,
        device: &WgpuDeviceHandle,
    ) -> Option<ImportResult> {
        Some(import_sysmem(buffer, video_info, device, &self.pool))
    }
}

fn import_sysmem(
    buffer: &gst::Buffer,
    video_info: &gst_video::VideoInfo,
    device: &WgpuDeviceHandle,
    pool: &Mutex<Option<TexturePool>>,
) -> ImportResult {
    let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, video_info)
        .map_err(|_| SinkError::Import("failed to map buffer readable".into()))?;

    let width = video_info.width();
    let height = video_info.height();
    let format = video_info.format();

    let wgpu_format = gst_to_wgpu_format(format)
        .ok_or_else(|| SinkError::UnsupportedMemory(format!("{format:?}")))?;

    let key = TextureKey {
        width,
        height,
        format: wgpu_format,
    };

    let (texture, sender) = {
        let mut pool_guard = pool.lock().unwrap();
        let pool_inner = pool_guard.get_or_insert_with(|| TexturePool::new(key, 2));
        if pool_inner.key != key {
            *pool_inner = TexturePool::new(key, 2);
        }
        let sender = pool_inner.sender();
        let texture = pool_inner.take();
        (texture, sender)
    };

    let texture =
        texture.unwrap_or_else(|| create_texture(&device.device, width, height, wgpu_format));

    if wgpu_format.is_multi_planar_format() {
        upload_multi_planar(&device.queue, &frame, &texture, width, height)?;
    } else {
        upload_single_plane(&device.queue, &frame, &texture, width, height)?;
    }

    Ok((PooledTexture::pooled(texture, sender), None))
}

fn create_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("wgpusink_sysmem"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn upload_single_plane(
    queue: &wgpu::Queue,
    frame: &gst_video::VideoFrameRef<&gst::BufferRef>,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<(), SinkError> {
    let data = frame
        .plane_data(0)
        .map_err(|e| SinkError::Import(e.to_string()))?;
    let stride = frame.plane_stride()[0] as u32;

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(stride),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    Ok(())
}

fn upload_multi_planar(
    queue: &wgpu::Queue,
    frame: &gst_video::VideoFrameRef<&gst::BufferRef>,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<(), SinkError> {
    let y_data = frame
        .plane_data(0)
        .map_err(|e| SinkError::Import(e.to_string()))?;
    let y_stride = frame.plane_stride()[0] as u32;

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::Plane0,
        },
        y_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(y_stride),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    let uv_data = frame
        .plane_data(1)
        .map_err(|e| SinkError::Import(e.to_string()))?;
    let uv_stride = frame.plane_stride()[1] as u32;
    let uv_height = height / 2;

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::Plane1,
        },
        uv_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(uv_stride),
            rows_per_image: Some(uv_height),
        },
        wgpu::Extent3d {
            width: width / 2,
            height: uv_height,
            depth_or_array_layers: 1,
        },
    );

    Ok(())
}
