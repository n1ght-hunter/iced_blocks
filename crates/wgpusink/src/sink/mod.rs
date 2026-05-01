pub(crate) mod context;
pub mod frame;
pub(crate) mod imp;

use gst::glib;
use gst::prelude::*;

glib::wrapper! {
    pub struct WgpuVideoSink(ObjectSubclass<imp::WgpuVideoSinkImp>)
        @extends gst_video::VideoSink, gst_base::BaseSink, gst::Element, gst::Object;
}

impl WgpuVideoSink {
    /// Connect a callback that fires when a new frame is available in the
    /// `frame-slot`. Runs on the GStreamer streaming thread — must not block.
    pub fn connect_new_frame<F: Fn(&Self) + Send + Sync + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_closure(
            "new-frame",
            false,
            glib::closure!(|sink: &Self| {
                f(sink);
            }),
        )
    }
}

pub(crate) fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "wgpuvideosink",
        gst::Rank::NONE,
        WgpuVideoSink::static_type(),
    )
}
