# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial crate. Embeds a Servo 0.1 webview as a regular iced widget backed by `SoftwareRenderingContext` + a wgpu-textured quad.
- `ServoRuntime` — shared Servo engine + rendering context. Constructed once per process (Servo's startup options are a global singleton) and passed into every `ServoWebViewController`.
- `ServoWebViewController` — per-webview handle with `navigate / go_back / go_forward / reload / can_go_back / can_go_forward / reload / activate / deactivate / tick / subscription` plus `title / url / current_cursor / scale_factor` getters. History uses Servo's built-in session-history API — no custom stack.
- `frame(&controller)` — renders the Servo webview through the generic `iced_frame::FrameWidget` via the `FrameSource` trait.
- Full event coverage: mouse (move / buttons / wheel / extra back/forward), keyboard (via W3C `keyboard-types`), touch (multi-finger), IME composition (`Opened`/`Preedit`/`Commit`/`Closed`), and window `Focused`/`Unfocused`. Mouse Back/Forward buttons drive browser history directly.
- `on_new_webview_requested` callback — fires when page content calls `window.open` or clicks a `target="_blank"` link, handing the embedder the target URL so it can open a new tab / new window / etc. The library does not redirect popups itself.
- Host ↔ page JavaScript bridge: `ServoWebViewController::evaluate_javascript(script, callback)` for the direct passthrough form, and `evaluate_javascript_task(script) -> iced::Task<Result<JSValue, JavaScriptEvaluationError>>` for the iced-native form. The `Task` variant bridges Servo's async callback through a `futures::oneshot` so apps can chain results into their `update()` via `.map(Message::…)`. `JSValue` and `JavaScriptEvaluationError` re-exported at the crate root so embedders don't need a direct `servo` dep.
- Cursor feedback via `mouse_interaction` — all 35 Servo `Cursor` variants mapped to `iced::mouse::Interaction`.
- HiDPI: scale factor plumbed from `Viewport::scale_factor()` through `Primitive::prepare` into `WebView::set_hidpi_scale_factor`, so pages render crisp on high-DPI monitors.
- Resize debouncing (100 ms) so drag-resize storms collapse into a single `webview.resize` call.
- `ServoRuntime::set_preference` / `default_user_agent` — runtime preference control and platform-aware user agent string.
- `ServoWebViewController::load_status` / `status_text` — page load lifecycle and hover-link URL from Servo delegate callbacks. `LoadStatus` and `PrefValue` re-exported at the crate root.
- Optional `no-wgl` feature to forward to `servo/no-wgl` (ANGLE/D3D11 on Windows).
- `examples/basic.rs` — minimal single-webview example.
