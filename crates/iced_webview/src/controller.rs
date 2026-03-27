//! WebView lifecycle management: creation via thread-local staging,
//! positioning, navigation, JS evaluation, and focus control.

use std::{
    cell::RefCell,
    collections::HashMap,
    hash::{Hash, Hasher},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use iced::{Rectangle, Task, window};
use tracing::{error, info};
use wry::{
    Rect, WebViewBuilder,
    dpi::{LogicalPosition, LogicalSize},
};

use crate::ipc::{self, IpcReceiver};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static STAGED: RefCell<HashMap<u64, wry::WebView>> = RefCell::new(HashMap::new());
}

pub enum Content {
    Url(String),
    Html(String),
}

impl Default for Content {
    fn default() -> Self {
        Self::Url(String::new())
    }
}

pub struct WebViewConfig {
    content: Content,
    transparent: bool,
    devtools: bool,
    initialization_scripts: Vec<String>,
    customize: Option<Box<dyn FnOnce(&mut WebViewBuilder) + Send>>,
}

impl Default for WebViewConfig {
    fn default() -> Self {
        Self {
            content: Content::default(),
            transparent: false,
            devtools: false,
            initialization_scripts: Vec::new(),
            customize: None,
        }
    }
}

impl WebViewConfig {
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.content = Content::Url(url.into());
        self
    }

    pub fn html(mut self, html: impl Into<String>) -> Self {
        self.content = Content::Html(html.into());
        self
    }

    pub fn transparent(mut self, transparent: bool) -> Self {
        self.transparent = transparent;
        self
    }

    pub fn devtools(mut self, devtools: bool) -> Self {
        self.devtools = devtools;
        self
    }

    pub fn initialization_script(mut self, script: impl Into<String>) -> Self {
        self.initialization_scripts.push(script.into());
        self
    }

    pub fn customize(
        mut self,
        f: impl FnOnce(&mut WebViewBuilder) + Send + 'static,
    ) -> Self {
        self.customize = Some(Box::new(f));
        self
    }
}

struct SharedState {
    webview: Option<wry::WebView>,
    last_bounds: Option<Rectangle>,
}

#[derive(Clone)]
pub(crate) struct BoundsSender(Rc<RefCell<SharedState>>);

impl BoundsSender {
    pub(crate) fn apply(&self, bounds: Rectangle) {
        let mut state = self.0.borrow_mut();
        state.last_bounds = Some(bounds);
        if let Some(webview) = &state.webview {
            let rect = Rect {
                position: LogicalPosition::new(bounds.x as f64, bounds.y as f64).into(),
                size: LogicalSize::new(bounds.width as f64, bounds.height as f64).into(),
            };
            if let Err(e) = webview.set_bounds(rect) {
                error!("Failed to set WebView bounds: {e}");
            }
        }
    }

    pub(crate) fn refocus_parent(&self) {
        let state = self.0.borrow();
        if let Some(webview) = &state.webview {
            let _ = webview.focus_parent();
        }
    }
}

pub struct WebViewController {
    id: u64,
    shared: Rc<RefCell<SharedState>>,
    config: WebViewConfig,
    ipc_rx: Option<IpcReceiver>,
}

impl WebViewController {
    pub fn new(config: WebViewConfig) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            shared: Rc::new(RefCell::new(SharedState {
                webview: None,
                last_bounds: None,
            })),
            config,
            ipc_rx: None,
        }
    }

    pub(crate) fn bounds_sender(&self) -> BoundsSender {
        BoundsSender(Rc::clone(&self.shared))
    }

    pub fn create_task<M: Send + 'static>(
        &mut self,
        window_id: window::Id,
        on_result: fn(Result<(), String>) -> M,
    ) -> Task<M> {
        let id = self.id;
        let content = std::mem::take(&mut self.config.content);
        let transparent = self.config.transparent;
        let devtools = self.config.devtools;
        let scripts = std::mem::take(&mut self.config.initialization_scripts);
        let customize = self.config.customize.take();

        let (ipc_tx, ipc_rx) = ipc::ipc_channel();
        self.ipc_rx = Some(ipc_rx);

        window::run(window_id, move |window| {
            let result = build_webview(id, window, content, transparent, devtools, scripts, customize, ipc_tx);
            match &result {
                Ok(()) => info!("WebView created successfully"),
                Err(e) => error!("Failed to create WebView: {e}"),
            }
            result
        })
        .map(on_result)
    }

    /// Extract the webview from thread-local staging into the controller.
    /// Must be called from `update()` after `create_task` resolves with `Ok`.
    pub fn take_staged(&mut self) {
        let webview = STAGED.with(|cell| cell.borrow_mut().remove(&self.id));
        let mut state = self.shared.borrow_mut();
        state.webview = webview;

        if let (Some(webview), Some(bounds)) = (&state.webview, state.last_bounds) {
            let rect = Rect {
                position: LogicalPosition::new(bounds.x as f64, bounds.y as f64).into(),
                size: LogicalSize::new(bounds.width as f64, bounds.height as f64).into(),
            };
            if let Err(e) = webview.set_bounds(rect) {
                error!("Failed to set initial WebView bounds: {e}");
            }
        }
    }

    pub fn set_visible(&self, visible: bool) {
        let state = self.shared.borrow();
        if let Some(webview) = &state.webview
            && let Err(e) = webview.set_visible(visible)
        {
            error!("Failed to set WebView visibility: {e}");
        }
    }

    pub fn navigate(&self, url: &str) {
        let state = self.shared.borrow();
        if let Some(webview) = &state.webview
            && let Err(e) = webview.load_url(url)
        {
            error!("Failed to navigate WebView: {e}");
        }
    }

    pub fn evaluate_script(&self, js: &str) {
        let state = self.shared.borrow();
        if let Some(webview) = &state.webview
            && let Err(e) = webview.evaluate_script(js)
        {
            error!("Failed to evaluate script: {e}");
        }
    }

    /// Returns a subscription that yields [`IpcMessage`]s sent from the page
    /// via `window.ipc.postMessage()`.
    ///
    /// Call this from your app's `subscription()` and `.map()` the output to
    /// your message type. The subscription becomes active after
    /// [`create_task`](Self::create_task) resolves.
    pub fn ipc_subscription(&self) -> iced::Subscription<crate::ipc::IpcMessage> {
        let Some(ipc_rx) = &self.ipc_rx else {
            return iced::Subscription::none();
        };

        iced::Subscription::run_with(
            IpcSubData { rx: Arc::clone(ipc_rx) },
            build_ipc_stream,
        )
    }

    pub fn destroy(&mut self) {
        self.shared.borrow_mut().webview = None;
    }

    pub fn is_active(&self) -> bool {
        self.shared.borrow().webview.is_some()
    }
}

struct IpcSubData {
    rx: Arc<Mutex<Option<futures::channel::mpsc::UnboundedReceiver<crate::ipc::IpcMessage>>>>,
}

impl Hash for IpcSubData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.rx).hash(state);
    }
}

fn build_ipc_stream(
    data: &IpcSubData,
) -> futures::channel::mpsc::UnboundedReceiver<crate::ipc::IpcMessage> {
    data.rx.lock().unwrap().take().expect("ipc receiver already consumed")
}

fn build_webview(
    id: u64,
    window: &dyn iced::window::Window,
    content: Content,
    transparent: bool,
    devtools: bool,
    scripts: Vec<String>,
    customize: Option<Box<dyn FnOnce(&mut WebViewBuilder) + Send>>,
    ipc_tx: ipc::IpcSender,
) -> Result<(), String> {
    remove_clip_children(window);

    let window_handle = window
        .window_handle()
        .map_err(|e| format!("Failed to get window handle: {e}"))?;

    let mut builder = WebViewBuilder::new()
        .with_transparent(transparent)
        .with_devtools(devtools)
        .with_focused(false)
        .with_ipc_handler(move |request| {
            let _ = ipc_tx.unbounded_send(crate::ipc::IpcMessage {
                body: request.into_body(),
            });
        });

    builder = match content {
        Content::Html(html) => builder.with_html(html),
        Content::Url(url) => builder.with_url(url),
    };

    for script in &scripts {
        builder = builder.with_initialization_script(script);
    }

    if let Some(f) = customize {
        f(&mut builder);
    }

    let webview = builder
        .build_as_child(&window_handle)
        .map_err(|e| e.to_string())?;

    STAGED.with(|cell| {
        cell.borrow_mut().insert(id, webview);
    });

    Ok(())
}

#[cfg(windows)]
fn remove_clip_children(window: &dyn iced::window::Window) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_STYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_CLIPCHILDREN,
    };

    let Ok(handle) = window.window_handle() else {
        return;
    };
    let wry::raw_window_handle::RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return;
    };

    let hwnd = win32.hwnd.get() as *mut core::ffi::c_void;

    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        if style & WS_CLIPCHILDREN as isize != 0 {
            SetWindowLongPtrW(hwnd, GWL_STYLE, style & !(WS_CLIPCHILDREN as isize));
        }
    }
}

#[cfg(not(windows))]
fn remove_clip_children(_window: &dyn iced::window::Window) {}
