# wgpu_interop

Import external GPU textures into wgpu as zero-copy shared resources.

Bridges wgpu with other GPU APIs using platform-specific memory sharing:

| Backend | External Api | Platform | Mechanism | Status |
|---------|--------------|----------|-----------|--------|
| `DX12` | `D3D12Resource` | Windows | Direct D3D12 resource wrap | ✅ |
| `DX12` | `D3D11SharedHandle` / `D3D11Interop` | Windows | NTHANDLE → D3D12 | ✅ |
| `DX12` | `VulkanImage` | Windows | NTHANDLE → D3D12 | ✅ |
| `DX12` | `GlesTexture` | Windows | WGL_NV_DX_interop2 → D3D11 → D3D12 | ✅ |
| `Vulkan` | `VulkanImage` | Linux / Windows | Opaque fd or NTHANDLE | ✅ |
| `Vulkan` | `GlesTexture` | Linux | EXT_memory_object_fd → Vulkan | ✅ |
| `Vulkan` | `GlesTexture` | Windows | WGL_NV_DX_interop2 → NTHANDLE → Vulkan | planned |
| `Vulkan` | `D3D11SharedHandle` / `D3D11Interop` | Windows | NTHANDLE → `VK_KHR_external_memory_win32` | planned |
| `GLES` | `GlesTexture` | All | Direct GL texture wrap via HAL | ✅ |
| `Metal` | `MetalTexture` | macOS | IOSurface | planned |
| `Metal` | `GlesTexture` | macOS | IOSurface-backed GL texture → Metal | planned |

## Usage

```rust
use wgpu_interop::{DeviceInterop, D3D12Resource};

let desc = wgpu::TextureDescriptor {
    label: Some("imported"),
    size: wgpu::Extent3d { width: 1920, height: 1080, depth_or_array_layers: 1 },
    format: wgpu::TextureFormat::Rgba8Unorm,
    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
    mip_level_count: 1,
    sample_count: 1,
    dimension: wgpu::TextureDimension::D2,
    view_formats: &[],
};

let texture = unsafe {
    wgpu_device.import_external_texture(D3D12Resource { resource }, &desc)?
};
```

## Testing

Tests create real GPU devices and must run in separate processes. Use [cargo-nextest](https://nexte.st/):

```bash
cargo nextest run -p wgpu_interop
```

## Feature flags

- **`advanced`** — Exposes the `backends` module with low-level wrapping functions (`wrap_resource`, `wrap_image`, etc.) for direct HAL-level access.
