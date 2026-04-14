# iced_native_surface

A generic placeholder widget for embedding native OS child surfaces inside an [Iced](https://github.com/iced-rs/iced) layout.

The widget reserves layout space, reports its current bounds to a `BoundsSink` whenever they change, and asks the sink to refocus the parent window when the user clicks outside the reserved area. Backends — webview engines, video players, native map views, GL canvases — implement `BoundsSink` to reposition their underlying native surface.

Used by [`iced_wry`](../iced_wry).
