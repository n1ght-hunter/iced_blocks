use std::sync::{Arc, Mutex};

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::subclass::prelude::*;
use gst_video::subclass::prelude::*;

use crate::api::{
    ColorPrimaries, ColorRange, Colorimetry, FrameSlot, MatrixCoefficients, TransferCharacteristic,
    WgpuDeviceHandle,
};
use crate::backend::{self, Backend};
use crate::sink::context;
use crate::sink::frame::{FrameGuard, WgpuFrame};

#[derive(Default)]
pub struct WgpuVideoSinkImp {
    device: Mutex<Option<Arc<WgpuDeviceHandle>>>,
    backend: Mutex<Option<Box<dyn Backend>>>,
    video_info: Mutex<Option<gst_video::VideoInfo>>,
    colorimetry: Mutex<Colorimetry>,
    frame_slot: FrameSlot,
    /// Eager-initialized GL context for `memory:GLMemory` zero-copy import.
    /// Created at `NullToReady` so it's available to advertise to upstream
    /// before caps negotiation. `None` if init failed (unsupported platform).
    #[cfg(feature = "gl")]
    pub(crate) gl_bundle: Mutex<Option<Arc<crate::backend::gl_context::GlContextBundle>>>,
}

#[glib::object_subclass]
impl ObjectSubclass for WgpuVideoSinkImp {
    const NAME: &'static str = "WgpuVideoSink";
    type Type = super::WgpuVideoSink;
    type ParentType = gst_video::VideoSink;
}

impl ObjectImpl for WgpuVideoSinkImp {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: std::sync::OnceLock<Vec<glib::ParamSpec>> = std::sync::OnceLock::new();
        PROPERTIES.get_or_init(|| {
            vec![
                glib::ParamSpecBoxed::builder::<FrameSlot>("frame-slot")
                    .nick("Frame Slot")
                    .blurb("Thread-safe slot for pulling the latest WgpuFrame")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
            ]
        })
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "frame-slot" => self.frame_slot.to_value(),
            _ => unimplemented!(),
        }
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: std::sync::OnceLock<Vec<glib::subclass::Signal>> =
            std::sync::OnceLock::new();
        SIGNALS.get_or_init(|| vec![glib::subclass::Signal::builder("new-frame").build()])
    }
}

impl GstObjectImpl for WgpuVideoSinkImp {}

impl ElementImpl for WgpuVideoSinkImp {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static METADATA: std::sync::OnceLock<gst::subclass::ElementMetadata> =
            std::sync::OnceLock::new();
        Some(METADATA.get_or_init(|| {
            gst::subclass::ElementMetadata::new(
                "wgpu Video Sink",
                "Sink/Video",
                "Delivers decoded video frames as wgpu::Texture",
                match env!("CARGO_PKG_AUTHORS") {
                    "" => "Unknown",
                    authors => authors,
                },
            )
        }))
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: std::sync::OnceLock<Vec<gst::PadTemplate>> =
            std::sync::OnceLock::new();
        PAD_TEMPLATES.get_or_init(|| {
            let caps = build_caps_template();
            let pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();
            vec![pad_template]
        })
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        match transition {
            gst::StateChange::NullToReady => {
                let device_handle = self.device.lock().unwrap().as_ref().cloned();
                if let Some(device_handle) = device_handle {
                    self.on_null_to_ready(&device_handle);
                }
            }
            gst::StateChange::ReadyToNull => {
                *self.backend.lock().unwrap() = None;
            }
            _ => {}
        }

        self.parent_change_state(transition)
    }

    #[cfg(feature = "gl")]
    fn set_context(&self, ctx: &gst::Context) {
        // Let gst-gl pick up any GstGLDisplay / GstGLContext from upstream.
        // We don't currently use them ourselves (we own our own GL context),
        // but accepting them prevents upstream from re-querying.
        let _ = gst_gl::functions::gl_handle_set_context(self.obj().as_ref(), ctx);
        self.parent_set_context(ctx);
    }
}

impl BaseSinkImpl for WgpuVideoSinkImp {
    #[cfg(feature = "gl")]
    fn query(&self, query: &mut gst::QueryRef) -> bool {
        // Respond to `NeedContext` queries with our wrapped GL display/context
        // so upstream `glupload` / hardware decoders allocate `GstGLMemory`
        // in *our* context — that's what makes zero-copy import work.
        if let gst::QueryViewMut::Context(ctx_query) = query.view_mut()
            && let Some(bundle) = self.gl_bundle.lock().unwrap().as_ref()
            && gst_gl::functions::gl_handle_context_query(
                self.obj().as_ref(),
                ctx_query,
                Some(&bundle.gst_gl_display),
                Some(&bundle.gst_gl_context),
                None::<&gst_gl::GLContext>,
            )
        {
            return true;
        }
        BaseSinkImplExt::parent_query(self, query)
    }

    fn set_caps(&self, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        let video_info = gst_video::VideoInfo::from_caps(caps)
            .map_err(|_| gst::loggable_error!(gst::CAT_RUST, "Failed to parse VideoInfo"))?;

        let colorimetry = parse_colorimetry(&video_info);
        *self.colorimetry.lock().unwrap() = colorimetry;

        let device_handle = self.device.lock().unwrap();
        if let Some(handle) = device_handle.as_ref() {
            let new_backend = backend::select_backend(caps, handle, self);
            *self.backend.lock().unwrap() = Some(new_backend);
        }

        *self.video_info.lock().unwrap() = Some(video_info);

        self.parent_set_caps(caps)
    }
}

impl VideoSinkImpl for WgpuVideoSinkImp {
    fn show_frame(&self, buffer: &gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        let video_info = self
            .video_info
            .lock()
            .unwrap()
            .clone()
            .ok_or(gst::FlowError::NotNegotiated)?;

        let device_handle = self
            .device
            .lock()
            .unwrap()
            .clone()
            .ok_or(gst::FlowError::Error)?;

        let backend_lock = self.backend.lock().unwrap();
        let backend = backend_lock.as_ref().ok_or(gst::FlowError::Error)?;

        let (texture, sync) = backend
            .try_import(buffer, &video_info, &device_handle)
            .ok_or(gst::FlowError::NotSupported)?
            .map_err(|e| {
                tracing::error!("Frame import failed: {e}");
                gst::FlowError::Error
            })?;

        let pts = buffer
            .pts()
            .map(|pts| std::time::Duration::from_nanos(pts.nseconds()));
        let duration = buffer
            .duration()
            .map(|dur| std::time::Duration::from_nanos(dur.nseconds()));

        let guard = FrameGuard::new(buffer.clone(), sync);
        let colorimetry = self.colorimetry.lock().unwrap().clone();

        let frame = WgpuFrame::new(
            texture,
            pts,
            duration,
            colorimetry,
            video_info.width(),
            video_info.height(),
            guard,
        );

        drop(backend_lock);

        self.frame_slot.push(frame);
        self.obj().emit_by_name::<()>("new-frame", &[]);

        Ok(gst::FlowSuccess::Ok)
    }
}

impl WgpuVideoSinkImp {
    pub(crate) fn set_device(&self, device: Arc<WgpuDeviceHandle>) {
        *self.device.lock().unwrap() = Some(device);
    }

    fn on_null_to_ready(&self, device_handle: &Arc<WgpuDeviceHandle>) {
        if let Some(identity) = context::extract_device_identity(&device_handle.device) {
            context::post_device_context(self.obj().as_ref(), &identity);
        }

        #[cfg(feature = "gl")]
        {
            let mut slot = self.gl_bundle.lock().unwrap();
            if slot.is_none()
                && let Some(bundle) =
                    crate::backend::gl_context::GlContextBundle::new(&device_handle.device)
            {
                *slot = Some(Arc::new(bundle));
            }
            if let Some(bundle) = slot.as_ref() {
                context::post_gl_context(self.obj().as_ref(), bundle);
            }
        }
    }
}

fn build_caps_template() -> gst::Caps {
    let mut caps = gst::Caps::new_empty();
    let formats = gst::List::new([
        "NV12",
        "P010_10LE",
        "RGBA",
        "BGRA",
        "RGBx",
        "BGRx",
        "RGB10A2_LE",
        "GRAY8",
        "GRAY16_LE",
    ]);

    #[cfg(all(feature = "d3d12", target_os = "windows"))]
    {
        let s = gst::Structure::builder("video/x-raw")
            .field("format", &formats)
            .field("width", gst::IntRange::new(1i32, i32::MAX))
            .field("height", gst::IntRange::new(1i32, i32::MAX))
            .build();
        let mut c = gst::Caps::new_empty();
        c.get_mut()
            .unwrap()
            .append_structure_full(s, Some(gst::CapsFeatures::new(["memory:D3D12Memory"])));
        caps.get_mut().unwrap().append(c);
    }

    #[cfg(feature = "vulkan")]
    {
        let s = gst::Structure::builder("video/x-raw")
            .field("format", &formats)
            .field("width", gst::IntRange::new(1i32, i32::MAX))
            .field("height", gst::IntRange::new(1i32, i32::MAX))
            .build();
        let mut c = gst::Caps::new_empty();
        c.get_mut()
            .unwrap()
            .append_structure_full(s, Some(gst::CapsFeatures::new(["memory:VulkanImage"])));
        caps.get_mut().unwrap().append(c);
    }

    #[cfg(all(feature = "dmabuf", target_os = "linux"))]
    {
        let s = gst::Structure::builder("video/x-raw")
            .field("format", &formats)
            .field("width", gst::IntRange::new(1i32, i32::MAX))
            .field("height", gst::IntRange::new(1i32, i32::MAX))
            .build();
        let mut c = gst::Caps::new_empty();
        c.get_mut()
            .unwrap()
            .append_structure_full(s, Some(gst::CapsFeatures::new(["memory:DMABuf"])));
        caps.get_mut().unwrap().append(c);
    }

    #[cfg(feature = "gl")]
    {
        let s = gst::Structure::builder("video/x-raw")
            .field("format", &formats)
            .field("width", gst::IntRange::new(1i32, i32::MAX))
            .field("height", gst::IntRange::new(1i32, i32::MAX))
            .build();
        let mut c = gst::Caps::new_empty();
        c.get_mut()
            .unwrap()
            .append_structure_full(s, Some(gst::CapsFeatures::new(["memory:GLMemory"])));
        caps.get_mut().unwrap().append(c);
    }

    // sysmem fallback — always available
    {
        let s = gst::Structure::builder("video/x-raw")
            .field("format", &formats)
            .field("width", gst::IntRange::new(1i32, i32::MAX))
            .field("height", gst::IntRange::new(1i32, i32::MAX))
            .build();
        caps.get_mut().unwrap().append_structure(s);
    }

    caps
}

fn parse_colorimetry(info: &gst_video::VideoInfo) -> Colorimetry {
    let c = info.colorimetry();

    let primaries = match c.primaries() {
        gst_video::VideoColorPrimaries::Bt709 => ColorPrimaries::Bt709,
        gst_video::VideoColorPrimaries::Bt2020 => ColorPrimaries::Bt2020,
        gst_video::VideoColorPrimaries::Smpte170m | gst_video::VideoColorPrimaries::Bt470bg => {
            ColorPrimaries::Bt601
        }
        _ => ColorPrimaries::Bt709,
    };

    let transfer = match c.transfer() {
        gst_video::VideoTransferFunction::Bt709 => TransferCharacteristic::Bt709,
        gst_video::VideoTransferFunction::Srgb => TransferCharacteristic::Srgb,
        gst_video::VideoTransferFunction::Smpte2084 => TransferCharacteristic::Pq,
        gst_video::VideoTransferFunction::AribStdB67 => TransferCharacteristic::Hlg,
        _ => TransferCharacteristic::Bt709,
    };

    let matrix = match c.matrix() {
        gst_video::VideoColorMatrix::Bt709 => MatrixCoefficients::Bt709,
        gst_video::VideoColorMatrix::Bt2020 => MatrixCoefficients::Bt2020Ncl,
        gst_video::VideoColorMatrix::Bt601 => MatrixCoefficients::Bt601,
        _ => MatrixCoefficients::Bt709,
    };

    let range = match c.range() {
        gst_video::VideoColorRange::Range0_255 => ColorRange::Full,
        gst_video::VideoColorRange::Range16_235 => ColorRange::Limited,
        _ => ColorRange::Limited,
    };

    Colorimetry {
        primaries,
        transfer,
        matrix,
        range,
        hdr_metadata: None,
    }
}
