//! GStreamer sink element that delivers decoded video frames as `wgpu::Texture`.
//!
//! Zero-copy where possible (D3D12, Vulkan, Metal), system-memory fallback always.
//! Uses static plugin registration — no `.so`/`.dll` install required.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Register the plugin before building any pipeline
//! wgpusink::plugin_register_static().unwrap();
//!
//! // Create a sink wired to your wgpu device
//! let sink = wgpusink::WgpuSink::new(device, queue).unwrap();
//!
//! // Build a pipeline
//! let pipeline = gst::parse::launch(&format!(
//!     "videotestsrc ! videoconvert ! {}", sink.element().name()
//! )).unwrap();
//!
//! // Pull frames from the render loop
//! if let Some(frame) = sink.slot().take() {
//!     // frame.texture is a wgpu::Texture ready to sample
//! }
//! ```

mod api;
pub(crate) mod backend;
pub mod error;
#[allow(clippy::module_inception)]
pub mod sink;
pub(crate) mod sync;

pub use api::{
    ColorPrimaries, ColorRange, Colorimetry, ContentLightLevel, FrameSlot, HdrMetadata,
    MasteringDisplayInfo, MatrixCoefficients, TransferCharacteristic, WgpuDeviceHandle, WgpuSink,
    WgpuSinkBuilder,
};
pub use error::SinkError;
pub use sink::WgpuVideoSink;
pub use sink::frame::{FrameGuard, PooledTexture, WgpuFrame};

fn plugin_init(plugin: &gst::Plugin) -> Result<(), gst::glib::BoolError> {
    sink::register(plugin)
}

gst::plugin_define!(
    wgpusink,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MIT/Apache-2.0",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
