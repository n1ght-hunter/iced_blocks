# wgpusink

GStreamer sink element that delivers decoded video frames as `wgpu::Texture`.

Zero-copy where possible (D3D12), system-memory fallback always. Uses static plugin registration — no `.so` / `.dll` install required.

| Backend | Memory feature | Platform | Mechanism | Status |
|---------|---------------|----------|-----------|--------|
| `sysmem` | none | All | `queue.write_texture` upload, pooled textures | ✅ |
| `d3d12` | `memory:D3D12Memory` | Windows | Direct `ID3D12Resource` wrap + fence sync | ✅ |
| `gl` | `memory:GLMemory` | Windows / Linux | Internal WGL/EGL context posted as `gst.gl.app_context`, then GPU blit via `wgpu_interop::GlInterop` | ✅ |
| `vulkan` | `memory:VulkanImage` | Linux / Windows | `VkImage` import + `vkDeviceWaitIdle` sync | ⚠️ best-effort |

## Supported formats

`RGBA`, `BGRA`, `RGBx`, `BGRx`, `RGB10A2_LE`, `GRAY8`, `GRAY16_LE`, `NV12`, `P010_10LE`.

Multi-planar formats (`NV12`, `P010_10LE`) are exposed as a single `wgpu::Texture` with `Plane0` / `Plane1` aspects (requires `wgpu::Features::TEXTURE_FORMAT_NV12`).

## Usage

```rust,ignore
// Register the plugin once at startup.
wgpusink::plugin_register_static().unwrap();

// Wire a sink to your wgpu device.
let sink = wgpusink::WgpuSink::new(device, queue).unwrap();

// Build any GStreamer pipeline that ends in this sink.
let pipeline = gst::parse::launch(&format!(
    "videotestsrc ! videoconvert ! {}", sink.element().name()
)).unwrap();

// In your render loop, pull the latest decoded frame.
if let Some(frame) = sink.slot().take() {
    // frame.texture is a wgpu::Texture ready to sample.
    // frame.colorimetry tells you which YUV→RGB matrix to apply.
}
```

A `new-frame` signal fires on the streaming thread whenever a frame is pushed — connect via [`WgpuVideoSink::connect_new_frame`] to wake your render loop.

## Examples

```bash
cargo run -p wgpusink --example testsrc
```

Renders `videotestsrc` into a winit + wgpu window.

## Feature flags

- **`sysmem`** — system-memory upload path (always enabled; the universal fallback).
- **`d3d12`** — zero-copy import of `GstD3D12Memory` on Windows.
- **`vulkan`** — zero-copy import of `GstVulkanImageMemory` on Linux / Windows. Requires the GStreamer Vulkan plugin (`gstreamer-vulkan-1.0`) and that wgpu and GStreamer share the same `VkDevice` (currently best-effort — see notes).
- **`gl`** — zero-copy import of `GstGLMemory` on Windows / Linux. Spins up an internal WGL (Windows) or EGL (Linux) context for the GPU blit. macOS is unsupported until `wgpu_interop` lands the IOSurface bridge.
- **`dmabuf`** — Linux DMABuf import. *(stub)*

## How the GL backend works

At `NullToReady` `wgpusink` creates an internal GL context (raw WGL on Windows, surfman/EGL on Linux) and posts it as a `gst.gl.GLDisplay` + `gst.gl.app_context` message on the bus. It also responds to `NeedContext` queries via `gst_gl_handle_context_query`. Together these tell upstream decoders ("`nvh264dec`", `glupload`, etc.) to allocate `GstGLMemory` *inside our context*, which is what makes zero-copy import work.

Per frame, the GL backend pulls the texture name out of `GstGLMemory` and hands it to `wgpu_interop::GlInterop`, which GPU-blits it into a wgpu-side shared resource:

- Windows: `WGL_NV_DX_interop2` → D3D11 shared NTHANDLE → D3D12 (then wgpu).
- Linux: `EXT_memory_object_fd` → Vulkan exportable image (then wgpu).

If GL context creation fails (no display on Linux, missing extensions on Windows) the backend silently falls back to sysmem.

macOS GL is currently unsupported — `wgpu_interop`'s IOSurface bridge for GL→Metal isn't wired yet.

## Notes on the Vulkan backend

The Vulkan path is **best-effort** today. `gst_to_wgpu` zero-copy of `VkImage` requires that wgpu and GStreamer use the *same* `VkDevice`, but `gstreamer-vulkan` v0.25 doesn't expose enough device-creation introspection (queue family + extension list) to wire wgpu-hal's `device_from_raw` reliably. Until that lands the backend will fall back to sysmem unless your pipeline happens to align both sides on the same VkDevice manually.

Sysmem remains the universal fallback path and is always exercised when zero-copy isn't viable, so playback never stops working — it just gets slower.
