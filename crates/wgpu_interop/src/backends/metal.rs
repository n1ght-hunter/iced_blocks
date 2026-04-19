// TODO: Metal texture wrapping via IOSurface.
//
// Requires `metal` crate v0.32 (matching wgpu-hal 27).
// Signature: `Device::texture_from_raw(raw: metal::Texture, format,
//   raw_type: MTLTextureType, array_layers, mip_levels, copy_size: CopyExtent)`
