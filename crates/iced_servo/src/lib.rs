//! Embeds a [Servo](https://servo.org) webview inside an
//! [Iced](https://github.com/iced-rs/iced) application.
//!
//! Servo renders into a `SoftwareRenderingContext`, the result is read
//! back via `read_to_image`, and a custom [`Widget`](iced::advanced::Widget)
//! uploads the pixels to a persistent `wgpu::Texture` and draws it as
//! a regular iced widget. Input flows the other way: iced's native
//! event loop delivers mouse, keyboard, touch, and IME events to the
//! widget, which translates and forwards them to Servo via
//! `WebView::notify_input_event`.

mod controller;
mod delegate;
mod input;
mod primitive;
mod widget;

pub use controller::{Content, ServoRuntime, ServoWebViewController, WebViewConfig};
pub use widget::{ServoWidget, ServoWidgetState, shader};

/// JavaScript value returned from [`ServoWebViewController::evaluate_javascript`]
/// and the [`Task`](iced::Task)-based variant. Covers primitives,
/// arrays, objects, and opaque DOM-handle tokens (`Element`, `Window`,
/// `ShadowRoot`, `Frame`). The DOM handles are WebDriver-style ID
/// strings and cannot currently be round-tripped back into a later
/// script — treat them as diagnostic.
pub use servo::JSValue;

/// Error variant returned when a JavaScript evaluation fails. Covers
/// both compile-time and runtime failures plus a few "the webview
/// wasn't ready" cases; the [`Task`](iced::Task) variant also uses
/// [`InternalError`](servo::JavaScriptEvaluationError::InternalError)
/// for the case where the controller is dropped mid-evaluation.
pub use servo::JavaScriptEvaluationError;
