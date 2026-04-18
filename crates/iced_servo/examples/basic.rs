//! Minimal iced_servo example: build a runtime, build a single
//! controller, drop the [`shader`](iced_servo::shader) widget into the
//! view, pump `tick()` on a heartbeat. No URL bar, no tabs, no
//! navigation buttons — see `examples/browser.rs` for the full
//! tabbed-browser version.

use iced::widget::container;
use iced::{Element, Length, Subscription, Task};
use iced_servo::{ServoRuntime, ServoWebViewController, WebViewConfig};

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

struct App {
    runtime: ServoRuntime,
    controller: ServoWebViewController,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let runtime = ServoRuntime::new(dpi::PhysicalSize::new(1024, 768))
            .expect("failed to build Servo runtime");
        let controller = ServoWebViewController::new(
            &runtime,
            WebViewConfig::default().url("https://servo.org"),
            1.0,
        )
        .expect("failed to build Servo controller");
        controller.activate();
        (
            Self {
                runtime,
                controller,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.controller.tick();
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        container(iced_servo::frame(&self.controller))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::Tick),
            self.runtime.subscription().map(|_| Message::Tick),
        ])
    }
}
