# iced_servo

[![crates.io](https://img.shields.io/crates/v/iced_servo.svg)](https://crates.io/crates/iced_servo) [![docs.rs](https://docs.rs/iced_servo/badge.svg)](https://docs.rs/iced_servo)

Embed a [Servo](https://servo.org) webview as a regular widget inside an [Iced](https://github.com/iced-rs/iced) application.

Servo renders into an offscreen `SoftwareRenderingContext`; the resulting frame is read back via `read_to_image` and drawn through the generic [`iced_frame`](../iced_frame) widget (a wgpu-textured quad) that sits in the widget tree like any other element. `ServoWebViewController` implements the `FrameSource` trait from `iced_frame`. Mouse, keyboard, touch, IME, and window-focus events flow the other way: iced's native event system delivers them to the widget, which translates and forwards them to Servo via `WebView::notify_input_event`. Cross-platform by construction — there are no platform-specific child windows, WndProcs, `NSView` subviews, or X11/Wayland subsurfaces to maintain.

## Requirements

- An iced application using the `wgpu` renderer (the default). The widget relies on `iced_wgpu::primitive::Renderer` to draw its textured quad, so `iced_tiny_skia`-only apps won't compile against this crate.
- **Only one `ServoRuntime` per process.** Servo's startup options live in a process-wide singleton; constructing a second runtime panics at `set_opts`. Build one runtime at app startup and pass it into every controller.

## Architecture

```text
ServoRuntime       — one per process: owns Servo + SoftwareRenderingContext
   │
   ├── ServoWebViewController (tab 1) — one WebView, frame slot, delegate
   ├── ServoWebViewController (tab 2) — …
   └── ServoWebViewController (tab 3) — …
```

Each controller owns one `servo::WebView`. All controllers share the same runtime's rendering context, so only the active tab should be `tick()`-pumped on a given frame; calling `activate()` / `deactivate()` when switching tabs shows/hides the underlying webview so Servo's paint goes to the right pixels.

When page content calls `window.open` or clicks a `target="_blank"` link, Servo fires `request_create_new`. The crate satisfies the request with a throwaway popup whose only job is to capture the popup's first navigation URL and hand it to the embedder via the `on_new_webview_requested` callback. The app decides what to do with it — the `basic` example opens a new browser tab.

## Usage

```rust
use iced_servo::{ServoRuntime, ServoWebViewController, WebViewConfig, frame};

// Once per app:
let runtime = ServoRuntime::new(dpi::PhysicalSize::new(1024, 768))?;

// Per tab / view:
let controller = ServoWebViewController::new(
    &runtime,
    WebViewConfig::default().url("https://servo.org"),
    1.0,
)?;
controller.activate();

controller.on_new_webview_requested(|url| {
    // Open a new tab, or whatever — the URL was captured from a
    // window.open / target="_blank" link.
});

// In view():
frame(&controller).width(Length::Fill).height(Length::Fill)

// In update(), drive the active tab's controller on every tick:
controller.tick();
```

See `examples/browser.rs` for a full tabbed browser with URL bar, back/forward/reload buttons, and per-tab session history. `examples/basic.rs` is the minimal "load a page in a window" version.

### Host ↔ page interaction

Servo 0.1 exposes asynchronous JavaScript evaluation through the WebView API; `iced_servo` surfaces it both as a callback and as an `iced::Task` so it composes naturally with `update()`:

```rust
use iced_servo::{JSValue, JavaScriptEvaluationError};

// Returns a Task<Result<JSValue, JavaScriptEvaluationError>>.
Message::ReadTitle => controller
    .evaluate_javascript_task("document.title")
    .map(Message::TitleLoaded),

Message::TitleLoaded(Ok(JSValue::String(title))) => {
    self.page_title = title;
    Task::none()
}
```

`JSValue` is a structured enum (`Number`, `String`, `Array`, `Object`, plus opaque DOM-handle variants), so nested data round-trips losslessly without a JSON intermediate. There is currently no Rust-side DOM AST — every host→page interaction is "compose a JS expression, eval, parse the returned `JSValue`". Use Servo's `UserContentManager` to inject document-start scripts if you need page→host messaging.

## Build prerequisites

Servo is a heavy dependency. The first build downloads hundreds of crates and several GB; expect 30+ minutes. Each platform needs Servo's system toolchain prerequisites — see the official setup guide:

**<https://book.servo.org/hacking/building-servo.html>**

Summary for each platform:

**Windows:**
- Visual Studio 2022 with Win10/11 SDK ≥ 10.0.19041, MSVC v143 build tools, C++ ATL
- LLVM/Clang **19+** on PATH with `LIBCLANG_PATH` pointing at the `bin` dir containing `libclang.dll`
- [MozillaBuild](https://ftp.mozilla.org/pub/mozilla/libraries/win32/MozillaBuildSetup-Latest.exe) (needed by `mozjs_sys` for SpiderMonkey)
- Python 3

**macOS:**
- Xcode Command Line Tools, Homebrew packages listed in the Servo book

**Linux:**
- See the Servo book for distro-specific package lists (build-essential, cmake, libssl-dev, etc.)

Optional: enable the `no-wgl` feature on Windows to use ANGLE (D3D11) instead of WGL — matches what servoshell ships with:

```toml
iced_servo = { version = "0.1", features = ["no-wgl"] }
```

## Examples

```sh
cargo run -p iced_servo --example basic
```

The smallest possible integration: one runtime, one controller, one frame widget. Loads `https://servo.org` and renders it. ~70 lines total.

```sh
cargo run -p iced_servo --example browser
```

A real tabbed browser built on the same crate: URL bar, back/forward/reload, close/new-tab, and `target="_blank"` links that open new tabs via `on_new_webview_requested`.

## Performance notes

`SoftwareRenderingContext` renders on Microsoft's WARP software rasterizer on Windows and Mesa/EGL elsewhere; pages are laid out by Servo but every frame goes through CPU rasterization and a GPU→CPU→GPU round-trip per paint. Heavy, JS-driven sites (Google Search, Gmail) are noticeably slower than a GPU-backed browser. The `OffscreenRenderingContext` path (when available in Servo's public API) is the future upgrade target, with zero pixels crossing the CPU.
