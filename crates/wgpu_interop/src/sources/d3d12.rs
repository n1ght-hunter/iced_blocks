use windows::Win32::Graphics::Direct3D12;

/// Direct import of a raw D3D12 resource.
pub struct D3D12Resource {
    pub resource: Direct3D12::ID3D12Resource,
}
