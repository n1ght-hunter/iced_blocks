# iced_frame

[![crates.io](https://img.shields.io/crates/v/iced_frame.svg)](https://crates.io/crates/iced_frame) [![docs.rs](https://docs.rs/iced_frame/badge.svg)](https://docs.rs/iced_frame)

[Iced](https://github.com/iced-rs/iced) widget that renders an offscreen RGBA frame buffer as a wgpu-textured quad.

Any source that produces frames — a webview engine, a video decoder, a software renderer — can plug into this widget by implementing the `FrameSource` trait. The widget handles the wgpu pipeline (texture create / resize / upload / draw), click-to-focus state, and redraw requests. The implementor supplies frames, cursor state, and event handling.

## Usage

```rust
use iced_frame::{Frame, FrameSource, FrameWidget, SizeRequestSlot, frame};
use iced_frame::{ContentFit, Alignment, FilterMode};

// Implement FrameSource for your renderer:
impl FrameSource for MyRenderer {
    fn frame_slot(&self) -> Arc<Mutex<Option<Frame>>> { ... }
    fn size_request_slot(&self) -> SizeRequestSlot { ... }
    fn cursor(&self) -> mouse::Interaction { ... }
    fn handle_event(&self, event, bounds, cursor, focused) -> bool { ... }
}

// In view():
frame(&my_renderer)
    .content_fit(ContentFit::Contain)
    .alignment(Alignment::Center)
    .filter(FilterMode::Linear)
    .width(Length::Fill)
    .height(Length::Fill)
```

## Content fit modes

| Mode | Behavior |
|------|----------|
| `Fill` (default) | Stretch to fill — ignores aspect ratio |
| `Contain` | Scale uniformly, entire frame visible (letterbox) |
| `Cover` | Scale uniformly, widget fully covered (crop) |
| `FitWidth` | Scale uniformly to match widget width; clip/letterbox height |
| `FitHeight` | Scale uniformly to match widget height; clip/letterbox width |
| `None` | No scaling — native pixel size, clip overflow |

## Alignment

9 anchor positions for when the frame doesn't fill the widget: `TopLeft`, `TopCenter`, `TopRight`, `CenterLeft`, `Center` (default), `CenterRight`, `BottomLeft`, `BottomCenter`, `BottomRight`.

## Filter mode

`Linear` (default, smooth) or `Nearest` (sharp pixel boundaries).

## Example

```sh
cargo run -p iced_frame --example demo
```

Interactive demo with pick lists for all fit/alignment/filter modes and a "Resize to fit" button that regenerates the checkerboard at the widget's physical pixel size.

## Requirements

Requires the iced `wgpu` renderer (the default). The widget uses `iced_wgpu::primitive::Renderer` to draw its textured quad.
