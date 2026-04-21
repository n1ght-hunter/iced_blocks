use std::sync::{Arc, Mutex};

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use crate::error::SinkError;
use crate::sink::frame::WgpuFrame;

/// Bundles the app's wgpu device and queue.
pub struct WgpuDeviceHandle {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

/// Builder for creating a `wgpuvideosink` element wired to the app's wgpu device.
pub struct WgpuSinkBuilder {
    device: wgpu::Device,
    queue: wgpu::Queue,
    sync: bool,
}

impl WgpuSinkBuilder {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self {
            device,
            queue,
            sync: true,
        }
    }

    /// Enable/disable PTS-based synchronization (default: true).
    pub fn sync(mut self, sync: bool) -> Self {
        self.sync = sync;
        self
    }

    /// Build the GStreamer element.
    pub fn build(self) -> Result<gst::Element, SinkError> {
        let element = gst::ElementFactory::make("wgpuvideosink")
            .build()
            .map_err(|e| SinkError::Gst(e.to_string()))?;

        element.set_property("sync", self.sync);

        let imp = element
            .downcast_ref::<super::sink::WgpuVideoSink>()
            .expect("element is WgpuVideoSink")
            .imp();

        let handle = Arc::new(WgpuDeviceHandle {
            device: self.device,
            queue: self.queue,
        });
        imp.set_device(handle);

        Ok(element)
    }
}

/// Convenience wrapper that creates a `wgpuvideosink` with a [`FrameSlot`]
/// for render-loop consumption.
pub struct WgpuSink {
    element: gst::Element,
    slot: FrameSlot,
}

impl WgpuSink {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Result<Self, SinkError> {
        let element = WgpuSinkBuilder::new(device, queue).build()?;
        let slot: FrameSlot = element.property("frame-slot");
        Ok(Self { element, slot })
    }

    pub fn element(&self) -> &gst::Element {
        &self.element
    }

    pub fn slot(&self) -> &FrameSlot {
        &self.slot
    }
}

/// Thread-safe slot for render-loop apps.
///
/// The GStreamer streaming thread pushes frames via [`push`](Self::push),
/// and the render thread pulls them via [`take`](Self::take).
///
/// Registered as a GLib boxed type so it can be exposed as a GObject property.
#[derive(Clone, glib::Boxed)]
#[boxed_type(name = "WgpuFrameSlot")]
pub struct FrameSlot {
    inner: Arc<Mutex<Option<WgpuFrame>>>,
}

impl FrameSlot {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }

    /// Replace the current frame. The previous frame's [`FrameGuard`](super::FrameGuard)
    /// drops here, releasing the old GstBuffer back to the decoder pool.
    pub fn push(&self, frame: WgpuFrame) {
        *self.inner.lock().unwrap() = Some(frame);
    }

    /// Take the latest frame, if any.
    pub fn take(&self) -> Option<WgpuFrame> {
        self.inner.lock().unwrap().take()
    }
}

impl Default for FrameSlot {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed colorimetry information from GStreamer caps.
#[derive(Clone, Debug, Default)]
pub struct Colorimetry {
    pub primaries: ColorPrimaries,
    pub transfer: TransferCharacteristic,
    pub matrix: MatrixCoefficients,
    pub range: ColorRange,
    pub hdr_metadata: Option<HdrMetadata>,
}

#[derive(Clone, Debug, Default)]
pub enum ColorPrimaries {
    #[default]
    Bt709,
    Bt601,
    Bt2020,
}

#[derive(Clone, Debug, Default)]
pub enum TransferCharacteristic {
    #[default]
    Bt709,
    Srgb,
    Pq,
    Hlg,
}

#[derive(Clone, Debug, Default)]
pub enum MatrixCoefficients {
    #[default]
    Bt709,
    Bt601,
    Bt2020Ncl,
}

#[derive(Clone, Debug, Default)]
pub enum ColorRange {
    #[default]
    Limited,
    Full,
}

/// HDR static metadata, when present in the bitstream.
#[derive(Clone, Debug)]
pub struct HdrMetadata {
    pub mastering_display: Option<MasteringDisplayInfo>,
    pub content_light_level: Option<ContentLightLevel>,
}

/// SMPTE ST 2086 mastering display color volume metadata.
#[derive(Clone, Debug)]
pub struct MasteringDisplayInfo {
    /// Display primaries in CIE 1931 chromaticity coordinates,
    /// ordered `[red, green, blue]`.
    pub primaries: [[f64; 2]; 3],
    /// White point in CIE 1931 chromaticity coordinates.
    pub white_point: [f64; 2],
    /// Maximum display luminance in cd/m² (nits).
    pub max_luminance: f64,
    /// Minimum display luminance in cd/m² (nits).
    pub min_luminance: f64,
}

/// CTA-861.3 content light level information.
#[derive(Clone, Debug)]
pub struct ContentLightLevel {
    /// Maximum Content Light Level — peak pixel luminance, in cd/m² (nits).
    pub max_cll: u32,
    /// Maximum Frame-Average Light Level — peak frame-average luminance,
    /// in cd/m² (nits).
    pub max_fall: u32,
}
