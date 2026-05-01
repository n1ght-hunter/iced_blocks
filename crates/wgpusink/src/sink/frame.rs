use std::any::Any;
use std::sync::mpsc;
use std::time::Duration;

use crate::api::Colorimetry;

/// A wgpu texture that optionally returns to a pool on drop.
///
/// Zero-copy backends use [`unmanaged()`](Self::unmanaged) — the texture
/// drops normally. The sysmem backend uses [`pooled()`](Self::pooled) —
/// the texture returns to the pool for reuse on the next frame.
pub struct PooledTexture {
    texture: Option<wgpu::Texture>,
    returns: Option<mpsc::SyncSender<wgpu::Texture>>,
}

impl PooledTexture {
    #[cfg_attr(
        not(any(feature = "d3d12", feature = "vulkan", feature = "gl")),
        expect(dead_code, reason = "used by zero-copy backends")
    )]
    pub(crate) fn unmanaged(texture: wgpu::Texture) -> Self {
        Self {
            texture: Some(texture),
            returns: None,
        }
    }

    pub(crate) fn pooled(texture: wgpu::Texture, sender: mpsc::SyncSender<wgpu::Texture>) -> Self {
        Self {
            texture: Some(texture),
            returns: Some(sender),
        }
    }
}

impl Drop for PooledTexture {
    fn drop(&mut self) {
        if let (Some(tex), Some(sender)) = (self.texture.take(), self.returns.take()) {
            let _ = sender.send(tex);
        }
    }
}

impl std::ops::Deref for PooledTexture {
    type Target = wgpu::Texture;
    fn deref(&self) -> &wgpu::Texture {
        self.texture
            .as_ref()
            .expect("PooledTexture used after drop")
    }
}

/// A decoded video frame delivered as a `wgpu::Texture`.
///
/// Check `texture.format()` to determine the pixel layout:
/// - `Rgba8Unorm` / `Bgra8Unorm` — sample directly.
/// - `NV12` — create plane views with `TextureAspect::Plane0` (`R8Unorm`,
///   full resolution) and `Plane1` (`Rg8Unorm`, half width and height).
///   Requires [`wgpu::Features::TEXTURE_FORMAT_NV12`].
///
/// Use `colorimetry` to select the correct YUV→RGB matrix.
pub struct WgpuFrame {
    pub texture: PooledTexture,
    pub pts: Option<Duration>,
    pub duration: Option<Duration>,
    pub colorimetry: Colorimetry,
    pub width: u32,
    pub height: u32,
    _guard: FrameGuard,
}

impl WgpuFrame {
    pub(crate) fn new(
        texture: PooledTexture,
        pts: Option<Duration>,
        duration: Option<Duration>,
        colorimetry: Colorimetry,
        width: u32,
        height: u32,
        guard: FrameGuard,
    ) -> Self {
        Self {
            texture,
            pts,
            duration,
            colorimetry,
            width,
            height,
            _guard: guard,
        }
    }
}

/// Prevents the underlying GstBuffer (and its GPU memory) from being
/// recycled until the wgpu command that samples this texture has completed.
///
/// Keep this alive until after `queue.submit()` returns for any command
/// buffer that references the frame's texture.
pub struct FrameGuard {
    _buffer: gst::Buffer,
    _sync: Option<Box<dyn Any + Send>>,
}

impl FrameGuard {
    pub(crate) fn new(buffer: gst::Buffer, sync: Option<Box<dyn Any + Send>>) -> Self {
        Self {
            _buffer: buffer,
            _sync: sync,
        }
    }
}
