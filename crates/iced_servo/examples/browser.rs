//! Tabbed-browser iced_servo example. Each tab owns its own
//! [`ServoWebViewController`]; clicking a `target="_blank"` / `window.open`
//! link in one tab opens a new tab via the controller's
//! `on_new_webview_requested` callback. Back / Forward / Reload operate
//! on the active tab only.
//!
//! For the smallest possible "load a page in a window" usage, see
//! `examples/basic.rs` instead.

use std::cell::RefCell;
use std::rc::Rc;

use iced::{
    Element, Length, Subscription, Task,
    widget::{button, column, row, text, text_input},
};
use iced_servo::{ServoRuntime, ServoWebViewController, WebViewConfig};
use url::Url;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    iced::application(App::new, App::update, App::view)
        .title("iced_servo basic")
        .subscription(App::subscription)
        .run()
}

const HOME: &str = "https://servo.org";

struct Tab {
    id: usize,
    controller: ServoWebViewController,
    url_input: String,
    last_seen_url: Option<String>,
    /// URL captured by this tab's `on_new_webview_requested` handler.
    /// Drained on the next `Tick` and promoted into a brand new tab.
    pending_new_tab: Rc<RefCell<Option<Url>>>,
}

impl Tab {
    fn new(id: usize, runtime: &ServoRuntime, url: &str) -> Self {
        let controller =
            ServoWebViewController::new(runtime, WebViewConfig::default().url(url), 1.0)
                .expect("failed to build Servo controller");
        let pending_new_tab: Rc<RefCell<Option<Url>>> = Rc::new(RefCell::new(None));
        {
            let slot = Rc::clone(&pending_new_tab);
            controller.on_new_webview_requested(move |url| {
                *slot.borrow_mut() = Some(url);
            });
        }
        // New tabs start hidden. The caller (`App::update`) calls
        // `controller.activate()` when this tab becomes the active one.
        controller.deactivate();
        Self {
            id,
            controller,
            url_input: url.to_string(),
            last_seen_url: None,
            pending_new_tab,
        }
    }

    fn display_title(&self) -> String {
        self.controller
            .title()
            .filter(|t| !t.is_empty())
            .or_else(|| self.controller.url())
            .unwrap_or_else(|| "New tab".into())
    }
}

struct App {
    runtime: ServoRuntime,
    tabs: Vec<Tab>,
    active: usize,
    next_id: usize,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    SelectTab(usize),
    CloseTab(usize),
    NewTab,
    UrlChanged(usize, String),
    Go(usize),
    Back(usize),
    Forward(usize),
    Reload(usize),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let runtime = ServoRuntime::new(dpi::PhysicalSize::new(1024, 768))
            .expect("failed to build Servo runtime");
        let first = Tab::new(0, &runtime, HOME);
        first.controller.activate();
        (
            Self {
                runtime,
                tabs: vec![first],
                active: 0,
                next_id: 1,
            },
            Task::none(),
        )
    }

    fn set_active(&mut self, new_idx: usize) {
        if new_idx == self.active || new_idx >= self.tabs.len() {
            return;
        }
        self.tabs[self.active].controller.deactivate();
        self.active = new_idx;
        self.tabs[self.active].controller.activate();
    }

    fn tab_index(&self, id: usize) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                // Only pump the active tab. Background tabs share the
                // same rendering context — letting them paint here
                // would clobber the active tab's pixels.
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.controller.tick();

                    // Keep the URL bar in sync with the page's notion
                    // of the current URL, but only when it actually
                    // changes — otherwise typing into the bar fights
                    // the page's own URL every frame.
                    let current = tab.controller.url();
                    if current != tab.last_seen_url {
                        if let Some(url) = current.clone() {
                            tab.url_input = url;
                        }
                        tab.last_seen_url = current;
                    }
                }

                // Collect any popup URLs captured during this tick and
                // promote them into new tabs.
                let mut new_tab_urls = Vec::new();
                for tab in &self.tabs {
                    if let Some(url) = tab.pending_new_tab.borrow_mut().take() {
                        new_tab_urls.push(url);
                    }
                }
                for url in new_tab_urls {
                    let id = self.next_id;
                    self.next_id += 1;
                    let tab = Tab::new(id, &self.runtime, url.as_str());
                    self.tabs.push(tab);
                    let new_idx = self.tabs.len() - 1;
                    self.set_active(new_idx);
                }
                Task::none()
            }
            Message::SelectTab(id) => {
                if let Some(idx) = self.tab_index(id) {
                    self.set_active(idx);
                }
                Task::none()
            }
            Message::CloseTab(id) => {
                if let Some(idx) = self.tab_index(id) {
                    let was_active = idx == self.active;
                    self.tabs[idx].controller.deactivate();
                    self.tabs.remove(idx);
                    if self.tabs.is_empty() {
                        let id = self.next_id;
                        self.next_id += 1;
                        let tab = Tab::new(id, &self.runtime, HOME);
                        tab.controller.activate();
                        self.tabs.push(tab);
                        self.active = 0;
                    } else {
                        if idx < self.active || self.active >= self.tabs.len() {
                            self.active = self.active.saturating_sub(1);
                        }
                        if was_active {
                            self.tabs[self.active].controller.activate();
                        }
                    }
                }
                Task::none()
            }
            Message::NewTab => {
                let id = self.next_id;
                self.next_id += 1;
                let tab = Tab::new(id, &self.runtime, HOME);
                self.tabs.push(tab);
                let new_idx = self.tabs.len() - 1;
                self.set_active(new_idx);
                Task::none()
            }
            Message::UrlChanged(id, value) => {
                if let Some(idx) = self.tab_index(id) {
                    self.tabs[idx].url_input = value;
                }
                Task::none()
            }
            Message::Go(id) => {
                if let Some(idx) = self.tab_index(id) {
                    let target = normalize_url(&self.tabs[idx].url_input);
                    self.tabs[idx].controller.navigate(&target);
                    self.tabs[idx].url_input = target;
                }
                Task::none()
            }
            Message::Back(id) => {
                if let Some(idx) = self.tab_index(id) {
                    self.tabs[idx].controller.go_back();
                }
                Task::none()
            }
            Message::Forward(id) => {
                if let Some(idx) = self.tab_index(id) {
                    self.tabs[idx].controller.go_forward();
                }
                Task::none()
            }
            Message::Reload(id) => {
                if let Some(idx) = self.tab_index(id) {
                    self.tabs[idx].controller.reload();
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // Tab strip across the top. Each tab is a button showing the
        // page title (or URL fallback) plus a small × to close it.
        let mut tab_strip = row![].spacing(4).padding(4);
        for (idx, tab) in self.tabs.iter().enumerate() {
            let label = tab.display_title();
            let truncated: String = label.chars().take(24).collect();
            let suffix = if label.chars().count() > 24 {
                "…"
            } else {
                ""
            };
            let active_marker = if idx == self.active { "▸ " } else { "" };
            let label_text = text(format!("{active_marker}{truncated}{suffix}")).size(12);
            let close = button(text("×").size(12))
                .on_press(Message::CloseTab(tab.id))
                .padding(2);
            let tab_row = row![
                button(label_text)
                    .on_press(Message::SelectTab(tab.id))
                    .padding(4),
                close,
            ]
            .spacing(2);
            tab_strip = tab_strip.push(tab_row);
        }
        tab_strip = tab_strip.push(button(text("+")).on_press(Message::NewTab).padding(4));

        let active_tab = &self.tabs[self.active];
        let id = active_tab.id;

        let back = {
            let mut b = button(text("←")).padding(6);
            if active_tab.controller.can_go_back() {
                b = b.on_press(Message::Back(id));
            }
            b
        };
        let forward = {
            let mut b = button(text("→")).padding(6);
            if active_tab.controller.can_go_forward() {
                b = b.on_press(Message::Forward(id));
            }
            b
        };
        let reload = button(text("⟳")).on_press(Message::Reload(id)).padding(6);

        let url_bar = row![
            back,
            forward,
            reload,
            text_input("Enter a URL…", &active_tab.url_input)
                .on_input(move |s| Message::UrlChanged(id, s))
                .on_submit(Message::Go(id))
                .padding(6),
            button(text("Go")).on_press(Message::Go(id)).padding(6),
        ]
        .spacing(6)
        .padding(6);

        let status = text(format!(
            "{}  ·  {}",
            active_tab.controller.title().unwrap_or_else(|| "—".into()),
            active_tab.controller.url().unwrap_or_else(|| "—".into()),
        ))
        .size(12);

        column![
            tab_strip,
            url_bar,
            iced_servo::frame(&active_tab.controller)
                .width(Length::Fill)
                .height(Length::Fill),
            status,
        ]
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        // One heartbeat is enough — on every tick we pump every
        // controller. Combine it with the runtime's wake-drain stream
        // so Servo's `EventLoopWaker` channel doesn't back up.
        Subscription::batch([
            iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::Tick),
            self.runtime.subscription().map(|_| Message::Tick),
        ])
    }
}

/// Accept bare hostnames and add `https://` when no scheme is present.
fn normalize_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return HOME.to_string();
    }
    if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}
