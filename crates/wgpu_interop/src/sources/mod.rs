//! External texture source types.

#[cfg(target_os = "windows")]
pub mod d3d11;

#[cfg(target_os = "windows")]
pub mod d3d12;

pub mod vulkan;

#[cfg(target_vendor = "apple")]
pub mod iosurface;

#[cfg(target_vendor = "apple")]
pub mod metal;

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub mod gl;

pub mod gles;
