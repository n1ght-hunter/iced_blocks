# iced_blocks

Building blocks for creating applications in the [Iced](https://github.com/iced-rs/iced) framework.

A collection of reusable widgets and utilities extracted from personal projects.

## Crates

- [`iced_frame`](crates/iced_frame) Generic widget that renders an offscreen RGBA frame buffer as a wgpu texture
- [`iced_servo`](crates/iced_servo) Embeds a Servo webview inside an Iced application via offscreen rendering
- [`iced_wry`](crates/iced_wry) Embeds a WRY webview as a child window inside an Iced application
- [`iced_mc_skin`](crates/iced_mc_skin) Iced shader widget for rendering 3D Minecraft player skins
- [`iced_browser`](crates/iced_browser) Tabbed web browser built on `iced_servo` is currently just used to test iced_servo (not published)

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.

## License

Licensed under either of
- MIT License (LICENSE-MIT or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.