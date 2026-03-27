//! Embeds a [WRY](https://github.com/tauri-apps/wry) WebView as a child
//! window inside an Iced application. The placeholder widget reserves layout
//! space and repositions the native webview directly via shared state.

mod controller;
mod ipc;
mod widget;

pub use controller::{Content, WebViewConfig, WebViewController};
pub use ipc::IpcMessage;
pub use widget::WebViewPlaceholder;

/// Create a webview placeholder widget bound to the given controller.
///
/// The widget reserves layout space, repositions the native webview on
/// resize, and returns focus to the parent window when the user clicks
/// outside the webview area.
pub fn webview<Message>(controller: &WebViewController) -> WebViewPlaceholder<Message> {
    WebViewPlaceholder::new().bounds_sender(controller.bounds_sender())
}
