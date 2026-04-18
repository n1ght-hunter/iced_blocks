# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Widget layout now uses [`iced_frame`](../iced_frame). The native child window is positioned via `SizeRequestSlot::bounds()` which carries physical-pixel position and size. `WebViewController::apply_bounds()` reads it each tick to reposition the WRY child window.
- Added `WryFrameHandle` — a cloneable `FrameSource` handle for use with `FrameWidget`. Use `frame(&controller.frame_handle())` in `view()`.
- Removed the old `WebViewPlaceholder` widget and `BoundsSender`.

## [0.1.0](https://github.com/n1ght-hunter/iced_blocks/releases/tag/iced_wry-v0.1.0) - 2026-03-27

### Changed

- rename iced_webview to iced_wry ([#6](https://github.com/n1ght-hunter/iced_blocks/pull/6))
