mod tab;
mod theme;
mod url_bar;

use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Color, Element, Length, Subscription, Task};
use iced_servo::{LoadStatus, ServoRuntime};

use tab::Tab;
use theme::browser_theme;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    iced::application(App::new, App::update, App::view)
        .title("Browser")
        .theme(App::theme)
        .subscription(App::subscription)
        .run()
}

const HOME: &str = "https://www.google.com";

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
        let ua = runtime.default_user_agent().replace("Servo/", "Gecko/");
        runtime.set_preference("user_agent", iced_servo::PrefValue::Str(ua));
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
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.controller.tick();
                    tab.sync_url();
                }

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
                if let Some(idx) = self.tab_index(id)
                    && let Some(target) = url_bar::input_to_url(&self.tabs[idx].url_input)
                    && self.tabs[idx].controller.navigate(&target).is_ok()
                {
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
        let active_tab = &self.tabs[self.active];
        let id = active_tab.id;

        // Tab strip
        let mut tab_buttons: Vec<Element<'_, Message>> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(idx, tab)| {
                let label = tab.display_title();
                let truncated: String = label.chars().take(20).collect();
                let suffix = if label.chars().count() > 20 {
                    "…"
                } else {
                    ""
                };

                let is_active = idx == self.active;
                let bg = if is_active {
                    Color::from_rgb(0.20, 0.20, 0.25)
                } else {
                    Color::from_rgb(0.12, 0.12, 0.15)
                };

                let tab_label = container(
                    row![
                        button(text(format!("{truncated}{suffix}")).size(12))
                            .on_press(Message::SelectTab(tab.id))
                            .padding([4, 8])
                            .style(button::text),
                        button(text("×").size(12))
                            .on_press(Message::CloseTab(tab.id))
                            .padding([4, 6])
                            .style(button::text),
                    ]
                    .spacing(2),
                )
                .style(move |_theme: &_| container::Style {
                    background: Some(bg.into()),
                    border: iced::Border::default().rounded(4),
                    ..Default::default()
                })
                .padding(2);

                tab_label.into()
            })
            .collect();

        tab_buttons.push(
            button(text("+").size(14))
                .on_press(Message::NewTab)
                .padding([4, 10])
                .style(button::text)
                .into(),
        );

        let tab_strip = container(
            scrollable(row(tab_buttons).spacing(4).padding(4)).direction(
                scrollable::Direction::Horizontal(scrollable::Scrollbar::new()),
            ),
        )
        .style(|_theme: &_| container::Style {
            background: Some(Color::from_rgb(0.10, 0.10, 0.13).into()),
            ..Default::default()
        });

        // URL bar
        let back = {
            let mut b = button(text("←").size(14))
                .padding([4, 8])
                .style(button::text);
            if active_tab.controller.can_go_back() {
                b = b.on_press(Message::Back(id));
            }
            b
        };
        let forward = {
            let mut b = button(text("→").size(14))
                .padding([4, 8])
                .style(button::text);
            if active_tab.controller.can_go_forward() {
                b = b.on_press(Message::Forward(id));
            }
            b
        };
        let reload = button(text("⟳").size(14))
            .on_press(Message::Reload(id))
            .padding([4, 8])
            .style(button::text);

        let loading = active_tab.controller.load_status() == LoadStatus::Started;
        let loading_indicator = if loading {
            text("⏳").size(12)
        } else {
            text("").size(12)
        };

        let url_bar = container(
            row![
                back,
                forward,
                reload,
                loading_indicator,
                text_input("Search or enter URL…", &active_tab.url_input)
                    .on_input(move |s| Message::UrlChanged(id, s))
                    .on_submit(Message::Go(id))
                    .padding(6)
                    .size(13),
            ]
            .spacing(4)
            .padding(6)
            .align_y(iced::Alignment::Center),
        )
        .style(|_theme: &_| container::Style {
            background: Some(Color::from_rgb(0.14, 0.14, 0.17).into()),
            ..Default::default()
        });

        // Webview + optional hover link overlay
        let webview = iced_servo::frame(&active_tab.controller)
            .width(Length::Fill)
            .height(Length::Fill);

        let webview_area: Element<'_, Message> =
            if let Some(hover_url) = active_tab.controller.status_text() {
                // Stack the hover URL in the bottom-left corner over the webview
                iced::widget::stack![
                    webview,
                    container(container(text(hover_url).size(11)).padding([2, 8]).style(
                        |_theme: &_| container::Style {
                            background: Some(Color::from_rgba(0.10, 0.10, 0.13, 0.85).into(),),
                            border: iced::Border::default().rounded(3),
                            ..Default::default()
                        }
                    ),)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(iced::Alignment::Start)
                    .align_y(iced::Alignment::End)
                    .padding(4),
                ]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            } else {
                webview.into()
            };

        column![tab_strip, url_bar, webview_area].into()
    }

    fn theme(&self) -> iced::Theme {
        browser_theme()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::Tick),
            self.runtime.subscription().map(|_| Message::Tick),
        ])
    }
}
