//! Embeds a [WRY](https://github.com/tauri-apps/wry) WebView as a child
//! window inside an Iced application. The [`FrameWidget`] reserves
//! layout space and reports bounds via [`iced_frame::SizeRequestSlot`]; the
//! controller reads those bounds each tick to reposition the native
//! child window.

mod controller;
mod ipc;

pub use controller::{Content, WebViewConfig, WebViewController, WryFrameHandle};
pub use iced_frame::{FrameSource, FrameWidget, frame};
pub use ipc::IpcMessage;
