# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial crate. Generic iced widget (`FrameWidget`) that renders any `FrameSource` as a wgpu-textured quad.
- `FrameSource` trait: `frame_slot`, `size_request_slot`, `cursor`, `handle_event`.
- `Frame` struct: raw pixel data with configurable `wgpu::TextureFormat` (defaults to `Rgba8Unorm`).
- `ContentFit` enum: `Fill`, `Contain`, `Cover`, `FitWidth`, `FitHeight`, `None` — CSS-like object-fit behavior.
- `Alignment` enum: 9 anchor positions (`TopLeft` through `BottomRight`) for when the frame doesn't fill the widget.
- `FilterMode` enum: `Linear` (smooth) or `Nearest` (sharp pixels) texture sampling.
- `SizeRequestSlot` newtype carrying `WidgetBounds` (physical-pixel position, size, and DPI scale) for the widget→source communication channel. Position is included so native-surface embedders (e.g. WRY) can reposition child windows.
- UV transform uniform with discard shader for proper clip/letterbox/crop rendering.
- `examples/demo.rs` — interactive demo with pick lists for all fit/alignment/filter modes and a "Resize to fit" button.
