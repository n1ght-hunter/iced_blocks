# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Extracted `WebViewPlaceholder` into the new [`iced_native_surface`](../iced_native_surface) crate. `iced_wry::WebViewPlaceholder` is now a public type alias for `iced_native_surface::NativeSurfacePlaceholder`, so existing imports keep working.
- `BoundsSender` now implements `iced_native_surface::BoundsSink`.

## [0.1.0](https://github.com/n1ght-hunter/iced_blocks/releases/tag/iced_wry-v0.1.0) - 2026-03-27

### Changed

- rename iced_webview to iced_wry ([#6](https://github.com/n1ght-hunter/iced_blocks/pull/6))
