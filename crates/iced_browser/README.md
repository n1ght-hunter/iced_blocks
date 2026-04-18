# iced_browser

A tabbed web browser built on [`iced_servo`](../iced_servo) and [`iced_frame`](../iced_frame). Not published — this is an example application showing how to build a browser with the workspace's crates.

## Features

- Tabbed browsing with per-tab session history
- URL bar with smart input: direct URLs, domain names, file paths, or search queries (DuckDuckGo)
- Back / forward / reload
- Dark theme
- Hover link URL overlay
- `target="_blank"` / `window.open` links open new tabs
- Firefox-compatible user agent

## Run

```sh
cargo run -p iced_browser -r
```
