use iced::{Element, Task, widget::column, window};
use iced_webview::{WebViewConfig, WebViewController, webview};

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();
    iced::application(App::new, App::update, App::view)
        .title("WebView Example")
        .run()
}

struct App {
    controller: WebViewController,
}

#[derive(Debug, Clone)]
enum Message {
    GotWindow(Option<window::Id>),
    WebViewReady(Result<(), String>),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let config = WebViewConfig::default()
            .url("https://iced.rs")
            .devtools(true);

        (
            Self {
                controller: WebViewController::new(config),
            },
            window::oldest().map(Message::GotWindow),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::GotWindow(Some(id)) => self
                .controller
                .create_task(id, Message::WebViewReady),
            Message::GotWindow(None) => Task::none(),
            Message::WebViewReady(Ok(())) => {
                self.controller.take_staged();
                Task::none()
            }
            Message::WebViewReady(Err(e)) => {
                eprintln!("WebView failed: {e}");
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        column![webview(&self.controller)].into()
    }
}
