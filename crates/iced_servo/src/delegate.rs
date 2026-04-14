//! `WebViewDelegate` and `ServoDelegate` implementations. The main
//! delegate writes all state changes into a shared `DelegateState`
//! owned by the controller. The application reads it back through
//! controller getters (`current_cursor`, `title`, `url`) and the shader
//! widget polls `latest_frame` + `current_cursor` directly.
//!
//! A second, much smaller delegate — [`PopupCaptureDelegate`] — is
//! attached to the throwaway webview that Servo forces us to create
//! when page content calls `window.open` or clicks a `target="_blank"`
//! link. Its only job is to intercept the popup's first navigation
//! attempt, capture the target URL, and hand it to the controller so
//! the embedder can open a new tab (or whatever it wants) with that
//! URL. See [`WebViewBridge::request_create_new`] for the full story.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Mutex},
};

use image::RgbaImage;
use servo::{
    Cursor, NavigationRequest, RenderingContext, ServoDelegate, ServoError, WebView,
    WebViewDelegate,
};
use tracing::{debug, error, warn};
use url::Url;

/// State shared between the delegate and the controller. Lives on the iced
/// main thread; the only cross-thread field is `latest_frame`, which is
/// handed to wgpu's `queue.write_texture` via a `Mutex`.
pub(crate) struct DelegateState {
    /// Backreference to the main webview. Populated by the controller
    /// right after `WebViewBuilder::build`.
    pub(crate) webview: RefCell<Option<WebView>>,

    /// Shared rendering context. The popup capture path needs this to
    /// satisfy `CreateNewWebViewRequest`, which forces us to build a
    /// new webview against a rendering context before it will hand us
    /// the popup's URL.
    pub(crate) rendering_context: RefCell<Option<Rc<dyn RenderingContext>>>,

    /// A `window.open` / `target="_blank"` popup that was built by
    /// `request_create_new` and is being kept alive just long enough
    /// for its own `request_navigation` to fire. The popup capture
    /// delegate clears this slot from inside that callback, dropping
    /// the popup handle at the end of the call.
    pub(crate) pending_popup_webview: RefCell<Option<WebView>>,

    /// URL captured from a popup's first navigation attempt. The
    /// controller's `tick()` drains this and fires the embedder's
    /// `on_new_webview_requested` handler with it.
    pub(crate) pending_new_url: RefCell<Option<Url>>,

    /// Set by `notify_new_frame_ready`, cleared by the controller's `tick`
    /// after paint+present+read_to_image. Flag-based because painting
    /// inside the delegate callback (which runs inside `spin_event_loop`)
    /// doesn't reach the presentation surface.
    pub(crate) needs_paint: Cell<bool>,

    /// The most recent cursor reported by Servo, read by the widget's
    /// `mouse_interaction` to pick an `iced::mouse::Interaction`.
    pub(crate) current_cursor: Cell<Cursor>,

    /// Latest rendered frame pixels. Written by the controller's `tick`
    /// after `read_to_image`, consumed by `Primitive::prepare`.
    pub(crate) latest_frame: Arc<Mutex<Option<RgbaImage>>>,

    /// Current page URL and title, from the respective delegate callbacks.
    /// The application polls these through `ServoWebViewController::{url,
    /// title}`.
    pub(crate) current_url: RefCell<Option<Url>>,
    pub(crate) current_title: RefCell<Option<String>>,
}

pub(crate) struct WebViewBridge {
    pub(crate) state: Rc<DelegateState>,
}

impl WebViewDelegate for WebViewBridge {
    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.state.needs_paint.set(true);
    }

    fn notify_url_changed(&self, _webview: WebView, url: Url) {
        *self.state.current_url.borrow_mut() = Some(url);
    }

    fn notify_page_title_changed(&self, _webview: WebView, title: Option<String>) {
        *self.state.current_title.borrow_mut() = title;
    }

    fn notify_cursor_changed(&self, _webview: WebView, cursor: Cursor) {
        self.state.current_cursor.set(cursor);
    }

    fn notify_crashed(&self, _webview: WebView, reason: String, backtrace: Option<String>) {
        if let Some(bt) = backtrace {
            error!("Servo webview crashed: {reason}\n{bt}");
        } else {
            error!("Servo webview crashed: {reason}");
        }
    }

    fn notify_closed(&self, _webview: WebView) {
        debug!("Servo webview closed");
    }

    fn request_navigation(&self, _webview: WebView, navigation_request: NavigationRequest) {
        navigation_request.allow();
    }

    /// Page content called `window.open` or clicked a `target="_blank"`
    /// link. Servo's API forces us to satisfy the request by actually
    /// building a new webview — we can't just look up the URL and move
    /// on. The cleanest workaround is to build a throwaway popup with
    /// a [`PopupCaptureDelegate`] that intercepts the popup's first
    /// `request_navigation`, grabs the URL, denies the navigation, and
    /// stashes the URL in `DelegateState::pending_new_url`. The
    /// controller's `tick()` then fires the embedder's
    /// `on_new_webview_requested` handler with that URL — the app
    /// decides (open a new tab, load in place, etc.) and the throwaway
    /// popup is dropped.
    fn request_create_new(
        &self,
        _parent_webview: WebView,
        request: servo::CreateNewWebViewRequest,
    ) {
        let Some(rc) = self.state.rendering_context.borrow().clone() else {
            warn!("request_create_new before rendering_context was installed");
            return;
        };
        let popup = request
            .builder(rc)
            .delegate(Rc::new(PopupCaptureDelegate {
                state: Rc::clone(&self.state),
            }))
            .build();
        // Keep the popup alive until its `request_navigation` fires;
        // dropping it right now would lose the URL we're trying to
        // capture. The capture delegate clears this slot on that
        // callback, at which point the popup drops cleanly.
        *self.state.pending_popup_webview.borrow_mut() = Some(popup);
    }
}

/// Tiny delegate attached to the throwaway popup webview built in
/// response to `window.open` / `target="_blank"`. Its one job: grab the
/// URL the popup was going to navigate to, deny the navigation so the
/// popup never actually loads anything, and drop the popup. The rest
/// of the delegate methods use the trait defaults.
pub(crate) struct PopupCaptureDelegate {
    pub(crate) state: Rc<DelegateState>,
}

impl WebViewDelegate for PopupCaptureDelegate {
    fn request_navigation(&self, _webview: WebView, navigation_request: NavigationRequest) {
        let url = navigation_request.url.clone();
        navigation_request.deny();
        *self.state.pending_new_url.borrow_mut() = Some(url);
        // Drop the popup webview at the end of this callback. Servo
        // has finished talking to it by the time we return from
        // `request_navigation`, so releasing the handle here is safe.
        let _ = self.state.pending_popup_webview.borrow_mut().take();
    }
}

pub(crate) struct ServoBridge;

impl ServoDelegate for ServoBridge {
    fn notify_error(&self, error: ServoError) {
        warn!("Servo engine error: {error:?}");
    }
}
