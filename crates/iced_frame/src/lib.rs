//! [Iced](https://github.com/iced-rs/iced) widget that renders an
//! offscreen RGBA frame buffer as a wgpu-textured quad.
//!
//! Any source that produces frames (a webview engine, a video decoder,
//! a software renderer, …) can plug into this widget by implementing
//! the [`FrameSource`] trait. The widget handles the wgpu pipeline
//! (texture create / resize / upload / draw), click-to-focus state,
//! and redraw requests — the implementor supplies frames, cursor
//! state, and event handling.

mod primitive;
mod widget;

use std::sync::{Arc, Mutex};

use iced::mouse;

pub use primitive::SizeRequestSlot;
pub use widget::{FrameWidget, frame};

/// How the frame's content fits within the widget bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ContentFit {
    /// Stretch to fill — ignores aspect ratio.
    #[default]
    Fill,
    /// Scale uniformly so the entire frame is visible (letterbox).
    Contain,
    /// Scale uniformly so the widget is entirely covered (crop).
    Cover,
    /// Scale uniformly to match the widget width. Height is clipped
    /// or letterboxed depending on the frame's aspect ratio.
    FitWidth,
    /// Scale uniformly to match the widget height. Width is clipped
    /// or letterboxed depending on the frame's aspect ratio.
    FitHeight,
    /// No scaling — draw at native pixel size, clip overflow.
    None,
}

/// Where the frame is anchored when it doesn't fill the widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Alignment {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    #[default]
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// Texture sampling filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FilterMode {
    /// Bilinear interpolation — smooth.
    #[default]
    Linear,
    /// Nearest-neighbor — sharp pixel boundaries.
    Nearest,
}

impl FilterMode {
    pub(crate) fn to_wgpu(self) -> wgpu::FilterMode {
        match self {
            Self::Linear => wgpu::FilterMode::Linear,
            Self::Nearest => wgpu::FilterMode::Nearest,
        }
    }
}

impl std::fmt::Display for ContentFit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fill => f.write_str("Fill"),
            Self::Contain => f.write_str("Contain"),
            Self::Cover => f.write_str("Cover"),
            Self::FitWidth => f.write_str("FitWidth"),
            Self::FitHeight => f.write_str("FitHeight"),
            Self::None => f.write_str("None"),
        }
    }
}

impl std::fmt::Display for Alignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TopLeft => f.write_str("TopLeft"),
            Self::TopCenter => f.write_str("TopCenter"),
            Self::TopRight => f.write_str("TopRight"),
            Self::CenterLeft => f.write_str("CenterLeft"),
            Self::Center => f.write_str("Center"),
            Self::CenterRight => f.write_str("CenterRight"),
            Self::BottomLeft => f.write_str("BottomLeft"),
            Self::BottomCenter => f.write_str("BottomCenter"),
            Self::BottomRight => f.write_str("BottomRight"),
        }
    }
}

impl std::fmt::Display for FilterMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Linear => f.write_str("Linear"),
            Self::Nearest => f.write_str("Nearest"),
        }
    }
}

/// A raw frame — the minimal data the widget needs to upload a texture.
#[derive(Debug)]
pub struct Frame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Pixel format of `data`. Defaults to `Rgba8Unorm`.
    pub format: wgpu::TextureFormat,
}

impl Frame {
    pub fn new(data: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            data,
            width,
            height,
            format: wgpu::TextureFormat::Rgba8Unorm,
        }
    }

    pub fn with_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.format = format;
        self
    }
}

/// Trait that a frame source must implement to be rendered by
/// [`FrameWidget`].
pub trait FrameSource: Clone + 'static {
    /// Handle to the shared frame buffer.
    fn frame_slot(&self) -> Arc<Mutex<Option<Frame>>>;

    /// Slot the widget writes the current physical-pixel size into.
    fn size_request_slot(&self) -> SizeRequestSlot;

    /// Current cursor the source wants displayed.
    fn cursor(&self) -> mouse::Interaction;

    /// Called on every iced event. Returns `true` if consumed.
    fn handle_event(
        &self,
        event: &iced::Event,
        bounds: iced::Rectangle,
        cursor: mouse::Cursor,
        focused: bool,
    ) -> bool;
}
