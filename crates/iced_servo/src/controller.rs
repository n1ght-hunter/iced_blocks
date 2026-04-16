//! Servo webview lifecycle, split in two:
//!
//! - [`ServoRuntime`] owns the process-global Servo engine and the one
//!   [`SoftwareRenderingContext`] that every webview renders into. Servo
//!   stores its startup options in a process-wide singleton, so there can
//!   only ever be **one** `Servo` in the process — a second
//!   `ServoBuilder::build()` panics with "Already initialized". Put the
//!   runtime at the top of your app and share it across every tab.
//! - [`ServoWebViewController`] is a per-webview handle: one page's
//!   navigation state, delegate bridge, frame slot, and `tick()` pump.
//!   Multiple controllers can live on one runtime — that's how the
//!   example implements browser tabs. Only the visible tab's controller
//!   needs `tick()` called each frame.
//!
//! `Servo` is `!Send + !Sync`; both the runtime and controllers live on
//! the iced main thread (which is where `App::new`, `update`, `view`,
//! and `Program::update` all run).

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use dpi::PhysicalSize;
use euclid::{Box2D, Point2D, Scale};
use futures::{
    channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded},
    channel::oneshot,
    stream::{BoxStream, StreamExt},
};
use iced::{Subscription, Task};
use servo::{
    EventLoopWaker, JSValue, JavaScriptEvaluationError, RenderingContext, Servo, ServoBuilder,
    SoftwareRenderingContext, WebView, WebViewBuilder,
};
use tracing::error;
use url::Url;

use iced_frame::{Frame, FrameSource, SizeRequestSlot};

use crate::delegate::{DelegateState, ServoBridge, WebViewBridge};

/// Handler fired when page content requests a new webview (e.g. via
/// `window.open` or a `target="_blank"` link). The URL is the target
/// the popup would have navigated to; the embedder decides what to do
/// with it (open a new tab, load in place, ignore, …).
type NewWebViewHandler = Rc<dyn Fn(Url)>;

/// How long a requested webview size must remain stable (no new
/// `request_size` call with a different value) before the controller's
/// `tick` actually applies it. Drag-resize fires resize requests every
/// frame; without this debounce, Servo gets a flood of resize events
/// and can never finish laying out a single frame at the target size.
const RESIZE_DEBOUNCE: Duration = Duration::from_millis(100);

/// What a webview should load on startup.
#[derive(Clone)]
pub enum Content {
    Url(String),
    Html(String),
}

impl Default for Content {
    fn default() -> Self {
        Self::Url("about:blank".into())
    }
}

/// Construction-time configuration for a Servo webview.
#[derive(Default)]
pub struct WebViewConfig {
    content: Content,
}

impl WebViewConfig {
    /// Load the given URL on startup.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.content = Content::Url(url.into());
        self
    }

    /// Load the given HTML string on startup. Convenience wrapper around a
    /// `data:` URL.
    pub fn html(mut self, html: impl Into<String>) -> Self {
        self.content = Content::Html(html.into());
        self
    }
}

/// Shared Servo engine + rendering context. Build one of these at app
/// startup and pass it into each [`ServoWebViewController::new`] call.
/// Cheap to clone (`Rc` inside) — passing one to multiple tabs does not
/// duplicate the underlying engine.
///
/// Servo stores its global options in a process-wide singleton, so you
/// must only construct one runtime per process. Building a second one
/// will panic at `set_opts`.
#[derive(Clone)]
pub struct ServoRuntime {
    inner: Rc<ServoRuntimeInner>,
}

struct ServoRuntimeInner {
    servo: Servo,
    rendering_context: Rc<SoftwareRenderingContext>,
    rendering_context_dyn: Rc<dyn RenderingContext>,
    wake_rx: RefCell<Option<UnboundedReceiver<()>>>,
}

impl ServoRuntime {
    /// Build the shared Servo engine backed by a software rendering
    /// context of the given size. Subsequent webview resizes (driven by
    /// the shader widget's layout) will push the rendering context to
    /// the right dimensions through `webview.resize()` — the initial
    /// size is only what the first frame gets laid out at.
    pub fn new(initial_size: PhysicalSize<u32>) -> Result<Self, String> {
        let w = initial_size.width.max(1);
        let h = initial_size.height.max(1);
        let initial_size = PhysicalSize::new(w, h);

        let rendering_context = SoftwareRenderingContext::new(initial_size)
            .map_err(|e| format!("SoftwareRenderingContext::new failed: {e:?}"))?;
        let rendering_context = Rc::new(rendering_context);
        rendering_context
            .make_current()
            .map_err(|e| format!("SoftwareRenderingContext::make_current failed: {e:?}"))?;

        let (wake_tx, wake_rx) = unbounded::<()>();
        let waker: Box<dyn EventLoopWaker> = Box::new(ChannelWaker { tx: wake_tx });

        let servo = ServoBuilder::default().event_loop_waker(waker).build();
        // Intentionally skip `servo.setup_logging()` — embedders that
        // have already installed a global `log` crate logger (via
        // `tracing-subscriber` or similar) would otherwise hit
        // `SetLoggerError`. Servo's log output still flows through any
        // existing bridge.
        servo.set_delegate(Rc::new(ServoBridge));

        let rendering_context_dyn: Rc<dyn RenderingContext> = Rc::clone(&rendering_context) as _;

        Ok(Self {
            inner: Rc::new(ServoRuntimeInner {
                servo,
                rendering_context,
                rendering_context_dyn,
                wake_rx: RefCell::new(Some(wake_rx)),
            }),
        })
    }

    fn servo(&self) -> &Servo {
        &self.inner.servo
    }

    fn rendering_context(&self) -> &Rc<SoftwareRenderingContext> {
        &self.inner.rendering_context
    }

    fn rendering_context_dyn(&self) -> Rc<dyn RenderingContext> {
        Rc::clone(&self.inner.rendering_context_dyn)
    }

    /// Iced subscription that drains the Servo wake channel. Mount this
    /// once per app (not once per tab) — all webviews share the same
    /// event loop waker.
    pub fn subscription(&self) -> Subscription<()> {
        let wake_rx = self.inner.wake_rx.borrow_mut().take();
        let data = WakeSubData {
            wake_rx: Arc::new(Mutex::new(wake_rx)),
        };
        iced::Subscription::run_with(data, build_wake_drain)
    }
}

/// Per-webview state. Fields that mutate (resize debounce, scale,
/// handler slot) use `Cell` / `RefCell` directly — the outer controller
/// itself is just an `Rc<Inner>`, not `Rc<RefCell<Inner>>`, so most
/// accesses are a single deref.
struct Inner {
    runtime: ServoRuntime,
    webview: WebView,
    delegate_state: Rc<DelegateState>,

    /// Physical-pixel size the shader widget most recently requested,
    /// plus the [`Instant`] at which it was set. `tick()` only applies
    /// the resize once the value has been stable for [`RESIZE_DEBOUNCE`],
    /// so a window-drag that fires hundreds of resize requests per
    /// second collapses to a single Servo `webview.resize` after the
    /// user releases.
    pending_resize: Cell<Option<(PhysicalSize<u32>, Instant)>>,

    /// HiDPI scale factor — the ratio between iced's logical pixels and
    /// device pixels. Updated by the widget each `Primitive::prepare`
    /// via the `Viewport::scale_factor()`.
    scale_factor: Cell<f32>,

    /// Handler invoked from `tick()` whenever Servo requests a new
    /// webview (e.g. a `window.open` / `target="_blank"` link).
    new_webview_handler: RefCell<Option<NewWebViewHandler>>,

    /// Shared slot written to by the shader widget's
    /// `Primitive::prepare` with the current physical-pixel size and
    /// scale factor. `tick()` drains it and forwards the size to Servo.
    /// `Arc<Mutex<...>>` because `Primitive` must be `Send + Sync`.
    size_request: SizeRequestSlot,
}

/// Embedder handle for a single Servo webview. Cheap to clone — it's
/// just an `Rc` internally. Each tab/view in an app owns one controller;
/// they all share the same [`ServoRuntime`].
#[derive(Clone)]
pub struct ServoWebViewController {
    inner: Rc<Inner>,
}

impl ServoWebViewController {
    /// Build a new webview on the given runtime. The initial HiDPI
    /// scale factor is fed straight to Servo; the physical size is
    /// taken from the runtime's rendering context and the shader
    /// widget's first `Primitive::prepare` will push any updated size
    /// a frame or two later.
    pub fn new(
        runtime: &ServoRuntime,
        config: WebViewConfig,
        scale_factor: f32,
    ) -> Result<Self, String> {
        let delegate_state = Rc::new(DelegateState {
            webview: RefCell::new(None),
            rendering_context: RefCell::new(Some(runtime.rendering_context_dyn())),
            pending_popup_webview: RefCell::new(None),
            pending_new_url: RefCell::new(None),
            needs_paint: Cell::new(false),
            current_cursor: Cell::new(servo::Cursor::Default),
            latest_frame: Arc::new(Mutex::new(None)),
            current_url: RefCell::new(None),
            current_title: RefCell::new(None),
        });

        let delegate = Rc::new(WebViewBridge {
            state: Rc::clone(&delegate_state),
        });

        let initial_url = match config.content {
            Content::Url(s) => Url::parse(&s).ok(),
            Content::Html(html) => Url::parse(&format!("data:text/html;charset=utf-8,{html}")).ok(),
        };

        let mut builder = WebViewBuilder::new(runtime.servo(), runtime.rendering_context_dyn())
            .delegate(delegate)
            .hidpi_scale_factor(Scale::new(scale_factor));
        if let Some(url) = initial_url {
            builder = builder.url(url);
        }

        let webview = builder.build();
        *delegate_state.webview.borrow_mut() = Some(webview.clone());

        Ok(Self {
            inner: Rc::new(Inner {
                runtime: runtime.clone(),
                webview,
                delegate_state,
                pending_resize: Cell::new(None),
                scale_factor: Cell::new(scale_factor),
                new_webview_handler: RefCell::new(None),
                size_request: SizeRequestSlot::new(),
            }),
        })
    }

    /// Evaluate a JavaScript snippet in the webview and deliver the
    /// result through a callback. Servo runs the script asynchronously
    /// (on the next `spin_event_loop` tick that pumps the script
    /// thread) and invokes `callback` on the main thread when the
    /// result is ready or an error occurs.
    ///
    /// Prefer [`evaluate_javascript_task`](Self::evaluate_javascript_task)
    /// for iced applications — it hands you back a `Task<Message>` you
    /// can return from `update()`, which is the ergonomic fit for the
    /// Elm-style model. Use this lower-level callback form when you
    /// need to drive side effects without going through the Message
    /// enum.
    pub fn evaluate_javascript(
        &self,
        script: impl ToString,
        callback: impl FnOnce(Result<JSValue, JavaScriptEvaluationError>) + 'static,
    ) {
        self.inner.webview.evaluate_javascript(script, callback);
    }

    /// Evaluate a JavaScript snippet and wrap the asynchronous result
    /// in an iced [`Task`]. Typical usage in an app's `update`:
    ///
    /// ```ignore
    /// Message::RunScript => controller
    ///     .evaluate_javascript_task("document.title")
    ///     .map(Message::ScriptResult),
    /// ```
    ///
    /// The task resolves once Servo has finished executing the script.
    /// If the oneshot channel bridging Servo's callback to the task is
    /// dropped (e.g. the controller is destroyed mid-eval), the task
    /// resolves to `Err(JavaScriptEvaluationError::InternalError)` — we
    /// reuse that variant rather than inventing a new one because an
    /// embedder drop genuinely looks like "Servo couldn't complete the
    /// evaluation" from the caller's point of view.
    pub fn evaluate_javascript_task(
        &self,
        script: impl ToString,
    ) -> Task<Result<JSValue, JavaScriptEvaluationError>> {
        let (tx, rx) = oneshot::channel();
        self.inner
            .webview
            .evaluate_javascript(script, move |result| {
                let _ = tx.send(result);
            });
        Task::future(async move {
            rx.await
                .unwrap_or(Err(JavaScriptEvaluationError::InternalError))
        })
    }

    /// Navigate the webview to a new URL.
    pub fn navigate(&self, url: &str) {
        let Ok(parsed) = Url::parse(url) else {
            error!("Servo navigate: url failed to parse: {url}");
            return;
        };
        self.inner.webview.load(parsed);
    }

    /// Go back one step in Servo's session history.
    pub fn go_back(&self) {
        let _ = self.inner.webview.go_back(1);
    }

    /// Go forward one step in Servo's session history.
    pub fn go_forward(&self) {
        let _ = self.inner.webview.go_forward(1);
    }

    /// Reload the current page.
    pub fn reload(&self) {
        self.inner.webview.reload();
    }

    /// Whether Servo's session history has any back entries.
    pub fn can_go_back(&self) -> bool {
        self.inner.webview.can_go_back()
    }

    /// Whether Servo's session history has any forward entries.
    pub fn can_go_forward(&self) -> bool {
        self.inner.webview.can_go_forward()
    }

    /// Mark this webview as the one that should paint into the shared
    /// rendering context. Call it on the active tab whenever the user
    /// switches tabs; it focuses + shows this webview, blurs/hides any
    /// other webview on the same runtime, and forces a repaint so the
    /// shader widget picks up a fresh frame on the very next tick.
    pub fn activate(&self) {
        self.inner.webview.show();
        self.inner.webview.focus();
        self.inner.delegate_state.needs_paint.set(true);
    }

    /// Hide + blur this webview so it stops painting into the shared
    /// rendering context while another tab is active.
    pub fn deactivate(&self) {
        self.inner.webview.blur();
        self.inner.webview.hide();
    }

    /// Register a handler to receive URLs from `window.open` /
    /// `target="_blank"` navigations. Servo fires a `request_create_new`
    /// for these; we capture the URL the popup would have loaded and
    /// hand it to the embedder here. A typical app opens a new tab with
    /// a fresh `ServoWebViewController` loading the given URL. Only one
    /// handler can be registered at a time — calling this replaces the
    /// previous handler.
    pub fn on_new_webview_requested(&self, handler: impl Fn(Url) + 'static) {
        *self.inner.new_webview_handler.borrow_mut() = Some(Rc::new(handler));
    }

    /// Current page title, as reported by the most recent delegate callback.
    pub fn title(&self) -> Option<String> {
        self.inner.delegate_state.current_title.borrow().clone()
    }

    /// Current page URL, as reported by the most recent delegate callback.
    pub fn url(&self) -> Option<String> {
        self.inner
            .delegate_state
            .current_url
            .borrow()
            .as_ref()
            .map(|u| u.to_string())
    }

    /// Latest Servo-reported cursor; the widget's `mouse_interaction` reads
    /// this to tell iced which OS cursor to show.
    pub fn current_cursor(&self) -> servo::Cursor {
        self.inner.delegate_state.current_cursor.get()
    }

    /// Current cached HiDPI scale factor.
    pub fn scale_factor(&self) -> f32 {
        self.inner.scale_factor.get()
    }

    /// Update the cached HiDPI scale factor (call from
    /// `window::Event::Rescaled`).
    pub fn set_scale_factor(&self, scale_factor: f32) {
        self.inner.scale_factor.set(scale_factor);
    }

    /// Request a resize in physical pixels. The controller's `tick` drains
    /// this and actually calls `webview.resize`, but only after the same
    /// size has been pending for at least [`RESIZE_DEBOUNCE`] — see the
    /// debouncing rationale on [`Inner::pending_resize`].
    pub fn request_size(&self, size: PhysicalSize<u32>) {
        // Reset the debounce timer iff the requested size actually
        // changed; otherwise leave the existing timestamp so a
        // steady-state size (Length::Fill firing draw() every frame at
        // the same size) gets applied promptly instead of being
        // indefinitely deferred.
        let prior = self.inner.pending_resize.get();
        match prior {
            Some((existing, _)) if existing == size => {}
            _ => self.inner.pending_resize.set(Some((size, Instant::now()))),
        }
    }

    /// Returns the controller's current webview handle so widget-side input
    /// translation can call `notify_input_event`.
    pub fn webview(&self) -> Option<WebView> {
        Some(self.inner.webview.clone())
    }

    /// Pump the Servo event loop and, if a new frame became ready
    /// during the spin, paint and read it out into the shared frame
    /// buffer. The runtime's `spin_event_loop` drives *every* webview
    /// on the runtime, so calling `tick()` on one controller moves all
    /// of them forward — but only the controller on which `tick()` is
    /// called writes pixels into its own frame slot.
    ///
    /// Call this on every `Tick` produced by the app's subscription.
    pub fn tick(&self) {
        let inner = &self.inner;

        // Fire the `on_new_webview_requested` handler if Servo's
        // `request_create_new` captured a popup URL since the last tick.
        // Clone the handler out of the `RefCell` before invoking it so
        // the embedder can call back into the controller (e.g. to
        // construct a new one) without re-entrancy trouble.
        if let Some(url) = inner.delegate_state.pending_new_url.borrow_mut().take()
            && let Some(handler) = inner.new_webview_handler.borrow().clone()
        {
            handler(url);
        }

        // Drain the most recent size request from the shader widget
        // and feed it to the debounced resize path.
        if let Some((size, scale)) = inner.size_request.size() {
            if (inner.scale_factor.get() - scale).abs() > f32::EPSILON {
                inner.scale_factor.set(scale);
                // Servo no-ops internally if unchanged, but we guard
                // the call anyway to swallow float-compare jitter.
                inner.webview.set_hidpi_scale_factor(Scale::new(scale));
            }
            match inner.pending_resize.get() {
                Some((existing, _)) if existing == size => {}
                _ => inner.pending_resize.set(Some((size, Instant::now()))),
            }
        }

        let rendering_context = inner.runtime.rendering_context();

        // Apply a pending resize *only* once the requested size has
        // been stable for `RESIZE_DEBOUNCE`. Drag-resize storms us with
        // a new size every frame; forwarding each one means Servo
        // never finishes laying out a single frame at the target size.
        if let Some((new_size, requested_at)) = inner.pending_resize.get() {
            let current = rendering_context.size();
            if new_size != current && requested_at.elapsed() >= RESIZE_DEBOUNCE {
                inner.pending_resize.set(None);
                // Only call `webview.resize` — Servo's `WebView::resize`
                // internally sends `resize_rendering_context` to its
                // paint thread, which resizes surfman itself. Calling
                // `rendering_context.resize` ourselves from the main
                // thread races with that and leaves WebRender in a bad
                // state (`notify_new_frame_ready` stops firing).
                inner.webview.resize(new_size);
            } else if new_size == current {
                inner.pending_resize.set(None);
            }
        }

        inner.runtime.servo().spin_event_loop();

        if inner.delegate_state.needs_paint.replace(false) {
            if let Err(e) = rendering_context.make_current() {
                error!("SoftwareRenderingContext::make_current failed: {e:?}");
                return;
            }
            inner.webview.paint();

            // Read BEFORE present. Surfman's `present` swaps buffers
            // with `PreserveBuffer::No`, leaving the new back buffer's
            // contents undefined; reading after present returns
            // garbage.
            let size = rendering_context.size();
            let rect = Box2D::new(
                Point2D::new(0, 0),
                Point2D::new(size.width as i32, size.height as i32),
            );
            if let Some(rgba) = rendering_context.read_to_image(rect) {
                let frame = Frame::new(rgba.into_raw(), size.width, size.height);
                *inner.delegate_state.latest_frame.lock().unwrap() = Some(frame);
            }

            rendering_context.present();
        }
    }
}

impl FrameSource for ServoWebViewController {
    fn frame_slot(&self) -> Arc<Mutex<Option<Frame>>> {
        Arc::clone(&self.inner.delegate_state.latest_frame)
    }

    fn size_request_slot(&self) -> SizeRequestSlot {
        self.inner.size_request.clone()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        crate::input::cursor_to_interaction(self.inner.delegate_state.current_cursor.get())
    }

    fn handle_event(
        &self,
        event: &iced::Event,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
        focused: bool,
    ) -> bool {
        crate::input::translate_event(event, bounds, cursor, focused, self)
    }
}

struct WakeSubData {
    wake_rx: Arc<Mutex<Option<UnboundedReceiver<()>>>>,
}

impl std::hash::Hash for WakeSubData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.wake_rx) as usize).hash(state);
    }
}

fn build_wake_drain(data: &WakeSubData) -> BoxStream<'static, ()> {
    let rx = data.wake_rx.lock().unwrap().take();
    match rx {
        Some(rx) => Box::pin(rx.map(|()| ())),
        None => Box::pin(futures::stream::pending()),
    }
}

/// Send + Sync wake bridge handed to Servo. Servo calls `wake()` from any
/// thread; we forward to an unbounded futures channel that the runtime's
/// subscription drains.
struct ChannelWaker {
    tx: UnboundedSender<()>,
}

impl EventLoopWaker for ChannelWaker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(ChannelWaker {
            tx: self.tx.clone(),
        })
    }

    fn wake(&self) {
        let _ = self.tx.unbounded_send(());
    }
}
